//! Phase 2 panels: the layer shelf (Interaction specification › Layer shelf), the minimal layer
//! overview with its assumption lines and Details tab (› Inspector: layer overview), question
//! cards attached to shelf rows (never modal), the import report's Review list, and the drop
//! overlay.

use egui::{Align2, Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;

use lm_core::model::{Assumption, AssumptionKind, Choice, ImportStatus, LayerId, LayerKind, Subject, Treatment, group_thousands};

use crate::theme::{self, BODY, CAPTION, PANEL_MARGIN, ROW_HEIGHT, Theme};
use crate::{FileRequest, Shell};

/// Shelf and inspector widths (Layout).
pub const SHELF_WIDTH: f32 = 280.0;
pub const INSPECTOR_WIDTH: f32 = 360.0;
/// A joined table's row under its layer: indent and height (name above a caption).
const SUBLAYER_INDENT: f32 = 20.0;
const SUBLAYER_HEIGHT: f32 = 40.0;

fn frame(t: &Theme) -> egui::Frame {
    crate::chrome::panel_frame(t)
}

fn semibold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("semibold".into()))
}

/// The middle of a long name gives way first ("district_bou…_2026.shp").
fn middle_truncate(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let head: String = s.chars().take(keep / 2 + keep % 2).collect();
    let tail: String = s.chars().skip(n - keep / 2).collect();
    format!("{head}…{tail}")
}

fn color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// A row's badge (Layer shelf › Badges).
enum Badge {
    None,
    Assumption,
    Decision,
    Working,
    Failed,
}

fn paint_badge(ui: &Ui, at: egui::Pos2, b: &Badge, t: &Theme) {
    let p = ui.painter();
    match b {
        Badge::None => {}
        Badge::Assumption => {
            p.circle_filled(at, 4.0, t.assumption);
        }
        Badge::Decision => {
            p.text(at, Align2::CENTER_CENTER, icon::WARNING, theme::icon_font(16.0), t.warning);
        }
        Badge::Working => {
            let r = Rect::from_center_size(at, vec2(14.0, 14.0));
            egui::Spinner::new().size(14.0).color(t.text_secondary).paint_at(ui, r);
        }
        Badge::Failed => {
            p.text(at, Align2::CENTER_CENTER, icon::WARNING_CIRCLE, theme::icon_font(16.0), t.critical);
        }
    }
}

/// The layer shelf: a floating panel at the left, rows top to bottom (the top row draws on top).
pub fn shelf(shell: &mut Shell, ui: &mut Ui, map: Rect) {
    let t = *shell.theme();
    if !shell.shelf_open || (shell.ws.doc.layers.is_empty() && shell.ws.imports.is_empty()) {
        return;
    }
    let pos = pos2(map.left() + PANEL_MARGIN, map.top() + PANEL_MARGIN);
    let max_h = (map.height() - 2.0 * PANEL_MARGIN - 40.0).max(120.0);
    egui::Area::new(egui::Id::new("lm-shelf")).order(egui::Order::Middle).fixed_pos(pos).show(ui.ctx(), |ui| {
        frame(&t).show(ui, |ui| {
            ui.set_width(SHELF_WIDTH - 2.0 * theme::PANEL_PADDING);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Layers").font(semibold(theme::HEADLINE)));
                if shell.ws.doc.layers.len() > 1 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let link = ui.link(RichText::new("Remove all").size(CAPTION).color(t.text_secondary));
                        if link
                            .on_hover_text(format!("Remove every layer from the map ({} brings them back)", crate::keys::shortcut("Z")))
                            .clicked()
                        {
                            shell.command(ui.ctx(), crate::Command::RemoveAllLayers);
                        }
                    });
                }
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(max_h - 60.0)
                .min_scrolled_height((max_h - 60.0).min(420.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    import_rows(shell, ui, &t);
                    layer_rows(shell, ui, &t);
                    let r = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
                    crate::a11y::button(&r.1, "Open file");
                    let col = if r.1.hovered() { t.accent } else { t.text_secondary };
                    ui.painter().text(
                        r.0.left_center() + vec2(4.0, 0.0),
                        Align2::LEFT_CENTER,
                        format!("{}  Open file", icon::PLUS),
                        egui::FontId::proportional(BODY),
                        col,
                    );
                    if r.1.clicked() {
                        shell.file_requests.push(FileRequest::Open);
                    }
                });
        });
    });
}

/// A menu item with a leading symbol in the label's colour and weight (Apple's standard symbols
/// where one exists: `menus.md › Icons`). Every item in a menu gets one, so the labels line up.
fn menu_item(ui: &mut Ui, t: &Theme, glyph: &str, label: &str) -> egui::Response {
    let mut job = egui::text::LayoutJob::default();
    let format = |font| egui::TextFormat { font_id: font, color: t.text_primary, valign: egui::Align::Center, ..Default::default() };
    job.append(glyph, 0.0, format(theme::icon_font(15.0)));
    job.append(label, 8.0, format(egui::FontId::proportional(BODY)));
    let resp = ui.add(egui::Button::new(job));
    crate::a11y::button(&resp, label);
    resp
}

fn import_rows(shell: &mut Shell, ui: &mut Ui, t: &Theme) {
    let mut cancel = None;
    for row in &shell.ws.imports {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT + 14.0), Sense::hover());
        let p = ui.painter();
        p.text(
            r.left_top() + vec2(28.0, 8.0),
            Align2::LEFT_TOP,
            middle_truncate(&row.name(), 30),
            egui::FontId::proportional(BODY),
            t.text_primary,
        );
        let (sub, badge) = match &row.error {
            Some(e) => (e.to_string(), Badge::Failed),
            None => (row.progress(), Badge::Working),
        };
        p.text(
            r.left_top() + vec2(28.0, 26.0),
            Align2::LEFT_TOP,
            middle_truncate(&sub, 46),
            egui::FontId::proportional(CAPTION),
            if row.error.is_some() { t.critical } else { t.text_secondary },
        );
        paint_badge(ui, r.left_top() + vec2(12.0, 16.0), &badge, t);
        if let Some(e) = &row.error {
            resp.clone().on_hover_text(format!("{e}\n\nDetails: {}", e.details()));
        }
        // × on hover cancels (or dismisses a failed row).
        if resp.hovered() || row.error.is_some() {
            let x = Rect::from_center_size(pos2(r.right() - 12.0, r.top() + 16.0), vec2(22.0, 22.0));
            let xr = ui.interact(x, egui::Id::new(("lm-cancel", row.key)), Sense::click());
            crate::a11y::button(&xr, format!("{} {}", if row.error.is_some() { "Dismiss" } else { "Cancel" }, row.name()));
            ui.painter().text(
                x.center(),
                Align2::CENTER_CENTER,
                icon::X,
                theme::icon_font(14.0),
                if xr.hovered() { t.text_primary } else { t.text_secondary },
            );
            if xr.clicked() {
                cancel = Some(row.key);
            }
            xr.on_hover_text(if row.error.is_some() { "Dismiss" } else { "Cancel" });
        }
    }
    if let Some(k) = cancel {
        shell.ws.cancel(k);
    }
}

fn layer_rows(shell: &mut Shell, ui: &mut Ui, t: &Theme) {
    let n = shell.ws.doc.layers.len();
    let ids: Vec<LayerId> = shell.ws.doc.layers.iter().rev().map(|l| l.id).collect();
    let mut rects: Vec<Rect> = Vec::with_capacity(n);
    for (d, id) in ids.iter().enumerate() {
        let Some(l) = shell.ws.doc.layer(*id).cloned() else { continue };
        // While one of its joined tables is picked, that row is the highlighted one.
        let child_focused = shell.join_focus.as_ref().is_some_and(|(p, _)| p == id);
        let selected = shell.ws.selected() == Some(*id) && !child_focused;
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &l.display_name));
        crate::a11y::describe(&resp, &crate::a11y::spoken(&l.profile.subtitle()));
        let p = ui.painter();
        if selected {
            p.rect_filled(r, CornerRadius::same(theme::RADIUS_CONTROL), t.accent_weak);
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(theme::RADIUS_CONTROL), t.surface_raised);
        }
        // Eye: visibility only.
        let eye = Rect::from_center_size(r.left_center() + vec2(12.0, 0.0), vec2(24.0, 24.0));
        let eye_r = ui.interact(eye, egui::Id::new(("lm-eye", id.0)), Sense::click());
        crate::a11y::toggle(&eye_r, format!("Show {}", l.display_name), l.visible);
        ui.painter().text(
            eye.center(),
            Align2::CENTER_CENTER,
            if l.visible { icon::EYE } else { icon::EYE_SLASH },
            theme::icon_font(16.0),
            if l.visible { t.text_primary } else { t.text_secondary },
        );
        if eye_r.clicked() {
            let tx = shell.ws.doc.set_visible_tx(*id, !l.visible).map(|mut x| {
                x.label = format!("{} {}", if l.visible { "Hide" } else { "Show" }, l.display_name);
                x
            });
            shell.ws.commit(tx);
        }
        // Swatch.
        let sw = Rect::from_center_size(r.left_center() + vec2(36.0, 0.0), vec2(14.0, 14.0));
        let key = color(l.style.key_color(l.profile.kind));
        match l.profile.kind {
            LayerKind::Points => {
                ui.painter().circle(sw.center(), 5.0, key, Stroke::new(1.0, Color32::WHITE));
            }
            LayerKind::Lines => {
                ui.painter().line_segment([sw.left_bottom(), sw.right_top()], Stroke::new(2.0, key));
            }
            LayerKind::Raster => {
                for i in 0..4 {
                    let c = [
                        Color32::from_rgb(0x44, 0x01, 0x54),
                        Color32::from_rgb(0x3B, 0x52, 0x8B),
                        Color32::from_rgb(0x21, 0x91, 0x8C),
                        Color32::from_rgb(0xFD, 0xE7, 0x25),
                    ][i];
                    let x0 = sw.left() + sw.width() * i as f32 / 4.0;
                    ui.painter().rect_filled(Rect::from_min_max(pos2(x0, sw.top()), pos2(x0 + sw.width() / 4.0, sw.bottom())), 0.0, c);
                }
            }
            LayerKind::Table => {
                ui.painter().text(sw.center(), Align2::CENTER_CENTER, icon::TABLE, theme::icon_font(14.0), t.text_secondary);
            }
            LayerKind::Areas => {
                ui.painter().rect(sw, CornerRadius::same(3), key.gamma_multiply(0.5), Stroke::new(1.0, key), egui::StrokeKind::Inside);
            }
        }
        // Name, or the rename field.
        let name_rect = Rect::from_min_max(pos2(r.left() + 50.0, r.top()), pos2(r.right() - 26.0, r.bottom()));
        if shell.rename.as_ref().is_some_and(|(rid, _)| rid == id) {
            // Return or a click elsewhere keeps the new name; Esc keeps the old one. The field takes
            // the keyboard once, when renaming starts (taking it every frame meant it never ended).
            let mut done = false;
            let mut cancel = false;
            let started = shell.rename_started;
            if let Some((_, text)) = shell.rename.as_mut() {
                let te = ui.put(name_rect.shrink2(vec2(0.0, 4.0)), egui::TextEdit::singleline(text).font(egui::FontId::proportional(BODY)));
                if started {
                    te.request_focus();
                }
                let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
                cancel = esc;
                done = te.lost_focus() || (te.has_focus() && enter);
            }
            shell.rename_started = false;
            if cancel {
                shell.rename = None;
            } else if done && let Some((rid, text)) = shell.rename.take() {
                {
                    let name = text.trim().to_string();
                    if !name.is_empty() && name != l.display_name {
                        let tx = shell.ws.doc.rename_tx(rid, name.clone()).map(|mut x| {
                            x.label = format!("Rename {}", l.display_name);
                            x
                        });
                        shell.ws.commit(tx);
                    }
                }
            }
        } else {
            ui.painter().text(
                name_rect.left_center(),
                Align2::LEFT_CENTER,
                middle_truncate(&l.display_name, 26),
                egui::FontId::proportional(BODY),
                if l.visible { t.text_primary } else { t.text_secondary },
            );
        }
        // Badge.
        let badge = if shell.ws.is_busy(*id) {
            Badge::Working
        } else if l.profile.open_questions().next().is_some() {
            Badge::Decision
        } else if l.profile.visible_assumptions().next().is_some() {
            Badge::Assumption
        } else {
            Badge::None
        };
        paint_badge(ui, pos2(r.right() - 12.0, r.center().y), &badge, t);
        // A filtered layer shows a funnel (WP3.7).
        let filtered = l.filter.as_ref().is_some_and(|f| !f.is_empty());
        if filtered {
            ui.painter().text(pos2(r.right() - 30.0, r.center().y), Align2::CENTER_CENTER, icon::FUNNEL, theme::icon_font(14.0), t.accent);
        }
        // Joined tables: a chevron, in the next free place from the right, shows or hides them.
        let subs = shell.ws.sublayers(*id);
        let open = shell.ws.sublayers_open(*id);
        if !subs.is_empty() {
            let x = if filtered {
                r.right() - 48.0
            } else if matches!(badge, Badge::None) {
                r.right() - 12.0
            } else {
                r.right() - 30.0
            };
            let c = Rect::from_center_size(pos2(x, r.center().y), vec2(20.0, 24.0));
            let cr = ui.interact(c, egui::Id::new(("lm-sublayers", id.0)), Sense::click());
            crate::a11y::toggle(&cr, format!("Joined tables of {}", l.display_name), open);
            ui.painter().text(
                c.center(),
                Align2::CENTER_CENTER,
                if open { icon::CARET_DOWN } else { icon::CARET_RIGHT },
                theme::icon_font(13.0),
                if cr.hovered() { t.text_primary } else { t.text_secondary },
            );
            if cr.clicked() {
                shell.ws.set_sublayers_open(*id, !open);
            }
            let n = subs.len();
            cr.on_hover_text(match (open, n) {
                (true, 1) => "Hide the joined table".to_string(),
                (true, _) => format!("Hide the {n} joined tables"),
                (false, 1) => "Show the joined table".to_string(),
                (false, _) => format!("Show the {n} joined tables"),
            });
        }
        if resp.clicked() {
            shell.join_focus = None;
            // ⌘-click adds the layer to the selection as a companion (the expert path).
            let cmd = ui.input(|i| i.modifiers.command);
            match shell.ws.selected() {
                Some(active) if cmd && active != *id => {
                    if !shell.ws.selection.layers.contains(id) {
                        shell.ws.selection.layers.push(*id);
                    }
                }
                _ => shell.ws.select_layer(*id),
            }
        }
        // Double-click zooms to the layer (owner's choice: done far more often than renaming).
        if resp.double_clicked() {
            shell.zoom_to = shell.ws.extent(*id);
        }
        resp.clone().on_hover_text(l.profile.subtitle());
        // Drag to reorder.
        if resp.drag_started() {
            shell.drag_from = Some(d);
        }
        resp.context_menu(|ui| {
            if l.profile.kind != LayerKind::Raster && menu_item(ui, t, icon::TABLE, "View data").clicked() {
                shell.table_for = Some(*id);
                ui.close();
            }
            if l.profile.kind == LayerKind::Table && menu_item(ui, t, icon::LINK, "Match to a layer…").clicked() {
                crate::join_ui::open_picker(shell, *id);
                ui.close();
            }
            if menu_item(ui, t, icon::PAINT_BRUSH, "Style…").clicked() {
                shell.ws.select_layer(*id);
                shell.inspector_tab = 1;
                ui.close();
            }
            if menu_item(ui, t, icon::FRAME_CORNERS, "Zoom to layer").clicked() {
                shell.zoom_to = shell.ws.extent(*id);
                ui.close();
            }
            if menu_item(ui, t, icon::PENCIL_SIMPLE, "Rename").clicked() {
                shell.rename = Some((*id, l.display_name.clone()));
                shell.rename_started = true;
                ui.close();
            }
            ui.separator();
            if menu_item(ui, t, icon::MINUS_CIRCLE, "Remove from map").clicked() {
                let tx = shell.ws.doc.remove_layer_tx(*id).map(|mut x| {
                    x.label = format!("Remove {}", l.display_name);
                    x
                });
                shell.ws.commit(tx);
                if shell.ws.selected() == Some(*id) {
                    shell.ws.clear_selection();
                }
                ui.close();
            }
        });
        // Joined tables, indented under the layer (owner's request, 2026-10-18).
        let mut block = r;
        // A grid's variable (and level) pop-ups under its name (WP6.12).
        if l.profile.grid.is_some() {
            block = block.union(grid_pickers(shell, ui, t, &l));
        }
        if open {
            for s in &subs {
                block = block.union(sublayer_row(shell, ui, t, &l, s));
            }
        }
        rects.push(block);
        // A question card attaches to its row; the map stays usable. While the layer's overview
        // is open it shows the card there instead, and the row just says so.
        if let Some(q) = l.profile.open_questions().next().cloned() {
            if shell.ws.selected() == Some(*id) {
                ui.label(RichText::new("Needs a decision: see the layer's panel").size(CAPTION).color(t.warning));
            } else {
                question_card(shell, ui, t, *id, &q);
            }
        }
    }
    // The insertion line while dragging; drop on release.
    if let Some(from) = shell.drag_from {
        let y = ui.input(|i| i.pointer.interact_pos()).map_or(0.0, |p| p.y);
        let to = rects.iter().position(|r| y < r.center().y).unwrap_or(rects.len());
        let line_y = rects.get(to).map_or_else(|| rects.last().map_or(0.0, |r| r.bottom()), |r| r.top());
        if let (Some(first), true) = (rects.first(), to != from && to != from + 1) {
            ui.painter().line_segment([pos2(first.left(), line_y), pos2(first.right(), line_y)], Stroke::new(2.0, t.accent));
        }
        if ui.input(|i| i.pointer.any_released()) {
            shell.drag_from = None;
            let target = if to > from { to - 1 } else { to };
            if target != from && n > 0 {
                // Display order is top to bottom; the document lists bottom to top.
                let tx = shell.ws.doc.move_tx(n - 1 - from, n - 1 - target).map(|mut x| {
                    x.label = "Reorder layers".into();
                    x
                });
                shell.ws.commit(tx);
            }
        }
    }
}

/// A macOS pop-up button: the current choice at the left, the up–down chevron at the right, on
/// the raised fill with the control edge. Clicking opens its menu (see [`grid_pickers`]).
fn popup_button(ui: &mut Ui, t: &Theme, text: &str, tip: &str, width: f32, enabled: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let edge = if enabled && resp.hovered() { t.text_secondary } else { t.control_stroke };
    p.rect(r, CornerRadius::same(theme::RADIUS_CONTROL), t.surface, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    let color = if enabled { t.text_primary } else { t.text_secondary };
    let galley = p.layout_no_wrap(middle_truncate(text, ((width - 34.0) / 6.6) as usize), egui::FontId::proportional(CAPTION + 1.0), color);
    p.galley(pos2(r.left() + 8.0, r.center().y - galley.size().y / 2.0), galley, color);
    if enabled {
        p.text(pos2(r.right() - 11.0, r.center().y), Align2::CENTER_CENTER, icon::CARET_UP_DOWN, theme::icon_font(12.0), t.text_secondary);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, enabled, tip));
    resp.on_hover_text(tip)
}

/// One item of a pop-up's menu, `width` wide: a checkmark before the current choice, as macOS
/// menus show it (the other items keep its space, so the labels line up).
fn choice_item(ui: &mut Ui, t: &Theme, label: &str, on: bool, width: f32) -> egui::Response {
    let mut job = egui::text::LayoutJob::default();
    let format = |font, color| egui::TextFormat { font_id: font, color, valign: egui::Align::Center, ..Default::default() };
    job.append(icon::CHECK, 0.0, format(theme::icon_font(13.0), if on { t.text_primary } else { Color32::TRANSPARENT }));
    job.append(label, 8.0, format(egui::FontId::proportional(BODY), t.text_primary));
    let resp = ui.add(egui::Button::new(job).frame(false).min_size(vec2(width, 24.0)));
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label));
    resp
}

/// The width of a pop-up's menu: its widest choice, at least the button's width.
fn menu_width(ui: &Ui, labels: impl Iterator<Item = String>, min: f32) -> f32 {
    let widest =
        labels.map(|l| ui.painter().layout_no_wrap(l, egui::FontId::proportional(BODY), Color32::WHITE).size().x).fold(0.0, f32::max);
    (widest + 13.0 + 8.0 + 2.0 * ui.spacing().button_padding.x + 4.0).max(min).min(420.0)
}

/// A grid layer's variable pop-up ("2 m temperature (°C)") under its name, and, for a variable
/// with levels, the level pop-up under it. Picking redraws the layer, as one undo step.
fn grid_pickers(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &lm_core::document::DocLayer) -> Rect {
    let Some(g) = l.profile.grid.clone() else { return Rect::NOTHING };
    let id = l.id;
    let busy = shell.ws.is_busy(id);
    let width = (ui.available_width() - 58.0).max(80.0);
    let shown = g.shown().cloned();
    let label = shown.as_ref().map_or_else(|| g.variable.clone(), |v| v.label());
    let mut out = Rect::NOTHING;
    let mut pick: Option<(String, usize)> = None;
    let r = ui.horizontal(|ui| {
        ui.add_space(50.0);
        // A file with one variable just names it.
        if g.variables.len() < 2 {
            ui.label(RichText::new(middle_truncate(&label, 34)).size(CAPTION).color(t.text_secondary)).on_hover_text("Variable shown");
            return;
        }
        let resp = popup_button(ui, t, &label, "Variable shown", width, !busy);
        let w = menu_width(ui, g.variables.iter().map(|v| v.label()), width);
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_width(w);
            for v in &g.variables {
                if choice_item(ui, t, &v.label(), v.name == g.variable, w).clicked() && v.name != g.variable {
                    pick = Some((v.name.clone(), 0));
                    ui.close();
                }
            }
        });
    });
    out = out.union(r.response.rect);
    if let Some(v) = shown.filter(|v| v.levels.as_ref().is_some_and(|lv| lv.values.len() > 1)) {
        let n = v.levels.as_ref().map_or(0, |lv| lv.values.len());
        let r = ui.horizontal(|ui| {
            ui.add_space(50.0);
            let resp = popup_button(ui, t, &v.level_label(g.level), "Level shown", width, !busy);
            let w = menu_width(ui, (0..n.min(200)).map(|k| v.level_label(k)), width);
            egui::Popup::menu(&resp).show(|ui| {
                ui.set_width(w);
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for k in 0..n {
                        if choice_item(ui, t, &v.level_label(k), k == g.level, w).clicked() && k != g.level {
                            pick = Some((v.name.clone(), k));
                            ui.close();
                        }
                    }
                });
            });
        });
        out = out.union(r.response.rect);
    }
    if let Some((var, level)) = pick
        && let Err(e) = shell.ws.choose_variable(id, &var, level)
    {
        let now = ui.input(|i| i.time);
        shell.set_notice(e, now);
    }
    ui.add_space(2.0);
    out
}

/// A joined table's row under its layer: the table glyph, its name and how many of the layer's
/// records it matched. A click shows the join in the inspector (its card while it is the latest
/// join, otherwise its fields in Details); the menu has Unjoin, which puts the table back.
fn sublayer_row(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &lm_core::document::DocLayer, s: &crate::workspace::Sublayer) -> Rect {
    let parent = l.id;
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), SUBLAYER_HEIGHT), Sense::click());
    let focused = shell.ws.selected() == Some(parent) && shell.join_focus.as_ref().is_some_and(|(p, n)| *p == parent && *n == s.table);
    let x = r.left() + SUBLAYER_INDENT;
    let bg = Rect::from_min_max(pos2(x + 24.0, r.top()), r.max);
    if focused {
        ui.painter().rect_filled(bg, CornerRadius::same(theme::RADIUS_CONTROL), t.accent_weak);
    } else if resp.hovered() {
        ui.painter().rect_filled(bg, CornerRadius::same(theme::RADIUS_CONTROL), t.surface_raised);
    }
    let p = ui.painter();
    p.text(pos2(x + 36.0, r.center().y), Align2::CENTER_CENTER, icon::TABLE, theme::icon_font(15.0), t.text_secondary);
    let text = if l.visible { t.text_primary } else { t.text_secondary };
    p.text(pos2(x + 50.0, r.top() + 4.0), Align2::LEFT_TOP, middle_truncate(&s.table, 24), egui::FontId::proportional(BODY), text);
    let caption = match s.matched {
        Some((m, n)) => format!("joined · {} of {} matched", group_thousands(m as u64), group_thousands(n as u64)),
        None => "joined".into(),
    };
    p.text(pos2(x + 50.0, r.top() + 22.0), Align2::LEFT_TOP, &caption, egui::FontId::proportional(CAPTION), t.text_secondary);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            focused,
            format!("Joined table {}, {}", s.table, crate::a11y::spoken(&caption)),
        )
    });
    let show = |shell: &mut Shell, details: bool| {
        shell.ws.select_layer(parent);
        shell.join_focus = Some((parent, s.table.clone()));
        shell.join_scroll = true;
        let card = shell.ws.join_report(parent).is_some_and(|r| r.table_name == s.table);
        shell.inspector_tab = if card && !details { 0 } else { 2 };
    };
    if resp.clicked() {
        show(shell, false);
    }
    resp.clone().on_hover_text(format!("Joined to {}: its fields are part of the layer", l.display_name));
    resp.context_menu(|ui| {
        if menu_item(ui, t, icon::LIST_BULLETS, "Show joined fields").clicked() {
            show(shell, true);
            ui.close();
        }
        ui.separator();
        let unjoin = ui.add_enabled_ui(s.can_unjoin, |ui| menu_item(ui, t, icon::LINK_BREAK, "Unjoin")).inner;
        if unjoin.on_disabled_hover_text("The table wasn't opened in this session, so it can't be put back").clicked() {
            shell.ws.unjoin(parent, &s.table);
            shell.join_focus = None;
            ui.close();
        }
    });
    r
}

/// "Where should this data be?" (WP2.4): name a place (or use the map's centre); the coordinate
/// systems that put the data near it are listed nearest first; Open as a table is the Not sure.
fn place_card(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId, shown: f64, now: f64) {
    let key = egui::Id::new(("lm-place", id.0));
    let mut query: String = ui.data(|d| d.get_temp::<String>(key)).unwrap_or_default();
    let place_key = egui::Id::new(("lm-place-at", id.0));
    let mut place: Option<(f64, f64, String)> = ui.data(|d| d.get_temp(place_key));
    egui::Frame::new()
        .fill(t.surface_raised)
        .stroke(Stroke::new(1.0, t.warning.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(theme::RADIUS_CARD))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(icon::WARNING).font(theme::icon_font(14.0)).color(t.warning));
                ui.label(RichText::new("Where should this data be?").font(semibold(BODY)));
            });
            ui.label(
                RichText::new("This file doesn't say where on Earth its coordinates are. Name a place it covers.")
                    .size(CAPTION)
                    .color(t.text_secondary),
            );
            let te = ui.add(egui::TextEdit::singleline(&mut query).hint_text("A city or town, e.g. Shanghai"));
            let _ = te;
            for r in shell.search.lookup(&query, 4) {
                if ui.selectable_label(false, format!("{}  ·  {}", r.title, r.detail)).clicked() {
                    place = Some((r.choice.lon, r.choice.lat, r.title.clone()));
                    query = r.title.clone();
                }
            }
            if ui.link("Use the middle of the map").clicked() {
                let c = shell.map.center_lonlat();
                place = Some((c.x, c.y, "the middle of the map".into()));
            }
            if let Some((lon, lat, name)) = &place {
                let cands = shell.ws.place_candidates(id, (*lon, *lat));
                ui.add_space(4.0);
                if cands.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "No coordinate system puts this data near {name}. Try another place, or open it as a table."
                        ))
                        .color(t.text_secondary),
                    );
                }
                for c in cands {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{} — lands {} km from {name}",
                            c.name,
                            if c.distance_km < 10.0 { format!("{:.1}", c.distance_km) } else { format!("{:.0}", c.distance_km) }
                        ));
                        if ui.add(egui::Button::new(RichText::new("Use this").color(t.on_accent)).fill(t.accent)).clicked()
                            && let Err(e) = shell.ws.place_layer(id, c.epsg, Some(now - shown))
                        {
                            shell.set_notice(e, now);
                        }
                    });
                }
            }
            if ui.link("Open as a table").clicked() {
                let _ =
                    shell.ws.change_assumption(id, AssumptionKind::NoCrsProjected, Subject::Layer, Choice::not_sure(), Some(now - shown));
            }
        });
    ui.data_mut(|d| {
        d.insert_temp(key, query);
        if let Some(p) = place {
            d.insert_temp(place_key, p);
        }
    });
}

/// A question card: the question, the recommended answer preselected, Not sure, and Use this.
fn question_card(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId, q: &Assumption) {
    let now = ui.input(|i| i.time);
    let shown = *shell.question_seen.entry((id.0, q.kind as u8)).or_insert(now);
    if q.kind == AssumptionKind::NoCrsProjected {
        place_card(shell, ui, t, id, shown, now);
        return;
    }
    egui::Frame::new()
        .fill(t.surface_raised)
        .stroke(Stroke::new(1.0, t.warning.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(theme::RADIUS_CARD))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(icon::WARNING).font(theme::icon_font(14.0)).color(t.warning));
                ui.label(RichText::new(&q.line).font(semibold(BODY)));
            });
            // Radio buttons show their circles (resting controls have no outline elsewhere).
            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, t.text_secondary);
            let key = egui::Id::new(("lm-q", id.0, q.kind as u8));
            let mut pick: String = ui.data(|d| d.get_temp::<String>(key)).unwrap_or_else(|| q.chosen.id.clone());
            for c in &q.alternatives {
                let label = if c.id == q.chosen.id && c.id != "not_sure" { format!("{} (recommended)", c.label) } else { c.label.clone() };
                ui.radio_value(&mut pick, c.id.clone(), label);
            }
            if !q.alternatives.iter().any(|c| c.id == "not_sure") {
                ui.radio_value(&mut pick, "not_sure".to_string(), "Not sure");
            }
            ui.data_mut(|d| d.insert_temp(key, pick.clone()));
            if q.kind == AssumptionKind::ChinaOffsetNamed && ui.link("Compare on the map first").clicked() {
                shell.begin_compare(id, q.chosen.id == "bd09");
            }
            if ui.add(egui::Button::new(RichText::new("Use this").color(t.on_accent)).fill(t.accent)).clicked() {
                let choice = q.alternatives.iter().find(|c| c.id == pick).cloned().unwrap_or_else(Choice::not_sure);
                let result = shell.ws.change_assumption(id, q.kind, q.subject.clone(), choice, Some(now - shown));
                if let Err(e) = result {
                    shell.set_notice(e, now);
                }
            }
        });
}

/// The inspector: the selected layer's overview (Overview · Details).
pub fn inspector(shell: &mut Shell, ui: &mut Ui, map: Rect) {
    let t = *shell.theme();
    let Some(id) = shell.ws.selected() else { return };
    let Some(l) = shell.ws.doc.layer(id).cloned() else {
        shell.ws.clear_selection();
        return;
    };
    let pos = pos2(map.right() - PANEL_MARGIN - INSPECTOR_WIDTH, map.top() + PANEL_MARGIN);
    let max_h = map.height() - 2.0 * PANEL_MARGIN - 64.0;
    let mut close = false;
    // What the card shows: one object, several, a raster cell, or the layer.
    let rows = shell.ws.selection.rows_in(id);
    let pixel = shell.ws.selection.pixel.clone();
    // While a search runs (WP3.10) the panel stays the layer's, listing the matches.
    let finding = shell.find.active_for(id);
    let (title, subtitle) = if finding {
        (l.display_name.clone(), l.profile.subtitle())
    } else {
        match (rows.as_slice(), &pixel) {
            ([row], _) => crate::inspect::object_header(shell, &l, *row),
            ([], Some(p)) => (format!("{:.4}°, {:.4}°", p.lat, p.lon), format!("{} · cell", l.display_name)),
            ([], None) => (l.display_name.clone(), l.profile.subtitle()),
            (many, _) => (format!("{} selected", group_thousands(many.len() as u64)), l.display_name.clone()),
        }
    };
    egui::Area::new(egui::Id::new("lm-inspector")).order(egui::Order::Middle).fixed_pos(pos).show(ui.ctx(), |ui| {
        frame(&t).show(ui, |ui| {
            ui.set_width(INSPECTOR_WIDTH - 2.0 * theme::PANEL_PADDING);
            ui.set_max_width(INSPECTOR_WIDTH - 2.0 * theme::PANEL_PADDING);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(INSPECTOR_WIDTH - 2.0 * theme::PANEL_PADDING - 40.0);
                    ui.add(egui::Label::new(RichText::new(&title).font(semibold(theme::HEADLINE))).wrap());
                    ui.label(RichText::new(&subtitle).size(CAPTION).color(t.text_secondary));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    let x = ui.add(egui::Button::new(RichText::new(icon::X).font(theme::icon_font(14.0))).frame(false));
                    crate::a11y::button(&x, "Close the panel");
                    if x.on_hover_text("Close").clicked() {
                        close = true;
                    }
                });
            });
            let object_view = (!rows.is_empty() || pixel.is_some()) && !finding;
            // Find in a layer: at the top of the layer's panel, above the tabs.
            if !object_view && l.profile.kind != lm_core::model::LayerKind::Raster && shell.flags.shows(3) {
                ui.add_space(8.0);
                crate::find_ui::bar(shell, ui, &t, &l);
            }
            if !object_view && !finding {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let tabs: &[(u8, &str)] = if shell.flags.shows(3) {
                        &[(0, "Overview"), (1, "Style"), (2, "Details")]
                    } else {
                        &[(0, "Overview"), (2, "Details")]
                    };
                    for &(tab, name) in tabs {
                        if ui.selectable_label(shell.inspector_tab == tab, name).clicked() {
                            shell.inspector_tab = tab;
                        }
                    }
                });
            } else if rows.len() > 1 && !finding {
                // The selection chip: "3 selected · Clear".
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} selected", group_thousands(rows.len() as u64))).size(CAPTION));
                    if ui.link("Clear").clicked() {
                        shell.ws.selection.features.clear();
                    }
                });
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(max_h - 90.0)
                .min_scrolled_height((max_h - 90.0).min(560.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if shell.composer.as_ref().is_some_and(|c| c.main == id) {
                        crate::analysis::composer(shell, ui, &t, &l);
                    } else if finding {
                        crate::find_ui::results(shell, ui, &t, &l);
                    } else if shell.match_draft.as_ref().is_some_and(|d| d.table == id) {
                        crate::join_ui::picker(shell, ui, &t, &l);
                    } else if let Some((cid, field)) = shell.compare_peers.clone().filter(|(c, _)| *c == id) {
                        ranking(shell, ui, &t, &l, field.as_deref(), rows.first().copied());
                        if ui.link("Back").clicked() {
                            shell.compare_peers = None;
                        }
                        let _ = cid;
                    } else if let [row] = rows.as_slice() {
                        crate::inspect::object_body(shell, ui, &t, &l, *row);
                    } else if let Some(p) = &pixel {
                        pixel_body(shell, ui, &t, &l, p);
                    } else if !rows.is_empty() {
                        crate::insight::actions(shell, ui, &t, &l, None);
                    } else {
                        match shell.inspector_tab {
                            0 => overview(shell, ui, &t, id, &l),
                            1 => crate::inspect::style_tab(shell, ui, &t, &l),
                            _ => details(shell, ui, &t, id, &l),
                        }
                    }
                });
        });
    });
    // The Esc ladder (Keyboard): popovers first, then the object selection, then the inspector.
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if close {
        shell.ws.clear_selection();
        shell.compare_peers = None;
    } else if esc && shell.change_for.is_none() && shell.chooser.is_none() && shell.filter_draft.is_none() {
        if shell.composer.is_some() {
            // Esc discards a preview and closes the composer, but never cancels a running job.
            if !shell.ws.plan_running() {
                crate::analysis::close(shell);
            }
        } else if shell.compare_peers.is_some() {
            shell.compare_peers = None;
        } else if shell.match_draft.is_some() {
            shell.match_draft = None;
        } else {
            shell.ws.selection.step_back();
        }
    }
}

/// A raster cell's card: its value with the unit, how it compares, and the cell's actions.
fn pixel_body(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &lm_core::document::DocLayer, p: &lm_core::selection::PixelPick) {
    let Some(lm_jobs::import::LayerData::Raster(r)) = shell.ws.data(l.id).map(|d| d.data.clone()) else { return };
    let band = lm_jobs::inspect::style_band(&r, &l.style);
    let unit = l.profile.bands.get(band).and_then(|b| b.unit.value.clone());
    // A grid's cell is read again from the time step shown now (WP6.12), and named by its
    // variable and date rather than a band number.
    let (value, what) = match &l.profile.grid {
        Some(g) => (lm_jobs::inspect::raster_value_at(&r, band, p.lon, p.lat).unwrap_or(None), g.caption(shell.ws.shown_time(l.id))),
        None => (p.value, format!("Band {}", band + 1)),
    };
    match value {
        Some(v) => {
            let (value, phrase) = lm_jobs::inspect::pixel_insight(&r, band, v, unit.as_deref());
            ui.label(RichText::new(value).font(semibold(theme::TITLE)));
            ui.label(RichText::new(what).size(CAPTION).color(t.text_secondary));
            if let Some(ph) = phrase {
                ui.label(ph);
            }
        }
        None => {
            ui.label(RichText::new("No value").font(semibold(theme::TITLE)).color(t.text_secondary));
            ui.label(RichText::new("This cell has no data.").color(t.text_secondary));
        }
    }
    crate::insight::actions(shell, ui, t, l, None);
}

/// "Compare with other [Districts]": every object ranked by one field, the selected one marked.
fn ranking(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &lm_core::document::DocLayer, field: Option<&str>, row: Option<usize>) {
    let Some(ds) = crate::inspect::dataset(shell, l.id) else { return };
    let measures: Vec<String> = l
        .profile
        .fields
        .iter()
        .filter(|f| matches!(l.role(f).value, lm_core::model::FieldRole::Measure(_)))
        .map(|f| f.name.clone())
        .collect();
    let Some(mut field) = field.map(String::from).or_else(|| measures.first().cloned()) else {
        ui.label(RichText::new("This layer has no values to compare.").color(t.text_secondary));
        return;
    };
    ui.horizontal(|ui| {
        ui.label("Compare by");
        egui::ComboBox::from_id_salt(("lm-rank-field", l.id.0)).selected_text(&field).show_ui(ui, |ui| {
            for m in &measures {
                ui.selectable_value(&mut field, m.clone(), m);
            }
        });
    });
    shell.compare_peers = Some((l.id, Some(field.clone())));
    let mask = shell.ws.derived(l.id).and_then(|d| d.mask.clone());
    let ranked = lm_jobs::inspect::ranking(l, &ds, &field, mask.as_deref().map(|m| m.as_slice()));
    let Some(max) = ranked.iter().map(|r| r.3.abs()).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))) else { return };
    ui.label(
        RichText::new(format!("{} {}, highest first", group_thousands(ranked.len() as u64), lm_jobs::inspect::noun_plural(l)))
            .size(CAPTION)
            .color(t.text_secondary),
    );
    let w = ui.available_width();
    egui::ScrollArea::vertical().id_salt(("lm-rank", l.id.0)).max_height(420.0).show_rows(ui, 22.0, ranked.len(), |ui, range| {
        for i in range {
            let Some((r, name, value, v)) = ranked.get(i) else { continue };
            let me = Some(*r) == row;
            let (rr, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
            resp.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    me,
                    crate::a11y::spoken(&format!("{}. {name}: {value}", i + 1)),
                )
            });
            let p = ui.painter();
            let frac = if max > 0.0 { (v.abs() / max) as f32 } else { 0.0 };
            let bar = egui::Rect::from_min_size(rr.min + vec2(0.0, 3.0), vec2((w * 0.45) * frac, 16.0));
            p.rect_filled(bar, 3.0, if me { t.accent } else { t.accent_weak });
            p.text(
                rr.left_center() + vec2(w * 0.47, 0.0),
                Align2::LEFT_CENTER,
                middle_truncate(name, 18),
                theme::regular(CAPTION),
                if me { t.accent } else { t.text_primary },
            );
            p.text(rr.right_center(), Align2::RIGHT_CENTER, value, theme::regular(CAPTION), t.text_secondary);
            if resp.clicked() {
                shell.ws.selection = lm_core::selection::SelectionContext::object(lm_core::selection::FeatureRef { layer: l.id, row: *r });
            }
        }
    });
}

/// The overlap chooser (Inspector: object card): "3 objects here", topmost first; hovering a
/// row highlights its object, clicking selects it.
pub fn chooser(shell: &mut Shell, ui: &mut Ui) {
    let Some((at, hits)) = shell.chooser.clone() else { return };
    let t = *shell.theme();
    let mut pick = None;
    let mut hover = None;
    let resp =
        egui::Area::new(egui::Id::new("lm-chooser")).order(egui::Order::Foreground).fixed_pos(at + vec2(8.0, 8.0)).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(280.0);
                // Hit testing keeps the nearest 100 per layer; more than that reads "100+".
                let many = if hits.len() >= lm_mapview::hit::MAX_HITS_PER_LAYER { "+" } else { "" };
                ui.label(RichText::new(format!("{}{many} objects here", group_thousands(hits.len() as u64))).font(semibold(BODY)));
                for f in hits.iter().take(20) {
                    let Some(l) = shell.ws.doc.layer(f.layer) else { continue };
                    let (title, _) = crate::inspect::object_header(shell, l, f.row);
                    let r = ui.horizontal(|ui| {
                        crate::inspect::swatch(ui, l.profile.kind, l.style.key_color(l.profile.kind), 7.0);
                        ui.add(
                            egui::Button::new(format!("{} · {}", middle_truncate(&title, 22), middle_truncate(&l.display_name, 14)))
                                .frame(false),
                        )
                    });
                    if r.inner.hovered() {
                        hover = Some(*f);
                    }
                    if r.inner.clicked() {
                        pick = Some(*f);
                    }
                }
                if hits.len() > 20 {
                    ui.label(RichText::new(format!("and {} more", hits.len() - 20)).size(CAPTION).color(t.text_secondary));
                }
            });
        });
    shell.chooser_hover = hover;
    shell.ws.hover = hover;
    if let Some(f) = pick {
        shell.ws.selection = lm_core::selection::SelectionContext::object(f);
        shell.chooser = None;
        shell.chooser_hover = None;
    } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.chooser = None;
        shell.chooser_hover = None;
    }
    let _ = resp;
}

fn status_lines(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId, l: &lm_core::document::DocLayer) {
    let p = &l.profile;
    // The count is what was placed; exclusions are the records left out.
    let opened = p.count;
    if !p.exclusions.is_empty() {
        let (one, many) = p.kind.nouns();
        let _ = one;
        let rec = if p.kind == LayerKind::Table { many } else { "records" };
        ui.label(format!(
            "{} {rec} opened; {} {} no usable location.",
            group_thousands(opened),
            group_thousands(p.exclusions.len() as u64),
            if p.exclusions.len() == 1 { "has" } else { "have" }
        ));
        ui.horizontal(|ui| {
            if ui.link(format!("Review {}", group_thousands(p.exclusions.len() as u64))).clicked() {
                shell.review_for = Some(id);
            }
            if ui.link("Export problem list").clicked() {
                shell.export_problems(id);
            }
        });
    }
    if p.kind == LayerKind::Table && p.extent_wgs84.is_none() {
        ui.label(RichText::new("Opened as a table: there is nothing to place on the map yet.").color(t.text_secondary));
    }
}

fn assumption_line(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId, a: &Assumption) {
    ui.horizontal_wrapped(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 4.0, t.assumption);
        ui.label(&a.line);
        let link = match a.kind {
            AssumptionKind::ChinaOffsetPossible => "Compare",
            AssumptionKind::RasterNoUnit => "Add unit",
            _ => "Change",
        };
        let resp = ui.link(link);
        if resp.clicked() {
            if a.kind == AssumptionKind::ChinaOffsetPossible {
                shell.begin_compare(id, false);
            } else {
                shell.change_for = Some((id, a.kind, a.subject.clone(), resp.rect));
            }
        }
    });
}

/// The Compare banner (WP2.5): a pill 12 px below the toolbar while the Compare view is open.
pub fn compare_banner(shell: &mut Shell, ui: &mut Ui, map: Rect) {
    let t = *shell.theme();
    let Some((name, bd09)) =
        shell.ws.compare.as_ref().and_then(|c| shell.ws.doc.layer(c.layer).map(|l| (l.display_name.clone(), c.from_bd09)))
    else {
        return;
    };
    let mut action: Option<Option<bool>> = None;
    let mut switch = false;
    egui::Area::new(egui::Id::new("lm-compare"))
        .order(egui::Order::Foreground)
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(pos2(map.center().x, map.top() + PANEL_MARGIN))
        .show(ui.ctx(), |ui| {
            frame(&t).show(ui, |ui| {
                ui.set_max_width(560.0);
                ui.label(RichText::new(format!("Compare {name}")).font(semibold(BODY)));
                ui.horizontal_wrapped(|ui| {
                    let dot = |ui: &mut Ui, c: Color32| {
                        let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                        ui.painter().circle(r.center(), 5.0, c, Stroke::new(1.5, Color32::WHITE));
                    };
                    let key = shell
                        .ws
                        .doc
                        .layer(shell.ws.compare.as_ref().map_or(LayerId(0), |c| c.layer))
                        .map_or(t.accent, |l| color(l.style.key_color(l.profile.kind)));
                    dot(ui, key);
                    ui.label("as in the file");
                    dot(ui, Color32::from_rgb(0xD5, 0x5E, 0x00));
                    ui.label(if bd09 { "converted from Baidu (BD-09)" } else { "converted from Amap/Tencent (GCJ-02)" });
                });
                ui.label(RichText::new("Keep the set that sits on the streets.").size(CAPTION).color(t.text_secondary));
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new("Keep converted").color(t.on_accent)).fill(t.accent)).clicked() {
                        action = Some(Some(true));
                    }
                    if ui.button("Keep as in the file").clicked() {
                        action = Some(Some(false));
                    }
                    if ui.link(if bd09 { "Try Amap/Tencent instead" } else { "Try Baidu instead" }).clicked() {
                        switch = true;
                    }
                    if ui.link("Close").clicked() {
                        action = Some(None);
                    }
                });
            });
        });
    if switch && let Some(c) = shell.ws.compare.as_ref().map(|c| c.layer) {
        shell.begin_compare(c, !bd09);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = Some(None);
    }
    if let Some(keep) = action {
        let now = ui.input(|i| i.time);
        // Seconds since the layer's China question was first shown, for the question log.
        let seconds = shell.ws.compare.as_ref().and_then(|c| {
            [AssumptionKind::ChinaOffsetNamed, AssumptionKind::ChinaOffsetPossible]
                .iter()
                .find_map(|k| shell.question_seen.get(&(c.layer.0, *k as u8)).map(|t| now - t))
        });
        if let Err(e) = shell.ws.end_compare(keep, seconds) {
            shell.set_notice(e, now);
        }
    }
}

fn overview(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId, l: &lm_core::document::DocLayer) {
    // A source that moved since the map was saved (WP5.2): Locate… or Remove layer.
    crate::saving::missing_card(shell, ui, t, l);
    // Kept results (WP4.6): the card of the result that made this layer, and cards about it.
    crate::analysis::kept_cards(shell, ui, t, l);
    // Joining a table (WP3.11): the result on the joined layer, the question on a table.
    crate::join_ui::report(shell, ui, t, l);
    if l.profile.kind == lm_core::model::LayerKind::Table {
        crate::join_ui::table_status(shell, ui, t, id);
    }
    if let Some(q) = l.profile.open_questions().next().cloned() {
        question_card(shell, ui, t, id, &q);
        ui.add_space(8.0);
    }
    status_lines(shell, ui, t, id, l);
    let visible: Vec<Assumption> = l.profile.visible_assumptions().cloned().collect();
    if !visible.is_empty() {
        ui.add_space(6.0);
        let expanded_key = egui::Id::new(("lm-more", id.0));
        let expanded = ui.data(|d| d.get_temp::<bool>(expanded_key)).unwrap_or(false);
        for a in visible.iter().take(if expanded { usize::MAX } else { 2 }) {
            assumption_line(shell, ui, t, id, a);
        }
        if visible.len() > 2 && !expanded && ui.link(format!("+{} more", visible.len() - 2)).clicked() {
            ui.data_mut(|d| d.insert_temp(expanded_key, true));
        }
    }
    // A grid's own assumption lines (WP6.12): visible, with nothing to change to.
    for line in l.profile.grid.iter().flat_map(|g| g.lines.iter()) {
        ui.horizontal_wrapped(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, t.assumption);
            ui.label(line);
        });
    }
    for n in &l.profile.notices {
        ui.label(RichText::new(n).size(CAPTION).color(t.text_secondary));
    }
    // Legend, generated from the style (a table isn't drawn, so it has none).
    if l.profile.kind != lm_core::model::LayerKind::Table {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Legend").font(semibold(BODY)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Style…").clicked() {
                    shell.inspector_tab = 1;
                }
            });
        });
        crate::inspect::legend(shell, ui, t, l);
    }
    if let Some(f) = &l.filter {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(icon::FUNNEL).font(theme::icon_font(14.0)).color(t.accent));
            ui.label(RichText::new("Filtered:").color(t.accent));
            ui.label(f.conditions.iter().map(|c| c.chip()).collect::<Vec<_>>().join("; "));
            if ui.link("Clear").clicked() {
                let tx = shell.ws.doc.set_filter_tx(id, None);
                shell.ws.commit(tx);
            }
        });
    }
    // Field summary: up to five measures, each with its range and a 20-bin histogram.
    if let Some(index) = shell.ws.layer_index(id) {
        let measures: Vec<&lm_core::model::FieldProfile> =
            l.profile.fields.iter().filter(|f| index.measures.contains_key(&f.name)).take(5).collect();
        if !measures.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Values").font(semibold(BODY)));
            for f in measures {
                let unit = f.unit.value.clone();
                if let Some((range, counts)) = lm_jobs::inspect::field_summary(&index, &f.name, unit.as_deref()) {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width() - 70.0);
                            ui.label(&f.name);
                            ui.label(RichText::new(range).size(CAPTION).color(t.text_secondary));
                        });
                        histogram(ui, t, &counts);
                    });
                }
            }
        }
    }
    if let Some(g) = &l.profile.grid
        && let Some(b) = l.profile.bands.first()
        && let (Some(lo), Some(hi)) = (b.min, b.max)
    {
        // A grid's range is over the sampled time steps (WP6.12), with the time span.
        let unit = b.unit.value.clone().map(|u| format!(" {u}")).unwrap_or_default();
        let over =
            if g.has_time() { format!(" over {} of {} time steps", g.sampled, group_thousands(g.steps() as u64)) } else { String::new() };
        ui.label(format!(
            "{}: {} to {}{unit}{over}",
            g.shown().map_or_else(|| b.name.clone(), |v| v.long_name.clone().unwrap_or_else(|| v.name.clone())),
            fmt_num(lo),
            fmt_num(hi)
        ));
        if let Some(range) = g.time_range() {
            ui.label(RichText::new(format!("Time: {range}")).size(CAPTION).color(t.text_secondary));
        }
    }
    for b in l.profile.bands.iter().filter(|_| l.profile.grid.is_none()) {
        if let (Some(lo), Some(hi)) = (b.min, b.max) {
            let unit = b.unit.value.clone().map(|u| format!(" {u}")).unwrap_or_default();
            let name = if b.name.is_empty() || b.name.eq_ignore_ascii_case(&format!("band {}", b.index)) {
                String::new()
            } else {
                format!(" ({})", b.name)
            };
            ui.label(format!("Band {}{name}: {} to {}{unit}", b.index, fmt_num(lo), fmt_num(hi)));
        }
    }
    crate::insight::actions(shell, ui, t, l, None);
}

/// A 60 px histogram of 20 bins.
fn histogram(ui: &mut Ui, t: &Theme, counts: &[u64]) {
    let (r, _) = ui.allocate_exact_size(vec2(60.0, 24.0), Sense::hover());
    let max = counts.iter().copied().max().unwrap_or(0).max(1) as f32;
    let w = r.width() / counts.len().max(1) as f32;
    for (k, c) in counts.iter().enumerate() {
        let h = (*c as f32 / max) * r.height();
        let bar = egui::Rect::from_min_max(
            pos2(r.left() + k as f32 * w, r.bottom() - h),
            pos2(r.left() + (k as f32 + 1.0) * w - 0.5, r.bottom()),
        );
        ui.painter().rect_filled(bar, 0.0, t.accent.gamma_multiply(0.7));
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        let s = group_thousands(v.abs() as u64);
        if v < 0.0 { format!("−{s}") } else { s }
    } else {
        let s = format!("{v:.2}");
        if v < 0.0 { s.replacen('-', "−", 1) } else { s }
    }
}

fn details(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId, l: &lm_core::document::DocLayer) {
    let p = &l.profile;
    let row = |ui: &mut Ui, k: &str, v: String| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(k).color(t.text_secondary));
            ui.label(v);
        });
    };
    row(
        ui,
        "Source",
        match &p.source.member {
            Some(m) => format!("{} › {m}", p.source.path),
            None => p.source.path.clone(),
        },
    );
    row(ui, "Format", p.format.clone());
    if let Some(e) = &p.encoding {
        row(ui, "Text encoding", e.clone());
    }
    row(ui, "Coordinate system", format!("{} — {}", p.crs.value.label(), p.crs.evidence.0));
    if let Some(e) = p.extent_wgs84 {
        row(ui, "Extent", format!("{:.4}° to {:.4}° E, {:.4}° to {:.4}° N", e.min_x, e.max_x, e.min_y, e.max_y));
    }
    let status = match shell.ws.data(id).map(|d| d.status.clone()) {
        Some(ImportStatus::Complete) => "Complete".to_string(),
        Some(ImportStatus::CompleteWithExclusions) => {
            format!("Complete, with {} records excluded", group_thousands(p.exclusions.len() as u64))
        }
        Some(ImportStatus::NeedsDecision) => "Needs a decision".into(),
        Some(ImportStatus::Canceled) => "Canceled".into(),
        Some(ImportStatus::Failed(m)) => format!("Failed: {m}"),
        None => "Not loaded (the source is read again when the document opens)".into(),
    };
    row(ui, "Import", status);
    if !p.fields.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Fields").font(semibold(BODY)));
        let mut role_change: Option<(String, Option<lm_core::model::FieldRole>)> = None;
        let mut star: Option<(String, bool)> = None;
        // One row per field: star, name with its evidence wrapped beneath, role chip. Fixed widths
        // keep the inspector from growing with long evidence text.
        // The layer's own fields first, then each joined table's under its name (WP3.11).
        let joined: Vec<&str> = p.joins.iter().flat_map(|j| j.fields.iter().map(String::as_str)).collect();
        let mut groups: Vec<(Option<String>, Vec<&lm_core::model::FieldProfile>)> =
            vec![(None, p.fields.iter().filter(|f| !joined.contains(&f.name.as_str())).collect())];
        for j in &p.joins {
            groups.push((Some(j.table.clone()), p.fields.iter().filter(|f| j.fields.contains(&f.name)).collect()));
        }
        for (k, (title, fields)) in groups.iter().enumerate() {
            if let Some(title) = title {
                ui.add_space(8.0);
                // The joined table picked on the shelf is in the accent colour, scrolled to once.
                let focused = shell.join_focus.as_ref().is_some_and(|(p, n)| *p == id && n == title);
                let c = if focused { t.accent } else { t.text_secondary };
                let header = ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::LINK).font(theme::icon_font(13.0)).color(c));
                    ui.label(RichText::new(format!("From {title}")).size(CAPTION).color(c));
                });
                if focused && shell.join_scroll {
                    header.response.scroll_to_me(Some(egui::Align::TOP));
                    shell.join_scroll = false;
                }
            } else if fields.is_empty() && k == 0 {
                continue;
            }
            for f in fields.iter().copied() {
                let known = l.role(f);
                let starred = l.starred.contains(&f.name);
                let is_measure = matches!(known.value, lm_core::model::FieldRole::Measure(_) | lm_core::model::FieldRole::Category);
                let star_icon = if starred { icon::STAR_FOUR } else { icon::STAR };
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if is_measure {
                        let star_r = ui.add_sized(
                            vec2(18.0, 18.0),
                            egui::Button::new(RichText::new(star_icon).color(if starred { t.accent } else { t.text_secondary }))
                                .frame(false),
                        );
                        crate::a11y::toggle(&star_r, format!("Show {} in insight", f.name), starred);
                        if star_r.on_hover_text(if starred { "Remove from insight" } else { "Show in insight" }).clicked() {
                            star = Some((f.name.clone(), !starred));
                        }
                    } else {
                        ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
                    }
                    let text_w = (ui.available_width() - 92.0).max(80.0);
                    ui.vertical(|ui| {
                        ui.set_width(text_w);
                        ui.add(egui::Label::new(&f.name).wrap());
                        ui.add(egui::Label::new(RichText::new(&known.evidence.0).size(CAPTION).color(t.text_secondary)).wrap());
                    });
                    // The role chip, with Change.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let chip = egui::Button::new(RichText::new(role_name(known.value)).size(CAPTION))
                            .fill(t.accent_weak)
                            .corner_radius(egui::CornerRadius::same(10));
                        egui::containers::menu::MenuButton::from_button(chip).ui(ui, |ui| {
                            ui.label(RichText::new("Treat this field as").size(CAPTION).color(t.text_secondary));
                            for r in ROLE_CHOICES {
                                if ui.selectable_label(known.value == r, role_name(r)).clicked() {
                                    role_change = Some((f.name.clone(), Some(r)));
                                    ui.close();
                                }
                            }
                            if l.roles.contains_key(&f.name) && ui.button("Back to the detected role").clicked() {
                                role_change = Some((f.name.clone(), None));
                                ui.close();
                            }
                        });
                    });
                });
            }
        }
        if let Some((field, to)) = role_change {
            let tx = shell.ws.doc.set_role_tx(id, &field, to);
            shell.ws.commit(tx);
        }
        if let Some((field, to)) = star {
            let tx = shell.ws.doc.set_starred_tx(id, &field, to);
            shell.ws.commit(tx);
        }
    }
    if !p.assumptions.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Assumptions").font(semibold(BODY)));
        for a in p.assumptions.clone() {
            let how = match a.treatment {
                Treatment::Silent => "applied quietly",
                Treatment::Visible => "shown",
                Treatment::Question if a.confirmed_by_user => "answered",
                Treatment::Question => "needs a decision",
                Treatment::Block => "blocks actions",
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{} ({how}; chosen: {})", a.line, a.chosen.label));
                if a.alternatives.len() > 1 {
                    let resp = ui.link("Change");
                    if resp.clicked() {
                        shell.change_for = Some((id, a.kind, a.subject.clone(), resp.rect));
                    }
                }
            });
        }
    }
}

/// Roles a user can give a field (WP3.3).
const ROLE_CHOICES: [lm_core::model::FieldRole; 6] = [
    lm_core::model::FieldRole::Measure(lm_core::model::MeasureKind::Unknown),
    lm_core::model::FieldRole::Measure(lm_core::model::MeasureKind::Count),
    lm_core::model::FieldRole::Category,
    lm_core::model::FieldRole::Identifier,
    lm_core::model::FieldRole::Name,
    lm_core::model::FieldRole::Text,
];

/// A role in words for chips.
fn role_name(r: lm_core::model::FieldRole) -> &'static str {
    use lm_core::model::{FieldRole, MeasureKind};
    match r {
        FieldRole::Measure(MeasureKind::Count) => "Count",
        FieldRole::Measure(MeasureKind::Density) => "Density",
        FieldRole::Measure(_) => "Value",
        FieldRole::Identifier => "ID",
        FieldRole::Name => "Name",
        FieldRole::Category => "Category",
        FieldRole::Text => "Text",
        FieldRole::Date => "Date",
        FieldRole::Coordinate => "Coordinate",
        FieldRole::Unknown => "Unknown",
    }
}

/// The Change popover for an assumption: its alternatives, Not sure last.
pub fn change_popover(shell: &mut Shell, ui: &mut Ui) {
    let t = *shell.theme();
    let Some((id, kind, subject, anchor)) = shell.change_for.clone() else { return };
    let Some(a) =
        shell.ws.doc.layer(id).and_then(|l| l.profile.assumptions.iter().find(|a| a.kind == kind && a.subject == subject).cloned())
    else {
        shell.change_for = None;
        return;
    };
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("lm-change"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.left_bottom() + vec2(0.0, 4.0))
        .show(ui.ctx(), |ui| {
            frame(&t).show(ui, |ui| {
                ui.set_max_width(300.0);
                ui.label(RichText::new(&a.line).size(CAPTION).color(t.text_secondary));
                ui.add_space(4.0);
                let mut alts = a.alternatives.clone();
                // "Add unit": the common units for cell values, to pick from.
                if kind == AssumptionKind::RasterNoUnit {
                    alts = std::iter::once(Choice::new("unknown", "Unknown"))
                        .chain(
                            ["°C", "K", "m", "mm", "%", "people", "people per km²", "kg/ha"]
                                .iter()
                                .map(|u| Choice::new(format!("unit:{u}"), *u)),
                        )
                        .collect();
                }
                if !alts.iter().any(|c| c.id == "not_sure")
                    && matches!(subject, Subject::Layer)
                    && a.consequence != lm_core::model::Consequence::DisplayOnly
                {
                    alts.push(Choice::not_sure());
                }
                for c in alts {
                    let selected = c.id == a.chosen.id;
                    if ui.selectable_label(selected, &c.label).clicked() {
                        if !selected {
                            let now = ui.input(|i| i.time);
                            if let Err(e) = shell.ws.change_assumption(id, kind, subject.clone(), c, None) {
                                shell.set_notice(e, now);
                            }
                        }
                        close = true;
                    }
                }
            });
        });
    let clicked_out = ui.input(|i| i.pointer.any_click())
        && !area.response.hovered()
        && !anchor.contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or_default());
    if close || clicked_out || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.change_for = None;
    }
}

/// The import report's Review list: the excluded records with a Reason column (read-only).
pub fn review(shell: &mut Shell, ctx: &egui::Context) {
    let Some(id) = shell.review_for else { return };
    let Some(l) = shell.ws.doc.layer(id).cloned() else {
        shell.review_for = None;
        return;
    };
    let mut open = true;
    let mut export = false;
    egui::Window::new(format!("Excluded records — {}", l.display_name))
        .open(&mut open)
        .collapsible(false)
        .default_width(460.0)
        .default_height(360.0)
        .show(ctx, |ui| {
            ui.label(format!(
                "{} records were left out of the map. Your file has not been changed.",
                group_thousands(l.profile.exclusions.len() as u64)
            ));
            if ui.button("Export problem list…").clicked() {
                export = true;
            }
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new(("lm-review", id.0)).striped(true).num_columns(2).show(ui, |ui| {
                    ui.label(RichText::new("Row").strong());
                    ui.label(RichText::new("Reason").strong());
                    ui.end_row();
                    for e in &l.profile.exclusions {
                        ui.label(group_thousands(e.row));
                        ui.label(&e.reason);
                        ui.end_row();
                    }
                });
            });
        });
    if export {
        shell.export_problems(id);
    }
    if !open {
        shell.review_for = None;
    }
}

/// While files are dragged over the window: "Drop to add to the map".
pub fn drop_overlay(shell: &Shell, ui: &Ui, full: Rect) {
    let t = shell.theme();
    if ui.input(|i| i.raw.hovered_files.is_empty()) {
        return;
    }
    let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lm-drop")));
    p.rect_filled(full, 0.0, t.surface.gamma_multiply(0.85));
    p.rect_stroke(full.shrink(12.0), CornerRadius::same(theme::RADIUS_PANEL), Stroke::new(2.0, t.accent), egui::StrokeKind::Inside);
    p.text(
        full.center(),
        Align2::CENTER_CENTER,
        "Drop to add to the map",
        egui::FontId::new(theme::TITLE, egui::FontFamily::Name("semibold".into())),
        t.text_primary,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_names_lose_their_middle() {
        assert_eq!(middle_truncate("schools", 26), "schools");
        let s = middle_truncate("district_boundaries_shanghai_2026_final", 20);
        assert_eq!(s.chars().count(), 20);
        assert!(s.starts_with("district_b") && s.ends_with("26_final"), "{s}");
    }
}
