//! Plans, previews, background execution, cancellation, the stale-result guard, atomic commit and
//! compound undo. The interface reaches the engine only through here.
//!
//! Phase 1: only the developer "Load fixture…" job (WP1.6), so map feel can be tested with F1
//! before import exists (Phase 2). The full job system arrives in Phase 4.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod actions;
pub mod find;
pub mod fingerprint;
pub mod import;
pub mod inspect;
pub mod jobs;
pub mod join;
pub mod library;
pub mod plan_exec;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

pub use import::Value;
pub use rustgis_engine::data::GeometryType;
pub use rustgis_engine::map::Layer;

/// The vector files directly in `dir` (a fixture folder such as `fixtures/generated/F1`), or
/// `dir` itself when it is a file.
fn fixture_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(path)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    !name.starts_with('.')
                        && matches!(p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("geojson" | "csv"))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Reads a fixture (a file, or every GeoJSON/CSV in a folder) on a background thread, so the UI
/// thread never does file I/O. Layer ids are assigned by the map when they are added.
pub fn load_fixture(path: PathBuf) -> Receiver<Result<Vec<Layer>, String>> {
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new().name("lm-load-fixture".into()).spawn(move || {
        let files = fixture_files(&path);
        if files.is_empty() {
            let _ = tx.send(Err(format!("{}: no GeoJSON or CSV files", path.display())));
            return;
        }
        let mut layers = Vec::new();
        for f in files {
            match rustgis_engine::data::io::read_path(&f) {
                Ok((ds, _)) => layers.push(Layer::new(0, ds)),
                Err(e) => {
                    let _ = tx.send(Err(format!("{}: {e}", f.display())));
                    return;
                }
            }
        }
        let _ = tx.send(Ok(layers));
    });
    if let Err(e) = spawned {
        log::error!("could not start the fixture loader: {e}");
    }
    rx
}
