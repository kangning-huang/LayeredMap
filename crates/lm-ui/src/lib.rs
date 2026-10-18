//! The map-first shell: top bar, map controls, basemap chooser, scale bar, attribution (Phase 1);
//! opening files, the layer shelf, the layer overview and undo (Phase 2); composer, result cards,
//! table drawer (later phases). Never calls
//! `rustgis-data` or `rustgis-analysis`: all work goes through `lm-jobs`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod analysis;
pub mod ask;
pub mod basemap;
mod chrome;
pub mod diagnostics;
#[cfg(not(target_arch = "wasm32"))]
mod dictation;
pub mod draw;
pub mod examples;
pub mod export;
pub mod export_data;
pub mod find_ui;
pub mod form;
pub mod illustrations;
pub mod insight;
pub mod inspect;
pub mod join_ui;
pub mod library;
#[cfg(test)]
mod offline_harness;
pub mod panels;
pub mod saving;
#[cfg(test)]
mod screenshots;
pub mod search;
pub mod table;
pub mod theme;
pub mod update;
pub mod workspace;

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
    /// Fly to longitude, latitude, zoom, as choosing a search result does (scripts and tests).
    FlyTo(f64, f64, f64),
    /// Open files (the Open dialog's result, a drop, Open With).
    Open(Vec<PathBuf>),
    /// Ask the app for the Open dialog (⌘O, the Open button).
    RequestOpen,
    /// Open the world basics (cities, countries, elevation).
    OpenExamples,
    /// Open one entry of the Examples menu, by id ("storms").
    OpenExample(String),
    Undo,
    Redo,
    /// Show or hide the layer shelf (⌘L, the Layers button).
    ToggleLayers,
    /// Remove every layer at once, as one undoable step (the shelf's "Remove all", the menu bar).
    RemoveAllLayers,
    /// Developer: save the document (manifest.json) into a folder.
    SaveDocument(PathBuf),
    /// Developer: open a document folder.
    OpenDocument(PathBuf),
    /// Developer: ask the app for a folder to save the document into.
    RequestSaveDocument,
    /// Developer: ask the app for a document folder to open.
    RequestOpenDocument,
    /// Scripts: a click on the map at longitude, latitude (⇧ held when true), as a pointer
    /// click there would be.
    Click(f64, f64, bool),
    /// Scripts: select the shelf's n-th layer from the top.
    SelectLayerAt(usize),
    /// Scripts: show an inspector tab (0 Overview, 1 Style, 2 Details).
    InspectorTab(u8),
    /// Scripts and ⌘T: open or close the table drawer.
    ToggleTable,
    /// Scripts: apply the selected layer's n-th style preset.
    ApplyPreset(usize),
    /// Scripts: press Esc (one step of the Esc ladder).
    Escape,
    /// Find in the selected layer (WP3.10; scripts and tests).
    Find(String),
    /// Start drawing an area (WP4.4): from the assistant or the developer surface.
    StartDrawing(draw::Mode),
    /// Make an area layer from an outline in longitude/latitude (scripts and tests).
    DrawArea(Vec<(f64, f64)>),
    /// Developer surface (scripts and tests): open the composer for an analysis action on the
    /// selected layer (and selected object), with parameters such as `distance=500 m`.
    ComposeAction(String, Vec<(String, String)>),
    /// Developer surface: keep the composer's preview.
    KeepResult,
    /// Pro's Ask box (scripts and tests): send a request, keep its plan.
    Ask(String),
    KeepAsk,
    Dictate,
    /// ⌘S: save the map to its file (Save As the first time).
    Save,
    /// ⇧⌘S: save the map to a file the user picks.
    SaveAs,
    /// Save the map as a `.lmap` file here (the Save As dialog's result).
    SaveTo(PathBuf),
    /// Use this file for a layer whose source is missing (the Locate… dialog's result).
    Relink(lm_core::model::LayerId, PathBuf),
    /// The recovery prompt's answer: Restore (true) or Discard.
    Restore(bool),
    /// Share › Map image… (File › Export Map Image…, ⌘E): the map image sheet (WP5.3).
    ExportMapImage,
    /// Share › Data… (File › Export Data…): the data export dialog (WP5.3).
    ExportData,
    /// View › Work Offline: strict offline mode (WP6.6) on or off.
    SetOffline(bool),
    /// Help › Export Diagnostics…: show the app's report for review before it is saved.
    ReviewDiagnostics(String),
    /// Open the data library (WP5.4: the welcome card, File › Browse data library).
    OpenLibrary,
    /// Add a library dataset to the map by its id (scripts and tests; the panel's Add to map).
    AddFromLibrary(String),
}

/// A file dialog the shell asks the app to show (the shell itself never opens dialogs).
#[derive(Clone, Debug, PartialEq)]
pub enum FileRequest {
    /// Choose files to open.
    Open,
    /// Save text to a file the user picks.
    SaveText { title: String, suggested_name: String, contents: String },
    /// Save bytes (an exported image or data file) to a file the user picks.
    SaveBytes { title: String, suggested_name: String, bytes: Vec<u8> },
    /// Developer: pick a folder to save the document into.
    SaveDocument,
    /// Developer: pick a document folder to open.
    OpenDocument,
    /// Pick where to save the map (a `.lmap` file).
    SaveAs { suggested_name: String },
    /// Pick the file a missing layer's source moved to.
    Locate { layer: lm_core::model::LayerId, file_name: String },
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
    /// Strict offline mode (View › Work Offline): no network request of any kind.
    pub offline: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { theme: ThemePref::System, units: Units::Metric, basemap: "auto".into(), strength: 1.0, view: None, offline: false }
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
    pub(crate) map: MapView,
    theme: Theme,
    theme_applied: Option<Theme>,
    pub(crate) search: search::Search,
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
    /// Phase 2: the document, imports and their layers.
    pub ws: workspace::Workspace,
    pub(crate) shelf_open: bool,
    pub(crate) inspector_tab: u8,
    pub(crate) change_for: Option<(lm_core::model::LayerId, lm_core::model::AssumptionKind, lm_core::model::Subject, egui::Rect)>,
    pub(crate) review_for: Option<lm_core::model::LayerId>,
    pub(crate) rename: Option<(lm_core::model::LayerId, String)>,
    /// Renaming just started: the field takes the keyboard this frame only.
    pub(crate) rename_started: bool,
    /// When each question card was first shown (for seconds-to-answer in the question log).
    pub(crate) question_seen: std::collections::HashMap<(u64, u8), f64>,
    pub(crate) drag_from: Option<usize>,
    pub(crate) zoom_to: Option<lm_core::model::Extent>,
    pub(crate) welcome_closed: bool,
    file_requests: Vec<FileRequest>,
    /// Where answered question cards are logged (set by the app).
    question_log: Option<PathBuf>,
    /// Phase 3: the layer whose table drawer is open, its view, and a filter being written.
    pub(crate) table_for: Option<lm_core::model::LayerId>,
    /// A joined table picked on the shelf (its layer and name): the inspector shows that join,
    /// scrolling to it once while `join_scroll` is set.
    pub(crate) join_focus: Option<(lm_core::model::LayerId, String)>,
    pub(crate) join_scroll: bool,
    pub(crate) table: table::TableView,
    pub(crate) filter_draft: Option<table::FilterDraft>,
    /// "Compare with other [Districts]": the ranking chart open for a layer and field.
    pub(crate) compare_peers: Option<(lm_core::model::LayerId, Option<String>)>,
    /// Find in a layer (WP3.10).
    pub(crate) find: find_ui::FindState,
    /// An area being drawn (WP4.4).
    pub(crate) drawing: Option<draw::Drawing>,
    /// The overlap chooser: where the click was and what it hit, topmost first.
    pub(crate) chooser: Option<(egui::Pos2, Vec<lm_core::selection::FeatureRef>)>,
    /// A chooser row being hovered: its object is highlighted.
    pub(crate) chooser_hover: Option<lm_core::selection::FeatureRef>,
    /// "Your data" search results, with the query and layers they were found for.
    pub(crate) data_search: (String, Vec<chrome::DataRow>),
    /// The daily basemap build the map reads, once the newest one is known.
    archive_url: Option<String>,
    /// The developer flag (`LM_DEV=1` or a debug build): shows assistant actions and the
    /// composer (Phase 4's developer surface).
    pub developer: bool,
    /// LayeredMap Pro (the Pro app bundle, or `LM_TIER=pro`): the analysis actions are buttons
    /// and open the composer. Until the assistant (Phase 7) exists this is how Pro asks.
    pub pro: bool,
    /// A question being composed (developer surface).
    pub(crate) composer: Option<analysis::Composer>,
    /// "Summarize in an area I draw": the raster to summarize once the area is drawn.
    pub(crate) after_drawing: Option<lm_core::model::LayerId>,
    /// The Ask box (Pro, early test build of WP7.1).
    pub(crate) ask: ask::AskState,
    /// The map's file, autosave and recovery (WP5.1, WP5.2).
    pub doc_file: saving::DocFile,
    /// The map image sheet and the data export dialog, while open (WP5.3).
    pub(crate) export_sheet: Option<export::ImageSheet>,
    pub(crate) data_export: Option<export_data::DataDialog>,
    /// A diagnostics report open for review (Help › Export Diagnostics…).
    pub(crate) diagnostics: Option<String>,
    /// The data library (WP5.4).
    pub(crate) library: library::LibraryState,
}

impl Shell {
    pub fn new(ctx: &egui::Context, flags: Flags, prefs: Prefs, system: System) -> Shell {
        ctx.set_fonts(theme::fonts());
        // The renderer keeps only label text a font can draw: give it Chinese.
        if !lm_mapview::add_label_font(theme::NOTO_SC_REGULAR.to_vec()) {
            log::warn!("the bundled Chinese font could not be registered for map labels");
        }
        // Offline mode is set before the map exists, so not even its first frame asks for tiles.
        lm_net::set_offline(prefs.offline);
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
            archive_url: None,
            developer: false,
            pro: false,
            composer: None,
            after_drawing: None,
            ask: ask::AskState::default(),
            doc_file: saving::DocFile::default(),
            export_sheet: None,
            data_export: None,
            diagnostics: None,
            library: Default::default(),
            find: find_ui::FindState::default(),
            drawing: None,
            ws: workspace::Workspace::with_waker(ctx),
            shelf_open: true,
            inspector_tab: 0,
            change_for: None,
            review_for: None,
            rename: None,
            rename_started: false,
            question_seen: Default::default(),
            drag_from: None,
            zoom_to: None,
            welcome_closed: false,
            file_requests: Vec::new(),
            question_log: None,
            table_for: None,
            join_focus: None,
            join_scroll: false,
            table: Default::default(),
            filter_draft: None,
            compare_peers: None,
            chooser: None,
            chooser_hover: None,
            data_search: Default::default(),
            prefs,
        }
    }

    /// Whether analysis actions are offered: in Pro, and on the developer surface.
    pub fn analysis_actions(&self) -> bool {
        self.pro || self.developer
    }

    /// Whether export (Share, File › Export…) is offered (WP5.3; see [`export::shown`]).
    pub fn export_shown(&self) -> bool {
        export::shown(self.flags, self.developer, self.pro)
    }

    /// The map image sheet is open.
    pub fn exporting_image(&self) -> bool {
        self.export_sheet.is_some()
    }

    /// "LayeredMap" or "LayeredMap Pro".
    pub fn app_name(&self) -> &'static str {
        if self.pro { "LayeredMap Pro" } else { "LayeredMap" }
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
            Command::Click(lon, lat, shift) => {
                let at = self.map.lonlat_to_screen(lm_mapview::Coord::new(lon, lat)).unwrap_or_default();
                self.map_click(lm_mapview::Coord::new(lon, lat), at, shift);
            }
            Command::SelectLayerAt(n) => {
                if let Some(id) = self.ws.doc.layers.iter().rev().nth(n).map(|l| l.id) {
                    self.ws.select_layer(id);
                }
            }
            Command::InspectorTab(tab) => self.inspector_tab = tab,
            Command::StartDrawing(mode) => {
                self.find.clear();
                self.drawing = Some(draw::Drawing::new(mode));
            }
            Command::DrawArea(ring) => {
                let name = draw::next_name(self.ws.doc.layers.iter().map(|l| l.display_name.clone()));
                if let Some(layer) = lm_jobs::import::drawn_area(&name, &ring) {
                    self.ws.add_imported(vec![layer], u64::MAX);
                    if let Some(id) = self.ws.doc.layers.iter().find(|l| l.display_name == name).map(|l| l.id) {
                        analysis::area_drawn(self, id);
                    }
                }
            }
            Command::ComposeAction(action, params) => analysis::compose_by_id(self, &action, params),
            Command::KeepResult => {
                analysis::keep(self);
            }
            Command::Ask(text) => ask::submit(self, ctx, text),
            Command::KeepAsk => ask::keep_shown(self),
            Command::Dictate => ask::dictate(self, ctx),
            Command::ExportMapImage => {
                if self.export_shown() && self.export_sheet.is_none() {
                    self.data_export = None;
                    self.export_sheet = Some(export::ImageSheet::new(self));
                }
            }
            Command::ExportData => {
                if self.export_shown() {
                    match export_data::DataDialog::new(self, None) {
                        Some(d) => self.data_export = Some(d),
                        None => self.notice = Some(("Open a file first: there is no data to export.".into(), t)),
                    }
                }
            }
            Command::SetOffline(on) => {
                self.prefs.offline = on;
                lm_net::set_offline(on);
                self.basemap_changed();
                // The daily-build lookup put off at launch runs once back online.
                if !on && lm_mapview::archive::wanted() {
                    basemap::start_archive_lookup(ctx);
                }
                let msg = if on { "Working offline: LayeredMap won't use the network." } else { "Back online." };
                self.notice = Some((msg.into(), t));
            }
            Command::ReviewDiagnostics(report) => diagnostics::open(self, report),
            Command::OpenLibrary => library::open(self, ctx),
            Command::AddFromLibrary(id) => library::add(self, ctx, &id),
            Command::Find(text) => {
                if let Some(id) = self.ws.selected() {
                    self.find.layer = Some(id);
                    self.find.query = text;
                }
            }
            Command::ToggleTable => {
                self.table_for = match self.table_for {
                    Some(_) => None,
                    None => self.ws.selected().or_else(|| {
                        self.ws.doc.layers.iter().rev().find(|l| l.profile.kind != lm_core::model::LayerKind::Raster).map(|l| l.id)
                    }),
                };
            }
            Command::ApplyPreset(n) => {
                if let Some(l) = self.ws.selected().and_then(|id| self.ws.doc.layer(id)).cloned()
                    && let Some((_, s)) = lm_jobs::inspect::presets(&l).into_iter().nth(n)
                {
                    let tx = self.ws.doc.set_style_tx(l.id, s);
                    self.ws.commit(tx);
                }
            }
            Command::Escape => {
                if self.drawing.take().is_some() || self.ask.escape() {
                } else if let Some(id) = self.find.layer.filter(|id| self.find.active_for(*id)) {
                    self.find.clear();
                    self.ws.selection = lm_core::selection::SelectionContext::layer(id);
                } else if self.chooser.take().is_none() && self.compare_peers.take().is_none() {
                    self.ws.selection.step_back();
                }
            }
            Command::FlyTo(lon, lat, zoom) => {
                self.map.fly_to(lm_mapview::Coord::new(lon, lat), zoom, t, self.last_map_size, self.reduce_motion);
            }
            Command::Open(paths) => {
                // A map file opens as the document; other files as data.
                let (maps, paths): (Vec<PathBuf>, Vec<PathBuf>) =
                    paths.into_iter().partition(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(lm_doc::EXTENSION)));
                if let Some(m) = maps.first() {
                    saving::open(self, ctx, m, t);
                }
                if !paths.is_empty() {
                    let waker = ctx.clone();
                    self.ws.open(paths, move || waker.request_repaint());
                }
            }
            Command::Save => saving::save(self, t),
            Command::SaveAs => saving::save_as(self),
            Command::SaveTo(path) => saving::save_to(self, path, t),
            Command::Relink(id, path) => saving::relink(self, id, &path, t),
            Command::Restore(yes) => saving::answer_recovery(self, ctx, yes, t),
            Command::RequestOpen => self.file_requests.push(FileRequest::Open),
            Command::OpenExamples => {
                let waker = ctx.clone();
                self.ws.open(examples::paths(), move || waker.request_repaint());
            }
            Command::OpenExample(id) => {
                let waker = ctx.clone();
                self.ws.open(examples::paths_of(&id), move || waker.request_repaint());
            }
            Command::RequestSaveDocument => self.file_requests.push(FileRequest::SaveDocument),
            Command::RequestOpenDocument => self.file_requests.push(FileRequest::OpenDocument),
            Command::Undo => {
                if let Some(label) = self.ws.undo() {
                    self.notice = Some((format!("Undid {}", lower_first(&label)), t));
                }
            }
            Command::Redo => {
                if let Some(label) = self.ws.redo() {
                    self.notice = Some((format!("Redid {}", lower_first(&label)), t));
                }
            }
            Command::ToggleLayers => self.shelf_open = !self.shelf_open,
            Command::RemoveAllLayers => {
                let n = self.ws.doc.layers.len();
                if let Some(tx) = self.ws.doc.remove_all_tx() {
                    self.ws.commit(Some(tx));
                    self.ws.clear_selection();
                    self.find.clear();
                    self.table_for = None;
                    let what = if n == 1 { "1 layer".to_string() } else { format!("{n} layers") };
                    self.notice = Some((format!("Removed {what}. Undo with ⌘Z."), t));
                }
            }
            Command::SaveDocument(dir) => {
                self.ws.doc.view = {
                    let c = self.map.center_lonlat();
                    Some((c.x, c.y, self.map.zoom()))
                };
                let msg = match lm_doc::save(&self.ws.doc, &dir) {
                    Ok(()) => format!("Saved the document in {}", dir.display()),
                    Err(e) => e.to_string(),
                };
                self.notice = Some((msg, t));
            }
            Command::OpenDocument(path) => saving::open(self, ctx, &path, t),
            Command::LoadFixture(path) => {
                self.notice = Some((format!("Loading {}…", path.display()), t));
                self.loading = Some(lm_jobs::load_fixture(path));
            }
        }
    }

    /// The window title: the map's name, with "— Edited" where the system shows no Edited state
    /// of its own (macOS shows it from [`Shell::edited`]).
    pub fn window_title(&self) -> String {
        let name = self.doc_file.name();
        if self.doc_file.edited() && !cfg!(target_os = "macos") { format!("{name} — Edited") } else { name }
    }

    /// Whether the map has changes its file doesn't have.
    pub fn edited(&self) -> bool {
        self.doc_file.edited()
    }

    /// The map's file, once saved or opened.
    pub fn document_path(&self) -> Option<&std::path::Path> {
        self.doc_file.path.as_deref()
    }

    /// Starts autosave and the recovery journal in `dir` (the app's Autosave folder); after a
    /// crash the recovery prompt shows. Also makes closing ask about unsaved changes.
    pub fn start_autosave(&mut self, dir: &std::path::Path) {
        saving::start(self, dir);
    }

    /// The window asked to close: false while "Save changes?" needs an answer.
    pub fn may_close(&mut self) -> bool {
        saving::may_close(self)
    }

    /// A clean quit (the journal is marked clean).
    pub fn shutdown(&mut self) {
        saving::shutdown(self);
    }

    /// File dialogs the app should show, oldest first.
    pub fn take_file_requests(&mut self) -> Vec<FileRequest> {
        std::mem::take(&mut self.file_requests)
    }

    /// The data library's catalog (`library::catalog_url`); `None` hides the library.
    pub fn set_library_catalog(&mut self, url: Option<String>) {
        self.library.url = url;
    }

    /// Whether the data library is offered (the app shows File › Browse data library).
    pub fn library_available(&self) -> bool {
        library::available(self)
    }

    /// Where answered question cards are logged (the app's support folder).
    pub fn set_question_log(&mut self, path: PathBuf) {
        self.question_log = Some(path);
    }

    pub(crate) fn push_file_request(&mut self, r: FileRequest) {
        self.file_requests.push(r);
    }

    pub(crate) fn set_notice(&mut self, text: String, t: f64) {
        self.notice = Some((text, t));
    }

    /// Asks the app to save a layer's problem list (WP2.6).
    pub(crate) fn export_problems(&mut self, id: lm_core::model::LayerId) {
        let Some(data) = self.ws.data(id) else { return };
        let name = self.ws.doc.layer(id).map_or_else(|| "layer".to_string(), |l| l.display_name.clone());
        self.file_requests.push(FileRequest::SaveText {
            title: "Export problem list".into(),
            suggested_name: format!("{name} problems.csv"),
            contents: lm_jobs::import::problem_list_csv(data),
        });
    }

    /// Opens the Compare view and flies to street level at one of the layer's points.
    pub(crate) fn begin_compare(&mut self, id: lm_core::model::LayerId, from_bd09: bool) {
        match self.ws.start_compare(id, from_bd09) {
            Ok(()) => {
                if let Some((lon, lat)) = self.ws.compare.as_ref().map(|c| c.focus) {
                    let size = self.last_map_size;
                    self.map.fly_to(lm_mapview::Coord::new(lon, lat), 16.0, self.last_t(), size, self.reduce_motion);
                }
            }
            Err(e) => self.notice = Some((e, self.last_t())),
        }
    }

    fn last_t(&self) -> f64 {
        self.notice.as_ref().map_or(0.0, |(_, t)| *t).max(self.map.last_time())
    }

    /// Asks the app to show the Open dialog.
    pub(crate) fn request_open(&mut self) {
        self.file_requests.push(FileRequest::Open);
    }

    /// Opens the bundled examples.
    pub(crate) fn open_examples(&mut self) {
        let wake = self.ws.wake.clone();
        self.ws.open(examples::paths(), move || {
            if let Some(w) = &wake {
                w();
            }
        });
    }

    /// The value range a raster layer's colours spread over (its style's, or 2–98% of cells).
    pub(crate) fn raster_range(&self, id: lm_core::model::LayerId) -> Option<(f64, f64)> {
        let l = self.ws.doc.layer(id)?;
        match &self.ws.data(id)?.data {
            lm_jobs::import::LayerData::Raster(r) => lm_jobs::inspect::raster_window(r, &l.style),
            lm_jobs::import::LayerData::Vector(_) => None,
        }
    }

    /// A raster band's whole range.
    pub(crate) fn raster_full_range(&self, id: lm_core::model::LayerId) -> Option<(f64, f64)> {
        let l = self.ws.doc.layer(id)?;
        match &self.ws.data(id)?.data {
            lm_jobs::import::LayerData::Raster(r) => lm_jobs::inspect::raster_full_range(r, lm_jobs::inspect::style_band(r, &l.style)),
            lm_jobs::import::LayerData::Vector(_) => None,
        }
    }

    /// A click on the map (WP3.1): the object under it, the overlap chooser when several are, a
    /// raster cell, or — on empty map — clearing the object selection (the layer stays).
    fn map_click(&mut self, lonlat: lm_mapview::Coord, screen: egui::Pos2, shift: bool) {
        use lm_core::selection::{FeatureRef, PixelPick, SelectionContext};
        self.chooser = None;
        // A click on the map ends a search: what was clicked is shown, not the matches.
        self.find.clear();
        let hits = self.ws.hit_test(&self.map, lonlat);
        match hits.len() {
            0 => {
                if let Some(p) = self.ws.raster_pick(lonlat) {
                    self.ws.selection = SelectionContext::pixel(PixelPick { layer: p.0, lon: lonlat.x, lat: lonlat.y, value: p.1 });
                } else {
                    self.ws.selection.features.clear();
                    self.ws.selection.pixel = None;
                }
            }
            1 => {
                let f: FeatureRef = hits[0];
                if shift {
                    self.ws.selection.toggle(f);
                } else {
                    self.ws.selection = SelectionContext::object(f);
                }
            }
            _ => self.chooser = Some((screen, hits)),
        }
    }

    fn fly_to_extent(&mut self, e: lm_core::model::Extent) {
        let e = lm_mapview::Extent { min_x: e.min_x, min_y: e.min_y, max_x: e.max_x, max_y: e.max_y };
        let (left, right) = self.panel_insets();
        self.map.zoom_to_extent_between(&e, self.last_map_size, left, right);
    }

    /// How much of the map the layer shelf (left) and the inspector (right) cover.
    pub(crate) fn panel_insets(&self) -> (f32, f32) {
        let left = if self.shelf_open && !self.ws.doc.layers.is_empty() { panels::SHELF_WIDTH + theme::PANEL_MARGIN } else { 0.0 };
        let right = if self.ws.selected().is_some() { panels::INSPECTOR_WIDTH + theme::PANEL_MARGIN } else { 0.0 };
        (left, right)
    }

    /// Phase 2 bookkeeping each frame: imports, the map's layers, fitting new data, the log.
    fn update_workspace(&mut self, ctx: &egui::Context) {
        let t = ctx.input(|i| i.time);
        analysis::poll(self);
        ask::poll(self);
        library::poll(self, ctx);
        // Files dropped on the window (Open With and the Dock icon arrive through the app).
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if !dropped.is_empty() {
            self.command(ctx, Command::Open(dropped));
        }
        self.ws.poll();
        if let Some(n) = self.ws.notice.take() {
            self.notice = Some((n, t));
        }
        self.ws.sync_map(&mut self.map);
        if let Some(e) = self.ws.fit.take() {
            self.fly_to_extent(e);
        }
        if let Some(e) = self.zoom_to.take() {
            self.fly_to_extent(e);
        }
        if self.ws.new_data.is_some() && self.notice.as_ref().is_none_or(|(_, at)| t - at > 4.0) {
            self.notice = Some(("New data added outside the view. Zoom to new data with ⌘0.".into(), t));
            self.ws.new_data = None;
        }
        if !self.ws.answers.is_empty() {
            let answers = std::mem::take(&mut self.ws.answers);
            if let Some(path) = &self.question_log {
                let log = lm_doc::QuestionLog::new(path.clone());
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
                for a in answers {
                    if let Err(e) = log.append(&lm_core::question::QuestionEvent::new(now, a.kind, &a.answer, a.seconds)) {
                        log::warn!("question log: {e}");
                    }
                }
            }
        }
        if !self.ws.imports.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    fn basemap_changed(&mut self) {
        let (prefs, dark) = (self.prefs.clone(), self.theme.dark);
        self.map.edit_map(move |m| basemap::apply(m, &prefs, dark));
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
            // Selected: a 3 px accent outline with a 2 px halo, white in light, black in dark.
            let a = self.theme.accent;
            let halo = if self.theme.dark { [0, 0, 0, 255] } else { [255, 255, 255, 255] };
            self.map.set_selection_style(lm_mapview::SelectionStyle::Outline {
                accent: lm_mapview::map::Rgba([a.r(), a.g(), a.b(), 255]),
                halo: lm_mapview::map::Rgba(halo),
            });
        }
        // The newest daily basemap build, once found, replaces the pinned one.
        if let Some(u) = lm_mapview::archive::found()
            && self.archive_url.as_ref() != Some(&u)
        {
            self.archive_url = Some(u);
            self.basemap_changed();
        }
        // The map image sheet takes the whole window (and the keyboard) while it is open.
        if self.export_sheet.is_none() {
            self.shortcuts(&ctx);
        }
        self.poll_loading(&ctx);
        self.update_workspace(&ctx);
        saving::frame(self, &ctx);
        saving::close_if_saved(self, &ctx);
        if self.export_sheet.is_some() {
            export::sheet(self, ui, ctx.content_rect());
            return;
        }

        // Full-bleed: the whole window, whatever margins the host's panel has.
        let full = ctx.content_rect();
        let toolbar = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), theme::TOOLBAR_HEIGHT));
        let map_rect = egui::Rect::from_min_max(egui::pos2(full.min.x, toolbar.max.y), full.max);
        self.last_map_size = map_rect.size();

        // The map first, so the chrome draws over it.
        let keyboard = ctx.memory(|m| m.focused().is_none());
        let drawing = self.drawing.is_some();
        let out =
            self.map.show(ui, map_rect, ViewOptions { backdrop: self.backdrop, reduce_motion: system.reduce_motion, keyboard, drawing });
        if drawing {
            draw::frame(self, ui, map_rect, &out);
        } else if let Some(lonlat) = out.clicked.filter(|_| self.flags.shows(3)) {
            let (pos, shift) = ctx.input(|i| (i.pointer.interact_pos(), i.modifiers.shift));
            self.map_click(lonlat, pos.unwrap_or(map_rect.center()), shift);
        }
        // "Fit range to view" follows the view once the camera settles.
        if !out.moving {
            let tl = self.map.screen_to_lonlat(map_rect.left_top());
            let br = self.map.screen_to_lonlat(map_rect.right_bottom());
            if let (Some(a), Some(b)) = (tl, br) {
                let e = lm_core::model::Extent { min_x: a.x.min(b.x), min_y: a.y.min(b.y), max_x: a.x.max(b.x), max_y: a.y.max(b.y) };
                if self.ws.view_extent != Some(e) {
                    self.ws.view_extent = Some(e);
                }
            }
        }
        // The controls move left of the inspector when it is open (Layout).
        let overlay_rect = if self.ws.selected().is_some() {
            egui::Rect::from_min_max(
                map_rect.min,
                egui::pos2(map_rect.max.x - panels::INSPECTOR_WIDTH - theme::PANEL_MARGIN, map_rect.max.y),
            )
        } else {
            map_rect
        };
        chrome::map_overlays(self, ui, overlay_rect, system);
        let notice_rect = if self.shelf_open && !(self.ws.doc.layers.is_empty() && self.ws.imports.is_empty()) {
            egui::Rect::from_min_max(
                egui::pos2(map_rect.min.x + panels::SHELF_WIDTH + theme::PANEL_MARGIN, map_rect.min.y),
                overlay_rect.max,
            )
        } else {
            overlay_rect
        };
        chrome::notices_in(self, ui, notice_rect);
        panels::shelf(self, ui, map_rect);
        panels::inspector(self, ui, map_rect);
        {
            let (left, right) = self.panel_insets();
            table::drawer(self, ui, map_rect, left, right);
        }
        panels::chooser(self, ui);
        panels::change_popover(self, ui);
        panels::review(self, &ctx);
        panels::compare_banner(self, ui, overlay_rect);
        ask::ask_box(self, ui, map_rect);
        update::notice(self, ui, overlay_rect);
        chrome::toolbar(self, ui, toolbar, system);
        if self.credits_open {
            chrome::credits(self, &ctx);
        }
        export_data::dialog(self, &ctx);
        diagnostics::window(self, &ctx);
        library::panel(self, &ctx);
        panels::drop_overlay(self, ui, full);
        saving::recovery_prompt(self, ui, map_rect);
        saving::close_prompt(self, ui, map_rect);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let export_image = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::E));
        if export_image {
            self.command(ctx, Command::ExportMapImage);
        }
        let (find, zero, debug, open, layers, table_key) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::F),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0),
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::F),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::O),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::L),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::T),
            )
        });
        if table_key && self.flags.shows(3) {
            self.command(ctx, Command::ToggleTable);
        }
        if open {
            self.file_requests.push(FileRequest::Open);
        }
        if layers {
            self.shelf_open = !self.shelf_open;
        }
        // Undo and redo, unless a text field has the keyboard (it undoes its own typing).
        if ctx.memory(|m| m.focused().is_none()) {
            let (redo, undo) = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z),
                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
                )
            });
            if redo {
                self.command(ctx, Command::Redo);
            } else if undo {
                self.command(ctx, Command::Undo);
            }
        }
        if debug {
            self.debug_overlay = !self.debug_overlay;
        } else if find {
            // ⌘F finds in the selected layer; with none selected it searches places and data.
            match self.ws.selected().and_then(|id| self.ws.doc.layer(id)) {
                Some(l) if l.profile.kind != lm_core::model::LayerKind::Raster => self.find.focus = true,
                _ => self.focus_search = true,
            }
        }
        if zero {
            self.command(ctx, Command::ZoomToAll);
        }
        // ⌘S and ⇧⌘S come from the menu bar on macOS; elsewhere the shell takes them.
        #[cfg(not(target_os = "macos"))]
        {
            let (save_as, save) = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S),
                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
                )
            });
            if save_as {
                self.command(ctx, Command::SaveAs);
            } else if save {
                self.command(ctx, Command::Save);
            }
        }
    }

    fn poll_loading(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.loading else { return };
        let t = ctx.input(|i| i.time);
        match rx.try_recv() {
            Ok(Ok(layers)) => {
                let n: usize = layers.iter().map(|l| l.dataset.features.len()).sum();
                self.map.edit_map(move |m| {
                    for l in &layers {
                        m.add_layer(l.clone());
                    }
                    // Points above areas (index 0 draws on top).
                    m.layers.sort_by_key(|l| u8::from(l.dataset.geometry_type != lm_jobs::GeometryType::Point));
                });
                self.notice = Some((format!("Loaded {} features", lm_core::model::group_thousands(n as u64)), t));
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

/// "Open schools.csv" → "open schools.csv", for "Undid …".
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Exports the question log (JSON lines) to CSV for pilot analysis; returns the rows written.
pub fn export_question_log(log: &std::path::Path, out: &std::path::Path) -> Result<usize, String> {
    lm_doc::export_csv(log, out).map_err(|e| e.to_string())
}
