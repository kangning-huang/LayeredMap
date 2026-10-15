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
    ("lm-jobs", &["lm-core", "lm-analysis", "rustgis-engine", "rustgis-analysis"]),
    ("lm-analysis", &["rustgis-geom", "rustgis-data"]),
    ("lm-doc", &["lm-core", "rustgis-data"]),
    ("lm-mapview", &["rustgis-map", "rustgis-tiles", "rustgis-render"]),
    ("lm-ui", &["lm-core", "lm-actions", "lm-jobs", "lm-doc", "lm-mapview"]),
    ("layeredmap", &["lm-ui"]),
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
