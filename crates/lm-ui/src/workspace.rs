//! The workspace (Phase 2): the document and its undo stack, imports in progress, and keeping the
//! map's layers in step with the document. Every document change is one transaction (WP2.10).
//!
//! Layer data (features, pixels) lives here beside the document, keyed by layer and data
//! revision, so undoing a changed assumption brings back the matching data. Only the document is
//! saved; data is read again from the source files.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use lm_core::document::{Document, Op, Style, Transaction};
use lm_core::model::{AssumptionKind, Choice, Crs, Extent, ImportStatus, LayerId, LayerKind, Subject};
use lm_core::undo::UndoStack;
use lm_jobs::import::{ImportError, ImportEvent, ImportJob, ImportedLayer, LayerData, Stage};
use lm_mapview::map::Map;

/// A file being imported, or one that failed (its row stays until dismissed).
pub struct ImportRow {
    pub key: u64,
    pub path: PathBuf,
    pub job: Option<ImportJob>,
    pub stage: Stage,
    pub preparing: bool,
    pub error: Option<ImportError>,
    /// Files opened together; the first located one fits the map when the map was empty.
    pub batch: u64,
}

impl ImportRow {
    pub fn name(&self) -> String {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string()
    }

    /// Progress text for the shelf row ("Reading 1.2 of 3.4 MB").
    pub fn progress(&self) -> String {
        if self.preparing {
            return "Preparing a faster preview…".into();
        }
        if self.stage == Stage::Metadata
            && let Some(job) = &self.job
        {
            let (done, total) = job.ticket.bytes();
            if total > 0 {
                return format!("Reading {} of {}", mb(done), mb(total));
            }
        }
        self.stage.label().into()
    }
}

fn mb(b: u64) -> String {
    let m = b as f64 / 1_000_000.0;
    if m >= 100.0 { format!("{m:.0} MB") } else { format!("{m:.1} MB") }
}

/// A question card answered, for the question log (no file names or values).
#[derive(Clone, Debug, PartialEq)]
pub struct QuestionAnswer {
    pub kind: AssumptionKind,
    pub answer: Choice,
    pub seconds: f64,
}

/// A re-read running off the interface thread (other coordinate columns, another sheet, a chosen
/// coordinate system). Its result replaces the layer's data in one undo step.
struct PendingEdit {
    id: LayerId,
    label: String,
    /// The revision the result will carry (from a counter that only goes up).
    rev: u32,
    /// The layer's revision when the edit started: a result for a layer that changed meanwhile
    /// is dropped rather than committed over the newer state.
    base_rev: u32,
    fit: bool,
    rx: std::sync::mpsc::Receiver<Result<ImportedLayer, String>>,
}

/// Heavy work done off the interface thread that ends with a small change to the workspace.
type Finish = Box<dyn FnOnce(&mut Workspace) + Send>;

/// A map layer built ahead in both copies (view and render thread).
struct ReadyLayer {
    id: LayerId,
    rev: u32,
    map_id: u32,
    hit: Option<Arc<lm_mapview::hit::HitIndex>>,
    for_view: lm_mapview::map::Layer,
    for_render: lm_mapview::map::Layer,
}

/// What the map currently holds for a layer.
#[derive(Clone, PartialEq)]
struct Synced {
    id: LayerId,
    map_id: u32,
    rev: u32,
}

/// The Compare view for China offsets (WP2.5): a layer's points as in the file, and the same
/// points converted, shown together over the street map until one set is kept.
pub struct Compare {
    pub layer: LayerId,
    pub from_bd09: bool,
    /// The converted points' map layer in both copies, until the map takes it.
    map_pair: Option<(lm_mapview::map::Layer, lm_mapview::map::Layer)>,
    /// Where to look: a point of the layer, at street level.
    pub focus: (f64, f64),
}

/// The map id of the Compare view's converted points.
const COMPARE_MAP_ID: u32 = MAP_ID_BASE - 1;

/// Map layer ids for LayeredMap's layers start here, clear of anything the map assigns itself.
const MAP_ID_BASE: u32 = 1_000_000;

mod join;
pub use join::{JoinOffer, JoinQuestion, JoinReport};

#[derive(Default)]
pub struct Workspace {
    pub doc: Document,
    pub undo: UndoStack,
    data: HashMap<(LayerId, u32), Arc<ImportedLayer>>,
    pub imports: Vec<ImportRow>,
    /// What is selected (interaction state: never saved, never an undo step).
    pub selection: lm_core::selection::SelectionContext,
    /// An object highlighted from the overlap chooser.
    pub hover: Option<lm_core::selection::FeatureRef>,
    synced: Vec<Synced>,
    /// The order, visibility and styles last sent to the map.
    order_key: String,
    next_key: u64,
    next_batch: u64,
    /// Batches that fit the map as their layers arrive (the map was empty when they started),
    /// with the extent gathered so far.
    fit_batches: Vec<(u64, Option<Extent>)>,
    /// Where to fly after this frame (the first extent of a batch on an empty map).
    pub fit: Option<Extent>,
    /// A notice to show ("Zoom to new data", errors), taken by the shell.
    pub notice: Option<String>,
    /// Extent of the newest layers when the map wasn't empty, for "Zoom to new data".
    pub new_data: Option<Extent>,
    /// Answers to question cards, for the question log (taken by the shell).
    pub answers: Vec<QuestionAnswer>,
    pub compare: Option<Compare>,
    pending: Vec<PendingEdit>,
    /// Background work that finishes with a closure applied here.
    /// Each job's layer (`None` for work that doesn't make the layer busy: map layers, the
    /// Compare preview) and what to do if it fails.
    jobs: Vec<(Option<LayerId>, std::sync::mpsc::Receiver<Finish>)>,
    /// Map layers being built, and built.
    building: Vec<(LayerId, u32)>,
    ready: Vec<ReadyLayer>,
    next_rev: u32,
    next_map_id: u32,
    /// Wakes the interface when background work finishes (set by the shell).
    pub wake: Option<Arc<dyn Fn() + Send + Sync>>,
    compare_synced: Option<(LayerId, bool)>,
    /// Per-layer results worked out from the data for drawing (colour and size by value, the
    /// filter's row mask), with the key they were made for.
    derived: HashMap<LayerId, Derived>,
    /// Keys being worked out.
    deriving: HashMap<LayerId, String>,
    /// The map's view in WGS 84, set by the shell when the camera settles ("Fit range to view").
    pub view_extent: Option<Extent>,
    /// Insight indexes per layer, with the key they were built for (data, filter, roles).
    indexes: HashMap<LayerId, (String, Arc<lm_jobs::inspect::LayerIndex>)>,
    /// Hit-testing indexes per layer and data revision.
    hits: HashMap<(LayerId, u32), Arc<lm_mapview::hit::HitIndex>>,
    indexing: HashMap<LayerId, String>,
    /// Find-in-a-layer indexes (WP3.10) per layer and data revision, built on first use.
    finds: HashMap<LayerId, (u32, Arc<lm_jobs::find::FindIndex>)>,
    finding: HashMap<LayerId, u32>,
    /// Joining tables to places (WP3.11).
    joins: join::JoinState,
}

/// Drawing state derived from a layer's data (WP3.5, WP3.7).
#[derive(Clone, Default)]
pub struct Derived {
    pub key: String,
    pub classes: Option<Arc<lm_jobs::inspect::RowClasses>>,
    pub mask: Option<Arc<Vec<bool>>>,
    /// Records the filter keeps (all when unfiltered).
    pub shown: usize,
    symbols: Option<Arc<lm_mapview::map::RowSymbols>>,
}

impl Workspace {
    /// Whether no background work is running (tests wait for this).
    pub fn idle(&self) -> bool {
        self.jobs.is_empty() && self.building.is_empty() && self.pending.is_empty() && self.ready.is_empty()
    }

    /// The layer the inspector shows (the selection's layer).
    pub fn selected(&self) -> Option<LayerId> {
        self.selection.active_layer
    }

    /// Selects a whole layer (shelf click).
    pub fn select_layer(&mut self, id: LayerId) {
        self.selection = lm_core::selection::SelectionContext::layer(id);
    }

    pub fn clear_selection(&mut self) {
        self.selection = lm_core::selection::SelectionContext::default();
    }

    /// A workspace whose background work repaints `ctx` when it finishes.
    pub fn with_waker(ctx: &egui::Context) -> Self {
        let c = ctx.clone();
        Workspace { wake: Some(Arc::new(move || c.request_repaint())), ..Workspace::default() }
    }

    /// Starts importing files (Open, a drop, Open With): one batch.
    pub fn open(&mut self, paths: Vec<PathBuf>, wake: impl Fn() + Send + Sync + Clone + 'static) {
        if paths.is_empty() {
            return;
        }
        self.next_batch += 1;
        let batch = self.next_batch;
        if self.doc.layers.is_empty() && self.imports.is_empty() {
            self.fit_batches.push((batch, None));
        }
        for path in paths {
            self.next_key += 1;
            let w = wake.clone();
            let job = lm_jobs::import::start(path.clone(), w);
            self.imports.push(ImportRow {
                key: self.next_key,
                path,
                job: Some(job),
                stage: Stage::Waiting,
                preparing: false,
                error: None,
                batch,
            });
        }
    }

    /// Cancels an import, or dismisses a failed row.
    pub fn cancel(&mut self, key: u64) {
        if let Some(i) = self.imports.iter().position(|r| r.key == key) {
            if let Some(job) = &self.imports[i].job {
                job.cancel();
            } else {
                self.imports.remove(i);
            }
        }
    }

    /// Takes news from running imports. Returns whether the document changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for e in self.poll_pending() {
            self.notice = Some(e);
            changed = true;
        }
        self.poll_jobs();
        let mut finished: Vec<(usize, Result<Vec<ImportedLayer>, ImportError>)> = Vec::new();
        for (i, row) in self.imports.iter_mut().enumerate() {
            let Some(job) = &row.job else { continue };
            for e in job.poll() {
                match e {
                    ImportEvent::Stage(s) => row.stage = s,
                    ImportEvent::Preparing => row.preparing = true,
                    ImportEvent::Done(r) => finished.push((i, r)),
                }
            }
        }
        // Back to front, so indices stay valid while rows are removed.
        for (i, result) in finished.into_iter().rev() {
            let batch = self.imports[i].batch;
            // A × clicked as the import finished still wins: nothing is added.
            let canceled = self.imports[i].job.as_ref().is_some_and(|j| j.ticket.is_canceled());
            match result {
                Ok(_) if canceled => {
                    self.imports.remove(i);
                }
                Ok(layers) => {
                    self.imports.remove(i);
                    changed |= self.add_imported(layers, batch);
                }
                Err(ImportError::Canceled) => {
                    self.imports.remove(i);
                }
                Err(e) => {
                    let row = &mut self.imports[i];
                    row.job = None;
                    row.error = Some(e);
                }
            }
        }
        let imports = &self.imports;
        self.fit_batches.retain(|(b, _)| imports.iter().any(|r| r.batch == *b && r.job.is_some()));
        changed
    }

    /// Adds a finished import's layers in one transaction ("Open schools.csv").
    pub fn add_imported(&mut self, layers: Vec<ImportedLayer>, batch: u64) -> bool {
        if layers.is_empty() {
            return false;
        }
        let file = layers[0].profile.source.file_name().to_string();
        let mut ops = Vec::new();
        let mut added = Vec::new();
        let mut extent: Option<Extent> = None;
        for layer in layers {
            let mut tx = self.doc.add_layer_tx(layer.profile.clone());
            // Several layers from one file go in together; each op's index assumes the earlier
            // ones are in, so apply as we go and gather the ops for one undo step.
            if let Some(Op::AddLayer { layer: l, .. }) = tx.ops.first_mut() {
                l.display_name = layer.name.clone();
                added.push((l.id, l.profile.data_rev));
            }
            if let Some(e) = layer.profile.extent_wgs84 {
                extent = Some(match extent {
                    None => e,
                    Some(a) => Extent {
                        min_x: a.min_x.min(e.min_x),
                        min_y: a.min_y.min(e.min_y),
                        max_x: a.max_x.max(e.max_x),
                        max_y: a.max_y.max(e.max_y),
                    },
                });
            }
            if self.doc.apply(&tx).is_ok() {
                ops.extend(tx.ops);
                if let Some((id, rev)) = added.last() {
                    self.data.insert((*id, *rev), Arc::new(layer));
                }
            }
        }
        if ops.is_empty() {
            return false;
        }
        // The ops are already applied: undo the batch and commit it as one step.
        let tx = Transaction::new(format!("Open {file}"), ops);
        let _ = self.doc.apply(&tx.inverse());
        if self.undo.commit(&mut self.doc, tx).is_err() {
            return false;
        }
        if let Some(e) = extent {
            if let Some((_, gathered)) = self.fit_batches.iter_mut().find(|(b, _)| *b == batch) {
                let all = match *gathered {
                    None => e,
                    Some(a) => Extent {
                        min_x: a.min_x.min(e.min_x),
                        min_y: a.min_y.min(e.min_y),
                        max_x: a.max_x.max(e.max_x),
                        max_y: a.max_y.max(e.max_y),
                    },
                };
                *gathered = Some(all);
                self.fit = Some(all);
            } else {
                self.new_data = Some(e);
            }
        }
        if self.selected().is_none()
            && let Some((id, _)) = added.first()
        {
            self.select_layer(*id);
        }
        true
    }

    pub fn commit(&mut self, tx: Option<Transaction>) -> bool {
        match tx {
            Some(tx) => self.undo.commit(&mut self.doc, tx).is_ok(),
            None => false,
        }
    }

    /// Commits one moment of a slider drag; the whole drag is one undo step (WP3.8).
    pub fn commit_gesture(&mut self, tx: Option<Transaction>, key: &str) -> bool {
        match tx {
            Some(tx) => self.undo.commit_gesture(&mut self.doc, tx, key).is_ok(),
            None => false,
        }
    }

    pub fn end_gesture(&mut self) {
        self.undo.end_gesture();
    }

    /// Whether a layer is being read again (undo and redo wait for it).
    fn rereading(&self) -> bool {
        !self.pending.is_empty() || self.jobs.iter().any(|(l, _)| l.is_some())
    }

    pub fn undo(&mut self) -> Option<String> {
        if self.rereading() {
            self.notice = Some("Still reading a layer. Undo in a moment.".into());
            return None;
        }
        self.undo.undo(&mut self.doc)
    }

    pub fn redo(&mut self) -> Option<String> {
        if self.rereading() {
            self.notice = Some("Still reading a layer. Redo in a moment.".into());
            return None;
        }
        self.undo.redo(&mut self.doc)
    }

    /// The data a document layer draws, if any.
    pub fn data(&self, id: LayerId) -> Option<&Arc<ImportedLayer>> {
        let l = self.doc.layer(id)?;
        self.data.get(&(id, l.profile.data_rev))
    }

    /// Answers a question or changes an assumption on a layer (WP2.3): reruns only what the
    /// answer affects, keeps the style, and is one undo step.
    pub fn change_assumption(
        &mut self,
        id: LayerId,
        kind: AssumptionKind,
        subject: Subject,
        choice: Choice,
        seconds: Option<f64>,
    ) -> Result<(), String> {
        let Some(layer) = self.doc.layer(id).cloned() else { return Err("That layer is gone.".into()) };
        self.not_busy(id, &layer.display_name)?;
        let was_question = layer
            .profile
            .assumptions
            .iter()
            .any(|a| a.kind == kind && a.treatment == lm_core::model::Treatment::Question && !a.confirmed_by_user);
        if let Some(secs) = seconds.filter(|_| was_question) {
            self.answers.push(QuestionAnswer { kind, answer: choice.clone(), seconds: secs });
        }
        let label = format!("Change how {} is read", layer.display_name);
        match kind {
            // Not sure on "Where should this data be?" goes to the re-read branch below (it may
            // be placed data with no raw cells).
            AssumptionKind::CsvCoordinateColumns | AssumptionKind::CsvAmbiguousColumns | AssumptionKind::NoCrsProjected
                if choice.id.starts_with("columns:") || (choice.id == "not_sure" && kind != AssumptionKind::NoCrsProjected) =>
            {
                let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
                if data.raw.is_none() {
                    return self.record(id, kind, subject, choice, label);
                }
                let wake = self.wake.clone();
                self.spawn_edit(id, label, false, wake, move || lm_jobs::import::relocate_csv(&data, &choice).map_err(|e| e.to_string()));
                Ok(())
            }
            AssumptionKind::ChinaOffsetPossible | AssumptionKind::ChinaOffsetNamed if matches!(choice.id.as_str(), "gcj02" | "bd09") => {
                self.convert_offset(id, &choice, kind, subject)
            }
            AssumptionKind::ExcelSheet if choice.id.starts_with("sheet:") => {
                let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
                let wake = self.wake.clone();
                self.spawn_edit(id, format!("Open another sheet of {}", layer.display_name), true, wake, move || {
                    lm_jobs::import::choose_sheet(&data, &choice).map_err(|e| e.to_string())
                });
                Ok(())
            }
            AssumptionKind::RasterNoData | AssumptionKind::RasterKelvin | AssumptionKind::LandsatScale | AssumptionKind::RasterNoUnit => {
                let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
                let wake = self.wake.clone();
                self.spawn_edit(id, label, false, wake, move || {
                    lm_jobs::import::rechoose_raster(&data, kind, &subject, &choice).map_err(|e| e.to_string())
                });
                Ok(())
            }
            AssumptionKind::ShapefileGbk if matches!(choice.id.as_str(), "gbk" | "utf8" | "windows1252") => {
                let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
                let wake = self.wake.clone();
                self.spawn_edit(id, label, false, wake, move || {
                    lm_jobs::import::reread_encoding(&data, &choice).map_err(|e| e.to_string())
                });
                Ok(())
            }
            // "Choose where it should be…", and Not sure on data that was placed by hand: the file is
            // read again with its coordinates as stored (never projecting placed data twice), and
            // the question comes back (or, for Not sure, the layer opens as a table).
            AssumptionKind::NoCrsProjected | AssumptionKind::NoCrsLonLat if choice.id == "find_place" || choice.id == "not_sure" => {
                let saved = layer.profile.clone();
                let as_table = choice.id == "not_sure";
                let wake = self.wake.clone();
                self.spawn_edit(id, format!("Place {} again", layer.display_name), false, wake, move || {
                    let fresh = lm_jobs::import::fresh_read(&saved.source).map_err(|e| e.to_string())?;
                    let mut others = saved.clone();
                    others.assumptions.retain(|a| !matches!(a.kind, AssumptionKind::NoCrsProjected | AssumptionKind::NoCrsLonLat));
                    let mut layer = lm_jobs::import::reapply_choices(fresh, &others).map_err(|e| e.to_string())?;
                    let p = &mut layer.profile;
                    p.assumptions.retain(|a| !matches!(a.kind, AssumptionKind::NoCrsProjected | AssumptionKind::NoCrsLonLat));
                    p.assumptions.push(lm_core::model::Assumption {
                        kind: AssumptionKind::NoCrsProjected,
                        subject: Subject::Layer,
                        chosen: if as_table { Choice::not_sure() } else { Choice::new("find_place", "Choose where it should be…") },
                        alternatives: vec![Choice::new("find_place", "Choose where it should be…"), Choice::not_sure()],
                        confidence: lm_core::model::Confidence::Unknown,
                        consequence: lm_core::model::Consequence::ChangesLocations,
                        treatment: if as_table { lm_core::model::Treatment::Visible } else { lm_core::model::Treatment::Question },
                        line: if as_table {
                            "Opened as a table: where this data is hasn't been chosen.".into()
                        } else {
                            "Where should this data be?".into()
                        },
                        confirmed_by_user: as_table,
                    });
                    p.crs = lm_core::model::Known::unknown(Crs::Unknown, "Not chosen yet.");
                    p.extent_wgs84 = None;
                    p.kind = LayerKind::Table;
                    Ok(layer)
                });
                Ok(())
            }
            _ => self.record(id, kind, subject, choice, label),
        }
    }

    /// Runs `work` on its own thread; the closure it returns is applied here by `poll`. `layer` is
    /// the layer it concerns (its row shows the working badge meanwhile).
    fn spawn_job(&mut self, layer: Option<LayerId>, on_fail: Finish, work: impl FnOnce() -> Finish + Send + 'static) {
        let (tx, rx) = std::sync::mpsc::channel::<Finish>();
        let wake = self.wake.clone();
        let spawned = std::thread::Builder::new().name("lm-work".into()).spawn(move || {
            let f = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|_| {
                Box::new(move |w: &mut Workspace| {
                    w.notice = Some("That couldn't be done. Nothing was changed.".into());
                    on_fail(w);
                }) as Finish
            });
            let _ = tx.send(f);
            if let Some(w) = wake {
                w();
            }
        });
        if spawned.is_ok() {
            self.jobs.push((layer, rx));
        }
    }

    fn poll_jobs(&mut self) {
        let mut done = Vec::new();
        self.jobs.retain(|(_, rx)| match rx.try_recv() {
            Ok(f) => {
                done.push(f);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for f in done {
            f(self);
        }
    }

    /// Runs a re-read of a layer's data on its own thread; `poll` commits the result.
    fn spawn_edit(
        &mut self,
        id: LayerId,
        label: String,
        fit: bool,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
        work: impl FnOnce() -> Result<ImportedLayer, String> + Send + 'static,
    ) {
        let base_rev = self.doc.layer(id).map_or(0, |l| l.profile.data_rev);
        self.next_rev = self.next_rev.max(base_rev) + 1;
        let rev = self.next_rev;
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("lm-reread".into()).spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| Err("The layer couldn't be read again.".into()));
            let _ = tx.send(r);
            if let Some(w) = wake {
                w();
            }
        });
        if spawned.is_ok() {
            self.pending.push(PendingEdit { id, label, rev, base_rev, fit, rx });
        }
    }

    /// Whether a layer is being re-read (its row shows the working badge).
    pub fn is_busy(&self, id: LayerId) -> bool {
        self.pending.iter().any(|p| p.id == id) || self.jobs.iter().any(|(l, _)| *l == Some(id))
    }

    /// Refuses a second change to a layer while the first is still being worked out.
    fn not_busy(&self, id: LayerId, name: &str) -> Result<(), String> {
        if self.is_busy(id) { Err(format!("Still reading {name}. Try again in a moment.")) } else { Ok(()) }
    }

    /// Commits re-reads that have finished. Returns errors to show.
    fn poll_pending(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut done = Vec::new();
        for (i, p) in self.pending.iter().enumerate() {
            match p.rx.try_recv() {
                Ok(r) => done.push((i, r)),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => done.push((i, Err("The layer couldn't be read again.".into()))),
            }
        }
        for (i, r) in done.into_iter().rev() {
            let p = self.pending.remove(i);
            let current = self.doc.layer(p.id).map(|l| l.profile.data_rev);
            match r {
                Ok(_) if current != Some(p.base_rev) => {
                    errors.push("The layer changed while it was being read again, so that change was not applied.".into());
                }
                Ok(mut new) => {
                    new.profile.data_rev = p.rev;
                    let fit = new.profile.extent_wgs84.filter(|_| p.fit);
                    self.data.insert((p.id, p.rev), Arc::new(new.clone()));
                    let tx = self.doc.set_profile_tx(p.id, new.profile).map(|mut t| {
                        t.label = p.label.clone();
                        t
                    });
                    if self.commit(tx) && fit.is_some() {
                        self.fit = fit;
                    }
                }
                Err(e) => errors.push(e),
            }
        }
        errors
    }

    /// Blocks until every re-read has finished (tests, and before saving).
    pub fn wait_pending(&mut self) {
        while !self.pending.is_empty() || !self.jobs.is_empty() {
            for e in self.poll_pending() {
                self.notice = Some(e);
            }
            self.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// Records a changed choice (and marks it confirmed) without re-reading data.
    fn record(&mut self, id: LayerId, kind: AssumptionKind, subject: Subject, choice: Choice, label: String) -> Result<(), String> {
        let Some(layer) = self.doc.layer(id).cloned() else { return Err("That layer is gone.".into()) };
        // The line must say what is now in effect.
        let mut profile = layer.profile.clone();
        let mut found = false;
        for a in profile.assumptions.iter_mut().filter(|a| a.kind == kind && a.subject == subject) {
            a.line = line_for(kind, &choice).unwrap_or_else(|| a.line.clone());
            a.chosen = choice.clone();
            a.confirmed_by_user = true;
            found = true;
        }
        if !found {
            return Err("That assumption couldn't be changed.".into());
        }
        let tx = self.doc.set_profile_tx(id, profile).map(|mut t| {
            t.label = label;
            t
        });
        if self.commit(tx) { Ok(()) } else { Err("That assumption couldn't be changed.".into()) }
    }

    /// Converts a layer's points from GCJ-02 or BD-09 to GPS coordinates as a new layer beside it
    /// (WP2.5). The conversion runs off the interface thread; then, in one undo step, the GPS
    /// layer is added, the original hidden, and the original's question closed. The source file is
    /// untouched.
    fn convert_offset(&mut self, id: LayerId, choice: &Choice, kind: AssumptionKind, subject: Subject) -> Result<(), String> {
        let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
        if !matches!(data.data, LayerData::Vector(_)) {
            return Err("Only points, lines and areas can be converted.".into());
        }
        let base_rev = self.doc.layer(id).map_or(0, |l| l.profile.data_rev);
        let choice = choice.clone();
        self.spawn_job(Some(id), Box::new(|_| {}), move || {
            let LayerData::Vector(ds) = &data.data else { return Box::new(|_: &mut Workspace| {}) };
            match lm_jobs::import::convert_offset(ds, choice.id == "bd09") {
                Ok((mut converted, recipe)) => {
                    converted.source = None;
                    Box::new(move |w: &mut Workspace| w.finish_convert(id, base_rev, choice, kind, subject, converted, recipe))
                }
                Err(e) => Box::new(move |w: &mut Workspace| w.notice = Some(e.to_string())),
            }
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_convert(
        &mut self,
        id: LayerId,
        base_rev: u32,
        choice: Choice,
        kind: AssumptionKind,
        subject: Subject,
        ds: lm_jobs::import::Dataset,
        recipe: String,
    ) {
        let Some(orig) = self.doc.layer(id).cloned().filter(|l| l.profile.data_rev == base_rev) else {
            self.notice = Some("The layer changed while it was being converted, so nothing was added.".into());
            return;
        };
        let mut profile = orig.profile.clone();
        profile.crs = lm_core::model::Known::certain(Crs::Wgs84, recipe.clone());
        profile.notices.push(recipe);
        // The GPS copy is converted: it no longer offers a conversion.
        profile.assumptions.retain(|a| !(a.kind == kind && a.subject == subject));
        profile.data_rev = 0;
        let e = ds.extent();
        profile.extent_wgs84 = e.min_x.is_finite().then_some(Extent { min_x: e.min_x, min_y: e.min_y, max_x: e.max_x, max_y: e.max_y });
        let mut add = self.doc.add_layer_tx(profile.clone());
        let mut new_id = None;
        if let Some(Op::AddLayer { layer: l, .. }) = add.ops.first_mut() {
            l.display_name = format!("{} (GPS)", orig.display_name);
            l.style = orig.style.clone();
            new_id = Some(l.id);
        }
        let Some(new_id) = new_id else { return };
        self.data.insert(
            (new_id, 0),
            Arc::new(ImportedLayer {
                name: format!("{} (GPS)", orig.display_name),
                profile,
                data: LayerData::Vector(Arc::new(ds)),
                status: ImportStatus::Complete,
                raw: None,
            }),
        );
        let mut ops = add.ops;
        if let Some(hide) = self.doc.set_visible_tx(id, false) {
            ops.extend(hide.ops);
        }
        // The original's question is answered by this conversion too, and its line says so.
        let mut original = orig.profile.clone();
        for a in original.assumptions.iter_mut().filter(|a| a.kind == kind && a.subject == subject) {
            a.chosen = choice.clone();
            a.confirmed_by_user = true;
            a.treatment = lm_core::model::Treatment::Visible;
            a.line = format!(
                "Converted to GPS coordinates in “{} (GPS)”; this layer is hidden and keeps the positions as in the file.",
                orig.display_name
            );
        }
        if let Some(answer) = self.doc.set_profile_tx(id, original) {
            ops.extend(answer.ops);
        }
        let tx = Transaction::new(format!("Convert {} to GPS coordinates", orig.display_name), ops);
        if self.commit(Some(tx)) {
            self.select_layer(new_id);
        } else {
            self.notice = Some("The converted layer couldn't be added.".into());
        }
    }

    /// "Where should this data be?": the candidates near a place (longitude, latitude).
    pub fn place_candidates(&self, id: LayerId, place: (f64, f64)) -> Vec<lm_jobs::import::PlaceCandidate> {
        self.data(id).map(|d| lm_jobs::import::place_candidates(d, place)).unwrap_or_default()
    }

    /// Places a layer read with raw coordinates using the coordinate system the user picked:
    /// re-reads nothing, converts the coordinates, keeps the style; one undo step.
    pub fn place_layer(&mut self, id: LayerId, epsg: u16, seconds: Option<f64>) -> Result<(), String> {
        let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
        let Some(layer) = self.doc.layer(id).cloned() else { return Err("That layer is gone.".into()) };
        self.not_busy(id, &layer.display_name)?;
        if let Some(secs) = seconds {
            let name = lm_jobs::import::crs_name(epsg);
            self.answers.push(QuestionAnswer {
                kind: AssumptionKind::NoCrsProjected,
                answer: Choice::new(format!("epsg:{epsg}"), name),
                seconds: secs,
            });
        }
        let wake = self.wake.clone();
        let saved = layer.profile.clone();
        self.spawn_edit(id, format!("Place {}", layer.display_name), true, wake, move || {
            // Always from the coordinates as stored: data placed before is read again first.
            let base = if matches!(saved.crs.value, Crs::Unknown) {
                (*data).clone()
            } else {
                let fresh = lm_jobs::import::fresh_read(&saved.source).map_err(|e| e.to_string())?;
                let mut others = saved.clone();
                others.assumptions.retain(|a| !matches!(a.kind, AssumptionKind::NoCrsProjected | AssumptionKind::NoCrsLonLat));
                lm_jobs::import::reapply_choices(fresh, &others).map_err(|e| e.to_string())?
            };
            lm_jobs::import::reproject(&base, u32::from(epsg)).map_err(|e| e.to_string())
        });
        Ok(())
    }

    /// Starts comparing a layer's points with the same points converted from GCJ-02 (or BD-09).
    /// The conversion and the map layer are built off the interface thread.
    pub fn start_compare(&mut self, id: LayerId, from_bd09: bool) -> Result<(), String> {
        let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
        let LayerData::Vector(ds) = &data.data else { return Err("Only points, lines and areas can be compared.".into()) };
        let mut focus = None;
        if let Some(g) = ds.features.iter().find_map(|f| f.geometry.as_ref()) {
            g.for_each_coord(|c| {
                if focus.is_none() {
                    focus = Some((c.x, c.y));
                }
            });
        }
        let focus = focus.ok_or("This layer has nothing to compare.")?;
        self.compare = Some(Compare { layer: id, from_bd09, map_pair: None, focus });
        self.spawn_job(None, Box::new(|w: &mut Workspace| w.compare = None), move || {
            let LayerData::Vector(ds) = &data.data else { return Box::new(|_: &mut Workspace| {}) };
            match lm_jobs::import::convert_offset(ds, from_bd09) {
                Ok((converted, recipe)) => {
                    let mut profile = data.profile.clone();
                    profile.notices = vec![recipe];
                    let preview = ImportedLayer {
                        name: format!("{} (converted)", data.name),
                        profile,
                        data: LayerData::Vector(Arc::new(converted)),
                        status: ImportStatus::Complete,
                        raw: None,
                    };
                    let a = map_layer(COMPARE_MAP_ID, &preview);
                    let b = a.clone();
                    Box::new(move |w: &mut Workspace| {
                        if let Some(c) = w.compare.as_mut().filter(|c| c.layer == id && c.from_bd09 == from_bd09) {
                            c.map_pair = Some((a, b));
                        }
                    })
                }
                Err(e) => Box::new(move |w: &mut Workspace| {
                    w.compare = None;
                    w.notice = Some(e.to_string());
                }),
            }
        });
        Ok(())
    }

    /// Ends the Compare view: keep the converted points (a new GPS layer), keep the points as
    /// they are (the assumption is confirmed), or just close it.
    pub fn end_compare(&mut self, keep: Option<bool>, seconds: Option<f64>) -> Result<(), String> {
        let Some(c) = self.compare.take() else { return Ok(()) };
        let Some(a) = self.doc.layer(c.layer).and_then(|l| {
            l.profile
                .assumptions
                .iter()
                .find(|a| matches!(a.kind, AssumptionKind::ChinaOffsetPossible | AssumptionKind::ChinaOffsetNamed))
                .cloned()
        }) else {
            return Ok(());
        };
        match keep {
            Some(true) => {
                let choice = a
                    .alternatives
                    .iter()
                    .find(|x| x.id == if c.from_bd09 { "bd09" } else { "gcj02" })
                    .cloned()
                    .unwrap_or_else(|| Choice::new("gcj02", "Amap/Tencent coordinates (GCJ-02): convert"));
                self.change_assumption(c.layer, a.kind, a.subject, choice, seconds)
            }
            Some(false) => {
                let choice = a
                    .alternatives
                    .iter()
                    .find(|x| x.id == "wgs84")
                    .cloned()
                    .unwrap_or_else(|| Choice::new("wgs84", "GPS coordinates (WGS 84): keep where they are"));
                self.change_assumption(c.layer, a.kind, a.subject, choice, seconds)
            }
            None => Ok(()),
        }
    }

    /// Reopens a saved document as it was saved (names, styles, order, visibility, answers,
    /// placements, conversions). Each source file is read once, off the interface thread, and each
    /// layer's data is derived again from it by the choices the document records. A file that is
    /// missing or has changed is said in a notice.
    pub fn restore(&mut self, doc: Document) {
        let mut by_path: Vec<(String, Vec<lm_core::document::DocLayer>)> = Vec::new();
        for l in &doc.layers {
            match by_path.iter_mut().find(|(p, _)| *p == l.profile.source.path) {
                Some((_, v)) => v.push(l.clone()),
                None => by_path.push((l.profile.source.path.clone(), vec![l.clone()])),
            }
        }
        self.next_rev = doc.layers.iter().map(|l| l.profile.data_rev).max().unwrap_or(0);
        self.doc = doc;
        self.undo = UndoStack::new();
        self.clear_selection();
        for (path, layers) in by_path {
            let Some(first) = layers.first().map(|l| l.id) else { continue };
            self.spawn_job(Some(first), Box::new(|_| {}), move || {
                // The source check may hash the file: here, off the interface thread.
                let state = lm_doc::check_source(&layers[0].profile.source);
                let file = layers[0].profile.source.file_name().to_string();
                let mut notes = Vec::new();
                let mut out = Vec::new();
                match state {
                    lm_doc::SourceState::Missing => {
                        let folder = std::path::Path::new(&path)
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("its folder")
                            .to_string();
                        notes.push(format!("Can't find {file}. It was last seen in {folder}."));
                    }
                    other => {
                        if other == lm_doc::SourceState::Changed {
                            notes.push(format!("{file} has changed since this document was saved; it was read as it is now."));
                        }
                        match lm_jobs::import::import_path(std::path::Path::new(&path), &lm_jobs::import::Ctx::detached(Default::default()))
                        {
                            Err(e) => notes.push(e.to_string()),
                            Ok(read) => {
                                for dl in layers {
                                    // The layer's own part of the file; a part that's gone is said.
                                    let base = match &dl.profile.source.member {
                                        Some(m) => read.iter().find(|r| r.profile.source.member.as_ref() == Some(m)),
                                        None => read.first().filter(|_| read.len() == 1),
                                    };
                                    let Some(base) = base else {
                                        notes.push(format!("{} is no longer in {file}.", dl.display_name));
                                        continue;
                                    };
                                    match derive_saved(base, &dl) {
                                        Ok(mut layer) => {
                                            layer.name = dl.display_name.clone();
                                            layer.profile = dl.profile.clone();
                                            out.push((dl.id, dl.profile.data_rev, layer));
                                        }
                                        Err(e) => notes.push(format!("{}: {e}", dl.display_name)),
                                    }
                                }
                            }
                        }
                    }
                }
                Box::new(move |w: &mut Workspace| {
                    for (id, rev, layer) in out {
                        w.data.insert((id, rev), Arc::new(layer));
                    }
                    if let Some(n) = notes.into_iter().next() {
                        w.notice = Some(n);
                    }
                })
            });
        }
    }

    /// Brings the map's layers in line with the document: adds and removes only what changed,
    /// then sets order, visibility and style in one cheap edit.
    pub fn sync_map(&mut self, view: &mut lm_mapview::MapView) {
        self.derive_layers();
        self.index_layers();
        self.try_joins();
        self.prune_data();
        let wanted: Vec<(LayerId, u32)> = self
            .doc
            .layers
            .iter()
            .filter(|l| drawable(&l.profile))
            .filter(|l| self.data.contains_key(&(l.id, l.profile.data_rev)))
            .map(|l| (l.id, l.profile.data_rev))
            .collect();
        // Remove what's gone or changed revision.
        let stale: Vec<u32> = self.synced.iter().filter(|s| !wanted.contains(&(s.id, s.rev))).map(|s| s.map_id).collect();
        if !stale.is_empty() {
            view.edit_map(move |m: &mut Map| m.layers.retain(|l| !stale.contains(&l.id)));
            self.synced.retain(|s| wanted.contains(&(s.id, s.rev)));
        }
        // Add what's new: map layers are built in both copies off the interface thread, then
        // handed to the map without copying here.
        for (id, rev) in &wanted {
            if self.synced.iter().any(|s| s.id == *id && s.rev == *rev) || self.building.contains(&(*id, *rev)) {
                continue;
            }
            let Some(data) = self.data.get(&(*id, *rev)).cloned() else { continue };
            self.next_map_id += 1;
            let map_id = MAP_ID_BASE + self.next_map_id;
            let (id, rev) = (*id, *rev);
            self.building.push((id, rev));
            self.spawn_job(None, Box::new(move |w: &mut Workspace| w.building.retain(|b| *b != (id, rev))), move || {
                let for_view = map_layer(map_id, &data);
                let for_render = for_view.clone();
                // The hit-testing index (WP3.1), built here with the map layer, off the
                // interface thread.
                let hit = match &data.data {
                    LayerData::Vector(ds) => Some(Arc::new(lm_mapview::hit::HitIndex::build(ds))),
                    LayerData::Raster(_) => None,
                };
                Box::new(move |w: &mut Workspace| {
                    w.building.retain(|b| *b != (id, rev));
                    w.ready.push(ReadyLayer { id, rev, map_id, hit, for_view, for_render });
                })
            });
        }
        for r in std::mem::take(&mut self.ready) {
            if !wanted.contains(&(r.id, r.rev)) || self.synced.iter().any(|s| s.id == r.id && s.rev == r.rev) {
                continue;
            }
            view.add_layer_pair(r.for_view, r.for_render);
            if let Some(h) = r.hit {
                self.hits.retain(|(id, _), _| *id != r.id);
                self.hits.insert((r.id, r.rev), h);
            }
            self.synced.push(Synced { id: r.id, map_id: r.map_id, rev: r.rev });
            self.order_key.clear();
        }
        // The Compare view's converted points.
        let want_compare = self
            .compare
            .as_ref()
            .filter(|c| c.map_pair.is_some() || self.compare_synced == Some((c.layer, c.from_bd09)))
            .map(|c| (c.layer, c.from_bd09));
        if want_compare != self.compare_synced {
            view.edit_map(move |m: &mut Map| m.layers.retain(|l| l.id != COMPARE_MAP_ID));
            if let Some((a, b)) = self.compare.as_mut().and_then(|c| c.map_pair.take()) {
                view.add_layer_pair(a, b);
            }
            self.compare_synced = want_compare;
            self.order_key.clear();
        }
        // Order (the map draws index 0 on top; the document lists bottom to top), visibility, style.
        let order: Vec<(u32, bool, Style, LayerKind)> = self
            .doc
            .layers
            .iter()
            .rev()
            .filter_map(|l| self.synced.iter().find(|s| s.id == l.id).map(|s| (s.map_id, l.visible, l.style.clone(), l.profile.kind)))
            .collect();
        // What each map layer takes from derived state and the selection.
        let extras: Vec<(u32, String, Vec<usize>)> = self
            .doc
            .layers
            .iter()
            .filter_map(|l| {
                let s = self.synced.iter().find(|s| s.id == l.id)?;
                let key = self.derived.get(&l.id).map(|d| d.key.clone()).unwrap_or_default();
                Some((s.map_id, key, self.highlighted(l.id)))
            })
            .collect();
        let key = format!("{order:?}{extras:?}");
        if self.order_key != key {
            self.order_key = key;
            let per_layer: Vec<(
                u32,
                Option<Arc<lm_mapview::map::RowSymbols>>,
                Option<Arc<Vec<bool>>>,
                Vec<usize>,
                Option<lm_core::document::LabelStyle>,
            )> = self
                .doc
                .layers
                .iter()
                .filter_map(|l| {
                    let s = self.synced.iter().find(|s| s.id == l.id)?;
                    let d = self.derived.get(&l.id);
                    Some((
                        s.map_id,
                        d.and_then(|d| d.symbols.clone()),
                        d.and_then(|d| d.mask.clone()),
                        self.highlighted(l.id),
                        l.style.labels.clone(),
                    ))
                })
                .collect();
            view.edit_map(move |m: &mut Map| {
                let mut compare = None;
                let mut ours: Vec<lm_mapview::map::Layer> = Vec::new();
                let mut others = Vec::new();
                for l in m.layers.drain(..) {
                    if l.id == COMPARE_MAP_ID {
                        compare = Some(l);
                    } else if l.id >= MAP_ID_BASE {
                        ours.push(l);
                    } else {
                        others.push(l);
                    }
                }
                // The Compare view's converted points draw above everything.
                if let Some(mut c) = compare {
                    apply_style(&mut c, &compare_style(), LayerKind::Points);
                    m.layers.push(c);
                }
                for (map_id, visible, style, kind) in &order {
                    if let Some(pos) = ours.iter().position(|l| l.id == *map_id) {
                        let mut l = ours.swap_remove(pos);
                        l.visible = *visible;
                        apply_style(&mut l, style, *kind);
                        if let Some((_, symbols, mask, rows, labels)) = per_layer.iter().find(|p| p.0 == *map_id) {
                            l.row_symbols = symbols.clone();
                            l.row_mask = mask.clone();
                            l.selection = rows.clone();
                            match labels {
                                Some(ls) => {
                                    l.labels.enabled = true;
                                    l.labels.field = ls.field.clone();
                                    l.labels.size = ls.size;
                                }
                                None => l.labels.enabled = false,
                            }
                        }
                        m.layers.push(l);
                    }
                }
                m.layers.extend(others);
            });
        }
    }

    /// What was derived for a layer's drawing (legend, filter mask), once worked out.
    pub fn derived(&self, id: LayerId) -> Option<&Derived> {
        self.derived.get(&id)
    }

    /// The key of what a layer's drawing needs from its data: a change means work it out again.
    fn derive_key(&self, l: &lm_core::document::DocLayer) -> String {
        let fit = l.style.color_by.as_ref().is_some_and(|c| c.fit_to_view);
        let view = if fit { format!("{:?}", self.view_extent) } else { String::new() };
        format!(
            "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{view}",
            l.profile.data_rev, l.style.color_by, l.style.size_by, l.filter, l.roles, l.style.fill, l.style.stroke, l.style.point_size
        )
    }

    /// Rows drawn highlighted in a layer: the selection, plus a chooser row being hovered.
    fn highlighted(&self, id: LayerId) -> Vec<usize> {
        let mut rows = self.selection.rows_in(id);
        if let Some(h) = self.hover.filter(|h| h.layer == id && !rows.contains(&h.row)) {
            rows.push(h.row);
        }
        rows
    }

    /// The objects under a click, topmost layer first (visible vector layers; rows the filter
    /// hides are not hit).
    pub fn hit_test(&self, view: &lm_mapview::MapView, lonlat: lm_mapview::Coord) -> Vec<lm_core::selection::FeatureRef> {
        use lm_mapview::hit::{HitLayer, HitSymbol};
        let mut held = Vec::new();
        for l in self.doc.layers.iter().rev().filter(|l| l.visible && drawable(&l.profile) && l.profile.kind != LayerKind::Raster) {
            let rev = l.profile.data_rev;
            let (Some(index), Some(data)) = (self.hits.get(&(l.id, rev)), self.data.get(&(l.id, rev))) else { continue };
            let LayerData::Vector(ds) = &data.data else { continue };
            let point_size = match &l.style.size_by {
                Some(sb) => sb.max_size,
                None => l.style.point_size,
            };
            let symbol = HitSymbol { point_size: f64::from(point_size), line_width: f64::from(l.style.line_width) };
            held.push((l.id, index.clone(), ds.clone(), symbol));
        }
        let layers: Vec<HitLayer<'_>> = held
            .iter()
            .enumerate()
            .map(|(k, (_, index, ds, symbol))| HitLayer { id: k as u32, index, data: ds, symbol: *symbol })
            .collect();
        view.hit_test(&layers, lonlat)
            .into_iter()
            .filter_map(|h| {
                let id = held.get(h.layer as usize)?.0;
                let shown = self.derived.get(&id).and_then(|d| d.mask.as_ref()).is_none_or(|m| m.get(h.row).copied().unwrap_or(true));
                shown.then_some(lm_core::selection::FeatureRef { layer: id, row: h.row })
            })
            .collect()
    }

    /// The topmost visible raster with a value under a click: its layer and the shown band's
    /// value (None for NoData).
    pub fn raster_pick(&self, lonlat: lm_mapview::Coord) -> Option<(LayerId, Option<f64>)> {
        for l in self.doc.layers.iter().rev().filter(|l| l.visible && l.profile.kind == LayerKind::Raster) {
            let Some(data) = self.data.get(&(l.id, l.profile.data_rev)) else { continue };
            let LayerData::Raster(r) = &data.data else { continue };
            if let Some(v) = lm_jobs::inspect::raster_value_at(r, lm_jobs::inspect::style_band(r, &l.style), lonlat.x, lonlat.y) {
                return Some((l.id, v));
            }
        }
        None
    }

    /// A layer's insight index, once built for its current data, filter and roles.
    pub fn layer_index(&self, id: LayerId) -> Option<Arc<lm_jobs::inspect::LayerIndex>> {
        let l = self.doc.layer(id)?;
        let (key, ix) = self.indexes.get(&id)?;
        (*key == Self::index_key(l)).then(|| ix.clone())
    }

    fn index_key(l: &lm_core::document::DocLayer) -> String {
        format!("{}|{:?}|{:?}", l.profile.data_rev, l.filter, l.roles)
    }

    /// A layer's find index for its current data; the first call starts building it in the
    /// background and returns None until it is ready.
    pub fn find_index(&mut self, id: LayerId) -> Option<Arc<lm_jobs::find::FindIndex>> {
        let rev = self.doc.layer(id)?.profile.data_rev;
        if let Some((r, ix)) = self.finds.get(&id)
            && *r == rev
        {
            return Some(ix.clone());
        }
        if self.finding.get(&id) == Some(&rev) {
            return None;
        }
        let data = self.data.get(&(id, rev)).cloned()?;
        let LayerData::Vector(ds) = &data.data else { return None };
        let ds = ds.clone();
        self.finding.insert(id, rev);
        self.spawn_job(
            None,
            Box::new(move |w: &mut Workspace| {
                if w.finding.get(&id) == Some(&rev) {
                    w.finding.remove(&id);
                }
            }),
            move || {
                let ix = Arc::new(lm_jobs::find::FindIndex::build(&ds));
                Box::new(move |w: &mut Workspace| {
                    if w.finding.get(&id) == Some(&rev) {
                        w.finding.remove(&id);
                    }
                    w.finds.insert(id, (rev, ix));
                })
            },
        );
        None
    }

    /// Builds insight indexes in the background for layers whose data, filter or roles changed.
    fn index_layers(&mut self) {
        let wanted: Vec<(LayerId, String, lm_core::document::DocLayer)> =
            self.doc.layers.iter().filter(|l| l.profile.kind != LayerKind::Raster).map(|l| (l.id, Self::index_key(l), l.clone())).collect();
        self.indexes.retain(|id, _| wanted.iter().any(|(w, _, _)| w == id));
        self.finds.retain(|id, _| wanted.iter().any(|(w, _, _)| w == id));
        for (id, key, layer) in wanted {
            if self.indexes.get(&id).is_some_and(|(k, _)| *k == key) || self.indexing.get(&id) == Some(&key) {
                continue;
            }
            let Some(data) = self.data.get(&(id, layer.profile.data_rev)).cloned() else { continue };
            let LayerData::Vector(ds) = &data.data else { continue };
            let ds = ds.clone();
            self.indexing.insert(id, key.clone());
            let fail_key = key.clone();
            self.spawn_job(
                None,
                Box::new(move |w: &mut Workspace| {
                    if w.indexing.get(&id) == Some(&fail_key) {
                        w.indexing.remove(&id);
                    }
                }),
                move || {
                    let mask = lm_jobs::inspect::filter_mask(&ds, layer.filter.as_ref());
                    let ix = Arc::new(lm_jobs::inspect::build_index(&layer, &ds, mask.as_deref()));
                    Box::new(move |w: &mut Workspace| {
                        if w.indexing.get(&id) == Some(&key) {
                            w.indexing.remove(&id);
                        }
                        w.indexes.insert(id, (key, ix));
                    })
                },
            );
        }
    }

    /// Keeps layer data only for revisions the document or the undo stack can still reach, so
    /// undo memory stays bounded (WP3.8). Rasters are shared, never copied, either way.
    fn prune_data(&mut self) {
        // Nothing to prune while each layer holds one revision.
        if self.data.len() <= self.doc.layers.len() {
            return;
        }
        let mut keep: std::collections::HashSet<(LayerId, u32)> = self.doc.layers.iter().map(|l| (l.id, l.profile.data_rev)).collect();
        for tx in self.undo.transactions() {
            for op in &tx.ops {
                match op {
                    Op::AddLayer { layer, .. } | Op::RemoveLayer { layer, .. } => {
                        keep.insert((layer.id, layer.profile.data_rev));
                    }
                    Op::SetProfile { id, from, to } => {
                        keep.insert((*id, from.data_rev));
                        keep.insert((*id, to.data_rev));
                    }
                    _ => {}
                }
            }
        }
        // Revisions still being prepared (a re-read about to commit) stay too.
        for p in &self.pending {
            keep.insert((p.id, p.rev));
            keep.insert((p.id, p.base_rev));
        }
        self.data.retain(|k, _| keep.contains(k));
    }

    /// Starts working out colour classes and filter masks for layers whose key changed.
    fn derive_layers(&mut self) {
        let wanted: Vec<(LayerId, String, lm_core::document::DocLayer)> =
            self.doc.layers.iter().filter(|l| l.profile.kind != LayerKind::Raster).map(|l| (l.id, self.derive_key(l), l.clone())).collect();
        self.derived.retain(|id, _| wanted.iter().any(|(w, _, _)| w == id));
        for (id, key, layer) in wanted {
            if self.derived.get(&id).is_some_and(|d| d.key == key) || self.deriving.get(&id) == Some(&key) {
                continue;
            }
            let Some(data) = self.data.get(&(id, layer.profile.data_rev)).cloned() else { continue };
            let LayerData::Vector(ds) = &data.data else { continue };
            let ds = ds.clone();
            self.deriving.insert(id, key.clone());
            let view = self.view_extent;
            let fail_key = key.clone();
            self.spawn_job(
                None,
                Box::new(move |w: &mut Workspace| {
                    if w.deriving.get(&id) == Some(&fail_key) {
                        w.deriving.remove(&id);
                    }
                }),
                move || {
                    let mask = lm_jobs::inspect::filter_mask(&ds, layer.filter.as_ref());
                    let fit = layer.style.color_by.as_ref().is_some_and(|c| c.fit_to_view);
                    let in_view = view.filter(|_| fit);
                    let classes = lm_jobs::inspect::classes_for(&layer, &ds, mask.as_deref(), in_view.as_ref());
                    let symbols = classes.as_ref().map(|c| Arc::new(row_symbols(c, &layer.style, layer.profile.kind)));
                    // Placed records the filter keeps (rows without a location aren't on the map).
                    let shown = ds
                        .features
                        .iter()
                        .enumerate()
                        .filter(|(r, f)| f.geometry.is_some() && mask.as_ref().is_none_or(|m| m.get(*r).copied().unwrap_or(true)))
                        .count();
                    let d = Derived { key: key.clone(), classes: classes.map(Arc::new), mask: mask.map(Arc::new), shown, symbols };
                    Box::new(move |w: &mut Workspace| {
                        if w.deriving.get(&id) == Some(&key) {
                            w.deriving.remove(&id);
                        }
                        w.derived.insert(id, d);
                        w.order_key.clear();
                    })
                },
            );
        }
    }

    /// The extent of a layer, for Zoom to layer.
    pub fn extent(&self, id: LayerId) -> Option<Extent> {
        self.doc.layer(id)?.profile.extent_wgs84
    }

    /// Whether anything needs a decision (amber badge somewhere).
    pub fn needs_decision(&self) -> bool {
        self.doc.layers.iter().any(|l| l.profile.open_questions().next().is_some())
    }
}

/// The Compare view's converted points: vermillion (Okabe–Ito), larger, with a white edge.
fn compare_style() -> Style {
    let mut s = Style::for_kind(LayerKind::Points, [0xD5, 0x5E, 0x00, 0xFF], 0);
    s.point_size = 9.0;
    s
}

/// The line for a choice that changes how values are read but not the data itself.
fn line_for(kind: AssumptionKind, choice: &Choice) -> Option<String> {
    Some(match (kind, choice.id.as_str()) {
        (AssumptionKind::RasterCellMeaning, "measured") => "Treated as measured values (averaged, not summed).".into(),
        (AssumptionKind::RasterCellMeaning, "total") => "Treated as totals per cell (summed).".into(),
        (AssumptionKind::RasterCellMeaning, "category") => "Treated as categories (shares, not averages).".into(),
        (AssumptionKind::RasterCellMeaning, "not_sure") => {
            "Treated as measured values because the meaning is unconfirmed. Totals are hidden.".into()
        }
        (AssumptionKind::ChinaOffsetPossible | AssumptionKind::ChinaOffsetNamed, "wgs84") => {
            "Kept as GPS coordinates (WGS 84), as you chose.".into()
        }
        (AssumptionKind::ChinaOffsetPossible | AssumptionKind::ChinaOffsetNamed, "not_sure") => {
            "Points left where they are; they may sit a few hundred meters off if they came from a Chinese map app.".into()
        }
        (AssumptionKind::RasterNoUnit, _) => format!("Unit: {}.", choice.label),
        (_, "not_sure") => format!("Left as it is: {}.", choice.label.to_lowercase()),
        _ => format!("{}.", choice.label.trim_end_matches('.')),
    })
}

/// A saved layer's data derived again from a fresh read of its source, by the choices the
/// document records: another Excel sheet, other coordinate columns, a chosen coordinate system,
/// a GCJ-02/BD-09 conversion.
fn derive_saved(base: &ImportedLayer, saved: &lm_core::document::DocLayer) -> Result<ImportedLayer, String> {
    // A chosen text encoding needs the file read again in it; every other choice is re-applied.
    let encoding =
        saved.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ShapefileGbk && a.confirmed_by_user).map(|a| a.chosen.clone());
    let fresh = match encoding {
        Some(c) => lm_jobs::import::read_in_encoding(&saved.profile.source, &c).map_err(|e| e.to_string())?,
        None => base.clone(),
    };
    lm_jobs::import::reapply_choices(fresh, &saved.profile).map_err(|e| e.to_string())
}

/// Whether a layer can be drawn: located in GPS coordinates (or a raster with an extent).
pub fn drawable(p: &lm_core::model::LayerProfile) -> bool {
    p.kind != LayerKind::Table
        && p.extent_wgs84.is_some()
        && (p.kind == LayerKind::Raster || matches!(p.crs.value, Crs::Wgs84 | Crs::Epsg(_)))
}

fn rgba(c: [u8; 4]) -> lm_mapview::map::color::Rgba {
    lm_mapview::map::color::Rgba(c)
}

/// A map layer for imported data.
fn map_layer(map_id: u32, data: &ImportedLayer) -> lm_mapview::map::Layer {
    use lm_mapview::map::{Layer, RasterLayer};
    match &data.data {
        LayerData::Vector(ds) => {
            let mut l = Layer::new(map_id, (**ds).clone());
            l.labels.enabled = false;
            l.name = data.name.clone();
            l
        }
        LayerData::Raster(r) => {
            let mut rl = RasterLayer::from_path((**r).clone(), data.profile.source.path.clone());
            rl.data = Some(r.clone());
            Layer::new_raster(map_id, data.name.clone(), rl)
        }
    }
}

/// Per-row symbols from worked-out classes, in the style's symbol (area fill opacity, outlines).
fn row_symbols(c: &lm_jobs::inspect::RowClasses, s: &Style, kind: LayerKind) -> lm_mapview::map::RowSymbols {
    use lm_mapview::map::Symbol;
    let symbols = c
        .combos
        .iter()
        .map(|(color, size)| {
            let (fill, stroke) = match kind {
                LayerKind::Areas => {
                    let mut f = *color;
                    f[3] = (f32::from(f[3]) * s.fill_opacity.clamp(0.0, 1.0)).round() as u8;
                    (f, lm_core::document::darker(*color))
                }
                LayerKind::Lines => (*color, *color),
                _ => (*color, s.stroke),
            };
            let (size, stroke_width) = match kind {
                LayerKind::Points => (if *size > 0.0 { *size } else { s.point_size }, s.stroke_width),
                LayerKind::Lines => (0.0, s.line_width),
                _ => (0.0, s.stroke_width.max(1.0)),
            };
            Symbol { fill: rgba(fill), stroke: rgba(stroke), stroke_width, size, marker: Default::default() }
        })
        .collect();
    lm_mapview::map::RowSymbols { index: c.index.clone(), symbols }
}

/// Applies a document style to a map layer (Default symbols).
fn apply_style(l: &mut lm_mapview::map::Layer, s: &Style, kind: LayerKind) {
    use lm_mapview::map::{Renderer, Symbol};
    l.transparency = ((1.0 - s.opacity.clamp(0.0, 1.0)) * 100.0).round();
    if let Some(r) = l.raster.as_mut() {
        // A single band stretched through a ramp takes the document's palette, band, range and
        // display thresholds, as stops worked out from the band's statistics the same way the
        // renderer stretches (so the legend matches). Category, colour-table and RGB rasters
        // keep their own colours.
        let data = r.data.clone();
        if let lm_mapview::map::RasterRenderer::Stretch { band, ramp, invert } = &mut r.style.renderer
            && let Some(data) = data
            && let Some((stretch, stops)) = lm_jobs::inspect::raster_ramp(&data, s)
        {
            use lm_mapview::map::color::{CustomRamp, RampKind};
            let stops = stops.into_iter().map(|(x, c)| (Some(x), rgba(c))).collect();
            if let Ok(custom) = CustomRamp::new("LayeredMap", RampKind::Sequential, stops) {
                *band = lm_jobs::inspect::style_band(&data, s);
                *ramp = lm_mapview::map::Ramp::Custom(Box::new(custom));
                *invert = false;
                r.style.stretch = stretch;
            }
        }
        return;
    }
    let fill = match kind {
        LayerKind::Areas => {
            let mut c = s.fill;
            c[3] = (f32::from(c[3]) * s.fill_opacity.clamp(0.0, 1.0)).round() as u8;
            c
        }
        _ => s.fill,
    };
    let (size, stroke_width) = match kind {
        LayerKind::Points => (s.point_size, s.stroke_width),
        LayerKind::Lines => (0.0, s.line_width),
        _ => (0.0, s.stroke_width.max(1.0)),
    };
    let symbol = Symbol { fill: rgba(fill), stroke: rgba(s.stroke), stroke_width, size, marker: Default::default() };
    l.renderer = Renderer::Single(symbol);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
    }

    fn import(rel: &str) -> Vec<ImportedLayer> {
        lm_jobs::import::import_path(&fixture(rel), &lm_jobs::import::Ctx::detached(Default::default())).unwrap()
    }

    #[test]
    fn opening_a_file_is_one_undo_step_and_fits_an_empty_map() {
        let mut w = Workspace::default();
        w.fit_batches.push((1, None));
        assert!(w.add_imported(import("V2/mixed.geojson"), 1));
        assert_eq!(w.doc.layers.len(), 3);
        assert!(w.fit.is_some());
        assert_eq!(w.undo.undo_label(), Some("Open mixed.geojson"));
        w.undo();
        assert!(w.doc.layers.is_empty(), "one undo removes all three");
        w.redo();
        assert_eq!(w.doc.layers.len(), 3);
    }

    #[test]
    fn answering_the_column_question_relocates_and_undoes() {
        let mut w = Workspace::default();
        w.add_imported(import("T2/deliveries.csv"), 1);
        let id = w.doc.layers[0].id;
        let q = w.doc.layers[0].profile.open_questions().next().unwrap().clone();
        let pick = q.alternatives.iter().find(|c| c.id.starts_with("columns:")).unwrap().clone();
        w.change_assumption(id, q.kind, q.subject.clone(), pick, Some(3.0)).unwrap();
        w.wait_pending();
        assert!(w.doc.layers[0].profile.open_questions().next().is_none());
        assert_eq!(w.answers.len(), 1, "logged for the question budget");
        assert_eq!(w.data(id).unwrap().profile.data_rev, 1);
        w.undo();
        assert!(w.doc.layers[0].profile.open_questions().next().is_some(), "undo brings the question back");
        assert_eq!(w.data(id).unwrap().profile.data_rev, 0);
    }

    #[test]
    fn converting_amap_points_adds_a_gps_layer_and_hides_the_original() {
        let mut w = Workspace::default();
        w.add_imported(import("C1/amap_pois.csv"), 1);
        let id = w.doc.layers[0].id;
        let a = w.doc.layers[0].profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetNamed).unwrap().clone();
        w.change_assumption(id, a.kind, a.subject.clone(), Choice::new("gcj02", "Amap (GCJ-02)"), Some(2.0)).unwrap();
        w.wait_pending();
        let orig = w.doc.layer(id).unwrap();
        assert!(orig.profile.open_questions().next().is_none(), "the original's question is closed too");
        assert_eq!(w.doc.layers.len(), 2);
        assert!(!w.doc.layer(id).unwrap().visible);
        let new = w.doc.layers.iter().find(|l| l.id != id).unwrap();
        assert!(new.profile.notices.iter().any(|n| n == "Converted from GCJ-02 to WGS 84"), "{:?}", new.profile.notices);
        w.undo();
        assert_eq!(w.doc.layers.len(), 1);
        assert!(w.doc.layer(id).unwrap().visible);
    }
}

#[cfg(test)]
mod compare_tests {
    use super::*;

    #[test]
    fn compare_then_keep_converted_or_as_is() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/T1/schools.csv");
        let layers = lm_jobs::import::import_path(&path, &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        let mut w = Workspace::default();
        w.add_imported(layers, 1);
        let id = w.doc.layers[0].id;
        w.start_compare(id, false).unwrap();
        w.wait_pending();
        assert!(w.compare.as_ref().is_some_and(|c| c.map_pair.is_some()));
        w.end_compare(Some(false), None).unwrap();
        assert_eq!(w.doc.layers.len(), 1, "kept as is");
        let a = w.doc.layers[0].profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetPossible).unwrap();
        assert!(a.confirmed_by_user);
        w.start_compare(id, false).unwrap();
        w.wait_pending();
        w.end_compare(Some(true), None).unwrap();
        w.wait_pending();
        assert_eq!(w.doc.layers.len(), 2, "a converted GPS layer");
    }
}

#[cfg(test)]
mod sheet_tests {
    use super::*;

    #[test]
    fn choosing_another_sheet_is_one_undo_step() {
        let xlsx = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/T5/survey.xlsx");
        let layers = lm_jobs::import::import_path(&xlsx, &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        let mut w = Workspace::default();
        w.add_imported(layers, 1);
        let id = w.doc.layers[0].id;
        let sheet =
            |w: &Workspace| w.doc.layers[0].profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ExcelSheet).unwrap().clone();
        let a = sheet(&w);
        let other = a.alternatives.iter().find(|c| c.id.starts_with("sheet:") && c.id != a.chosen.id).unwrap().clone();
        w.change_assumption(id, a.kind, a.subject.clone(), other.clone(), None).unwrap();
        w.wait_pending();
        assert_eq!(sheet(&w).chosen.id, other.id);
        w.undo();
        assert_eq!(sheet(&w).chosen.id, a.chosen.id);
    }
}

#[cfg(test)]
mod race_tests {
    use super::*;

    /// Review finding (S1): two quick changes must not leave the document saying one thing and
    /// the data another. A second change while the first re-read runs is refused.
    #[test]
    fn a_second_change_waits_for_the_first() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/T2/deliveries.csv");
        let layers = lm_jobs::import::import_path(&path, &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        let mut w = Workspace::default();
        w.add_imported(layers, 1);
        let id = w.doc.layers[0].id;
        let q = w.doc.layers[0].profile.open_questions().next().unwrap().clone();
        let picks: Vec<Choice> = q.alternatives.iter().filter(|c| c.id.starts_with("columns:")).cloned().collect();
        assert!(picks.len() >= 2);
        w.change_assumption(id, q.kind, q.subject.clone(), picks[0].clone(), None).unwrap();
        let second = w.change_assumption(id, q.kind, q.subject.clone(), picks[1].clone(), None);
        assert!(second.is_err(), "refused while the first re-read runs");
        w.wait_pending();
        let l = w.doc.layer(id).unwrap();
        let chosen = l.profile.assumptions.iter().find(|a| a.kind == q.kind).unwrap().chosen.id.clone();
        assert_eq!(chosen, picks[0].id);
        assert_eq!(w.data(id).unwrap().profile.data_rev, l.profile.data_rev, "document and data agree");
    }
}

#[cfg(test)]
mod restore_tests {
    use super::*;

    /// Review finding (S1): reopening a saved document must restore it as saved.
    #[test]
    fn a_saved_document_reopens_as_saved() {
        let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let open = |f: &str| lm_jobs::import::import_path(&fx.join(f), &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        let mut w = Workspace::default();
        w.add_imported(open("T2/deliveries.csv"), 1);
        w.add_imported(open("C1/amap_pois.csv"), 2);
        w.add_imported(open("V4/districts_gk.shp"), 3);
        w.add_imported(open("V2/mixed.geojson"), 4);
        let id_of = |w: &Workspace, name: &str| w.doc.layers.iter().find(|l| l.display_name == name).unwrap().id;
        // Answer T2's question, convert C1, place V4, rename and hide one layer.
        let t2 = id_of(&w, "deliveries");
        let q = w.doc.layer(t2).unwrap().profile.open_questions().next().unwrap().clone();
        let pick = q.alternatives.iter().find(|c| c.id.starts_with("columns:")).unwrap().clone();
        w.change_assumption(t2, q.kind, q.subject, pick, None).unwrap();
        let c1 = id_of(&w, "amap_pois");
        let a = w.doc.layer(c1).unwrap().profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetNamed).unwrap().clone();
        w.change_assumption(c1, a.kind, a.subject, Choice::new("gcj02", "Amap"), None).unwrap();
        w.wait_pending();
        let v4 = id_of(&w, "districts_gk");
        w.place_layer(v4, 4549, None).unwrap();
        w.wait_pending();
        let tx = w.doc.rename_tx(t2, "my deliveries");
        w.commit(tx);
        let saved = w.doc.clone();
        let dir = std::env::temp_dir().join(format!("lm-restore-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        lm_doc::save(&saved, &dir).unwrap();
        let loaded = lm_doc::load(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        let mut r = Workspace::default();
        r.restore(loaded);
        r.wait_pending();
        assert_eq!(r.doc.layers.len(), saved.layers.len(), "no layer duplicated");
        for (a, b) in r.doc.layers.iter().zip(&saved.layers) {
            assert_eq!((a.id, &a.display_name, a.visible), (b.id, &b.display_name, b.visible));
            let d = r.data(a.id).unwrap_or_else(|| panic!("{} has no data", a.display_name));
            let orig = w.data(b.id).unwrap();
            let (LayerData::Vector(x), LayerData::Vector(y)) = (&d.data, &orig.data) else { continue };
            assert_eq!(x.features.len(), y.features.len(), "{}", a.display_name);
            let first = |ds: &lm_jobs::import::Dataset| ds.features.iter().find_map(|f| f.geometry.as_ref().map(|g| g.extent()));
            let (ex, ey) = (first(x).unwrap(), first(y).unwrap());
            assert!(
                (ex.min_x - ey.min_x).abs() < 1e-9 && (ex.min_y - ey.min_y).abs() < 1e-9,
                "{} lands where it was saved",
                a.display_name
            );
        }
        assert!(r.doc.layers.iter().any(|l| l.display_name == "my deliveries"));
    }
}

#[cfg(test)]
mod second_review_tests {
    use super::*;

    fn open(rel: &str) -> Vec<ImportedLayer> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel);
        lm_jobs::import::import_path(&p, &lm_jobs::import::Ctx::detached(Default::default())).unwrap()
    }

    fn extent(w: &Workspace, id: LayerId) -> Extent {
        w.doc.layer(id).unwrap().profile.extent_wgs84.unwrap()
    }

    /// N3: placing a layer, asking to place it again and choosing the same system lands it in
    /// the same place (never projected twice).
    #[test]
    fn placing_again_starts_from_the_stored_coordinates() {
        let mut w = Workspace::default();
        w.add_imported(open("V4/districts_gk.shp"), 1);
        let id = w.doc.layers[0].id;
        w.place_layer(id, 4549, None).unwrap();
        w.wait_pending();
        let first = extent(&w, id);
        w.change_assumption(
            id,
            AssumptionKind::NoCrsProjected,
            Subject::Layer,
            Choice::new("find_place", "Choose where it should be…"),
            None,
        )
        .unwrap();
        w.wait_pending();
        assert!(w.doc.layer(id).unwrap().profile.open_questions().next().is_some(), "the question is back");
        w.place_layer(id, 4549, None).unwrap();
        w.wait_pending();
        let again = extent(&w, id);
        assert!((first.min_x - again.min_x).abs() < 1e-9 && (first.min_y - again.min_y).abs() < 1e-9, "{first:?} vs {again:?}");
    }

    /// N2: reopening applies raster and encoding choices to the data, not only to the lines.
    #[test]
    fn reopening_keeps_raster_and_encoding_choices() {
        let mut w = Workspace::default();
        w.add_imported(open("V5/places_gbk.shp"), 1);
        w.add_imported(open("R6/scaled.tif"), 2);
        let v5 = w.doc.layers.iter().find(|l| l.profile.kind != LayerKind::Raster).unwrap().id;
        let r6 = w.doc.layers.iter().find(|l| l.profile.kind == LayerKind::Raster).unwrap().id;
        w.change_assumption(v5, AssumptionKind::ShapefileGbk, Subject::Layer, Choice::new("utf8", "Unicode (UTF-8)"), None).unwrap();
        w.wait_pending();
        let nd = w.doc.layer(r6).unwrap().profile.assumptions.iter().find(|a| a.kind == AssumptionKind::RasterNoData).unwrap().clone();
        w.change_assumption(r6, nd.kind, nd.subject, Choice::new("value", "a real value"), None).unwrap();
        w.wait_pending();
        let doc = w.doc.clone();
        let mut r = Workspace::default();
        r.restore(doc);
        r.wait_pending();
        let LayerData::Raster(raster) = &r.data(r6).unwrap().data else { panic!() };
        assert_eq!(raster.nodata, None, "the −9999 border is a real value, as chosen");
        let LayerData::Vector(ds) = &r.data(v5).unwrap().data else { panic!() };
        let name = ds.field_index("name").unwrap();
        assert_ne!(ds.value(0, name), &lm_jobs::import::Value::Text("人民广场".into()), "read as UTF-8, as chosen");
        assert_eq!(r.data(v5).unwrap().profile.encoding.as_deref(), Some("UTF-8"));
    }

    /// N6: the GPS copy doesn't offer to convert again; the original's line says what happened.
    #[test]
    fn a_converted_layer_does_not_offer_to_convert_again() {
        let mut w = Workspace::default();
        w.add_imported(open("C1/amap_pois.csv"), 1);
        let id = w.doc.layers[0].id;
        let a = w.doc.layers[0].profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetNamed).unwrap().clone();
        w.change_assumption(id, a.kind, a.subject, Choice::new("gcj02", "Amap"), None).unwrap();
        w.wait_pending();
        let gps = w.doc.layers.iter().find(|l| l.id != id).unwrap();
        assert!(
            !gps.profile
                .assumptions
                .iter()
                .any(|a| matches!(a.kind, AssumptionKind::ChinaOffsetNamed | AssumptionKind::ChinaOffsetPossible))
        );
        let orig =
            w.doc.layer(id).unwrap().profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetNamed).unwrap().clone();
        assert!(orig.line.starts_with("Converted to GPS coordinates"), "{}", orig.line);
    }
}

#[cfg(test)]
mod third_review_tests {
    use super::*;

    fn open(rel: &str) -> Vec<ImportedLayer> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel);
        lm_jobs::import::import_path(&p, &lm_jobs::import::Ctx::detached(Default::default())).unwrap()
    }

    /// Not sure on a placed layer opens it as a table (not drawn).
    #[test]
    fn not_sure_on_a_placed_layer_opens_it_as_a_table() {
        let mut w = Workspace::default();
        w.add_imported(open("V4/districts_gk.shp"), 1);
        let id = w.doc.layers[0].id;
        w.place_layer(id, 4549, None).unwrap();
        w.wait_pending();
        w.change_assumption(id, AssumptionKind::NoCrsProjected, Subject::Layer, Choice::not_sure(), None).unwrap();
        w.wait_pending();
        let l = w.doc.layer(id).unwrap();
        assert_eq!(l.profile.kind, LayerKind::Table);
        assert!(!drawable(&l.profile));
    }

    /// A converted GPS copy stays converted after its text encoding is changed.
    #[test]
    fn a_gps_copy_stays_converted_after_an_encoding_change() {
        let dir = std::env::temp_dir().join(format!("lm-amap-shp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/V5");
        for ext in ["shp", "shx", "dbf"] {
            std::fs::copy(src.join(format!("places_gbk.{ext}")), dir.join(format!("amap_places.{ext}"))).unwrap();
        }
        let layers =
            lm_jobs::import::import_path(&dir.join("amap_places.shp"), &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        let mut w = Workspace::default();
        w.add_imported(layers, 1);
        let id = w.doc.layers[0].id;
        let a = w.doc.layers[0]
            .profile
            .assumptions
            .iter()
            .find(|a| a.kind == AssumptionKind::ChinaOffsetNamed)
            .cloned()
            .expect("the amap name hint asks");
        w.change_assumption(id, a.kind, a.subject, Choice::new("gcj02", "Amap"), None).unwrap();
        w.wait_pending();
        let gps = w.doc.layers.iter().find(|l| l.id != id).unwrap().id;
        let before = w.doc.layer(gps).unwrap().profile.extent_wgs84.unwrap();
        w.change_assumption(gps, AssumptionKind::ShapefileGbk, Subject::Layer, Choice::new("utf8", "Unicode (UTF-8)"), None).unwrap();
        w.wait_pending();
        let p = &w.doc.layer(gps).unwrap().profile;
        assert!(p.notices.iter().any(|n| n.starts_with("Converted from")), "{:?}", p.notices);
        assert!(p.open_questions().next().is_none());
        let after = p.extent_wgs84.unwrap();
        assert!((before.min_x - after.min_x).abs() < 1e-9, "still converted: {before:?} vs {after:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
