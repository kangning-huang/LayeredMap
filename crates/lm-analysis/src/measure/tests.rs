use super::*;
use crate::overlay::locate_in_area;
use proptest::prelude::*;
use rustgis_data::Dataset;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn c(x: f64, y: f64) -> Coord {
    Coord::new(x, y)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Coord> {
    vec![c(x0, y0), c(x1, y0), c(x1, y1), c(x0, y1), c(x0, y0)]
}

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn read(rel: &str) -> Dataset {
    rustgis_data::io::read_path(&fixture(rel)).unwrap().0
}

fn csv(rel: &str) -> Vec<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(fixture(rel)).unwrap();
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head: Vec<String> = lines.next().unwrap().split(',').map(|s| s.trim().to_string()).collect();
    lines.map(|l| head.iter().cloned().zip(l.split(',').map(|s| s.trim().to_string())).collect()).collect()
}

/// The label point of an area must be strictly inside it (inside a part, not in a hole, not on
/// an edge), and inside the largest part.
fn assert_inside(g: &Geometry, what: &str) {
    let Geometry::Polygon(polys) = g else { panic!("{what}: not an area") };
    let p = label_point(g).unwrap_or_else(|| panic!("{what}: no label point"));
    assert_eq!(locate_in_area(polys, p), Location::Inside, "{what}: label point {p:?} not inside");
    let area = |rings: &Vec<Vec<Coord>>| polygon_area_perimeter(std::slice::from_ref(rings)).area_m2;
    let max = polys.iter().map(area).fold(0.0, f64::max);
    assert!(
        polys.iter().any(|rings| area(rings) >= max * (1.0 - 1e-12) && locate_in_polygon(rings, p) == Location::Inside),
        "{what}: not in the largest part"
    );
}

/// The label point of a line is one of its vertices.
fn assert_on_line(g: &Geometry, what: &str) {
    let Geometry::Line(parts) = g else { panic!("{what}: not a line") };
    let p = label_point(g).unwrap();
    assert!(parts.iter().flatten().any(|v| *v == p), "{what}: {p:?} is not a vertex");
}

fn id(d: &Dataset, row: usize) -> String {
    d.value(row, d.field_index("feature_id").or_else(|| d.field_index("island_id")).or_else(|| d.field_index("area_id")).unwrap())
        .to_string()
}

#[test]
fn n11_measurements_match_the_reference() {
    let want: BTreeMap<String, BTreeMap<String, String>> =
        csv("N11/expected_measure.csv").into_iter().map(|r| (r["feature_id"].clone(), r)).collect();
    let mut checked = 0;
    let mut worst = 0.0f64;
    for layer in ["areas", "lines", "points"] {
        let d = read(&format!("N11/{layer}.geojson"));
        for (row, f) in d.features.iter().enumerate() {
            let fid = id(&d, row);
            let w = &want[&fid];
            let g = f.geometry.as_ref().unwrap();
            let m = measurements(g);
            let num = |k: &str| w.get(k).filter(|s| !s.is_empty()).map(|s| s.parse::<f64>().unwrap());
            for (k, got) in [("area_km2", m.area_km2), ("perimeter_km", m.perimeter_km), ("length_km", m.length_km)] {
                match (got, num(k)) {
                    (Some(g), Some(x)) => {
                        let rel = (g - x).abs() / x;
                        assert!(rel <= 1e-4, "{fid} {k}: {g} vs {x}");
                        worst = worst.max(rel);
                        checked += 1;
                    }
                    // M_AM: the reference leaves the split polygon's perimeter blank.
                    (Some(_), None) if fid == "M_AM" && k == "perimeter_km" => {}
                    (None, None) => {}
                    (g, x) => panic!("{fid} {k}: {g:?} vs {x:?}"),
                }
            }
            match g {
                Geometry::Point(_) | Geometry::MultiPoint(_) => {
                    assert_eq!((m.lon, m.lat), (num("lon"), num("lat")), "{fid}: coordinates are exact");
                }
                Geometry::Line(_) => assert_on_line(g, &fid),
                Geometry::Polygon(_) => assert_inside(g, &fid),
            }
            assert_eq!((m.lon, m.lat), label_point(g).map(|p| (Some(p.x), Some(p.y))).unwrap());
        }
    }
    assert_eq!(checked, 10 * 2 - 1 + 5);
    println!("N11: largest relative error {worst:.2e}");
}

#[test]
fn label_points_of_every_fixture_area_are_inside() {
    for rel in [
        "N9/islands.geojson",
        "N10/areas.geojson",
        "N10/polygons.geojson",
        "N11/areas.geojson",
        "N1/areas.geojson",
        "N2/areas.geojson",
        "V1/districts.geojson",
        "N6/areas.geojson",
    ] {
        let Ok((d, _)) = rustgis_data::io::read_path(&fixture(rel)) else { continue };
        for (row, f) in d.features.iter().enumerate() {
            if let Some(g @ Geometry::Polygon(_)) = &f.geometry {
                assert_inside(g, &format!("{rel} row {row}"));
            }
        }
    }
    for rel in ["N9/lines.geojson", "N10/lines.geojson", "N11/lines.geojson"] {
        let d = read(rel);
        for (row, f) in d.features.iter().enumerate() {
            assert_on_line(f.geometry.as_ref().unwrap(), &format!("{rel} row {row}"));
        }
    }
}

#[test]
fn c_shapes_combs_holes_and_multipolygons() {
    // C: the centroid and the bounding-box centre lie in the mouth.
    let cshape = vec![c(0.0, 0.0), c(3.0, 0.0), c(3.0, 0.5), c(0.5, 0.5), c(0.5, 2.5), c(3.0, 2.5), c(3.0, 3.0), c(0.0, 3.0), c(0.0, 0.0)];
    assert_inside(&Geometry::Polygon(vec![vec![cshape]]), "C");
    // Comb.
    let comb = vec![
        c(10.0, 0.0),
        c(14.0, 0.0),
        c(14.0, 3.0),
        c(13.5, 3.0),
        c(13.5, 0.5),
        c(12.5, 0.5),
        c(12.5, 3.0),
        c(12.0, 3.0),
        c(12.0, 0.5),
        c(11.0, 0.5),
        c(11.0, 3.0),
        c(10.5, 3.0),
        c(10.5, 0.5),
        c(10.0, 0.5),
    ];
    assert_inside(&Geometry::Polygon(vec![vec![comb]]), "comb");
    // A square with a big hole in the middle: never in the hole.
    let frame = Geometry::Polygon(vec![vec![rect(0.0, 0.0, 1.0, 1.0), rect(0.01, 0.01, 0.99, 0.99)]]);
    assert_inside(&frame, "frame");
    // Many holes on the scan lines.
    let holes: Vec<Vec<Coord>> = std::iter::once(rect(0.0, 0.0, 10.0, 10.0))
        .chain(
            (0..9).flat_map(|i| (0..9).map(move |j| rect(f64::from(i) + 0.6, f64::from(j) + 0.6, f64::from(i) + 1.4, f64::from(j) + 1.4))),
        )
        .collect();
    assert_inside(&Geometry::Polygon(vec![holes]), "sieve");
    // Largest part wins, wherever it is listed.
    let small = vec![rect(0.0, 0.0, 0.1, 0.1)];
    let big = vec![rect(5.0, 5.0, 7.0, 7.0)];
    let g = Geometry::Polygon(vec![small.clone(), big.clone(), small]);
    let p = label_point(&g).unwrap();
    assert!(p.x > 5.0 && p.x < 7.0 && p.y > 5.0 && p.y < 7.0, "{p:?}");
    // A square's label point is near its middle (pole of inaccessibility).
    let p = label_point(&Geometry::Polygon(vec![vec![rect(0.0, 0.0, 2.0, 2.0)]])).unwrap();
    assert!((p.x - 1.0).abs() < 0.01 && (p.y - 1.0).abs() < 0.01, "{p:?}");
    // Split at 180°: inside the larger part.
    let am = Geometry::Polygon(vec![vec![rect(179.0, 0.0, 180.0, 1.0)], vec![rect(-180.0, 0.0, -179.5, 1.0)]]);
    assert_inside(&am, "antimeridian");
    assert!(label_point(&am).unwrap().x > 179.0);
    // Clockwise exterior.
    let mut cw = rect(0.0, 60.0, 1.0, 61.0);
    cw.reverse();
    assert_inside(&Geometry::Polygon(vec![vec![cw]]), "clockwise");
    // Degenerate: zero area gives a vertex (on the geometry); empty gives nothing.
    let flat = Geometry::Polygon(vec![vec![vec![c(0.0, 0.0), c(1.0, 0.0), c(2.0, 0.0), c(0.0, 0.0)]]]);
    assert_eq!(label_point(&flat), Some(c(0.0, 0.0)));
    assert_eq!(label_point(&Geometry::Polygon(vec![])), None);
    assert_eq!(measurements(&Geometry::Polygon(vec![])), Measurements::default());
}

#[test]
fn line_and_point_label_points() {
    // The vertex nearest the middle of the length, not the middle vertex by count.
    let l = Geometry::Line(vec![vec![c(0.0, 0.0), c(0.1, 0.0), c(0.2, 0.0), c(6.0, 0.0), c(10.0, 0.0)]]);
    assert_eq!(label_point(&l), Some(c(6.0, 0.0)));
    let l = Geometry::Line(vec![vec![c(0.0, 0.0), c(4.5, 0.0), c(6.0, 0.0), c(10.0, 0.0)]]);
    assert_eq!(label_point(&l), Some(c(4.5, 0.0)));
    // Parts count in order; the gap between them is not length.
    let l = Geometry::Line(vec![vec![c(0.0, 0.0), c(1.0, 0.0)], vec![c(50.0, 0.0), c(50.2, 0.0), c(52.0, 0.0)]]);
    assert_eq!(label_point(&l), Some(c(50.2, 0.0)));
    assert_eq!(label_point(&Geometry::Line(vec![vec![c(3.0, 4.0)]])), Some(c(3.0, 4.0)));
    assert_eq!(label_point(&Geometry::Line(vec![])), None);
    assert_eq!(label_point(&Geometry::Point(c(1.5, 2.5))), Some(c(1.5, 2.5)));
    assert_eq!(label_point(&Geometry::Point(c(f64::NAN, 2.5))), None);
    assert_eq!(label_point(&Geometry::MultiPoint(vec![c(f64::NAN, 0.0), c(1.0, 2.0)])), Some(c(1.0, 2.0)));
    let m = measurements(&Geometry::Point(c(1.5, 2.5)));
    assert_eq!(m, Measurements { lon: Some(1.5), lat: Some(2.5), ..Measurements::default() });
    let m = measurements(&Geometry::Line(vec![vec![c(0.0, 0.0), c(1.0, 0.0)]]));
    assert!((m.length_km.unwrap() - 111.319_490_793).abs() < 1e-6);
    assert_eq!((m.area_km2, m.perimeter_km), (None, None));
}

/// A star-shaped ring around (cx, cy) from angles and radii, optionally with a hole at the
/// centre, so the centroid often falls in the hole.
fn star(cx: f64, cy: f64, radii: &[f64], hole: Option<f64>) -> Geometry {
    let n = radii.len();
    let mut ring: Vec<Coord> = radii
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            c(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    ring.push(ring[0]);
    let mut rings = vec![ring];
    if let Some(h) = hole {
        let mut hr: Vec<Coord> = (0..12)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / 12.0;
                c(cx + h * a.cos(), cy + h * a.sin())
            })
            .collect();
        hr.push(hr[0]);
        rings.push(hr);
    }
    Geometry::Polygon(vec![rings])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn label_points_are_strictly_inside(cx in -170.0f64..170.0, cy in -80.0f64..80.0,
                                       radii in prop::collection::vec(0.2f64..2.0, 3..40), hole in prop::option::of(0.01f64..0.19)) {
        let g = star(cx, cy, &radii, hole);
        let Geometry::Polygon(polys) = &g else { panic!() };
        let p = label_point(&g).unwrap();
        prop_assert_eq!(locate_in_area(polys, p), Location::Inside, "{:?}", p);
    }

    #[test]
    fn line_label_points_are_vertices(pts in prop::collection::vec((-180.0f64..180.0, -85.0f64..85.0), 1..30)) {
        let line: Vec<Coord> = pts.iter().map(|&(x, y)| c(x, y)).collect();
        let p = label_point(&Geometry::Line(vec![line.clone()])).unwrap();
        prop_assert!(line.contains(&p));
    }
}
