//! Declared companions of a user-selected geospatial file (App Sandbox related items).
//!
//! Under the App Sandbox, opening a `.shp` grants access only to that file. The companion
//! extensions declared here are what Info.plist marks with `NSIsRelatedItemType`, and what the
//! app asks for through file coordination (docs/lm/spike/sandbox-related-items.md,
//! docs/lm/app-store.md › The sandbox work).
//!
//! Pure path rules: no directory listing and no I/O, so a sandboxed build can request each
//! sibling by path even when `read_dir` on the folder would be refused.

use std::path::{Path, PathBuf};

/// Shapefile parts that travel with a `.shp` (case of the stem follows the primary file).
pub const SHAPEFILE_SIDECARS: &[&str] = &["dbf", "shx", "prj", "cpg", "qix", "sbn", "sbx", "shp.xml", "qmd"];

/// World files and metadata that travel with a GeoTIFF.
pub const GEOTIFF_SIDECARS: &[&str] = &["tfw", "tifw", "tiffw", "wld", "aux.xml", "ovr", "aux", "xml"];

/// Projection / schema sidecars that travel with a CSV or TSV.
pub const TABLE_SIDECARS: &[&str] = &["prj", "csvt"];

/// The sidecar extensions declared for a primary file, by its extension (ASCII, case-insensitive).
pub fn sidecar_extensions(primary_ext: &str) -> &'static [&'static str] {
    match primary_ext.to_ascii_lowercase().as_str() {
        "shp" => SHAPEFILE_SIDECARS,
        "tif" | "tiff" => GEOTIFF_SIDECARS,
        "csv" | "tsv" => TABLE_SIDECARS,
        _ => &[],
    }
}

/// Paths of the companions a sandboxed open of `primary` should request.
///
/// Each path keeps `primary`'s directory and stem and appends the sidecar extension (including
/// multi-dot ones such as `aux.xml`). Missing files are still listed: the coordinator decides
/// whether they exist. An empty list means this format has no declared related items.
pub fn declared_siblings(primary: &Path) -> Vec<PathBuf> {
    let Some(name) = primary.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return Vec::new();
    };
    let Some(dir) = primary.parent() else {
        return Vec::new();
    };
    sidecar_extensions(ext).iter().map(|e| dir.join(format!("{stem}.{e}"))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_shapefile_lists_its_usual_parts() {
        let got = declared_siblings(Path::new("/Users/ana/Maps/districts.shp"));
        let names: Vec<_> = got.iter().filter_map(|p| p.file_name()?.to_str()).collect();
        assert_eq!(
            names,
            [
                "districts.dbf",
                "districts.shx",
                "districts.prj",
                "districts.cpg",
                "districts.qix",
                "districts.sbn",
                "districts.sbx",
                "districts.shp.xml",
                "districts.qmd"
            ]
        );
    }

    #[test]
    fn a_geotiff_keeps_multi_dot_sidecars() {
        let got = declared_siblings(Path::new("/data/dem.tif"));
        let names: Vec<_> = got.iter().filter_map(|p| p.file_name()?.to_str()).collect();
        assert!(names.contains(&"dem.aux.xml"), "{names:?}");
        assert!(names.contains(&"dem.tfw"), "{names:?}");
    }

    #[test]
    fn a_csv_asks_for_prj_and_csvt() {
        let got = declared_siblings(Path::new("schools.csv"));
        let names: Vec<_> = got.iter().filter_map(|p| p.file_name()?.to_str()).collect();
        assert_eq!(names, ["schools.prj", "schools.csvt"]);
    }

    #[test]
    fn unrelated_extensions_have_no_siblings() {
        assert!(declared_siblings(Path::new("map.geojson")).is_empty());
        assert!(declared_siblings(Path::new("readme")).is_empty());
    }
}
