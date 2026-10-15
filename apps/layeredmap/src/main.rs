//! LayeredMap: the app entry point (WP1.1). A window that remembers its size and position, the
//! native macOS menu bar (LayeredMap, File, Edit, View, Window, Help), stored preferences, and the
//! map-first shell from `lm-ui`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod menu;

use lm_core::phase::Flags;
use lm_ui::{Command, Prefs, Shell, System, ThemePref, Units};

struct App {
    shell: Shell,
    menu: Option<menu::MenuBar>,
    system: System,
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

fn load_prefs(storage: Option<&dyn eframe::Storage>) -> Prefs {
    let mut p = Prefs::default();
    let Some(s) = storage else { return p };
    p.theme = match s.get_string("lm.theme").as_deref() {
        Some("light") => ThemePref::Light,
        Some("dark") => ThemePref::Dark,
        _ => ThemePref::System,
    };
    p.units = if s.get_string("lm.units").as_deref() == Some("imperial") { Units::Imperial } else { Units::Metric };
    if let Some(b) = s.get_string("lm.basemap").filter(|b| lm_core::basemap::providers().iter().any(|p| p.id == b)) {
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
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
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
        storage.set_string("lm.basemap", p.basemap);
        storage.set_string("lm.strength", p.strength.to_string());
        if let Some((lon, lat, z)) = p.view {
            storage.set_string("lm.view", format!("{lon},{lat},{z}"));
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LayeredMap")
            .with_app_id("app.layeredmap.desktop")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1024.0, 640.0]),
        // The window's size and position are remembered between launches.
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "LayeredMap",
        options,
        Box::new(move |cc| {
            let mut prefs = load_prefs(cc.storage);
            // `--view lon,lat,zoom` starts there (scripts, screenshots, the feel harness).
            if let Some(v) = view {
                prefs.view = Some(v);
            }
            let system = System { dark_mode: cc.egui_ctx.system_theme() == Some(egui::Theme::Dark), reduce_motion: reduce_motion() };
            let shell = Shell::new(&cc.egui_ctx, flags, prefs, system);
            let menu = menu::MenuBar::install(developer);
            Ok(Box::new(App { shell, menu, system }))
        }),
    )
}
