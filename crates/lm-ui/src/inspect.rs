//! Phase 3 inspector parts: the object card (Inspector: object card), the Style tab (WP3.5) and
//! the legend generated from the style. Everything numeric comes from `lm_jobs::inspect`.

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, vec2};

use lm_core::document::{Classes, ColorBy, DocLayer, LabelStyle, SizeBy, Style};
use lm_core::model::{FieldRole, LayerId, LayerKind};
use lm_core::palette;
use lm_jobs::import::LayerData;
use lm_jobs::inspect::{self as data, LegendItem};

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, Theme};

pub(crate) fn color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn semibold(size: f32) -> egui::FontId {
    theme::semibold(size)
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).font(semibold(BODY)));
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
    if let Some(b) = band {
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
    let id = l.id;
    if l.profile.kind == LayerKind::Raster {
        raster_style(shell, ui, t, l);
        return;
    }
    // Presets: thumbnails drawn from the layer's own style.
    section(ui, "Presets");
    let presets = data::presets(l);
    ui.horizontal_wrapped(|ui| {
        for (name, s) in presets {
            let on = s.color_by == l.style.color_by && s.size_by == l.style.size_by && s.labels == l.style.labels;
            let resp = preset_thumb(ui, t, l, &s, &name, on);
            if resp.clicked() && !on {
                set_style(shell, id, s);
            }
        }
    });
    let measures = fields_with(l, |r| matches!(r, FieldRole::Measure(_)));
    let categories = fields_with(l, |r| r == FieldRole::Category);
    // Colour.
    section(ui, "Color");
    let mut s = l.style.clone();
    let current = s.color_by.as_ref().map(|c| c.field.clone());
    egui::ComboBox::from_id_salt(("lm-color-by", id.0))
        .selected_text(current.clone().unwrap_or_else(|| "One color".into()))
        .width(220.0)
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
    if s.color_by.is_none() {
        ui.horizontal(|ui| {
            for c in lm_core::document::OKABE_ITO {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                let on = s.fill == c;
                ui.painter().circle(
                    r.center(),
                    8.0,
                    color(c),
                    Stroke::new(if on { 2.5 } else { 1.0 }, if on { t.accent } else { t.border }),
                );
                if resp.clicked() {
                    s.fill = c;
                    s.stroke = match l.profile.kind {
                        LayerKind::Areas => lm_core::document::darker(c),
                        LayerKind::Lines => c,
                        _ => s.stroke,
                    };
                }
            }
        });
    }
    if let Some(cb) = s.color_by.as_mut().filter(|c| !c.categories) {
        ui.horizontal(|ui| {
            let smooth = cb.classes == Classes::Smooth;
            if ui.selectable_label(smooth, "Smooth").clicked() {
                cb.classes = Classes::Smooth;
            }
            if ui.selectable_label(!smooth, "Groups").clicked() && smooth {
                cb.classes = Classes::Groups { count: 5, equal_steps: false };
            }
        });
        ui.horizontal(|ui| {
            for p in palette::SEQUENTIAL {
                let on = cb.palette == p;
                let resp = ui.vertical(|ui| {
                    ramp(ui, p, 52.0);
                    ui.label(RichText::new(palette::label(p)).size(CAPTION).color(if on { t.accent } else { t.text_secondary }));
                });
                if resp.response.interact(Sense::click()).clicked() {
                    cb.palette = p.into();
                }
            }
        });
        egui::CollapsingHeader::new("Classes").id_salt(("lm-classes", id.0)).show(ui, |ui| {
            if let Classes::Groups { count, equal_steps } = &mut cb.classes {
                ui.horizontal(|ui| {
                    ui.label("Groups");
                    let mut n = *count;
                    egui::ComboBox::from_id_salt(("lm-class-count", id.0)).selected_text(n.to_string()).show_ui(ui, |ui| {
                        for k in 2..=7u8 {
                            ui.selectable_value(&mut n, k, k.to_string());
                        }
                    });
                    *count = n;
                });
                ui.horizontal(|ui| {
                    if ui.selectable_label(!*equal_steps, "Equal counts").clicked() {
                        *equal_steps = false;
                    }
                    if ui.selectable_label(*equal_steps, "Equal steps").clicked() {
                        *equal_steps = true;
                    }
                });
            } else {
                ui.label(RichText::new("Smooth colors every value along the palette.").color(t.text_secondary));
            }
        });
        ui.checkbox(&mut cb.fit_to_view, "Fit range to view");
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
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} per km² (geodesic area)", cb.field)).size(CAPTION).color(t.text_secondary));
            if ui.small_button("Use totals").clicked() {
                cb.per_km2 = false;
            }
        });
    }
    section(ui, "Legend");
    legend(shell, ui, t, l);
    // Size (points).
    if l.profile.kind == LayerKind::Points && !measures.is_empty() {
        section(ui, "Size");
        let current = s.size_by.as_ref().map(|c| c.field.clone());
        egui::ComboBox::from_id_salt(("lm-size-by", id.0))
            .selected_text(current.clone().unwrap_or_else(|| "Same size".into()))
            .width(220.0)
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
        if s.size_by.is_none() {
            ui.add(egui::Slider::new(&mut s.point_size, 3.0..=16.0).text("px"));
        }
    }
    // Opacity and outline.
    section(ui, "Look");
    let before = l.style.clone();
    let mut opacity = s.opacity;
    let resp = ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("Opacity").custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
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
        if ui.checkbox(&mut outline, "Outline").changed() {
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
        ui.add(egui::Slider::new(&mut s.line_width, 1.0..=8.0).text("Width"));
    }
    // Labels.
    let texts = fields_with(l, |r| matches!(r, FieldRole::Name | FieldRole::Text | FieldRole::Category | FieldRole::Identifier));
    if !texts.is_empty() {
        let current = s.labels.as_ref().map(|c| c.field.clone());
        ui.horizontal(|ui| {
            ui.label("Labels");
            egui::ComboBox::from_id_salt(("lm-labels", id.0)).selected_text(current.clone().unwrap_or_else(|| "Off".into())).show_ui(
                ui,
                |ui| {
                    if ui.selectable_label(current.is_none(), "Off").clicked() {
                        s.labels = None;
                    }
                    for f in &texts {
                        if ui.selectable_label(current.as_deref() == Some(f), f).clicked() {
                            s.labels = Some(LabelStyle { field: f.clone(), size: 12.0 });
                        }
                    }
                },
            );
        });
        if let Some(ls) = s.labels.as_mut() {
            ui.add(egui::Slider::new(&mut ls.size, 9.0..=20.0).text("Label size"));
        }
    }
    if s != l.style {
        set_style(shell, id, s);
    }
}

/// A preset thumbnail: a 56 × 40 tile drawn from the layer's style and fields, with its name.
fn preset_thumb(ui: &mut Ui, t: &Theme, l: &DocLayer, s: &Style, name: &str, on: bool) -> egui::Response {
    let size = vec2(64.0, 62.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let tile = egui::Rect::from_min_size(r.min, vec2(64.0, 40.0));
    p.rect(
        tile,
        CornerRadius::same(6),
        t.surface_raised,
        Stroke::new(if on { 2.0 } else { 1.0 }, if on { t.accent } else { t.border }),
        egui::StrokeKind::Inside,
    );
    let colors: Vec<[u8; 4]> = match &s.color_by {
        Some(cb) if cb.categories => lm_core::document::OKABE_ITO.iter().take(4).copied().collect(),
        Some(cb) => palette::classes(&cb.palette, 4),
        None => vec![s.fill; 4],
    };
    let sizes: [f32; 4] = if s.size_by.is_some() { [4.0, 7.0, 10.0, 14.0] } else { [7.0; 4] };
    for (k, size) in sizes.iter().enumerate() {
        let c = colors.get(k).copied().unwrap_or(s.fill);
        let x = tile.left() + 10.0 + k as f32 * 14.0;
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
                let rr = egui::Rect::from_center_size(egui::pos2(x, tile.center().y), vec2(12.0, 22.0));
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
        p.text(tile.center_bottom() - vec2(0.0, 6.0), egui::Align2::CENTER_BOTTOM, "Abc", theme::regular(10.0), t.text_primary);
    }
    let short: String = if name.chars().count() > 10 { name.chars().take(9).collect::<String>() + "…" } else { name.to_string() };
    p.text(
        egui::pos2(r.center().x, r.bottom()),
        egui::Align2::CENTER_BOTTOM,
        short,
        theme::regular(CAPTION),
        if on { t.accent } else { t.text_secondary },
    );
    resp.on_hover_text(name)
}

fn raster_style(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let id = l.id;
    let mut s = l.style.clone();
    if l.profile.bands.len() > 1 {
        section(ui, "Band");
        let cur = s.band.max(1);
        egui::ComboBox::from_id_salt(("lm-band", id.0)).selected_text(format!("Band {cur}")).show_ui(ui, |ui| {
            for b in &l.profile.bands {
                let label = if b.name.is_empty() { format!("Band {}", b.index) } else { format!("Band {} ({})", b.index, b.name) };
                if ui.selectable_label(cur == b.index, label).clicked() {
                    s.band = b.index;
                    s.stretch = None;
                }
            }
        });
    }
    section(ui, "Colors");
    ui.horizontal(|ui| {
        for p in palette::SEQUENTIAL {
            let on = s.ramp == p;
            let resp = ui.vertical(|ui| {
                ramp(ui, p, 52.0);
                ui.label(RichText::new(palette::label(p)).size(CAPTION).color(if on { t.accent } else { t.text_secondary }));
            });
            if resp.response.interact(Sense::click()).clicked() {
                s.ramp = p.into();
            }
        }
    });
    section(ui, "Range");
    let full = shell.raster_full_range(id);
    let (lo, hi) = shell.raster_range(id).unwrap_or((0.0, 1.0));
    ui.label(
        RichText::new(format!("{} to {} (2–98% of cells by default)", data::number_text(lo), data::number_text(hi)))
            .color(t.text_secondary),
    );
    if let Some((flo, fhi)) = full {
        let mut a = lo;
        let mut b = hi;
        let r1 = ui.add(egui::Slider::new(&mut a, flo..=fhi).text("Low"));
        let r2 = ui.add(egui::Slider::new(&mut b, flo..=fhi).text("High"));
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
        if s.stretch.is_some() && ui.link("Back to 2–98%").clicked() {
            s.stretch = None;
        }
        // Show only values above…
        let mut on = s.above.is_some();
        if ui.checkbox(&mut on, "Show only values above…").changed() {
            s.above = on.then_some((lo + hi) / 2.0);
        }
        if let Some(mut v) = s.above
            && ui.add(egui::Slider::new(&mut v, flo..=fhi)).changed()
        {
            let mut g = l.style.clone();
            g.above = Some(v);
            let tx = shell.ws.doc.set_style_tx(id, g);
            shell.ws.commit_gesture(tx, &format!("above:{}", id.0));
            return;
        }
    }
    if s.like.is_some() && ui.link("Stop highlighting").clicked() {
        s.like = None;
    }
    section(ui, "Look");
    let mut opacity = s.opacity;
    let resp = ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("Opacity").custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
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
    section(ui, "Legend");
    legend(shell, ui, t, l);
    if s != l.style {
        set_style(shell, id, s);
    }
}
