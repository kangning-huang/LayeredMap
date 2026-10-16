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
    rev: u32,
    fit: bool,
    rx: std::sync::mpsc::Receiver<Result<ImportedLayer, String>>,
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
    data: Arc<ImportedLayer>,
    /// Where to look: a point of the layer, at street level.
    pub focus: (f64, f64),
}

/// The map id of the Compare view's converted points.
const COMPARE_MAP_ID: u32 = MAP_ID_BASE - 1;

/// Map layer ids for LayeredMap's layers start here, clear of anything the map assigns itself.
const MAP_ID_BASE: u32 = 1_000_000;

#[derive(Default)]
pub struct Workspace {
    pub doc: Document,
    pub undo: UndoStack,
    data: HashMap<(LayerId, u32), Arc<ImportedLayer>>,
    pub imports: Vec<ImportRow>,
    pub selected: Option<LayerId>,
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
    /// Wakes the interface when background work finishes (set by the shell).
    pub wake: Option<Arc<dyn Fn() + Send + Sync>>,
    compare_synced: Option<(LayerId, bool)>,
}

impl Workspace {
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
            match result {
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
        if self.selected.is_none() {
            self.selected = added.first().map(|(id, _)| *id);
        }
        true
    }

    pub fn commit(&mut self, tx: Option<Transaction>) -> bool {
        match tx {
            Some(tx) => self.undo.commit(&mut self.doc, tx).is_ok(),
            None => false,
        }
    }

    pub fn undo(&mut self) -> Option<String> {
        self.undo.undo(&mut self.doc)
    }

    pub fn redo(&mut self) -> Option<String> {
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
            AssumptionKind::CsvCoordinateColumns | AssumptionKind::CsvAmbiguousColumns | AssumptionKind::NoCrsProjected
                if choice.id.starts_with("columns:") || choice.id == "not_sure" =>
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
            _ => self.record(id, kind, subject, choice, label),
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
        let rev = self.doc.layer(id).map_or(0, |l| l.profile.data_rev) + 1;
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
            self.pending.push(PendingEdit { id, label, rev, fit, rx });
        }
    }

    /// Whether a layer is being re-read (its row shows the working badge).
    pub fn is_busy(&self, id: LayerId) -> bool {
        self.pending.iter().any(|p| p.id == id)
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
            match r {
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
        while !self.pending.is_empty() {
            for e in self.poll_pending() {
                self.notice = Some(e);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// Records a changed choice (and marks it confirmed) without re-reading data.
    fn record(&mut self, id: LayerId, kind: AssumptionKind, subject: Subject, choice: Choice, label: String) -> Result<(), String> {
        let tx = self.doc.set_assumption_tx(id, kind, subject, choice).map(|mut t| {
            t.label = label;
            t
        });
        if self.commit(tx) { Ok(()) } else { Err("That assumption couldn't be changed.".into()) }
    }

    /// Converts a layer's points from GCJ-02 or BD-09 to GPS coordinates as a new layer beside it;
    /// the original is hidden in the same step, and the source file is untouched (WP2.5).
    fn convert_offset(&mut self, id: LayerId, choice: &Choice, kind: AssumptionKind, subject: Subject) -> Result<(), String> {
        let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
        let LayerData::Vector(ds) = &data.data else { return Err("Only points, lines and areas can be converted.".into()) };
        let converted = lm_jobs::import::convert_offset(ds, choice.id == "bd09").map_err(|e| e.to_string())?;
        let Some(orig) = self.doc.layer(id).cloned() else { return Err("That layer is gone.".into()) };
        let mut profile = orig.profile.clone();
        profile.crs = lm_core::model::Known::certain(Crs::Wgs84, converted.1.clone());
        profile.notices.push(converted.1.clone());
        for a in profile.assumptions.iter_mut().filter(|a| a.kind == kind && a.subject == subject) {
            a.chosen = choice.clone();
            a.confirmed_by_user = true;
        }
        profile.data_rev = 0;
        let mut ds = converted.0;
        ds.source = None;
        let e = ds.extent();
        profile.extent_wgs84 = e.min_x.is_finite().then_some(Extent { min_x: e.min_x, min_y: e.min_y, max_x: e.max_x, max_y: e.max_y });
        let mut add = self.doc.add_layer_tx(profile.clone());
        let mut new_id = None;
        if let Some(Op::AddLayer { layer: l, .. }) = add.ops.first_mut() {
            l.display_name = format!("{} (GPS)", orig.display_name);
            l.style = orig.style.clone();
            new_id = Some(l.id);
        }
        let Some(new_id) = new_id else { return Err("The converted layer couldn't be added.".into()) };
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
        let tx = Transaction::new(format!("Convert {} to GPS coordinates", orig.display_name), ops);
        if self.commit(Some(tx)) {
            self.selected = Some(new_id);
            Ok(())
        } else {
            Err("The converted layer couldn't be added.".into())
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
        if let Some(secs) = seconds {
            let name = lm_jobs::import::crs_name(epsg);
            self.answers.push(QuestionAnswer {
                kind: AssumptionKind::NoCrsProjected,
                answer: Choice::new(format!("epsg:{epsg}"), name),
                seconds: secs,
            });
        }
        let wake = self.wake.clone();
        self.spawn_edit(id, format!("Place {}", layer.display_name), true, wake, move || {
            lm_jobs::import::reproject(&data, u32::from(epsg)).map_err(|e| e.to_string())
        });
        Ok(())
    }

    /// Starts comparing a layer's points with the same points converted from GCJ-02 (or BD-09).
    pub fn start_compare(&mut self, id: LayerId, from_bd09: bool) -> Result<(), String> {
        let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
        let LayerData::Vector(ds) = &data.data else { return Err("Only points, lines and areas can be compared.".into()) };
        let (converted, recipe) = lm_jobs::import::convert_offset(ds, from_bd09).map_err(|e| e.to_string())?;
        let mut focus = None;
        if let Some(g) = ds.features.iter().find_map(|f| f.geometry.as_ref()) {
            g.for_each_coord(|c| {
                if focus.is_none() {
                    focus = Some((c.x, c.y));
                }
            });
        }
        let focus = focus.ok_or("This layer has nothing to compare.")?;
        let mut profile = data.profile.clone();
        profile.notices = vec![recipe];
        let preview = ImportedLayer {
            name: format!("{} (converted)", data.name),
            profile,
            data: LayerData::Vector(Arc::new(converted)),
            status: ImportStatus::Complete,
            raw: None,
        };
        self.compare = Some(Compare { layer: id, from_bd09, data: Arc::new(preview), focus });
        Ok(())
    }

    /// Ends the Compare view: keep the converted points (a new GPS layer), keep the points as
    /// they are (the assumption is confirmed), or just close it.
    pub fn end_compare(&mut self, keep: Option<bool>) -> Result<(), String> {
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
                self.change_assumption(c.layer, a.kind, a.subject, choice, None)
            }
            Some(false) => {
                let choice = a
                    .alternatives
                    .iter()
                    .find(|x| x.id == "wgs84")
                    .cloned()
                    .unwrap_or_else(|| Choice::new("wgs84", "GPS coordinates (WGS 84): keep where they are"));
                self.change_assumption(c.layer, a.kind, a.subject, choice, None)
            }
            None => Ok(()),
        }
    }

    /// Brings the map's layers in line with the document: adds and removes only what changed,
    /// then sets order, visibility and style in one cheap edit.
    pub fn sync_map(&mut self, view: &mut lm_mapview::MapView) {
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
        // Add what's new.
        for (id, rev) in &wanted {
            if self.synced.iter().any(|s| s.id == *id && s.rev == *rev) {
                continue;
            }
            let Some(data) = self.data.get(&(*id, *rev)).cloned() else { continue };
            let map_id = MAP_ID_BASE + (id.0 as u32).wrapping_mul(64).wrapping_add(rev % 64);
            let layer = map_layer(map_id, &data);
            view.edit_map(move |m: &mut Map| {
                m.layers.retain(|l| l.id != map_id);
                m.layers.push(layer.clone());
            });
            self.synced.push(Synced { id: *id, map_id, rev: *rev });
        }
        // The Compare view's converted points.
        let want_compare = self.compare.as_ref().map(|c| (c.layer, c.from_bd09));
        if want_compare != self.compare_synced {
            let layer = self.compare.as_ref().map(|c| map_layer(COMPARE_MAP_ID, &c.data));
            view.edit_map(move |m: &mut Map| {
                m.layers.retain(|l| l.id != COMPARE_MAP_ID);
                if let Some(l) = &layer {
                    m.layers.insert(0, l.clone());
                }
            });
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
        let key = format!("{order:?}");
        if self.order_key != key {
            self.order_key = key;
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
                        m.layers.push(l);
                    }
                }
                m.layers.extend(others);
            });
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

/// Applies a document style to a map layer (Default symbols).
fn apply_style(l: &mut lm_mapview::map::Layer, s: &Style, kind: LayerKind) {
    use lm_mapview::map::{Renderer, Symbol};
    l.transparency = ((1.0 - s.opacity.clamp(0.0, 1.0)) * 100.0).round();
    if let Some(r) = l.raster.as_mut() {
        // A single band stretched through a ramp takes the document's ramp (Data palettes:
        // viridis by default for measures). Category, colour-table and RGB rasters keep their own.
        if let lm_mapview::map::RasterRenderer::Stretch { ramp, invert, .. } = &mut r.style.renderer {
            *ramp = match s.ramp.as_str() {
                "heat" => lm_mapview::map::Ramp::Thermal,
                "blues" => lm_mapview::map::Ramp::Lagoon,
                "greys" => lm_mapview::map::Ramp::Greys,
                // viridis, cividis: Aurora is the engine's perceptually even indigo–teal–yellow ramp.
                _ => lm_mapview::map::Ramp::Aurora,
            };
            *invert = false;
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
        assert!(w.compare.is_some());
        w.end_compare(Some(false)).unwrap();
        assert_eq!(w.doc.layers.len(), 1, "kept as is");
        let a = w.doc.layers[0].profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetPossible).unwrap();
        assert!(a.confirmed_by_user);
        w.start_compare(id, false).unwrap();
        w.end_compare(Some(true)).unwrap();
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
