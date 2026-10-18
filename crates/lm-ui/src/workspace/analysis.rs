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
}

impl Workspace {
    /// The plan input for a document layer: its current data and the rows in scope.
    pub fn plan_input(&self, id: LayerId, rows: Option<Vec<usize>>) -> Option<PlanInput> {
        let layer = self.doc.layer(id)?.clone();
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
        self.analysis.preview =
            layer.map(|l| Preview { generation, map_pair: Some((map_layer(PREVIEW_MAP_ID, l), map_layer(PREVIEW_MAP_ID, l))) });
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
                layer: l.clone(),
                data: d.data.clone(),
                rows,
                path: path.exists().then_some(path),
            });
            filters.push(l.filter.clone());
        }
        (layers, filters)
    }

    /// Shows a plan's preview: its last new layer over the map, and the rows its last filter on
    /// an open layer leaves (as a highlight).
    pub fn preview_plan(&mut self, p: Option<&lm_ask::PlanResult>) {
        let Some(p) = p else {
            self.set_preview(None, 0);
            self.analysis.highlight = None;
            return;
        };
        let last_new = p.layers.iter().rev().find(|l| l.alias.is_some()).map(|l| ImportedLayer {
            name: l.name.clone(),
            profile: l.layer.profile.clone(),
            data: l.data.clone(),
            status: lm_core::model::ImportStatus::Complete,
            raw: None,
        });
        self.analysis.preview_generation += 1;
        let generation = (1 << 40) + self.analysis.preview_generation;
        self.set_preview(last_new.as_ref(), generation);
        self.analysis.highlight = p
            .layers
            .iter()
            .rev()
            .filter(|l| l.alias.is_none())
            .find_map(|l| l.rows.as_ref().filter(|r| r.len() <= 20_000).map(|r| (l.layer.id, r.clone())));
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
        if let Some(id) = last {
            self.select_layer(id);
        }
        self.preview_plan(None);
        true
    }
}
