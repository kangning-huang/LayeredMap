//! Inspector form controls in the macOS manner (Apple design review of the Style tab,
//! 2026-10-17): a label column with controls beside it, a segmented control, a checkbox whose
//! box is always visible, and selectable tiles that share one selection language (an accent
//! ring plus a semibold name, never colour alone).

use egui::{Color32, CornerRadius, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::theme::{self, BODY, CAPTION, Theme};

/// The width of the label column.
pub const LABEL_W: f32 = 72.0;
/// The height of one control row.
pub const ROW_H: f32 = 26.0;

/// One form row: a secondary-text label in the label column, the control beside it.
pub fn row<R>(ui: &mut Ui, t: &Theme, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, ROW_H), Sense::hover());
        ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, label, theme::regular(BODY), t.text_secondary);
        add(ui)
    })
    .inner
}

/// A row with an empty label column: a control that belongs to the row above.
pub fn indented<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + ui.spacing().item_spacing.x);
        add(ui)
    })
    .inner
}

/// A group divider: space, a hairline, space.
pub fn divider(ui: &mut Ui, t: &Theme) {
    ui.add_space(8.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.border));
    ui.add_space(8.0);
}

/// A group heading.
pub fn heading(ui: &mut Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).font(theme::semibold(BODY)));
    ui.add_space(2.0);
}

/// A segmented control of equal segments filling `width`; returns the segment clicked.
pub fn segmented(ui: &mut Ui, t: &Theme, selected: usize, labels: &[&str], width: f32) -> Option<usize> {
    let (r, _) = ui.allocate_exact_size(vec2(width, ROW_H - 2.0), Sense::hover());
    let p = ui.painter();
    p.rect(r, CornerRadius::same(theme::RADIUS_CONTROL), t.surface_raised, Stroke::new(1.0, t.control_stroke), StrokeKind::Inside);
    let w = r.width() / labels.len() as f32;
    let mut out = None;
    for (k, label) in labels.iter().enumerate() {
        let seg = Rect::from_min_size(pos2(r.left() + k as f32 * w, r.top()), vec2(w, r.height()));
        let resp = ui.interact(seg, ui.id().with(("segment", label, k)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let on = k == selected;
        let p = ui.painter();
        if on {
            p.rect(
                seg.shrink(2.0),
                CornerRadius::same(theme::RADIUS_CONTROL - 2),
                t.surface,
                Stroke::new(1.0, t.control_stroke),
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            p.rect_filled(seg.shrink(2.0), CornerRadius::same(theme::RADIUS_CONTROL - 2), t.border);
        }
        if k > 0 && !on && k != selected + 1 {
            p.vline(seg.left(), seg.y_range().shrink(6.0), Stroke::new(1.0, t.border));
        }
        let font = if on { theme::semibold(CAPTION) } else { theme::regular(CAPTION) };
        p.text(seg.center(), egui::Align2::CENTER_CENTER, *label, font, t.text_primary);
        if resp.clicked() && !on {
            out = Some(k);
        }
    }
    out
}

/// A checkbox: a 14 px box with a 3:1 edge when off, filled with the accent and checked when on.
pub fn checkbox(ui: &mut Ui, t: &Theme, on: &mut bool, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), theme::regular(BODY), t.text_primary);
    let size = vec2(14.0 + 8.0 + galley.size().x, ROW_H);
    let (r, mut resp) = ui.allocate_exact_size(size, Sense::click());
    let box_r = Rect::from_center_size(pos2(r.left() + 7.0, r.center().y), vec2(14.0, 14.0));
    let p = ui.painter();
    if *on {
        p.rect_filled(box_r, CornerRadius::same(3), t.accent);
        let c = t.on_accent;
        let a = pos2(box_r.left() + 3.2, box_r.center().y + 0.2);
        let b = pos2(box_r.left() + 6.0, box_r.bottom() - 3.5);
        let d = pos2(box_r.right() - 3.0, box_r.top() + 3.5);
        p.line_segment([a, b], Stroke::new(1.8, c));
        p.line_segment([b, d], Stroke::new(1.8, c));
    } else {
        let edge = if resp.hovered() { t.text_secondary } else { t.control_stroke };
        p.rect(box_r, CornerRadius::same(3), t.surface, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
    p.galley(pos2(box_r.right() + 8.0, r.center().y - galley.size().y / 2.0), galley, t.text_primary);
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, label));
    resp
}

/// The frame of a selectable tile: raised fill, a hairline edge, or a 2 px accent ring when
/// selected (with a 1 px gap, as macOS draws selection around thumbnails).
pub fn tile_frame(ui: &Ui, t: &Theme, r: Rect, on: bool, hovered: bool) {
    let p = ui.painter();
    p.rect(
        r,
        CornerRadius::same(theme::RADIUS_CONTROL),
        t.surface_raised,
        Stroke::new(1.0, if hovered { t.control_stroke } else { t.border }),
        StrokeKind::Inside,
    );
    if on {
        p.rect_stroke(r.expand(2.0), CornerRadius::same(theme::RADIUS_CONTROL + 2), Stroke::new(2.0, t.accent), StrokeKind::Inside);
    }
}

/// A tile's caption under it, wrapped to two lines at most: semibold when selected.
pub fn tile_caption(ui: &Ui, t: &Theme, r: Rect, text: &str, on: bool) {
    let font = if on { theme::semibold(CAPTION) } else { theme::regular(CAPTION) };
    let color = if on { t.text_primary } else { t.text_secondary };
    let mut job = egui::text::LayoutJob::simple(text.to_string(), font, color, r.width());
    job.wrap.max_rows = 2;
    job.halign = egui::Align::Center;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(pos2(r.center().x, r.top()), galley, color);
}

/// A colour swatch button (a filled circle); selected gets the accent ring.
pub fn swatch_button(ui: &mut Ui, t: &Theme, c: Color32, on: bool, name: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let p = ui.painter();
    p.circle(r.center(), 8.5, c, Stroke::new(1.0, t.border));
    if on {
        p.circle_stroke(r.center(), 11.0, Stroke::new(2.0, t.accent));
    } else if resp.hovered() {
        p.circle_stroke(r.center(), 11.0, Stroke::new(1.0, t.control_stroke));
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, name));
    resp.on_hover_text(name)
}

/// Gives pop-ups, value boxes and slider rails inside `add` a visible 3:1 edge.
pub fn with_visible_controls<R>(ui: &mut Ui, t: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.control_stroke);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.text_secondary);
        add(ui)
    })
    .inner
}

/// A slider as macOS draws one: the track filled with the accent from the minimum to the thumb.
pub fn slider(ui: &mut Ui, t: &Theme, slider: egui::Slider<'_>) -> Response {
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.slider_trailing_fill = true;
        v.selection.bg_fill = t.accent;
        ui.spacing_mut().slider_width = (ui.available_width() - 74.0).max(80.0);
        ui.add(slider.trailing_fill(true))
    })
    .inner
}
