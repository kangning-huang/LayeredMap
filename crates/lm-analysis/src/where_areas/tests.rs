use super::*;
use crate::control::Cancelled;
use crate::overlay::{Location, locate_in_area};
use geo::{Area, Validation};
use proptest::prelude::*;
use rustgis_data::raster::{ColorHint, GeoTransform, Raster};
use std::collections::VecDeque;

mod fixtures;
mod p4;
mod stress;

const ND: f32 = -9999.0;

fn raster(w: u32, h: u32, transform: GeoTransform, crs: Crs, data: Vec<f32>) -> Raster {
    Raster::new("t", w, h, vec![data], transform, crs, Some(f64::from(ND)), ColorHint::Gray, "Float32").unwrap()
}

fn lonlat(w: u32, h: u32, x0: f64, y0: f64, cell: f64, data: Vec<f32>) -> Raster {
    raster(w, h, GeoTransform { origin_x: x0, origin_y: y0, pixel_width: cell, pixel_height: -cell }, Crs::LonLat, data)
}

/// `#` = 1, `.` = 0, `x` = NoData, one string per row (top first).
fn picture(rows: &[&str]) -> (u32, u32, Vec<f32>) {
    let h = rows.len() as u32;
    let w = rows[0].len() as u32;
    let data = rows
        .iter()
        .flat_map(|r| {
            assert_eq!(r.len() as u32, w);
            r.chars().map(|c| match c {
                '#' => 1.0,
                '.' => 0.0,
                _ => ND,
            })
        })
        .collect();
    (w, h, data)
}

fn find(r: &Raster, test: Test) -> WhereResult {
    let mut c = [Condition { band: MemoryBand::new(r, 0).unwrap(), test }];
    find_where(&mut c, &Control::none()).unwrap()
}

/// 8-connected labels by breadth-first search, numbered in raster order of the first cell.
fn brute_labels(mask: &[bool], w: usize, h: usize) -> (Vec<u32>, usize) {
    let mut lab = vec![0u32; w * h];
    let mut n = 0;
    for start in 0..w * h {
        if !mask[start] || lab[start] != 0 {
            continue;
        }
        n += 1;
        lab[start] = n as u32;
        let mut q = VecDeque::from([start]);
        while let Some(i) = q.pop_front() {
            let (c, r) = ((i % w) as i64, (i / w) as i64);
            for dr in -1..=1 {
                for dc in -1..=1 {
                    let (cc, rr) = (c + dc, r + dr);
                    if cc < 0 || rr < 0 || cc >= w as i64 || rr >= h as i64 {
                        continue;
                    }
                    let j = rr as usize * w + cc as usize;
                    if mask[j] && lab[j] == 0 {
                        lab[j] = n as u32;
                        q.push_back(j);
                    }
                }
            }
        }
    }
    (lab, n)
}

fn to_geo(g: &Geometry) -> geo::MultiPolygon<f64> {
    let Geometry::Polygon(parts) = g else { panic!("not a polygon") };
    geo::MultiPolygon::new(
        parts
            .iter()
            .map(|rings| {
                let ls = |r: &Vec<Coord>| geo::LineString::new(r.iter().map(|c| geo::Coord { x: c.x, y: c.y }).collect());
                geo::Polygon::new(ls(&rings[0]), rings[1..].iter().map(ls).collect())
            })
            .collect(),
    )
}

/// The structural checks every result must pass: areas match an independent labelling, each
/// polygon covers exactly its cells' centres, is valid (OGC), is oriented, has closed rings, and
/// its planar area is its cell count.
fn check(res: &WhereResult, mask: &[bool], w: usize, h: usize, t: &GeoTransform) {
    let (lab, n) = brute_labels(mask, w, h);
    assert_eq!(res.areas.len(), n, "area count");
    assert_eq!(res.cells_in, mask.iter().filter(|&&b| b).count() as u64);
    assert_eq!(res.grid_cells, (w * h) as u64);
    let cell = (t.pixel_width * t.pixel_height).abs();
    for (k, a) in res.areas.iter().enumerate() {
        let want = (k + 1) as u32;
        let n_cells = lab.iter().filter(|&&l| l == want).count() as u64;
        assert_eq!(a.cells, n_cells, "area {k} cells");
        let Geometry::Polygon(parts) = &a.geometry else { panic!() };
        for rings in parts {
            for (i, r) in rings.iter().enumerate() {
                assert!(r.len() >= 5 && r.first() == r.last(), "ring closed");
                let s = signed_area(&r[..r.len() - 1]);
                assert_eq!(s > 0.0, i == 0, "exterior counter-clockwise, holes clockwise");
            }
        }
        let mp = to_geo(&a.geometry);
        assert!(mp.is_valid(), "area {k} invalid: {:?}\n{:?}", mp.validation_errors(), a.geometry);
        assert!((mp.unsigned_area() - n_cells as f64 * cell).abs() < 1e-9 * (n_cells as f64 * cell).max(1.0), "area {k} planar area");
        // Every cell centre is inside exactly its own area.
        for r in 0..h {
            for c in 0..w {
                let p = t.to_world(c as f64 + 0.5, r as f64 + 0.5);
                let loc = locate_in_area(parts, p);
                let mine = lab[r * w + c] == want;
                assert_eq!(loc == Location::Inside, mine, "area {k}, cell ({c}, {r})");
                assert_ne!(loc, Location::Boundary);
            }
        }
    }
}

fn check_picture(rows: &[&str]) -> WhereResult {
    let (w, h, data) = picture(rows);
    let mask: Vec<bool> = data.iter().map(|&v| v == 1.0).collect();
    let r = lonlat(w, h, 100.0, 30.0, 0.5, data);
    let res = find(&r, Test::AtLeast(0.5));
    check(&res, &mask, w as usize, h as usize, &r.transform);
    res
}

fn parts(a: &WhereArea) -> Vec<usize> {
    let Geometry::Polygon(p) = &a.geometry else { panic!() };
    p.iter().map(|rings| rings.len() - 1).collect()
}

// ---- Shapes -------------------------------------------------------------------------------------

#[test]
fn one_cell_is_one_square() {
    let res = check_picture(&["...", ".#.", "..."]);
    assert_eq!(res.areas.len(), 1);
    let Geometry::Polygon(p) = &res.areas[0].geometry else { panic!() };
    assert_eq!(p[0][0].len(), 5);
    assert_eq!(p[0][0][0], Coord::new(100.5, 29.5));
}

#[test]
fn a_corner_contact_is_one_area_of_two_parts() {
    let res = check_picture(&["#.", ".#"]);
    assert_eq!(res.areas.len(), 1);
    assert_eq!(parts(&res.areas[0]), vec![0, 0]);
    let res = check_picture(&[".#", "#."]);
    assert_eq!(parts(&res.areas[0]), vec![0, 0]);
}

#[test]
fn a_ring_of_cells_has_a_hole() {
    let res = check_picture(&["###", "#.#", "###"]);
    assert_eq!(parts(&res.areas[0]), vec![1]);
}

#[test]
fn a_bay_closed_at_a_corner_is_a_hole_touching_the_outline() {
    // The centre is enclosed by cells that meet at the top-right corner only.
    let res = check_picture(&["##.", "#.#", "###"]);
    assert_eq!(res.areas.len(), 1);
    assert_eq!(parts(&res.areas[0]), vec![1]);
    let res = check_picture(&[".##", "#.#", "###"]);
    assert_eq!(parts(&res.areas[0]), vec![1]);
}

#[test]
fn cells_meeting_only_at_corners_around_a_gap_leave_it_out() {
    let res = check_picture(&[".#.", "#.#", ".#."]);
    assert_eq!(res.areas.len(), 1);
    assert_eq!(parts(&res.areas[0]), vec![0, 0, 0, 0]);
}

#[test]
fn an_island_touching_a_hole_at_a_corner_is_a_part_inside_it() {
    let res = check_picture(&["#######", "##....#", "#.#...#", "#.....#", "#.....#", "#.....#", "#######"]);
    assert_eq!(res.areas.len(), 1);
    let mut p = parts(&res.areas[0]);
    p.sort();
    assert_eq!(p, vec![0, 1]);
}

#[test]
fn two_holes_meeting_at_a_corner() {
    let res = check_picture(&["####", "#.##", "##.#", "####"]);
    assert_eq!(parts(&res.areas[0]), vec![2]);
}

#[test]
fn nested_areas_are_separate() {
    let res = check_picture(&["#######", "#.....#", "#.###.#", "#.#.#.#", "#.###.#", "#.....#", "#######"]);
    assert_eq!(res.areas.len(), 2);
    assert_eq!(parts(&res.areas[0]), vec![1]);
    assert_eq!(parts(&res.areas[1]), vec![1]);
}

#[test]
fn a_checkerboard_is_one_area_of_single_cells() {
    let rows: Vec<String> = (0..6).map(|r| (0..6).map(|c| if (r + c) % 2 == 0 { '#' } else { '.' }).collect()).collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let res = check_picture(&rows);
    assert_eq!(res.areas.len(), 1);
    assert_eq!(parts(&res.areas[0]).len(), 18);
}

#[test]
fn nodata_is_out() {
    let res = check_picture(&["#x#", "###", "x.x"]);
    assert_eq!(res.areas.len(), 1);
    assert_eq!(res.areas[0].cells, 5);
}

#[test]
fn the_whole_grid_and_nothing() {
    let res = check_picture(&["###", "###"]);
    assert_eq!(res.areas.len(), 1);
    let res = check_picture(&["...", "..."]);
    assert!(res.areas.is_empty());
    assert_eq!(res.cells_in, 0);
}

#[test]
fn south_up_and_east_to_west_grids_are_oriented_too() {
    let (w, h, data) = picture(&["##.", "#.#", "###"]);
    let mask: Vec<bool> = data.iter().map(|&v| v == 1.0).collect();
    for (pw, ph) in [(0.5, 0.5), (-0.5, -0.5), (-0.5, 0.5)] {
        let t = GeoTransform { origin_x: 100.0, origin_y: 30.0, pixel_width: pw, pixel_height: ph };
        let r = raster(w, h, t, Crs::LonLat, data.clone());
        let res = find(&r, Test::AtLeast(0.5));
        check(&res, &mask, w as usize, h as usize, &t);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, ..ProptestConfig::default() })]
    #[test]
    fn random_masks_match_an_independent_labelling(w in 1usize..14, h in 1usize..12, density in 0.2f64..0.8, seed in any::<u64>()) {
        let mut s = seed | 1;
        let data: Vec<f32> = (0..w * h)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                let u = (s >> 11) as f64 / (1u64 << 53) as f64;
                if u < 0.05 { ND } else if u < density { 1.0 } else { 0.0 }
            })
            .collect();
        let mask: Vec<bool> = data.iter().map(|&v| v == 1.0).collect();
        let r = lonlat(w as u32, h as u32, -10.0, 50.0, 0.25, data);
        let res = find(&r, Test::AtLeast(0.5));
        check(&res, &mask, w, h, &r.transform);
    }
}

// ---- Values, areas, means -----------------------------------------------------------------------

#[test]
fn tests_compare_values_after_scale_and_offset() {
    let r = lonlat(5, 1, 0.0, 1.0, 1.0, vec![10.0, 20.0, 30.0, 40.0, ND]);
    let run = |t: Test, scale: f64, offset: f64| {
        let mut c = [Condition { band: MemoryBand::new(&r, 0).unwrap().with_scale_offset(scale, offset), test: t }];
        find_where(&mut c, &Control::none()).unwrap().cells_in
    };
    assert_eq!(run(Test::AtLeast(20.0), 1.0, 0.0), 3);
    assert_eq!(run(Test::AtMost(20.0), 1.0, 0.0), 2);
    assert_eq!(run(Test::Between(30.0, 20.0), 1.0, 0.0), 2);
    assert_eq!(run(Test::AnyOf(vec![10.0, 40.0, 50.0]), 1.0, 0.0), 2);
    assert_eq!(run(Test::AtLeast(2.0), 0.1, 0.0), 3);
    assert_eq!(run(Test::AtLeast(25.0), 1.0, 5.0), 3);
    // NoData never passes, whatever the test.
    assert_eq!(run(Test::AtMost(f64::INFINITY), 1.0, 0.0), 4);
}

#[test]
fn bad_conditions_are_refused() {
    let r = lonlat(2, 1, 0.0, 1.0, 1.0, vec![1.0, 2.0]);
    for t in [Test::AtLeast(f64::NAN), Test::Between(0.0, f64::NAN), Test::AnyOf(vec![])] {
        let mut c = [Condition { band: MemoryBand::new(&r, 0).unwrap(), test: t }];
        assert!(matches!(find_where(&mut c, &Control::none()), Err(ZonalError::Input(_))));
    }
    assert!(matches!(find_where(&mut [], &Control::none()), Err(ZonalError::Input(_))));
}

#[test]
fn areas_are_latitude_band_sums_and_means_are_area_weighted() {
    // One column from 60° N to 0°: cells of very different areas.
    let data: Vec<f32> = (0..6).map(|i| i as f32).collect();
    let r = lonlat(1, 6, 10.0, 60.0, 10.0, data);
    let res = find(&r, Test::AtLeast(0.0));
    assert_eq!(res.areas.len(), 1);
    let a = &res.areas[0];
    let rows: Vec<f64> = (0..6).map(|i| zonal::lat_band_area_m2(60.0 - 10.0 * i as f64, 50.0 - 10.0 * i as f64, 10.0)).collect();
    let total: f64 = rows.iter().sum();
    assert!((a.area_m2 - total).abs() <= 1e-12 * total);
    assert!((a.area_m2 - zonal::lat_band_area_m2(60.0, 0.0, 10.0)).abs() <= 1e-9 * total);
    let mean: f64 = rows.iter().enumerate().map(|(i, x)| i as f64 * x).sum::<f64>() / total;
    assert!((a.means[0].unwrap() - mean).abs() < 1e-12);
    // The plain mean of 0..5 would be 2.5; the southern (larger) cells pull it up.
    assert!(a.means[0].unwrap() > 2.6);
}

#[test]
fn other_rasters_are_sampled_at_cell_centres() {
    // First grid: 4 × 4 cells of 1°; second: 2 × 2 cells of 2°, shifted by a quarter degree.
    let first = lonlat(4, 4, 0.0, 4.0, 1.0, vec![1.0; 16]);
    let second = lonlat(2, 2, -0.25, 4.25, 2.0, vec![5.0, 0.0, ND, 5.0]);
    let mut c = [
        Condition { band: MemoryBand::new(&first, 0).unwrap(), test: Test::AtLeast(1.0) },
        Condition { band: MemoryBand::new(&second, 0).unwrap(), test: Test::AtLeast(5.0) },
    ];
    let res = find_where(&mut c, &Control::none()).unwrap();
    // Centres x = 0.5…3.5 → columns floor((x + 0.25) / 2) = 0, 0, 1, 1; likewise rows. The
    // second raster passes at its cells (0, 0) and (1, 1): two 2 × 2 blocks meeting at a corner.
    let mask = [true, true, false, false, true, true, false, false, false, false, true, true, false, false, true, true];
    check(&res, &mask, 4, 4, &first.transform);
    assert_eq!(res.areas.len(), 1);
    assert_eq!(parts(&res.areas[0]), vec![0, 0]);
    assert_eq!(res.areas[0].means[1], Some(5.0));
}

#[test]
fn a_projected_grid_is_converted_and_densified() {
    // Web Mercator, 3 × 2 cells of 10 km near 31° N.
    let t = GeoTransform { origin_x: 13_400_000.0, origin_y: 3_640_000.0, pixel_width: 10_000.0, pixel_height: -10_000.0 };
    let r = raster(3, 2, t, Crs::WebMercator, vec![1.0, 1.0, 1.0, 0.0, 1.0, 0.0]);
    let res = find(&r, Test::AtLeast(1.0));
    assert_eq!(res.areas.len(), 1);
    let a = &res.areas[0];
    let Geometry::Polygon(p) = &a.geometry else { panic!() };
    // Every cell corner along the outline: 10 cell edges, so 10 vertices and the closing one.
    assert_eq!(p[0][0].len(), 11);
    let geod = crate::geodesic::polygon_area_perimeter(p).area_m2;
    assert!((geod - a.area_m2).abs() < 1e-6 * a.area_m2, "{geod} vs {}", a.area_m2);
    // A Web Mercator cell of 10 km at 31° N is about (10 km · cos 31°)² on the ground.
    let k = 31.2f64.to_radians().cos();
    assert!((a.area_m2 / (4.0 * 1e8 * k * k) - 1.0).abs() < 0.01);
}

// ---- Streaming and control ----------------------------------------------------------------------

/// A smooth field with noise: blobs, holes, corner contacts.
fn field(w: usize, h: usize, seed: u64) -> Vec<f32> {
    let mut s = seed | 1;
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f64, (i / w) as f64);
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let noise = (s >> 11) as f64 / (1u64 << 53) as f64;
            ((x * 0.031).sin() * (y * 0.043).cos() + 0.5 * (x * 0.11 + y * 0.07).sin() + 0.6 * noise) as f32
        })
        .collect()
}

#[test]
fn areas_spanning_many_strips_match_an_independent_labelling() {
    // 4,096 columns: strips of 64 rows, so areas cross several strip boundaries.
    let (w, h) = (4096usize, 200usize);
    let data = field(w, h, 7);
    let mask: Vec<bool> = data.iter().map(|&v| v >= 0.6).collect();
    let r = lonlat(w as u32, h as u32, -180.0, 80.0, 360.0 / w as f64, data);
    let res = find(&r, Test::AtLeast(0.6));
    let (lab, n) = brute_labels(&mask, w, h);
    assert!(n > 50, "{n} areas");
    assert_eq!(res.areas.len(), n);
    let mut cells = vec![0u64; n];
    for &l in &lab {
        if l > 0 {
            cells[l as usize - 1] += 1;
        }
    }
    for (a, c) in res.areas.iter().zip(&cells) {
        assert_eq!(a.cells, *c);
        assert!(to_geo(&a.geometry).is_valid());
    }
}

#[test]
fn a_cancel_stops_the_work() {
    let data = field(512, 512, 3);
    let r = lonlat(512, 512, 0.0, 50.0, 0.01, data);
    let ctl = Control::none();
    ctl.cancel();
    let mut c = [Condition { band: MemoryBand::new(&r, 0).unwrap(), test: Test::AtLeast(0.0) }];
    assert_eq!(find_where(&mut c, &ctl), Err(ZonalError::Cancelled));
    // Cancelled from inside, at the first progress report.
    let flag = std::sync::Arc::new(std::sync::OnceLock::<Control>::new());
    let f2 = flag.clone();
    let ctl = Control::with_progress(move |p| {
        if p.done > 0
            && let Some(c) = f2.get()
        {
            c.cancel();
        }
    });
    flag.set(ctl.clone()).unwrap();
    let mut c = [Condition { band: MemoryBand::new(&r, 0).unwrap(), test: Test::AtLeast(0.0) }];
    assert_eq!(find_where(&mut c, &ctl), Err(ZonalError::Cancelled));
    let _ = Cancelled;
}

#[test]
fn progress_is_rows_then_areas() {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s = seen.clone();
    let ctl = Control::with_progress(move |p| s.lock().unwrap().push(p));
    let r = lonlat(3, 3, 0.0, 3.0, 1.0, vec![1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0]);
    let mut c = [Condition { band: MemoryBand::new(&r, 0).unwrap(), test: Test::AtLeast(1.0) }];
    find_where(&mut c, &ctl).unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.first().unwrap().phase, PHASE_CELLS);
    let last = seen.last().unwrap();
    assert_eq!((last.phase.as_str(), last.done, last.total), (PHASE_TRACE, 4, Some(4)));
}
