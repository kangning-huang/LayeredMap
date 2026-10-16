//! China's coordinate offsets: WGS 84 ↔ GCJ-02 ("Mars coordinates", used by Amap, Tencent and
//! most Chinese web maps) ↔ BD-09 (Baidu).
//!
//! The forward formulas are the widely circulated reverse-engineered ones, as implemented by
//! `prcoords` and `coord-convert` (fixture C2). The inverses have no closed form; they are
//! iterated until the forward image of the estimate matches the input to about 1e-9° (≈0.1 mm),
//! far below the plan's 0.5 m bound.
//!
//! **What "China" means here.** GCJ-02 is the identity outside a crude rectangle
//! (72.004–137.8347°E, 0.8293–55.8271°N), the one the reference libraries use, so results match
//! them. Caveat (fixtures/C2/README.md): that rectangle also contains Seoul, Hanoi, Taipei and
//! more, which these formulas *do* shift; a proper in/out-of-China policy (a boundary polygon) is
//! a separate decision. The GCJ-02 ↔ BD-09 step has no such rule in the libraries: it is applied
//! everywhere.
//!
//! All functions take and return `Coord { x: lon, y: lat }` in degrees. Non-finite input is
//! returned unchanged.

use rustgis_data::Dataset;
use rustgis_geom::Coord;
use std::f64::consts::PI;

/// Krasovsky 1940 semi-major axis used by GCJ-02 (metres).
const GCJ_A: f64 = 6_378_245.0;
/// Krasovsky 1940 first eccentricity squared.
const GCJ_EE: f64 = 0.006_693_421_622_965_943;
/// BD-09's angular constant (π · 3000 / 180).
const BD_X_PI: f64 = PI * 3000.0 / 180.0;
/// Inverse iterations stop once the forward residual is below this (degrees; ≈0.1 mm).
const INVERSE_TOL_DEG: f64 = 1e-9;
/// Hard cap on inverse iterations (they converge in 2–4).
const INVERSE_MAX_ITER: usize = 30;

/// A coordinate system that may carry China's offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OffsetSystem {
    /// Unshifted WGS 84 (GPS, OpenStreetMap).
    Wgs84,
    /// GCJ-02 (Amap / Gaode, Tencent, Google Maps in mainland China).
    Gcj02,
    /// BD-09 (Baidu Maps), GCJ-02 with a further offset.
    Bd09,
}

impl OffsetSystem {
    /// The system's display name, as used in recipes: "WGS 84", "GCJ-02", "BD-09".
    pub fn label(self) -> &'static str {
        match self {
            OffsetSystem::Wgs84 => "WGS 84",
            OffsetSystem::Gcj02 => "GCJ-02",
            OffsetSystem::Bd09 => "BD-09",
        }
    }
}

/// True when the reference libraries treat (lon, lat) as outside China, where GCJ-02 is the
/// identity. This is the libraries' rectangle, not a border; see the module notes.
pub fn outside_china(lon: f64, lat: f64) -> bool {
    !(72.004..=137.8347).contains(&lon) || !(0.8293..=55.8271).contains(&lat)
}

fn transform_lat(x: f64, y: f64) -> f64 {
    let mut r = -100.0 + 2.0 * x + 3.0 * y + 0.2 * y * y + 0.1 * x * y + 0.2 * x.abs().sqrt();
    r += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    r += (20.0 * (y * PI).sin() + 40.0 * (y / 3.0 * PI).sin()) * 2.0 / 3.0;
    r += (160.0 * (y / 12.0 * PI).sin() + 320.0 * (y * PI / 30.0).sin()) * 2.0 / 3.0;
    r
}

fn transform_lon(x: f64, y: f64) -> f64 {
    let mut r = 300.0 + x + 2.0 * y + 0.1 * x * x + 0.1 * x * y + 0.1 * x.abs().sqrt();
    r += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    r += (20.0 * (x * PI).sin() + 40.0 * (x / 3.0 * PI).sin()) * 2.0 / 3.0;
    r += (150.0 * (x / 12.0 * PI).sin() + 300.0 * (x / 30.0 * PI).sin()) * 2.0 / 3.0;
    r
}

/// WGS 84 → GCJ-02. Identity outside China (see [`outside_china`]).
pub fn wgs84_to_gcj02(c: Coord) -> Coord {
    if !c.is_finite() || outside_china(c.x, c.y) {
        return c;
    }
    let (lon, lat) = (c.x, c.y);
    let d_lat = transform_lat(lon - 105.0, lat - 35.0);
    let d_lon = transform_lon(lon - 105.0, lat - 35.0);
    let rad_lat = lat / 180.0 * PI;
    let magic = 1.0 - GCJ_EE * rad_lat.sin().powi(2);
    let sqrt_magic = magic.sqrt();
    let d_lat = (d_lat * 180.0) / ((GCJ_A * (1.0 - GCJ_EE)) / (magic * sqrt_magic) * PI);
    let d_lon = (d_lon * 180.0) / (GCJ_A / sqrt_magic * rad_lat.cos() * PI);
    Coord::new(lon + d_lon, lat + d_lat)
}

/// Inverts `forward` near `target` by fixed-point iteration from `start`.
fn invert(target: Coord, start: Coord, forward: impl Fn(Coord) -> Coord) -> Coord {
    if !target.is_finite() {
        return target;
    }
    let mut est = start;
    for _ in 0..INVERSE_MAX_ITER {
        let img = forward(est);
        let (dx, dy) = (target.x - img.x, target.y - img.y);
        if !(dx.is_finite() && dy.is_finite()) {
            break;
        }
        est = Coord::new(est.x + dx, est.y + dy);
        if dx.abs() < INVERSE_TOL_DEG && dy.abs() < INVERSE_TOL_DEG {
            break;
        }
    }
    est
}

/// GCJ-02 → WGS 84, iterated until the residual is ≈1e-9°. Identity outside China.
pub fn gcj02_to_wgs84(c: Coord) -> Coord {
    if !c.is_finite() || outside_china(c.x, c.y) {
        return c;
    }
    // Rough inverse (subtract the offset at the GCJ point), then refine.
    let g = wgs84_to_gcj02(c);
    let start = Coord::new(2.0 * c.x - g.x, 2.0 * c.y - g.y);
    invert(c, start, wgs84_to_gcj02)
}

/// GCJ-02 → BD-09 (applied everywhere, like the reference libraries).
pub fn gcj02_to_bd09(c: Coord) -> Coord {
    if !c.is_finite() {
        return c;
    }
    let (x, y) = (c.x, c.y);
    let z = (x * x + y * y).sqrt() + 0.00002 * (y * BD_X_PI).sin();
    let theta = y.atan2(x) + 0.000003 * (x * BD_X_PI).cos();
    Coord::new(z * theta.cos() + 0.0065, z * theta.sin() + 0.006)
}

/// BD-09 → GCJ-02: the usual closed-form approximation, refined by iteration to ≈1e-9°.
pub fn bd09_to_gcj02(c: Coord) -> Coord {
    if !c.is_finite() {
        return c;
    }
    let (x, y) = (c.x - 0.0065, c.y - 0.006);
    let z = (x * x + y * y).sqrt() - 0.00002 * (y * BD_X_PI).sin();
    let theta = y.atan2(x) - 0.000003 * (x * BD_X_PI).cos();
    let start = Coord::new(z * theta.cos(), z * theta.sin());
    invert(c, start, gcj02_to_bd09)
}

/// WGS 84 → BD-09 (through GCJ-02).
pub fn wgs84_to_bd09(c: Coord) -> Coord {
    gcj02_to_bd09(wgs84_to_gcj02(c))
}

/// BD-09 → WGS 84 (through GCJ-02).
pub fn bd09_to_wgs84(c: Coord) -> Coord {
    gcj02_to_wgs84(bd09_to_gcj02(c))
}

/// Converts one coordinate between any two systems.
pub fn convert(c: Coord, from: OffsetSystem, to: OffsetSystem) -> Coord {
    use OffsetSystem::*;
    match (from, to) {
        (Wgs84, Wgs84) | (Gcj02, Gcj02) | (Bd09, Bd09) => c,
        (Wgs84, Gcj02) => wgs84_to_gcj02(c),
        (Gcj02, Wgs84) => gcj02_to_wgs84(c),
        (Gcj02, Bd09) => gcj02_to_bd09(c),
        (Bd09, Gcj02) => bd09_to_gcj02(c),
        (Wgs84, Bd09) => wgs84_to_bd09(c),
        (Bd09, Wgs84) => bd09_to_wgs84(c),
    }
}

/// Why a dataset could not be converted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// `from` and `to` are the same system; there is nothing to convert.
    SameSystem,
    /// The dataset holds only a window of a larger source; a derived layer would be incomplete.
    Windowed,
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::SameSystem => f.write_str("the source and target coordinate systems are the same"),
            ConvertError::Windowed => f.write_str("the layer is read in place and only partly loaded"),
        }
    }
}

impl std::error::Error for ConvertError {}

/// A converted copy of a dataset: the derived layer and its recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    pub dataset: Dataset,
    /// E.g. "Converted from GCJ-02 to WGS 84" (English; the UI translates it).
    pub recipe: String,
}

/// The recipe line for a conversion: "Converted from GCJ-02 to WGS 84".
pub fn recipe(from: OffsetSystem, to: OffsetSystem) -> String {
    format!("Converted from {} to {}", from.label(), to.label())
}

/// Converts every geometry of `src` from `from` to `to`, returning a new dataset (named
/// "<name> (<to>)", with no file source). `src` is never modified; attributes, fields, ids and
/// features without geometry are copied as they are.
pub fn convert_dataset(src: &Dataset, from: OffsetSystem, to: OffsetSystem) -> Result<Converted, ConvertError> {
    if from == to {
        return Err(ConvertError::SameSystem);
    }
    if src.windowed {
        return Err(ConvertError::Windowed);
    }
    let mut dataset = Dataset::new(format!("{} ({})", src.name, to.label()), src.geometry_type, src.fields.clone());
    dataset.features = src
        .features
        .iter()
        .map(|f| {
            let mut f = f.clone();
            f.geometry = f.geometry.map(|g| g.map_coords(&|c| convert(c, from, to)));
            f
        })
        .collect();
    Ok(Converted { dataset, recipe: recipe(from, to) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_data::{Feature, Geometry, GeometryType};
    use rustgis_geom::haversine_m;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn fixture(rel: &str) -> String {
        let p: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", rel].iter().collect();
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    }

    /// Simple CSV (no quoting in these fixtures): rows as header → value maps.
    fn rows(rel: &str) -> Vec<HashMap<String, String>> {
        let text = fixture(rel);
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let header: Vec<String> = lines.next().unwrap().split(',').map(|s| s.trim().to_string()).collect();
        lines.map(|l| header.iter().cloned().zip(l.split(',').map(|s| s.trim().to_string())).collect()).collect()
    }

    fn num(r: &HashMap<String, String>, k: &str) -> f64 {
        r[k].parse().unwrap_or_else(|e| panic!("{k}={:?}: {e}", r[k]))
    }

    fn pt(r: &HashMap<String, String>, x: &str, y: &str) -> Coord {
        Coord::new(num(r, x), num(r, y))
    }

    fn deg_diff(a: Coord, b: Coord) -> f64 {
        (a.x - b.x).abs().max((a.y - b.y).abs())
    }

    /// Values are printed with 9 decimals.
    const VEC_TOL_DEG: f64 = 2e-9;

    #[test]
    fn c2_vectors_forward_and_round_trip() {
        let rows = rows("C2/vectors.csv");
        assert_eq!(rows.len(), 58);
        let (mut max_gcj, mut max_bd, mut max_rt_gcj, mut max_rt_bd, mut max_rt_bdg) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for r in &rows {
            let name = &r["name"];
            let wgs = pt(r, "wgs84_lon", "wgs84_lat");
            let gcj = wgs84_to_gcj02(wgs);
            for impl_ in ["prcoords", "coordconvert"] {
                let want_gcj = pt(r, &format!("gcj02_lon_{impl_}"), &format!("gcj02_lat_{impl_}"));
                let d = deg_diff(gcj, want_gcj);
                max_gcj = max_gcj.max(d);
                assert!(d < VEC_TOL_DEG, "{name} GCJ vs {impl_}: {d:e}°");
                // BD columns are computed from A's GCJ-02.
                let a_gcj = pt(r, "gcj02_lon_prcoords", "gcj02_lat_prcoords");
                let want_bd = pt(r, &format!("bd09_lon_{impl_}"), &format!("bd09_lat_{impl_}"));
                let d = deg_diff(gcj02_to_bd09(a_gcj), want_bd);
                max_bd = max_bd.max(d);
                assert!(d < VEC_TOL_DEG, "{name} BD vs {impl_}: {d:e}°");
                // Round trips from the libraries' values.
                let m = haversine_m(gcj02_to_wgs84(want_gcj), wgs);
                max_rt_gcj = max_rt_gcj.max(m);
                assert!(m < 0.5, "{name} GCJ→WGS ({impl_}): {m} m");
                let m = haversine_m(bd09_to_gcj02(want_bd), a_gcj);
                max_rt_bdg = max_rt_bdg.max(m);
                assert!(m < 0.5, "{name} BD→GCJ ({impl_}): {m} m");
                if r["region"] == "china" {
                    let m = haversine_m(bd09_to_wgs84(want_bd), wgs);
                    max_rt_bd = max_rt_bd.max(m);
                    assert!(m < 0.5, "{name} BD→WGS ({impl_}): {m} m");
                }
            }
            if r["region"] == "outside" {
                assert!(outside_china(wgs.x, wgs.y), "{name}");
                assert_eq!(gcj, wgs, "{name}: GCJ must be the identity");
                assert_eq!(gcj02_to_wgs84(wgs), wgs, "{name}");
            } else {
                assert!(!outside_china(wgs.x, wgs.y), "{name}");
                assert!(haversine_m(gcj, wgs) > 50.0, "{name}: expected a real shift");
            }
        }
        eprintln!(
            "C2: max |GCJ−ref| {max_gcj:e}°, max |BD−ref| {max_bd:e}°, round trips: GCJ→WGS {max_rt_gcj:e} m, \
             BD→GCJ {max_rt_bdg:e} m, BD→WGS {max_rt_bd:e} m"
        );
    }

    #[test]
    fn inverse_is_tight() {
        // The iterative inverse lands within ~1e-7° of the true WGS 84 everywhere in China.
        let mut max = 0f64;
        for i in 0..=40 {
            for j in 0..=40 {
                let wgs = Coord::new(73.0 + 64.0 * i as f64 / 40.0, 1.0 + 54.0 * j as f64 / 40.0);
                let gcj = wgs84_to_gcj02(wgs);
                if outside_china(gcj.x, gcj.y) {
                    continue;
                }
                max = max.max(deg_diff(gcj02_to_wgs84(gcj), wgs));
                max = max.max(deg_diff(bd09_to_wgs84(wgs84_to_bd09(wgs)), wgs));
            }
        }
        eprintln!("grid round-trip max {max:e}°");
        assert!(max < 1e-7, "{max:e}");
    }

    #[test]
    fn outside_china_rectangle_and_non_finite() {
        assert!(!outside_china(121.47, 31.23));
        assert!(outside_china(139.77, 35.68)); // Tokyo
        assert!(outside_china(71.9, 40.0));
        assert!(outside_china(100.0, 0.5));
        assert!(outside_china(100.0, 56.0));
        assert!(outside_china(f64::NAN, 30.0));
        // Documented caveat: the rectangle contains Seoul, which is shifted.
        assert!(!outside_china(126.978, 37.566));
        let nan = Coord::new(f64::NAN, 31.0);
        for f in [wgs84_to_gcj02, gcj02_to_wgs84, gcj02_to_bd09, bd09_to_gcj02, wgs84_to_bd09, bd09_to_wgs84] {
            let out = f(nan);
            assert!(out.x.is_nan() && out.y == 31.0);
            let inf = f(Coord::new(f64::INFINITY, 1e308));
            assert!(inf.x.is_infinite());
        }
    }

    fn fixture_points(rel: &str) -> Vec<(String, Coord)> {
        rows(rel).iter().map(|r| (r["name"].clone(), pt(r, "lng", "lat"))).collect()
    }

    fn check_fixture(input: &str, expected: &str, from: OffsetSystem, tol_deg: f64) -> f64 {
        let pts = fixture_points(input);
        let want: HashMap<String, Coord> = rows(expected).iter().map(|r| (r["name"].clone(), pt(r, "wgs84_lon", "wgs84_lat"))).collect();
        assert_eq!(pts.len(), want.len());
        let mut max = 0f64;
        for (name, c) in &pts {
            let got = convert(*c, from, OffsetSystem::Wgs84);
            let d = deg_diff(got, want[name]);
            max = max.max(d);
            assert!(d <= tol_deg, "{input} {name}: {d:e}° (> {tol_deg:e})");
            assert!(haversine_m(got, want[name]) < 0.5);
        }
        max
    }

    #[test]
    fn c1_amap_gcj02_to_wgs84() {
        // expected.json: ~1e-7° with an iterative inverse. Inputs and expected values are printed
        // with 6 decimals (±5e-7°), so the bound is that rounding plus the GCJ Jacobian (~1.02).
        let max = check_fixture("C1/amap_pois.csv", "C1/expected_wgs84.csv", OffsetSystem::Gcj02, 1.1e-6);
        eprintln!("C1 max {max:e}°");
    }

    #[test]
    fn c3_baidu_bd09_to_wgs84() {
        let max = check_fixture("C3/baidu_pois.csv", "C3/expected_wgs84.csv", OffsetSystem::Bd09, 1.1e-6);
        eprintln!("C3 max {max:e}°");
    }

    fn sample_dataset() -> Dataset {
        let mut ds = Dataset::new("amap_pois", GeometryType::Point, Vec::new());
        let c = Coord::new(121.4782, 31.2285);
        ds.features = vec![
            Feature { id: 1, geometry: Some(Geometry::Point(c)), attrs: Vec::new() },
            Feature { id: 2, geometry: Some(Geometry::MultiPoint(vec![c, Coord::new(121.5, 31.3)])), attrs: Vec::new() },
            Feature { id: 3, geometry: Some(Geometry::Line(vec![vec![c, Coord::new(121.5, 31.3)]])), attrs: Vec::new() },
            Feature {
                id: 4,
                geometry: Some(Geometry::Polygon(vec![vec![vec![c, Coord::new(121.5, 31.3), Coord::new(121.4, 31.3), c]]])),
                attrs: Vec::new(),
            },
            Feature { id: 5, geometry: None, attrs: Vec::new() },
        ];
        ds.source = Some("amap_pois.csv".into());
        ds
    }

    #[test]
    fn convert_dataset_makes_a_derived_copy() {
        let src = sample_dataset();
        let before = src.clone();
        let out = convert_dataset(&src, OffsetSystem::Gcj02, OffsetSystem::Wgs84).unwrap();
        assert_eq!(src, before, "source unchanged");
        assert_eq!(out.recipe, "Converted from GCJ-02 to WGS 84");
        assert_eq!(out.dataset.name, "amap_pois (WGS 84)");
        assert_eq!(out.dataset.source, None);
        assert_eq!(out.dataset.features.len(), src.features.len());
        for (a, b) in src.features.iter().zip(&out.dataset.features) {
            assert_eq!(a.id, b.id);
            match (&a.geometry, &b.geometry) {
                (Some(ga), Some(gb)) => {
                    assert_eq!(ga.kind(), gb.kind());
                    let mut ca = Vec::new();
                    ga.for_each_coord(|c| ca.push(c));
                    let mut cb = Vec::new();
                    gb.for_each_coord(|c| cb.push(c));
                    assert_eq!(ca.len(), cb.len());
                    for (x, y) in ca.iter().zip(&cb) {
                        assert_eq!(*y, gcj02_to_wgs84(*x));
                        assert!(haversine_m(*x, *y) > 100.0);
                    }
                }
                (None, None) => {}
                _ => panic!("geometry presence changed"),
            }
        }
        // And back again, through BD-09.
        let bd = convert_dataset(&out.dataset, OffsetSystem::Wgs84, OffsetSystem::Bd09).unwrap();
        assert_eq!(bd.recipe, "Converted from WGS 84 to BD-09");
        let back = convert_dataset(&bd.dataset, OffsetSystem::Bd09, OffsetSystem::Gcj02).unwrap();
        for (a, b) in src.features.iter().zip(&back.dataset.features) {
            if let (Some(ga), Some(gb)) = (&a.geometry, &b.geometry) {
                let mut ca = Vec::new();
                ga.for_each_coord(|c| ca.push(c));
                let mut cb = Vec::new();
                gb.for_each_coord(|c| cb.push(c));
                for (x, y) in ca.iter().zip(&cb) {
                    assert!(haversine_m(*x, *y) < 0.01);
                }
            }
        }
    }

    #[test]
    fn convert_dataset_refusals() {
        let mut src = sample_dataset();
        assert_eq!(convert_dataset(&src, OffsetSystem::Gcj02, OffsetSystem::Gcj02), Err(ConvertError::SameSystem));
        src.windowed = true;
        assert_eq!(convert_dataset(&src, OffsetSystem::Gcj02, OffsetSystem::Wgs84), Err(ConvertError::Windowed));
    }
}
