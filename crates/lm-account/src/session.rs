//! The signed-in session: who (for the Account pane), the access token (memory only) and the
//! refresh token (the system's credential store). What is stored is one hex-encoded record, so it
//! can be handed to the `security` tool or PowerShell on standard input without quoting.

use serde_json::{Value as Json, json};

use crate::config::Region;

/// Refresh when the access token has less than this left (seconds).
pub const REFRESH_MARGIN: i64 = 300;

/// Who is signed in: only what the Account pane shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    /// The sign-in service's user id.
    pub id: String,
    /// How they signed in (`google`, `apple`, `email`, …).
    pub provider: String,
    /// When the provider gave one.
    pub email: Option<String>,
}

impl User {
    /// "kim@example.com", or how they signed in when there is no email (Sign in with Apple can
    /// hide it, X gives none).
    pub fn display(&self) -> String {
        match &self.email {
            Some(e) if !e.is_empty() => e.clone(),
            _ => format!("Signed in with {}", provider_label(&self.provider)),
        }
    }
}

/// "Apple", "Google", "X", "email" for a provider id.
pub fn provider_label(id: &str) -> &str {
    match id {
        "apple" => "Apple",
        "google" => "Google",
        "x" | "twitter" => "X",
        "email" => "email",
        "phone" => "a phone number",
        "wechat" => "WeChat",
        "alipay" => "Alipay",
        other => other,
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    pub user: User,
    pub region: Region,
    /// The access token and when it expires (Unix seconds). `None` after a restore, until the
    /// first refresh.
    pub access: Option<(String, i64)>,
    pub refresh: String,
}

// Tokens never reach a log.
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("user", &self.user).field("region", &self.region).finish_non_exhaustive()
    }
}

impl Session {
    /// The access token when it has more than [`REFRESH_MARGIN`] left at `now`.
    pub fn fresh_access(&self, now: i64) -> Option<&str> {
        self.access.as_ref().filter(|(_, exp)| *exp - now > REFRESH_MARGIN).map(|(t, _)| t.as_str())
    }

    /// The session in a token answer (`grant_type=pkce` or `refresh_token`).
    pub fn from_token_response(body: &str, region: Region, now: i64) -> Result<Session, String> {
        let j: Json = serde_json::from_str(body).map_err(|_| "the sign-in service's answer isn't readable".to_string())?;
        let token = |k: &str| j.get(k).and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        let access = token("access_token").ok_or("the answer has no access token")?;
        let refresh = token("refresh_token").ok_or("the answer has no refresh token")?;
        // `expires_at` when given (absolute), else `expires_in` from now; never more than a day.
        let expires = j
            .get("expires_at")
            .and_then(Json::as_i64)
            .or_else(|| j.get("expires_in").and_then(Json::as_i64).map(|s| now + s))
            .ok_or("the answer has no expiry")?
            .min(now + 86_400);
        let u = &j["user"];
        let id = u.get("id").and_then(Json::as_str).filter(|s| !s.is_empty()).ok_or("the answer names no user")?;
        let provider = u["app_metadata"].get("provider").and_then(Json::as_str).unwrap_or("email");
        let email = u.get("email").and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        Ok(Session { user: User { id: id.into(), provider: provider.into(), email }, region, access: Some((access, expires)), refresh })
    }

    /// The record kept in the credential store: region, refresh token, user. Hex of JSON.
    pub fn to_record(&self) -> String {
        let j = json!({
            "v": 1,
            "region": self.region.id(),
            "refresh_token": self.refresh,
            "user_id": self.user.id,
            "provider": self.user.provider,
            "email": self.user.email,
        });
        j.to_string().bytes().map(|b| format!("{b:02x}")).collect()
    }

    /// A stored record back (no access token: the first use refreshes).
    pub fn from_record(hex: &str) -> Option<Session> {
        let hex = hex.trim();
        if !hex.len().is_multiple_of(2) {
            return None;
        }
        let bytes: Option<Vec<u8>> = (0..hex.len() / 2).map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()).collect();
        let j: Json = serde_json::from_slice(&bytes?).ok()?;
        if j.get("v").and_then(Json::as_i64) != Some(1) {
            return None;
        }
        let s = |k: &str| j.get(k).and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        Some(Session {
            user: User { id: s("user_id")?, provider: s("provider").unwrap_or_else(|| "email".into()), email: s("email") },
            region: Region::from_id(&s("region")?)?,
            access: None,
            refresh: s("refresh_token")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_792_000_000;

    fn answer(expires: &str) -> String {
        format!(
            r#"{{"access_token":"eyJ.a.b","token_type":"bearer",{expires},"refresh_token":"r3fr3sh",
                "user":{{"id":"6f1d2c3b-4a59-4e8f-9a0b-1c2d3e4f5a6b","email":"kim@example.com",
                "app_metadata":{{"provider":"google","providers":["google"]}},"user_metadata":{{"full_name":"Not Kept"}}}}}}"#
        )
    }

    #[test]
    fn a_token_answer_reads() {
        let s = Session::from_token_response(&answer(r#""expires_in":3600,"expires_at":1792003600"#), Region::International, NOW).unwrap();
        assert_eq!(s.user.id, "6f1d2c3b-4a59-4e8f-9a0b-1c2d3e4f5a6b");
        assert_eq!(s.user.provider, "google");
        assert_eq!(s.user.display(), "kim@example.com");
        assert_eq!(s.access, Some(("eyJ.a.b".into(), NOW + 3600)));
        assert_eq!(s.refresh, "r3fr3sh");
        // Without expires_at, expires_in counts from now.
        let s = Session::from_token_response(&answer(r#""expires_in":60"#), Region::China, NOW).unwrap();
        assert_eq!(s.access.as_ref().map(|a| a.1), Some(NOW + 60));
        assert_eq!(s.region, Region::China);
        // An expiry far away is capped at a day.
        let s = Session::from_token_response(&answer(r#""expires_at":9999999999"#), Region::International, NOW).unwrap();
        assert_eq!(s.access.as_ref().map(|a| a.1), Some(NOW + 86_400));
    }

    #[test]
    fn incomplete_answers_are_refused() {
        assert!(Session::from_token_response("{", Region::International, NOW).is_err());
        assert!(
            Session::from_token_response(r#"{"access_token":"a","expires_in":1,"user":{"id":"u"}}"#, Region::International, NOW).is_err()
        );
        assert!(
            Session::from_token_response(r#"{"access_token":"a","refresh_token":"r","user":{"id":"u"}}"#, Region::International, NOW)
                .is_err()
        );
        assert!(
            Session::from_token_response(r#"{"access_token":"a","refresh_token":"r","expires_in":1}"#, Region::International, NOW).is_err()
        );
    }

    #[test]
    fn refresh_is_due_five_minutes_before_expiry() {
        let mut s = Session::from_token_response(&answer(r#""expires_in":3600"#), Region::International, NOW).unwrap();
        assert_eq!(s.fresh_access(NOW), Some("eyJ.a.b"));
        assert_eq!(s.fresh_access(NOW + 3600 - REFRESH_MARGIN - 1), Some("eyJ.a.b"));
        assert_eq!(s.fresh_access(NOW + 3600 - REFRESH_MARGIN), None);
        assert_eq!(s.fresh_access(NOW + 7200), None);
        s.access = None;
        assert_eq!(s.fresh_access(NOW), None);
    }

    #[test]
    fn the_stored_record_round_trips_without_the_access_token() {
        let s = Session::from_token_response(&answer(r#""expires_in":3600"#), Region::International, NOW).unwrap();
        let rec = s.to_record();
        assert!(rec.bytes().all(|b| b.is_ascii_hexdigit()), "hex only, safe on any command's standard input");
        assert!(!rec.contains("eyJ"), "the access token is never stored");
        let back = Session::from_record(&rec).unwrap();
        assert_eq!(back.user, s.user);
        assert_eq!(back.refresh, "r3fr3sh");
        assert_eq!(back.access, None);
        assert!(Session::from_record("zz").is_none());
        assert!(Session::from_record("abc").is_none());
        let old: String = r#"{"v":2}"#.bytes().map(|b| format!("{b:02x}")).collect();
        assert!(Session::from_record(&old).is_none());
        // Debug output names the user, never a token.
        assert!(!format!("{s:?}").contains("r3fr3sh") && !format!("{s:?}").contains("eyJ"));
    }

    #[test]
    fn without_an_email_the_provider_is_named() {
        let u = User { id: "u".into(), provider: "x".into(), email: None };
        assert_eq!(u.display(), "Signed in with X");
        let u = User { id: "u".into(), provider: "apple".into(), email: Some(String::new()) };
        assert_eq!(u.display(), "Signed in with Apple");
    }
}
