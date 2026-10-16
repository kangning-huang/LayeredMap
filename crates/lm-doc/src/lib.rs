//! Document format, autosave, recovery journal, source fingerprints and relinking.
//!
//! Phase 2 (WP2.10): document v0 (`manifest`: `save`/`load` of `manifest.json`), source
//! fingerprints (`source`) and the question log (`qlog`).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod error;
pub mod manifest;
pub mod qlog;
pub mod source;

pub use error::{DocError, Result};
pub use manifest::{FORMAT_NAME, MANIFEST_FILE, load, save};
pub use qlog::{QuestionLog, export_csv};
pub use source::{SourceState, check_source, fingerprint};

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fresh, empty temporary directory, removed when dropped.
    pub struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("lm-doc-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
