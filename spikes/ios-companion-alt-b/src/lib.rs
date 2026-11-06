//! Throwaway iPhone Alt B (persistent sheet) spike: GeoJSON decode behind a tiny C ABI for Swift.
//!
//! Product crates keep `unsafe_code = "forbid"`. This spike allows `unsafe` only for
//! the `extern "C"` surface in [`ffi`]. Ask is stubbed on the Swift side.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
// SPIKE-ONLY: C ABI for Swift. Do not copy this pattern into `crates/lm-*`.
#![allow(unsafe_code)]

mod ffi;

use rustgis_data::GeometryType;
use rustgis_data::io::{read_geojson, read_path, to_geojson};
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
    let (dataset, report) = read_path(Path::new(path)).map_err(|e| e.to_string())?;
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
