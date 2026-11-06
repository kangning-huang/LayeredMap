//! Throwaway iPhone Alt C (persistent sheet) spike: GeoJSON decode behind a tiny C ABI for Swift.
//!
//! Product crates keep `unsafe_code = "forbid"`. This spike allows `unsafe` only for
//! the `extern "C"` surface in [`ffi`]. Ask is stubbed on the Swift side.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
// SPIKE-ONLY: C ABI for Swift. Do not copy this pattern into `crates/lm-*`.
#![allow(unsafe_code)]

pub mod archive;
pub mod columns;
pub mod drawable;
mod ffi;
pub mod placement;
pub mod store;
pub mod zipstream;
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
    let (v, n) = open_one(Path::new(path), near, chosen, None)?;
    Ok((v.to_string(), n))
}

/// Every layer in a file, as a JSON array of envelopes: one for most files, one per layer for a
/// `.zip` (see [`archive`]). The count is the features of all of them.
///
/// With `out_dir`, each layer is written there as files (see [`store`]) and its envelope carries
/// their paths instead of `geojson`: the app never holds a layer as one big string.
pub fn open_path_envelopes_at(path: &str, near: Option<(f64, f64)>, chosen: Option<u16>) -> Result<(String, usize), String> {
    open_path_layers(path, near, chosen, None)
}

pub fn open_path_layers(path: &str, near: Option<(f64, f64)>, chosen: Option<u16>, out_dir: Option<&Path>) -> Result<(String, usize), String> {
    let p = Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let layers = if ext == "zip" {
        open_zip(p, near, chosen, out_dir)?
    } else {
        let (v, n) = open_one(p, near, chosen, out_dir.map(|d| (d, "0")))?;
        if let Some(why) = too_big(n, cells(&v), MAX_LAYER_FEATURES, MAX_LAYER_CELLS) {
            return Err(why);
        }
        vec![(v, n)]
    };
    let n = layers.iter().map(|(_, n)| n).sum();
    Ok((serde_json::Value::Array(layers.into_iter().map(|(v, _)| v).collect()).to_string(), n))
}

/// What a phone holds at once. Measured in the simulator (2026-11): white storks, 222,661 rows ×
/// 32 columns, peaked at 2.7 GB; NYC's street trees, 683,788 × 45, at 4.9 GB; Geofabrik's Taiwan,
/// 20 layers and 2.2 million shapes, at 5.1 GB. iOS ends an app well before a phone's memory is
/// full, so beyond these the file is refused (or, from a zip, the layer left out) with a reason
/// rather than the app vanishing mid-import.
pub const MAX_LAYER_FEATURES: usize = 400_000;
pub const MAX_LAYER_CELLS: usize = 12_000_000;
pub const MAX_OPEN_FEATURES: usize = 800_000;
pub const MAX_OPEN_CELLS: usize = 16_000_000;

/// Rows × columns of an envelope: what its properties cost to hold.
fn cells(v: &serde_json::Value) -> usize {
    let n = v["count"].as_u64().unwrap_or(0) as usize;
    let cols = v["fields"].as_u64().unwrap_or(1).max(1) as usize;
    n * cols
}

fn too_big(features: usize, cells: usize, max_features: usize, max_cells: usize) -> Option<String> {
    (features > max_features || cells > max_cells).then(|| {
        format!(
            "This file has {} records — more than NimbleMap on iPhone can hold at once. Open it on the Mac, or download a smaller part: most data portals let you filter by area or date before you export.",
            thousands(features)
        )
    })
}

/// `12,345`.
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The reader's notes as a person should read them: no paths inside the app, nothing about
/// files in an archive that were never data, and no second count of rows the app already
/// counts ("N rows were left out").
fn tidy_messages(messages: &mut Vec<String>, placed: bool) {
    messages.retain(|m| {
        !(m.contains("unsupported file type") || (m.ends_with("rows had no usable coordinates") && m.split(' ').next().is_some_and(|w| w.parse::<u64>().is_ok())))
    });
    if placed {
        // The reader's "give the file's coordinate system" is answered (placed, or it was
        // longitude/latitude with a few stray rows all along).
        messages.retain(|m| !m.contains("aren't longitude/latitude") && !m.contains("doesn't seem to match"));
    }
    for m in messages.iter_mut() {
        if m.starts_with('/')
            && let Some(i) = m.find(": ")
        {
            *m = m[i + 2..].to_string();
        }
    }
}

fn open_one(
    p: &Path,
    near: Option<(f64, f64)>,
    chosen: Option<u16>,
    out: Option<(&Path, &str)>,
) -> Result<(serde_json::Value, usize), String> {
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let sheet = rustgis_data::sheets::EXTENSIONS.contains(&ext.as_str());
    // A spreadsheet goes through its own reader (`read_path` doesn't take one): the first sheet.
    let (mut dataset, mut report) = if sheet {
        let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("sheet").to_string();
        rustgis_data::sheets::read(&bytes, &name, None, &CsvOptions::default()).map_err(|e| e.to_string())?
    } else {
        read_path(p).map_err(|e| e.to_string())?
    };
    // A table whose coordinate columns `rustgis-data` didn't recognise by name ("reclat",
    // "decimalLongitude", "Longitude (WGS84)", a `location` column of "(41.8, -87.6)"…): find them
    // as the desktop does.
    if dataset.geometry_type == GeometryType::None && (matches!(ext.as_str(), "csv" | "tsv" | "txt") || sheet) {
        let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("table").to_string();
        // Read the table again from the columns found (a spreadsheet through its own reader).
        let table_name = dataset.name.clone();
        let reread = |x: String, y: String| -> Option<(rustgis_data::Dataset, rustgis_data::ReadReport)> {
            let bytes = std::fs::read(p).ok()?;
            let opts = CsvOptions { x: Some(x), y: Some(y), ..CsvOptions::default() };
            if sheet {
                rustgis_data::sheets::read(&bytes, &file_name, None, &opts).ok()
            } else {
                read_csv_with(&String::from_utf8_lossy(&bytes), &table_name, &opts).ok()
            }
        };
        let located = columns::locate(&mut dataset, &file_name);
        if matches!(located, Some(columns::Located::Columns { .. } | columns::Located::Projected { .. })) {
            // The table is read again from the columns found; the first reading has done its job.
            // Holding both at once doubled the reader's peak (white storks: ~800 MB).
            dataset.features = Vec::new();
        }
        match located {
            Some(columns::Located::Columns { x, y }) => {
                if let Some((ds, mut rep)) = reread(x.clone(), y.clone())
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
                if let Some((ds, rep)) = reread(x, y)
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
    let placed = placement.as_ref().is_none_or(|pl| pl["how"] != "unplaced");
    tidy_messages(&mut report.messages, placed && dataset.geometry_type != GeometryType::None);
    // Only shapes MapLibre will draw: one bad ring blanks a whole layer.
    let lost = if placed { drawable::make_drawable(&mut dataset) } else { 0 };
    if lost > 0 {
        report.messages.push(if lost == 1 {
            "1 shape was too broken to draw and was left off the map.".to_string()
        } else {
            format!("{} shapes were too broken to draw and were left off the map.", thousands(lost))
        });
    }
    let count = dataset.features.len();
    let geometry = match dataset.geometry_type {
        GeometryType::Point => "point",
        GeometryType::Polyline => "line",
        GeometryType::Polygon => "area",
        GeometryType::None => "table",
    };
    let mut envelope = json!({
        "name": dataset.name,
        "geometry": geometry,
        "skipped": report.skipped,
        "unreadable": report.unreadable.len(),
        "messages": report.messages,
        "placement": placement,
        "count": count,
        "fields": dataset.fields.len(),
    });
    match out {
        Some((dir, stem)) => {
            let w = store::write_layer(&dataset, dir, stem)?;
            let path = |p: &Path| serde_json::Value::String(p.to_string_lossy().into_owned());
            envelope["files"] = json!({"map": path(&w.map), "idx": path(&w.idx), "geo": path(&w.geo), "cols": path(&w.cols)});
        }
        None => envelope["geojson"] = to_geojson(&dataset),
    }
    Ok((envelope, count))
}

/// Every layer of a zip (see [`archive::plan`]), each read as a file of its own: extracted one
/// at a time into a scratch folder, so only one member is ever unpacked in memory.
fn open_zip(
    p: &Path,
    near: Option<(f64, f64)>,
    chosen: Option<u16>,
    out_dir: Option<&Path>,
) -> Result<Vec<(serde_json::Value, usize)>, String> {
    let zip_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("archive").to_string();
    // Read from the file, a member at a time (see `zipstream`): never the whole archive in memory.
    let mut file = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let entries = zipstream::entries(&mut file)?;
    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    let (members, more) = archive::plan(&names);
    if members.is_empty() {
        let shown: Vec<&str> = names.iter().filter(|n| !n.ends_with('/')).map(String::as_str).take(5).collect();
        return Err(format!("{zip_name} holds no map files NimbleMap opens (it holds {}).", shown.join(", ")));
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("lm-zip-{}-{stamp}", std::process::id()));
    let mut out: Vec<(serde_json::Value, usize)> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    let mut not_here: Vec<String> = Vec::new();
    let mut too_many: Vec<String> = Vec::new();
    let mut used = (0usize, 0usize);
    for (i, m) in members.iter().enumerate() {
        let leaf = m.path.rsplit('/').next().unwrap_or(&m.path).to_string();
        let ext = Path::new(&leaf).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "tif" | "tiff" | "nc") {
            // Map images and grids open through their own reader; not from inside an archive yet.
            not_here.push(leaf);
            continue;
        }
        let want: std::collections::HashSet<&str> = std::iter::once(m.path.as_str()).chain(m.parts.iter().map(String::as_str)).collect();
        let sub = dir.join(i.to_string());
        std::fs::create_dir_all(&sub).map_err(|e| e.to_string())?;
        // One stem, lower-case extensions: the reader pairs a .shp with its parts by name.
        let stem = Path::new(&leaf).file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string();
        for e in entries.iter().filter(|e| want.contains(e.name.as_str())) {
            let x = Path::new(&e.name).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            zipstream::extract(&mut file, e, &sub.join(format!("{stem}.{x}")))?;
        }
        let tag = i.to_string();
        let opened = open_one(&sub.join(format!("{stem}.{ext}")), near, chosen, out_dir.map(|d| (d, tag.as_str())));
        // The extracted member is done with as soon as it is read (or written out).
        let _ = std::fs::remove_dir_all(&sub);
        match opened {
            Ok((mut v, n)) => {
                let c = cells(&v);
                if n > MAX_LAYER_FEATURES || c > MAX_LAYER_CELLS || used.0 + n > MAX_OPEN_FEATURES || used.1 + c > MAX_OPEN_CELLS {
                    too_many.push(format!("{stem} ({})", thousands(n)));
                } else {
                    used = (used.0 + n, used.1 + c);
                    v["name"] = serde_json::Value::String(stem);
                    v["member"] = serde_json::Value::String(m.path.clone());
                    out.push((v, n));
                }
            }
            Err(e) => failed.push(format!("{leaf} ({e})")),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    // Layers with places first: tables beside them are the archive's paperwork.
    let mapped = out.iter().any(|(v, _)| v["geometry"] != "table");
    let mut tables: Vec<String> = Vec::new();
    if mapped {
        out.retain(|(v, _)| {
            let keep = v["geometry"] != "table";
            if !keep {
                tables.push(v["name"].as_str().unwrap_or("").to_string());
            }
            keep
        });
    }
    let Some(first) = out.first_mut() else {
        if !too_many.is_empty() {
            return Err(format!(
                "Every layer in {zip_name} is more than NimbleMap on iPhone can hold at once ({}). Open it on the Mac, or download a smaller extract.",
                too_many.join(", ")
            ));
        }
        return Err(failed.into_iter().next().unwrap_or_else(|| format!("Nothing in {zip_name} could be opened.")));
    };
    let mut notes: Vec<String> = Vec::new();
    if !tables.is_empty() {
        let shown: Vec<&str> = tables.iter().map(String::as_str).take(4).collect();
        notes.push(format!(
            "Also in the zip: {} without locations ({}{}).",
            if tables.len() == 1 { "1 table".to_string() } else { format!("{} tables", tables.len()) },
            shown.join(", "),
            if tables.len() > shown.len() { ", …" } else { "" }
        ));
    }
    if !not_here.is_empty() {
        notes.push(format!("Also in the zip, to open on their own after unzipping: {}.", not_here.join(", ")));
    }
    if more > 0 {
        notes.push(format!("{more} more layers in the zip weren't opened (at most {} at once).", archive::MAX_LAYERS));
    }
    if !too_many.is_empty() {
        notes.push(format!(
            "Left out, as more than an iPhone can hold alongside the rest: {}. Open them on the Mac, or download a smaller extract.",
            too_many.join(", ")
        ));
    }
    if !failed.is_empty() {
        notes.push(format!("Couldn't open: {}.", failed.join("; ")));
    }
    if let Some(msgs) = first.0["messages"].as_array_mut() {
        msgs.extend(notes.into_iter().map(serde_json::Value::String));
    }
    Ok(out)
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

    /// LM_FILES=<paths, one per line>: what the phone opens from each (every layer of a zip).
    #[test]
    #[ignore]
    fn open_real_files() {
        let Ok(list) = std::env::var("LM_FILES") else { return };
        for path in list.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let t = std::time::Instant::now();
            let out = std::env::var("LM_OUT").ok().map(std::path::PathBuf::from);
            match open_path_layers(path, None, None, out.as_deref()) {
                Ok((out, n)) => {
                    let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
                    let layers = v.as_array().cloned().unwrap_or_default();
                    println!("LMIOS {path}\t{} layers, {n} features, {} JSON bytes, {:.1}s", layers.len(), out.len(), t.elapsed().as_secs_f64());
                    for l in layers {
                        let feats = l["count"].as_u64().unwrap_or(0);
                        let located = l["geojson"]["features"].as_array().map_or(-1, |a| a.iter().filter(|f| !f["geometry"].is_null()).count() as i64);
                        println!("LMIOS   {} · {} · {feats} ({located} located) · placement {}", l["name"], l["geometry"], l["placement"]);
                        for m in l["messages"].as_array().cloned().unwrap_or_default() {
                            println!("LMIOS     note: {m}");
                        }
                    }
                }
                Err(e) => println!("LMIOS {path}\tERROR {e}"),
            }
        }
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
