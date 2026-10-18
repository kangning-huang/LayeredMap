//! Export (WP5.3): the map image sheet (Interaction specification › Screen states: "Share → Map
//! image…: a full-window sheet showing the exact output · Save, or Cancel") and, in
//! `export_data`, the data export dialog.
//!
//! The image is drawn offscreen at its output size by `lm_mapview::export` on a background thread
//! (the renderer waits for basemap tiles there); the sheet's preview is the same picture composed
//! at a smaller scale, so the preview is what gets saved. Previews of actions are left out, and a
//! basemap whose terms forbid exported images is replaced by the offline one, in the preview too.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use egui::{Align2, Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use lm_core::basemap::{Provider, Source};
use lm_core::document::DocLayer;
use lm_core::model::{LayerId, LayerKind};
use lm_core::palette;
use lm_jobs::inspect::LegendItem;
use lm_mapview::export::{self as ex, Entry, LegendBlock, Swatch};
use lm_mapview::map::{Camera, Map};

use crate::theme::{self, BODY, CAPTION, Theme};
use crate::{FileRequest, Shell, form};

/// Export is Phase 5 work (the toolbar's Share button, File › Export Map Image… and Export
/// Data…).
pub const EXPORT_PHASE: u8 = 5;

/// Whether export is offered: once the build reaches Phase 5 (unless `LM_PHASE` rolls it back),
/// and ahead of that in developer and Pro builds, so it can be tried while Phase 5 is built.
pub fn shown(flags: lm_core::phase::Flags, developer: bool, pro: bool) -> bool {
    (flags.shows(EXPORT_PHASE) && EXPORT_PHASE <= lm_core::phase::BUILD_PHASE) || developer || pro
}

/// The image sizes offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// 1920 × 1080: a 16:9 slide.
    Slide,
    /// A4 portrait at 300 dpi: 2480 × 3508.
    A4,
    /// 1080 × 1080.
    Square,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Slide, Preset::A4, Preset::Square];

    /// The output's size in pixels.
    pub fn pixels(self) -> (u32, u32) {
        match self {
            Preset::Slide => (1920, 1080),
            Preset::A4 => (2480, 3508),
            Preset::Square => (1080, 1080),
        }
    }

    /// Pixels per point. Slides and squares are laid out at 1280 × 720 and 720 × 720 points, as on
    /// a laptop screen; A4 at 96 points per inch (794 × 1123), printed at 300 dpi.
    pub fn ppp(self) -> f32 {
        match self {
            Preset::Slide | Preset::Square => 1.5,
            Preset::A4 => 300.0 / 96.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Preset::Slide => "Slide 16:9",
            Preset::A4 => "Report A4",
            Preset::Square => "Square",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Preset::Slide => "1920 × 1080 px",
            Preset::A4 => "2480 × 3508 px · A4 at 300 dpi",
            Preset::Square => "1080 × 1080 px",
        }
    }

    fn file_suffix(self) -> &'static str {
        match self {
            Preset::Slide => "slide",
            Preset::A4 => "A4",
            Preset::Square => "square",
        }
    }
}

/// The app's fonts for the exported image.
pub fn fonts() -> ex::Fonts {
    ex::Fonts {
        regular: include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
        semibold: include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"),
        fallback: theme::NOTO_SC_REGULAR,
    }
}

/// The basemap an export uses: the one shown, unless its terms forbid exported images (then the
/// offline map). The second value is the provider replaced, if any.
pub fn export_basemap(shown: &Provider) -> (Provider, Option<Provider>) {
    if shown.export_allowed { (shown.clone(), None) } else { (lm_core::basemap::provider("offline"), Some(shown.clone())) }
}

/// The map drawn at full size, and its copy at the preview's scale.
pub struct Drawn {
    key: String,
    full: ex::Pixmap,
    small: ex::Pixmap,
    /// The preview's scale (preview pixels per output pixel).
    k: f32,
    occupancy: ex::Occupancy,
    attribution: Vec<String>,
    tiles_missing: bool,
}

/// The map image sheet.
pub struct ImageSheet {
    pub preset: Preset,
    pub title: String,
    pub caption: String,
    /// The caption was typed over; otherwise it follows the layers.
    pub caption_edited: bool,
    /// The visible layers, top first, and whether each is in the legend.
    pub legend: Vec<(LayerId, bool)>,
    /// Where the legend was dragged (points); `None` places it.
    pub legend_at: Option<(f32, f32)>,
    /// What the user saw: the camera and the size (points) of the part of the map not under
    /// panels, with that part's centre.
    seen: (Camera, (f32, f32)),
    /// The map to draw: previews left out, the export basemap applied.
    map: Arc<Map>,
    pub basemap: Provider,
    /// The basemap replaced because its terms forbid export.
    pub replaced: Option<Provider>,
    pub preview_excluded: bool,
    job: Option<(String, Receiver<Option<Drawn>>)>,
    drawn: Option<Drawn>,
    texture: Option<(String, egui::TextureHandle, ex::Layout)>,
    /// The longest side of the preview, in pixels.
    preview_px: f32,
}

impl ImageSheet {
    /// A sheet for what the shell shows now.
    pub fn new(shell: &Shell) -> ImageSheet {
        let (basemap, replaced) = export_basemap(&lm_core::basemap::provider(&shell.prefs.basemap));
        let mut map = crate::workspace::map_for_export(shell.map_view().map());
        if replaced.is_some() || basemap.id != shell.prefs.basemap {
            let prefs = crate::Prefs { basemap: basemap.id.to_string(), ..shell.prefs.clone() };
            crate::basemap::apply(&mut map, &prefs, shell.theme().dark);
        }
        // The part of the map the panels leave free, and its centre.
        let size = shell.last_map_size;
        let (left, right) = shell.panel_insets();
        let (left, right) = if size.x - left - right >= 240.0 { (left, right) } else { (0.0, 0.0) };
        let free = (size.x - left - right, size.y);
        let cam = shell.map_view().camera();
        let center = cam.to_map(f64::from(left + free.0 / 2.0), f64::from(size.y / 2.0), f64::from(size.x), f64::from(size.y));
        let legend = shell.ws.doc.layers.iter().rev().filter(|l| l.visible).map(|l| (l.id, true)).collect();
        let mut s = ImageSheet {
            preset: Preset::Slide,
            title: String::new(),
            caption: String::new(),
            caption_edited: false,
            legend,
            legend_at: None,
            seen: (Camera { center, res: cam.res }, free),
            map: Arc::new(map),
            basemap,
            replaced,
            preview_excluded: shell.ws.has_preview() || shell.ws.compare.is_some(),
            job: None,
            drawn: None,
            texture: None,
            preview_px: 1400.0,
        };
        s.caption = auto_caption(shell);
        s
    }

    fn has_title(&self) -> bool {
        !self.title.trim().is_empty()
    }

    /// The map's area on the sheet (points) and the camera that shows what the user saw in it.
    fn map_area(&self) -> (ex::Area, Camera) {
        let (w, h) = self.preset.pixels();
        let ppp = self.preset.ppp();
        let area = ex::map_area((w as f32 / ppp, h as f32 / ppp), self.has_title());
        let (cam, seen) = self.seen;
        (area, ex::fit_camera(cam.center, cam.res, seen, (area.w, area.h)))
    }

    /// What the map was (or is being) drawn for.
    fn draw_key(&self) -> String {
        format!("{:?}|{}", self.preset, self.has_title())
    }

    /// The credits: the basemap's, the tile layers' and Natural Earth (always drawn beneath).
    pub fn attribution(&self) -> String {
        let mut credits: Vec<String> = Vec::new();
        let mut add = |c: &str| {
            let c = c.trim();
            if !c.is_empty() && !credits.iter().any(|x| x == c) {
                credits.push(c.to_string());
            }
        };
        if let Some(d) = &self.drawn {
            d.attribution.iter().for_each(|c| add(c));
        }
        add(self.basemap.attribution);
        // Satellite imagery carries OpenStreetMap place names and boundaries.
        if self.basemap.source == Source::Imagery {
            add(lm_core::basemap::provider("light").attribution);
        }
        add("Natural Earth");
        credits.join(" · ")
    }

    /// Starts drawing the map at full size when what it is drawn for changed.
    fn ensure_drawn(&mut self, ctx: &egui::Context) {
        let key = self.draw_key();
        if self.drawn.as_ref().is_some_and(|d| d.key == key) || self.job.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let (area, camera) = self.map_area();
        let ppp = self.preset.ppp();
        let pixels = ex::map_pixels(area, ppp);
        let (w, h) = self.preset.pixels();
        let k = (self.preview_px / w.max(h) as f32).min(1.0);
        let small = (((pixels.0 as f32) * k).round().max(1.0) as u32, ((pixels.1 as f32) * k).round().max(1.0) as u32);
        let map = self.map.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let waker = ctx.clone();
        let job_key = key.clone();
        let spawned = std::thread::Builder::new().name("lm-export".into()).spawn(move || {
            let out = ex::render_map((*map).clone(), camera, pixels, ppp).and_then(|img| {
                let small = ex::downsample(&img.pixmap, small.0, small.1)?;
                Some(Drawn {
                    key: job_key,
                    full: img.pixmap,
                    small,
                    k,
                    occupancy: img.occupancy,
                    attribution: img.attribution,
                    tiles_missing: img.tiles_missing,
                })
            });
            let _ = tx.send(out);
            waker.request_repaint();
        });
        match spawned {
            Ok(_) => self.job = Some((key, rx)),
            Err(e) => log::error!("could not start drawing the export: {e}"),
        }
    }

    fn poll(&mut self) {
        let Some((_, rx)) = &self.job else { return };
        match rx.try_recv() {
            Ok(d) => {
                if let Some(d) = d {
                    self.drawn = Some(d);
                    self.texture = None;
                }
                self.job = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.job = None,
        }
    }

    /// Whether the map is still being drawn for the current settings.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.drawn.as_ref().is_none_or(|d| d.key != self.draw_key())
    }

    /// The sheet at full size, with the legend generated from the shell's layers.
    pub fn sheet(&self, shell: &Shell) -> ex::Sheet {
        let (w, h) = self.preset.pixels();
        let (_, camera) = self.map_area();
        let lat = rustgis_lat(camera);
        let m_per_pt = camera.res * lat.to_radians().cos();
        let (len, label) = crate::chrome::scale(m_per_pt, shell.prefs.units, 110.0);
        let (_, land, _, _) = self.map.basemap.palette();
        let on: Vec<LayerId> = self.legend.iter().filter(|(_, on)| *on).map(|(id, _)| *id).collect();
        ex::Sheet {
            width: w,
            height: h,
            ppp: self.preset.ppp(),
            title: self.title.trim().to_string(),
            caption: self.caption.trim().to_string(),
            attribution: self.attribution(),
            legend: legend_blocks(shell, &on),
            legend_at: self.legend_at,
            scale: (len.is_finite() && len > 0.0).then_some((len as f32, label)),
            ground: land.0,
        }
    }

    /// The finished image as PNG bytes (`None` while the map is still being drawn).
    pub fn image(&self, shell: &Shell) -> Option<(ex::Pixmap, ex::Layout)> {
        let d = self.drawn.as_ref().filter(|d| d.key == self.draw_key())?;
        ex::compose(&d.full, &self.sheet(shell), &fonts(), Some(&d.occupancy))
    }

    /// What the reader should know before saving (Export warnings).
    pub fn warnings(&self, shell: &Shell) -> Vec<String> {
        let mut out = Vec::new();
        for (id, on) in &self.legend {
            let Some(l) = shell.ws.doc.layer(*id) else { continue };
            let scale = l.style.color_by.is_some() || l.style.size_by.is_some() || l.profile.kind == LayerKind::Raster;
            if scale && !on {
                out.push(format!("{} is colored by value but has no legend.", l.display_name));
            }
        }
        if self.preview_excluded {
            out.push("The preview isn't included. Keep it to put it in the image.".into());
        }
        for (id, _) in &self.legend {
            let Some(l) = shell.ws.doc.layer(*id) else { continue };
            if l.filter.is_none() {
                continue;
            }
            if let Some(d) = shell.ws.derived(*id) {
                let total = l.profile.count;
                let (_, many) = l.profile.kind.nouns();
                out.push(format!(
                    "{} is filtered: the image shows {} of {} {many}.",
                    l.display_name,
                    lm_core::model::group_thousands(d.shown as u64),
                    lm_core::model::group_thousands(total)
                ));
            }
        }
        if let Some(r) = &self.replaced {
            out.push(format!("The {} basemap's terms don't allow exported images, so the offline basemap is used.", r.name));
        }
        if !self.basemap.export_terms.is_empty() {
            out.push(self.basemap.export_terms.to_string());
        }
        if self.drawn.as_ref().is_some_and(|d| d.tiles_missing) {
            out.push("Some of the background couldn't be loaded, so the image shows less detail.".into());
        }
        out
    }
}

/// The latitude of a camera's centre.
fn rustgis_lat(c: Camera) -> f64 {
    lm_mapview::camera::center_lonlat(&c).y
}

/// "Data: Districts (districts.geojson); Heat (lst_july.tif). Made with LayeredMap."
pub fn auto_caption(shell: &Shell) -> String {
    let parts: Vec<String> = shell
        .ws
        .doc
        .layers
        .iter()
        .rev()
        .filter(|l| l.visible)
        .map(|l| {
            let file = std::path::Path::new(&l.profile.source.path).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            if file.is_empty() || file == l.display_name { l.display_name.clone() } else { format!("{} ({file})", l.display_name) }
        })
        .collect();
    if parts.is_empty() { "Made with LayeredMap.".into() } else { format!("Data: {}. Made with LayeredMap.", parts.join("; ")) }
}

/// A legend swatch for a layer's colour as the map draws it: areas translucent with a darker
/// outline, points with their edge, lines at their width.
fn swatch_for(l: &DocLayer, color: [u8; 4], stroke: Option<[u8; 4]>, diameter: Option<f32>) -> Swatch {
    let s = &l.style;
    let alpha = |a: u8, k: f32| (f32::from(a) * k.clamp(0.0, 1.0)).round() as u8;
    let o = s.opacity;
    match l.profile.kind {
        LayerKind::Areas => {
            let mut fill = color;
            fill[3] = alpha(fill[3], s.fill_opacity * o);
            let mut edge = stroke.unwrap_or(s.stroke);
            edge[3] = alpha(edge[3], o);
            Swatch::Area { fill, stroke: edge }
        }
        LayerKind::Lines => {
            let mut c = color;
            c[3] = alpha(c[3], o);
            Swatch::Line { color: c, width: s.line_width }
        }
        _ => {
            let mut fill = color;
            fill[3] = alpha(fill[3], o);
            let mut edge = s.stroke;
            edge[3] = alpha(edge[3], o);
            Swatch::Point { fill, stroke: edge, diameter: diameter.unwrap_or(s.point_size) }
        }
    }
}

fn ramp_stops(name: &str) -> Vec<[u8; 4]> {
    (0..48).map(|k| palette::sample(name, (f64::from(k) + 0.5) / 48.0)).collect()
}

/// The legend for the layers `ids` (top first), generated from their styles as the inspector's
/// legend is. Layers drawn in one colour share an untitled block.
pub fn legend_blocks(shell: &Shell, ids: &[LayerId]) -> Vec<LegendBlock> {
    let mut out: Vec<LegendBlock> = Vec::new();
    for id in ids {
        let Some(l) = shell.ws.doc.layer(*id) else { continue };
        if l.profile.kind == LayerKind::Raster {
            let band = l.profile.bands.get((l.style.band.max(1) - 1) as usize).or(l.profile.bands.first());
            let unit = band.and_then(|b| b.unit.value.clone()).map(|u| format!(" {u}")).unwrap_or_default();
            let (lo, hi) = shell.raster_range(*id).or(l.style.stretch).unwrap_or((0.0, 1.0));
            let n = |v: f64| format!("{}{unit}", lm_jobs::inspect::number_text(v));
            out.push(LegendBlock {
                title: l.display_name.clone(),
                entries: vec![
                    Entry::Ramp { stops: ramp_stops(&l.style.ramp), low: n(lo), high: n(hi) },
                    Entry::Item { swatch: Swatch::NoData, label: "No data".into() },
                ],
            });
            continue;
        }
        let classes =
            shell.ws.derived(*id).and_then(|d| d.classes.clone()).filter(|_| l.style.color_by.is_some() || l.style.size_by.is_some());
        let Some(c) = classes else {
            let item = Entry::Item { swatch: swatch_for(l, l.style.key_color(l.profile.kind), None, None), label: l.display_name.clone() };
            match out.last_mut() {
                Some(b) if b.title.is_empty() => b.entries.push(item),
                _ => out.push(LegendBlock { title: String::new(), entries: vec![item] }),
            }
            continue;
        };
        let title = match &c.legend.title {
            Some(t) => format!("{} · {t}", l.display_name),
            None => l.display_name.clone(),
        };
        let entries = c
            .legend
            .items
            .iter()
            .map(|item| match item {
                LegendItem::Swatch { color, label } => {
                    Entry::Item { swatch: swatch_for(l, *color, Some(lm_core::document::darker(*color)), None), label: label.clone() }
                }
                LegendItem::Ramp { palette: p, low, high } => Entry::Ramp { stops: ramp_stops(p), low: low.clone(), high: high.clone() },
                LegendItem::Size { diameter, label } => {
                    Entry::Item { swatch: swatch_for(l, l.style.fill, None, Some(*diameter)), label: label.clone() }
                }
                LegendItem::NoData { count } => {
                    Entry::Item { swatch: Swatch::NoData, label: format!("No data ({})", lm_core::model::group_thousands(*count)) }
                }
            })
            .collect();
        out.push(LegendBlock { title, entries });
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The sheet

/// The width of the sheet's settings column.
const SIDEBAR_W: f32 = 340.0;

/// Draws the full-window sheet; Cancel, Esc or Save closes it.
pub fn sheet(shell: &mut Shell, ui: &mut Ui, full: Rect) {
    let Some(mut s) = shell.export_sheet.take() else { return };
    let t = *shell.theme();
    let ctx = ui.ctx().clone();
    s.poll();
    if !s.caption_edited {
        s.caption = auto_caption(shell);
    }
    s.preview_px = (1000.0 * ctx.pixels_per_point()).clamp(600.0, 2000.0);
    s.ensure_drawn(&ctx);
    let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut save = false;

    // Everything else is covered: the sheet takes the whole window.
    let p = ui.painter_at(full);
    p.rect_filled(full, 0.0, t.surface_raised);
    ui.allocate_rect(full, Sense::click_and_drag());
    let side = Rect::from_min_max(pos2(full.right() - SIDEBAR_W, full.top()), full.max);
    let canvas = Rect::from_min_max(full.min, pos2(side.left(), full.bottom()));
    p.rect_filled(side, 0.0, t.surface);
    p.vline(side.left(), side.y_range(), Stroke::new(1.0, t.border));

    preview(shell, &mut s, ui, canvas, &t);

    let mut side_ui = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink2(vec2(theme::PANEL_PADDING + 4.0, 20.0))));
    let ui = &mut side_ui;
    ui.label(RichText::new("Export map image").font(theme::semibold(theme::TITLE)));
    ui.add_space(12.0);
    let buttons_h = 44.0;
    let body_h = ui.available_height() - buttons_h;
    egui::ScrollArea::vertical().max_height(body_h).auto_shrink([false, true]).show(ui, |ui| {
        form::heading(ui, "Size");
        let sel = Preset::ALL.iter().position(|p| *p == s.preset).unwrap_or(0);
        let labels: Vec<&str> = Preset::ALL.iter().map(|p| p.label()).collect();
        if let Some(k) = form::segmented(ui, &t, sel, &labels, ui.available_width())
            && let Some(p) = Preset::ALL.get(k)
        {
            s.preset = *p;
            s.legend_at = None;
        }
        ui.label(RichText::new(format!("{} · PNG", s.preset.detail())).size(CAPTION).color(t.text_secondary));
        form::divider(ui, &t);
        form::heading(ui, "Title");
        ui.add(egui::TextEdit::singleline(&mut s.title).hint_text("Add a title (optional)").desired_width(f32::INFINITY));
        ui.add_space(8.0);
        form::heading(ui, "Caption");
        let before = s.caption.clone();
        ui.add(egui::TextEdit::multiline(&mut s.caption).desired_rows(3).desired_width(f32::INFINITY));
        if s.caption != before {
            s.caption_edited = true;
        }
        if s.caption_edited {
            if ui.link("Use the caption made from the layers").clicked() {
                s.caption_edited = false;
                s.caption = auto_caption(shell);
            }
        } else {
            ui.label(RichText::new("Made from the layers; edit it if you like.").size(CAPTION).color(t.text_secondary));
        }
        form::divider(ui, &t);
        form::heading(ui, "Legend");
        if s.legend.is_empty() {
            ui.label(RichText::new("No layers are shown.").color(t.text_secondary));
        }
        for (id, on) in s.legend.iter_mut() {
            let name = shell.ws.doc.layer(*id).map_or_else(String::new, |l| l.display_name.clone());
            form::checkbox(ui, &t, on, &name);
        }
        let hint = if s.legend_at.is_some() { None } else { Some("It places itself clear of the data; drag it to move it.") };
        match hint {
            Some(h) => {
                ui.label(RichText::new(h).size(CAPTION).color(t.text_secondary));
            }
            None => {
                if ui.link("Place the legend automatically").clicked() {
                    s.legend_at = None;
                }
            }
        }
        form::divider(ui, &t);
        let warnings = s.warnings(shell);
        for w in &warnings {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(icon::WARNING).font(theme::icon_font(16.0)).color(t.warning));
                ui.add(egui::Label::new(RichText::new(w).color(t.text_primary)).wrap());
            });
            ui.add_space(4.0);
        }
        ui.horizontal_top(|ui| {
            ui.label(RichText::new(icon::INFO).font(theme::icon_font(16.0)).color(t.text_secondary));
            ui.add(
                egui::Label::new(
                    RichText::new(format!("Credits are always included: {}", s.attribution())).size(CAPTION).color(t.text_secondary),
                )
                .wrap(),
            );
        });
    });
    // Cancel and Save at the bottom right, as a macOS sheet has them.
    let row = Rect::from_min_max(
        pos2(side.left() + theme::PANEL_PADDING, side.bottom() - buttons_h - 12.0),
        pos2(side.right() - theme::PANEL_PADDING, side.bottom() - 12.0),
    );
    let mut row_ui = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
    let ready = !s.busy();
    let save_btn = egui::Button::new(RichText::new("Save…").color(t.on_accent)).fill(t.accent).min_size(vec2(88.0, 30.0));
    if row_ui.add_enabled(ready, save_btn).on_disabled_hover_text("Drawing the map at full size…").clicked() {
        save = true;
    }
    if row_ui.add(egui::Button::new("Cancel").min_size(vec2(80.0, 30.0))).clicked() {
        close = true;
    }
    if save {
        match s.image(shell).map(|(pm, _)| ex::encode_png(&pm)) {
            Some(Ok(bytes)) => {
                let name = if s.title.trim().is_empty() { "Map".to_string() } else { s.title.trim().replace(['/', ':', '\\'], "-") };
                shell.push_file_request(FileRequest::SaveBytes {
                    title: "Export map image".into(),
                    suggested_name: format!("{name} ({}).png", s.preset.file_suffix()),
                    bytes,
                });
                close = true;
            }
            Some(Err(e)) => shell.set_notice(format!("The image couldn't be made: {e}"), ctx.input(|i| i.time)),
            None => {}
        }
    }
    if s.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    if !close {
        shell.export_sheet = Some(s);
    }
}

/// The preview: the sheet drawn at a smaller scale, fitted into `canvas`; the legend can be
/// dragged on it.
fn preview(shell: &Shell, s: &mut ImageSheet, ui: &mut Ui, canvas: Rect, t: &Theme) {
    let (w, h) = s.preset.pixels();
    let area = canvas.shrink(40.0);
    let scale = (area.width() / w as f32).min((area.height() - 24.0) / h as f32);
    let size = vec2(w as f32 * scale, h as f32 * scale);
    let img = Rect::from_center_size(pos2(area.center().x, area.center().y - 12.0), size);
    let p = ui.painter_at(canvas);
    p.rect_filled(img.expand(1.0), CornerRadius::ZERO, t.border);
    let full_sheet = s.sheet(shell);
    match s.drawn.as_ref().filter(|d| d.key == s.draw_key()) {
        Some(d) => {
            let small = full_sheet.at_scale(d.k);
            let key = format!("{small:?}");
            if s.texture.as_ref().is_none_or(|(k, _, _)| *k != key)
                && let Some((pm, lay)) = ex::compose(&d.small, &small, &fonts(), Some(&d.occupancy))
            {
                let tex = ui.ctx().load_texture("lm-export-preview", ex::color_image(&pm), egui::TextureOptions::LINEAR);
                s.texture = Some((key, tex, lay));
            }
        }
        None => {
            p.rect_filled(img, CornerRadius::ZERO, Color32::WHITE);
        }
    }
    if let Some((_, tex, lay)) = &s.texture {
        p.image(tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        // Dragging the legend: preview points → sheet points.
        if let Some(a) = lay.legend {
            let k = img.width() / (w as f32 / s.preset.ppp());
            let r = Rect::from_min_size(img.min + vec2(a.x * k, a.y * k), vec2(a.w * k, a.h * k));
            let resp = ui.interact(r, egui::Id::new("lm-export-legend"), Sense::drag()).on_hover_cursor(egui::CursorIcon::Grab);
            if resp.dragged() {
                let d = resp.drag_delta() / k;
                s.legend_at = Some((a.x + d.x, a.y + d.y));
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            if resp.hovered() || resp.dragged() {
                p.rect_stroke(r.expand(2.0), CornerRadius::same(theme::RADIUS_CARD), Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
            }
        }
    }
    let busy = s.busy();
    if busy {
        let pill = Rect::from_center_size(img.center(), vec2(240.0, 36.0));
        p.rect_filled(pill, CornerRadius::same(18), t.surface.gamma_multiply(0.95));
        p.text(pill.center(), Align2::CENTER_CENTER, "Drawing the map at full size…", theme::regular(BODY), t.text_primary);
    }
    let note = format!("{} · exactly as saved", s.preset.detail());
    p.text(pos2(img.center().x, img.bottom() + 16.0), Align2::CENTER_TOP, note, theme::regular(CAPTION), t.text_secondary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_their_exact_sizes() {
        assert_eq!(Preset::Slide.pixels(), (1920, 1080));
        assert_eq!(Preset::A4.pixels(), (2480, 3508));
        assert_eq!(Preset::Square.pixels(), (1080, 1080));
        // A4 at 300 dpi: 210 × 297 mm.
        let (w, h) = Preset::A4.pixels();
        assert!((f64::from(w) / 300.0 * 25.4 - 210.0).abs() < 0.5 && (f64::from(h) / 300.0 * 25.4 - 297.0).abs() < 0.5);
    }

    #[test]
    fn a_basemap_whose_terms_forbid_export_is_replaced_by_the_offline_one() {
        let shown = Provider { export_allowed: false, ..lm_core::basemap::provider("light") };
        let (used, replaced) = export_basemap(&shown);
        assert_eq!(used.id, "offline");
        assert_eq!(replaced.map(|p| p.id), Some("light"));
        for p in lm_core::basemap::providers() {
            let (used, replaced) = export_basemap(&p);
            assert_eq!(used.id, p.id, "today's providers all allow export");
            assert!(replaced.is_none());
        }
    }

    #[test]
    fn export_shows_in_developer_and_pro_builds_before_its_phase() {
        let f = lm_core::phase::Flags::default();
        assert_eq!(shown(f, false, false), EXPORT_PHASE <= lm_core::phase::BUILD_PHASE);
        assert!(shown(f, true, false) && shown(f, false, true));
    }
}
