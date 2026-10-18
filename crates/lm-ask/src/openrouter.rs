//! The assistant service for the test build: OpenRouter's chat completions, with the key from the
//! macOS keychain (service `openrouter`, account `layeredmap`). The key is read when a request is
//! sent and never logged or written to a file. (The release talks to LayeredMap's own assistant
//! service instead, WP7.3.)

use std::time::Duration;

use serde_json::Value as Json;

const URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const SERVICE: &str = "openrouter";
const ACCOUNT: &str = "layeredmap";

pub struct OpenRouter {
    key: String,
}

impl OpenRouter {
    pub fn new(key: String) -> Self {
        OpenRouter { key }
    }
}

impl crate::session::Transport for OpenRouter {
    fn complete(&self, body: &Json) -> Result<Json, String> {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(90))).http_status_as_error(false).build().into();
        let mut resp = agent
            .post(URL)
            .header("Authorization", &format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://layeredmap.app")
            .header("X-Title", "LayeredMap Pro")
            .send(body.to_string())
            .map_err(|e| format!("Couldn't reach the assistant service: {e}"))?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().map_err(|e| format!("The assistant service's answer was cut off: {e}"))?;
        let json: Json =
            serde_json::from_str(&text).map_err(|_| format!("The assistant service answered {status} with something unreadable."))?;
        match status {
            200 => Ok(json),
            401 | 403 => Err("The OpenRouter key was refused. Check it, then paste it again.".into()),
            402 => Err("The OpenRouter account is out of credit.".into()),
            _ => {
                Err(format!("The assistant service answered {status}: {}", json["error"]["message"].as_str().unwrap_or("no reason given")))
            }
        }
    }
}

/// The key from the keychain, if one is saved.
#[cfg(target_os = "macos")]
pub fn read_key() -> Option<String> {
    let out = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
        .output()
        .ok()?;
    let k = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !k.is_empty()).then_some(k)
}

/// Saves the key in the login keychain (replacing an earlier one).
#[cfg(target_os = "macos")]
pub fn save_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if !key.starts_with("sk-or-") || key.len() < 20 {
        return Err("That doesn't look like an OpenRouter key (they start with sk-or-).".into());
    }
    let ok = std::process::Command::new("/usr/bin/security")
        .args(["add-generic-password", "-U", "-s", SERVICE, "-a", ACCOUNT, "-l", "LayeredMap Pro: OpenRouter key", "-w", key])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok { Ok(()) } else { Err("Couldn't save the key in the keychain.".into()) }
}

#[cfg(not(target_os = "macos"))]
pub fn read_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.is_empty())
}

#[cfg(not(target_os = "macos"))]
pub fn save_key(_key: &str) -> Result<(), String> {
    Err("Set OPENROUTER_API_KEY in the environment.".into())
}
