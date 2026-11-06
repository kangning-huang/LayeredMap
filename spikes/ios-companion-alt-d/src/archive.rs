//! What to open from a `.zip`: every layer in it, not just the first.
//!
//! The site's downloads come zipped in many shapes: several Shapefiles (the 2024 eclipse: 8;
//! Geofabrik: 20), a GeoJSON beside its metadata (US wind turbines), a CSV beside the same table
//! as a spreadsheet (simplemaps), dozens of tables of which a few hold places (Pleiades), one
//! tab-separated table (a GBIF download), the same layers twice in two text encodings (Japan's
//! railways). The rules are the desktop's, so both apps open the same layers:
//!
//! - Folders, `__MACOSX/`, `._*` and files that aren't data are ignored.
//! - The same file name in several folders is one layer: the copy in a folder named like `utf…`
//!   wins, else the first.
//! - The same layer in several formats is one layer: geojson > gpkg > shp > csv/tsv > xlsx/xls >
//!   kml > gpx.
//! - At most [`MAX_LAYERS`].

/// The most layers opened from one archive.
pub const MAX_LAYERS: usize = 30;

const DATA: &[&str] = &["geojson", "json", "csv", "tsv", "xlsx", "xls", "kml", "gpx", "gpkg", "shp", "tif", "tiff", "nc"];
const SIDECARS: &[&str] = &["shx", "dbf", "prj", "cpg"];

/// One layer to open: its main file in the archive and the sidecars that travel with it.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub path: String,
    pub parts: Vec<String>,
}

fn ext(n: &str) -> String {
    std::path::Path::new(n).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn file_name(n: &str) -> &str {
    n.rsplit('/').next().unwrap_or(n)
}

fn stem(n: &str) -> String {
    let f = file_name(n);
    f.get(..f.len().saturating_sub(ext(n).len() + 1)).unwrap_or(f).to_lowercase()
}

/// The path without its extension, compared without case: how a .shp finds its sidecars.
fn base(n: &str) -> String {
    n.get(..n.len().saturating_sub(ext(n).len() + 1)).unwrap_or(n).to_lowercase()
}

fn rank(e: &str) -> usize {
    match e {
        "geojson" | "json" => 0,
        "gpkg" => 1,
        "shp" => 2,
        "csv" | "tsv" => 3,
        "xlsx" | "xls" => 4,
        "kml" => 5,
        "gpx" => 6,
        _ => 7,
    }
}

fn ignored(n: &str) -> bool {
    n.ends_with('/') || n.split('/').any(|p| p == "__MACOSX" || p.starts_with("._"))
}

/// The layers to open from an archive holding `names`, and how many data files were left out
/// beyond [`MAX_LAYERS`].
pub fn plan(names: &[String]) -> (Vec<Member>, usize) {
    let data: Vec<&String> = names.iter().filter(|n| !ignored(n) && DATA.contains(&ext(n).as_str())).collect();

    // The same file name in several folders: one, preferring a `utf…` folder.
    let utf = |n: &str| n.split('/').rev().skip(1).any(|p| p.to_lowercase().contains("utf"));
    let mut by_name: Vec<(String, &String)> = Vec::new();
    for n in data {
        let key = file_name(n).to_lowercase();
        match by_name.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => {
                if utf(n) && !utf(slot.1) {
                    slot.1 = n;
                }
            }
            None => by_name.push((key, n)),
        }
    }

    // The same layer in several formats: one, by format.
    let mut by_stem: Vec<(String, &String)> = Vec::new();
    for (_, n) in by_name {
        let e = ext(n);
        // A raster or NetCDF is never "the same layer" as a table of the same name.
        let key = if matches!(e.as_str(), "tif" | "tiff" | "nc") { format!("{}.{e}", stem(n)) } else { stem(n) };
        match by_stem.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => {
                if rank(&e) < rank(&ext(slot.1)) {
                    slot.1 = n;
                }
            }
            None => by_stem.push((key, n)),
        }
    }

    let total = by_stem.len();
    let members = by_stem
        .into_iter()
        .take(MAX_LAYERS)
        .map(|(_, n)| {
            let parts = if ext(n) == "shp" {
                names.iter().filter(|p| base(p) == base(n) && SIDECARS.contains(&ext(p).as_str())).cloned().collect()
            } else {
                Vec::new()
            };
            Member { path: n.clone(), parts }
        })
        .collect();
    (members, total.saturating_sub(MAX_LAYERS))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn japan_keeps_the_utf8_geojson_of_each_layer() {
        let names = s(&[
            "KS-META-N02-24.xml",
            "Shift-JIS/",
            "Shift-JIS/N02-24_Station.shp",
            "Shift-JIS/N02-24_Station.dbf",
            "Shift-JIS/N02-24_RailroadSection.shp",
            "UTF-8/N02-24_Station.shp",
            "UTF-8/N02-24_Station.dbf",
            "UTF-8/N02-24_Station.shx",
            "UTF-8/N02-24_Station.geojson",
            "UTF-8/N02-24_RailroadSection.shp",
            "UTF-8/N02-24_RailroadSection.geojson",
            "UTF-8/N02-24.xml",
        ]);
        let (m, more) = plan(&names);
        let paths: Vec<&str> = m.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(paths, ["UTF-8/N02-24_Station.geojson", "UTF-8/N02-24_RailroadSection.geojson"]);
        assert_eq!(more, 0);
    }

    #[test]
    fn csv_wins_over_the_same_table_as_a_spreadsheet_and_text_is_ignored() {
        let (m, _) = plan(&s(&["license.txt", "worldcities.csv", "worldcities.xlsx"]));
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].path, "worldcities.csv");
    }

    #[test]
    fn shapefiles_bring_their_sidecars() {
        let (m, _) = plan(&s(&["center.shp", "center.SHX", "center.dbf", "center.prj", "ppath.shp", "ppath.dbf", "__MACOSX/._center.shp"]));
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].parts.len(), 3);
    }

    #[test]
    fn many_layers_are_capped() {
        let names: Vec<String> = (0..40).map(|i| format!("t{i}.csv")).collect();
        let (m, more) = plan(&names);
        assert_eq!((m.len(), more), (MAX_LAYERS, 10));
    }
}
