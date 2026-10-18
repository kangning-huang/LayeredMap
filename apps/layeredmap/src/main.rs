//! LayeredMap: the app entry point (WP1.1). A window that remembers its size and position, the
//! native macOS menu bar (LayeredMap, File, Edit, View, Window, Help), stored preferences, and the
//! map-first shell from `lm-ui`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod diagnostics;
mod feel;
// The spec's one exception to "no unsafe" (Architecture › Rules enforced in CI): native macOS
// hooks live in the app's macOS module, with a SAFETY comment on each block and a second review.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;
mod menu;

use lm_core::phase::Flags;
use lm_ui::{Command, FileRequest, Prefs, Shell, System, ThemePref, Units};

/// Where answered question cards are logged: the app's support folder (never data values or
/// file names; Assumptions and questions › Question budget instrumentation).
fn question_log_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/LayeredMap/question-log.jsonl")
}

/// Where autosaves and the recovery journal live (WP5.1).
fn autosave_dir() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/LayeredMap/Autosave")
}

fn lm_doc_export(log: &std::path::Path, out: &std::path::Path) -> Result<usize, String> {
    lm_ui::export_question_log(log, out)
}

struct App {
    shell: Shell,
    menu: Option<menu::MenuBar>,
    system: System,
    /// The feel harness, when launched with `--feel`.
    feel: Option<feel::Feel>,
    /// `--launch-probe`: when the process started.
    probe: Option<std::time::Instant>,
    /// Harness runs don't touch the user's stored preferences.
    ephemeral: bool,
    /// `--tour "lon,lat,zoom;…"`: flies to each place in turn, 6 s apart, then quits (checking
    /// what a search jump looks like without driving the keyboard).
    tour: Vec<Step>,
    tour_next: Option<f64>,
    /// The window title and Edited state last set.
    title: (String, bool),
}

/// The system's Reduce Motion setting (macOS), read once at launch.
fn reduce_motion() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

const BASEMAP_KEY: &str = "lm.basemap.v2";

fn load_prefs(storage: Option<&dyn eframe::Storage>) -> Prefs {
    let mut p = Prefs::default();
    let Some(s) = storage else { return p };
    p.theme = match s.get_string("lm.theme").as_deref() {
        Some("light") => ThemePref::Light,
        Some("dark") => ThemePref::Dark,
        _ => ThemePref::System,
    };
    p.units = if s.get_string("lm.units").as_deref() == Some("imperial") { Units::Imperial } else { Units::Metric };
    // `lm.basemap.v2`: builds before Auto existed always stored "light" (the old default), which
    // would otherwise pin the map to Light now that the default follows the appearance.
    if let Some(b) = s.get_string(BASEMAP_KEY).filter(|b| lm_core::basemap::providers().iter().any(|p| p.id == b)) {
        p.basemap = b;
    }
    if let Some(v) = s.get_string("lm.strength").and_then(|v| v.parse::<f32>().ok()) {
        p.strength = v.clamp(0.4, 1.0);
    }
    p.offline = s.get_string("lm.offline").as_deref() == Some("on");
    p.view = s.get_string("lm.view").and_then(|v| {
        let n: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        match n.as_slice() {
            [lon, lat, z] => Some((*lon, *lat, *z)),
            _ => None,
        }
    });
    p
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.system.dark_mode = ctx.system_theme().map_or(self.system.dark_mode, |t| t == egui::Theme::Dark);
        if let Some(f) = &mut self.feel {
            let t = ctx.input(|i| i.time);
            let busy = self.shell.is_loading();
            if !busy && f.step(&mut self.shell, t) {
                match f.write() {
                    Ok(()) => println!("feel: done"),
                    Err(e) => eprintln!("feel: {e}"),
                }
                self.feel = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
        if self.shell.map_view().renders > 0 && (!self.tour.is_empty() || self.tour_next.is_some()) {
            let t = ctx.input(|i| i.time);
            let next = *self.tour_next.get_or_insert(t + 2.0);
            if t >= next {
                if self.tour.is_empty() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    let step = self.tour.remove(0);
                    let wait = match step {
                        Step::Fly(..) => 6.0,
                        // The assistant answers in a few seconds; give it time before the next step.
                        Step::Other(Command::Ask(_)) => 15.0,
                        _ => 2.5,
                    };
                    self.shell.command(ctx, step.command());
                    self.tour_next = Some(t + wait);
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        // The map's name in the title bar, and the Edited state (WP5.1).
        let title = (self.shell.window_title(), self.shell.edited());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.0.clone()));
            #[cfg(target_os = "macos")]
            macos::set_edited(title.1, self.shell.document_path());
            self.title = title;
        }
        // Closing with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.shell.may_close() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if let Some(m) = &self.menu {
            for action in m.poll() {
                match action {
                    menu::Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    menu::Action::LoadFixture => {
                        if let Some(dir) = rfd::FileDialog::new().set_title("Load fixture (a folder of GeoJSON/CSV files)").pick_folder() {
                            self.shell.command(ctx, Command::LoadFixture(dir));
                        }
                    }
                    menu::Action::Shell(cmd) => self.shell.command(ctx, cmd),
                    menu::Action::SaveDocument => self.shell.command(ctx, Command::RequestSaveDocument),
                    menu::Action::OpenDocument => self.shell.command(ctx, Command::RequestOpenDocument),
                    menu::Action::ToggleOffline => {
                        let on = !self.shell.prefs.offline;
                        self.shell.command(ctx, Command::SetOffline(on));
                    }
                    menu::Action::ExportDiagnostics => {
                        let settings = diagnostics::Settings {
                            app: self.shell.app_name(),
                            basemap: self.shell.prefs.basemap.clone(),
                            offline: self.shell.prefs.offline,
                            layers: self.shell.ws.doc.layers.len(),
                        };
                        let report = diagnostics::report(&settings, &diagnostics::logs_dir());
                        self.shell.command(ctx, Command::ReviewDiagnostics(report));
                    }
                    menu::Action::ExportQuestionLog => {
                        let log = question_log_path();
                        if let Some(out) =
                            rfd::FileDialog::new().set_title("Export question log").set_file_name("question log.csv").save_file()
                        {
                            match lm_doc_export(&log, &out) {
                                Ok(n) => log::info!("exported {n} question-log rows"),
                                Err(e) => log::warn!("question log export: {e}"),
                            }
                        }
                    }
                }
            }
            m.sync(&self.shell);
        }
        // Files from Finder's Open With and drops on the Dock icon: the same pipeline as Open.
        #[cfg(target_os = "macos")]
        {
            let opened = macos::take_opened();
            if !opened.is_empty() {
                self.shell.command(ctx, Command::Open(opened));
            }
        }
        for req in self.shell.take_file_requests() {
            match req {
                FileRequest::Open => {
                    if let Some(paths) = rfd::FileDialog::new().set_title("Open").pick_files() {
                        self.shell.command(ctx, Command::Open(paths));
                    }
                }
                FileRequest::SaveText { title, suggested_name, contents } => {
                    if let Some(path) = rfd::FileDialog::new().set_title(title).set_file_name(suggested_name).save_file()
                        && let Err(e) = std::fs::write(&path, contents)
                    {
                        log::warn!("could not write {}: {e}", path.display());
                    }
                }
                FileRequest::SaveBytes { title, suggested_name, bytes } => {
                    if let Some(path) = rfd::FileDialog::new().set_title(title).set_file_name(suggested_name).save_file()
                        && let Err(e) = std::fs::write(&path, bytes)
                    {
                        log::warn!("could not write {}: {e}", path.display());
                    }
                }
                FileRequest::SaveDocument => {
                    if let Some(dir) = rfd::FileDialog::new().set_title("Save document into a folder").pick_folder() {
                        self.shell.command(ctx, Command::SaveDocument(dir));
                    }
                }
                FileRequest::OpenDocument => {
                    if let Some(dir) = rfd::FileDialog::new().set_title("Open a document folder").pick_folder() {
                        self.shell.command(ctx, Command::OpenDocument(dir));
                    }
                }
                FileRequest::SaveAs { suggested_name } => {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save")
                        .set_file_name(suggested_name)
                        .add_filter("LayeredMap map", &["lmap"])
                        .save_file()
                    {
                        self.shell.command(ctx, Command::SaveTo(path));
                    }
                }
                FileRequest::Locate { layer, file_name } => {
                    if let Some(path) = rfd::FileDialog::new().set_title(format!("Locate {file_name}")).set_file_name(file_name).pick_file()
                    {
                        self.shell.command(ctx, Command::Relink(layer, path));
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.ui(ui, self.system);
        if let Some(start) = self.probe
            && self.shell.map_view().renders > 0
        {
            println!("launch_ms={:.0}", start.elapsed().as_secs_f64() * 1000.0);
            self.probe = None;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self) {
        self.shell.shutdown();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.ephemeral {
            return;
        }
        let p = self.shell.prefs_to_save();
        storage.set_string(
            "lm.theme",
            match p.theme {
                ThemePref::System => "system",
                ThemePref::Light => "light",
                ThemePref::Dark => "dark",
            }
            .into(),
        );
        storage.set_string("lm.units", if p.units == Units::Imperial { "imperial" } else { "metric" }.into());
        storage.set_string(BASEMAP_KEY, p.basemap);
        storage.set_string("lm.strength", p.strength.to_string());
        storage.set_string("lm.offline", if p.offline { "on" } else { "off" }.into());
        if let Some((lon, lat, z)) = p.view {
            storage.set_string("lm.view", format!("{lon},{lat},{z}"));
        }
    }
}

/// One step of a `--tour` script (developer testing without touching the pointer or keyboard):
/// `lon,lat,zoom` flies there; `click lon,lat` (or `shift-click lon,lat`) clicks the map there;
/// `layer N` selects the shelf's n-th layer from the top; `tab N` shows an inspector tab;
/// `table` toggles the table drawer; `preset N` applies a style preset; `esc` presses Esc;
/// `basemap ID` switches the basemap (`auto`, `light`, `dark`, `satellite`, `offline`);
/// `find TEXT` finds in the selected layer; `draw` / `draw rect` starts drawing an area;
/// `area lon,lat lon,lat lon,lat…` makes an area layer from an outline;
/// `action ID key=value,key=value` opens the composer for an analysis action on the selected
/// layer (developer surface; `with=NAME` picks the second layer); `keep` keeps its preview;
/// `ask TEXT` sends a request to Pro's Ask box (waits 15 s); `keep-ask` keeps its plan.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Fly(f64, f64, f64),
    Other(Command),
}

impl Step {
    fn parse(s: &str) -> Option<Step> {
        let s = s.trim();
        let nums = |r: &str| r.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).collect::<Vec<f64>>();
        let (word, rest) = s.split_once(' ').unwrap_or((s, ""));
        Some(match word {
            "click" | "shift-click" => match nums(rest).as_slice() {
                [lon, lat] => Step::Other(Command::Click(*lon, *lat, word == "shift-click")),
                _ => return None,
            },
            "layer" => Step::Other(Command::SelectLayerAt(rest.trim().parse().ok()?)),
            "tab" => Step::Other(Command::InspectorTab(rest.trim().parse().ok()?)),
            "table" => Step::Other(Command::ToggleTable),
            "preset" => Step::Other(Command::ApplyPreset(rest.trim().parse().ok()?)),
            "esc" => Step::Other(Command::Escape),
            "basemap" => Step::Other(Command::SetBasemap(rest.trim().to_string())),
            "find" => Step::Other(Command::Find(rest.to_string())),
            "draw" => Step::Other(Command::StartDrawing(if rest.trim() == "rect" {
                lm_ui::draw::Mode::Rectangle
            } else {
                lm_ui::draw::Mode::Polygon
            })),
            "area" => {
                let ring: Vec<(f64, f64)> = rest
                    .split_whitespace()
                    .filter_map(|p| p.split_once(',').and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?))))
                    .collect();
                Step::Other(Command::DrawArea(ring))
            }
            "action" => {
                let (id, args) = rest.trim().split_once(' ').unwrap_or((rest.trim(), ""));
                let params = args
                    .split(',')
                    .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
                    .collect();
                Step::Other(Command::ComposeAction(id.to_string(), params))
            }
            "keep" => Step::Other(Command::KeepResult),
            "ask" => Step::Other(Command::Ask(rest.trim().to_string())),
            "keep-ask" => Step::Other(Command::KeepAsk),
            "dictate" => Step::Other(Command::Dictate),
            "undo" => Step::Other(Command::Undo),
            "redo" => Step::Other(Command::Redo),
            _ => match nums(s).as_slice() {
                [lon, lat, z] => Step::Fly(*lon, *lat, *z),
                _ => return None,
            },
        })
    }

    fn command(self) -> Command {
        match self {
            Step::Fly(lon, lat, z) => Command::FlyTo(lon, lat, z),
            Step::Other(c) => c,
        }
    }
}

/// LayeredMap Pro: the Pro app bundle carries `Contents/Resources/tier` reading "pro"
/// (`packaging/lm/bundle-macos.sh` with `LM_TIER=pro`); `LM_TIER=pro` does the same for a run
/// outside a bundle. Until accounts exist (Phase 7) the tier is the app you open.
fn is_pro() -> bool {
    if std::env::var("LM_TIER").is_ok_and(|v| v.eq_ignore_ascii_case("pro")) {
        return true;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("Resources").join("tier")))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("pro"))
}

fn main() -> eframe::Result {
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let feel_out = arg("--feel").map(std::path::PathBuf::from);
    let runs = arg("--runs").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
    let fixture = arg("--fixture").map(std::path::PathBuf::from);
    let probe = args.iter().any(|a| a == "--launch-probe");
    let tour: Vec<Step> = arg("--tour").map(|v| v.split(';').filter_map(Step::parse).collect()).unwrap_or_default();
    let ephemeral = feel_out.is_some() || probe || !tour.is_empty();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("layeredmap {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let view = args.iter().position(|a| a == "--view").and_then(|i| args.get(i + 1)).and_then(|v| {
        let n: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        match n.as_slice() {
            [lon, lat, z] => Some((*lon, *lat, *z)),
            _ => None,
        }
    });
    let flags = Flags::from_env(std::env::var("LM_PHASE").ok().as_deref());
    let developer = cfg!(debug_assertions) || std::env::var_os("LM_DEV").is_some();
    let pro = is_pro();
    let app_name = if pro { "LayeredMap Pro" } else { "LayeredMap" };
    // Crash logs and the log file stay on this Mac (~/Library/Logs/LayeredMap).
    diagnostics::install_logger();
    diagnostics::install_panic_hook(app_name);
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app_name)
            .with_app_id(if pro { "app.layeredmap.pro" } else { "app.layeredmap.desktop" })
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1024.0, 640.0])
            // The Strata logo, for runs outside the app bundle (the bundle has AppIcon.icns).
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon/icon-256.png")).unwrap_or_default()),
        // The window's size and position are remembered between launches.
        persist_window: !ephemeral,
        ..Default::default()
    };
    if feel_out.is_some() {
        // Frame time is the real cost, not the display's cadence.
        options.wgpu_options.surface.present_mode = eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync;
    }
    #[cfg(target_os = "macos")]
    macos::install_open_handler();
    eframe::run_native(
        app_name,
        options,
        Box::new(move |cc| {
            let mut prefs = if ephemeral { Prefs::default() } else { load_prefs(cc.storage) };
            // `--view lon,lat,zoom` starts there (scripts, screenshots, the feel harness).
            if let Some(v) = view {
                prefs.view = Some(v);
            }
            if feel_out.is_some() && prefs.view.is_none() {
                // Shanghai at about zoom 11, as in the WP0.3 spike.
                prefs.view = Some((121.47, 31.23, 11.0));
            }
            let system = System { dark_mode: cc.egui_ctx.system_theme() == Some(egui::Theme::Dark), reduce_motion: reduce_motion() };
            let mut shell = Shell::new(&cc.egui_ctx, flags, prefs, system);
            shell.developer = developer;
            shell.pro = pro;
            shell.set_library_catalog(lm_ui::library::catalog_url(std::env::var("LM_LIBRARY_URL").ok().as_deref(), developer));
            lm_ui::basemap::start_archive_lookup(&cc.egui_ctx);
            if !ephemeral {
                shell.set_question_log(question_log_path());
                shell.start_autosave(&autosave_dir());
                // The update feed (off unless built with LM_UPDATE_FEED_URL), on its own thread.
                lm_ui::update::start(&cc.egui_ctx, pro);
            }
            // Files passed on the command line (and, through macOS, Open With) are opened.
            let files: Vec<std::path::PathBuf> =
                std::env::args().skip(1).filter(|a| !a.starts_with('-')).map(std::path::PathBuf::from).filter(|p| p.is_file()).collect();
            if !files.is_empty() {
                shell.command(&cc.egui_ctx, Command::Open(files));
            }
            if let Some(dir) = fixture {
                shell.command(&cc.egui_ctx, Command::LoadFixture(dir));
            }
            let feel = feel_out.map(|out| feel::Feel::new(out, runs, shell.map_view().camera()));
            #[cfg(target_os = "macos")]
            macos::set_context(&cc.egui_ctx);
            let menu = menu::MenuBar::install(developer, shell.export_shown(), shell.library_available(), &cc.egui_ctx);
            Ok(Box::new(App {
                shell,
                menu,
                system,
                feel,
                probe: probe.then_some(started),
                ephemeral,
                tour,
                tour_next: None,
                title: (app_name.to_string(), false),
            }))
        }),
    )
}
