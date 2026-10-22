//! Which of the files the user dropped or picked are opened, and where they are held.

use std::path::{Path, PathBuf};

/// What the file picker offers (the desktop importers' formats that work in the browser).
pub const PICK_EXTS: &[&str] = &[
    "geojson", "json", "csv", "tsv", "txt", "xlsx", "xls", "ods", "kml", "kmz", "gpx", "shp", "shx", "dbf", "prj", "cpg", "zip", "gpkg",
    "tif", "tiff", "nc", "nc4",
];

/// A Shapefile's companions: opened with their `.shp`, never on their own when it is there.
const SIDECARS: &[&str] = &["shx", "dbf", "prj", "cpg", "sbn", "sbx", "qmd", "qix", "fix", "xml"];

fn ext(name: &str) -> String {
    Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

fn stem(name: &str) -> String {
    let n = name.strip_suffix(".shp.xml").unwrap_or(name);
    Path::new(n).file_stem().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

/// The folder a batch of files is held under (`/dropped/3`): one per drop or pick, so a
/// Shapefile's parts sit together and two files of the same name in different drops don't mix.
pub fn batch_dir(batch: u64) -> PathBuf {
    PathBuf::from(format!("/dropped/{batch}"))
}

/// A file name as the browser gave it, made safe as one path component.
pub fn clean_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    let base: String = base.chars().map(|c| if c.is_control() { '_' } else { c }).collect();
    if base.is_empty() || base == "." || base == ".." { "file".into() } else { base }
}

/// The files of a batch to open: everything but a Shapefile's companions whose `.shp` is in the
/// batch too.
pub fn to_open(names: &[String]) -> Vec<String> {
    let shps: Vec<String> = names.iter().filter(|n| ext(n) == "shp").map(|n| stem(n)).collect();
    names.iter().filter(|n| !(SIDECARS.contains(&ext(n).as_str()) && shps.contains(&stem(n)))).cloned().collect()
}

/// The site address an example file is fetched from, relative to the page (`/examples/x y.csv`
/// → `examples/x%20y.csv`).
pub fn example_url(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let enc: String = name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    Some(format!("examples/{enc}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_shapefiles_parts_open_as_one() {
        assert_eq!(
            to_open(&v(&["Roads.shp", "roads.DBF", "roads.shx", "roads.prj", "roads.shp.xml", "a.csv"])),
            v(&["Roads.shp", "a.csv"])
        );
        // A lone .dbf is a table: opened.
        assert_eq!(to_open(&v(&["table.dbf"])), v(&["table.dbf"]));
    }

    #[test]
    fn names_are_one_component() {
        assert_eq!(clean_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_name("C:\\x\\a.csv"), "a.csv");
        assert_eq!(clean_name(".."), "file");
        assert_eq!(batch_dir(3).join(clean_name("a b.csv")), PathBuf::from("/dropped/3/a b.csv"));
    }

    #[test]
    fn example_urls_are_encoded() {
        assert_eq!(example_url(Path::new("/examples/storm tracks.gpkg")).as_deref(), Some("examples/storm%20tracks.gpkg"));
    }
}
