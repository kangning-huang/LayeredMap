use super::*;
use proptest::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn fixture_path(rel: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", rel].iter().collect()
}

fn fixture(rel: &str) -> String {
    let p = fixture_path(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn raster(rel: &str) -> Raster {
    rustgis_data::raster::read_path(&fixture_path(rel)).unwrap()
}

/// Areas of a GeoJSON fixture by their id column.
fn areas(text: &str, id: &str) -> Vec<(String, Geometry)> {
    let (ds, _) = rustgis_data::io::read_geojson(text, "areas").unwrap();
    let col = ds.fields.iter().position(|f| f.name == id).unwrap();
    ds.features
        .iter()
        .map(|f| {
            let rustgis_data::Value::Text(v) = &f.attrs[col] else { panic!("{id}") };
            (v.clone(), f.geometry.clone().unwrap())
        })
        .collect()
}

fn csv_rows(text: &str) -> Vec<HashMap<String, String>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    lines.map(|l| header.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect()
}

fn rel_err(got: f64, want: f64) -> f64 {
    (got - want).abs() / want.abs().max(1e-300)
}

fn run(src: &mut dyn RasterSource, geoms: &[Geometry], role: ValueRole) -> Vec<ZonalSummary> {
    summarize(src, geoms, role, &Control::none()).unwrap()
}

fn poly(rings: Vec<Vec<(f64, f64)>>) -> Geometry {
    Geometry::Polygon(vec![rings.into_iter().map(|r| r.into_iter().map(|(x, y)| Coord::new(x, y)).collect()).collect()])
}

// ---- Fixtures -------------------------------------------------------------------------------

#[test]
fn n3_matches_the_hand_calculation() {
    let r = raster("N3/grid4x4.tif");
    let a = areas(&fixture("N3/polygon.geojson"), "area_id");
    let geoms: Vec<Geometry> = a.iter().map(|(_, g)| g.clone()).collect();
    let s = &run(&mut MemoryBand::new(&r, 0).unwrap(), &geoms, ValueRole::Measured)[0];
    let want = &csv_rows(&fixture("N3/expected_zonal_mean.csv"))[0];
    let num = |k: &str| want[k].parse::<f64>().unwrap();
    let errs = [
        ("mean", rel_err(s.mean.unwrap(), num("mean"))),
        ("coverage", rel_err(s.coverage, num("coverage"))),
        ("covered_valid_area_m2", rel_err(s.covered_valid_area_m2, num("covered_valid_area_m2"))),
        ("polygon_area_m2", rel_err(s.polygon_area_m2, num("polygon_area_m2"))),
    ];
    eprintln!("N3 relative errors: {errs:?}");
    for (k, e) in errs {
        assert!(e < 1e-9, "{k}: relative error {e}");
    }
    assert_eq!((s.min, s.max), (Some(5.0), Some(16.0)));
    // Row 1: 5, 7, 8; row 2: 10, 11, 12; row 3: 13–16. NoData: 6 (whole) and 9 (half).
    assert_eq!((s.valid_cells, s.nodata_cells), (10, 2));
    assert!((s.valid_fraction_sum - 7.5).abs() < 1e-12);
    // The unweighted mean would be 84 / 7.5 = 11.2.
    assert!((s.mean.unwrap() - 11.2).abs() > 0.05);
    // Hand calculation of the row areas (km²).
    for (lat, want_km2) in [(60.0, 6123.1409), (59.0, 6309.8057), (58.0, 6494.4470)] {
        assert!((lat_band_area_m2(lat, lat + 1.0, 1.0) / 1e6 - want_km2).abs() < 1e-4);
    }
}

/// Star, triangle, polygon with a hole and a polygon half off the raster over N3's grid, against
/// `reference/zonal_mean.py` (exactextract with a Geod cell-area weight raster).
fn n3_adversarial() -> Vec<(&'static str, Geometry)> {
    fn star(cx: f64, cy: f64, r1: f64, r2: f64, n: usize, rot: f64) -> Vec<(f64, f64)> {
        let mut v: Vec<(f64, f64)> = (0..2 * n)
            .map(|k| {
                let r = if k % 2 == 0 { r1 } else { r2 };
                let a = rot + std::f64::consts::PI * k as f64 / n as f64;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        v.push(v[0]);
        v
    }
    let mut hole = star(12.1, 60.1, 1.0, 0.4, 5, 0.3);
    hole.reverse();
    vec![
        ("S1", poly(vec![star(12.0, 60.0, 1.7, 0.6, 7, 0.1)])),
        ("T1", poly(vec![vec![(10.2, 58.3), (13.9, 59.1), (11.1, 61.8), (10.2, 58.3)]])),
        ("H1", poly(vec![vec![(10.1, 58.1), (13.8, 58.4), (13.6, 61.7), (10.3, 61.9), (10.1, 58.1)], hole])),
        ("O1", poly(vec![vec![(9.5, 61.3), (11.2, 61.5), (11.0, 62.6), (9.4, 62.4), (9.5, 61.3)]])),
    ]
}

#[test]
fn n3_adversarial_shapes_match_exactextract_to_1_ppm() {
    // id, mean, min, max, coverage, covered_valid_area_m2, polygon_area_m2
    let want = [
        ("H1", 8.81632123143, 1.0, 16.0, 0.861668731679, 60287796905.7, 69966327765.2),
        ("O1", 1.12028435036, 1.0, 2.0, 0.363980060362, 3928518250.67, 10793223801.2),
        ("S1", 9.00233982361, 2.0, 16.0, 0.770655244841, 14841398079.5, 19258154899.8),
        ("T1", 10.1183917957, 1.0, 16.0, 0.759552460529, 29103235247.3, 38316293817.3),
    ];
    let r = raster("N3/grid4x4.tif");
    let shapes = n3_adversarial();
    let geoms: Vec<Geometry> = shapes.iter().map(|(_, g)| g.clone()).collect();
    let got = run(&mut MemoryBand::new(&r, 0).unwrap(), &geoms, ValueRole::Measured);
    for ((id, _), s) in shapes.iter().zip(&got) {
        let w = want.iter().find(|w| w.0 == *id).unwrap();
        let errs = [
            rel_err(s.mean.unwrap(), w.1),
            // The reference's coverage is Σa/A_P; ours divides by max(A_P, Σ_all a) so it never
            // exceeds 1 (H1's grid over-measures it by 1.7e-4).
            rel_err(s.coverage, w.4 * w.6 / f64::max(w.6, s.covered_area_m2)),
            rel_err(s.covered_valid_area_m2, w.5),
            rel_err(s.polygon_area_m2, w.6),
        ];
        eprintln!("{id}: mean/coverage/valid area/polygon area relative errors {errs:?}");
        assert!(errs.iter().all(|e| *e < 1e-6), "{id}: {errs:?}");
        assert_eq!((s.min, s.max), (Some(w.2), Some(w.3)), "{id}");
        assert!(s.coverage <= 1.0 + 1e-9);
    }
}

#[test]
fn r3_v1_weighted_means_match_exactextract_to_1_ppm() {
    // reference/zonal_mean.py fixtures/R3/landcover.tif fixtures/V1/districts.geojson district_id
    // (category codes averaged here only to exercise the weighted mean on multipolygons and holes).
    let want = [
        ("D01", 24.1918913771, 237193863.293),
        ("D02", 41.2104322087, 237193863.293),
        ("D03", 46.8127156362, 237193863.293),
        ("D04", 56.0983881137, 221380939.168),
        ("D05", 57.9321378416, 237565051.901),
        ("D06", 59.8386477179, 211168928.079),
        ("D07", 59.414907665, 237565051.901),
        ("D08", 52.7792525372, 237565051.901),
        ("D09", 60.7306142731, 237934609.202),
        ("D10", 60.1108733712, 237934609.202),
        ("D11", 40.2174453956, 237934609.202),
        ("D12", 47.3912925888, 237934609.202),
        ("D13", 44.840386345, 238302533.062),
        ("D14", 42.3177671987, 238302533.062),
        ("D15", 42.5244182848, 238302533.062),
        ("D16", 39.891719282, 238302533.062),
        ("D17", 55.5485932798, 26396123.6437),
    ];
    let r = raster("R3/landcover.tif");
    let a = areas(&fixture("V1/districts.geojson"), "district_id");
    let geoms: Vec<Geometry> = a.iter().map(|(_, g)| g.clone()).collect();
    let got = run(&mut MemoryBand::new(&r, 0).unwrap(), &geoms, ValueRole::Measured);
    let mut worst: f64 = 0.0;
    for ((id, _), s) in a.iter().zip(&got) {
        let w = want.iter().find(|w| w.0 == id).unwrap();
        let e = rel_err(s.mean.unwrap(), w.1).max(rel_err(s.covered_valid_area_m2, w.2));
        worst = worst.max(e);
        assert!(e < 1e-6, "{id}: {e}");
        assert!(s.coverage <= 1.0 + 1e-9, "{id}: {}", s.coverage);
    }
    eprintln!("R3×V1: worst relative error {worst:e}");
}

#[test]
fn n8_category_shares() {
    let r = raster("R3/landcover.tif");
    let a = areas(&fixture("V1/districts.geojson"), "district_id");
    let geoms: Vec<Geometry> = a.iter().map(|(_, g)| g.clone()).collect();
    let got = run(&mut MemoryBand::new(&r, 0).unwrap(), &geoms, ValueRole::Category);
    let mut by_id: HashMap<&str, &ZonalSummary> = HashMap::new();
    for ((id, _), s) in a.iter().zip(&got) {
        by_id.insert(id, s);
        assert_eq!(s.mean, None, "categories are never averaged");
        let total: f64 = s.categories.iter().map(|c| c.share).sum();
        assert!((total - 1.0).abs() < 1e-12, "{id}: shares sum to {total}");
    }
    let rows = csv_rows(&fixture("N8/expected_category_shares.csv"));
    let (mut worst_share, mut worst_area): (f64, f64) = (0.0, 0.0);
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for row in &rows {
        let s = by_id[row["area_id"].as_str()];
        *seen.entry(row["area_id"].as_str()).or_default() += 1;
        let cat: f64 = row["category"].parse().unwrap();
        let c = s.categories.iter().find(|c| c.value == cat).unwrap_or_else(|| panic!("{} lacks {cat}", row["area_id"]));
        worst_share = worst_share.max((c.share - row["share"].parse::<f64>().unwrap()).abs());
        worst_area = worst_area.max(rel_err(c.area_m2 / 1e6, row["area_km2"].parse().unwrap()));
    }
    for (id, s) in &by_id {
        assert_eq!(s.categories.len(), seen[id], "{id}: categories listed");
    }
    eprintln!("N8: worst share error {worst_share:e}, worst area relative error {worst_area:e}");
    assert!(worst_share < 1e-6 && worst_area < 1e-6);
}

#[test]
fn r6_samples_with_scale_offset_and_blank_nodata() {
    let r = raster("R6/scaled.tif");
    // R6 has no NoData tag; the app's assumption (-9999) is applied by the caller.
    let mut src = MemoryBand::new(&r, 0).unwrap().with_scale_offset(0.1, -100.0);
    src.grid.nodata = Some(-9999.0);
    let rows = csv_rows(&fixture("R6/points.csv"));
    let pts: Vec<Coord> = rows.iter().map(|p| Coord::new(p["lon"].parse().unwrap(), p["lat"].parse().unwrap())).collect();
    let got = sample_points(&mut src, &pts, &Control::none()).unwrap();
    let want = csv_rows(&fixture("R6/expected_raster_sample.csv"));
    assert_eq!(got.len(), want.len());
    for ((p, g), w) in rows.iter().zip(&got).zip(&want) {
        assert_eq!(p["point_id"], w["point_id"]);
        match (g, w["value"].as_str()) {
            (None, "") => {}
            (Some(v), s) if !s.is_empty() => assert!((v - s.parse::<f64>().unwrap()).abs() < 1e-9, "{}: {v} vs {s}", p["point_id"]),
            _ => panic!("{}: got {g:?}, want {:?}", p["point_id"], w["value"]),
        }
    }
}

#[test]
fn n3_samples_cells_nodata_and_outside() {
    let r = raster("N3/grid4x4.tif");
    let mut src = MemoryBand::new(&r, 0).unwrap();
    let pts = [
        Coord::new(10.5, 61.5),   // 1
        Coord::new(11.0, 61.0),   // on a corner: the cell to its right and below → 6 → NoData
        Coord::new(12.0, 60.0),   // corner → row 2 col 2 → 11
        Coord::new(13.99, 58.01), // 16
        Coord::new(14.0, 59.0),   // east edge: outside
        Coord::new(9.0, 60.0),    // outside
        Coord::new(f64::NAN, 60.0),
    ];
    let got = sample_points(&mut src, &pts, &Control::none()).unwrap();
    assert_eq!(got, vec![Some(1.0), None, Some(11.0), Some(16.0), None, None, None]);
}

// ---- Properties -----------------------------------------------------------------------------

/// A raster of `w × h` cells with values from `f(col, row)`.
fn grid_raster(w: u32, h: u32, origin: (f64, f64), px: (f64, f64), nodata: Option<f64>, f: impl Fn(u32, u32) -> f32) -> Raster {
    let mut data = Vec::with_capacity((w * h) as usize);
    for r in 0..h {
        for c in 0..w {
            data.push(f(c, r));
        }
    }
    let t = GeoTransform { origin_x: origin.0, origin_y: origin.1, pixel_width: px.0, pixel_height: -px.1 };
    Raster::new("t", w, h, vec![data], t, Crs::LonLat, nodata, Default::default(), "Float32").unwrap()
}

fn star_ring(cx: f64, cy: f64, radii: &[f64], jitter: &[f64], rot: f64) -> Vec<(f64, f64)> {
    let n = radii.len();
    let mut v: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let a = rot + std::f64::consts::TAU * (k as f64 + 0.3 * jitter[k % jitter.len()]) / n as f64;
            (cx + radii[k] * a.cos(), cy + radii[k] * a.sin())
        })
        .collect();
    v.push(v[0]);
    v
}

/// Shoelace area of a ring in pixel units of a north-up grid.
fn pixel_area(ring: &[(f64, f64)], origin: (f64, f64), px: (f64, f64)) -> f64 {
    let p: Vec<(f64, f64)> = ring.iter().map(|(x, y)| ((x - origin.0) / px.0, (origin.1 - y) / px.1)).collect();
    let mut s = 0.0;
    for i in 0..p.len() - 1 {
        s += p[i].0 * p[i + 1].1 - p[i + 1].0 * p[i].1;
    }
    0.5 * s.abs()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

    /// Coverage never exceeds 1 by more than 1e-9; for a polygon inside the raster the covered
    /// fractions add up to its exact planar area in cells; a constant raster's mean is the constant.
    #[test]
    fn coverage_is_exact_and_bounded(
        w in 1u32..30, h in 1u32..30,
        lon0 in -170.0f64..150.0, top in -60.0f64..85.0,
        pxs in 0.0005f64..1.0,
        radii in proptest::collection::vec(0.5f64..1.0, 6..14),
        jitter in proptest::collection::vec(-1.0f64..1.0, 6..14),
        cx in 0.0f64..1.0, cy in 0.0f64..1.0, size in 0.05f64..1.5, rot in 0.0f64..6.3,
        hole in proptest::bool::ANY, nodata_every in 0u32..5, constant in -1e3f64..1e3,
    ) {
        let pxs_y = pxs.min((top + 89.0) / h as f64);
        let px = (pxs, pxs_y);
        let origin = (lon0, top);
        let (ew, eh) = (w as f64 * px.0, h as f64 * px.1);
        let (x, y) = (lon0 + cx * ew, top - cy * eh);
        let rad = size * ew.min(eh);
        let rs: Vec<f64> = radii.iter().map(|r| r * rad).collect();
        let outer = star_ring(x, y, &rs, &jitter, rot);
        let mut rings = vec![outer.clone()];
        let mut planar = pixel_area(&outer, origin, px);
        if hole {
            let inner = star_ring(x, y, &[0.2 * rad; 8], &[0.0], 0.0);
            planar -= pixel_area(&inner, origin, px);
            rings.push(inner);
        }
        let geom = poly(rings);
        let nd = |c: u32, r: u32| nodata_every > 0 && (c * 7 + r * 3).is_multiple_of(nodata_every + 2);
        let ras = grid_raster(w, h, origin, px, Some(-9999.0), |c, r| if nd(c, r) { -9999.0 } else { constant as f32 });
        let s = &run(&mut MemoryBand::new(&ras, 0).unwrap(), std::slice::from_ref(&geom), ValueRole::Measured)[0];
        prop_assert!(s.coverage <= 1.0 + 1e-9 && s.coverage >= 0.0, "coverage {}", s.coverage);
        prop_assert!((0.0..=1.0).contains(&s.nodata_fraction));
        if let Some(m) = s.mean {
            prop_assert!((m - f64::from(constant as f32)).abs() <= 1e-9 * constant.abs().max(1.0), "mean {m}");
        }
        let inside = rs.iter().all(|r| x - r > lon0 && x + r < lon0 + ew && y + r < top && y - r > top - eh);
        if inside && nodata_every == 0 {
            prop_assert!((s.valid_fraction_sum - planar).abs() <= 1e-9 * planar.max(1.0), "Σf {} vs {}", s.valid_fraction_sum, planar);
            prop_assert!(s.coverage > 0.98, "coverage {}", s.coverage);
        }
    }
}

#[test]
fn constant_raster_mean_min_max_with_scale_and_offset() {
    let ras = grid_raster(50, 40, (100.0, 40.0), (0.01, 0.01), None, |_, _| 7.0);
    let g = poly(vec![vec![(100.013, 39.71), (100.4, 39.65), (100.31, 39.98), (100.013, 39.71)]]);
    let s = &run(&mut MemoryBand::new(&ras, 0).unwrap().with_scale_offset(0.5, 3.0), &[g], ValueRole::Measured)[0];
    assert!((s.mean.unwrap() - 6.5).abs() < 1e-12, "{:?}", s.mean);
    assert_eq!((s.min, s.max), (Some(6.5), Some(6.5)));
}

#[test]
fn totals_for_count_and_density_rasters() {
    let ras = grid_raster(20, 20, (10.0, 50.0), (0.1, 0.1), None, |_, _| 3.0);
    // A rectangle of 5 × 4 cells plus half a column on the east (11.0–11.55, 49.0–49.4).
    let g = poly(vec![vec![(11.0, 49.0), (11.55, 49.0), (11.55, 49.4), (11.0, 49.4), (11.0, 49.0)]]);
    let count = &run(&mut MemoryBand::new(&ras, 0).unwrap(), std::slice::from_ref(&g), ValueRole::Count)[0];
    let t = count.total.unwrap();
    assert_eq!(t.formula, TotalFormula::CountTimesFraction);
    assert!((t.value - 3.0 * 5.5 * 4.0).abs() < 1e-9, "{}", t.value);
    let dens = &run(&mut MemoryBand::new(&ras, 0).unwrap(), &[g], ValueRole::Density { unit_area_m2: 1e6 })[0];
    let t = dens.total.unwrap();
    assert_eq!(t.formula, TotalFormula::DensityTimesArea);
    assert!(rel_err(t.value, 3.0 * dens.covered_valid_area_m2 / 1e6) < 1e-12);
    // The rectangle's edges follow meridians and parallels, so its area is a band's.
    assert!(rel_err(dens.polygon_area_m2, lat_band_area_m2(49.0, 49.4, 0.55)) < 1e-12);
    assert!(rel_err(dens.covered_valid_area_m2, dens.polygon_area_m2) < 1e-12);
    let g = poly(vec![vec![(10.0, 49.0), (10.5, 49.0), (10.5, 49.5), (10.0, 49.0)]]);
    assert!(run(&mut MemoryBand::new(&ras, 0).unwrap(), &[g], ValueRole::Measured)[0].total.is_none());
}

#[test]
fn a_polygon_larger_than_the_raster_costs_only_the_raster() {
    let ras = grid_raster(10, 8, (0.0, 1.0), (0.1, 0.125), None, |c, r| (c + r) as f32);
    let g = poly(vec![vec![(-50.0, -40.0), (60.0, -40.0), (60.0, 40.0), (-50.0, 40.0), (-50.0, -40.0)]]);
    let s = &run(&mut MemoryBand::new(&ras, 0).unwrap(), &[g], ValueRole::Measured)[0];
    assert!((s.valid_fraction_sum - 80.0).abs() < 1e-9);
    assert!(rel_err(s.covered_area_m2, lat_band_area_m2(0.0, 1.0, 1.0)) < 1e-12);
    assert!(s.coverage < 0.001);
    assert_eq!((s.min, s.max), (Some(0.0), Some(16.0)));
}

#[test]
fn non_polygons_and_disjoint_areas_get_empty_summaries() {
    let ras = grid_raster(4, 4, (0.0, 4.0), (1.0, 1.0), None, |_, _| 1.0);
    let far = poly(vec![vec![(20.0, 20.0), (21.0, 20.0), (21.0, 21.0), (20.0, 20.0)]]);
    let got = run(&mut MemoryBand::new(&ras, 0).unwrap(), &[Geometry::Point(Coord::new(1.0, 1.0)), far], ValueRole::Measured);
    assert_eq!(got[0], ZonalSummary::default());
    assert_eq!((got[1].mean, got[1].coverage, got[1].valid_cells), (None, 0.0, 0));
    assert!(got[1].polygon_area_m2 > 0.0);
}

#[test]
fn summaries_are_deterministic() {
    let r = raster("R3/landcover.tif");
    let a = areas(&fixture("V1/districts.geojson"), "district_id");
    let geoms: Vec<Geometry> = a.iter().map(|(_, g)| g.clone()).collect();
    let one = run(&mut MemoryBand::new(&r, 0).unwrap(), &geoms, ValueRole::Category);
    let two = run(&mut MemoryBand::new(&r, 0).unwrap(), &geoms, ValueRole::Category);
    assert_eq!(one, two);
    let bits = |v: &[ZonalSummary]| v.iter().map(|s| s.covered_area_m2.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&one), bits(&two));
}

#[test]
fn projected_cells_are_weighted_by_their_ellipsoidal_area() {
    // A Web Mercator raster over 30–50°N, 0–20°E: its outline follows meridians and parallels,
    // so its cells must add up to that band's area on the ellipsoid.
    let (x0, y1) = (0.0, rustgis_geom::lonlat_to_merc(Coord::new(0.0, 50.0)).y);
    let (x1, y0) = (rustgis_geom::lonlat_to_merc(Coord::new(20.0, 30.0)).x, rustgis_geom::lonlat_to_merc(Coord::new(20.0, 30.0)).y);
    let (w, h) = (200u32, 250u32);
    let t = GeoTransform { origin_x: x0, origin_y: y1, pixel_width: (x1 - x0) / f64::from(w), pixel_height: -(y1 - y0) / f64::from(h) };
    let ras = Raster::new("m", w, h, vec![vec![2.0; (w * h) as usize]], t, Crs::WebMercator, None, Default::default(), "Float32").unwrap();
    let whole = poly(vec![vec![(-1.0, 29.0), (21.0, 29.0), (21.0, 51.0), (-1.0, 51.0), (-1.0, 29.0)]]);
    let inner = poly(vec![vec![(5.0, 35.0), (12.0, 36.0), (8.0, 44.0), (5.0, 35.0)]]);
    let got = run(&mut MemoryBand::new(&ras, 0).unwrap(), &[whole, inner], ValueRole::Measured);
    let band = lat_band_area_m2(30.0, 50.0, 20.0);
    let e = rel_err(got[0].covered_area_m2, band);
    eprintln!("Web Mercator: cells vs band area relative error {e:e}; inner coverage {}", got[1].coverage);
    assert!(e < 1e-6);
    assert_eq!(got[1].mean, Some(2.0));
    assert!(got[1].coverage <= 1.0 + 1e-9 && got[1].coverage > 0.999);
}

// ---- Streaming, cancel, stress --------------------------------------------------------------

/// A raster made up on the fly: as large as wanted, never held in memory.
struct Synthetic {
    grid: Grid,
}

impl Synthetic {
    fn new(side: u32, origin: (f64, f64), deg: f64) -> Synthetic {
        let px = deg / f64::from(side);
        Synthetic {
            grid: Grid {
                width: side,
                height: side,
                transform: GeoTransform { origin_x: origin.0, origin_y: origin.1, pixel_width: px, pixel_height: -px },
                crs: Crs::LonLat,
                nodata: Some(-9999.0),
                scale: 1.0,
                offset: 0.0,
            },
        }
    }
}

impl RasterSource for Synthetic {
    fn grid(&self) -> &Grid {
        &self.grid
    }
    fn read_window(&mut self, col0: u32, row0: u32, cols: u32, rows: u32, out: &mut [f32]) -> Result<(), ZonalError> {
        for r in 0..rows {
            let row = row0 + r;
            for c in 0..cols {
                let col = col0 + c;
                let h = (col.wrapping_mul(73_856_093) ^ row.wrapping_mul(19_349_663)) % 1000;
                out[(r * cols + c) as usize] = if h == 0 { -9999.0 } else { 20.0 + h as f32 * 0.01 };
            }
        }
        Ok(())
    }
}

/// `n` jittered 24-gons tiling (and slightly overlapping) the extent.
fn tiles(n_across: usize, n_down: usize, origin: (f64, f64), deg: f64) -> Vec<Geometry> {
    let mut seed = 12_345u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (tw, th) = (deg / n_across as f64, deg / n_down as f64);
    let mut out = Vec::new();
    for j in 0..n_down {
        for i in 0..n_across {
            let (cx, cy) = (origin.0 + (i as f64 + 0.5) * tw, origin.1 - (j as f64 + 0.5) * th);
            let ring: Vec<Coord> = (0..=24)
                .map(|k| {
                    let a = std::f64::consts::TAU * (k % 24) as f64 / 24.0;
                    let r = 0.62 + 0.1 * rnd();
                    Coord::new(cx + r * tw * a.cos(), cy + r * th * a.sin())
                })
                .collect();
            let mut ring = ring;
            if let Some(f) = ring.first().copied() {
                *ring.last_mut().unwrap() = f;
            }
            out.push(Geometry::Polygon(vec![vec![ring]]));
        }
    }
    out
}

#[test]
fn a_cancel_lands_within_a_second() {
    // 40,000² cells (6.4 GB as float32), one area over all of it: minutes of work.
    let mut src = Synthetic::new(40_000, (121.0, 31.6), 0.6);
    let geoms = vec![poly(vec![vec![(120.9, 30.9), (121.7, 30.9), (121.7, 31.7), (120.9, 31.7), (120.9, 30.9)]])];
    let ctl = Control::none();
    let c2 = ctl.clone();
    let worker = std::thread::spawn(move || {
        let r = summarize(&mut src, &geoms, ValueRole::Measured, &c2);
        (r, Instant::now())
    });
    std::thread::sleep(Duration::from_millis(300));
    let cancelled_at = Instant::now();
    ctl.cancel();
    let (r, done) = worker.join().unwrap();
    let lag = done.saturating_duration_since(cancelled_at);
    eprintln!("cancel landed after {lag:?}");
    assert_eq!(r, Err(ZonalError::Cancelled));
    assert!(lag < Duration::from_secs(1), "{lag:?}");
}

/// N7 approximation: R4's size (23,040² float32 cells, 2.1 GB) over Shanghai, 3,000 areas.
/// `cargo test --release -p lm-analysis zonal::tests::n7_stress -- --ignored --nocapture`
#[test]
#[ignore = "stress: run in release"]
fn n7_stress() {
    let (origin, deg) = ((120.85, 31.88), 1.0);
    let mut src = Synthetic::new(23_040, origin, deg);
    let geoms = tiles(60, 50, origin, deg);
    assert_eq!(geoms.len(), 3000);
    let reports = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let r2 = reports.clone();
    let ctl = Control::with_progress(move |p| {
        r2.store(p.done, std::sync::atomic::Ordering::Relaxed);
    });
    let t = Instant::now();
    let out = summarize(&mut src, &geoms, ValueRole::Measured, &ctl).unwrap();
    let el = t.elapsed();
    let cells: u64 = out.iter().map(|s| s.valid_cells + s.nodata_cells).sum();
    eprintln!("N7 approximation: 3,000 areas over 23,040² cells in {el:?} ({cells} covered cells visited)");
    assert_eq!(reports.load(std::sync::atomic::Ordering::Relaxed), 3000);
    // Border tiles reach past the raster; inner ones lose only the 0.1 % NoData cells.
    assert!(out.iter().all(|s| s.coverage > 0.3 && s.coverage <= 1.0 + 1e-9));
    assert!(out.iter().filter(|s| s.coverage > 0.998).count() > 2500);
    assert!(el < Duration::from_secs(300));

    // Cancel midway through the same job.
    let ctl = Control::none();
    let c2 = ctl.clone();
    let worker = std::thread::spawn(move || {
        let r = summarize(&mut src, &geoms, ValueRole::Measured, &c2);
        (r.is_err(), Instant::now())
    });
    std::thread::sleep(el / 3);
    let at = Instant::now();
    ctl.cancel();
    let (err, done) = worker.join().unwrap();
    eprintln!("N7 cancel landed after {:?}", done.saturating_duration_since(at));
    assert!(err && done.saturating_duration_since(at) < Duration::from_secs(1));
}

/// Tile side of the file-backed stress raster (a COG's usual 512 px).
const TILE: u32 = 512;
/// Tiles kept decoded (128 MB): about three rows of tiles across the raster.
const TILE_CACHE: usize = 128;

/// A raw little-endian float32 file stored in 512 × 512 tiles, read a tile at a time with a
/// bounded first-in-first-out cache, as a cloud-optimised GeoTIFF reader would.
struct TiledFile {
    grid: Grid,
    file: std::fs::File,
    across: u32,
    cache: HashMap<(u32, u32), Vec<f32>>,
    order: std::collections::VecDeque<(u32, u32)>,
    bytes: Vec<u8>,
}

impl TiledFile {
    fn tile(&mut self, tx: u32, ty: u32) -> Result<&Vec<f32>, ZonalError> {
        use std::io::{Read, Seek, SeekFrom};
        if !self.cache.contains_key(&(tx, ty)) {
            let n = (TILE * TILE) as usize;
            self.bytes.resize(n * 4, 0);
            let at = (u64::from(ty) * u64::from(self.across) + u64::from(tx)) * n as u64 * 4;
            self.file.seek(SeekFrom::Start(at)).map_err(|e| ZonalError::Read(e.to_string()))?;
            self.file.read_exact(&mut self.bytes).map_err(|e| ZonalError::Read(e.to_string()))?;
            let t: Vec<f32> = self.bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
            if self.order.len() >= TILE_CACHE
                && let Some(old) = self.order.pop_front()
            {
                self.cache.remove(&old);
            }
            self.order.push_back((tx, ty));
            self.cache.insert((tx, ty), t);
        }
        Ok(&self.cache[&(tx, ty)])
    }
}

impl RasterSource for TiledFile {
    fn grid(&self) -> &Grid {
        &self.grid
    }
    fn read_window(&mut self, col0: u32, row0: u32, cols: u32, rows: u32, out: &mut [f32]) -> Result<(), ZonalError> {
        for ty in row0 / TILE..=(row0 + rows - 1) / TILE {
            for tx in col0 / TILE..=(col0 + cols - 1) / TILE {
                let t = self.tile(tx, ty)?;
                let (ra, rb) = ((ty * TILE).max(row0), ((ty + 1) * TILE).min(row0 + rows));
                let (ca, cb) = ((tx * TILE).max(col0), ((tx + 1) * TILE).min(col0 + cols));
                for r in ra..rb {
                    let src = &t[((r - ty * TILE) * TILE + ca - tx * TILE) as usize..((r - ty * TILE) * TILE + cb - tx * TILE) as usize];
                    let o = ((r - row0) * cols + ca - col0) as usize;
                    out[o..o + src.len()].copy_from_slice(src);
                }
            }
        }
        Ok(())
    }
}

/// N7 with the raster read from a 2.1 GB tiled file on disk (`LM_N7_RAW` names it; written if
/// missing and removed afterwards when the test made it).
#[test]
#[ignore = "stress: run in release; writes 2.1 GB"]
fn n7_stress_from_file() {
    use std::io::Write;
    let (origin, deg, side) = ((120.85, 31.88), 1.0, 23_040u32);
    let mut synth = Synthetic::new(side, origin, deg);
    let path = std::env::var_os("LM_N7_RAW").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("lm_n7_r4_tiled.f32"));
    let across = side / TILE;
    let made = !path.exists();
    if made {
        let t = Instant::now();
        let mut f = std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(&path).unwrap());
        let mut tile = vec![0f32; (TILE * TILE) as usize];
        let mut bytes = Vec::with_capacity(tile.len() * 4);
        for ty in 0..across {
            for tx in 0..across {
                synth.read_window(tx * TILE, ty * TILE, TILE, TILE, &mut tile).unwrap();
                bytes.clear();
                bytes.extend(tile.iter().flat_map(|v| v.to_le_bytes()));
                f.write_all(&bytes).unwrap();
            }
        }
        f.flush().unwrap();
        eprintln!("wrote {} in {:?}", path.display(), t.elapsed());
    }
    let grid = synth.grid.clone();
    let file = std::fs::File::open(&path).unwrap();
    let mut src = TiledFile { grid, file, across, cache: HashMap::new(), order: Default::default(), bytes: Vec::new() };
    let geoms = tiles(60, 50, origin, deg);
    let t = Instant::now();
    let from_file = summarize(&mut src, &geoms, ValueRole::Measured, &Control::none()).unwrap();
    let el = t.elapsed();
    eprintln!("N7 from file: 3,000 areas over 23,040² cells (2.1 GB, 512 px tiles) in {el:?}");
    let in_memory = summarize(&mut synth, &geoms, ValueRole::Measured, &Control::none()).unwrap();
    assert_eq!(from_file, in_memory);
    if made {
        let _ = std::fs::remove_file(&path);
    }
    assert!(el < Duration::from_secs(300));
}
