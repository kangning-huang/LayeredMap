//! The developer route to the assistant: OpenRouter's chat completions, with the developer's key
//! from the macOS keychain (service `openrouter`, account `layeredmap`; on Windows and elsewhere
//! the `OPENROUTER_API_KEY` environment variable, as Credential Manager would need unsafe Win32
//! calls for a developer-only path). The key is read when a request is sent and never logged or
//! written to a file. The app itself talks to NimbleMap's Ask service (`crate::proxy`, WP7.3).

use std::time::Duration;

use serde_json::Value as Json;

const URL: &str = "https://openrouter.ai/api/v1/chat/completions";
#[cfg(target_os = "macos")]
const SERVICE: &str = "openrouter";
#[cfg(target_os = "macos")]
const ACCOUNT: &str = "layeredmap";

/// What the assistant says while the app works offline.
// A const can't use format!, so the name is a literal: keep it in step with lm_core::brand::NAME.
pub const OFFLINE: &str = "NimbleMap is working offline, so the assistant can't be reached. Turn off View › Work Offline to ask.";

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
        // Strict offline mode (WP6.6): nothing is sent; the Ask box says why.
        lm_net::check(lm_net::Purpose::Assistant).map_err(|_| OFFLINE.to_string())?;
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(90))).http_status_as_error(false).build().into();
        let mut resp = agent
            .post(URL)
            .header("Authorization", &format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://layeredmap.app")
            .header("X-Title", lm_core::brand::PRO_NAME)
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

/// The developer's key: this Mac's keychain (`OPENROUTER_API_KEY` on Windows and elsewhere). It
/// is used only on the developer route (`LM_ASK_ROUTE=direct`, or a run without an account
/// config): no build carries a key; the app asks NimbleMap's Ask service with the account's token
/// (docs/lm/accounts-design.md › The Ask box: which route).
pub fn read_key() -> Option<String> {
    keychain_key()
}

#[cfg(target_os = "macos")]
fn keychain_key() -> Option<String> {
    use security_framework::passwords::get_generic_password;
    use security_framework_sys::base::errSecItemNotFound;
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => {
            let k = String::from_utf8(bytes).ok()?.trim().to_string();
            (!k.is_empty()).then_some(k)
        }
        Err(e) if e.code() == errSecItemNotFound => None,
        Err(_) => None,
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.is_empty())
}

#[cfg(test)]
mod tests {
    use crate::session::Transport as _;

    /// Offline, the request never leaves: the gate refuses it and the user is told why.
    #[test]
    fn offline_sends_nothing() {
        let _net = crate::NET_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        lm_net::set_offline(true);
        let before = lm_net::counts();
        let r = super::OpenRouter::new("sk-or-not-a-key".into()).complete(&serde_json::json!({}));
        lm_net::set_offline(false);
        assert_eq!(r.unwrap_err(), super::OFFLINE);
        assert_eq!(lm_net::counts().allowed_total(), before.allowed_total());
    }
}
