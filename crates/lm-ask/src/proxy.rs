//! The Ask proxy (WP7.3, WP7.6; docs/lm/accounts-design.md): the same chat-completions body the
//! direct transport sends, posted to NimbleMap's own service with the signed-in account's access
//! token instead of an OpenRouter key. The service checks the token and Pro, applies the daily
//! allowance and holds the key. The body is passed on as built, so the privacy rule of
//! [`crate::context`] and [`crate::resolve`] (no feature values, except `resolve_feature`'s
//! matches) holds unchanged.

use std::time::Duration;

use serde_json::Value as Json;

use crate::openrouter::OFFLINE;

pub struct Proxy {
    /// `…/functions/v1/ask`.
    url: String,
    /// The service's public (anon) key, sent as `apikey`.
    anon_key: String,
    /// The account's access token (fresh: the caller refreshes it first).
    token: String,
}

impl Proxy {
    pub fn new(url: String, anon_key: String, token: String) -> Self {
        Proxy { url, anon_key, token }
    }
}

// The Ask box's messages as English templates (catalog keys, `lm_core::i18n`). They are built on
// the request's background thread, where the interface language isn't known: the English goes
// out through `Transport`, and the interface translates it where it shows it (`lm-ui`'s
// `account_ui::shown`, which matches it against [`TEMPLATES`]). `{pro}` and `{app}` are the brand.
const SIGNED_OUT: &str = lm_core::tn!("You're signed out. Sign in again in Settings › Account to use {pro}.");
const NOT_PRO: &str = lm_core::tn!("{pro} isn't on for this account.");
const NOT_YET: &str = lm_core::tn!("This account can't use {pro} yet. Try again in a minute.");
const MONTH_TOMORROW: &str = lm_core::tn!("You've used this month's assistant allowance. It renews tomorrow.");
const MONTH_DAYS: &str = lm_core::tn!("You've used this month's assistant allowance. It renews in {n} days.");
const DAY_HOUR: &str = lm_core::tn!("You've used today's assistant allowance. It renews within the hour.");
const DAY_HOURS: &str = lm_core::tn!("You've used today's assistant allowance. It renews in about {n} hours.");
const NO_CREDIT: &str = lm_core::tn!("The assistant service is out of credit. It has been told; try again later.");
const BUSY: &str = lm_core::tn!("The assistant service is busy. Try again in a minute.");
const ANSWERED: &str = lm_core::tn!("The assistant service answered {status}: {reason}");
const NO_REASON: &str = lm_core::tn!("no reason given");
const NOT_HTTPS: &str = lm_core::tn!("The assistant service's address isn't https.");
const UNREACHABLE: &str = lm_core::tn!("Couldn't reach the assistant service: {error}");
const CUT_OFF: &str = lm_core::tn!("The assistant service's answer was cut off: {error}");
const UNREADABLE: &str = lm_core::tn!("The assistant service answered {status} with something unreadable.");
/// [`OFFLINE`] as a template.
const OFFLINE_WORDS: &str =
    lm_core::tn!("{app} is working offline, so the assistant can't be reached. Turn off View › Work Offline to ask.");

/// Every message template, for the interface to recognise and translate the English.
pub const TEMPLATES: &[&str] = &[
    SIGNED_OUT,
    NOT_PRO,
    NOT_YET,
    MONTH_TOMORROW,
    MONTH_DAYS,
    DAY_HOUR,
    DAY_HOURS,
    NO_CREDIT,
    BUSY,
    ANSWERED,
    NOT_HTTPS,
    UNREACHABLE,
    CUT_OFF,
    UNREADABLE,
    OFFLINE_WORDS,
];

/// A template filled the English way, the brand included.
fn english(template: &str, args: &[(&str, &str)]) -> String {
    let mut all = args.to_vec();
    all.extend([("app", lm_core::brand::NAME), ("pro", lm_core::brand::PRO_NAME)]);
    lm_core::i18n::fill(template, &all)
}

/// The service's error answer, `{"error": {"code", "message", …}}`, in the Ask box's words.
pub fn error_words(status: u16, json: &Json) -> String {
    let e = &json["error"];
    let code = e["code"].as_str().unwrap_or("");
    match (status, code) {
        (401, _) => english(SIGNED_OUT, &[]),
        (403, "not_pro") => english(NOT_PRO, &[]),
        (403, _) => english(NOT_YET, &[]),
        (429, "monthly_limit") => {
            let days = e["resets_in_seconds"].as_u64().map(|s| s.div_ceil(86_400)).unwrap_or(30).max(1);
            if days == 1 { english(MONTH_TOMORROW, &[]) } else { english(MONTH_DAYS, &[("n", &days.to_string())]) }
        }
        (429, _) => {
            let hours = e["resets_in_seconds"].as_u64().map(|s| s.div_ceil(3600)).unwrap_or(24).max(1);
            if hours == 1 { english(DAY_HOUR, &[]) } else { english(DAY_HOURS, &[("n", &hours.to_string())]) }
        }
        (_, "upstream_credit") => english(NO_CREDIT, &[]),
        (_, "upstream_busy") => english(BUSY, &[]),
        _ => english(ANSWERED, &[("status", &status.to_string()), ("reason", e["message"].as_str().unwrap_or(NO_REASON))]),
    }
}

/// Whether an Ask box message is the service's `not_pro` refusal (the account doesn't have Pro):
/// the app then locks Pro at once (`lm_account::Account::note_ask_answer`).
pub fn is_not_pro(message: &str) -> bool {
    message == english(NOT_PRO, &[])
}

impl crate::session::Transport for Proxy {
    fn complete(&self, body: &Json) -> Result<Json, String> {
        if !(self.url.starts_with("https://") || self.url.starts_with("http://127.0.0.1:")) {
            return Err(english(NOT_HTTPS, &[]));
        }
        // Strict offline mode (WP6.6): nothing is sent; the Ask box says why.
        lm_net::check(lm_net::Purpose::Assistant).map_err(|_| OFFLINE.to_string())?;
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(90))).http_status_as_error(false).build().into();
        let mut resp = agent
            .post(&self.url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("apikey", &self.anon_key)
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|e| english(UNREACHABLE, &[("error", &e.to_string())]))?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().map_err(|e| english(CUT_OFF, &[("error", &e.to_string())]))?;
        let json: Json = serde_json::from_str(&text).map_err(|_| english(UNREADABLE, &[("status", &status.to_string())]))?;
        if status == 200 { Ok(json) } else { Err(error_words(status, &json)) }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::session::Transport as _;

    /// A one-shot local service: records each request's headers and body, answers `replies` in turn.
    pub(crate) fn service(replies: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<(Vec<String>, String)>>>) {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (stream, (status, answer)) in listener.incoming().flatten().zip(replies) {
                let mut r = BufReader::new(stream);
                let mut headers = Vec::new();
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    headers.push(h.trim().to_string());
                }
                let len = headers
                    .iter()
                    .find_map(|h| h.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
                    .unwrap_or(0);
                let mut body = vec![0u8; len];
                let _ = r.read_exact(&mut body);
                log.lock().unwrap().push((headers, String::from_utf8_lossy(&body).to_string()));
                let mut s = r.into_inner();
                let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
            }
        });
        (format!("http://127.0.0.1:{port}/functions/v1/ask"), seen)
    }

    #[test]
    fn offline_sends_nothing() {
        let _net = crate::NET_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        lm_net::set_offline(true);
        let before = lm_net::counts();
        let r = Proxy::new("https://example.invalid/ask".into(), "anon".into(), "tok".into()).complete(&json!({}));
        lm_net::set_offline(false);
        assert_eq!(r.unwrap_err(), OFFLINE);
        assert_eq!(lm_net::counts().allowed_total(), before.allowed_total());
    }

    /// The body arrives exactly as the session built it, with the token and the anon key.
    #[test]
    fn the_body_goes_unchanged_with_the_token() {
        let _net = crate::NET_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let reply = json!({"choices": [{"message": {"content": "hello"}}]});
        let (url, seen) = service(vec![(200, reply.to_string())]);
        let body = json!({"model": crate::MODEL, "messages": [{"role": "user", "content": "schools near rivers"}], "temperature": 0});
        let got = Proxy::new(url, "anon-key".into(), "access-1".into()).complete(&body).unwrap();
        assert_eq!(got, reply);
        let seen = seen.lock().unwrap();
        let (headers, sent) = &seen[0];
        assert_eq!(serde_json::from_str::<Json>(sent).unwrap(), body);
        assert!(headers.iter().any(|h| h.eq_ignore_ascii_case("authorization: Bearer access-1")), "{headers:?}");
        assert!(headers.iter().any(|h| h.eq_ignore_ascii_case("apikey: anon-key")), "{headers:?}");
        assert!(headers[0].starts_with("POST /functions/v1/ask "), "{headers:?}");
    }

    #[test]
    fn the_services_refusals_read_as_sentences() {
        let pro = lm_core::brand::PRO_NAME;
        let e = |status: u16, j: Json| error_words(status, &j);
        assert!(e(401, json!({"error": {"code": "unauthorized"}})).contains("Sign in again"));
        assert_eq!(e(403, json!({"error": {"code": "not_pro"}})), format!("{pro} isn't on for this account."));
        assert!(is_not_pro(&e(403, json!({"error": {"code": "not_pro"}}))));
        assert!(!is_not_pro(&e(403, json!({"error": {"code": "no_profile"}}))));
        assert!(e(429, json!({"error": {"code": "daily_limit", "resets_in_seconds": 5400}})).ends_with("in about 2 hours."));
        assert!(e(429, json!({"error": {"code": "daily_limit", "resets_in_seconds": 60}})).ends_with("within the hour."));
        assert!(
            e(429, json!({"error": {"code": "monthly_limit", "resets_in_seconds": 12 * 86_400 - 5}}))
                .ends_with("this month's assistant allowance. It renews in 12 days.")
        );
        assert!(e(429, json!({"error": {"code": "monthly_limit", "resets_in_seconds": 3_600}})).ends_with("It renews tomorrow."));
        assert_eq!(e(500, json!({})), "The assistant service answered 500: no reason given");
        assert_eq!(english(OFFLINE_WORDS, &[]), OFFLINE);
        assert!(e(503, json!({"error": {"code": "upstream_credit"}})).contains("out of credit"));
        assert!(e(502, json!({"error": {"code": "upstream", "message": "The model service answered 500."}})).contains("answered 500"));
        // Through the transport.
        let _net = crate::NET_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let (url, _) = service(vec![(429, json!({"error": {"code": "daily_limit", "resets_in_seconds": 3600}}).to_string())]);
        let r = Proxy::new(url, "a".into(), "t".into()).complete(&json!({}));
        assert!(r.unwrap_err().contains("allowance"));
    }

    #[test]
    fn plain_http_goes_nowhere_but_this_computer() {
        let _net = crate::NET_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let r = Proxy::new("http://ask.example.com/ask".into(), "a".into(), "t".into()).complete(&json!({}));
        assert!(r.unwrap_err().contains("https"));
    }
}
