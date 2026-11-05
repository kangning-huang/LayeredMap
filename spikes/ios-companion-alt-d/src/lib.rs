//! Throwaway iPhone Alt C (persistent sheet) spike: GeoJSON decode behind a tiny C ABI for Swift.
//!
//! Product crates keep `unsafe_code = "forbid"`. This spike allows `unsafe` only for
//! the `extern "C"` surface in [`ffi`]. Ask is stubbed on the Swift side.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
// SPIKE-ONLY: C ABI for Swift. Do not copy this pattern into `crates/lm-*`.
#![allow(unsafe_code)]

pub mod columns;
mod ffi;
pub mod placement;
pub mod netcdf;
pub mod raster;

use rustgis_data::GeometryType;
use rustgis_data::io::{CsvOptions, read_csv_with, read_geojson, read_path, to_geojson};
use serde_json::json;
use std::path::Path;

/// Decode GeoJSON text into a FeatureCollection JSON string and feature count.
pub fn decode_geojson_text(text: &str, name: &str) -> Result<(String, usize), String> {
    let (dataset, _report) = read_geojson(text, name).map_err(|e| e.to_string())?;
    let count = dataset.features.len();
    let json = to_geojson(&dataset);
    Ok((json.to_string(), count))
}

/// Read any vector file `rustgis-data` reads by extension (GeoJSON, CSV with coordinates, KML,
/// KMZ, GPX, a zipped shapefile, GeoPackage, …) into an envelope for Swift:
/// `{"name", "geometry": point|line|area|table, "skipped", "unreadable", "messages", "geojson"}`.
pub fn open_path_envelope(path: &str) -> Result<(String, usize), String> {
    open_path_envelope_at(path, None, None)
}

/// [`open_path_envelope`], placing data that isn't in longitude/latitude (see [`placement`]):
/// near `near` (longitude, latitude), or in the grid `chosen`.
pub fn open_path_envelope_at(path: &str, near: Option<(f64, f64)>, chosen: Option<u16>) -> Result<(String, usize), String> {
    let p = Path::new(path);
    let (mut dataset, mut report) = read_path(p).map_err(|e| e.to_string())?;
    // A table whose coordinate columns `rustgis-data` didn't recognise by name ("reclat",
    // "decimalLongitude", "Longitude (WGS84)", a `location` column of "(41.8, -87.6)"…): find them
    // as the desktop does.
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if dataset.geometry_type == GeometryType::None && matches!(ext.as_str(), "csv" | "tsv" | "txt") {
        let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("table").to_string();
        match columns::locate(&mut dataset, &file_name) {
            Some(columns::Located::Columns { x, y }) => {
                let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&bytes);
                let opts = CsvOptions { x: Some(x.clone()), y: Some(y.clone()), ..CsvOptions::default() };
                if let Ok((ds, mut rep)) = read_csv_with(&text, &dataset.name, &opts)
                    && ds.geometry_type == GeometryType::Point
                    && !ds.features.is_empty()
                {
                    rep.messages.push(format!("Places from the columns “{y}” (latitude) and “{x}” (longitude)."));
                    dataset = ds;
                    report = rep;
                }
            }
            Some(columns::Located::OneColumn { name, skipped }) => {
                report.messages.push(format!("Places from the column “{name}”."));
                report.skipped += skipped;
            }
            Some(columns::Located::Projected { x, y }) => {
                // Read as they are (metres); `placement` below puts them in longitude/latitude.
                let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&bytes);
                let opts = CsvOptions { x: Some(x), y: Some(y), ..CsvOptions::default() };
                if let Ok((ds, rep)) = read_csv_with(&text, &dataset.name, &opts)
                    && ds.geometry_type == GeometryType::Point
                {
                    dataset = ds;
                    report = rep;
                }
            }
            None => {}
        }
    }
    if dataset.geometry_type != GeometryType::None {
        report.messages.extend(placement::tidy_lonlat(&mut dataset));
    }
    let placement = if dataset.geometry_type == GeometryType::None { None } else { placement::place(&mut dataset, p, near, chosen) };
    if placement.as_ref().is_some_and(|pl| pl["how"] != "unplaced") {
        // The reader's "give the file's coordinate system" is answered now.
        report.messages.retain(|m| !m.contains("aren't longitude/latitude") && !m.contains("doesn't seem to match"));
    }
    let count = dataset.features.len();
    let geometry = match dataset.geometry_type {
        GeometryType::Point => "point",
        GeometryType::Polyline => "line",
        GeometryType::Polygon => "area",
        GeometryType::None => "table",
    };
    let envelope = json!({
        "name": dataset.name,
        "geometry": geometry,
        "skipped": report.skipped,
        "unreadable": report.unreadable.len(),
        "messages": report.messages,
        "placement": placement,
        "geojson": to_geojson(&dataset),
    });
    Ok((envelope.to_string(), count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_csv_with_coordinates() {
        let dir = std::env::temp_dir().join(format!("lm-ios-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("quakes.csv");
        std::fs::write(&file, "place,latitude,longitude,magnitude\nA,10.5,20.25,6.1\nB,-3,40,7.2\n").expect("write");
        let (out, n) = open_path_envelope(&file.to_string_lossy()).expect("open");
        assert_eq!(n, 2);
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["geometry"], "point");
        assert_eq!(v["geojson"]["features"][1]["properties"]["magnitude"], 7.2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One file per layout the open-data survey found (2026-11): each must open as places.
    #[test]
    fn opens_tables_located_the_survey_ways() {
        let dir = std::env::temp_dir().join(format!("lm-ios-survey-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let cases: &[(&str, &str, usize)] = &[
            // The row with blank coordinates stays, without a location, as `rustgis-data` keeps any.
            ("meteorites.csv", "name,mass (g),reclat,reclong,GeoLocation\nAachen,21,50.775000,6.083330,\"(50.775, 6.08333)\"\nX,1,,,\nAarhus,720,56.18,10.23,\"(56.18, 10.23)\"\n", 3),
            ("gbif.csv", "species,decimalLatitude,decimalLongitude\na,-33.9,151.2\nb,40.7,-74.0\n", 2),
            ("census.csv", "GEOID,INTPTLAT,INTPTLON\n1,+31.2,-087.6\n2,+32.0,-086.1\n", 2),
            ("spanish.csv", "nombre,latitud,longitud\na,4.6,-74.08\nb,6.25,-75.56\n", 2),
            ("socrata_wkt.csv", "name,the_geom\na,POINT (-87.6 41.8)\nb,POINT (-87.7 41.9)\n", 2),
            ("socrata_pair.csv", "name,Location 1\na,\"(41.8, -87.6)\"\nb,\"123 Main St\nChicago, IL\n(41.9, -87.7)\"\n", 2),
        ];
        for (name, text, n) in cases {
            let file = dir.join(name);
            std::fs::write(&file, text).expect("write");
            let (out, count) = open_path_envelope(&file.to_string_lossy()).unwrap_or_else(|e| panic!("{name}: {e}"));
            let v: serde_json::Value = serde_json::from_str(&out).expect("json");
            assert_eq!(v["geometry"], "point", "{name}: {}", v["messages"]);
            assert_eq!(count, *n, "{name}");
        }
        // Metres in a national grid: not placed, and said why.
        let file = dir.join("stateplane.csv");
        std::fs::write(&file, "id,X_COORD,Y_COORD\n1,1150000.5,1890000.25\n2,1160000,1895000\n").expect("write");
        let (out, _) = open_path_envelope(&file.to_string_lossy()).expect("open");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["placement"]["how"], "unplaced", "nowhere to guess near: {v}");
        // NYC's own grid (NY Long Island, US feet), with the map near Manhattan: placed there.
        let file = dir.join("nyc.csv");
        std::fs::write(&file, "id,X Coordinate (State Plane),Y Coordinate (State Plane)\n1,987000,212000\n2,990000,215000\n").expect("write");
        let (out, _) = open_path_envelope_at(&file.to_string_lossy(), Some((-73.98, 40.75)), None).expect("open");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["placement"]["how"], "guess", "{v}");
        let c = &v["geojson"]["features"][0]["geometry"]["coordinates"];
        let (lon, lat) = (c[0].as_f64().unwrap_or(0.0), c[1].as_f64().unwrap_or(0.0));
        assert!((lon + 73.98).abs() < 0.1 && (lat - 40.75).abs() < 0.1, "landed at {lon},{lat}: {}", v["placement"]);
        println!("nyc: {}", v["placement"]);
    }

    #[test]
    fn opens_bundled_examples() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/examples/");
        for (file, kind, min) in
            [("earthquakes.csv", "point", 14_000), ("countries.geojson", "area", 170), ("volcanoes.csv", "point", 1_500)]
        {
            let (out, n) = open_path_envelope(&format!("{root}{file}")).expect(file);
            let v: serde_json::Value = serde_json::from_str(&out).expect("json");
            assert_eq!(v["geometry"], kind, "{file}");
            assert!(n >= min, "{file}: {n}");
            println!("{file}: {n} features, skipped {}, messages {}", v["skipped"], v["messages"]);
        }
    }

    #[test]
    fn open_rejects_unknown_extension() {
        assert!(open_path_envelope("/nonexistent/file.xyz").is_err());
    }

    #[test]
    fn decodes_point_feature_collection() {
        let text = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"n":1},"geometry":{"type":"Point","coordinates":[121.5,31.2]}}
        ]}"#;
        let (out, n) = decode_geojson_text(text, "t").expect("decode");
        assert_eq!(n, 1);
        assert!(out.contains("FeatureCollection"));
        assert!(out.contains("121.5"));
    }

    #[test]
    fn rejects_empty() {
        assert!(decode_geojson_text("", "t").is_err());
    }
}
