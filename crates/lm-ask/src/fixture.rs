//! A recorded answer for the Ask box (`LM_ASK_FIXTURE=<file>`): the model's replies come from a
//! file instead of the network, so a plan can be shown in the real app, filmed or checked without
//! a paid call. The plan is still checked and run on the map's layers like any other.
//!
//! The file is JSON, either `submit_plan`'s arguments (`{"steps": [...]}`, as the model sends
//! them) or `{"replies": [...]}`, chat-completion replies answered one per request in order.
//!
//! `LM_ASK_RECORD=<file>` is the other half: a real request's replies are written to the file as
//! they arrive, in the `{"replies": [...]}` form, so a paid answer can be kept and replayed exactly.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::session::Transport;

/// The environment variable naming a recorded answer.
pub const ENV: &str = "LM_ASK_FIXTURE";
/// The environment variable naming where to record a real request's replies.
pub const RECORD_ENV: &str = "LM_ASK_RECORD";

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

/// Passes requests to another transport and writes every reply to a file as it arrives
/// (`{"replies": [...]}`, which [`Fixture`] replays). Only the replies are written: the key travels
/// in the transport's headers, never in a body or a reply.
pub struct Recorder<'a> {
    inner: &'a dyn Transport,
    path: PathBuf,
    replies: RefCell<Vec<Json>>,
}

impl<'a> Recorder<'a> {
    pub fn new(inner: &'a dyn Transport, path: PathBuf) -> Recorder<'a> {
        Recorder { inner, path, replies: RefCell::new(Vec::new()) }
    }

    /// Where `LM_ASK_RECORD` asks for the replies to be written, when it is set.
    pub fn path_from_env() -> Option<PathBuf> {
        std::env::var_os(RECORD_ENV).filter(|p| !p.is_empty()).map(PathBuf::from)
    }
}

impl Transport for Recorder<'_> {
    fn complete(&self, body: &Json) -> Result<Json, String> {
        let reply = self.inner.complete(body)?;
        let mut r = self.replies.borrow_mut();
        r.push(reply.clone());
        let text = serde_json::to_string_pretty(&json!({ "replies": *r })).unwrap_or_default();
        if let Err(e) = std::fs::write(&self.path, text) {
            log::warn!("could not record the answer to {}: {e}", self.path.display());
        }
        Ok(reply)
    }
}
