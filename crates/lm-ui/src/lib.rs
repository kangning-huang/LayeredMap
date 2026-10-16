//! The map-first shell: top bar, map controls, basemap chooser, scale bar, attribution (Phase 1);
//! layer shelf, inspector, composer, result cards, table drawer (later phases). Never calls
//! `rustgis-data` or `rustgis-analysis`: all work goes through `lm-jobs`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod basemap;
mod chrome;
#[cfg(test)]
mod screenshots;
pub mod search;
pub mod theme;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use lm_core::phase::Flags;
/// The map widget, for the app's feel harness (camera scripts and counters).
pub use lm_mapview as mapview;
use lm_mapview::{MapView, ViewOptions};
pub use theme::{Theme, ThemePref};

/// Distances on the scale bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Units {
    #[default]
    Metric,
    Imperial,
}

/// What the menu bar (or a shortcut) asks the shell to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    SetTheme(ThemePref),
    SetUnits(Units),
    ToggleDebugOverlay,
    ZoomIn,
    ZoomOut,
    /// ⌘0: zoom to all data (the world while there is none).
    ZoomToAll,
    FocusSearch,
    ShowCredits,
    SetBasemap(String),
    /// Developer only: load a fixture (a file or a folder of GeoJSON/CSV) as map layers.
    LoadFixture(PathBuf),
}

/// What the shell remembers between launches (the app stores it).
#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    pub theme: ThemePref,
    pub units: Units,
    pub basemap: String,
    /// "Background strength", 0.4–1.0.
    pub strength: f32,
    /// The last view: centre longitude, latitude and zoom. `None` on first launch.
    pub view: Option<(f64, f64, f64)>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { theme: ThemePref::System, units: Units::Metric, basemap: "auto".into(), strength: 1.0, view: None }
    }
}

/// Facts about the system the app reads and passes in each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct System {
    pub dark_mode: bool,
    pub reduce_motion: bool,
}

/// The LayeredMap shell.
pub struct Shell {
    pub flags: Flags,
    pub prefs: Prefs,
    map: MapView,
    theme: Theme,
    theme_applied: Option<Theme>,
    search: search::Search,
    debug_overlay: bool,
    credits_open: bool,
    basemap_open: bool,
    focus_search: bool,
    /// The map's ground colour, painted before the first frame arrives.
    backdrop: egui::Color32,
    loading: Option<Receiver<Result<Vec<lm_jobs::Layer>, String>>>,
    reduce_motion: bool,
    notice: Option<(String, f64)>,
    last_map_size: egui::Vec2,
}

impl Shell {
    pub fn new(ctx: &egui::Context, flags: Flags, prefs: Prefs, system: System) -> Shell {
        ctx.set_fonts(theme::fonts());
        // The renderer keeps only label text a font can draw: give it Chinese.
        if !lm_mapview::add_label_font(theme::NOTO_SC_REGULAR.to_vec()) {
            log::warn!("the bundled Chinese font could not be registered for map labels");
        }
        let dark = basemap::dark_for(prefs.theme, system.dark_mode);
        let map = basemap::initial_map(&prefs, dark);
        let backdrop = basemap::backdrop(&map);
        Shell {
            flags,
            map: MapView::new(map, ctx),
            theme: Theme::of(dark),
            theme_applied: None,
            search: search::Search::new(),
            debug_overlay: false,
            credits_open: false,
            basemap_open: false,
            focus_search: false,
            backdrop,
            loading: None,
            reduce_motion: system.reduce_motion,
            notice: None,
            last_map_size: egui::vec2(1280.0, 748.0),
            prefs,
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn debug_overlay(&self) -> bool {
        self.debug_overlay
    }

    /// A fixture is being loaded.
    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    pub fn map_view(&self) -> &MapView {
        &self.map
    }

    pub fn map_view_mut(&mut self) -> &mut MapView {
        &mut self.map
    }

    /// The prefs as they stand, with the current view, for the app to store.
    pub fn prefs_to_save(&self) -> Prefs {
        let c = self.map.center_lonlat();
        Prefs { view: Some((c.x, c.y, self.map.zoom())), ..self.prefs.clone() }
    }

    pub fn command(&mut self, ctx: &egui::Context, cmd: Command) {
        let t = ctx.input(|i| i.time);
        match cmd {
            Command::SetTheme(p) => self.prefs.theme = p,
            Command::SetUnits(u) => self.prefs.units = u,
            Command::ToggleDebugOverlay => self.debug_overlay = !self.debug_overlay,
            Command::ZoomIn => self.map.zoom_by(1.0, t, self.last_map_size),
            Command::ZoomOut => self.map.zoom_by(-1.0, t, self.last_map_size),
            Command::ZoomToAll => {
                let e = self.map.map().full_extent();
                let e = if e.is_empty() || !e.min_x.is_finite() { basemap::WORLD } else { e };
                self.map.zoom_to_extent(&e, self.last_map_size);
            }
            Command::FocusSearch => self.focus_search = true,
            Command::ShowCredits => self.credits_open = true,
            Command::SetBasemap(id) => {
                self.prefs.basemap = id;
                self.basemap_changed();
            }
            Command::LoadFixture(path) => {
                self.notice = Some((format!("Loading {}…", path.display()), t));
                self.loading = Some(lm_jobs::load_fixture(path));
            }
        }
    }

    fn basemap_changed(&mut self) {
        let (prefs, dark) = (self.prefs.clone(), self.theme.dark);
        self.map.edit_map(|m| basemap::apply(m, &prefs, dark));
        self.backdrop = basemap::backdrop(self.map.map());
    }

    /// One frame of the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui, system: System) {
        let ctx = ui.ctx().clone();
        self.reduce_motion = system.reduce_motion;
        // Theme: follows the system unless overridden; switching also restyles the basemap.
        let dark = basemap::dark_for(self.prefs.theme, system.dark_mode);
        if dark != self.theme.dark {
            self.theme = Theme::of(dark);
            self.basemap_changed();
        }
        // Fonts set with `set_fonts` take effect from the next frame; until the bundled ones (and
        // the semibold family the chrome uses) are active, draw only the map's ground colour.
        let fonts_ready = ctx.fonts(|f| f.definitions().families.contains_key(&egui::FontFamily::Name("semibold".into())));
        if !fonts_ready {
            ui.painter().rect_filled(ui.max_rect(), 0.0, self.backdrop);
            ctx.request_repaint();
            return;
        }
        if self.theme_applied != Some(self.theme) {
            theme::apply(&ctx, &self.theme);
            self.theme_applied = Some(self.theme);
        }
        self.shortcuts(&ctx);
        self.poll_loading(&ctx);

        // Full-bleed: the whole window, whatever margins the host's panel has.
        let full = ctx.content_rect();
        let toolbar = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), theme::TOOLBAR_HEIGHT));
        let map_rect = egui::Rect::from_min_max(egui::pos2(full.min.x, toolbar.max.y), full.max);
        self.last_map_size = map_rect.size();

        // The map first, so the chrome draws over it.
        let keyboard = ctx.memory(|m| m.focused().is_none());
        let out = self.map.show(ui, map_rect, ViewOptions { backdrop: self.backdrop, reduce_motion: system.reduce_motion, keyboard });
        let _ = out;
        chrome::map_overlays(self, ui, map_rect, system);
        chrome::toolbar(self, ui, toolbar, system);
        if self.credits_open {
            chrome::credits(self, &ctx);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (find, zero, debug) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::F),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0),
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::F),
            )
        });
        if debug {
            self.debug_overlay = !self.debug_overlay;
        } else if find {
            self.focus_search = true;
        }
        if zero {
            self.command(ctx, Command::ZoomToAll);
        }
    }

    fn poll_loading(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.loading else { return };
        let t = ctx.input(|i| i.time);
        match rx.try_recv() {
            Ok(Ok(layers)) => {
                let n: usize = layers.iter().map(|l| l.dataset.features.len()).sum();
                self.map.edit_map(|m| {
                    for l in layers {
                        m.add_layer(l);
                    }
                    // Points above areas (index 0 draws on top).
                    m.layers.sort_by_key(|l| u8::from(l.dataset.geometry_type != lm_jobs::GeometryType::Point));
                });
                self.notice = Some((format!("Loaded {n} features"), t));
                self.loading = None;
            }
            Ok(Err(e)) => {
                self.notice = Some((e, t));
                self.loading = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.loading = None,
        }
    }
}
