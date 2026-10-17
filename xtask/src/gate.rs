//! LayeredMap's gate runs that need more than CI (Gate protocol › Automated checks: "large
//! fixtures and stress runs use cargo xtask stress on your Mac"; fuzzing from Phase 2 on).
//!
//! - `stress`: the ignored large-fixture tests of `lm-jobs` and `lm-analysis` (N7), in release mode, with the generated
//!   fixtures. Each check prints a `STRESS` line (fixture, measured value, threshold, pass/fail);
//!   the run ends with them as a table for the gate report.
//! - `fuzz [--secs N] [--target T]`: `cargo fuzz` on each import adapter (CSV, GeoJSON,
//!   Shapefile, DBF, GeoTIFF header), seeded from the fixtures, all targets at once for N seconds
//!   each (default 3600, Gate 2's hour). Logs go to `fuzz/logs/<target>.log`, findings to
//!   `fuzz/artifacts/<target>/`. Needs a nightly toolchain and cargo-fuzz.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The generated fixtures: `$LM_FIXTURES_DIR`, else `fixtures/generated`.
fn generated(root: &Path) -> PathBuf {
    std::env::var_os("LM_FIXTURES_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("fixtures/generated"))
}

/// A nested cargo without the variables `cargo run` set for the xtask: build scripts that watch
/// them (ring watches `CARGO_MANIFEST_DIR`) would otherwise rebuild everything.
fn nested(program: &str) -> Command {
    let mut cmd = Command::new(program);
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy();
        if k == "CARGO"
            || k == "RUSTUP_TOOLCHAIN"
            || k.starts_with("CARGO_PKG_")
            || k.starts_with("CARGO_MANIFEST_")
            || k.starts_with("CARGO_BIN_")
            || matches!(k.as_ref(), "CARGO_CRATE_NAME" | "CARGO_PRIMARY_PACKAGE")
        {
            cmd.env_remove(k.as_ref());
        }
    }
    cmd
}

/// One `STRESS` line from a test: fixture, measured, threshold, result.
#[derive(Debug, PartialEq)]
pub struct StressRow {
    pub fixture: String,
    pub measured: String,
    pub threshold: String,
    pub result: String,
}

pub fn parse_stress(line: &str) -> Option<StressRow> {
    let mut it = line.strip_prefix("STRESS\t")?.split('\t');
    Some(StressRow {
        fixture: it.next()?.to_string(),
        measured: it.next()?.to_string(),
        threshold: it.next()?.to_string(),
        result: it.next()?.trim().to_string(),
    })
}

/// The stress tests (package, test), each run alone so they don't compete for the machine.
const STRESS_TESTS: &[(&str, &str)] = &[
    ("lm-jobs", "import::tests::large_fixtures"),
    ("lm-jobs", "import::gate2_tests::ui_stall_during_large_imports"),
    // Gate 4, N7: R4 across 3,000 areas through the tiled GeoTIFF reader.
    ("lm-analysis", "zonal::tests::n7_r4_cog_stress"),
];

/// `cargo xtask stress`.
pub fn stress(root: &Path, args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: cargo xtask stress (generated fixtures from $LM_FIXTURES_DIR or fixtures/generated)".into());
    }
    let fixtures = generated(root);
    if !fixtures.is_dir() {
        return Err(format!("{}: no generated fixtures; run cargo xtask fixtures --large, or set LM_FIXTURES_DIR", fixtures.display()));
    }
    let mut rows = Vec::new();
    let mut failed = Vec::new();
    for (package, test) in STRESS_TESTS {
        let args = ["test", "-p", package, "--release", "--lib", "--", "--ignored", "--nocapture", "--exact", test];
        println!("$ LM_FIXTURES_DIR={} cargo {}", fixtures.display(), args.join(" "));
        let mut child = nested(env!("CARGO"))
            .args(args)
            .env("LM_FIXTURES_DIR", &fixtures)
            .current_dir(root)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                println!("{line}");
                rows.extend(parse_stress(&line));
            }
        }
        if !child.wait().map_err(|e| e.to_string())?.success() {
            failed.push(format!("{package} {test}"));
        }
    }
    println!("\n{}", stress_table(&rows));
    let fails = rows.iter().filter(|r| r.result == "FAIL").count();
    if failed.is_empty() && fails == 0 {
        Ok(())
    } else {
        Err(format!("stress: {fails} check(s) failed; failing tests: {}", failed.join(", ")))
    }
}

/// The rows as a Markdown table, ready for § 4 of the gate report.
pub fn stress_table(rows: &[StressRow]) -> String {
    let mut s = String::from("| Fixture | Measured | Threshold | Result |\n|---|---|---|---|\n");
    for r in rows {
        s.push_str(&format!("| {} | {} | {} | {} |\n", r.fixture, r.measured, r.threshold, r.result));
    }
    s
}

/// The fuzz targets (`fuzz/fuzz_targets/*.rs`).
pub const FUZZ_TARGETS: &[&str] = &["csv", "geojson", "shapefile", "dbf", "geotiff_header"];

/// Seed inputs for a target: (seed file name, prefix byte for the Shapefile target's mode, source).
fn seeds(root: &Path, target: &str) -> Vec<(String, Option<u8>, PathBuf)> {
    let fx = root.join("fixtures");
    let gen_dir = generated(root);
    let mut v: Vec<(Option<u8>, PathBuf)> = Vec::new();
    let all = |ext: &str| -> Vec<PathBuf> {
        let mut out = Vec::new();
        for dir in std::fs::read_dir(&fx).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            if dir.file_name().is_some_and(|n| n == "generated" || n == "snapshots") {
                continue;
            }
            for f in std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()) {
                let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && !name.starts_with("expected") && f.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
                    out.push(f);
                }
            }
        }
        out.sort();
        out
    };
    match target {
        "csv" => {
            v.extend(all("csv").into_iter().map(|p| (None, p)));
            v.push((None, gen_dir.join("X5/x5_formulas.csv")));
        }
        "geojson" => {
            v.extend(all("geojson").into_iter().map(|p| (None, p)));
            v.push((None, gen_dir.join("X3/x3_nan.geojson")));
            v.push((None, gen_dir.join("X3/x3_nesting.geojson")));
        }
        "shapefile" => {
            for (i, p) in all("shp").into_iter().enumerate() {
                v.push((Some((i % 3) as u8), p));
            }
            v.extend(all("zip").into_iter().map(|p| (Some(3), p)));
            v.push((Some(3), gen_dir.join("X2/x2_traversal.zip")));
            v.push((Some(3), gen_dir.join("X1/x1_bomb.zip")));
        }
        "dbf" => v.extend(all("dbf").into_iter().map(|p| (None, p))),
        "geotiff_header" => {
            v.extend(all("tif").into_iter().map(|p| (None, p)));
            v.push((None, gen_dir.join("X4/x4_billion.tif")));
        }
        _ => {}
    }
    v.into_iter()
        .filter(|(_, p)| p.is_file())
        .map(|(prefix, p)| {
            let parent = p.parent().and_then(|d| d.file_name()).and_then(|n| n.to_str()).unwrap_or("x").to_string();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("seed").to_string();
            (format!("seed-{parent}-{name}"), prefix, p)
        })
        .collect()
}

/// Copies a target's seeds into `fuzz/corpus/<target>` (kept between runs: the corpus grows).
fn seed_corpus(root: &Path, target: &str) -> Result<usize, String> {
    let dir = root.join("fuzz/corpus").join(target);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut n = 0;
    for (name, prefix, src) in seeds(root, target) {
        let dst = dir.join(&name);
        if dst.exists() {
            continue;
        }
        let mut bytes = prefix.map(|b| vec![b]).unwrap_or_default();
        bytes.extend(std::fs::read(&src).map_err(|e| format!("{}: {e}", src.display()))?);
        std::fs::write(&dst, bytes).map_err(|e| format!("{}: {e}", dst.display()))?;
        n += 1;
    }
    Ok(n)
}

/// What one fuzz run found.
#[derive(Debug, Default, PartialEq)]
pub struct FuzzResult {
    pub executions: u64,
    pub crash: Option<String>,
}

/// Reads libFuzzer's output: the final execution count and any crash, timeout or out-of-memory
/// artifact.
pub fn parse_fuzz(lines: &[String]) -> FuzzResult {
    let mut r = FuzzResult::default();
    for l in lines {
        if let Some(n) = l.trim().strip_prefix("stat::number_of_executed_units:") {
            r.executions = n.trim().parse().unwrap_or(0);
        } else if let Some(i) = l.find("Test unit written to ") {
            r.crash = Some(l.get(i + "Test unit written to ".len()..).unwrap_or("").trim().to_string());
        } else if r.executions == 0
            && let Some(rest) = l.strip_prefix("Done ").and_then(|s| s.split_whitespace().next())
        {
            r.executions = rest.parse().unwrap_or(0);
        }
    }
    r
}

/// `cargo xtask fuzz [--secs N] [--target T]`.
pub fn fuzz(root: &Path, args: &[String]) -> Result<(), String> {
    let mut secs = 3600u64;
    let mut targets: Vec<&str> = FUZZ_TARGETS.to_vec();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--secs" => secs = it.next().and_then(|v| v.parse().ok()).ok_or("--secs N")?,
            "--target" => {
                let t = it.next().ok_or("--target T")?;
                let t = FUZZ_TARGETS
                    .iter()
                    .find(|x| *x == t)
                    .ok_or_else(|| format!("unknown fuzz target {t}; one of {}", FUZZ_TARGETS.join(", ")))?;
                targets = vec![t];
            }
            other => return Err(format!("fuzz: unknown argument {other}; usage: cargo xtask fuzz [--secs N] [--target T]")),
        }
    }
    let ok = nested("cargo").args(["+nightly", "fuzz", "--version"]).output().is_ok_and(|o| o.status.success());
    if !ok {
        return Err(
            "cargo fuzz needs a nightly toolchain and cargo-fuzz: rustup toolchain install nightly && cargo install cargo-fuzz".into()
        );
    }
    // Sanitizer builds get their own target directory, so they don't evict the normal build.
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(|d| PathBuf::from(format!("{}-fuzz", PathBuf::from(d).display())))
        .unwrap_or_else(|| root.join("fuzz/target"));
    // Build once, then run the binaries directly: `cargo fuzz run` holds cargo's build lock while
    // it fuzzes, which would make the targets wait for each other.
    let mut build = nested("cargo");
    build.args(["+nightly", "fuzz", "build", "--target-dir"]).arg(&target_dir);
    if let [t] = targets.as_slice() {
        build.arg(t);
    }
    println!("$ {build:?}");
    if !build.current_dir(root).env_remove("CARGO_TARGET_DIR").status().map_err(|e| e.to_string())?.success() {
        return Err("cargo fuzz build failed".into());
    }
    let host = Command::new("rustc").arg("-vV").output().map_err(|e| e.to_string()).map(|o| {
        String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.strip_prefix("host: ").map(str::to_string)).unwrap_or_default()
    })?;
    let logs = root.join("fuzz/logs");
    std::fs::create_dir_all(&logs).map_err(|e| format!("{}: {e}", logs.display()))?;
    // Every target runs at once, each on its own core, for `secs` seconds.
    let mut running = Vec::new();
    for t in &targets {
        let seeded = seed_corpus(root, t)?;
        let artifacts = root.join("fuzz/artifacts").join(t);
        std::fs::create_dir_all(&artifacts).map_err(|e| format!("{}: {e}", artifacts.display()))?;
        let log = logs.join(format!("{t}.log"));
        let file = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
        let bin = target_dir.join(&host).join("release").join(t);
        let mut cmd = Command::new(&bin);
        cmd.arg(format!("-artifact_prefix={}/", artifacts.display()))
            .arg(format!("-max_total_time={secs}"))
            // A hang is an input that takes over 10 s. A single allocation over 512 MB from a
            // fuzzed file is a finding; the RSS limit is raised from libFuzzer's 2 GB because
            // the sanitizer's quarantine keeps freed raster buffers (each bounded by the readers'
            // limits) and the RSS creeps up over a long run without any live leak.
            .args(["-timeout=10", "-malloc_limit_mb=512", "-rss_limit_mb=4096", "-print_final_stats=1"])
            .arg(root.join("fuzz/corpus").join(t));
        println!("fuzzing {t} for {secs} s ({seeded} new seeds); log: {}", log.display());
        let child = cmd.current_dir(root).stdout(Stdio::null()).stderr(file).spawn().map_err(|e| format!("{}: {e}", bin.display()))?;
        running.push((t.to_string(), child, log));
    }
    let mut results = Vec::new();
    for (t, mut child, log) in running {
        let status = child.wait().map_err(|e| e.to_string())?;
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        let mut r = parse_fuzz(&lines);
        if !status.success() && r.crash.is_none() {
            r.crash = Some(format!("the fuzzer exited with {status}; see {}", log.display()));
        }
        results.push((t, r));
    }
    println!("\n| Target | Seconds | Executions | Crashes |\n|---|---|---|---|");
    for (t, r) in &results {
        println!("| {t} | {secs} | {} | {} |", r.executions, r.crash.as_deref().map_or("0".to_string(), |c| format!("1 ({c})")));
    }
    let crashed: Vec<&str> = results.iter().filter(|(_, r)| r.crash.is_some()).map(|(t, _)| t.as_str()).collect();
    if crashed.is_empty() { Ok(()) } else { Err(format!("fuzz: findings in {}", crashed.join(", "))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_lines_are_read_and_tabled() {
        let r = parse_stress("STRESS\tT3\t1000000 rows in 2.10 s\topens, >= 1,000,000 rows\tpass").unwrap();
        assert_eq!(r.fixture, "T3");
        assert_eq!(r.result, "pass");
        assert!(parse_stress("test import::tests::large_fixtures ... ok").is_none());
        assert!(stress_table(&[r]).contains("| T3 | 1000000 rows in 2.10 s | opens, >= 1,000,000 rows | pass |"));
    }

    #[test]
    fn libfuzzer_output_gives_executions_and_findings() {
        let lines: Vec<String> = [
            "#1048576 pulse  cov: 1200 ft: 3000 corp: 90/12Kb exec/s: 3495 rss: 300Mb",
            "Done 1048576 runs in 300 second(s)",
            "stat::number_of_executed_units: 1048600",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(parse_fuzz(&lines), FuzzResult { executions: 1_048_600, crash: None });
        let crash: Vec<String> =
            ["==1== ERROR: libFuzzer: deadly signal", "artifact_prefix='x/'; Test unit written to fuzz/artifacts/csv/crash-ab12"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        assert_eq!(parse_fuzz(&crash).crash.as_deref(), Some("fuzz/artifacts/csv/crash-ab12"));
    }

    #[test]
    fn every_target_has_a_source_and_seeds() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for t in FUZZ_TARGETS {
            assert!(root.join("fuzz/fuzz_targets").join(format!("{t}.rs")).is_file(), "{t}");
            assert!(!seeds(root, t).is_empty(), "{t} has no seeds");
        }
    }
}
