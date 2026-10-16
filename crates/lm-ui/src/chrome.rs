//! The chrome around the map (WP1.4; Visual design system › Layout; Interaction specification ›
//! Toolbar, Screen states): toolbar and search, map controls, basemap chooser, scale bar,
//! attribution and credits, notices, the debug overlay, the welcome card.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;
use lm_core::basemap::{OFFLINE_MAX_ZOOM, Source, providers};

use crate::theme::{self, CAPTION, ICON_HIT, PANEL_MARGIN, RADIUS_CARD, RADIUS_CONTROL, RADIUS_PANEL, regular, semibold};
use crate::{Shell, System, Units};

/// Phases that give each toolbar button something to do (it is disabled until then).
const OPEN_PHASE: u8 = 2;
const LAYERS_PHASE: u8 = 2;
const UNDO_PHASE: u8 = 2;
const SHARE_PHASE: u8 = 5;
/// The welcome card appears once Open file and Try an example work.
const WELCOME_PHASE: u8 = 2;

fn panel_frame(t: &theme::Theme) -> egui::Frame {
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
    painter.text(rect.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, "LayeredMap", semibold(theme::HEADLINE), t.text_primary);

    let width = (rect.width() * 0.34).clamp(280.0, 480.0);
    let search_rect = Rect::from_center_size(pos2(rect.center().x - width * 0.15, rect.center().y), vec2(width, 32.0));
    search_field(shell, ui, search_rect);

    // Buttons at the right, disabled until their phase.
    let mut x = rect.right() - 12.0;
    let flags = shell.flags;
    for (glyph, label, phase) in [
        (icon::EXPORT, "Share", SHARE_PHASE),
        (icon::ARROW_COUNTER_CLOCKWISE, "Undo", UNDO_PHASE),
        (icon::STACK, "Layers", LAYERS_PHASE),
        (icon::FOLDER_OPEN, "Open", OPEN_PHASE),
    ] {
        let enabled = flags.shows(phase) && phase <= lm_core::phase::BUILD_PHASE;
        let color = if enabled { t.text_primary } else { t.text_secondary.gamma_multiply(0.7) };
        let text = painter.layout_no_wrap(label.to_string(), regular(theme::BODY), color);
        let w = 18.0 + 6.0 + text.size().x + 20.0;
        let r = Rect::from_min_size(pos2(x - w, rect.center().y - 16.0), vec2(w, 32.0));
        x -= w + 4.0;
        let resp = ui.allocate_rect(r, if enabled { Sense::click() } else { Sense::hover() });
        if enabled && resp.hovered() {
            painter.rect_filled(r, CornerRadius::same(RADIUS_CONTROL), t.surface_raised);
        }
        painter.text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, glyph, theme::icon_font(18.0), color);
        painter.galley(pos2(r.left() + 34.0, r.center().y - text.size().y / 2.0), text, color);
        if !enabled {
            resp.on_hover_text(format!("{label} arrives in a later phase"));
        }
    }
}

fn search_field(shell: &mut Shell, ui: &mut Ui, rect: Rect) {
    let t = *shell.theme();
    let id = egui::Id::new("lm-search");
    let edit = egui::TextEdit::singleline(&mut shell.search.query)
        .id(id)
        .hint_text("Search places or your data")
        .desired_width(rect.width() - 40.0)
        .margin(egui::Margin { left: 32, right: 10, top: 7, bottom: 7 });
    let resp = ui.put(rect, edit);
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
    let has_focus = resp.has_focus() || ui.memory(|m| m.has_focus(id));
    let (down, up, enter, esc) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    let n = shell.search.rows.len();
    if has_focus && n > 0 {
        if down {
            shell.search.selected = (shell.search.selected + 1).min(n - 1);
        }
        if up {
            shell.search.selected = shell.search.selected.saturating_sub(1);
        }
    }
    let mut chosen = None;
    // Return in the field (egui moves focus away on Enter, so check `lost_focus` too).
    if (has_focus || resp.lost_focus()) && enter && n > 0 {
        chosen = shell.search.rows.get(shell.search.selected).map(|r| r.choice.clone());
    }
    if esc {
        shell.search.clear();
        resp.surrender_focus();
    }
    if shell.search.open && !shell.search.query.trim().is_empty() {
        let list = Rect::from_min_size(rect.left_bottom() + vec2(0.0, 6.0), vec2(rect.width(), 0.0));
        egui::Area::new(egui::Id::new("lm-search-results")).order(egui::Order::Foreground).fixed_pos(list.min).show(ui.ctx(), |ui| {
            panel_frame(&t).inner_margin(egui::Margin::same(8)).corner_radius(CornerRadius::same(RADIUS_CARD)).show(ui, |ui| {
                ui.set_width(rect.width() - 16.0);
                if !shell.search.ready() {
                    ui.label(egui::RichText::new("Loading places…").color(t.text_secondary).size(CAPTION));
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
                } else if shell.search.rows.is_empty() {
                    ui.label(egui::RichText::new("No places match").color(t.text_secondary).size(CAPTION));
                } else {
                    ui.label(egui::RichText::new("Places").font(semibold(CAPTION)).color(t.text_secondary));
                    for (i, row) in shell.search.rows.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                        if i == shell.search.selected || resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(RADIUS_CONTROL), t.accent_weak);
                        }
                        ui.painter().text(
                            r.left_top() + vec2(8.0, 4.0),
                            Align2::LEFT_TOP,
                            &row.title,
                            regular(theme::BODY),
                            t.text_primary,
                        );
                        ui.painter().text(
                            r.left_bottom() + vec2(8.0, -4.0),
                            Align2::LEFT_BOTTOM,
                            &row.detail,
                            regular(CAPTION),
                            t.text_secondary,
                        );
                        if resp.clicked() {
                            chosen = Some(row.choice.clone());
                        }
                    }
                }
            });
        });
    }
    if let Some(c) = chosen {
        let tnow = ui.input(|i| i.time);
        let size = shell.last_map_size;
        let reduce = shell.reduce_motion;
        shell.map_view_mut().fly_to(lm_mapview::Coord::new(c.lon, c.lat), c.zoom, tnow, size, reduce);
        shell.search.query = c.label;
        shell.search.open = false;
        ui.memory_mut(|m| m.surrender_focus(id));
    }
}

/// Everything drawn over the map.
pub fn map_overlays(shell: &mut Shell, ui: &mut Ui, map: Rect, system: System) {
    let t = *shell.theme();
    controls(shell, ui, map);
    scale_bar(shell, ui, map);
    attribution(shell, ui, map);
    notices(shell, ui, map);
    if shell.debug_overlay() {
        debug_overlay(shell, ui, map);
    }
    if shell.flags.shows(WELCOME_PHASE) && WELCOME_PHASE <= lm_core::phase::BUILD_PHASE {
        welcome(&t, ui, map);
    }
    let _ = system;
}

fn icon_button(ui: &mut Ui, r: Rect, glyph: &str, tip: &str, t: &theme::Theme, active: bool) -> egui::Response {
    let resp = ui.allocate_rect(r, Sense::click()).on_hover_text(tip);
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
    if icon_button(ui, r_in, icon::PLUS, "Zoom in", &t, false).clicked() {
        shell.map_view_mut().zoom_by(1.0, now, size);
    }
    if icon_button(ui, r_out, icon::MINUS, "Zoom out", &t, false).clicked() {
        shell.map_view_mut().zoom_by(-1.0, now, size);
    }
    let open = shell.basemap_open;
    if icon_button(ui, r_base, icon::MAP_TRIFOLD, "Basemap", &t, open).clicked() {
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
                ui.label(egui::RichText::new("Basemap").font(semibold(theme::BODY)));
                ui.horizontal(|ui| {
                    for p in providers() {
                        ui.vertical(|ui| {
                            let (r, resp) = ui.allocate_exact_size(vec2(96.0, 64.0), Sense::click());
                            crate::basemap::paint_thumbnail(ui.painter(), r, &p, t.dark);
                            let selected = shell.prefs.basemap == p.id;
                            let stroke = if selected { Stroke::new(2.0, t.accent) } else { Stroke::new(1.0, t.border) };
                            ui.painter().rect_stroke(r, CornerRadius::same(RADIUS_CONTROL), stroke, egui::StrokeKind::Outside);
                            ui.label(egui::RichText::new(p.name).size(CAPTION));
                            if resp.clicked() && !selected {
                                shell.prefs.basemap = p.id.into();
                                shell.basemap_changed();
                            }
                        });
                    }
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Background strength").size(CAPTION).color(t.text_secondary));
                let mut pct = (shell.prefs.strength * 100.0).round();
                let online = lm_core::basemap::provider(&shell.prefs.basemap).source != Source::Offline;
                let resp = ui.add_enabled(online, egui::Slider::new(&mut pct, 40.0..=100.0).suffix("%").step_by(5.0).fixed_decimals(0));
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

/// The scale bar's length in points and its label, for `m_per_pt` metres per point.
pub fn scale(m_per_pt: f64, units: Units, max_pt: f64) -> (f64, String) {
    match units {
        Units::Metric => {
            let m = nice(m_per_pt * max_pt);
            let label = if m >= 1000.0 { format!("{} km", m / 1000.0) } else { format!("{m} m") };
            (m / m_per_pt, label)
        }
        Units::Imperial => {
            let ft_per_pt = m_per_pt / 0.3048;
            let ft_max = ft_per_pt * max_pt;
            if ft_max >= 5280.0 {
                let mi = nice(ft_max / 5280.0);
                (mi * 5280.0 / ft_per_pt, format!("{mi} mi"))
            } else {
                let ft = nice(ft_max);
                (ft / ft_per_pt, format!("{ft} ft"))
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
    let mut credits: Vec<String> = shell.map_view().attribution().iter().filter(|s| !s.trim().is_empty()).cloned().collect();
    let p = lm_core::basemap::provider(&shell.prefs.basemap);
    if credits.is_empty() {
        credits.push(p.attribution.into());
    }
    if !credits.iter().any(|c| c.contains("Natural Earth")) {
        credits.push("Natural Earth".into());
    }
    let text = credits.join(" · ");
    let galley = ui.painter().layout_no_wrap(text, regular(CAPTION), t.text_secondary);
    let r = Rect::from_min_size(map.right_bottom() - galley.size() - vec2(PANEL_MARGIN + 6.0, PANEL_MARGIN - 2.0), galley.size())
        .expand2(vec2(6.0, 3.0));
    let resp = ui.allocate_rect(r, Sense::click()).on_hover_text("Credits");
    ui.painter().rect_filled(r, CornerRadius::same(4), t.surface.gamma_multiply(0.85));
    ui.painter().galley(r.min + vec2(6.0, 3.0), galley, t.text_secondary);
    if resp.clicked() {
        shell.credits_open = true;
    }
}

pub fn credits(shell: &mut Shell, ctx: &egui::Context) {
    let mut open = shell.credits_open;
    egui::Window::new("Credits").open(&mut open).collapsible(false).resizable(false).default_width(420.0).show(ctx, |ui| {
        for (what, who) in [
            ("Map data", "© OpenStreetMap contributors (ODbL), through the Protomaps basemap"),
            ("Offline basemap", "Natural Earth (public domain)"),
            ("Place search", "GeoNames (geonames.org), CC BY 4.0"),
            ("Fonts", "Inter and Noto Sans SC (SIL Open Font License 1.1)"),
            ("Icons", "Phosphor Icons (MIT)"),
            ("Engine", "RustGIS (Apache-2.0)"),
        ] {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(what).strong());
                ui.label(who);
            });
        }
    });
    shell.credits_open = open;
}

/// One notice at a time, as a pill 12 px below the toolbar.
fn notices(shell: &mut Shell, ui: &Ui, map: Rect) {
    let t = *shell.theme();
    let now = ui.input(|i| i.time);
    let p = lm_core::basemap::provider(&shell.prefs.basemap);
    let offline_text = if p.source == Source::Offline {
        (shell.map_view().zoom() > OFFLINE_MAX_ZOOM).then_some("Detailed background unavailable offline".to_string())
    } else if !shell.map_view().tile_errors().is_empty() {
        Some("Detailed background unavailable offline. Your data and tools still work.".to_string())
    } else {
        None
    };
    let transient = shell.notice.as_ref().filter(|(_, at)| now - at < 4.0).map(|(s, _)| s.clone());
    let Some(text) = offline_text.or(transient) else { return };
    let galley = ui.painter().layout_no_wrap(text, regular(theme::BODY), t.text_primary);
    let r = Rect::from_center_size(pos2(map.center().x, map.top() + PANEL_MARGIN + 16.0), galley.size() + vec2(28.0, 14.0));
    ui.painter().rect(r, CornerRadius::same(16), t.surface, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    ui.painter().galley(r.min + vec2(14.0, 7.0), galley, t.text_primary);
    if shell.notice.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn debug_overlay(shell: &Shell, ui: &Ui, map: Rect) {
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
fn welcome(t: &theme::Theme, ui: &mut Ui, map: Rect) {
    let pos = pos2(map.center().x, map.bottom() - 48.0);
    egui::Area::new(egui::Id::new("lm-welcome")).pivot(Align2::CENTER_BOTTOM).fixed_pos(pos).show(ui.ctx(), |ui| {
        panel_frame(t).show(ui, |ui| {
            ui.label(egui::RichText::new("Open a file to see your data on the map.").size(theme::BODY));
            ui.horizontal(|ui| {
                let _ = ui.button("Open file…");
                let _ = ui.button("Try an example");
            });
        });
    });
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
