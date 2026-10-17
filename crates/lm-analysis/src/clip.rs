//! "Keep the part of [layer] inside [other]" and "Remove the part of [layer] inside [other]"
//! (puzzles › mechanics › A2 `keep_inside`, `remove_inside`; upstream `clip` and `erase` in
//! `rustgis-analysis::tools`).
//!
//! # Definition
//!
//! - **Mask.** The union of the polygons of every feature of the mask layer, with edges taken as
//!   straight lines in longitude/latitude (the convention the app draws with). Rings are oriented
//!   (exteriors counter-clockwise, holes clockwise) before the union, as upstream does, because
//!   the overlay reads its fill rule from the winding; the union is made once
//!   (`geo::unary_union`, one sweep over all rings) and its polygons go into an R-tree of
//!   bounding boxes, so each feature meets only the mask polygons near it.
//! - **Areas.** Keep: the intersection with the mask; remove: the difference. Holes and
//!   multipolygons are handled by `geo::BooleanOps` (i_overlay) with the non-zero fill rule on
//!   oriented rings, so overlapping parts of one feature count once.
//! - **Lines.** Keep: the parts inside the mask; remove: the parts outside (`geo`'s `clip` of a
//!   MultiLineString, `invert` for remove). Where a line runs exactly along the mask's edge,
//!   that stretch is decided by `geo`'s clip (it is kept by keep and dropped by remove; see the
//!   tests), not by a separate rule.
//! - **Points.** A point inside the mask or on its boundary is kept by keep and removed by
//!   remove, decided exactly (`overlay::locate_in_area`, Shewchuk's predicate) against the mask
//!   layer's own polygons, which is the same set as their union. A multipoint keeps the points
//!   that remain and stays a multipoint.
//! - **Attributes** are copied, and each output feature keeps its source feature's id;
//!   [`ClipResult::source_rows`] gives each output row's input row.
//! - **Empty results** (nothing left, or no geometry to begin with) are dropped and counted in
//!   [`ClipResult::dropped`].
//!
//! # Precision
//!
//! `geo`'s boolean operations (i_overlay) compute on integers: coordinates are snapped to a grid
//! of `2^-(29 − ⌊log₂ h⌋)` degrees, `h` being half the larger side of the bounding box of the
//! feature and the mask polygons near it (3.7e-9° for a span of 4°, 3e-8° for China). Every
//! output vertex within two grid steps of an input vertex gets that vertex's exact coordinates
//! back, so vertices that come from the inputs are exact; only new crossing points keep the
//! grid's rounding, at most half a step in each coordinate. An area is then off by at most
//! about one step times the length of the new edges: well under 1e-6 relative for any feature
//! wider than a few hundred grid steps, but a long sliver whose sides are new edges can be
//! off by more than 1 m² (a sliver along 100 km of a China-sized mask: up to a few hundred m²).
//!
//! # Antimeridian
//!
//! Nothing wraps: coordinates are used as given, edges straight in longitude/latitude, so a
//! polygon written from 179° to 181° and one from −180° to −179° are on opposite sides of the
//! plane and don't meet. A mask that crosses the antimeridian must be split there (as Natural
//! Earth and GeoJSON (RFC 7946) data are). Such inputs never make the action fail or panic.
//!
//! # Speed
//!
//! The control is checked before the union and before every feature; progress is reported as
//! features done of features total.

use geo::bool_ops::FillRule;
use geo::orient::Direction;
use geo::{
    BooleanOps, BoundingRect, Coord as GC, CoordsIter, LineString, MapCoordsInPlace, MultiLineString, MultiPolygon, OpType, Orient, Polygon,
};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree, RTreeObject};
use rustgis_data::{Coord, Dataset, Feature, Geometry};
use std::collections::HashMap;

use crate::control::{Cancelled, Control};
use crate::overlay::{Location, locate_in_area};

const PHASE_MASK: &str = "Merging the areas of the mask";
const PHASE_KEEP: &str = "Keeping the part inside";
const PHASE_REMOVE: &str = "Removing the part inside";

/// The clipped layer.
#[derive(Clone, Debug)]
pub struct ClipResult {
    /// Same name, fields and geometry type as the input layer.
    pub dataset: Dataset,
    /// Input features left with nothing (or with no geometry to begin with).
    pub dropped: u64,
    /// For each output feature, its row in the input layer.
    pub source_rows: Vec<usize>,
}

/// Keep the part of each feature of `layer` inside the union of `mask`'s polygons (edges straight in lon/lat). Points: kept if inside or on the boundary.
pub fn keep_inside(layer: &Dataset, mask: &Dataset, ctl: &Control) -> Result<ClipResult, Cancelled> {
    run(layer, mask, true, ctl)
}

/// Remove the part inside (difference). Points: removed if inside or on the boundary.
pub fn remove_inside(layer: &Dataset, mask: &Dataset, ctl: &Control) -> Result<ClipResult, Cancelled> {
    run(layer, mask, false, ctl)
}

type Indexed = GeomWithData<Rectangle<[f64; 2]>, usize>;

/// The mask: its union (for areas and lines) and its source polygons (for points), indexed.
struct Mask {
    union: Vec<Polygon<f64>>,
    union_index: RTree<Indexed>,
    source: Vec<Vec<Vec<Coord>>>,
    source_index: RTree<Indexed>,
}

impl Mask {
    fn build(mask: &Dataset, ctl: &Control) -> Result<Mask, Cancelled> {
        ctl.report(PHASE_MASK, 0, None);
        let mut polys = Vec::new();
        let mut source = Vec::new();
        for (i, f) in mask.features.iter().enumerate() {
            if i % 1024 == 0 {
                ctl.check()?;
            }
            let Some(Geometry::Polygon(parts)) = &f.geometry else { continue };
            for rings in parts {
                if let Some(p) = to_polygon(rings) {
                    polys.push(p.orient(Direction::Default));
                    source.push(rings.iter().map(|r| r.iter().copied().filter(|c| c.is_finite()).collect()).collect());
                }
            }
        }
        ctl.check()?;
        let union: Vec<Polygon<f64>> = if polys.is_empty() { Vec::new() } else { geo::unary_union(polys.iter()).0 };
        ctl.check()?;
        let union_index = RTree::bulk_load(
            union.iter().enumerate().filter_map(|(i, p)| rect(p.bounding_rect()?).map(|r| GeomWithData::new(r, i))).collect(),
        );
        let source_index = RTree::bulk_load(
            source
                .iter()
                .enumerate()
                .filter_map(|(i, rings): (usize, &Vec<Vec<Coord>>)| {
                    let outer = rings.first()?;
                    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
                    for c in outer {
                        lo = [lo[0].min(c.x), lo[1].min(c.y)];
                        hi = [hi[0].max(c.x), hi[1].max(c.y)];
                    }
                    (lo[0] <= hi[0]).then(|| GeomWithData::new(Rectangle::from_corners(lo, hi), i))
                })
                .collect(),
        );
        Ok(Mask { union, union_index, source, source_index })
    }

    /// The union's polygons whose boxes meet `bb`.
    fn near(&self, bb: AABB<[f64; 2]>) -> MultiPolygon<f64> {
        let mut idx: Vec<usize> = self.union_index.locate_in_envelope_intersecting(&bb).map(|g| g.data).collect();
        idx.sort_unstable();
        MultiPolygon::new(idx.into_iter().filter_map(|i| self.union.get(i).cloned()).collect())
    }

    /// Inside or on the boundary of the mask.
    fn touches(&self, c: Coord) -> bool {
        self.source_index
            .locate_all_at_point(&[c.x, c.y])
            .any(|g| self.source.get(g.data).is_some_and(|rings| locate_in_area(std::slice::from_ref(rings), c) != Location::Outside))
    }
}

fn run(layer: &Dataset, mask: &Dataset, keep: bool, ctl: &Control) -> Result<ClipResult, Cancelled> {
    let m = Mask::build(mask, ctl)?;
    let phase = if keep { PHASE_KEEP } else { PHASE_REMOVE };
    let total = layer.features.len() as u64;
    let mut out = Dataset::new(layer.name.clone(), layer.geometry_type, layer.fields.clone());
    let mut dropped = 0u64;
    let mut source_rows = Vec::new();
    ctl.report(phase, 0, Some(total));
    for (row, f) in layer.features.iter().enumerate() {
        ctl.check()?;
        let g = f.geometry.as_ref().and_then(|g| clip_one(g, &m, keep));
        match g {
            Some(g) => {
                out.features.push(Feature { id: f.id, geometry: Some(g), attrs: f.attrs.clone() });
                source_rows.push(row);
            }
            None => dropped += 1,
        }
        if row % 256 == 255 {
            ctl.report(phase, row as u64 + 1, Some(total));
        }
    }
    ctl.report(phase, total, Some(total));
    Ok(ClipResult { dataset: out, dropped, source_rows })
}

/// What is left of one geometry; None when nothing is.
fn clip_one(g: &Geometry, m: &Mask, keep: bool) -> Option<Geometry> {
    match g {
        Geometry::Polygon(parts) => {
            let polys: Vec<Polygon<f64>> = parts.iter().filter_map(|r| to_polygon(r)).map(|p| p.orient(Direction::Default)).collect();
            let subject = MultiPolygon::new(polys);
            let bb = rect(subject.bounding_rect()?)?.envelope();
            let near = m.near(bb);
            if near.0.is_empty() {
                return if keep { None } else { Some(g.clone()) };
            }
            let op = if keep { OpType::Intersection } else { OpType::Difference };
            let mut out = subject.boolean_op_with_fill_rule(&near, op, FillRule::NonZero);
            let snap = Snap::new(subject.coords_iter(), &near, bb);
            out.map_coords_in_place(|c| snap.fix(c));
            from_multipolygon(&out)
        }
        Geometry::Line(parts) => {
            let lines: Vec<LineString<f64>> = parts
                .iter()
                .map(|p| LineString::new(p.iter().filter(|c| c.is_finite()).map(|c| GC { x: c.x, y: c.y }).collect()))
                .filter(|l: &LineString<f64>| l.0.len() >= 2)
                .collect();
            let subject = MultiLineString::new(lines);
            let bb = rect(subject.bounding_rect()?)?.envelope();
            let near = m.near(bb);
            if near.0.is_empty() {
                return if keep { None } else { from_lines(&subject) };
            }
            let mut out = near.clip_with_fill_rule(&subject, !keep, FillRule::NonZero);
            let snap = Snap::new(subject.coords_iter(), &near, bb);
            out.map_coords_in_place(|c| snap.fix(c));
            from_lines(&out)
        }
        Geometry::Point(c) => (c.is_finite() && m.touches(*c) == keep).then_some(Geometry::Point(*c)),
        Geometry::MultiPoint(pts) => {
            let left: Vec<Coord> = pts.iter().copied().filter(|c| c.is_finite() && m.touches(*c) == keep).collect();
            (!left.is_empty()).then_some(Geometry::MultiPoint(left))
        }
    }
}

/// Puts back input vertices that the overlay moved. `geo`'s boolean operations (i_overlay)
/// work on integers: every coordinate is snapped to a grid of `2^-(29 − ⌊log₂ h⌋)` (`h` half the
/// larger side of the inputs' bounding box; about 4e-9° for a span of 4°). An output vertex within
/// two grid steps of an input vertex (of the feature, or of the mask near it) is that vertex, so
/// it gets its exact coordinates back; only new crossing points keep the grid's rounding.
struct Snap {
    tol: f64,
    cells: HashMap<(i64, i64), Vec<GC<f64>>>,
}

impl Snap {
    fn new(subject: impl Iterator<Item = GC<f64>>, near: &MultiPolygon<f64>, bb: AABB<[f64; 2]>) -> Snap {
        let subject: Vec<GC<f64>> = subject.collect();
        let (mut lo, mut hi) = (bb.lower(), bb.upper());
        for c in near.coords_iter() {
            lo = [lo[0].min(c.x), lo[1].min(c.y)];
            hi = [hi[0].max(c.x), hi[1].max(c.y)];
        }
        let half = 0.5 * (hi[0] - lo[0]).max(hi[1] - lo[1]);
        let step = if half > 0.0 && half.is_finite() { (half.log2().floor() - 29.0).exp2() } else { 0.0 };
        let tol = 2.0 * step;
        let mut cells: HashMap<(i64, i64), Vec<GC<f64>>> = HashMap::new();
        if tol > 0.0 {
            let (blo, bhi) = (bb.lower(), bb.upper());
            let inside = |c: &GC<f64>| c.x >= blo[0] - tol && c.x <= bhi[0] + tol && c.y >= blo[1] - tol && c.y <= bhi[1] + tol;
            for c in subject.into_iter().chain(near.coords_iter().filter(inside)) {
                cells.entry(((c.x / tol).floor() as i64, (c.y / tol).floor() as i64)).or_default().push(c);
            }
        }
        Snap { tol, cells }
    }

    fn fix(&self, c: GC<f64>) -> GC<f64> {
        if self.tol == 0.0 {
            return c;
        }
        let (kx, ky) = ((c.x / self.tol).floor() as i64, (c.y / self.tol).floor() as i64);
        let mut best = (self.tol, c);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &v in self.cells.get(&(kx + dx, ky + dy)).into_iter().flatten() {
                    let d = (v.x - c.x).abs().max((v.y - c.y).abs());
                    if d <= best.0 {
                        best = (d, v);
                    }
                }
            }
        }
        best.1
    }
}

/// A polygon from rings (first = exterior), finite coordinates only; None without an exterior.
fn to_polygon(rings: &[Vec<Coord>]) -> Option<Polygon<f64>> {
    let ls = |r: &Vec<Coord>| -> Option<LineString<f64>> {
        let pts: Vec<GC<f64>> = r.iter().filter(|c| c.is_finite()).map(|c| GC { x: c.x, y: c.y }).collect();
        (pts.len() >= 3).then(|| LineString::new(pts))
    };
    let mut it = rings.iter();
    let outer = ls(it.next()?)?;
    Some(Polygon::new(outer, it.filter_map(ls).collect()))
}

fn from_multipolygon(mp: &MultiPolygon<f64>) -> Option<Geometry> {
    let back = |l: &LineString<f64>| -> Vec<Coord> {
        let mut v: Vec<Coord> = l.0.iter().map(|c| Coord::new(c.x, c.y)).collect();
        v.dedup();
        if v.len() > 1
            && v.first() != v.last()
            && let Some(&f) = v.first()
        {
            v.push(f);
        }
        v
    };
    let mut polys: Vec<Vec<Vec<Coord>>> = Vec::with_capacity(mp.0.len());
    for p in &mp.0 {
        let outer = back(p.exterior());
        if outer.len() < 4 {
            continue;
        }
        polys.push(std::iter::once(outer).chain(p.interiors().iter().map(back).filter(|r| r.len() >= 4)).collect());
    }
    (!polys.is_empty()).then_some(Geometry::Polygon(polys))
}

fn from_lines(ml: &MultiLineString<f64>) -> Option<Geometry> {
    let parts: Vec<Vec<Coord>> =
        ml.0.iter()
            .map(|l| {
                let mut v: Vec<Coord> = l.0.iter().map(|c| Coord::new(c.x, c.y)).collect();
                v.dedup();
                v
            })
            .filter(|v| v.len() >= 2)
            .collect();
    (!parts.is_empty()).then_some(Geometry::Line(parts))
}

fn rect(r: geo::Rect<f64>) -> Option<Rectangle<[f64; 2]>> {
    let (lo, hi) = (r.min(), r.max());
    [lo.x, lo.y, hi.x, hi.y].iter().all(|v| v.is_finite()).then(|| Rectangle::from_corners([lo.x, lo.y], [hi.x, hi.y]))
}

#[cfg(test)]
mod tests;
