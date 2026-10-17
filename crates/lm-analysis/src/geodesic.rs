//! Geodesic length, area and perimeter on the WGS 84 ellipsoid, with GeographicLib's Rust port
//! (`geographiclib-rs`), and their friendly formatting (spec › Phase 3 › WP3.1: "Add geodesic
//! length and area with GeographicLib's Rust port, tested against fixture N5's length and area
//! cases"; spec › Inspector: object card › Header: "Districts · area · 23.4 km²").
//!
//! Coordinates are `Coord { x: lon, y: lat }` in degrees. Edges between vertices are geodesics,
//! as in `pyproj.Geod`, which made fixture N5's expected values.
//!
//! **Polygons.** Each ring's area is computed signed and taken as an absolute value, so ring
//! orientation does not matter: the first ring of each polygon adds its area, every further ring
//! (a hole) subtracts its own. A multi-polygon sums its parts (each clamped at zero). Rings may be
//! closed (last vertex = first) or open. A ring is assumed to enclose less than half the Earth.
//!
//! **Antimeridian.** RFC 7946 splits a shape that crosses 180° into parts that meet along the
//! ±180° meridian. Lines need nothing special (the parts' lengths add up). For polygons the areas
//! add up, but the split adds an artificial seam to the perimeter, so an edge whose two ends both
//! lie exactly on ±180° is treated as seam and left out of the perimeter (not the area). An
//! unsplit edge that crosses 180° (179.5° → −179.5°) is fine too: geodesics take the short way.
//!
//! **Formatting.** Thousands separators always. Metric: length "640 m" (under 10 m one decimal),
//! from 1 km "3.2 km" (one decimal below 100 km, none above); area "8,200 m²" below 0.1 km²,
//! then "0.45 km²" (two decimals below 1 km²), "23.4 km²" (one below 100 km²), "1,234 km²".
//! Hectares are not used: the spec's microcopy uses m² and km² only. Imperial: length "640 ft"
//! below 1,000 ft, then miles ("0.42 mi" below 1 mi, "3.2 mi" below 100 mi, "1,234 mi"); area
//! "8,200 sq ft" below 1 acre, acres up to 1 mi² ("45.6 acres", "1.0 acre"), then "23.4 mi²".

use crate::insight::{format_count, format_number};
use geographiclib_rs::{Geodesic, InverseGeodesic, PolygonArea, Winding};
use rustgis_data::Geometry;
use rustgis_geom::Coord;
use std::sync::OnceLock;

fn wgs84() -> &'static Geodesic {
    static G: OnceLock<Geodesic> = OnceLock::new();
    G.get_or_init(Geodesic::wgs84)
}

/// Geodesic length of one polyline (metres). Non-finite vertices are skipped.
pub fn path_length_m(pts: &[Coord]) -> f64 {
    let g = wgs84();
    let mut prev: Option<Coord> = None;
    let mut total = 0.0;
    for c in pts.iter().copied().filter(|c| c.is_finite()) {
        if let Some(p) = prev {
            let s: f64 = g.inverse(p.y, p.x, c.y, c.x);
            total += s;
        }
        prev = Some(c);
    }
    total
}

/// Geodesic length of a multi-part line (metres): the sum of its parts.
pub fn line_length_m(parts: &[Vec<Coord>]) -> f64 {
    parts.iter().map(|p| path_length_m(p)).sum()
}

/// Area and perimeter of a shape.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AreaPerimeter {
    /// Square metres, never negative.
    pub area_m2: f64,
    /// Metres: every ring's boundary (holes included), without RFC 7946 seams.
    pub perimeter_m: f64,
}

/// A ring's vertices without the closing repeat and without non-finite points.
fn ring_vertices(ring: &[Coord]) -> Vec<Coord> {
    let mut v: Vec<Coord> = ring.iter().copied().filter(|c| c.is_finite()).collect();
    if v.len() > 1 && v.first() == v.last() {
        v.pop();
    }
    v
}

fn on_antimeridian(c: Coord) -> bool {
    c.x.abs() == 180.0
}

/// Unsigned area of one ring (m²) and its perimeter without seam edges (m).
pub fn ring_area_perimeter(ring: &[Coord]) -> AreaPerimeter {
    let v = ring_vertices(ring);
    if v.len() < 2 {
        return AreaPerimeter::default();
    }
    let g = wgs84();
    let mut perimeter = 0.0;
    for (i, a) in v.iter().enumerate() {
        let Some(b) = v.get((i + 1) % v.len()) else { continue };
        if on_antimeridian(*a) && on_antimeridian(*b) {
            continue;
        }
        let s: f64 = g.inverse(a.y, a.x, b.y, b.x);
        perimeter += s;
    }
    if v.len() < 3 {
        return AreaPerimeter { area_m2: 0.0, perimeter_m: perimeter };
    }
    let mut pa = PolygonArea::new(g, Winding::CounterClockwise);
    for c in &v {
        pa.add_point(c.y, c.x);
    }
    let (_, signed, _) = pa.compute(true);
    AreaPerimeter { area_m2: signed.abs(), perimeter_m: perimeter }
}

/// Area and perimeter of polygons of rings (first ring exterior, the rest holes), as held by
/// [`Geometry::Polygon`].
pub fn polygon_area_perimeter(polys: &[Vec<Vec<Coord>>]) -> AreaPerimeter {
    let mut out = AreaPerimeter::default();
    for rings in polys {
        let mut area = 0.0;
        for (k, ring) in rings.iter().enumerate() {
            let r = ring_area_perimeter(ring);
            area += if k == 0 { r.area_m2 } else { -r.area_m2 };
            out.perimeter_m += r.perimeter_m;
        }
        out.area_m2 += area.max(0.0);
    }
    out
}

/// What a geometry measures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Measure {
    /// Points have no size.
    None,
    /// Lines: total length (m).
    Length(f64),
    /// Areas.
    Area(AreaPerimeter),
}

/// Measures a geometry geodesically.
pub fn measure(g: &Geometry) -> Measure {
    match g {
        Geometry::Point(_) | Geometry::MultiPoint(_) => Measure::None,
        Geometry::Line(parts) => Measure::Length(line_length_m(parts)),
        Geometry::Polygon(polys) => Measure::Area(polygon_area_perimeter(polys)),
    }
}

/// Length of a line geometry (m); `None` for other kinds.
pub fn geometry_length_m(g: &Geometry) -> Option<f64> {
    if let Geometry::Line(parts) = g { Some(line_length_m(parts)) } else { None }
}

/// Area of a polygon geometry (m²); `None` for other kinds.
pub fn geometry_area_m2(g: &Geometry) -> Option<f64> {
    if let Geometry::Polygon(p) = g { Some(polygon_area_perimeter(p).area_m2) } else { None }
}

/// Unit system for sizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Units {
    /// Metres, kilometres, m², km².
    #[default]
    Metric,
    /// Feet, miles, sq ft, acres, mi².
    Imperial,
}

const M_PER_FT: f64 = 0.3048;
const M_PER_MI: f64 = 1609.344;
const M2_PER_ACRE: f64 = 4_046.856_422_4;
const M2_PER_MI2: f64 = M_PER_MI * M_PER_MI;

/// `x` with `d` fixed decimals and thousands separators.
fn fixed(x: f64, d: u8) -> String {
    format_number(x, Some(d))
}

/// One decimal below 100, none from 100 on.
fn one_then_none(x: f64) -> String {
    if format!("{x:.1}").parse::<f64>().unwrap_or(x) < 100.0 { fixed(x, 1) } else { fixed(x, 0) }
}

/// Friendly length: "640 m", "3.2 km", "640 ft", "3.2 mi" (see the module notes).
pub fn format_length(m: f64, units: Units) -> String {
    if !m.is_finite() || m < 0.0 {
        return crate::insight::NO_VALUE.to_string();
    }
    match units {
        Units::Metric => {
            if m < 999.5 {
                format!("{} m", if m < 9.95 { fixed(m, 1) } else { fixed(m, 0) })
            } else {
                format!("{} km", one_then_none(m / 1000.0))
            }
        }
        Units::Imperial => {
            let ft = m / M_PER_FT;
            if ft < 999.5 {
                format!("{} ft", fixed(ft, 0))
            } else {
                let mi = m / M_PER_MI;
                let s = if mi < 0.995 { fixed(mi, 2) } else { one_then_none(mi) };
                format!("{s} mi")
            }
        }
    }
}

/// Friendly area: "8,200 m²", "23.4 km²", "8,200 sq ft", "45.6 acres", "23.4 mi²".
pub fn format_area(m2: f64, units: Units) -> String {
    if !m2.is_finite() || m2 < 0.0 {
        return crate::insight::NO_VALUE.to_string();
    }
    match units {
        Units::Metric => {
            if m2 < 99_999.5 {
                format!("{} m²", if m2 < 9.95 { fixed(m2, 1) } else { fixed(m2, 0) })
            } else {
                let km2 = m2 / 1e6;
                let s = if km2 < 0.995 { fixed(km2, 2) } else { one_then_none(km2) };
                format!("{s} km²")
            }
        }
        Units::Imperial => {
            let acres = m2 / M2_PER_ACRE;
            if acres < 1.0 {
                format!("{} sq ft", format_count((m2 / (M_PER_FT * M_PER_FT)).round() as usize))
            } else if m2 < M2_PER_MI2 {
                let s = one_then_none(acres);
                format!("{s} {}", if s == "1.0" { "acre" } else { "acres" })
            } else {
                format!("{} mi²", one_then_none(m2 / M2_PER_MI2))
            }
        }
    }
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

    /// Relative tolerance against N5 (values printed to 12 significant digits).
    const REL: f64 = 1e-9;

    fn close(got: f64, want: f64) -> bool {
        (got - want).abs() <= REL * want.abs().max(1.0)
    }

    #[test]
    fn n5_measures() {
        let (ds, _) = rustgis_data::io::read_geojson(&fixture("N5/cases.geojson"), "cases").unwrap();
        let id_col = ds.fields.iter().position(|f| f.name == "feature_id").unwrap();
        let geoms: HashMap<String, Geometry> = ds
            .features
            .iter()
            .map(|f| {
                let rustgis_data::Value::Text(id) = &f.attrs[id_col] else { panic!("feature_id") };
                (id.clone(), f.geometry.clone().unwrap())
            })
            .collect();
        let csv = fixture("N5/expected_measure.csv");
        let mut lines = csv.lines().filter(|l| !l.trim().is_empty());
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        let mut checked = 0;
        for line in lines {
            let row: HashMap<&str, &str> = header.iter().copied().zip(line.split(',')).collect();
            let id = row["feature_id"];
            let g = &geoms[id];
            let num = |k: &str| row.get(k).filter(|s| !s.is_empty()).map(|s| s.parse::<f64>().unwrap());
            match (measure(g), num("length_m"), num("area_m2")) {
                (Measure::None, None, None) => {}
                (Measure::Length(l), Some(want), None) => {
                    assert!(close(l, want), "{id}: length {l} vs {want}");
                    checked += 1;
                }
                (Measure::Area(ap), None, Some(want)) => {
                    assert!(close(ap.area_m2, want), "{id}: area {} vs {want}", ap.area_m2);
                    if let Some(p) = num("perimeter_m") {
                        assert!(close(ap.perimeter_m, p), "{id}: perimeter {} vs {p}", ap.perimeter_m);
                    }
                    checked += 1;
                }
                (m, l, a) => panic!("{id}: {m:?} vs length {l:?} area {a:?}"),
            }
        }
        assert_eq!(checked, 6);
    }

    fn c(x: f64, y: f64) -> Coord {
        Coord::new(x, y)
    }

    fn square(x0: f64, y0: f64, s: f64) -> Vec<Coord> {
        vec![c(x0, y0), c(x0 + s, y0), c(x0 + s, y0 + s), c(x0, y0 + s), c(x0, y0)]
    }

    #[test]
    fn orientation_holes_and_closure() {
        let ccw = square(0.0, 0.0, 1.0);
        let mut cw = ccw.clone();
        cw.reverse();
        let a = ring_area_perimeter(&ccw);
        let b = ring_area_perimeter(&cw);
        assert!(close(a.area_m2, 12_308_778_361.469_452));
        assert!(close(a.area_m2, b.area_m2));
        assert!(close(a.perimeter_m, 443_770.917_248_302));
        // Open ring: same.
        let open = &ccw[..4];
        assert!(close(ring_area_perimeter(open).area_m2, a.area_m2));
        // Hole subtracted whatever its orientation (same as the outer ring here).
        let hole = square(0.25, 0.25, 0.5);
        let h = ring_area_perimeter(&hole).area_m2;
        for outer in [ccw.clone(), cw.clone()] {
            let p = polygon_area_perimeter(&[vec![outer, hole.clone()]]);
            assert!(close(p.area_m2, a.area_m2 - h));
            assert!(close(p.perimeter_m, a.perimeter_m + ring_area_perimeter(&hole).perimeter_m));
        }
        // Multi-polygon sums.
        let two = polygon_area_perimeter(&[vec![ccw.clone()], vec![square(10.0, 0.0, 1.0)]]);
        assert!(two.area_m2 > 1.99 * a.area_m2);
        // Degenerate.
        assert_eq!(polygon_area_perimeter(&[vec![vec![c(0.0, 0.0)]]]), AreaPerimeter::default());
        assert_eq!(measure(&Geometry::Point(c(1.0, 2.0))), Measure::None);
        assert_eq!(geometry_area_m2(&Geometry::Point(c(1.0, 2.0))), None);
    }

    #[test]
    fn antimeridian_split_matches_unsplit() {
        // poly_am from N5 written unsplit: the same vertices, including those on 180°, but one
        // ring whose edges cross 180° directly.
        let whole = vec![c(179.5, 10.0), c(180.0, 10.0), c(-179.5, 10.0), c(-179.5, 11.0), c(-180.0, 11.0), c(179.5, 11.0)];
        let w = ring_area_perimeter(&whole);
        let split = polygon_area_perimeter(&[
            vec![vec![c(180.0, 10.0), c(180.0, 11.0), c(179.5, 11.0), c(179.5, 10.0), c(180.0, 10.0)]],
            vec![vec![c(-179.5, 10.0), c(-179.5, 11.0), c(-180.0, 11.0), c(-180.0, 10.0), c(-179.5, 10.0)]],
        ]);
        assert!(close(split.area_m2, w.area_m2), "{} vs {}", split.area_m2, w.area_m2);
        assert!(close(split.perimeter_m, w.perimeter_m), "{} vs {}", split.perimeter_m, w.perimeter_m);
        assert!(close(w.area_m2, 12_108_258_313.0));
        // Lines: split and unsplit agree.
        let l1 = line_length_m(&[vec![c(179.5, 10.0), c(180.0, 10.0)], vec![c(-180.0, 10.0), c(-179.5, 10.0)]]);
        let l2 = path_length_m(&[c(179.5, 10.0), c(180.0, 10.0), c(-179.5, 10.0)]);
        assert!(close(l1, l2), "{l1} vs {l2}");
    }

    #[test]
    fn formatting() {
        let m = Units::Metric;
        let i = Units::Imperial;
        assert_eq!(format_length(640.4, m), "640 m");
        assert_eq!(format_length(3.24, m), "3.2 m");
        assert_eq!(format_length(999.4, m), "999 m");
        assert_eq!(format_length(999.6, m), "1.0 km");
        assert_eq!(format_length(3_200.0, m), "3.2 km");
        assert_eq!(format_length(99_960.0, m), "100 km");
        assert_eq!(format_length(1_234_000.0, m), "1,234 km");
        assert_eq!(format_length(195.0, i), "640 ft");
        assert_eq!(format_length(675.0, i), "0.42 mi");
        assert_eq!(format_length(5_150.0, i), "3.2 mi");
        assert_eq!(format_length(f64::NAN, i), "No value");
        assert_eq!(format_area(8_213.0, m), "8,213 m²");
        assert_eq!(format_area(450_000.0, m), "0.45 km²");
        assert_eq!(format_area(23_400_000.0, m), "23.4 km²");
        assert_eq!(format_area(4.3e6, m), "4.3 km²");
        assert_eq!(format_area(1.234e9, m), "1,234 km²");
        assert_eq!(format_area(761.8, i), "8,200 sq ft");
        assert_eq!(format_area(4_046.86, i), "1.0 acre");
        assert_eq!(format_area(184_536.0, i), "45.6 acres");
        assert_eq!(format_area(60_605_000.0, i), "23.4 mi²");
    }
}
