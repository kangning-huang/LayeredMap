//! LayeredMap's additions to the xtask: the layering rules for the `lm-*` crates and the app
//! (build plan › Architecture › Rules enforced in CI).
//!
//! Each LayeredMap crate may depend only on the internal crates its row of the module table
//! names. On top of that:
//! - below `lm-ui`, only `lm-mapview` may depend on egui, and nothing but the app depends on
//!   eframe or rfd;
//! - `lm-ui` never depends on `rustgis-data` or `rustgis-analysis`: all work goes through `lm-jobs`.

use std::collections::BTreeMap;

/// Crate directory (under `crates/`, or `apps/` for the app) and the internal crates it may use.
pub const ALLOWED: &[(&str, &[&str])] = &[
    ("lm-core", &[]),
    ("lm-profile", &["lm-core", "rustgis-data", "rustgis-geom"]),
    ("lm-actions", &["lm-core", "lm-profile"]),
    ("lm-jobs", &["lm-core", "lm-profile", "lm-analysis", "rustgis-engine", "rustgis-analysis"]),
    ("lm-analysis", &["rustgis-geom", "rustgis-data"]),
    ("lm-doc", &["lm-core", "rustgis-data"]),
    ("lm-mapview", &["rustgis-map", "rustgis-tiles", "rustgis-render", "rustgis-geom"]),
    ("lm-ui", &["lm-core", "lm-actions", "lm-jobs", "lm-doc", "lm-mapview"]),
    ("layeredmap", &["lm-ui", "lm-core"]),
];

/// The crates that may depend on egui (and its companions).
const EGUI_OK: &[&str] = &["lm-mapview", "lm-ui", "layeredmap"];
const EGUI: &[&str] = &["egui", "egui_extras", "egui_kittest", "winit"];
/// Only the app may use these.
const APP_ONLY: &[&str] = &["eframe", "rfd"];
/// Never used by `lm-ui`, whatever the table says.
const UI_NEVER: &[&str] = &["rustgis-data", "rustgis-analysis"];

/// The names of the normal dependencies in a Cargo.toml: `[dependencies]` and every
/// `[target.….dependencies]` table (dev- and build-dependencies don't ship).
pub fn dependencies(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            let table = l.trim_matches(['[', ']']).trim();
            inside = table == "dependencies" || (table.starts_with("target.") && table.ends_with(".dependencies"));
            continue;
        }
        if !inside || l.is_empty() || l.starts_with('#') {
            continue;
        }
        let name = l.split(['=', ' ', '.']).next().unwrap_or("").trim().trim_matches('"');
        if !name.is_empty() {
            out.push(name.to_string());
        }
    }
    out
}

/// Every rule a crate's dependencies break, as messages.
pub fn check(krate: &str, deps: &[String]) -> Vec<String> {
    let mut errs = Vec::new();
    let Some((_, allowed)) = ALLOWED.iter().find(|(k, _)| *k == krate) else {
        return vec![format!("{krate}: not in the LayeredMap module table (xtask/src/lm.rs)")];
    };
    for d in deps {
        let d = d.as_str();
        let internal = d.starts_with("lm-") || d.starts_with("rustgis") || d == "layeredmap";
        if internal && !allowed.contains(&d) {
            errs.push(format!("{krate} depends on {d}, which its row of the module table doesn't allow"));
        }
        if EGUI.contains(&d) && !EGUI_OK.contains(&krate) {
            errs.push(format!("{krate} depends on {d}: below lm-ui only lm-mapview may use egui"));
        }
        if APP_ONLY.contains(&d) && krate != "layeredmap" {
            errs.push(format!("{krate} depends on {d}: only the app may"));
        }
        if krate == "lm-ui" && UI_NEVER.contains(&d) {
            errs.push(format!("lm-ui depends on {d}: the interface goes through lm-jobs"));
        }
    }
    errs
}

/// Checks every LayeredMap crate in the workspace at `root`.
pub fn layers(root: &std::path::Path) -> Result<(), String> {
    let mut manifests = BTreeMap::new();
    for (krate, _) in ALLOWED {
        let dir = if *krate == "layeredmap" { root.join("apps").join(krate) } else { root.join("crates").join(krate) };
        let text = std::fs::read_to_string(dir.join("Cargo.toml")).map_err(|e| format!("{}: {e}", dir.display()))?;
        manifests.insert(*krate, text);
    }
    // A new lm- crate must get a row before it can be used.
    if let Ok(entries) = std::fs::read_dir(root.join("crates")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("lm-") && !manifests.contains_key(name.as_str()) {
                return Err(format!("crates/{name}: not in the LayeredMap module table (xtask/src/lm.rs)"));
            }
        }
    }
    let errs: Vec<String> = manifests.iter().flat_map(|(k, text)| check(k, &dependencies(text))).collect();
    if errs.is_empty() {
        println!("lm layers: ok ({} crates)", manifests.len());
        Ok(())
    } else {
        Err(errs.join("\n"))
    }
}

/// The app binary in the release target directory (honours `CARGO_TARGET_DIR`).
fn app_binary(root: &std::path::Path) -> std::path::PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("target"));
    target.join("release").join(if cfg!(windows) { "layeredmap.exe" } else { "layeredmap" })
}

fn build_app(root: &std::path::Path) -> Result<(), String> {
    let st = std::process::Command::new(env!("CARGO"))
        .args(["build", "--release", "-p", "layeredmap"])
        .current_dir(root)
        .status()
        .map_err(|e| e.to_string())?;
    if st.success() { Ok(()) } else { Err("cargo build -p layeredmap failed".into()) }
}

/// p50/p95/p99 over a script's frames, from the app's summary.csv rows.
fn percentile(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len().saturating_sub(1)) as f64 * q).round() as usize).copied().unwrap_or(0.0)
}

/// `cargo xtask feel [--runs N] [--out DIR]`: the Map feel harness (WP1.6) on F1 over the online
/// basemap. Fails when a script's 95th-percentile frame time misses the 33 ms hard limit; reports
/// whether the 60 fps goal (p95 ≤ 16.7 ms) holds on at least 3 of the 4 scripts.
pub fn feel(root: &std::path::Path, args: &[String]) -> Result<(), String> {
    let mut runs = "3".to_string();
    let mut out = root.join("docs/lm/feel");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs" => runs = it.next().ok_or("--runs N")?.clone(),
            "--out" => out = std::path::PathBuf::from(it.next().ok_or("--out DIR")?),
            other => return Err(format!("feel: unknown argument {other}")),
        }
    }
    let fixtures = std::env::var_os("LM_FIXTURES_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("fixtures/generated"));
    if !fixtures.join("F1/f1_points.geojson").is_file() {
        crate::fixtures::run(root, &["--only".into(), "F1".into(), "--out".into(), fixtures.display().to_string()])?;
    }
    build_app(root)?;
    let st = std::process::Command::new(app_binary(root))
        .args(["--feel", &out.display().to_string(), "--runs", &runs, "--fixture", &fixtures.join("F1").display().to_string()])
        .status()
        .map_err(|e| e.to_string())?;
    if !st.success() {
        return Err("the feel run failed".into());
    }
    let text = std::fs::read_to_string(out.join("summary.csv")).map_err(|e| e.to_string())?;
    let mut by_script: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        if let (Some(s), Some(p95)) = (cols.first(), cols.get(4).and_then(|v| v.parse::<f64>().ok())) {
            by_script.entry(s.to_string()).or_default().push(p95);
        }
    }
    let mut goal_met = 0;
    let mut hard_fail = Vec::new();
    println!("script      worst p95 (ms)   median p95 (ms)");
    for (s, mut v) in by_script.clone() {
        let worst = v.iter().copied().fold(0.0, f64::max);
        let median = percentile(&mut v, 0.5);
        println!("{s:<11} {worst:>14.2} {median:>17.2}");
        if worst <= 16.7 {
            goal_met += 1;
        }
        if worst > 33.0 {
            hard_fail.push(s);
        }
    }
    println!("60 fps goal met on {goal_met} of {} scripts (Gate 1 needs 3); results in {}", by_script.len(), out.display());
    if hard_fail.is_empty() { Ok(()) } else { Err(format!("hard limit (p95 ≤ 33 ms) missed: {}", hard_fail.join(", "))) }
}

/// `cargo xtask launch [--n 10]`: cold launch to the first map frame, 95th percentile over n
/// launches (Gate 1: ≤ 2 s).
pub fn launch(root: &std::path::Path, args: &[String]) -> Result<(), String> {
    let n: usize = match args {
        [flag, v] if flag == "--n" => v.parse().map_err(|_| "--n N")?,
        [] => 10,
        _ => return Err("usage: cargo xtask launch [--n N]".into()),
    };
    build_app(root)?;
    let mut ms = Vec::new();
    for _ in 0..n {
        let out = std::process::Command::new(app_binary(root)).arg("--launch-probe").output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(v) = text.lines().find_map(|l| l.strip_prefix("launch_ms=")).and_then(|v| v.trim().parse::<f64>().ok()) {
            ms.push(v);
        }
    }
    if ms.len() < n {
        return Err(format!("only {} of {n} launches reported a time", ms.len()));
    }
    let p95 = percentile(&mut ms, 0.95);
    println!(
        "cold launch to the first map frame over {n} launches: p50 {:.0} ms, p95 {p95:.0} ms (limit 2000 ms)",
        percentile(&mut ms, 0.5)
    );
    if p95 <= 2000.0 { Ok(()) } else { Err("cold launch p95 over 2 s".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deps(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_real_workspace_passes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        layers(root).unwrap();
    }

    // Gate 0: "the layering check fails when a forbidden dependency is added on purpose".
    #[test]
    fn forbidden_dependencies_fail() {
        assert!(!check("lm-core", &deps(&["egui"])).is_empty());
        assert!(!check("lm-profile", &deps(&["eframe"])).is_empty());
        assert!(!check("lm-mapview", &deps(&["rfd"])).is_empty());
        assert!(!check("lm-ui", &deps(&["rustgis-data"])).is_empty());
        assert!(!check("lm-ui", &deps(&["rustgis-analysis"])).is_empty());
        assert!(!check("lm-actions", &deps(&["lm-jobs"])).is_empty());
        assert!(!check("lm-core", &deps(&["rustgis-geom"])).is_empty());
        assert!(!check("lm-unknown", &deps(&[])).is_empty());
        assert!(check("lm-mapview", &deps(&["egui", "rustgis-map", "serde"])).is_empty());
        assert!(check("layeredmap", &deps(&["lm-ui", "eframe", "egui"])).is_empty());
    }

    #[test]
    fn reads_target_tables_and_skips_dev_dependencies() {
        let toml = "[package]\nname = \"x\"\n\n[dependencies]\nserde = { workspace = true }\nlm-core = { path = \"../lm-core\" }\n\n\
                    [target.'cfg(target_os = \"macos\")'.dependencies]\nobjc2 = \"0.6\"\n\n[dev-dependencies]\negui = \"1\"\n";
        assert_eq!(dependencies(toml), ["serde", "lm-core", "objc2"]);
    }

    #[test]
    fn a_forbidden_dependency_in_a_real_manifest_fails() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut text = std::fs::read_to_string(root.join("crates/lm-core/Cargo.toml")).unwrap();
        text.push_str("egui = { workspace = true }\n");
        assert!(!check("lm-core", &dependencies(&text)).is_empty());
    }
}
