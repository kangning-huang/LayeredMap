//! Action registry, eligibility checks, deterministic relevance ranking and per-phase feature
//! flags (Insight cards and suggested actions).
//!
//! Phase 3 (WP3.2): the registry skeleton, read from `rules.toml` (the spec's Selection →
//! suggested actions table as data), and the relevance engine: [`suggest`] turns a [`Context`]
//! into up to three actions plus More actions. Display-only actions (`Surface::Card`) are the
//! free app's suggested-action buttons; analysis actions (`Surface::Assistant`) appear only
//! behind the developer flag. Phase 4 completes the specs (decisions, previews, outputs).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod context;
pub mod registry;
pub mod suggest;

pub use context::{Context, FieldSummary, OpenLayer, Selection};
pub use registry::{
    ActionSpec, CompanionKind, CompanionSpec, Group, Need, OtherLayer, OutputContract, PreviewPolicy, RULES_TOML, Registry, Requirement,
    Row, RulesError, SelectionKey, Surface, Variant, registry,
};
pub use suggest::{Ineligible, Options, Suggested, Suggestions, Unavailable, check, suggest};
