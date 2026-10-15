//! WP0.2 capability audit (throwaway): opens every fixture file with RustGIS's own readers and
//! prints one Markdown table row each — opens?, layers, features, fields, extent, time, messages.
//! Whether it "lands in the right place" is judged against each fixture's expected.json by hand
//! (docs/lm/capabilities.md).
//!
//!   cargo run --release -p lm-capability-audit -- fixtures

use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name.starts_with("expected") || name.ends_with(".md") || name.ends_with("TODO.csv") {
            continue;
        }
        if p.is_dir() {
            files(&p, out);
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if ["geojson", "json", "csv", "tsv", "shp", "zip", "gpkg", "xlsx", "tif", "tiff", "kml", "gpx"].contains(&ext.as_str()) {
            out.push(p);
        }
    }
}

fn cell(s: &str) -> String {
    let s = s.replace('|', "\\|").replace('\n', " ");
    if s.chars().count() > 90 { format!("{}…", s.chars().take(90).collect::<String>()) } else { s }
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "fixtures".into()));
    let mut list = Vec::new();
    files(&root, &mut list);
    println!("| File | Opens | Layers · features · fields | Extent (lon/lat or native) | Time | Messages |");
    println!("|---|---|---|---|---|---|");
    for p in list {
        let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let t = std::time::Instant::now();
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        if ext == "tif" || ext == "tiff" {
            match rustgis_data::raster::read_path(&p) {
                Ok(r) => {
                    let e = r.extent();
                    println!(
                        "| {rel} | yes | raster {}×{} ({} bands) | {:.3}, {:.3}, {:.3}, {:.3} | {:.2} s | crs {:?} |",
                        r.width,
                        r.height,
                        r.bands.len(),
                        e.min_x,
                        e.min_y,
                        e.max_x,
                        e.max_y,
                        t.elapsed().as_secs_f64(),
                        r.crs
                    );
                }
                Err(e) => println!("| {rel} | **no** | — | — | {:.2} s | {} |", t.elapsed().as_secs_f64(), cell(&e.to_string())),
            }
            continue;
        }
        // Skip multi-gigabyte text files' full read cost being mistaken for a failure: still read.
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let res = if rustgis_data::io::is_multi_layer_name(name) {
            std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| {
                rustgis_data::io::read_layers(name, &b)
                    .map(|ls| ls.into_iter().map(|l| (l.dataset, l.report)).collect::<Vec<_>>())
                    .map_err(|e| e.to_string())
            })
        } else {
            rustgis_data::io::read_path(&p).map(|x| vec![x]).map_err(|e| e.to_string())
        };
        let secs = t.elapsed().as_secs_f64();
        match res {
            Ok(layers) => {
                let desc: Vec<String> = layers
                    .iter()
                    .map(|(d, _)| format!("{} {:?} · {} · {}", d.name, d.geometry_type, d.features.len(), d.fields.len()))
                    .collect();
                let ext: Vec<String> = layers
                    .iter()
                    .map(|(d, _)| {
                        let e = d.extent();
                        format!("{:.3}, {:.3}, {:.3}, {:.3}", e.min_x, e.min_y, e.max_x, e.max_y)
                    })
                    .collect();
                let msgs: Vec<String> = layers
                    .iter()
                    .flat_map(|(_, r)| {
                        let mut m = r.messages.clone();
                        if r.skipped > 0 {
                            m.insert(0, format!("{} skipped", r.skipped));
                        }
                        m
                    })
                    .collect();
                println!(
                    "| {rel} | yes | {} | {} | {secs:.2} s ({:.0} MB) | {} |",
                    cell(&desc.join("; ")),
                    cell(&ext.join("; ")),
                    size as f64 / 1e6,
                    cell(&msgs.join("; "))
                );
            }
            Err(e) => println!("| {rel} | **no** | — | — | {secs:.2} s | {} |", cell(&e)),
        }
    }
}
