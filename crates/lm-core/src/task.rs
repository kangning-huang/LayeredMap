//! Background work that also runs in the browser (apps/nimblemap-web).
//!
//! On the desktop [`spawn`] starts a named thread, exactly as `std::thread::Builder` does. The
//! browser build (`wasm32-unknown-unknown`, no threads) queues the closure instead, and the web
//! app runs the queue between frames with [`pump`]: the work still happens off the frame that
//! asked for it, so the interface can show "Loading…" first, and every caller keeps its
//! channel-and-poll shape unchanged.

/// Runs `f` in the background: on its own thread (desktop) or queued for [`pump`] (browser).
pub fn spawn<F>(name: &str, f: F) -> std::io::Result<()>
where
    F: FnOnce() + Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::Builder::new().name(name.into()).spawn(f).map(|_| ())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        queue().lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_back(Box::new(f));
        Ok(())
    }
}

/// Whether background work runs on threads (false in the browser).
pub const THREADS: bool = !cfg!(target_arch = "wasm32");

#[cfg(target_arch = "wasm32")]
type Task = Box<dyn FnOnce() + Send>;

#[cfg(target_arch = "wasm32")]
fn queue() -> &'static std::sync::Mutex<std::collections::VecDeque<Task>> {
    static Q: std::sync::OnceLock<std::sync::Mutex<std::collections::VecDeque<Task>>> = std::sync::OnceLock::new();
    Q.get_or_init(Default::default)
}

/// Browser: runs the queued work, at most `max` tasks (what they queue in turn waits for the
/// next call). Returns how many are still waiting. Desktop: nothing to do.
pub fn pump(max: usize) -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        for _ in 0..max {
            let next = queue().lock().unwrap_or_else(std::sync::PoisonError::into_inner).pop_front();
            match next {
                Some(t) => t(),
                None => break,
            }
        }
        queue().lock().unwrap_or_else(std::sync::PoisonError::into_inner).len()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = max;
        0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn spawn_runs_the_closure() {
        let (tx, rx) = std::sync::mpsc::channel();
        super::spawn("lm-test", move || {
            let _ = tx.send(7);
        })
        .unwrap();
        super::pump(8);
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), 7);
    }
}
