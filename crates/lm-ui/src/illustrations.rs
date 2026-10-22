//! Small illustrations beside suggested actions (owner's request, 2026-10-17): each shows what
//! the action will *do* to the map, for people who have never used GIS. Design rules, from
//! Apple's guidance (`icons.md`, `sf-symbols.md`) applied to what GIS manuals draw
//! (`docs/lm/reviews/2026-10-17-analysis-illustrations.md`):
//!
//! * **Show the result, not the method.** The input is faint, the result is the one strong
//!   thing (hierarchical rendering: one accent colour at three strengths, grey for context).
//! * **One idea, a handful of shapes.** Dots are places, a soft outline is an area, a few tinted
//!   squares are a grid of values, bars are a ranking, a ruler is a measurement.
//! * **One family.** The same 56 × 32 frame, 1.5 pt round strokes, top-down view, no shading.
//! * **No words.** The single exception is the numeral on "Count", where the number *is* the
//!   idea. The label next to it always says what happens; the picture never replaces it.
//! * **Drawn, not bitmaps**, in the theme's colours, so light, dark and any accent work.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use crate::theme::{self, Theme};

/// The illustration's drawn size, and the 56 × 32 grid it is designed on.
pub const SIZE: egui::Vec2 = vec2(70.0, 40.0);
const DESIGN: egui::Vec2 = vec2(56.0, 32.0);
const STROKE: f32 = 1.5;

/// Whether an action has an illustration.
pub fn has(id: &str) -> bool {
    !matches!(id, "open_file" | "try_example" | "browse_library")
}

struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    accent: Color32,
    grey: Color32,
    ground: Color32,
    /// Outlines drawn over a grid of values: the text colour, so they read over any tint.
    ink: Color32,
    /// On a dark ground the faintest tints are lifted, or they disappear.
    dark: bool,
}

impl Pen<'_> {
    /// A point in the frame from 0–56 × 0–32 design units.
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.r.left() + x * self.r.width() / DESIGN.x, self.r.top() + y * self.r.height() / DESIGN.y)
    }

    fn pts(&self, v: &[(f32, f32)]) -> Vec<Pos2> {
        v.iter().map(|(x, y)| self.at(*x, *y)).collect()
    }

    fn strong(&self) -> Stroke {
        Stroke::new(STROKE, self.accent)
    }

    fn soft(&self) -> Stroke {
        Stroke::new(STROKE, self.grey)
    }

    fn tint(&self, k: f32) -> Color32 {
        self.accent.gamma_multiply(if self.dark { 0.3 + 0.7 * k } else { k })
    }

    fn area(&self, v: &[(f32, f32)], fill: Color32, stroke: Stroke) {
        let pts = self.pts(v);
        self.p.add(Shape::convex_polygon(pts, fill, stroke));
    }

    fn dashed_area(&self, v: &[(f32, f32)], fill: Color32, color: Color32) {
        let mut pts = self.pts(v);
        self.p.add(Shape::convex_polygon(pts.clone(), fill, Stroke::NONE));
        pts.push(pts[0]);
        self.p.extend(Shape::dashed_line(&pts, Stroke::new(STROKE, color), 3.0, 2.0));
    }

    /// A place: a dot with a ring in the ground colour so it reads over fills.
    fn dot(&self, x: f32, y: f32, color: Color32) {
        let c = self.at(x, y);
        self.p.circle(c, 2.6, color, Stroke::new(1.0, self.ground));
    }

    /// A small grid of values (a raster): `cols × rows` squares tinted by `levels` (0–1).
    fn grid(&self, x: f32, y: f32, cell: f32, cols: usize, levels: &[f32]) {
        for (i, l) in levels.iter().enumerate() {
            let (c, r) = ((i % cols) as f32, (i / cols) as f32);
            let a = self.at(x + c * cell, y + r * cell);
            let b = self.at(x + (c + 1.0) * cell - 1.0, y + (r + 1.0) * cell - 1.0);
            self.p.rect_filled(Rect::from_two_pos(a, b), 1.0, self.tint(0.15 + 0.7 * l));
        }
    }

    /// "→" between a before and an after.
    fn arrow(&self, x0: f32, x1: f32, y: f32) {
        let s = self.soft();
        self.p.line_segment([self.at(x0, y), self.at(x1, y)], s);
        self.p.line_segment([self.at(x1 - 3.0, y - 3.0), self.at(x1, y)], s);
        self.p.line_segment([self.at(x1 - 3.0, y + 3.0), self.at(x1, y)], s);
    }

    fn line(&self, v: &[(f32, f32)], stroke: Stroke) {
        self.p.add(Shape::line(self.pts(v), stroke));
    }

    /// A ruler's ticks along a straight segment.
    fn ticks(&self, a: (f32, f32), b: (f32, f32), n: usize) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (nx, ny) = (-dy / len * 2.5, dx / len * 2.5);
        for i in 0..=n {
            let k = i as f32 / n as f32;
            let (x, y) = (a.0 + dx * k, a.1 + dy * k);
            let big = if i == 0 || i == n { 1.6 } else { 1.0 };
            self.p.line_segment([self.at(x, y), self.at(x + nx * big, y + ny * big)], self.strong());
        }
    }
}

/// The blob all "area" illustrations share: a soft, slightly irregular region.
const AREA: [(f32, f32); 6] = [(14.0, 5.0), (34.0, 4.0), (43.0, 13.0), (40.0, 27.0), (19.0, 28.0), (10.0, 17.0)];

/// Paints the illustration for `id` in `rect` (any size of the 56 × 32 aspect).
pub fn paint(p: &Painter, rect: Rect, id: &str, t: &Theme) {
    let pen = Pen {
        p,
        r: rect,
        accent: t.accent,
        grey: t.text_secondary.gamma_multiply(0.7),
        ground: t.surface,
        ink: t.text_primary.gamma_multiply(0.75),
        dark: t.dark,
    };
    let faint = pen.tint(0.12);
    match id {
        // Count: places inside an area become one number.
        "count_in" | "count_here" => {
            pen.area(&AREA, faint, pen.soft());
            pen.dot(20.0, 12.0, t.accent);
            pen.dot(30.0, 19.0, t.accent);
            pen.dot(22.0, 22.0, t.accent);
            pen.dot(50.0, 24.0, pen.grey);
            let c = pen.at(44.0, 8.0);
            p.circle_filled(c, 7.0, t.accent);
            p.text(c, egui::Align2::CENTER_CENTER, "3", theme::semibold(9.5), t.on_accent);
        }
        // Which area: the one containing the place lights up.
        "which_container" => {
            pen.area(&[(5.0, 6.0), (27.0, 4.0), (27.0, 28.0), (7.0, 26.0)], Color32::TRANSPARENT, pen.soft());
            pen.area(&[(29.0, 4.0), (49.0, 7.0), (51.0, 25.0), (29.0, 28.0)], pen.tint(0.25), pen.strong());
            pen.dot(39.0, 16.0, t.accent);
        }
        // What's inside: the places in the area are picked out; the others fade.
        "whats_inside" | "summarize_data_here" => {
            pen.area(&AREA, faint, pen.strong());
            pen.dot(20.0, 13.0, t.accent);
            pen.dot(31.0, 11.0, t.accent);
            pen.dot(27.0, 21.0, t.accent);
            pen.dot(5.0, 9.0, pen.grey.gamma_multiply(0.6));
            pen.dot(50.0, 22.0, pen.grey.gamma_multiply(0.6));
            pen.dot(48.0, 6.0, pen.grey.gamma_multiply(0.6));
        }
        // Summarize: many values under an area become the area's one value.
        "summarize_each" | "summarize_here" => {
            pen.grid(3.0, 4.0, 8.0, 3, &[0.1, 0.5, 0.9, 0.3, 0.7, 1.0, 0.0, 0.4, 0.6]);
            pen.area(
                &[(6.0, 7.0), (20.0, 6.0), (24.0, 18.0), (16.0, 26.0), (6.0, 22.0)],
                Color32::TRANSPARENT,
                Stroke::new(STROKE, pen.ink),
            );
            pen.arrow(30.0, 36.0, 16.0);
            pen.area(&[(39.0, 7.0), (51.0, 6.0), (54.0, 18.0), (47.0, 26.0), (39.0, 22.0)], pen.tint(0.6), pen.strong());
        }
        // Summarize in an area I draw: the area is drawn by hand (dashed).
        "summarize_drawn" => {
            pen.grid(3.0, 4.0, 8.0, 3, &[0.2, 0.6, 0.9, 0.4, 0.8, 1.0, 0.1, 0.3, 0.7]);
            pen.dashed_area(&[(6.0, 7.0), (21.0, 5.0), (25.0, 17.0), (15.0, 27.0), (5.0, 20.0)], Color32::TRANSPARENT, pen.ink);
            pen.arrow(30.0, 36.0, 16.0);
            pen.area(&[(39.0, 7.0), (51.0, 6.0), (54.0, 18.0), (47.0, 26.0), (39.0, 22.0)], pen.tint(0.6), pen.strong());
        }
        // Within a distance: a ring around each place; overlapping rings merge.
        "buffer" => {
            let (a, b) = (pen.at(22.0, 16.0), pen.at(35.0, 16.0));
            let rr = 10.0 * rect.height() / DESIGN.y;
            p.circle_filled(a, rr, pen.tint(0.22));
            p.circle_filled(b, rr, pen.tint(0.22));
            p.circle_stroke(a, rr, Stroke::new(STROKE, pen.tint(0.7)));
            p.circle_stroke(b, rr, Stroke::new(STROKE, pen.tint(0.7)));
            pen.dot(22.0, 16.0, t.accent);
            pen.dot(35.0, 16.0, t.accent);
        }
        // Measure an area: the shape filled, with a ruler along its base.
        "measure_area" => {
            pen.area(&[(12.0, 6.0), (40.0, 5.0), (46.0, 20.0), (10.0, 22.0)], pen.tint(0.3), pen.strong());
            pen.ticks((10.0, 27.0), (46.0, 27.0), 6);
            pen.line(&[(10.0, 27.0), (46.0, 27.0)], pen.strong());
        }
        // Measure a length: a path with a ruler's ticks along it.
        "measure_length" => {
            pen.line(&[(6.0, 24.0), (20.0, 10.0), (34.0, 20.0), (50.0, 8.0)], Stroke::new(2.0, t.accent));
            pen.ticks((6.0, 28.5), (50.0, 28.5), 8);
            pen.line(&[(6.0, 28.5), (50.0, 28.5)], pen.soft());
        }
        // Measure from here: two places joined by a measured line.
        "measure_from_here" => {
            pen.p.extend(Shape::dashed_line(&pen.pts(&[(10.0, 22.0), (46.0, 10.0)]), pen.strong(), 3.0, 2.0));
            pen.dot(10.0, 22.0, t.accent);
            pen.dot(46.0, 10.0, t.accent);
            pen.ticks((10.0, 28.0), (46.0, 28.0), 6);
        }
        // Add raster values to places: each place takes the value of the square it sits on.
        "add_raster_values" => {
            pen.grid(3.0, 4.0, 8.0, 3, &[0.1, 0.4, 0.2, 0.5, 0.9, 0.3, 0.2, 0.6, 0.1]);
            pen.dot(15.0, 16.0, t.accent);
            pen.arrow(29.0, 35.0, 16.0);
            // The place again, now carrying its square's value as a swatch beside it.
            pen.dot(40.0, 16.0, t.accent);
            let tag = Rect::from_two_pos(pen.at(44.0, 11.0), pen.at(53.0, 21.0));
            p.rect(tag, 2.0, pen.tint(0.15 + 0.7 * 0.9), Stroke::new(1.0, pen.ink), egui::StrokeKind::Inside);
        }
        // Rank: bars, longest first.
        "rank_by_value" => {
            for (i, (len, k)) in [(40.0, 1.0), (30.0, 0.55), (21.0, 0.4), (12.0, 0.3)].iter().enumerate() {
                let y = 5.0 + i as f32 * 6.5;
                p.rect_filled(Rect::from_two_pos(pen.at(8.0, y), pen.at(8.0 + len, y + 4.0)), 2.0, pen.tint(*k));
            }
        }
        // Compare with others: one bar among its peers is highlighted.
        "compare_peers" => {
            for (i, h) in [12.0_f32, 20.0, 15.0, 24.0, 9.0].iter().enumerate() {
                let x = 9.0 + i as f32 * 8.5;
                let color = if i == 3 { t.accent } else { pen.tint(0.3) };
                p.rect_filled(Rect::from_two_pos(pen.at(x, 28.0 - h), pen.at(x + 5.5, 28.0)), 1.5, color);
            }
        }
        // Colour by a value: areas shaded from light to dark.
        "style_by_value" => {
            let tiles = [(4.0, 0.2), (17.5, 0.45), (31.0, 0.7), (44.5, 1.0)];
            for (x, k) in tiles {
                pen.area(&[(x, 7.0), (x + 9.0, 5.0), (x + 11.0, 25.0), (x + 1.0, 27.0)], pen.tint(k), Stroke::new(1.0, t.surface));
            }
        }
        // Filter: some places stay, the rest are hidden (outlines only).
        "filter" => {
            for (x, y, keep) in [(10.0, 10.0, true), (22.0, 22.0, false), (30.0, 9.0, true), (40.0, 21.0, true), (49.0, 11.0, false)] {
                if keep {
                    pen.dot(x, y, t.accent);
                } else {
                    p.circle_stroke(pen.at(x, y), 2.6, Stroke::new(1.0, pen.grey.gamma_multiply(0.6)));
                }
            }
        }
        // View data: a small table.
        "view_data" => {
            let r = Rect::from_two_pos(pen.at(10.0, 5.0), pen.at(46.0, 27.0));
            p.rect(r, 3.0, Color32::TRANSPARENT, pen.soft(), egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_two_pos(pen.at(10.0, 5.0), pen.at(46.0, 11.0)), 3.0, pen.tint(0.35));
            for y in [16.5, 22.0] {
                p.line_segment([pen.at(10.0, y), pen.at(46.0, y)], pen.soft());
            }
            p.line_segment([pen.at(24.0, 5.0), pen.at(24.0, 27.0)], pen.soft());
        }
        // Change colours and range: a grid recoloured along a ramp.
        "raster_colors" => {
            pen.grid(5.0, 4.0, 7.0, 4, &[0.0, 0.3, 0.6, 0.9, 0.2, 0.5, 0.8, 1.0, 0.1, 0.4, 0.7, 0.95]);
            for i in 0..8 {
                let x = 36.0 + i as f32 * 2.2;
                p.rect_filled(Rect::from_two_pos(pen.at(x, 8.0), pen.at(x + 2.3, 24.0)), 0.0, pen.tint(0.1 + 0.9 * i as f32 / 7.0));
            }
        }
        // Show only values above…: the high squares stay, the rest go grey.
        "threshold" => {
            let levels = [0.2, 0.9, 0.4, 1.0, 0.8, 0.1, 0.3, 0.95, 0.5, 0.85, 0.2, 0.6];
            for (i, l) in levels.iter().enumerate() {
                let (c, r) = ((i % 4) as f32, (i / 4) as f32);
                let a = pen.at(12.0 + c * 8.0, 4.0 + r * 8.0);
                let b = pen.at(12.0 + (c + 1.0) * 8.0 - 1.0, 4.0 + (r + 1.0) * 8.0 - 1.0);
                let color = if *l >= 0.8 { pen.tint(0.25 + 0.7 * l) } else { pen.grey.gamma_multiply(0.18) };
                p.rect_filled(Rect::from_two_pos(a, b), 1.0, color);
            }
        }
        // Where values are like this: the chosen square and those like it light up.
        "similar_values" => {
            let like = [1usize, 6, 8];
            for i in 0..12 {
                let (c, r) = ((i % 4) as f32, (i / 4) as f32);
                let a = pen.at(12.0 + c * 8.0, 4.0 + r * 8.0);
                let b = pen.at(12.0 + (c + 1.0) * 8.0 - 1.0, 4.0 + (r + 1.0) * 8.0 - 1.0);
                let color = if like.contains(&i) { pen.tint(0.6) } else { pen.grey.gamma_multiply(0.15) };
                p.rect_filled(Rect::from_two_pos(a, b), 1.0, color);
                if i == 6 {
                    p.rect_stroke(Rect::from_two_pos(a, b).expand(1.0), 1.5, pen.strong(), egui::StrokeKind::Outside);
                }
            }
        }
        // Map using coordinate columns: a table's rows become places.
        "map_coordinates" => {
            for y in [8.0, 15.0, 22.0] {
                p.line_segment([pen.at(6.0, y), pen.at(22.0, y)], pen.soft());
            }
            pen.arrow(26.0, 32.0, 15.0);
            pen.dot(40.0, 9.0, t.accent);
            pen.dot(47.0, 18.0, t.accent);
            pen.dot(38.0, 23.0, t.accent);
        }
        // Match a table to a layer: two lists joined.
        "match_table" => {
            for y in [8.0, 15.0, 22.0] {
                p.line_segment([pen.at(6.0, y), pen.at(20.0, y)], pen.soft());
                p.line_segment([pen.at(36.0, y), pen.at(50.0, y)], pen.soft());
                p.line_segment([pen.at(22.0, y), pen.at(34.0, y)], pen.strong());
            }
        }
        // Anything else: a neutral layer glyph, so a row of buttons never mixes with and without.
        _ => {
            pen.area(&[(16.0, 14.0), (28.0, 8.0), (40.0, 14.0), (28.0, 20.0)], pen.tint(0.2), pen.soft());
            pen.area(&[(16.0, 19.0), (28.0, 13.0), (40.0, 19.0), (28.0, 25.0)], pen.tint(0.45), pen.strong());
        }
    }
}

/// An action button with its illustration on the left and the label beside it.
pub fn button(ui: &mut egui::Ui, t: &Theme, id: &str, label: &str) -> egui::Response {
    let w = ui.available_width();
    let h = 52.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let visuals = ui.style().interact(&resp);
    ui.painter().rect_filled(rect, theme::RADIUS_CONTROL, visuals.weak_bg_fill);
    let art = Rect::from_min_size(rect.left_top() + vec2(8.0, (h - SIZE.y) / 2.0), SIZE);
    if has(id) {
        paint(ui.painter(), art, id, t);
    }
    let text_left = art.right() + 10.0;
    let galley = ui.painter().layout(label.to_string(), theme::regular(theme::BODY), t.text_primary, rect.right() - text_left - 8.0);
    ui.painter().galley(egui::pos2(text_left, rect.center().y - galley.size().y / 2.0), galley, t.text_primary);
    resp
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;

    use crate::theme::Theme;

    /// Every action, as the gallery and the review show them.
    pub const GALLERY: &[(&str, &str)] = &[
        ("count_in", "Count places in each country"),
        ("which_container", "Which country is it in?"),
        ("whats_inside", "What's inside?"),
        ("summarize_each", "Summarize elevation in each country"),
        ("summarize_drawn", "Summarize in an area I draw"),
        ("buffer", "Show an area within a distance"),
        ("measure_area", "Measure its area"),
        ("measure_length", "Measure length"),
        ("measure_from_here", "Measure from here"),
        ("add_raster_values", "Add elevation values to each place"),
        ("rank_by_value", "Rank countries by a value"),
        ("compare_peers", "Compare with other countries"),
        ("style_by_value", "Color by a value"),
        ("filter", "Filter places"),
        ("view_data", "View data"),
        ("raster_colors", "Change colors and range"),
        ("threshold", "Show only values above…"),
        ("similar_values", "Show where values are like this"),
        ("map_coordinates", "Map using coordinate columns"),
        ("match_table", "Match to another layer"),
    ];

    /// Renders every illustration as a button, light and dark, at 2×
    /// (`LM_GALLERY=dir cargo test -p lm-ui illustration_gallery -- --ignored`).
    #[test]
    #[ignore]
    fn illustration_gallery() {
        let Some(dir) = std::env::var_os("LM_GALLERY").map(std::path::PathBuf::from) else { return };
        std::fs::create_dir_all(&dir).unwrap();
        for dark in [false, true] {
            let t = Theme::of(dark);
            let mut fonts_set = false;
            let mut h = Harness::builder().with_size(egui::vec2(800.0, 640.0)).with_pixels_per_point(2.0).wgpu().build_ui(move |ui| {
                // The bundled fonts take effect from the next frame.
                if !fonts_set {
                    ui.ctx().set_fonts(crate::theme::fonts());
                    fonts_set = true;
                    return;
                }
                ui.ctx().set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                egui::Frame::new().fill(t.surface).inner_margin(16.0).show(ui, |ui| {
                    ui.columns(2, |cols| {
                        for (i, (id, label)) in GALLERY.iter().enumerate() {
                            let ui = &mut cols[i % 2];
                            super::button(ui, &t, id, label);
                            ui.add_space(4.0);
                        }
                    });
                });
            });
            h.run_steps(4);
            h.render().unwrap().save(dir.join(if dark { "illustrations-dark.png" } else { "illustrations-light.png" })).unwrap();
        }
    }
}
