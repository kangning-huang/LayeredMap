//! Actions as jobs (WP4.3–WP4.6): a plan runs in the worker pool through a generation slot, its
//! output shows as a preview over the map, and Keep commits the layer (or the new fields), its
//! style and the result record in one undo step.

use std::sync::Arc;

use lm_core::document::{Classes, ColorBy, Op, Transaction};
use lm_core::model::LayerId;
use lm_core::recipe::ResultRecord;
use lm_jobs::actions::{self, ActionError, ActionOutput, ActionResult, Plan, PlanInput};
use lm_jobs::import::{ImportedLayer, LayerData};
use lm_jobs::jobs::{JobError, JobOutcome, PlanSlot, Pool};

use super::{MAP_ID_BASE, Workspace, map_layer};

/// The map id of an action's preview layer.
pub(super) const PREVIEW_MAP_ID: u32 = MAP_ID_BASE - 2;

/// The map ids of an Ask answer's overlays: the answer's objects from a layer that isn't on the
/// map (a built-in World layer, or a step's layer other than the one previewed), drawn
/// highlighted over everything, from `OVERLAY_MAP_ID` down.
const OVERLAY_MAP_ID: u32 = MAP_ID_BASE - 3;
const MAX_OVERLAYS: u32 = 10;
/// The map id of the lines from an object to the nearest one found (`nearest_distance`).
pub(super) const CONNECTOR_MAP_ID: u32 = OVERLAY_MAP_ID - MAX_OVERLAYS;
/// The map id of a ranking's top areas, drawn in place of the result layer when the ranking
/// covers more areas than an answer shows (the other areas aren't drawn).
pub(super) const RANKED_MAP_ID: u32 = CONNECTOR_MAP_ID - 1;

/// What a preview map id draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PreviewPart {
    /// The result layer (the area, the counted areas…), striped/outlined.
    Result,
    /// An answer's objects from a layer not on the map, highlighted.
    Overlay,
    /// Lines from objects to the nearest ones found.
    Connector,
    /// A ranking's top areas (the result layer cut to them): a light fill and outline.
    Ranked,
}

/// Which preview part a map id is, if it is one.
pub(super) fn preview_part(id: u32) -> Option<PreviewPart> {
    match id {
        PREVIEW_MAP_ID => Some(PreviewPart::Result),
        CONNECTOR_MAP_ID => Some(PreviewPart::Connector),
        RANKED_MAP_ID => Some(PreviewPart::Ranked),
        _ if (CONNECTOR_MAP_ID + 1..=OVERLAY_MAP_ID).contains(&id) => Some(PreviewPart::Overlay),
        _ => None,
    }
}

/// What the slot delivers.
pub type RunOutcome = Result<Arc<ActionResult>, ActionError>;

/// The preview on the map: the result's new layer, drawn above everything until kept or
/// discarded. Never saved.
pub(super) struct Preview {
    pub generation: u64,
    /// The map layers to hand the map, once: the result layer (`PREVIEW_MAP_ID`), then an Ask
    /// answer's overlays and connectors.
    pub pairs: Option<Vec<(lm_mapview::map::Layer, lm_mapview::map::Layer)>>,
}

/// The analysis side of the workspace: the pool (made on first use), the slot, the preview.
#[derive(Default)]
pub struct Analysis {
    pool: Option<Pool>,
    pub slot: PlanSlot<RunOutcome>,
    pub(super) preview: Option<Preview>,
    pub(super) preview_synced: Option<u64>,
    /// When the current run started, for the estimate log.
    pub started: Option<web_time::Instant>,
    pub(super) highlight: Option<(LayerId, Vec<usize>)>,
    /// Generations for Ask previews (counted apart from the slot's).
    pub(super) preview_generation: u64,
    /// While an Ask plan is previewed: the rows each layer it filters will show once kept
    /// (drawn in place of the layer's own filter), so the preview is what Keep gives.
    pub(super) preview_masks: Vec<(LayerId, Arc<Vec<bool>>)>,
    /// While an Ask plan is previewed: the answer's objects in layers on the map, highlighted.
    pub(super) answer_highlights: Vec<(LayerId, Vec<usize>)>,
    /// Objects shown with the answer as context (the city `nearest_distance` measured from),
    /// highlighted too.
    pub(super) context_highlights: Vec<(LayerId, Vec<usize>)>,
    /// The overlay map ids that are context, not answer (and the connectors).
    pub(super) context_overlays: Vec<u32>,
    /// Layers drawn faintly as context while the answer is empty (the layer the plan filtered).
    pub(super) preview_faint: Vec<LayerId>,
    /// The previewed plan's answer found nothing (the Ask box says so; nothing is highlighted).
    pub(super) answer_empty: bool,
}

/// What a preview puts on the map, for the evaluation harness ([`Workspace::preview_shown`]).
pub struct PreviewShown<'a> {
    /// The result layer drawn over the map (with its highlighted rows in `selection`).
    pub layer: Option<&'a lm_mapview::map::Layer>,
    /// Rows highlighted in layers on the map (the composer's, then an Ask answer's).
    pub highlights: Vec<&'a (LayerId, Vec<usize>)>,
    /// An answer's objects drawn as overlays (all highlighted).
    pub overlays: Vec<&'a lm_mapview::map::Layer>,
    /// What is shown with the answer as context: highlighted rows in layers on the map, and
    /// overlays (the object measured from, the connector lines).
    pub context: Vec<&'a (LayerId, Vec<usize>)>,
    pub context_overlays: Vec<&'a lm_mapview::map::Layer>,
    /// The answer found nothing.
    pub empty: bool,
    /// Layers drawn faintly as context.
    pub faint: &'a [LayerId],
}

/// The most rows an Ask preview highlights (and Keep selects); a larger answer stays a filter.
const MAX_HIGHLIGHT: usize = 20_000;

/// An Ask answer's frame counts as local, and the map flies to it, when it spans at most this
/// many degrees of longitude and latitude: a city's or a country's surroundings, not an ocean.
const LOCAL_SPAN_DEG: (f64, f64) = (60.0, 40.0);

/// How much room an answer's area gets around it: the frame is the area grown by this many times
/// its own size on each side, so the area sits in the middle third with what crosses it in view.
const AREA_ROOM: f64 = 1.0;

impl Workspace {
    /// The plan input for a document layer: its current data and the rows in scope.
    pub fn plan_input(&self, id: LayerId, rows: Option<Vec<usize>>) -> Option<PlanInput> {
        let layer = self.as_shown(self.doc.layer(id)?);
        let data = self.data(id)?.data.clone();
        let path = std::path::PathBuf::from(&layer.profile.source.path);
        let path = path.exists().then_some(path);
        Some(PlanInput { layer, data, rows, path })
    }

    /// Runs `plan` in the background, replacing any run in flight; the previous preview stays
    /// until the new result arrives. Returns the slot's generation.
    pub fn run_plan(&mut self, plan: Plan) -> u64 {
        let wake = self.wake.clone();
        let pool = self.analysis.pool.get_or_insert_with(|| Pool::new(wake));
        self.analysis.started = Some(web_time::Instant::now());
        self.analysis.slot.submit(pool, move |ctx| match actions::run(&plan, &ctx.control) {
            Ok(r) => Ok(Ok(Arc::new(r))),
            Err(ActionError::Cancelled) => Err(JobError::Cancelled),
            Err(e) => Ok(Err(e)),
        })
    }

    /// Cancels the run in flight (its result is never delivered) and drops the preview.
    pub fn cancel_plan(&mut self) {
        self.analysis.slot.cancel();
        self.analysis.started = None;
        self.set_preview(None, 0);
    }

    pub fn plan_running(&self) -> bool {
        self.analysis.slot.is_running()
    }

    pub fn plan_progress(&self) -> Option<lm_jobs::jobs::Progress> {
        self.analysis.slot.progress()
    }

    /// The current run's result, once, when it arrives (and a preview of its new layer).
    pub fn poll_plan(&mut self) -> Option<(u64, RunOutcome)> {
        let d = self.analysis.slot.poll()?;
        self.analysis.started = None;
        let out = match d.outcome {
            JobOutcome::Done(r) => r,
            JobOutcome::Failed(m) => Err(ActionError::Failed(m)),
            JobOutcome::Cancelled => return None,
        };
        match &out {
            Ok(r) => match &r.output {
                ActionOutput::NewLayer { layer, .. } => self.set_preview(Some(layer), d.generation),
                _ => self.set_preview(None, 0),
            },
            Err(_) => self.set_preview(None, 0),
        }
        Some((d.generation, out))
    }

    fn set_preview(&mut self, layer: Option<&ImportedLayer>, generation: u64) {
        self.set_preview_selected(layer, generation, Vec::new());
    }

    /// [`Self::set_preview`] with `selected` rows of the preview layer drawn highlighted.
    fn set_preview_selected(&mut self, layer: Option<&ImportedLayer>, generation: u64, selected: Vec<usize>) {
        self.set_preview_parts(layer, generation, selected, Vec::new());
    }

    /// The preview: the result layer (with `selected` rows highlighted) and `extra` map layers
    /// (overlays and connectors, already given their ids). None of either: no preview.
    fn set_preview_parts(
        &mut self,
        layer: Option<&ImportedLayer>,
        generation: u64,
        selected: Vec<usize>,
        extra: Vec<lm_mapview::map::Layer>,
    ) {
        let mut pairs = Vec::new();
        if let Some(l) = layer {
            let mut a = map_layer(PREVIEW_MAP_ID, l);
            a.selection = selected;
            pairs.push((a.clone(), a));
        }
        pairs.extend(extra.into_iter().map(|a| (a.clone(), a)));
        self.analysis.preview = (!pairs.is_empty()).then_some(Preview { generation, pairs: Some(pairs) });
    }

    /// The rows an Ask preview shows in a layer in place of its own filter's.
    pub(super) fn preview_mask(&self, id: LayerId) -> Option<Arc<Vec<bool>>> {
        self.analysis.preview_masks.iter().find(|(l, _)| *l == id).map(|(_, m)| m.clone())
    }

    pub fn has_preview(&self) -> bool {
        self.analysis.preview.is_some()
    }

    /// What the preview puts on the map, read by the evaluation harness
    /// (`tools/lm/llm_eval/v3`, `examples/ask_eval.rs`): the layer drawn over the map (until the
    /// map takes it) and the rows highlighted in a layer. Changes nothing.
    pub fn preview_shown(&self) -> PreviewShown<'_> {
        let pairs: Vec<&lm_mapview::map::Layer> =
            self.analysis.preview.as_ref().and_then(|p| p.pairs.as_ref()).into_iter().flatten().map(|(l, _)| l).collect();
        let ctx = &self.analysis.context_overlays;
        PreviewShown {
            layer: pairs.iter().copied().find(|l| l.id == PREVIEW_MAP_ID || l.id == RANKED_MAP_ID),
            highlights: self.analysis.highlight.iter().chain(self.analysis.answer_highlights.iter()).collect(),
            overlays: pairs.iter().copied().filter(|l| l.id != PREVIEW_MAP_ID && l.id != RANKED_MAP_ID && !ctx.contains(&l.id)).collect(),
            context: self.analysis.context_highlights.iter().collect(),
            context_overlays: pairs.into_iter().filter(|l| ctx.contains(&l.id)).collect(),
            empty: self.analysis.answer_empty,
            faint: &self.analysis.preview_faint,
        }
    }

    /// Whether the previewed Ask answer found nothing.
    pub fn answer_empty(&self) -> bool {
        self.analysis.answer_empty
    }

    /// Whether a layer is drawn faintly as context for an empty Ask answer.
    pub(super) fn preview_faint(&self, id: LayerId) -> bool {
        self.analysis.preview_faint.contains(&id)
    }

    /// Rows an Ask answer highlights in a layer on the map.
    pub(super) fn answer_rows(&self, id: LayerId) -> impl Iterator<Item = usize> + '_ {
        self.analysis
            .answer_highlights
            .iter()
            .chain(self.analysis.context_highlights.iter())
            .filter(move |(l, _)| *l == id)
            .flat_map(|(_, r)| r.iter().copied())
    }

    /// Keeps a result: its layer or fields, its style and its record, as one undo step labelled
    /// with the action. Returns the record's id.
    pub fn keep_result(&mut self, r: &ActionResult, about: Option<LayerId>) -> Option<u64> {
        let mut ops: Vec<Op> = Vec::new();
        let mut layer_id = None;
        match &r.output {
            ActionOutput::Card => {}
            ActionOutput::NewLayer { layer, color_by } => {
                let tx = self.doc.add_layer_tx(layer.profile.clone());
                let mut tx_ops = tx.ops;
                if let Some(Op::AddLayer { layer: l, .. }) = tx_ops.first_mut() {
                    l.display_name = layer.name.clone();
                    if let Some(field) = color_by {
                        l.style.color_by = Some(ColorBy {
                            field: field.clone(),
                            categories: false,
                            classes: Classes::Groups { count: 5, equal_steps: false },
                            palette: "viridis".into(),
                            per_km2: false,
                            fit_to_view: false,
                        });
                    }
                    layer_id = Some((l.id, l.profile.data_rev));
                }
                ops.extend(tx_ops);
            }
            ActionOutput::NewFields { layer, data, fields } => {
                let l = self.doc.layer(*layer)?.clone();
                self.next_rev = self.next_rev.max(l.profile.data_rev) + 1;
                let rev = self.next_rev;
                let mut profile = l.profile.clone();
                profile.data_rev = rev;
                let derived = lm_jobs::import::derived_layer(&l.display_name, "Result", (**data).clone());
                profile.fields.extend(derived.profile.fields.iter().filter(|f| fields.contains(&f.name)).cloned());
                let imported = ImportedLayer {
                    name: l.display_name.clone(),
                    profile: profile.clone(),
                    data: LayerData::Vector(data.clone()),
                    status: lm_core::model::ImportStatus::Complete,
                    raw: None,
                    style: None,
                };
                self.data.insert((*layer, rev), Arc::new(imported));
                ops.extend(self.doc.set_profile_tx(*layer, profile)?.ops);
                layer_id = Some((*layer, rev));
            }
        }
        let id = self.doc.next_result_id.max(1);
        let record = ResultRecord {
            id,
            layer: layer_id.map(|(l, _)| l),
            recipe: r.recipe.clone(),
            version: 1,
            history: Vec::new(),
            card: r.card.clone(),
            about,
        };
        ops.push(Op::AddResult { index: self.doc.results.len(), record: Box::new(record) });
        if !self.commit(Some(Transaction::new(r.label.clone(), ops))) {
            return None;
        }
        if let (ActionOutput::NewLayer { layer, .. }, Some((lid, rev))) = (&r.output, layer_id) {
            self.data.insert((lid, rev), Arc::new((**layer).clone()));
            self.select_layer(lid);
        }
        self.set_preview(None, 0);
        Some(id)
    }

    /// The kept result a layer came from, if any.
    pub fn result_for_layer(&self, id: LayerId) -> Option<&ResultRecord> {
        self.doc.results.iter().rev().find(|r| r.layer == Some(id))
    }

    /// Kept results about a layer that made no layer of their own (cards), newest first.
    pub fn cards_about(&self, id: LayerId) -> Vec<&ResultRecord> {
        self.doc.results.iter().rev().filter(|r| r.layer.is_none() && r.about == Some(id)).collect()
    }

    /// Rows an action asked to show on the map while its card is open.
    pub fn set_highlight(&mut self, h: Option<(LayerId, Vec<usize>)>) {
        self.analysis.highlight = h;
    }

    /// Takes the preview's map layers for the map (once), or says they should go (empty).
    pub(super) fn preview_change(&mut self) -> Option<Vec<(lm_mapview::map::Layer, lm_mapview::map::Layer)>> {
        let want = self.analysis.preview.as_ref().map(|p| p.generation);
        if want == self.analysis.preview_synced && self.analysis.preview.as_ref().is_none_or(|p| p.pairs.is_none()) {
            return None;
        }
        self.analysis.preview_synced = want;
        Some(self.analysis.preview.as_mut().and_then(|p| p.pairs.take()).unwrap_or_default())
    }
}

/// The Ask box (Phase 7, early test build): the map as plan layers, and keeping a plan.
impl Workspace {
    /// Every layer as the assistant's plan works on it (name, data, the rows its filter shows)
    /// and each layer's filter, in the document's order.
    pub fn ask_inputs(&self) -> (Vec<lm_jobs::plan_exec::PlanLayer>, Vec<Option<lm_core::filter::Filter>>) {
        let mut layers = Vec::new();
        let mut filters = Vec::new();
        for l in &self.doc.layers {
            let Some(d) = self.data(l.id) else { continue };
            let rows = match (&d.data, &l.filter) {
                (LayerData::Vector(ds), Some(f)) => lm_jobs::inspect::filter_mask(ds, Some(f))
                    .map(|m| m.iter().enumerate().filter(|(_, k)| **k).map(|(r, _)| r).collect::<Vec<_>>()),
                _ => None,
            };
            let path = std::path::PathBuf::from(&l.profile.source.path);
            layers.push(lm_jobs::plan_exec::PlanLayer {
                name: l.display_name.clone(),
                alias: None,
                layer: self.as_shown(l),
                data: d.data.clone(),
                rows,
                path: path.exists().then_some(path),
            });
            filters.push(l.filter.clone());
        }
        (layers, filters)
    }

    /// Shows a plan's preview as Keep would leave the map: its last new layer over the map
    /// (an area outlined), each layer it filters showing only the rows the filter keeps, and the
    /// answer: the final step's result set (what "what's inside" found, what `count_in` counted
    /// or `join_by_location` matched in a few areas, the nearest objects `nearest_distance`
    /// found and the objects measured from, what `keep_inside`/`remove_inside` kept, the rows a
    /// final filter leaves). Answer objects in a layer on the map are highlighted there; those in
    /// a layer that isn't (a built-in World layer) are drawn as a highlighted overlay. An answer
    /// of nothing highlights nothing, draws the filtered layer faintly and says so. When the
    /// answer is local the map frames it: the area searched with room around it, else the
    /// answer's objects.
    pub fn preview_plan(&mut self, p: Option<&lm_ask::PlanResult>) {
        self.analysis.highlight = None;
        self.analysis.answer_highlights.clear();
        self.analysis.context_highlights.clear();
        self.analysis.context_overlays.clear();
        self.analysis.preview_faint.clear();
        self.analysis.answer_empty = false;
        let Some(p) = p else {
            self.set_preview(None, 0);
            self.analysis.preview_masks.clear();
            self.answer_fit = None;
            return;
        };
        let last_new = p.layers.iter().rev().find(|l| l.alias.is_some());
        let last_imported = last_new.map(|l| ImportedLayer {
            name: l.name.clone(),
            profile: l.layer.profile.clone(),
            data: l.data.clone(),
            status: lm_core::model::ImportStatus::Complete,
            raw: None,
            style: None,
        });
        self.analysis.preview_generation += 1;
        let generation = (1 << 40) + self.analysis.preview_generation;
        // The plan's filters, as the layers will show them once kept.
        self.analysis.preview_masks = p
            .layers
            .iter()
            .filter(|l| l.alias.is_none() && self.doc.layer(l.layer.id).is_some())
            .filter(|l| p.outcomes.iter().any(|o| matches!(o, lm_jobs::plan_exec::StepOutcome::Filtered { layer, .. } if *layer == l.name)))
            .filter_map(|l| {
                let (LayerData::Vector(ds), Some(rows)) = (&l.data, &l.rows) else { return None };
                let mut mask = vec![false; ds.features.len()];
                for r in rows {
                    if let Some(m) = mask.get_mut(*r) {
                        *m = true;
                    }
                }
                Some((l.layer.id, Arc::new(mask)))
            })
            .collect();
        let found = plan_answer(p);
        let mut preview_rows = Vec::new();
        let mut extra = Vec::new();
        match &found {
            Found::Sets(sets, _) => {
                let mut overlay_id = OVERLAY_MAP_ID;
                // The first set is the answer; later ones (the objects measured from) are context.
                for (k, (l, rows)) in sets.iter().enumerate() {
                    let context = k > 0;
                    if l.alias.is_none() && self.doc.layer(l.layer.id).is_some() {
                        let to = if context { &mut self.analysis.context_highlights } else { &mut self.analysis.answer_highlights };
                        to.push((l.layer.id, rows.clone()));
                    } else if last_new.is_some_and(|n| n.alias.is_some() && n.alias == l.alias) {
                        preview_rows = rows.clone();
                    } else if overlay_id > CONNECTOR_MAP_ID
                        && let Some(sub) = subset_layer(l, rows)
                    {
                        let mut a = map_layer(overlay_id, &sub);
                        a.selection = (0..a.dataset.features.len()).collect();
                        extra.push(a);
                        if context {
                            self.analysis.context_overlays.push(overlay_id);
                        }
                        overlay_id -= 1;
                    }
                }
                let lines = p.outcomes.iter().rev().find_map(|o| match o {
                    lm_jobs::plan_exec::StepOutcome::NewFields { connectors, matched: Some(_), .. } if !connectors.is_empty() => {
                        Some(connectors)
                    }
                    _ => None,
                });
                if let Some(lines) = lines {
                    extra.push(connector_layer(lines));
                    self.analysis.context_overlays.push(CONNECTOR_MAP_ID);
                }
            }
            Found::Empty => {
                // Nothing found: nothing lit, the filtered layers faint behind the area searched.
                self.analysis.answer_empty = true;
                self.analysis.preview_faint = self.analysis.preview_masks.iter().map(|(id, _)| *id).collect();
            }
            Found::None => {}
        }
        // A ranking of more areas than an answer shows (every country by its cities): only the
        // top ones the card lists are drawn, lightly; framed when they fit, else the view stays.
        if let Some((l, top)) = ranked_top(p, last_new)
            && let Some(sub) = subset_layer(l, &top)
        {
            let mut a = map_layer(RANKED_MAP_ID, &sub);
            a.name = l.name.clone();
            extra.retain(|x| x.id != RANKED_MAP_ID);
            extra.push(a);
            self.set_preview_parts(None, generation, Vec::new(), extra);
            self.answer_fit = answer_frame(None, &[(l, top)]);
            return;
        }
        self.set_preview_parts(last_imported.as_ref(), generation, preview_rows, extra);
        self.answer_fit = match &found {
            Found::Sets(_, true) => None,
            Found::Sets(sets, false) => answer_frame(last_new, sets),
            Found::Empty => answer_frame(last_new, &[]),
            Found::None => None,
        };
    }

    /// Keeps a plan as one undo step labelled `label`: fields added to open layers, their new
    /// filters, the new layers (with their filters and colours), and a result record per card.
    pub fn keep_plan(&mut self, label: String, p: &lm_ask::PlanResult) -> bool {
        use lm_jobs::plan_exec::StepOutcome;
        let mut ops: Vec<Op> = Vec::new();
        // The final filter of each layer the plan filtered, by its name in the plan.
        let mut filters: Vec<(String, lm_core::filter::Filter)> = Vec::new();
        for o in &p.outcomes {
            if let StepOutcome::Filtered { layer, filter, .. } = o {
                filters.retain(|(l, _)| l != layer);
                filters.push((layer.clone(), filter.clone()));
            }
        }
        let filter_of = |name: &str| filters.iter().find(|(l, _)| l == name).map(|(_, f)| f.clone());
        let mut new_data: Vec<(usize, Arc<ImportedLayer>)> = Vec::new();
        let mut changed: Vec<(LayerId, u32, Arc<ImportedLayer>)> = Vec::new();
        // Plan layer name -> document layer id, for the records.
        let mut ids: Vec<(String, LayerId)> = Vec::new();
        for l in &p.layers {
            if l.alias.is_some() {
                continue;
            }
            let id = l.layer.id;
            let Some(doc_layer) = self.doc.layer(id).cloned() else { continue };
            ids.push((l.name.clone(), id));
            let same = match (self.data(id).map(|d| &d.data), &l.data) {
                (Some(LayerData::Vector(a)), LayerData::Vector(b)) => Arc::ptr_eq(a, b),
                _ => true,
            };
            if !same && let LayerData::Vector(ds) = &l.data {
                self.next_rev = self.next_rev.max(doc_layer.profile.data_rev) + 1;
                let rev = self.next_rev;
                let mut profile = doc_layer.profile.clone();
                profile.data_rev = rev;
                let derived = lm_jobs::import::derived_layer(&doc_layer.display_name, "Result", (**ds).clone());
                let have: Vec<String> = profile.fields.iter().map(|f| f.name.clone()).collect();
                profile.fields.extend(derived.profile.fields.iter().filter(|f| !have.contains(&f.name)).cloned());
                let imported = ImportedLayer {
                    name: doc_layer.display_name.clone(),
                    profile: profile.clone(),
                    data: l.data.clone(),
                    status: lm_core::model::ImportStatus::Complete,
                    raw: None,
                    style: None,
                };
                changed.push((id, rev, Arc::new(imported)));
                match self.doc.set_profile_tx(id, profile) {
                    Some(tx) => ops.extend(tx.ops),
                    None => return false,
                }
            }
            if let Some(f) = filter_of(&l.name)
                && doc_layer.filter.as_ref() != Some(&f)
            {
                ops.push(Op::SetFilter { id, from: doc_layer.filter.clone(), to: Some(f) });
            }
        }
        // New layers: added in plan order, each with its final data, filter and colour.
        for l in p.layers.iter().filter(|l| l.alias.is_some()) {
            let LayerData::Vector(ds) = &l.data else { continue };
            let mut profile = l.layer.profile.clone();
            let derived = lm_jobs::import::derived_layer(&l.name, "Result", (**ds).clone());
            let have: Vec<String> = profile.fields.iter().map(|f| f.name.clone()).collect();
            profile.fields.extend(derived.profile.fields.iter().filter(|f| !have.contains(&f.name)).cloned());
            let tx = self.doc.add_layer_tx(profile.clone());
            let mut tx_ops = tx.ops;
            if let Some(Op::AddLayer { layer: dl, .. }) = tx_ops.first_mut() {
                dl.display_name = l.name.clone();
                dl.filter = filter_of(&l.name).or_else(|| l.alias.as_deref().and_then(filter_of));
                let color = p.outcomes.iter().find_map(|o| match o {
                    StepOutcome::NewLayer { alias, color_by, .. } if Some(alias.as_str()) == l.alias.as_deref() => color_by.clone(),
                    _ => None,
                });
                if let Some(field) = color {
                    dl.style.color_by = Some(ColorBy {
                        field,
                        categories: false,
                        classes: Classes::Groups { count: 5, equal_steps: false },
                        palette: "viridis".into(),
                        per_km2: false,
                        fit_to_view: false,
                    });
                }
                ids.push((l.name.clone(), dl.id));
                if let Some(a) = &l.alias {
                    ids.push((a.clone(), dl.id));
                }
                let imported = ImportedLayer {
                    name: l.name.clone(),
                    profile,
                    data: l.data.clone(),
                    status: lm_core::model::ImportStatus::Complete,
                    raw: None,
                    style: None,
                };
                new_data.push((dl.id.0 as usize, Arc::new(imported)));
            }
            ops.extend(tx_ops);
        }
        // An answer in a layer that isn't on the map (a built-in World layer) is kept as a layer
        // of its own: the objects the preview drew, named after the step that found them.
        let mut answer_layers: Vec<(String, LayerId, usize)> = Vec::new();
        if let Found::Sets(sets, _) = plan_answer(p) {
            // The answer's own set only; context (the object measured from) isn't kept.
            for (l, rows) in sets.iter().take(1) {
                if l.alias.is_some() || self.doc.layer(l.layer.id).is_some() {
                    continue;
                }
                let Some(sub) = subset_layer(l, rows) else { continue };
                let name = answer_title(p, &l.name).unwrap_or_else(|| sub.name.clone());
                let tx = self.doc.add_layer_tx(sub.profile.clone());
                let mut tx_ops = tx.ops;
                if let Some(Op::AddLayer { layer: dl, .. }) = tx_ops.first_mut() {
                    dl.display_name = name.clone();
                    answer_layers.push((l.name.clone(), dl.id, rows.len()));
                    new_data.push((dl.id.0 as usize, Arc::new(ImportedLayer { name, ..sub })));
                }
                ops.extend(tx_ops);
            }
        }
        let id_of = |name: &str| ids.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, id)| *id);
        let first_id = self.doc.next_result_id.max(1);
        let mut k = 0;
        for o in &p.outcomes {
            let (card, recipe, layer, about) = match o {
                StepOutcome::NewLayer { name, alias, card, recipe, .. } => (card, recipe, id_of(alias).or_else(|| id_of(name)), None),
                StepOutcome::NewFields { layer, card, recipe, .. } => (card, recipe, id_of(layer), None),
                StepOutcome::Card { card, recipe, highlight } => (card, recipe, None, highlight.as_ref().and_then(|(l, _)| id_of(l))),
                _ => continue,
            };
            let record = ResultRecord {
                id: first_id + k as u64,
                layer,
                recipe: (**recipe).clone(),
                version: 1,
                history: Vec::new(),
                card: (**card).clone(),
                about,
            };
            ops.push(Op::AddResult { index: self.doc.results.len() + k, record: Box::new(record) });
            k += 1;
        }
        if ops.is_empty() || !self.commit(Some(Transaction::new(label, ops))) {
            return false;
        }
        for (id, rev, d) in changed {
            self.data.insert((id, rev), d);
        }
        let mut last = None;
        for (raw, d) in new_data {
            let id = LayerId(raw as u64);
            let rev = self.doc.layer(id).map_or(0, |l| l.profile.data_rev);
            self.data.insert((id, rev), d);
            last = Some(id);
        }
        // What the preview highlighted stays highlighted, as the selection: the answer's
        // features in their (now filtered) layer, or every object of the layer kept from a
        // built-in layer's answer. Otherwise the newest layer is selected.
        let answer = match plan_answer(p) {
            Found::Sets(sets, false) => sets.iter().find_map(|(l, rows)| match answer_layers.iter().find(|(name, ..)| *name == l.name) {
                Some((_, id, n)) => Some((*id, (0..*n).collect::<Vec<usize>>())),
                None => id_of(l.alias.as_deref().unwrap_or(&l.name)).map(|id| (id, rows.clone())),
            }),
            _ => None,
        };
        match (answer.filter(|(_, rows)| !rows.is_empty()), last) {
            (Some((id, rows)), _) => {
                self.selection = lm_core::selection::SelectionContext {
                    active_layer: Some(id),
                    features: rows.into_iter().map(|row| lm_core::selection::FeatureRef { layer: id, row }).collect(),
                    ..Default::default()
                };
            }
            (None, Some(id)) => self.select_layer(id),
            (None, None) => {}
        }
        self.preview_plan(None);
        true
    }
}

/// A plan's answer, as the preview shows it.
enum Found<'a> {
    /// The answer's objects: (plan layer, rows), the answer's own set first. `filter`: they are
    /// just what a final filter leaves (lit as before, not framed, and Keep selects the layer).
    Sets(Vec<(&'a lm_jobs::plan_exec::PlanLayer, Vec<usize>)>, bool),
    /// The answer is that nothing matched (0 within 100 km of New York).
    Empty,
    /// No set of objects answers this plan (a ranking, a summary, a value).
    None,
}

/// The objects that answer a plan: the final step's result set (skipping display steps).
/// What a step found (`matched`, a card's highlight), what keep_inside/remove_inside kept, or the
/// rows a filter leaves. A later step that makes no such set (a ranking, a buffer) leaves the
/// answer to the steps before it. Sets larger than a highlight can show are left out.
fn plan_answer(p: &lm_ask::PlanResult) -> Found<'_> {
    use lm_jobs::plan_exec::StepOutcome;
    let layer = |name: &str| {
        let n = name.trim();
        p.layers.iter().rfind(|l| l.alias.as_deref() == Some(n)).or_else(|| p.layers.iter().rfind(|l| l.name.eq_ignore_ascii_case(n)))
    };
    let resolve = |sets: Vec<(String, Vec<usize>)>| -> Found<'_> {
        if sets.iter().all(|(_, rows)| rows.is_empty()) {
            return Found::Empty;
        }
        let found: Vec<_> = sets
            .into_iter()
            .filter(|(_, rows)| !rows.is_empty() && rows.len() <= MAX_HIGHLIGHT)
            .filter_map(|(name, rows)| layer(&name).map(|l| (l, rows)))
            .collect();
        if found.is_empty() { Found::None } else { Found::Sets(found, false) }
    };
    for o in p.outcomes.iter().rev() {
        match o {
            StepOutcome::Display => continue,
            StepOutcome::Asked(_) | StepOutcome::Refused(_) => return Found::None,
            StepOutcome::Filtered { layer: name, shown, .. } => {
                let Some(l) = layer(name) else { return Found::None };
                return match &l.rows {
                    Some(_) if *shown == 0 => Found::Empty,
                    Some(rows) if rows.len() <= MAX_HIGHLIGHT => Found::Sets(vec![(l, rows.clone())], true),
                    _ => Found::None,
                };
            }
            StepOutcome::NewLayer { alias, recipe, .. } if matches!(recipe.action.as_str(), "keep_inside" | "remove_inside") => {
                let Some(l) = layer(alias) else { return Found::None };
                let LayerData::Vector(ds) = &l.data else { return Found::None };
                return resolve(vec![(alias.clone(), (0..ds.features.len()).collect())]);
            }
            o => match o.matched() {
                Some(sets) => return resolve(sets),
                None => continue,
            },
        }
    }
    Found::None
}

/// A final ranking of the plan's last new layer when that layer has more objects than an answer
/// shows: the layer and the rows of the top ones the card lists (at most `ANSWER_AREAS`).
fn ranked_top<'a>(
    p: &'a lm_ask::PlanResult,
    last_new: Option<&'a lm_jobs::plan_exec::PlanLayer>,
) -> Option<(&'a lm_jobs::plan_exec::PlanLayer, Vec<usize>)> {
    use lm_jobs::plan_exec::StepOutcome;
    let l = last_new?;
    let LayerData::Vector(ds) = &l.data else { return None };
    if ds.features.len() <= lm_jobs::actions::ANSWER_AREAS {
        return None;
    }
    let StepOutcome::Card { card, recipe, .. } = p.outcomes.iter().rev().find(|o| !matches!(o, StepOutcome::Display))? else { return None };
    if recipe.action != "rank_by_value" || recipe.inputs.first().is_none_or(|i| i.name != l.name && i.name != l.layer.display_name) {
        return None;
    }
    let lm_core::recipe::Answer::Ranking { rows, .. } = &card.answer else { return None };
    let top: Vec<usize> = rows.iter().filter(|r| r.value.is_some()).take(lm_jobs::actions::ANSWER_AREAS).map(|r| r.row).collect();
    (!top.is_empty()).then_some((l, top))
}

/// The title of the step whose answer is in `layer` (for a kept built-in answer's layer name).
fn answer_title(p: &lm_ask::PlanResult, layer: &str) -> Option<String> {
    use lm_jobs::plan_exec::StepOutcome;
    p.outcomes.iter().rev().find_map(|o| match o {
        StepOutcome::Card { card, highlight: Some((l, _)), .. } if l == layer => Some(card.title.clone()),
        StepOutcome::NewLayer { card, .. } | StepOutcome::NewFields { card, .. }
            if o.matched().is_some_and(|sets| sets.first().is_some_and(|(l, _)| l == layer)) =>
        {
            Some(card.title.clone())
        }
        _ => None,
    })
}

/// A layer of `rows` of a plan layer (an answer from a layer that isn't on the map).
fn subset_layer(l: &lm_jobs::plan_exec::PlanLayer, rows: &[usize]) -> Option<ImportedLayer> {
    let LayerData::Vector(ds) = &l.data else { return None };
    let mut sub = lm_jobs::import::Dataset::new(l.name.clone(), ds.geometry_type, ds.fields.clone());
    for r in rows {
        sub.features.push(ds.features.get(*r)?.clone());
    }
    Some(lm_jobs::import::derived_layer(&l.name, "Result", sub))
}

/// The connector lines (`nearest_distance`) as a map layer.
fn connector_layer(lines: &[[(f64, f64); 2]]) -> lm_mapview::map::Layer {
    let mut ds = lm_jobs::import::Dataset::new("Nearest", lm_jobs::GeometryType::Polyline, Vec::new());
    for [a, b] in lines {
        ds.push_next(
            Some(lm_jobs::import::Geometry::Line(vec![vec![lm_jobs::import::Coord::new(a.0, a.1), lm_jobs::import::Coord::new(b.0, b.1)]])),
            Vec::new(),
        );
    }
    map_layer(CONNECTOR_MAP_ID, &lm_jobs::import::derived_layer("Nearest", "Result", ds))
}

/// Where the map goes for an answer: the plan's last new layer when it is an area of at most a
/// few objects (the area searched: a buffer, the counted area), with [`AREA_ROOM`] around it;
/// otherwise the answer's objects with a tenth of their size around them. None when that isn't
/// local ([`LOCAL_SPAN_DEG`]), or when there is no area and no object (an empty answer with
/// nothing searched).
fn answer_frame(
    last_new: Option<&lm_jobs::plan_exec::PlanLayer>,
    sets: &[(&lm_jobs::plan_exec::PlanLayer, Vec<usize>)],
) -> Option<lm_core::model::Extent> {
    let union = |ds: &lm_jobs::import::Dataset, rows: &mut dyn Iterator<Item = usize>| {
        rows.filter_map(|r| lm_jobs::inspect::row_extent(ds, r)).reduce(|a, e| lm_core::model::Extent {
            min_x: a.min_x.min(e.min_x),
            min_y: a.min_y.min(e.min_y),
            max_x: a.max_x.max(e.max_x),
            max_y: a.max_y.max(e.max_y),
        })
    };
    let area = last_new.filter(|l| l.layer.profile.kind == lm_core::model::LayerKind::Areas).and_then(|l| match &l.data {
        LayerData::Vector(ds) if ds.features.len() <= lm_jobs::actions::ANSWER_AREAS => union(ds, &mut (0..ds.features.len())),
        _ => None,
    });
    let (e, room) = match area {
        Some(e) => (e, AREA_ROOM),
        None => {
            let e = sets
                .iter()
                .filter_map(|(l, rows)| match &l.data {
                    LayerData::Vector(ds) => union(ds, &mut rows.iter().copied()),
                    LayerData::Raster(_) => None,
                })
                .reduce(|a, e| lm_core::model::Extent {
                    min_x: a.min_x.min(e.min_x),
                    min_y: a.min_y.min(e.min_y),
                    max_x: a.max_x.max(e.max_x),
                    max_y: a.max_y.max(e.max_y),
                })?;
            (e, 0.1)
        }
    };
    let (w, h) = (e.max_x - e.min_x, e.max_y - e.min_y);
    let pad = w.max(h) * room;
    let f = lm_core::model::Extent {
        min_x: e.min_x - pad,
        min_y: (e.min_y - pad).max(-85.0),
        max_x: e.max_x + pad,
        max_y: (e.max_y + pad).min(85.0),
    };
    (f.max_x - f.min_x <= LOCAL_SPAN_DEG.0 && f.max_y - f.min_y <= LOCAL_SPAN_DEG.1).then_some(f)
}

#[cfg(test)]
pub(crate) mod ask_preview_tests {
    use super::*;
    use lm_jobs::plan_exec::{Step, execute};

    /// fixtures/A1: five storm tracks around Tokyo, category 5 except row 2: rows 0 (through
    /// Tokyo) and 3 (about 50 km east) pass within 100 km; row 2 passes too but is category 3;
    /// rows 1 and 4 stay far. And the city of Tokyo.
    pub(crate) fn tokyo_layers() -> Vec<ImportedLayer> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/A1");
        let ctx = lm_jobs::import::Ctx::detached(Default::default());
        let mut out = Vec::new();
        for (f, name) in [("storms.geojson", "Storms"), ("cities.geojson", "Cities")] {
            let mut ls = lm_jobs::import::import_path(&dir.join(f), &ctx).unwrap();
            let mut l = ls.remove(0);
            l.name = name.into();
            out.push(l);
        }
        out
    }

    /// "How many Category 5 storms have passed within 100 km of Tokyo?" as the assistant plans it.
    pub(crate) fn tokyo_steps(last: &str) -> Vec<Step> {
        let last = match last {
            "whats_inside" => serde_json::json!({"name": "whats_inside", "args": {"area": {"layer": "#2"}, "layer": "Storms"}}),
            _ => serde_json::json!({"name": "count_in", "args": {"points": "Storms", "areas": "#2"}}),
        };
        serde_json::from_value(serde_json::json!([
            {"name": "filter", "args": {"layer": "Storms", "conditions": [{"field": "strength", "op": "is", "value": "Category 5"}]}},
            {"name": "buffer", "args": {"object": {"layer": "Cities", "field": "name", "value": "Tokyo"}, "distance": "100 km"}},
            last
        ]))
        .unwrap()
    }

    /// The plan run on the workspace's layers, as the Ask box gets it.
    pub(crate) fn plan_on(w: &Workspace, steps: Vec<Step>) -> lm_ask::PlanResult {
        let (mut layers, _) = w.ask_inputs();
        let outcomes = execute(&mut layers, &steps, 0, &lm_jobs::jobs::Control::none()).unwrap();
        lm_ask::PlanResult { steps, layers, outcomes, notes: Vec::new() }
    }

    fn workspace() -> (Workspace, LayerId) {
        let mut w = Workspace::default();
        assert!(w.add_imported(tokyo_layers(), 1));
        let storms = w.doc.layers.iter().find(|l| l.display_name == "Storms").map(|l| l.id).unwrap();
        (w, storms)
    }

    #[test]
    fn a_count_in_preview_highlights_the_counted_tracks_not_the_filter() {
        for last in ["count_in", "whats_inside"] {
            let (mut w, storms) = workspace();
            let p = plan_on(&w, tokyo_steps(last));
            w.preview_plan(Some(&p));
            assert_eq!(
                w.analysis.answer_highlights,
                vec![(storms, vec![0, 3])],
                "{last}: the 2 tracks that answer, not the 4 category-5 ones"
            );
            // The source layer shows what its filter will keep; the area is the preview layer.
            let mask = w.preview_mask(storms).unwrap();
            assert_eq!(mask.as_slice(), &[true, true, false, true, true]);
            let shown = w.preview_shown().layer.map(|a| a.dataset.geometry_type);
            assert_eq!(shown, Some(lm_jobs::GeometryType::Polygon), "{last}");
            // The view frames the 100 km area with room around it, not the whole Pacific.
            let e = w.answer_fit.unwrap();
            assert!(e.min_x < 138.5 && e.max_x > 140.9 && e.min_y < 34.8 && e.max_y > 36.6, "{e:?}");
            assert!(e.max_x - e.min_x < 8.0, "{e:?}");
        }
    }

    #[test]
    fn keep_keeps_what_the_preview_showed_and_discard_drops_it() {
        let (mut w, storms) = workspace();
        let p = plan_on(&w, tokyo_steps("count_in"));
        w.preview_plan(Some(&p));
        assert!(w.keep_plan("Ask: Category 5 near Tokyo".into(), &p));
        // The filter is applied, the area and the count are layers, and the 2 tracks stay
        // highlighted as the selection.
        let l = w.doc.layer(storms).unwrap();
        assert_eq!(l.filter.as_ref().map(|f| f.conditions.len()), Some(1));
        assert_eq!(w.doc.layers.len(), 4);
        assert_eq!(w.selection.active_layer, Some(storms));
        assert_eq!(w.selection.rows_in(storms), vec![0, 3]);
        assert!(w.analysis.answer_highlights.is_empty() && w.analysis.preview.is_none() && w.analysis.preview_masks.is_empty());
        assert_eq!(w.doc.results.len(), 2, "the buffer's and the count's records");
        // One undo step takes it all back.
        w.undo();
        assert_eq!(w.doc.layers.len(), 2);
        assert!(w.doc.layer(storms).unwrap().filter.is_none());
        // Discard: nothing changes and the preview goes.
        let (mut w, storms) = workspace();
        let p = plan_on(&w, tokyo_steps("whats_inside"));
        w.preview_plan(Some(&p));
        w.preview_plan(None);
        assert!(w.analysis.answer_highlights.is_empty() && w.analysis.preview.is_none() && w.preview_mask(storms).is_none());
        assert_eq!((w.doc.layers.len(), w.doc.results.len()), (2, 0));
    }

    #[test]
    fn a_plan_without_an_answer_set_still_highlights_its_filter() {
        let (mut w, storms) = workspace();
        let steps = tokyo_steps("count_in").into_iter().take(1).collect();
        let p = plan_on(&w, steps);
        w.preview_plan(Some(&p));
        assert_eq!(w.analysis.answer_highlights, vec![(storms, vec![0, 1, 3, 4])]);
        assert!(w.answer_fit.is_none(), "a plain filter isn't framed");
        assert!(w.keep_plan("Ask: Category 5".into(), &p));
        assert_eq!(w.selection.rows_in(storms), Vec::<usize>::new(), "Keep applies the filter; nothing more to select");
    }

    /// A map of the storms only, with fixtures/A1's cities as a built-in layer the plan can use
    /// ("World cities", not on the map), as the Ask box gives the plan its reference layers.
    fn storms_and_builtin_cities() -> (Workspace, LayerId, Vec<lm_jobs::plan_exec::PlanLayer>) {
        let mut w = Workspace::default();
        let mut ls = tokyo_layers();
        let cities = ls.remove(1);
        assert!(w.add_imported(ls, 1));
        let storms = w.doc.layers[0].id;
        let (mut layers, _) = w.ask_inputs();
        let style = lm_core::document::Style::for_profile(&cities.profile, [0, 0, 0, 255]);
        layers.push(lm_jobs::plan_exec::PlanLayer {
            name: "World cities".into(),
            alias: None,
            layer: lm_core::document::DocLayer::new(LayerId(9_000_001), "World cities".into(), cities.profile.clone(), style),
            data: cities.data.clone(),
            rows: None,
            path: None,
        });
        (w, storms, layers)
    }

    fn run(mut layers: Vec<lm_jobs::plan_exec::PlanLayer>, steps: serde_json::Value) -> lm_ask::PlanResult {
        let steps: Vec<Step> = serde_json::from_value(steps).unwrap();
        let outcomes = execute(&mut layers, &steps, 0, &lm_jobs::jobs::Control::none()).unwrap();
        lm_ask::PlanResult { steps, layers, outcomes, notes: Vec::new() }
    }

    /// Harness v3 bug 1 (w02, "Category 5 within 100 km of New York" = 0): an empty answer lights
    /// nothing (not the filter), says so, draws the filtered layer faintly and frames the area
    /// searched.
    #[test]
    fn w02_an_empty_answer_highlights_nothing_and_frames_the_area_searched() {
        let (mut w, storms) = workspace();
        let p = plan_on(
            &w,
            serde_json::from_value(serde_json::json!([
                {"name": "filter", "args": {"layer": "Storms", "conditions": [{"field": "strength", "op": "is", "value": "Category 5"}]}},
                {"name": "buffer", "args": {"object": {"layer": "Cities", "field": "name", "value": "Tokyo"}, "distance": "5 km"}},
                {"name": "count_here", "args": {"points": "Storms", "area": {"layer": "#2"}}}
            ]))
            .unwrap(),
        );
        w.preview_plan(Some(&p));
        let shown = w.preview_shown();
        assert!(shown.empty && shown.highlights.is_empty() && shown.overlays.is_empty());
        assert_eq!(shown.faint, &[storms][..], "the filtered storms stay as faint context");
        assert!(shown.layer.is_some_and(|l| l.name.starts_with("Within 5 km")), "the area searched is drawn");
        assert!(w.highlighted(storms).is_empty());
        let e = w.answer_fit.unwrap();
        assert!(e.min_x < 139.69 && e.max_x > 139.69 && e.max_x - e.min_x < 1.0, "{e:?}");
        // Count in it, the same.
        let p = plan_on(
            &w,
            serde_json::from_value(serde_json::json!([
                {"name": "filter", "args": {"layer": "Storms", "conditions": [{"field": "strength", "op": "is", "value": "Category 5"}]}},
                {"name": "buffer", "args": {"object": {"layer": "Cities", "field": "name", "value": "Tokyo"}, "distance": "5 km"}},
                {"name": "count_in", "args": {"points": "Storms", "areas": "#2"}}
            ]))
            .unwrap(),
        );
        w.preview_plan(Some(&p));
        assert!(w.answer_empty() && w.analysis.answer_highlights.is_empty());
        w.preview_plan(None);
        assert!(!w.answer_empty() && w.preview_shown().faint.is_empty());
    }

    /// Harness v3 bug 2 (x01, "Which cities are within 50 km of Vesuvius?"): the answer's objects
    /// in a built-in layer that isn't on the map are drawn as a highlighted overlay and framed;
    /// Keep adds them as a layer of their own and selects it.
    #[test]
    fn x01_an_answer_in_a_builtin_layer_is_drawn_framed_and_kept_as_a_layer() {
        let (mut w, _, layers) = storms_and_builtin_cities();
        let p = run(
            layers,
            serde_json::json!([
                {"name": "buffer", "args": {"object": {"layer": "Storms", "field": "name", "value": "Weak"}, "distance": "50 km"}},
                {"name": "whats_inside", "args": {"area": {"layer": "#1"}, "layer": "World cities"}}
            ]),
        );
        w.preview_plan(Some(&p));
        let shown = w.preview_shown();
        assert!(shown.highlights.is_empty(), "nothing is lit on a layer the map doesn't have");
        assert_eq!(shown.overlays.len(), 1);
        let o = shown.overlays[0];
        assert_eq!((o.dataset.features.len(), o.selection.clone()), (1, vec![0]), "Tokyo, drawn and highlighted");
        assert!(w.answer_fit.is_some());
        let before = w.doc.layers.len();
        assert!(w.keep_plan("Ask: cities near Weak".into(), &p));
        assert_eq!(w.doc.layers.len(), before + 2, "the buffer and the cities found");
        let kept = w.doc.layers.iter().find(|l| l.display_name == "World cities within 50 km of Weak").unwrap();
        assert_eq!(w.selection.active_layer, Some(kept.id));
        assert_eq!(w.selection.rows_in(kept.id), vec![0]);
    }

    /// Harness v3 bug 3 (n01, "Which volcano is closest to Tokyo?"): the nearest object found is
    /// highlighted, the object measured from (Tokyo, in a built-in layer) is drawn, and a line
    /// joins them.
    #[test]
    fn n01_nearest_highlights_the_nearest_and_the_source_with_a_connector() {
        let (mut w, storms, layers) = storms_and_builtin_cities();
        let p = run(
            layers,
            serde_json::json!([
                {"name": "filter", "args": {"layer": "World cities", "conditions": [{"field": "name", "op": "is", "value": "Tokyo"}]}},
                {"name": "nearest_distance", "args": {"layer": "World cities", "other": "Storms"}}
            ]),
        );
        w.preview_plan(Some(&p));
        let shown = w.preview_shown();
        assert_eq!(shown.highlights, vec![&(storms, vec![2])], "Weak, the nearest storm");
        assert!(shown.overlays.is_empty(), "the answer is on the map");
        let ids: Vec<u32> = shown.context_overlays.iter().map(|l| l.id).collect();
        assert_eq!(ids, vec![OVERLAY_MAP_ID, CONNECTOR_MAP_ID], "Tokyo drawn as context, and the line to Weak");
        assert_eq!(shown.context_overlays[0].selection, vec![0]);
        assert_eq!(shown.context_overlays[1].dataset.features.len(), 1);
        assert!(w.answer_fit.is_some());
    }

    /// Harness v3 bug 5 (o01, "Magnitude 8+ quakes at sea"): after a filter, remove_inside's
    /// survivors are the answer: the preview highlights them, not the filter's rows.
    #[test]
    fn o01_a_filter_then_remove_inside_highlights_the_survivors() {
        let (mut w, storms) = workspace();
        for action in ["remove_inside", "keep_inside"] {
            let p = plan_on(
                &w,
                serde_json::from_value(serde_json::json!([
                    {"name": "buffer", "args": {"object": {"layer": "Cities", "field": "name", "value": "Tokyo"}, "distance": "100 km"}},
                    {"name": "filter", "args": {"layer": "Storms", "conditions": [{"field": "strength", "op": "is", "value": "Category 5"}]}},
                    {"name": action, "args": {"layer": "Storms", "areas": "#1"}}
                ]))
                .unwrap(),
            );
            w.preview_plan(Some(&p));
            let shown = w.preview_shown();
            assert!(shown.highlights.is_empty(), "{action}: not the filter's 4 storms");
            let l = shown.layer.unwrap();
            assert!(!l.dataset.features.is_empty());
            assert_eq!(l.selection, (0..l.dataset.features.len()).collect::<Vec<_>>(), "{action}: every survivor lit");
            assert!(w.highlighted(storms).is_empty());
        }
    }

    /// The release screenshot pro-ask ("Which country has the most cities?": count_in per area,
    /// then rank_by_value), on fixtures V1 + T1 (schools in 17 districts): only the top 5 areas
    /// the card lists are drawn, lightly and without the thick highlight, and framed; the counts
    /// step gets no card of its own, while a lone final count_in keeps its card.
    #[test]
    fn pro_ask_which_country_has_the_most_cities_draws_the_top_5_only() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let ctx = lm_jobs::import::Ctx::detached(Default::default());
        let mut w = Workspace::default();
        for (f, name) in [("V1/districts.geojson", "Districts"), ("T1/schools.csv", "Schools")] {
            let mut l = lm_jobs::import::import_path(&dir.join(f), &ctx).unwrap().remove(0);
            l.name = name.into();
            assert!(w.add_imported(vec![l], 1));
        }
        let ranked = plan_on(
            &w,
            serde_json::from_value(serde_json::json!([
                {"name": "count_in", "args": {"points": "Schools", "areas": "Districts"}},
                {"name": "rank_by_value", "args": {"layer": "#1", "field": "schools"}}
            ]))
            .unwrap(),
        );
        w.preview_plan(Some(&ranked));
        let shown = w.preview_shown();
        let l = shown.layer.unwrap();
        assert_eq!(l.id, RANKED_MAP_ID);
        assert_eq!(l.dataset.features.len(), 5, "the top 5 of 17 districts");
        assert!(l.selection.is_empty(), "a light fill and outline, not the thick highlight");
        assert!(shown.highlights.is_empty());
        assert!(w.answer_fit.is_some(), "Shanghai's top 5 fit, so they are framed");
        let cards = crate::ask::answer_cards(&ranked);
        assert_eq!(cards.len(), 1, "only the ranking: {:?}", cards.iter().map(|c| &c.0.title).collect::<Vec<_>>());
        assert!(cards[0].0.title.contains("ranked"));
        // A lone final count_in keeps its card and draws its areas as before.
        let counted = plan_on(
            &w,
            serde_json::from_value(serde_json::json!([{"name": "count_in", "args": {"points": "Schools", "areas": "Districts"}}])).unwrap(),
        );
        assert_eq!(crate::ask::answer_cards(&counted).len(), 1);
        w.preview_plan(Some(&counted));
        assert_eq!(w.preview_shown().layer.map(|l| (l.id, l.dataset.features.len())), Some((PREVIEW_MAP_ID, 17)));
        // Without the final ranking, the counts are drawn whole.
        let mut ranked = ranked;
        ranked.outcomes.truncate(1);
        w.preview_plan(Some(&ranked));
        assert_eq!(w.preview_shown().layer.map(|l| l.id), Some(PREVIEW_MAP_ID));
    }
}
