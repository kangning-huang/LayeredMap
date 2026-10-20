//! Drawing raster layers.
//!
//! Each screen pixel is traced back to the raster's grid: Web Mercator → the raster's own
//! coordinate system → pixel position. The full transformation runs on a coarse grid of screen
//! points and is interpolated in between, which is exact for Web Mercator and lon/lat rasters
//! and well under a pixel off for projected ones. Values are then sampled (nearest or bilinear),
//! optionally re-stretched from the pixels in view (DRA), and coloured.

use rustgis_data::raster::{BandStats, ColorHint, Crs, Raster};
use rustgis_geom::{Coord, Extent, extent_to_merc};
use rustgis_map::Globe;
use rustgis_map::projection::Flat;
use rustgis_map::raster::{Colorizer, RasterRenderer, RasterStyle, Resampling, hillshade};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

use crate::View;

/// Screen pixels between exactly transformed points.
const STEP: usize = 8;

/// The raster pixel position under a work pixel (`k` device pixels), or `None` off the
/// projection.
/// Where a raster is drawn: on the globe, or on the flat map in a projection (with its code).
#[derive(Clone, Copy)]
pub(crate) enum Place<'a> {
    Globe(&'a Globe),
    Flat(&'a Flat, &'a str),
}

fn source(r: &Raster, view: &View, place: Place, k: f64, dx: f64, dy: f64) -> Option<(f64, f64)> {
    let ppp = view.pixels_per_point / k;
    let (sx, sy) = (dx / ppp, dy / ppp);
    let n = match place {
        Place::Globe(g) => r.transformer().from_lonlat(g.inverse(sx, sy)?)?,
        Place::Flat(Flat::Mercator, _) => r.transformer().from_merc(view.camera.to_map(sx, sy, view.width, view.height))?,
        Place::Flat(flat, key) => {
            let m = view.camera.to_map(sx, sy, view.width, view.height);
            // A raster already in the map's coordinate system needs no reprojection.
            if key.strip_prefix("EPSG:").and_then(|c| c.parse::<u16>().ok()) == Some(r.crs.epsg()) {
                m
            } else {
                r.transformer().from_lonlat(flat.inverse(m)?)?
            }
        }
    };
    let (c, row) = r.transform.to_pixel(n);
    (c.is_finite() && row.is_finite()).then_some((c, row))
}

fn sample(r: &Raster, band: usize, c: f64, row: f64, resampling: Resampling) -> f32 {
    let nearest = || r.value(band, c.floor() as i64, row.floor() as i64).unwrap_or(f32::NAN);
    if resampling == Resampling::Nearest {
        return nearest();
    }
    // Bilinear between the four pixel centres around the point.
    let (x, y) = (c - 0.5, row - 0.5);
    let (i, j) = (x.floor(), y.floor());
    let (fx, fy) = ((x - i) as f32, (y - j) as f32);
    let (i, j) = (i as i64, j as i64);
    match (r.value(band, i, j), r.value(band, i + 1, j), r.value(band, i, j + 1), r.value(band, i + 1, j + 1)) {
        (Some(a), Some(b), Some(cc), Some(d)) => {
            let top = a + (b - a) * fx;
            let bottom = cc + (d - cc) * fx;
            top + (bottom - top) * fy
        }
        _ => nearest(),
    }
}

fn place_is_flat(p: &Place) -> bool {
    matches!(p, Place::Flat(..))
}

/// The longitude/latitude box a raster covers on the Web Mercator map. A grid in plain
/// longitude/latitude keeps its own longitudes, which may run past ±180° (a grid laid out over
/// 0 to 360°, or one whose first column straddles the antimeridian), where its extent stops.
fn drawn_lonlat(r: &Raster) -> Extent {
    let e = r.extent();
    if r.crs != Crs::LonLat || e.is_empty() {
        return e;
    }
    let (a, b) = (r.transform.origin_x, r.transform.origin_x + r.transform.pixel_width * r.width as f64);
    if !(a.is_finite() && b.is_finite()) {
        return e;
    }
    // Within a world of either side, as the world copies assume.
    Extent::new(a.min(b).max(-540.0), e.min_y, a.max(b).min(540.0), e.max_y)
}

/// One raster's pixels in one screen box (the view's own, or a world copy's): the box in work
/// pixels and, for each of its pixels, the band values, hillshade and alpha.
struct Patch {
    x0: usize,
    y0: usize,
    bw: usize,
    bh: usize,
    vals: Vec<f32>,
    shade: Vec<u8>,
    alpha: Vec<u8>,
}

/// Draws a raster layer onto `pm` at `opacity`, in `view` and again in each of `copies` (the
/// views of the neighbouring world copies, empty unless the flat map repeats the world). Each
/// copy samples only its own part of the screen; the colours (and a view-based stretch) are
/// worked out once over all of them, so the copies match.
pub(crate) fn draw(
    pm: &mut Pixmap,
    r: &Raster,
    style: &RasterStyle,
    view: &View,
    copies: &[View],
    opacity: f32,
    full_res: bool,
    place: Place,
) {
    // On high-density screens, compute at logical-pixel resolution and scale up (unless asked
    // for full resolution, as exports are): a quarter of the work at 2×.
    let k = if view.pixels_per_point >= 1.5 && !full_res { view.pixels_per_point } else { 1.0 };
    let (w, h) = ((pm.width() as f64 / k).ceil(), (pm.height() as f64 / k).ceil());
    let nb = style.renderer.bands().len().max(1);
    let patches: Vec<Patch> = std::iter::once(view)
        .chain(copies.iter().filter(|_| place_is_flat(&place)))
        .filter_map(|v| patch(r, style, v, k, (w, h), place))
        .collect();
    if patches.is_empty() {
        return;
    }

    // Dynamic range adjustment: statistics of the values in view (in every copy).
    let view_stats: Option<Vec<BandStats>> = style.dra.then(|| {
        (0..nb)
            .map(|b| BandStats::from_values(&patches.iter().flat_map(|p| p.vals.iter().skip(b).step_by(nb).copied()).collect::<Vec<_>>()))
            .collect()
    });
    let colorizer = Colorizer::new(style, r, view_stats.as_deref());
    let quality = if style.resampling == Resampling::Bilinear { FilterQuality::Bilinear } else { FilterQuality::Nearest };

    // Pass 2: colours, premultiplied, into a pixmap the size of each screen box.
    for p in &patches {
        let Some(mut lay) = Pixmap::new(p.bw as u32, p.bh as u32) else { continue };
        let data = lay.data_mut();
        for i in 0..p.bw * p.bh {
            let Some(v) = p.vals.get(i * nb..(i + 1) * nb) else { continue };
            let Some(mut c) = colorizer.color(v, p.shade.get(i).copied()) else { continue };
            if let Some(a) = p.alpha.get(i) {
                c[3] = ((c[3] as u16 * *a as u16 + 127) / 255) as u8;
            }
            let a = c[3] as u16;
            if let Some(px) = data.get_mut(i * 4..i * 4 + 4) {
                let pm = |x: u8| ((x as u16 * a + 127) / 255) as u8;
                px.copy_from_slice(&[pm(c[0]), pm(c[1]), pm(c[2]), c[3]]);
            }
        }
        let at = Transform::from_row(k as f32, 0.0, 0.0, k as f32, (p.x0 as f64 * k) as f32, (p.y0 as f64 * k) as f32);
        pm.draw_pixmap(0, 0, lay.as_ref(), &PixmapPaint { opacity, quality, ..PixmapPaint::default() }, at, None);
    }
}

/// Pass 1 for one view: the raster's screen box, clipped to the `w` × `h` work pixels, and the
/// band values (and hillshade) for each of its pixels. `None` when the raster is out of view.
fn patch(r: &Raster, style: &RasterStyle, view: &View, k: f64, (w, h): (f64, f64), place: Place) -> Option<Patch> {
    let ppp = view.pixels_per_point / k;
    // The raster's bounds on screen, clipped to the view.
    let ((ax, ay), (bx, by)) = match place {
        // On the globe: the box around the visible part of a grid of points over the extent.
        Place::Globe(g) => {
            let e = r.extent();
            let mut b = Extent::EMPTY;
            for i in 0..=24 {
                for j in 0..=24 {
                    let c = Coord::new(e.min_x + e.width() * i as f64 / 24.0, e.min_y + e.height() * j as f64 / 24.0);
                    if let Some((x, y)) = g.forward(c) {
                        b.expand(Coord::new(x, y));
                    }
                }
            }
            // A raster under the centre of view may reach past every sample point: take the disc.
            if e.contains(view.camera.center_lonlat()) {
                let (cx, cy) = g.center();
                let r = g.disc_radius();
                ((cx - r, cy - r), (cx + r, cy + r))
            } else if b.is_empty() {
                return None;
            } else {
                // A margin for the parts between the sample points.
                let m = (b.width().max(b.height()) / 24.0).max(2.0);
                ((b.min_x - m, b.min_y - m), (b.max_x + m, b.max_y + m))
            }
        }
        Place::Flat(Flat::Mercator, _) => {
            let e = extent_to_merc(&drawn_lonlat(r));
            (view.to_screen(Coord::new(e.min_x, e.max_y)), view.to_screen(Coord::new(e.max_x, e.min_y)))
        }
        Place::Flat(flat, _) => {
            let e = flat.extent(&r.extent());
            if e.is_empty() {
                return None;
            }
            (view.to_screen(Coord::new(e.min_x, e.max_y)), view.to_screen(Coord::new(e.max_x, e.min_y)))
        }
    };
    let clip = |v: f64, max: f64| if v.is_finite() { v.clamp(0.0, max) } else { 0.0 };
    // A rotated raster turns about its centre: its pixels stay within half its diagonal of it.
    let rot = style.rotation.rem_euclid(360.0);
    let rotated = rot.abs() > 1e-6 && place_is_flat(&place);
    let centre = (((ax + bx) / 2.0) * ppp, ((ay + by) / 2.0) * ppp);
    let (sin, cos) = (-rot.to_radians()).sin_cos();
    let (x0, x1, y0, y1) = if rotated && !(centre.0.is_finite() && centre.1.is_finite() && (bx - ax).hypot(by - ay).is_finite()) {
        (0, w as usize, 0, h as usize)
    } else if rotated {
        let reach = ((bx - ax).hypot(by - ay) / 2.0 * ppp).ceil() + 1.0;
        (
            clip((centre.0 - reach).floor(), w) as usize,
            clip((centre.0 + reach).ceil(), w) as usize,
            clip((centre.1 - reach).floor(), h) as usize,
            clip((centre.1 + reach).ceil(), h) as usize,
        )
    } else {
        (
            clip((ax.min(bx) * ppp).floor(), w) as usize,
            clip((ax.max(bx) * ppp).ceil(), w) as usize,
            clip((ay.min(by) * ppp).floor(), h) as usize,
            clip((ay.max(by) * ppp).ceil(), h) as usize,
        )
    };
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (bw, bh) = (x1 - x0, y1 - y0);

    // Exact source positions on a coarse grid.
    let (gw, gh) = (bw / STEP + 2, bh / STEP + 2);
    let mut grid = Vec::with_capacity(gw * gh);
    for gy in 0..gh {
        for gx in 0..gw {
            grid.push(source(r, view, place, k, (x0 + gx * STEP) as f64, (y0 + gy * STEP) as f64));
        }
    }
    let at = |px: usize, py: usize| -> Option<(f64, f64)> {
        if rotated {
            // Back-rotate the screen pixel about the centre, then find its source exactly.
            let (dx, dy) = (px as f64 + 0.5 - centre.0, py as f64 + 0.5 - centre.1);
            return source(r, view, place, k, centre.0 + dx * cos - dy * sin, centre.1 + dx * sin + dy * cos);
        }
        let u = (px - x0) as f64 + 0.5;
        let v = (py - y0) as f64 + 0.5;
        let (gx, gy) = ((u / STEP as f64) as usize, (v / STEP as f64) as usize);
        let (fx, fy) = (u / STEP as f64 - gx as f64, v / STEP as f64 - gy as f64);
        let g = |x: usize, y: usize| grid.get(y * gw + x).copied().flatten();
        match (g(gx, gy), g(gx + 1, gy), g(gx, gy + 1), g(gx + 1, gy + 1)) {
            (Some(a), Some(b), Some(c), Some(d)) => {
                let lerp = |p: f64, q: f64, t: f64| p + (q - p) * t;
                let top = (lerp(a.0, b.0, fx), lerp(a.1, b.1, fx));
                let bottom = (lerp(c.0, d.0, fx), lerp(c.1, d.1, fx));
                Some((lerp(top.0, bottom.0, fy), lerp(top.1, bottom.1, fy)))
            }
            // Near the edge of the projection's valid area: transform this pixel exactly.
            _ => source(r, view, place, k, px as f64 + 0.5, py as f64 + 0.5),
        }
    };

    // Pass 1: band values (and hillshade) for each screen pixel.
    let bands = style.renderer.bands();
    let nb = bands.len().max(1);
    let relief = match style.renderer {
        RasterRenderer::ShadedRelief { azimuth, altitude, z_factor, .. } => Some((azimuth, altitude, z_factor)),
        _ => None,
    };
    let (cell_x, cell_y) = r.cell_size_m();
    // Blending neighbours would invent values that aren't classes or palette entries.
    let resampling = match style.renderer {
        RasterRenderer::UniqueValues { .. } | RasterRenderer::Colormap => Resampling::Nearest,
        _ => style.resampling,
    };
    let mut vals = vec![f32::NAN; bw * bh * nb];
    let mut shade = if relief.is_some() { vec![0u8; bw * bh] } else { Vec::new() };
    // RGBA images carry their transparency in band 4.
    let alpha_band = match (&style.renderer, &r.color) {
        (RasterRenderer::Rgb { .. }, ColorHint::Rgb { alpha: true }) if r.band_count() >= 4 => Some(3),
        (RasterRenderer::Stretch { band: 0, .. } | RasterRenderer::ShadedRelief { band: 0, .. }, ColorHint::GrayAlpha) => Some(1),
        _ => None,
    };
    let mut alpha = if alpha_band.is_some() { vec![0u8; bw * bh] } else { Vec::new() };
    for py in y0..y1 {
        for px in x0..x1 {
            let Some((c, row)) = at(px, py) else { continue };
            if c < 0.0 || row < 0.0 || c >= r.width as f64 || row >= r.height as f64 {
                continue;
            }
            let i = (py - y0) * bw + (px - x0);
            if let (Some(ab), Some(a)) = (alpha_band, alpha.get_mut(i)) {
                *a = r.value(ab, c.floor() as i64, row.floor() as i64).map(|v| v.clamp(0.0, 255.0) as u8).unwrap_or(0);
                if *a == 0 {
                    continue;
                }
            }
            for (bi, b) in bands.iter().enumerate() {
                if let Some(slot) = vals.get_mut(i * nb + bi) {
                    *slot = sample(r, *b, c, row, resampling);
                }
            }
            if let (Some((az, alt, zf)), Some(b)) = (relief, bands.first()) {
                let (ci, ri) = (c.floor() as i64, row.floor() as i64);
                let centre = r.value(*b, ci, ri).unwrap_or(0.0) as f64;
                let mut z = [centre; 9];
                for (k, (dx, dy)) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)].into_iter().enumerate()
                {
                    z[k] = r.value(*b, ci + dx, ri + dy).map(f64::from).unwrap_or(centre);
                }
                if let Some(s) = shade.get_mut(i) {
                    *s = hillshade(&z, cell_x, cell_y, az, alt, zf);
                }
            }
        }
    }
    Some(Patch { x0, y0, bw, bh, vals, shade, alpha })
}

#[cfg(test)]
mod tests {
    use rustgis_data::raster::GeoTransform;
    use rustgis_geom::lonlat_to_merc;
    use rustgis_map::Camera;
    use rustgis_map::raster::RasterStyle;

    use super::*;

    #[test]
    fn draws_a_utm_raster_where_it_belongs() {
        // 100 × 100 cells of 1 km in UTM 17N around 81°W, 42°N, valued by column.
        let gt = GeoTransform { origin_x: 450_000.0, origin_y: 4_700_000.0, pixel_width: 1000.0, pixel_height: -1000.0 };
        let data: Vec<f32> = (0..10_000).map(|i| (i % 100) as f32).collect();
        let r = Raster::new("utm", 100, 100, vec![data], gt, Crs::from_epsg(32617).unwrap(), None, ColorHint::Gray, "Float32").unwrap();
        let style = RasterStyle::default_for(&r);
        let camera = Camera { center: lonlat_to_merc(Coord::new(-81.0, 42.0)), res: 2000.0 };
        let view = View { width: 200.0, height: 200.0, pixels_per_point: 1.0, camera, dark: false };
        let mut pm = Pixmap::new(200, 200).unwrap();
        draw(&mut pm, &r, &style, &view, &[], 1.0, false, Place::Flat(&Flat::Mercator, "mercator"));
        let alpha = |x: u32, y: u32| pm.pixel(x, y).map(|p| p.alpha()).unwrap_or(0);
        assert_eq!(alpha(100, 100), 255, "the centre is covered");
        assert_eq!(alpha(2, 2), 0, "the corner of the view is outside the raster");
        // West is low (dark), east high (light).
        let lum = |x: u32| pm.pixel(x, 100).map(|p| p.red()).unwrap_or(0);
        assert!(lum(70) < lum(130), "{} vs {}", lum(70), lum(130));
        // Every renderer draws without panicking, at odd view sizes too.
        for kind in ["rgb", "classify", "unique", "shadedRelief"] {
            let mut s = style.clone();
            s.renderer = match kind {
                "rgb" => RasterRenderer::Rgb { red: 0, green: 0, blue: 0 },
                "classify" => rustgis_map::raster::classify(&r, 0, 5, Default::default(), Default::default()),
                "unique" => rustgis_map::raster::unique_values(&r, 0, Default::default()),
                _ => RasterRenderer::ShadedRelief { band: 0, ramp: None, azimuth: 315.0, altitude: 45.0, z_factor: 1.0 },
            };
            s.dra = true;
            s.resampling = Resampling::Bilinear;
            draw(&mut pm, &r, &s, &view, &[], 0.5, false, Place::Flat(&Flat::Mercator, "mercator"));
        }
        // High-density screens draw at logical resolution, scaled up.
        let hi = View { pixels_per_point: 2.0, ..view };
        let mut pm2 = Pixmap::new(400, 400).unwrap();
        draw(&mut pm2, &r, &style, &hi, &[], 1.0, false, Place::Flat(&Flat::Mercator, "mercator"));
        assert_eq!(pm2.pixel(200, 200).map(|p| p.alpha()), Some(255));
        assert_eq!(pm2.pixel(4, 4).map(|p| p.alpha()), Some(0));
        let far = View { width: 3.0, height: 1.0, pixels_per_point: 2.0, camera: Camera::world(), dark: false };
        let mut tiny = Pixmap::new(6, 2).unwrap();
        draw(&mut tiny, &r, &style, &far, &[], 1.0, true, Place::Flat(&Flat::Mercator, "mercator"));
    }
    /// A grid whose first column straddles the antimeridian (a 0–360° NetCDF shown as −180 to
    /// 180°) fills that cell on both sides of 180° when the world repeats, though its extent
    /// stops at −180°; each side is drawn by the world it lies in.
    #[test]
    fn a_grid_across_the_antimeridian_fills_its_cell_on_both_sides() {
        // 16 × 8 cells of 22.5° from 191.25°W, valued by column: column 0 is centred on 180°.
        let gt = GeoTransform { origin_x: -191.25, origin_y: 90.0, pixel_width: 22.5, pixel_height: -22.5 };
        let data: Vec<f32> = (0..16 * 8).map(|i| (i % 16) as f32).collect();
        let r = Raster::new("lon360", 16, 8, vec![data], gt, Crs::LonLat, None, ColorHint::Gray, "Float32").unwrap();
        assert_eq!((r.extent().min_x, r.extent().max_x), (-180.0, 168.75));
        let style = RasterStyle::default_for(&r);
        // Centred on 180°, 100 km per pixel: about 1° per pixel either side of x = 200.
        let camera = Camera { center: lonlat_to_merc(Coord::new(180.0, 0.0)), res: 100_000.0 };
        let view = View { width: 400.0, height: 200.0, pixels_per_point: 1.0, camera, dark: false };
        let world = 2.0 * std::f64::consts::PI * rustgis_geom::MERCATOR_R;
        let east = View { camera: Camera { center: Coord::new(camera.center.x - world, camera.center.y), ..camera }, ..view };
        let flat = Place::Flat(&Flat::Mercator, "mercator");
        let (x_w, x_e) = (200 - 5, 200 + 5); // 175°E and 175°W
        let alpha = |pm: &Pixmap, x: u32| pm.pixel(x, 100).map(|p| p.alpha()).unwrap_or(0);

        // Alone, the view's own world draws nothing past 168.75°E.
        let mut once = Pixmap::new(400, 200).unwrap();
        draw(&mut once, &r, &style, &view, &[], 1.0, false, flat);
        assert_eq!((alpha(&once, x_w), alpha(&once, x_e)), (0, 0));
        // With the copy a world east, the straddling column covers 175°E and 175°W alike.
        let mut both = Pixmap::new(400, 200).unwrap();
        draw(&mut both, &r, &style, &view, &[east], 1.0, false, flat);
        assert_eq!((alpha(&both, x_w), alpha(&both, x_e)), (255, 255));
        assert_eq!(both.pixel(x_w, 100), both.pixel(x_e, 100), "the same cell");
        // Column 15 (146.25–168.75°E) still lies west of it.
        assert_eq!(alpha(&both, 200 - 20), 255);
        assert_ne!(both.pixel(200 - 20, 100), both.pixel(x_w, 100));
    }

    /// A rotated raster turns about its centre and is drawn wherever it turns to, past its
    /// unrotated box, and nowhere beyond half its diagonal.
    #[test]
    fn a_rotated_raster_draws_where_it_turns_to() {
        // 40 × 4 cells of 1 km in Web Mercator: 40 km wide, 4 km tall.
        let gt = GeoTransform { origin_x: -20_000.0, origin_y: 2_000.0, pixel_width: 1000.0, pixel_height: -1000.0 };
        let r = Raster::new("bar", 40, 4, vec![vec![1.0; 160]], gt, Crs::WebMercator, None, ColorHint::Gray, "Float32").unwrap();
        let mut style = RasterStyle::default_for(&r);
        let camera = Camera { center: Coord::new(0.0, 0.0), res: 500.0 };
        let view = View { width: 200.0, height: 200.0, pixels_per_point: 1.0, camera, dark: false };
        let alpha = |pm: &Pixmap, x: u32, y: u32| pm.pixel(x, y).map(|p| p.alpha()).unwrap_or(0);
        let mut flat = Pixmap::new(200, 200).unwrap();
        draw(&mut flat, &r, &style, &view, &[], 1.0, false, Place::Flat(&Flat::Mercator, "mercator"));
        // Unrotated: 80 px wide, 8 px tall about the centre.
        assert_eq!((alpha(&flat, 100, 100), alpha(&flat, 150, 100), alpha(&flat, 100, 130)), (255, 0, 0));
        // A quarter turn: 8 px wide, 80 px tall.
        style.rotation = 90.0;
        let mut turned = Pixmap::new(200, 200).unwrap();
        draw(&mut turned, &r, &style, &view, &[], 1.0, false, Place::Flat(&Flat::Mercator, "mercator"));
        assert_eq!((alpha(&turned, 100, 100), alpha(&turned, 100, 135), alpha(&turned, 100, 65)), (255, 255, 255));
        assert_eq!((alpha(&turned, 130, 100), alpha(&turned, 100, 150)), (0, 0));
    }
}
