//! Throwaway iPhone companion spike: GeoJSON decode behind a tiny C ABI for Swift.
//!
//! Product crates keep `unsafe_code = "forbid"`. This spike allows `unsafe` only for
//! the `extern "C"` surface in [`ffi`]. Ask is stubbed on the Swift side.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
// SPIKE-ONLY: C ABI for Swift. Do not copy this pattern into `crates/lm-*`.
#![allow(unsafe_code)]

mod ffi;

use rustgis_data::io::{read_geojson, to_geojson};

/// Decode GeoJSON text into a FeatureCollection JSON string and feature count.
pub fn decode_geojson_text(text: &str, name: &str) -> Result<(String, usize), String> {
    let (dataset, _report) = read_geojson(text, name).map_err(|e| e.to_string())?;
    let count = dataset.features.len();
    let json = to_geojson(&dataset);
    Ok((json.to_string(), count))
}

#[cfg(test)]
mod tests {
    use super::*;

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
