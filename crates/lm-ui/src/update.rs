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
use lm_core::{i18n, tl};

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
    /// Where to download it (https only): the app's installer, else the download page.
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

/// The feed's downloads for this platform: `download` holds the macOS disk images and
/// `download_windows` the Windows installers (WP6.2; older builds ignore the field).
pub const DOWNLOAD_KEY: &str = if cfg!(windows) { "download_windows" } else { "download" };

/// The feed's key for the app's download. There is one app; the key keeps the name it had when
/// there were two, so feeds stay readable by builds already out there (docs/lm/update-feed.md).
const APP_KEY: &str = "free";

/// Reads a feed: the newer version it names than `current`, or `None` when it names nothing newer, can't be read, is of a format this build doesn't know, offers no
/// https link, or needs a newer macOS than `macos` (when known).
pub fn parse(feed: &str, current: &str, macos: Option<&str>, chinese: bool) -> Option<Update> {
    parse_for(DOWNLOAD_KEY, feed, current, macos, chinese)
}

/// [`parse`] with the downloads under `download_key` (a Windows build never links a .dmg).
fn parse_for(download_key: &str, feed: &str, current: &str, macos: Option<&str>, chinese: bool) -> Option<Update> {
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
    let url = https(j.get(download_key).and_then(|d| d.get(APP_KEY))).or_else(|| https(j.get("page")))?;
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
/// NimbleMap Help uses it too, to open the Chinese pages.
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
    // Windows: the first of Settings › Time & language › Language & region › Preferred languages.
    #[cfg(windows)]
    {
        let profile =
            winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Control Panel\International\User Profile").ok();
        let first: Option<String> =
            profile.and_then(|k| k.get_value::<Vec<String>, _>("Languages").ok()).and_then(|l| l.into_iter().next());
        first.is_some_and(|l| l.starts_with("zh"))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        false
    }
}

/// Starts the launch check against the compiled-in feed, once per run (the app calls this; tests
/// don't). Off when [`FEED_URL`] is empty.
pub fn start(ctx: &egui::Context) {
    start_with(FEED_URL, ctx);
}

/// [`start`] against a given feed address (the offline harness uses one that must never be
/// contacted).
pub fn start_with(feed_url: &str, ctx: &egui::Context) {
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
                Ok(text) => parse(&text, env!("CARGO_PKG_VERSION"), macos_version().as_deref(), chinese),
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
        let _ = ctx;
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
                let label = ui.label(RichText::new(tl!("A new version is available")).size(CAPTION).color(t.text_primary));
                if let Some(n) = &u.notes {
                    label.on_hover_text(i18n::fmt(
                        "{app} {version}: {notes}",
                        &[("app", lm_core::brand::shown(false)), ("version", &u.version), ("notes", n)],
                    ));
                }
                ui.hyperlink_to(RichText::new(i18n::fmt("Download {version}", &[("version", &u.version)])).size(CAPTION), &u.url);
                let close = ui.add(egui::Button::new(RichText::new(icon::X).font(theme::icon_font(11.0))).frame(false));
                crate::a11y::button(&close, tl!("Close the update notice"));
                if close.on_hover_text(tl!("Close")).clicked() {
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

    /// The macOS reading of a feed, on any system.
    fn mac(feed: &str, current: &str, macos: Option<&str>, chinese: bool) -> Option<Update> {
        parse_for("download", feed, current, macos, chinese)
    }

    const FEED: &str = r#"{
        "format": 1,
        "version": "0.3.0",
        "released": "2026-11-02",
        "notes": { "en": "Saving and exporting.", "zh": "保存与导出。" },
        "download": {
            "free": "https://download.example.cn/NimbleMap-0.3.0.dmg",
            "pro": "https://download.example.cn/NimbleMap-Pro-0.3.0.dmg"
        },
        "page": "https://download.example.cn/",
        "minimum_macos": "13.0"
    }"#;

    #[test]
    fn a_newer_version_gives_the_apps_download() {
        let u = mac(FEED, "0.2.0", Some("15.1"), false).unwrap();
        assert_eq!(u.version, "0.3.0");
        assert_eq!(u.url, "https://download.example.cn/NimbleMap-0.3.0.dmg");
        assert_eq!(u.notes.as_deref(), Some("Saving and exporting."));
        // One app: the old Pro entry is never read.
        let zh = mac(FEED, "0.2.0", None, true).unwrap();
        assert_eq!(zh.url, "https://download.example.cn/NimbleMap-0.3.0.dmg");
        assert_eq!(zh.notes.as_deref(), Some("保存与导出。"));
    }

    /// Windows reads its own installers, and the page when the feed has none: never a .dmg.
    #[test]
    fn windows_links_its_installer_or_the_page() {
        let u = parse_for("download_windows", FEED, "0.2.0", None, false).unwrap();
        assert_eq!(u.url, "https://download.example.cn/");
        let win = FEED.replace(
            "\"page\":",
            r#""download_windows": { "free": "https://download.example.cn/NimbleMap-0.3.0-x64-setup.exe" }, "page":"#,
        );
        let u = parse_for("download_windows", &win, "0.2.0", None, false).unwrap();
        assert_eq!(u.url, "https://download.example.cn/NimbleMap-0.3.0-x64-setup.exe");
        assert_eq!(DOWNLOAD_KEY, if cfg!(windows) { "download_windows" } else { "download" });
    }

    #[test]
    fn nothing_newer_or_nothing_usable_shows_nothing() {
        assert_eq!(mac(FEED, "0.3.0", None, false), None);
        assert_eq!(mac(FEED, "0.4.0", None, false), None);
        // Too old a macOS for the new version.
        assert_eq!(mac(FEED, "0.2.0", Some("12.7"), false), None);
        // A format this build doesn't know.
        assert_eq!(mac(&FEED.replace("\"format\": 1", "\"format\": 2"), "0.2.0", None, false), None);
        // Only https links; with no tier link, the page; with neither, nothing.
        let http = FEED.replace("https://download.example.cn/NimbleMap-0.3.0.dmg", "http://download.example.cn/x.dmg");
        assert_eq!(mac(&http, "0.2.0", None, false).unwrap().url, "https://download.example.cn/");
        let none = r#"{"format":1,"version":"9.0","download":{"free":"javascript:alert(1)"},"page":"file:///etc"}"#;
        assert_eq!(mac(none, "0.2.0", None, false), None);
        assert_eq!(mac("not json", "0.2.0", None, false), None);
        assert_eq!(mac(r#"{"format":1}"#, "0.2.0", None, false), None);
        // Bare minimum: a version and a page.
        let min = r#"{"format":1,"version":"0.2.1","page":"https://example.org/get"}"#;
        assert_eq!(mac(min, "0.2.0", None, false).unwrap().notes, None);
        assert!(!is_https("https://"));
        assert!(!is_https("https://host/ with space"));
    }

    #[test]
    fn the_check_is_off_without_a_feed_address() {
        // This build has no feed compiled in unless LM_UPDATE_FEED_URL was set.
        if FEED_URL.is_empty() {
            start(&egui::Context::default());
            assert_eq!(*STATE.lock().unwrap_or_else(|e| e.into_inner()), State::Idle);
        }
    }
}
