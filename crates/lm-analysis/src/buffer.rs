//! "Show an area within a distance" and "Measure distance or area" (spec › WP4.5 Certified
//! actions): a geodesic buffer at a distance in metres or kilometres, labelled
//! "Straight-line distance", and geodesic measurement on WGS 84.
//!
//! **Points** become geodesic circles: [`CIRCLE_VERTICES`] vertices from GeographicLib's direct
//! problem at equal azimuth steps. An inscribed n-gon falls short of the circle's area by
//! `1 − sin(2π/n)/(2π/n) ≈ (2π/n)²/6`; with n = 256 (1.4° steps) that is 1.0e-4, ten times
//! inside the spec's 0.1% for points. (Fixture N5's reference uses 360 vertices, 5e-5 short.)
//!
//! **Lines and areas** are buffered one feature at a time in a local azimuthal equidistant (AEQD)
//! projection centred on the feature (mean latitude, circular-mean longitude), as N5's reference
//! does: the input is densified along geodesics (steps ≤ [`DENSIFY_M`]), projected with exact
//! ellipsoidal AEQD (forward = GeographicLib inverse from the centre, giving distance and azimuth;
//! back = GeographicLib direct), buffered in the plane with round joins and caps at the same 1.4°
//! angular step as the circles, densified again and projected back. AEQD keeps distances from
//! the centre exact, so the planar buffer is accurate for features up to a few hundred km across
//! (N5's 1° cases at 50 km are within 7e-5 of the reference); for continent-sized features it
//! degrades with distance from the centre.
//!
//! **Output** is RFC 7946 lon/lat: exteriors counter-clockwise, holes clockwise, rings closed,
//! longitudes in [−180, 180]. A buffer that crosses the antimeridian is split there into parts
//! that meet along ±180° (vertices exactly on ±180°, so [`crate::geodesic`] drops the seam from
//! perimeters and the parts' areas add up). A buffer ring that winds around a pole is closed along
//! the pole (lat ±90°) before splitting, so its area is the cap, not the rest of the globe.
//!
//! **Layer-wide** buffers merge overlaps by default ([`BufferOptions::merge`]): the per-feature
//! buffers are unioned in lon/lat (vertices are geodesically exact and edges at most a few hundred
//! metres long, so straight lon/lat edges stand in for geodesics). The union runs as a tree of
//! small unions over spatially sorted batches, checking for a cancel between every one.
//!
//! **Measure** wraps [`crate::geodesic`]: the geodesic length along a path and the geodesic
//! area (holes subtracted) and perimeter of polygons.

use crate::control::{Cancelled, Control};
use crate::geodesic::{self, AreaPerimeter};
use geo::algorithm::buffer::{Buffer, BufferStyle, LineCap, LineJoin};
use geo::{BooleanOps, Coord as GCoord, LineString, MultiLineString, MultiPolygon, Orient, Polygon, Rect, orient::Direction};
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use rustgis_data::Geometry;
use rustgis_geom::Coord;
use std::sync::OnceLock;

/// What the action calls its distance (spec: "Labelled 'Straight-line distance'").
pub const DISTANCE_LABEL: &str = "Straight-line distance";

/// Vertices of a point's geodesic circle (1.4° azimuth steps; see the module notes).
pub const CIRCLE_VERTICES: usize = 256;

/// Longest geodesic step when densifying a feature before and after the AEQD buffer (metres), as
/// in N5's reference. Very long features use longer steps so no feature exceeds
/// [`MAX_DENSIFY_VERTICES`].
pub const DENSIFY_M: f64 = 100.0;

/// Cap on the vertices densification may make per feature.
pub const MAX_DENSIFY_VERTICES: f64 = 50_000.0;

/// Largest buffer distance accepted (metres): a quarter of the Earth's circumference, beyond
/// which "within a distance" stops being a local area.
pub const MAX_DISTANCE_M: f64 = 10_000_000.0;

/// Longest lon/lat edge in the output (degrees): longer edges, which the overlay makes by
/// dropping collinear vertices, are split so geodesic edges follow them.
const LONLAT_STEP_DEG: f64 = 0.1;

/// Polygons per union in the merge tree: small enough that each union finishes well inside the
/// 50 ms cancel budget.
const UNION_BATCH: usize = 32;

/// The angular step of round joins and caps (radians), matching [`CIRCLE_VERTICES`].
fn join_angle() -> f64 {
    std::f64::consts::TAU / CIRCLE_VERTICES as f64
}

fn wgs84() -> &'static Geodesic {
    static G: OnceLock<Geodesic> = OnceLock::new();
    G.get_or_init(Geodesic::wgs84)
}

/// The units a distance was typed in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DistanceUnit {
    #[default]
    Meters,
    Kilometers,
}

impl DistanceUnit {
    /// `value` in these units, in metres.
    pub fn to_meters(self, value: f64) -> f64 {
        match self {
            DistanceUnit::Meters => value,
            DistanceUnit::Kilometers => value * 1000.0,
        }
    }
}

/// How a layer is buffered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferOptions {
    /// Straight-line (geodesic) distance in metres; must be > 0 and ≤ [`MAX_DISTANCE_M`].
    pub distance_m: f64,
    /// Merge overlapping buffers into one shape (the default); false keeps one per feature.
    pub merge: bool,
}

impl BufferOptions {
    /// Merged buffers at `value` `unit`.
    pub fn new(value: f64, unit: DistanceUnit) -> BufferOptions {
        BufferOptions { distance_m: unit.to_meters(value), merge: true }
    }
}

/// Why a buffer was not made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BufferError {
    Cancelled,
    /// The distance is not a positive number up to [`MAX_DISTANCE_M`].
    InvalidDistance(f64),
}

impl From<Cancelled> for BufferError {
    fn from(_: Cancelled) -> Self {
        BufferError::Cancelled
    }
}

impl std::fmt::Display for BufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BufferError::Cancelled => write!(f, "Cancelled"),
            BufferError::InvalidDistance(_) => write!(f, "Enter a distance greater than zero and at most 10,000 km"),
        }
    }
}

impl std::error::Error for BufferError {}

/// The result of a layer-wide buffer.
#[derive(Clone, Debug, PartialEq)]
pub enum BufferOutput {
    /// One (multi-)polygon covering every feature's buffer; `None` when nothing had a shape.
    Merged(Option<Geometry>),
    /// One buffer per input feature, in input order; `None` where a feature had no shape.
    PerFeature(Vec<Option<Geometry>>),
}

fn check_distance(d: f64) -> Result<(), BufferError> {
    if d.is_finite() && d > 0.0 && d <= MAX_DISTANCE_M { Ok(()) } else { Err(BufferError::InvalidDistance(d)) }
}

/// The geodesic buffer of one geometry at `distance_m`, as a polygon geometry; `None` when the
/// geometry has no finite vertices.
pub fn buffer_geometry(g: &Geometry, distance_m: f64, ctl: &Control) -> Result<Option<Geometry>, BufferError> {
    check_distance(distance_m)?;
    let mp = buffer_lonlat(g, distance_m, ctl)?;
    Ok(to_geometry(&mp))
}

/// Buffers every feature of a layer (`None` = a feature without geometry). Reports progress as
/// "Buffering features" then, when merging, "Merging overlaps".
pub fn buffer_layer(geoms: &[Option<Geometry>], opts: &BufferOptions, ctl: &Control) -> Result<BufferOutput, BufferError> {
    check_distance(opts.distance_m)?;
    let total = geoms.len() as u64;
    let mut per: Vec<MultiPolygon<f64>> = Vec::with_capacity(geoms.len());
    for (i, g) in geoms.iter().enumerate() {
        if i % 64 == 0 {
            ctl.check()?;
            ctl.report("Buffering features", i as u64, Some(total));
        }
        per.push(match g {
            Some(g) => buffer_lonlat(g, opts.distance_m, ctl)?,
            None => MultiPolygon::new(vec![]),
        });
    }
    ctl.report("Buffering features", total, Some(total));
    if !opts.merge {
        return Ok(BufferOutput::PerFeature(per.iter().map(to_geometry).collect()));
    }
    let polys: Vec<Polygon<f64>> = per.into_iter().flat_map(|mp| mp.0).collect();
    let merged = union_all(polys, ctl)?;
    Ok(BufferOutput::Merged(to_geometry(&merged)))
}

/// Unions polygons (lon/lat, standard orientation) as a tree of batch unions over a spatial
/// sort, checking `ctl` between unions.
fn union_all(mut polys: Vec<Polygon<f64>>, ctl: &Control) -> Result<MultiPolygon<f64>, Cancelled> {
    // Morton order of the first vertex keeps each batch spatially compact, so unions shrink.
    let key = |p: &Polygon<f64>| -> u64 {
        let c = p.exterior().0.first().copied().unwrap_or(GCoord { x: 0.0, y: 0.0 });
        let qx = (((c.x + 180.0) / 360.0).clamp(0.0, 1.0) * 65535.0) as u32;
        let qy = (((c.y + 90.0) / 180.0).clamp(0.0, 1.0) * 65535.0) as u32;
        let mut k = 0u64;
        for b in 0..16 {
            k |= u64::from((qx >> b) & 1) << (2 * b);
            k |= u64::from((qy >> b) & 1) << (2 * b + 1);
        }
        k
    };
    polys.sort_by_cached_key(key);
    let mut level: Vec<MultiPolygon<f64>> = polys.into_iter().map(|p| MultiPolygon::new(vec![p])).collect();
    let mut round = 0u64;
    loop {
        if level.len() <= 1 {
            return Ok(level.pop().unwrap_or_else(|| MultiPolygon::new(vec![])));
        }
        let n = level.len().div_ceil(UNION_BATCH) as u64;
        let mut next = Vec::with_capacity(n as usize);
        for (i, batch) in level.chunks(UNION_BATCH).enumerate() {
            ctl.check()?;
            ctl.report("Merging overlaps", i as u64, Some(n));
            next.push(if batch.len() == 1 { batch[0].clone() } else { geo::unary_union(batch.iter()) });
        }
        level = next;
        round += 1;
        debug_assert!(round < 64);
    }
}

/// A geometry's buffer in lon/lat, split at the antimeridian.
fn buffer_lonlat(g: &Geometry, d: f64, ctl: &Control) -> Result<MultiPolygon<f64>, Cancelled> {
    match g {
        Geometry::Point(c) => Ok(point_buffer(*c, d)),
        Geometry::MultiPoint(cs) => {
            let polys: Vec<Polygon<f64>> = cs.iter().flat_map(|c| point_buffer(*c, d).0).collect();
            union_all(polys, ctl)
        }
        Geometry::Line(parts) => aeqd_buffer(parts.iter().map(|p| vec![p.clone()]).collect(), false, d, ctl),
        Geometry::Polygon(polys) => aeqd_buffer(polys.clone(), true, d, ctl),
    }
}

/// A point's geodesic circle in lon/lat.
fn point_buffer(c: Coord, d: f64) -> MultiPolygon<f64> {
    if !c.is_finite() {
        return MultiPolygon::new(vec![]);
    }
    let g = wgs84();
    let lat0 = c.y.clamp(-90.0, 90.0);
    // Counter-clockwise: azimuth runs clockwise from north, so walk it backwards.
    let ring: Vec<(f64, f64)> = (0..CIRCLE_VERTICES)
        .map(|i| {
            let az = -(i as f64) * 360.0 / CIRCLE_VERTICES as f64;
            let (lat, lon): (f64, f64) = g.direct(lat0, c.x, az, d);
            (lon, lat)
        })
        .collect();
    let mut out = Vec::new();
    for p in finish_polygon(vec![ring], c.x) {
        out.extend(split_antimeridian(p));
    }
    MultiPolygon::new(out)
}

/// Exact ellipsoidal azimuthal equidistant projection centred at (lat0, lon0).
struct Aeqd {
    lat0: f64,
    lon0: f64,
}

impl Aeqd {
    fn forward(&self, lon: f64, lat: f64) -> GCoord<f64> {
        let (s, azi, _, _): (f64, f64, f64, f64) = wgs84().inverse(self.lat0, self.lon0, lat, lon);
        let a = azi.to_radians();
        GCoord { x: s * a.sin(), y: s * a.cos() }
    }

    fn inverse(&self, p: GCoord<f64>) -> (f64, f64) {
        let s = p.x.hypot(p.y);
        let azi = p.x.atan2(p.y).to_degrees();
        let (lat, lon): (f64, f64) = wgs84().direct(self.lat0, self.lon0, azi, s);
        (lon, lat)
    }
}

/// `x` wrapped into (−180, 180].
fn wrap180(x: f64) -> f64 {
    let r = (x + 180.0).rem_euclid(360.0) - 180.0;
    if r == -180.0 { 180.0 } else { r }
}

/// Points along the geodesic from `a` to `b` (lon, lat), including `a`, excluding `b`, with steps
/// no longer than `step` metres.
fn densify_edge(a: Coord, b: Coord, step: f64, out: &mut Vec<Coord>) {
    out.push(a);
    let g = wgs84();
    let (s, azi, _, _): (f64, f64, f64, f64) = g.inverse(a.y, a.x, b.y, b.x);
    let n = (s / step).ceil();
    if n.is_nan() || n <= 1.0 {
        return;
    }
    for k in 1..(n as usize) {
        let (lat, lon): (f64, f64) = g.direct(a.y, a.x, azi, s * k as f64 / n);
        out.push(Coord::new(lon, lat));
    }
}

/// A path densified along geodesics (closing vertex kept as given).
fn densify_path(pts: &[Coord], step: f64) -> Vec<Coord> {
    let mut out = Vec::with_capacity(pts.len());
    for w in pts.windows(2) {
        densify_edge(w[0], w[1], step, &mut out);
    }
    if let Some(l) = pts.last() {
        out.push(*l);
    }
    out
}

/// A planar ring densified to edges no longer than `step`.
fn densify_planar(ring: &LineString<f64>, step: f64) -> Vec<GCoord<f64>> {
    let mut out = Vec::with_capacity(ring.0.len());
    for w in ring.0.windows(2) {
        let (a, b) = (w[0], w[1]);
        out.push(a);
        let n = ((b.x - a.x).hypot(b.y - a.y) / step).ceil();
        if n > 1.0 {
            for k in 1..(n as usize) {
                let t = k as f64 / n;
                out.push(GCoord { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });
            }
        }
    }
    if let Some(l) = ring.0.last() {
        out.push(*l);
    }
    out
}

/// Buffers lines (each part a one-ring "polygon", `areas` false) or polygons in a local AEQD.
fn aeqd_buffer(parts: Vec<Vec<Vec<Coord>>>, areas: bool, d: f64, ctl: &Control) -> Result<MultiPolygon<f64>, Cancelled> {
    let parts: Vec<Vec<Vec<Coord>>> = parts
        .into_iter()
        .map(|rings| {
            rings.into_iter().map(|r| r.into_iter().filter(|c| c.is_finite()).collect::<Vec<_>>()).filter(|r| !r.is_empty()).collect()
        })
        .filter(|rings: &Vec<Vec<Coord>>| !rings.is_empty())
        .collect();
    let all: Vec<Coord> = parts.iter().flatten().flatten().copied().collect();
    let Some(first) = all.first().copied() else { return Ok(MultiPolygon::new(vec![])) };
    // Centre: mean latitude and circular-mean longitude (180° for a shape split at 180°).
    let n = all.len() as f64;
    let lat0 = (all.iter().map(|c| c.y).sum::<f64>() / n).clamp(-90.0, 90.0);
    let (sx, cx) = all.iter().fold((0.0, 0.0), |(s, c), p| (s + p.x.to_radians().sin(), c + p.x.to_radians().cos()));
    let lon0 = if sx.abs() < 1e-12 && cx.abs() < 1e-12 { first.x } else { wrap180(sx.atan2(cx).to_degrees()) };
    let proj = Aeqd { lat0, lon0 };

    // Step: DENSIFY_M, longer for very long features.
    let length: f64 = parts.iter().flatten().map(|r| geodesic::path_length_m(r)).sum();
    let step = DENSIFY_M.max(length / MAX_DENSIFY_VERTICES);
    let out_step = DENSIFY_M.max((length + std::f64::consts::TAU * d) / MAX_DENSIFY_VERTICES);

    let style = || BufferStyle::new(d).line_join(LineJoin::Round(join_angle())).line_cap(LineCap::Round(join_angle()));
    let mut planar = MultiPolygon::new(vec![]);
    if areas {
        let mut polys = Vec::new();
        for rings in &parts {
            ctl.check()?;
            let mut it = rings.iter().map(|r| {
                let mut r = r.clone();
                if r.first() != r.last() {
                    r.push(r[0]);
                }
                LineString::new(densify_path(&r, step).iter().map(|c| proj.forward(c.x, c.y)).collect())
            });
            let Some(ext) = it.next() else { continue };
            polys.push(Polygon::new(ext, it.collect()));
        }
        if !polys.is_empty() {
            ctl.check()?;
            planar = MultiPolygon::new(polys).buffer_with_style(style());
        }
    } else {
        let mut lines = Vec::new();
        for rings in &parts {
            ctl.check()?;
            for r in rings {
                lines.push(LineString::new(densify_path(r, step).iter().map(|c| proj.forward(c.x, c.y)).collect()));
            }
        }
        ctl.check()?;
        planar = MultiLineString::new(lines).buffer_with_style(style());
    }

    // Back to lon/lat.
    let mut out = Vec::new();
    for p in &planar.0 {
        ctl.check()?;
        let back = |ls: &LineString<f64>| -> Vec<(f64, f64)> {
            let mut v: Vec<(f64, f64)> = densify_planar(ls, out_step).into_iter().map(|c| proj.inverse(c)).collect();
            v.pop(); // closing repeat
            v
        };
        let mut rings = vec![back(p.exterior())];
        rings.extend(p.interiors().iter().map(back));
        for q in finish_polygon(rings, lon0) {
            out.extend(split_antimeridian(q));
        }
    }
    Ok(MultiPolygon::new(out))
}

/// Unwraps each ring's longitudes continuously (starting near `lon_ref`), closes a ring that
/// winds around a pole along that pole, and orients the result (exterior CCW, holes CW). The
/// result may have longitudes outside [−180, 180]; [`split_antimeridian`] folds them back.
fn finish_polygon(rings: Vec<Vec<(f64, f64)>>, lon_ref: f64) -> Vec<Polygon<f64>> {
    let mut lines: Vec<LineString<f64>> = Vec::new();
    for (k, ring) in rings.into_iter().enumerate() {
        if ring.len() < 3 {
            if k == 0 {
                return vec![];
            }
            continue;
        }
        let mut v: Vec<GCoord<f64>> = Vec::with_capacity(ring.len() + 4);
        let mut prev = lon_ref + wrap180(ring[0].0 - lon_ref);
        v.push(GCoord { x: prev, y: ring[0].1 });
        for &(lon, lat) in &ring[1..] {
            prev += wrap180(lon - prev);
            v.push(GCoord { x: prev, y: lat });
        }
        let start = v[0].x;
        let net = prev + wrap180(ring[0].0 - prev) - start;
        if net.abs() > 180.0 {
            // Winds around a pole: the one nearer the ring.
            let pole = if ring.iter().map(|p| p.1).sum::<f64>() >= 0.0 { 90.0 } else { -90.0 };
            let end = start + net;
            v.push(GCoord { x: end, y: ring[0].1 });
            v.push(GCoord { x: end, y: pole });
            v.push(GCoord { x: start, y: pole });
        }
        v.push(v[0]);
        lines.push(LineString::new(v));
    }
    let mut it = lines.into_iter();
    let Some(ext) = it.next() else { return vec![] };
    vec![Polygon::new(ext, it.collect()).orient(Direction::Default)]
}

/// Folds a polygon whose longitudes may run past ±180° back into [−180, 180], split along the
/// antimeridian, with seam vertices exactly on ±180°.
fn split_antimeridian(p: Polygon<f64>) -> Vec<Polygon<f64>> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in p.exterior().0.iter() {
        lo = lo.min(c.x);
        hi = hi.max(c.x);
    }
    if lo >= -180.0 && hi <= 180.0 {
        return vec![p];
    }
    let mut out = Vec::new();
    let (ylo, yhi) = p.exterior().0.iter().fold((90.0f64, -90.0f64), |(a, b), c| (a.min(c.y), b.max(c.y)));
    // Windows [−180, 180] + 360k over the shape, each cut tight to the shape so the overlay's
    // integer grid stays as fine as the shape itself.
    let (mx, my) = (1e-3 * (hi - lo) + 1e-9, 1e-3 * (yhi - ylo) + 1e-9);
    let (k0, k1) = (((lo + 180.0) / 360.0).floor() as i32, ((hi + 180.0) / 360.0).floor() as i32);
    for k in k0..=k1 {
        let off = 360.0 * f64::from(k);
        let (x0, x1) = ((-180.0 + off).max(lo - mx), (180.0 + off).min(hi + mx));
        if x1 <= x0 {
            continue;
        }
        let window = Rect::new(GCoord { x: x0, y: ylo - my }, GCoord { x: x1, y: yhi + my }).to_polygon();
        for q in p.intersection(&window).0 {
            let fix = |ls: &LineString<f64>| -> LineString<f64> {
                LineString::new(
                    ls.0.iter()
                        .map(|c| {
                            let mut x = c.x - off;
                            if (x - 180.0).abs() < 1e-7 {
                                x = 180.0;
                            } else if (x + 180.0).abs() < 1e-7 {
                                x = -180.0;
                            }
                            GCoord { x: x.clamp(-180.0, 180.0), y: c.y.clamp(-90.0, 90.0) }
                        })
                        .collect(),
                )
            };
            let q = Polygon::new(fix(q.exterior()), q.interiors().iter().map(fix).collect());
            out.push(q.orient(Direction::Default));
        }
    }
    out
}

/// A lon/lat multi-polygon as a polygon geometry; `None` when empty.
fn to_geometry(mp: &MultiPolygon<f64>) -> Option<Geometry> {
    let snap = |c: &GCoord<f64>| -> Coord {
        let y = if (c.y.abs() - 90.0).abs() < 1e-6 { 90f64.copysign(c.y) } else { c.y };
        Coord::new(c.x, y)
    };
    let ring = |ls: &LineString<f64>| -> Vec<Coord> {
        let mut v: Vec<Coord> = Vec::with_capacity(ls.0.len());
        for w in ls.0.windows(2) {
            let (a, b) = (snap(&w[0]), snap(&w[1]));
            v.push(a);
            // The overlay drops vertices that are collinear in lon/lat; put them back on long
            // edges so geodesic edges follow the straight lon/lat line (a parallel near a pole).
            let n = ((b.x - a.x).abs().max((b.y - a.y).abs()) / LONLAT_STEP_DEG).ceil();
            if n > 1.0 && !(a.y.abs() == 90.0 && a.y == b.y) && a.x != b.x {
                for k in 1..(n as usize) {
                    let t = k as f64 / n;
                    v.push(Coord::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
                }
            }
        }
        if let Some(l) = ls.0.last() {
            v.push(snap(l));
        }
        if v.len() > 1 && v.first() != v.last() {
            v.push(v[0]);
        }
        v
    };
    let polys: Vec<Vec<Vec<Coord>>> =
        mp.0.iter()
            .filter(|p| p.exterior().0.len() >= 4)
            .map(|p| std::iter::once(ring(p.exterior())).chain(p.interiors().iter().map(ring)).collect())
            .collect();
    if polys.is_empty() { None } else { Some(Geometry::Polygon(polys)) }
}

/// Geodesic distance along a path of clicked points (metres).
pub fn measure_distance_m(path: &[Coord]) -> f64 {
    geodesic::path_length_m(path)
}

/// Geodesic area (holes subtracted) and perimeter of polygons of rings (first ring exterior).
pub fn measure_area(polys: &[Vec<Vec<Coord>>]) -> AreaPerimeter {
    geodesic::polygon_area_perimeter(polys)
}

/// What a geometry measures geodesically (length for lines, area and perimeter for areas).
pub fn measure_geometry(g: &Geometry) -> geodesic::Measure {
    geodesic::measure(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn fixture(rel: &str) -> String {
        let p: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", rel].iter().collect();
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    }

    fn n5_geoms() -> HashMap<String, Geometry> {
        let (ds, _) = rustgis_data::io::read_geojson(&fixture("N5/cases.geojson"), "cases").unwrap();
        let id_col = ds.fields.iter().position(|f| f.name == "feature_id").unwrap();
        ds.features
            .iter()
            .map(|f| {
                let rustgis_data::Value::Text(id) = &f.attrs[id_col] else { panic!("feature_id") };
                (id.clone(), f.geometry.clone().unwrap())
            })
            .collect()
    }

    fn area(g: &Geometry) -> f64 {
        geodesic::geometry_area_m2(g).unwrap()
    }

    fn polys(g: &Geometry) -> &Vec<Vec<Vec<Coord>>> {
        let Geometry::Polygon(p) = g else { panic!("not a polygon: {g:?}") };
        p
    }

    /// Every vertex in range, every ring closed and no ring wider than 180° of longitude.
    fn assert_sane(g: &Geometry) {
        for poly in polys(g) {
            for ring in poly {
                assert!(ring.len() >= 4 && ring.first() == ring.last(), "ring not closed");
                let (lo, hi) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), c| (a.min(c.x), b.max(c.x)));
                assert!(lo >= -180.0 && hi <= 180.0, "longitude out of range {lo}..{hi}");
                assert!(ring.iter().all(|c| (-90.0..=90.0).contains(&c.y)));
            }
        }
    }

    #[test]
    fn n5_buffer_areas() {
        let geoms = n5_geoms();
        let csv = fixture("N5/expected_buffer.csv");
        let mut checked = 0;
        for line in csv.lines().skip(1).filter(|l| !l.trim().is_empty()) {
            let f: Vec<&str> = line.split(',').collect();
            let (id, d, want) = (f[0], f[1].parse::<f64>().unwrap(), f[2].parse::<f64>().unwrap());
            let g = &geoms[id];
            let b = buffer_geometry(g, d, &Control::none()).unwrap().unwrap();
            assert_sane(&b);
            let got = area(&b);
            let rel = (got - want) / want;
            // Spec: points within 0.1%, lines and areas within 0.5% (N5 suggests 0.1% for all).
            let tol = 1e-3;
            eprintln!("{id:9} {d:>6} m  got {got:.6e}  want {want:.6e}  rel {rel:+.2e}");
            assert!(rel.abs() <= tol, "{id} at {d} m: {got} vs {want} (rel {rel:e})");
            if id.ends_with("_am") {
                // Split at 180°, both sides present.
                let xs: Vec<f64> = polys(&b).iter().flatten().flatten().map(|c| c.x).collect();
                assert!(xs.contains(&180.0) && xs.contains(&-180.0), "{id}: not split at 180°");
                assert!(polys(&b).len() >= 2);
            }
            checked += 1;
        }
        assert_eq!(checked, 18);
    }

    #[test]
    fn a_circle_is_a_circle() {
        // Geodesic circle area vs the exact area of a geodesic disc ≈ πr² for small r.
        for (lat, d) in [(0.0, 1000.0), (60.0, 1000.0), (-45.0, 250.0), (89.0, 2000.0)] {
            let b = buffer_geometry(&Geometry::Point(Coord::new(10.0, lat)), d, &Control::none()).unwrap().unwrap();
            let want = std::f64::consts::PI * d * d;
            let rel = (area(&b) - want) / want;
            assert!(rel < 0.0 && rel > -2e-4, "lat {lat}: rel {rel:e}");
        }
    }

    #[test]
    fn a_buffer_around_a_pole_is_a_cap() {
        let d = 5000.0;
        let want = std::f64::consts::PI * d * d;
        for lat in [89.99, -89.99, 90.0] {
            let b = buffer_geometry(&Geometry::Point(Coord::new(30.0, lat)), d, &Control::none()).unwrap().unwrap();
            assert_sane(&b);
            let rel = (area(&b) - want) / want;
            assert!(rel.abs() < 2e-3, "lat {lat}: area {} rel {rel:e}", area(&b));
        }
        // A line passing close to the pole.
        let line = Geometry::Line(vec![vec![Coord::new(0.0, 89.98), Coord::new(180.0, 89.98)]]);
        let b = buffer_geometry(&line, d, &Control::none()).unwrap().unwrap();
        assert_sane(&b);
        let a = area(&b);
        // ~4.4 km long line: πd² + 2d·len, give or take.
        assert!(a > want && a < want + 2.0 * d * 5000.0, "line near pole: {a}");
    }

    #[test]
    fn antimeridian_point_does_not_wrap() {
        for x in [180.0, -180.0, 179.9999, -179.9999] {
            let b = buffer_geometry(&Geometry::Point(Coord::new(x, -30.0)), 2000.0, &Control::none()).unwrap().unwrap();
            assert_sane(&b);
            let want = std::f64::consts::PI * 4e6;
            assert!(((area(&b) - want) / want).abs() < 2e-4);
            // Every part hugs the antimeridian (none spans the globe).
            for poly in polys(&b) {
                assert!(poly[0].iter().all(|c| c.x.abs() > 179.9));
            }
        }
    }

    #[test]
    fn area_grows_with_distance() {
        let geoms = n5_geoms();
        let mut ids: Vec<&String> = geoms.keys().collect();
        ids.sort();
        for id in ids {
            let mut last = 0.0;
            for d in [10.0, 100.0, 1000.0, 5000.0, 20_000.0, 80_000.0] {
                let a = area(&buffer_geometry(&geoms[id], d, &Control::none()).unwrap().unwrap());
                assert!(a > last, "{id}: {a} at {d} not above {last}");
                last = a;
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn buffer_grows_with_distance(x in -180.0f64..180.0, y in -85.0f64..85.0, dx in -0.05f64..0.05, dy in -0.05f64..0.05,
                                      d in 10.0f64..20_000.0, k in 1.05f64..3.0) {
            let line = Geometry::Line(vec![vec![Coord::new(x, y), Coord::new(x + dx, y + dy)]]);
            let a1 = area(&buffer_geometry(&line, d, &Control::none()).unwrap().unwrap());
            let a2 = area(&buffer_geometry(&line, d * k, &Control::none()).unwrap().unwrap());
            proptest::prop_assert!(a2 > a1, "{a1} then {a2}");
        }
    }

    #[test]
    fn deterministic() {
        let geoms = n5_geoms();
        let mut layer: Vec<Option<Geometry>> = {
            let mut ids: Vec<&String> = geoms.keys().collect();
            ids.sort();
            ids.into_iter().map(|id| Some(geoms[id].clone())).collect()
        };
        layer.push(None);
        let opts = BufferOptions::new(30.0, DistanceUnit::Kilometers);
        let a = buffer_layer(&layer, &opts, &Control::none()).unwrap();
        let b = buffer_layer(&layer, &opts, &Control::none()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn merge_unions_overlaps_and_per_feature_keeps_them() {
        // Two points 1 km apart with 1 km buffers overlap; a third is far away.
        let pts = [Coord::new(0.0, 0.0), Coord::new(0.009, 0.0), Coord::new(5.0, 5.0)];
        let layer: Vec<Option<Geometry>> = pts.iter().map(|c| Some(Geometry::Point(*c))).chain([None]).collect();
        let mut opts = BufferOptions::new(1.0, DistanceUnit::Kilometers);
        assert!(opts.merge);
        let BufferOutput::Merged(Some(m)) = buffer_layer(&layer, &opts, &Control::none()).unwrap() else { panic!() };
        assert_sane(&m);
        assert_eq!(polys(&m).len(), 2);
        opts.merge = false;
        let BufferOutput::PerFeature(per) = buffer_layer(&layer, &opts, &Control::none()).unwrap() else { panic!() };
        assert_eq!(per.len(), 4);
        assert!(per[3].is_none());
        let sum: f64 = per.iter().flatten().map(area).sum();
        let merged = area(&m);
        assert!(merged < sum * 0.95 && merged > sum * 0.6, "{merged} vs {sum}");
        let circle = area(per[0].as_ref().unwrap());
        assert!((circle - std::f64::consts::PI * 1e6).abs() / (std::f64::consts::PI * 1e6) < 2e-4);
    }

    #[test]
    fn polygon_with_hole_and_negative_inputs() {
        let sq = |x0: f64, y0: f64, s: f64| {
            vec![Coord::new(x0, y0), Coord::new(x0 + s, y0), Coord::new(x0 + s, y0 + s), Coord::new(x0, y0 + s), Coord::new(x0, y0)]
        };
        // A 0.1° square with a 0.06° hole: a 1 km buffer keeps a smaller hole.
        let g = Geometry::Polygon(vec![vec![sq(0.0, 0.0, 0.1), sq(0.02, 0.02, 0.06)]]);
        let b = buffer_geometry(&g, 1000.0, &Control::none()).unwrap().unwrap();
        assert_eq!(polys(&b).len(), 1);
        assert_eq!(polys(&b)[0].len(), 2, "hole kept");
        assert!(area(&b) > area(&g));
        // A 5 km buffer fills the hole.
        let b = buffer_geometry(&g, 5000.0, &Control::none()).unwrap().unwrap();
        assert_eq!(polys(&b)[0].len(), 1, "hole filled");
        for d in [0.0, -1.0, f64::NAN, f64::INFINITY, 2e7] {
            assert!(matches!(buffer_geometry(&g, d, &Control::none()), Err(BufferError::InvalidDistance(_))), "{d}");
        }
        assert_eq!(buffer_geometry(&Geometry::MultiPoint(vec![]), 10.0, &Control::none()), Ok(None));
        assert_eq!(DistanceUnit::Kilometers.to_meters(2.5), 2500.0);
        assert_eq!(DISTANCE_LABEL, "Straight-line distance");
    }

    #[test]
    fn a_cancel_stops_the_buffer() {
        let ctl = Control::none();
        ctl.cancel();
        let layer = vec![Some(Geometry::Point(Coord::new(0.0, 0.0)))];
        assert_eq!(buffer_layer(&layer, &BufferOptions::new(1.0, DistanceUnit::Meters), &ctl), Err(BufferError::Cancelled));
    }

    #[test]
    fn measure_matches_n5() {
        let geoms = n5_geoms();
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-4 * b;
        let Geometry::Line(l) = &geoms["line_60n"] else { panic!() };
        assert!(close(measure_distance_m(&l[0]), 55_799.470_393_3));
        let Geometry::Polygon(p) = &geoms["poly_eq"] else { panic!() };
        let ap = measure_area(p);
        assert!(close(ap.area_m2, 12_308_778_361.5) && close(ap.perimeter_m, 443_770.917_248));
        assert!(matches!(measure_geometry(&geoms["poly_am"]), geodesic::Measure::Area(a) if close(a.area_m2, 12_108_258_313.0)));
    }

    /// Gate 4 stress: 10,000 points clustered in a city, buffered 1 km and merged, in under 30 s.
    /// `cargo test -p lm-analysis --release -- --ignored stress_10k --nocapture`
    #[test]
    #[ignore]
    fn stress_10k_points_1km_merged() {
        // xorshift; clusters around a few centres in a ~30 km city, Gaussian-ish spread.
        let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut rnd = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        let centres: Vec<(f64, f64)> = (0..30).map(|_| (116.4 + (rnd() - 0.5) * 0.45, 39.9 + (rnd() - 0.5) * 0.35)).collect();
        let layer: Vec<Option<Geometry>> = (0..10_000)
            .map(|i| {
                let (cx, cy) = centres[i % centres.len()];
                let g = |r: &mut dyn FnMut() -> f64| (r() + r() + r() - 1.5) * 0.03;
                Some(Geometry::Point(Coord::new(cx + g(&mut rnd), cy + g(&mut rnd))))
            })
            .collect();
        // The longest gap between progress reports, which come with every cancel check.
        let gap = std::sync::Arc::new(std::sync::Mutex::new((std::time::Instant::now(), 0.0f64)));
        let g2 = gap.clone();
        let ctl = Control::with_progress(move |_| {
            let mut g = g2.lock().unwrap();
            g.1 = g.1.max(g.0.elapsed().as_secs_f64());
            g.0 = std::time::Instant::now();
        });
        let t = std::time::Instant::now();
        gap.lock().unwrap().0 = t;
        let out = buffer_layer(&layer, &BufferOptions::new(1.0, DistanceUnit::Kilometers), &ctl).unwrap();
        let secs = t.elapsed().as_secs_f64();
        let BufferOutput::Merged(Some(m)) = out else { panic!() };
        let verts: usize = polys(&m).iter().flatten().map(|r| r.len()).sum();
        let holes: usize = polys(&m).iter().map(|p| p.len() - 1).sum();
        let max_gap = gap.lock().unwrap().1;
        eprintln!(
            "stress: 10,000 points, 1 km, merged in {secs:.2} s -> {} polygons, {holes} holes, {verts} vertices, {:.1} km²; longest gap between checks {:.1} ms",
            polys(&m).len(),
            area(&m) / 1e6,
            max_gap * 1e3
        );
        assert!(secs < 30.0);
        assert!(max_gap < 0.2);
    }
}
