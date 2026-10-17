//! Puzzle P4 ("The Tea Caravan Notebook") on the real WorldClim 10′ rasters, against
//! `tools/lm/puzzles/solve.py`. Skips when the puzzle data is absent.
//!
//! `cargo test -p lm-analysis --release --lib where_areas::tests::p4 -- --ignored --nocapture`

use super::*;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn data_dir() -> Option<PathBuf> {
    let d =
        std::env::var_os("LM_PUZZLE_DATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/Volumes/LaCie/data/layeredmap-puzzles"));
    d.join("worldclim/wc2.1_10m_elev.tif").exists().then_some(d)
}

fn wc(d: &Path, name: &str) -> Raster {
    rustgis_data::raster::read_path(&d.join("worldclim").join(name)).unwrap()
}

/// Area on solve.py's sphere (R = 6371.0088 km) of an outline whose edges run along parallels
/// and meridians: −R² ∮ sin φ dλ (exact for such edges).
fn sphere_area_km2(g: &Geometry) -> f64 {
    const R_KM: f64 = 6371.0088;
    let Geometry::Polygon(parts) = g else { return 0.0 };
    let mut s = 0.0;
    for rings in parts {
        for r in rings {
            for w in r.windows(2) {
                if w[0].y == w[1].y {
                    s -= w[0].y.to_radians().sin() * (w[1].x - w[0].x).to_radians();
                }
            }
        }
    }
    R_KM * R_KM * s.abs()
}

fn centroid(g: &Geometry) -> Coord {
    use geo::Centroid;
    let c = to_geo(g).centroid().unwrap();
    Coord::new(c.x(), c.y())
}

/// The four raster clues, and optionally a fifth raster that must be 1.
fn p4_conditions<'a>(rs: [&'a Raster; 4], extra: Option<&'a Raster>) -> Vec<Condition<'a>> {
    let tests = [Test::Between(1000.0, 2000.0), Test::AtLeast(17.0), Test::AtLeast(1300.0), Test::AtLeast(10.0)];
    let mut c: Vec<Condition<'a>> =
        rs.into_iter().zip(tests).map(|(r, test)| Condition { band: MemoryBand::new(r, 0).unwrap(), test }).collect();
    if let Some(m) = extra {
        c.push(Condition { band: MemoryBand::new(m, 0).unwrap(), test: Test::AnyOf(vec![1.0]) });
    }
    c
}

#[test]
#[ignore]
fn p4_tea_hills_agree_with_the_solver() {
    let Some(d) = data_dir() else {
        eprintln!("P4: puzzle data not found (set LM_PUZZLE_DATA); skipped");
        return;
    };
    let elev = wc(&d, "wc2.1_10m_elev.tif");
    let bio1 = wc(&d, "wc2.1_10m_bio_1.tif");
    let bio12 = wc(&d, "wc2.1_10m_bio_12.tif");
    let jan = wc(&d, "wc2.1_10m_tavg_01.tif");
    let conditions = |extra| p4_conditions([&elev, &bio1, &bio12, &jan], extra);

    // The four raster clues (solve.py: 356 candidate regions after the fourth clue).
    let t = Instant::now();
    let res = find_where(&mut conditions(None), &Control::none()).unwrap();
    eprintln!("P4 four conditions: {} areas, {} cells in, {:.2} s", res.areas.len(), res.cells_in, t.elapsed().as_secs_f64());
    assert_eq!(res.areas.len(), 356);
    for a in &res.areas {
        assert!(to_geo(&a.geometry).is_valid());
    }

    // The puzzle's plan: keep the part of the areas inside the China polygon (clip, edges
    // straight in lon/lat), then the areas of at least 10,000 km².
    let mut found = rustgis_data::Dataset::new("found", rustgis_data::GeometryType::Polygon, vec![]);
    for (i, a) in res.areas.iter().enumerate() {
        found.features.push(rustgis_data::Feature { id: i as u64 + 1, geometry: Some(a.geometry.clone()), attrs: vec![] });
    }
    let (countries, _) =
        rustgis_data::io::read_path(&d.join("naturalearth/ne_10m_admin_0_countries/ne_10m_admin_0_countries.shp")).unwrap();
    let admin = countries.field_index("ADMIN").unwrap();
    let mut china_layer = rustgis_data::Dataset::new("china", rustgis_data::GeometryType::Polygon, countries.fields.clone());
    china_layer.features =
        countries.features.iter().filter(|f| matches!(&f.attrs[admin], rustgis_data::Value::Text(t) if t == "China")).cloned().collect();
    let t = Instant::now();
    let clipped = crate::clip::keep_inside(&found, &china_layer, &Control::none()).unwrap();
    let clip_secs = t.elapsed().as_secs_f64();
    let big: Vec<(usize, f64)> = clipped
        .dataset
        .features
        .iter()
        .zip(&clipped.source_rows)
        .filter_map(|(f, &r)| match &f.geometry {
            Some(Geometry::Polygon(p)) => Some((r, crate::geodesic::polygon_area_perimeter(p).area_m2)),
            _ => None,
        })
        .filter(|&(_, a)| a >= 1e10)
        .collect();
    eprintln!(
        "P4 keep_inside China: {} of {} areas left ({} dropped) in {clip_secs:.2} s; at least 10,000 km²: {:?} (area, km²)",
        clipped.dataset.features.len(),
        found.features.len(),
        clipped.dropped,
        big.iter().map(|&(r, a)| (r + 1, (a / 1e6).round())).collect::<Vec<_>>()
    );
    assert_eq!(big.len(), 1);

    // "Inside China" as solve.py applies it: rasterio.features.rasterize of the Natural Earth
    // polygon (a cell is in when its centre is), as a fifth condition.
    let china: Vec<Vec<Vec<Coord>>> = countries
        .features
        .iter()
        .filter(|f| matches!(&f.attrs[admin], rustgis_data::Value::Text(t) if t == "China"))
        .filter_map(|f| match &f.geometry {
            Some(Geometry::Polygon(p)) => Some(p.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(!china.is_empty());
    let (w, h) = (elev.width as usize, elev.height as usize);
    let tr = elev.transform;
    let (mut lo, mut hi) = (Coord::new(f64::INFINITY, f64::INFINITY), Coord::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for c in china.iter().flatten().flatten() {
        lo = Coord::new(lo.x.min(c.x), lo.y.min(c.y));
        hi = Coord::new(hi.x.max(c.x), hi.y.max(c.y));
    }
    let mut mask = vec![0f32; w * h];
    for r in 0..h {
        for c in 0..w {
            let p = tr.to_world(c as f64 + 0.5, r as f64 + 0.5);
            if p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y && locate_in_area(&china, p) != Location::Outside {
                mask[r * w + c] = 1.0;
            }
        }
    }
    let mask = Raster::new("china", w as u32, h as u32, vec![mask], tr, elev.crs, None, ColorHint::Gray, "Float32").unwrap();
    let t = Instant::now();
    let res = find_where(&mut conditions(Some(&mask)), &Control::none()).unwrap();
    eprintln!("P4 inside China: {} areas, {:.2} s", res.areas.len(), t.elapsed().as_secs_f64());
    assert_eq!(res.areas.len(), 15);
    let big: Vec<&WhereArea> = res.areas.iter().filter(|a| a.area_m2 >= 1e10).collect();
    assert_eq!(big.len(), 1, "one connected area of at least 10,000 km²");
    let a = big[0];
    let sphere = sphere_area_km2(&a.geometry);
    let c = centroid(&a.geometry);
    let simao = Coord::new(100.978, 22.781);
    let Geometry::Polygon(parts) = &a.geometry else { panic!() };
    eprintln!(
        "P4 answer: {} cells, {:.0} km² on the ellipsoid, {:.0} km² on solve.py's sphere (solve.py: 182 cells, 57,516 km²), centroid {:.3}° N {:.3}° E (solve.py: 23.037, 100.078), Simao inside: {}",
        a.cells,
        a.area_m2 / 1e6,
        sphere,
        c.y,
        c.x,
        locate_in_area(parts, simao) == Location::Inside
    );
    assert_eq!(a.cells, 182);
    assert!((sphere - 57_516.0).abs() < 0.5, "sphere area {sphere}");
    // solve.py weights cell centres by their areas; the planar centroid weights them equally.
    assert!((c.y - 23.037).abs() < 0.01 && (c.x - 100.078).abs() < 0.01);
    assert_eq!(locate_in_area(parts, simao), Location::Inside);
    let next = res.areas.iter().filter(|x| x.area_m2 < 1e10).map(|x| sphere_area_km2(&x.geometry)).fold(0.0, f64::max);
    eprintln!("P4 next largest: {next:.0} km² on the sphere (solve.py: 1,576)");
}
