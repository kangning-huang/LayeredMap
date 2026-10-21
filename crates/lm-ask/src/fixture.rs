//! A recorded answer for the Ask box (`LM_ASK_FIXTURE=<file>`): the model's replies come from a
//! file instead of the network, so a plan can be shown in the real app, filmed or checked without
//! a paid call. The plan is still checked and run on the map's layers like any other.
//!
//! The file is JSON, either `submit_plan`'s arguments (`{"steps": [...]}`, as the model sends
//! them) or `{"replies": [...]}`, chat-completion replies answered one per request in order.

use std::cell::RefCell;
use std::path::Path;

use serde_json::{Value as Json, json};

use crate::session::Transport;

/// The environment variable naming a recorded answer.
pub const ENV: &str = "LM_ASK_FIXTURE";

/// Replays recorded replies.
pub struct Fixture {
    replies: RefCell<Vec<Json>>,
}

impl Fixture {
    /// The recorded answer in `path`.
    pub fn open(path: &Path) -> Result<Fixture, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("The recorded answer {} can't be read: {e}", path.display()))?;
        let v: Json = serde_json::from_str(&text).map_err(|e| format!("The recorded answer {} isn't JSON: {e}", path.display()))?;
        Fixture::from_json(v)
    }

    /// A recorded answer from its JSON (see the module).
    pub fn from_json(v: Json) -> Result<Fixture, String> {
        let replies = match (v.get("replies").and_then(Json::as_array), v.get("steps")) {
            (Some(r), _) => r.clone(),
            (None, Some(_)) => vec![json!({"choices": [{"message": {"content": null, "tool_calls": [
                {"id": "fixture", "type": "function", "function": {"name": "submit_plan", "arguments": v.to_string()}}
            ]}}]})],
            (None, None) => return Err("A recorded answer needs \"steps\" or \"replies\".".into()),
        };
        Ok(Fixture { replies: RefCell::new(replies) })
    }

    /// The recorded answer `LM_ASK_FIXTURE` names, when it is set.
    pub fn from_env() -> Option<Result<Fixture, String>> {
        let p = std::env::var_os(ENV).filter(|p| !p.is_empty())?;
        Some(Fixture::open(Path::new(&p)))
    }
}

impl Transport for Fixture {
    fn complete(&self, _body: &Json) -> Result<Json, String> {
        let mut r = self.replies.borrow_mut();
        if r.is_empty() {
            return Err("The recorded answer has no more replies.".into());
        }
        Ok(r.remove(0))
    }
}
