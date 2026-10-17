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
