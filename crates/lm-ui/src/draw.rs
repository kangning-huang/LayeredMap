//! Drawing an area (WP4.4). Polygon: click to add points, double-click or Return to finish, Esc
//! to cancel. Rectangle: drag. A banner reads "Drawing — Esc to cancel". The area becomes a layer
//! in WGS 84 named "Area 1", "Area 2"… that can be renamed. Drawing starts from Pro's assistant
//! or the developer surface ("Summarize in an area I draw"), never from a free-tier button.

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::Shell;
use crate::theme::{self, PANEL_MARGIN};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Polygon,
    Rectangle,
}

/// An area being drawn, in longitude/latitude.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub mode: Mode,
    pub points: Vec<(f64, f64)>,
    /// The rectangle being dragged: its two corners.
    pub drag: Option<((f64, f64), (f64, f64))>,
}

/// What a drawing step asks the shell to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Continue,
    /// The outline is complete: make the layer.
    Finish(Vec<(f64, f64)>),
}

impl Drawing {
    pub fn new(mode: Mode) -> Drawing {
        Drawing { mode, points: Vec::new(), drag: None }
    }

    /// A click adds a point (polygon). A click on (or very near) the last point is ignored, so a
    /// double-click's two clicks add one point.
    pub fn click(&mut self, at: (f64, f64), same_as_last: impl Fn((f64, f64), (f64, f64)) -> bool) -> Step {
        if self.mode == Mode::Polygon && self.points.last().is_none_or(|l| !same_as_last(*l, at)) {
            self.points.push(at);
        }
        Step::Continue
    }

    /// Double-click or Return: finish a polygon of at least three points.
    pub fn finish(&mut self) -> Step {
        if self.mode == Mode::Polygon && self.points.len() >= 3 { Step::Finish(self.points.clone()) } else { Step::Continue }
    }

    /// A drag (rectangle): the corners so far; released, the rectangle finishes unless it has no
    /// size.
    pub fn drag(&mut self, a: (f64, f64), b: (f64, f64), released: bool) -> Step {
        if self.mode != Mode::Rectangle {
            return Step::Continue;
        }
        self.drag = Some((a, b));
        if released {
            self.drag = None;
            if (a.0 - b.0).abs() > 0.0 && (a.1 - b.1).abs() > 0.0 {
                return Step::Finish(vec![(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1)]);
            }
        }
        Step::Continue
    }
}

/// The next free "Area N" name.
pub fn next_name(existing: impl Iterator<Item = String>) -> String {
    let used: Vec<u32> = existing.filter_map(|n| n.strip_prefix("Area ").and_then(|k| k.parse().ok())).collect();
    let n = (1..).find(|k| !used.contains(k)).unwrap_or(1);
    format!("Area {n}")
}

/// Handles this frame's map input while drawing, and paints the outline and the banner.
pub fn frame(shell: &mut Shell, ui: &Ui, map_rect: Rect, out: &lm_mapview::MapOutput) {
    let Some(mut d) = shell.drawing.take() else { return };
    let t = *shell.theme();
    let near = |shell: &Shell, a: (f64, f64), b: (f64, f64)| -> bool {
        let pa = shell.map_view().lonlat_to_screen(lm_mapview::Coord::new(a.0, a.1));
        let pb = shell.map_view().lonlat_to_screen(lm_mapview::Coord::new(b.0, b.1));
        matches!((pa, pb), (Some(pa), Some(pb)) if pa.distance(pb) < 6.0)
    };
    let mut step = Step::Continue;
    if let Some(c) = out.clicked {
        step = d.click((c.x, c.y), |a, b| near(shell, a, b));
    }
    if out.double_clicked.is_some() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        step = d.finish();
    }
    if let Some((a, b, released)) = out.drag {
        step = d.drag((a.x, a.y), (b.x, b.y), released);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.after_drawing = None;
        return; // cancelled: the drawing is dropped
    }
    match step {
        Step::Finish(ring) => {
            let name = next_name(shell.ws.doc.layers.iter().map(|l| l.display_name.clone()));
            if let Some(layer) = lm_jobs::import::drawn_area(&name, &ring) {
                shell.ws.add_imported(vec![layer], u64::MAX);
                if let Some(id) = shell.ws.doc.layers.iter().find(|l| l.display_name == name).map(|l| l.id) {
                    shell.ws.select_layer(id);
                    crate::analysis::area_drawn(shell, id);
                }
            }
        }
        Step::Continue => {
            paint(shell, ui, &t, map_rect, &d);
            shell.drawing = Some(d);
            ui.ctx().request_repaint();
        }
    }
}

fn paint(shell: &Shell, ui: &Ui, t: &theme::Theme, map_rect: Rect, d: &Drawing) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("lm-drawing")));
    let to_screen = |p: (f64, f64)| shell.map_view().lonlat_to_screen(lm_mapview::Coord::new(p.0, p.1));
    let mut pts: Vec<Pos2> = match (d.mode, d.drag) {
        (Mode::Rectangle, Some((a, b))) => [(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1)].iter().filter_map(|p| to_screen(*p)).collect(),
        _ => d.points.iter().filter_map(|p| to_screen(*p)).collect(),
    };
    // The rubber band to the pointer while placing polygon points.
    if d.mode == Mode::Polygon
        && let Some(hover) = ui.input(|i| i.pointer.hover_pos()).filter(|p| map_rect.contains(*p))
        && !pts.is_empty()
    {
        pts.push(hover);
    }
    let stroke = Stroke::new(2.0, t.accent);
    if pts.len() >= 3 {
        painter.add(egui::Shape::convex_polygon(pts.clone(), t.accent.gamma_multiply(0.12), Stroke::NONE));
    }
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], stroke);
    }
    if pts.len() >= 3 {
        painter.line_segment([pts[pts.len() - 1], pts[0]], Stroke::new(1.0, t.accent.gamma_multiply(0.6)));
    }
    for p in d.points.iter().filter_map(|p| to_screen(*p)) {
        painter.circle(p, 4.0, Color32::WHITE, stroke);
    }
    // The banner.
    let text = match d.mode {
        Mode::Polygon => "Drawing — click to add points, double-click to finish, Esc to cancel",
        Mode::Rectangle => "Drawing — drag a rectangle, Esc to cancel",
    };
    let fg = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lm-drawing-banner")));
    let galley = fg.layout_no_wrap(text.to_string(), theme::regular(theme::BODY), t.on_accent);
    let size = galley.size() + vec2(28.0, 14.0);
    let r = Rect::from_min_size(pos2(map_rect.center().x - size.x / 2.0, map_rect.top() + PANEL_MARGIN + 44.0), size);
    fg.rect(r, CornerRadius::same(16), t.accent, Stroke::NONE, StrokeKind::Inside);
    fg.galley(r.min + vec2(14.0, 7.0), galley, t.on_accent);
    let _ = Align2::CENTER_CENTER;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(a: (f64, f64), b: (f64, f64)) -> bool {
        a == b
    }

    #[test]
    fn a_polygon_needs_three_points_and_double_clicks_add_one() {
        let mut d = Drawing::new(Mode::Polygon);
        d.click((0.0, 0.0), exact);
        d.click((1.0, 0.0), exact);
        assert_eq!(d.finish(), Step::Continue);
        d.click((1.0, 1.0), exact);
        d.click((1.0, 1.0), exact); // the second click of a double-click
        assert_eq!(d.finish(), Step::Finish(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]));
    }

    #[test]
    fn a_rectangle_finishes_on_release_unless_it_has_no_size() {
        let mut d = Drawing::new(Mode::Rectangle);
        assert_eq!(d.drag((0.0, 0.0), (2.0, 1.0), false), Step::Continue);
        assert_eq!(d.drag((0.0, 0.0), (0.0, 1.0), true), Step::Continue);
        assert_eq!(d.drag((0.0, 0.0), (2.0, 1.0), true), Step::Finish(vec![(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)]));
    }

    #[test]
    fn areas_are_numbered_from_the_first_free_number() {
        assert_eq!(next_name(["Area 1", "schools", "Area 3"].iter().map(|s| s.to_string())), "Area 2");
        assert_eq!(next_name(std::iter::empty()), "Area 1");
    }
}
