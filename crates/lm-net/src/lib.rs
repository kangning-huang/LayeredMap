//! The one network gate (WP6.6 Strict offline mode; Gate 5's network-blocking harness).
//!
//! Every network request NimbleMap makes asks [`allowed`] first, with what it is for: basemap
//! tiles and the daily-build lookup, the data library, the update check and the update's
//! download, Pro's assistant,
//! Pro's account sign-in and Help › Send Feedback….
//! When the user turns on offline mode ([`set_offline`]) every one of them is refused here, so
//! blocking is enforced in one place. The gate also counts what it allowed and refused, which is
//! how the harness proves that core tasks make no network calls.
//!
//! Requests made with [`get_text`] go through the gate on their own. Code with its own HTTP
//! client (the tile fetcher in `rustgis-tiles`, the assistant's transport) calls [`allowed`] or
//! [`check`] right before each request. `cargo xtask layers` fails when a NimbleMap source file
//! uses `ureq` without asking the gate.
//!
//! Place search is not here: it reads the bundled GeoNames index and never goes online.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// What a request is for (shown in diagnostics, and so a refusal can say what was blocked).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// Basemap tiles, imagery, and the lookup of the newest daily basemap build.
    Basemap,
    /// Online place search (none yet: search is local).
    Search,
    /// The data library's catalog and downloads (WP5.4).
    Library,
    /// The check for a new version (WP5.7): at launch, and NimbleMap › Check for Updates….
    UpdateCheck,
    /// Pro's assistant service.
    Assistant,
    /// Pro's account: signing in, refreshing the session, signing out, deleting the account
    /// (`lm-account`, docs/lm/accounts-design.md).
    Account,
    /// Help › Send Feedback…: one message to the feedback function, sent only when the user
    /// presses Send (`lm_account::feedback`, docs/lm/feedback.md).
    Feedback,
    /// Update Now: the new version's disk image, downloaded only when the user presses it
    /// (docs/lm/update-feed.md).
    UpdateDownload,
}

impl Purpose {
    pub const ALL: [Purpose; N] = [
        Purpose::Basemap,
        Purpose::Search,
        Purpose::Library,
        Purpose::UpdateCheck,
        Purpose::Assistant,
        Purpose::Account,
        Purpose::Feedback,
        Purpose::UpdateDownload,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Purpose::Basemap => "basemap",
            Purpose::Search => "search",
            Purpose::Library => "data library",
            Purpose::UpdateCheck => "update check",
            Purpose::Assistant => "assistant",
            Purpose::Account => "account service",
            Purpose::Feedback => "feedback service",
            Purpose::UpdateDownload => "update download",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// How many purposes there are.
const N: usize = 8;

static OFFLINE: AtomicBool = AtomicBool::new(false);
static ALLOWED: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static REFUSED: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];

/// Turns strict offline mode on or off (View › Work Offline). Takes effect for the next request.
pub fn set_offline(on: bool) {
    if OFFLINE.swap(on, Ordering::SeqCst) != on {
        log::info!("network: offline mode {}", if on { "on" } else { "off" });
    }
}

/// Whether strict offline mode is on.
pub fn offline() -> bool {
    OFFLINE.load(Ordering::SeqCst)
}

/// The gate. Every network request asks here first, right before it is sent: `false` means
/// don't send it. Each answer is counted.
pub fn allowed(purpose: Purpose) -> bool {
    if offline() {
        REFUSED[purpose.index()].fetch_add(1, Ordering::SeqCst);
        false
    } else {
        ALLOWED[purpose.index()].fetch_add(1, Ordering::SeqCst);
        true
    }
}

/// A request refused because offline mode is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offline(pub Purpose);

impl std::fmt::Display for Offline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{OFFLINE_MESSAGE} (the {} was not contacted)", self.0.label())
    }
}

impl std::error::Error for Offline {}

/// What a refused request reports.
// No internal dependencies here (the network gate), and a const can't use format!: keep the
// name in step with lm_core::brand::NAME.
pub const OFFLINE_MESSAGE: &str = "NimbleMap is working offline (View › Work Offline)";

/// [`allowed`] as a `Result`, for `?`.
pub fn check(purpose: Purpose) -> Result<(), Offline> {
    if allowed(purpose) { Ok(()) } else { Err(Offline(purpose)) }
}

/// Requests the gate allowed and refused since launch, per purpose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub allowed: [u64; N],
    pub refused: [u64; N],
}

impl Counts {
    pub fn allowed_total(&self) -> u64 {
        self.allowed.iter().sum()
    }

    pub fn refused_total(&self) -> u64 {
        self.refused.iter().sum()
    }

    /// Every purpose with its counts, for diagnostics: "basemap 120 allowed, 0 refused".
    pub fn lines(&self) -> Vec<String> {
        Purpose::ALL
            .iter()
            .map(|p| format!("{}: {} allowed, {} refused", p.label(), self.allowed[p.index()], self.refused[p.index()]))
            .collect()
    }
}

/// The counts so far.
pub fn counts() -> Counts {
    let mut c = Counts::default();
    for p in Purpose::ALL {
        c.allowed[p.index()] = ALLOWED[p.index()].load(Ordering::SeqCst);
        c.refused[p.index()] = REFUSED[p.index()].load(Ordering::SeqCst);
    }
    c
}

/// Fetches a small text document (a JSON feed) over HTTPS through the gate. Blocking: call it
/// on a background thread, never on the UI thread. `max_bytes` caps what is read.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_text(purpose: Purpose, url: &str, timeout: std::time::Duration, max_bytes: u64) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err(format!("{url}: only https addresses are fetched"));
    }
    check(purpose).map_err(|e| e.to_string())?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // concat! needs a literal: keep in step with lm_core::brand::NAME.
        .user_agent(concat!("NimbleMap/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut resp = agent.get(url).call().map_err(|e| e.to_string())?;
    resp.body_mut().with_config().limit(max_bytes).read_to_string().map_err(|e| e.to_string())
}

/// Downloads a large file (the update's disk image) over HTTPS through the gate, writing it to
/// `out`. Redirects are followed only to other `https://` addresses. `progress` is called after
/// each chunk with the bytes so far and the size when the server gave one; returning `false`
/// cancels the download. Blocking: call it on a background thread. At most `max_bytes` are read.
/// Returns the number of bytes written.
#[cfg(not(target_arch = "wasm32"))]
pub fn download(
    purpose: Purpose,
    url: &str,
    max_bytes: u64,
    out: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<u64, String> {
    use std::io::Read;
    use std::time::Duration;
    if !url.starts_with("https://") {
        return Err(format!("{url}: only https addresses are fetched"));
    }
    check(purpose).map_err(|e| e.to_string())?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        // No timeout for the whole body (a slow connection may take minutes); connecting and the
        // response's headers have to come promptly.
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .https_only(true)
        .max_redirects(5)
        .user_agent(concat!("NimbleMap/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let total = resp.body().content_length();
    if total.is_some_and(|t| t > max_bytes) {
        return Err(format!("{url}: larger than {max_bytes} bytes"));
    }
    let mut reader = resp.body_mut().with_config().limit(max_bytes).reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut got = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        if !progress(got, total) {
            return Err(CANCELLED.into());
        }
    }
    if total.is_some_and(|t| t != got) {
        return Err(format!("{url}: the download stopped early ({got} of {} bytes)", total.unwrap_or(0)));
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(got)
}

/// What [`download`] returns when its `progress` callback cancelled it.
pub const CANCELLED: &str = "cancelled";

#[cfg(test)]
mod tests {
    use super::*;

    // One test: the gate is process-wide, and tests run in parallel.
    #[test]
    fn offline_refuses_every_purpose_and_counts() {
        set_offline(false);
        let before = counts();
        assert!(allowed(Purpose::Basemap));
        assert!(check(Purpose::Assistant).is_ok());
        set_offline(true);
        assert!(offline());
        for p in Purpose::ALL {
            assert!(!allowed(p), "{p:?}");
        }
        let e = check(Purpose::UpdateCheck).unwrap_err();
        assert!(e.to_string().contains("offline") && e.to_string().contains("update check"), "{e}");
        #[cfg(not(target_arch = "wasm32"))]
        {
            let r = get_text(Purpose::Library, "https://example.invalid/catalog.json", std::time::Duration::from_secs(1), 1024);
            assert!(r.unwrap_err().contains("offline"));
            // Plain http is never fetched, whatever the mode.
            assert!(get_text(Purpose::Library, "http://example.invalid/", std::time::Duration::from_secs(1), 1).is_err());
            let mut sink = Vec::new();
            let r = download(Purpose::UpdateDownload, "https://example.invalid/x.dmg", 1024, &mut sink, &mut |_, _| true);
            assert!(r.unwrap_err().contains("offline"));
            let r = download(Purpose::UpdateDownload, "http://example.invalid/x.dmg", 1024, &mut sink, &mut |_, _| true);
            assert!(r.unwrap_err().contains("only https"));
        }
        let after = counts();
        assert_eq!(after.allowed_total() - before.allowed_total(), 2);
        assert!(after.refused_total() - before.refused_total() >= 8);
        assert_eq!(after.lines().len(), Purpose::ALL.len());
        set_offline(false);
        assert!(!offline());
    }
}
