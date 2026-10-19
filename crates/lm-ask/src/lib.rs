//! NimbleMap Pro's Ask box (Phase 7, early test build): a request in plain words becomes a plan
//! of NimbleMap's certified actions, checked and run as a preview before the user keeps it.
//!
//! The prompt, rules, worked examples and tools are the ones the evaluation harness measured
//! (`tools/lm/assistant/export_assets.py`). The model sees the map's layers, fields, filters and
//! counts, never feature values, except what [`resolve`] returns for the user's own words.

pub mod context;
#[cfg(not(target_arch = "wasm32"))]
pub mod openrouter;
mod pov;
pub mod reference;
pub mod resolve;
pub mod session;
pub mod tools;

pub use session::{Answer, PlanResult, Request, Turn, Usage, ask};

/// The model, chosen on the evaluation (docs/lm/llm-eval.md): Claude Haiku 5.5 with `submit_plan`.
pub const MODEL: &str = "anthropic/claude-haiku-5.5";

#[cfg(test)]
mod tests;
