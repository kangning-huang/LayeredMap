//! The sheet as a PDF page (WP6.5). [`PdfCanvas`] takes the same drawing calls as the PNG (in
//! the output's pixels, y down) and writes them as PDF operators under one transform that maps
//! the sheet's pixels onto the page in points, so the two outputs can't drift apart:
//!
//! - shapes (cards, rules, swatches, the scale bar, the north arrow, the frame) become paths;
//! - text becomes real text in the sheet's own fonts — Inter, and Noto Sans SC for Chinese and
//!   anything else Inter lacks — embedded as subsets (CID fonts, Identity-H, with a ToUnicode map
//!   so it can be selected, searched and copied). Both fonts are under the SIL Open Font Licence,
//!   which allows embedding them in documents;
//! - the map is one RGB image at the sheet's full resolution.
//!
//! Translucent colours use one ExtGState per alpha value.

use std::collections::BTreeMap;

use ab_glyph::Font;
use pdf_writer::types::{CidFontType, FontFlags, LineCapStyle, LineJoinStyle, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PdfRect, Ref, Str, TextStr};
use tiny_skia::{LineCap, LineJoin, PathSegment, Pixmap, Rect, Stroke};

use super::{Canvas, Face, Fonts, Placed};

/// The map's image, compressed as it is drawn.
struct Image {
    width: u32,
    height: u32,
    /// Zlib-compressed RGB.
    rgb_z: Vec<u8>,
}

/// The glyphs one font has shown: their ids in the subset (also their CIDs), widths and text.
#[derive(Default)]
struct FontUse {
    remapper: subsetter::GlyphRemapper,
    /// New glyph id → (advance in thousandths of an em, the text it shows).
    glyphs: BTreeMap<u16, (f32, String)>,
}

/// A PDF page being drawn: the content stream and what it uses.
pub(super) struct PdfCanvas {
    content: Content,
    /// The page in points.
    page: (f32, f32),
    fonts: [FontUse; 3],
    /// Alpha values used, each with its graphics state `/A<n>`.
    alphas: Vec<u8>,
    images: Vec<Image>,
}

fn zlib(bytes: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(bytes, 6)
}

/// A colour component, 0–1.
fn unit(v: u8) -> f32 {
    f32::from(v) / 255.0
}

impl PdfCanvas {
    /// A page `page_mm` large for a sheet `width` × `height` pixels: the sheet fills the page.
    pub(super) fn new(width: u32, height: u32, page_mm: (f64, f64)) -> PdfCanvas {
        let mm = |v: f64| if v.is_finite() && v > 0.0 { v.min(5000.0) } else { 210.0 };
        let page = ((mm(page_mm.0) * 72.0 / 25.4) as f32, (mm(page_mm.1) * 72.0 / 25.4) as f32);
        let mut content = Content::new();
        // Sheet pixels (y down) → page points (y up).
        let (kx, ky) = (page.0 / width.max(1) as f32, page.1 / height.max(1) as f32);
        content.save_state();
        content.transform([kx, 0.0, 0.0, -ky, 0.0, page.1]);
        PdfCanvas { content, page, fonts: Default::default(), alphas: Vec::new(), images: Vec::new() }
    }

    /// Sets the alpha for what follows (`/A<n> gs`), when not opaque.
    fn alpha(&mut self, a: u8) {
        let i = match self.alphas.iter().position(|x| *x == a) {
            Some(i) => i,
            None => {
                self.alphas.push(a);
                self.alphas.len() - 1
            }
        };
        let name = format!("A{i}");
        self.content.set_parameters(Name(name.as_bytes()));
    }

    /// Writes `path` as PDF path operators (quadratic curves raised to cubic).
    fn path(&mut self, path: &tiny_skia::Path) {
        let mut last = (0.0f32, 0.0f32);
        let mut start = last;
        for seg in path.segments() {
            match seg {
                PathSegment::MoveTo(p) => {
                    self.content.move_to(p.x, p.y);
                    last = (p.x, p.y);
                    start = last;
                }
                PathSegment::LineTo(p) => {
                    self.content.line_to(p.x, p.y);
                    last = (p.x, p.y);
                }
                PathSegment::QuadTo(c, p) => {
                    let c1 = (last.0 + 2.0 / 3.0 * (c.x - last.0), last.1 + 2.0 / 3.0 * (c.y - last.1));
                    let c2 = (p.x + 2.0 / 3.0 * (c.x - p.x), p.y + 2.0 / 3.0 * (c.y - p.y));
                    self.content.cubic_to(c1.0, c1.1, c2.0, c2.1, p.x, p.y);
                    last = (p.x, p.y);
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    self.content.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y);
                    last = (p.x, p.y);
                }
                PathSegment::Close => {
                    self.content.close_path();
                    last = start;
                }
            }
        }
    }

    fn fill_color(&mut self, c: [u8; 4]) {
        self.content.set_fill_rgb(unit(c[0]), unit(c[1]), unit(c[2]));
        if c[3] < 255 {
            self.alpha(c[3]);
        }
    }

    /// The finished document: fonts subset and embedded, the page and its resources.
    pub(super) fn finish(mut self, face: &Face, fonts: &Fonts, title: &str) -> Result<Vec<u8>, String> {
        self.content.restore_state();
        let content = self.content.finish();

        let mut pdf = Pdf::new();
        let mut next = Ref::new(1);
        let mut alloc = || next.bump();
        let catalog = alloc();
        let pages = alloc();
        let page = alloc();
        let contents = alloc();
        let info = alloc();

        let mut font_refs: Vec<(usize, Ref)> = Vec::new();
        let bytes = [fonts.regular, fonts.semibold, fonts.fallback];
        for (slot, used) in self.fonts.iter().enumerate() {
            if used.glyphs.is_empty() {
                continue;
            }
            if face.font(slot).is_none() {
                continue;
            }
            let type0 = alloc();
            let refs = [alloc(), alloc(), alloc(), alloc()];
            write_font(&mut pdf, bytes[slot], used, type0, refs)?;
            font_refs.push((slot, type0));
        }
        let image_refs: Vec<Ref> = self.images.iter().map(|_| alloc()).collect();
        let gs_refs: Vec<Ref> = self.alphas.iter().map(|_| alloc()).collect();

        pdf.catalog(catalog).pages(pages).viewer_preferences().display_doc_title(true);
        pdf.pages(pages).kids([page]).count(1);
        let mut p = pdf.page(page);
        p.parent(pages).media_box(PdfRect::new(0.0, 0.0, self.page.0, self.page.1)).contents(contents);
        let mut res = p.resources();
        let mut f = res.fonts();
        for (slot, r) in &font_refs {
            f.pair(Name(format!("F{slot}").as_bytes()), *r);
        }
        f.finish();
        let mut x = res.x_objects();
        for (i, r) in image_refs.iter().enumerate() {
            x.pair(Name(format!("Im{i}").as_bytes()), *r);
        }
        x.finish();
        let mut g = res.ext_g_states();
        for (i, r) in gs_refs.iter().enumerate() {
            g.pair(Name(format!("A{i}").as_bytes()), *r);
        }
        g.finish();
        res.finish();
        p.finish();

        pdf.stream(contents, &zlib(&content)).filter(Filter::FlateDecode);
        for (img, r) in self.images.iter().zip(&image_refs) {
            let mut x = pdf.image_xobject(*r, &img.rgb_z);
            x.filter(Filter::FlateDecode);
            x.width(i32::try_from(img.width).unwrap_or(i32::MAX));
            x.height(i32::try_from(img.height).unwrap_or(i32::MAX));
            x.color_space().device_rgb();
            x.bits_per_component(8);
            x.interpolate(true);
        }
        for (a, r) in self.alphas.iter().zip(&gs_refs) {
            pdf.ext_graphics(*r).non_stroking_alpha(unit(*a)).stroking_alpha(unit(*a));
        }
        let title: String = title.chars().filter(|c| !c.is_control()).take(300).collect();
        let mut i = pdf.document_info(info);
        if !title.is_empty() {
            i.title(TextStr(&title));
        }
        i.creator(TextStr("LayeredMap")).producer(TextStr(concat!("LayeredMap ", env!("CARGO_PKG_VERSION"))));
        i.finish();
        Ok(pdf.finish())
    }
}

impl Canvas for PdfCanvas {
    fn fill_path(&mut self, path: &tiny_skia::Path, c: [u8; 4]) {
        self.content.save_state();
        self.fill_color(c);
        self.path(path);
        self.content.fill_nonzero();
        self.content.restore_state();
    }

    fn stroke_path(&mut self, path: &tiny_skia::Path, c: [u8; 4], stroke: &Stroke) {
        self.content.save_state();
        self.content.set_stroke_rgb(unit(c[0]), unit(c[1]), unit(c[2]));
        if c[3] < 255 {
            self.alpha(c[3]);
        }
        self.content.set_line_width(stroke.width);
        self.content.set_line_cap(match stroke.line_cap {
            LineCap::Butt => LineCapStyle::ButtCap,
            LineCap::Round => LineCapStyle::RoundCap,
            LineCap::Square => LineCapStyle::ProjectingSquareCap,
        });
        self.content.set_line_join(match stroke.line_join {
            LineJoin::Round => LineJoinStyle::RoundJoin,
            LineJoin::Bevel => LineJoinStyle::BevelJoin,
            _ => LineJoinStyle::MiterJoin,
        });
        self.content.set_miter_limit(stroke.miter_limit);
        self.path(path);
        self.content.stroke();
        self.content.restore_state();
    }

    fn fill_rect(&mut self, r: Rect, c: [u8; 4]) {
        self.content.save_state();
        self.fill_color(c);
        self.content.rect(r.x(), r.y(), r.width(), r.height());
        self.content.fill_nonzero();
        self.content.restore_state();
    }

    fn glyphs(&mut self, face: &Face, glyphs: &[Placed], px: f32, c: [u8; 4]) {
        if px <= 0.0 || !px.is_finite() {
            return;
        }
        self.content.save_state();
        self.fill_color(c);
        self.content.begin_text();
        // Runs of one font: each starts at its first glyph's pen position; within a run the
        // glyphs' own advances (the /W widths) move the pen, adjusted where the layout kerned.
        let mut i = 0;
        while i < glyphs.len() {
            let slot = glyphs[i].slot;
            let Some(font) = face.font(slot) else {
                i += 1;
                continue;
            };
            let upem = font.units_per_em().unwrap_or(1000.0).max(1.0);
            let end = glyphs[i..].iter().position(|g| g.slot != slot).map_or(glyphs.len(), |n| i + n);
            let run = &glyphs[i..end];
            self.content.set_font(Name(format!("F{slot}").as_bytes()), px);
            // The text space is flipped back upright inside the page's y-down transform.
            self.content.set_text_matrix([1.0, 0.0, 0.0, -1.0, run[0].x, run[0].y]);
            let used = &mut self.fonts[slot.min(2)];
            let mut items: Vec<(Option<f32>, u16)> = Vec::with_capacity(run.len());
            let mut pen = run[0].x;
            for g in run {
                let advance = font.h_advance_unscaled(g.id) / upem * 1000.0;
                let cid = used.remapper.remap(g.id.0);
                used.glyphs.entry(cid).or_insert_with(|| (advance, g.ch.to_string()));
                // The TJ adjustment that puts this glyph where the layout did.
                let adjust = (pen - g.x) * 1000.0 / px;
                items.push(((adjust.abs() > 0.01).then_some(adjust), cid));
                pen = g.x + advance * px / 1000.0;
            }
            let mut tj = self.content.show_positioned();
            let mut it = tj.items();
            for (adjust, cid) in items {
                if let Some(a) = adjust {
                    it.adjust(a);
                }
                it.show(Str(&cid.to_be_bytes()));
            }
            it.finish();
            tj.finish();
            i = end;
        }
        self.content.end_text();
        self.content.restore_state();
    }

    fn image(&mut self, pm: &Pixmap, r: Rect) {
        // RGB on white (the sheet's ground), as the PNG draws it.
        let mut rgb = Vec::with_capacity(pm.width() as usize * pm.height() as usize * 3);
        for px in pm.pixels() {
            let c = px.demultiply();
            let a = u16::from(c.alpha());
            let over = |v: u8| ((u16::from(v) * a + 255 * (255 - a) + 127) / 255) as u8;
            rgb.extend_from_slice(&[over(c.red()), over(c.green()), over(c.blue())]);
        }
        let i = self.images.len();
        self.images.push(Image { width: pm.width(), height: pm.height(), rgb_z: zlib(&rgb) });
        self.content.save_state();
        // The image's unit square, top row first, onto `r` in the y-down sheet.
        self.content.transform([r.width(), 0.0, 0.0, -r.height(), r.x(), r.y() + r.height()]);
        self.content.x_object(Name(format!("Im{i}").as_bytes()));
        self.content.restore_state();
    }
}

/// The table `tag` of an OpenType font file.
fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let n = u16::from_be_bytes(font.get(4..6)?.try_into().ok()?) as usize;
    for k in 0..n {
        let rec = font.get(12 + 16 * k..28 + 16 * k)?;
        if &rec[..4] == tag {
            let off = u32::from_be_bytes(rec[8..12].try_into().ok()?) as usize;
            let len = u32::from_be_bytes(rec[12..16].try_into().ok()?) as usize;
            return font.get(off..off.checked_add(len)?);
        }
    }
    None
}

/// Six capital letters naming a subset (PDF 9.6.4), from the glyphs it holds.
fn subset_tag(used: &FontUse) -> String {
    let mut h: u32 = 0x811C_9DC5;
    for g in used.remapper.remapped_gids() {
        for b in g.to_be_bytes() {
            h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
    }
    (0..6)
        .map(|k| {
            let v = (h >> (k * 5)) % 26;
            char::from(b'A' + v as u8)
        })
        .collect()
}

/// Writes one font: the Type 0 font `type0`, its CID font, descriptor, embedded subset and
/// ToUnicode map (`refs`).
fn write_font(pdf: &mut Pdf, data: &[u8], used: &FontUse, type0: Ref, refs: [Ref; 4]) -> Result<(), String> {
    let [cid_ref, descriptor_ref, file_ref, cmap_ref] = refs;
    let face = ttf_parser::Face::parse(data, 0).map_err(|e| format!("The font can't be read: {e}"))?;
    let upem = f32::from(face.units_per_em()).max(1.0);
    let k = 1000.0 / upem;
    let ps_name = face
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
        .and_then(|n| n.to_string().or_else(|| std::str::from_utf8(n.name).ok().map(str::to_string)))
        .unwrap_or_else(|| "LayeredMapFont".into());
    let ps_name: String = ps_name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let base = format!("{}+{ps_name}", subset_tag(used));
    let cff = table(data, b"CFF ").is_some();

    let subset = subsetter::subset(data, 0, &used.remapper).map_err(|e| format!("The font couldn't be embedded: {e}"))?;
    let file = if cff { table(&subset, b"CFF ").ok_or("The font couldn't be embedded.")?.to_vec() } else { subset };

    pdf.type0_font(type0)
        .base_font(Name(base.as_bytes()))
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_ref)
        .to_unicode(cmap_ref);

    let system = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 };
    let mut cid = pdf.cid_font(cid_ref);
    cid.subtype(if cff { CidFontType::Type0 } else { CidFontType::Type2 });
    cid.base_font(Name(base.as_bytes()));
    cid.system_info(system);
    cid.font_descriptor(descriptor_ref);
    cid.default_width(0.0);
    if !cff {
        cid.cid_to_gid_map_predefined(Name(b"Identity"));
    }
    let mut w = cid.widths();
    for (id, (advance, _)) in &used.glyphs {
        w.consecutive(*id, [*advance]);
    }
    w.finish();
    cid.finish();

    let bbox = face.global_bounding_box();
    let ascent = f32::from(face.ascender()) * k;
    let descent = f32::from(face.descender()) * k;
    let cap = face.capital_height().map_or(ascent * 0.7, |c| f32::from(c) * k);
    let mut d = pdf.font_descriptor(descriptor_ref);
    d.name(Name(base.as_bytes()))
        .flags(FontFlags::SYMBOLIC)
        .bbox(PdfRect::new(f32::from(bbox.x_min) * k, f32::from(bbox.y_min) * k, f32::from(bbox.x_max) * k, f32::from(bbox.y_max) * k))
        .italic_angle(face.italic_angle())
        .ascent(ascent)
        .descent(descent)
        .cap_height(cap)
        .stem_v(if face.is_bold() || face.weight().to_number() >= 600 { 120.0 } else { 80.0 });
    if cff {
        d.font_file3(file_ref);
    } else {
        d.font_file2(file_ref);
    }
    d.finish();

    let z = zlib(&file);
    let mut s = pdf.stream(file_ref, &z);
    s.filter(Filter::FlateDecode);
    if cff {
        s.pair(Name(b"Subtype"), Name(b"CIDFontType0C"));
    } else {
        s.pair(Name(b"Length1"), i32::try_from(file.len()).unwrap_or(i32::MAX));
    }
    s.finish();

    let mut cmap = UnicodeCmap::new(Name(b"Custom"), system);
    for (id, (_, text)) in &used.glyphs {
        if *id != 0 && !text.is_empty() {
            cmap.pair_with_multiple(*id, text.chars());
        }
    }
    pdf.cmap(cmap_ref, &zlib(&cmap.finish())).filter(Filter::FlateDecode);
    Ok(())
}
