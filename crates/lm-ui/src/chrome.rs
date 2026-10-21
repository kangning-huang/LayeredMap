//! The chrome around the map (WP1.4; Visual design system › Layout; Interaction specification ›
//! Toolbar, Screen states): toolbar and search, map controls, basemap chooser, scale bar,
//! attribution and credits, notices, the debug overlay, the welcome card.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use lm_core::basemap::{OFFLINE_MAX_ZOOM, Source, providers};
use lm_core::{i18n, tl, tn};

use crate::theme::{self, CAPTION, ICON_HIT, PANEL_MARGIN, RADIUS_CARD, RADIUS_CONTROL, RADIUS_PANEL, regular, semibold};
use crate::{Shell, System, Units};

/// Phases that give each toolbar button something to do (it is disabled until then).
const OPEN_PHASE: u8 = 2;
const LAYERS_PHASE: u8 = 2;
const UNDO_PHASE: u8 = 2;
const SHARE_PHASE: u8 = crate::export::EXPORT_PHASE;
/// The welcome card appears once Open file and the Examples menu work.
const WELCOME_PHASE: u8 = 2;

pub(crate) fn panel_frame(t: &theme::Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(RADIUS_PANEL))
        .inner_margin(egui::Margin::same(theme::PANEL_PADDING as i8))
        .shadow(if t.dark {
            egui::epaint::Shadow::NONE
        } else {
            egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(31) }
        })
}

/// The 52 px toolbar: name, search (280–480 px, left of centre), Open · Layers · Undo · Share.
pub fn toolbar(shell: &mut Shell, ui: &mut Ui, rect: Rect, _system: System) {
    let t = *shell.theme();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.surface);
    painter.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, t.border));
    // The app icon before the name (owner's request, 2026-10-18).
    let icon_size = 26.0;
    let icon_rect = Rect::from_min_size(pos2(rect.left() + 14.0, rect.center().y - icon_size / 2.0), vec2(icon_size, icon_size));
    let name_x = match app_icon(ui.ctx()) {
        Some(tex) => {
            painter.image(tex.id(), icon_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            icon_rect.right() + 8.0
        }
        None => rect.left() + 16.0,
    };
    let name = painter.text(
        pos2(name_x, rect.center().y),
        Align2::LEFT_CENTER,
        lm_core::brand::shown(false),
        semibold(theme::HEADLINE),
        t.text_primary,
    );
    if shell.pro() {
        // "Pro" as a small accent badge after the name, while Pro is unlocked.
        let g = painter.layout_no_wrap("PRO".into(), semibold(10.0), t.on_accent); // i18n: brand
        let badge = Rect::from_min_size(pos2(name.right() + 6.0, rect.center().y - g.size().y / 2.0 - 2.0), g.size() + vec2(10.0, 4.0));
        painter.rect_filled(badge, 4.0, t.accent);
        painter.galley(badge.min + vec2(5.0, 2.0), g, t.on_accent);
    }

    let width = (rect.width() * 0.34).clamp(280.0, 480.0);
    let search_rect = Rect::from_center_size(pos2(rect.center().x - width * 0.15, rect.center().y), vec2(width, 32.0));
    search_field(shell, ui, search_rect);

    // Buttons at the right, disabled until their phase. They are laid out from the right but
    // made from the left, so Tab meets them in reading order.
    let mut x = rect.right() - 12.0;
    let flags = shell.flags;
    let mut buttons = Vec::new();
    for (glyph, label, phase) in [
        (icon::EXPORT, tn!("Share"), SHARE_PHASE),
        (icon::ARROW_COUNTER_CLOCKWISE, tn!("Undo"), UNDO_PHASE),
        (icon::STACK, tn!("Layers"), LAYERS_PHASE),
        (icon::FOLDER_OPEN, tn!("Open"), OPEN_PHASE),
    ] {
        // Share comes with export, released ahead of its phase (`export::shown`).
        let built = if phase == SHARE_PHASE { shell.export_shown() } else { flags.shows(phase) && phase <= lm_core::phase::BUILD_PHASE };
        let enabled = built && (label != "Undo" || shell.ws.undo.can_undo());
        let color = if enabled { t.text_primary } else { t.text_secondary.gamma_multiply(0.7) };
        let text = painter.layout_no_wrap(tl!(label).to_string(), regular(theme::BODY), color);
        let w = 18.0 + 6.0 + text.size().x + 20.0;
        let r = Rect::from_min_size(pos2(x - w, rect.center().y - 16.0), vec2(w, 32.0));
        x -= w + 4.0;
        buttons.push((glyph, label, built, enabled, color, text, r));
    }
    for (glyph, label, built, enabled, color, text, r) in buttons.into_iter().rev() {
        let resp = ui.allocate_rect(r, if enabled { Sense::click() } else { Sense::hover() });
        crate::a11y::button(&resp, tl!(label));
        if enabled && resp.hovered() {
            painter.rect_filled(r, CornerRadius::same(RADIUS_CONTROL), t.surface_raised);
        }
        painter.text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, glyph, theme::icon_font(18.0), color);
        painter.galley(pos2(r.left() + 34.0, r.center().y - text.size().y / 2.0), text, color);
        if !built {
            resp.on_hover_text(i18n::fmt("{label} arrives in a later phase", &[("label", tl!(label))]));
            continue;
        }
        // Undo labels come from lm-core in English (they are also saved in the history).
        let tip = match label {
            "Undo" => shell
                .ws
                .undo
                .undo_label()
                .map_or_else(|| tl!("Nothing to undo").to_string(), |l| i18n::fmt("Undo {action}", &[("action", &lower_first(l))])),
            "Open" => i18n::fmt("Open files ({key})", &[("key", &crate::keys::shortcut("O"))]),
            "Layers" => i18n::fmt("Show or hide the layer list ({key})", &[("key", &crate::keys::shortcut("L"))]),
            "Share" => i18n::fmt("Export a map image ({key}) or data", &[("key", &crate::keys::shortcut("E"))]),
            other => tl!(other).to_string(),
        };
        let clicked = resp.clicked();
        let resp = resp.on_hover_text(tip);
        if label == "Share" {
            share_menu(shell, &resp);
            continue;
        }
        if clicked {
            match label {
                "Open" => shell.command(ui.ctx(), crate::Command::RequestOpen),
                "Layers" => shell.command(ui.ctx(), crate::Command::ToggleLayers),
                "Undo" => shell.command(ui.ctx(), crate::Command::Undo),
                _ => {}
            }
        }
    }
}

/// Share's menu: Map image…, Data… and Map package… (Interaction specification › Screen states ›
/// Export; WP6.5).
fn share_menu(shell: &mut Shell, resp: &egui::Response) {
    let ctx = resp.ctx.clone();
    let has_data = !shell.ws.doc.layers.is_empty();
    egui::Popup::menu(resp).show(|ui| {
        ui.set_min_width(180.0);
        if ui.button(tl!("Map image…")).clicked() {
            shell.command(&ctx, crate::Command::ExportMapImage);
        }
        if ui.add_enabled(has_data, egui::Button::new(tl!("Data…"))).on_disabled_hover_text(tl!("Open a file first")).clicked() {
            shell.command(&ctx, crate::Command::ExportData);
        }
        if ui.add_enabled(has_data, egui::Button::new(tl!("Map package…"))).on_disabled_hover_text(tl!("Open a file first")).clicked() {
            shell.command(&ctx, crate::Command::ExportPackage);
        }
    });
}

/// The app icon (the same picture as the Dock's), as a texture made once.
fn app_icon(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("lm-app-icon");
    if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(tex);
    }
    let bytes = include_bytes!("../../../assets/icon/icon-256.png");
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let tex = ctx.load_texture("app-icon", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn search_field(shell: &mut Shell, ui: &mut Ui, rect: Rect) {
    let t = *shell.theme();
    let id = egui::Id::new("lm-search");
    let edit = egui::TextEdit::singleline(&mut shell.search.query)
        .id(id)
        .hint_text(tl!("Search places or your data"))
        .desired_width(rect.width() - 40.0)
        .margin(egui::Margin { left: 32, right: 10, top: 7, bottom: 7 });
    let resp = ui.put(rect, edit);
    crate::a11y::label(&resp, tl!("Search places or your data"));
    ui.painter().text(
        pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        icon::MAGNIFYING_GLASS,
        theme::icon_font(16.0),
        t.text_secondary,
    );
    if shell.focus_search {
        resp.request_focus();
        shell.focus_search = false;
    }
    if resp.changed() || resp.gained_focus() {
        shell.search.open = !shell.search.query.trim().is_empty();
    }
    shell.search.update();
    // "Your data" (WP3.9): layer names, and values in each layer's first five text fields.
    let key = format!(
        "{}|{:?}",
        shell.search.query,
        shell.ws.doc.layers.iter().map(|l| (l.id.0, l.profile.data_rev, l.display_name.len())).collect::<Vec<_>>()
    );
    if shell.data_search.0 != key {
        shell.data_search = (key, your_data(shell));
    }
    let data_rows = shell.data_search.1.clone();
    let has_focus = resp.has_focus() || ui.memory(|m| m.has_focus(id));
    let (down, up, enter, esc) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    let nd = data_rows.len();
    let n = nd + shell.search.rows.len();
    if has_focus && n > 0 {
        if down {
            shell.search.selected = (shell.search.selected + 1).min(n - 1);
        }
        if up {
            shell.search.selected = shell.search.selected.saturating_sub(1);
        }
    }
    let pick_at = |shell: &Shell, i: usize| -> Option<Pick> {
        if i < nd { data_rows.get(i).map(|d| d.pick.clone()) } else { shell.search.rows.get(i - nd).map(|r| Pick::Place(r.choice.clone())) }
    };
    let mut chosen: Option<Pick> = None;
    // Return in the field (egui moves focus away on Enter, so check `lost_focus` too).
    if (has_focus || resp.lost_focus()) && enter && n > 0 {
        chosen = pick_at(shell, shell.search.selected);
    }
    if esc {
        shell.search.clear();
        resp.surrender_focus();
    }
    if shell.search.open && !shell.search.query.trim().is_empty() {
        let list = Rect::from_min_size(rect.left_bottom() + vec2(0.0, 6.0), vec2(rect.width(), 0.0));
        let selected = shell.search.selected;
        egui::Area::new(egui::Id::new("lm-search-results")).order(egui::Order::Foreground).fixed_pos(list.min).show(ui.ctx(), |ui| {
            panel_frame(&t).inner_margin(egui::Margin::same(8)).corner_radius(CornerRadius::same(RADIUS_CARD)).show(ui, |ui| {
                ui.set_width(rect.width() - 16.0);
                let result_row = |ui: &mut Ui, i: usize, title: &str, detail: &str| -> bool {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                    crate::a11y::button(&resp, i18n::fmt("{title}, {detail}", &[("title", title), ("detail", detail)]));
                    if i == selected {
                        // The highlighted result is read as the arrows move it (the field keeps focus).
                        let spoken = crate::a11y::spoken(&i18n::fmt(
                            "{title}, {detail}, {i} of {n}",
                            &[("title", title), ("detail", detail), ("i", &i18n::count(i as u64 + 1)), ("n", &i18n::count(n as u64))],
                        ));
                        crate::a11y::live(ui.ctx(), egui::Id::new("lm-search-highlight"), &spoken);
                    }
                    if i == selected || resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(RADIUS_CONTROL), t.accent_weak);
                    }
                    ui.painter().text(r.left_top() + vec2(8.0, 4.0), Align2::LEFT_TOP, title, regular(theme::BODY), t.text_primary);
                    ui.painter().text(r.left_bottom() + vec2(8.0, -4.0), Align2::LEFT_BOTTOM, detail, regular(CAPTION), t.text_secondary);
                    resp.clicked()
                };
                if nd > 0 {
                    ui.label(egui::RichText::new(tl!("Your data")).font(semibold(CAPTION)).color(t.text_secondary));
                    for (i, d) in data_rows.iter().enumerate() {
                        if result_row(ui, i, &d.title, &d.detail) {
                            chosen = Some(d.pick.clone());
                        }
                    }
                }
                if !shell.search.ready() {
                    ui.label(egui::RichText::new(tl!("Loading places…")).color(t.text_secondary).size(CAPTION));
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
                } else if shell.search.rows.is_empty() {
                    if nd == 0 {
                        ui.label(egui::RichText::new(tl!("No places match")).color(t.text_secondary).size(CAPTION));
                    }
                } else {
                    ui.label(egui::RichText::new(tl!("Places")).font(semibold(CAPTION)).color(t.text_secondary));
                    for (k, row) in shell.search.rows.iter().enumerate() {
                        if result_row(ui, nd + k, &row.title, &row.detail) {
                            chosen = Some(Pick::Place(row.choice.clone()));
                        }
                    }
                }
            });
        });
    }
    match chosen {
        Some(Pick::Place(c)) => {
            let tnow = ui.input(|i| i.time);
            let size = shell.last_map_size;
            let reduce = shell.reduce_motion;
            shell.map_view_mut().fly_to(lm_mapview::Coord::new(c.lon, c.lat), c.zoom, tnow, size, reduce);
            shell.search.query = c.label;
            shell.search.open = false;
            ui.memory_mut(|m| m.surrender_focus(id));
        }
        Some(Pick::Layer(lid)) => {
            shell.ws.select_layer(lid);
            shell.zoom_to = shell.ws.extent(lid);
            shell.search.open = false;
            ui.memory_mut(|m| m.surrender_focus(id));
        }
        Some(Pick::Object(f)) => {
            // Choosing a value selects its object and opens its card.
            shell.ws.selection = lm_core::selection::SelectionContext::object(f);
            if let Some(ds) = crate::inspect::dataset(shell, f.layer) {
                shell.zoom_to = lm_jobs::inspect::row_extent(&ds, f.row);
            }
            shell.search.open = false;
            ui.memory_mut(|m| m.surrender_focus(id));
        }
        None => {}
    }
}

/// What choosing a search result does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pick {
    Place(crate::search::Choice),
    Layer(lm_core::model::LayerId),
    Object(lm_core::selection::FeatureRef),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DataRow {
    title: String,
    detail: String,
    pick: Pick,
}

/// Matches in the user's layers: layer names first, then values (at most five rows).
fn your_data(shell: &Shell) -> Vec<DataRow> {
    let q = shell.search.query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for l in shell.ws.doc.layers.iter().rev() {
        if l.display_name.to_lowercase().contains(&q) {
            out.push(DataRow { title: l.display_name.clone(), detail: l.profile.subtitle(), pick: Pick::Layer(l.id) });
        }
    }
    for l in shell.ws.doc.layers.iter().rev() {
        if out.len() >= 5 {
            break;
        }
        let Some(ds) = crate::inspect::dataset(shell, l.id) else { continue };
        for h in lm_jobs::inspect::search_values(l, &ds, &q, 5 - out.len()) {
            out.push(DataRow {
                title: h.text,
                detail: format!("{} · {}", l.display_name, h.field),
                pick: Pick::Object(lm_core::selection::FeatureRef { layer: l.id, row: h.row }),
            });
        }
    }
    out.truncate(5);
    out
}

/// Everything drawn over the map.
pub fn map_overlays(shell: &mut Shell, ui: &mut Ui, map: Rect, system: System) {
    let t = *shell.theme();
    controls(shell, ui, map);
    scale_bar(shell, ui, map);
    attribution(shell, ui, map);
    if shell.debug_overlay() {
        debug_overlay(shell, ui, map);
    }
    let _ = (system, t);
}

/// The first-launch card, while the map is empty (drawn before the map, for the Tab order).
pub(crate) fn welcome_card(shell: &mut Shell, ui: &mut Ui, map: Rect) {
    let t = *shell.theme();
    let empty = shell.ws.doc.layers.is_empty() && shell.ws.imports.is_empty();
    if shell.flags.shows(WELCOME_PHASE)
        && WELCOME_PHASE <= lm_core::phase::BUILD_PHASE
        && empty
        && !shell.welcome_closed
        && !shell.library.open
    {
        match welcome(&t, ui, map, crate::library::available(shell)) {
            Some(Welcome::Open) => shell.command(ui.ctx(), crate::Command::RequestOpen),
            Some(Welcome::Example(id)) => shell.command(ui.ctx(), crate::Command::OpenExample(id.to_string())),
            Some(Welcome::Library) => shell.command(ui.ctx(), crate::Command::OpenLibrary),
            Some(Welcome::Close) => shell.welcome_closed = true,
            None => {}
        }
    }
}

fn icon_button(ui: &mut Ui, r: Rect, glyph: &str, tip: &str, t: &theme::Theme, active: bool) -> egui::Response {
    let resp = ui.allocate_rect(r, Sense::click()).on_hover_text(tip);
    crate::a11y::button(&resp, tip);
    let p = ui.painter();
    let fill = if active {
        t.accent_weak
    } else if resp.hovered() {
        t.surface_raised
    } else {
        t.surface
    };
    p.rect(r, CornerRadius::same(RADIUS_CARD), fill, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    p.text(r.center(), Align2::CENTER_CENTER, glyph, theme::icon_font(18.0), t.text_primary);
    resp
}

/// Bottom right, stacked: zoom in, zoom out, basemap (36 × 36 each).
fn controls(shell: &mut Shell, ui: &mut Ui, map: Rect) {
    let t = *shell.theme();
    let x = map.right() - PANEL_MARGIN - ICON_HIT;
    let bottom = map.bottom() - PANEL_MARGIN - 26.0;
    let r_base = Rect::from_min_size(pos2(x, bottom - ICON_HIT), vec2(ICON_HIT, ICON_HIT));
    let r_out = r_base.translate(vec2(0.0, -ICON_HIT - 12.0));
    let r_in = r_out.translate(vec2(0.0, -ICON_HIT - 2.0));
    let now = ui.input(|i| i.time);
    let size = shell.last_map_size;
    if icon_button(ui, r_in, icon::PLUS, tl!("Zoom in"), &t, false).clicked() {
        shell.map_view_mut().zoom_by(1.0, now, size);
    }
    if icon_button(ui, r_out, icon::MINUS, tl!("Zoom out"), &t, false).clicked() {
        shell.map_view_mut().zoom_by(-1.0, now, size);
    }
    let open = shell.basemap_open;
    if icon_button(ui, r_base, icon::MAP_TRIFOLD, tl!("Basemap"), &t, open).clicked() {
        shell.basemap_open = !shell.basemap_open;
    }
    if shell.basemap_open {
        basemap_chooser(shell, ui, r_base);
    }
}

/// The basemap chooser popover: Auto, Light, Dark, Offline thumbnails and "Background strength".
fn basemap_chooser(shell: &mut Shell, ui: &mut Ui, anchor: Rect) {
    let t = *shell.theme();
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("lm-basemap-chooser"))
        .order(egui::Order::Foreground)
        .pivot(Align2::RIGHT_BOTTOM)
        .fixed_pos(anchor.left_bottom() + vec2(-8.0, 0.0))
        .show(ui.ctx(), |ui| {
            panel_frame(&t).show(ui, |ui| {
                ui.label(egui::RichText::new(tl!("Basemap")).font(semibold(theme::BODY)));
                ui.horizontal(|ui| {
                    for p in providers() {
                        ui.vertical(|ui| {
                            let (r, resp) = ui.allocate_exact_size(vec2(96.0, 64.0), Sense::click());
                            crate::basemap::paint_thumbnail(ui.painter(), r, &p, t.dark);
                            let selected = shell.prefs.basemap == p.id;
                            crate::a11y::choice(&resp, tl!(p.name), selected);
                            let stroke = if selected { Stroke::new(2.0, t.accent) } else { Stroke::new(1.0, t.border) };
                            ui.painter().rect_stroke(r, CornerRadius::same(RADIUS_CONTROL), stroke, egui::StrokeKind::Outside);
                            ui.label(egui::RichText::new(tl!(p.name)).size(CAPTION));
                            if resp.clicked() && !selected {
                                shell.prefs.basemap = p.id.into();
                                shell.basemap_changed();
                            }
                        });
                    }
                });
                ui.add_space(4.0);
                if lm_net::offline() {
                    ui.label(egui::RichText::new(tl!("Working offline: online backgrounds are off")).size(CAPTION).color(t.text_secondary));
                }
                ui.label(egui::RichText::new(tl!("Background strength")).size(CAPTION).color(t.text_secondary));
                let mut pct = (shell.prefs.strength * 100.0).round();
                let online = lm_core::basemap::provider(&shell.prefs.basemap).source != Source::Offline && !lm_net::offline();
                let resp = ui.add_enabled(online, egui::Slider::new(&mut pct, 40.0..=100.0).suffix("%").step_by(5.0).fixed_decimals(0));
                crate::a11y::label(&resp, tl!("Background strength"));
                if resp.changed() {
                    shell.prefs.strength = pct / 100.0;
                    shell.basemap_changed();
                }
            });
        });
    // Clicking elsewhere closes it (not the button, which toggles).
    if ui.input(|i| i.pointer.any_click())
        && !area.response.hovered()
        && !anchor.contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or_default())
    {
        close = true;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if close {
        shell.basemap_open = false;
    }
}

/// A round number of metres (1, 2 or 5 × 10ⁿ) at most `max`.
pub fn nice(max: f64) -> f64 {
    if !(max.is_finite() && max > 0.0) {
        return 0.0;
    }
    let p = 10f64.powf(max.log10().floor());
    [5.0, 2.0, 1.0].into_iter().map(|m| m * p).find(|v| *v <= max).unwrap_or(p)
}

/// A nice scale value as text: whole numbers with thousands separators ("5,000"), fractions as
/// written ("0.5").
fn nice_text(v: f64) -> String {
    if v >= 1.0 && v.fract() == 0.0 { i18n::count(v as u64) } else { i18n::number(&format!("{v}")) }
}

/// The scale bar's length in points and its label, for `m_per_pt` metres per point.
pub fn scale(m_per_pt: f64, units: Units, max_pt: f64) -> (f64, String) {
    match units {
        Units::Metric => {
            let m = nice(m_per_pt * max_pt);
            let label = if m >= 1000.0 {
                i18n::fmt("{n} km", &[("n", &nice_text(m / 1000.0))])
            } else {
                i18n::fmt("{n} m", &[("n", &nice_text(m))])
            };
            (m / m_per_pt, label)
        }
        Units::Imperial => {
            let ft_per_pt = m_per_pt / 0.3048;
            let ft_max = ft_per_pt * max_pt;
            if ft_max >= 5280.0 {
                let mi = nice(ft_max / 5280.0);
                (mi * 5280.0 / ft_per_pt, i18n::fmt("{n} mi", &[("n", &nice_text(mi))]))
            } else {
                let ft = nice(ft_max);
                (ft / ft_per_pt, i18n::fmt("{n} ft", &[("n", &nice_text(ft))]))
            }
        }
    }
}

fn scale_bar(shell: &Shell, ui: &Ui, map: Rect) {
    let t = *shell.theme();
    let cam = shell.map_view().camera();
    let lat = shell.map_view().center_lonlat().y.to_radians();
    let m_per_pt = cam.res * lat.cos();
    let (len, label) = scale(m_per_pt, shell.prefs.units, 110.0);
    let p = ui.painter();
    let base = pos2(map.left() + PANEL_MARGIN, map.bottom() - PANEL_MARGIN);
    let bg = Rect::from_min_max(base + vec2(0.0, -26.0), base + vec2(len as f32 + 12.0, 0.0));
    p.rect_filled(bg, CornerRadius::same(RADIUS_CONTROL), t.surface.gamma_multiply(0.85));
    let y = base.y - 6.0;
    let x0 = base.x + 6.0;
    let x1 = x0 + len as f32;
    let s = Stroke::new(1.5, t.text_primary);
    p.line_segment([pos2(x0, y), pos2(x1, y)], s);
    p.line_segment([pos2(x0, y), pos2(x0, y - 5.0)], s);
    p.line_segment([pos2(x1, y), pos2(x1, y - 5.0)], s);
    p.text(pos2(x0, y - 7.0), Align2::LEFT_BOTTOM, label, regular(CAPTION), t.text_primary);
}

fn attribution(shell: &mut Shell, ui: &mut Ui, map: Rect) {
    let t = *shell.theme();
    // Only the layers drawn at this zoom, each credit once (the satellite basemap's close-up map
    // is hidden at world zoom; its labels and close-up map both credit OpenStreetMap).
    let view = shell.map_view();
    let text = crate::basemap::credits_line(view.map(), view.zoom(), view.attribution());
    let galley = ui.painter().layout_no_wrap(text.clone(), regular(CAPTION), t.text_secondary);
    let r = Rect::from_min_size(map.right_bottom() - galley.size() - vec2(PANEL_MARGIN + 6.0, PANEL_MARGIN - 2.0), galley.size())
        .expand2(vec2(6.0, 3.0));
    let resp = ui.allocate_rect(r, Sense::click()).on_hover_text(tl!("Credits"));
    crate::a11y::button(&resp, i18n::fmt("Credits: {text}", &[("text", &text)]));
    ui.painter().rect_filled(r, CornerRadius::same(4), t.surface.gamma_multiply(0.85));
    ui.painter().galley(r.min + vec2(6.0, 3.0), galley, t.text_secondary);
    if resp.clicked() {
        shell.credits_open = true;
    }
}

pub fn credits(shell: &mut Shell, ctx: &egui::Context) {
    let mut open = shell.credits_open;
    egui::Window::new(tl!("Credits"))
        .id(egui::Id::new("Credits"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            for (what, who) in [
                (tn!("Map data"), tn!("© OpenStreetMap contributors (ODbL), through the Protomaps basemap")),
                (tn!("Offline basemap"), tn!("Natural Earth (public domain)")),
                (tn!("Place search"), tn!("GeoNames (geonames.org), CC BY 4.0")),
                (tn!("Chinese place names"), tn!("OpenCC (Apache-2.0)")),
                (tn!("Fonts"), tn!("Inter and Noto Sans SC (SIL Open Font License 1.1)")),
                (tn!("Icons"), tn!("Phosphor Icons (MIT)")),
                (tn!("Engine"), tn!("RustGIS (Apache-2.0)")),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(tl!(what)).strong());
                    ui.label(tl!(who));
                });
            }
        });
    shell.credits_open = open;
}

/// One notice at a time, as a pill 12 px below the toolbar, centred in the free part of the map.
pub(crate) fn notices_in(shell: &mut Shell, ui: &Ui, map: Rect) {
    notices(shell, ui, map);
}

fn notices(shell: &mut Shell, ui: &Ui, map: Rect) {
    let t = *shell.theme();
    let now = ui.input(|i| i.time);
    let p = lm_core::basemap::provider(&shell.prefs.basemap);
    let offline_text = if lm_net::offline() && p.source != Source::Offline {
        (shell.map_view().zoom() > OFFLINE_MAX_ZOOM).then_some(tl!("Working offline: the detailed background is off").to_string())
    } else if p.source == Source::Offline {
        (shell.map_view().zoom() > OFFLINE_MAX_ZOOM).then_some(tl!("Detailed background unavailable offline").to_string())
    } else if !shell.map_view().tile_errors().is_empty() && !lm_mapview::archive::looking() {
        Some(tl!("Detailed background unavailable offline. Your data and tools still work.").to_string())
    } else {
        None
    };
    let transient = shell.notice.as_ref().filter(|(_, at)| now - at < 4.0).map(|(s, _)| s.clone());
    let Some(text) = offline_text.or(transient) else { return };
    // Read once when it appears (a polite live region).
    crate::a11y::live(ui.ctx(), egui::Id::new("lm-notice-live"), &crate::a11y::spoken(&text));
    // Wrapped to the free part of the map and drawn above the panels, so neither ever cuts it off.
    let max_w = (map.width() - 2.0 * PANEL_MARGIN - 28.0).max(120.0);
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lm-notice")));
    let galley = painter.layout(text, regular(theme::BODY), t.text_primary, max_w);
    let size = galley.size() + vec2(28.0, 14.0);
    // One line centres 16 px below the panel margin, as before; more lines hang from the margin.
    let top = if galley.rows.len() > 1 { map.top() + PANEL_MARGIN } else { map.top() + PANEL_MARGIN + 16.0 - size.y / 2.0 };
    let r = Rect::from_min_size(pos2(map.center().x - size.x / 2.0, top), size);
    let radius = if galley.rows.len() > 1 { CornerRadius::same(theme::RADIUS_CARD) } else { CornerRadius::same(16) };
    painter.rect(r, radius, t.surface, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    painter.galley(r.min + vec2(14.0, 7.0), galley, t.text_primary);
    if shell.notice.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn debug_overlay(shell: &Shell, ui: &Ui, map: Rect) {
    // i18n: developer only (the debug overlay)
    let mv = shell.map_view();
    let s = mv.stats();
    let text = format!(
        "{:.0} fps · p95 {:.1} ms\nlast render {:.0} ms · renders {}\nzoom {:.2}{}",
        s.fps(),
        s.p95_ms(),
        mv.last_render_ms(),
        mv.renders,
        mv.zoom(),
        if mv.tiles_pending() { " · tiles loading" } else { "" }
    );
    let p = ui.painter();
    let galley = p.layout_no_wrap(text, egui::FontId::monospace(12.0), Color32::WHITE);
    let r = Rect::from_min_size(map.min + vec2(PANEL_MARGIN, PANEL_MARGIN), galley.size()).expand(8.0);
    p.rect_filled(r.translate(vec2(8.0, 8.0)), CornerRadius::same(RADIUS_CARD), Color32::from_black_alpha(190));
    p.galley(r.min + vec2(16.0, 16.0), galley, Color32::WHITE);
    ui.ctx().request_repaint();
}

/// First launch (Screen states): "Open a file to see your data on the map." Phase 2 onwards.
enum Welcome {
    Open,
    Example(&'static str),
    Library,
    Close,
}

/// The first-launch card (Screen states › First launch). "Examples" lists the example datasets;
/// "Browse data library" (WP5.4) shows when a library catalog is set.
fn welcome(t: &theme::Theme, ui: &mut Ui, map: Rect, library: bool) -> Option<Welcome> {
    let pos = pos2(map.center().x, map.bottom() - 48.0);
    let mut out = None;
    egui::Area::new(egui::Id::new("lm-welcome")).pivot(Align2::CENTER_BOTTOM).fixed_pos(pos).show(ui.ctx(), |ui| {
        panel_frame(t).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Open a file to see your data on the map.")).size(theme::BODY));
                let close = ui.add(egui::Button::new(egui::RichText::new(icon::X).font(theme::icon_font(12.0))).frame(false));
                crate::a11y::button(&close, tl!("Close"));
                if close.on_hover_text(tl!("Close")).clicked() {
                    out = Some(Welcome::Close);
                }
            });
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new(tl!("Open file…")).color(t.on_accent)).fill(t.accent)).clicked() {
                    out = Some(Welcome::Open);
                }
                // Examples ▾: a list to choose from (owner's request, 2026-10-18).
                // The caret is an icon glyph: it must be set in the icon font, the word in the text font.
                let mut label = egui::text::LayoutJob::default();
                label.append(
                    &format!("{} ", tl!("Examples")),
                    0.0,
                    egui::TextFormat { font_id: theme::regular(theme::BODY), color: t.text_primary, ..Default::default() },
                );
                label.append(
                    icon::CARET_DOWN,
                    0.0,
                    egui::TextFormat { font_id: theme::icon_font(12.0), color: t.text_secondary, ..Default::default() },
                );
                let menu = ui.menu_button(label, |ui| {
                    for e in crate::examples::EXAMPLES {
                        if example_row(ui, t, e).clicked() {
                            out = Some(Welcome::Example(e.id));
                            ui.close();
                        }
                    }
                });
                crate::a11y::button(&menu.response, tl!("Examples"));
                if library && ui.button(tl!("Browse data library")).on_hover_text(tl!("Course datasets to add to the map")).clicked() {
                    out = Some(Welcome::Library);
                }
            });
        });
    });
    out
}

/// One row of the Examples menu: the example's icon on a soft tile of its tint, then its title
/// and a line about it. The whole row is one button.
fn example_row(ui: &mut Ui, t: &theme::Theme, e: &crate::examples::Example) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(344.0, 46.0), Sense::click());
    let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!(e.title)));
    crate::a11y::describe(&r, tl!(e.blurb));
    let p = ui.painter();
    if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(theme::RADIUS_CONTROL), t.accent.gamma_multiply(0.10));
    }
    let [cr, cg, cb] = e.tint;
    let tint = Color32::from_rgb(cr, cg, cb);
    let tile = Rect::from_min_size(pos2(rect.left() + 6.0, rect.center().y - 16.0), vec2(32.0, 32.0));
    p.rect_filled(tile, CornerRadius::same(8), tint.gamma_multiply(if t.dark { 0.30 } else { 0.16 }));
    p.text(tile.center(), Align2::CENTER_CENTER, e.icon, theme::icon_font(20.0), tint);
    let x = tile.right() + 10.0;
    p.text(pos2(x, rect.center().y - 9.0), Align2::LEFT_CENTER, tl!(e.title), theme::regular(theme::BODY), t.text_primary);
    p.text(pos2(x, rect.center().y + 9.0), Align2::LEFT_CENTER, tl!(e.blurb), theme::regular(theme::CAPTION), t.text_secondary);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_numbers() {
        assert_eq!(nice(0.0), 0.0);
        assert_eq!(nice(7.3), 5.0);
        assert_eq!(nice(19.9), 10.0);
        assert_eq!(nice(1999.0), 1000.0);
        assert_eq!(nice(2000.0), 2000.0);
    }

    #[test]
    fn scale_bar_labels() {
        // 10 m per point, at most 110 points: 1 km = 100 points.
        let (len, label) = scale(10.0, Units::Metric, 110.0);
        assert_eq!(label, "1 km");
        assert!((len - 100.0).abs() < 1e-9);
        let (_, label) = scale(0.5, Units::Metric, 110.0);
        assert_eq!(label, "50 m");
        let (len, label) = scale(100.0, Units::Imperial, 110.0);
        assert_eq!(label, "5 mi");
        assert!(len <= 110.0);
        let (_, label) = scale(0.3048, Units::Imperial, 110.0);
        assert_eq!(label, "100 ft");
    }
}
