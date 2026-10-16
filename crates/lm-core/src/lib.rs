//! State model and data contracts (Architecture › Data contracts). No UI, no file or network I/O.
//!
//! Phase 1: phase feature flags and the basemap provider record. Phase 2: the data contracts
//! (`model`).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod basemap;
pub mod model;
pub mod phase;
pub mod places;
