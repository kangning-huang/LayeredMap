//! Point and area overlay, the certified actions of WP4.5 (spec › WP4.5 Certified actions):
//! "Count [points] in each [area]", "Which [area] is it in?" and "What's inside?"; and A6
//! "Add [area] details to each [feature]" ([`join_by_location`], `docs/lm/puzzles/mechanics.md`
//! › A6).
//!
//! # Definition
//!
//! Everything is planar in WGS 84 longitude/latitude, as GeoPandas `sjoin(predicate="intersects")`
//! is, the reference the fixtures were made with (`reference/count_in_areas.py`).
//!
//! - **Boundary.** A point on an area's boundary *intersects* it: it counts in every area that
//!   touches it, and "Which area" lists them all. This includes a hole's ring: the hole's edge
//!   belongs to the area.
//! - **Holes.** A point strictly inside a hole is outside that area.
//! - **Multipolygons.** An area intersects a point when any of its polygons does; a point touching
//!   two polygons of the same area counts once in it.
//! - **Overlap.** Overlapping areas each count their points. [`CountInAreas::counted_more_than_once`]
//!   is the number of distinct points that fell in two or more areas.
//! - **Zero.** Every area row stays in the output, with 0 when nothing is in it.
//! - **Missing geometry.** Point rows with no geometry, an empty one, a non-point one or a
//!   non-finite coordinate are excluded and counted. Area rows with no geometry, an empty one, a
//!   non-polygon one or a non-finite coordinate get 0 and are listed.
//! - **Multipoints.** A multipoint row intersects an area when any of its points does; it counts
//!   once per area (it is one row).
//! - **Join by location.** Each feature is placed by its point (a multipoint by all its points,
//!   a line or an area by its [`crate::measure::label_point`]) and takes the *first* area row,
//!   in row order, that the point touches (inside or on the boundary), the GeoPandas
//!   `sjoin(predicate="intersects")` match with the lowest area index. Features whose point lies
//!   on the boundary of any area are counted ([`JoinByLocation::on_boundary`]), as are features in
//!   no area ([`JoinByLocation::outside`]).
//!
//! # Exactness
//!
//! "On the boundary" is decided with Shewchuk's exact orientation predicate (`robust::orient2d`):
//! no epsilon, so a point exactly on an edge is on it and one a float's width off is not. The
//! inside test is a crossing count with the half-open rule, again on exact orientations, so the
//! answer for every input point is the exact answer for the input coordinates.
//!
//! # Speed
//!
//! The polygons of the area layer go into an R-tree of bounding boxes; each point visits only the
//! polygons whose box holds it. Work is checked for cancel at least every 2^18 edge tests (well
//! under 50 ms) and progress is reported as points done of points total.

use crate::control::{Cancelled, Control};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};
use rustgis_data::{Coord, Dataset, Extent, Geometry};

/// Where a point lies relative to an area (or a ring).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Inside,
    Boundary,
    Outside,
}

impl Location {
    /// Inside or on the boundary: the `intersects` predicate.
    pub fn touches(self) -> bool {
        self != Location::Outside
    }
}

/// The result of "Count [points] in each [area]".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CountInAreas {
    /// One count per area row, in row order; zero-count areas included.
    pub counts: Vec<u64>,
    /// Distinct point rows counted in two or more areas (for the notice).
    pub counted_more_than_once: u64,
    /// Point rows with a geometry that fell in no area.
    pub outside_all: u64,
    /// Point rows excluded for having no usable point geometry.
    pub points_without_geometry: u64,
    /// Area rows without a usable polygon geometry (each counted 0).
    pub areas_without_geometry: Vec<usize>,
}

/// The result of "Which [area] is it in?".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WhichArea {
    /// For each point row, the area rows it is in or on, ascending; empty when none.
    pub areas: Vec<Vec<usize>>,
    /// Point rows excluded for having no usable point geometry (their `areas` entry is empty).
    pub points_without_geometry: Vec<usize>,
    /// Area rows without a usable polygon geometry.
    pub areas_without_geometry: Vec<usize>,
}

/// The result of "What's inside?".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WhatsInside {
    /// The rows that intersect the area, ascending.
    pub rows: Vec<usize>,
    /// Rows without a usable geometry, which were not tested.
    pub without_geometry: Vec<usize>,
}

/// The result of "Add [area] details to each [feature]".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JoinByLocation {
    /// For each feature row, the first area row containing its point (boundary counts); `None`
    /// when it is in no area or has no usable geometry.
    pub area_of: Vec<Option<usize>>,
    /// Features whose point lies on the boundary of at least one area.
    pub on_boundary: u64,
    /// Features with a usable geometry that fell in no area.
    pub outside: u64,
    /// Feature rows without a usable geometry (or without a label point).
    pub without_geometry: Vec<usize>,
    /// Area rows without a usable polygon geometry (never matched).
    pub areas_without_geometry: Vec<usize>,
}

const PHASE_JOIN: &str = "Finding the area of each feature";
const PHASE_COUNT: &str = "Counting points in areas";
const PHASE_COUNT_LINES: &str = "Counting lines in areas";
const PHASE_WHICH: &str = "Finding the area of each point";
const PHASE_INSIDE: &str = "Finding what's inside the area";
const PHASE_INDEX: &str = "Indexing areas";

/// Counts the lines of `lines` that pass through or touch each area of `areas`: a line counts
/// once in every area it shares at least one point with (a vertex inside or on the area, or a
/// segment crossing its boundary), however many times it enters. `counted_more_than_once` is
/// the number of lines in two or more areas, `points_without_geometry` the lines without a
/// usable geometry.
pub fn count_lines_in_areas(lines: &Dataset, areas: &Dataset, ctl: &Control) -> Result<CountInAreas, Cancelled> {
    let index = AreaIndex::build(areas, ctl)?;
    let mut out = CountInAreas {
        counts: vec![0; areas.features.len()],
        areas_without_geometry: index.without_geometry.clone(),
        ..CountInAreas::default()
    };
    index.for_each_line(lines, ctl, |hits| match hits {
        None => out.points_without_geometry += 1,
        Some([]) => out.outside_all += 1,
        Some(hits) => {
            if hits.len() > 1 {
                out.counted_more_than_once += 1;
            }
            for &a in hits {
                if let Some(c) = out.counts.get_mut(a) {
                    *c += 1;
                }
            }
        }
    })?;
    Ok(out)
}

/// Whether segment `a-b` (a point when equal) shares at least one point with a polygon (its
/// rings), exactly: an end inside or on it, or a crossing of one of its edges.
fn segment_touches_polygon(a: Coord, b: Coord, rings: &[Vec<Coord>]) -> bool {
    locate_in_polygon(rings, a).touches()
        || locate_in_polygon(rings, b).touches()
        || rings.iter().any(|ring| edges(ring).any(|(c, d)| segments_intersect(a, b, c, d)))
}

/// Counts the points of `points` in each area of `areas` (see the module rules).
pub fn count_points_in_areas(points: &Dataset, areas: &Dataset, ctl: &Control) -> Result<CountInAreas, Cancelled> {
    let index = AreaIndex::build(areas, ctl)?;
    let mut out = CountInAreas {
        counts: vec![0; areas.features.len()],
        areas_without_geometry: index.without_geometry.clone(),
        ..CountInAreas::default()
    };
    index.for_each_point(points, ctl, PHASE_COUNT, |_, hits| match hits {
        None => out.points_without_geometry += 1,
        Some([]) => out.outside_all += 1,
        Some(hits) => {
            if hits.len() > 1 {
                out.counted_more_than_once += 1;
            }
            for &a in hits {
                if let Some(c) = out.counts.get_mut(a) {
                    *c += 1;
                }
            }
        }
    })?;
    Ok(out)
}

/// Lists, for each point of `points`, every area of `areas` it is in or on (see the module rules).
pub fn which_area(points: &Dataset, areas: &Dataset, ctl: &Control) -> Result<WhichArea, Cancelled> {
    let index = AreaIndex::build(areas, ctl)?;
    let mut out = WhichArea {
        areas: Vec::with_capacity(points.features.len()),
        areas_without_geometry: index.without_geometry.clone(),
        ..WhichArea::default()
    };
    index.for_each_point(points, ctl, PHASE_WHICH, |row, hits| match hits {
        None => {
            out.points_without_geometry.push(row);
            out.areas.push(Vec::new());
        }
        Some(hits) => out.areas.push(hits.to_vec()),
    })?;
    Ok(out)
}

/// For each feature of `features` (points; lines/areas by their label point), the first area row
/// that contains it (boundary counts), else None (see the module rules).
pub fn join_by_location(features: &Dataset, areas: &Dataset, ctl: &Control) -> Result<JoinByLocation, Cancelled> {
    let index = AreaIndex::build(areas, ctl)?;
    let total = features.features.len() as u64;
    let mut out = JoinByLocation {
        area_of: Vec::with_capacity(features.features.len()),
        areas_without_geometry: index.without_geometry.clone(),
        ..JoinByLocation::default()
    };
    let mut work = Work::new(ctl);
    let mut probes: Vec<Coord> = Vec::new();
    for (row, feat) in features.features.iter().enumerate() {
        if row % 65_536 == 0 {
            ctl.report(PHASE_JOIN, row as u64, Some(total));
        }
        work.add(32)?;
        probes.clear();
        match feat.geometry.as_ref().filter(|g| usable(g)) {
            Some(Geometry::Point(c)) => probes.push(*c),
            Some(Geometry::MultiPoint(v)) => probes.extend_from_slice(v),
            Some(g) => probes.extend(crate::measure::label_point(g)),
            None => {}
        }
        if probes.is_empty() {
            out.without_geometry.push(row);
            out.area_of.push(None);
            continue;
        }
        let mut first: Option<usize> = None;
        let mut boundary = false;
        for &p in &probes {
            for e in index.tree.locate_in_envelope_intersecting(&AABB::from_point([p.x, p.y])) {
                let Some(part) = index.parts.get(e.data) else { continue };
                if first.is_some_and(|a| a <= part.area) && boundary {
                    continue;
                }
                work.add(polygon_size(part.rings))?;
                match locate_in_polygon(part.rings, p) {
                    Location::Boundary => {
                        boundary = true;
                        first = Some(first.map_or(part.area, |a| a.min(part.area)));
                    }
                    Location::Inside => first = Some(first.map_or(part.area, |a| a.min(part.area))),
                    Location::Outside => {}
                }
            }
        }
        if boundary {
            out.on_boundary += 1;
        }
        if first.is_none() {
            out.outside += 1;
        }
        out.area_of.push(first);
    }
    ctl.report(PHASE_JOIN, total, Some(total));
    Ok(out)
}

/// The rows of `layer` that intersect the polygon `area`: points inside or on its boundary, lines
/// (and polygons) that cross, touch or lie in it. A non-polygon or empty `area` contains nothing.
pub fn whats_inside(area: &Geometry, layer: &Dataset, ctl: &Control) -> Result<WhatsInside, Cancelled> {
    let mut out = WhatsInside::default();
    let polys = match area {
        Geometry::Polygon(p) if usable_polygon(area) => p.as_slice(),
        _ => &[],
    };
    let bbox = area.extent();
    let total = layer.features.len() as u64;
    let mut work = Work::new(ctl);
    for (row, f) in layer.features.iter().enumerate() {
        if row % 65_536 == 0 {
            ctl.report(PHASE_INSIDE, row as u64, Some(total));
        }
        work.add(16)?;
        let Some(g) = f.geometry.as_ref().filter(|g| usable(g)) else {
            out.without_geometry.push(row);
            continue;
        };
        if polys.is_empty() || !bbox.intersects(&g.extent()) {
            continue;
        }
        if geometry_intersects(polys, g, &mut work)? {
            out.rows.push(row);
        }
    }
    ctl.report(PHASE_INSIDE, total, Some(total));
    Ok(out)
}

/// Where `p` lies relative to one polygon (`rings[0]` the exterior, the rest holes).
pub fn locate_in_polygon(rings: &[Vec<Coord>], p: Coord) -> Location {
    let mut it = rings.iter();
    let Some(outer) = it.next() else { return Location::Outside };
    match locate_in_ring(outer, p) {
        Location::Inside => {}
        other => return other,
    }
    let mut in_hole = false;
    for hole in it {
        match locate_in_ring(hole, p) {
            Location::Boundary => return Location::Boundary,
            Location::Inside => in_hole = true,
            Location::Outside => {}
        }
    }
    if in_hole { Location::Outside } else { Location::Inside }
}

/// Where `p` lies relative to an area of one or more polygons: on the boundary of any of them
/// beats inside another (both touch).
pub fn locate_in_area(polygons: &[Vec<Vec<Coord>>], p: Coord) -> Location {
    let mut best = Location::Outside;
    for rings in polygons {
        match locate_in_polygon(rings, p) {
            Location::Boundary => return Location::Boundary,
            Location::Inside => best = Location::Inside,
            Location::Outside => {}
        }
    }
    best
}

/// Where `p` lies relative to a ring (closed or not; the closing edge is implied), exactly.
pub fn locate_in_ring(ring: &[Coord], p: Coord) -> Location {
    let mut inside = false;
    for (a, b) in edges(ring) {
        match crossing(a, b, p) {
            Crossing::OnEdge => return Location::Boundary,
            Crossing::Crosses => inside = !inside,
            Crossing::No => {}
        }
    }
    if inside { Location::Inside } else { Location::Outside }
}

/// Whether segments `a-b` and `c-d` share at least one point (touching and overlap included), exactly.
pub fn segments_intersect(a: Coord, b: Coord, c: Coord, d: Coord) -> bool {
    let d1 = orient(c, d, a);
    let d2 = orient(c, d, b);
    let d3 = orient(a, b, c);
    let d4 = orient(a, b, d);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    (d1 == 0.0 && in_box(c, d, a)) || (d2 == 0.0 && in_box(c, d, b)) || (d3 == 0.0 && in_box(a, b, c)) || (d4 == 0.0 && in_box(a, b, d))
}

// ---------------------------------------------------------------------------------------------

enum Crossing {
    OnEdge,
    Crosses,
    No,
}

/// Exact orientation: > 0 when `c` is left of `a→b`, < 0 right, 0 collinear.
fn orient(a: Coord, b: Coord, c: Coord) -> f64 {
    robust::orient2d(robust::Coord { x: a.x, y: a.y }, robust::Coord { x: b.x, y: b.y }, robust::Coord { x: c.x, y: c.y })
}

/// Whether `p` is within the closed bounding box of `a` and `b` (given `p` is collinear with them,
/// that means on the segment).
fn in_box(a: Coord, b: Coord, p: Coord) -> bool {
    a.x.min(b.x) <= p.x && p.x <= a.x.max(b.x) && a.y.min(b.y) <= p.y && p.y <= a.y.max(b.y)
}

/// Whether `p` is on edge `a-b`, or the edge crosses the ray from `p` towards +x (half-open in y,
/// so a vertex on the ray is counted once).
fn crossing(a: Coord, b: Coord, p: Coord) -> Crossing {
    // Cheap rejects first: the edge is wholly above, below or left of p.
    if (a.y > p.y && b.y > p.y) || (a.y < p.y && b.y < p.y) || (a.x < p.x && b.x < p.x) {
        return Crossing::No;
    }
    let o = orient(a, b, p);
    if o == 0.0 && in_box(a, b, p) {
        return Crossing::OnEdge;
    }
    let up = a.y <= p.y && b.y > p.y;
    let down = b.y <= p.y && a.y > p.y;
    if (up && o > 0.0) || (down && o < 0.0) { Crossing::Crosses } else { Crossing::No }
}

/// The edges of a ring, closing it if its last vertex isn't its first.
fn edges(ring: &[Coord]) -> impl Iterator<Item = (Coord, Coord)> + '_ {
    let next = ring.iter().skip(1).chain(ring.first());
    ring.iter().zip(next).map(|(a, b)| (*a, *b))
}

fn finite(g: &Geometry) -> bool {
    let mut ok = true;
    g.for_each_coord(|c| ok &= c.is_finite());
    ok
}

/// A geometry with at least one coordinate, all finite.
pub(crate) fn usable(g: &Geometry) -> bool {
    g.coord_count() > 0 && finite(g)
}

fn usable_polygon(g: &Geometry) -> bool {
    matches!(g, Geometry::Polygon(p) if p.iter().any(|r| r.first().is_some_and(|o| !o.is_empty()))) && finite(g)
}

/// Counts edge tests and checks for cancel every 2^18 of them.
pub(crate) struct Work<'a> {
    ctl: &'a Control,
    n: u64,
}

impl<'a> Work<'a> {
    pub(crate) fn new(ctl: &'a Control) -> Self {
        Work { ctl, n: 0 }
    }
    pub(crate) fn add(&mut self, n: usize) -> Result<(), Cancelled> {
        self.n += n as u64;
        if self.n >= 1 << 18 {
            self.n = 0;
            self.ctl.check()?;
        }
        Ok(())
    }
}

fn polygon_size(rings: &[Vec<Coord>]) -> usize {
    rings.iter().map(Vec::len).sum()
}

/// One polygon of an area row.
struct Part<'a> {
    area: usize,
    rings: &'a [Vec<Coord>],
}

/// The polygons of an area layer in an R-tree of their bounding boxes.
struct AreaIndex<'a> {
    parts: Vec<Part<'a>>,
    tree: RTree<GeomWithData<Rectangle<[f64; 2]>, usize>>,
    without_geometry: Vec<usize>,
}

impl<'a> AreaIndex<'a> {
    fn build(areas: &'a Dataset, ctl: &Control) -> Result<Self, Cancelled> {
        let mut parts = Vec::new();
        let mut boxes = Vec::new();
        let mut without_geometry = Vec::new();
        let mut work = Work::new(ctl);
        ctl.report(PHASE_INDEX, 0, Some(areas.features.len() as u64));
        for (row, f) in areas.features.iter().enumerate() {
            let polys = match f.geometry.as_ref() {
                Some(g @ Geometry::Polygon(p)) if usable_polygon(g) => p,
                _ => {
                    without_geometry.push(row);
                    continue;
                }
            };
            for rings in polys {
                let Some(outer) = rings.first().filter(|o| !o.is_empty()) else { continue };
                work.add(outer.len())?;
                let mut e = Extent::EMPTY;
                outer.iter().for_each(|c| e.expand(*c));
                boxes.push(GeomWithData::new(Rectangle::from_corners([e.min_x, e.min_y], [e.max_x, e.max_y]), parts.len()));
                parts.push(Part { area: row, rings });
            }
        }
        ctl.check()?;
        let tree = RTree::bulk_load(boxes);
        ctl.check()?;
        Ok(AreaIndex { parts, tree, without_geometry })
    }

    /// Calls `f(hits)` for each line row in order: `None` for an unusable geometry, else the
    /// ascending, distinct area rows the line touches.
    fn for_each_line(&self, lines: &Dataset, ctl: &Control, mut f: impl FnMut(Option<&[usize]>)) -> Result<(), Cancelled> {
        let total = lines.features.len() as u64;
        let mut work = Work::new(ctl);
        let mut hits: Vec<usize> = Vec::new();
        for (row, feat) in lines.features.iter().enumerate() {
            if row % 1024 == 0 {
                ctl.report(PHASE_COUNT_LINES, row as u64, Some(total));
            }
            let parts: &[Vec<Coord>] = match feat.geometry.as_ref() {
                Some(Geometry::Line(p)) if p.iter().flatten().all(|c| c.is_finite()) && p.iter().any(|q| !q.is_empty()) => p,
                _ => {
                    f(None);
                    continue;
                }
            };
            work.add(parts.iter().map(Vec::len).sum::<usize>())?;
            hits.clear();
            // Segment by segment, only against the areas whose boxes the segment's box meets:
            // a long track spans a huge box, but each of its segments meets few areas.
            for part in parts {
                let segs: Vec<(Coord, Coord)> =
                    if part.len() == 1 { vec![(part[0], part[0])] } else { part.windows(2).map(|w| (w[0], w[1])).collect() };
                for (a, b) in segs {
                    let envelope = AABB::from_corners([a.x.min(b.x), a.y.min(b.y)], [a.x.max(b.x), a.y.max(b.y)]);
                    for cand in self.tree.locate_in_envelope_intersecting(&envelope) {
                        let Some(area) = self.parts.get(cand.data) else { continue };
                        if hits.contains(&area.area) {
                            continue;
                        }
                        work.add(polygon_size(area.rings))?;
                        if segment_touches_polygon(a, b, area.rings) {
                            hits.push(area.area);
                        }
                    }
                }
            }
            hits.sort_unstable();
            f(Some(&hits));
        }
        ctl.report(PHASE_COUNT_LINES, total, Some(total));
        Ok(())
    }

    /// Calls `f(row, hits)` for each point row in order: `None` for an unusable geometry, else the
    /// ascending, distinct area rows it touches.
    fn for_each_point(
        &self,
        points: &Dataset,
        ctl: &Control,
        phase: &str,
        mut f: impl FnMut(usize, Option<&[usize]>),
    ) -> Result<(), Cancelled> {
        let total = points.features.len() as u64;
        let mut work = Work::new(ctl);
        let mut hits: Vec<usize> = Vec::new();
        for (row, feat) in points.features.iter().enumerate() {
            if row % 65_536 == 0 {
                ctl.report(phase, row as u64, Some(total));
            }
            work.add(32)?;
            let coords: &[Coord] = match feat.geometry.as_ref() {
                Some(Geometry::Point(c)) if c.is_finite() => std::slice::from_ref(c),
                Some(Geometry::MultiPoint(v)) if !v.is_empty() && v.iter().all(|c| c.is_finite()) => v,
                _ => {
                    f(row, None);
                    continue;
                }
            };
            hits.clear();
            for &p in coords {
                for e in self.tree.locate_in_envelope_intersecting(&AABB::from_point([p.x, p.y])) {
                    let Some(part) = self.parts.get(e.data) else { continue };
                    if hits.contains(&part.area) {
                        continue;
                    }
                    work.add(polygon_size(part.rings))?;
                    if locate_in_polygon(part.rings, p).touches() {
                        hits.push(part.area);
                    }
                }
            }
            hits.sort_unstable();
            f(row, Some(&hits));
        }
        ctl.report(phase, total, Some(total));
        Ok(())
    }
}

/// Whether geometry `g` intersects the area `polys` (at least one polygon with an exterior).
fn geometry_intersects(polys: &[Vec<Vec<Coord>>], g: &Geometry, work: &mut Work) -> Result<bool, Cancelled> {
    let area_size: usize = polys.iter().map(|r| polygon_size(r)).sum();
    match g {
        Geometry::Point(p) => {
            work.add(area_size)?;
            Ok(locate_in_area(polys, *p).touches())
        }
        Geometry::MultiPoint(v) => {
            for p in v {
                work.add(area_size)?;
                if locate_in_area(polys, *p).touches() {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Geometry::Line(parts) => {
            for line in parts {
                // A vertex inside or on the area…
                if let Some(p) = line.first() {
                    work.add(area_size)?;
                    if locate_in_area(polys, *p).touches() {
                        return Ok(true);
                    }
                }
                // …or a segment meeting any ring. If neither, the line is wholly outside (or in a hole).
                let segs: Vec<(Coord, Coord)> = line.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect();
                if segments_meet_rings(&segs, polys, work)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Geometry::Polygon(other) => {
            // Some vertex of either inside the other, or boundaries that meet.
            for rings in other {
                if let Some(p) = rings.first().and_then(|o| o.first()) {
                    work.add(area_size)?;
                    if locate_in_area(polys, *p).touches() {
                        return Ok(true);
                    }
                }
            }
            for rings in polys {
                if let Some(p) = rings.first().and_then(|o| o.first()) {
                    work.add(polygon_size(rings))?;
                    if locate_in_area(other, *p).touches() {
                        return Ok(true);
                    }
                }
            }
            let segs: Vec<(Coord, Coord)> = other.iter().flatten().flat_map(|r| edges(r)).collect();
            segments_meet_rings(&segs, polys, work)
        }
    }
}

fn segments_meet_rings(segs: &[(Coord, Coord)], polys: &[Vec<Vec<Coord>>], work: &mut Work) -> Result<bool, Cancelled> {
    for ring in polys.iter().flatten() {
        let mut rb = Extent::EMPTY;
        ring.iter().for_each(|c| rb.expand(*c));
        for &(a, b) in segs {
            if !rb.intersects(&Extent::from_points(a, b)) {
                continue;
            }
            work.add(ring.len())?;
            if edges(ring).any(|(c, d)| segments_intersect(a, b, c, d)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rustgis_data::{Field, FieldType, GeometryType, Value};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn c(x: f64, y: f64) -> Coord {
        Coord::new(x, y)
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Coord> {
        vec![c(x0, y0), c(x1, y0), c(x1, y1), c(x0, y1), c(x0, y0)]
    }

    fn points(cs: &[Coord]) -> Dataset {
        let mut d = Dataset::new("points", GeometryType::Point, vec![]);
        for p in cs {
            d.push_next(Some(Geometry::Point(*p)), vec![]);
        }
        d
    }

    fn areas(polys: Vec<Vec<Vec<Vec<Coord>>>>) -> Dataset {
        let mut d = Dataset::new("areas", GeometryType::Polygon, vec![]);
        for p in polys {
            d.push_next(Some(Geometry::Polygon(p)), vec![]);
        }
        d
    }

    fn lines(ls: Vec<Vec<Vec<Coord>>>) -> Dataset {
        let mut d = Dataset::new("lines", GeometryType::Polyline, vec![]);
        for l in ls {
            d.push_next(Some(Geometry::Line(l)), vec![]);
        }
        d
    }

    /// Storm-track style lines (owner's test: storms crossing circles around cities): a line
    /// counts once in each area it shares a point with, whether or not a vertex lies inside.
    #[test]
    fn lines_count_once_in_every_area_they_touch() {
        // Area 0: [0,10]²; area 1: [20,30]×[0,10]; area 2: [0,10]² with a hole [4,6]².
        let a = areas(vec![
            vec![vec![rect(0.0, 0.0, 10.0, 10.0)]],
            vec![vec![rect(20.0, 0.0, 30.0, 10.0)]],
            vec![vec![rect(40.0, 0.0, 50.0, 10.0), rect(44.0, 4.0, 46.0, 6.0)]],
        ]);
        let l = lines(vec![
            vec![vec![c(-5.0, 5.0), c(35.0, 5.0)]],   // crosses 0 and 1, no vertex inside either
            vec![vec![c(10.0, 12.0), c(10.0, 10.0)]], // touches area 0 at its corner
            vec![vec![c(2.0, 2.0), c(3.0, 3.0)]],     // wholly inside area 0
            vec![vec![c(-5.0, 20.0), c(35.0, 20.0)]], // misses everything
            vec![vec![c(44.5, 4.5), c(45.5, 5.5)]],   // inside the hole of area 2
            vec![vec![c(5.0, -5.0), c(5.0, 15.0), c(6.0, 15.0), c(6.0, -5.0)]], // enters area 0 twice
        ]);
        let r = count_lines_in_areas(&l, &a, &Control::none()).unwrap();
        assert_eq!(r.counts, vec![4, 1, 0]);
        assert_eq!(r.outside_all, 2);
        assert_eq!(r.counted_more_than_once, 1);
    }

    fn fixture(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
    }

    fn read(rel: &str) -> Dataset {
        rustgis_data::io::read_path(&fixture(rel)).unwrap().0
    }

    fn ids(d: &Dataset, field: &str) -> Vec<String> {
        let i = d.field_index(field).unwrap();
        (0..d.features.len()).map(|r| d.value(r, i).to_string()).collect()
    }

    /// Rows of a small CSV as maps from header to value.
    fn csv(rel: &str) -> Vec<BTreeMap<String, String>> {
        let text = std::fs::read_to_string(fixture(rel)).unwrap();
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let head: Vec<String> = lines.next().unwrap().split(',').map(|s| s.trim().to_string()).collect();
        lines.map(|l| head.iter().cloned().zip(l.split(',').map(|s| s.trim().to_string())).collect()).collect()
    }

    fn check_counts(areas_rel: &str, area_field: &str, points_rel: &str, expected_rel: &str) -> CountInAreas {
        let a = read(areas_rel);
        let p = read(points_rel);
        let r = count_points_in_areas(&p, &a, &Control::none()).unwrap();
        let got: BTreeMap<String, u64> = ids(&a, area_field).into_iter().zip(r.counts.iter().copied()).collect();
        let want = csv(expected_rel);
        assert_eq!(got.len(), want.len());
        for row in want {
            assert_eq!(got.get(&row["area_id"]).copied(), Some(row["count"].parse::<u64>().unwrap()), "{expected_rel} {}", row["area_id"]);
        }
        r
    }

    #[test]
    fn n1_counts_match_the_reference() {
        let r = check_counts("N1/areas.geojson", "area_id", "N1/points.geojson", "N1/expected_count_in_areas.csv");
        // P04 P05 P06 (A;B) and P09 P10 P12 P15 (B;C).
        assert_eq!(r.counted_more_than_once, 7);
        assert_eq!(r.outside_all, 2);
        assert_eq!(r.points_without_geometry, 0);
        assert!(r.areas_without_geometry.is_empty());
    }

    #[test]
    fn n1_which_area_matches_the_reference() {
        let a = read("N1/areas.geojson");
        let p = read("N1/points.geojson");
        let r = which_area(&p, &a, &Control::none()).unwrap();
        let area_ids = ids(&a, "area_id");
        let got: BTreeMap<String, String> = ids(&p, "point_id")
            .into_iter()
            .zip(r.areas.iter().map(|v| v.iter().map(|&i| area_ids[i].clone()).collect::<Vec<_>>().join(";")))
            .collect();
        let want = csv("N1/expected_which_area.csv");
        assert_eq!(want.len(), 20);
        for row in want {
            assert_eq!(got[&row["point_id"]], row["area_ids"], "{}", row["point_id"]);
            assert_eq!(r.areas[ids(&p, "point_id").iter().position(|x| *x == row["point_id"]).unwrap()].len().to_string(), row["n_areas"]);
        }
    }

    #[test]
    fn n2_counts_match_the_reference() {
        let r = check_counts("N2/areas.geojson", "area_id", "N2/points.geojson", "N2/expected_count_in_areas.csv");
        // Q03, Q12 in the hole; Q09 in M's gap.
        assert!(r.outside_all >= 3);
    }

    #[test]
    fn n2_hole_and_multipolygon_rules() {
        let a = read("N2/areas.geojson");
        let p = read("N2/points.geojson");
        let r = which_area(&p, &a, &Control::none()).unwrap();
        let pid = ids(&p, "point_id");
        let at = |id: &str| r.areas[pid.iter().position(|x| x == id).unwrap()].clone();
        assert_eq!(at("Q03"), Vec::<usize>::new(), "in the hole");
        assert_eq!(at("Q12"), Vec::<usize>::new(), "in the hole");
        assert_eq!(at("Q04"), vec![1], "in the island");
        assert_eq!(at("Q05"), vec![0], "on the hole's ring");
        assert_eq!(at("Q09"), Vec::<usize>::new(), "in M's gap");
        assert_eq!(at("Q10"), vec![2], "on M's edge");
        assert_eq!(at("Q11"), vec![0], "on H's outer edge");
    }

    #[test]
    fn t1_schools_in_v1_districts_match_the_reference() {
        let r = check_counts("V1/districts.geojson", "district_id", "T1/schools.csv", "T1/expected_count_in_areas.csv");
        assert_eq!(r.points_without_geometry, 6, "the blank-coordinate rows");
    }

    #[test]
    fn missing_geometries_are_excluded_and_reported() {
        let mut p = points(&[c(0.5, 0.5)]);
        p.push_next(None, vec![]);
        p.push_next(Some(Geometry::MultiPoint(vec![])), vec![]);
        p.push_next(Some(Geometry::Point(c(f64::NAN, 0.0))), vec![]);
        p.push_next(Some(Geometry::MultiPoint(vec![c(0.25, 0.25), c(0.75, 0.75)])), vec![]);
        let mut a = areas(vec![vec![vec![rect(0.0, 0.0, 1.0, 1.0)]]]);
        a.push_next(None, vec![]);
        a.push_next(Some(Geometry::Polygon(vec![])), vec![]);
        a.push_next(Some(Geometry::Point(c(0.5, 0.5))), vec![]);
        let r = count_points_in_areas(&p, &a, &Control::none()).unwrap();
        assert_eq!(r.counts, vec![2, 0, 0, 0], "a multipoint row counts once");
        assert_eq!(r.points_without_geometry, 3);
        assert_eq!(r.areas_without_geometry, vec![1, 2, 3]);
        assert_eq!(r.outside_all, 0);
        let w = which_area(&p, &a, &Control::none()).unwrap();
        assert_eq!(w.points_without_geometry, vec![1, 2, 3]);
        assert_eq!(w.areas.len(), 5);
    }

    #[test]
    fn ring_location_is_exact() {
        let sq = rect(0.0, 0.0, 1.0, 1.0);
        assert_eq!(locate_in_ring(&sq, c(0.5, 0.5)), Location::Inside);
        assert_eq!(locate_in_ring(&sq, c(0.0, 0.0)), Location::Boundary);
        assert_eq!(locate_in_ring(&sq, c(1.0, 0.3)), Location::Boundary);
        assert_eq!(locate_in_ring(&sq, c(1.0 + f64::EPSILON, 0.3)), Location::Outside);
        assert_eq!(locate_in_ring(&sq, c(2.0, 0.0)), Location::Outside, "on the line of an edge, past it");
        // An unclosed ring is closed implicitly.
        assert_eq!(locate_in_ring(&sq[..4], c(0.0, 0.5)), Location::Boundary);
        // A diagonal edge: a point exactly on it, and the next float off it.
        let tri = vec![c(0.0, 0.0), c(3.0, 1.0), c(0.0, 1.0)];
        assert_eq!(locate_in_ring(&tri, c(1.5, 0.5)), Location::Boundary);
        assert_eq!(locate_in_ring(&tri, c(1.5, 0.5f64.next_up())), Location::Inside);
        assert_eq!(locate_in_ring(&tri, c(1.5, 0.5f64.next_down())), Location::Outside);
        // A ray through a vertex is counted once.
        let diamond = vec![c(0.0, -1.0), c(1.0, 0.0), c(0.0, 1.0), c(-1.0, 0.0)];
        assert_eq!(locate_in_ring(&diamond, c(-0.5, 0.0)), Location::Inside);
        assert_eq!(locate_in_ring(&diamond, c(-2.0, 0.0)), Location::Outside);
        assert_eq!(locate_in_ring(&[], c(0.0, 0.0)), Location::Outside);
    }

    #[test]
    fn whats_inside_points_and_lines() {
        let area = Geometry::Polygon(vec![vec![rect(0.0, 0.0, 4.0, 4.0), rect(1.0, 1.0, 3.0, 3.0)]]);
        let mut d = Dataset::new("mixed", GeometryType::Polyline, vec![]);
        let line = |v: Vec<Coord>| Some(Geometry::Line(vec![v]));
        d.push_next(Some(Geometry::Point(c(0.5, 0.5))), vec![]); // 0 inside
        d.push_next(Some(Geometry::Point(c(4.0, 2.0))), vec![]); // 1 on the edge
        d.push_next(Some(Geometry::Point(c(2.0, 2.0))), vec![]); // 2 in the hole: no
        d.push_next(line(vec![c(-1.0, 2.0), c(-0.5, 2.0)]), vec![]); // 3 outside: no
        d.push_next(line(vec![c(-1.0, 2.0), c(0.0, 2.0)]), vec![]); // 4 touches the edge
        d.push_next(line(vec![c(-1.0, -1.0), c(5.0, 5.0)]), vec![]); // 5 crosses, no vertex inside
        d.push_next(line(vec![c(1.5, 1.5), c(2.5, 2.5)]), vec![]); // 6 wholly in the hole: no
        d.push_next(line(vec![c(1.5, 1.5), c(2.5, 3.0)]), vec![]); // 7 touches the hole's ring
        d.push_next(None, vec![]); // 8
        d.push_next(Some(Geometry::Polygon(vec![vec![rect(-2.0, -2.0, 6.0, 6.0)]])), vec![]); // 9 covers it
        d.push_next(Some(Geometry::Polygon(vec![vec![rect(1.5, 1.5, 2.5, 2.5)]])), vec![]); // 10 in the hole: no
        let r = whats_inside(&area, &d, &Control::none()).unwrap();
        assert_eq!(r.rows, vec![0, 1, 4, 5, 7, 9]);
        assert_eq!(r.without_geometry, vec![8]);
        let none = whats_inside(&Geometry::Point(c(0.0, 0.0)), &d, &Control::none()).unwrap();
        assert!(none.rows.is_empty());
    }

    #[test]
    fn segments_touching_and_collinear() {
        assert!(segments_intersect(c(0.0, 0.0), c(2.0, 2.0), c(0.0, 2.0), c(2.0, 0.0)));
        assert!(segments_intersect(c(0.0, 0.0), c(1.0, 0.0), c(1.0, 0.0), c(1.0, 5.0)));
        assert!(segments_intersect(c(0.0, 0.0), c(2.0, 0.0), c(1.0, 0.0), c(3.0, 0.0)));
        assert!(!segments_intersect(c(0.0, 0.0), c(1.0, 0.0), c(2.0, 0.0), c(3.0, 0.0)));
        assert!(!segments_intersect(c(0.0, 0.0), c(1.0, 1.0), c(0.0, 1.0), c(0.4, 0.6)));
    }

    #[test]
    fn a_cancel_stops_the_count() {
        let p = points(&(0..300_000).map(|i| c((i % 1000) as f64 / 1000.0, (i / 1000) as f64 / 300.0)).collect::<Vec<_>>());
        let a = areas(vec![vec![vec![rect(0.0, 0.0, 1.0, 1.0)]]]);
        let ctl = Control::none();
        ctl.cancel();
        assert_eq!(count_points_in_areas(&p, &a, &ctl), Err(Cancelled));
        assert_eq!(which_area(&p, &a, &ctl), Err(Cancelled));
        assert_eq!(whats_inside(&Geometry::Polygon(vec![vec![rect(0.0, 0.0, 1.0, 1.0)]]), &p, &ctl), Err(Cancelled));
    }

    #[test]
    fn progress_ends_at_the_total() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let s = seen.clone();
        let ctl = Control::with_progress(move |p| s.lock().unwrap().push(p));
        let p = points(&[c(0.5, 0.5), c(2.0, 2.0)]);
        count_points_in_areas(&p, &areas(vec![vec![vec![rect(0.0, 0.0, 1.0, 1.0)]]]), &ctl).unwrap();
        let last = seen.lock().unwrap().last().cloned().unwrap();
        assert_eq!((last.phase.as_str(), last.done, last.total), (PHASE_COUNT, 2, Some(2)));
    }

    #[test]
    fn attributes_dont_matter_and_rows_stay_aligned() {
        let mut a = Dataset::new("a", GeometryType::Polygon, vec![Field::new("name", FieldType::Text)]);
        a.push_next(Some(Geometry::Polygon(vec![vec![rect(0.0, 0.0, 1.0, 1.0)]])), vec![Value::Text("x".into())]);
        a.push_next(Some(Geometry::Polygon(vec![vec![rect(5.0, 5.0, 6.0, 6.0)]])), vec![Value::Text("empty".into())]);
        let r = count_points_in_areas(&points(&[c(0.2, 0.2), c(1.0, 1.0)]), &a, &Control::none()).unwrap();
        assert_eq!(r.counts, vec![2, 0]);
    }

    #[test]
    fn n10_join_matches_the_reference() {
        let a = read("N10/areas.geojson");
        let area_ids = ids(&a, "area_id");
        let want = csv("N10/expected_join.csv");
        let mut seen = 0;
        for layer in ["points", "lines", "polygons"] {
            let f = read(&format!("N10/{layer}.geojson"));
            let r = join_by_location(&f, &a, &Control::none()).unwrap();
            assert_eq!(r.areas_without_geometry, vec![9], "A10");
            let fids = ids(&f, "feature_id");
            let mut boundary = 0;
            let mut outside = 0;
            for row in want.iter().filter(|w| w["layer"] == layer) {
                let i = fids.iter().position(|x| *x == row["feature_id"]).unwrap();
                let got = r.area_of[i].map(|k| area_ids[k].clone()).unwrap_or_default();
                assert_eq!(got, row["area_id"], "{layer} {}", row["feature_id"]);
                let g = f.features[i].geometry.as_ref();
                let probes: Vec<Coord> = match g {
                    Some(Geometry::Point(c)) => vec![*c],
                    Some(Geometry::MultiPoint(v)) => v.clone(),
                    Some(g) => crate::measure::label_point(g).into_iter().collect(),
                    None => vec![],
                };
                // Lines and areas: the reference doesn't know LayeredMap's label point, so the
                // boundary flag is recomputed from it here.
                let on = probes.iter().any(|p| {
                    a.features.iter().any(|af| matches!(&af.geometry, Some(Geometry::Polygon(ps)) if ps.iter().any(|rings| locate_in_polygon(rings, *p) == Location::Boundary)))
                });
                match row["on_boundary"].as_str() {
                    "true" => assert!(on, "{}", row["feature_id"]),
                    "false" => assert!(!on, "{}", row["feature_id"]),
                    _ => {}
                }
                boundary += u64::from(on);
                outside += u64::from(g.is_some() && row["area_id"].is_empty());
                seen += 1;
            }
            assert_eq!(r.on_boundary, boundary, "{layer}");
            assert_eq!(r.outside, outside, "{layer}");
            if layer == "points" {
                assert_eq!(r.without_geometry, vec![fids.iter().position(|x| x == "P15").unwrap()]);
                assert_eq!(r.on_boundary, 6);
            }
        }
        assert_eq!(seen, want.len());
    }

    #[test]
    fn join_takes_the_first_area_and_uses_the_label_point() {
        let a =
            areas(vec![vec![vec![rect(0.0, 0.0, 1.0, 1.0)]], vec![vec![rect(1.0, 0.0, 3.0, 1.0)]], vec![vec![rect(-1.0, -1.0, 5.0, 5.0)]]]);
        let mut f = Dataset::new("f", GeometryType::Polygon, vec![]);
        // Straddles areas 0 and 1, mostly in 1: its label point is in 1 (and in 2, listed later).
        f.push_next(Some(Geometry::Polygon(vec![vec![rect(0.9, 0.1, 2.9, 0.9)]])), vec![]);
        // A line whose middle vertex is in 0.
        f.push_next(Some(Geometry::Line(vec![vec![c(4.0, 4.0), c(0.5, 0.5), c(-3.0, -3.0)]])), vec![]);
        // On the edge between 0 and 1.
        f.push_next(Some(Geometry::Point(c(1.0, 0.5))), vec![]);
        f.push_next(Some(Geometry::Point(c(9.0, 9.0))), vec![]);
        f.push_next(Some(Geometry::Polygon(vec![])), vec![]);
        let r = join_by_location(&f, &a, &Control::none()).unwrap();
        assert_eq!(r.area_of, vec![Some(1), Some(0), Some(0), None, None]);
        assert_eq!((r.on_boundary, r.outside, r.without_geometry.clone()), (1, 1, vec![4]));
        // Agrees with "Which area" for points: the first of its list.
        let p = points(&[c(1.0, 0.5), c(2.0, 0.5), c(9.0, 9.0), c(4.0, 4.0)]);
        let w = which_area(&p, &a, &Control::none()).unwrap();
        let j = join_by_location(&p, &a, &Control::none()).unwrap();
        assert_eq!(j.area_of, w.areas.iter().map(|v| v.first().copied()).collect::<Vec<_>>());
        let ctl = Control::none();
        ctl.cancel();
        assert_eq!(join_by_location(&p, &a, &ctl), Err(Cancelled));
    }

    /// Brute force: the definition applied point by point, no index.
    fn brute(p: &[Coord], a: &[Vec<Vec<Vec<Coord>>>]) -> Vec<Vec<usize>> {
        p.iter().map(|q| (0..a.len()).filter(|&i| locate_in_area(&a[i], *q).touches()).collect()).collect()
    }

    /// Coordinates on a 1/8 grid, so many points land exactly on edges and corners.
    fn grid() -> impl Strategy<Value = f64> {
        (0i32..=80).prop_map(|i| f64::from(i) / 8.0)
    }

    fn arb_rect() -> impl Strategy<Value = Vec<Vec<Vec<Coord>>>> {
        (grid(), grid(), 1i32..24, 1i32..24).prop_map(|(x, y, w, h)| vec![vec![rect(x, y, x + f64::from(w) / 8.0, y + f64::from(h) / 8.0)]])
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn index_matches_brute_force(pts in prop::collection::vec((grid(), grid()), 0..60), rs in prop::collection::vec(arb_rect(), 0..12)) {
            let cs: Vec<Coord> = pts.iter().map(|&(x, y)| c(x, y)).collect();
            let w = which_area(&points(&cs), &areas(rs.clone()), &Control::none()).unwrap();
            prop_assert_eq!(&w.areas, &brute(&cs, &rs));
            // And an independent check: rectangles are closed boxes.
            let boxed: Vec<Vec<usize>> = cs.iter().map(|q| (0..rs.len()).filter(|&i| {
                let e = Geometry::Polygon(rs[i].clone()).extent();
                e.min_x <= q.x && q.x <= e.max_x && e.min_y <= q.y && q.y <= e.max_y
            }).collect()).collect();
            prop_assert_eq!(&w.areas, &boxed);
            let n = count_points_in_areas(&points(&cs), &areas(rs.clone()), &Control::none()).unwrap();
            for (i, k) in n.counts.iter().enumerate() {
                prop_assert_eq!(*k as usize, w.areas.iter().filter(|v| v.contains(&i)).count());
            }
            prop_assert_eq!(n.counted_more_than_once as usize, w.areas.iter().filter(|v| v.len() > 1).count());
            prop_assert_eq!(n.outside_all as usize, w.areas.iter().filter(|v| v.is_empty()).count());
            // Deterministic.
            prop_assert_eq!(n, count_points_in_areas(&points(&cs), &areas(rs), &Control::none()).unwrap());
        }

        #[test]
        fn counts_never_drop_when_an_area_grows(pts in prop::collection::vec((grid(), grid()), 0..60), (x, y, w, h) in (grid(), grid(), 1i32..24, 1i32..24), grow in 0i32..8) {
            let cs: Vec<Coord> = pts.iter().map(|&(x, y)| c(x, y)).collect();
            let (w, h, g) = (f64::from(w) / 8.0, f64::from(h) / 8.0, f64::from(grow) / 8.0);
            let small = count_points_in_areas(&points(&cs), &areas(vec![vec![vec![rect(x, y, x + w, y + h)]]]), &Control::none()).unwrap();
            let big = count_points_in_areas(&points(&cs), &areas(vec![vec![vec![rect(x - g, y - g, x + w + g, y + h + g)]]]), &Control::none()).unwrap();
            prop_assert!(big.counts[0] >= small.counts[0]);
        }

        #[test]
        fn a_point_on_a_shared_edge_is_in_both(x in grid(), y0 in grid(), y1 in grid(), t in 0.0f64..=1.0, rot in 0usize..4) {
            // Two rectangles sharing the vertical edge x, from y0 to y0+h.
            let h = (y1 - y0).abs().max(0.125);
            let lo = y0.min(y1);
            let mut left = rect(x - 1.0, lo, x, lo + h);
            let mut right = rect(x, lo, x + 1.0, lo + h);
            // Start the rings at a different vertex: same polygon.
            left.pop(); left.rotate_left(rot); left.push(left[0]);
            right.pop(); right.rotate_left(rot); right.push(right[0]);
            let p = c(x, lo + h * t);
            let w = which_area(&points(&[p]), &areas(vec![vec![vec![left]], vec![vec![right]]]), &Control::none()).unwrap();
            prop_assert_eq!(&w.areas[0], &vec![0, 1]);
        }

        #[test]
        fn a_point_in_a_hole_is_outside_and_on_its_ring_touches(t in 0.0f64..=1.0, side in 0usize..4) {
            let hole = rect(1.0, 1.0, 3.0, 3.0);
            let poly = vec![rect(0.0, 0.0, 4.0, 4.0), hole.clone()];
            let (a, b) = (hole[side], hole[side + 1]);
            let on = c(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            prop_assert_eq!(locate_in_polygon(&poly, c(1.0 + 2.0 * t.clamp(0.01, 0.99), 2.0)), Location::Outside);
            prop_assert!(locate_in_polygon(&poly, on).touches());
        }
    }

    /// Gate 4 stress: count F2-sized points (1,000,000) in V7-sized areas (100,000 polygons) in
    /// under 60 s. Run with
    /// `cargo test --release -p lm-analysis -- --ignored --nocapture stress_count_1m_points_in_100k_areas`.
    #[test]
    #[ignore]
    fn stress_count_1m_points_in_100k_areas() {
        // A deterministic generator (xorshift), no extra dependency.
        let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut rnd = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        // 100,000 polygons of 32 vertices on a 400 × 250 grid over Shanghai, each slightly
        // overlapping its neighbours, about every 10th with a hole.
        let (x0, y0, x1, y1) = (120.8, 30.6, 122.2, 31.9);
        let (nx, ny) = (400usize, 250usize);
        let (dx, dy) = ((x1 - x0) / nx as f64, (y1 - y0) / ny as f64);
        let circle = |cx: f64, cy: f64, rx: f64, ry: f64, n: usize| -> Vec<Coord> {
            let mut r: Vec<Coord> = (0..n)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / n as f64;
                    c(cx + rx * a.cos(), cy + ry * a.sin())
                })
                .collect();
            r.push(r[0]);
            r
        };
        let mut polys = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                let (cx, cy) = (x0 + (i as f64 + 0.5) * dx, y0 + (j as f64 + 0.5) * dy);
                let mut rings = vec![circle(cx, cy, dx * 0.6, dy * 0.6, 32)];
                if (i + j) % 10 == 0 {
                    rings.push(circle(cx, cy, dx * 0.2, dy * 0.2, 16));
                }
                polys.push(vec![rings]);
            }
        }
        let a = areas(polys);
        let p = points(&(0..1_000_000).map(|_| c(x0 + rnd() * (x1 - x0), y0 + rnd() * (y1 - y0))).collect::<Vec<_>>());
        let t = std::time::Instant::now();
        let r = count_points_in_areas(&p, &a, &Control::none()).unwrap();
        let secs = t.elapsed().as_secs_f64();
        let total: u64 = r.counts.iter().sum();
        println!(
            "stress: 1,000,000 points in 100,000 areas in {secs:.2} s; {total} counts, {} counted more than once, {} outside",
            r.counted_more_than_once, r.outside_all
        );
        assert_eq!(r.counts.len(), 100_000);
        assert!(secs < 60.0, "took {secs:.1} s");
    }
}
