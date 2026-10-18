//! "Add the distance to the nearest [other] to each [feature]" and its same-layer variant "Add
//! the distance to the nearest other [feature]" (island isolation): new action A3
//! (`docs/lm/puzzles/mechanics.md` › A3).
//!
//! # Definition
//!
//! For each feature, the shortest geodesic distance on the WGS 84 ellipsoid from any point of it
//! to any point of any target feature, and the target's row.
//!
//! - **Touching is 0.** When the feature and a target intersect (a point in or on a polygon, a
//!   point on a line, crossing or touching edges, one polygon inside another) the distance is 0.
//!   This test is planar in longitude/latitude and exact, the same convention and the same
//!   predicates as [`crate::overlay`] (`locate_in_polygon`, `segments_intersect`).
//! - **Otherwise** both the feature and the targets are densified along geodesics to steps of at
//!   most [`DENSIFY_M`] (1 km, recorded in [`NearestResult::densify_m`]), and the distance is the
//!   minimum over vertex-to-segment distances in both directions (each feature vertex to each
//!   target segment, each target vertex to each feature segment). A vertex-to-segment distance is
//!   measured in the azimuthal equidistant projection centred on the vertex: the distances to the
//!   segment's ends are exact geodesics (GeographicLib's inverse problem) and the segment, at most
//!   1 km long, is a straight chord between them (the chord's error is millimetres).
//! - **Ties** go to the lower target row; a feature touching several targets reports the lowest.
//! - **Same layer.** The targets are the features themselves; a feature never matches itself, and
//!   a multipart feature's other parts are not "other" (the whole row is excluded).
//! - **Missing geometry.** Feature rows with no geometry, an empty one or a non-finite coordinate
//!   get `None` and are listed in [`NearestResult::without_geometry`]; such target rows are never
//!   matched and are listed in [`NearestResult::targets_without_geometry`]. With no target at all
//!   every feature gets `None`.
//!
//! Edges are geodesics for the distance (as in [`crate::geodesic`]) and straight lon/lat lines for
//! the touching test (as everywhere in the app's overlay); the two differ only for edges long
//! enough to bow visibly, and only for features within a few hundred metres of each other.
//!
//! # Speed and the pruning bounds
//!
//! Two safe lower bounds on geodesic distance prune the search, so the answer is the minimum over
//! every pair, not a sample:
//!
//! - **The chord.** No path on the ellipsoid is shorter than the straight line between its ends,
//!   so the Earth-centred (ECEF) distance between two boxes bounds the geodesic distance between
//!   anything inside them; it is within 0.1% for distances up to 1,000 km. Target segments sit in
//!   R-trees of ECEF boxes (no antimeridian or pole special case), each padded by the most a
//!   geodesic of that length can bow away from its chord (`len²/(8·ρ_min)`, `ρ_min = a(1 − e²)`
//!   the smallest radius of curvature).
//! - **The triangle inequality.** A leaf of up to [`LEAF`] consecutive segments has a centre vertex
//!   and a radius (the geodesic distance to its farthest point); two leaves' points are at least
//!   `d(centres) − r₁ − r₂` apart.
//!
//! A point searches the target segments best first by the chord bound and stops when the bound
//! passes the best distance found. A line or area is cut into leaves; each leaf's chord distance
//! to the nearest other target orders them and stops the search; within a leaf, only target leaves
//! inside its box grown by the best distance, and passing the triangle bound, are measured pair by
//! pair. Targets are grouped into runs of at most [`RUN`] consecutive segments of one row, so the
//! same-layer search skips a feature's own row a run at a time, however large it is.
//!
//! The run trees are built in chunks of [`CHUNK`] runs after a linear spatial sort, so each bulk
//! load finishes well inside the 50 ms cancel budget and most chunks are pruned whole; work is
//! checked for cancel every thousand or so geodesic problems, and progress reports features done
//! of features total.

use crate::control::{Cancelled, Control};
use crate::overlay::{Work, locate_in_area, locate_in_polygon, segments_intersect, usable};
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, Envelope, ParentNode, RTree, RTreeNode, RTreeObject};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::OnceLock;

use rustgis_data::{Coord, Dataset, Geometry};

/// Longest geodesic step after densification (metres): A3's "at most 1 km".
pub const DENSIFY_M: f64 = 1000.0;

/// Runs per R-tree chunk: each bulk load stays well under 50 ms.
pub const CHUNK: usize = 1 << 15;

/// Segments per leaf, the unit of the triangle-inequality bound (at most 8 km of densified line).
pub const LEAF: usize = 8;

/// Feature leaves per group, the coarser unit of the same bound.
const GROUP: usize = 32;

/// Smallest radius of curvature on WGS 84, a(1 − e²) (metres).
const RHO_MIN: f64 = 6_335_439.327_292_82;

/// Work units per geodesic problem, against `Work`'s cancel check every 2^18 units (so a check at
/// least every 1,024 problems, a few milliseconds).
const GEO: usize = 256;

/// Work units per R-tree candidate visited (a check at least every 4,096 candidates).
const TREE: usize = 64;

const PHASE_INDEX: &str = "Preparing the other layer";
const PHASE_SEARCH: &str = "Measuring the distance to the nearest feature";

/// The nearest target of one feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nearest {
    /// Metres; 0 when the feature touches or overlaps the target.
    pub distance_m: f64,
    /// The target's row.
    pub target_row: usize,
}

/// The result of "Add the distance to the nearest [other]".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NearestResult {
    /// One entry per feature row, in row order; `None` without a usable geometry or without any
    /// target.
    pub nearest: Vec<Option<Nearest>>,
    /// Feature rows without a usable geometry.
    pub without_geometry: Vec<usize>,
    /// The densification step used (metres), for the card.
    pub densify_m: f64,
    /// Target rows without a usable geometry (never matched).
    pub targets_without_geometry: Vec<usize>,
}

/// For each feature of `features`, the shortest geodesic (WGS 84) distance to any target feature
/// (0 when touching/overlapping). `same_layer`: targets are `features` itself (and `targets` is
/// not read); a feature never matches itself. See the module notes for the definition.
pub fn nearest_distance(features: &Dataset, targets: &Dataset, same_layer: bool, ctl: &Control) -> Result<NearestResult, Cancelled> {
    let targets = if same_layer { features } else { targets };
    search_all(features, targets, |row| same_layer.then_some(row), ctl)
}

/// [`nearest_distance`] where feature `i` never matches target row `skip[i]` (its own row when
/// `features` is a subset of `targets`, as for "the nearest other object of the whole layer" on
/// a filtered layer: owner's decision, 2026-10-17).
pub fn nearest_distance_skipping(
    features: &Dataset,
    targets: &Dataset,
    skip: &[Option<usize>],
    ctl: &Control,
) -> Result<NearestResult, Cancelled> {
    search_all(features, targets, |row| skip.get(row).copied().flatten(), ctl)
}

fn search_all(
    features: &Dataset,
    targets: &Dataset,
    skip_of: impl Fn(usize) -> Option<usize>,
    ctl: &Control,
) -> Result<NearestResult, Cancelled> {
    let mut work = Work::new(ctl);
    let index = Targets::build(targets, ctl, &mut work)?;
    let total = features.features.len() as u64;
    let mut out = NearestResult {
        nearest: Vec::with_capacity(features.features.len()),
        without_geometry: Vec::new(),
        densify_m: DENSIFY_M,
        targets_without_geometry: index.without_geometry.clone(),
    };
    let mut search = Search::new(&index);
    for (row, f) in features.features.iter().enumerate() {
        if row % 256 == 0 {
            ctl.report(PHASE_SEARCH, row as u64, Some(total));
        }
        work.add(GEO)?;
        let Some(g) = f.geometry.as_ref().filter(|g| usable(g)) else {
            out.without_geometry.push(row);
            out.nearest.push(None);
            continue;
        };
        let skip = skip_of(row);
        out.nearest.push(search.nearest(g, skip, &mut work)?);
    }
    ctl.report(PHASE_SEARCH, total, Some(total));
    Ok(out)
}

// ---------------------------------------------------------------------------------------------

fn wgs84() -> &'static Geodesic {
    static G: OnceLock<Geodesic> = OnceLock::new();
    G.get_or_init(Geodesic::wgs84)
}

/// Earth-centred, Earth-fixed coordinates (metres) of a geodetic lon/lat (degrees) on WGS 84.
fn ecef(c: Coord) -> [f64; 3] {
    const A: f64 = 6_378_137.0;
    const F: f64 = 1.0 / 298.257_223_563;
    const E2: f64 = F * (2.0 - F);
    let (sl, cl) = c.y.to_radians().sin_cos();
    let (sn, cn) = c.x.to_radians().sin_cos();
    let n = A / (1.0 - E2 * sl * sl).sqrt();
    [n * cl * cn, n * cl * sn, n * (1.0 - E2) * sl]
}

/// A safe lower bound on the geodesic distance (m) between things whose ECEF boxes are
/// `sqrt(d2)` apart: the chord.
fn lower_bound_m(d2: f64) -> f64 {
    d2.sqrt()
}

/// Squared distance between two boxes (0 when they overlap).
fn box_gap2(a: &AABB<[f64; 3]>, b: &AABB<[f64; 3]>) -> f64 {
    let (al, ah, bl, bh) = (a.lower(), a.upper(), b.lower(), b.upper());
    (0..3).map(|k| (bl[k] - ah[k]).max(al[k] - bh[k]).max(0.0).powi(2)).sum()
}

/// `e` grown by `by` metres on every side.
fn grown(e: &AABB<[f64; 3]>, by: f64) -> AABB<[f64; 3]> {
    let (lo, hi) = (e.lower(), e.upper());
    AABB::from_corners([lo[0] - by, lo[1] - by, lo[2] - by], [hi[0] + by, hi[1] + by, hi[2] + by])
}

/// A segment of the densified targets: vertex indices (equal for a point) and its length (m).
#[derive(Clone, Copy, Debug)]
struct Seg {
    a: usize,
    b: usize,
    len: f64,
}

/// At most [`RUN`] consecutive segments of one target row, with their own R-tree when there are
/// more than [`SMALL_RUN`]. The search walks a tree of runs and then the segments of each run, so
/// a feature's own row is skipped a run at a time (the same-layer variant), however large it is.
struct Run {
    row: usize,
    first: usize,
    len: usize,
    /// The index of the run's first leaf in [`Targets::leaves`].
    leaf0: usize,
    tree: Option<RTree<Box3>>,
}

/// Up to [`LEAF`] consecutive target segments (within one run).
struct TLeaf {
    run: usize,
    first: usize,
    len: usize,
}

/// Longest run of segments (about 500 km of densified line).
const RUN: usize = 512;

/// Runs up to this many segments are scanned instead of indexed.
const SMALL_RUN: usize = 32;

type Box3 = GeomWithData<Rectangle<[f64; 3]>, usize>;
type Box2 = GeomWithData<Rectangle<[f64; 2]>, usize>;

/// The ECEF box of every point on the geodesic segment `a`–`b` of length `len` (m): the chord's
/// box padded by the most a curve of that length with curvature at most `1/ρ_min` (any geodesic)
/// bows away from its chord, `len²/(8·ρ_min)`, with margin.
fn seg_box(pa: [f64; 3], pb: [f64; 3], len: f64) -> ([f64; 3], [f64; 3]) {
    let pad = len * len / (7.0 * RHO_MIN) + 1e-3;
    let lo = [pa[0].min(pb[0]) - pad, pa[1].min(pb[1]) - pad, pa[2].min(pb[2]) - pad];
    let hi = [pa[0].max(pb[0]) + pad, pa[1].max(pb[1]) + pad, pa[2].max(pb[2]) + pad];
    (lo, hi)
}

/// A geometry densified along geodesics: its vertices and segments (vertex index pairs with
/// their length in metres). Points have vertices and no segments.
#[derive(Default)]
struct Dense {
    verts: Vec<Coord>,
    segs: Vec<(usize, usize, f64)>,
}

impl Dense {
    /// Appends a path (closed when `close` and its last vertex isn't its first).
    fn path(&mut self, pts: &[Coord], close: bool, work: &mut Work) -> Result<(), Cancelled> {
        let Some(&first) = pts.first() else { return Ok(()) };
        let mut prev = self.verts.len();
        self.verts.push(first);
        let closing = (close && pts.len() > 2 && pts.last() != Some(&first)).then_some(first);
        let g = wgs84();
        for &b in pts.iter().skip(1).chain(closing.iter()) {
            let Some(&a) = self.verts.get(prev) else { break };
            let (s, azi, _, _): (f64, f64, f64, f64) = g.inverse(a.y, a.x, b.y, b.x);
            let n = (s / DENSIFY_M).ceil().max(1.0);
            let n = if n.is_finite() { n as usize } else { 1 };
            work.add(GEO * (n + 1))?;
            let step = s / n as f64;
            for k in 1..n {
                let (lat, lon): (f64, f64) = g.direct(a.y, a.x, azi, s * k as f64 / n as f64);
                self.verts.push(Coord::new(lon, lat));
                self.segs.push((prev, self.verts.len() - 1, step));
                prev = self.verts.len() - 1;
            }
            self.verts.push(b);
            self.segs.push((prev, self.verts.len() - 1, step));
            prev = self.verts.len() - 1;
        }
        Ok(())
    }

    fn of(g: &Geometry, work: &mut Work) -> Result<Dense, Cancelled> {
        let mut d = Dense::default();
        match g {
            Geometry::Point(c) => d.verts.push(*c),
            Geometry::MultiPoint(v) => d.verts.extend_from_slice(v),
            Geometry::Line(parts) => {
                for p in parts {
                    d.path(p, false, work)?;
                }
            }
            Geometry::Polygon(polys) => {
                for ring in polys.iter().flatten() {
                    d.path(ring, true, work)?;
                }
            }
        }
        Ok(d)
    }
}

/// One part of a target, for the touching test.
enum PartKind<'a> {
    Area(&'a [Vec<Coord>]),
    Path(&'a [Coord]),
    Point(Coord),
}

impl PartKind<'_> {
    /// A vertex of the part (for "is it inside the feature?").
    fn probe(&self) -> Option<Coord> {
        match self {
            PartKind::Area(rings) => rings.first().and_then(|r| r.first()).copied(),
            PartKind::Path(p) => p.first().copied(),
            PartKind::Point(c) => Some(*c),
        }
    }
}

struct Part<'a> {
    row: usize,
    kind: PartKind<'a>,
}

/// The original edges of a geometry, closing rings; a point (or a one-vertex line) is a
/// zero-length edge.
fn edges_of(g: &Geometry, mut f: impl FnMut(Coord, Coord)) {
    let mut path = |p: &[Coord], close: bool| {
        if p.len() == 1 {
            if let Some(&a) = p.first() {
                f(a, a);
            }
            return;
        }
        for w in p.windows(2) {
            if let [a, b] = w {
                f(*a, *b);
            }
        }
        if close
            && let (Some(&a), Some(&b)) = (p.last(), p.first())
            && a != b
        {
            f(a, b);
        }
    };
    match g {
        Geometry::Point(c) => path(std::slice::from_ref(c), false),
        Geometry::MultiPoint(v) => v.iter().for_each(|c| path(std::slice::from_ref(c), false)),
        Geometry::Line(parts) => parts.iter().for_each(|p| path(p, false)),
        Geometry::Polygon(polys) => polys.iter().flatten().for_each(|r| path(r, true)),
    }
}

fn box2(a: Coord, b: Coord) -> AABB<[f64; 2]> {
    AABB::from_corners([a.x.min(b.x), a.y.min(b.y)], [a.x.max(b.x), a.y.max(b.y)])
}

/// `items` reordered by a coarse spatial cell (a counting sort, linear and checked for cancel),
/// so each chunk [`chunked`] makes covers a compact region and most chunks are pruned whole.
fn spatial_order<T>(items: Vec<T>, cells: usize, key: impl Fn(&T) -> usize, work: &mut Work) -> Result<Vec<T>, Cancelled> {
    if items.len() <= CHUNK {
        return Ok(items);
    }
    let keys: Vec<usize> = items.iter().map(|it| key(it).min(cells - 1)).collect();
    let mut start = vec![0usize; cells + 1];
    for &k in &keys {
        if let Some(s) = start.get_mut(k + 1) {
            *s += 1;
        }
    }
    let mut acc = 0;
    for s in start.iter_mut() {
        acc += *s;
        *s = acc;
    }
    let mut slots: Vec<Option<T>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    for (it, k) in items.into_iter().zip(keys) {
        work.add(4)?;
        if let Some(pos) = start.get_mut(k) {
            if let Some(slot) = slots.get_mut(*pos) {
                *slot = Some(it);
            }
            *pos += 1;
        }
    }
    Ok(slots.into_iter().flatten().collect())
}

/// The cell of a box on the globe: the cube face its centre normal points to, then a 16 × 16 grid
/// on that face (6 × 256 cells).
fn cell3(r: &Box3) -> usize {
    let e = r.envelope();
    let (lo, hi) = (e.lower(), e.upper());
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let k = (0..3).max_by(|&i, &j| c[i].abs().total_cmp(&c[j].abs())).unwrap_or(0);
    let m = c[k].abs().max(1e-300);
    let face = 2 * k + usize::from(c[k] < 0.0);
    let (u, v) = (c[(k + 1) % 3] / m, c[(k + 2) % 3] / m);
    let grid = |x: f64| (((x + 1.0) * 8.0).floor().clamp(0.0, 15.0)) as usize;
    face * 256 + grid(u) * 16 + grid(v)
}

/// The cell of a lon/lat box: a 64 × 32 grid of the world.
fn cell2(r: &Box2) -> usize {
    let e = r.envelope();
    let (lo, hi) = (e.lower(), e.upper());
    let (x, y) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    let i = ((x + 180.0) / 360.0 * 64.0).floor().clamp(0.0, 63.0) as usize;
    let j = ((y + 90.0) / 180.0 * 32.0).floor().clamp(0.0, 31.0) as usize;
    j * 64 + i
}

/// Bulk-loads `items` in chunks, checking for cancel between them.
fn chunked<T: RTreeObject>(mut items: Vec<T>, ctl: &Control) -> Result<Vec<RTree<T>>, Cancelled> {
    let mut trees = Vec::new();
    while !items.is_empty() {
        ctl.check()?;
        // Off the end, so each split copies one chunk.
        let chunk = items.split_off(items.len().saturating_sub(CHUNK));
        trees.push(RTree::bulk_load(chunk));
    }
    ctl.check()?;
    Ok(trees)
}

/// The target layer, densified and indexed.
struct Targets<'a> {
    verts: Vec<Coord>,
    segs: Vec<Seg>,
    runs: Vec<Run>,
    run_trees: Vec<RTree<Box3>>,
    leaves: Vec<TLeaf>,
    parts: Vec<Part<'a>>,
    part_trees: Vec<RTree<Box2>>,
    edges: Vec<(Coord, Coord, usize)>,
    edge_trees: Vec<RTree<Box2>>,
    without_geometry: Vec<usize>,
}

impl<'a> Targets<'a> {
    fn build(ds: &'a Dataset, ctl: &Control, work: &mut Work) -> Result<Self, Cancelled> {
        let total = ds.features.len() as u64;
        let mut t = Targets {
            verts: Vec::new(),
            segs: Vec::new(),
            runs: Vec::new(),
            run_trees: Vec::new(),
            leaves: Vec::new(),
            parts: Vec::new(),
            part_trees: Vec::new(),
            edges: Vec::new(),
            edge_trees: Vec::new(),
            without_geometry: Vec::new(),
        };
        let mut run_boxes: Vec<Box3> = Vec::new();
        let mut part_boxes: Vec<Box2> = Vec::new();
        for (row, f) in ds.features.iter().enumerate() {
            if row % 256 == 0 {
                ctl.report(PHASE_INDEX, row as u64, Some(total));
            }
            work.add(GEO)?;
            let Some(g) = f.geometry.as_ref().filter(|g| usable(g)) else {
                t.without_geometry.push(row);
                continue;
            };
            // Parts, for the touching test.
            let mut add_part = |kind: PartKind<'a>, pts: &mut dyn Iterator<Item = Coord>| {
                let mut lo = [f64::INFINITY; 2];
                let mut hi = [f64::NEG_INFINITY; 2];
                for c in pts {
                    lo = [lo[0].min(c.x), lo[1].min(c.y)];
                    hi = [hi[0].max(c.x), hi[1].max(c.y)];
                }
                if lo[0] <= hi[0] {
                    part_boxes.push(GeomWithData::new(Rectangle::from_corners(lo, hi), t.parts.len()));
                    t.parts.push(Part { row, kind });
                }
            };
            match g {
                Geometry::Point(c) => add_part(PartKind::Point(*c), &mut std::iter::once(*c)),
                Geometry::MultiPoint(v) => v.iter().for_each(|c| add_part(PartKind::Point(*c), &mut std::iter::once(*c))),
                Geometry::Line(parts) => {
                    for p in parts.iter().filter(|p| !p.is_empty()) {
                        add_part(PartKind::Path(p), &mut p.iter().copied());
                    }
                }
                Geometry::Polygon(polys) => {
                    for rings in polys.iter().filter(|r| r.first().is_some_and(|o| !o.is_empty())) {
                        let outer = rings.first().map(|o| o.as_slice()).unwrap_or(&[]);
                        add_part(PartKind::Area(rings), &mut outer.iter().copied());
                    }
                }
            }
            edges_of(g, |a, b| t.edges.push((a, b, row)));
            // Densified segments, for the distance.
            let d = Dense::of(g, work)?;
            let base = t.verts.len();
            let normals: Vec<[f64; 3]> = d.verts.iter().map(|c| ecef(*c)).collect();
            let mut boxes: Vec<Box3> = Vec::new();
            if d.segs.is_empty() {
                for (k, n) in normals.iter().enumerate() {
                    let (lo, hi) = seg_box(*n, *n, 0.0);
                    boxes.push(GeomWithData::new(Rectangle::from_corners(lo, hi), t.segs.len()));
                    t.segs.push(Seg { a: base + k, b: base + k, len: 0.0 });
                }
            } else {
                for &(a, b, len) in &d.segs {
                    let (Some(na), Some(nb)) = (normals.get(a), normals.get(b)) else { continue };
                    let (lo, hi) = seg_box(*na, *nb, len);
                    boxes.push(GeomWithData::new(Rectangle::from_corners(lo, hi), t.segs.len()));
                    t.segs.push(Seg { a: base + a, b: base + b, len });
                }
            }
            t.verts.extend(d.verts);
            // Runs of consecutive segments (a MultiPoint's points are consecutive too).
            for chunk in boxes.chunks(RUN) {
                let Some(first) = chunk.first() else { continue };
                let mut env = first.envelope();
                chunk.iter().for_each(|b| env.merge(&b.envelope()));
                run_boxes.push(GeomWithData::new(Rectangle::from_aabb(env), t.runs.len()));
                let tree = (chunk.len() > SMALL_RUN).then(|| RTree::bulk_load(chunk.to_vec()));
                let leaf0 = t.leaves.len();
                for k in (0..chunk.len()).step_by(LEAF) {
                    t.leaves.push(TLeaf { run: t.runs.len(), first: first.data + k, len: LEAF.min(chunk.len() - k) });
                }
                t.runs.push(Run { row, first: first.data, len: chunk.len(), leaf0, tree });
            }
        }
        work.add(1 << 18)?;
        let edge_boxes: Vec<Box2> =
            t.edges.iter().enumerate().map(|(i, (a, b, _))| GeomWithData::new(Rectangle::from_aabb(box2(*a, *b)), i)).collect();
        t.run_trees = chunked(spatial_order(run_boxes, 6 * 256, cell3, work)?, ctl)?;
        t.part_trees = chunked(spatial_order(part_boxes, 64 * 32, cell2, work)?, ctl)?;
        t.edge_trees = chunked(spatial_order(edge_boxes, 64 * 32, cell2, work)?, ctl)?;
        ctl.report(PHASE_INDEX, total, Some(total));
        Ok(t)
    }
}

/// The best (distance, row) so far; ties go to the lower row.
#[derive(Clone, Copy)]
struct Best {
    d: f64,
    row: usize,
}

impl Best {
    const NONE: Best = Best { d: f64::INFINITY, row: usize::MAX };

    fn result(self) -> Option<Nearest> {
        self.d.is_finite().then_some(Nearest { distance_m: self.d, target_row: self.row })
    }

    fn offer(&mut self, d: f64, row: usize) {
        if d < self.d || (d == self.d && row < self.row) {
            *self = Best { d, row };
        }
    }
}

/// Planar distance from the origin to segment `a`–`b`.
fn origin_to_seg(a: [f64; 2], b: [f64; 2]) -> f64 {
    point_to_seg([0.0, 0.0], a, b)
}

fn point_to_seg(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (a[0] + t * dx - p[0]).hypot(a[1] + t * dy - p[1])
}

fn rc(p: [f64; 2]) -> Coord {
    Coord::new(p[0], p[1])
}

/// Planar distance between segments `p`–`q` and `c`–`d` (0 when they meet).
fn seg_to_seg(p: [f64; 2], q: [f64; 2], c: [f64; 2], d: [f64; 2]) -> f64 {
    if segments_intersect(rc(p), rc(q), rc(c), rc(d)) {
        return 0.0;
    }
    point_to_seg(p, c, d).min(point_to_seg(q, c, d)).min(point_to_seg(c, p, q)).min(point_to_seg(d, p, q))
}

/// `p` in the azimuthal equidistant projection centred on `center` (metres).
fn aeqd(center: Coord, p: Coord) -> [f64; 2] {
    let (s, azi, _, _): (f64, f64, f64, f64) = wgs84().inverse(center.y, center.x, p.y, p.x);
    let (sa, ca) = azi.to_radians().sin_cos();
    [s * sa, s * ca]
}

/// A leaf of consecutive feature segments, with its box, centre, radius and the chord distance
/// to the nearest other target run.
struct FLeaf {
    first: usize,
    len: usize,
    center: Coord,
    radius: f64,
    env: AABB<[f64; 3]>,
    lb: f64,
}

/// A node of a run tree in the best-first box search.
enum Node<'a> {
    Parent(&'a ParentNode<Box3>),
    Leaf(&'a Box3),
}

struct Queued<'a> {
    d2: f64,
    node: Node<'a>,
}

impl PartialEq for Queued<'_> {
    fn eq(&self, o: &Self) -> bool {
        self.d2.total_cmp(&o.d2) == Ordering::Equal
    }
}
impl Eq for Queued<'_> {}
impl PartialOrd for Queued<'_> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Queued<'_> {
    /// Reversed, so the heap pops the nearest first.
    fn cmp(&self, o: &Self) -> Ordering {
        o.d2.total_cmp(&self.d2)
    }
}

/// A disc holding every point of some consecutive segments: the middle segment's first vertex,
/// and the distance to the farthest vertex plus half the longest segment (a point of a segment is
/// within half its length of one of its ends).
fn disc(
    verts: &[Coord],
    segs: impl ExactSizeIterator<Item = (usize, usize, f64)> + Clone,
    work: &mut Work,
) -> Result<Option<(Coord, f64)>, Cancelled> {
    let n = segs.len();
    let Some(center) = segs.clone().nth(n / 2).and_then(|(a, _, _)| verts.get(a)).copied() else { return Ok(None) };
    let mut radius = 0.0f64;
    let mut longest = 0.0f64;
    let mut last: Option<usize> = None;
    for (a, b, len) in segs {
        for v in [a, b] {
            if last == Some(v) {
                continue;
            }
            work.add(GEO)?;
            if let Some(c) = verts.get(v) {
                radius = radius.max(geodesic_m(center, *c));
            }
            last = Some(v);
        }
        longest = longest.max(len);
    }
    Ok(Some((center, radius + longest / 2.0)))
}

/// The geodesic distance between two points (m).
fn geodesic_m(a: Coord, b: Coord) -> f64 {
    wgs84().inverse(a.y, a.x, b.y, b.x)
}

/// The per-feature search over [`Targets`], with a projection cache.
struct Search<'t, 'a> {
    t: &'t Targets<'a>,
    /// Target vertex index → its position in the current projection.
    cache: HashMap<usize, [f64; 2]>,
    /// Each target leaf's centre and radius, computed when first needed.
    leaf_disc: Vec<Option<(Coord, f64)>>,
    /// Each target run's centre and radius, computed when first needed.
    run_disc: Vec<Option<(Coord, f64)>>,
}

impl<'t, 'a> Search<'t, 'a> {
    fn new(t: &'t Targets<'a>) -> Self {
        Search { t, cache: HashMap::new(), leaf_disc: Vec::new(), run_disc: Vec::new() }
    }

    fn nearest(&mut self, g: &Geometry, skip: Option<usize>, work: &mut Work) -> Result<Option<Nearest>, Cancelled> {
        if let Some(row) = self.touching(g, skip, work)? {
            return Ok(Some(Nearest { distance_m: 0.0, target_row: row }));
        }
        let dense = Dense::of(g, work)?;
        let mut best = Best::NONE;
        if let [v] = dense.verts.as_slice() {
            self.near_vertex(*v, skip, &mut best, work)?;
            return Ok(best.result());
        }
        // The feature's segments; a multipoint's points are zero-length segments.
        let fsegs: Vec<(usize, usize, f64)> =
            if dense.segs.is_empty() { (0..dense.verts.len()).map(|i| (i, i, 0.0)).collect() } else { dense.segs.clone() };
        let pos: Vec<[f64; 3]> = dense.verts.iter().map(|c| ecef(*c)).collect();
        let mut leaves: Vec<FLeaf> = Vec::new();
        for (k, chunk) in fsegs.chunks(LEAF).enumerate() {
            let Some(&(mid, _, _)) = chunk.get(chunk.len() / 2) else { continue };
            let Some(&center) = dense.verts.get(mid) else { continue };
            let mut env: Option<AABB<[f64; 3]>> = None;
            let mut radius = 0.0f64;
            let mut longest = 0.0f64;
            for &(a, b, len) in chunk {
                let (Some(pa), Some(pb), Some(va), Some(vb)) = (pos.get(a), pos.get(b), dense.verts.get(a), dense.verts.get(b)) else {
                    continue;
                };
                let (lo, hi) = seg_box(*pa, *pb, len);
                let e = AABB::from_corners(lo, hi);
                env = Some(env.map_or(e, |mut x| {
                    x.merge(&e);
                    x
                }));
                work.add(2 * GEO)?;
                radius = radius.max(geodesic_m(center, *va)).max(geodesic_m(center, *vb));
                longest = longest.max(len);
            }
            let Some(env) = env else { continue };
            let lb = self.run_gap(&env, skip, work)?;
            leaves.push(FLeaf { first: k * LEAF, len: chunk.len(), center, radius: radius + longest / 2.0, env, lb });
        }
        let Some(closest) = leaves.iter().filter(|l| l.lb.is_finite()).min_by(|a, b| a.lb.total_cmp(&b.lb)) else { return Ok(None) };
        // A good first distance: from the centre of the closest leaf.
        let seed = closest.center;
        self.near_vertex(seed, skip, &mut best, work)?;
        // Groups of consecutive leaves, each with a disc, so a whole group far from every target
        // run (by the triangle inequality) is skipped at once.
        let mut groups: Vec<(f64, Vec<FLeaf>, Coord, f64, AABB<[f64; 3]>)> = Vec::new();
        let mut it = leaves.into_iter().peekable();
        while it.peek().is_some() {
            let group: Vec<FLeaf> = it.by_ref().take(GROUP).collect();
            let Some(mid) = group.get(group.len() / 2) else { continue };
            let center = mid.center;
            let mut env = mid.env;
            let mut radius = 0.0f64;
            for l in &group {
                work.add(GEO)?;
                radius = radius.max(geodesic_m(center, l.center) + l.radius);
                env.merge(&l.env);
            }
            let lb = group.iter().map(|l| l.lb).fold(f64::INFINITY, f64::min);
            groups.push((lb, group, center, radius, env));
        }
        groups.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (lb, mut group, center, radius, env) in groups {
            if lb > best.d || best.d == 0.0 {
                break;
            }
            if self.near_runs(&env, center, radius, skip, best.d, work)?.is_empty() {
                continue;
            }
            group.sort_by(|a, b| a.lb.total_cmp(&b.lb));
            for leaf in &group {
                if leaf.lb > best.d || best.d == 0.0 {
                    break;
                }
                self.near_leaf(leaf, &dense.verts, &fsegs, skip, &mut best, work)?;
            }
        }
        Ok(best.result())
    }

    /// The target runs of other rows that can hold a point within `best` of a disc (`center`,
    /// `radius`) whose points lie in `env`: inside `env` grown by `best`, and passing the triangle
    /// bound.
    fn near_runs(
        &mut self,
        env: &AABB<[f64; 3]>,
        center: Coord,
        radius: f64,
        skip: Option<usize>,
        best: f64,
        work: &mut Work,
    ) -> Result<Vec<usize>, Cancelled> {
        let t = self.t;
        let env = grown(env, best);
        let mut found: Vec<usize> = Vec::new();
        for tree in &t.run_trees {
            if tree.root().envelope().intersects(&env) {
                found.extend(tree.locate_in_envelope_intersecting(&env).map(|e| e.data));
            }
        }
        work.add(TREE * found.len())?;
        let mut out = Vec::new();
        for id in found {
            if t.runs.get(id).is_none_or(|r| Some(r.row) == skip) {
                continue;
            }
            let Some((c, r)) = self.run_disc(id, work)? else { continue };
            work.add(GEO)?;
            if geodesic_m(center, c) - radius - r <= best {
                out.push(id);
            }
        }
        Ok(out)
    }

    /// Target run `id`'s centre (its middle segment's first vertex) and radius.
    fn run_disc(&mut self, id: usize, work: &mut Work) -> Result<Option<(Coord, f64)>, Cancelled> {
        if self.run_disc.len() < self.t.runs.len() {
            self.run_disc.resize(self.t.runs.len(), None);
        }
        if let Some(Some(d)) = self.run_disc.get(id) {
            return Ok(Some(*d));
        }
        let t = self.t;
        let Some(run) = t.runs.get(id) else { return Ok(None) };
        let d = disc(&t.verts, t.segs.iter().skip(run.first).take(run.len).map(|s| (s.a, s.b, s.len)), work)?;
        if let (Some(slot), Some(d)) = (self.run_disc.get_mut(id), d) {
            *slot = Some(d);
        }
        Ok(d)
    }

    /// The chord distance from `env` to the nearest target run of another row (∞ if none).
    fn run_gap(&self, env: &AABB<[f64; 3]>, skip: Option<usize>, work: &mut Work) -> Result<f64, Cancelled> {
        let mut heap: BinaryHeap<Queued> =
            self.t.run_trees.iter().map(|t| Queued { d2: box_gap2(&t.root().envelope(), env), node: Node::Parent(t.root()) }).collect();
        while let Some(q) = heap.pop() {
            work.add(TREE)?;
            match q.node {
                Node::Leaf(b) => {
                    if self.t.runs.get(b.data).is_some_and(|r| Some(r.row) != skip) {
                        return Ok(lower_bound_m(q.d2));
                    }
                }
                Node::Parent(p) => {
                    for ch in p.children() {
                        heap.push(match ch {
                            RTreeNode::Leaf(b) => Queued { d2: box_gap2(&b.envelope(), env), node: Node::Leaf(b) },
                            RTreeNode::Parent(c) => Queued { d2: box_gap2(&c.envelope(), env), node: Node::Parent(c) },
                        });
                    }
                }
            }
        }
        Ok(f64::INFINITY)
    }

    /// Target leaf `id`'s centre (its middle segment's first vertex) and radius: the farthest
    /// vertex plus half the longest segment, so every point of the leaf is within it.
    fn leaf_disc(&mut self, id: usize, work: &mut Work) -> Result<Option<(Coord, f64)>, Cancelled> {
        if self.leaf_disc.len() < self.t.leaves.len() {
            self.leaf_disc.resize(self.t.leaves.len(), None);
        }
        if let Some(Some(d)) = self.leaf_disc.get(id) {
            return Ok(Some(*d));
        }
        let t = self.t;
        let Some(leaf) = t.leaves.get(id) else { return Ok(None) };
        let d = disc(&t.verts, t.segs.iter().skip(leaf.first).take(leaf.len).map(|s| (s.a, s.b, s.len)), work)?;
        if let (Some(slot), Some(d)) = (self.leaf_disc.get_mut(id), d) {
            *slot = Some(d);
        }
        Ok(d)
    }

    /// The lowest target row the geometry touches (planar lon/lat, exact), if any.
    fn touching(&self, g: &Geometry, skip: Option<usize>, work: &mut Work) -> Result<Option<usize>, Cancelled> {
        let t = self.t;
        let mut best: Option<usize> = None;
        let wanted = |row: usize, best: Option<usize>| Some(row) != skip && best.is_none_or(|b| row < b);
        // A vertex of each feature part in or on a target polygon.
        let mut probes: Vec<Coord> = Vec::new();
        match g {
            Geometry::Point(c) => probes.push(*c),
            Geometry::MultiPoint(v) => probes.extend_from_slice(v),
            Geometry::Line(parts) => probes.extend(parts.iter().filter_map(|p| p.first())),
            Geometry::Polygon(polys) => probes.extend(polys.iter().filter_map(|r| r.first().and_then(|o| o.first()))),
        }
        for p in &probes {
            let env = AABB::from_point([p.x, p.y]);
            for tree in &t.part_trees {
                for e in tree.locate_in_envelope_intersecting(&env) {
                    let Some(part) = t.parts.get(e.data) else { continue };
                    if !wanted(part.row, best) {
                        continue;
                    }
                    if let PartKind::Area(rings) = part.kind {
                        work.add(rings.iter().map(Vec::len).sum())?;
                        if locate_in_polygon(rings, *p).touches() {
                            best = Some(part.row);
                        }
                    }
                }
            }
        }
        // A vertex of each target part in or on the feature's polygons.
        if let Geometry::Polygon(polys) = g {
            let size: usize = polys.iter().flatten().map(Vec::len).sum();
            for rings in polys {
                let Some(outer) = rings.first().filter(|o| !o.is_empty()) else { continue };
                let mut env = box2(outer[0], outer[0]);
                outer.iter().for_each(|c| env.merge(&AABB::from_point([c.x, c.y])));
                for tree in &t.part_trees {
                    for e in tree.locate_in_envelope_intersecting(&env) {
                        let Some(part) = t.parts.get(e.data) else { continue };
                        if !wanted(part.row, best) {
                            continue;
                        }
                        work.add(size)?;
                        if part.kind.probe().is_some_and(|p| locate_in_area(polys, p).touches()) {
                            best = Some(part.row);
                        }
                    }
                }
            }
        }
        // Edges that meet.
        let mut err = Ok(());
        edges_of(g, |a, b| {
            if err.is_err() {
                return;
            }
            let env = box2(a, b);
            for tree in &t.edge_trees {
                for e in tree.locate_in_envelope_intersecting(&env) {
                    let Some(&(c, d, row)) = t.edges.get(e.data) else { continue };
                    if !wanted(row, best) {
                        continue;
                    }
                    if let Err(x) = work.add(1) {
                        err = Err(x);
                        return;
                    }
                    if segments_intersect(a, b, c, d) {
                        best = Some(row);
                    }
                }
            }
        });
        err?;
        Ok(best)
    }

    /// Target vertex `i` projected around `center`, cached for the current centre.
    fn project(&mut self, center: Coord, i: usize, work: &mut Work) -> Result<[f64; 2], Cancelled> {
        if let Some(p) = self.cache.get(&i) {
            return Ok(*p);
        }
        work.add(GEO)?;
        let p = self.t.verts.get(i).map_or([f64::INFINITY; 2], |c| aeqd(center, *c));
        self.cache.insert(i, p);
        Ok(p)
    }

    /// The padded normal box of target segment `i` (for runs without a tree).
    fn seg_envelope(&self, i: usize) -> Option<AABB<[f64; 3]>> {
        let seg = self.t.segs.get(i)?;
        let (a, b) = (self.t.verts.get(seg.a)?, self.t.verts.get(seg.b)?);
        let (lo, hi) = seg_box(ecef(*a), ecef(*b), seg.len);
        Some(AABB::from_corners(lo, hi))
    }

    /// Nearest target segments to vertex `v`, best first, until the lower bound passes `best`:
    /// first the runs, then the segments of each run.
    fn near_vertex(&mut self, v: Coord, skip: Option<usize>, best: &mut Best, work: &mut Work) -> Result<(), Cancelled> {
        let t = self.t;
        let nv = ecef(v);
        let mut order: Vec<(f64, &RTree<Box3>)> = t.run_trees.iter().map(|tr| (tr.root().envelope().distance_2(&nv), tr)).collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.cache.clear();
        for (d2, tree) in order {
            if lower_bound_m(d2) > best.d {
                break;
            }
            for (e, d2) in tree.nearest_neighbor_iter_with_distance_2(&nv) {
                work.add(TREE)?;
                if lower_bound_m(d2) > best.d {
                    break;
                }
                let Some(run) = t.runs.get(e.data) else { continue };
                if Some(run.row) == skip {
                    continue;
                }
                match &run.tree {
                    Some(rt) => {
                        for (se, d2) in rt.nearest_neighbor_iter_with_distance_2(&nv) {
                            work.add(TREE)?;
                            if lower_bound_m(d2) > best.d {
                                break;
                            }
                            self.vertex_to_seg(v, se.data, run.row, best, work)?;
                        }
                    }
                    None => {
                        for i in run.first..run.first + run.len {
                            work.add(TREE)?;
                            if self.seg_envelope(i).is_some_and(|env| lower_bound_m(env.distance_2(&nv)) <= best.d) {
                                self.vertex_to_seg(v, i, run.row, best, work)?;
                            }
                        }
                    }
                }
                if best.d == 0.0 {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// Offers the distance from `v` to target segment `i` (of target `row`).
    fn vertex_to_seg(&mut self, v: Coord, i: usize, row: usize, best: &mut Best, work: &mut Work) -> Result<(), Cancelled> {
        let Some(&seg) = self.t.segs.get(i) else { return Ok(()) };
        let a = self.project(v, seg.a, work)?;
        let b = if seg.b == seg.a { a } else { self.project(v, seg.b, work)? };
        best.offer(origin_to_seg(a, b), row);
        Ok(())
    }

    /// Every pair of a feature leaf's segments and the target segments that can beat `best`:
    /// target leaves inside the leaf's box grown by `best` that pass the triangle bound.
    #[allow(clippy::too_many_arguments)]
    fn near_leaf(
        &mut self,
        fl: &FLeaf,
        verts: &[Coord],
        fsegs: &[(usize, usize, f64)],
        skip: Option<usize>,
        best: &mut Best,
        work: &mut Work,
    ) -> Result<(), Cancelled> {
        let t = self.t;
        let env = grown(&fl.env, best.d);
        let mut cands: Vec<usize> = Vec::new();
        for id in self.near_runs(&fl.env, fl.center, fl.radius, skip, best.d, work)? {
            let Some(run) = t.runs.get(id) else { continue };
            let leaf_of = |i: usize| run.leaf0 + (i - run.first) / LEAF;
            match &run.tree {
                Some(rt) => cands.extend(rt.locate_in_envelope_intersecting(&env).map(|se| leaf_of(se.data))),
                None => cands.extend(
                    (run.first..run.first + run.len).filter(|&i| self.seg_envelope(i).is_some_and(|b| b.intersects(&env))).map(leaf_of),
                ),
            }
        }
        work.add(TREE * cands.len())?;
        cands.sort_unstable();
        cands.dedup();
        // The triangle inequality: d(x, y) ≥ d(centres) − r_feature − r_target.
        let mut near: Vec<(f64, usize)> = Vec::new();
        for id in cands {
            let Some((c, r)) = self.leaf_disc(id, work)? else { continue };
            work.add(GEO)?;
            let lb = geodesic_m(fl.center, c) - fl.radius - r;
            if lb <= best.d {
                near.push((lb, id));
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(a, b, _) in fsegs.iter().skip(fl.first).take(fl.len) {
            let (Some(&p), Some(&q)) = (verts.get(a), verts.get(b)) else { continue };
            self.cache.clear();
            work.add(GEO)?;
            let qp = if a == b { [0.0, 0.0] } else { aeqd(p, q) };
            for &(lb, id) in &near {
                if lb > best.d {
                    break;
                }
                let Some(leaf) = t.leaves.get(id) else { continue };
                let Some(row) = t.runs.get(leaf.run).map(|r| r.row) else { continue };
                for i in leaf.first..leaf.first + leaf.len {
                    let Some(&seg) = t.segs.get(i) else { continue };
                    let c = self.project(p, seg.a, work)?;
                    let d = if seg.b == seg.a { c } else { self.project(p, seg.b, work)? };
                    best.offer(seg_to_seg([0.0, 0.0], qp, c, d), row);
                    if best.d == 0.0 {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
