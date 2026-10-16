//! Text for exported images: label rasterisation (with halos) and the scale bar.

use std::sync::OnceLock;

use ab_glyph::{Font, FontArc, FontRef, GlyphId, PxScale, ScaleFont};
use tiny_skia::{Paint, Pixmap, Rect, Transform};

use crate::{PlacedLabel, View};
use rustgis_map::Rgba;

/// The interface font (`bold` or regular).
fn primary(bold: bool) -> Option<&'static FontArc> {
    static R: OnceLock<Option<FontArc>> = OnceLock::new();
    static B: OnceLock<Option<FontArc>> = OnceLock::new();
    let f = crate::fonts::ui_fonts();
    let load = |bytes: &'static [u8]| FontRef::try_from_slice(bytes).ok().map(FontArc::new);
    if bold { B.get_or_init(|| load(&f.semibold)).as_ref() } else { R.get_or_init(|| load(&f.regular)).as_ref() }
}

/// The fonts text is drawn with, in the order a character is looked for (see [`crate::fonts`]).
struct Chain {
    /// The interface font, then the registered fallbacks.
    fonts: Vec<FontArc>,
    /// Whether the installed CJK font comes last (looked up only when a character needs it).
    system: bool,
}

impl Chain {
    fn new(bold: bool) -> Chain {
        let mut fonts: Vec<FontArc> = primary(bold).into_iter().cloned().collect();
        fonts.extend(crate::fonts::fallbacks());
        Chain { fonts, system: true }
    }

    /// The first font with a glyph for `c`: its place in the chain, the font and the glyph.
    fn find(&self, c: char) -> Option<(usize, &FontArc, GlyphId)> {
        let system = std::iter::once_with(|| if self.system { crate::fonts::system_cjk() } else { None }).flatten();
        self.fonts.iter().chain(system).enumerate().find_map(|(i, f)| {
            let g = f.glyph_id(c);
            (g.0 != 0).then_some((i, f, g))
        })
    }

    /// The glyph to draw `c` with: from the first font that has it, else the first font's
    /// missing-glyph box. `None` without any font.
    fn pick(&self, c: char) -> Option<(usize, &FontArc, GlyphId)> {
        let first = self.fonts.first()?;
        Some(self.find(c).unwrap_or((0, first, GlyphId(0))))
    }

    /// The scale that gives `font` the same em size as the first font has at `px` (fonts differ
    /// in how tall their ascent and descent are for the same em).
    fn scale(&self, font: &FontArc, px: f32) -> PxScale {
        let em = |f: &FontArc| {
            let h = f.height_unscaled();
            let r = f.units_per_em().unwrap_or(h) / h;
            if r.is_finite() && r > 0.0 { r } else { 1.0 }
        };
        match self.fonts.first() {
            Some(first) => PxScale::from(px * em(first) / em(font)),
            None => PxScale::from(px),
        }
    }

    fn covers(&self, text: &str) -> bool {
        if self.fonts.is_empty() {
            return true;
        }
        let invisible = |c: char| {
            c.is_whitespace()
                || c.is_control()
                || matches!(c, '\u{AD}' | '\u{200B}'..='\u{200F}' | '\u{2060}'..='\u{2064}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}')
        };
        text.chars().all(|c| invisible(c) || self.find(c).is_some())
    }

    /// Width in pixels at `px`, kerning pairs from the same font.
    fn width(&self, text: &str, px: f32, kern: bool) -> Option<f32> {
        let mut w = 0.0;
        let mut prev: Option<(usize, GlyphId)> = None;
        for ch in text.chars().take(500) {
            let (i, f, id) = self.pick(ch)?;
            let sf = f.as_scaled(self.scale(f, px));
            if kern && let Some((_, p)) = prev.filter(|(pi, _)| *pi == i) {
                w += sf.kern(p, id);
            }
            w += sf.h_advance(id);
            prev = Some((i, id));
        }
        Some(w)
    }
}

/// Whether the label fonts (`bold` or regular, with their fallbacks) have a glyph for every
/// character of `text`, spaces and invisible characters (zero-width joiners, variation selectors,
/// soft hyphens) aside.
pub fn covers(text: &str, bold: bool) -> bool {
    Chain::new(bold).covers(text)
}

/// Text width in points at `size` (font px ≈ size × 1.33 for point sizes).
pub fn measure(text: &str, size: f32, bold: bool) -> f32 {
    Chain::new(bold).width(text, size * 1.33, true).unwrap_or(text.chars().count() as f32 * size * 0.6)
}

fn blend(pm: &mut Pixmap, x: i32, y: i32, c: Rgba, cov: f32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    if x < 0 || y < 0 || x >= w || y >= h || cov <= 0.0 {
        return;
    }
    let idx = (y * w + x) as usize;
    let data = pm.pixels_mut();
    let Some(px) = data.get_mut(idx) else { return };
    let a = (c.a() as f32 / 255.0) * cov.min(1.0);
    let inv = 1.0 - a;
    let r = (c.r() as f32 * a + px.red() as f32 * inv).round() as u8;
    let g = (c.g() as f32 * a + px.green() as f32 * inv).round() as u8;
    let b = (c.b() as f32 * a + px.blue() as f32 * inv).round() as u8;
    let al = ((a * 255.0) + px.alpha() as f32 * inv).round().min(255.0) as u8;
    if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(r.min(al), g.min(al), b.min(al), al) {
        *px = p;
    }
}

fn draw_text(pm: &mut Pixmap, text: &str, cx: f32, cy: f32, px_size: f32, color: Rgba, bold: bool, offset: (f32, f32)) {
    let chain = Chain::new(bold);
    let Some(first) = chain.fonts.first() else { return };
    let Some(width) = chain.width(text, px_size, false) else { return };
    // The line sits on the interface font's metrics, whichever fonts its characters come from.
    let sf = first.as_scaled(PxScale::from(px_size));
    let mut x = cx - width / 2.0 + offset.0;
    let base = cy + (sf.ascent() + sf.descent()) / 2.0 + offset.1;
    for ch in text.chars().take(500) {
        let Some((_, f, id)) = chain.pick(ch) else { return };
        let scale = chain.scale(f, px_size);
        let g = id.with_scale_and_position(scale, ab_glyph::point(x, base));
        x += f.as_scaled(scale).h_advance(id);
        if let Some(og) = f.outline_glyph(g) {
            let bb = og.px_bounds();
            og.draw(|gx, gy, cov| blend(pm, bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32, color, cov));
        }
    }
}

/// Draws a placed label (logical coordinates) with its halo.
pub fn draw(pm: &mut Pixmap, l: &PlacedLabel, ppp: f32) {
    let px = l.size * 1.33 * ppp;
    let (cx, cy) = (l.center_x() * ppp, l.y * ppp);
    if let Some(h) = l.halo {
        let r = 1.2 * ppp;
        for (dx, dy) in
            [(-r, 0.0), (r, 0.0), (0.0, -r), (0.0, r), (-r * 0.7, -r * 0.7), (r * 0.7, r * 0.7), (-r * 0.7, r * 0.7), (r * 0.7, -r * 0.7)]
        {
            draw_text(pm, &l.text, cx, cy, px, h, l.bold, (dx, dy));
        }
    }
    draw_text(pm, &l.text, cx, cy, px, l.color, l.bold, (0.0, 0.0));
}

/// A two-tone scale bar in the lower-left corner.
/// `ground_per_pt` is ground metres per logical pixel at the centre of view.
pub fn scale_bar(pm: &mut Pixmap, view: &View, ground_per_pt: f64) {
    let ppp = view.pixels_per_point as f32;
    let target = rustgis_geom::nice_distance(ground_per_pt * 120.0);
    if target <= 0.0 {
        return;
    }
    let len = (target / ground_per_pt) as f32;
    let (x0, y0) = (16.0 * ppp, pm.height() as f32 - 22.0 * ppp);
    let mut bg = Paint::default();
    bg.set_color_rgba8(255, 255, 255, 210);
    if let Some(r) = Rect::from_xywh(x0 - 6.0 * ppp, y0 - 18.0 * ppp, len * ppp + 12.0 * ppp, 30.0 * ppp) {
        pm.fill_rect(r, &bg, Transform::identity(), None);
    }
    for i in 0..4 {
        let mut p = Paint::default();
        if i % 2 == 0 {
            p.set_color_rgba8(40, 40, 40, 255);
        } else {
            p.set_color_rgba8(250, 250, 250, 255);
        }
        if let Some(r) = Rect::from_xywh(x0 + len * ppp * i as f32 / 4.0, y0, len * ppp / 4.0, 5.0 * ppp) {
            pm.fill_rect(r, &p, Transform::identity(), None);
        }
    }
    let label = PlacedLabel {
        text: rustgis_geom::format_distance(target),
        x: (x0 / ppp) + len / 2.0,
        y: y0 / ppp - 8.0,
        size: 8.0,
        color: Rgba::rgb(30, 30, 30),
        halo: None,
        bold: false,
        icon: None,
        align: 0.0,
    };
    draw(pm, &label, ppp);
}

/// Horizontal alignment for [`text_at`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
}

/// Draws one line of text at pixel position (`x`, `y`) (`y` is the line's middle), `px_size`
/// pixels tall, optionally with a white halo.
pub fn text_at(pm: &mut Pixmap, text: &str, x: f32, y: f32, px_size: f32, color: Rgba, bold: bool, align: Align, halo: bool) {
    if !(x.is_finite() && y.is_finite() && px_size.is_finite()) || px_size <= 0.0 {
        return;
    }
    let cx = match align {
        Align::Center => x,
        Align::Left => x + measure(text, px_size / 1.33, bold) / 2.0,
    };
    if halo {
        let r = (px_size * 0.09).max(1.0);
        for (dx, dy) in [(-r, 0.0), (r, 0.0), (0.0, -r), (0.0, r)] {
            draw_text(pm, text, cx, y, px_size, Rgba::new(255, 255, 255, 230), bold, (dx, dy));
        }
    }
    draw_text(pm, text, cx, y, px_size, color, bold, (0.0, 0.0));
}

#[cfg(test)]
mod coverage_tests {
    #[test]
    fn knows_which_characters_it_can_draw() {
        // Which scripts are covered depends on the fonts this computer has (a system font may
        // cover Arabic); Latin always is, and a private-use character never is.
        for bold in [false, true] {
            assert!(super::covers("Syria", bold));
            assert!(super::covers("Türkiye Ελλάδα Беларусь", bold));
            assert!(!super::covers("Syria \u{10FFFD}", bold));
            // Invisible characters don't need a glyph.
            assert!(super::covers("Syria\u{200B}\u{FE0F}", bold));
        }
    }

    use ab_glyph::{Font, FontArc, FontRef};

    use super::Chain;

    static NOTO_SC: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-Regular.otf");

    fn ubuntu() -> FontArc {
        FontArc::new(FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).unwrap())
    }

    fn noto_sc() -> FontArc {
        FontArc::new(FontRef::try_from_slice(NOTO_SC).unwrap())
    }

    #[test]
    fn falls_back_on_a_cjk_font() {
        // Without a CJK font, a Chinese-only name is not drawable (labels drop it).
        let latin = Chain { fonts: vec![ubuntu()], system: false };
        assert!(latin.covers("Shanghai"));
        assert!(!latin.covers("上海市"));
        assert!(!latin.covers("Shanghai 上海市"));
        // With one after the Latin font, both scripts are, each from its own font.
        let both = Chain { fonts: vec![ubuntu(), noto_sc()], system: false };
        assert!(both.covers("上海市") && both.covers("Shanghai 上海市"));
        assert_eq!(both.find('S').map(|(i, ..)| i), Some(0));
        assert_eq!(both.find('上').map(|(i, ..)| i), Some(1));
        assert!(!both.covers("上海 \u{10FFFD}"));
        // Chinese characters are about one em wide at the Latin font's em size, not missing-glyph
        // boxes.
        let w = both.width("上海市", 20.0, true).unwrap();
        let u = ubuntu();
        let em = 20.0 * u.units_per_em().unwrap() / u.height_unscaled();
        assert!((w - 3.0 * em).abs() < 0.2 * em, "{w} vs 3 × {em}");
    }

    #[test]
    fn registered_fallbacks_draw_cjk() {
        assert!(!crate::fonts::add_fallback(b"not a font".to_vec(), 0));
        assert!(!crate::fonts::add_fallback(NOTO_SC.to_vec(), 7), "no such face");
        assert!(crate::fonts::add_fallback(NOTO_SC.to_vec(), 0));
        for bold in [false, true] {
            assert!(super::covers("上海市", bold));
            assert!(super::covers("Syria", bold) && !super::covers("Syria \u{10FFFD}", bold));
        }
        // And it is drawn: ink where the text is.
        let mut pm = tiny_skia::Pixmap::new(120, 40).unwrap();
        super::text_at(&mut pm, "上海市", 60.0, 20.0, 24.0, rustgis_map::Rgba::rgb(0, 0, 0), false, super::Align::Center, false);
        let ink = pm.pixels().iter().filter(|p| p.alpha() > 128).count();
        assert!(ink > 100, "{ink} pixels inked");
    }
}
