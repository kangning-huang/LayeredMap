//! The update check (WP5.7): at launch the app reads a small JSON feed (`docs/lm/update-feed.md`)
//! on a background thread and, when it names a newer version, shows a quiet notice, "A new version
//! is available", with a download link. Nothing is downloaded or installed by the app.
//!
//! The feed's address is compiled in ([`FEED_URL`], from `LM_UPDATE_FEED_URL` at build time);
//! empty means the check is off. It goes through the network gate, so offline mode blocks it,
//! and without a network it simply fails quietly.

use std::sync::Mutex;

use egui::{Align2, RichText, pos2};
use egui_phosphor::regular as icon;

use crate::Shell;
use crate::theme::{self, CAPTION, PANEL_MARGIN};

/// Where the feed lives. Set at build time (`LM_UPDATE_FEED_URL=https://… cargo build`); the
/// China-reachable download host is the owner's choice (spec › WP5.7). Empty: no check.
pub const FEED_URL: &str = match option_env!("LM_UPDATE_FEED_URL") {
    Some(u) => u,
    None => "",
};

/// The feed format this build reads.
pub const FORMAT: u64 = 1;
/// The largest feed read.
const MAX_BYTES: u64 = 64 * 1024;

/// A newer version, as the feed describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub version: String,
    /// Where to download it (https only): the tier's own installer, else the download page.
    pub url: String,
    /// What is new, in the interface's language when the feed has it.
    pub notes: Option<String>,
}

/// Compares two versions written `MAJOR.MINOR.PATCH`, optionally with a `-pre` suffix (a
/// pre-release sorts before its release). Missing parts count as 0; `None` when either isn't a
/// version.
pub fn compare(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    fn parse(v: &str) -> Option<(Vec<u64>, Option<&str>)> {
        let v = v.trim().trim_start_matches('v');
        let (num, pre) = match v.split_once('-') {
            Some((n, p)) => (n, Some(p)),
            None => (v, None),
        };
        let parts: Option<Vec<u64>> = num.split('.').map(|p| p.parse().ok()).collect();
        let mut parts = parts.filter(|p| !p.is_empty() && p.len() <= 4)?;
        parts.resize(4, 0);
        Some((parts, pre))
    }
    let (na, pa) = parse(a)?;
    let (nb, pb) = parse(b)?;
    Some(na.cmp(&nb).then_with(|| match (pa, pb) {
        (None, None) => std::cmp::Ordering::Equal,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(x), Some(y)) => x.cmp(y),
    }))
}

/// Reads a feed: the newer version it names for this build (`current`, Free or `pro`), or `None`
/// when it names nothing newer, can't be read, is of a format this build doesn't know, offers no
/// https link, or needs a newer macOS than `macos` (when known).
pub fn parse(feed: &str, current: &str, pro: bool, macos: Option<&str>, chinese: bool) -> Option<Update> {
    let j: serde_json::Value = serde_json::from_str(feed).ok()?;
    if j.get("format")?.as_u64()? != FORMAT {
        return None;
    }
    let version = j.get("version")?.as_str()?.trim().to_string();
    if compare(&version, current)? != std::cmp::Ordering::Greater {
        return None;
    }
    if let (Some(min), Some(os)) = (j.get("minimum_macos").and_then(|v| v.as_str()), macos)
        && compare(os, min) == Some(std::cmp::Ordering::Less)
    {
        return None;
    }
    let https = |v: Option<&serde_json::Value>| v.and_then(|v| v.as_str()).map(str::trim).filter(|u| is_https(u)).map(str::to_string);
    let tier = if pro { "pro" } else { "free" };
    let url = https(j.get("download").and_then(|d| d.get(tier))).or_else(|| https(j.get("page")))?;
    let notes = j.get("notes").and_then(|n| {
        let pick = |k: &str| n.get(k).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        if chinese { pick("zh").or_else(|| pick("en")) } else { pick("en") }
    });
    Some(Update { version, url, notes })
}

/// An `https://` address with a host and nothing that could break out of a link.
fn is_https(u: &str) -> bool {
    u.strip_prefix("https://").is_some_and(|rest| {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !u.chars().any(|c| c.is_whitespace() || c.is_control())
    })
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Idle,
    Checking,
    Found(Update),
    Done,
}

static STATE: Mutex<State> = Mutex::new(State::Idle);

fn set(s: State) {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = s;
}

/// The newer version, once the check has found one.
pub fn found() -> Option<Update> {
    match &*STATE.lock().unwrap_or_else(|e| e.into_inner()) {
        State::Found(u) => Some(u.clone()),
        _ => None,
    }
}

/// Whether a started check has finished (found something or not).
pub fn finished() -> bool {
    matches!(*STATE.lock().unwrap_or_else(|e| e.into_inner()), State::Found(_) | State::Done)
}

/// The running macOS version (`sw_vers`), on the checking thread.
fn macos_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/bin/sw_vers").arg("-productVersion").output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Whether the system's first language is Chinese (for the feed's notes), on the checking
/// thread. The interface's own language setting (WP5.5) replaces this when it exists. Help ›
/// LayeredMap Help uses it too, to open the Chinese pages.
pub fn prefers_chinese() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.split(['"', ',', '(', ')']).map(str::trim).find(|l| !l.is_empty()).map(|l| l.starts_with("zh")))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Starts the launch check against the compiled-in feed, once per run (the app calls this; tests
/// don't). Off when [`FEED_URL`] is empty.
pub fn start(ctx: &egui::Context, pro: bool) {
    start_with(FEED_URL, ctx, pro);
}

/// [`start`] against a given feed address (the offline harness uses one that must never be
/// contacted).
pub fn start_with(feed_url: &str, ctx: &egui::Context, pro: bool) {
    if feed_url.trim().is_empty() {
        return;
    }
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if *s != State::Idle {
            return;
        }
        *s = State::Checking;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let url = feed_url.trim().to_string();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("lm-update-check".into()).spawn(move || {
            let chinese = prefers_chinese();
            let r = lm_net::get_text(lm_net::Purpose::UpdateCheck, &url, std::time::Duration::from_secs(10), MAX_BYTES);
            let found = match r {
                Ok(text) => parse(&text, env!("CARGO_PKG_VERSION"), pro, macos_version().as_deref(), chinese),
                Err(e) => {
                    // No network, offline mode, or a host that's down: skipped without a word.
                    log::info!("update check skipped: {e}");
                    None
                }
            };
            if let Some(u) = &found {
                log::info!("update check: {} is available", u.version);
            }
            set(found.map_or(State::Done, State::Found));
            ctx.request_repaint();
        });
        if let Err(e) = spawned {
            log::warn!("update check: {e}");
            set(State::Done);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (ctx, pro);
        set(State::Done);
    }
}

/// The quiet notice in the map's lower right, above the attribution, until closed.
pub(crate) fn notice(shell: &Shell, ui: &egui::Ui, map: egui::Rect) {
    let Some(u) = found() else { return };
    let closed_id = egui::Id::new("lm-update-closed");
    if ui.ctx().data(|d| d.get_temp::<String>(closed_id)).as_deref() == Some(u.version.as_str()) {
        return;
    }
    let t = *shell.theme();
    let pos = pos2(map.right() - PANEL_MARGIN, map.bottom() - 34.0);
    egui::Area::new(egui::Id::new("lm-update")).pivot(Align2::RIGHT_BOTTOM).fixed_pos(pos).show(ui.ctx(), |ui| {
        crate::chrome::panel_frame(&t).show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label(RichText::new("A new version is available").size(CAPTION).color(t.text_primary));
                if let Some(n) = &u.notes {
                    label.on_hover_text(format!("LayeredMap {}: {n}", u.version));
                }
                ui.hyperlink_to(RichText::new(format!("Download {}", u.version)).size(CAPTION), &u.url);
                let close = egui::Button::new(RichText::new(icon::X).font(theme::icon_font(11.0))).frame(false);
                if ui.add(close).on_hover_text("Close").clicked() {
                    ui.ctx().data_mut(|d| d.insert_temp(closed_id, u.version.clone()));
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering::*;

    use super::*;

    #[test]
    fn versions_compare_numerically_with_prereleases_first() {
        assert_eq!(compare("0.10.0", "0.9.9"), Some(Greater));
        assert_eq!(compare("1.0", "1.0.0"), Some(Equal));
        assert_eq!(compare("v1.2.3", "1.2.3"), Some(Equal));
        assert_eq!(compare("1.0.0-beta.1", "1.0.0"), Some(Less));
        assert_eq!(compare("1.0.0-beta.2", "1.0.0-beta.1"), Some(Greater));
        assert_eq!(compare("one", "1.0"), None);
        assert_eq!(compare("", "1.0"), None);
    }

    const FEED: &str = r#"{
        "format": 1,
        "version": "0.3.0",
        "released": "2026-11-02",
        "notes": { "en": "Saving and exporting.", "zh": "保存与导出。" },
        "download": {
            "free": "https://download.example.cn/LayeredMap-0.3.0.dmg",
            "pro": "https://download.example.cn/LayeredMap-Pro-0.3.0.dmg"
        },
        "page": "https://download.example.cn/",
        "minimum_macos": "13.0"
    }"#;

    #[test]
    fn a_newer_version_gives_the_tiers_download() {
        let u = parse(FEED, "0.2.0", false, Some("15.1"), false).unwrap();
        assert_eq!(u.version, "0.3.0");
        assert_eq!(u.url, "https://download.example.cn/LayeredMap-0.3.0.dmg");
        assert_eq!(u.notes.as_deref(), Some("Saving and exporting."));
        let pro = parse(FEED, "0.2.0", true, None, true).unwrap();
        assert_eq!(pro.url, "https://download.example.cn/LayeredMap-Pro-0.3.0.dmg");
        assert_eq!(pro.notes.as_deref(), Some("保存与导出。"));
    }

    #[test]
    fn nothing_newer_or_nothing_usable_shows_nothing() {
        assert_eq!(parse(FEED, "0.3.0", false, None, false), None);
        assert_eq!(parse(FEED, "0.4.0", false, None, false), None);
        // Too old a macOS for the new version.
        assert_eq!(parse(FEED, "0.2.0", false, Some("12.7"), false), None);
        // A format this build doesn't know.
        assert_eq!(parse(&FEED.replace("\"format\": 1", "\"format\": 2"), "0.2.0", false, None, false), None);
        // Only https links; with no tier link, the page; with neither, nothing.
        let http = FEED.replace("https://download.example.cn/LayeredMap-0.3.0.dmg", "http://download.example.cn/x.dmg");
        assert_eq!(parse(&http, "0.2.0", false, None, false).unwrap().url, "https://download.example.cn/");
        let none = r#"{"format":1,"version":"9.0","download":{"free":"javascript:alert(1)"},"page":"file:///etc"}"#;
        assert_eq!(parse(none, "0.2.0", false, None, false), None);
        assert_eq!(parse("not json", "0.2.0", false, None, false), None);
        assert_eq!(parse(r#"{"format":1}"#, "0.2.0", false, None, false), None);
        // Bare minimum: a version and a page.
        let min = r#"{"format":1,"version":"0.2.1","page":"https://example.org/get"}"#;
        assert_eq!(parse(min, "0.2.0", false, None, false).unwrap().notes, None);
        assert!(!is_https("https://"));
        assert!(!is_https("https://host/ with space"));
    }

    #[test]
    fn the_check_is_off_without_a_feed_address() {
        // This build has no feed compiled in unless LM_UPDATE_FEED_URL was set.
        if FEED_URL.is_empty() {
            start(&egui::Context::default(), false);
            assert_eq!(*STATE.lock().unwrap_or_else(|e| e.into_inner()), State::Idle);
        }
    }
}
