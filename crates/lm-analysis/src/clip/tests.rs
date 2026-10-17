use super::*;
use crate::geodesic::{line_length_m, polygon_area_perimeter};
use rustgis_data::{Field, FieldType, GeometryType, Value};
use std::collections::HashMap;
use std::path::PathBuf;

fn fixture(rel: &str) -> String {
    let p: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", rel].iter().collect();
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn layer(rel: &str) -> Dataset {
    rustgis_data::io::read_geojson(&fixture(rel), "layer").unwrap().0
}

fn id_of(ds: &Dataset, row: usize) -> String {
    let col = ds.field_index("id").unwrap();
    match ds.value(row, col) {
        Value::Text(t) => t.clone(),
        v => panic!("id {v:?}"),
    }
}

/// Lines as parts each starting at its smaller end, parts sorted; points sorted.
fn canonical(g: &Geometry) -> String {
    let xy = |c: &Coord| format!("{:?} {:?}", c.x, c.y);
    match g {
        Geometry::Line(parts) => {
            let mut ps: Vec<Vec<Coord>> = parts
                .iter()
                .map(|p| {
                    let (a, b) = (p[0], p[p.len() - 1]);
                    if (b.x, b.y) < (a.x, a.y) { p.iter().rev().copied().collect() } else { p.clone() }
                })
                .collect();
            ps.sort_by(|a, b| a.iter().map(|c| (c.x, c.y)).partial_cmp(b.iter().map(|c| (c.x, c.y))).unwrap());
            ps.iter().map(|p| p.iter().map(xy).collect::<Vec<_>>().join(";")).collect::<Vec<_>>().join("|")
        }
        Geometry::Point(c) => xy(c),
        Geometry::MultiPoint(v) => {
            let mut v = v.clone();
            v.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
            v.iter().map(xy).collect::<Vec<_>>().join(";")
        }
        Geometry::Polygon(_) => String::new(),
    }
}

fn parts(g: &Geometry) -> usize {
    match g {
        Geometry::Point(_) => 1,
        Geometry::MultiPoint(v) => v.len(),
        Geometry::Line(p) => p.len(),
        Geometry::Polygon(p) => p.len(),
    }
}

#[test]
fn n12_matches_the_reference() {
    let mask = layer("N12/mask.geojson");
    let text = fixture("N12/expected_clip.csv");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    let rows: Vec<HashMap<&str, &str>> = lines.map(|l| header.iter().copied().zip(l.split(',')).collect()).collect();
    let mut worst_area = 0.0f64;
    let mut worst_sliver = 0.0f64;
    let mut checked = 0;
    for name in ["areas", "lines", "points"] {
        let ds = layer(&format!("N12/{name}.geojson"));
        for op in ["keep", "remove"] {
            let res =
                if op == "keep" { keep_inside(&ds, &mask, &Control::none()) } else { remove_inside(&ds, &mask, &Control::none()) }.unwrap();
            assert_eq!(res.dataset.features.len(), res.source_rows.len());
            assert_eq!(res.dataset.geometry_type, ds.geometry_type);
            let got: HashMap<String, &Feature> =
                res.source_rows.iter().zip(&res.dataset.features).map(|(&r, f)| (id_of(&ds, r), f)).collect();
            let want: Vec<&HashMap<&str, &str>> = rows.iter().filter(|r| r["op"] == op && r["layer"] == name).collect();
            assert_eq!(want.len(), ds.features.len());
            let kept = want.iter().filter(|r| r["kept"] == "true").count();
            assert_eq!(res.dataset.features.len(), kept, "{op} {name}: features left");
            assert_eq!(res.dropped as usize, ds.features.len() - kept, "{op} {name}: dropped");
            for w in want {
                let id = w["id"];
                let Some(f) = got.get(id) else {
                    assert_eq!(w["kept"], "false", "{op} {id}: dropped but the reference keeps it");
                    continue;
                };
                assert_eq!(w["kept"], "true", "{op} {id}: kept but the reference drops it");
                let g = f.geometry.as_ref().unwrap();
                assert_eq!(parts(g), w["parts"].parse::<usize>().unwrap(), "{op} {id}: parts");
                match g {
                    Geometry::Polygon(p) => {
                        let a = polygon_area_perimeter(p).area_m2;
                        let e: f64 = w["geod_area_m2"].parse().unwrap();
                        let err = (a - e).abs();
                        assert!(err <= 1.0 || err <= 1e-6 * e, "{op} {id}: area {a} vs {e}");
                        if err > 1.0 {
                            worst_area = worst_area.max(err / e);
                        } else if e < 1e6 {
                            worst_sliver = worst_sliver.max(err);
                        }
                    }
                    Geometry::Line(p) => {
                        let l = line_length_m(p);
                        let e: f64 = w["length_m"].parse().unwrap();
                        assert!((l - e).abs() <= 1e-9 * e, "{op} {id}: length {l} vs {e}");
                        assert_eq!(canonical(g), w["coords"], "{op} {id}: coordinates");
                    }
                    _ => assert_eq!(canonical(g), w["coords"], "{op} {id}: coordinates"),
                }
                // Attributes and ids are copied from the source.
                let src = &ds.features[res.source_rows[res.dataset.features.iter().position(|x| std::ptr::eq(x, *f)).unwrap()]];
                assert_eq!((f.id, &f.attrs), (src.id, &src.attrs));
                checked += 1;
            }
        }
    }
    eprintln!("N12: {checked} features compared; worst relative area error {worst_area:.1e}; worst sliver error {worst_sliver:.1e} m²");
}

fn ds(kind: GeometryType, geoms: Vec<Option<Geometry>>) -> Dataset {
    let mut d = Dataset::new("t", kind, vec![Field::new("n", FieldType::Integer)]);
    for (i, g) in geoms.into_iter().enumerate() {
        d.features.push(Feature { id: i as u64 + 1, geometry: g, attrs: vec![Value::Int(i as i64)] });
    }
    d
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Coord> {
    vec![Coord::new(x0, y0), Coord::new(x1, y0), Coord::new(x1, y1), Coord::new(x0, y1), Coord::new(x0, y0)]
}

fn area_of(g: &Geometry) -> f64 {
    let Geometry::Polygon(p) = g else { panic!() };
    polygon_area_perimeter(p).area_m2
}

#[test]
fn overlapping_mask_areas_and_overlapping_parts_count_once() {
    let mask = ds(
        GeometryType::Polygon,
        vec![
            Some(Geometry::Polygon(vec![vec![square(0.0, 0.0, 2.0, 2.0)]])),
            Some(Geometry::Polygon(vec![vec![square(1.0, 0.0, 3.0, 2.0)]])),
        ],
    );
    // A feature in the overlap, and one whose two parts overlap each other.
    let inner = Geometry::Polygon(vec![vec![square(1.25, 0.5, 1.75, 1.5)]]);
    let doubled = Geometry::Polygon(vec![vec![square(0.5, 0.5, 1.5, 1.5)], vec![square(1.0, 0.5, 2.5, 1.5)]]);
    let layer = ds(GeometryType::Polygon, vec![Some(inner.clone()), Some(doubled)]);
    let res = keep_inside(&layer, &mask, &Control::none()).unwrap();
    assert_eq!(res.dataset.features.len(), 2);
    let a0 = area_of(res.dataset.features[0].geometry.as_ref().unwrap());
    assert!((a0 - area_of(&inner)).abs() < 1e-6 * a0);
    let a1 = area_of(res.dataset.features[1].geometry.as_ref().unwrap());
    let want = area_of(&Geometry::Polygon(vec![vec![square(0.5, 0.5, 2.5, 1.5)]]));
    assert!((a1 - want).abs() < 1e-6 * want, "{a1} vs {want}");
    let res = remove_inside(&layer, &mask, &Control::none()).unwrap();
    assert_eq!((res.dataset.features.len(), res.dropped), (0, 2));
}

#[test]
fn an_empty_mask_keeps_nothing_and_removes_nothing() {
    let mask = ds(GeometryType::Polygon, vec![None]);
    let g = Geometry::Polygon(vec![vec![square(0.0, 0.0, 1.0, 1.0)]]);
    let line = Geometry::Line(vec![vec![Coord::new(0.0, 0.0), Coord::new(1.0, 1.0)]]);
    let pts = Geometry::MultiPoint(vec![Coord::new(0.0, 0.0), Coord::new(1.0, 1.0)]);
    for layer in
        [ds(GeometryType::Polygon, vec![Some(g)]), ds(GeometryType::Polyline, vec![Some(line)]), ds(GeometryType::Point, vec![Some(pts)])]
    {
        let k = keep_inside(&layer, &mask, &Control::none()).unwrap();
        assert_eq!((k.dataset.features.len(), k.dropped), (0, 1));
        let r = remove_inside(&layer, &mask, &Control::none()).unwrap();
        assert_eq!((r.dataset.features.len(), r.dropped), (1, 0));
        assert_eq!(r.dataset.features[0].geometry, layer.features[0].geometry, "unchanged");
        assert_eq!(r.source_rows, vec![0]);
    }
}

#[test]
fn a_line_along_the_mask_edge() {
    // Documents geo's clip: a stretch running exactly along the mask's edge.
    let mask = ds(GeometryType::Polygon, vec![Some(Geometry::Polygon(vec![vec![square(0.0, 0.0, 2.0, 2.0)]]))]);
    let along = Geometry::Line(vec![vec![Coord::new(0.0, -1.0), Coord::new(0.0, 3.0)]]);
    let layer = ds(GeometryType::Polyline, vec![Some(along)]);
    let k = keep_inside(&layer, &mask, &Control::none()).unwrap();
    let r = remove_inside(&layer, &mask, &Control::none()).unwrap();
    let len = |res: &ClipResult| {
        res.dataset
            .features
            .iter()
            .filter_map(|f| f.geometry.as_ref())
            .map(|g| match g {
                Geometry::Line(p) => line_length_m(p),
                _ => 0.0,
            })
            .sum::<f64>()
    };
    let total = line_length_m(&[vec![Coord::new(0.0, -1.0), Coord::new(0.0, 3.0)]]);
    let edge = line_length_m(&[vec![Coord::new(0.0, 0.0), Coord::new(0.0, 2.0)]]);
    eprintln!("along the edge: keep {:.0} m, remove {:.0} m (edge {:.0} m of {:.0} m)", len(&k), len(&r), edge, total);
    // Nothing is lost or counted twice.
    assert!((len(&k) + len(&r) - total).abs() < 1e-6 * total);
    assert!((len(&k) - edge).abs() < 1e-6 * total, "the edge stretch is kept by keep");
}

#[test]
fn points_on_the_boundary_are_inside() {
    let holed = Geometry::Polygon(vec![vec![square(0.0, 0.0, 4.0, 4.0), square(1.0, 1.0, 2.0, 2.0)]]);
    let mask = ds(GeometryType::Polygon, vec![Some(holed)]);
    let pts: Vec<Option<Geometry>> = [(0.5, 0.5), (1.5, 1.5), (1.0, 1.5), (4.0, 4.0), (5.0, 0.0), (f64::NAN, 0.0)]
        .iter()
        .map(|&(x, y)| Some(Geometry::Point(Coord::new(x, y))))
        .collect();
    let layer = ds(GeometryType::Point, pts);
    let k = keep_inside(&layer, &mask, &Control::none()).unwrap();
    assert_eq!(k.source_rows, vec![0, 2, 3]);
    let r = remove_inside(&layer, &mask, &Control::none()).unwrap();
    assert_eq!(r.source_rows, vec![1, 4]);
    assert_eq!((k.dropped, r.dropped), (3, 4));
}

#[test]
fn the_antimeridian_is_not_wrapped_and_nothing_panics() {
    // A mask written across 180° (lon 170…190) and one split at it; features on both sides.
    let across = Geometry::Polygon(vec![vec![square(170.0, -10.0, 190.0, 10.0)]]);
    let split = Geometry::Polygon(vec![vec![square(170.0, -10.0, 180.0, 10.0)], vec![square(-180.0, -10.0, -170.0, 10.0)]]);
    let feats = vec![
        Some(Geometry::Polygon(vec![vec![square(175.0, -5.0, 185.0, 5.0)]])),
        Some(Geometry::Polygon(vec![vec![square(-178.0, -5.0, -172.0, 5.0)]])),
        Some(Geometry::Line(vec![vec![Coord::new(160.0, 0.0), Coord::new(200.0, 0.0)]])),
        Some(Geometry::Point(Coord::new(-175.0, 0.0))),
        Some(Geometry::Point(Coord::new(185.0, 0.0))),
    ];
    for m in [across, split] {
        let mask = ds(GeometryType::Polygon, vec![Some(m)]);
        for kind in [GeometryType::Polygon, GeometryType::Polyline, GeometryType::Point] {
            let layer = ds(kind, feats.clone());
            let k = keep_inside(&layer, &mask, &Control::none()).unwrap();
            let r = remove_inside(&layer, &mask, &Control::none()).unwrap();
            assert_eq!(k.dataset.features.len() as u64 + k.dropped, 5);
            assert_eq!(r.dataset.features.len() as u64 + r.dropped, 5);
        }
    }
    // Planar, as given: the polygon at −178…−172 is outside the mask written as 170…190.
    let mask = ds(GeometryType::Polygon, vec![Some(Geometry::Polygon(vec![vec![square(170.0, -10.0, 190.0, 10.0)]]))]);
    let layer = ds(GeometryType::Polygon, vec![Some(Geometry::Polygon(vec![vec![square(-178.0, -5.0, -172.0, 5.0)]]))]);
    assert_eq!(keep_inside(&layer, &mask, &Control::none()).unwrap().dropped, 1);
}

#[test]
fn degenerate_inputs_dont_fail() {
    let mask = ds(
        GeometryType::Polygon,
        vec![
            Some(Geometry::Polygon(vec![vec![square(0.0, 0.0, 1.0, 1.0)]])),
            Some(Geometry::Polygon(vec![vec![vec![Coord::new(0.0, 0.0), Coord::new(f64::NAN, 1.0)]]])),
            Some(Geometry::Polygon(vec![])),
            Some(Geometry::Point(Coord::new(0.5, 0.5))),
        ],
    );
    let layer = ds(
        GeometryType::Polygon,
        vec![
            Some(Geometry::Polygon(vec![vec![vec![
                Coord::new(0.2, 0.2),
                Coord::new(0.8, 0.2),
                Coord::new(0.5, 0.2),
                Coord::new(0.2, 0.2),
            ]]])),
            Some(Geometry::Line(vec![vec![Coord::new(0.5, 0.5)]])),
            Some(Geometry::MultiPoint(vec![])),
            Some(Geometry::Polygon(vec![vec![square(0.5, 0.5, 1.5, 1.5)]])),
        ],
    );
    let k = keep_inside(&layer, &mask, &Control::none()).unwrap();
    assert_eq!(k.source_rows, vec![3]);
    let r = remove_inside(&layer, &mask, &Control::none()).unwrap();
    assert_eq!(r.source_rows, vec![3]);
}

#[test]
fn a_cancel_stops_the_work() {
    let mask = ds(GeometryType::Polygon, vec![Some(Geometry::Polygon(vec![vec![square(0.0, 0.0, 1.0, 1.0)]]))]);
    let layer = ds(GeometryType::Point, vec![Some(Geometry::Point(Coord::new(0.5, 0.5)))]);
    let ctl = Control::none();
    ctl.cancel();
    assert_eq!(keep_inside(&layer, &mask, &ctl).unwrap_err(), Cancelled);
    assert_eq!(remove_inside(&layer, &mask, &ctl).unwrap_err(), Cancelled);
}
