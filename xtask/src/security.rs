//! LayeredMap's security and licence tasks (WP6.8; Gate 5's licence audit):
//!
//! - `cargo xtask audit`: `cargo audit` and `cargo deny check` (both need the network for the
//!   advisory database, so they are not part of `cargo xtask ci`; CI runs them as their own job).
//! - `cargo xtask licenses`: `cargo about` renders every crate in the app, with its licence text,
//!   into `ATTRIBUTION-crates.md`.
//! - `cargo xtask sbom [--out FILE]`: a CycloneDX 1.5 software bill of materials for the app,
//!   from `cargo metadata` and the lockfile's checksums.
//! - The network-gate check (part of `cargo xtask layers` and `ci`): a LayeredMap source file
//!   that uses `ureq` must ask `lm_net`'s gate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

/// Runs a cargo subcommand that may not be installed, with the install line when it isn't.
fn cargo_tool(root: &Path, tool: &str, args: &[&str]) -> Result<(), String> {
    let installed = Command::new(env!("CARGO")).args([tool, "--version"]).output().is_ok_and(|o| o.status.success());
    if !installed {
        let krate = if tool == "about" { "cargo-about --features cli".to_string() } else { format!("cargo-{tool}") };
        return Err(format!("cargo {tool} is not installed: cargo install --locked {krate}"));
    }
    println!("$ cargo {tool} {}", args.join(" "));
    let st = Command::new(env!("CARGO")).arg(tool).args(args).current_dir(root).status().map_err(|e| e.to_string())?;
    if st.success() { Ok(()) } else { Err(format!("cargo {tool} {} failed", args.join(" "))) }
}

/// `cargo xtask audit`: known vulnerabilities (RustSec), then licences, bans and sources.
pub fn audit(root: &Path) -> Result<(), String> {
    cargo_tool(root, "audit", &[])?;
    cargo_tool(root, "deny", &["check"])
}

/// `cargo xtask licenses`: writes `ATTRIBUTION-crates.md` for the app.
pub fn licenses(root: &Path) -> Result<(), String> {
    cargo_tool(
        root,
        "about",
        &[
            "generate",
            "--locked",
            "-m",
            "apps/layeredmap/Cargo.toml",
            "-c",
            "about.toml",
            "packaging/lm/about.hbs",
            "-o",
            "ATTRIBUTION-crates.md",
        ],
    )?;
    println!("licenses: wrote ATTRIBUTION-crates.md");
    Ok(())
}

/// `name version` → SHA-256 checksum, from Cargo.lock.
pub fn lock_checksums(lock: &str) -> BTreeMap<(String, String), String> {
    let mut out = BTreeMap::new();
    for pkg in lock.split("[[package]]").skip(1) {
        let field = |k: &str| {
            pkg.lines()
                .find_map(|l| l.strip_prefix(k).and_then(|r| r.trim().strip_prefix('=')).map(|v| v.trim().trim_matches('"').to_string()))
        };
        if let (Some(n), Some(v), Some(c)) = (field("name"), field("version"), field("checksum")) {
            out.insert((n, v), c);
        }
    }
    out
}

/// A CycloneDX 1.5 BOM of `root_name` (a workspace package) and everything it links: normal
/// dependencies only, as `cargo metadata` resolves them for the platform it was asked about.
pub fn cyclonedx(metadata: &Value, checksums: &BTreeMap<(String, String), String>, root_name: &str) -> Result<Value, String> {
    let packages = metadata["packages"].as_array().ok_or("cargo metadata: no packages")?;
    let by_id: BTreeMap<&str, &Value> = packages.iter().filter_map(|p| Some((p["id"].as_str()?, p))).collect();
    let nodes = metadata["resolve"]["nodes"].as_array().ok_or("cargo metadata: no resolve graph")?;
    let deps_of: BTreeMap<&str, Vec<&str>> = nodes
        .iter()
        .filter_map(|n| {
            let id = n["id"].as_str()?;
            let deps = n["deps"]
                .as_array()?
                .iter()
                .filter(|d| d["dep_kinds"].as_array().is_some_and(|k| k.iter().any(|k| k["kind"].is_null())))
                .filter_map(|d| d["pkg"].as_str())
                .collect();
            Some((id, deps))
        })
        .collect();
    let root_id = packages
        .iter()
        .find(|p| p["name"] == root_name && p["source"].is_null())
        .and_then(|p| p["id"].as_str())
        .ok_or_else(|| format!("{root_name}: not a workspace package"))?;
    // Everything reachable from the root through normal dependencies.
    let mut seen = BTreeSet::new();
    let mut todo = vec![root_id];
    while let Some(id) = todo.pop() {
        if seen.insert(id) {
            todo.extend(deps_of.get(id).into_iter().flatten().copied());
        }
    }
    let purl = |p: &Value| format!("pkg:cargo/{}@{}", p["name"].as_str().unwrap_or(""), p["version"].as_str().unwrap_or(""));
    let mut components = Vec::new();
    let mut dependencies = Vec::new();
    for id in &seen {
        let Some(p) = by_id.get(id) else { continue };
        let reference = purl(p);
        let mut deps: Vec<String> = deps_of.get(id).into_iter().flatten().filter_map(|d| by_id.get(d)).map(|d| purl(d)).collect();
        deps.sort();
        deps.dedup();
        dependencies.push(json!({ "ref": reference, "dependsOn": deps }));
        if *id == root_id {
            continue;
        }
        let name = p["name"].as_str().unwrap_or("");
        let version = p["version"].as_str().unwrap_or("");
        let mut c = json!({
            "type": "library",
            "bom-ref": reference,
            "name": name,
            "version": version,
            "purl": reference,
            "scope": "required",
        });
        if let Some(l) = p["license"].as_str() {
            c["licenses"] = json!([{ "expression": l.replace('/', " OR ") }]);
        }
        if let Some(sum) = checksums.get(&(name.to_string(), version.to_string())) {
            c["hashes"] = json!([{ "alg": "SHA-256", "content": sum }]);
        }
        let mut refs = Vec::new();
        if let Some(r) = p["repository"].as_str() {
            refs.push(json!({ "type": "vcs", "url": r }));
        }
        if p["source"].is_null() {
            c["description"] = json!("part of this repository");
        } else {
            refs.push(json!({ "type": "distribution", "url": format!("https://crates.io/crates/{name}/{version}") }));
        }
        if !refs.is_empty() {
            c["externalReferences"] = Value::Array(refs);
        }
        components.push(c);
    }
    components.sort_by(|a, b| a["bom-ref"].as_str().cmp(&b["bom-ref"].as_str()));
    dependencies.sort_by(|a, b| a["ref"].as_str().cmp(&b["ref"].as_str()));
    let root = by_id.get(root_id).ok_or("no root package")?;
    // A serial number derived from the contents: the same lockfile gives the same BOM.
    let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for c in &components {
        for b in c["bom-ref"].as_str().unwrap_or("").bytes() {
            h = (h ^ u128::from(b)).wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
        }
    }
    let hex = format!("{h:032x}");
    let serial = format!("urn:uuid:{}-{}-4{}-8{}-{}", &hex[0..8], &hex[8..12], &hex[13..16], &hex[17..20], &hex[20..32]);
    Ok(json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": serial,
        "version": 1,
        "metadata": {
            "tools": [{ "vendor": "LayeredMap", "name": "cargo xtask sbom" }],
            "component": {
                "type": "application",
                "bom-ref": purl(root),
                "name": root_name,
                "version": root["version"],
                "purl": purl(root),
                "licenses": [{ "expression": root["license"].as_str().unwrap_or("Apache-2.0") }],
            },
        },
        "components": components,
        "dependencies": dependencies,
    }))
}

/// The app's resolved dependency graph for one platform.
fn metadata(root: &Path, target: &str) -> Result<Value, String> {
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--filter-platform", target, "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

/// `cargo xtask sbom [--out FILE] [--target TRIPLE]`.
pub fn sbom(root: &Path, args: &[String]) -> Result<(), String> {
    let mut out: Option<PathBuf> = None;
    let mut target = "aarch64-apple-darwin".to_string();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out FILE")?)),
            "--target" => target = it.next().ok_or("--target TRIPLE")?.clone(),
            other => return Err(format!("sbom: unknown argument {other}; usage: cargo xtask sbom [--out FILE] [--target TRIPLE]")),
        }
    }
    let m = metadata(root, &target)?;
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    let bom = cyclonedx(&m, &lock_checksums(&lock), "layeredmap")?;
    let version = bom["metadata"]["component"]["version"].as_str().unwrap_or("0").to_string();
    let out = out.unwrap_or_else(|| root.join(format!("dist/lm/layeredmap-{version}-{target}.cdx.json")));
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&bom).map_err(|e| e.to_string())?;
    std::fs::write(&out, text + "\n").map_err(|e| format!("{}: {e}", out.display()))?;
    println!("sbom: {} components → {}", bom["components"].as_array().map_or(0, Vec::len), out.display());
    Ok(())
}

/// Source files (path, text) that use `ureq` without asking the network gate.
pub fn ungated(files: &[(String, String)]) -> Vec<String> {
    files
        .iter()
        .filter(|(_, text)| text.contains("ureq::") && !text.contains("lm_net::"))
        .map(|(path, _)| format!("{path} uses ureq without lm_net's gate (strict offline mode, WP6.6)"))
        .collect()
}

fn rust_files(dir: &Path, out: &mut Vec<(String, String)>, root: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            rust_files(&p, out, root);
        } else if name.ends_with(".rs")
            && let Ok(text) = std::fs::read_to_string(&p)
        {
            out.push((p.strip_prefix(root).unwrap_or(&p).display().to_string(), text));
        }
    }
}

/// The network-gate check over every LayeredMap crate and the app, plus the tile fetcher's gate.
pub fn net_lint(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    for e in std::fs::read_dir(root.join("crates")).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with("lm-") {
            rust_files(&e.path().join("src"), &mut files, root);
        }
    }
    rust_files(&root.join("apps/layeredmap/src"), &mut files, root);
    // lm-net itself is the gate.
    files.retain(|(p, _)| !p.starts_with("crates/lm-net/"));
    let mut errs = ungated(&files);
    let cache = std::fs::read_to_string(root.join("crates/tiles/src/cache.rs")).map_err(|e| format!("crates/tiles/src/cache.rs: {e}"))?;
    if !cache.contains("if !super::gate_allows(url)") {
        errs.push("crates/tiles/src/cache.rs: the tile fetcher no longer asks the network gate before a request".into());
    }
    if errs.is_empty() {
        println!("network gate: ok ({} files)", files.len());
        Ok(())
    } else {
        Err(errs.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
    }

    #[test]
    fn the_real_workspace_asks_the_gate() {
        net_lint(root()).unwrap();
    }

    #[test]
    fn ureq_without_the_gate_fails() {
        let files = vec![
            ("crates/lm-x/src/a.rs".to_string(), "let a: ureq::Agent = todo;".to_string()),
            ("crates/lm-x/src/b.rs".to_string(), "lm_net::check(p)?; let a: ureq::Agent = todo;".to_string()),
            ("crates/lm-x/src/c.rs".to_string(), "fn nothing() {}".to_string()),
        ];
        assert_eq!(ungated(&files).len(), 1);
        assert!(ungated(&files)[0].starts_with("crates/lm-x/src/a.rs"));
    }

    #[test]
    fn checksums_come_from_the_lockfile() {
        let lock = "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"1.0.0\"\nsource = \"registry+x\"\nchecksum = \"abc\"\n\n[[package]]\nname = \"local\"\nversion = \"0.1.0\"\n";
        let c = lock_checksums(lock);
        assert_eq!(c.get(&("a".into(), "1.0.0".into())).map(String::as_str), Some("abc"));
        assert_eq!(c.len(), 1);
    }

    /// The app's real BOM: what it links (ureq, egui), not what only tests or builds use.
    #[test]
    fn the_app_bom_lists_what_it_links() {
        let m = metadata(root(), "aarch64-apple-darwin").unwrap();
        let lock = std::fs::read_to_string(root().join("Cargo.lock")).unwrap();
        let bom = cyclonedx(&m, &lock_checksums(&lock), "layeredmap").unwrap();
        let names: BTreeSet<&str> = bom["components"].as_array().unwrap().iter().filter_map(|c| c["name"].as_str()).collect();
        for n in ["lm-ui", "lm-net", "rustgis-tiles", "ureq", "egui", "rustls"] {
            assert!(names.contains(n), "{n} missing");
        }
        for n in ["egui_kittest", "layeredmap", "xtask", "rustgis-python"] {
            assert!(!names.contains(n), "{n} listed");
        }
        let ureq = bom["components"].as_array().unwrap().iter().find(|c| c["name"] == "ureq").unwrap();
        assert!(ureq["purl"].as_str().unwrap().starts_with("pkg:cargo/ureq@3."));
        assert_eq!(ureq["hashes"][0]["alg"], "SHA-256");
        assert!(ureq["licenses"][0]["expression"].as_str().unwrap().contains("MIT"));
        assert_eq!(bom["specVersion"], "1.5");
        // Same inputs, same serial number.
        assert_eq!(cyclonedx(&m, &lock_checksums(&lock), "layeredmap").unwrap()["serialNumber"], bom["serialNumber"]);
    }
}
