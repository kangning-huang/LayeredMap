//! Map images at an exact output size (WP5.3). The map is drawn offscreen by RustGIS's CPU
//! renderer at the export's own pixel size (never a screenshot of the window), waiting for its
//! tiles; then the sheet — title, legend, scale bar, caption and credits — is drawn around it with
//! the app's fonts. The export sheet's preview is the same drawing at a smaller scale, so what the
//! preview shows is what is saved.
//!
//! Everything is laid out in points; `ppp` (pixels per point) turns points into the output's
//! pixels. The output is PNG; [`encode_pdf`] wraps the same pixels in a page for Phase 6.

use ab_glyph::{Font, FontArc, FontRef, GlyphId, PxScale, ScaleFont};
use rustgis_map::Map;
use rustgis_map::camera::Camera;
pub use rustgis_render::Pixmap;
use rustgis_render::View;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapPaint, Rect, Stroke, Transform};

/// Space between the map's edge and a card on it (points).
pub const MARGIN: f32 = 16.0;
/// Text size of the caption, credits, legend and scale bar (points).
pub const TEXT: f32 = 12.0;
const LINE: f32 = 16.0;
/// The title's size, and the band it sits in above the map.
pub const TITLE: f32 = 22.0;
pub const TITLE_BAND: f32 = 60.0;
/// The band under the map: up to two lines of caption with the credits.
pub const FOOTER: f32 = 52.0;
/// Side padding of the title and footer bands.
const SIDE: f32 = 24.0;
const PRIMARY: [u8; 4] = [0x1B, 0x1D, 0x21, 0xFF];
const SECONDARY: [u8; 4] = [0x5F, 0x66, 0x70, 0xFF];
const BORDER: [u8; 4] = [0xE3, 0xE5, 0xE8, 0xFF];
const CARD: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xF0];
/// The legend's ramp width (points).
const RAMP_W: f32 = 160.0;
/// Missing data: light gray with a diagonal hatch (Data palettes › Missing data).
pub const NO_DATA: [u8; 4] = [0xD0, 0xD3, 0xD8, 0xFF];

/// A rectangle in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Area {
        Area { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
    /// In pixels at `ppp`, rounded outward to whole pixels.
    fn px(&self, ppp: f32) -> Option<Rect> {
        Rect::from_xywh((self.x * ppp).round(), (self.y * ppp).round(), (self.w * ppp).round().max(1.0), (self.h * ppp).round().max(1.0))
    }
}

/// The app's fonts for the sheet (Inter Regular and SemiBold, then Noto Sans SC for characters
/// Inter lacks).
#[derive(Clone, Copy)]
pub struct Fonts {
    pub regular: &'static [u8],
    pub semibold: &'static [u8],
    pub fallback: &'static [u8],
}

/// How a legend entry's symbol is drawn: as it is on the map.
#[derive(Clone, Debug, PartialEq)]
pub enum Swatch {
    /// An area's fill (with the map's opacity, drawn over the map's ground) and outline.
    Area {
        fill: [u8; 4],
        stroke: [u8; 4],
    },
    Point {
        fill: [u8; 4],
        stroke: [u8; 4],
        diameter: f32,
    },
    Line {
        color: [u8; 4],
        width: f32,
    },
    /// "No data": gray, hatched.
    NoData,
}

/// One legend row.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Item {
        swatch: Swatch,
        label: String,
    },
    /// A continuous ramp from low (left) to high (right), with its end values.
    Ramp {
        stops: Vec<[u8; 4]>,
        low: String,
        high: String,
    },
}

/// One layer's part of the legend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LegendBlock {
    pub title: String,
    pub entries: Vec<Entry>,
}

/// What the exported image shows, and at which size.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    /// The output's size in pixels.
    pub width: u32,
    pub height: u32,
    /// Pixels per point.
    pub ppp: f32,
    /// Empty: no title band.
    pub title: String,
    pub caption: String,
    /// The map's credits (never empty: the basemap's at least).
    pub attribution: String,
    pub legend: Vec<LegendBlock>,
    /// Where the user dragged the legend (its top-left corner, in points); `None` places it.
    pub legend_at: Option<(f32, f32)>,
    /// The scale bar's length in points and its label.
    pub scale: Option<(f32, String)>,
    /// The map's ground colour, under area swatches (their fills are translucent on the map too).
    pub ground: [u8; 4],
}

impl Sheet {
    /// The output's size in points.
    pub fn size_pt(&self) -> (f32, f32) {
        (self.width as f32 / self.ppp, self.height as f32 / self.ppp)
    }

    /// The same sheet `k` times as many pixels across (a preview: `k` < 1). The layout, in
    /// points, stays the same.
    pub fn at_scale(&self, k: f32) -> Sheet {
        let width = ((self.width as f32 * k).round() as u32).max(1);
        let k = width as f32 / self.width as f32;
        Sheet { width, height: ((self.height as f32 * k).round() as u32).max(1), ppp: self.ppp * k, ..self.clone() }
    }
}

/// Where everything sits on the sheet, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    pub title: Option<Area>,
    pub map: Area,
    pub footer: Area,
    pub legend: Option<Area>,
    pub scale: Option<Area>,
}

/// The map's part of a sheet `size` points large, with or without a title band.
pub fn map_area(size: (f32, f32), has_title: bool) -> Area {
    let top = if has_title { TITLE_BAND } else { 0.0 };
    Area::new(0.0, top, size.0, (size.1 - top - FOOTER).max(1.0))
}

/// The map area in whole pixels at `ppp`: the size the map is rendered at.
pub fn map_pixels(area: Area, ppp: f32) -> (u32, u32) {
    (((area.w * ppp).round() as u32).max(1), ((area.h * ppp).round() as u32).max(1))
}

/// The camera that shows the view the user saw (`center` and `res` over `seen` points) whole in
/// an area `area` points large.
pub fn fit_camera(center: rustgis_geom::Coord, res: f64, seen: (f32, f32), area: (f32, f32)) -> Camera {
    let k = (f64::from(seen.0) / f64::from(area.0.max(1.0))).max(f64::from(seen.1) / f64::from(area.1.max(1.0)));
    let k = if k.is_finite() && k > 0.0 { k } else { 1.0 };
    Camera { center, res: res * k }
}

// ---------------------------------------------------------------------------------------------
// Where the data is

/// Which parts of the map area hold data (a coarse grid over it), so the legend keeps clear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Occupancy {
    cols: usize,
    rows: usize,
    cells: Vec<bool>,
}

impl Occupancy {
    /// The share (0–1) of `r` (in points, relative to the map area `map`) that holds data.
    pub fn share(&self, map: Area, r: Area) -> f32 {
        if self.cells.is_empty() || map.w <= 0.0 || map.h <= 0.0 {
            return 0.0;
        }
        let cx = |x: f32| (((x - map.x) / map.w) * self.cols as f32).clamp(0.0, self.cols as f32);
        let cy = |y: f32| (((y - map.y) / map.h) * self.rows as f32).clamp(0.0, self.rows as f32);
        let (c0, c1) = (cx(r.x).floor() as usize, cx(r.right()).ceil() as usize);
        let (r0, r1) = (cy(r.y).floor() as usize, cy(r.bottom()).ceil() as usize);
        let (mut n, mut hit) = (0usize, 0usize);
        for row in r0..r1.min(self.rows) {
            for col in c0..c1.min(self.cols) {
                n += 1;
                hit += usize::from(self.cells.get(row * self.cols + col).copied().unwrap_or(false));
            }
        }
        if n == 0 { 0.0 } else { hit as f32 / n as f32 }
    }

    /// Cells across and down.
    pub fn grid(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }
}

/// Where `map`'s data layers draw, at `camera` over an area `size` points large: the layers drawn
/// small on a blank ground with no tiles, every cell not left blank holding data.
pub fn occupancy(map: &mut Map, camera: Camera, size: (f32, f32)) -> Occupancy {
    const COLS: usize = 48;
    let ppp = (192.0 / size.0.max(1.0)).min(1.0);
    let tiles = std::mem::take(&mut map.tiles);
    let basemap = std::mem::replace(&mut map.basemap, rustgis_map::Basemap::Blank);
    let saved = map.camera;
    map.camera = camera;
    let view = View { width: f64::from(size.0), height: f64::from(size.1), pixels_per_point: f64::from(ppp), camera, dark: false };
    let drawn = rustgis_render::render(map, &view);
    map.tiles = tiles;
    map.basemap = basemap;
    map.camera = saved;
    let Some(r) = drawn else { return Occupancy::default() };
    let (w, h) = (r.pixmap.width() as usize, r.pixmap.height() as usize);
    let rows = ((COLS as f32 * size.1 / size.0.max(1.0)).round() as usize).clamp(1, 4 * COLS);
    let mut cells = vec![false; COLS * rows];
    for (i, px) in r.pixmap.pixels().iter().enumerate() {
        let c = px.demultiply();
        if c.red() < 0xF6 || c.green() < 0xF6 || c.blue() < 0xF6 {
            let (x, y) = (i % w, i / w);
            let (col, row) = (x * COLS / w.max(1), y * rows / h.max(1));
            if let Some(cell) = cells.get_mut(row.min(rows - 1) * COLS + col.min(COLS - 1)) {
                *cell = true;
            }
        }
    }
    Occupancy { cols: COLS, rows, cells }
}

// ---------------------------------------------------------------------------------------------
// The map, offscreen

/// The map drawn for an export.
pub struct MapImage {
    pub pixmap: Pixmap,
    /// The credits the map's tile layers asked for.
    pub attribution: Vec<String>,
    /// Some tiles couldn't be had in time (or at all): the image shows less detail.
    pub tiles_missing: bool,
    pub occupancy: Occupancy,
}

/// Draws `map` offscreen at `camera` into an image `pixels` large at `ppp` pixels per point,
/// waiting (up to a minute) for basemap tiles. Blocking: call it off the interface thread.
/// Selection highlights are not drawn.
pub fn render_map(mut map: Map, camera: Camera, pixels: (u32, u32), ppp: f32) -> Option<MapImage> {
    for l in &mut map.layers {
        l.selection.clear();
    }
    map.camera = camera;
    let size = (pixels.0 as f32 / ppp, pixels.1 as f32 / ppp);
    let view = View { width: f64::from(size.0), height: f64::from(size.1), pixels_per_point: f64::from(ppp), camera, dark: false };
    let r = rustgis_render::render_export(&mut map, &view)?;
    let occupancy = occupancy(&mut map, camera, size);
    Some(MapImage { pixmap: r.pixmap, attribution: r.attribution, tiles_missing: r.tiles_pending || !r.tile_errors.is_empty(), occupancy })
}

/// `pm` reduced to `w` × `h` by averaging (the preview's copy of the map).
pub fn downsample(pm: &Pixmap, w: u32, h: u32) -> Option<Pixmap> {
    let mut out = Pixmap::new(w.max(1), h.max(1))?;
    let (sw, sh) = (pm.width() as usize, pm.height() as usize);
    let (dw, dh) = (out.width() as usize, out.height() as usize);
    let src = pm.data();
    let dst = out.data_mut();
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh / dh).max(y * sh / dh + 1).min(sh));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw / dw).max(x * sw / dw + 1).min(sw));
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (sy * sw + sx) * 4;
                    for (k, a) in acc.iter_mut().enumerate() {
                        *a += u32::from(src.get(i + k).copied().unwrap_or(0));
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)).max(1) as u32;
            let o = (y * dw + x) * 4;
            for (k, a) in acc.iter().enumerate() {
                if let Some(d) = dst.get_mut(o + k) {
                    *d = (a / n) as u8;
                }
            }
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// Text

struct Face {
    regular: Option<FontArc>,
    semibold: Option<FontArc>,
    fallback: Option<FontArc>,
}

impl Face {
    fn new(f: &Fonts) -> Face {
        let load = |b: &'static [u8]| FontRef::try_from_slice(b).ok().map(FontArc::new);
        Face { regular: load(f.regular), semibold: load(f.semibold), fallback: load(f.fallback) }
    }

    fn primary(&self, bold: bool) -> Option<&FontArc> {
        if bold { self.semibold.as_ref().or(self.regular.as_ref()) } else { self.regular.as_ref() }
    }

    /// The font and glyph for `c`: the primary font's, else the fallback's, else the primary's
    /// missing-glyph box.
    fn pick(&self, c: char, bold: bool) -> Option<(&FontArc, GlyphId)> {
        let first = self.primary(bold)?;
        let g = first.glyph_id(c);
        if g.0 != 0 {
            return Some((first, g));
        }
        if let Some(f) = &self.fallback {
            let g = f.glyph_id(c);
            if g.0 != 0 {
                return Some((f, g));
            }
        }
        Some((first, g))
    }

    /// The scale giving `font` an em `px` pixels tall.
    fn scale(font: &FontArc, px: f32) -> PxScale {
        let h = font.height_unscaled();
        let em = font.units_per_em().unwrap_or(h);
        PxScale::from(px * h / em.max(1.0))
    }

    fn width(&self, text: &str, px: f32, bold: bool) -> f32 {
        let mut w = 0.0;
        let mut prev: Option<(*const FontArc, GlyphId)> = None;
        for c in text.chars() {
            let Some((f, g)) = self.pick(c, bold) else { return text.chars().count() as f32 * px * 0.55 };
            let sf = f.as_scaled(Self::scale(f, px));
            if let Some((pf, pg)) = prev
                && std::ptr::eq(pf, f)
            {
                w += sf.kern(pg, g);
            }
            w += sf.h_advance(g);
            prev = Some((f as *const FontArc, g));
        }
        w
    }

    /// Draws one line with its left end at `x` and its middle at `y` (pixels).
    fn draw(&self, pm: &mut Pixmap, text: &str, x: f32, y: f32, px: f32, color: [u8; 4], bold: bool) {
        let Some(first) = self.primary(bold) else { return };
        let sf = first.as_scaled(Self::scale(first, px));
        let base = y + (sf.ascent() + sf.descent()) / 2.0;
        let mut pen = x;
        let mut prev: Option<(*const FontArc, GlyphId)> = None;
        for c in text.chars() {
            let Some((f, g)) = self.pick(c, bold) else { return };
            let scaled = f.as_scaled(Self::scale(f, px));
            if let Some((pf, pg)) = prev
                && std::ptr::eq(pf, f)
            {
                pen += scaled.kern(pg, g);
            }
            let glyph = g.with_scale_and_position(Self::scale(f, px), ab_glyph::point(pen, base));
            pen += scaled.h_advance(g);
            prev = Some((f as *const FontArc, g));
            if let Some(og) = f.outline_glyph(glyph) {
                let bb = og.px_bounds();
                og.draw(|gx, gy, cov| blend(pm, bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32, color, cov));
            }
        }
    }

    /// `text` cut to fit `max` pixels, with an ellipsis when cut.
    fn fit(&self, text: &str, px: f32, bold: bool, max: f32) -> String {
        if self.width(text, px, bold) <= max {
            return text.to_string();
        }
        let mut s: Vec<char> = text.chars().collect();
        while !s.is_empty() {
            s.pop();
            let t: String = s.iter().collect::<String>().trim_end().to_string() + "…";
            if self.width(&t, px, bold) <= max {
                return t;
            }
        }
        String::new()
    }

    /// `text` wrapped to lines at most `max` pixels wide (at spaces; anywhere in text without
    /// them, such as Chinese), at most `lines` of them, the last one cut with an ellipsis.
    fn wrap(&self, text: &str, px: f32, bold: bool, max: f32, lines: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut line = String::new();
        let mut rest = text.trim().to_string();
        while !rest.is_empty() {
            // The next piece: a word with its following space, or one character of text that
            // doesn't use spaces.
            let first = rest.chars().next().unwrap_or(' ');
            let piece: String = if first.is_ascii() || first.is_alphabetic() && first.len_utf8() < 3 {
                let end = rest.find(' ').map_or(rest.len(), |i| i + 1);
                rest[..end].to_string()
            } else {
                first.to_string()
            };
            rest = rest[piece.len()..].to_string();
            let candidate = format!("{line}{piece}");
            if self.width(candidate.trim_end(), px, bold) <= max || line.is_empty() {
                line = candidate;
            } else {
                out.push(line.trim_end().to_string());
                line = piece;
            }
        }
        if !line.trim().is_empty() {
            out.push(line.trim_end().to_string());
        }
        if out.len() > lines {
            let tail = out[lines - 1..].join(" ");
            out.truncate(lines - 1);
            out.push(self.fit(&tail, px, bold, max));
        }
        out.into_iter().map(|l| self.fit(&l, px, bold, max)).collect()
    }
}

/// Blends `color` with coverage `cov` into one pixel.
fn blend(pm: &mut Pixmap, x: i32, y: i32, c: [u8; 4], cov: f32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    if x < 0 || y < 0 || x >= w || y >= h || cov <= 0.0 {
        return;
    }
    let Some(px) = pm.pixels_mut().get_mut((y * w + x) as usize) else { return };
    let a = f32::from(c[3]) / 255.0 * cov.min(1.0);
    let inv = 1.0 - a;
    let ch = |src: u8, dst: u8| (f32::from(src) * a + f32::from(dst) * inv).round() as u8;
    let al = (a * 255.0 + f32::from(px.alpha()) * inv).round().min(255.0) as u8;
    let (r, g, b) = (ch(c[0], px.red()), ch(c[1], px.green()), ch(c[2], px.blue()));
    if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(r.min(al), g.min(al), b.min(al), al) {
        *px = p;
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing

fn paint(c: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c[0], c[1], c[2], c[3]));
    p.anti_alias = true;
    p
}

fn rounded(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let k = radius.min(r.width() / 2.0).min(r.height() / 2.0);
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    let mut pb = PathBuilder::new();
    pb.move_to(l + k, t);
    pb.line_to(rt - k, t);
    pb.quad_to(rt, t, rt, t + k);
    pb.line_to(rt, b - k);
    pb.quad_to(rt, b, rt - k, b);
    pb.line_to(l + k, b);
    pb.quad_to(l, b, l, b - k);
    pb.line_to(l, t + k);
    pb.quad_to(l, t, l + k, t);
    pb.close();
    pb.finish()
}

/// A card on the map: white, a hairline edge, 8 pt corners.
fn card(pm: &mut Pixmap, a: Area, ppp: f32) {
    let Some(r) = a.px(ppp) else { return };
    if let Some(path) = rounded(r, 8.0 * ppp) {
        pm.fill_path(&path, &paint(CARD), FillRule::Winding, Transform::identity(), None);
        pm.stroke_path(&path, &paint(BORDER), &Stroke { width: ppp.max(1.0), ..Stroke::default() }, Transform::identity(), None);
    }
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: [u8; 4]) {
    if let Some(r) = Rect::from_xywh(x, y, w.max(0.5), h.max(0.5)) {
        pm.fill_rect(r, &paint(c), Transform::identity(), None);
    }
}

fn line(pm: &mut Pixmap, a: (f32, f32), b: (f32, f32), width: f32, c: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.0, a.1);
    pb.line_to(b.0, b.1);
    if let Some(p) = pb.finish() {
        pm.stroke_path(&p, &paint(c), &Stroke { width, ..Stroke::default() }, Transform::identity(), None);
    }
}

/// A legend swatch, 14 points square, its middle at (`x` + 7, `y`) in points.
fn swatch(pm: &mut Pixmap, s: &Swatch, x: f32, y: f32, ppp: f32, ground: [u8; 4]) {
    let (x, y, k) = (x * ppp, y * ppp, ppp);
    let side = 14.0 * k;
    let (left, top) = (x, y - side / 2.0);
    match s {
        Swatch::Area { fill, stroke } => {
            fill_rect(pm, left, top, side, side, ground);
            fill_rect(pm, left, top, side, side, *fill);
            if let Some(r) = Rect::from_xywh(left + 0.5 * k, top + 0.5 * k, side - k, side - k) {
                let path = PathBuilder::from_rect(r);
                pm.stroke_path(&path, &paint(*stroke), &Stroke { width: k, ..Stroke::default() }, Transform::identity(), None);
            }
        }
        Swatch::Point { fill, stroke, diameter } => {
            let d = diameter.clamp(4.0, 16.0) * k;
            if let Some(c) = PathBuilder::from_circle(x + side / 2.0, y, d / 2.0) {
                pm.fill_path(&c, &paint(*fill), FillRule::Winding, Transform::identity(), None);
                pm.stroke_path(&c, &paint(*stroke), &Stroke { width: 1.5 * k, ..Stroke::default() }, Transform::identity(), None);
            }
        }
        Swatch::Line { color, width } => line(pm, (left, y), (left + side, y), width.clamp(1.0, 4.0) * k, *color),
        Swatch::NoData => {
            fill_rect(pm, left, top, side, side, NO_DATA);
            // The diagonal hatch, clipped to the square.
            let step = 4.0 * k;
            let mut c = step;
            while c < 2.0 * side {
                let (u0, u1) = ((c - side).max(0.0), c.min(side));
                line(pm, (left + u0, top + side - (c - u0)), (left + u1, top + side - (c - u1)), k, [0x9A, 0x9A, 0x9A, 0xFF]);
                c += step;
            }
        }
    }
}

/// The legend's size in points.
fn legend_size(face: &Face, blocks: &[LegendBlock], max_w: f32) -> (f32, f32) {
    let px = TEXT;
    let mut w: f32 = 120.0;
    let mut h = 12.0;
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            h += 10.0;
        }
        if !b.title.is_empty() {
            w = w.max(face.width(&b.title, px, true));
            h += LINE + 4.0;
        }
        for e in &b.entries {
            match e {
                Entry::Item { label, .. } => {
                    w = w.max(22.0 + face.width(label, px, false));
                    h += 20.0;
                }
                Entry::Ramp { .. } => {
                    w = w.max(RAMP_W);
                    h += 12.0 + LINE + 4.0;
                }
            }
        }
    }
    ((w + 24.0).min(max_w), h + 12.0)
}

fn draw_legend(pm: &mut Pixmap, face: &Face, blocks: &[LegendBlock], a: Area, ppp: f32, ground: [u8; 4]) {
    card(pm, a, ppp);
    let px = TEXT * ppp;
    let inner = a.w - 24.0;
    let (x, mut y) = (a.x + 12.0, a.y + 12.0);
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            y += 10.0;
        }
        if !b.title.is_empty() {
            let t = face.fit(&b.title, px, true, inner * ppp);
            face.draw(pm, &t, x * ppp, (y + LINE / 2.0) * ppp, px, PRIMARY, true);
            y += LINE + 4.0;
        }
        for e in &b.entries {
            match e {
                Entry::Item { swatch: s, label } => {
                    swatch(pm, s, x, y + 10.0, ppp, ground);
                    let t = face.fit(label, px, false, (inner - 22.0) * ppp);
                    face.draw(pm, &t, (x + 22.0) * ppp, (y + 10.0) * ppp, px, PRIMARY, false);
                    y += 20.0;
                }
                Entry::Ramp { stops, low, high } => {
                    let n = stops.len().max(1);
                    let w = RAMP_W.min(inner) / n as f32;
                    for (k, c) in stops.iter().enumerate() {
                        fill_rect(pm, (x + k as f32 * w) * ppp, (y + 1.0) * ppp, (w * ppp).ceil() + 0.5, 10.0 * ppp, *c);
                    }
                    y += 12.0;
                    face.draw(pm, low, x * ppp, (y + LINE / 2.0) * ppp, px, SECONDARY, false);
                    let hw = face.width(high, px, false);
                    face.draw(pm, high, (x + RAMP_W.min(inner)) * ppp - hw, (y + LINE / 2.0) * ppp, px, SECONDARY, false);
                    y += LINE + 4.0;
                }
            }
        }
    }
}

/// The scale bar card's size for a bar `len` points long.
fn scale_size(len: f32) -> (f32, f32) {
    (len + 12.0, 30.0)
}

/// Lays the sheet out: the bands, the map, the scale bar at the map's bottom left, and the legend
/// where it was dragged, else bottom left above the scale bar unless data is there (then the
/// corner with the least data).
pub fn layout(sheet: &Sheet, fonts: &Fonts, occupancy: Option<&Occupancy>) -> Layout {
    let face = Face::new(fonts);
    layout_with(sheet, &face, occupancy)
}

fn layout_with(sheet: &Sheet, face: &Face, occupancy: Option<&Occupancy>) -> Layout {
    let size = sheet.size_pt();
    let has_title = !sheet.title.trim().is_empty();
    let map = map_area(size, has_title);
    let title = has_title.then(|| Area::new(0.0, 0.0, size.0, TITLE_BAND));
    let footer = Area::new(0.0, map.bottom(), size.0, size.1 - map.bottom());
    let scale = sheet.scale.as_ref().map(|(len, _)| {
        let (w, h) = scale_size(*len);
        Area::new(map.x + MARGIN, map.bottom() - MARGIN - h, w, h)
    });
    let legend = (!sheet.legend.is_empty()).then(|| {
        let (w, h) = legend_size(face, &sheet.legend, (map.w * 0.45).max(140.0));
        let clamp =
            |x: f32, y: f32| Area::new(x.clamp(map.x, (map.right() - w).max(map.x)), y.clamp(map.y, (map.bottom() - h).max(map.y)), w, h);
        if let Some((x, y)) = sheet.legend_at {
            return clamp(x, y);
        }
        let above_scale = scale.map_or(map.bottom() - MARGIN, |s| s.y - 8.0);
        let candidates = [
            clamp(map.x + MARGIN, above_scale - h),
            clamp(map.x + MARGIN, map.y + MARGIN),
            clamp(map.right() - MARGIN - w, map.bottom() - MARGIN - h),
            clamp(map.right() - MARGIN - w, map.y + MARGIN),
        ];
        let Some(occ) = occupancy else { return candidates[0] };
        // The first corner (bottom left first) with almost no data, else the least covered.
        candidates.iter().find(|c| occ.share(map, **c) < 0.03).copied().unwrap_or_else(|| {
            let mut best = candidates[0];
            for c in &candidates[1..] {
                if occ.share(map, *c) < occ.share(map, best) - 0.05 {
                    best = *c;
                }
            }
            best
        })
    });
    Layout { title, map, footer, legend, scale }
}

/// The footer's lines: caption lines and the credits, each with its baseline row (0 or 1).
fn footer_lines(face: &Face, sheet: &Sheet, width: f32) -> (Vec<String>, String, usize) {
    let px = TEXT;
    let inner = width - 2.0 * SIDE;
    let credit_w = face.width(&sheet.attribution, px, false);
    // Credits that would leave the caption less than half the width get a line of their own.
    if credit_w > inner * 0.5 {
        let credits = face.fit(&sheet.attribution, px, false, inner);
        let caption = face.wrap(&sheet.caption, px, false, inner, 1);
        (caption, credits, 1)
    } else {
        let caption = face.wrap(&sheet.caption, px, false, inner - credit_w - 24.0, 2);
        let row = caption.len().saturating_sub(1);
        (caption, sheet.attribution.clone(), row)
    }
}

/// Draws the whole sheet: `map` (the map area's pixels at the sheet's scale) and everything
/// around and on it.
pub fn compose(map: &Pixmap, sheet: &Sheet, fonts: &Fonts, occupancy: Option<&Occupancy>) -> Option<(Pixmap, Layout)> {
    let face = Face::new(fonts);
    let lay = layout_with(sheet, &face, occupancy);
    let ppp = sheet.ppp;
    let mut pm = Pixmap::new(sheet.width, sheet.height)?;
    pm.fill(Color::WHITE);
    // The map, scaled to its area when its pixel size differs by rounding.
    if let Some(r) = lay.map.px(ppp) {
        let (sx, sy) = (r.width() / map.width() as f32, r.height() / map.height() as f32);
        let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..PixmapPaint::default() };
        pm.draw_pixmap(0, 0, map.as_ref(), &paint, Transform::from_row(sx, 0.0, 0.0, sy, r.left(), r.top()), None);
    }
    if let Some(t) = lay.title {
        let px = TITLE * ppp;
        let text = face.fit(sheet.title.trim(), px, true, (t.w - 2.0 * SIDE) * ppp);
        face.draw(&mut pm, &text, SIDE * ppp, (t.y + t.h / 2.0 + 2.0) * ppp, px, PRIMARY, true);
        line(&mut pm, (0.0, t.bottom() * ppp), (t.w * ppp, t.bottom() * ppp), ppp.max(1.0), BORDER);
    }
    // Footer: caption at the left, credits at the right of the last caption line (or a line of
    // their own when long).
    let f = lay.footer;
    line(&mut pm, (0.0, f.y * ppp), (f.w * ppp, f.y * ppp), ppp.max(1.0), BORDER);
    let (caption, credits, credit_row) = footer_lines(&face, sheet, f.w);
    let rows = caption.len().max(credit_row + 1).max(1);
    let top = f.y + (f.h - rows as f32 * LINE) / 2.0;
    let px = TEXT * ppp;
    for (i, l) in caption.iter().enumerate() {
        face.draw(&mut pm, l, SIDE * ppp, (top + (i as f32 + 0.5) * LINE) * ppp, px, SECONDARY, false);
    }
    let cw = face.width(&credits, px, false);
    face.draw(&mut pm, &credits, (f.w - SIDE) * ppp - cw, (top + (credit_row as f32 + 0.5) * LINE) * ppp, px, SECONDARY, false);
    // Scale bar: a line with end ticks and its distance, as on screen.
    if let (Some(a), Some((len, label))) = (lay.scale, sheet.scale.as_ref()) {
        card(&mut pm, a, ppp);
        let (x0, y) = ((a.x + 6.0) * ppp, (a.bottom() - 6.0) * ppp);
        let x1 = x0 + len * ppp;
        let w = 1.5 * ppp;
        line(&mut pm, (x0, y), (x1, y), w, PRIMARY);
        line(&mut pm, (x0, y + w / 2.0), (x0, y - 5.0 * ppp), w, PRIMARY);
        line(&mut pm, (x1, y + w / 2.0), (x1, y - 5.0 * ppp), w, PRIMARY);
        face.draw(&mut pm, label, x0, y - 13.0 * ppp, px, PRIMARY, false);
    }
    if let Some(a) = lay.legend {
        draw_legend(&mut pm, &face, &sheet.legend, a, ppp, sheet.ground);
    }
    Some((pm, lay))
}

// ---------------------------------------------------------------------------------------------
// Output

/// The image as PNG bytes.
pub fn encode_png(pm: &Pixmap) -> Result<Vec<u8>, String> {
    pm.encode_png().map_err(|e| e.to_string())
}

/// The image as a one-page PDF `width_mm` × `height_mm` (PDF export arrives in Phase 6).
pub fn encode_pdf(pm: &Pixmap, width_mm: f64, height_mm: f64, title: &str) -> Vec<u8> {
    rustgis_render::pdf::write(&[rustgis_render::pdf::Page::new(pm, width_mm, height_mm)], title)
}

/// The image for an egui texture.
pub fn color_image(pm: &Pixmap) -> egui::ColorImage {
    egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data())
}

/// The colour of one pixel (unpremultiplied RGBA); `None` outside the image.
pub fn pixel(pm: &Pixmap, x: u32, y: u32) -> Option<[u8; 4]> {
    let c = pm.pixel(x, y)?.demultiply();
    Some([c.red(), c.green(), c.blue(), c.alpha()])
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
    const INTER_SB: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
    const NOTO: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-Regular.otf");
    const FONTS: Fonts = Fonts { regular: INTER, semibold: INTER_SB, fallback: NOTO };

    fn sheet(width: u32, height: u32, ppp: f32) -> Sheet {
        Sheet {
            width,
            height,
            ppp,
            title: "Heat study".into(),
            caption: "Data: Districts (districts.geojson). Made with LayeredMap.".into(),
            attribution: "Natural Earth".into(),
            legend: vec![LegendBlock {
                title: "Population".into(),
                entries: vec![
                    Entry::Item {
                        swatch: Swatch::Area { fill: [200, 30, 30, 255], stroke: [140, 20, 20, 255] },
                        label: "Over 1,000,000".into(),
                    },
                    Entry::Item { swatch: Swatch::NoData, label: "No data".into() },
                ],
            }],
            legend_at: None,
            scale: Some((100.0, "10 km".into())),
            ground: [0xF6, 0xF5, 0xF1, 0xFF],
        }
    }

    fn blank(w: u32, h: u32) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::from_rgba8(0xF6, 0xF5, 0xF1, 0xFF));
        p
    }

    #[test]
    fn the_layout_keeps_bands_and_cards_inside() {
        let s = sheet(1920, 1080, 1.5);
        let lay = layout(&s, &FONTS, None);
        let (w, h) = s.size_pt();
        assert_eq!(lay.title.map(|t| t.h), Some(TITLE_BAND));
        assert!((lay.map.bottom() + FOOTER - h).abs() < 1e-3);
        let legend = lay.legend.unwrap();
        let scale = lay.scale.unwrap();
        // Legend bottom left, above the scale bar; both inside the map.
        assert!(legend.x < w / 2.0 && legend.bottom() <= scale.y);
        assert!(lay.map.contains(legend.x, legend.y) && lay.map.contains(scale.right(), scale.bottom()));
        // No title: the map starts at the top.
        let lay = layout(&Sheet { title: "  ".into(), ..s }, &FONTS, None);
        assert!(lay.title.is_none() && lay.map.y == 0.0);
    }

    #[test]
    fn the_legend_moves_off_the_data_and_stays_where_dragged() {
        let s = sheet(1080, 1080, 1.5);
        let map = map_area(s.size_pt(), true);
        // Data along the whole left half.
        let cols = 48;
        let rows = 48;
        let cells = (0..rows * cols).map(|i| i % cols < cols / 2).collect();
        let occ = Occupancy { cols, rows, cells };
        let lay = layout(&s, &FONTS, Some(&occ));
        let legend = lay.legend.unwrap();
        assert!(legend.x > map.w / 2.0, "moved right: {legend:?}");
        assert!(occ.share(map, legend) < 0.03);
        let dragged = layout(&Sheet { legend_at: Some((40.0, 200.0)), ..s.clone() }, &FONTS, Some(&occ)).legend.unwrap();
        assert_eq!((dragged.x, dragged.y), (40.0, 200.0));
        // Dragged too far: kept inside the map.
        let far = layout(&Sheet { legend_at: Some((5000.0, -50.0)), ..s }, &FONTS, None).legend.unwrap();
        assert!(far.right() <= map.right() + 1e-3 && far.y >= map.y);
    }

    #[test]
    fn composing_gives_the_exact_size_with_a_title_and_credits() {
        let s = sheet(1920, 1080, 1.5);
        let lay = layout(&s, &FONTS, None);
        let (mw, mh) = map_pixels(lay.map, s.ppp);
        let (pm, lay) = compose(&blank(mw, mh), &s, &FONTS, None).unwrap();
        assert_eq!((pm.width(), pm.height()), (1920, 1080));
        // Dark text in the title band and in the footer's right end (the credits).
        let dark_in = |a: Area| {
            let r = a.px(s.ppp).unwrap();
            (r.top() as u32..r.bottom() as u32)
                .flat_map(|y| (r.left() as u32..r.right() as u32).map(move |x| (x, y)))
                .filter(|(x, y)| pixel(&pm, *x, *y).is_some_and(|c| c[0] < 160))
                .count()
        };
        assert!(dark_in(lay.title.unwrap()) > 100);
        let f = lay.footer;
        assert!(dark_in(Area::new(f.w * 0.8, f.y, f.w * 0.2, f.h)) > 20, "credits at the right of the footer");
        // The legend's area swatch: the fill over the ground, exactly.
        let legend = lay.legend.unwrap();
        let (sx, sy) = (((legend.x + 12.0 + 7.0) * s.ppp) as u32, ((legend.y + 12.0 + LINE + 4.0 + 10.0) * s.ppp) as u32);
        assert_eq!(pixel(&pm, sx, sy), Some([200, 30, 30, 255]));
    }

    #[test]
    fn the_preview_is_the_same_layout_smaller() {
        let s = sheet(2480, 3508, 2480.0 / 793.7);
        let p = s.at_scale(0.2);
        assert_eq!(p.width, 496);
        let (a, b) = (layout(&s, &FONTS, None), layout(&p, &FONTS, None));
        let close = |x: f32, y: f32| (x - y).abs() < 1.0;
        assert!(close(a.map.h, b.map.h) && close(a.legend.unwrap().y, b.legend.unwrap().y));
        let small = downsample(&blank(400, 300), 40, 30).unwrap();
        assert_eq!(pixel(&small, 10, 10), Some([0xF6, 0xF5, 0xF1, 0xFF]));
    }

    #[test]
    fn long_captions_wrap_and_long_credits_get_their_own_line() {
        let face = Face::new(&FONTS);
        let long =
            "Data: Districts (districts.geojson); Heat (summer surface temperature, 2024); Schools (schools.csv). Made with LayeredMap.";
        let lines = face.wrap(long, TEXT, false, 300.0, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| face.width(l, TEXT, false) <= 300.0));
        assert!(lines[1].ends_with('…'));
        let zh = face.wrap("数据：上海市各区人口与夏季地表温度，由某某大学提供。使用 LayeredMap 制作。", TEXT, false, 120.0, 3);
        assert!(zh.len() >= 2);
        let s = Sheet {
            attribution: "Sentinel-2 cloudless by EOX (Copernicus Sentinel data 2024) · Natural Earth".into(),
            ..sheet(1080, 1080, 1.5)
        };
        let (_, _, row) = footer_lines(&face, &s, 400.0);
        assert_eq!(row, 1);
    }

    #[test]
    fn a_map_is_drawn_offscreen_at_the_asked_size() {
        let mut map = Map::new("t");
        map.basemap = rustgis_map::Basemap::Paper;
        let cam = Camera { center: rustgis_geom::lonlat_to_merc(rustgis_geom::Coord::new(121.47, 31.23)), res: 300.0 };
        let img = render_map(map, cam, (900, 600), 1.5).unwrap();
        assert_eq!((img.pixmap.width(), img.pixmap.height()), (900, 600));
        assert!(!img.tiles_missing);
        // No data layers: nothing occupied.
        assert_eq!(img.occupancy.share(Area::new(0.0, 0.0, 600.0, 400.0), Area::new(0.0, 0.0, 600.0, 400.0)), 0.0);
    }

    #[test]
    fn fitting_shows_the_whole_view() {
        let c = rustgis_geom::Coord::new(0.0, 0.0);
        // A wide screen view into a tall page: the page zooms out by the width ratio.
        let cam = fit_camera(c, 100.0, (1200.0, 700.0), (600.0, 900.0));
        assert!((cam.res - 200.0).abs() < 1e-9);
    }

    #[test]
    fn png_and_pdf_come_out() {
        let pm = blank(30, 20);
        let png = encode_png(&pm).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(encode_pdf(&pm, 297.0, 210.0, "t").starts_with(b"%PDF"));
    }
}
