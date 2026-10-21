//! The one network gate (WP6.6 Strict offline mode; Gate 5's network-blocking harness).
//!
//! Every network request NimbleMap makes asks [`allowed`] first, with what it is for: basemap
//! tiles and the daily-build lookup, the data library, the update check, Pro's assistant,
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
    /// The launch-time check for a new version (WP5.7).
    UpdateCheck,
    /// Pro's assistant service.
    Assistant,
    /// Pro's account: signing in, refreshing the session, signing out, deleting the account
    /// (`lm-account`, docs/lm/accounts-design.md).
    Account,
    /// Help › Send Feedback…: one message to the feedback function, sent only when the user
    /// presses Send (`lm_account::feedback`, docs/lm/feedback.md).
    Feedback,
}

impl Purpose {
    pub const ALL: [Purpose; 7] = [
        Purpose::Basemap,
        Purpose::Search,
        Purpose::Library,
        Purpose::UpdateCheck,
        Purpose::Assistant,
        Purpose::Account,
        Purpose::Feedback,
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
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

static OFFLINE: AtomicBool = AtomicBool::new(false);
static ALLOWED: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];
static REFUSED: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

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
    pub allowed: [u64; 7],
    pub refused: [u64; 7],
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
        }
        let after = counts();
        assert_eq!(after.allowed_total() - before.allowed_total(), 2);
        assert!(after.refused_total() - before.refused_total() >= 8);
        assert_eq!(after.lines().len(), 7);
        set_offline(false);
        assert!(!offline());
    }
}
