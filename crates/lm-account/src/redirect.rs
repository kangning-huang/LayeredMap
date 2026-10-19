//! The loopback redirect (RFC 8252 §7.3): a listener on `127.0.0.1` at a port the system picks,
//! for the length of one sign-in. The browser lands on `/callback?state=…&code=…`; only a request
//! with this sign-in's `state` is taken. Anything else gets an error page and the wait goes on.

use crate::url;

/// What a request to the loopback said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Callback {
    /// The authorization code, to exchange with the verifier.
    Code(String),
    /// The service or the provider refused (the person cancelled, or a setup problem): its reason.
    Error(String),
}

/// Why a request to the loopback isn't this sign-in's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejected {
    /// Not `GET /callback` (a favicon, a stray request).
    NotCallback,
    /// No `state`, a different one, or more than one.
    State,
    /// A query that can't be decoded, or a code that doesn't look like one.
    Malformed,
}

/// Reads a request line (`GET /callback?… HTTP/1.1`) against the state this sign-in sent.
pub fn parse_callback(request_line: &str, expected_state: &str) -> Result<Callback, Rejected> {
    let mut parts = request_line.split(' ');
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else { return Err(Rejected::NotCallback) };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/callback" {
        return Err(Rejected::NotCallback);
    }
    let pairs = url::query_pairs(query).ok_or(Rejected::Malformed)?;
    let values = |k: &str| pairs.iter().filter(|(n, _)| n == k).map(|(_, v)| v.as_str()).collect::<Vec<_>>();
    // The state first: nothing else in a request without this sign-in's state is believed.
    match values("state").as_slice() {
        [s] if !expected_state.is_empty() && same(s, expected_state) => {}
        _ => return Err(Rejected::State),
    }
    if let [e, ..] = values("error").as_slice() {
        let why = values("error_description").first().copied().filter(|d| !d.is_empty()).unwrap_or(e);
        return Ok(Callback::Error(why.chars().take(200).collect()));
    }
    match values("code").as_slice() {
        [c] if (1..=512).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b)) => {
            Ok(Callback::Code(c.to_string()))
        }
        [] => Ok(Callback::Error(lm_core::tn!("the sign-in service sent no code").into())),
        _ => Err(Rejected::Malformed),
    }
}

/// Compares in time independent of where the strings differ.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The page the browser shows once the app has its answer: plain, nothing loaded from outside.
pub fn page(ok: bool) -> String {
    let name = lm_core::brand::NAME;
    let (title, line) = if ok {
        ("You're signed in", format!("You can close this tab and go back to {name}."))
    } else {
        ("Sign-in didn't finish", format!("Go back to {name} and try again."))
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title>\
         <meta name=\"viewport\" content=\"width=device-width\"></head>\
         <body style=\"font-family:-apple-system,system-ui,sans-serif;margin:15vh auto;max-width:28em;text-align:center\">\
         <h1 style=\"font-weight:600\">{title}</h1><p>{line}</p></body></html>"
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::Loopback;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io::{Read as _, Write as _};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use super::{Callback, Rejected, page, parse_callback};
    use crate::AccountError;

    /// A listener on `127.0.0.1` for one sign-in.
    pub struct Loopback {
        listener: TcpListener,
        port: u16,
    }

    impl Loopback {
        /// Binds `127.0.0.1` at a port the system picks (never all interfaces: no firewall prompt,
        /// nothing reachable from the network).
        pub fn bind() -> std::io::Result<Loopback> {
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            listener.set_nonblocking(true)?;
            let port = listener.local_addr()?.port();
            Ok(Loopback { listener, port })
        }

        pub fn port(&self) -> u16 {
            self.port
        }

        /// Waits for the browser's redirect with `state`, until `deadline` or `cancel`.
        pub fn wait(&self, state: &str, deadline: Instant, cancel: &AtomicBool) -> Result<String, AccountError> {
            self.wait_until(state, deadline, || cancel.load(Ordering::SeqCst))
        }

        /// The same, stopping as soon as `stopped` says so (checked every 50 ms).
        pub fn wait_until(&self, state: &str, deadline: Instant, stopped: impl Fn() -> bool) -> Result<String, AccountError> {
            loop {
                if stopped() {
                    return Err(AccountError::Cancelled);
                }
                if Instant::now() >= deadline {
                    return Err(AccountError::TimedOut);
                }
                match self.listener.accept() {
                    Ok((stream, _)) => {
                        if let Some(answer) = serve(stream, state) {
                            return answer;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                    Err(e) => return Err(AccountError::Listener(e.to_string())),
                }
            }
        }
    }

    /// Reads one request and answers it: `Some` when it ended the sign-in.
    fn serve(mut stream: TcpStream, state: &str) -> Option<Result<String, AccountError>> {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
        let mut buf = Vec::with_capacity(1024);
        let mut chunk = [0u8; 1024];
        // The request line and headers, at most 8 KB.
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 8192 {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(chunk.get(..n).unwrap_or_default()),
            }
        }
        let text = String::from_utf8_lossy(&buf);
        let line = text.lines().next().unwrap_or("");
        let (status, body, answer) = match parse_callback(line, state) {
            Ok(Callback::Code(code)) => ("200 OK", page(true), Some(Ok(code))),
            Ok(Callback::Error(why)) => ("200 OK", page(false), Some(Err(AccountError::Denied(why)))),
            Err(Rejected::NotCallback) => ("404 Not Found", String::new(), None),
            Err(Rejected::State | Rejected::Malformed) => ("400 Bad Request", page(false), None),
        };
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(body.as_bytes()));
        let _ = stream.flush();
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = "Zm9vYmFyLXN0YXRl";

    #[test]
    fn the_right_state_gives_the_code() {
        let line = format!("GET /callback?state={STATE}&code=8c1f-2b.~_x HTTP/1.1");
        assert_eq!(parse_callback(&line, STATE), Ok(Callback::Code("8c1f-2b.~_x".into())));
        // Order doesn't matter.
        let line = format!("GET /callback?code=abc&state={STATE} HTTP/1.1");
        assert_eq!(parse_callback(&line, STATE), Ok(Callback::Code("abc".into())));
    }

    #[test]
    fn a_wrong_missing_or_doubled_state_is_rejected() {
        for q in [
            "code=abc".to_string(),
            "state=other&code=abc".to_string(),
            format!("state={STATE}x&code=abc"),
            format!("state={}&code=abc", &STATE[1..]),
            format!("state={STATE}&state=other&code=abc"),
            format!("state={STATE}&state={STATE}&code=abc"),
        ] {
            assert_eq!(parse_callback(&format!("GET /callback?{q} HTTP/1.1"), STATE), Err(Rejected::State), "{q}");
        }
        // An empty expected state never matches.
        assert_eq!(parse_callback("GET /callback?state=&code=abc HTTP/1.1", ""), Err(Rejected::State));
        // An error without the state is not believed either.
        assert_eq!(parse_callback("GET /callback?error=access_denied HTTP/1.1", STATE), Err(Rejected::State));
    }

    #[test]
    fn other_requests_are_not_the_callback() {
        for line in
            ["GET /favicon.ico HTTP/1.1", "POST /callback?state=x&code=y HTTP/1.1", "GET /callbackx?state=x HTTP/1.1", "", "garbage"]
        {
            assert_eq!(parse_callback(line, STATE), Err(Rejected::NotCallback), "{line}");
        }
    }

    #[test]
    fn errors_and_odd_codes() {
        let line = format!("GET /callback?state={STATE}&error=access_denied&error_description=The+user+cancelled HTTP/1.1");
        assert_eq!(parse_callback(&line, STATE), Ok(Callback::Error("The user cancelled".into())));
        let line = format!("GET /callback?state={STATE}&error=server_error HTTP/1.1");
        assert_eq!(parse_callback(&line, STATE), Ok(Callback::Error("server_error".into())));
        let line = format!("GET /callback?state={STATE} HTTP/1.1");
        assert!(matches!(parse_callback(&line, STATE), Ok(Callback::Error(_))));
        for code in ["a%20b", "a%3Cscript%3E", &"a".repeat(513)] {
            let line = format!("GET /callback?state={STATE}&code={code} HTTP/1.1");
            assert_eq!(parse_callback(&line, STATE), Err(Rejected::Malformed), "{code}");
        }
        let line = format!("GET /callback?state={STATE}&code=a&code=b HTTP/1.1");
        assert_eq!(parse_callback(&line, STATE), Err(Rejected::Malformed));
        assert_eq!(parse_callback("GET /callback?state=%zz HTTP/1.1", STATE), Err(Rejected::Malformed));
    }

    #[test]
    fn the_pages_name_the_app_and_load_nothing() {
        for ok in [true, false] {
            let p = page(ok);
            assert!(p.contains(lm_core::brand::NAME));
            assert!(!p.contains("http") && !p.contains("<script"), "{p}");
        }
    }

    /// A stray request and a wrong state don't end the wait; the right one does.
    #[test]
    fn the_listener_waits_for_this_sign_ins_state() {
        use std::io::{Read as _, Write as _};
        let lb = Loopback::bind().unwrap();
        let port = lb.port();
        let browser = std::thread::spawn(move || {
            let mut answers = Vec::new();
            for path in ["/favicon.ico".to_string(), "/callback?state=wrong&code=evil".into(), format!("/callback?state={STATE}&code=good")]
            {
                let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
                let mut r = String::new();
                s.read_to_string(&mut r).unwrap();
                answers.push(r.lines().next().unwrap_or("").to_string());
            }
            answers
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let code = lb.wait(STATE, deadline, &std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert_eq!(code, "good");
        let answers = browser.join().unwrap();
        assert_eq!(answers, ["HTTP/1.1 404 Not Found", "HTTP/1.1 400 Bad Request", "HTTP/1.1 200 OK"]);
    }

    #[test]
    fn the_listener_stops_on_cancel_and_deadline() {
        let lb = Loopback::bind().unwrap();
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        let later = std::time::Instant::now() + std::time::Duration::from_secs(10);
        assert!(matches!(lb.wait(STATE, later, &cancelled), Err(crate::AccountError::Cancelled)));
        let past = std::time::Instant::now();
        assert!(matches!(lb.wait(STATE, past, &std::sync::atomic::AtomicBool::new(false)), Err(crate::AccountError::TimedOut)));
    }
}
