//! Phase 3 inspector parts: the object card (Inspector: object card), the Style tab (WP3.5) and
//! the legend generated from the style. Everything numeric comes from `lm_jobs::inspect`.

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, vec2};

use lm_core::document::{Classes, ColorBy, DocLayer, LabelStyle, SizeBy, Style};
use lm_core::model::{FieldRole, LayerId, LayerKind};
use lm_core::palette;
use lm_jobs::import::LayerData;
use lm_jobs::inspect::{self as data, LegendItem};

use crate::theme::{self, CAPTION, Theme};
use crate::{Shell, form};

pub(crate) fn color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// The dataset behind a vector layer, when loaded.
pub(crate) fn dataset(shell: &Shell, id: LayerId) -> Option<std::sync::Arc<lm_jobs::import::Dataset>> {
    match &shell.ws.data(id)?.data {
        LayerData::Vector(ds) => Some(ds.clone()),
        LayerData::Raster(_) => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Object card

/// The header of an object card: title and subtitle ("Districts · area · 23.4 km²").
pub fn object_header(shell: &Shell, l: &DocLayer, row: usize) -> (String, String) {
    let Some(ds) = dataset(shell, l.id) else { return (l.display_name.clone(), String::new()) };
    let title = data::object_title(l, &ds, row);
    let (noun, _) = l.profile.kind.nouns();
    let mut sub = format!("{} · {noun}", l.display_name);
    if let Some(m) = data::measure_text(&ds, row, shell.prefs.units == crate::Units::Imperial) {
        sub.push_str(" · ");
        sub.push_str(&m);
    }
    (title, sub)
}

/// The body of an object card: insight strip, facts, suggested actions, footer.
pub fn object_body(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer, row: usize) {
    let Some(ds) = dataset(shell, l.id) else {
        ui.label(RichText::new("The data is still loading.").color(t.text_secondary));
        return;
    };
    let shown = crate::insight::strip(shell, ui, t, l, &ds, row);
    // Facts: up to six, then Show all. Fields already in the insight strip aren't repeated.
    let key = egui::Id::new(("lm-all-facts", l.id.0));
    let all = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    let (mut facts, total) = data::facts(l, &ds, row, 6 + shown.len(), all);
    if !all {
        facts.retain(|f| !shown.contains(&f.field));
        facts.truncate(6);
    }
    ui.add_space(8.0);
    egui::Grid::new(("lm-facts", l.id.0, row)).num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        for f in &facts {
            ui.label(RichText::new(&f.field).color(t.text_secondary));
            match &f.value {
                Some(v) => {
                    let unit = f.unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
                    ui.add(egui::Label::new(format!("{v}{unit}")).wrap());
                }
                None => {
                    ui.label(RichText::new("No value").color(t.text_secondary));
                }
            }
            ui.end_row();
        }
    });
    if !all && total > facts.len() && ui.link(format!("Show all {total} fields")).clicked() {
        ui.data_mut(|d| d.insert_temp(key, true));
    } else if all && ui.link("Show fewer fields").clicked() {
        ui.data_mut(|d| d.insert_temp(key, false));
    }
    crate::insight::actions(shell, ui, t, l, Some(row));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.link("View in table").clicked() {
            shell.table_for = Some(l.id);
        }
        ui.label(RichText::new("·").color(t.text_secondary));
        if ui.link("Zoom to").clicked()
            && let Some(e) = data::row_extent(&ds, row)
        {
            shell.zoom_to = Some(e);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Legend

/// The legend for a layer's current style (generated, never typed).
pub fn legend(shell: &Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let derived = shell.ws.derived(l.id).and_then(|d| d.classes.clone());
    if l.profile.kind == LayerKind::Raster {
        raster_legend(shell, ui, t, l);
        return;
    }
    let Some(c) = derived.filter(|_| l.style.color_by.is_some() || l.style.size_by.is_some()) else {
        ui.horizontal(|ui| {
            swatch(ui, l.profile.kind, l.style.key_color(l.profile.kind), 7.0);
            let count = shell.ws.derived(l.id).map_or(l.profile.count, |d| d.shown as u64);
            let (one, many) = l.profile.kind.nouns();
            ui.label(format!("{} {}", lm_core::model::group_thousands(count), if count == 1 { one } else { many }));
        });
        return;
    };
    if let Some(title) = &c.legend.title {
        ui.label(RichText::new(title).size(CAPTION).color(t.text_secondary));
    }
    for item in &c.legend.items {
        match item {
            LegendItem::Swatch { color: col, label } => {
                ui.horizontal(|ui| {
                    swatch(ui, l.profile.kind, *col, 7.0);
                    ui.label(label);
                });
            }
            LegendItem::Ramp { palette: p, low, high } => {
                ramp(ui, p, 200.0);
                ui.horizontal(|ui| {
                    ui.set_width(200.0);
                    ui.label(RichText::new(low).size(CAPTION));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(high).size(CAPTION));
                    });
                });
            }
            LegendItem::Size { diameter, label } => {
                ui.horizontal(|ui| {
                    let d = *diameter;
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, d.max(10.0) + 2.0), Sense::hover());
                    ui.painter().circle(r.center(), d / 2.0, color(l.style.fill), Stroke::new(1.0, Color32::WHITE));
                    ui.label(label);
                });
            }
            LegendItem::NoData { count } => {
                ui.horizontal(|ui| {
                    no_data_swatch(ui);
                    ui.label(format!("No data ({})", lm_core::model::group_thousands(*count)));
                });
            }
        }
    }
    if c.legend.fitted_to_view {
        ui.label(RichText::new("◉ Range fitted to the view").size(CAPTION).color(t.accent));
    }
}

fn raster_legend(shell: &Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let band = l.profile.bands.get((l.style.band.max(1) - 1) as usize).or(l.profile.bands.first());
    let unit = band.and_then(|b| b.unit.value.clone());
    let (lo, hi) = shell.raster_range(l.id).or(l.style.stretch).unwrap_or((0.0, 1.0));
    if let Some(g) = &l.profile.grid {
        // A grid names its variable, level and date (WP6.12).
        ui.label(RichText::new(g.caption(shell.ws.shown_time(l.id))).size(CAPTION).color(t.text_secondary));
    } else if let Some(b) = band {
        let u = unit.as_deref().map(|u| format!(" ({u})")).unwrap_or_default();
        ui.label(RichText::new(format!("Band {}{u}", b.index)).size(CAPTION).color(t.text_secondary));
    }
    ramp(ui, &l.style.ramp, 200.0);
    ui.horizontal(|ui| {
        ui.set_width(200.0);
        ui.label(RichText::new(data::number_text(lo)).size(CAPTION));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(data::number_text(hi)).size(CAPTION));
        });
    });
    if let Some(a) = l.style.above {
        ui.label(RichText::new(format!("Showing only values above {}", data::number_text(a))).size(CAPTION).color(t.accent));
    }
    if let Some((a, b)) = l.style.like {
        ui.label(RichText::new(format!("Highlighting {} to {}", data::number_text(a), data::number_text(b))).size(CAPTION).color(t.accent));
    }
    ui.horizontal(|ui| {
        no_data_swatch(ui);
        ui.label("No data");
    });
}

pub(crate) fn swatch(ui: &mut Ui, kind: LayerKind, c: [u8; 4], size: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let col = color(c);
    match kind {
        LayerKind::Points | LayerKind::Table => {
            ui.painter().circle(r.center(), size / 2.0, col, Stroke::new(1.0, Color32::WHITE));
        }
        LayerKind::Lines => {
            ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(2.5, col));
        }
        _ => {
            ui.painter().rect(
                r.shrink(1.0),
                CornerRadius::same(3),
                col.gamma_multiply(0.6),
                Stroke::new(1.0, col),
                egui::StrokeKind::Inside,
            );
        }
    }
}

fn no_data_swatch(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let p = ui.painter();
    let r = r.shrink(1.0);
    p.rect_filled(r, CornerRadius::same(3), color(palette::NO_DATA));
    // The diagonal hatch.
    let hatch = Stroke::new(1.0, Color32::from_gray(0x9A));
    let clip = p.with_clip_rect(r);
    let mut x = r.left() - r.height();
    while x < r.right() {
        clip.line_segment([egui::pos2(x, r.bottom()), egui::pos2(x + r.height(), r.top())], hatch);
        x += 4.0;
    }
}

/// A palette drawn left (low) to right (high).
pub(crate) fn ramp(ui: &mut Ui, name: &str, width: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(width, 12.0), Sense::hover());
    let steps = 64;
    let w = r.width() / steps as f32;
    for k in 0..steps {
        let c = palette::sample(name, (k as f64 + 0.5) / steps as f64);
        let x = r.left() + k as f32 * w;
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, r.top()), vec2(w + 0.5, r.height())), 0.0, color(c));
    }
}

// ---------------------------------------------------------------------------------------------
// Style tab

/// A style change from the Style tab: one undo step ("Change style").
fn set_style(shell: &mut Shell, id: LayerId, s: Style) {
    let tx = shell.ws.doc.set_style_tx(id, s);
    shell.ws.commit(tx);
}

fn fields_with(l: &DocLayer, pred: impl Fn(FieldRole) -> bool) -> Vec<String> {
    l.profile.fields.iter().filter(|f| pred(l.role(f).value)).map(|f| f.name.clone()).collect()
}

pub fn style_tab(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let t = *t;
    form::with_visible_controls(ui, &t, |ui| style_form(shell, ui, &t, l));
}

/// The Style tab as an inspector form: presets, then Color, Size and Labels, then Appearance,
/// then the legend the style produces. Labels sit in one column, controls beside them.
fn style_form(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let id = l.id;
    if l.profile.kind == LayerKind::Raster {
        raster_style(shell, ui, t, l);
        return;
    }
    // Presets: thumbnails drawn from the layer's own style and fields.
    ui.add_space(4.0);
    let presets = data::presets(l);
    let gap = 8.0;
    let tile_w = ((ui.available_width() - 3.0 * gap) / 4.0).floor();
    for chunk in presets.chunks(4) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (name, s) in chunk {
                let on = s.color_by == l.style.color_by && s.size_by == l.style.size_by && s.labels == l.style.labels;
                if preset_thumb(ui, t, l, s, name, on, tile_w).clicked() && !on {
                    set_style(shell, id, s.clone());
                }
            }
        });
        ui.add_space(4.0);
    }
    form::divider(ui, t);
    let measures = fields_with(l, |r| matches!(r, FieldRole::Measure(_)));
    let categories = fields_with(l, |r| r == FieldRole::Category);
    let mut s = l.style.clone();
    // Color.
    let current = s.color_by.as_ref().map(|c| c.field.clone());
    form::row(ui, t, "Color", |ui| {
        egui::ComboBox::from_id_salt(("lm-color-by", id.0))
            .selected_text(current.clone().unwrap_or_else(|| "One color".into()))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "One color").clicked() {
                    s.color_by = None;
                }
                for f in measures.iter().chain(&categories) {
                    if ui.selectable_label(current.as_deref() == Some(f), f).clicked() {
                        let cat = categories.contains(f);
                        let unit = l.field(f).and_then(|p| p.unit.value.clone());
                        let pal = if palette::is_temperature(unit.as_deref()) { "heat" } else { "viridis" };
                        s.color_by = Some(ColorBy {
                            field: f.clone(),
                            categories: cat,
                            classes: Classes::Groups { count: 5, equal_steps: false },
                            palette: pal.into(),
                            per_km2: false,
                            fit_to_view: false,
                        });
                    }
                }
            });
    });
    if s.color_by.is_none() {
        form::indented(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (c, name) in lm_core::document::OKABE_ITO.iter().zip(SWATCH_NAMES) {
                if form::swatch_button(ui, t, color(*c), s.fill == *c, name).clicked() {
                    s.fill = *c;
                    s.stroke = match l.profile.kind {
                        LayerKind::Areas => lm_core::document::darker(*c),
                        LayerKind::Lines => *c,
                        _ => s.stroke,
                    };
                }
            }
        });
    }
    if let Some(cb) = s.color_by.as_mut().filter(|c| !c.categories) {
        form::indented(ui, |ui| {
            let smooth = cb.classes == Classes::Smooth;
            match form::segmented(ui, t, usize::from(!smooth), &["Smooth", "Groups"], ui.available_width()) {
                Some(0) => cb.classes = Classes::Smooth,
                Some(_) => cb.classes = Classes::Groups { count: 5, equal_steps: false },
                None => {}
            }
        });
        ui.add_space(2.0);
        form::row(ui, t, "Palette", |ui| {
            if let Some(p) = palette_tiles(ui, t, &cb.palette) {
                cb.palette = p.into();
            }
        });
        if let Classes::Groups { count, equal_steps } = &mut cb.classes {
            form::row(ui, t, "Groups", |ui| {
                let mut n = *count;
                egui::ComboBox::from_id_salt(("lm-class-count", id.0)).width(48.0).selected_text(n.to_string()).show_ui(ui, |ui| {
                    for k in 2..=7u8 {
                        ui.selectable_value(&mut n, k, k.to_string());
                    }
                });
                *count = n;
                if let Some(k) = form::segmented(ui, t, usize::from(*equal_steps), &["Equal counts", "Equal steps"], ui.available_width()) {
                    *equal_steps = k == 1;
                }
            });
        }
        form::indented(ui, |ui| form::checkbox(ui, t, &mut cb.fit_to_view, "Fit range to view"));
    }
    // The count guard.
    if data::count_guard(l) {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(3.0, 32.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, t.assumption);
            ui.label("Larger areas tend to have higher counts.");
            if ui.link("Color by count per km² instead?").clicked()
                && let Some(cb) = s.color_by.as_mut()
            {
                cb.per_km2 = true;
            }
        });
    }
    if let Some(cb) = s.color_by.as_mut().filter(|c| c.per_km2) {
        form::indented(ui, |ui| {
            ui.label(RichText::new(format!("{} per km² (geodesic area)", cb.field)).size(CAPTION).color(t.text_secondary));
            if ui.small_button("Use totals").clicked() {
                cb.per_km2 = false;
            }
        });
    }
    // Size (points).
    if l.profile.kind == LayerKind::Points && !measures.is_empty() {
        let current = s.size_by.as_ref().map(|c| c.field.clone());
        form::row(ui, t, "Size", |ui| {
            egui::ComboBox::from_id_salt(("lm-size-by", id.0))
                .selected_text(current.clone().unwrap_or_else(|| "Same size".into()))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    if ui.selectable_label(current.is_none(), "Same size").clicked() {
                        s.size_by = None;
                    }
                    for f in &measures {
                        if ui.selectable_label(current.as_deref() == Some(f), f).clicked() {
                            s.size_by = Some(SizeBy { field: f.clone(), max_size: 24.0 });
                        }
                    }
                });
        });
        if s.size_by.is_none() {
            form::indented(ui, |ui| form::slider(ui, t, egui::Slider::new(&mut s.point_size, 3.0..=16.0).suffix(" px")));
        }
    }
    // Labels.
    let texts = fields_with(l, |r| matches!(r, FieldRole::Name | FieldRole::Text | FieldRole::Category | FieldRole::Identifier));
    if !texts.is_empty() {
        let current = s.labels.as_ref().map(|c| c.field.clone());
        form::row(ui, t, "Labels", |ui| {
            egui::ComboBox::from_id_salt(("lm-labels", id.0))
                .selected_text(current.clone().unwrap_or_else(|| "Off".into()))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    if ui.selectable_label(current.is_none(), "Off").clicked() {
                        s.labels = None;
                    }
                    for f in &texts {
                        if ui.selectable_label(current.as_deref() == Some(f), f).clicked() {
                            s.labels = Some(LabelStyle { field: f.clone(), size: 12.0 });
                        }
                    }
                });
        });
        if let Some(ls) = s.labels.as_mut() {
            form::indented(ui, |ui| form::slider(ui, t, egui::Slider::new(&mut ls.size, 9.0..=20.0).suffix(" pt")));
        }
    }
    form::divider(ui, t);
    // Appearance: opacity (one undo step per drag) and outline or width.
    let before = l.style.clone();
    let mut opacity = s.opacity;
    let resp = form::row(ui, t, "Opacity", |ui| {
        form::slider(ui, t, egui::Slider::new(&mut opacity, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)))
    });
    if resp.changed() && (opacity - before.opacity).abs() > f32::EPSILON {
        let mut g = before.clone();
        g.opacity = opacity;
        let tx = shell.ws.doc.set_style_tx(id, g);
        shell.ws.commit_gesture(tx, &format!("opacity:{}", id.0));
        return;
    }
    if resp.drag_stopped() || resp.lost_focus() {
        shell.ws.end_gesture();
    }
    if l.profile.kind != LayerKind::Lines {
        let mut outline = s.stroke_width > 0.0;
        if form::indented(ui, |ui| form::checkbox(ui, t, &mut outline, "Outline")).changed() {
            s.stroke_width = if outline {
                match l.profile.kind {
                    LayerKind::Points => 1.5,
                    _ => 1.0,
                }
            } else {
                0.0
            };
        }
    } else {
        form::row(ui, t, "Width", |ui| form::slider(ui, t, egui::Slider::new(&mut s.line_width, 1.0..=8.0).suffix(" px")));
    }
    form::divider(ui, t);
    form::heading(ui, "Legend");
    legend(shell, ui, t, l);
    if s != l.style {
        set_style(shell, id, s);
    }
}

/// Names of the single-colour swatches (Okabe–Ito), for tooltips and screen readers.
const SWATCH_NAMES: [&str; 8] = ["Blue", "Orange", "Bluish green", "Vermilion", "Reddish purple", "Sky blue", "Yellow", "Black"];

/// The sequential palettes as tiles (ramp and name); returns the one clicked.
fn palette_tiles(ui: &mut Ui, t: &Theme, current: &str) -> Option<&'static str> {
    let gap = 6.0;
    let n = palette::SEQUENTIAL.len() as f32;
    let w = ((ui.available_width() - (n - 1.0) * gap) / n).floor();
    let mut out = None;
    ui.spacing_mut().item_spacing.x = gap;
    for p in palette::SEQUENTIAL {
        let on = current == p;
        let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
        let tile = egui::Rect::from_min_size(r.min, vec2(w, 22.0));
        form::tile_frame(ui, t, tile, on, resp.hovered());
        let inner = tile.shrink(4.0);
        let steps = 32;
        let sw = inner.width() / steps as f32;
        for k in 0..steps {
            let c = palette::sample(p, (k as f64 + 0.5) / steps as f64);
            let x = inner.left() + k as f32 * sw;
            ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, inner.top()), vec2(sw + 0.5, inner.height())), 0.0, color(c));
        }
        form::tile_caption(ui, t, egui::Rect::from_min_max(egui::pos2(r.left(), tile.bottom() + 3.0), r.max), palette::label(p), on);
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, palette::label(p)));
        if resp.clicked() && !on {
            out = Some(p);
        }
    }
    out
}

/// A preset tile: a thumbnail drawn from the layer's style and fields, with its full name
/// (two lines at most) under it.
fn preset_thumb(ui: &mut Ui, t: &Theme, l: &DocLayer, s: &Style, name: &str, on: bool, w: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 74.0), Sense::click());
    let tile = egui::Rect::from_min_size(r.min, vec2(w, 42.0));
    form::tile_frame(ui, t, tile, on, resp.hovered());
    let p = ui.painter();
    let colors: Vec<[u8; 4]> = match &s.color_by {
        Some(cb) if cb.categories => lm_core::document::OKABE_ITO.iter().take(4).copied().collect(),
        Some(cb) => palette::classes(&cb.palette, 4),
        None => vec![s.fill; 4],
    };
    let sizes: [f32; 4] = if s.size_by.is_some() { [4.0, 7.0, 10.0, 14.0] } else { [7.0; 4] };
    let step = (w - 20.0) / 3.0;
    for (k, size) in sizes.iter().enumerate() {
        let c = colors.get(k).copied().unwrap_or(s.fill);
        let x = tile.left() + 10.0 + k as f32 * step;
        match l.profile.kind {
            LayerKind::Points | LayerKind::Table => {
                p.circle(egui::pos2(x, tile.center().y), size / 2.0, color(c), Stroke::new(1.0, Color32::WHITE));
            }
            LayerKind::Lines => {
                p.line_segment(
                    [egui::pos2(x - 5.0, tile.bottom() - 10.0), egui::pos2(x + 5.0, tile.top() + 10.0)],
                    Stroke::new(2.5, color(c)),
                );
            }
            _ => {
                let rr = egui::Rect::from_center_size(egui::pos2(x, tile.center().y), vec2(step - 3.0, 24.0));
                p.rect(
                    rr,
                    CornerRadius::same(2),
                    color(c).gamma_multiply(0.7),
                    Stroke::new(1.0, color(lm_core::document::darker(c))),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
    if s.labels.is_some() {
        let at = tile.center_bottom() - vec2(0.0, 4.0);
        let galley = p.layout_no_wrap("Abc".into(), theme::semibold(10.0), t.text_primary);
        let bg = egui::Rect::from_center_size(at - vec2(0.0, galley.size().y / 2.0), galley.size() + vec2(6.0, 0.0));
        p.rect_filled(bg, CornerRadius::same(3), t.surface.gamma_multiply(0.85));
        p.galley(bg.min + vec2(3.0, 0.0), galley, t.text_primary);
    }
    form::tile_caption(ui, t, egui::Rect::from_min_max(egui::pos2(r.left(), tile.bottom() + 4.0), r.max), name, on);
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, name));
    resp.on_hover_text(name)
}

fn raster_style(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let id = l.id;
    let mut s = l.style.clone();
    ui.add_space(4.0);
    if l.profile.bands.len() > 1 {
        let cur = s.band.max(1);
        form::row(ui, t, "Band", |ui| {
            egui::ComboBox::from_id_salt(("lm-band", id.0)).width(ui.available_width()).selected_text(format!("Band {cur}")).show_ui(
                ui,
                |ui| {
                    for b in &l.profile.bands {
                        let label = if b.name.is_empty() { format!("Band {}", b.index) } else { format!("Band {} ({})", b.index, b.name) };
                        if ui.selectable_label(cur == b.index, label).clicked() {
                            s.band = b.index;
                            s.stretch = None;
                        }
                    }
                },
            );
        });
    }
    form::row(ui, t, "Palette", |ui| {
        if let Some(p) = palette_tiles(ui, t, &s.ramp) {
            s.ramp = p.into();
        }
    });
    let full = shell.raster_full_range(id);
    let (lo, hi) = shell.raster_range(id).unwrap_or((0.0, 1.0));
    if let Some((flo, fhi)) = full {
        let mut a = lo;
        let mut b = hi;
        let r1 = form::row(ui, t, "From", |ui| form::slider(ui, t, egui::Slider::new(&mut a, flo..=fhi)));
        let r2 = form::row(ui, t, "To", |ui| form::slider(ui, t, egui::Slider::new(&mut b, flo..=fhi)));
        if (r1.changed() || r2.changed()) && a < b {
            let mut g = l.style.clone();
            g.stretch = Some((a, b));
            let tx = shell.ws.doc.set_style_tx(id, g);
            shell.ws.commit_gesture(tx, &format!("stretch:{}", id.0));
            return;
        }
        if r1.drag_stopped() || r2.drag_stopped() {
            shell.ws.end_gesture();
        }
        form::indented(ui, |ui| match &l.profile.grid {
            // A grid keeps one range for every time step (WP6.12), so a change in colour is a
            // change in value; reset goes back to the range of the sampled steps.
            Some(g) => {
                let default = lm_jobs::import::netcdf::default_stretch(&l.profile);
                let steps = if g.sampled == 1 { "the time step".to_string() } else { format!("{} time steps", g.sampled) };
                if s.stretch != default && default.is_some() {
                    if ui.link(format!("Reset to the middle 96% of {steps}")).clicked() {
                        s.stretch = default;
                    }
                } else {
                    let what = if g.has_time() {
                        format!("Same for every time step: the middle 96% of {steps}")
                    } else {
                        "Covers the middle 96% of cells".into()
                    };
                    ui.label(RichText::new(what).size(CAPTION).color(t.text_secondary));
                }
            }
            None => {
                if s.stretch.is_some() {
                    if ui.link("Reset to the middle 96% of cells").clicked() {
                        s.stretch = None;
                    }
                } else {
                    ui.label(RichText::new("Covers the middle 96% of cells").size(CAPTION).color(t.text_secondary));
                }
            }
        });
        // Show only values above…
        let mut on = s.above.is_some();
        if form::indented(ui, |ui| form::checkbox(ui, t, &mut on, "Only show values above")).changed() {
            s.above = on.then_some((lo + hi) / 2.0);
        }
        if let Some(mut v) = s.above
            && form::indented(ui, |ui| form::slider(ui, t, egui::Slider::new(&mut v, flo..=fhi))).changed()
        {
            let mut g = l.style.clone();
            g.above = Some(v);
            let tx = shell.ws.doc.set_style_tx(id, g);
            shell.ws.commit_gesture(tx, &format!("above:{}", id.0));
            return;
        }
    } else {
        form::row(ui, t, "Range", |ui| {
            ui.label(RichText::new(format!("{} to {}", data::number_text(lo), data::number_text(hi))).color(t.text_secondary))
        });
    }
    if s.like.is_some() && form::indented(ui, |ui| ui.link("Stop highlighting")).clicked() {
        s.like = None;
    }
    form::divider(ui, t);
    let mut opacity = s.opacity;
    let resp = form::row(ui, t, "Opacity", |ui| {
        form::slider(ui, t, egui::Slider::new(&mut opacity, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)))
    });
    if resp.changed() {
        let mut g = l.style.clone();
        g.opacity = opacity;
        let tx = shell.ws.doc.set_style_tx(id, g);
        shell.ws.commit_gesture(tx, &format!("opacity:{}", id.0));
        return;
    }
    if resp.drag_stopped() {
        shell.ws.end_gesture();
    }
    form::divider(ui, t);
    form::heading(ui, "Legend");
    legend(shell, ui, t, l);
    if s != l.style {
        set_style(shell, id, s);
    }
}
