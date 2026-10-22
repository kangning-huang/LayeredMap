//! The job system (WP4.3): a worker pool that runs analysis against immutable input snapshots,
//! cooperative cancellation, progress, the stale-result guard and the plumbing for an atomic
//! commit. Phase 4 actions are started by the Pro assistant and the developer surface, never by
//! buttons in the free app; this module doesn't care who submits.
//!
//! How the pieces fit, seen from the interface thread:
//! - [`Pool`] owns the worker threads. [`Pool::submit`] queues a job and returns a [`JobHandle`].
//!   A job is a closure over `Arc` data handles (the snapshot): it never sees the live document.
//! - A job gets a [`JobCtx`]: the shared [`Control`] (cancel + progress, see `lm_analysis::control`)
//!   and a per-job temporary directory that is removed when the job ends, however it ends.
//! - [`JobHandle::poll`] never blocks: `None` while running, then exactly one [`JobOutcome`].
//!   A cancelled job's value is never delivered, even if it finished just before the cancel.
//! - [`PlanSlot`] is one preview slot (one per composer or assistant plan): each submit bumps the
//!   slot's generation and cancels the previous job, and only the current generation's result
//!   is delivered, so a stale preview can never appear.
//! - Jobs build their output in memory and hand it over at the end as an [`Output`] (new layers
//!   with their data, the result card): [`Output::commit`] puts layers, styles and the undo
//!   entry in one [`Transaction`]. A cancelled job hands over nothing, so there is nothing partial.
//! - [`estimate`] predicts the run time from the inputs' size, to decide automatic preview
//!   (under 1 s) and to warn before a job that will take more than 30 s.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use web_time::{Duration, Instant};

pub use lm_analysis::control::{Cancelled, Control, Progress};
use lm_core::document::{ApplyError, DocLayer, Document, Op, Style, Transaction};
use lm_core::model::{LayerId, LayerProfile};
use lm_core::undo::UndoStack;

/// Locks a mutex, carrying on with the data if a job panicked while holding it (jobs run under
/// `catch_unwind`; the guarded state is always left consistent).
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Asks the interface to repaint (egui's `request_repaint`), from any thread.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Unique within a pool, in submission order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

/// Why a job's closure stopped without a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobError {
    /// It saw the cancel flag (`control.check()?`).
    Cancelled,
    /// It could not be done; the message is shown to the user as is.
    Failed(String),
}

impl From<Cancelled> for JobError {
    fn from(_: Cancelled) -> Self {
        JobError::Cancelled
    }
}

/// How a job ended, as delivered to the interface thread (once).
#[derive(Clone, Debug, PartialEq)]
pub enum JobOutcome<T> {
    Done(T),
    /// Nothing was made; the message is for the user. A panic inside a job arrives here.
    Failed(String),
    /// Cancelled: whatever it made was dropped.
    Cancelled,
}

/// The message a job that panicked is reported with.
pub const PANIC_MESSAGE: &str = "That couldn't be done. Nothing was changed.";

/// What a running job can use: the cancel flag and progress sink, and a scratch directory.
pub struct JobCtx {
    pub control: Control,
    id: JobId,
    temp_root: PathBuf,
    temp: Mutex<Option<PathBuf>>,
}

impl JobCtx {
    pub fn id(&self) -> JobId {
        self.id
    }

    /// Err when cancelled: call it at every chunk boundary (at least every 50 ms).
    pub fn check(&self) -> Result<(), Cancelled> {
        self.control.check()
    }

    /// Reports the phase in plain words and the amount done; shown verbatim.
    pub fn report(&self, phase: &str, done: u64, total: Option<u64>) {
        self.control.report(phase, done, total);
    }

    /// This job's own scratch directory, created on first use and removed with everything in it
    /// when the job ends (done, failed, cancelled or panicked). Nothing in it outlives the job:
    /// what must be kept goes in the returned value.
    pub fn temp_dir(&self) -> std::io::Result<PathBuf> {
        let mut t = lock(&self.temp);
        if let Some(p) = t.as_ref() {
            return Ok(p.clone());
        }
        let p = self.temp_root.join(format!("job-{}", self.id.0));
        std::fs::create_dir_all(&p)?;
        *t = Some(p.clone());
        Ok(p)
    }
}

impl Drop for JobCtx {
    fn drop(&mut self) {
        if let Some(p) = lock(&self.temp).take()
            && let Err(e) = std::fs::remove_dir_all(&p)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("could not remove the job's temporary folder {}: {e}", p.display());
        }
    }
}

enum State<T> {
    Queued,
    Running,
    Done(JobOutcome<T>),
    /// The outcome was handed to the interface.
    Taken,
}

#[derive(Default)]
struct ProgressLog {
    latest: Option<Progress>,
    /// Distinct phases in the order first reported (consecutive repeats folded), capped.
    phases: Vec<String>,
    last_wake: Option<Instant>,
}

const MAX_PHASES: usize = 64;
/// Progress wakes the interface at most this often (a phase change always wakes it).
const PROGRESS_WAKE_EVERY: Duration = Duration::from_millis(33);

struct Shared<T> {
    state: Mutex<State<T>>,
    finished: Condvar,
    progress: Mutex<ProgressLog>,
}

impl<T> Shared<T> {
    fn finish(&self, outcome: JobOutcome<T>, control: &Control) {
        let mut s = lock(&self.state);
        // A cancel that raced with the end of the job still wins: its value is dropped here.
        let outcome = if control.is_cancelled() { JobOutcome::Cancelled } else { outcome };
        *s = State::Done(outcome);
        drop(s);
        self.finished.notify_all();
    }
}

/// A submitted job, held by the interface thread. Dropping it cancels the job.
pub struct JobHandle<T> {
    id: JobId,
    generation: u64,
    control: Control,
    shared: Arc<Shared<T>>,
}

impl<T> std::fmt::Debug for JobHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandle").field("id", &self.id).field("generation", &self.generation).field("control", &self.control).finish()
    }
}

impl<T> JobHandle<T> {
    pub fn id(&self) -> JobId {
        self.id
    }

    /// The plan generation this job was submitted for (0 outside a [`PlanSlot`]).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The job's control (its cancel flag is shared with the work).
    pub fn control(&self) -> &Control {
        &self.control
    }

    /// Asks the job to stop at its next check. From now on [`JobHandle::poll`] never gives
    /// `Done`.
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// The latest progress, without blocking on the job.
    pub fn progress(&self) -> Option<Progress> {
        lock(&self.shared.progress).latest.clone()
    }

    /// The phases reported so far, verbatim, in order.
    pub fn phases(&self) -> Vec<String> {
        lock(&self.shared.progress).phases.clone()
    }

    /// The outcome once, when the job has ended; `None` while queued or running, and after the
    /// outcome was taken. Never blocks on the job (the lock is held only to swap the state).
    pub fn poll(&self) -> Option<JobOutcome<T>> {
        let mut s = lock(&self.shared.state);
        match &*s {
            State::Queued | State::Running | State::Taken => None,
            State::Done(_) => match std::mem::replace(&mut *s, State::Taken) {
                State::Done(_) if self.control.is_cancelled() => Some(JobOutcome::Cancelled),
                State::Done(o) => Some(o),
                _ => None,
            },
        }
    }

    /// True once the worker is done with the job: its temporary folder is gone and its
    /// outcome is (or was) available.
    pub fn is_finished(&self) -> bool {
        matches!(&*lock(&self.shared.state), State::Done(_) | State::Taken)
    }

    /// Waits until the job has ended, at most `timeout`. For tests, the developer surface and
    /// shutdown; never call it on the interface thread.
    pub fn wait_finished(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut s = lock(&self.shared.state);
        loop {
            if matches!(&*s, State::Done(_) | State::Taken) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            s = self.shared.finished.wait_timeout(s, deadline - now).unwrap_or_else(PoisonError::into_inner).0;
        }
    }
}

impl<T> Drop for JobHandle<T> {
    fn drop(&mut self) {
        // Nobody can receive the result any more: stop the work.
        self.control.cancel();
    }
}

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Jobs queued or running, to wait for quiet (tests, shutdown).
#[derive(Default)]
struct Active {
    count: Mutex<usize>,
    zero: Condvar,
}

impl Active {
    fn add(&self) {
        *lock(&self.count) += 1;
    }

    fn done(&self) {
        let mut c = lock(&self.count);
        *c = c.saturating_sub(1);
        if *c == 0 {
            self.zero.notify_all();
        }
    }
}

/// The worker threads. One per app; jobs are independent (a job must never wait for another
/// job's result, which is what keeps the pool free of deadlocks).
pub struct Pool {
    tx: Mutex<Option<Sender<Task>>>,
    workers: Vec<JoinHandle<()>>,
    next_id: AtomicU64,
    temp_root: PathBuf,
    wake: Option<Wake>,
    active: Arc<Active>,
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool").field("workers", &self.workers.len()).field("temp_root", &self.temp_root).finish()
    }
}

/// The default pool size: the available parallelism, at least 2.
pub fn default_workers() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(2)
}

impl Pool {
    /// A pool of [`default_workers`] threads whose jobs' scratch folders go under the system
    /// temporary directory (`layeredmap-jobs/<process id>`).
    pub fn new(wake: Option<Wake>) -> Pool {
        // The browser has no temporary folder or process id (jobs there make no scratch files).
        #[cfg(target_arch = "wasm32")]
        let root = PathBuf::from("/layeredmap-jobs");
        #[cfg(not(target_arch = "wasm32"))]
        let root = std::env::temp_dir().join("layeredmap-jobs").join(std::process::id().to_string());
        Pool::with(default_workers(), root, wake)
    }

    /// A pool of `workers` threads (at least 1) with scratch folders under `temp_root`, which is
    /// removed when the pool is dropped.
    pub fn with(workers: usize, temp_root: PathBuf, wake: Option<Wake>) -> Pool {
        let (tx, rx) = channel::<Task>();
        let rx = Arc::new(Mutex::new(rx));
        let mut handles = Vec::new();
        // The browser build has no threads: jobs are queued on `lm_core::task` instead (`submit_for`).
        let workers = if cfg!(target_arch = "wasm32") { 0 } else { workers.max(1) };
        for i in 0..workers {
            let rx = rx.clone();
            match std::thread::Builder::new().name(format!("lm-job-{i}")).spawn(move || worker(&rx)) {
                Ok(h) => handles.push(h),
                Err(e) => log::error!("could not start job worker {i}: {e}"),
            }
        }
        Pool {
            tx: Mutex::new(Some(tx)),
            workers: handles,
            next_id: AtomicU64::new(1),
            temp_root,
            wake,
            active: Arc::new(Active::default()),
        }
    }

    pub fn workers(&self) -> usize {
        self.workers.len()
    }

    pub fn temp_root(&self) -> &Path {
        &self.temp_root
    }

    /// Queues a job outside any plan slot (generation 0).
    pub fn submit<T, F>(&self, job: F) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(&JobCtx) -> Result<T, JobError> + Send + 'static,
    {
        self.submit_for(0, job)
    }

    /// Queues a job for a plan generation. The closure must own its inputs (Arc snapshots).
    pub fn submit_for<T, F>(&self, generation: u64, job: F) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(&JobCtx) -> Result<T, JobError> + Send + 'static,
    {
        let id = JobId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let shared =
            Arc::new(Shared { state: Mutex::new(State::Queued), finished: Condvar::new(), progress: Mutex::new(ProgressLog::default()) });
        let control = {
            let shared = shared.clone();
            let wake = self.wake.clone();
            Control::with_progress(move |p: Progress| {
                let mut log = lock(&shared.progress);
                let new_phase = log.phases.last() != Some(&p.phase);
                if new_phase && log.phases.len() < MAX_PHASES {
                    log.phases.push(p.phase.clone());
                }
                log.latest = Some(p);
                let now = Instant::now();
                let due = new_phase || log.last_wake.is_none_or(|t| now.duration_since(t) >= PROGRESS_WAKE_EVERY);
                if due {
                    log.last_wake = Some(now);
                }
                drop(log);
                if due && let Some(w) = &wake {
                    w();
                }
            })
        };
        let handle = JobHandle { id, generation, control: control.clone(), shared: shared.clone() };
        let ctx = JobCtx { control: control.clone(), id, temp_root: self.temp_root.clone(), temp: Mutex::new(None) };
        let wake = self.wake.clone();
        let active = self.active.clone();
        let task: Task = Box::new(move || {
            let outcome = if control.is_cancelled() {
                drop(ctx);
                JobOutcome::Cancelled
            } else {
                *lock(&shared.state) = State::Running;
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&ctx)));
                // The scratch folder goes before the outcome is visible.
                drop(ctx);
                match r {
                    Ok(Ok(v)) => JobOutcome::Done(v),
                    Ok(Err(JobError::Cancelled)) => JobOutcome::Cancelled,
                    Ok(Err(JobError::Failed(m))) => JobOutcome::Failed(m),
                    Err(_) => {
                        log::error!("job {} panicked", id.0);
                        JobOutcome::Failed(PANIC_MESSAGE.into())
                    }
                }
            };
            shared.finish(outcome, &control);
            active.done();
            if let Some(w) = wake {
                w();
            }
        });
        self.active.add();
        #[cfg(target_arch = "wasm32")]
        let sent = lock(&self.tx).as_ref().map(|_| lm_core::task::spawn("lm-job", task));
        #[cfg(not(target_arch = "wasm32"))]
        let sent = lock(&self.tx).as_ref().map(|tx| tx.send(task));
        if !matches!(sent, Some(Ok(()))) {
            // No workers (shutting down): the job ends at once as failed.
            handle.shared.finish(JobOutcome::Failed("The job system has stopped.".into()), &handle.control);
            self.active.done();
        }
        handle
    }

    /// Waits until no job is queued or running, at most `timeout`. Not for the interface thread.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut c = lock(&self.active.count);
        while *c > 0 {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            c = self.active.zero.wait_timeout(c, deadline - now).unwrap_or_else(PoisonError::into_inner).0;
        }
        true
    }
}

fn worker(rx: &Mutex<Receiver<Task>>) {
    loop {
        let task = lock(rx).recv();
        match task {
            Ok(t) => t(),
            Err(_) => return,
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        // Closing the queue ends each worker after the jobs already queued; their handles were
        // cancelled when dropped, so those end at their first check.
        lock(&self.tx).take();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
        let _ = std::fs::remove_dir_all(&self.temp_root);
    }
}

/// A plan slot's result, delivered once and only for the slot's current generation.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivered<T> {
    pub job: JobId,
    pub generation: u64,
    pub outcome: JobOutcome<T>,
}

/// One preview slot (one per composer or assistant plan). The generation rule: every
/// [`PlanSlot::submit`] or [`PlanSlot::cancel`] bumps the generation and cancels the job in
/// flight; [`PlanSlot::poll`] delivers only a result whose generation equals the current one.
/// So after the last parameter change, the only result that can reach the screen is the one
/// computed from the last parameters.
pub struct PlanSlot<T> {
    generation: u64,
    current: Option<JobHandle<T>>,
}

impl<T> Default for PlanSlot<T> {
    fn default() -> Self {
        PlanSlot { generation: 0, current: None }
    }
}

impl<T> std::fmt::Debug for PlanSlot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanSlot").field("generation", &self.generation).field("current", &self.current).finish()
    }
}

impl<T: Send + 'static> PlanSlot<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current generation (bumped by every submit and cancel).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// True when `generation` is still the slot's current plan (check it again before a commit
    /// made from a delivered preview).
    pub fn is_current(&self, generation: u64) -> bool {
        generation == self.generation
    }

    /// Replaces the plan: cancels the previous job, bumps the generation and queues `job`.
    /// Returns the new generation.
    pub fn submit<F>(&mut self, pool: &Pool, job: F) -> u64
    where
        F: FnOnce(&JobCtx) -> Result<T, JobError> + Send + 'static,
    {
        self.cancel();
        self.current = Some(pool.submit_for(self.generation, job));
        self.generation
    }

    /// Drops the plan: cancels the job in flight and bumps the generation, so nothing from it
    /// is delivered.
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(h) = self.current.take() {
            h.cancel();
        }
    }

    /// True while a job for the current plan is queued or running.
    pub fn is_running(&self) -> bool {
        self.current.as_ref().is_some_and(|h| !h.is_finished())
    }

    pub fn progress(&self) -> Option<Progress> {
        self.current.as_ref().and_then(JobHandle::progress)
    }

    /// The current plan's outcome, once; never blocks.
    pub fn poll(&mut self) -> Option<Delivered<T>> {
        let h = self.current.as_ref()?;
        let outcome = h.poll()?;
        let (job, generation) = (h.id(), h.generation());
        self.current = None;
        // Always true (an older handle is replaced on submit); kept as the explicit guard.
        (generation == self.generation).then_some(Delivered { job, generation, outcome })
    }
}

/// A layer a job made, held in memory until commit: its profile, display name, style (None:
/// the default style in the next palette colour) and its data (e.g. an `Arc` dataset).
#[derive(Clone, Debug)]
pub struct NewLayer<D> {
    pub display_name: String,
    pub profile: LayerProfile,
    pub style: Option<Style>,
    pub data: D,
}

/// Everything a finished action hands over, committed in one transaction: the undo label
/// ("Count schools in each district"), the new layers and the result card.
#[derive(Clone, Debug)]
pub struct Output<D, C> {
    pub label: String,
    pub layers: Vec<NewLayer<D>>,
    pub card: C,
}

/// What [`Output::commit`] gives back for the interface to attach: each new layer's id with its
/// data (for the map) and the card (for the result panel). The undo entry is already recorded.
#[derive(Clone, Debug)]
pub struct Committed<D, C> {
    pub layers: Vec<(LayerId, D)>,
    pub card: C,
    pub label: String,
}

impl<D, C> Output<D, C> {
    /// The single transaction adding every layer (with its style), placed by the stacking rule.
    /// Allocates layer ids from `doc` (ids are never reused, so an abandoned commit only skips
    /// numbers). Returns the ids in the order of `layers`.
    pub fn transaction(&self, doc: &mut Document) -> (Transaction, Vec<LayerId>) {
        let mut ops = Vec::with_capacity(self.layers.len());
        let mut ids = Vec::with_capacity(self.layers.len());
        // Placing later layers depends on the earlier ones: work it out on a scratch copy of the
        // layer order (kinds only matter, so the copy keeps the real layers' order).
        let mut scratch = Document { layers: doc.layers.clone(), user_ordered: doc.user_ordered, ..Document::default() };
        for l in &self.layers {
            let id = doc.allocate_layer_id();
            let style = l.style.clone().unwrap_or_else(|| Style::for_profile(&l.profile, scratch.next_color()));
            let index = scratch.insert_index(l.profile.kind);
            let layer = DocLayer::new(id, l.display_name.clone(), l.profile.clone(), style);
            scratch.layers.insert(index.min(scratch.layers.len()), layer.clone());
            ops.push(Op::AddLayer { index, layer: Box::new(layer) });
            ids.push(id);
        }
        (Transaction::new(self.label.clone(), ops), ids)
    }

    /// Commits the output as one undo step: all layers and styles, or nothing (on error the
    /// document and the undo stack are unchanged). The card is returned only on success.
    pub fn commit(self, doc: &mut Document, undo: &mut UndoStack) -> Result<Committed<D, C>, ApplyError> {
        let (tx, ids) = self.transaction(doc);
        undo.commit(doc, tx)?;
        Ok(Committed { layers: ids.into_iter().zip(self.layers.into_iter().map(|l| l.data)).collect(), card: self.card, label: self.label })
    }
}

// ---------------------------------------------------------------------------------------------
// Cost estimate
// ---------------------------------------------------------------------------------------------

/// The kinds of work the estimator knows (the certified actions, WP4.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionKind {
    /// Count points in each area.
    CountPointsInAreas,
    /// Which area is each point in.
    WhichArea,
    /// What's inside an area.
    WhatsInside,
    /// A geodesic buffer.
    Buffer,
    /// Measure distance or area.
    Measure,
    /// Summarize raster values in each area (fractional coverage).
    Summarize,
    /// Rank by a value.
    Rank,
    /// Raster values at points.
    SampleRaster,
}

/// The size of an action's inputs. `features`/`vertices` are the main layer (the points for a
/// count, the areas for a summary, the layer to buffer); `against_*` the other vector layer
/// (the areas a count goes into); `cells` the raster cells the action reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputSummary {
    pub features: u64,
    pub vertices: u64,
    pub against_features: u64,
    pub against_vertices: u64,
    pub cells: u64,
}

/// The estimate and what follows from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub seconds: f64,
    /// Under [`AUTO_PREVIEW_SECONDS`]: the preview runs as parameters change.
    pub auto_preview: bool,
    /// Over [`WARN_SECONDS`]: say how long before running.
    pub warn: bool,
}

pub const AUTO_PREVIEW_SECONDS: f64 = 1.0;
pub const WARN_SECONDS: f64 = 30.0;

/// Per-operation costs in seconds, release build. Measured ones come from the micro benchmarks
/// in this module's `calibrate` test (Apple silicon laptop, 2026-10: edge test 0.6 ns, haversine
/// 21 ns, weighted cell 1.3 ns, sort comparison 0.7 ns) times a safety factor of 3 for the real
/// code's overhead (indexing, allocation, slower machines). `GEODESIC` assumes GeographicLib
/// (about 25× the haversine stand-in); `FEATURE`, `INDEX_VERTEX` and `UNION` are educated guesses
/// to replace once the certified actions exist. Deliberately rough: the estimate only has to land
/// on the right side of 1 s and 30 s.
pub mod cost {
    /// Fixed cost of any job: queueing, snapshot, building the output layer.
    pub const BASE: f64 = 0.005;
    /// Per feature handled (read, index entry, output row).
    pub const FEATURE: f64 = 2.0e-7;
    /// Per polygon vertex put into the spatial index.
    pub const INDEX_VERTEX: f64 = 6.0e-8;
    /// Per point-in-polygon edge test (crossing number).
    pub const EDGE_TEST: f64 = 2.0e-9;
    /// Per geodesic computation (one vertex of a measure, one generated buffer vertex).
    pub const GEODESIC: f64 = 1.5e-6;
    /// Buffer vertices generated per input vertex (arc segments).
    pub const BUFFER_SEGMENTS: f64 = 16.0;
    /// Per output vertex and log2 step when merging buffers (union).
    pub const UNION: f64 = 1.0e-7;
    /// Per raster cell read and weighted (coverage fraction × cell area).
    pub const CELL: f64 = 4.0e-9;
    /// Per comparison when sorting.
    pub const COMPARE: f64 = 2.2e-9;
}

fn log2(n: f64) -> f64 {
    n.max(2.0).log2()
}

/// Expected run time of an action: a sum of per-operation costs (see [`cost`]):
/// - count / which / inside: each point is tested against one candidate area's edges after an
///   index lookup: `FEATURE·(points + areas) + INDEX_VERTEX·area_vertices
///   + EDGE_TEST·points·(area_vertices / areas)`;
/// - buffer: `GEODESIC·vertices·SEGMENTS + UNION·n·log2(n)` with `n = vertices·SEGMENTS`;
/// - measure: `GEODESIC·vertices`;
/// - summarize: `CELL·cells + GEODESIC·area_vertices + FEATURE·areas`;
/// - rank: `COMPARE·n·log2(n)`;
/// - raster values at points: `FEATURE·points + CELL·cells`;
///
/// plus `BASE`.
pub fn estimate(kind: ActionKind, s: &InputSummary) -> Estimate {
    use cost::*;
    let f = s.features as f64;
    let v = s.vertices as f64;
    let af = s.against_features as f64;
    let av = s.against_vertices as f64;
    let cells = s.cells as f64;
    let work = match kind {
        ActionKind::CountPointsInAreas | ActionKind::WhichArea | ActionKind::WhatsInside => {
            FEATURE * (f + af) + INDEX_VERTEX * av + EDGE_TEST * f * (av / af.max(1.0))
        }
        ActionKind::Buffer => {
            let n = v * BUFFER_SEGMENTS;
            GEODESIC * n + UNION * n * log2(n)
        }
        ActionKind::Measure => GEODESIC * v,
        ActionKind::Summarize => CELL * cells + GEODESIC * v + FEATURE * f,
        ActionKind::Rank => COMPARE * f * log2(f),
        ActionKind::SampleRaster => FEATURE * f + CELL * cells,
    };
    let seconds = BASE + work;
    Estimate { seconds, auto_preview: seconds < AUTO_PREVIEW_SECONDS, warn: seconds > WARN_SECONDS }
}

/// A short recent history of estimates against measured times, for the developer surface to
/// check the constants (not used to adjust them automatically).
#[derive(Debug, Default)]
pub struct EstimateLog {
    entries: VecDeque<(ActionKind, f64, f64)>,
}

impl EstimateLog {
    const CAP: usize = 100;

    pub fn record(&mut self, kind: ActionKind, estimated: f64, measured: f64) {
        if self.entries.len() == Self::CAP {
            self.entries.pop_front();
        }
        self.entries.push_back((kind, estimated, measured));
    }

    /// (kind, estimated seconds, measured seconds), oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &(ActionKind, f64, f64)> {
        self.entries.iter()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use lm_core::model::{Crs, Known, LayerKind, SourceRef};

    /// A small deterministic generator (no rand dependency).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("lm-jobs-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn entries(dir: &Path) -> usize {
        std::fs::read_dir(dir).map(|rd| rd.count()).unwrap_or(0)
    }

    /// A long job that checks its control every `step` for up to `total`, then returns `value`.
    fn long_job(value: u64, step: Duration, total: Duration) -> impl FnOnce(&JobCtx) -> Result<u64, JobError> + Send + 'static {
        move |ctx| {
            let start = Instant::now();
            while start.elapsed() < total {
                ctx.check()?;
                std::thread::sleep(step);
            }
            Ok(value)
        }
    }

    #[test]
    fn a_cancel_lands_within_200_ms() {
        let pool = Pool::with(2, temp_root("latency"), None);
        let mut worst = Duration::ZERO;
        for _ in 0..5 {
            let h = pool.submit(long_job(1, Duration::from_millis(5), Duration::from_secs(10)));
            std::thread::sleep(Duration::from_millis(30));
            let t = Instant::now();
            h.cancel();
            assert!(h.wait_finished(Duration::from_millis(200)), "the cancel took longer than 200 ms");
            worst = worst.max(t.elapsed());
            assert_eq!(h.poll(), Some(JobOutcome::Cancelled));
        }
        eprintln!("worst cancel latency: {worst:?}");
        assert!(worst < Duration::from_millis(200));
    }

    #[test]
    fn a_hundred_random_cancels_leave_no_temp_dirs_and_deliver_nothing() {
        let root = temp_root("cancels");
        let pool = Pool::with(4, root.clone(), None);
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut handles = Vec::new();
        for i in 0..100u64 {
            let h = pool.submit(move |ctx: &JobCtx| {
                let dir = ctx.temp_dir().map_err(|e| JobError::Failed(e.to_string()))?;
                // Writes scratch files in chunks, checking between them; the output is built in
                // memory and only handed over at the end.
                let mut out = Vec::new();
                for chunk in 0..40u64 {
                    ctx.check()?;
                    std::fs::write(dir.join(format!("chunk-{chunk}.bin")), [0u8; 256]).map_err(|e| JobError::Failed(e.to_string()))?;
                    out.push(chunk);
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok((i, out))
            });
            std::thread::sleep(Duration::from_micros(rng.below(20_000)));
            h.cancel();
            assert_eq!(h.poll().filter(|o| matches!(o, JobOutcome::Done(_))), None, "a cancelled job delivered a result");
            handles.push(h);
        }
        assert!(pool.wait_idle(Duration::from_secs(20)));
        for h in &handles {
            assert!(h.is_finished());
            let o = h.poll();
            assert!(o.is_none() || o == Some(JobOutcome::Cancelled), "a cancelled job delivered {o:?}");
        }
        assert_eq!(entries(&root), 0, "temporary folders were left behind");
        drop(pool);
        assert!(!root.exists());
    }

    #[test]
    fn a_finished_job_removes_its_temp_dir_before_delivering() {
        let root = temp_root("done");
        let pool = Pool::with(2, root.clone(), None);
        let h = pool.submit(|ctx: &JobCtx| {
            let d = ctx.temp_dir().map_err(|e| JobError::Failed(e.to_string()))?;
            std::fs::write(d.join("x"), b"x").map_err(|e| JobError::Failed(e.to_string()))?;
            Ok(d)
        });
        assert!(h.wait_finished(Duration::from_secs(5)));
        let Some(JobOutcome::Done(d)) = h.poll() else { panic!("not done") };
        assert!(!d.exists());
        assert_eq!(h.poll(), None, "an outcome is delivered once");
        // A panicking job also cleans up and reports a plain message.
        let h = pool.submit(|ctx: &JobCtx| -> Result<(), JobError> {
            ctx.temp_dir().map_err(|e| JobError::Failed(e.to_string()))?;
            panic!("boom")
        });
        assert!(h.wait_finished(Duration::from_secs(5)));
        assert_eq!(h.poll(), Some(JobOutcome::Failed(PANIC_MESSAGE.into())));
        assert_eq!(entries(&root), 0);
    }

    #[test]
    fn a_thousand_rapid_changes_show_only_the_final_parameters() {
        let pool = Pool::with(4, temp_root("race"), None);
        let mut slot: PlanSlot<u64> = PlanSlot::new();
        let mut rng = Rng(42);
        let mut params = std::collections::HashMap::new();
        let mut delivered = 0;
        let start = Instant::now();
        let mut last = 0;
        for _ in 0..1000 {
            let p = rng.next();
            let work = Duration::from_micros(rng.below(3000));
            let generation = slot.submit(&pool, move |ctx: &JobCtx| {
                let t = Instant::now();
                while t.elapsed() < work {
                    ctx.check()?;
                    std::thread::sleep(Duration::from_micros(200));
                }
                Ok(p)
            });
            params.insert(generation, p);
            last = p;
            if rng.below(3) == 0 {
                std::thread::sleep(Duration::from_micros(rng.below(2000)));
            }
            if let Some(d) = slot.poll() {
                assert_eq!(d.generation, slot.generation());
                if let JobOutcome::Done(v) = d.outcome {
                    assert_eq!(Some(&v), params.get(&d.generation), "a result from other parameters was shown");
                    delivered += 1;
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let on_screen = loop {
            if let Some(d) = slot.poll() {
                let JobOutcome::Done(v) = d.outcome else { panic!("the final plan did not finish: {:?}", d.outcome) };
                break Some(v);
            }
            assert!(Instant::now() < deadline, "the final plan never finished");
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(on_screen, Some(last));
        assert!(pool.wait_idle(Duration::from_secs(10)));
        assert_eq!(slot.poll(), None);
        eprintln!("race: 1000 submissions in {:?}, {delivered} intermediate previews shown", start.elapsed());
    }

    #[test]
    fn the_pool_does_not_deadlock_under_load() {
        let pool = Arc::new(Pool::with(2, temp_root("load"), None));
        let t = Instant::now();
        let threads: Vec<_> = (0..4u64)
            .map(|k| {
                let pool = pool.clone();
                std::thread::spawn(move || {
                    let mut rng = Rng(k + 7);
                    let mut hs = Vec::new();
                    for i in 0..500u64 {
                        let h = pool.submit(move |ctx: &JobCtx| {
                            ctx.check()?;
                            ctx.report("Working", i, Some(500));
                            match i % 250 {
                                0 => panic!("a failing job"),
                                1 => Err(JobError::Failed("no".into())),
                                _ => Ok(i),
                            }
                        });
                        if rng.below(10) == 0 {
                            h.cancel();
                        }
                        hs.push((i, h));
                    }
                    for (i, h) in hs {
                        assert!(h.wait_finished(Duration::from_secs(20)), "job {i} never finished");
                        let o = h.poll().unwrap();
                        if h.control().is_cancelled() {
                            assert_eq!(o, JobOutcome::Cancelled);
                        } else {
                            match i % 250 {
                                0 => assert_eq!(o, JobOutcome::Failed(PANIC_MESSAGE.into())),
                                1 => assert_eq!(o, JobOutcome::Failed("no".into())),
                                _ => assert_eq!(o, JobOutcome::Done(i)),
                            }
                        }
                    }
                })
            })
            .collect();
        for th in threads {
            th.join().unwrap();
        }
        assert!(pool.wait_idle(Duration::from_secs(5)));
        eprintln!("load: 2000 jobs on 2 workers in {:?}", t.elapsed());
    }

    #[test]
    fn progress_phases_are_reported_verbatim() {
        let wakes = Arc::new(AtomicU64::new(0));
        let w = wakes.clone();
        let pool = Pool::with(
            2,
            temp_root("progress"),
            Some(Arc::new(move || {
                w.fetch_add(1, Ordering::Relaxed);
            })),
        );
        let h = pool.submit(|ctx: &JobCtx| {
            ctx.report("Reading districts", 0, None);
            for i in 0..=1000 {
                ctx.report("Counting points in areas", i, Some(1000));
            }
            ctx.report("Building the result layer", 1, Some(1));
            Ok(())
        });
        assert!(h.wait_finished(Duration::from_secs(5)));
        assert_eq!(h.phases(), vec!["Reading districts", "Counting points in areas", "Building the result layer"]);
        assert_eq!(h.progress(), Some(Progress { phase: "Building the result layer".into(), done: 1, total: Some(1) }));
        assert_eq!(h.poll(), Some(JobOutcome::Done(())));
        let n = wakes.load(Ordering::Relaxed);
        assert!((3..100).contains(&n), "progress wakes are throttled: {n}");
    }

    #[test]
    fn dropping_a_handle_cancels_its_job() {
        let pool = Pool::with(1, temp_root("drop"), None);
        let h = pool.submit(long_job(1, Duration::from_millis(2), Duration::from_secs(10)));
        let c = h.control().clone();
        drop(h);
        assert!(c.is_cancelled());
        assert!(pool.wait_idle(Duration::from_millis(500)));
    }

    #[test]
    fn a_cancelled_slot_delivers_nothing() {
        let pool = Pool::with(2, temp_root("slot"), None);
        let mut slot = PlanSlot::new();
        let g = slot.submit(&pool, |_: &JobCtx| Ok(5u8));
        assert!(pool.wait_idle(Duration::from_secs(5)));
        slot.cancel();
        assert!(!slot.is_current(g));
        assert_eq!(slot.poll(), None);
    }

    fn profile(file: &str, kind: LayerKind) -> LayerProfile {
        LayerProfile {
            source: SourceRef { path: format!("/derived/{file}"), member: None, size: 0, mtime: 0, sha256: None },
            format: "Result".into(),
            kind,
            count: 3,
            extent_wgs84: None,
            crs: Known::certain(Crs::Wgs84, "computed"),
            fields: vec![],
            bands: vec![],
            assumptions: vec![],
            exclusions: vec![],
            encoding: None,
            notices: vec![],
            data_rev: 0,
            joins: Vec::new(),
            grid: None,
        }
    }

    #[test]
    fn an_output_commits_as_one_undo_step() {
        let mut doc = Document::new();
        let mut undo = UndoStack::new();
        let tx = doc.add_layer_tx(profile("districts.shp", LayerKind::Areas));
        undo.commit(&mut doc, tx).unwrap();
        let before = doc.clone();
        let out = Output {
            label: "Count schools in each district".to_string(),
            layers: vec![
                NewLayer { display_name: "Schools per district".into(), profile: profile("a", LayerKind::Areas), style: None, data: 1u8 },
                NewLayer { display_name: "Counted schools".into(), profile: profile("b", LayerKind::Points), style: None, data: 2u8 },
            ],
            card: "248 schools in 12 districts",
        };
        let c = out.commit(&mut doc, &mut undo).unwrap();
        assert_eq!(c.layers.len(), 2);
        assert_eq!(doc.layers.len(), 3);
        assert_eq!(c.card, "248 schools in 12 districts");
        assert_eq!(undo.undo_label(), Some("Count schools in each district"));
        let after = doc.clone();
        undo.undo(&mut doc);
        assert_eq!(doc.layers, before.layers, "undo returns to the state before");
        undo.redo(&mut doc);
        assert_eq!(doc.layers, after.layers, "redo gives the identical result");
    }

    #[test]
    fn estimates_decide_preview_and_warning() {
        let small = InputSummary { features: 248, vertices: 248, against_features: 12, against_vertices: 6_000, ..Default::default() };
        let e = estimate(ActionKind::CountPointsInAreas, &small);
        assert!(e.auto_preview && !e.warn, "{e:?}");
        let big = InputSummary { features: 20, vertices: 200_000, cells: 10_000_000_000, ..Default::default() };
        let e = estimate(ActionKind::Summarize, &big);
        assert!(!e.auto_preview && e.warn, "{e:?}");
        let mid = InputSummary { features: 5_000, vertices: 100_000, ..Default::default() };
        let e = estimate(ActionKind::Buffer, &mid);
        assert!(!e.auto_preview && !e.warn, "{e:?}");
        assert!(estimate(ActionKind::Rank, &InputSummary { features: 1_000_000, ..Default::default() }).auto_preview);
    }

    /// Micro benchmarks behind the constants in [`cost`]. Run with
    /// `cargo test -p lm-jobs --release jobs::tests::calibrate -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn calibrate() {
        fn time(n: f64, f: impl FnOnce() -> f64) -> f64 {
            let t = Instant::now();
            let r = f();
            let s = t.elapsed().as_secs_f64();
            std::hint::black_box(r);
            s / n
        }
        let mut rng = Rng(3);
        // Point in polygon: crossing-number test over the edges of a 1,000-vertex ring.
        let ring: Vec<(f64, f64)> = (0..1000)
            .map(|i| {
                let a = i as f64 / 1000.0 * std::f64::consts::TAU;
                let r = 1.0 + (rng.below(100) as f64) / 1000.0;
                (r * a.cos(), r * a.sin())
            })
            .collect();
        let pts: Vec<(f64, f64)> =
            (0..10_000).map(|_| ((rng.below(2000) as f64 / 1000.0) - 1.0, (rng.below(2000) as f64 / 1000.0) - 1.0)).collect();
        let edge = time((pts.len() * ring.len()) as f64, || {
            let mut inside = 0.0;
            for &(x, y) in &pts {
                let mut c = false;
                let mut j = ring.len() - 1;
                for i in 0..ring.len() {
                    let (xi, yi) = ring[i];
                    let (xj, yj) = ring[j];
                    if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                        c = !c;
                    }
                    j = i;
                }
                if c {
                    inside += 1.0;
                }
            }
            inside
        });
        // Geodesic: a haversine distance plus a direct step (stand-in for GeographicLib, which
        // costs several times more).
        let geo = time(1_000_000.0, || {
            let mut s = 0.0;
            for i in 0..1_000_000 {
                let (la1, lo1) = (0.5 + i as f64 * 1e-7, 2.0f64);
                let (la2, lo2) = (0.51f64, 2.01 + i as f64 * 1e-7);
                let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
                let d = 2.0 * h.sqrt().asin();
                s += (la1.sin() * d.cos() + la1.cos() * d.sin() * 0.3f64.cos()).asin();
            }
            s
        });
        // Raster cells: weighted accumulate with a per-row cell area.
        let cells: Vec<f32> = (0..4_000_000).map(|i| (i % 97) as f32).collect();
        let cell = time(cells.len() as f64, || {
            let (mut sw, mut swx) = (0.0f64, 0.0f64);
            for (r, row) in cells.chunks(2000).enumerate() {
                let area = (r as f64 * 1e-4).cos();
                for &v in row {
                    if v != 0.0 {
                        sw += area;
                        swx += area * f64::from(v);
                    }
                }
            }
            swx / sw
        });
        // Sorting: per comparison (n log2 n of them).
        let mut vals: Vec<f64> = (0..1_000_000).map(|_| rng.next() as f64).collect();
        let n = vals.len() as f64;
        let cmp = time(n * n.log2(), || {
            vals.sort_by(f64::total_cmp);
            vals[0]
        });
        eprintln!("edge test {edge:.2e} s, geodesic {geo:.2e} s, cell {cell:.2e} s, compare {cmp:.2e} s");
    }
}
