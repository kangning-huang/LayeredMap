//! The native menu bar (WP1.1), via `muda`: LayeredMap, File, Edit, View, Window, Help. Items for
//! features not built yet are hidden, not grayed out. Cut/Copy/Paste are left to egui's own key
//! handling: a menu accelerator for ⌘C would take the key before the text field sees it.

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use lm_ui::{Command, Shell, ThemePref, Units};

pub enum Action {
    Shell(Command),
    Quit,
    LoadFixture,
}

pub struct MenuBar {
    _menu: Menu,
    theme: [CheckMenuItem; 3],
    units: [CheckMenuItem; 2],
    debug: CheckMenuItem,
}

fn cmd(key: Code) -> Option<Accelerator> {
    Some(Accelerator::new(Modifiers::META, key))
}

impl MenuBar {
    /// Builds the menu bar and installs it in the app (macOS). `None` if that fails, or on other
    /// platforms for now (Windows packaging is Phase 6).
    pub fn install(developer: bool) -> Option<MenuBar> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = developer;
            None
        }
        #[cfg(target_os = "macos")]
        {
            match Self::build(developer) {
                Ok(m) => {
                    m._menu.init_for_nsapp();
                    Some(m)
                }
                Err(e) => {
                    log::error!("menu bar: {e}");
                    None
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn build(developer: bool) -> muda::Result<MenuBar> {
        let menu = Menu::new();
        let about = AboutMetadata {
            name: Some("LayeredMap".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some("A map-first GIS. Built on RustGIS.".into()),
            license: Some("Apache-2.0".into()),
            ..Default::default()
        };
        let app = Submenu::with_items(
            "LayeredMap",
            true,
            &[
                &PredefinedMenuItem::about(Some("About LayeredMap"), Some(about)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                // Our own Quit, so the window closes normally and its state is saved.
                &MenuItem::with_id("quit", "Quit LayeredMap", true, cmd(Code::KeyQ)),
            ],
        )?;
        let file = Submenu::new("File", true);
        if developer {
            file.append(&MenuItem::with_id("load-fixture", "Load fixture…", true, None))?;
            file.append(&PredefinedMenuItem::separator())?;
        }
        file.append(&MenuItem::with_id("close", "Close Window", true, cmd(Code::KeyW)))?;
        let edit = Submenu::with_items("Edit", true, &[&MenuItem::with_id("find", "Find…", true, cmd(Code::KeyF))])?;
        let theme = [
            CheckMenuItem::with_id("theme-system", "Use System Appearance", true, true, None),
            CheckMenuItem::with_id("theme-light", "Light", true, false, None),
            CheckMenuItem::with_id("theme-dark", "Dark", true, false, None),
        ];
        let units = [
            CheckMenuItem::with_id("units-metric", "Metric Units", true, true, None),
            CheckMenuItem::with_id("units-imperial", "Imperial Units", true, false, None),
        ];
        let debug = CheckMenuItem::with_id(
            "debug",
            "Show Frame Rate",
            true,
            false,
            Some(Accelerator::new(Modifiers::META | Modifiers::ALT, Code::KeyF)),
        );
        let view = Submenu::with_items(
            "View",
            true,
            &[
                &MenuItem::with_id("zoom-in", "Zoom In", true, cmd(Code::Equal)),
                &MenuItem::with_id("zoom-out", "Zoom Out", true, cmd(Code::Minus)),
                &MenuItem::with_id("zoom-all", "Zoom to All Data", true, cmd(Code::Digit0)),
                &PredefinedMenuItem::separator(),
                &theme[0],
                &theme[1],
                &theme[2],
                &PredefinedMenuItem::separator(),
                &units[0],
                &units[1],
                &PredefinedMenuItem::separator(),
                &debug,
                &PredefinedMenuItem::fullscreen(None),
            ],
        )?;
        let window = Submenu::with_items(
            "Window",
            true,
            &[
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::maximize(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::bring_all_to_front(None),
            ],
        )?;
        let help = Submenu::with_items("Help", true, &[&MenuItem::with_id("credits", "Credits", true, None)])?;
        for m in [&app, &file, &edit, &view, &window, &help] {
            menu.append(m)?;
        }
        Ok(MenuBar { _menu: menu, theme, units, debug })
    }

    /// The actions the menu asked for since the last poll.
    pub fn poll(&self) -> Vec<Action> {
        let mut out = Vec::new();
        while let Ok(e) = MenuEvent::receiver().try_recv() {
            let a = match e.id().0.as_str() {
                "quit" | "close" => Action::Quit,
                "load-fixture" => Action::LoadFixture,
                "find" => Action::Shell(Command::FocusSearch),
                "zoom-in" => Action::Shell(Command::ZoomIn),
                "zoom-out" => Action::Shell(Command::ZoomOut),
                "zoom-all" => Action::Shell(Command::ZoomToAll),
                "theme-system" => Action::Shell(Command::SetTheme(ThemePref::System)),
                "theme-light" => Action::Shell(Command::SetTheme(ThemePref::Light)),
                "theme-dark" => Action::Shell(Command::SetTheme(ThemePref::Dark)),
                "units-metric" => Action::Shell(Command::SetUnits(Units::Metric)),
                "units-imperial" => Action::Shell(Command::SetUnits(Units::Imperial)),
                "debug" => Action::Shell(Command::ToggleDebugOverlay),
                "credits" => Action::Shell(Command::ShowCredits),
                _ => continue,
            };
            out.push(a);
        }
        out
    }

    /// Keeps the check marks in step with the shell (a shortcut may have changed a setting).
    pub fn sync(&self, shell: &Shell) {
        let t = shell.prefs.theme;
        for (item, v) in self.theme.iter().zip([ThemePref::System, ThemePref::Light, ThemePref::Dark]) {
            if item.is_checked() != (t == v) {
                item.set_checked(t == v);
            }
        }
        let u = shell.prefs.units;
        for (item, v) in self.units.iter().zip([Units::Metric, Units::Imperial]) {
            if item.is_checked() != (u == v) {
                item.set_checked(u == v);
            }
        }
        if self.debug.is_checked() != shell.debug_overlay() {
            self.debug.set_checked(shell.debug_overlay());
        }
    }
}
