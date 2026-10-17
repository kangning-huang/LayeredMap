//! "Add area and coordinates to each [feature]": new action A4 (`docs/lm/puzzles/mechanics.md`
//! › A4), and the label point it reports.
//!
//! # Definition
//!
//! - **Areas** get `area_km2` and `perimeter_km`, **lines** `length_km`, all geodesic on WGS 84
//!   with GeographicLib (Karney) through [`crate::geodesic`]: holes subtracted, ring orientation
//!   irrelevant, RFC 7946 antimeridian seams left out of perimeters.
//! - **Every feature** gets `lon`, `lat` (degrees) of its [`label_point`], a point guaranteed to
//!   lie on the feature:
//!   - a point: the point itself (a multipoint: its first finite point);
//!   - a line: the vertex nearest the middle of its geodesic length, counting the parts in order
//!     (the gaps between parts are not length);
//!   - an area: a point strictly inside its largest part (by geodesic area), never in a hole.
//!     A scan line through the part (at a height no vertex has) gives an interior seed, the
//!     midpoint of the inside span whose point lies farthest from the boundary; a
//!     pole-of-inaccessibility search (quadtree cells, best first, x scaled by the cosine of the
//!     latitude) then moves it towards the point farthest from every edge, under a work budget.
//!     Whatever it finds must pass the exact point-in-polygon test of [`crate::overlay`] as
//!     *inside*; otherwise the seed is used, and for a part with no inside at all (zero area) its
//!     first vertex, which is on the feature.
//! - **Missing geometry.** A geometry with no usable coordinate gets nothing (all `None`).
//!   Non-finite vertices are skipped by lengths and areas as in [`crate::geodesic`], and polygon
//!   parts with a non-finite vertex are not chosen for the label point.

use crate::geodesic::{line_length_m, path_length_m, polygon_area_perimeter};
use crate::overlay::{Location, locate_in_polygon};
use rustgis_data::{Coord, Geometry};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// What "Add area and coordinates" writes for one feature.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measurements {
    /// Areas only: geodesic area (km²).
    pub area_km2: Option<f64>,
    /// Areas only: geodesic perimeter, holes included, seams excluded (km).
    pub perimeter_km: Option<f64>,
    /// Lines only: geodesic length (km).
    pub length_km: Option<f64>,
    /// Longitude of the label point (degrees).
    pub lon: Option<f64>,
    /// Latitude of the label point (degrees).
    pub lat: Option<f64>,
}

/// A point guaranteed on the geometry: the point itself; for lines the vertex nearest the middle
/// of the length; for polygons a point inside the largest part, never in a hole (module notes).
pub fn label_point(g: &Geometry) -> Option<Coord> {
    match g {
        Geometry::Point(c) => c.is_finite().then_some(*c),
        Geometry::MultiPoint(v) => v.iter().copied().find(|c| c.is_finite()),
        Geometry::Line(parts) => line_label(parts),
        Geometry::Polygon(polys) => polygon_label(polys),
    }
}

/// Geodesic measurements and the label point's coordinates (module notes).
pub fn measurements(g: &Geometry) -> Measurements {
    let Some(lp) = label_point(g) else { return Measurements::default() };
    let mut m = Measurements { lon: Some(lp.x), lat: Some(lp.y), ..Measurements::default() };
    match g {
        Geometry::Line(parts) => m.length_km = Some(line_length_m(parts) / 1000.0),
        Geometry::Polygon(polys) => {
            let ap = polygon_area_perimeter(polys);
            m.area_km2 = Some(ap.area_m2 / 1e6);
            m.perimeter_km = Some(ap.perimeter_m / 1000.0);
        }
        Geometry::Point(_) | Geometry::MultiPoint(_) => {}
    }
    m
}

// ---------------------------------------------------------------------------------------------

fn line_label(parts: &[Vec<Coord>]) -> Option<Coord> {
    let mut at: Vec<(f64, Coord)> = Vec::new();
    let mut total = 0.0;
    for part in parts {
        let mut prev: Option<Coord> = None;
        for c in part.iter().copied().filter(|c| c.is_finite()) {
            if let Some(p) = prev {
                total += path_length_m(&[p, c]);
            }
            at.push((total, c));
            prev = Some(c);
        }
    }
    let half = total / 2.0;
    let mut best: Option<(f64, Coord)> = None;
    for &(s, c) in &at {
        let d = (s - half).abs();
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c)
}

fn polygon_label(polys: &[Vec<Vec<Coord>>]) -> Option<Coord> {
    let mut best: Option<(f64, &Vec<Vec<Coord>>)> = None;
    for rings in polys {
        if !rings.first().is_some_and(|o| !o.is_empty()) || !rings.iter().flatten().all(|c| c.is_finite()) {
            continue;
        }
        let a = polygon_area_perimeter(std::slice::from_ref(rings)).area_m2;
        if best.is_none_or(|(ba, _)| a > ba) {
            best = Some((a, rings));
        }
    }
    let (_, rings) = best?;
    interior_point(rings)
}

/// Even-odd inside test (fast, not exact; the exact one decides at the end).
fn inside_fast(rings: &[Vec<Coord>], x: f64, y: f64) -> bool {
    let mut inside = false;
    for ring in rings {
        let n = ring.len();
        for i in 0..n {
            let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % n)) else { continue };
            if (a.y > y) != (b.y > y) && x < a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Signed distance (positive inside) from (x, y) to the rings, in coordinates with x scaled by `kx`.
fn signed_distance(rings: &[Vec<Coord>], kx: f64, x: f64, y: f64) -> f64 {
    let mut d2 = f64::INFINITY;
    for ring in rings {
        let n = ring.len();
        for i in 0..n {
            let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % n)) else { continue };
            let (ax, ay, bx, by) = (a.x * kx, a.y, b.x * kx, b.y);
            let (dx, dy) = (bx - ax, by - ay);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 { (((x * kx - ax) * dx + (y - ay) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let (ex, ey) = (ax + t * dx - x * kx, ay + t * dy - y);
            d2 = d2.min(ex * ex + ey * ey);
        }
    }
    let d = d2.sqrt();
    if inside_fast(rings, x, y) { d } else { -d }
}

fn strictly_inside(rings: &[Vec<Coord>], c: Coord) -> bool {
    c.is_finite() && locate_in_polygon(rings, c) == Location::Inside
}

/// A quadtree cell of the pole-of-inaccessibility search (unscaled centre, scaled half-size).
struct Cell {
    x: f64,
    y: f64,
    h: f64,
    d: f64,
    max: f64,
}

impl Cell {
    fn new(rings: &[Vec<Coord>], kx: f64, x: f64, y: f64, h: f64) -> Cell {
        let d = signed_distance(rings, kx, x, y);
        Cell { x, y, h, d, max: d + h * std::f64::consts::SQRT_2 }
    }
}

impl PartialEq for Cell {
    fn eq(&self, o: &Self) -> bool {
        self.max.total_cmp(&o.max) == Ordering::Equal
    }
}
impl Eq for Cell {}
impl PartialOrd for Cell {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Cell {
    fn cmp(&self, o: &Self) -> Ordering {
        self.max.total_cmp(&o.max)
    }
}

/// A point strictly inside the polygon `rings` (exterior first), or its first vertex when it has
/// no inside.
fn interior_point(rings: &[Vec<Coord>]) -> Option<Coord> {
    let outer = rings.first()?;
    let first = *outer.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in outer {
        (x0, y0, x1, y1) = (x0.min(c.x), y0.min(c.y), x1.max(c.x), y1.max(c.y));
    }
    let kx = ((y0 + y1) / 2.0).to_radians().cos().max(1e-6);

    // Seed: scan lines at heights between distinct vertex heights.
    let mut ys: Vec<f64> = rings.iter().flatten().map(|c| c.y).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    let mut gaps: Vec<(f64, f64)> = ys.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect();
    let cy = (y0 + y1) / 2.0;
    let central = gaps.iter().copied().find(|(a, b)| *a <= cy && cy <= *b);
    gaps.sort_by(|a, b| (b.1 - b.0).total_cmp(&(a.1 - a.0)));
    gaps.truncate(8);
    gaps.extend(central);
    let mut seeds: Vec<(f64, Coord)> = Vec::new();
    for (ga, gb) in gaps {
        let y = ga + (gb - ga) / 2.0;
        if y <= ga || y >= gb {
            continue;
        }
        let mut xs: Vec<f64> = Vec::new();
        for ring in rings {
            let n = ring.len();
            for i in 0..n {
                let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % n)) else { continue };
                if (a.y < y) != (b.y < y) {
                    xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        let mut spans: Vec<(f64, f64)> = xs.as_chunks::<2>().0.iter().map(|&[a, b]| (a, b)).filter(|(a, b)| b > a).collect();
        spans.sort_by(|a, b| (b.1 - b.0).total_cmp(&(a.1 - a.0)));
        for (a, b) in spans.into_iter().take(4) {
            let c = Coord::new(a + (b - a) / 2.0, y);
            seeds.push((signed_distance(rings, kx, c.x, c.y), c));
        }
    }
    seeds.sort_by(|a, b| b.0.total_cmp(&a.0));
    let Some(&(_, seed)) = seeds.iter().find(|(_, c)| strictly_inside(rings, *c)) else { return Some(first) };

    // Pole of inaccessibility from the seed, under a budget of distance evaluations.
    let n: usize = rings.iter().map(Vec::len).sum::<usize>().max(1);
    let budget = (4_000_000 / n).clamp(16, 1000);
    let size = ((x1 - x0) * kx).max(y1 - y0);
    if size.is_nan() || size <= 0.0 {
        return Some(seed);
    }
    let precision = size * 1e-3;
    let mut best = Cell::new(rings, kx, seed.x, seed.y, 0.0);
    let mut heap = BinaryHeap::new();
    heap.push(Cell::new(rings, kx, (x0 + x1) / 2.0, (y0 + y1) / 2.0, size / 2.0));
    let mut evaluated = 1;
    while let Some(cell) = heap.pop() {
        if cell.d > best.d {
            best = Cell { h: 0.0, max: cell.d, ..cell };
        }
        if cell.max - best.d <= precision || evaluated >= budget {
            continue;
        }
        let h = cell.h / 2.0;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            heap.push(Cell::new(rings, kx, cell.x + sx * h / kx, cell.y + sy * h, h));
        }
        evaluated += 4;
    }
    let pole = Coord::new(best.x, best.y);
    Some(if strictly_inside(rings, pole) { pole } else { seed })
}

#[cfg(test)]
mod tests;
