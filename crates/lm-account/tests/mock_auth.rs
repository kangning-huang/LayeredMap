//! The whole flow against a tiny local mock of Supabase Auth and the account function: the
//! authorize URL, the browser's redirect to the loopback, the code exchange with the PKCE
//! verifier, refresh with rotation, a reused refresh token, sign-out, delete, the email link, the
//! email's 6-digit code (and how it ends the wait for the link), and offline mode. The real HTTP client is used (through the network gate); storage is in memory.

use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lm_account::store::{MemoryStore, SecretStore as _};
use lm_account::{Account, AccountError, Config, Method, Provider, Region, Session};

/// Tests share the process-wide network gate (offline mode): one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct Mock {
    /// The PKCE challenge the authorize URL carried (the test copies it in, as Supabase keeps it).
    challenge: String,
    /// Refresh tokens still valid; each use replaces it (rotation).
    valid_refresh: Vec<String>,
    issued: u32,
    access_valid: Vec<String>,
    expires_in: i64,
    deleted: bool,
    /// The code the email carried, and whether verify answers 429 (too many attempts).
    email_code: String,
    verify_limited: bool,
    /// What the account function says about Pro (`profiles.pro`); `None`: no profile (404).
    pro: Option<bool>,
    /// (method, path with query, headers, body) of every request.
    seen: Vec<(String, String, HashMap<String, String>, String)>,
}

fn tokens(m: &mut Mock) -> String {
    m.issued += 1;
    let (a, r) = (format!("access-{}", m.issued), format!("refresh-{}", m.issued));
    m.access_valid.push(a.clone());
    m.valid_refresh.push(r.clone());
    format!(
        r#"{{"access_token":"{a}","token_type":"bearer","expires_in":{},"refresh_token":"{r}",
            "user":{{"id":"6f1d2c3b-4a59-4e8f-9a0b-1c2d3e4f5a6b","email":"kim@example.com","app_metadata":{{"provider":"google"}}}}}}"#,
        m.expires_in
    )
}

fn route(m: &mut Mock, method: &str, path: &str, headers: &HashMap<String, String>, body: &str) -> (u16, String) {
    let j: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    if path.starts_with("/auth/") && headers.get("apikey").map(String::as_str) != Some("anon-key") {
        return (401, r#"{"message":"no apikey"}"#.into());
    }
    let bearer = headers.get("authorization").and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("").to_string();
    match (method, path.split('?').next().unwrap_or("")) {
        ("POST", "/auth/v1/token") if path.ends_with("grant_type=pkce") => {
            let verifier = j["code_verifier"].as_str().unwrap_or("");
            if j["auth_code"] != "code-123" || lm_account::pkce::challenge_s256(verifier) != m.challenge {
                return (
                    400,
                    r#"{"error":"invalid_grant","error_description":"code challenge does not match previously saved code verifier"}"#
                        .into(),
                );
            }
            (200, tokens(m))
        }
        ("POST", "/auth/v1/token") if path.ends_with("grant_type=refresh_token") => {
            let r = j["refresh_token"].as_str().unwrap_or("").to_string();
            if m.deleted || !m.valid_refresh.contains(&r) {
                return (400, r#"{"error":"invalid_grant","error_description":"Invalid Refresh Token: Already Used"}"#.into());
            }
            m.valid_refresh.retain(|x| *x != r);
            (200, tokens(m))
        }
        ("POST", "/auth/v1/logout") => {
            m.access_valid.retain(|a| *a != bearer);
            (204, String::new())
        }
        ("POST", "/auth/v1/otp") => (200, "{}".into()),
        ("POST", "/auth/v1/verify") => {
            if m.verify_limited {
                return (429, r#"{"code":429,"error_code":"over_request_rate_limit","msg":"Request rate limit reached"}"#.into());
            }
            if j["token"] == "000000" {
                return (400, r#"{"code":400,"error_code":"validation_failed","msg":"Verify requires a verification type"}"#.into());
            }
            if j["type"] != "email" || j["email"] != "kim@example.com" || m.email_code.is_empty() || j["token"] != m.email_code.as_str() {
                return (403, r#"{"code":403,"error_code":"otp_expired","msg":"Token has expired or is invalid"}"#.into());
            }
            // A code is used once.
            m.email_code.clear();
            (200, tokens(m))
        }
        ("GET", "/functions/v1/account") => {
            if !m.access_valid.contains(&bearer) {
                return (401, r#"{"error":{"code":"unauthorized","message":"Sign in again."}}"#.into());
            }
            match m.pro {
                Some(pro) => (200, format!(r#"{{"pro":{pro},"plan":null,"today":{{"requests":0,"tokens":0}}}}"#)),
                None => (404, r#"{"error":{"code":"no_profile","message":"This account has no profile yet."}}"#.into()),
            }
        }
        ("DELETE", "/functions/v1/account") => {
            if !m.access_valid.contains(&bearer) {
                return (401, r#"{"error":{"code":"unauthorized","message":"Sign in again."}}"#.into());
            }
            m.deleted = true;
            (204, String::new())
        }
        _ => (404, "{}".into()),
    }
}

fn serve(stream: TcpStream, mock: &Arc<Mutex<Mock>>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let mut headers = HashMap::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0u8; len];
    let _ = reader.read_exact(&mut body);
    let body = String::from_utf8_lossy(&body).to_string();
    let (status, answer) = {
        let mut m = mock.lock().unwrap();
        m.seen.push((method.clone(), path.clone(), headers.clone(), body.clone()));
        route(&mut m, &method, &path, &headers, &body)
    };
    let mut s = reader.into_inner();
    let _ = write!(
        s,
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
        answer.len()
    );
}

/// Starts the mock; its base URL.
fn start(mock: Arc<Mutex<Mock>>) -> String {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            serve(s, &mock);
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn config(base: &str) -> Config {
    Config::parse(&format!(
        r#"{{"international": {{"auth_url": "{base}", "anon_key": "anon-key", "proxy_url": "{base}/functions/v1",
             "providers": ["apple", "google", "x", "email"]}}}}"#
    ))
    .unwrap()
}

fn query(url: &str) -> HashMap<String, String> {
    lm_account::url::query_pairs(url.split_once('?').map_or("", |(_, q)| q)).unwrap().into_iter().collect()
}

/// The browser: follows the redirect to the loopback with `extra` appended, returns the status line.
fn browse(redirect: &str, extra: &str) -> String {
    let rest = redirect.strip_prefix("http://127.0.0.1:").unwrap();
    let (port, path) = rest.split_once('/').unwrap();
    let mut s = TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap())).unwrap();
    write!(s, "GET /{path}{extra} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
    let mut r = String::new();
    let _ = s.read_to_string(&mut r);
    r.lines().next().unwrap_or("").to_string()
}

fn requests(mock: &Arc<Mutex<Mock>>) -> usize {
    mock.lock().unwrap().seen.len()
}

#[test]
fn sign_in_refresh_reuse_and_delete() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 1, ..Default::default() }));
    let base = start(mock.clone());
    let store = Arc::new(MemoryStore::default());
    let account = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), store.clone());

    // The authorize URL: the provider, the loopback redirect with a state, the S256 challenge.
    let sign_in = account.begin(Region::International, &Method::Provider(Provider::Google)).unwrap();
    let url = sign_in.url().unwrap().to_string();
    assert!(url.starts_with(&format!("{base}/auth/v1/authorize?")), "{url}");
    let q = query(&url);
    assert_eq!(q["provider"], "google");
    assert_eq!(q["code_challenge_method"], "s256");
    let redirect = q["redirect_to"].clone();
    assert!(redirect.starts_with("http://127.0.0.1:") && redirect.contains("/callback?state="), "{redirect}");
    mock.lock().unwrap().challenge = q["code_challenge"].clone();

    // The browser: a request with another state is turned away, then the real redirect.
    let wrong = redirect.split("state=").next().unwrap().to_string() + "state=forged";
    let browser = std::thread::spawn(move || (browse(&wrong, "&code=evil"), browse(&redirect, "&code=code-123")));
    let user = account.complete(sign_in, &AtomicBool::new(false), Duration::from_secs(20)).unwrap();
    let (forged, real) = browser.join().unwrap();
    assert_eq!((forged.as_str(), real.as_str()), ("HTTP/1.1 400 Bad Request", "HTTP/1.1 200 OK"));
    assert_eq!(user.email.as_deref(), Some("kim@example.com"));
    assert_eq!(account.user().map(|(u, r)| (u.provider, r)), Some(("google".into(), Region::International)));

    // Stored: the refresh token, never the access token.
    let rec = store.load().unwrap().unwrap();
    let stored = Session::from_record(&rec).unwrap();
    assert_eq!(stored.refresh, "refresh-1");
    assert_eq!(stored.access, None);

    // The access token expires in a second, inside the refresh margin: the next use refreshes,
    // and the rotated refresh token replaces the stored one.
    mock.lock().unwrap().expires_in = 3600;
    assert_eq!(account.access_token().unwrap(), "access-2");
    assert_eq!(Session::from_record(&store.load().unwrap().unwrap()).unwrap().refresh, "refresh-2");
    let before = requests(&mock);
    assert_eq!(account.access_token().unwrap(), "access-2");
    assert_eq!(requests(&mock), before, "a fresh token needs no request");

    // Another copy of the old record (refresh-1, already used): the service refuses it, and that
    // copy signs out and forgets it.
    let old_store = Arc::new(MemoryStore::with(&rec));
    let old = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), old_store.clone());
    assert!(old.restore().is_some());
    assert_eq!(old.access_token(), Err(AccountError::SignedOut));
    assert!(old.user().is_none());
    assert_eq!(old_store.load().unwrap(), None);

    // Delete: the account function is called with the access token; then everything is gone here.
    account.delete_account().unwrap();
    assert!(account.user().is_none());
    assert_eq!(store.load().unwrap(), None);
    let m = mock.lock().unwrap();
    let del = m.seen.iter().find(|(meth, ..)| meth == "DELETE").unwrap();
    assert_eq!(del.1, "/functions/v1/account");
    assert_eq!(del.2.get("authorization").map(String::as_str), Some("Bearer access-2"));
    // The verifier went only to the token endpoint, and nothing carried the refresh token in a URL.
    for (_, path, _, body) in &m.seen {
        assert!(!path.contains("refresh-"), "{path}");
        if !path.contains("grant_type=pkce") {
            assert!(!body.contains("code_verifier"), "{body}");
        }
    }
}

/// After signing in the app asks the account function whether Pro is on (the entitlement), keeps
/// the answer with the stored session (so an offline launch stays unlocked), and follows the
/// server when it changes: the owner flipping `profiles.pro` needs no app update.
#[test]
fn the_entitlement_comes_from_the_account_function_and_is_kept() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, pro: Some(true), ..Default::default() }));
    let base = start(mock.clone());
    let store = Arc::new(MemoryStore::default());
    let account = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), store.clone());
    assert_eq!(account.access(false), lm_account::Access::SignedOut);
    let sign_in = account.begin(Region::International, &Method::Provider(Provider::Google)).unwrap();
    let q = query(sign_in.url().unwrap());
    mock.lock().unwrap().challenge = q["code_challenge"].clone();
    let redirect = q["redirect_to"].clone();
    let browser = std::thread::spawn(move || browse(&redirect, "&code=code-123"));
    account.complete(sign_in, &AtomicBool::new(false), Duration::from_secs(20)).unwrap();
    browser.join().unwrap();
    assert_eq!(account.access(false), lm_account::Access::Checking);

    let e = account.refresh_entitlement().unwrap();
    assert!(e.pro && e.plan.is_none());
    assert_eq!(account.access(false), lm_account::Access::Unlocked);
    {
        let m = mock.lock().unwrap();
        let get = m.seen.iter().find(|(meth, ..)| meth == "GET").unwrap();
        assert_eq!(get.1, "/functions/v1/account");
        assert_eq!(get.2.get("authorization").map(String::as_str), Some("Bearer access-1"));
        assert_eq!(get.2.get("apikey").map(String::as_str), Some("anon-key"));
    }

    // Kept with the session: another launch, offline, is still unlocked.
    let rec = store.load().unwrap().unwrap();
    assert_eq!(Session::from_record(&rec).and_then(|s| s.entitlement).map(|e| e.pro), Some(true));
    let later = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), Arc::new(MemoryStore::with(&rec)));
    later.restore();
    lm_net::set_offline(true);
    let offline = later.refresh_entitlement();
    let unlocked = later.access(false);
    lm_net::set_offline(false);
    assert_eq!(offline, Err(AccountError::Offline));
    assert_eq!(unlocked, lm_account::Access::Unlocked);

    // The server turns Pro off (later: payment lapsed): the next check locks it.
    mock.lock().unwrap().pro = Some(false);
    assert!(!account.refresh_entitlement().unwrap().pro);
    assert_eq!(account.access(false), lm_account::Access::NotPro);
    // No profile at all counts as no Pro.
    mock.lock().unwrap().pro = None;
    assert!(!account.refresh_entitlement().unwrap().pro);
    // A refresh of the access token keeps the entitlement.
    mock.lock().unwrap().pro = Some(true);
    account.refresh_entitlement().unwrap();
    let rec = store.load().unwrap().unwrap();
    let short_store = Arc::new(MemoryStore::with(&rec));
    let short = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), short_store.clone());
    short.restore();
    // (The restored session has no access token: this is a refresh, with token rotation.)
    short.access_token().unwrap();
    assert_eq!(short.access(false), lm_account::Access::Unlocked);
    let rotated = Session::from_record(&short_store.load().unwrap().unwrap()).unwrap();
    assert_ne!(rotated.refresh, Session::from_record(&rec).unwrap().refresh);
    assert!(rotated.entitlement.is_some_and(|e| e.pro));
}

#[test]
fn sign_out_revokes_at_the_service_and_forgets_here() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, ..Default::default() }));
    let base = start(mock.clone());
    let store = Arc::new(MemoryStore::default());
    let account = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), store.clone());
    let sign_in = account.begin(Region::International, &Method::Provider(Provider::Apple)).unwrap();
    let q = query(sign_in.url().unwrap());
    assert_eq!(q["provider"], "apple");
    mock.lock().unwrap().challenge = q["code_challenge"].clone();
    let redirect = q["redirect_to"].clone();
    let browser = std::thread::spawn(move || browse(&redirect, "&code=code-123"));
    account.complete(sign_in, &AtomicBool::new(false), Duration::from_secs(20)).unwrap();
    browser.join().unwrap();

    account.sign_out().unwrap();
    assert!(account.user().is_none());
    assert_eq!(store.load().unwrap(), None);
    let m = mock.lock().unwrap();
    let out = m.seen.iter().find(|(_, p, ..)| p.starts_with("/auth/v1/logout")).unwrap();
    assert_eq!(out.1, "/auth/v1/logout?scope=local");
    assert_eq!(out.2.get("authorization").map(String::as_str), Some("Bearer access-1"));
}

#[test]
fn the_email_link_comes_back_to_the_loopback() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, ..Default::default() }));
    let base = start(mock.clone());
    let account = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), Arc::new(MemoryStore::default()));
    assert!(matches!(account.begin(Region::International, &Method::Email("not an address".into())), Err(AccountError::Denied(_))));
    let sign_in = account.begin(Region::International, &Method::Email(" kim@example.com ".into())).unwrap();
    assert_eq!(sign_in.url(), None, "the email carries the link");
    let (path, body) = {
        let m = mock.lock().unwrap();
        let otp = m.seen.iter().find(|(_, p, ..)| p.starts_with("/auth/v1/otp")).unwrap();
        (otp.1.clone(), otp.3.clone())
    };
    let j: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(j["email"], "kim@example.com");
    assert_eq!(j["code_challenge_method"], "s256");
    mock.lock().unwrap().challenge = j["code_challenge"].as_str().unwrap().to_string();
    // The link in the email: Supabase sends the browser to redirect_to with the code.
    let redirect = query(&path)["redirect_to"].clone();
    let browser = std::thread::spawn(move || browse(&redirect, "&code=code-123"));
    let user = account.complete(sign_in, &AtomicBool::new(false), Duration::from_secs(20)).unwrap();
    browser.join().unwrap();
    assert_eq!(user.email.as_deref(), Some("kim@example.com"));
}

#[test]
fn a_refused_provider_or_a_wrong_verifier_ends_the_sign_in() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, ..Default::default() }));
    let base = start(mock.clone());
    let store = Arc::new(MemoryStore::default());
    let account = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), store.clone());

    // The person cancels at the provider.
    let sign_in = account.begin(Region::International, &Method::Provider(Provider::X)).unwrap();
    assert_eq!(query(sign_in.url().unwrap())["provider"], "x");
    let redirect = query(sign_in.url().unwrap())["redirect_to"].clone();
    let browser = std::thread::spawn(move || browse(&redirect, "&error=access_denied&error_description=The+user+cancelled"));
    let r = account.complete(sign_in, &AtomicBool::new(false), Duration::from_secs(20));
    browser.join().unwrap();
    assert_eq!(r, Err(AccountError::Denied("The user cancelled".into())));

    // A code exchanged with another verifier (the mock holds a different challenge).
    let sign_in = account.begin(Region::International, &Method::Provider(Provider::Google)).unwrap();
    mock.lock().unwrap().challenge = "not-this-sign-ins-challenge".into();
    let redirect = query(sign_in.url().unwrap())["redirect_to"].clone();
    let browser = std::thread::spawn(move || browse(&redirect, "&code=code-123"));
    let r = account.complete(sign_in, &AtomicBool::new(false), Duration::from_secs(20));
    browser.join().unwrap();
    assert!(matches!(r, Err(AccountError::Denied(ref why)) if why.contains("code verifier")), "{r:?}");
    assert!(account.user().is_none());
    assert_eq!(store.load().unwrap(), None);
}

#[test]
fn offline_nothing_goes_out_and_sign_out_still_forgets() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, ..Default::default() }));
    let base = start(mock.clone());
    let store = Arc::new(MemoryStore::default());
    let account = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), store.clone());
    let stale = Session {
        user: lm_account::User { id: "u".into(), provider: "google".into(), email: None },
        region: Region::International,
        access: None,
        refresh: "refresh-x".into(),
        entitlement: None,
    };
    store.save(&stale.to_record()).unwrap();
    assert!(account.restore().is_some());

    lm_net::set_offline(true);
    let before = lm_net::counts();
    let begin = account.begin(Region::International, &Method::Provider(Provider::Google));
    let token = account.access_token();
    let delete = account.delete_account();
    let email = account.begin(Region::International, &Method::Email("kim@example.com".into()));
    account.sign_out().unwrap();
    let after = lm_net::counts();
    lm_net::set_offline(false);

    assert!(matches!(begin, Err(AccountError::Offline)));
    assert_eq!(token, Err(AccountError::Offline));
    assert_eq!(delete, Err(AccountError::Offline));
    assert!(matches!(email, Err(AccountError::Offline)));
    assert_eq!(requests(&mock), 0, "the mock saw nothing");
    assert_eq!(after.allowed_total(), before.allowed_total());
    assert!(after.refused[lm_net::Purpose::Account as usize] > before.refused[lm_net::Purpose::Account as usize]);
    // Signed out here even though the service wasn't told.
    assert!(account.user().is_none());
    assert_eq!(store.load().unwrap(), None);
}

/// An email sign-in: begin (the email is sent), then the link's wait on another thread, as the app
/// runs it.
fn email_sign_in(account: &Arc<Account>) -> std::thread::JoinHandle<Result<lm_account::User, AccountError>> {
    let sign_in = account.begin(Region::International, &Method::Email("kim@example.com".into())).unwrap();
    let a = account.clone();
    std::thread::spawn(move || a.complete(sign_in, &AtomicBool::new(false), Duration::from_secs(20)))
}

fn redirect_of_last_email(mock: &Arc<Mutex<Mock>>) -> String {
    let m = mock.lock().unwrap();
    let otp = m.seen.iter().rev().find(|(_, p, ..)| p.starts_with("/auth/v1/otp")).unwrap();
    query(&otp.1)["redirect_to"].clone()
}

#[test]
fn the_emails_code_signs_in_and_ends_the_wait_for_the_link() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, email_code: "123456".into(), ..Default::default() }));
    let base = start(mock.clone());
    let store = Arc::new(MemoryStore::default());
    let account = Arc::new(Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), store.clone()));
    let waiting = email_sign_in(&account);
    let redirect = redirect_of_last_email(&mock);

    // Typed with a space, as the email shows it.
    let user = account.verify_code(Region::International, " kim@example.com", "123 456").unwrap();
    assert_eq!(user.email.as_deref(), Some("kim@example.com"));
    let body = {
        let m = mock.lock().unwrap();
        let v = m.seen.iter().find(|(_, p, ..)| p == "/auth/v1/verify").unwrap();
        assert_eq!(v.2.get("apikey").map(String::as_str), Some("anon-key"));
        v.3.clone()
    };
    let j: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(j, serde_json::json!({"type": "email", "email": "kim@example.com", "token": "123456"}));

    // The link's wait stops on its own, promptly, and keeps nothing.
    let started = std::time::Instant::now();
    assert_eq!(waiting.join().unwrap(), Err(AccountError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(5));
    // Its listener is closed: the link opened now reaches nothing.
    let port: u16 = redirect.strip_prefix("http://127.0.0.1:").and_then(|r| r.split('/').next()).unwrap().parse().unwrap();
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "the loopback is closed");

    // One session: the code's, stored once; no code exchange was made.
    assert_eq!(Session::from_record(&store.load().unwrap().unwrap()).unwrap().refresh, "refresh-1");
    assert_eq!(account.access_token().unwrap(), "access-1");
    let m = mock.lock().unwrap();
    assert_eq!(m.issued, 1);
    assert!(!m.seen.iter().any(|(_, p, ..)| p.contains("grant_type=pkce")));
}

#[test]
fn a_wrong_code_leaves_the_link_waiting_and_the_link_still_wins() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, email_code: "123456".into(), ..Default::default() }));
    let base = start(mock.clone());
    let store = Arc::new(MemoryStore::default());
    let account = Arc::new(Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), store.clone()));

    // Not six digits: refused here, nothing sent.
    let before = requests(&mock);
    for bad in ["12345", "1234567", "12a456", ""] {
        assert_eq!(account.verify_code(Region::International, "kim@example.com", bad), Err(AccountError::CodeFormat), "{bad}");
    }
    assert_eq!(requests(&mock), before);

    let challenge = |mock: &Arc<Mutex<Mock>>| {
        let mut m = mock.lock().unwrap();
        let otp = m.seen.iter().rev().find(|(_, p, ..)| p.starts_with("/auth/v1/otp")).unwrap();
        let j: serde_json::Value = serde_json::from_str(&otp.3).unwrap();
        m.challenge = j["code_challenge"].as_str().unwrap().to_string();
    };
    let waiting = email_sign_in(&account);
    challenge(&mock);
    // A wrong code (the service answers 403), then one it can't read (400): the same plain words.
    let wrong = account.verify_code(Region::International, "kim@example.com", "654-321");
    assert_eq!(wrong, Err(AccountError::WrongCode));
    assert_eq!(wrong.unwrap_err().to_string(), "That code didn't work. Check it, or send a new email.");
    assert_eq!(account.verify_code(Region::International, "kim@example.com", "000000"), Err(AccountError::WrongCode));
    assert!(account.user().is_none());

    // The link still works afterwards, and then a late code changes nothing.
    let redirect = redirect_of_last_email(&mock);
    assert_eq!(browse(&redirect, "&code=code-123"), "HTTP/1.1 200 OK");
    let user = waiting.join().unwrap().unwrap();
    assert_eq!(user.email.as_deref(), Some("kim@example.com"));
    let issued = mock.lock().unwrap().issued;
    let late = account.verify_code(Region::International, "kim@example.com", "123456").unwrap();
    assert_eq!(late, user);
    assert_eq!(mock.lock().unwrap().issued, issued, "no second session");
    assert_eq!(Session::from_record(&store.load().unwrap().unwrap()).unwrap().refresh, "refresh-1");
}

#[test]
fn a_rejected_code_reads_plainly_and_too_many_attempts_say_so() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, email_code: "123456".into(), ..Default::default() }));
    let base = start(mock.clone());
    let account = Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), Arc::new(MemoryStore::default()));
    // A code that was already used is as wrong as a mistyped one.
    account.verify_code(Region::International, "kim@example.com", "123456").unwrap();
    account.sign_out().unwrap();
    assert_eq!(account.verify_code(Region::International, "kim@example.com", "123456"), Err(AccountError::WrongCode));

    mock.lock().unwrap().verify_limited = true;
    let r = account.verify_code(Region::International, "kim@example.com", "123456");
    assert_eq!(r, Err(AccountError::TooManyAttempts));
    assert!(r.unwrap_err().to_string().starts_with("Too many attempts"));
    assert!(account.user().is_none());

    // Offline, nothing goes out.
    let before = requests(&mock);
    lm_net::set_offline(true);
    let r = account.verify_code(Region::International, "kim@example.com", "123456");
    lm_net::set_offline(false);
    assert_eq!(r, Err(AccountError::Offline));
    assert_eq!(requests(&mock), before);
}

/// "Send a new email": the newer sign-in replaces the older one, whose wait stops.
#[test]
fn a_new_email_stops_the_older_wait() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    lm_net::set_offline(false);
    let mock = Arc::new(Mutex::new(Mock { expires_in: 3600, email_code: "123456".into(), ..Default::default() }));
    let base = start(mock.clone());
    let account = Arc::new(Account::new(config(&base), Arc::new(lm_account::http::GatedHttp), Arc::new(MemoryStore::default())));
    let first = email_sign_in(&account);
    let second = email_sign_in(&account);
    assert_eq!(first.join().unwrap(), Err(AccountError::Cancelled));
    account.verify_code(Region::International, "kim@example.com", "123456").unwrap();
    assert_eq!(second.join().unwrap(), Err(AccountError::Cancelled));
    assert!(account.user().is_some());
}
