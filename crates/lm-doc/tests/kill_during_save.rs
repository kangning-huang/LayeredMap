//! Gate 5's save kill test: a child process saves a document over and over, alternating two
//! versions, and is killed with SIGKILL (`kill -9`) at a random moment, 200 times. After every
//! kill the file must open, as the old version or the new one, never damaged.
//!
//! The child is this test binary run again with `LM_KILL_CHILD` set; `LM_KILL_RUNS` changes the
//! number of kills.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lm_core::document::Document;
use lm_core::model::*;
use lm_doc::{OwnedData, load_lmap, save_lmap};
use rustgis_data::{Coord, Dataset, Field, FieldType, Geometry, GeometryType, Value};

fn profile(name: &str) -> LayerProfile {
    LayerProfile {
        source: SourceRef { path: name.into(), member: None, size: 0, mtime: 0, sha256: None },
        format: "Result".into(),
        kind: LayerKind::Points,
        count: 0,
        extent_wgs84: None,
        crs: Known::certain(Crs::Wgs84, "computed"),
        fields: vec![],
        bands: vec![],
        assumptions: vec![],
        exclusions: vec![],
        encoding: None,
        notices: vec![],
        data_rev: 0,
        joins: Vec::new(),
    }
}

/// Version `v` (0 or 1): a layer named after it, with an owned result large enough that a save
/// takes a while (about 400 kB of FlatGeobuf).
fn version(v: u8) -> (Document, Vec<(LayerId, OwnedData)>) {
    let mut d = Document::new();
    let tx = d.add_layer_tx(profile("Result"));
    d.apply(&tx).unwrap();
    d.layers[0].display_name = format!("Version {v}");
    d.view = Some((f64::from(v), 0.0, 3.0));
    let mut ds = Dataset::new("pts", GeometryType::Point, vec![Field::new("n", FieldType::Integer), Field::new("label", FieldType::Text)]);
    let n = 6_000 + usize::from(v) * 500;
    for i in 0..n {
        let x = (i as f64 * 0.618_033_988_7).fract() * 360.0 - 180.0;
        let y = (i as f64 * 0.414_213_562_3).fract() * 170.0 - 85.0;
        ds.push(Some(Geometry::Point(Coord::new(x, y))), vec![Value::Int(i as i64), Value::Text(format!("point {i} of version {v}"))]);
    }
    let id = d.layers[0].id;
    (d, vec![(id, OwnedData::Vector(Arc::new(ds)))])
}

/// The child: saves the two versions in turn until it is killed.
#[test]
fn child_saves_forever() {
    let Some(path) = std::env::var_os("LM_KILL_CHILD").map(PathBuf::from) else { return };
    let versions = [version(0), version(1)];
    // Tells the parent the saves are starting (its clock for the kill starts here).
    println!("saving");
    let mut i = 0usize;
    loop {
        let (d, o) = &versions[i % 2];
        let _ = save_lmap(d, o, &path);
        i += 1;
    }
}

fn temps(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().contains(".saving-")).collect()
}

#[test]
#[cfg(unix)]
fn a_save_killed_at_any_moment_leaves_the_old_or_the_new_file() {
    if std::env::var_os("LM_KILL_CHILD").is_some() {
        return;
    }
    let runs: usize = std::env::var("LM_KILL_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let dir = std::env::temp_dir().join(format!("lm-doc-kill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Heat study.lmap");
    let versions = [version(0), version(1)];
    save_lmap(&versions[0].0, &versions[0].1, &path).unwrap();
    let exe = std::env::current_exe().unwrap();
    // A fixed linear congruential sequence: random-looking kill moments, the same every run.
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let (mut mid_write, mut seen) = (0, [0usize; 2]);
    for run in 0..runs {
        let mut child = std::process::Command::new(&exe)
            .args(["child_saves_forever", "--exact", "--nocapture", "--test-threads=1"])
            .env("LM_KILL_CHILD", &path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        // Wait for "saving" (the test harness prints its own lines first).
        let mut out = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while std::io::BufRead::read_line(&mut out, &mut line).unwrap() > 0 && !line.contains("saving") {
            line.clear();
        }
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let wait_ms = (seed >> 33) % 60;
        std::thread::sleep(Duration::from_millis(wait_ms));
        child.kill().unwrap(); // SIGKILL
        child.wait().unwrap();
        let left = temps(&dir);
        if !left.is_empty() {
            mid_write += 1;
        }
        let back = load_lmap(&path).unwrap_or_else(|e| panic!("run {run} (killed after {wait_ms} ms): the file does not open: {e}"));
        let which = versions.iter().position(|(d, _)| *d == back.doc).unwrap_or_else(|| panic!("run {run}: neither version"));
        let Some((_, OwnedData::Vector(ds))) = back.owned.first() else { panic!("run {run}: owned result missing") };
        let OwnedData::Vector(want) = &versions[which].1[0].1 else { unreachable!() };
        assert_eq!(ds.features.len(), want.features.len(), "run {run}");
        assert_eq!(ds.features.last(), want.features.last(), "run {run}");
        seen[which] += 1;
        for t in left {
            let _ = std::fs::remove_file(t);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("kill test: {runs} kills, {mid_write} during a write; opened version 0 {} times, version 1 {} times", seen[0], seen[1]);
    // The test means something only if many kills landed in the middle of a save.
    assert!(mid_write * 4 >= runs, "only {mid_write} of {runs} kills interrupted a save");
}
