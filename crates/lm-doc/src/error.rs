//! Errors in plain words: each message says what happened and, where it helps, what to do.

use std::path::Path;

pub type Result<T, E = DocError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("Could not read {path}: {source}")]
    Read { path: String, source: std::io::Error },
    #[error("Could not save to {path}: {source}")]
    Write { path: String, source: std::io::Error },
    #[error("{path} is not a {} document.", lm_core::brand::NAME)]
    NotADocument { path: String },
    #[error(
        "This map was saved by a newer version of {app} (document format {found}). This version opens format {supported} and older. Update {app} to open it.",
        app = lm_core::brand::NAME
    )]
    TooNew { found: u64, supported: u32 },
    #[error("The map file {path} is damaged and could not be opened: {detail}")]
    Damaged { path: String, detail: String },
    #[error("{path} is too large to be a {} document.", lm_core::brand::NAME)]
    TooLarge { path: String },
}

impl DocError {
    pub(crate) fn read(path: &Path, source: std::io::Error) -> Self {
        DocError::Read { path: path.display().to_string(), source }
    }
    pub(crate) fn write(path: &Path, source: std::io::Error) -> Self {
        DocError::Write { path: path.display().to_string(), source }
    }
}
