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
    SaveDocument,
    OpenDocument,
    ExportQuestionLog,
    ToggleOffline,
    ExportDiagnostics,
}

pub struct MenuBar {
    _menu: Menu,
    events: std::sync::mpsc::Receiver<MenuEvent>,
    theme: [CheckMenuItem; 3],
    units: [CheckMenuItem; 2],
    debug: CheckMenuItem,
    offline: CheckMenuItem,
}

fn cmd(key: Code) -> Option<Accelerator> {
    Some(Accelerator::new(Modifiers::META, key))
}

impl MenuBar {
    /// Builds the menu bar and installs it in the app (macOS). `None` if that fails, or on other
    /// platforms for now (Windows packaging is Phase 6).
    /// `export`: File › Export Map Image… and Export Data… are shown (WP5.3); `library`: File ›
    /// Browse data library… (WP5.4).
    pub fn install(developer: bool, export: bool, library: bool, ctx: &egui::Context) -> Option<MenuBar> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (developer, export, library, ctx);
            None
        }
        #[cfg(target_os = "macos")]
        {
            match Self::build(developer, export, library, ctx) {
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
    fn build(developer: bool, export: bool, library: bool, ctx: &egui::Context) -> muda::Result<MenuBar> {
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
        file.append(&MenuItem::with_id("open", "Open…", true, cmd(Code::KeyO)))?;
        let examples = Submenu::new("Open Example", true);
        for e in lm_ui::examples::EXAMPLES {
            examples.append(&MenuItem::with_id(format!("example:{}", e.id), e.title, true, None))?;
        }
        file.append(&examples)?;
        if library {
            file.append(&MenuItem::with_id("library", "Browse data library…", true, None))?;
        }
        file.append(&PredefinedMenuItem::separator())?;
        file.append(&MenuItem::with_id("save", "Save", true, cmd(Code::KeyS)))?;
        file.append(&MenuItem::with_id(
            "save-as",
            "Save As…",
            true,
            Some(Accelerator::new(Modifiers::META | Modifiers::SHIFT, Code::KeyS)),
        ))?;
        file.append(&PredefinedMenuItem::separator())?;
        if export {
            file.append(&MenuItem::with_id("export-image", "Export Map Image…", true, cmd(Code::KeyE)))?;
            file.append(&MenuItem::with_id("export-data", "Export Data…", true, None))?;
            file.append(&MenuItem::with_id("export-package", "Export Map Package…", true, None))?;
            file.append(&PredefinedMenuItem::separator())?;
        }
        if developer {
            file.append(&MenuItem::with_id("load-fixture", "Load fixture…", true, None))?;
            file.append(&MenuItem::with_id("save-document", "Save document (developer)…", true, None))?;
            file.append(&MenuItem::with_id("open-document", "Open document (developer)…", true, None))?;
            file.append(&MenuItem::with_id("export-questions", "Export question log…", true, None))?;
            file.append(&PredefinedMenuItem::separator())?;
        }
        file.append(&MenuItem::with_id("close", "Close Window", true, cmd(Code::KeyW)))?;
        // Undo and Redo carry no menu accelerators: ⌘Z must still reach a text field being
        // edited. The shell handles ⌘Z and ⇧⌘Z when no text field has the keyboard.
        let edit = Submenu::with_items(
            "Edit",
            true,
            &[
                &MenuItem::with_id("undo", "Undo (⌘Z)", true, None),
                &MenuItem::with_id("redo", "Redo (⇧⌘Z)", true, None),
                &PredefinedMenuItem::separator(),
                &MenuItem::with_id("find", "Find…", true, cmd(Code::KeyF)),
                &PredefinedMenuItem::separator(),
                &MenuItem::with_id("remove-all", "Remove All Layers", true, None),
            ],
        )?;
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
        // Strict offline mode (WP6.6): one switch for every network request.
        let offline = CheckMenuItem::with_id("offline", "Work Offline", true, false, None);
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
                &offline,
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
        let help = Submenu::with_items(
            "Help",
            true,
            &[&MenuItem::with_id("credits", "Credits", true, None), &MenuItem::with_id("diagnostics", "Export Diagnostics…", true, None)],
        )?;
        for m in [&app, &file, &edit, &view, &window, &help] {
            menu.append(m)?;
        }
        // egui draws only when something happens: a menu choice must wake it, or the choice would
        // wait for the next mouse move.
        let (tx, events) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let _ = tx.send(e);
            ctx.request_repaint();
        }));
        Ok(MenuBar { _menu: menu, events, theme, units, debug, offline })
    }

    /// The actions the menu asked for since the last poll.
    pub fn poll(&self) -> Vec<Action> {
        let mut out = Vec::new();
        while let Ok(e) = self.events.try_recv() {
            let a = match e.id().0.as_str() {
                "quit" | "close" => Action::Quit,
                "load-fixture" => Action::LoadFixture,
                "open" => Action::Shell(Command::RequestOpen),
                "open-examples" => Action::Shell(Command::OpenExamples),
                id if id.starts_with("example:") => Action::Shell(Command::OpenExample(id["example:".len()..].to_string())),
                "save" => Action::Shell(Command::Save),
                "save-as" => Action::Shell(Command::SaveAs),
                "export-image" => Action::Shell(Command::ExportMapImage),
                "export-data" => Action::Shell(Command::ExportData),
                "export-package" => Action::Shell(Command::ExportPackage),
                "library" => Action::Shell(Command::OpenLibrary),
                "undo" => Action::Shell(Command::Undo),
                "redo" => Action::Shell(Command::Redo),
                "save-document" => Action::SaveDocument,
                "open-document" => Action::OpenDocument,
                "export-questions" => Action::ExportQuestionLog,
                "find" => Action::Shell(Command::FocusSearch),
                "remove-all" => Action::Shell(Command::RemoveAllLayers),
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
                "offline" => Action::ToggleOffline,
                "diagnostics" => Action::ExportDiagnostics,
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
        if self.offline.is_checked() != shell.prefs.offline {
            self.offline.set_checked(shell.prefs.offline);
        }
    }
}
