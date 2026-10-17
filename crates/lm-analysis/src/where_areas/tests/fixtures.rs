//! N11 against `reference/find_where.py`.

use super::*;
use std::collections::HashMap;
use std::path::PathBuf;

fn fixture_path(rel: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", rel].iter().collect()
}

fn read(rel: &str) -> Raster {
    rustgis_data::raster::read_path(&fixture_path(rel)).unwrap()
}

fn text(rel: &str) -> String {
    std::fs::read_to_string(fixture_path(rel)).unwrap()
}

fn rel_err(got: f64, want: f64) -> f64 {
    (got - want).abs() / want.abs().max(1e-300)
}

/// Each edge cut into pieces no longer than `max` (straight in longitude/latitude), as shapely's
/// `segmentize` does.
fn densify(parts: &[Vec<Vec<Coord>>], max: f64) -> Vec<Vec<Vec<Coord>>> {
    parts
        .iter()
        .map(|rings| {
            rings
                .iter()
                .map(|r| {
                    let mut out = Vec::new();
                    for w in r.windows(2) {
                        let (a, b) = (w[0], w[1]);
                        let n = ((b.x - a.x).hypot(b.y - a.y) / max).ceil().max(1.0) as usize;
                        for s in 0..n {
                            let t = s as f64 / n as f64;
                            out.push(Coord::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)));
                        }
                    }
                    if let Some(&l) = r.last() {
                        out.push(l);
                    }
                    out
                })
                .collect()
        })
        .collect()
}

struct Expected {
    cells: u64,
    area_m2: f64,
    geod_area_m2: f64,
    means: Vec<f64>,
}

fn expected(case: &str) -> Vec<Expected> {
    let t = text("N11/expected_find_where.csv");
    let mut lines = t.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    lines
        .map(|l| header.iter().copied().zip(l.split(',')).collect::<HashMap<&str, &str>>())
        .filter(|row| row["case"] == case)
        .map(|row| Expected {
            cells: row["cells"].parse().unwrap(),
            area_m2: row["area_m2"].parse().unwrap(),
            geod_area_m2: row["geod_area_m2"].parse().unwrap(),
            means: ["mean_1", "mean_2"].iter().filter(|k| !row[**k].is_empty()).map(|k| row[*k].parse().unwrap()).collect(),
        })
        .collect()
}

fn labels(case: &str) -> Vec<Vec<u32>> {
    text(&format!("N11/expected_labels_{case}.csv")).lines().map(|l| l.split(',').map(|v| v.parse().unwrap()).collect()).collect()
}

/// Compares one case with the reference: cell sets (each cell centre inside exactly its own
/// area), counts, areas, Geod areas of the outlines, means, validity.
fn compare(case: &str, conditions: &mut [Condition<'_>], first: &Raster) {
    let res = find_where(conditions, &Control::none()).unwrap();
    let want = expected(case);
    let lab = labels(case);
    assert_eq!(res.areas.len(), want.len(), "case {case}: area count");
    assert_eq!(lab.len(), first.height as usize);
    let tf = first.transformer();
    let geographic = first.crs.is_geographic();
    let mut worst = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (k, (a, e)) in res.areas.iter().zip(&want).enumerate() {
        assert_eq!(a.cells, e.cells, "case {case} area {}: cells", k + 1);
        let Geometry::Polygon(parts) = &a.geometry else { panic!() };
        assert!(to_geo(&a.geometry).is_valid(), "case {case} area {}: invalid", k + 1);
        let ea = rel_err(a.area_m2, e.area_m2);
        assert!(ea < 1e-9, "case {case} area {}: area {} vs {} ({ea:e})", k + 1, a.area_m2, e.area_m2);
        // The outline's geodesic area: parallels followed by densifying to 1/64 cell (a
        // projected grid's outline already has a vertex at every cell corner).
        let dense = if geographic { densify(parts, first.transform.pixel_width.abs() / 64.0) } else { parts.clone() };
        let geod = crate::geodesic::polygon_area_perimeter(&dense).area_m2;
        let eg = rel_err(geod, e.geod_area_m2);
        let own = rel_err(geod, a.area_m2);
        assert!(eg < 1e-6, "case {case} area {}: Geod {geod} vs reference {} ({eg:e})", k + 1, e.geod_area_m2);
        assert!(own < 1e-6, "case {case} area {}: Geod {geod} vs cell sum {} ({own:e})", k + 1, a.area_m2);
        let mut em = 0.0f64;
        for (got, want) in a.means.iter().zip(&e.means) {
            em = em.max(rel_err(got.unwrap(), *want));
        }
        assert!(em < 1e-9, "case {case} area {}: means {:?} vs {:?}", k + 1, a.means, e.means);
        worst = (worst.0.max(ea), worst.1.max(eg), worst.2.max(own), worst.3.max(em));
        // Identical cell sets.
        for (r, row) in lab.iter().enumerate() {
            for (c, &l) in row.iter().enumerate() {
                let n = first.transform.to_world(c as f64 + 0.5, r as f64 + 0.5);
                let p = tf.to_lonlat(n).unwrap();
                let inside = locate_in_area(parts, p) == Location::Inside;
                assert_eq!(inside, l == (k + 1) as u32, "case {case} area {}: cell ({c}, {r})", k + 1);
            }
        }
    }
    let cells_in: u64 = lab.iter().flatten().filter(|&&l| l > 0).count() as u64;
    assert_eq!(res.cells_in, cells_in);
    eprintln!(
        "N11 {case}: {} areas; worst relative errors: area {:.1e}, Geod vs reference {:.1e}, Geod vs cell sum {:.1e}, means {:.1e}",
        res.areas.len(),
        worst.0,
        worst.1,
        worst.2,
        worst.3
    );
}

#[test]
fn n11_a_shapes_match_the_reference() {
    let r = read("N11/a_shapes.tif");
    let mut c = [Condition { band: MemoryBand::new(&r, 0).unwrap(), test: Test::Between(10.0, 20.0) }];
    compare("A", &mut c, &r);
}

#[test]
fn n11_b_two_grids_match_the_reference() {
    let first = read("N11/b_first.tif");
    let second = read("N11/b_second.tif");
    let mut c = [
        Condition { band: MemoryBand::new(&first, 0).unwrap().with_scale_offset(0.1, 0.0), test: Test::AtLeast(15.0) },
        Condition { band: MemoryBand::new(&second, 0).unwrap(), test: Test::AnyOf(vec![2.0, 4.0]) },
    ];
    compare("B", &mut c, &first);
}

#[test]
fn n11_c_projected_matches_the_reference() {
    let r = read("N11/c_utm.tif");
    assert_eq!(r.crs, Crs::Epsg(32651));
    let mut c = [Condition { band: MemoryBand::new(&r, 0).unwrap(), test: Test::AtMost(0.3) }];
    compare("C", &mut c, &r);
}
