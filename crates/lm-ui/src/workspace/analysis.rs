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

/// What the slot delivers.
pub type RunOutcome = Result<Arc<ActionResult>, ActionError>;

/// The preview on the map: the result's new layer, drawn above everything until kept or
/// discarded. Never saved.
pub(super) struct Preview {
    pub generation: u64,
    pub map_pair: Option<(lm_mapview::map::Layer, lm_mapview::map::Layer)>,
}

/// The analysis side of the workspace: the pool (made on first use), the slot, the preview.
#[derive(Default)]
pub struct Analysis {
    pool: Option<Pool>,
    pub slot: PlanSlot<RunOutcome>,
    pub(super) preview: Option<Preview>,
    pub(super) preview_synced: Option<u64>,
    /// When the current run started, for the estimate log.
    pub started: Option<std::time::Instant>,
    pub(super) highlight: Option<(LayerId, Vec<usize>)>,
    /// Generations for Ask previews (counted apart from the slot's).
    pub(super) preview_generation: u64,
    /// While an Ask plan is previewed: the rows each layer it filters will show once kept
    /// (drawn in place of the layer's own filter), so the preview is what Keep gives.
    pub(super) preview_masks: Vec<(LayerId, Arc<Vec<bool>>)>,
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
        self.analysis.started = Some(std::time::Instant::now());
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
        self.analysis.preview = layer.map(|l| {
            let mut a = map_layer(PREVIEW_MAP_ID, l);
            a.selection = selected;
            let b = a.clone();
            Preview { generation, map_pair: Some((a, b)) }
        });
    }

    /// The rows an Ask preview shows in a layer in place of its own filter's.
    pub(super) fn preview_mask(&self, id: LayerId) -> Option<Arc<Vec<bool>>> {
        self.analysis.preview_masks.iter().find(|(l, _)| *l == id).map(|(_, m)| m.clone())
    }

    pub fn has_preview(&self) -> bool {
        self.analysis.preview.is_some()
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

    /// Takes the preview map layer for the map (once), or says it should go.
    pub(super) fn preview_change(&mut self) -> Option<Option<(lm_mapview::map::Layer, lm_mapview::map::Layer)>> {
        let want = self.analysis.preview.as_ref().map(|p| p.generation);
        if want == self.analysis.preview_synced && self.analysis.preview.as_ref().is_none_or(|p| p.map_pair.is_none()) {
            return None;
        }
        self.analysis.preview_synced = want;
        Some(self.analysis.preview.as_mut().and_then(|p| p.map_pair.take()))
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
    /// features that answer the question highlighted: what "what's inside" found, what
    /// `count_in` counted in a few areas, what `join_by_location` matched to them. A plan
    /// without such an answer highlights the rows its last filter leaves. When the answer is
    /// local the map frames it: the area with room around it, else the answer's features.
    pub fn preview_plan(&mut self, p: Option<&lm_ask::PlanResult>) {
        let Some(p) = p else {
            self.set_preview(None, 0);
            self.analysis.highlight = None;
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
        let answer = plan_answer(p);
        let mut preview_rows = Vec::new();
        self.analysis.highlight = None;
        match answer {
            Some((l, rows)) if l.alias.is_none() => self.analysis.highlight = Some((l.layer.id, rows.to_vec())),
            Some((l, rows)) if last_new.is_some_and(|n| n.alias == l.alias) => preview_rows = rows.to_vec(),
            Some(_) => {}
            None => {
                self.analysis.highlight = p
                    .layers
                    .iter()
                    .rev()
                    .filter(|l| l.alias.is_none())
                    .find_map(|l| l.rows.as_ref().filter(|r| r.len() <= MAX_HIGHLIGHT).map(|r| (l.layer.id, r.clone())));
            }
        }
        self.set_preview_selected(last_imported.as_ref(), generation, preview_rows);
        self.answer_fit = answer.and_then(|(l, rows)| answer_frame(last_new, l, rows));
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
        // features in their (now filtered) layer. Otherwise the newest layer is selected.
        let answer = plan_answer(p).and_then(|(l, rows)| id_of(l.alias.as_deref().unwrap_or(&l.name)).map(|id| (id, rows.to_vec())));
        match (answer, last) {
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

/// The features that answer a plan (the last step that found some), with the plan layer they
/// are rows of; None when there are none, or more than a highlight shows.
fn plan_answer(p: &lm_ask::PlanResult) -> Option<(&lm_jobs::plan_exec::PlanLayer, &[usize])> {
    let (name, rows) = p.outcomes.iter().rev().find_map(|o| o.matched())?;
    if rows.is_empty() || rows.len() > MAX_HIGHLIGHT {
        return None;
    }
    let n = name.trim();
    let l =
        p.layers.iter().rfind(|l| l.alias.as_deref() == Some(n)).or_else(|| p.layers.iter().rfind(|l| l.name.eq_ignore_ascii_case(n)))?;
    Some((l, rows.as_slice()))
}

/// Where the map goes for an answer: the plan's last new layer when it is an area of at most a
/// few objects (the buffer or the counted area), with [`AREA_ROOM`] around it; otherwise the
/// answer's features with a tenth of their size around them. None when that isn't local
/// ([`LOCAL_SPAN_DEG`]).
fn answer_frame(
    last_new: Option<&lm_jobs::plan_exec::PlanLayer>,
    answer: &lm_jobs::plan_exec::PlanLayer,
    rows: &[usize],
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
            let LayerData::Vector(ds) = &answer.data else { return None };
            (union(ds, &mut rows.iter().copied())?, 0.1)
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
            assert_eq!(w.analysis.highlight, Some((storms, vec![0, 3])), "{last}: the 2 tracks that answer, not the 4 category-5 ones");
            // The source layer shows what its filter will keep; the area is the preview layer.
            let mask = w.preview_mask(storms).unwrap();
            assert_eq!(mask.as_slice(), &[true, true, false, true, true]);
            let shown = w.analysis.preview.as_ref().and_then(|p| p.map_pair.as_ref()).map(|(a, _)| a.dataset.geometry_type);
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
        assert!(w.analysis.highlight.is_none() && w.analysis.preview.is_none() && w.analysis.preview_masks.is_empty());
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
        assert!(w.analysis.highlight.is_none() && w.analysis.preview.is_none() && w.preview_mask(storms).is_none());
        assert_eq!((w.doc.layers.len(), w.doc.results.len()), (2, 0));
    }

    #[test]
    fn a_plan_without_an_answer_set_still_highlights_its_filter() {
        let (mut w, storms) = workspace();
        let steps = tokyo_steps("count_in").into_iter().take(1).collect();
        let p = plan_on(&w, steps);
        w.preview_plan(Some(&p));
        assert_eq!(w.analysis.highlight, Some((storms, vec![0, 1, 3, 4])));
        assert!(w.answer_fit.is_none(), "no answer to frame");
        assert!(w.keep_plan("Ask: Category 5".into(), &p));
        assert_eq!(w.selection.rows_in(storms), Vec::<usize>::new());
    }
}
