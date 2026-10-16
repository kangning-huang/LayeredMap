//! LayeredMap: the app entry point (WP1.1). A window that remembers its size and position, the
//! native macOS menu bar (LayeredMap, File, Edit, View, Window, Help), stored preferences, and the
//! map-first shell from `lm-ui`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod feel;
mod menu;

use lm_core::phase::Flags;
use lm_ui::{Command, Prefs, Shell, System, ThemePref, Units};

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
    tour: Vec<(f64, f64, f64)>,
    tour_next: Option<f64>,
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
                    let (lon, lat, z) = self.tour.remove(0);
                    self.shell.command(ctx, Command::FlyTo(lon, lat, z));
                }
                self.tour_next = Some(t + 6.0);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(m) = &self.menu {
            for action in m.poll() {
                match action {
                    menu::Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    menu::Action::LoadFixture => {
                        if let Some(dir) = rfd::FileDialog::new().set_title("Load fixture (a folder or a GeoJSON/CSV file)").pick_folder() {
                            self.shell.command(ctx, Command::LoadFixture(dir));
                        }
                    }
                    menu::Action::Shell(cmd) => self.shell.command(ctx, cmd),
                }
            }
            m.sync(&self.shell);
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
        if let Some((lon, lat, z)) = p.view {
            storage.set_string("lm.view", format!("{lon},{lat},{z}"));
        }
    }
}

fn main() -> eframe::Result {
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let feel_out = arg("--feel").map(std::path::PathBuf::from);
    let runs = arg("--runs").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
    let fixture = arg("--fixture").map(std::path::PathBuf::from);
    let probe = args.iter().any(|a| a == "--launch-probe");
    let tour: Vec<(f64, f64, f64)> = arg("--tour")
        .map(|v| {
            v.split(';')
                .filter_map(|p| {
                    let n: Vec<f64> = p.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    match n.as_slice() {
                        [lon, lat, z] => Some((*lon, *lat, *z)),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
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
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LayeredMap")
            .with_app_id("app.layeredmap.desktop")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1024.0, 640.0]),
        // The window's size and position are remembered between launches.
        persist_window: !ephemeral,
        ..Default::default()
    };
    if feel_out.is_some() {
        // Frame time is the real cost, not the display's cadence.
        options.wgpu_options.surface.present_mode = eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync;
    }
    eframe::run_native(
        "LayeredMap",
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
            if let Some(dir) = fixture {
                shell.command(&cc.egui_ctx, Command::LoadFixture(dir));
            }
            let feel = feel_out.map(|out| feel::Feel::new(out, runs, shell.map_view().camera()));
            let menu = menu::MenuBar::install(developer, &cc.egui_ctx);
            Ok(Box::new(App { shell, menu, system, feel, probe: probe.then_some(started), ephemeral, tour, tour_next: None }))
        }),
    )
}
