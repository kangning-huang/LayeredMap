//! Gate 2's automated checks: profile snapshots for every V, T, C, R and X fixture, and the UI
//! stall check (no stall over 100 ms during fixture imports; over 50 ms is logged).
//!
//! Snapshots: `fixtures/snapshots/<ID>.json`. Any difference fails with a line diff;
//! `LM_UPDATE_SNAPSHOTS=1 cargo test -p lm-jobs snapshot` rewrites them (then explain every
//! changed snapshot in the gate report).

use std::path::{Path, PathBuf};

use super::tests::{fixtures, generated, generated_file, sha, stress_line};
use super::*;

fn snapshots() -> PathBuf {
    fixtures().join("snapshots")
}

fn updating() -> bool {
    std::env::var("LM_UPDATE_SNAPSHOTS").is_ok_and(|v| v == "1")
}

/// The input files of a fixture folder: data files, not expected answers or placeholders.
fn inputs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|f| {
            let n = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            f.is_file()
                && !n.starts_with('.')
                && !n.starts_with("expected")
                && !n.contains("TODO")
                && matches!(ext.as_str(), "csv" | "tsv" | "geojson" | "json" | "shp" | "zip" | "tif" | "tiff" | "gpkg" | "xlsx")
        })
        .collect();
    v.sort();
    v
}

fn file_name_of(p: &Path) -> String {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

/// Imports each file, checking that no file in its folder changes (the hash check).
fn import_all(files: &[PathBuf], failures: &mut Vec<String>) -> Vec<(String, Result<Vec<ImportedLayer>, ImportError>)> {
    files
        .iter()
        .map(|f| {
            let dir = f.parent().unwrap_or(Path::new("."));
            let siblings: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
            let before: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
            let r = import_path(f, &Ctx::detached(Ticket::default()));
            let after: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
            if before != after {
                failures.push(format!("{}: importing it changed a source file", f.display()));
            }
            (file_name_of(f), r)
        })
        .collect()
}

/// A line diff (longest common subsequence) with two lines of context.
fn diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut ops: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] > lcs[i + 1][j]) {
            ops.push(('+', b[j]));
            j += 1;
        } else {
            ops.push(('-', a[i]));
            i += 1;
        }
    }
    let near = |k: usize| ops.iter().skip(k.saturating_sub(2)).take(k.min(2) + 3).any(|(c, _)| *c != ' ');
    let mut out = String::new();
    let mut gap = false;
    for (k, (c, line)) in ops.iter().enumerate() {
        if *c != ' ' || near(k) {
            out.push_str(&format!("{c} {line}\n"));
            gap = false;
        } else if !gap {
            out.push_str("  …\n");
            gap = true;
        }
    }
    out
}

/// Compares a fixture's results with its approved snapshot (or rewrites it when updating).
/// Returns a failure message on any difference.
fn compare(id: &str, results: &[(String, Result<Vec<ImportedLayer>, ImportError>)]) -> Option<String> {
    let doc = serde_json::json!({
        "fixture": id,
        "files": results.iter().map(|(f, r)| snapshot::summary(f, r)).collect::<Vec<_>>(),
    });
    let text = format!("{}\n", serde_json::to_string_pretty(&doc).unwrap());
    let path = snapshots().join(format!("{id}.json"));
    let approved = std::fs::read_to_string(&path).ok();
    if updating() {
        if approved.as_deref() != Some(text.as_str()) {
            std::fs::create_dir_all(snapshots()).unwrap();
            std::fs::write(&path, &text).unwrap();
            eprintln!("updated {}", path.display());
        }
        return None;
    }
    match approved {
        None => Some(format!("{id}: no approved snapshot at {} (run with LM_UPDATE_SNAPSHOTS=1)", path.display())),
        Some(a) if a == text => None,
        Some(a) => Some(format!("{id}: the profile differs from {} (- approved, + now):\n{}", path.display(), diff(&a, &text))),
    }
}

/// For the large stress fixtures (T3, R4): compares and prints a STRESS line.
pub(super) fn check_generated_snapshot(id: &str, results: &[(String, Result<Vec<ImportedLayer>, ImportError>)]) -> bool {
    match compare(id, results) {
        None => stress_line(&format!("{id} snapshot"), "matches", "approved snapshot", true),
        Some(d) => {
            eprintln!("{d}");
            stress_line(&format!("{id} snapshot"), "differs", "approved snapshot", false)
        }
    }
}

/// Gate 2: every hand-made V, T, C and R fixture opens (or fails honestly) and its profile
/// matches the approved snapshot; no source file changes.
#[test]
fn every_hand_made_fixture_matches_its_approved_snapshot() {
    let mut ids: Vec<String> = std::fs::read_dir(fixtures())
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.len() >= 2 && n.starts_with(['V', 'T', 'C', 'R']) && n[1..].chars().all(|c| c.is_ascii_digit()))
        .collect();
    ids.sort();
    assert!(ids.len() >= 15, "{ids:?}");
    let mut failures = Vec::new();
    for id in &ids {
        let files = inputs(&fixtures().join(id));
        let results = import_all(&files, &mut failures);
        failures.extend(compare(id, &results));
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

/// Gate 2: the small generated fixtures (V7, V8, R7) match their snapshots, and every X fixture
/// is refused with its expected message (X3 and X5 may open; their snapshot says how).
#[test]
fn generated_fixtures_match_their_approved_snapshots() {
    let Some(dir) = generated() else {
        eprintln!("skipped: no generated fixtures (set LM_FIXTURES_DIR or run cargo xtask fixtures)");
        return;
    };
    let mut failures = Vec::new();
    for id in ["V7", "V8", "R7", "X1", "X2", "X3", "X4", "X5"] {
        let files = inputs(&dir.join(id));
        if files.is_empty() {
            eprintln!("skipped {id}: not generated");
            continue;
        }
        let results = import_all(&files, &mut failures);
        if matches!(id, "X1" | "X2" | "X4") {
            for (f, r) in &results {
                if !matches!(r, Err(ImportError::Refused(_))) {
                    failures.push(format!("{id}/{f} must be refused"));
                }
            }
        }
        failures.extend(compare(id, &results));
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

#[test]
fn the_snapshot_diff_shows_only_what_changed() {
    let d = diff("a\nb\nc\nd\ne\nf\ng\n", "a\nb\nc\nD\ne\nf\ng\n");
    assert!(d.contains("- d\n+ D\n"), "{d}");
    assert!(!d.contains("  a\n"), "far context is elided: {d}");
}

