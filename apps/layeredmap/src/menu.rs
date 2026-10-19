//! The native menu bar (WP1.1), via `muda`: NimbleMap (`lm_core::brand::NAME`), File, Edit, View, Window, Help on macOS;
//! File, Edit, View, Help on the window on Windows (WP6.2), with Exit in File, About in Help and
//! Ctrl in place of ⌘. Items for features not built yet are hidden, not grayed out. Cut/Copy/Paste
//! are left to egui's own key handling: a menu accelerator for ⌘C would take the key before the
//! text field sees it.

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use lm_core::brand::NAME;
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
    Help,
    ToggleFullscreen,
}

pub struct MenuBar {
    _menu: Menu,
    events: std::sync::mpsc::Receiver<MenuEvent>,
    theme: [CheckMenuItem; 3],
    units: [CheckMenuItem; 2],
    debug: CheckMenuItem,
    offline: CheckMenuItem,
}

/// ⌘ on macOS, Ctrl on Windows.
const CMD: Modifiers = if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL };

fn cmd(key: Code) -> Option<Accelerator> {
    Some(Accelerator::new(CMD, key))
}

impl MenuBar {
    /// Builds the menu bar and installs it: in the app on macOS, on the window `hwnd` on Windows.
    /// `None` if that fails, or on other platforms (Linux is best effort).
    /// `export`: File › Export Map Image… and Export Data… are shown (WP5.3); `library`: File ›
    /// Browse data library… (WP5.4); `settings`: Settings… (⌘,) for Pro's account (WP7.4), in the
    /// app menu on macOS and in File on Windows.
    pub fn install(
        developer: bool,
        export: bool,
        library: bool,
        settings: bool,
        ctx: &egui::Context,
        hwnd: Option<isize>,
    ) -> Option<MenuBar> {
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = (developer, export, library, settings, ctx, hwnd);
            None
        }
        #[cfg(any(target_os = "macos", windows))]
        {
            match Self::build(developer, export, library, settings, ctx) {
                Ok(m) => {
                    #[cfg(target_os = "macos")]
                    {
                        let _ = hwnd;
                        m._menu.init_for_nsapp();
                    }
                    #[cfg(windows)]
                    if !crate::windows::attach_menu(&m._menu, hwnd?) {
                        return None;
                    }
                    Some(m)
                }
                Err(e) => {
                    log::error!("menu bar: {e}");
                    None
                }
            }
        }
    }

    #[cfg(any(target_os = "macos", windows))]
    fn build(developer: bool, export: bool, library: bool, settings: bool, ctx: &egui::Context) -> muda::Result<MenuBar> {
        let menu = Menu::new();
        let about = AboutMetadata {
            name: Some(NAME.into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some("A map-first GIS. Built on RustGIS.".into()),
            license: Some("Apache-2.0".into()),
            ..Default::default()
        };
        #[cfg(target_os = "macos")]
        let app = Submenu::with_items(
            NAME,
            true,
            &[
                &PredefinedMenuItem::about(Some(&*format!("About {NAME}")), Some(about)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                // Our own Quit, so the window closes normally and its state is saved.
                &MenuItem::with_id("quit", format!("Quit {NAME}"), true, cmd(Code::KeyQ)),
            ],
        )?;
        // Settings… under About, as macOS apps have it; hidden while there is nothing to set.
        #[cfg(target_os = "macos")]
        if settings {
            app.insert(&MenuItem::with_id("settings", "Settings…", true, cmd(Code::Comma)), 2)?;
            app.insert(&PredefinedMenuItem::separator(), 3)?;
        }
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
        file.append(&MenuItem::with_id("save-as", "Save As…", true, Some(Accelerator::new(CMD | Modifiers::SHIFT, Code::KeyS))))?;
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
        #[cfg(windows)]
        if settings {
            file.append(&MenuItem::with_id("settings", "Settings…", true, cmd(Code::Comma)))?;
            file.append(&PredefinedMenuItem::separator())?;
        }
        file.append(&MenuItem::with_id("close", "Close Window", true, cmd(Code::KeyW)))?;
        // Windows has no application menu: Exit ends File (Alt+F4 is the system's own).
        #[cfg(windows)]
        file.append(&MenuItem::with_id("quit", "Exit", true, None))?;
        // Undo and Redo carry no menu accelerators: ⌘Z must still reach a text field being
        // edited. The shell handles ⌘Z and ⇧⌘Z when no text field has the keyboard.
        let edit = Submenu::with_items(
            "Edit",
            true,
            &[
                &MenuItem::with_id("undo", format!("Undo ({})", lm_ui::keys::shortcut("Z")), true, None),
                &MenuItem::with_id("redo", format!("Redo ({})", lm_ui::keys::shortcut_shift("Z")), true, None),
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
        let debug =
            CheckMenuItem::with_id("debug", "Show Frame Rate", true, false, Some(Accelerator::new(CMD | Modifiers::ALT, Code::KeyF)));
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
                // muda's predefined Full Screen is macOS-only; Windows gets its own (F11).
                #[cfg(target_os = "macos")]
                &PredefinedMenuItem::fullscreen(None),
                #[cfg(windows)]
                &MenuItem::with_id("fullscreen", "Full Screen", true, Some(Accelerator::new(Modifiers::empty(), Code::F11))),
            ],
        )?;
        #[cfg(target_os = "macos")]
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
            &[
                // The help pages ship inside the app (WP6.10), so this works offline.
                &MenuItem::with_id("help", format!("{NAME} Help"), true, None),
                &PredefinedMenuItem::separator(),
                &MenuItem::with_id("credits", "Credits", true, None),
                &MenuItem::with_id("diagnostics", "Export Diagnostics…", true, None),
            ],
        )?;
        #[cfg(windows)]
        help.append_items(&[&PredefinedMenuItem::separator(), &PredefinedMenuItem::about(Some(&*format!("About {NAME}")), Some(about))])?;
        #[cfg(target_os = "macos")]
        let menus = [&app, &file, &edit, &view, &window, &help];
        #[cfg(windows)]
        let menus = [&file, &edit, &view, &help];
        for m in menus {
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
                "settings" => Action::Shell(Command::ShowSettings),
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
                "help" => Action::Help,
                "fullscreen" => Action::ToggleFullscreen,
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
