//! Gate 5's network-blocking harness (WP5.7), which also verifies strict offline mode (WP6.6).
//!
//! The core tasks (open files, search places, select, run an action and keep it, export, and in
//! Pro ask) run headlessly in a child test process whose every HTTP client is pointed at a local
//! tripwire (`ALL_PROXY`, `HTTPS_PROXY`, `HTTP_PROXY`; `ureq` sends every request to the proxy,
//! HTTPS included, and resolves no host itself). Two checks, each independent of the other:
//! - the network gate (`lm_net`) counts every request asked for: zero allowed;
//! - the tripwire counts every connection that got past the gate anyway: zero.
//!
//! A third child shows the tripwire works: one request deliberately sent past it is caught.
//! Run alone with `cargo test -p lm-ui offline_harness`.

use std::io::Read as _;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui_kittest::Harness;
use lm_core::phase::Flags;

use crate::{Command, FileRequest, Prefs, Shell, System};

/// A local listener that counts connections and answers none of them.
struct Tripwire {
    url: String,
    hits: Arc<AtomicUsize>,
}

impl Tripwire {
    fn new() -> Tripwire {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                h.fetch_add(1, Ordering::SeqCst);
                let mut s = stream;
                let _ = s.set_read_timeout(Some(std::time::Duration::from_millis(200)));
                let _ = s.read(&mut [0u8; 512]);
            }
        });
        Tripwire { url, hits }
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    /// Runs one of the ignored tests below in a child process wired to the tripwire.
    fn run(&self, test: &str) -> String {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([test, "--exact", "--ignored", "--nocapture", "--test-threads=1"])
            .env("ALL_PROXY", &self.url)
            .env("HTTPS_PROXY", &self.url)
            .env("HTTP_PROXY", &self.url)
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .output()
            .unwrap();
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "{test} failed:\n{text}");
        assert!(text.contains("1 passed"), "{test} didn't run:\n{text}");
        text
    }
}

#[test]
fn core_tasks_make_no_network_calls() {
    let wire = Tripwire::new();
    wire.run("offline_harness::strict_offline_core_tasks");
    assert_eq!(wire.hits(), 0, "a request got past the gate in offline mode");
    wire.run("offline_harness::core_tasks_need_no_network");
    assert_eq!(wire.hits(), 0, "a core task went online");
    // The tripwire itself: a request the gate allows reaches it.
    wire.run("offline_harness::tripwire_catches_a_request");
    assert!(wire.hits() >= 1, "the tripwire missed a request");
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn harness(offline: bool, basemap: &str) -> Harness<'static, Option<Shell>> {
    let prefs = Prefs { basemap: basemap.into(), offline, view: Some((121.35, 31.2, 10.0)), ..Prefs::default() };
    let system = System { dark_mode: false, reduce_motion: true };
    Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(
        move |ui, state: &mut Option<Shell>| {
            let shell = state.get_or_insert_with(|| {
                let mut s = Shell::new(ui.ctx(), Flags::up_to(4), prefs.clone(), system);
                s.developer = true;
                s.pro = true;
                s
            });
            shell.ui(ui, system);
        },
        None,
    )
}

/// Steps until `done` holds (at most about 10 s), then settles.
fn wait(h: &mut Harness<'static, Option<Shell>>, done: impl Fn(&Shell) -> bool) {
    for _ in 0..1000 {
        h.step();
        if h.state().as_ref().is_some_and(&done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(h);
}

fn settle(h: &mut Harness<'static, Option<Shell>>) {
    h.run_steps(5);
    for _ in 0..600 {
        h.step();
        if h.state()
            .as_ref()
            .is_some_and(|s| s.ws.idle() && !s.table.busy() && s.composer.as_ref().is_none_or(|c| c.result.is_some() || c.error.is_some()))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(10);
}

fn command(h: &mut Harness<'static, Option<Shell>>, cmd: Command) {
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, cmd);
    }
    settle(h);
}

/// Open two files, search for a place, select a layer and an object, count points in each area
/// and keep the result, export the layer's problem list, open the diagnostics review.
fn core_tasks(h: &mut Harness<'static, Option<Shell>>) {
    h.step();
    command(h, Command::Open(vec![fixtures().join("N1/areas.geojson"), fixtures().join("N1/points.geojson")]));
    wait(h, |s| s.ws.doc.layers.len() == 2 && s.ws.idle());
    assert_eq!(h.state().as_ref().unwrap().ws.doc.layers.len(), 2);

    // Place search (the bundled index).
    if let Some(s) = h.state_mut().as_mut() {
        s.search.query = "Shang".into();
        s.search.open = true;
    }
    for _ in 0..300 {
        h.step();
        if h.state_mut().as_mut().is_some_and(|s| s.search.ready() && !s.search.rows.is_empty()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!h.state().as_ref().unwrap().search.rows.is_empty(), "place search found nothing");

    // Select the points layer, then an area on the map.
    let points = h.state().as_ref().unwrap().ws.doc.layers.iter().find(|l| l.display_name == "points").map(|l| l.id).unwrap();
    if let Some(s) = h.state_mut().as_mut() {
        s.ws.select_layer(points);
    }
    settle(h);

    // Run an action and keep it.
    command(h, Command::ComposeAction("count_in".into(), vec![]));
    wait(h, |s| s.composer.as_ref().is_some_and(|c| c.result.is_some() || c.error.is_some()));
    let err = h.state().as_ref().and_then(|s| s.composer.as_ref().and_then(|c| c.error.clone()));
    assert_eq!(err, None);
    command(h, Command::KeepResult);
    wait(h, |s| s.ws.doc.layers.len() == 3);
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.doc.layers.len(), 3);
    assert!(s.ws.doc.layers.iter().any(|l| l.display_name == "Points in each area"));

    // Export: a file the app is asked to save.
    if let Some(s) = h.state_mut().as_mut() {
        s.export_problems(points);
    }
    command(h, Command::ReviewDiagnostics("report".into()));
    let s = h.state_mut().as_mut().unwrap();
    let asked = s.take_file_requests();
    assert!(asked.iter().any(|r| matches!(r, FileRequest::SaveText { .. })), "{asked:?}");
    assert!(s.diagnostics.is_some());
}

/// Offline mode on, with every launch-time request the app makes started as it would be.
#[test]
#[ignore = "run in a child process by core_tasks_make_no_network_calls"]
fn strict_offline_core_tasks() {
    // The online basemap is chosen; offline mode must keep it from asking for a single tile.
    let mut h = harness(true, "auto");
    h.step();
    let ctx = h.ctx.clone();
    crate::basemap::start_archive_lookup(&ctx);
    crate::update::start_with("https://updates.invalid/layeredmap/feed.json", &ctx, true);
    core_tasks(&mut h);

    // Pro's assistant explains instead of asking.
    command(&mut h, Command::Ask("how many points are in each area".into()));
    let msg = h.state().as_ref().unwrap().ask.message().map(str::to_string);
    assert!(msg.as_deref().is_some_and(|m| m.contains("offline")), "{msg:?}");

    for _ in 0..500 {
        if crate::update::finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(crate::update::finished() && crate::update::found().is_none());
    let s = h.state().as_ref().unwrap();
    assert!(s.map_view().map().tiles.is_empty(), "offline, the map has no tile layer");
    assert!(!lm_mapview::archive::looking() && lm_mapview::archive::found().is_none());
    let c = lm_net::counts();
    eprintln!("gate: {}", c.lines().join("; "));
    assert_eq!(c.allowed_total(), 0, "{:?}", c.lines());
    assert_eq!(c.refused[lm_net::Purpose::UpdateCheck as usize], 1, "the update check was refused at the gate: {:?}", c.lines());
}

/// Offline mode off, offline basemap: the core tasks never even ask to go online.
#[test]
#[ignore = "run in a child process by core_tasks_make_no_network_calls"]
fn core_tasks_need_no_network() {
    let mut h = harness(false, "offline");
    core_tasks(&mut h);
    let c = lm_net::counts();
    assert_eq!((c.allowed_total(), c.refused_total()), (0, 0), "{:?}", c.lines());
}

/// A request the gate allows goes out (and is caught by the tripwire).
#[test]
#[ignore = "run in a child process by core_tasks_make_no_network_calls"]
fn tripwire_catches_a_request() {
    let r = lm_net::get_text(lm_net::Purpose::UpdateCheck, "https://updates.invalid/feed.json", std::time::Duration::from_secs(5), 1024);
    assert!(r.is_err());
    assert_eq!(lm_net::counts().allowed_total(), 1);
}
