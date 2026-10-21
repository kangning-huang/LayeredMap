//! The native menu bar (WP1.1), via `muda`: NimbleMap (`lm_core::brand::NAME`), File, Edit, View, Window, Help on macOS;
//! File, Edit, View, Help on the window on Windows (WP6.2), with Exit in File, About in Help and
//! Ctrl in place of ⌘. Items for features not built yet are hidden, not grayed out. Cut/Copy/Paste
//! are left to egui's own key handling: a menu accelerator for ⌘C would take the key before the
//! text field sees it.
//!
//! The menus are in the interface language (WP5.5). muda can't re-title a whole bar in place, so
//! when the language changes the bar is built again and put back ([`MenuBar::relabel`]).

#[cfg(any(target_os = "macos", windows))]
use muda::accelerator::{Accelerator, Code, Modifiers};
#[cfg(any(target_os = "macos", windows))]
use muda::{AboutMetadata, MenuItem, PredefinedMenuItem, Submenu};
use muda::{CheckMenuItem, Menu, MenuEvent};

use lm_core::i18n::{self, Lang};
#[cfg(any(target_os = "macos", windows))]
use lm_core::tl;
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
    /// View › Language: each item with its preference value (`auto` or a language code).
    languages: Vec<(&'static str, CheckMenuItem)>,
    /// What the bar was built with, to build it again in another language.
    lang: Lang,
    options: Options,
    hwnd: Option<isize>,
}

/// Which optional items the bar shows.
#[derive(Clone, Copy)]
struct Options {
    #[allow(dead_code)] // read when building the bar on macOS/Windows
    developer: bool,
    #[allow(dead_code)]
    export: bool,
    #[allow(dead_code)]
    library: bool,
    #[allow(dead_code)]
    settings: bool,
}

/// ⌘ on macOS, Ctrl on Windows.
#[cfg(any(target_os = "macos", windows))]
const CMD: Modifiers = if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL };

#[cfg(any(target_os = "macos", windows))]
fn cmd(key: Code) -> Option<Accelerator> {
    Some(Accelerator::new(CMD, key))
}

impl MenuBar {
    /// Builds the menu bar in `lang` and installs it: in the app on macOS, on the window `hwnd` on
    /// Windows. `None` if that fails, or on other platforms (Linux is best effort).
    /// `export`: File › Export Map Image… and Export Data… are shown (WP5.3); `library`: File ›
    /// Browse data library… (WP5.4); `settings`: Settings… (⌘,) for Pro's account (WP7.4), in the
    /// app menu on macOS and in File on Windows.
    pub fn install(
        developer: bool,
        export: bool,
        library: bool,
        settings: bool,
        lang: Lang,
        ctx: &egui::Context,
        hwnd: Option<isize>,
    ) -> Option<MenuBar> {
        Self::install_with(Options { developer, export, library, settings }, lang, ctx, hwnd)
    }

    /// Builds the bar again when the interface language has changed (View › Language, or the
    /// system's language while the preference follows it). The old bar is taken down first: on
    /// Windows dropping a menu clears the window's menu, which must not happen to the new one.
    /// The new bar installs its own event handler, replacing the old one and its channel.
    pub fn relabel(this: &mut Option<MenuBar>, shell: &Shell, ctx: &egui::Context) {
        let lang = shell.language();
        let Some(old) = this.take_if(|m| m.lang != lang) else { return };
        let (options, hwnd) = (old.options, old.hwnd);
        drop(old);
        *this = Self::install_with(options, lang, ctx, hwnd);
        if this.is_none() {
            log::warn!("menu bar: couldn't rebuild it in {}", lang.english());
        }
    }

    fn install_with(options: Options, lang: Lang, ctx: &egui::Context, hwnd: Option<isize>) -> Option<MenuBar> {
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = (options, lang, ctx, hwnd);
            None
        }
        #[cfg(any(target_os = "macos", windows))]
        {
            // The menus are labelled as they are built, so in the bar's language.
            i18n::set_current(lang);
            match Self::build(options, lang, ctx, hwnd) {
                Ok(m) => {
                    #[cfg(target_os = "macos")]
                    m._menu.init_for_nsapp();
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
    fn build(options: Options, lang: Lang, ctx: &egui::Context, hwnd: Option<isize>) -> muda::Result<MenuBar> {
        let Options { developer, export, library, settings } = options;
        let menu = Menu::new();
        // The name as this language shows it: 凌波微图 in Chinese.
        let shown = lm_core::brand::shown_in(lang, false);
        let brand = [("app", shown)];
        // macOS titles its standard items itself ("Hide NimbleMap", "Minimize"): those stay as
        // muda makes them in English, and get our translation otherwise.
        #[cfg(target_os = "macos")]
        let standard = |template: &str| (lang != Lang::EN).then(|| i18n::fmt(template, &brand));
        let about = AboutMetadata {
            name: Some(shown.into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some(tl!("A map-first GIS. Built on RustGIS.").into()),
            license: Some("Apache-2.0".into()),
            ..Default::default()
        };
        #[cfg(target_os = "macos")]
        // macOS titles the application menu itself (from the bundle's name), so this one is moot.
        let app = Submenu::with_items(
            shown,
            true,
            &[
                &PredefinedMenuItem::about(Some(&*i18n::fmt("About {app}", &brand)), Some(about)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(standard("Hide {app}").as_deref()),
                &PredefinedMenuItem::hide_others(standard("Hide Others").as_deref()),
                &PredefinedMenuItem::show_all(standard("Show All").as_deref()),
                &PredefinedMenuItem::separator(),
                // Our own Quit, so the window closes normally and its state is saved.
                &MenuItem::with_id("quit", i18n::fmt("Quit {app}", &brand), true, cmd(Code::KeyQ)),
            ],
        )?;
        // Settings… under About, as macOS apps have it; hidden while there is nothing to set.
        #[cfg(target_os = "macos")]
        if settings {
            app.insert(&MenuItem::with_id("settings", tl!("Settings…"), true, cmd(Code::Comma)), 2)?;
            app.insert(&PredefinedMenuItem::separator(), 3)?;
        }
        let file = Submenu::new(tl!("File"), true);
        file.append(&MenuItem::with_id("open", tl!("Open…"), true, cmd(Code::KeyO)))?;
        let examples = Submenu::new(tl!("Open Example"), true);
        for e in lm_ui::examples::EXAMPLES {
            examples.append(&MenuItem::with_id(format!("example:{}", e.id), tl!(e.title), true, None))?; // i18n: menu id
        }
        file.append(&examples)?;
        if library {
            file.append(&MenuItem::with_id("library", tl!("Browse data library…"), true, None))?;
        }
        file.append(&PredefinedMenuItem::separator())?;
        file.append(&MenuItem::with_id("save", tl!("Save"), true, cmd(Code::KeyS)))?;
        file.append(&MenuItem::with_id("save-as", tl!("Save As…"), true, Some(Accelerator::new(CMD | Modifiers::SHIFT, Code::KeyS))))?;
        file.append(&PredefinedMenuItem::separator())?;
        if export {
            file.append(&MenuItem::with_id("export-image", tl!("Export Map Image…"), true, cmd(Code::KeyE)))?;
            file.append(&MenuItem::with_id("export-data", tl!("Export Data…"), true, None))?;
            file.append(&MenuItem::with_id("export-package", tl!("Export Map Package…"), true, None))?;
            file.append(&PredefinedMenuItem::separator())?;
        }
        if developer {
            file.append(&MenuItem::with_id("load-fixture", "Load fixture…", true, None))?; // i18n: developer only
            file.append(&MenuItem::with_id("save-document", "Save document (developer)…", true, None))?; // i18n: developer only
            file.append(&MenuItem::with_id("open-document", "Open document (developer)…", true, None))?; // i18n: developer only
            file.append(&MenuItem::with_id("export-questions", "Export question log…", true, None))?; // i18n: developer only
            file.append(&PredefinedMenuItem::separator())?;
        }
        #[cfg(windows)]
        if settings {
            file.append(&MenuItem::with_id("settings", tl!("Settings…"), true, cmd(Code::Comma)))?;
            file.append(&PredefinedMenuItem::separator())?;
        }
        file.append(&MenuItem::with_id("close", tl!("Close Window"), true, cmd(Code::KeyW)))?;
        // Windows has no application menu: Exit ends File (Alt+F4 is the system's own).
        #[cfg(windows)]
        file.append(&MenuItem::with_id("quit", tl!("Exit"), true, None))?;
        // Undo and Redo carry no menu accelerators: ⌘Z must still reach a text field being
        // edited. The shell handles ⌘Z and ⇧⌘Z when no text field has the keyboard.
        let edit = Submenu::with_items(
            tl!("Edit"),
            true,
            &[
                &MenuItem::with_id("undo", i18n::fmt("Undo ({key})", &[("key", &lm_ui::keys::shortcut("Z"))]), true, None),
                &MenuItem::with_id("redo", i18n::fmt("Redo ({key})", &[("key", &lm_ui::keys::shortcut_shift("Z"))]), true, None),
                &PredefinedMenuItem::separator(),
                &MenuItem::with_id("find", tl!("Find…"), true, cmd(Code::KeyF)),
                &PredefinedMenuItem::separator(),
                &MenuItem::with_id("remove-all", tl!("Remove All Layers"), true, None),
            ],
        )?;
        let theme = [
            CheckMenuItem::with_id("theme-system", tl!("Use System Appearance"), true, true, None),
            CheckMenuItem::with_id("theme-light", tl!("Light"), true, false, None),
            CheckMenuItem::with_id("theme-dark", tl!("Dark"), true, false, None),
        ];
        let units = [
            CheckMenuItem::with_id("units-metric", tl!("Metric Units"), true, true, None),
            CheckMenuItem::with_id("units-imperial", tl!("Imperial Units"), true, false, None),
        ];
        // View › Language: the system's choice, then each language by its own name (untranslated,
        // so anyone can find theirs). Checked in `sync`.
        let mut languages = vec![(i18n::AUTO, CheckMenuItem::with_id("lang:auto", tl!("System Language"), true, false, None))];
        for l in Lang::all() {
            let id = format!("lang:{}", l.code());
            languages.push((l.code(), CheckMenuItem::with_id(id, l.name(), true, false, None))); // i18n: each language's own name
        }
        let language = Submenu::new(tl!("Language"), true);
        language.append(&languages[0].1)?;
        language.append(&PredefinedMenuItem::separator())?;
        for (_, item) in &languages[1..] {
            language.append(item)?;
        }
        let debug =
            CheckMenuItem::with_id("debug", tl!("Show Frame Rate"), true, false, Some(Accelerator::new(CMD | Modifiers::ALT, Code::KeyF)));
        // Strict offline mode (WP6.6): one switch for every network request.
        let offline = CheckMenuItem::with_id("offline", tl!("Work Offline"), true, false, None);
        let view = Submenu::with_items(
            tl!("View"),
            true,
            &[
                &MenuItem::with_id("zoom-in", tl!("Zoom In"), true, cmd(Code::Equal)),
                &MenuItem::with_id("zoom-out", tl!("Zoom Out"), true, cmd(Code::Minus)),
                &MenuItem::with_id("zoom-all", tl!("Zoom to All Data"), true, cmd(Code::Digit0)),
                &PredefinedMenuItem::separator(),
                &theme[0],
                &theme[1],
                &theme[2],
                &PredefinedMenuItem::separator(),
                &units[0],
                &units[1],
                &PredefinedMenuItem::separator(),
                &language,
                &PredefinedMenuItem::separator(),
                &offline,
                &PredefinedMenuItem::separator(),
                &debug,
                // muda's predefined Full Screen is macOS-only; Windows gets its own (F11).
                #[cfg(target_os = "macos")]
                &PredefinedMenuItem::fullscreen(standard("Full Screen").as_deref()),
                #[cfg(windows)]
                &MenuItem::with_id("fullscreen", tl!("Full Screen"), true, Some(Accelerator::new(Modifiers::empty(), Code::F11))),
            ],
        )?;
        #[cfg(target_os = "macos")]
        let window = Submenu::with_items(
            tl!("Window"),
            true,
            &[
                &PredefinedMenuItem::minimize(standard("Minimize").as_deref()),
                &PredefinedMenuItem::maximize(standard("Zoom").as_deref()),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::bring_all_to_front(standard("Bring All to Front").as_deref()),
            ],
        )?;
        let help = Submenu::with_items(
            tl!("Help"),
            true,
            &[
                // The help pages ship inside the app (WP6.10), so this works offline.
                &MenuItem::with_id("help", i18n::fmt("{app} Help", &brand), true, None),
                // Posted publicly on GitHub Discussions through the feedback service (docs/lm/feedback.md).
                &MenuItem::with_id("feedback", tl!("Send Feedback…"), true, None),
                &PredefinedMenuItem::separator(),
                &MenuItem::with_id("credits", tl!("Credits"), true, None),
                &MenuItem::with_id("diagnostics", tl!("Export Diagnostics…"), true, None),
            ],
        )?;
        #[cfg(windows)]
        help.append_items(&[
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::about(Some(&*i18n::fmt("About {app}", &brand)), Some(about)),
        ])?;
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
        Ok(MenuBar { _menu: menu, events, theme, units, debug, offline, languages, lang, options, hwnd })
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
                "feedback" => Action::Shell(Command::ShowFeedback),
                "offline" => Action::ToggleOffline,
                "diagnostics" => Action::ExportDiagnostics,
                "help" => Action::Help,
                "fullscreen" => Action::ToggleFullscreen,
                id if id.starts_with("lang:") => Action::Shell(Command::SetLanguage(id["lang:".len()..].to_string())),
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
        // Anything that isn't a known language follows the system, as `i18n::resolve` does.
        let pref = Lang::from_code(&shell.prefs.language).map_or(i18n::AUTO, Lang::code);
        for (code, item) in &self.languages {
            if item.is_checked() != (*code == pref) {
                item.set_checked(*code == pref);
            }
        }
    }
}
