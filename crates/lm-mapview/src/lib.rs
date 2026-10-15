//! The map widget: camera, gestures, inertia, tile placeholders, hit testing, selection highlight.
//! The only crate below `lm-ui` that may depend on egui, and the only direct line from the
//! interface to RustGIS (its drawing path).
//!
//! Phase 0: empty. Starts in Phase 1 as an extraction of `rustgis-ui-egui`'s map view.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
