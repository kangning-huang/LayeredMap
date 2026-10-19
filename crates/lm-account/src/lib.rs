//! NimbleMap Pro's account (WP7.4; design: docs/lm/accounts-design.md). Sign-in happens in the
//! system browser with OAuth and PKCE, the redirect comes back to a loopback listener (RFC 8252),
//! the access token stays in memory and the refresh token in the Keychain or the Credential
//! Manager. Every request goes through the network gate (`lm_net`, purpose Account).
//!
//! The flow, on a background thread: [`Account::begin`] (binds the loopback, makes the PKCE
//! verifier and state, builds the authorize URL or sends the email link), the browser is opened
//! on [`SignIn::url`], then [`Account::complete`] waits for the redirect and exchanges the code.
//! [`Account::access_token`] refreshes when the token is about to expire.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod browser;
pub mod config;
pub mod http;
pub mod pkce;
pub mod redirect;
pub mod route;
pub mod session;
pub mod store;
pub mod url;

use std::sync::{Arc, Mutex};

pub use config::{Config, Provider, Region, RegionConfig};
pub use route::AskRoute;
pub use session::{Session, User};

/// How long a sign-in waits for the browser (an email link included).
pub const SIGN_IN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// What went wrong, in words the Account pane and the Ask box can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountError {
    /// No config for the region (or placeholders only).
    NotConfigured,
    /// Strict offline mode refused the request.
    Offline,
    Cancelled,
    TimedOut,
    /// The service or provider said no: its reason.
    Denied(String),
    /// No session, or the service no longer accepts it (revoked, reused, account deleted).
    SignedOut,
    Network(String),
    /// An unexpected answer: status and what it said.
    Server(u16, String),
    Storage(String),
    Listener(String),
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = lm_core::brand::NAME;
        match self {
            AccountError::NotConfigured => write!(f, "Sign-in isn't available in this build yet."),
            AccountError::Offline => {
                write!(f, "{name} is working offline, so it can't reach the sign-in service. Turn off View › Work Offline to sign in.")
            }
            AccountError::Cancelled => write!(f, "Sign-in was cancelled."),
            AccountError::TimedOut => write!(f, "Sign-in took too long. Try again; an email link must be opened on this computer."),
            AccountError::Denied(why) => write!(f, "Sign-in didn't finish: {why}"),
            AccountError::SignedOut => write!(f, "You're signed out. Sign in again to use {}.", lm_core::brand::PRO_NAME),
            AccountError::Network(e) => write!(f, "Couldn't reach the sign-in service: {e}"),
            AccountError::Server(status, msg) => write!(f, "The sign-in service answered {status}: {msg}"),
            AccountError::Storage(e) => write!(f, "Couldn't keep the sign-in on this computer: {e}"),
            AccountError::Listener(e) => write!(f, "Couldn't start sign-in: {e}"),
        }
    }
}

impl std::error::Error for AccountError {}

/// The service's reason in an error answer (`error_description`, `msg`, `message`, `error`).
fn reason(body: &str) -> String {
    let j: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let e = &j["error"];
    [&j["error_description"], &j["msg"], &j["message"], &e["message"], e]
        .iter()
        .find_map(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "no reason given".into())
        .chars()
        .take(200)
        .collect()
}

fn now() -> i64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// How to sign in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    /// A provider's page in the browser.
    Provider(Provider),
    /// A sign-in link sent to this address.
    Email(String),
}

/// A sign-in in progress: the loopback it waits on and the secrets to finish it.
#[cfg(not(target_arch = "wasm32"))]
pub struct SignIn {
    region: Region,
    loopback: redirect::Loopback,
    state: String,
    pkce: pkce::Pkce,
    url: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl SignIn {
    /// The page to open in the browser (`None` for an email link: the email carries it).
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

/// The account: the config, how to reach the service, where the session is kept, and the session.
pub struct Account {
    config: Config,
    http: Arc<dyn http::Http>,
    store: Arc<dyn store::SecretStore>,
    /// The tokens. Held across a refresh, so two requests never use one rotating refresh token.
    session: Mutex<Option<Session>>,
    /// Who is signed in, for the interface: never held across a network request.
    shown: Mutex<Option<(User, Region)>>,
    clock: fn() -> i64,
}

impl Account {
    pub fn new(config: Config, http: Arc<dyn http::Http>, store: Arc<dyn store::SecretStore>) -> Account {
        Account { config, http, store, session: Mutex::new(None), shown: Mutex::new(None), clock: now }
    }

    /// The same with a clock (Unix seconds), for tests.
    pub fn with_clock(mut self, clock: fn() -> i64) -> Account {
        self.clock = clock;
        self
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Reads the stored session (no network). Who is signed in, if anyone.
    pub fn restore(&self) -> Option<User> {
        let s = match self.store.load() {
            Ok(Some(rec)) => Session::from_record(&rec)?,
            Ok(None) => return None,
            Err(e) => {
                log::warn!("account: the stored session couldn't be read: {e}");
                return None;
            }
        };
        let user = s.user.clone();
        self.set(Some(s));
        Some(user)
    }

    /// Who is signed in, and where.
    pub fn user(&self) -> Option<(User, Region)> {
        self.shown.lock().ok().and_then(|g| g.clone())
    }

    /// A signed-in session without a sign-in (tests and screenshots).
    pub fn adopt(&self, s: Session) {
        self.set(Some(s));
    }

    fn set(&self, s: Option<Session>) {
        if let Ok(mut g) = self.shown.lock() {
            *g = s.as_ref().map(|s| (s.user.clone(), s.region));
        }
        if let Ok(mut g) = self.session.lock() {
            *g = s;
        }
    }

    fn region_config(&self, r: Region) -> Result<&RegionConfig, AccountError> {
        self.config.region(r).ok_or(AccountError::NotConfigured)
    }

    fn post(&self, rc: &RegionConfig, url: &str, body: &serde_json::Value) -> Result<http::Response, AccountError> {
        self.http.send("POST", url, &[("apikey", rc.anon_key.clone())], Some(&body.to_string()))
    }

    /// Starts a sign-in: binds the loopback, makes the verifier and state, and builds the page to
    /// open (or, for an email link, asks the service to send it).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn begin(&self, region: Region, method: &Method) -> Result<SignIn, AccountError> {
        let rc = self.region_config(region)?;
        if lm_net::offline() {
            return Err(AccountError::Offline);
        }
        let loopback = redirect::Loopback::bind().map_err(|e| AccountError::Listener(e.to_string()))?;
        let state = pkce::random_state().map_err(AccountError::Listener)?;
        let pkce = pkce::Pkce::generate().map_err(AccountError::Listener)?;
        let redirect = url::redirect_uri(loopback.port(), &state);
        let url = match method {
            Method::Provider(p) => {
                let id = rc.providers.iter().find(|(q, _)| q == p).map(|(_, id)| id.as_str()).ok_or(AccountError::NotConfigured)?;
                Some(url::authorize_url(&rc.auth_url, id, &redirect, &pkce.challenge))
            }
            Method::Email(address) => {
                let address = address.trim();
                if !plausible_email(address) {
                    return Err(AccountError::Denied("that doesn't look like an email address".into()));
                }
                let body = serde_json::json!({
                    "email": address,
                    "create_user": true,
                    "code_challenge": pkce.challenge,
                    "code_challenge_method": "s256",
                });
                let r = self.post(rc, &url::otp_url(&rc.auth_url, &redirect), &body)?;
                if !(200..300).contains(&r.status) {
                    return Err(match r.status {
                        429 => AccountError::Denied("too many emails were sent; wait a minute and try again".into()),
                        s => AccountError::Server(s, reason(&r.body)),
                    });
                }
                None
            }
        };
        Ok(SignIn { region, loopback, state, pkce, url })
    }

    /// Waits for the browser to come back, exchanges the code and keeps the session.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn complete(
        &self,
        sign_in: SignIn,
        cancel: &std::sync::atomic::AtomicBool,
        timeout: std::time::Duration,
    ) -> Result<User, AccountError> {
        let deadline = std::time::Instant::now() + timeout;
        let code = sign_in.loopback.wait(&sign_in.state, deadline, cancel)?;
        drop(sign_in.loopback);
        let rc = self.region_config(sign_in.region)?;
        let body = serde_json::json!({"auth_code": code, "code_verifier": sign_in.pkce.verifier});
        let r = self.post(rc, &format!("{}/auth/v1/token?grant_type=pkce", rc.auth_url), &body)?;
        if r.status != 200 {
            return Err(match r.status {
                400 | 401 | 403 | 404 => AccountError::Denied(reason(&r.body)),
                s => AccountError::Server(s, reason(&r.body)),
            });
        }
        let s = Session::from_token_response(&r.body, sign_in.region, (self.clock)()).map_err(|e| AccountError::Server(200, e))?;
        let user = s.user.clone();
        self.store.save(&s.to_record()).map_err(AccountError::Storage)?;
        self.set(Some(s));
        Ok(user)
    }

    /// A valid access token, refreshed first when it has less than five minutes left. A refresh
    /// the service refuses signs out here (the stored record is deleted).
    pub fn access_token(&self) -> Result<String, AccountError> {
        let mut guard = self.session.lock().map_err(|_| AccountError::Storage("session lock poisoned".into()))?;
        let s = guard.as_ref().ok_or(AccountError::SignedOut)?;
        let now = (self.clock)();
        if let Some(t) = s.fresh_access(now) {
            return Ok(t.to_string());
        }
        let region = s.region;
        let rc = self.region_config(region)?;
        let body = serde_json::json!({"refresh_token": s.refresh});
        let r = self.post(rc, &format!("{}/auth/v1/token?grant_type=refresh_token", rc.auth_url), &body)?;
        match r.status {
            200 => {
                let fresh = Session::from_token_response(&r.body, region, now).map_err(|e| AccountError::Server(200, e))?;
                // The refresh token was replaced: keep the new one before anything else can fail.
                if let Err(e) = self.store.save(&fresh.to_record()) {
                    log::warn!("account: the refreshed session couldn't be stored: {e}");
                }
                let token = fresh.fresh_access(now).map(str::to_string);
                if let Ok(mut g) = self.shown.lock() {
                    *g = Some((fresh.user.clone(), region));
                }
                *guard = Some(fresh);
                token.ok_or_else(|| AccountError::Server(200, "the new token is already expiring".into()))
            }
            400 | 401 | 403 => {
                log::info!("account: the session was refused ({}); signed out", reason(&r.body));
                *guard = None;
                drop(guard);
                if let Ok(mut g) = self.shown.lock() {
                    *g = None;
                }
                let _ = self.store.delete();
                Err(AccountError::SignedOut)
            }
            s => Err(AccountError::Server(s, reason(&r.body))),
        }
    }

    /// Signs out: revokes this device's session at the service when it can (best effort), then
    /// deletes the stored record and the tokens whatever the service said.
    pub fn sign_out(&self) -> Result<(), AccountError> {
        if !lm_net::offline() {
            match self.access_token() {
                Ok(token) => {
                    if let Some((_, region)) = self.user()
                        && let Ok(rc) = self.region_config(region)
                    {
                        let url = format!("{}/auth/v1/logout?scope=local", rc.auth_url);
                        let headers = [("apikey", rc.anon_key.clone()), ("Authorization", format!("Bearer {token}"))];
                        if let Err(e) = self.http.send("POST", &url, &headers, None) {
                            log::info!("account: sign-out not sent to the service: {e}");
                        }
                    }
                }
                Err(e) => log::info!("account: sign-out without the service: {e}"),
            }
        }
        self.set(None);
        self.store.delete().map_err(AccountError::Storage)
    }

    /// Deletes the account at the service (the proxy's `DELETE /account`), then signs out here.
    pub fn delete_account(&self) -> Result<(), AccountError> {
        let (_, region) = self.user().ok_or(AccountError::SignedOut)?;
        let rc = self.region_config(region)?;
        if lm_net::offline() {
            return Err(AccountError::Offline);
        }
        let token = self.access_token()?;
        let headers = [("apikey", rc.anon_key.clone()), ("Authorization", format!("Bearer {token}"))];
        let r = self.http.send("DELETE", &rc.account_url(), &headers, None)?;
        match r.status {
            200..=299 => {
                self.set(None);
                self.store.delete().map_err(AccountError::Storage)
            }
            401 => Err(AccountError::SignedOut),
            s => Err(AccountError::Server(s, reason(&r.body))),
        }
    }
}

/// Something@something.something, no spaces: enough to catch a slip before asking the service.
fn plausible_email(s: &str) -> bool {
    let Some((user, domain)) = s.split_once('@') else { return false };
    !user.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !s.contains(char::is_whitespace)
        && s.len() <= 254
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_addresses_are_checked_lightly() {
        for ok in ["kim@example.com", "a.b+c@mail.example.cn"] {
            assert!(plausible_email(ok), "{ok}");
        }
        for bad in ["", "kim", "kim@", "@example.com", "kim@example", "kim@.com", "kim@example.", "k im@example.com"] {
            assert!(!plausible_email(bad), "{bad}");
        }
    }

    #[test]
    fn error_answers_give_their_reason() {
        assert_eq!(reason(r#"{"error":"invalid_grant","error_description":"Invalid Refresh Token"}"#), "Invalid Refresh Token");
        assert_eq!(reason(r#"{"code":400,"msg":"Email rate limit exceeded"}"#), "Email rate limit exceeded");
        assert_eq!(reason(r#"{"error":{"code":"not_pro","message":"Pro isn't on"}}"#), "Pro isn't on");
        assert_eq!(reason("<html>"), "no reason given");
    }

    #[test]
    fn errors_read_as_sentences() {
        assert!(AccountError::Offline.to_string().contains("offline"));
        assert!(AccountError::SignedOut.to_string().contains(lm_core::brand::PRO_NAME));
        assert_eq!(AccountError::Denied("no".into()).to_string(), "Sign-in didn't finish: no");
    }

    fn signed_in(store: Arc<store::MemoryStore>, expires: i64) -> Account {
        let config =
            Config::parse(r#"{"international": {"auth_url": "https://a.example", "anon_key": "k", "proxy_url": "https://a.example/f"}}"#)
                .unwrap_or_default();
        let a = Account::new(config, Arc::new(NoNetwork), store).with_clock(|| 1_000);
        a.adopt(Session {
            user: User { id: "u".into(), provider: "google".into(), email: Some("kim@example.com".into()) },
            region: Region::International,
            access: Some(("tok".into(), expires)),
            refresh: "r".into(),
        });
        a
    }

    struct NoNetwork;
    impl http::Http for NoNetwork {
        fn send(&self, _: &str, _: &str, _: &[(&str, String)], _: Option<&str>) -> Result<http::Response, AccountError> {
            Err(AccountError::Network("no network in this test".into()))
        }
    }

    #[test]
    fn a_fresh_token_needs_no_request() {
        let a = signed_in(Arc::new(store::MemoryStore::default()), 1_000 + 3_600);
        assert_eq!(a.access_token().as_deref(), Ok("tok"));
        // Due for a refresh: the request goes out (and fails here), the session stays.
        let a = signed_in(Arc::new(store::MemoryStore::default()), 1_000 + 60);
        assert!(matches!(a.access_token(), Err(AccountError::Network(_))));
        assert!(a.user().is_some());
    }

    #[test]
    fn restore_reads_the_stored_record_and_sign_out_clears_it() {
        let store = Arc::new(store::MemoryStore::default());
        let a = signed_in(store.clone(), 0);
        let rec = a.session.lock().unwrap().as_ref().unwrap().to_record();
        store::SecretStore::save(store.as_ref(), &rec).unwrap();
        let b = Account::new(Config::default(), Arc::new(NoNetwork), store.clone());
        assert_eq!(b.restore().map(|u| u.display()).as_deref(), Some("kim@example.com"));
        assert_eq!(b.user().map(|(_, r)| r), Some(Region::International));
        // Signing out without the service still clears everything here.
        lm_net::set_offline(false);
        b.sign_out().unwrap();
        assert!(b.user().is_none());
        assert_eq!(store::SecretStore::load(store.as_ref()).unwrap(), None);
        assert_eq!(b.access_token(), Err(AccountError::SignedOut));
        // A broken record restores nothing.
        store::SecretStore::save(store.as_ref(), "00ff").unwrap();
        assert!(b.restore().is_none());
    }

    #[test]
    fn a_region_without_config_cant_sign_in() {
        let a = Account::new(Config::default(), Arc::new(NoNetwork), Arc::new(store::MemoryStore::default()));
        assert!(matches!(a.begin(Region::International, &Method::Provider(Provider::Google)), Err(AccountError::NotConfigured)));
        assert!(matches!(a.delete_account(), Err(AccountError::SignedOut)));
    }
}
