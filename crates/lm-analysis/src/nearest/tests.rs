use super::*;
use proptest::prelude::*;
use rustgis_data::GeometryType;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn c(x: f64, y: f64) -> Coord {
    Coord::new(x, y)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Coord> {
    vec![c(x0, y0), c(x1, y0), c(x1, y1), c(x0, y1), c(x0, y0)]
}

fn layer(geoms: Vec<Option<Geometry>>) -> Dataset {
    let mut d = Dataset::new("layer", GeometryType::Polygon, vec![]);
    for g in geoms {
        d.push_next(g, vec![]);
    }
    d
}

fn pts(cs: &[Coord]) -> Dataset {
    layer(cs.iter().map(|p| Some(Geometry::Point(*p))).collect())
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

fn csv(rel: &str) -> Vec<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(fixture(rel)).unwrap();
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head: Vec<String> = lines.next().unwrap().split(',').map(|s| s.trim().to_string()).collect();
    lines.map(|l| head.iter().cloned().zip(l.split(',').map(|s| s.trim().to_string())).collect()).collect()
}

/// A3's tolerance: 0.1% or 50 m, whichever is larger.
fn tolerance(want: f64) -> f64 {
    (want * 1e-3).max(50.0)
}

/// Runs one N9 pairing against its reference CSV; returns the largest error (m) and relative error.
fn check_n9(features: &str, fid: &str, targets: &str, tid: &str, expected: &str, same: bool) -> (f64, f64) {
    let f = read(&format!("N9/{features}.geojson"));
    let t = read(&format!("N9/{targets}.geojson"));
    let r = nearest_distance(&f, &t, same, &Control::none()).unwrap();
    assert_eq!(r.densify_m, 1000.0);
    let fids = ids(&f, fid);
    let tids = ids(&t, tid);
    let want = csv(&format!("N9/{expected}"));
    assert_eq!(want.len(), fids.len());
    let (mut worst, mut worst_rel) = (0.0f64, 0.0f64);
    for row in want {
        let i = fids.iter().position(|x| *x == row["feature_id"]).unwrap();
        let got = r.nearest[i].unwrap_or_else(|| panic!("{expected} {}: no result", row["feature_id"]));
        let d: f64 = row["distance_m"].parse().unwrap();
        assert_eq!(tids[got.target_row], row["nearest_id"], "{expected} {}: nearest ({} m vs {d} m)", row["feature_id"], got.distance_m);
        if d == 0.0 {
            assert_eq!(got.distance_m, 0.0, "{expected} {}: touching", row["feature_id"]);
        }
        let err = (got.distance_m - d).abs();
        assert!(err <= tolerance(d), "{expected} {}: {} m vs {d} m", row["feature_id"], got.distance_m);
        worst = worst.max(err);
        if d > 0.0 {
            worst_rel = worst_rel.max(err / d);
        }
    }
    println!("{expected}: largest error {worst:.3} m, {worst_rel:.2e} relative");
    (worst, worst_rel)
}

#[test]
fn n9_islands_same_layer_match_the_reference() {
    check_n9("islands", "island_id", "islands", "island_id", "expected_nearest_islands_same.csv", true);
}

#[test]
fn n9_points_same_layer_match_the_reference() {
    check_n9("points", "point_id", "points", "point_id", "expected_nearest_points_same.csv", true);
}

#[test]
fn n9_points_to_islands_match_the_reference() {
    check_n9("points", "point_id", "islands", "island_id", "expected_nearest_points_to_islands.csv", false);
}

#[test]
fn n9_points_to_lines_match_the_reference() {
    check_n9("points", "point_id", "lines", "line_id", "expected_nearest_points_to_lines.csv", false);
}

#[test]
fn n9_lines_to_islands_match_the_reference() {
    check_n9("lines", "line_id", "islands", "island_id", "expected_nearest_lines_to_islands.csv", false);
}

#[test]
fn n9_islands_to_points_match_the_reference() {
    check_n9("islands", "island_id", "points", "point_id", "expected_nearest_islands_to_points.csv", false);
}

#[test]
fn a_multipart_feature_excludes_all_its_parts() {
    // Two parts 0.0625° apart (about 7 km), another feature 0.5° away.
    let twin = Geometry::Polygon(vec![vec![rect(0.0, 0.0, 0.25, 0.25)], vec![rect(0.3125, 0.0, 0.5, 0.25)]]);
    let other = Geometry::Polygon(vec![vec![rect(1.0, 0.0, 1.25, 0.25)]]);
    let d = layer(vec![Some(twin), Some(other)]);
    let r = nearest_distance(&d, &d, true, &Control::none()).unwrap();
    let n = r.nearest[0].unwrap();
    assert_eq!(n.target_row, 1);
    assert!((n.distance_m - 55_660.0).abs() < 100.0, "{}", n.distance_m);
    assert_eq!(r.nearest[1].unwrap().target_row, 0);
    // A single feature on its own has no other.
    let one = pts(&[c(0.0, 0.0)]);
    assert_eq!(nearest_distance(&one, &one, true, &Control::none()).unwrap().nearest, vec![None]);
}

#[test]
fn touching_is_zero() {
    let sq = Geometry::Polygon(vec![vec![rect(0.0, 0.0, 1.0, 1.0), rect(0.25, 0.25, 0.75, 0.75)]]);
    let line = Geometry::Line(vec![vec![c(2.0, 0.0), c(2.0, 3.0)]]);
    let t = layer(vec![Some(sq), Some(line)]);
    let f = layer(vec![
        Some(Geometry::Point(c(0.1, 0.1))),                                    // inside
        Some(Geometry::Point(c(1.0, 0.5))),                                    // on the edge
        Some(Geometry::Point(c(0.5, 0.5))),                                    // in the hole: not 0
        Some(Geometry::Point(c(2.0, 1.5))),                                    // on the line
        Some(Geometry::Line(vec![vec![c(-1.0, 0.5), c(3.0, 0.5)]])),           // crosses both
        Some(Geometry::Polygon(vec![vec![rect(0.375, 0.375, 0.625, 0.625)]])), // in the hole
        Some(Geometry::Polygon(vec![vec![rect(-1.0, -1.0, 4.0, 4.0)]])),       // covers both
        Some(Geometry::MultiPoint(vec![c(10.0, 10.0), c(2.0, 0.0)])),          // one point on the line's end
    ]);
    let r = nearest_distance(&f, &t, false, &Control::none()).unwrap();
    let d: Vec<(f64, usize)> = r.nearest.iter().map(|n| n.map(|n| (n.distance_m, n.target_row)).unwrap()).collect();
    assert_eq!(d[0], (0.0, 0));
    assert_eq!(d[1], (0.0, 0));
    assert!((d[2].0 - 27_640.0).abs() < 100.0 && d[2].1 == 0, "{:?}", d[2]);
    assert_eq!(d[3], (0.0, 1));
    assert_eq!(d[4], (0.0, 0), "the lowest touching row");
    assert!((d[5].0 - 13_820.0).abs() < 100.0 && d[5].1 == 0, "{:?}", d[5]);
    assert_eq!(d[6], (0.0, 0));
    assert_eq!(d[7], (0.0, 1));
}

#[test]
fn across_the_antimeridian_and_at_the_pole() {
    let t = pts(&[c(179.9, 0.0), c(0.0, 90.0)]);
    let f = pts(&[c(-179.9, 0.0), c(180.0, 89.0)]);
    let r = nearest_distance(&f, &t, false, &Control::none()).unwrap();
    let a = r.nearest[0].unwrap();
    assert_eq!(a.target_row, 0);
    let s: f64 = wgs84().inverse(0.0, 179.9, 0.0, -179.9);
    assert!((a.distance_m - s).abs() < 1e-6, "{} vs {s}", a.distance_m);
    let b = r.nearest[1].unwrap();
    assert_eq!(b.target_row, 1);
    let s: f64 = wgs84().inverse(89.0, 180.0, 90.0, 0.0);
    assert!((b.distance_m - s).abs() < 1e-6, "{} vs {s}", b.distance_m);
    // A line crossing 180° unsplit: the point beside its middle is 0.25° away, not half the world.
    let l = layer(vec![Some(Geometry::Line(vec![vec![c(179.5, 0.0), c(-179.5, 0.0)]]))]);
    let r = nearest_distance(&pts(&[c(180.0, 0.25)]), &l, false, &Control::none()).unwrap();
    assert!((r.nearest[0].unwrap().distance_m - 27_640.0).abs() < 100.0, "{:?}", r.nearest[0]);
}

#[test]
fn a_line_against_points_finds_the_middle_of_a_segment() {
    // The closest approach is a target point opposite the middle of a feature segment (both
    // densified to 1 km; the point is 2 km off a 100 km segment, away from any vertex).
    let f = layer(vec![Some(Geometry::Line(vec![vec![c(0.0, 0.0), c(0.9, 0.0)]]))]);
    let t = pts(&[c(0.4504, 0.018)]);
    let r = nearest_distance(&f, &t, false, &Control::none()).unwrap();
    let s: f64 = wgs84().inverse(0.0, 0.4504, 0.018, 0.4504);
    assert!((r.nearest[0].unwrap().distance_m - s).abs() < 1.0, "{:?} vs {s}", r.nearest[0]);
}

#[test]
fn missing_geometry_and_empty_targets() {
    let mut f = pts(&[c(0.0, 0.0)]);
    f.push_next(None, vec![]);
    f.push_next(Some(Geometry::Point(c(f64::NAN, 0.0))), vec![]);
    f.push_next(Some(Geometry::MultiPoint(vec![])), vec![]);
    let mut t = pts(&[c(1.0, 0.0)]);
    t.push_next(None, vec![]);
    let r = nearest_distance(&f, &t, false, &Control::none()).unwrap();
    assert_eq!(r.without_geometry, vec![1, 2, 3]);
    assert_eq!(r.targets_without_geometry, vec![1]);
    assert_eq!(r.nearest.len(), 4);
    assert_eq!(r.nearest[0].unwrap().target_row, 0);
    assert!(r.nearest[1..].iter().all(Option::is_none));
    let none = nearest_distance(&f, &layer(vec![None]), false, &Control::none()).unwrap();
    assert!(none.nearest.iter().all(Option::is_none));
}

#[test]
fn ties_go_to_the_lower_row() {
    let t = pts(&[c(1.0, 0.0), c(-1.0, 0.0)]);
    let r = nearest_distance(&pts(&[c(0.0, 0.0)]), &t, false, &Control::none()).unwrap();
    assert_eq!(r.nearest[0].unwrap().target_row, 0);
    let t = pts(&[c(-1.0, 0.0), c(1.0, 0.0)]);
    let r = nearest_distance(&pts(&[c(0.0, 0.0)]), &t, false, &Control::none()).unwrap();
    assert_eq!(r.nearest[0].unwrap().target_row, 0);
}

#[test]
fn the_lower_bound_is_safe() {
    // The ECEF chord never exceeds the geodesic distance, and is tight up to 1,000 km.
    let g = wgs84();
    for (lat1, lon1, lat2, lon2) in [
        (0.0, 0.0, 0.0, 1.0),
        (0.0, 0.0, 1.0, 0.0),
        (89.0, 0.0, 89.9, 180.0),
        (45.0, 10.0, -45.0, 170.0),
        (0.0, 0.0, 0.5, 179.7),
        (-60.0, 30.0, -60.001, 30.0),
        (30.0, 0.0, 36.0, 7.0),
    ] {
        let s: f64 = g.inverse(lat1, lon1, lat2, lon2);
        let (a, b) = (ecef(c(lon1, lat1)), ecef(c(lon2, lat2)));
        let d2: f64 = (0..3).map(|k| (a[k] - b[k]).powi(2)).sum();
        assert!(lower_bound_m(d2) <= s, "{lat1},{lon1} → {lat2},{lon2}: {} > {s}", lower_bound_m(d2));
        if s < 1_000_000.0 {
            assert!(lower_bound_m(d2) > 0.999 * s);
        }
    }
    let e2 = (2.0 - 1.0 / 298.257_223_563) / 298.257_223_563;
    assert!((RHO_MIN - 6_378_137.0 * (1.0 - e2)).abs() < 1e-6);
}

#[test]
fn a_segment_box_holds_the_whole_segment() {
    let g = wgs84();
    for (a, b) in [
        (c(0.0, 0.0), c(0.009, 0.0)),
        (c(10.0, 80.0), c(10.04, 80.003)),
        (c(179.996, -5.0), c(-179.996, -5.0)),
        (c(0.0, 89.995), c(180.0, 89.996)),
    ] {
        let (s, azi, _, _): (f64, f64, f64, f64) = g.inverse(a.y, a.x, b.y, b.x);
        let (lo, hi) = seg_box(ecef(a), ecef(b), s);
        for k in 0..=20 {
            let (lat, lon): (f64, f64) = g.direct(a.y, a.x, azi, s * f64::from(k) / 20.0);
            let n = ecef(c(lon, lat));
            assert!((0..3).all(|i| lo[i] <= n[i] && n[i] <= hi[i]), "{a:?}–{b:?} at {k}/20");
        }
    }
}

#[test]
fn leaf_discs_hold_their_leaves() {
    let line: Vec<Coord> = (0..=40).map(|k| c(10.0 + 0.05 * f64::from(k), 60.0 + 0.01 * f64::from(k % 3))).collect();
    let t = layer(vec![Some(Geometry::Line(vec![line]))]);
    let ctl = Control::none();
    let mut work = Work::new(&ctl);
    let idx = Targets::build(&t, &ctl, &mut work).unwrap();
    let mut s = Search::new(&idx);
    let g = wgs84();
    for id in 0..idx.leaves.len() {
        let (center, r) = s.leaf_disc(id, &mut work).unwrap().unwrap();
        let leaf = &idx.leaves[id];
        for seg in &idx.segs[leaf.first..leaf.first + leaf.len] {
            let (a, b) = (idx.verts[seg.a], idx.verts[seg.b]);
            let (len, azi, _, _): (f64, f64, f64, f64) = g.inverse(a.y, a.x, b.y, b.x);
            for k in 0..=10 {
                let (lat, lon): (f64, f64) = g.direct(a.y, a.x, azi, len * f64::from(k) / 10.0);
                assert!(geodesic_m(center, c(lon, lat)) <= r + 1e-6);
            }
        }
    }
}

#[test]
fn many_chunks_give_the_same_answer() {
    // 40,000 target points: more than one R-tree chunk of runs.
    let mut rnd = rng(11);
    let t = pts(&(0..40_000).map(|_| c(rnd() * 360.0 - 180.0, rnd() * 180.0 - 90.0)).collect::<Vec<_>>());
    let f = pts(&[c(0.3, 40.0), c(170.0, 0.0), c(-179.99, -89.0)]);
    let r = nearest_distance(&f, &t, false, &Control::none()).unwrap();
    let ctl = Control::none();
    let mut work = Work::new(&ctl);
    let idx = Targets::build(&t, &ctl, &mut work).unwrap();
    assert!(idx.run_trees.len() > 1, "{} chunks", idx.run_trees.len());
    for (i, p) in f.features.iter().enumerate() {
        let Some(Geometry::Point(p)) = p.geometry else { panic!() };
        let (want, row) = t
            .features
            .iter()
            .enumerate()
            .map(|(k, q)| {
                let Some(Geometry::Point(q)) = q.geometry else { panic!() };
                let s: f64 = wgs84().inverse(p.y, p.x, q.y, q.x);
                (s, k)
            })
            .fold((f64::INFINITY, 0), |a, b| if b.0 < a.0 { b } else { a });
        let got = r.nearest[i].unwrap();
        assert!((got.distance_m - want).abs() < 1e-6, "{} vs {want}", got.distance_m);
        assert_eq!(got.target_row, row);
    }
}

/// Three rings of `n` vertices, 300 km across and thousands of km apart (lakes far apart).
fn lone_rings(n: usize) -> Dataset {
    let ring = |cx: f64, cy: f64| -> Geometry {
        let mut r: Vec<Coord> = (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                c(cx + 1.5 * a.cos() / cy.to_radians().cos(), cy + 1.5 * a.sin())
            })
            .collect();
        r.push(r[0]);
        Geometry::Polygon(vec![vec![r]])
    };
    layer(vec![Some(ring(0.0, 10.0)), Some(ring(40.0, 10.0)), Some(ring(20.0, 50.0))])
}

#[test]
fn large_lone_features_skip_themselves_a_run_at_a_time() {
    let d = lone_rings(1_500);
    let r = nearest_distance(&d, &d, true, &Control::none()).unwrap();
    let ring = |k: usize| -> Vec<Coord> {
        let Some(Geometry::Polygon(p)) = &d.features[k].geometry else { panic!() };
        p[0][0].clone()
    };
    // Brute force over the facing halves' vertices (edges are 0.6 km, nearly parallel there, so
    // vertex pairs come within centimetres of the segment distance).
    let (a, b) = (ring(0), ring(1));
    let mut want = f64::INFINITY;
    for p in a.iter().filter(|p| p.x > 0.5) {
        for q in b.iter().filter(|q| q.x < 39.5) {
            want = want.min(geodesic_m(*p, *q));
        }
    }
    let n0 = r.nearest[0].unwrap();
    assert_eq!(n0.target_row, 1);
    assert!(n0.distance_m <= want + 1e-6 && want - n0.distance_m < 0.5, "{} vs {want}", n0.distance_m);
    assert_eq!(r.nearest[1].unwrap().target_row, 0);
}

/// Brute force: the definition with no index or pruning.
fn brute(f: &Geometry, targets: &[Geometry]) -> Option<(f64, usize)> {
    let ctl = Control::none();
    let mut work = Work::new(&ctl);
    let fd = Dense::of(f, &mut work).unwrap();
    let mut best = Best { d: f64::INFINITY, row: usize::MAX };
    for (row, t) in targets.iter().enumerate() {
        let one = layer(vec![Some(t.clone())]);
        let idx = Targets::build(&one, &ctl, &mut work).unwrap();
        let s = Search::new(&idx);
        if s.touching(f, None, &mut work).unwrap().is_some() {
            best.offer(0.0, row);
            continue;
        }
        for v in &fd.verts {
            for sg in &idx.segs {
                best.offer(origin_to_seg(aeqd(*v, idx.verts[sg.a]), aeqd(*v, idx.verts[sg.b])), row);
            }
        }
        for &(a, b, _) in &fd.segs {
            let (p, q) = (fd.verts[a], fd.verts[b]);
            for sg in &idx.segs {
                best.offer(seg_to_seg([0.0, 0.0], aeqd(p, q), aeqd(p, idx.verts[sg.a]), aeqd(p, idx.verts[sg.b])), row);
            }
        }
    }
    best.d.is_finite().then_some((best.d, best.row))
}

fn arb_geom() -> impl Strategy<Value = Geometry> {
    // Small features (≤ 0.05°) anywhere in a 1° box beside the antimeridian.
    let at = (179.5f64..180.5, -0.5f64..0.5);
    prop_oneof![
        at.clone().prop_map(|(x, y)| Geometry::Point(c(wrap(x), y))),
        (at.clone(), -0.05f64..0.05, -0.05f64..0.05)
            .prop_map(|((x, y), dx, dy)| Geometry::Line(vec![vec![c(wrap(x), y), c(wrap(x + dx), y + dy)]])),
        (at, 0.005f64..0.05).prop_map(|((x, y), s)| {
            let x0 = x.min(179.95);
            Geometry::Polygon(vec![vec![rect(x0, y, x0 + s, y + s)]])
        }),
    ]
}

fn wrap(x: f64) -> f64 {
    if x > 180.0 { x - 360.0 } else { x }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn index_matches_brute_force(fs in prop::collection::vec(arb_geom(), 1..4), ts in prop::collection::vec(arb_geom(), 1..6)) {
        let f = layer(fs.iter().cloned().map(Some).collect());
        let t = layer(ts.iter().cloned().map(Some).collect());
        let r = nearest_distance(&f, &t, false, &Control::none()).unwrap();
        for (i, g) in fs.iter().enumerate() {
            let want = brute(g, &ts);
            let got = r.nearest[i].map(|n| (n.distance_m, n.target_row));
            match (got, want) {
                (Some((gd, gr)), Some((wd, wr))) => {
                    prop_assert!((gd - wd).abs() <= 1e-6 * wd.max(1.0), "{gd} vs {wd}");
                    prop_assert!(gr == wr || (gd - wd).abs() <= 1e-6 * wd.max(1.0));
                }
                (g, w) => prop_assert_eq!(g, w),
            }
        }
    }
}

fn arb_long() -> impl Strategy<Value = Geometry> {
    // Lines of up to 0.6° (dozens of 1 km segments: several leaves) and multipart features.
    let at = (-3.0f64..3.0, 58.0f64..62.0);
    let path = (at.clone(), prop::collection::vec((-0.2f64..0.2, -0.1f64..0.1), 1..4)).prop_map(|((x, y), steps)| {
        let mut v = vec![c(x, y)];
        for (dx, dy) in steps {
            let l = *v.last().unwrap();
            v.push(c(l.x + dx, l.y + dy));
        }
        v
    });
    prop_oneof![
        prop::collection::vec(path.clone(), 1..3).prop_map(Geometry::Line),
        (at, 0.01f64..0.3).prop_map(|((x, y), s)| Geometry::Polygon(vec![
            vec![rect(x, y, x + 2.0 * s, y + s)],
            vec![rect(x + 3.0 * s, y, x + 3.5 * s, y + 0.5 * s)]
        ])),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn same_layer_matches_brute_force(gs in prop::collection::vec(arb_long(), 2..5)) {
        let d = layer(gs.iter().cloned().map(Some).collect());
        let r = nearest_distance(&d, &d, true, &Control::none()).unwrap();
        for (i, g) in gs.iter().enumerate() {
            let others: Vec<Geometry> = gs.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, g)| g.clone()).collect();
            let want = brute(g, &others).map(|(dist, row)| (dist, if row >= i { row + 1 } else { row }));
            let got = r.nearest[i].map(|n| (n.distance_m, n.target_row));
            match (got, want) {
                (Some((gd, gr)), Some((wd, wr))) => {
                    prop_assert!((gd - wd).abs() <= 1e-3 + 1e-7 * wd, "{gd} vs {wd}");
                    prop_assert!(gr == wr || (gd - wd).abs() <= 1e-3 + 1e-7 * wd);
                }
                (g, w) => prop_assert_eq!(g, w),
            }
        }
    }
}

#[test]
fn a_cancel_stops_the_search() {
    let t = layer(vec![Some(Geometry::Line(vec![vec![c(0.0, 0.0), c(10.0, 0.0)]]))]);
    let ctl = Control::none();
    ctl.cancel();
    assert_eq!(nearest_distance(&pts(&[c(0.0, 1.0)]), &t, false, &ctl), Err(Cancelled));
}

#[test]
fn progress_ends_at_the_total() {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s = seen.clone();
    let ctl = Control::with_progress(move |p| s.lock().unwrap().push(p));
    nearest_distance(&pts(&[c(0.0, 0.0), c(1.0, 1.0)]), &pts(&[c(2.0, 2.0)]), false, &ctl).unwrap();
    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|p| p.phase == PHASE_INDEX));
    let last = seen.last().cloned().unwrap();
    assert_eq!((last.phase.as_str(), last.done, last.total), (PHASE_SEARCH, 2, Some(2)));
}

/// A deterministic xorshift generator.
fn rng(mut s: u64) -> impl FnMut() -> f64 {
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Synthetic stand-in for Natural Earth 10m coastlines: 2,000 random-walk lines of 200 vertices
/// (400,000 vertices, steps of about 3 km, so about 1.2 million km and 1.2 million 1 km segments),
/// scattered over the globe including both sides of 180° and the polar seas, except for one
/// empty ocean.
fn synthetic_coastlines() -> Dataset {
    let mut rnd = rng(0x9E37_79B9_7F4A_7C15);
    let mut d = Dataset::new("coast", GeometryType::Polyline, vec![]);
    let mut n = 0;
    while n < 2000 {
        let mut lat = (rnd() * 2.0 - 1.0).asin().to_degrees() * 0.95;
        let mut lon = rnd() * 360.0 - 180.0;
        // An empty ocean, like the South Pacific around Point Nemo: points there are up to about
        // 3,000 km from any line, the slowest case for the search.
        if (-170.0..-100.0).contains(&lon) && (-60.0..10.0).contains(&lat) {
            continue;
        }
        n += 1;
        let mut heading = rnd() * std::f64::consts::TAU;
        let mut line = Vec::with_capacity(200);
        for _ in 0..200 {
            line.push(c(lon, lat));
            heading += (rnd() - 0.5) * 1.2;
            let step = 0.027;
            lat = (lat + step * heading.cos()).clamp(-89.0, 89.0);
            lon += step * heading.sin() / lat.to_radians().cos().max(0.05);
            if lon > 180.0 {
                lon -= 360.0;
            } else if lon < -180.0 {
                lon += 360.0;
            }
            if (line.last().unwrap().x - lon).abs() > 180.0 {
                // Crossed 180°: start a new feature there (RFC 7946 style split).
                break;
            }
        }
        d.push_next(Some(Geometry::Line(vec![line])), vec![]);
    }
    d
}

/// Performance target (A3): 10,000 points against coastline-scale lines (~400k vertices) in under
/// 30 s release. Run with
/// `cargo test --release -p lm-analysis -- --ignored --nocapture stress_nearest_10k_points_to_coastlines`.
#[test]
#[ignore]
fn stress_nearest_10k_points_to_coastlines() {
    let coast = synthetic_coastlines();
    let vertices: usize = coast.features.iter().map(|f| f.geometry.as_ref().map_or(0, Geometry::coord_count)).sum();
    let mut rnd = rng(0x2545_F491_4F6C_DD1D);
    let p = pts(&(0..10_000).map(|_| c(rnd() * 360.0 - 180.0, (rnd() * 2.0 - 1.0).asin().to_degrees())).collect::<Vec<_>>());
    let last = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
    let gap = std::sync::Arc::new(std::sync::Mutex::new(0.0f64));
    let (l, g) = (last.clone(), gap.clone());
    let ctl = Control::with_progress(move |_| {
        let mut l = l.lock().unwrap();
        let mut g = g.lock().unwrap();
        *g = g.max(l.elapsed().as_secs_f64());
        *l = std::time::Instant::now();
    });
    let t = std::time::Instant::now();
    let r = nearest_distance(&p, &coast, false, &ctl).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let mut d: Vec<f64> = r.nearest.iter().map(|n| n.unwrap().distance_m).collect();
    d.sort_by(f64::total_cmp);
    println!(
        "stress: 10,000 points to {} lines ({vertices} vertices) in {secs:.2} s; median {:.0} km, max {:.0} km; longest gap between progress reports {:.3} s",
        coast.features.len(),
        d[d.len() / 2] / 1000.0,
        d[d.len() - 1] / 1000.0,
        *gap.lock().unwrap()
    );
    // Spot-check 3 points by brute force over the densified segments.
    let ctl = Control::none();
    let mut work = Work::new(&ctl);
    let idx = Targets::build(&coast, &ctl, &mut work).unwrap();
    for i in [0, 3_333, 9_999] {
        let Some(Geometry::Point(q)) = p.features[i].geometry else { panic!() };
        let want = idx.segs.iter().map(|s| origin_to_seg(aeqd(q, idx.verts[s.a]), aeqd(q, idx.verts[s.b]))).fold(f64::INFINITY, f64::min);
        assert_eq!(r.nearest[i].unwrap().distance_m, want, "point {i}");
    }
    let t = std::time::Instant::now();
    let build_secs = {
        let t = std::time::Instant::now();
        let again = Targets::build(&coast, &ctl, &mut work).unwrap();
        let s = t.elapsed().as_secs_f64();
        drop(again);
        s
    };
    let drop_secs = t.elapsed().as_secs_f64() - build_secs;
    println!(
        "index: built in {build_secs:.2} s ({} segments, {} chunks of runs), dropped in {:.1} ms",
        idx.segs.len(),
        idx.run_trees.len(),
        drop_secs * 1000.0
    );
    assert!(vertices > 350_000, "{vertices} vertices");
    assert!(secs < 30.0, "took {secs:.1} s");
}

/// Same layer, three lone rings of 50,000 vertices each: the feature's own row must not be walked
/// segment by segment from each of its vertices.
#[test]
#[ignore]
fn stress_same_layer_lone_rings() {
    let d = lone_rings(50_000);
    let t = std::time::Instant::now();
    let r = nearest_distance(&d, &d, true, &Control::none()).unwrap();
    let secs = t.elapsed().as_secs_f64();
    println!("stress: same layer, 3 rings of 50,000 vertices in {secs:.2} s; {:?}", r.nearest);
    assert!(r.nearest.iter().all(Option::is_some));
    assert!(secs < 10.0, "took {secs:.1} s");
}

/// Cancel latency on the stress input: a cancel lands within 200 ms.
#[test]
#[ignore]
fn stress_cancel_lands_quickly() {
    let coast = synthetic_coastlines();
    let mut rnd = rng(7);
    let p = pts(&(0..10_000).map(|_| c(rnd() * 360.0 - 180.0, rnd() * 160.0 - 80.0)).collect::<Vec<_>>());
    for delay_ms in [10u64, 150, 300, 500, 700, 900] {
        let ctl = Control::none();
        let c3 = ctl.clone();
        let flag = std::sync::Arc::new(std::sync::Mutex::new(None));
        let f2 = flag.clone();
        let h = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            *f2.lock().unwrap() = Some(std::time::Instant::now());
            c3.cancel();
        });
        let r = nearest_distance(&p, &coast, false, &ctl);
        let at = flag.lock().unwrap().map(|t| t.elapsed().as_secs_f64());
        h.join().unwrap();
        if r.is_ok() && at.is_none() {
            println!("cancel after {delay_ms} ms: the work had finished");
            continue;
        }
        assert_eq!(r, Err(Cancelled));
        let lag = at.unwrap();
        println!("cancel after {delay_ms} ms landed in {:.1} ms", lag * 1000.0);
        assert!(lag < 0.2, "{lag}");
    }
}

/// A subset measured against the whole layer, skipping each feature's own row, equals the
/// same-layer result for those rows.
#[test]
fn a_subset_against_the_whole_layer_matches_the_same_layer_result() {
    use rustgis_data::{Field, FieldType, GeometryType, Value};
    let mut ds = Dataset::new("pts", GeometryType::Point, vec![Field::new("id", FieldType::Integer)]);
    for (i, (x, y)) in [(0.0, 0.0), (0.5, 0.1), (3.0, 1.0), (3.2, 1.1), (10.0, -2.0)].iter().enumerate() {
        ds.push_next(Some(Geometry::Point(Coord::new(*x, *y))), vec![Value::Int(i as i64)]);
    }
    let whole = nearest_distance(&ds, &ds, true, &Control::none()).unwrap();
    let rows = [1usize, 4];
    let mut sub = Dataset::new("sub", GeometryType::Point, ds.fields.clone());
    for r in rows {
        sub.features.push(ds.features[r].clone());
    }
    let skip: Vec<Option<usize>> = rows.iter().map(|r| Some(*r)).collect();
    let part = nearest_distance_skipping(&sub, &ds, &skip, &Control::none()).unwrap();
    for (k, r) in rows.iter().enumerate() {
        let (a, b) = (part.nearest[k].as_ref().unwrap(), whole.nearest[*r].as_ref().unwrap());
        assert_eq!(a.target_row, b.target_row);
        assert!((a.distance_m - b.distance_m).abs() < 1e-6);
    }
}
