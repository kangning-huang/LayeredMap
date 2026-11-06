//! Data whose coordinates aren't longitude/latitude — metres in a national or state grid — put
//! where it belongs instead of off the edge of the world.
//!
//! In order:
//! 1. A `.prj` beside the file says the grid: use it. (A zipped Shapefile's own `.prj` is already
//!    applied by `rustgis-data`; this covers a CSV with a `.prj`, and a `.prj` the reader
//!    couldn't apply.)
//! 2. A grid the person chose ("Change…" on the layer).
//! 3. A guess, as the desktop's "Where should this data be?" makes it (`lm_profile::place_candidates`):
//!    every grid whose area of use covers `near` (where the map is, or where the person is) is
//!    tried, and the one that lands the data nearest is used. The others are offered to change to.
//! 4. Nothing to go on: say so (`unplaced`), and the app asks where the data is.

use rustgis_data::raster::{Crs, wkt::crs_from_wkt};
use rustgis_data::{Coord, Dataset, Geometry};
use serde_json::{Value, json};
use std::path::Path;

fn in_range(g: &Geometry) -> bool {
    let mut ok = true;
    g.for_each_coord(|c| ok &= c.x.is_finite() && c.y.is_finite() && c.x.abs() <= 180.0 && c.y.abs() <= 90.0);
    ok
}

/// Whether the data is in longitude/latitude: nearly every shape in range, not every one. NASA's
/// meteorite table has one find on Mars (Meridiani Planum, longitude 354°); requiring all of
/// them sent 38,000 good points off to be "placed" in a local grid.
fn is_lonlat(ds: &Dataset) -> bool {
    let shapes: Vec<&Geometry> = ds.features.iter().filter_map(|f| f.geometry.as_ref()).collect();
    shapes.is_empty() || shapes.iter().filter(|g| in_range(g)).count() as f64 >= 0.95 * shapes.len() as f64
}

/// Data in longitude/latitude with a few shapes that can't be: those come off the map (the rows
/// stay in the table), and so do points at exactly 0, 0 when there are several — a common
/// stand-in for "unknown" (6,214 of NASA's meteorites) that would otherwise pile up in the Gulf
/// of Guinea. Returns what to tell the person.
pub fn tidy_lonlat(ds: &mut Dataset) -> Vec<String> {
    if !is_lonlat(ds) {
        return Vec::new();
    }
    let mut messages = Vec::new();
    let mut off: Vec<(f64, f64)> = Vec::new();
    for f in &mut ds.features {
        if let Some(g) = &f.geometry
            && !in_range(g)
        {
            let mut first = None;
            g.for_each_coord(|c| {
                if first.is_none() {
                    first = Some((c.x, c.y));
                }
            });
            off.extend(first);
            f.geometry = None;
        }
    }
    match off.as_slice() {
        [] => {}
        [(x, y)] => messages.push(format!(
            "1 place was left off the map: its coordinates ({x}, {y}) aren't a longitude and latitude on Earth."
        )),
        [(x, y), ..] => messages.push(format!(
            "{} places were left off the map: their coordinates (such as {x}, {y}) aren't longitudes and latitudes on Earth.",
            crate::thousands(off.len())
        )),
    }
    let zero = |f: &rustgis_data::Feature| matches!(&f.geometry, Some(Geometry::Point(c)) if c.x == 0.0 && c.y == 0.0);
    let zeros = ds.features.iter().filter(|f| zero(f)).count();
    if zeros >= 2 {
        for f in &mut ds.features {
            if zero(f) {
                f.geometry = None;
            }
        }
        messages.push(format!(
            "{} rows had 0, 0 as their location — usually a stand-in for “unknown” — and were left off the map.",
            crate::thousands(zeros)
        ));
    }
    messages
}

fn grid_name(epsg: u16) -> String {
    lm_profile::crs_candidates::CANDIDATES
        .iter()
        .find(|(c, _, _)| *c == epsg)
        .map_or_else(|| format!("EPSG:{epsg}"), |(_, n, _)| (*n).to_string())
}

fn to_lonlat(crs: Crs, c: (f64, f64)) -> Option<(f64, f64)> {
    let t = crs.transformer().ok()?;
    t.to_lonlat(Coord::new(c.0, c.1)).map(|p| (p.x, p.y))
}

/// Every geometry converted from `crs`; false (and `ds` untouched) if any coordinate fails or
/// lands outside longitude/latitude.
fn convert(ds: &mut Dataset, crs: Crs) -> bool {
    let Ok(t) = crs.transformer() else { return false };
    let mut out: Vec<Option<Geometry>> = Vec::with_capacity(ds.features.len());
    for f in &ds.features {
        let Some(g) = &f.geometry else {
            out.push(None);
            continue;
        };
        let g = g.map_coords(&|c| t.to_lonlat(c).unwrap_or(Coord::new(f64::NAN, f64::NAN)));
        if !in_range(&g) {
            return false;
        }
        out.push(Some(g));
    }
    for (f, g) in ds.features.iter_mut().zip(out) {
        f.geometry = g;
    }
    true
}

fn alternatives(cands: &[lm_profile::PlaceCandidate], skip: Option<u16>) -> Vec<Value> {
    cands.iter().filter(|c| Some(c.epsg) != skip).map(|c| json!({"epsg": c.epsg, "name": c.name, "km": c.distance_km.round()})).collect()
}

/// Puts `ds` in longitude/latitude if it isn't already. `None` when there was nothing to do;
/// otherwise how it was placed, for the envelope's `placement`.
pub fn place(ds: &mut Dataset, path: &Path, near: Option<(f64, f64)>, chosen: Option<u16>) -> Option<Value> {
    if is_lonlat(ds) {
        return None;
    }
    let e = ds.extent();
    let center = ((e.min_x + e.max_x) / 2.0, (e.min_y + e.max_y) / 2.0);

    if let Some(crs) = rustgis_data::raster::read_prj(path).and_then(|w| crs_from_wkt(&w).ok())
        && chosen.is_none()
        && convert(ds, crs)
    {
        let name = if crs.epsg() > 0 { grid_name(crs.epsg()) } else { crs.label() };
        return Some(json!({"how": "prj", "name": name}));
    }

    let cands = near
        .map(|p| lm_profile::place_candidates(center, p, |code, xy| to_lonlat(Crs::from_epsg(u32::from(code))?, xy)))
        .unwrap_or_default();

    if let Some(code) = chosen
        && let Some(crs) = Crs::from_epsg(u32::from(code))
        && convert(ds, crs)
    {
        return Some(json!({"how": "chosen", "epsg": code, "name": grid_name(code), "alternatives": alternatives(&cands, Some(code))}));
    }

    for c in &cands {
        if let Some(crs) = Crs::from_epsg(u32::from(c.epsg))
            && convert(ds, crs)
        {
            return Some(json!({
                "how": "guess", "epsg": c.epsg, "name": c.name, "km": c.distance_km.round(),
                "alternatives": alternatives(&cands, Some(c.epsg)),
            }));
        }
    }
    Some(json!({"how": "unplaced", "triedNear": near.is_some()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shapefile_v4() -> Option<Dataset> {
        // V4: Shanghai's districts in CGCS2000 / 3-degree Gauss-Kruger CM 120E (EPSG:4549), .prj deleted.
        let shp = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/V4/districts_gk.shp");
        rustgis_data::io::read_path(&shp).ok().map(|(d, _)| d)
    }

    #[test]
    fn a_projected_shapefile_without_prj_is_placed_near_shanghai() {
        let Some(mut ds) = shapefile_v4() else { return };
        assert!(!is_lonlat(&ds), "the fixture should read as metres");
        let p = place(&mut ds, Path::new("/nonexistent/x.zip"), Some((121.47, 31.23)), None).expect("placed");
        assert_eq!(p["how"], "guess");
        assert_eq!(p["epsg"], 4549, "{p}");
        let e = ds.extent();
        assert!(e.min_x > 120.5 && e.max_x < 122.5 && e.min_y > 30.5 && e.max_y < 32.0, "{e:?}");
    }

    #[test]
    fn a_find_on_mars_and_unknown_zeros_dont_make_a_table_a_grid() {
        let dir = std::env::temp_dir().join(format!("lm-mars-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let csv = dir.join("meteorites.csv");
        let mut text = String::from("name,reclat,reclong\nMeridiani Planum,-1.946170,354.473330\nU1,0,0\nU2,0,0\n");
        for i in 0..40 {
            text.push_str(&format!("m{i},{},{}\n", 10.0 + f64::from(i) * 0.5, 20.0 + f64::from(i) * 0.5));
        }
        std::fs::write(&csv, text).expect("csv");
        let (out, _) = crate::open_path_envelope(&csv.to_string_lossy()).expect("open");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert!(v["placement"].is_null(), "longitude/latitude, not a grid: {}", v["placement"]);
        let msgs = v["messages"].to_string();
        assert!(msgs.contains("1 place was left off the map") && msgs.contains("354.47333"), "{msgs}");
        assert!(msgs.contains("2 rows had 0, 0"), "{msgs}");
        let located = v["geojson"]["features"].as_array().map_or(0, |a| a.iter().filter(|f| !f["geometry"].is_null()).count());
        assert_eq!(located, 40);
    }

    #[test]
    fn nowhere_to_go_on_says_unplaced() {
        let Some(mut ds) = shapefile_v4() else { return };
        let p = place(&mut ds, Path::new("/nonexistent/x.zip"), None, None).expect("raw");
        assert_eq!(p["how"], "unplaced");
        assert!(!is_lonlat(&ds), "left as it was");
    }

    #[test]
    fn a_chosen_grid_wins() {
        let Some(mut ds) = shapefile_v4() else { return };
        let p = place(&mut ds, Path::new("/nonexistent/x.zip"), Some((121.47, 31.23)), Some(4549)).expect("placed");
        assert_eq!(p["how"], "chosen");
    }

    #[test]
    fn a_csv_with_a_prj_beside_it_uses_it() {
        let dir = std::env::temp_dir().join(format!("lm-prj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        // EPSG:32651 (UTM 51N): roughly Shanghai.
        let csv = dir.join("sites.csv");
        std::fs::write(&csv, "name,easting,northing\na,354000,3456000\nb,356000,3458000\n").expect("csv");
        let prj = r#"PROJCS["WGS 84 / UTM zone 51N",GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563]],PRIMEM["Greenwich",0],UNIT["degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["latitude_of_origin",0],PARAMETER["central_meridian",123],PARAMETER["scale_factor",0.9996],PARAMETER["false_easting",500000],PARAMETER["false_northing",0],UNIT["metre",1],AUTHORITY["EPSG","32651"]]"#;
        std::fs::write(dir.join("sites.prj"), prj).expect("prj");
        let (mut ds, _) = rustgis_data::io::read_path(&csv).expect("read");
        let p = place(&mut ds, &csv, None, None).expect("placed");
        assert_eq!(p["how"], "prj", "{p}");
        let e = ds.extent();
        assert!(e.min_x > 121.0 && e.max_x < 122.0 && e.min_y > 31.0 && e.max_y < 31.5, "{e:?}");
    }
}
