//! State model and data contracts (Architecture › Data contracts). No UI, no file or network I/O.
//!
//! Phase 1: phase feature flags and the basemap provider record. Phase 2: the data contracts
//! (`model`), the document state with transactions (`document`), the undo core (`undo`) and the
//! question log record (`question`).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod basemap;
pub mod brand;
pub mod document;
pub mod filter;
pub mod format;
pub mod grid;
pub mod i18n;
pub mod model;
pub mod palette;
pub mod phase;
pub mod places;
pub mod platform;
pub mod question;
pub mod recipe;
pub mod selection;
pub mod undo;
