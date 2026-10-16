//! Shared harness for the import fuzz targets: each input is written to files in a private temp
//! folder and opened through the real pipeline (`lm_jobs::import::import_path`), exactly as a
//! file dropped on the window would be. Errors are fine; a panic, a hang or a runaway allocation
//! is a finding.

use std::path::PathBuf;

use lm_jobs::import::{Ctx, Ticket, import_path};

/// V3's Shapefile parts, the fixed companions for the `.dbf` and `.shp` targets.
pub const V3_SHP: &[u8] = include_bytes!("../../fixtures/V3/sites.shp");
pub const V3_SHX: &[u8] = include_bytes!("../../fixtures/V3/sites.shx");
pub const V3_DBF: &[u8] = include_bytes!("../../fixtures/V3/sites.dbf");

fn dir() -> PathBuf {
    std::env::temp_dir().join(format!("lm-fuzz-{}", std::process::id()))
}

/// Writes `files` (name, bytes) into an empty folder and imports the first one.
pub fn import(files: &[(&str, &[u8])]) {
    let d = dir();
    let _ = std::fs::remove_dir_all(&d);
    if std::fs::create_dir_all(&d).is_err() {
        return;
    }
    for (name, bytes) in files {
        if std::fs::write(d.join(name), bytes).is_err() {
            return;
        }
    }
    if let Some((first, _)) = files.first() {
        let _ = import_path(&d.join(first), &Ctx::detached(Ticket::default()));
    }
}
