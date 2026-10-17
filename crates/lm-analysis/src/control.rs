//! How long-running analysis cooperates with the job system (WP4.3): every loop that can take
//! more than 50 ms checks `Control::check` at least that often, so a cancel lands within 200 ms;
//! progress reports the phase and the amount done, never an invented percentage.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The job was cancelled; nothing it made is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

/// What a job is doing: a phase in plain words and how much of it is done.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// "Counting points in areas".
    pub phase: String,
    pub done: u64,
    /// None when the total isn't known in advance.
    pub total: Option<u64>,
}

/// A cancel flag and a progress sink, shared by a job and the work it runs.
#[derive(Clone, Default)]
pub struct Control {
    cancel: Arc<AtomicBool>,
    progress: Option<Arc<dyn Fn(Progress) + Send + Sync>>,
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Control").field("cancelled", &self.is_cancelled()).finish()
    }
}

impl Control {
    /// A control nothing listens to and nobody cancels (tests, small inputs).
    pub fn none() -> Control {
        Control::default()
    }

    pub fn with_progress(progress: impl Fn(Progress) + Send + Sync + 'static) -> Control {
        Control { cancel: Arc::new(AtomicBool::new(false)), progress: Some(Arc::new(progress)) }
    }

    /// Asks the work to stop at its next check.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Err when cancelled: call it inside every loop that can run longer than 50 ms.
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() { Err(Cancelled) } else { Ok(()) }
    }

    pub fn report(&self, phase: &str, done: u64, total: Option<u64>) {
        if let Some(p) = &self.progress {
            p(Progress { phase: phase.to_string(), done, total });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancel_is_seen_by_every_clone() {
        let c = Control::none();
        let d = c.clone();
        assert_eq!(d.check(), Ok(()));
        c.cancel();
        assert_eq!(d.check(), Err(Cancelled));
    }

    #[test]
    fn progress_is_reported_as_given() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let s = seen.clone();
        let c = Control::with_progress(move |p| s.lock().unwrap().push(p));
        c.report("Counting points in areas", 10, Some(20));
        assert_eq!(seen.lock().unwrap()[0], Progress { phase: "Counting points in areas".into(), done: 10, total: Some(20) });
    }
}
