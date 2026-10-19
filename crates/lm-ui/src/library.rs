//! The data library panel (WP5.4): course datasets from a catalog hosted next to the basemap.
//! Opened from the welcome card ("Browse data library") and File › Browse data library. "Add to
//! map" downloads into the cache with progress and Cancel, checks the checksums, and opens the file
//! through the normal import pipeline, assumptions included. Downloads, the catalog and its
//! thumbnails are read off the interface thread by `lm_jobs::library`; a dataset already in the
//! cache opens without the network.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, RichText, Sense, Stroke, Ui, vec2};
use egui_phosphor::regular as icon;
use lm_jobs::library::{self, Catalog, CatalogEvent, CatalogJob, Dataset, Download, Kind, LibraryError};

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, RADIUS_CARD, regular, semibold};

/// The phase that introduces the library (shown earlier only on the developer surface).
pub const LIBRARY_PHASE: u8 = 5;

/// The local test catalog (developer builds without `LM_LIBRARY_URL`).
const DEV_CATALOG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/library/catalog.json");

const CARD_W: f32 = 200.0;
const THUMB_H: f32 = 133.0;
const GAP: f32 = 12.0;
const COLUMNS: usize = 3;

/// The catalog to read: `LM_LIBRARY_URL` (the app passes its value), else the production catalog
/// once it has a host, else — in developer builds — the local test catalog.
pub fn catalog_url(env: Option<&str>, developer: bool) -> Option<String> {
    env.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| library::DEFAULT_CATALOG_URL.map(str::to_string))
        .or_else(|| developer.then(|| format!("file://{DEV_CATALOG}")))
}

/// The library panel's state.
#[derive(Default)]
pub struct LibraryState {
    pub open: bool,
    /// The catalog's URL; `None` hides the library.
    pub url: Option<String>,
    /// The download cache (`lm_jobs::library::cache_dir()` unless set).
    pub cache: Option<PathBuf>,
    job: Option<CatalogJob>,
    pub catalog: Option<Catalog>,
    /// The network failed and the catalog saved on an earlier visit is shown.
    saved_copy: bool,
    error: Option<String>,
    thumbs: HashMap<String, egui::TextureHandle>,
    /// Datasets already in the cache.
    cached: HashSet<String>,
    pub query: String,
    downloads: Vec<Download>,
    /// The last failure per dataset, in plain words.
    errors: HashMap<String, String>,
    /// Datasets asked for before the catalog arrived.
    wanted: Vec<String>,
}

impl LibraryState {
    pub(crate) fn cache(&self) -> PathBuf {
        self.cache.clone().unwrap_or_else(library::cache_dir)
    }

    /// How many thumbnails are in (tests).
    pub fn thumbs_loaded(&self) -> usize {
        self.thumbs.len()
    }

    fn downloading(&self, id: &str) -> Option<&Download> {
        self.downloads.iter().find(|d| d.id == id)
    }
}

/// Whether the library's entry points (welcome card, File menu) are shown.
pub fn available(shell: &Shell) -> bool {
    shell.library.url.is_some() && (shell.flags.shows(LIBRARY_PHASE) || shell.developer)
}

fn wake(shell: &Shell, ctx: &egui::Context) -> lm_jobs::jobs::Wake {
    shell.ws.wake.clone().unwrap_or_else(|| {
        let c = ctx.clone();
        Arc::new(move || c.request_repaint())
    })
}

/// Opens the panel, reading the catalog if it isn't in yet.
pub(crate) fn open(shell: &mut Shell, ctx: &egui::Context) {
    if shell.library.url.is_none() {
        return;
    }
    shell.library.open = true;
    if shell.library.catalog.is_none() && shell.library.job.is_none() {
        load(shell, ctx);
    }
}

fn load(shell: &mut Shell, ctx: &egui::Context) {
    let Some(url) = shell.library.url.clone() else { return };
    shell.library.error = None;
    let w = wake(shell, ctx);
    shell.library.job = Some(library::load_catalog(url, shell.library.cache(), w));
}

/// "Add to map": opens a cached dataset at once, otherwise starts its download.
pub(crate) fn add(shell: &mut Shell, ctx: &egui::Context, id: &str) {
    if shell.library.downloading(id).is_some() {
        return;
    }
    let Some(d) = shell.library.catalog.as_ref().and_then(|c| c.get(id)).cloned() else {
        // Not in yet (a script, or a click racing the catalog): add it once the catalog arrives.
        if !shell.library.wanted.iter().any(|w| w == id) {
            shell.library.wanted.push(id.to_string());
        }
        open(shell, ctx);
        return;
    };
    shell.library.errors.remove(id);
    let w = wake(shell, ctx);
    let job = library::download(&d, shell.library.cache(), w);
    shell.library.downloads.push(job);
}

/// Each frame: news from the catalog and the downloads.
pub(crate) fn poll(shell: &mut Shell, ctx: &egui::Context) {
    let events = shell.library.job.as_ref().map(|j| j.poll()).unwrap_or_default();
    for e in events {
        match e {
            CatalogEvent::Loaded { catalog, saved_copy, cached } => {
                shell.library.catalog = Some(catalog);
                shell.library.saved_copy = saved_copy;
                shell.library.cached = cached;
                for id in std::mem::take(&mut shell.library.wanted) {
                    add(shell, ctx, &id);
                }
            }
            CatalogEvent::Failed(e) => {
                shell.library.error = Some(e.to_string());
                shell.library.job = None;
                shell.library.wanted.clear();
            }
            CatalogEvent::Thumbnail { id, png } => match image::load_from_memory_with_format(&png, image::ImageFormat::Png) {
                Ok(img) => {
                    let img = img.to_rgba8();
                    let size = [img.width() as usize, img.height() as usize];
                    let tex = ctx.load_texture(
                        format!("library-{id}"),
                        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
                        egui::TextureOptions::LINEAR,
                    );
                    shell.library.thumbs.insert(id, tex);
                }
                Err(e) => log::warn!("data library: thumbnail of {id}: {e}"),
            },
        }
    }
    let t = ctx.input(|i| i.time);
    let mut finished = Vec::new();
    for (i, d) in shell.library.downloads.iter_mut().enumerate() {
        if let Some(r) = d.poll() {
            finished.push((i, d.id.clone(), r));
        }
    }
    let mut opened = false;
    for (i, id, r) in finished.into_iter().rev() {
        shell.library.downloads.remove(i);
        match r {
            Ok(path) => {
                shell.library.cached.insert(id);
                let w = wake(shell, ctx);
                shell.ws.open(vec![path], move || w());
                opened = true;
            }
            Err(LibraryError::Cancelled) => {}
            Err(e) => {
                shell.set_notice(e.to_string(), t);
                shell.library.errors.insert(id, e.to_string());
            }
        }
    }
    // The panel steps aside so the new layer is seen arriving.
    if opened && shell.library.downloads.is_empty() {
        shell.library.open = false;
    }
    if !shell.library.downloads.is_empty() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// "12.3 MB", "240 KB".
pub fn size_text(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e8 {
        format!("{:.0} MB", b / 1e6)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.0} KB", (b / 1e3).max(1.0))
    }
}

fn kind_icon(k: Kind) -> &'static str {
    match k {
        Kind::Points => icon::MAP_PIN,
        Kind::Lines => icon::LINE_SEGMENTS,
        Kind::Areas => icon::POLYGON,
        Kind::Raster => icon::GRID_FOUR,
        Kind::Table => icon::TABLE,
    }
}

/// What a card asks for.
enum CardAction {
    Add,
    Cancel,
}

/// The library window.
pub(crate) fn panel(shell: &mut Shell, ctx: &egui::Context) {
    if !shell.library.open {
        return;
    }
    let t = *shell.theme();
    let mut open = true;
    let mut actions: Vec<(String, CardAction)> = Vec::new();
    let mut retry = false;
    // Each card is CARD_W inside a 1 px border.
    let width = (CARD_W + 2.0) * COLUMNS as f32 + GAP * (COLUMNS as f32 - 1.0);
    egui::Window::new("Data library")
        .id(egui::Id::new("lm-library"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .default_width(width)
        .show(ctx, |ui| {
            let lib = &mut shell.library;
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::MAGNIFYING_GLASS).font(theme::icon_font(14.0)).color(t.text_secondary));
                ui.add(egui::TextEdit::singleline(&mut lib.query).hint_text("Search datasets").desired_width(240.0));
                if lib.saved_copy {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Offline: showing the list from your last visit").size(CAPTION).color(t.text_secondary));
                    });
                }
            });
            ui.add_space(theme::SPACE[2]);
            let Some(catalog) = &lib.catalog else {
                ui.allocate_ui(vec2(width, 200.0), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(64.0);
                        if let Some(e) = &lib.error {
                            ui.label(RichText::new(icon::WARNING_CIRCLE).font(theme::icon_font(24.0)).color(t.text_secondary));
                            ui.label(RichText::new(e).size(BODY));
                            ui.add_space(theme::SPACE[1]);
                            retry = ui.button("Try again").clicked();
                        } else {
                            ui.add(egui::Spinner::new().size(20.0).color(t.text_secondary));
                            ui.label(RichText::new("Loading the library…").size(BODY).color(t.text_secondary));
                        }
                    });
                });
                return;
            };
            let shown: Vec<&Dataset> = catalog.datasets.iter().filter(|d| d.matches(&lib.query)).collect();
            if shown.is_empty() {
                ui.allocate_ui(vec2(width, 120.0), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(48.0);
                        let text = if catalog.datasets.is_empty() {
                            "The library is empty.".to_string()
                        } else {
                            format!("No datasets match “{}”.", lib.query.trim())
                        };
                        ui.label(RichText::new(text).size(BODY).color(t.text_secondary));
                    });
                });
                return;
            }
            let max_h = (ctx.content_rect().height() - 200.0).clamp(280.0, 600.0);
            egui::ScrollArea::vertical().max_height(max_h).auto_shrink([true, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                for row in shown.chunks(COLUMNS) {
                    ui.horizontal_top(|ui| {
                        for d in row {
                            if let Some(a) = card(ui, &t, lib, d) {
                                actions.push((d.id.clone(), a));
                            }
                        }
                    });
                }
            });
        });
    if retry {
        load(shell, ctx);
    }
    for (id, a) in actions {
        match a {
            CardAction::Add => add(shell, ctx, &id),
            CardAction::Cancel => {
                if let Some(d) = shell.library.downloading(&id) {
                    d.cancel();
                }
            }
        }
    }
    if !open {
        shell.library.open = false;
    }
}

/// One dataset: thumbnail, title, size and licence, and Add to map (or the download's progress).
fn card(ui: &mut Ui, t: &theme::Theme, lib: &LibraryState, d: &Dataset) -> Option<CardAction> {
    let mut action = None;
    let frame = egui::Frame::new()
        .fill(t.surface_raised)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_CARD))
        .inner_margin(egui::Margin::ZERO);
    let resp = frame.show(ui, |ui| {
        ui.set_width(CARD_W);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            let (rect, _) = ui.allocate_exact_size(vec2(CARD_W, THUMB_H), Sense::hover());
            let top = CornerRadius { nw: RADIUS_CARD, ne: RADIUS_CARD, sw: 0, se: 0 };
            match lib.thumbs.get(&d.id) {
                Some(tex) => {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().add(egui::epaint::RectShape::filled(rect, top, Color32::WHITE).with_texture(tex.id(), uv));
                }
                None => {
                    ui.painter().rect_filled(rect, top, t.border);
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, kind_icon(d.kind), theme::icon_font(28.0), t.text_secondary);
                }
            }
            egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 10, top: 6, bottom: 10 }).show(ui, |ui| {
                ui.set_width(CARD_W - 20.0);
                ui.add(egui::Label::new(RichText::new(&d.title.en).font(semibold(BODY)).color(t.text_primary)).truncate());
                ui.add(egui::Label::new(RichText::new(&d.title.zh).font(regular(CAPTION)).color(t.text_secondary)).truncate());
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("{} · {} · {}", d.kind.label(), size_text(d.size()), d.license.name))
                            .font(regular(CAPTION))
                            .color(t.text_secondary),
                    )
                    .truncate(),
                );
                ui.add_space(4.0);
                match lib.downloading(&d.id) {
                    Some(dl) => {
                        ui.horizontal(|ui| {
                            ui.add(egui::ProgressBar::new(dl.fraction()).desired_width(CARD_W - 20.0 - 72.0).desired_height(6.0));
                            if ui.button("Cancel").clicked() {
                                action = Some(CardAction::Cancel);
                            }
                        });
                        ui.label(
                            RichText::new(format!("Downloading {} of {}", size_text(dl.done()), size_text(dl.total)))
                                .size(CAPTION)
                                .color(t.text_secondary),
                        );
                    }
                    None => {
                        ui.horizontal(|ui| {
                            let add = egui::Button::new(RichText::new("Add to map").color(t.on_accent)).fill(t.accent);
                            if ui.add(add).clicked() {
                                action = Some(CardAction::Add);
                            }
                            if lib.cached.contains(&d.id) {
                                ui.label(RichText::new(icon::CHECK_CIRCLE).font(theme::icon_font(14.0)).color(t.success))
                                    .on_hover_text("Downloaded: opens without the internet");
                                ui.label(RichText::new("Downloaded").size(CAPTION).color(t.text_secondary));
                            }
                        });
                    }
                }
                if let Some(e) = lib.errors.get(&d.id) {
                    ui.label(RichText::new(e).size(CAPTION).color(t.critical));
                }
            });
        });
    });
    resp.response.on_hover_ui(|ui| {
        ui.set_max_width(320.0);
        ui.label(&d.description.en);
        ui.label(RichText::new(&d.description.zh).color(t.text_secondary));
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} · {}", d.license.name, d.date)).size(CAPTION));
        ui.label(RichText::new(&d.citation).size(CAPTION).color(t.text_secondary));
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_plainly() {
        assert_eq!(size_text(5_000), "5 KB");
        assert_eq!(size_text(244_807), "245 KB");
        assert_eq!(size_text(12_345_678), "12.3 MB");
        assert_eq!(size_text(688_786_378), "689 MB");
        assert_eq!(size_text(1_600_000_000), "1.6 GB");
        assert_eq!(size_text(10), "1 KB");
    }

    #[test]
    fn the_catalog_url_comes_from_the_environment_then_the_build() {
        assert_eq!(catalog_url(Some(" https://h.cn/catalog.json "), false).as_deref(), Some("https://h.cn/catalog.json"));
        if library::DEFAULT_CATALOG_URL.is_none() {
            assert_eq!(catalog_url(None, false), None);
            assert_eq!(catalog_url(Some(""), false), None);
            let dev = catalog_url(None, true).unwrap_or_default();
            assert!(dev.starts_with("file://") && dev.ends_with("fixtures/library/catalog.json"), "{dev}");
            assert!(library::local_path(&dev).is_some_and(|p| p.is_file()));
        }
    }
}
