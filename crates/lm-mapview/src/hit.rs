//! Hit testing (build plan › Phase 3 › WP3.1): which objects a click lands on.
//!
//! Each vector layer gets a [`HitIndex`], an R-tree of its features' bounding boxes in Web
//! Mercator metres, built once per layer data and off the interface thread. A click then asks
//! the index for the few features whose boxes come near it and tests each one exactly, in screen
//! pixels at the current zoom:
//! - points hit within their drawn radius plus the tolerance;
//! - lines within half their width plus the tolerance of any segment;
//! - areas when the click is inside (even–odd, so holes are holes and any part of a
//!   multipolygon counts) or within the tolerance of an outline.
//!
//! The tolerance is 4 px, or 6 px for points drawn under 5 px. Distances are measured in Web
//! Mercator, the plane the map is drawn in, so a hit is exactly what the eye sees. The map draws
//! each feature at its own longitudes and again in the neighbouring copies of the world
//! (`ViewSettings::world_copies`, turned on by [`crate::MapView`]), so a click is tested where it
//! is and one and two worlds east and west of it: a storm track drawn past the antimeridian is hit
//! in whichever copy it was clicked.
//!
//! Hit testing reads features, never pixels; the basemap (and its labels) is never tested.

use rstar::RTree;
use rstar::primitives::{GeomWithData, Rectangle};
use rustgis_geom::{Coord, lonlat_to_merc};
use rustgis_map::RasterLayer;
use rustgis_render::rustgis_data::{Dataset, Geometry};

/// The click tolerance, logical pixels.
pub const TOLERANCE_PX: f64 = 4.0;
/// The tolerance for points drawn smaller than [`SMALL_POINT_PX`].
pub const SMALL_POINT_TOLERANCE_PX: f64 = 6.0;
/// Points drawn under this diameter (dense layers' 4 px dots) get the larger tolerance.
pub const SMALL_POINT_PX: f64 = 5.0;
/// The most hits one layer reports for a click (the nearest ones). When a layer returns this
/// many, the overlap chooser should say "100+ objects here".
pub const MAX_HITS_PER_LAYER: usize = 100;

type Entry = GeomWithData<Rectangle<[f32; 2]>, u32>;

/// An R-tree of a dataset's feature bounding boxes (Web Mercator metres, stored as `f32` rounded
/// outwards, so a box never shrinks). About 48 bytes per feature. Rows without geometry, or with
/// none that can be placed, are left out.
pub struct HitIndex {
    tree: RTree<Entry>,
    /// Rows in the dataset it was built from: a query against a different length is stale.
    rows: usize,
}

impl std::fmt::Debug for HitIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HitIndex").field("indexed", &self.tree.size()).field("rows", &self.rows).finish()
    }
}

impl HitIndex {
    /// Indexes `data` (longitude/latitude). Slow for large layers (about a second per million
    /// features): build it on a background thread when the layer's data arrives.
    pub fn build(data: &Dataset) -> HitIndex {
        let entries: Vec<Entry> = data
            .features
            .iter()
            .enumerate()
            .filter_map(|(row, f)| {
                let row = u32::try_from(row).ok()?;
                let e = f.geometry.as_ref()?.extent();
                if e.is_empty() || ![e.min_x, e.min_y, e.max_x, e.max_y].iter().all(|v| v.is_finite()) {
                    return None;
                }
                // Mercator is monotone in longitude and latitude, so the corners bound the box.
                let lo = lonlat_to_merc(Coord::new(e.min_x, e.min_y));
                let hi = lonlat_to_merc(Coord::new(e.max_x, e.max_y));
                let rect = Rectangle::from_corners([down(lo.x), down(lo.y)], [up(hi.x), up(hi.y)]);
                Some(GeomWithData::new(rect, row))
            })
            .collect();
        HitIndex { tree: RTree::bulk_load(entries), rows: data.features.len() }
    }

    /// Features indexed.
    pub fn len(&self) -> usize {
        self.tree.size()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    /// Rows in the dataset it was built from (rebuild when the layer's row count changes).
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The features of `data` that a click at `at` (Web Mercator metres) hits, nearest first, at
    /// `mpp` Mercator metres per logical pixel: at most [`MAX_HITS_PER_LAYER`], the nearest ones
    /// (zoomed far out, a click's few pixels can cover a whole city of features).
    pub fn query(&self, data: &Dataset, sym: &HitSymbol, at: Coord, mpp: f64) -> Vec<RowHit> {
        self.query_limited(data, sym, at, mpp, MAX_HITS_PER_LAYER)
    }

    /// [`HitIndex::query`] keeping at most `limit` hits.
    pub fn query_limited(&self, data: &Dataset, sym: &HitSymbol, at: Coord, mpp: f64, limit: usize) -> Vec<RowHit> {
        if !(mpp.is_finite() && mpp > 0.0 && at.is_finite()) || limit == 0 {
            return Vec::new();
        }
        let reach_px = sym.reach_px();
        // Boxes come nearest first (by distance from the click to the box, a lower bound on the
        // distance to the feature inside). Boxes are stored as f32, so that bound can be off by
        // the rounding of the click itself (about 2 m at the world's edge): allow for it.
        let q = [at.x as f32, at.y as f32];
        let slack_m = 2.0 * f64::from(q[0].abs().max(q[1].abs()).max(1.0)) * f64::from(f32::EPSILON) + 1e-3;
        // The best `limit` hits so far, worst on top.
        let mut best: std::collections::BinaryHeap<Ranked> = std::collections::BinaryHeap::new();
        for (e, d2) in self.tree.nearest_neighbor_iter_with_distance_2(&q) {
            let box_m = f64::from(d2).max(0.0).sqrt();
            let bound_px = (box_m * (1.0 - 1e-6) - slack_m).max(0.0) / mpp;
            if bound_px > reach_px {
                break; // every box from here on is out of reach
            }
            if best.len() >= limit && best.peek().is_some_and(|w| w.0.distance_px <= bound_px) {
                break; // nothing further can beat the hits kept
            }
            let row = e.data as usize;
            let Some(g) = data.features.get(row).and_then(|f| f.geometry.as_ref()) else { continue };
            if let Some(d) = hit_geometry(g, sym, at, mpp) {
                best.push(Ranked(RowHit { row, distance_px: d }));
                if best.len() > limit {
                    best.pop();
                }
            }
        }
        // Nearest first; at the same distance the later row, drawn on top, first.
        let mut out: Vec<RowHit> = best.into_iter().map(|r| r.0).collect();
        out.sort_by(rank);
        out
    }
}

/// Nearer first; at the same distance the later row (drawn on top) first.
fn rank(a: &RowHit, b: &RowHit) -> std::cmp::Ordering {
    a.distance_px.total_cmp(&b.distance_px).then(b.row.cmp(&a.row))
}

/// A hit ordered by [`rank`], for the heap of best hits.
struct Ranked(RowHit);

impl PartialEq for Ranked {
    fn eq(&self, o: &Self) -> bool {
        rank(&self.0, &o.0).is_eq()
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ranked {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        rank(&self.0, &o.0)
    }
}

/// The largest `f32` not above `v`.
fn down(v: f64) -> f32 {
    let f = v as f32;
    if f64::from(f) > v { f.next_down() } else { f }
}

/// The smallest `f32` not below `v`.
fn up(v: f64) -> f32 {
    let f = v as f32;
    if f64::from(f) < v { f.next_up() } else { f }
}

/// How a layer's features are drawn, for hit testing: the point diameter and line width in
/// logical pixels (the layer's symbol).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitSymbol {
    /// Point marker diameter (7 px by default, 4 px for dense layers).
    pub point_size: f64,
    /// Line width (2 px by default).
    pub line_width: f64,
}

impl Default for HitSymbol {
    fn default() -> Self {
        HitSymbol { point_size: 7.0, line_width: 2.0 }
    }
}

impl HitSymbol {
    /// The symbol a layer draws its first feature with (its renderer's size and width).
    pub fn of_layer(layer: &rustgis_map::Layer) -> HitSymbol {
        let s = layer.symbol(0);
        HitSymbol { point_size: f64::from(s.size), line_width: f64::from(s.stroke_width) }
    }

    /// The point tolerance: 6 px for points drawn under 5 px, else 4 px.
    pub fn point_tolerance_px(&self) -> f64 {
        if self.point_size < SMALL_POINT_PX { SMALL_POINT_TOLERANCE_PX } else { TOLERANCE_PX }
    }

    /// The radius in pixels within which a point hits.
    fn point_reach(&self) -> f64 {
        self.point_size.max(0.0) / 2.0 + self.point_tolerance_px()
    }

    /// The distance in pixels within which a line hits.
    fn line_reach(&self) -> f64 {
        self.line_width.max(0.0) / 2.0 + TOLERANCE_PX
    }

    /// The farthest a hit can be from the click, in pixels (the index search box).
    fn reach_px(&self) -> f64 {
        self.point_reach().max(self.line_reach()).max(TOLERANCE_PX)
    }
}

/// A feature a click hit within one layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowHit {
    pub row: usize,
    /// From the click to the feature's centre (points) or nearest edge (lines, area outlines);
    /// 0 inside an area.
    pub distance_px: f64,
}

/// Whether a click at `q` (Mercator metres) hits `g` (longitude/latitude); the distance in pixels
/// if so.
pub fn hit_geometry(g: &Geometry, sym: &HitSymbol, q: Coord, mpp: f64) -> Option<f64> {
    let px = |m: f64| m / mpp;
    match g {
        Geometry::Point(c) => {
            let d = px(dist(lonlat_to_merc(*c), q));
            (d <= sym.point_reach()).then_some(d)
        }
        Geometry::MultiPoint(v) => {
            let d = v.iter().map(|c| px(dist(lonlat_to_merc(*c), q))).fold(f64::INFINITY, f64::min);
            (d <= sym.point_reach()).then_some(d)
        }
        Geometry::Line(parts) => {
            let d = px(parts.iter().map(|p| path_distance(p, q, false)).fold(f64::INFINITY, f64::min));
            (d <= sym.line_reach()).then_some(d)
        }
        Geometry::Polygon(polys) => {
            // Even–odd over every ring of every part, as the renderer fills them.
            let mut inside = false;
            let mut edge = f64::INFINITY;
            for ring in polys.iter().flatten() {
                let (crossings, d) = ring_scan(ring, q);
                inside ^= crossings;
                edge = edge.min(d);
            }
            if inside {
                return Some(0.0);
            }
            let d = px(edge);
            (d <= TOLERANCE_PX).then_some(d)
        }
    }
}

fn dist(a: Coord, b: Coord) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// Distance from `q` to the segment `a`–`b`.
fn segment_distance(a: Coord, b: Coord, q: Coord) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((q.x - a.x) * dx + (q.y - a.y) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    dist(Coord::new(a.x + t * dx, a.y + t * dy), q)
}

/// The nearest distance from `q` to a polyline (closed: back to its start) in Mercator metres.
fn path_distance(path: &[Coord], q: Coord, closed: bool) -> f64 {
    let pts: Vec<Coord> = path.iter().map(|c| lonlat_to_merc(*c)).filter(|c| c.is_finite()).collect();
    match pts.as_slice() {
        [] => f64::INFINITY,
        [only] => dist(*only, q),
        _ => {
            let mut d = pts.windows(2).map(|w| segment_distance(w[0], w[1], q)).fold(f64::INFINITY, f64::min);
            if closed && let (Some(first), Some(last)) = (pts.first(), pts.last()) {
                d = d.min(segment_distance(*last, *first, q));
            }
            d
        }
    }
}

/// For a ring: whether a ray from `q` towards +x crosses it an odd number of times, and the
/// distance from `q` to its outline (Mercator metres). Rings close themselves.
fn ring_scan(ring: &[Coord], q: Coord) -> (bool, f64) {
    let pts: Vec<Coord> = ring.iter().map(|c| lonlat_to_merc(*c)).filter(|c| c.is_finite()).collect();
    let n = pts.len();
    if n == 0 {
        return (false, f64::INFINITY);
    }
    let mut odd = false;
    let mut d = f64::INFINITY;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        d = d.min(segment_distance(a, b, q));
        if (a.y > q.y) != (b.y > q.y) {
            let x = a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if q.x < x {
                odd = !odd;
            }
        }
    }
    (odd, d)
}

/// A selectable vector layer to hit-test, as the caller sees it.
#[derive(Clone, Copy)]
pub struct HitLayer<'a> {
    /// The caller's id for the layer, returned in each [`Hit`].
    pub id: u32,
    pub index: &'a HitIndex,
    pub data: &'a Dataset,
    pub symbol: HitSymbol,
}

/// One object a click hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hit {
    /// The caller's layer id ([`HitLayer::id`]).
    pub layer: u32,
    pub row: usize,
}

/// Every object a click at `at` (Web Mercator metres) hits in `layers` (visible, selectable,
/// topmost first), at `mpp` Mercator metres per logical pixel: the topmost layer's hits first,
/// each layer's nearest first. More than one hit means the overlap chooser.
pub fn hit_test_merc(layers: &[HitLayer<'_>], at: Coord, mpp: f64) -> Vec<Hit> {
    let mut out = Vec::new();
    for l in layers {
        if l.index.rows() != l.data.features.len() {
            // The layer's data changed since the index was built; its rows would be wrong.
            log::warn!("hit index for layer {} is stale; skipping it", l.id);
            continue;
        }
        // The click where it is first, then in the world copies (each object once).
        let start = out.len();
        for k in [0.0, -1.0, 1.0, -2.0, 2.0] {
            let p = Coord::new(at.x + k * WORLD_WIDTH, at.y);
            for h in l.index.query(l.data, &l.symbol, p, mpp) {
                let hit = Hit { layer: l.id, row: h.row };
                if !out[start..].contains(&hit) {
                    out.push(hit);
                }
            }
        }
    }
    out
}

/// The width of the Web Mercator world, metres (the distance between world copies).
const WORLD_WIDTH: f64 = 2.0 * std::f64::consts::PI * rustgis_geom::MERCATOR_R;

/// [`hit_test_merc`] for a click in longitude/latitude (as `MapOutput::clicked` gives it).
/// The click is also tested in the neighbouring world copies ([`hit_test_merc`]).
pub fn hit_test(layers: &[HitLayer<'_>], click: Coord, mpp: f64) -> Vec<Hit> {
    hit_test_merc(layers, lonlat_to_merc(click), mpp)
}

/// The band values of a raster layer's pixel under `lonlat`: one per band, `None` for a band
/// that is NoData there. `None` when the raster isn't loaded, the point is outside it, or every
/// band is NoData (nothing to identify). A click in a world copy (longitude past ±180°) reads
/// the cell it is drawn over: the longitude is wrapped into −180 to 180°, then tried a world west
/// and east, for a grid whose first column straddles the antimeridian or that is laid out over
/// 0 to 360°.
pub fn raster_values(layer: &RasterLayer, lonlat: Coord) -> Option<Vec<Option<f32>>> {
    let r = layer.data.as_ref()?;
    let lon = (lonlat.x + 180.0).rem_euclid(360.0) - 180.0;
    let (col, row) = [lon, lon - 360.0, lon + 360.0].into_iter().find_map(|x| r.pixel_at(Coord::new(x, lonlat.y)))?;
    let vals: Vec<Option<f32>> = (0..r.band_count()).map(|b| r.value(b, col, row)).collect();
    if vals.iter().all(Option::is_none) { None } else { Some(vals) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_render::rustgis_data::GeometryType;

    /// Mercator metres per pixel used by the tests (about zoom 12).
    const MPP: f64 = 38.0;

    fn ds(geoms: Vec<Geometry>) -> Dataset {
        let kind = geoms.first().map_or(GeometryType::Point, Geometry::kind);
        let mut d = Dataset::new("t", kind, vec![]);
        for g in geoms {
            d.push(Some(g), vec![]);
        }
        d
    }

    /// The lon/lat `px` pixels east and `py` pixels north of `base` (Mercator plane).
    fn offset(base: Coord, px: f64, py: f64) -> Coord {
        let m = lonlat_to_merc(base);
        rustgis_geom::merc_to_lonlat(Coord::new(m.x + px * MPP, m.y + py * MPP))
    }

    fn rows(index: &HitIndex, d: &Dataset, sym: HitSymbol, click: Coord) -> Vec<usize> {
        index.query(d, &sym, lonlat_to_merc(click), MPP).into_iter().map(|h| h.row).collect()
    }

    const BASE: Coord = Coord { x: 121.47, y: 31.23 };

    #[test]
    fn points_hit_within_radius_plus_tolerance_nearest_first() {
        let d = ds(vec![Geometry::Point(BASE), Geometry::Point(offset(BASE, 5.0, 0.0)), Geometry::Point(offset(BASE, 30.0, 0.0))]);
        let idx = HitIndex::build(&d);
        let sym = HitSymbol { point_size: 7.0, line_width: 2.0 };
        // 7 px points: radius 3.5 + 4 = 7.5 px.
        assert_eq!(rows(&idx, &d, sym, offset(BASE, 1.0, 0.0)), vec![0, 1], "nearest first");
        assert_eq!(rows(&idx, &d, sym, offset(BASE, 4.0, 0.0)), vec![1, 0]);
        assert_eq!(rows(&idx, &d, sym, offset(BASE, 0.0, 7.4)), vec![0]);
        assert!(rows(&idx, &d, sym, offset(BASE, 0.0, 7.6)).is_empty());
        assert_eq!(rows(&idx, &d, sym, offset(BASE, 37.0, 0.0)), vec![2]);
        assert!(rows(&idx, &d, sym, offset(BASE, 38.0, 0.0)).is_empty());
    }

    #[test]
    fn dense_points_get_six_pixels() {
        let d = ds(vec![Geometry::Point(BASE)]);
        let idx = HitIndex::build(&d);
        let dense = HitSymbol { point_size: 4.0, line_width: 2.0 };
        assert_eq!(dense.point_tolerance_px(), 6.0);
        // 4 px points: radius 2 + 6 = 8 px.
        assert_eq!(rows(&idx, &d, dense, offset(BASE, 7.9, 0.0)), vec![0]);
        assert!(rows(&idx, &d, dense, offset(BASE, 8.1, 0.0)).is_empty());
        // At 5 px the normal tolerance applies: 2.5 + 4 = 6.5 px.
        let five = HitSymbol { point_size: 5.0, line_width: 2.0 };
        assert!(rows(&idx, &d, five, offset(BASE, 6.6, 0.0)).is_empty());
        assert_eq!(rows(&idx, &d, five, offset(BASE, 6.4, 0.0)), vec![0]);
    }

    #[test]
    fn multipoints_hit_on_any_member() {
        let d = ds(vec![Geometry::MultiPoint(vec![BASE, offset(BASE, 100.0, 0.0)])]);
        let idx = HitIndex::build(&d);
        assert_eq!(rows(&idx, &d, HitSymbol::default(), offset(BASE, 102.0, 0.0)), vec![0]);
        assert!(rows(&idx, &d, HitSymbol::default(), offset(BASE, 50.0, 0.0)).is_empty());
    }

    #[test]
    fn lines_hit_within_half_width_plus_tolerance_of_a_segment() {
        // An L: east 100 px, then north 100 px.
        let line = vec![BASE, offset(BASE, 100.0, 0.0), offset(BASE, 100.0, 100.0)];
        let d = ds(vec![Geometry::Line(vec![line])]);
        let idx = HitIndex::build(&d);
        let sym = HitSymbol { point_size: 7.0, line_width: 2.0 };
        // 2 px line: 1 + 4 = 5 px from the segment.
        assert_eq!(rows(&idx, &d, sym, offset(BASE, 50.0, 4.9)), vec![0]);
        assert!(rows(&idx, &d, sym, offset(BASE, 50.0, 5.1)).is_empty());
        assert_eq!(rows(&idx, &d, sym, offset(BASE, 95.1, 50.0)), vec![0], "second segment");
        // Inside the L's bounding box but far from both segments.
        assert!(rows(&idx, &d, sym, offset(BASE, 50.0, 50.0)).is_empty());
        // Beyond the end: distance to the end point.
        assert_eq!(rows(&idx, &d, sym, offset(BASE, -3.0, -3.0)), vec![0]);
        // A wider line reaches farther.
        let wide = HitSymbol { point_size: 7.0, line_width: 6.0 };
        assert_eq!(rows(&idx, &d, wide, offset(BASE, 50.0, 6.9)), vec![0]);
    }

    fn square(c: Coord, half: f64) -> Vec<Coord> {
        vec![offset(c, -half, -half), offset(c, half, -half), offset(c, half, half), offset(c, -half, half), offset(c, -half, -half)]
    }

    #[test]
    fn polygons_hit_inside_not_in_holes_and_near_the_outline() {
        let d = ds(vec![Geometry::Polygon(vec![vec![square(BASE, 100.0), square(BASE, 30.0)]])]);
        let idx = HitIndex::build(&d);
        let sym = HitSymbol::default();
        let hit = |px: f64, py: f64| idx.query(&d, &sym, lonlat_to_merc(offset(BASE, px, py)), MPP);
        assert_eq!(hit(60.0, 0.0).first().map(|h| h.distance_px), Some(0.0), "inside");
        assert!(hit(0.0, 0.0).is_empty(), "in the hole");
        assert_eq!(hit(0.0, 27.0).len(), 1, "in the hole, within 4 px of its edge");
        assert!(hit(0.0, 25.0).is_empty());
        assert_eq!(hit(103.9, 0.0).len(), 1, "outside, within 4 px of the outline");
        assert!(hit(104.1, 0.0).is_empty());
    }

    #[test]
    fn multipolygons_hit_on_any_part() {
        let far = offset(BASE, 500.0, 0.0);
        let d = ds(vec![Geometry::Polygon(vec![vec![square(BASE, 20.0)], vec![square(far, 20.0)]])]);
        let idx = HitIndex::build(&d);
        let sym = HitSymbol::default();
        assert_eq!(rows(&idx, &d, sym, BASE), vec![0]);
        assert_eq!(rows(&idx, &d, sym, far), vec![0]);
        assert!(rows(&idx, &d, sym, offset(BASE, 250.0, 0.0)).is_empty(), "between the parts");
    }

    #[test]
    fn overlapping_polygons_later_rows_first() {
        let d = ds(vec![Geometry::Polygon(vec![vec![square(BASE, 100.0)]]), Geometry::Polygon(vec![vec![square(BASE, 50.0)]])]);
        let idx = HitIndex::build(&d);
        assert_eq!(rows(&idx, &d, HitSymbol::default(), BASE), vec![1, 0], "the one drawn on top first");
    }

    #[test]
    fn layers_are_reported_topmost_first() {
        let areas = ds(vec![Geometry::Polygon(vec![vec![square(BASE, 100.0)]])]);
        let points = ds(vec![Geometry::Point(offset(BASE, 2.0, 0.0)), Geometry::Point(BASE)]);
        let lines = ds(vec![Geometry::Line(vec![vec![offset(BASE, -50.0, 1.0), offset(BASE, 50.0, 1.0)]])]);
        let (ia, ip, il) = (HitIndex::build(&areas), HitIndex::build(&points), HitIndex::build(&lines));
        let layers = [
            HitLayer { id: 7, index: &ip, data: &points, symbol: HitSymbol::default() },
            HitLayer { id: 3, index: &il, data: &lines, symbol: HitSymbol::default() },
            HitLayer { id: 9, index: &ia, data: &areas, symbol: HitSymbol::default() },
        ];
        let hits = hit_test(&layers, BASE, MPP);
        let want = [Hit { layer: 7, row: 1 }, Hit { layer: 7, row: 0 }, Hit { layer: 3, row: 0 }, Hit { layer: 9, row: 0 }];
        assert_eq!(hits, want);
        // Far from everything: no hits.
        assert!(hit_test(&layers, offset(BASE, 400.0, 0.0), MPP).is_empty());
        // A stale index (the data grew) is skipped, not misread.
        let mut grown = points.clone();
        grown.push(Some(Geometry::Point(BASE)), vec![]);
        let stale = [HitLayer { id: 7, index: &ip, data: &grown, symbol: HitSymbol::default() }];
        assert!(hit_test(&stale, BASE, MPP).is_empty());
    }

    #[test]
    fn a_crowded_click_keeps_the_nearest_hits() {
        // 400 points in a line, 0.01 px apart: all within reach, only the nearest 100 kept.
        let d = ds((0..400).map(|i| Geometry::Point(offset(BASE, f64::from(i) * 0.01, 0.0))).collect());
        let idx = HitIndex::build(&d);
        let hits = idx.query(&d, &HitSymbol::default(), lonlat_to_merc(BASE), MPP);
        assert_eq!(hits.len(), MAX_HITS_PER_LAYER);
        assert_eq!(hits.iter().map(|h| h.row).collect::<Vec<_>>(), (0..100).collect::<Vec<_>>());
        // The same answer the full scan gives, truncated.
        let all = idx.query_limited(&d, &HitSymbol::default(), lonlat_to_merc(BASE), MPP, usize::MAX);
        assert_eq!(all.len(), 400);
        assert_eq!(&all[..100], &hits[..]);
    }

    #[test]
    fn a_track_past_the_antimeridian_is_hit_in_the_world_copy_it_is_drawn_in() {
        // An IBTrACS-style track stored unwrapped, 175°E to 183°E (= 177°W).
        let d = ds(vec![Geometry::Line(vec![vec![Coord::new(175.0, 20.0), Coord::new(183.0, 21.0)]])]);
        let idx = HitIndex::build(&d);
        let layers = [HitLayer { id: 0, index: &idx, data: &d, symbol: HitSymbol::default() }];
        let mpp = 2000.0;
        // Clicked east of 180° in the main world, and in the western copy (at 178°W).
        assert_eq!(hit_test(&layers, Coord::new(182.0, 20.875), mpp).len(), 1);
        assert_eq!(hit_test(&layers, Coord::new(-178.0, 20.875), mpp), vec![Hit { layer: 0, row: 0 }]);
        assert!(hit_test(&layers, Coord::new(-150.0, 20.875), mpp).is_empty());
    }

    #[test]
    fn the_antimeridian_neither_crashes_nor_hits_across_the_world() {
        let east = Coord::new(179.99, 0.0);
        let d = ds(vec![
            Geometry::Point(east),
            // A line drawn across the whole map (as the renderer draws it, from −179.9 to 179.9).
            Geometry::Line(vec![vec![Coord::new(-179.9, 10.0), Coord::new(179.9, 10.0)]]),
            Geometry::Point(Coord::new(f64::NAN, 0.0)),
        ]);
        let idx = HitIndex::build(&d);
        assert_eq!(idx.len(), 2, "unplaceable geometry is left out");
        let mpp = 2000.0;
        // A click just past 180° (over the empty side of the map) doesn't hit the point at −180 + ε
        // on the far side, and a click on the point does.
        assert_eq!(
            hit_test_merc(&[HitLayer { id: 0, index: &idx, data: &d, symbol: HitSymbol::default() }], lonlat_to_merc(east), mpp)
                .iter()
                .map(|h| h.row)
                .collect::<Vec<_>>(),
            vec![0]
        );
        let wrapped = Coord::new(-180.0 + 0.01, 0.0);
        assert!(idx.query(&d, &HitSymbol::default(), lonlat_to_merc(wrapped), mpp).is_empty());
        // Clicks far outside the world, and degenerate scales, are harmless.
        for (at, mpp) in [
            (Coord::new(1e9, 0.0), 10.0),
            (Coord::new(0.0, 0.0), 0.0),
            (Coord::new(f64::NAN, 0.0), 10.0),
            (Coord::new(0.0, 0.0), f64::INFINITY),
        ] {
            assert!(idx.query(&d, &HitSymbol::default(), at, mpp).is_empty());
        }
        // Mid-line, the long line hits.
        assert_eq!(rows(&idx, &d, HitSymbol::default(), Coord::new(0.0, 10.0)), vec![1]);
    }

    #[test]
    fn raster_values_are_none_outside_and_on_nodata() {
        use rustgis_render::rustgis_data::raster::{ColorHint, Crs, GeoTransform, Raster};
        // 4×3 lon/lat pixels of 0.5°, from 10°E 50°N; pixel (1, 1) is NoData.
        let gt = GeoTransform { origin_x: 10.0, origin_y: 50.0, pixel_width: 0.5, pixel_height: -0.5 };
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, -9999.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let r = Raster::new("small", 4, 3, vec![data], gt, Crs::LonLat, Some(-9999.0), ColorHint::Gray, "Float32").unwrap();
        let mut layer = RasterLayer::from_path(r, "small.tif");
        assert_eq!(raster_values(&layer, Coord::new(10.25, 49.75)), Some(vec![Some(1.0)]));
        assert_eq!(raster_values(&layer, Coord::new(11.75, 48.75)), Some(vec![Some(12.0)]));
        assert_eq!(raster_values(&layer, Coord::new(10.75, 49.25)), None, "NoData");
        assert_eq!(raster_values(&layer, Coord::new(9.0, 49.0)), None, "outside");
        // A click in a world copy east or west reads the cell it is drawn over.
        for k in [-2.0, -1.0, 1.0, 2.0] {
            assert_eq!(raster_values(&layer, Coord::new(10.25 + k * 360.0, 49.75)), Some(vec![Some(1.0)]), "{k} worlds");
        }
        layer.data = None;
        assert_eq!(raster_values(&layer, Coord::new(10.25, 49.75)), None, "not loaded");
    }

    /// A grid whose first column straddles the antimeridian (from 191.25°W, as a 0–360° NetCDF is
    /// shown) answers on both sides of 180°, and in the world copies.
    #[test]
    fn raster_values_across_the_antimeridian() {
        use rustgis_render::rustgis_data::raster::{ColorHint, Crs, GeoTransform, Raster};
        let gt = GeoTransform { origin_x: -191.25, origin_y: 90.0, pixel_width: 22.5, pixel_height: -22.5 };
        let data: Vec<f32> = (0..16 * 8).map(|i| (i % 16) as f32).collect();
        let r = Raster::new("lon360", 16, 8, vec![data], gt, Crs::LonLat, None, ColorHint::Gray, "Float32").unwrap();
        let layer = RasterLayer::from_path(r, "lon360.nc");
        for lon in [175.0, -175.0, -185.0, 535.0, -545.0] {
            assert_eq!(raster_values(&layer, Coord::new(lon, 10.0)), Some(vec![Some(0.0)]), "{lon}");
        }
        assert_eq!(raster_values(&layer, Coord::new(160.0, 10.0)), Some(vec![Some(15.0)]));
        assert_eq!(raster_values(&layer, Coord::new(160.0 - 360.0, 10.0)), Some(vec![Some(15.0)]));
    }

    /// `cargo test -p lm-mapview --release -- --ignored --nocapture hit_index_timing`
    #[test]
    #[ignore]
    fn hit_index_timing() {
        use std::time::Instant;
        // 100,000 small polygons (a 316×316 grid of 5-vertex squares, about 300 m) over a city.
        let mut polys = Vec::new();
        for i in 0..100_000 {
            let (x, y) = ((i % 316) as f64, (i / 316) as f64);
            let c = Coord::new(116.0 + x * 0.003, 39.5 + y * 0.003);
            let h = 0.0013;
            polys.push(Geometry::Polygon(vec![vec![vec![
                Coord::new(c.x - h, c.y - h),
                Coord::new(c.x + h, c.y - h),
                Coord::new(c.x + h, c.y + h),
                Coord::new(c.x - h, c.y + h),
                Coord::new(c.x - h, c.y - h),
            ]]]));
        }
        let polys = ds(polys);
        // 1,000,000 points scattered over the world (a cheap hash, so it is reproducible).
        let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        let pts = ds((0..1_000_000).map(|_| Geometry::Point(Coord::new(next() * 360.0 - 180.0, next() * 170.0 - 85.0))).collect());
        let t = Instant::now();
        let ip = HitIndex::build(&polys);
        let bp = t.elapsed();
        let t = Instant::now();
        let ipt = HitIndex::build(&pts);
        let bpt = t.elapsed();
        println!("build: 100k polygons {:.1} ms, 1M points {:.1} ms", bp.as_secs_f64() * 1e3, bpt.as_secs_f64() * 1e3);
        println!("index memory ~{} bytes/feature (node size)", std::mem::size_of::<rstar::RTreeNode<Entry>>());
        let layers = [
            HitLayer { id: 1, index: &ipt, data: &pts, symbol: HitSymbol { point_size: 4.0, line_width: 2.0 } },
            HitLayer { id: 2, index: &ip, data: &polys, symbol: HitSymbol::default() },
        ];
        // Queries at a city zoom (12), a country zoom (6) and the whole world (zoom 1).
        for (zoom, n) in [(12.0, 2000), (6.0, 2000), (1.0, 200)] {
            let mpp = crate::camera::res_of(zoom);
            let mut worst = 0.0f64;
            let mut total_hits = 0;
            let t = Instant::now();
            for _ in 0..n {
                let c = Coord::new(116.0 + next() * 0.95, 39.5 + next() * 0.95);
                let q = Instant::now();
                total_hits += hit_test(&layers, c, mpp).len();
                worst = worst.max(q.elapsed().as_secs_f64());
            }
            let mean = t.elapsed().as_secs_f64() / f64::from(n);
            println!(
                "zoom {zoom}: mean {:.3} ms, worst {:.3} ms, {:.1} hits per click",
                mean * 1e3,
                worst * 1e3,
                total_hits as f64 / f64::from(n)
            );
            assert!(worst < 0.010, "a query must take well under 10 ms");
        }
        drop((ip, ipt, polys, pts));
        // The V6 stress size: 1,346,007 small polygons (a 1161-wide grid over a country).
        let big: Vec<Geometry> = (0..1_346_007)
            .map(|i| {
                let (x, y) = ((i % 1161) as f64, (i / 1161) as f64);
                let c = Coord::new(100.0 + x * 0.02, 20.0 + y * 0.02);
                let h = 0.008;
                Geometry::Polygon(vec![vec![vec![
                    Coord::new(c.x - h, c.y - h),
                    Coord::new(c.x + h, c.y - h),
                    Coord::new(c.x + h, c.y + h),
                    Coord::new(c.x - h, c.y + h),
                    Coord::new(c.x - h, c.y - h),
                ]]])
            })
            .collect();
        let big = ds(big);
        let t = Instant::now();
        let ib = HitIndex::build(&big);
        println!("build: 1,346,007 polygons {:.1} ms ({} indexed)", t.elapsed().as_secs_f64() * 1e3, ib.len());
        let layers = [HitLayer { id: 3, index: &ib, data: &big, symbol: HitSymbol::default() }];
        for zoom in [12.0, 6.0, 1.0] {
            let mpp = crate::camera::res_of(zoom);
            let mut worst = 0.0f64;
            for _ in 0..500 {
                let c = Coord::new(100.0 + next() * 23.0, 20.0 + next() * 23.0);
                let q = Instant::now();
                let _ = hit_test(&layers, c, mpp);
                worst = worst.max(q.elapsed().as_secs_f64());
            }
            println!("1.35M polygons, zoom {zoom}: worst {:.3} ms", worst * 1e3);
            assert!(worst < 0.010);
        }
    }
}
