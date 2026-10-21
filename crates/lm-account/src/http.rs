//! The account's HTTP requests, behind a trait so the flow can be tested against a local mock
//! server. The real client asks the network gate (`lm_net`, purpose Account) before each request,
//! so strict offline mode blocks sign-in, refresh, sign-out and delete. Help › Send Feedback… uses
//! the same client under its own purpose ([`FeedbackHttp`], purpose Feedback).

/// An answer: status and body (small JSON).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

pub trait Http: Send + Sync {
    /// Sends `method url` with `headers` and an optional JSON body.
    fn send(&self, method: &str, url: &str, headers: &[(&str, String)], body: Option<&str>) -> Result<Response, crate::AccountError>;
}

/// The real client: ureq through the network gate. https only, except to this computer.
#[cfg(not(target_arch = "wasm32"))]
pub struct GatedHttp;

#[cfg(not(target_arch = "wasm32"))]
impl Http for GatedHttp {
    fn send(&self, method: &str, url: &str, headers: &[(&str, String)], body: Option<&str>) -> Result<Response, crate::AccountError> {
        send_gated(lm_net::Purpose::Account, method, url, headers, body)
    }
}

/// The same client for Help › Send Feedback…, counted and refused by the gate as feedback
/// (`lm_net::Purpose::Feedback`), not as the account.
#[cfg(not(target_arch = "wasm32"))]
pub struct FeedbackHttp;

#[cfg(not(target_arch = "wasm32"))]
impl Http for FeedbackHttp {
    fn send(&self, method: &str, url: &str, headers: &[(&str, String)], body: Option<&str>) -> Result<Response, crate::AccountError> {
        send_gated(lm_net::Purpose::Feedback, method, url, headers, body)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn send_gated(
    purpose: lm_net::Purpose,
    method: &str,
    url: &str,
    headers: &[(&str, String)],
    body: Option<&str>,
) -> Result<Response, crate::AccountError> {
    use crate::AccountError;
    if !crate::config::allowed_url(url) {
        return Err(AccountError::Network(format!("{url}: only https addresses are used")));
    }
    lm_net::check(purpose).map_err(|_| AccountError::Offline)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .http_status_as_error(false)
        // concat! needs a literal: keep in step with lm_core::brand::NAME.
        .user_agent(concat!("NimbleMap/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let net = |e: ureq::Error| AccountError::Network(e.to_string());
    let mut resp = match method {
        "GET" | "DELETE" => {
            let mut r = if method == "GET" { agent.get(url) } else { agent.delete(url) };
            for (k, v) in headers {
                r = r.header(*k, v.as_str());
            }
            r.call().map_err(net)?
        }
        _ => {
            let mut r = agent.post(url).header("Content-Type", "application/json");
            for (k, v) in headers {
                r = r.header(*k, v.as_str());
            }
            r.send(body.unwrap_or("{}")).map_err(net)?
        }
    };
    let status = resp.status().as_u16();
    let body = resp.body_mut().with_config().limit(256 * 1024).read_to_string().unwrap_or_default();
    Ok(Response { status, body })
}
