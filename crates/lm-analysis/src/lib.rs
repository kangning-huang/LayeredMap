//! Certified algorithms where upstream lacks the needed meaning: fractional-coverage zonal
//! summary, boundary-aware point counts, geodesic buffers, GCJ-02/BD-09 conversion, geodesic
//! length and area, nearest-feature distances, join by location, per-feature measurements and
//! label points, and attribute-only insight (ranks, medians, phrases). Tests first.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod buffer;
pub mod china;
pub mod control;
pub mod geodesic;
pub mod insight;
pub mod measure;
pub mod nearest;
pub mod overlay;
pub mod zonal;
