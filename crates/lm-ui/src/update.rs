//! Updates (WP5.7, docs/lm/update-feed.md): at launch, and when the user chooses NimbleMap ›
//! Check for Updates…, the app reads a small JSON feed on a background thread and, when it names a
//! newer version, shows a quiet notice in the map's lower right, "A new version is available",
//! with a download link and, on macOS, Update Now.
//!
//! Update Now downloads the new disk image in the background (a progress bar, cancellable),
//! checks the feed's Ed25519 signature over the version and the image's SHA-256 against the key
//! compiled in ([`verify`]), checks the image against that SHA-256, copies the new app next to the
//! running one ([`install`]) and, once the window has closed the usual way (unsaved changes are
//! asked about first), swaps it in and opens it. Where the app can't replace itself, the disk image
//! opens in Finder instead. Windows keeps the link.
//!
//! The feed's addresses are compiled in ([`FEED_URL`] and [`FEED_FALLBACK_URL`], from
//! `LM_UPDATE_FEED_URL` and `LM_UPDATE_FEED_FALLBACK_URL` at build time; the release build sets
//! both by default). With neither, the check is off (development builds). Every request goes
//! through the network gate, so offline mode blocks it; without a network the launch check fails
//! quietly.
//!
//! The built-in example maps and world layers ship inside the app, so they update with it; the
//! online basemap is always current. There is no separate data updater.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use egui::{Align2, RichText, pos2};
use egui_phosphor::regular as icon;
use lm_core::{i18n, tl};

use crate::Shell;
use crate::theme::{self, CAPTION, PANEL_MARGIN};

mod fetch;
pub mod install;
pub mod verify;

/// Where the feed lives: `https://nimblemap.com/update/feed.json` in release builds
/// (`packaging/lm/bundle-macos.sh` sets `LM_UPDATE_FEED_URL`). Empty: no check.
pub const FEED_URL: &str = match option_env!("LM_UPDATE_FEED_URL") {
    Some(u) => u,
    None => "",
};

/// The same feed on GitCode, read when the first can't be (mainland China may not reach
/// nimblemap.com, which GitHub Pages serves). From `LM_UPDATE_FEED_FALLBACK_URL`.
pub const FEED_FALLBACK_URL: &str = match option_env!("LM_UPDATE_FEED_FALLBACK_URL") {
    Some(u) => u,
    None => "",
};

/// The feed format this build reads.
pub const FORMAT: u64 = 1;

/// The running version.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// The feed addresses this build reads, in order (usable ones only, without repeats).
pub fn feed_urls() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for u in [FEED_URL.trim(), FEED_FALLBACK_URL.trim()] {
        if fetch::usable(u) && !v.contains(&u) {
            v.push(u);
        }
    }
    v
}

/// Whether this build checks for updates at all (the menu shows Check for Updates…).
pub fn available() -> bool {
    !feed_urls().is_empty()
}

/// A newer version, as the feed describes it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Update {
    pub version: String,
    /// The link the notice shows (https only): the app's installer, else the download page. The
    /// GitCode mirror's when the interface is in Chinese and the feed has one.
    pub url: String,
    /// What is new, in the interface's language when the feed has it.
    pub notes: Option<String>,
    /// The disk image on GitHub (the feed's `download.free`).
    pub github: Option<String>,
    /// The same disk image on GitCode (`download.gitcode`), for mainland China.
    pub gitcode: Option<String>,
    /// The disk image's SHA-256 (`sha256.free`, lowercase hex).
    pub sha256: Option<String>,
    /// The Ed25519 signature over the version and the SHA-256 (`signature`, base64).
    pub signature: Option<String>,
}

impl Update {
    /// Whether Update Now can install this one: on macOS, with a signature, a checksum and a disk
    /// image to download, and a key compiled in to check the signature with.
    pub fn installable(&self) -> bool {
        cfg!(target_os = "macos")
            && self.sha256.is_some()
            && self.signature.is_some()
            && (self.github.is_some() || self.gitcode.is_some())
            && verify::has_key()
    }

    /// Where to download the disk image from, in the order to try: GitCode first when the
    /// interface is in Chinese or the feed only came through its GitCode copy, else GitHub first.
    /// The other is tried when the first can't be reached.
    pub fn sources(&self, chinese: bool, via_mirror: bool) -> Vec<String> {
        let (first, second) = if chinese || via_mirror { (&self.gitcode, &self.github) } else { (&self.github, &self.gitcode) };
        [first, second].into_iter().flatten().cloned().collect()
    }
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
/// The key for the same file on the GitCode mirror (builds before 0.7.2 ignore it).
const MIRROR_KEY: &str = "gitcode";

/// Reads a feed: the newer version it names than `current`, or `None` when it names nothing newer, can't be read, is of a format this build doesn't know, offers no
/// https link, or needs a newer macOS than `macos` (when known). `chinese`: the interface is in
/// Chinese (the notes and the link prefer Chinese and GitCode).
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
    let link = |v: Option<&serde_json::Value>| v.and_then(|v| v.as_str()).map(str::trim).filter(|u| fetch::usable(u)).map(str::to_string);
    let downloads = j.get(download_key);
    let github = link(downloads.and_then(|d| d.get(APP_KEY)));
    let gitcode = link(downloads.and_then(|d| d.get(MIRROR_KEY)));
    let page = link(j.get("page"));
    let url = if chinese { gitcode.clone().or_else(|| github.clone()) } else { github.clone() }
        .or_else(|| page.clone())
        .or_else(|| gitcode.clone())?;
    let notes = j.get("notes").and_then(|n| {
        let pick = |k: &str| n.get(k).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        if chinese { pick("zh").or_else(|| pick("en")) } else { pick("en") }
    });
    let sha256 = j
        .get("sha256")
        .and_then(|s| s.get(APP_KEY))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| verify::is_sha256(s));
    let signature = j.get("signature").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    Some(Update { version, url, notes, github, gitcode, sha256, signature })
}

/// Why Update Now stopped without installing.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// View › Work Offline is on.
    Offline,
    /// Neither download address could be read to the end.
    Network(String),
    /// The file downloaded isn't the one the feed names.
    Checksum,
    /// The feed's signature is missing or doesn't verify.
    Signature,
    /// Not newer than the running version (never offered; refused again here).
    NotNewer,
    /// Copying or checking the new app failed (the reason, from the system, in English).
    Install(String),
}

/// What the update notice shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Phase {
    /// Nothing (also after a launch check that found nothing).
    #[default]
    Idle,
    /// A check is running; `manual` when the user asked (only that one shows "Checking…").
    Checking { manual: bool },
    /// A newer version: the offer.
    Found(Update),
    /// The user's check found nothing newer than this version (the running one).
    UpToDate(String),
    /// The user's check couldn't read the feed.
    Unreachable,
    /// Update Now is downloading: bytes so far and the size when known.
    Downloading { update: Update, got: u64, total: Option<u64> },
    /// Downloaded and verified; the new app is being copied and checked.
    Installing(Update),
    /// Staged: installed when the app quits, which it is about to do.
    Ready(Update),
    /// Update Now stopped; nothing was changed.
    Failed(Update, Failure),
    /// The app can't replace itself where it runs: the disk image is open in Finder.
    Manual(Update, install::Manual),
}

#[derive(Default)]
struct Inner {
    phase: Phase,
    /// Bumped by every check, Update Now and Cancel: a background result for an older one is
    /// dropped.
    generation: u64,
    /// The launch check has been started (it runs once).
    launched: bool,
    /// The interface was in Chinese when the check ran.
    chinese: bool,
    /// The feed came from the fallback address (so GitHub-hosted files may be unreachable too).
    via_mirror: bool,
    /// The offer was closed for this version.
    closed: Option<String>,
    /// The new version, copied and checked, to swap in at quit.
    staged: Option<install::Staged>,
    /// Open the new version after quitting (Update Now, Restart Now).
    relaunch: bool,
    /// The window should be asked to close (once) to finish installing.
    close_pending: bool,
}

/// The update checker and installer: its state is shared with its background threads.
#[derive(Clone, Default)]
pub struct Updater(Arc<Mutex<Inner>>);

impl Updater {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What the notice shows now.
    pub fn phase(&self) -> Phase {
        self.lock().phase.clone()
    }

    /// Sets what the notice shows (tests and screenshots).
    pub fn set_phase(&self, phase: Phase) {
        self.lock().phase = phase;
    }

    /// Whether a started check has finished (found something or not).
    pub fn finished(&self) -> bool {
        !matches!(self.lock().phase, Phase::Checking { .. })
    }

    /// The newer version, once a check has found one.
    pub fn found(&self) -> Option<Update> {
        match self.phase() {
            Phase::Found(u) => Some(u),
            _ => None,
        }
    }

    /// The launch check against `feeds` (the compiled-in ones: [`feed_urls`]), once per run.
    /// `chinese`: the interface language is Chinese.
    pub fn start(&self, feeds: Vec<&str>, ctx: &egui::Context, chinese: bool) {
        {
            let mut g = self.lock();
            if g.launched || g.phase != Phase::Idle {
                return;
            }
            g.launched = true;
        }
        self.check(feeds, ctx, chinese, false);
    }

    /// NimbleMap › Check for Updates…: checks now and says what it found, "You're up to date"
    /// included. Does nothing while an update is downloading or installing.
    pub fn check_now(&self, ctx: &egui::Context, chinese: bool) {
        self.check(feed_urls(), ctx, chinese, true);
    }

    fn check(&self, feeds: Vec<&str>, ctx: &egui::Context, chinese: bool, manual: bool) {
        if feeds.is_empty() {
            return;
        }
        let generation = {
            let mut g = self.lock();
            if matches!(g.phase, Phase::Checking { .. } | Phase::Downloading { .. } | Phase::Installing(_) | Phase::Ready(_)) {
                return;
            }
            g.generation += 1;
            g.phase = Phase::Checking { manual };
            g.chinese = chinese;
            if manual {
                g.closed = None;
            }
            g.generation
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let feeds: Vec<String> = feeds.iter().map(|s| s.to_string()).collect();
            let me = self.clone();
            let ctx2 = ctx.clone();
            let spawned = std::thread::Builder::new().name("lm-update-check".into()).spawn(move || {
                let r = read_feeds(&feeds, CURRENT, macos_version().as_deref(), chinese);
                let mut g = me.lock();
                if g.generation != generation {
                    return;
                }
                g.phase = match r {
                    Ok((Some(u), via_mirror)) => {
                        log::info!("update check: {} is available", u.version);
                        g.via_mirror = via_mirror;
                        Phase::Found(u)
                    }
                    Ok((None, _)) if manual => Phase::UpToDate(CURRENT.to_string()),
                    Err(e) if manual => {
                        log::info!("update check failed: {e}");
                        Phase::Unreachable
                    }
                    Ok((None, _)) => Phase::Idle,
                    // No network, offline mode, or a host that's down: skipped without a word.
                    Err(e) => {
                        log::info!("update check skipped: {e}");
                        Phase::Idle
                    }
                };
                drop(g);
                ctx2.request_repaint();
            });
            if let Err(e) = spawned {
                log::warn!("update check: {e}");
                self.lock().phase = if manual { Phase::Unreachable } else { Phase::Idle };
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (ctx, feeds);
            self.lock().phase = Phase::Idle;
        }
    }

    /// Update Now (also Try again after a failure): downloads, verifies and stages the offered
    /// version on a background thread. Only for an [`Update::installable`] offer.
    pub fn update_now(&self, ctx: &egui::Context) {
        let (update, generation, chinese, via_mirror) = {
            let mut g = self.lock();
            let u = match &g.phase {
                Phase::Found(u) | Phase::Failed(u, _) if u.installable() => u.clone(),
                _ => return,
            };
            g.generation += 1;
            g.phase = Phase::Downloading { update: u.clone(), got: 0, total: None };
            (u, g.generation, g.chinese, g.via_mirror)
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let me = self.clone();
            let ctx2 = ctx.clone();
            let spawned = std::thread::Builder::new().name("lm-update-install".into()).spawn(move || {
                let r = me.install_job(generation, &update, chinese, via_mirror, &ctx2);
                let mut g = me.lock();
                if g.generation != generation {
                    return;
                }
                g.phase = match r {
                    Ok(staged) => {
                        log::info!("update: {} is ready ({})", update.version, staged.new_app.display());
                        g.staged = Some(staged);
                        g.relaunch = true;
                        g.close_pending = true;
                        Phase::Ready(update)
                    }
                    Err(Stop::Cancelled) => Phase::Found(update),
                    Err(Stop::Manual(m, dmg)) => {
                        log::info!("update: can't replace the app here ({m:?}); opening {}", dmg.display());
                        install::open_in_finder(&dmg);
                        Phase::Manual(update, m)
                    }
                    Err(Stop::Failed(f)) => {
                        log::warn!("update: {} not installed: {f:?}", update.version);
                        Phase::Failed(update, f)
                    }
                };
                drop(g);
                ctx2.request_repaint();
            });
            if let Err(e) = spawned {
                self.lock().phase = Phase::Failed(Update::default(), Failure::Install(e.to_string()));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (ctx, update, generation, chinese, via_mirror);
        }
    }

    /// Cancel while downloading: back to the offer, the partial file removed.
    pub fn cancel(&self) {
        let mut g = self.lock();
        if let Phase::Downloading { update, .. } = &g.phase {
            let u = update.clone();
            g.generation += 1;
            g.phase = Phase::Found(u);
        }
    }

    /// Closes the notice: the offer stays closed for its version until the next check; anything
    /// else is cleared.
    pub fn dismiss(&self) {
        let mut g = self.lock();
        match &g.phase {
            Phase::Found(u) => g.closed = Some(u.version.clone()),
            Phase::UpToDate(_) | Phase::Unreachable | Phase::Failed(..) | Phase::Manual(..) => g.phase = Phase::Idle,
            _ => {}
        }
    }

    fn closed_for(&self, version: &str) -> bool {
        self.lock().closed.as_deref() == Some(version)
    }

    /// Restart Now (after Cancel in "Save changes?"): close the window again to finish.
    pub fn restart_now(&self) {
        let mut g = self.lock();
        if g.staged.is_some() {
            g.relaunch = true;
            g.close_pending = true;
        }
    }

    /// Whether the window should be asked to close now to finish installing (true once).
    pub fn take_close_request(&self) -> bool {
        std::mem::take(&mut self.lock().close_pending)
    }

    /// At quit: the staged new version, and whether to open it afterwards.
    pub fn take_staged(&self) -> Option<(install::Staged, bool)> {
        let mut g = self.lock();
        let relaunch = g.relaunch;
        g.staged.take().map(|s| (s, relaunch))
    }

    /// Download, verify, stage (on the install thread).
    #[cfg(not(target_arch = "wasm32"))]
    fn install_job(
        &self,
        generation: u64,
        u: &Update,
        chinese: bool,
        via_mirror: bool,
        ctx: &egui::Context,
    ) -> Result<install::Staged, Stop> {
        // No downgrades and no reinstalling the same version, whatever the feed says.
        if compare(&u.version, CURRENT) != Some(std::cmp::Ordering::Greater) {
            return Err(Stop::Failed(Failure::NotNewer));
        }
        // The signature first: nothing is downloaded for a feed that isn't the owner's.
        let (Some(sha), Some(sig)) = (&u.sha256, &u.signature) else { return Err(Stop::Failed(Failure::Signature)) };
        verify::verify(&u.version, sha, sig).map_err(|_| Stop::Failed(Failure::Signature))?;
        let dir = downloads_dir();
        std::fs::create_dir_all(&dir).map_err(|e| Stop::Failed(Failure::Install(format!("{}: {e}", dir.display()))))?;
        let dmg = dir.join(dmg_name(&u.version));
        // A copy downloaded before (Update Now pressed again) is used when it checks out.
        if verify::check_file(&dmg, sha).is_err() {
            let mut last = String::new();
            let mut done = false;
            for src in u.sources(chinese, via_mirror) {
                let mut progress = |got: u64, total: Option<u64>| {
                    let mut g = self.lock();
                    if g.generation != generation {
                        return false;
                    }
                    if let Phase::Downloading { got: g_got, total: g_total, .. } = &mut g.phase {
                        *g_got = got;
                        *g_total = total;
                    }
                    drop(g);
                    ctx.request_repaint();
                    true
                };
                match fetch::download(&src, &dmg, &mut progress) {
                    Ok(n) => {
                        log::info!("update: downloaded {n} bytes from {src}");
                        done = true;
                        break;
                    }
                    Err(e) if e == lm_net::CANCELLED => return Err(Stop::Cancelled),
                    Err(_) if lm_net::offline() => return Err(Stop::Failed(Failure::Offline)),
                    Err(e) => {
                        log::info!("update: {src}: {e}");
                        last = e;
                    }
                }
            }
            if !done {
                return Err(Stop::Failed(Failure::Network(last)));
            }
            if verify::check_file(&dmg, sha).is_err() {
                let _ = std::fs::remove_file(&dmg);
                return Err(Stop::Failed(Failure::Checksum));
            }
        }
        {
            let mut g = self.lock();
            if g.generation != generation {
                return Err(Stop::Cancelled);
            }
            g.phase = Phase::Installing(u.clone());
        }
        ctx.request_repaint();
        let bundle = install::running_bundle();
        install::placement(bundle.as_deref()).map_err(|m| Stop::Manual(m, dmg.clone()))?;
        let bundle = bundle.ok_or(Stop::Manual(install::Manual::NotABundle, dmg.clone()))?;
        install::stage(&dmg, &bundle, &u.version).map_err(|e| Stop::Failed(Failure::Install(e)))
    }
}

/// How the install thread stopped short of staging.
#[cfg(not(target_arch = "wasm32"))]
enum Stop {
    Cancelled,
    Manual(install::Manual, PathBuf),
    Failed(Failure),
}

/// Reads the first feed that answers; `true` with it when that was not the first address.
#[cfg(not(target_arch = "wasm32"))]
fn read_feeds(feeds: &[String], current: &str, macos: Option<&str>, chinese: bool) -> Result<(Option<Update>, bool), String> {
    let mut last = String::from("no feed address");
    for (i, url) in feeds.iter().enumerate() {
        match fetch::feed(url) {
            Ok(text) => return Ok((parse(&text, current, macos, chinese), i > 0)),
            Err(e) => {
                log::info!("update feed {url}: {e}");
                last = e;
            }
        }
    }
    Err(last)
}

/// The downloaded disk image's file name (only safe characters from the version).
fn dmg_name(version: &str) -> String {
    let v: String = version.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-')).collect();
    format!("NimbleMap-{v}.dmg")
}

/// Where update disk images are downloaded: `~/Library/Caches/LayeredMap/Updates`.
fn downloads_dir() -> PathBuf {
    lm_core::platform::current_user_dir(lm_core::platform::Folder::Caches)
        .unwrap_or_else(|| std::env::temp_dir().join("LayeredMap"))
        .join("Updates")
}

/// At launch, after an update: removes the old version and the downloaded disk image (the app
/// calls this first thing; it does nothing on other launches).
pub fn after_launch() {
    let Some(support) = lm_core::platform::current_user_dir(lm_core::platform::Folder::Support) else { return };
    if let Some(old) = install::after_update(&support, Some(&downloads_dir())) {
        log::info!("update: now {CURRENT}; removed the previous version ({})", old.display());
    }
}

/// At quit (the app's `on_exit`, after the user's state is saved): installs a staged update and
/// opens it when the user asked for that.
pub fn finish_at_exit(staged: Option<(install::Staged, bool)>) {
    let Some((s, relaunch)) = staged else { return };
    let support = lm_core::platform::current_user_dir(lm_core::platform::Folder::Support);
    install::finish(&s, relaunch, support.as_deref());
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

/// Whether the system's first language is Chinese. Help › NimbleMap Help used it to open the
/// Chinese pages before the interface had its own language setting (WP5.5).
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

/// Megabytes for the progress line, in the interface's number style (whole numbers; one decimal
/// below 10).
fn megabytes(bytes: u64) -> String {
    let mb = bytes as f64 / 1_000_000.0;
    i18n::number(&if mb < 10.0 { format!("{mb:.1}") } else { format!("{mb:.0}") })
}

fn accent_button(ui: &mut egui::Ui, t: &theme::Theme, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).size(CAPTION).color(t.on_accent)).fill(t.accent))
}

fn close_button(ui: &mut egui::Ui, updater: &Updater) {
    let close = ui.add(egui::Button::new(RichText::new(icon::X).font(theme::icon_font(11.0))).frame(false));
    crate::a11y::button(&close, tl!("Close the update notice"));
    if close.on_hover_text(tl!("Close")).clicked() {
        updater.dismiss();
    }
}

/// A line of wrapped notice text with an icon.
fn message(ui: &mut egui::Ui, t: &theme::Theme, glyph: &str, color: egui::Color32, text: &str) {
    ui.horizontal_top(|ui| {
        ui.label(RichText::new(glyph).font(theme::icon_font(14.0)).color(color));
        ui.add(egui::Label::new(RichText::new(text).size(CAPTION).color(t.text_primary)).wrap());
    });
}

/// The user-facing reason Update Now stopped.
fn failure_text(f: &Failure) -> String {
    let app = lm_core::brand::shown(false);
    match f {
        Failure::Offline => i18n::fmt("{app} is working offline. Turn off View › Work Offline, then try again.", &[("app", app)]),
        Failure::Network(_) => tl!("The download didn't finish. Check your connection and try again.").to_string(),
        Failure::Checksum => tl!("The downloaded file is damaged (its checksum doesn't match), so nothing was installed.").to_string(),
        Failure::Signature => tl!("The update's signature couldn't be verified, so nothing was installed.").to_string(),
        Failure::NotNewer => tl!("This version is not newer than the one you have.").to_string(),
        Failure::Install(_) => tl!("The new version couldn't be installed, and nothing was changed.").to_string(),
    }
}

/// The note when the app can't replace itself (the disk image is open in Finder).
fn manual_text(m: install::Manual) -> String {
    let app = [("app", lm_core::brand::shown(false))];
    match m {
        install::Manual::OnDiskImage | install::Manual::Translocated => i18n::fmt(
            "{app} can't update itself while it runs from the disk image or a temporary location. The new version is open in Finder: drag {app} to Applications, then open it from there.",
            &app,
        ),
        install::Manual::NotWritable | install::Manual::NotABundle => i18n::fmt(
            "{app} can't update itself in this folder (it may need an administrator). The new version is open in Finder: drag {app} to Applications to replace the old one.",
            &app,
        ),
    }
}

/// The quiet notice in the map's lower right, beside the map controls: the offer, the progress of
/// Update Now, and the answers to Check for Updates…. Also asks the window to close once an
/// update is staged (closing asks about unsaved changes first).
pub(crate) fn notice(shell: &Shell, ui: &egui::Ui, map: egui::Rect) {
    let updater = shell.updater.clone();
    if updater.take_close_request() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let phase = updater.phase();
    let app = lm_core::brand::shown(false);
    match &phase {
        Phase::Idle | Phase::Checking { manual: false } => return,
        Phase::Found(u) if updater.closed_for(&u.version) => return,
        _ => {}
    }
    let t = *shell.theme();
    // Left of the zoom and basemap buttons (chrome::controls), its foot level with theirs.
    let pos = pos2(map.right() - PANEL_MARGIN - theme::ICON_HIT - PANEL_MARGIN, map.bottom() - PANEL_MARGIN - 26.0);
    egui::Area::new(egui::Id::new("lm-update")).pivot(Align2::RIGHT_BOTTOM).fixed_pos(pos).show(ui.ctx(), |ui| {
        crate::chrome::panel_frame(&t).show(ui, |ui| match &phase {
            Phase::Idle | Phase::Checking { manual: false } => {}
            Phase::Checking { manual: true } => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(RichText::new(tl!("Checking for updates…")).size(CAPTION).color(t.text_primary));
                });
            }
            Phase::Found(u) => {
                ui.horizontal(|ui| {
                    let label = ui.label(RichText::new(tl!("A new version is available")).size(CAPTION).color(t.text_primary));
                    if let Some(n) = &u.notes {
                        label.on_hover_text(i18n::fmt("{app} {version}: {notes}", &[("app", app), ("version", &u.version), ("notes", n)]));
                    }
                    if u.installable() {
                        let b = accent_button(ui, &t, tl!("Update Now"));
                        let b = b.on_hover_text(i18n::fmt(
                            "Downloads {version}, checks its signature, installs it and reopens {app}. The built-in example maps and world layers update with it.",
                            &[("app", app), ("version", &u.version)],
                        ));
                        if b.clicked() {
                            updater.update_now(ui.ctx());
                        }
                    }
                    ui.hyperlink_to(RichText::new(i18n::fmt("Download {version}", &[("version", &u.version)])).size(CAPTION), &u.url);
                    close_button(ui, &updater);
                });
            }
            Phase::UpToDate(version) => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::CHECK_CIRCLE).font(theme::icon_font(14.0)).color(t.accent));
                    ui.label(
                        RichText::new(i18n::fmt("You're up to date: {app} {version} is the newest version.", &[("app", app), ("version", version)]))
                            .size(CAPTION)
                            .color(t.text_primary),
                    );
                    close_button(ui, &updater);
                });
            }
            Phase::Unreachable => {
                ui.set_max_width(380.0);
                ui.horizontal_top(|ui| {
                    message(
                        ui,
                        &t,
                        icon::WIFI_SLASH,
                        t.text_secondary,
                        tl!("Couldn't check for updates. Check your connection, or turn off View › Work Offline."),
                    );
                    close_button(ui, &updater);
                });
            }
            Phase::Downloading { update, got, total } => {
                ui.set_width(300.0);
                ui.label(
                    RichText::new(i18n::fmt("Downloading {app} {version}…", &[("app", app), ("version", &update.version)]))
                        .size(CAPTION)
                        .color(t.text_primary),
                );
                ui.horizontal(|ui| {
                    let frac = total.filter(|t| *t > 0).map_or(0.0, |t| (*got as f32 / t as f32).clamp(0.0, 1.0));
                    ui.add(egui::ProgressBar::new(frac).desired_width(150.0).desired_height(6.0).fill(t.accent));
                    let size = match total {
                        Some(t) => i18n::fmt("{done} of {total} MB", &[("done", &megabytes(*got)), ("total", &megabytes(*t))]),
                        None => i18n::fmt("{done} MB", &[("done", &megabytes(*got))]),
                    };
                    ui.label(RichText::new(size).size(CAPTION).color(t.text_secondary));
                    if ui.button(RichText::new(tl!("Cancel")).size(CAPTION)).clicked() {
                        updater.cancel();
                    }
                });
            }
            Phase::Installing(u) => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(
                        RichText::new(i18n::fmt("Installing {app} {version}…", &[("app", app), ("version", &u.version)]))
                            .size(CAPTION)
                            .color(t.text_primary),
                    );
                });
            }
            Phase::Ready(u) => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(i18n::fmt("{app} {version} is ready", &[("app", app), ("version", &u.version)]))
                            .size(CAPTION)
                            .color(t.text_primary),
                    );
                    if accent_button(ui, &t, tl!("Restart Now")).clicked() {
                        updater.restart_now();
                    }
                });
            }
            Phase::Failed(u, f) => {
                ui.set_max_width(380.0);
                let detail = match f {
                    Failure::Network(e) | Failure::Install(e) => Some(e.clone()),
                    _ => None,
                };
                ui.horizontal_top(|ui| {
                    let r = ui.scope(|ui| message(ui, &t, icon::WARNING, t.warning, &failure_text(f))).response;
                    if let Some(d) = detail {
                        r.on_hover_text(d);
                    }
                    close_button(ui, &updater);
                });
                ui.horizontal(|ui| {
                    if u.installable() && *f != Failure::NotNewer && ui.button(RichText::new(tl!("Try again")).size(CAPTION)).clicked() {
                        updater.update_now(ui.ctx());
                    }
                    if !u.url.is_empty() {
                        ui.hyperlink_to(RichText::new(i18n::fmt("Download {version}", &[("version", &u.version)])).size(CAPTION), &u.url);
                    }
                });
            }
            Phase::Manual(_, m) => {
                ui.set_max_width(380.0);
                ui.horizontal_top(|ui| {
                    message(ui, &t, icon::INFO, t.accent, &manual_text(*m));
                    close_button(ui, &updater);
                });
            }
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
        assert_eq!(compare("0.7.3-test", "0.7.2-test"), Some(Greater));
        assert_eq!(compare("0.7.2", "0.7.2-test"), Some(Greater));
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

    /// A format-1 feed as `cargo xtask update-sign` writes it (0.7.2 and later).
    const SIGNED: &str = r#"{
        "format": 1,
        "version": "0.7.2",
        "released": "2026-10-22",
        "download": {
            "free": "https://github.com/kangning-huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg",
            "pro": "https://github.com/kangning-huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg",
            "gitcode": "https://gitcode.com/KN-Huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg"
        },
        "sha256": {
            "free": "053AC3D7965CD2CFF0A1B85044CD926E40B552AE0E437526D223849ECF6A3C05",
            "pro": "053ac3d7965cd2cff0a1b85044cd926e40b552ae0e437526d223849ecf6a3c05"
        },
        "signature": "c2lnbmF0dXJl",
        "page": "https://nimblemap.com/download.html",
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
        // A feed from before signatures: readable, linked, but not installable.
        assert_eq!((u.sha256, u.signature, u.gitcode), (None, None, None));
    }

    #[test]
    fn a_signed_feed_names_both_downloads_the_checksum_and_the_signature() {
        let u = mac(SIGNED, "0.7.1", Some("15.1"), false).unwrap();
        assert_eq!(
            u.github.as_deref(),
            Some("https://github.com/kangning-huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg")
        );
        assert_eq!(u.gitcode.as_deref(), Some("https://gitcode.com/KN-Huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg"));
        // Upper-case hex is read as the same checksum.
        assert_eq!(u.sha256.as_deref(), Some("053ac3d7965cd2cff0a1b85044cd926e40b552ae0e437526d223849ecf6a3c05"));
        assert_eq!(u.signature.as_deref(), Some("c2lnbmF0dXJl"));
        assert_eq!(u.url, u.github.clone().unwrap());
        assert_eq!(u.installable(), cfg!(target_os = "macos") && verify::has_key());
        // A checksum that isn't one, or no signature: not installable.
        let bad = SIGNED.replace("053AC3D7", "zzzz");
        assert_eq!(mac(&bad, "0.7.1", None, false).unwrap().sha256, None);
        let unsigned = SIGNED.replace("\"signature\": \"c2lnbmF0dXJl\",", "");
        assert!(!mac(&unsigned, "0.7.1", None, false).unwrap().installable());
    }

    /// GitCode first in Chinese or when the feed only came from GitCode; GitHub first otherwise;
    /// each falls back to the other.
    #[test]
    fn the_download_follows_the_language_and_falls_back() {
        let en = mac(SIGNED, "0.7.1", None, false).unwrap();
        let (gh, gc) = (en.github.clone().unwrap(), en.gitcode.clone().unwrap());
        assert_eq!(en.sources(false, false), vec![gh.clone(), gc.clone()]);
        assert_eq!(en.sources(true, false), vec![gc.clone(), gh.clone()]);
        assert_eq!(en.sources(false, true), vec![gc.clone(), gh.clone()]);
        let zh = mac(SIGNED, "0.7.1", None, true).unwrap();
        assert_eq!(zh.url, gc);
        // With only one of them, that one.
        let only_gh = Update { gitcode: None, ..en.clone() };
        assert_eq!(only_gh.sources(true, true), vec![gh.clone()]);
        let only_gc = Update { github: None, ..en };
        assert_eq!(only_gc.sources(false, false), vec![gc]);
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
        // Windows never installs: the signed feed's disk images aren't its downloads.
        let u = parse_for("download_windows", SIGNED, "0.7.1", None, false).unwrap();
        assert!(!u.installable());
        assert_eq!((u.url.as_str(), u.github, u.gitcode), ("https://nimblemap.com/download.html", None, None));
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
        if !cfg!(feature = "update-test-hook") {
            assert_eq!(mac(none, "0.2.0", None, false), None);
        }
        assert_eq!(mac("not json", "0.2.0", None, false), None);
        assert_eq!(mac(r#"{"format":1}"#, "0.2.0", None, false), None);
        // Bare minimum: a version and a page.
        let min = r#"{"format":1,"version":"0.2.1","page":"https://example.org/get"}"#;
        assert_eq!(mac(min, "0.2.0", None, false).unwrap().notes, None);
    }

    #[test]
    fn the_check_is_off_without_a_feed_address() {
        // This build has no feed compiled in unless LM_UPDATE_FEED_URL was set.
        if FEED_URL.is_empty() && FEED_FALLBACK_URL.is_empty() {
            assert!(!available());
            let u = Updater::default();
            u.start(feed_urls(), &egui::Context::default(), false);
            u.check_now(&egui::Context::default(), false);
            assert_eq!(u.phase(), Phase::Idle);
        }
    }

    /// The install thread refuses before downloading anything: an older or the same version, and
    /// a signature that isn't the owner's (the test can't sign with the owner's key, so any
    /// signature here is a bad one).
    #[test]
    fn downgrades_and_bad_signatures_are_refused_before_downloading() {
        let ctx = egui::Context::default();
        let u = Updater::default();
        let offer = |version: &str| Update {
            version: version.into(),
            url: "https://example.invalid/x.dmg".into(),
            github: Some("https://example.invalid/x.dmg".into()),
            sha256: Some("053ac3d7965cd2cff0a1b85044cd926e40b552ae0e437526d223849ecf6a3c05".into()),
            signature: Some("c2lnbmF0dXJl".into()),
            ..Update::default()
        };
        for (version, want) in
            [(CURRENT.to_string(), Failure::NotNewer), ("0.0.1".to_string(), Failure::NotNewer), ("99.0.0".to_string(), Failure::Signature)]
        {
            let r = u.install_job(u.lock().generation, &offer(&version), false, false, &ctx);
            assert!(matches!(&r, Err(Stop::Failed(f)) if *f == want), "{version}");
        }
        // Cancel goes back to the offer and drops the download's result.
        u.set_phase(Phase::Downloading { update: offer("99.0.0"), got: 5, total: Some(10) });
        let before = u.lock().generation;
        u.cancel();
        assert_eq!(u.phase(), Phase::Found(offer("99.0.0")));
        assert!(u.lock().generation > before);
        // Closing the offer hides it for that version only.
        u.dismiss();
        assert!(u.closed_for("99.0.0") && !u.closed_for("99.0.1"));
    }

    #[test]
    fn a_staged_update_closes_the_window_once_and_installs_at_quit() {
        let u = Updater::default();
        assert!(!u.take_close_request());
        assert_eq!(u.take_staged(), None);
        let staged = install::Staged {
            bundle: "/Applications/NimbleMap.app".into(),
            new_app: "/Applications/.NimbleMap-update-1".into(),
            dmg: "/tmp/x.dmg".into(),
            version: "9.0.0".into(),
        };
        {
            let mut g = u.lock();
            g.staged = Some(staged.clone());
            g.close_pending = true;
            g.relaunch = true;
        }
        assert!(u.take_close_request());
        assert!(!u.take_close_request());
        // Restart Now after "Save changes?" was cancelled asks again.
        u.restart_now();
        assert!(u.take_close_request());
        assert_eq!(u.take_staged(), Some((staged, true)));
        assert_eq!(u.take_staged(), None);
    }

    /// The feed the website publishes (`web/lm/update/feed.json`, written by `cargo xtask
    /// update-sign`) verifies with the key this build carries; edited, it doesn't.
    #[test]
    fn the_published_feed_is_signed_with_the_compiled_in_key() {
        let text = include_str!("../../../web/lm/update/feed.json");
        let u = parse_for("download", text, "0.0.1", None, false).unwrap();
        let (sha, sig) = (u.sha256.clone().unwrap(), u.signature.clone().unwrap());
        assert_eq!(verify::verify(&u.version, &sha, &sig), Ok(()));
        assert!(u.installable() || !cfg!(target_os = "macos"));
        // A relabelled version or another file: refused.
        assert_eq!(verify::verify("99.0.0", &sha, &sig), Err(verify::Refusal::Signature));
        let other = format!("{}0", &sha[..63]);
        assert_eq!(verify::verify(&u.version, &other, &sig), Err(verify::Refusal::Signature));
    }

    #[test]
    fn download_names_are_safe() {
        assert_eq!(dmg_name("0.7.3-test"), "NimbleMap-0.7.3-test.dmg");
        assert_eq!(dmg_name("../../etc/x"), "NimbleMap-....etcx.dmg");
    }
}
