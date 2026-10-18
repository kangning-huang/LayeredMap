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

/// The key: the one built into a Pro test bundle (`Contents/Resources/assistant.dat`, written by
/// `packaging/lm/bundle-macos.sh` from the builder's keychain), else this Mac's keychain.
pub fn read_key() -> Option<String> {
    bundled_key().or_else(keychain_key)
}

/// An internal test build: one with the owner's key built in (the release has none and talks to
/// LayeredMap's assistant service). Such builds show each request's cost; the release never does.
pub fn internal_build() -> bool {
    bundled_key().is_some()
}

/// Scrambled so it isn't plain text in the bundle; not a secret store (see the bundle script).
const PAD: &[u8] = b"LayeredMap Pro test build: not a secret store.";

fn bundled_key() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    decode(&std::fs::read_to_string(exe.parent()?.parent()?.join("Resources").join("assistant.dat")).ok()?)
}

fn decode(hex: &str) -> Option<String> {
    let hex = hex.trim();
    let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()).collect();
    let key: Vec<u8> = bytes.iter().enumerate().map(|(i, b)| b ^ PAD[i % PAD.len()]).collect();
    String::from_utf8(key).ok().filter(|k| k.starts_with("sk-or-"))
}

#[cfg(target_os = "macos")]
fn keychain_key() -> Option<String> {
    let out = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
        .output()
        .ok()?;
    let k = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !k.is_empty()).then_some(k)
}

#[cfg(not(target_os = "macos"))]
fn keychain_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_built_in_key_unscrambles() {
        // As packaging/lm/bundle-macos.sh writes it: key bytes XOR the pad, as hex.
        let key = "sk-or-v1-0123456789abcdef";
        let hex: String = key.bytes().enumerate().map(|(i, b)| format!("{:02x}", b ^ super::PAD[i % super::PAD.len()])).collect();
        assert_eq!(super::decode(&hex).as_deref(), Some(key));
        assert_eq!(super::decode("zz"), None);
    }
}
