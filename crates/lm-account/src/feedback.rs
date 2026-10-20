//! Help › Send Feedback… (docs/lm/feedback.md): the request the app sends to the feedback
//! function, which posts it as a GitHub Discussion on the public repository. Anyone may send,
//! signed in or not. It lives in this crate because it uses the same service (`account.json`'s
//! international `proxy_url`, the publishable key) and the same gated client
//! (`http::FeedbackHttp`, network purpose Feedback). When someone is signed in to that service,
//! the session's access token goes with it (`Authorization: Bearer …`, as Ask sends it): the
//! server verifies it and keeps the account id privately, never on GitHub. The body never carries
//! an account id; the server wouldn't trust one.
//!
//! The draft is checked here with the server's own bounds, so the sheet can say what to change
//! before anything is sent. The request carries a random install id, made once on this computer
//! and kept in the support folder; the server counts messages per install under a keyed hash of
//! it. A reply address goes to a private table on the server and is never posted.

use serde_json::{Map, Value as Json};

use crate::config::RegionConfig;
use crate::http::{Http, Response};

/// Bounds on the message, in characters, after control characters are removed and the ends
/// trimmed (the server checks the same: `services/ask-proxy/supabase/functions/_shared/feedback.ts`).
pub const MIN_MESSAGE: usize = 5;
pub const MAX_MESSAGE: usize = 4000;
/// Bound on a typed title, in characters.
pub const MAX_TITLE: usize = 120;
/// Links allowed in the title and message together (more is treated as spam).
pub const MAX_LINKS: usize = 3;
/// Bound on a reply address (RFC 5321).
pub const MAX_EMAIL: usize = 254;

/// What the feedback is about: the segmented control's choice, and the Discussions category.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// An idea or suggestion (Ideas).
    #[default]
    Idea,
    /// A problem or bug (Q&A, titled "[Problem] …").
    Problem,
    /// Anything else (General).
    Other,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Idea, Kind::Problem, Kind::Other];

    /// The id the server reads.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Idea => "idea",
            Kind::Problem => "problem",
            Kind::Other => "other",
        }
    }

    /// The segment's words (English catalog keys: the sheet translates them, `tl!`).
    pub fn label(self) -> &'static str {
        match self {
            Kind::Idea => lm_core::tn!("Idea"),
            Kind::Problem => lm_core::tn!("Problem"),
            Kind::Other => lm_core::tn!("Other"),
        }
    }
}

/// What the user wrote in the sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub kind: Kind,
    pub message: String,
    /// Optional: the server takes the message's first line when it is empty.
    pub title: String,
    /// Optional reply address: kept privately on the server, never posted.
    pub email: String,
    /// "Include app version and macOS version" (on by default).
    pub include_diagnostics: bool,
}

impl Default for Draft {
    fn default() -> Self {
        Draft { kind: Kind::Idea, message: String::new(), title: String::new(), email: String::new(), include_diagnostics: true }
    }
}

/// What to change in a draft before it can be sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    MessageTooShort,
    MessageTooLong,
    TitleTooLong,
    TooManyLinks,
    BadEmail,
}

// English templates (catalog keys); the sheet translates them where it draws them.
const MESSAGE_TOO_SHORT: &str = lm_core::tn!("Write a few words about your idea or problem.");
const MESSAGE_TOO_LONG: &str = lm_core::tn!("The message is {n} characters long. Shorten it to {max} or fewer.");
const TITLE_TOO_LONG: &str = lm_core::tn!("Shorten the title to {max} characters or fewer.");
const TOO_MANY_LINKS: &str = lm_core::tn!("Include at most {max} links.");
const BAD_EMAIL: &str = lm_core::tn!("Check the email address, or leave it empty.");

impl Problem {
    /// Every template, for the catalogs.
    pub const TEMPLATES: &'static [&'static str] = &[MESSAGE_TOO_SHORT, MESSAGE_TOO_LONG, TITLE_TOO_LONG, TOO_MANY_LINKS, BAD_EMAIL];

    /// The message as an English template and its values.
    pub fn words(self, draft: &Draft) -> (&'static str, Vec<(&'static str, String)>) {
        match self {
            Problem::MessageTooShort => (MESSAGE_TOO_SHORT, vec![]),
            Problem::MessageTooLong => {
                (MESSAGE_TOO_LONG, vec![("n", chars(&clean(&draft.message)).to_string()), ("max", MAX_MESSAGE.to_string())])
            }
            Problem::TitleTooLong => (TITLE_TOO_LONG, vec![("max", MAX_TITLE.to_string())]),
            Problem::TooManyLinks => (TOO_MANY_LINKS, vec![("max", MAX_LINKS.to_string())]),
            Problem::BadEmail => (BAD_EMAIL, vec![]),
        }
    }
}

/// Characters (code points), as the server counts.
pub fn chars(s: &str) -> usize {
    s.chars().count()
}

/// The text as the server will read it: line breaks normalised, control characters (but line
/// feeds and tabs) and bidirectional overrides removed, the ends trimmed.
pub fn clean(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    let kept: String = s
        .chars()
        .filter(|&c| {
            c == '\n' || c == '\t' || !(c.is_control() || ('\u{202A}'..='\u{202E}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c))
        })
        .collect();
    kept.trim().to_string()
}

/// Links in the text: `http://`, `https://` and `www.` at the start of a word, as the server counts.
pub fn count_links(s: &str) -> usize {
    let lower = s.to_lowercase();
    let mut n = 0;
    for pat in ["https://", "http://", "www."] {
        let mut from = 0;
        while let Some(i) = lower.get(from..).and_then(|rest| rest.find(pat)) {
            let at = from + i;
            let word_start = lower.get(..at).and_then(|b| b.chars().next_back()).is_none_or(|c| !c.is_alphanumeric() && c != '_');
            if word_start {
                n += 1;
            }
            from = at + pat.len();
        }
    }
    n
}

/// A plausible email address: one `@`, something before it, a dot in the domain with at least
/// two characters after the last one, no spaces or brackets. The server checks the same.
pub fn valid_email(s: &str) -> bool {
    let s = s.trim();
    if s.len() > MAX_EMAIL || s.chars().any(|c| c.is_whitespace() || "<>()[]\\,;:\"".contains(c)) {
        return false;
    }
    let Some((local, domain)) = s.split_once('@') else { return false };
    if local.is_empty() || domain.contains('@') {
        return false;
    }
    match domain.rsplit_once('.') {
        Some((name, tld)) => !name.is_empty() && tld.chars().count() >= 2,
        None => false,
    }
}

/// Checks a draft with the server's bounds.
pub fn check(d: &Draft) -> Result<(), Problem> {
    let message = clean(&d.message);
    let n = chars(&message);
    if n < MIN_MESSAGE {
        return Err(Problem::MessageTooShort);
    }
    if n > MAX_MESSAGE {
        return Err(Problem::MessageTooLong);
    }
    let title = clean(&d.title);
    if chars(&title) > MAX_TITLE {
        return Err(Problem::TitleTooLong);
    }
    if count_links(&format!("{title}\n{message}")) > MAX_LINKS {
        return Err(Problem::TooManyLinks);
    }
    let email = d.email.trim();
    if !email.is_empty() && !valid_email(email) {
        return Err(Problem::BadEmail);
    }
    Ok(())
}

/// The app details a message may carry; none of it is personal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct About {
    /// "0.7.3".
    pub app_version: String,
    /// "macOS 14.5"; `None` when it can't be read.
    pub os_version: Option<String>,
    /// The interface language's code ("de", "zh").
    pub language: String,
}

impl About {
    /// This build on this system, in the interface language `language`. Runs `sw_vers` on macOS:
    /// call it on a background thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn current(language: &str) -> About {
        About { app_version: env!("CARGO_PKG_VERSION").to_string(), os_version: system_version(), language: language.to_string() }
    }
}

/// "macOS 14.5"; "Windows" or the system's name elsewhere.
#[cfg(not(target_arch = "wasm32"))]
pub fn system_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/bin/sw_vers").arg("-productVersion").output().ok()?;
        out.status.success().then(|| format!("macOS {}", String::from_utf8_lossy(&out.stdout).trim()))
    }
    #[cfg(windows)]
    {
        Some("Windows".to_string())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Some(std::env::consts::OS.to_string())
    }
}

/// The request body (JSON): the draft cleaned, the app details when the user agreed to send them
/// (the language always), and the install id. A field the user left empty is left out.
pub fn request_body(d: &Draft, about: &About, install_id: &str) -> String {
    let mut m = Map::new();
    m.insert("kind".into(), Json::from(d.kind.id()));
    m.insert("message".into(), Json::from(clean(&d.message)));
    let title = clean(&d.title);
    if !title.is_empty() {
        m.insert("title".into(), Json::from(title));
    }
    let email = d.email.trim();
    if !email.is_empty() {
        m.insert("contact_email".into(), Json::from(email));
    }
    m.insert("include_diagnostics".into(), Json::from(d.include_diagnostics));
    if d.include_diagnostics {
        m.insert("app_version".into(), Json::from(about.app_version.as_str()));
        if let Some(os) = &about.os_version {
            m.insert("os_version".into(), Json::from(os.as_str()));
        }
    }
    m.insert("language".into(), Json::from(about.language.as_str()));
    m.insert("install_id".into(), Json::from(install_id));
    Json::Object(m).to_string()
}

/// Why a message wasn't posted, in words the sheet can show (with what to do).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedbackError {
    /// This build has no feedback service (no account config).
    NotConfigured,
    /// Strict offline mode refused the request.
    Offline,
    /// Too many messages from this computer or network today.
    RateLimited,
    /// The server found the message too long.
    TooLong,
    /// The server refused the message: its reason (English).
    Invalid(String),
    /// The server or GitHub couldn't post it.
    Upstream,
    /// The service couldn't be reached.
    Network(String),
    /// An unexpected answer.
    Server(u16),
}

/// Where people can post by hand when this build can't send.
pub const DISCUSSIONS: &str = "github.com/kangning-huang/NimbleMap/discussions";

const NOT_CONFIGURED: &str = lm_core::tn!("Feedback can't be sent from this build. You can post on GitHub Discussions yourself: {url}");
const OFFLINE: &str = lm_core::tn!("{app} is working offline. Turn off View › Work Offline, then press Send again.");
const RATE_LIMITED: &str = lm_core::tn!("You've sent a lot of feedback today. Please try again tomorrow.");
const TOO_LONG: &str = lm_core::tn!("The message is too long. Shorten it, then press Send again.");
const INVALID: &str = lm_core::tn!("The feedback service didn't accept the message: {reason}");
const UPSTREAM: &str = lm_core::tn!("Your feedback couldn't be posted just now. Your text is still here: try again in a few minutes.");
const NETWORK: &str = lm_core::tn!("Couldn't reach the feedback service. Check your internet connection, then press Send again.");
const SERVER: &str = lm_core::tn!("The feedback service answered {status}. Try again in a few minutes.");

impl FeedbackError {
    /// Every template, for the catalogs.
    pub const TEMPLATES: &'static [&'static str] = &[NOT_CONFIGURED, OFFLINE, RATE_LIMITED, TOO_LONG, INVALID, UPSTREAM, NETWORK, SERVER];

    /// The message as an English template and its values (`{app}` is left for the caller).
    pub fn words(&self) -> (&'static str, Vec<(&'static str, String)>) {
        match self {
            FeedbackError::NotConfigured => (NOT_CONFIGURED, vec![("url", DISCUSSIONS.to_string())]),
            FeedbackError::Offline => (OFFLINE, vec![]),
            FeedbackError::RateLimited => (RATE_LIMITED, vec![]),
            FeedbackError::TooLong => (TOO_LONG, vec![]),
            FeedbackError::Invalid(why) => (INVALID, vec![("reason", why.clone())]),
            FeedbackError::Upstream => (UPSTREAM, vec![]),
            FeedbackError::Network(_) => (NETWORK, vec![]),
            FeedbackError::Server(status) => (SERVER, vec![("status", status.to_string())]),
        }
    }
}

impl std::fmt::Display for FeedbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (template, values) = self.words();
        let mut args: Vec<(&str, &str)> = values.iter().map(|(k, v)| (*k, v.as_str())).collect();
        args.push(("app", lm_core::brand::NAME));
        let mut text = template.to_string();
        for (k, v) in args {
            text = text.replace(&format!("{{{k}}}"), v);
        }
        f.write_str(&text)
    }
}

/// The discussion's address from the service's answer, or why there is none.
pub fn read_answer(r: &Response) -> Result<String, FeedbackError> {
    let j: Json = serde_json::from_str(&r.body).unwrap_or(Json::Null);
    if r.status == 200 {
        return match j.get("url").and_then(Json::as_str) {
            Some(u) if u.starts_with("https://github.com/") => Ok(u.to_string()),
            _ => Err(FeedbackError::Server(r.status)),
        };
    }
    let code = j.pointer("/error/code").and_then(Json::as_str).unwrap_or_default();
    let message = j.pointer("/error/message").and_then(Json::as_str).unwrap_or_default();
    Err(match code {
        "rate_limited" => FeedbackError::RateLimited,
        "too_long" => FeedbackError::TooLong,
        // The server's reason is short English it wrote itself; cap it anyway.
        "invalid" => FeedbackError::Invalid(message.chars().take(160).collect()),
        "upstream" => FeedbackError::Upstream,
        _ if r.status == 429 => FeedbackError::RateLimited,
        _ => FeedbackError::Server(r.status),
    })
}

/// The request's headers: the publishable key, and the access token when someone is signed in.
pub fn headers(service: &RegionConfig, token: Option<&str>) -> Vec<(&'static str, String)> {
    let mut h = vec![("apikey", service.anon_key.clone())];
    if let Some(t) = token.filter(|t| !t.is_empty()) {
        h.push(("Authorization", format!("Bearer {t}")));
    }
    h
}

/// Posts `body` to the feedback function of `service`, with the signed-in session's access token
/// when there is one (blocking: call it on a background thread).
pub fn send(http: &dyn Http, service: &RegionConfig, body: &str, token: Option<&str>) -> Result<String, FeedbackError> {
    let r = http.send("POST", &service.feedback_url(), &headers(service, token), Some(body)).map_err(|e| match e {
        crate::AccountError::Offline => FeedbackError::Offline,
        crate::AccountError::Network(m) => FeedbackError::Network(m),
        other => FeedbackError::Network(other.to_string()),
    })?;
    read_answer(&r)
}

// ── the install id ──────────────────────────────────────────────────────────────────────────────

/// The file in the support folder that holds the install id.
pub const INSTALL_ID_FILE: &str = "feedback-install-id";

/// A random (version 4) UUID from 16 random bytes.
pub fn uuid_v4(mut b: [u8; 16]) -> String {
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// Whether `s` is a UUID as [`uuid_v4`] writes it.
pub fn valid_install_id(s: &str) -> bool {
    s.len() == 36
        && s.char_indices()
            .all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_hexdigit() && !c.is_ascii_uppercase() })
}

/// This install's id: read from `dir`, or made (random) and kept there the first time. It is not
/// the account and says nothing about the person; deleting the file just makes a new one.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_id(dir: &std::path::Path) -> Result<String, String> {
    let path = dir.join(INSTALL_ID_FILE);
    if let Ok(text) = std::fs::read_to_string(&path) {
        let id = text.trim();
        if valid_install_id(id) {
            return Ok(id.to_string());
        }
    }
    let id = random_install_id()?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!("{INSTALL_ID_FILE}.tmp"));
    std::fs::write(&tmp, format!("{id}\n")).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(id)
}

#[cfg(not(target_arch = "wasm32"))]
fn random_install_id() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| format!("no random numbers from the system: {e}"))?;
    Ok(uuid_v4(b))
}

/// Everything Send does, on a background thread: the service from `account.json`, the install id
/// from the support folder (a fresh one for this message if it can't be kept), the app details,
/// the request, and the access token when `account` is signed in to the feedback service
/// ([`crate::Account::feedback_token`]). `language` is the interface language's code.
#[cfg(not(target_arch = "wasm32"))]
pub fn send_now(draft: &Draft, language: &str, account: Option<&crate::Account>) -> Result<String, FeedbackError> {
    check(draft).map_err(|p| FeedbackError::Invalid(format!("{p:?}")))?;
    let config = crate::Config::load().ok_or(FeedbackError::NotConfigured)?;
    let service = config.feedback().ok_or(FeedbackError::NotConfigured)?;
    let dir = lm_core::platform::current_user_dir(lm_core::platform::Folder::Support);
    let id = match dir.as_deref().map(install_id) {
        Some(Ok(id)) => id,
        other => {
            if let Some(Err(e)) = other {
                log::warn!("feedback: the install id couldn't be kept: {e}");
            }
            random_install_id().map_err(FeedbackError::Network)?
        }
    };
    let body = request_body(draft, &About::current(language), &id);
    let token = account.and_then(crate::Account::feedback_token);
    send(&crate::http::FeedbackHttp, service, &body, token.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn draft(message: &str) -> Draft {
        Draft { message: message.into(), ..Draft::default() }
    }

    fn about() -> About {
        About { app_version: "0.7.3".into(), os_version: Some("macOS 14.5".into()), language: "de".into() }
    }

    const ID: &str = "3b241101-e2bb-4255-8caf-4136c566a962";

    #[test]
    fn a_draft_is_checked_with_the_servers_bounds() {
        assert_eq!(check(&draft("A dark basemap, please.")), Ok(()));
        assert_eq!(check(&draft("")), Err(Problem::MessageTooShort));
        assert_eq!(check(&draft(" \n\u{0}hi\u{7} \n")), Err(Problem::MessageTooShort));
        assert_eq!(check(&draft(&"x".repeat(MAX_MESSAGE))), Ok(()));
        assert_eq!(check(&draft(&"x".repeat(MAX_MESSAGE + 1))), Err(Problem::MessageTooLong));
        // Characters, not bytes: 4,000 Chinese characters fit.
        assert_eq!(check(&draft(&"图".repeat(MAX_MESSAGE))), Ok(()));
        let title = Draft { title: "t".repeat(MAX_TITLE + 1), ..draft("A long enough message") };
        assert_eq!(check(&title), Err(Problem::TitleTooLong));
        let links = draft("https://a.example http://b.example www.c.example https://d.example");
        assert_eq!(check(&links), Err(Problem::TooManyLinks));
        assert_eq!(check(&draft("https://a.example http://b.example www.c.example")), Ok(()));
        let bad = Draft { email: "kim at example".into(), ..draft("A long enough message") };
        assert_eq!(check(&bad), Err(Problem::BadEmail));
        let good = Draft { email: " kim@example.com ".into(), ..draft("A long enough message") };
        assert_eq!(check(&good), Ok(()));
    }

    #[test]
    fn links_count_at_word_starts() {
        assert_eq!(count_links("see HTTPS://a.example and www.b.example"), 2);
        assert_eq!(count_links("awww.example is not a link"), 0);
        assert_eq!(count_links("(https://a.example)"), 1);
        assert_eq!(count_links("no links"), 0);
    }

    #[test]
    fn email_addresses() {
        for ok in ["kim@example.com", "a.b+c@sub.example.co.uk", "李@例子.中国"] {
            assert!(valid_email(ok), "{ok}");
        }
        for bad in ["", "kim", "kim@", "@example.com", "kim@example", "kim@example.c", "a b@example.com", "a@b@example.com", "<a@b.co>"] {
            assert!(!valid_email(bad), "{bad}");
        }
        assert!(!valid_email(&format!("{}@example.com", "a".repeat(250))));
    }

    #[test]
    fn cleaning_removes_control_and_bidi_characters() {
        assert_eq!(clean("  a\u{0}b\u{1b}c\r\nd\u{202e}e\u{2066}f\tg\rh  "), "abc\ndef\tg\nh");
    }

    #[test]
    fn the_request_body_carries_what_the_user_agreed_to() {
        let d = Draft {
            kind: Kind::Problem,
            message: " It crashes when I open a KML file.\r\n".into(),
            title: "Crash".into(),
            email: " kim@example.com ".into(),
            include_diagnostics: true,
        };
        let j: Json = serde_json::from_str(&request_body(&d, &about(), ID)).unwrap();
        assert_eq!(j["kind"], "problem");
        assert_eq!(j["message"], "It crashes when I open a KML file.");
        assert_eq!(j["title"], "Crash");
        assert_eq!(j["contact_email"], "kim@example.com");
        assert_eq!(j["include_diagnostics"], true);
        assert_eq!(j["app_version"], "0.7.3");
        assert_eq!(j["os_version"], "macOS 14.5");
        assert_eq!(j["language"], "de");
        assert_eq!(j["install_id"], ID);
        // Unticked: no versions; empty fields are left out; the language stays.
        let bare = Draft { include_diagnostics: false, ..draft("A dark basemap, please.") };
        let j: Json = serde_json::from_str(&request_body(&bare, &about(), ID)).unwrap();
        assert_eq!(j["kind"], "idea");
        assert_eq!(j["include_diagnostics"], false);
        for k in ["app_version", "os_version", "title", "contact_email"] {
            assert!(j.get(k).is_none(), "{k}");
        }
        assert_eq!(j["language"], "de");
        // No account data of any kind.
        let text = request_body(&d, &about(), ID);
        for k in ["user", "token", "session", "sub"] {
            assert!(!text.contains(&format!("\"{k}")), "{k}");
        }
    }

    #[test]
    fn kinds_have_the_servers_ids() {
        assert_eq!(Kind::ALL.map(Kind::id), ["idea", "problem", "other"]);
        assert_eq!(Draft::default().kind, Kind::Idea);
        assert!(Draft::default().include_diagnostics);
    }

    fn answer(status: u16, body: &str) -> Response {
        Response { status, body: body.into() }
    }

    #[test]
    fn answers_become_the_discussion_or_an_error() {
        let url = "https://github.com/kangning-huang/NimbleMap/discussions/42";
        assert_eq!(read_answer(&answer(200, &format!(r#"{{"url":"{url}"}}"#))), Ok(url.to_string()));
        assert_eq!(read_answer(&answer(200, r#"{"url":"https://evil.example/"}"#)), Err(FeedbackError::Server(200)));
        assert_eq!(read_answer(&answer(200, "not json")), Err(FeedbackError::Server(200)));
        let err = |status, code: &str| answer(status, &format!(r#"{{"error":{{"code":"{code}","message":"At most 3 links, please."}}}}"#));
        assert_eq!(read_answer(&err(429, "rate_limited")), Err(FeedbackError::RateLimited));
        assert_eq!(read_answer(&err(413, "too_long")), Err(FeedbackError::TooLong));
        assert_eq!(read_answer(&err(400, "invalid")), Err(FeedbackError::Invalid("At most 3 links, please.".into())));
        assert_eq!(read_answer(&err(502, "upstream")), Err(FeedbackError::Upstream));
        assert_eq!(read_answer(&answer(429, "")), Err(FeedbackError::RateLimited));
        assert_eq!(read_answer(&answer(500, "<html>")), Err(FeedbackError::Server(500)));
    }

    #[test]
    fn errors_read_as_english_with_the_brand() {
        assert!(FeedbackError::Offline.to_string().starts_with("NimbleMap is working offline"));
        assert!(FeedbackError::Server(500).to_string().contains("500"));
        assert!(FeedbackError::Invalid("x".into()).to_string().ends_with(": x"));
        for t in FeedbackError::TEMPLATES.iter().chain(Problem::TEMPLATES) {
            assert!(!t.is_empty());
        }
    }

    /// A fake client: records the request, answers what it was given.
    struct Fake {
        seen: Mutex<Vec<(String, String, Vec<(String, String)>, Option<String>)>>,
        answer: Result<Response, crate::AccountError>,
    }

    impl Http for Fake {
        fn send(&self, method: &str, url: &str, headers: &[(&str, String)], body: Option<&str>) -> Result<Response, crate::AccountError> {
            if let Ok(mut s) = self.seen.lock() {
                s.push((
                    method.into(),
                    url.into(),
                    headers.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                    body.map(str::to_string),
                ));
            }
            self.answer.clone()
        }
    }

    fn service() -> RegionConfig {
        RegionConfig {
            auth_url: "https://abc.supabase.co".into(),
            anon_key: "sb_publishable_test".into(),
            proxy_url: "https://abc.supabase.co/functions/v1".into(),
            providers: vec![],
        }
    }

    #[test]
    fn send_posts_to_the_feedback_function_with_the_publishable_key() {
        let url = "https://github.com/kangning-huang/NimbleMap/discussions/7";
        let fake = Fake { seen: Mutex::new(vec![]), answer: Ok(answer(200, &format!(r#"{{"url":"{url}"}}"#))) };
        let body = request_body(&draft("A dark basemap, please."), &about(), ID);
        assert_eq!(send(&fake, &service(), &body, None), Ok(url.to_string()));
        let seen = fake.seen.lock().unwrap();
        let (method, to, headers, sent) = &seen[0];
        assert_eq!((method.as_str(), to.as_str()), ("POST", "https://abc.supabase.co/functions/v1/feedback"));
        assert_eq!(headers, &vec![("apikey".to_string(), "sb_publishable_test".to_string())]);
        assert_eq!(sent.as_deref(), Some(body.as_str()));
    }

    #[test]
    fn the_token_goes_in_a_header_only_when_signed_in() {
        let url = "https://github.com/kangning-huang/NimbleMap/discussions/7";
        let fake = Fake { seen: Mutex::new(vec![]), answer: Ok(answer(200, &format!(r#"{{"url":"{url}"}}"#))) };
        let body = request_body(&draft("A dark basemap, please."), &about(), ID);
        assert_eq!(send(&fake, &service(), &body, Some("eyJ.access.sig")), Ok(url.to_string()));
        assert_eq!(send(&fake, &service(), &body, None), Ok(url.to_string()));
        assert_eq!(send(&fake, &service(), &body, Some("")), Ok(url.to_string()));
        let seen = fake.seen.lock().unwrap();
        let key = ("apikey".to_string(), "sb_publishable_test".to_string());
        assert_eq!(seen[0].2, vec![key.clone(), ("Authorization".to_string(), "Bearer eyJ.access.sig".to_string())]);
        assert_eq!(seen[1].2, vec![key.clone()], "signed out: the publishable key only");
        assert_eq!(seen[2].2, vec![key], "an empty token is no token");
        // The body is the same either way: the account is never in it.
        assert!(seen.iter().all(|(_, _, _, b)| b.as_deref() == Some(body.as_str())));
        assert!(!body.contains("eyJ") && !body.contains("user"));
    }

    #[test]
    fn offline_and_network_failures_say_what_to_do() {
        let offline = Fake { seen: Mutex::new(vec![]), answer: Err(crate::AccountError::Offline) };
        assert_eq!(send(&offline, &service(), "{}", None), Err(FeedbackError::Offline));
        let down = Fake { seen: Mutex::new(vec![]), answer: Err(crate::AccountError::Network("dns".into())) };
        assert_eq!(send(&down, &service(), "{}", None), Err(FeedbackError::Network("dns".into())));
    }

    #[test]
    fn install_ids_are_random_v4_uuids_made_once() {
        let id = uuid_v4([0xff; 16]);
        assert_eq!(id, "ffffffff-ffff-4fff-bfff-ffffffffffff");
        assert!(valid_install_id(&id));
        assert!(valid_install_id(ID));
        for bad in ["", "not-a-uuid", "3B241101-E2BB-4255-8CAF-4136C566A962", "3b241101e2bb42558caf4136c566a962xxxx"] {
            assert!(!valid_install_id(bad), "{bad}");
        }
        let dir = std::env::temp_dir().join(format!("lm-feedback-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = install_id(&dir.join("nested")).unwrap();
        assert!(valid_install_id(&first));
        assert_eq!(install_id(&dir.join("nested")).unwrap(), first, "kept, not made again");
        // A damaged file is replaced.
        std::fs::write(dir.join("nested").join(INSTALL_ID_FILE), "garbage").unwrap();
        let second = install_id(&dir.join("nested")).unwrap();
        assert!(valid_install_id(&second) && second != first);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
