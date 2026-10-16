use std::path::PathBuf;

use lm_core::model::{AssumptionKind, ImportStatus, LayerKind, Treatment};

use super::*;

pub(super) fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// The generated fixtures (`cargo xtask fixtures`): `$LM_FIXTURES_DIR`, else `fixtures/generated`,
/// when the folder exists.
pub(super) fn generated() -> Option<PathBuf> {
    let dir = std::env::var_os("LM_FIXTURES_DIR").map(PathBuf::from).unwrap_or_else(|| fixtures().join("generated"));
    dir.is_dir().then_some(dir)
}

/// The data file with extension `ext` in a generated fixture's folder (`T3` → `t3_rows.csv`).
pub(super) fn generated_file(id: &str, ext: &str) -> Option<PathBuf> {
    std::fs::read_dir(generated()?.join(id))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == ext) && !p.file_name().and_then(|n| n.to_str()).unwrap_or("").starts_with('.'))
}

fn open(rel: &str) -> Result<Vec<ImportedLayer>, ImportError> {
    import_path(&fixtures().join(rel), &Ctx::detached(Ticket::default()))
}

fn has(l: &ImportedLayer, k: AssumptionKind) -> bool {
    l.profile.assumptions.iter().any(|a| a.kind == k)
}

pub(super) fn sha(path: &std::path::Path) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(std::fs::read(path).unwrap()).to_vec()
}

#[test]
fn t1_csv_lands_with_a_visible_assumption_and_six_exclusions() {
    let l = open("T1/schools.csv").unwrap();
    let l = &l[0];
    assert_eq!(l.profile.kind, LayerKind::Points);
    assert_eq!(l.profile.count, 242, "248 rows, 6 without a location");
    assert_eq!(l.profile.exclusions.len(), 6, "{:?}", l.profile.exclusions);
    assert_eq!(l.status, ImportStatus::CompleteWithExclusions);
    let a = l.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::CsvCoordinateColumns).unwrap();
    assert_eq!(a.treatment, Treatment::Visible);
    assert!(a.line.starts_with("Located using "), "{}", a.line);
}

#[test]
fn t2_ambiguous_columns_need_a_decision_and_can_be_answered() {
    let l = open("T2/deliveries.csv").unwrap().remove(0);
    assert_eq!(l.status, ImportStatus::NeedsDecision);
    let q = l.profile.open_questions().next().unwrap().clone();
    assert_eq!(q.kind, AssumptionKind::CsvAmbiguousColumns);
    let pick = q.alternatives.iter().find(|c| c.id.starts_with("columns:")).unwrap().clone();
    let fixed = relocate_csv(&l, &pick).unwrap();
    assert_ne!(fixed.status, ImportStatus::NeedsDecision);
    let not_sure = relocate_csv(&l, &lm_core::model::Choice::not_sure()).unwrap();
    assert_eq!(not_sure.profile.kind, LayerKind::Table);
}

#[test]
fn t4_gbk_csv_keeps_raw_strings() {
    let l = open("T4/stations_gbk.csv").unwrap().remove(0);
    assert_eq!(l.profile.encoding.as_deref(), Some("GBK"));
    let LayerData::Vector(ds) = &l.data else { panic!() };
    let has_leading_zero = ds.features.iter().any(|f| f.attrs.iter().any(|v| matches!(v, rustgis_engine::data::Value::Text(t) if t.len() > 1 && t.starts_with('0') && t.chars().all(|c| c.is_ascii_digit()))));
    assert!(has_leading_zero, "leading-zero codes stay as written");
}

#[test]
fn v1_geojson_districts_open_as_areas() {
    let l = open("V1/districts.geojson").unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].profile.kind, LayerKind::Areas);
    assert_eq!(l[0].profile.count, 17);
}

#[test]
fn v2_mixed_geometry_is_split() {
    let l = open("V2/mixed.geojson").unwrap();
    assert_eq!(l.len(), 3, "points, lines and areas");
    assert!(l.iter().all(|x| has(x, AssumptionKind::MixedGeometrySplit)));
}

#[test]
fn v3_shapefile_without_prj_assumes_gps_coordinates() {
    let l = open("V3/sites.shp").unwrap().remove(0);
    assert!(has(&l, AssumptionKind::NoCrsLonLat), "{:?}", l.profile.assumptions);
    assert_eq!(l.profile.kind, LayerKind::Points);
}

#[test]
fn v4_projected_shapefile_asks_where_and_can_be_placed() {
    let l = open("V4/districts_gk.shp").unwrap().remove(0);
    assert!(has(&l, AssumptionKind::NoCrsProjected), "{:?}", l.profile.assumptions);
    assert_eq!(l.status, ImportStatus::NeedsDecision);
    assert!(l.profile.extent_wgs84.is_none(), "not drawn until placed");
}

#[test]
fn v5_gbk_dbf_is_read_as_chinese() {
    let l = open("V5/places_gbk.shp").unwrap().remove(0);
    assert_eq!(l.profile.encoding.as_deref(), Some("GBK"));
    let LayerData::Vector(ds) = &l.data else { panic!() };
    let name = ds.field_index("name").unwrap();
    assert_eq!(ds.value(0, name), &rustgis_engine::data::Value::Text("人民广场".into()));
}

#[test]
fn c1_amap_named_file_asks_to_convert() {
    let l = open("C1/amap_pois.csv").unwrap().remove(0);
    assert!(has(&l, AssumptionKind::ChinaOffsetNamed), "{:?}", l.profile.assumptions);
}

#[test]
fn r6_raster_opens_with_its_nodata_assumption() {
    let dir = fixtures().join("R6");
    let tif = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "tif")).unwrap();
    let l = import_path(&tif, &Ctx::detached(Ticket::default())).unwrap().remove(0);
    assert_eq!(l.profile.kind, LayerKind::Raster);
    assert!(has(&l, AssumptionKind::RasterNoData), "{:?}", l.profile.assumptions);
}

#[test]
fn every_hand_made_fixture_opens_and_stays_byte_identical() {
    let mut opened = 0;
    for dir in std::fs::read_dir(fixtures()).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        if dir.file_name().is_some_and(|n| n == "generated") {
            continue;
        }
        for f in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
            let n = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            // Expected-answer files sit beside the data; they are CSVs but not inputs.
            if n.starts_with('.')
                || n.starts_with("expected")
                || n.contains("TODO")
                || n == "vectors.csv"
                || !matches!(ext.as_str(), "csv" | "geojson" | "shp" | "zip" | "tif" | "gpkg" | "xlsx")
            {
                continue;
            }
            let siblings: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
            let before: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
            let r = import_path(&f, &Ctx::detached(Ticket::default()));
            let after: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
            assert_eq!(before, after, "{} changed a source file", f.display());
            let now: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
            assert_eq!(siblings.len(), now.len(), "{} left a file beside it", f.display());
            match r {
                Ok(layers) => {
                    assert!(!layers.is_empty(), "{}", f.display());
                    opened += 1;
                }
                // Some fixtures (V9's GeoPackage, P1) may honestly fail; never with a panic.
                Err(e) => eprintln!("{}: {e}", f.display()),
            }
        }
    }
    assert!(opened >= 15, "only {opened} fixtures opened");
}

#[test]
fn unsafe_archives_and_huge_rasters_are_refused_quickly() {
    let Some(generated) = generated() else {
        eprintln!("skipped: no generated fixtures (set LM_FIXTURES_DIR or run cargo xtask fixtures)");
        return;
    };
    for (f, word) in [("X1/x1_bomb.zip", "archives inside"), ("X2/x2_traversal.zip", "outside"), ("X4/x4_billion.tif", "pixels")] {
        let p = generated.join(f);
        if !p.exists() {
            continue;
        }
        let t = std::time::Instant::now();
        let err = import_path(&p, &Ctx::detached(Ticket::default())).err().unwrap();
        assert!(err.to_string().contains(word), "{f}: {err}");
        assert!(t.elapsed().as_secs_f64() < 1.0, "{f} took {:?}", t.elapsed());
    }
}

#[test]
fn a_canceled_import_returns_nothing() {
    let t = Ticket::default();
    t.cancel();
    assert_eq!(import_path(&fixtures().join("T1/schools.csv"), &Ctx::detached(t)).err(), Some(ImportError::Canceled));
}

#[test]
fn unsupported_and_missing_files_say_so_plainly() {
    let dir = std::env::temp_dir().join(format!("lm-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dwg = dir.join("plan.dwg");
    std::fs::write(&dwg, b"AC1032 not really").unwrap();
    assert_eq!(import_path(&dwg, &Ctx::detached(Ticket::default())).err().unwrap().to_string(), "LayeredMap can't open .dwg files yet.");
    let gone = import_path(&dir.join("gone.csv"), &Ctx::detached(Ticket::default())).err().unwrap();
    assert!(gone.to_string().contains("no longer where it was"), "{gone}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_job_thread_reports_stages_and_finishes() {
    let job = start(fixtures().join("V1/districts.geojson"), || {});
    let t = std::time::Instant::now();
    let mut stages = Vec::new();
    loop {
        for e in job.poll() {
            match e {
                ImportEvent::Stage(s) => stages.push(s),
                ImportEvent::Preparing => {}
                ImportEvent::Done(r) => {
                    assert!(r.is_ok());
                    assert!(stages.contains(&Stage::Profile));
                    return;
                }
            }
        }
        assert!(t.elapsed().as_secs() < 30, "import never finished");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// One stress result for the gate report (Stress tests: fixture ID · measured value · threshold ·
/// pass/fail), printed as a `STRESS` line that `cargo xtask stress` collects.
pub(super) fn stress_line(fixture: &str, measured: &str, threshold: &str, pass: bool) -> bool {
    println!("STRESS\t{fixture}\t{measured}\t{threshold}\t{}", if pass { "pass" } else { "FAIL" });
    pass
}

/// Imports a generated file and checks that it, and every file beside it, is byte-identical
/// afterwards (Gate 2: the source-unchanged hash check). Returns the result, the seconds the
/// import alone took, and whether the files are unchanged.
fn import_unchanged(id: &str, p: &std::path::Path) -> (Result<Vec<ImportedLayer>, ImportError>, f64, bool) {
    let siblings: Vec<PathBuf> = p.parent().and_then(|d| std::fs::read_dir(d).ok()).map_or_else(Vec::new, |rd| {
        rd.flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && !p.file_name().and_then(|n| n.to_str()).unwrap_or("").starts_with('.'))
            .collect()
    });
    let before: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
    let t = std::time::Instant::now();
    let r = import_path(p, &Ctx::detached(Ticket::default()));
    let secs = t.elapsed().as_secs_f64();
    let after: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
    let same = before == after;
    stress_line(&format!("{id} hash"), if same { "unchanged" } else { "CHANGED" }, "byte-identical", same);
    (r, secs, same)
}

/// The large stress fixtures (Gate 2): run with
/// `cargo test -p lm-jobs --release large_fixtures -- --ignored --nocapture`, or `cargo xtask stress`.
/// Every check runs and prints its line before the test fails on any of them.
#[test]
#[ignore = "needs the generated fixtures (several GB); run by hand or by cargo xtask stress"]
fn large_fixtures() {
    let Some(_) = generated() else {
        panic!("no generated fixtures: set LM_FIXTURES_DIR or run cargo xtask fixtures --large");
    };
    let mut ok = true;
    let mut missing = Vec::new();
    // X1: a ZIP bomb is rejected in under 1 second.
    match generated_file("X1", "zip") {
        Some(p) => {
            let (r, secs, same) = import_unchanged("X1", &p);
            let refused = matches!(r, Err(ImportError::Refused(_)));
            ok &= same & stress_line("X1", &format!("{secs:.3} s, refused: {refused}"), "refused in < 1 s", refused && secs < 1.0);
        }
        None => missing.push("X1"),
    }
    // T3: a 1,000,000-row CSV opens.
    match generated_file("T3", "csv") {
        Some(p) => {
            let (r, secs, same) = import_unchanged("T3", &p);
            let count = r.as_ref().ok().and_then(|l| l.first()).map_or(0, |l| l.profile.count);
            ok &= same & stress_line("T3", &format!("{count} rows in {secs:.2} s"), "opens, >= 1,000,000 rows", count >= 1_000_000);
            ok &= super::gate2_tests::check_generated_snapshot("T3", &[("t3_rows.csv".to_string(), r)]);
        }
        None => missing.push("T3"),
    }
    // R4: the 2 GB COG previews within 3 seconds.
    match generated_file("R4", "tif") {
        Some(p) => {
            // Timed first, before hashing reads the whole file into the system's cache.
            let t = std::time::Instant::now();
            let r = import_path(&p, &Ctx::detached(Ticket::default()));
            let secs = t.elapsed().as_secs_f64();
            let opened = r.is_ok();
            ok &= stress_line("R4", &format!("{secs:.2} s, opened: {opened}"), "preview < 3 s", opened && secs < 3.0);
            ok &= import_unchanged("R4", &p).2;
            ok &= super::gate2_tests::check_generated_snapshot("R4", &[("r4_cog.tif".to_string(), r)]);
        }
        None => missing.push("R4"),
    }
    // R5: the preparation step for the 4 GB TIFF without overviews can be canceled.
    match generated_file("R5", "tif") {
        Some(p) => {
            let before = sha(&p);
            let job = start(p.clone(), || {});
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let read = job.ticket.bytes();
            let t = std::time::Instant::now();
            job.cancel();
            let mut result = None;
            while t.elapsed().as_secs() < 10 {
                if let Some(ImportEvent::Done(r)) = job.poll().into_iter().find(|e| matches!(e, ImportEvent::Done(_))) {
                    result = Some(r);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let stopped = t.elapsed().as_secs_f64();
            let canceled = matches!(result, Some(Err(ImportError::Canceled)));
            ok &= stress_line(
                "R5",
                &format!("canceled after {} of {} bytes; stopped {stopped:.2} s after cancel", read.0, read.1),
                "canceled, stops < 2 s",
                canceled && stopped < 2.0,
            );
            let same = sha(&p) == before;
            ok &= stress_line("R5 hash", if same { "unchanged" } else { "CHANGED" }, "byte-identical", same);
        }
        None => missing.push("R5"),
    }
    println!("STRESS\tV6\tnot run: the 500 MB OpenStreetMap extract is not in the fixtures yet\topens, map stays interactive\tn/a");
    if !missing.is_empty() {
        eprintln!("missing generated fixtures: {} (cargo xtask fixtures --large)", missing.join(", "));
    }
    assert!(ok, "a stress check failed (see the STRESS lines)");
    assert!(missing.is_empty(), "missing generated fixtures: {missing:?}");
}

#[test]
fn the_problem_list_holds_each_excluded_row_and_defuses_formulas() {
    let l = open("T1/schools.csv").unwrap().remove(0);
    let csv = problem_list_csv(&l);
    assert_eq!(csv.lines().count(), 1 + 6, "{csv}");
    assert!(csv.lines().next().unwrap().starts_with("row,reason,"));
    let mut x5 = l.clone();
    x5.profile.exclusions = vec![lm_core::model::Exclusion { row: 1, reason: "=HYPERLINK(\"x\")".into() }];
    x5.raw = None;
    assert!(problem_list_csv(&x5).contains("\"'=HYPERLINK(\"\"x\"\")\""));
}

#[test]
fn v4_where_should_this_data_be_finds_its_zone_near_shanghai() {
    let l = open("V4/districts_gk.shp").unwrap().remove(0);
    let c = place_candidates(&l, (121.47, 31.23));
    assert!(!c.is_empty(), "no candidates");
    assert!(c.len() <= 6);
    eprintln!("{:?}", c.iter().map(|x| (x.epsg, x.name.clone(), x.distance_km.round())).collect::<Vec<_>>());
    let best = &c[0];
    assert_eq!(best.epsg, 4549, "V4 expected.json: expected_top_candidate_epsg");
    assert!(best.distance_km < 30.0, "{best:?}");
    let placed = reproject(&l, u32::from(best.epsg)).unwrap();
    assert_eq!(placed.profile.kind, LayerKind::Areas);
    let e = placed.profile.extent_wgs84.unwrap();
    assert!((120.8..122.2).contains(&e.min_x) && (30.6..31.9).contains(&e.min_y), "{e:?}");
    let a = placed.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::NoCrsProjected).unwrap();
    assert!(a.confirmed_by_user && a.line.contains("100 m"), "{}", a.line);
    assert_ne!(placed.status, ImportStatus::NeedsDecision);
}

fn expected(id: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(fixtures().join(id).join("expected.json")).unwrap()).unwrap()
}

fn kind_word(k: LayerKind) -> &'static str {
    match k {
        LayerKind::Points => "point",
        LayerKind::Lines => "line",
        LayerKind::Areas => "area",
        _ => "other",
    }
}

#[test]
fn v9_geopackage_opens_as_one_group_of_three_layers() {
    let layers = open("V9/city_layers.gpkg").unwrap();
    let exp = &expected("V9")["groups"]["city_layers.gpkg"];
    assert_eq!(layers.len() as u64, exp["layer_count"].as_u64().unwrap());
    // One group: the same source file, each layer a member named after its table.
    let path = &layers[0].profile.source.path;
    assert!(path.ends_with("city_layers.gpkg"), "{path}");
    for l in &layers {
        assert_eq!(&l.profile.source.path, path);
        assert_eq!(l.profile.source.member.as_deref(), Some(l.name.as_str()));
        assert_eq!(l.profile.format, "GeoPackage");
        assert_eq!(l.profile.crs.value, lm_core::model::Crs::Wgs84, "{}", l.name);
        assert_eq!(l.profile.crs.confidence, lm_core::model::Confidence::Certain, "declared, not assumed");
        assert!(l.profile.crs.evidence.0.contains("EPSG:4326"), "{}", l.profile.crs.evidence.0);
        assert!(l.profile.exclusions.is_empty(), "{:?}", l.profile.exclusions);
        assert_eq!(l.status, ImportStatus::Complete, "{}: {:?}", l.name, l.profile.assumptions);
        assert!(l.profile.extent_wgs84.is_some());
    }
    for e in exp["layers"].as_array().unwrap() {
        let name = e["name"].as_str().unwrap();
        let l = layers.iter().find(|l| l.name == name).unwrap_or_else(|| panic!("no layer {name}"));
        assert_eq!(kind_word(l.profile.kind), e["geometry"].as_str().unwrap(), "{name}");
        assert_eq!(l.profile.count, e["count"].as_u64().unwrap(), "{name}");
    }
    // expected.json lists no assumptions for V9: nothing shown about the file (silent field roles
    // aside); a China offset notice may be added by the catalog.
    for l in &layers {
        assert!(
            l.profile.assumptions.iter().all(|a| a.treatment == Treatment::Silent || a.kind == AssumptionKind::ChinaOffsetPossible),
            "{}: {:?}",
            l.name,
            l.profile.assumptions
        );
    }
}

#[test]
fn v9_zipped_shapefile_in_nested_folders_still_opens() {
    let layers = open("V9/roads_bundle.zip").unwrap();
    let exp = &expected("V9")["archives"]["roads_bundle.zip"];
    assert_eq!(layers.len(), 1);
    let l = &layers[0];
    let e = &exp["layers"][0];
    assert_eq!(l.name, e["name"].as_str().unwrap());
    assert_eq!(kind_word(l.profile.kind), e["geometry"].as_str().unwrap());
    assert_eq!(l.profile.count, e["count"].as_u64().unwrap());
    assert_eq!(l.profile.source.member.as_deref(), exp["shapefile_path"].as_str());
    assert_eq!(l.profile.crs.value, lm_core::model::Crs::Wgs84);
}

/// The .gpkg and every SQLite sidecar beside it (present or not), hashed.
fn gpkg_state(gpkg: &std::path::Path) -> Vec<(String, Option<Vec<u8>>)> {
    ["", "-wal", "-shm", "-journal"]
        .into_iter()
        .map(|s| {
            let p = gpkg::sidecar(gpkg, s);
            (s.to_string(), p.exists().then(|| sha(&p)))
        })
        .collect()
}

#[test]
fn a_geopackage_and_its_sidecars_stay_byte_identical() {
    let gpkg = fixtures().join("V9/city_layers.gpkg");
    let before = gpkg_state(&gpkg);
    assert!(before.iter().skip(1).all(|(_, h)| h.is_none()), "the fixture has no sidecars to begin with");
    open("V9/city_layers.gpkg").unwrap();
    assert_eq!(before, gpkg_state(&gpkg), "the GeoPackage or a sidecar changed or appeared");
}

/// A GeoPackage another program has open in WAL mode, with a change still in its -wal: the change
/// is read (from a settled copy), and the file, its -wal and -shm stay exactly as they were.
#[test]
fn a_geopackage_with_changes_in_its_wal_is_read_whole_and_left_alone() {
    let dir = std::env::temp_dir().join(format!("lm-gpkg-wal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let gpkg = dir.join("city.gpkg");
    std::fs::copy(fixtures().join("V9/city_layers.gpkg"), &gpkg).unwrap();
    let writer = rusqlite::Connection::open(&gpkg).unwrap();
    writer.query_row("PRAGMA journal_mode = WAL", [], |r| r.get::<_, String>(0)).unwrap();
    writer.execute_batch("PRAGMA wal_autocheckpoint = 0;").unwrap();
    // The R-tree triggers call GeoPackage SQL functions plain SQLite lacks.
    let triggers: Vec<String> = {
        let mut st = writer.prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
    };
    for t in triggers {
        writer.execute_batch(&format!("DROP TRIGGER \"{t}\";")).unwrap();
    }
    let cols: Vec<String> = {
        let mut st = writer.prepare("SELECT name FROM pragma_table_info('schools') WHERE pk = 0").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
    };
    let cols = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    writer.execute_batch(&format!("INSERT INTO schools ({cols}) SELECT {cols} FROM schools LIMIT 1;")).unwrap();
    let before = gpkg_state(&gpkg);
    assert!(before[1].1.is_some() && before[2].1.is_some(), "the writer keeps a -wal and -shm");
    let files_before = std::fs::read_dir(&dir).unwrap().count();
    let layers = import_path(&gpkg, &Ctx::detached(Ticket::default())).unwrap();
    assert_eq!(before, gpkg_state(&gpkg), "the GeoPackage or a sidecar changed");
    assert_eq!(files_before, std::fs::read_dir(&dir).unwrap().count());
    let schools = layers.iter().find(|l| l.name == "schools").unwrap();
    assert_eq!(schools.profile.count, 9, "the row still in the -wal is read");
    drop(writer);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn t5_excel_opens_the_only_sheet_with_coordinates() {
    let l = open("T5/survey.xlsx").unwrap().remove(0);
    let exp = expected("T5");
    let want = exp["chosen_sheet"].as_str().unwrap();
    let sheet = exp["sheets"].as_array().unwrap().iter().find(|s| s["name"] == want).unwrap();
    assert_eq!(l.profile.format, "Excel");
    assert_eq!(l.profile.source.member.as_deref(), Some(want));
    assert_eq!(l.profile.kind, LayerKind::Points);
    assert_eq!(l.profile.count, sheet["rows"].as_u64().unwrap());
    assert!(l.profile.exclusions.is_empty(), "{:?}", l.profile.exclusions);
    assert_eq!(l.status, ImportStatus::Complete);
    let a = l.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ExcelSheet).unwrap();
    assert_eq!(a.treatment, Treatment::Visible);
    assert_eq!(a.line, format!("Opened sheet '{want}'."));
    let names: Vec<&str> = exp["sheets"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(a.alternatives.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), names, "the other sheets are the alternatives");
    // crs_wgs84_assumed: for tables, the coordinate-columns line says "as GPS coordinates (WGS 84)".
    let c = l.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::CsvCoordinateColumns).unwrap();
    assert_eq!(c.treatment, Treatment::Visible);
    let coords = &sheet["coordinates"];
    assert_eq!(c.line, lm_profile::lines::csv_located(coords["x"].as_str().unwrap(), coords["y"].as_str().unwrap()));
    // The cells are kept as written, for relocation and the problem list.
    assert_eq!(l.raw.as_ref().map(|r| r.rows.len()), Some(20));
    let LayerData::Vector(ds) = &l.data else { panic!() };
    for (field, _) in exp["fields"].as_object().unwrap() {
        assert!(ds.field_index(field).is_some(), "no field {field}");
    }
}

#[test]
fn t5_another_sheet_can_be_chosen_and_columns_changed_without_losing_the_sheet() {
    let l = open("T5/survey.xlsx").unwrap().remove(0);
    let summary = lm_core::model::Choice::new("sheet:Summary", "Summary");
    let s = choose_sheet(&l, &summary).unwrap();
    assert_eq!(s.name, l.name);
    assert_eq!(s.profile.source.member.as_deref(), Some("Summary"));
    assert_eq!(s.profile.kind, LayerKind::Table);
    let a = s.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ExcelSheet).unwrap();
    assert_eq!((a.chosen.id.as_str(), a.confirmed_by_user), ("sheet:Summary", true));
    assert_eq!(a.line, "Opened sheet 'Summary'.");
    assert!(choose_sheet(&l, &lm_core::model::Choice::not_sure()).is_err());
    // Changing the coordinate columns keeps the format, the sheet and the sheet choice.
    let t = relocate_csv(&l, &lm_core::model::Choice::not_sure()).unwrap();
    assert_eq!(t.profile.kind, LayerKind::Table);
    assert_eq!(t.profile.format, "Excel");
    assert_eq!(t.profile.source.member.as_deref(), Some("Schools"));
    assert!(has(&t, AssumptionKind::ExcelSheet));
}

#[test]
fn excel_sources_stay_byte_identical() {
    let x = fixtures().join("T5/survey.xlsx");
    let before = sha(&x);
    open("T5/survey.xlsx").unwrap();
    assert_eq!(before, sha(&x));
}

#[test]
fn switched_off_p1_formats_are_refused_plainly() {
    let gpkg = fixtures().join("V9/city_layers.gpkg");
    let xlsx = fixtures().join("T5/survey.xlsx");
    let msg = |p: &std::path::Path, head: &[u8]| pick_adapter(p, head, false).err().map(|e| e.to_string());
    assert_eq!(msg(&gpkg, b"SQLite format 3\0"), Some("LayeredMap can't open .gpkg files yet.".into()));
    // An .xlsx starts like a ZIP: it must not reach the zipped-Shapefile reader.
    assert_eq!(msg(&xlsx, b"PK\x03\x04"), Some("LayeredMap can't open .xlsx files yet.".into()));
    assert_eq!(pick_adapter(&gpkg, b"SQLite format 3\0", true).map(|a| a.name()).ok(), Some("GeoPackage"));
    assert_eq!(pick_adapter(&xlsx, b"PK\x03\x04", true).map(|a| a.name()).ok(), Some("Excel"));
    assert_eq!(
        pick_adapter(&fixtures().join("V9/roads_bundle.zip"), b"PK\x03\x04", false).map(|a| a.name()).ok(),
        Some("Zipped Shapefile")
    );
}

#[test]
fn canceled_p1_imports_return_nothing() {
    for f in ["V9/city_layers.gpkg", "T5/survey.xlsx"] {
        let t = Ticket::default();
        t.cancel();
        assert_eq!(import_path(&fixtures().join(f), &Ctx::detached(t)).err(), Some(ImportError::Canceled), "{f}");
        // Canceled after detection: each reader checks the ticket while reading.
        let t = Ticket::default();
        let ctx = Ctx::detached(t.clone());
        let p = fixtures().join(f);
        let a = pick_adapter(&p, b"", true).unwrap();
        let m = a.open_metadata(&p, &ctx).unwrap();
        t.cancel();
        assert_eq!(a.stream_features(&p, &m, &ctx).err(), Some(ImportError::Canceled), "{f}");
    }
}

#[test]
fn zip_based_documents_are_not_taken_for_zipped_shapefiles() {
    // survey.xlsx-style input goes to Excel, by extension, before the ZIP magic is looked at.
    let xlsx = fixtures().join("T5/survey.xlsx");
    let head = std::fs::read(&xlsx).unwrap();
    assert!(head.starts_with(b"PK\x03\x04"));
    assert_eq!(pick_adapter(&xlsx, &head, true).map(|a| a.name()).ok(), Some("Excel"));
    assert_eq!(pick_adapter(std::path::Path::new("Survey.XLSX"), b"PK\x03\x04", true).map(|a| a.name()).ok(), Some("Excel"));
    for f in ["report.docx", "places.kmz", "slides.pptx", "sheet.ods"] {
        let ext = f.rsplit('.').next().unwrap();
        let e = pick_adapter(std::path::Path::new(f), b"PK\x03\x04", true).err().map(|e| e.to_string());
        assert_eq!(e, Some(format!("LayeredMap can't open .{ext} files yet.")), "{f}");
    }
    // A ZIP without a telling name is still tried as a zipped Shapefile.
    assert_eq!(pick_adapter(std::path::Path::new("download"), b"PK\x03\x04", true).map(|a| a.name()).ok(), Some("Zipped Shapefile"));
}

/// R6: scale 0.1 and offset −100 come from the GDAL metadata, so the values shown are decoded
/// (about 0–160), never the raw integers (a wrong number would be an S1).
#[test]
fn r6_scale_and_offset_are_applied() {
    let l = open("R6/scaled.tif").unwrap().remove(0);
    let b = &l.profile.bands[0];
    assert_eq!(b.scale, Some(0.1), "{b:?}");
    assert_eq!(b.offset, Some(-100.0));
    let (lo, hi) = (b.min.unwrap(), b.max.unwrap());
    assert!(lo >= -1.0 && hi <= 161.0 && hi > 50.0, "decoded range {lo}..{hi}");
}

#[test]
fn geojson_states_no_assumed_gps_line() {
    let l = open("V1/districts.geojson").unwrap().remove(0);
    assert!(!has(&l, AssumptionKind::NoCrsLonLat), "{:?}", l.profile.assumptions);
    assert_eq!(l.profile.crs.value, lm_core::model::Crs::Wgs84);
}

/// Review finding (S1): the NoData line must be true. R6's −9999 border is left out of the band's
/// numbers and drawn clear; choosing "a real value" puts it back, and the line says so.
#[test]
fn r6_detected_nodata_is_applied_and_can_be_changed() {
    let l = open("R6/scaled.tif").unwrap().remove(0);
    let LayerData::Raster(r) = &l.data else { panic!() };
    assert_eq!(r.nodata, Some(-9999.0));
    let b = &l.profile.bands[0];
    assert!(b.min.unwrap() > -1.0, "the −9999 border is out of the range: {:?}", b.min);
    let a = l.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::RasterNoData).unwrap().clone();
    let real = a.alternatives.iter().find(|c| c.id == "value").unwrap().clone();
    let changed = rechoose_raster(&l, a.kind, &a.subject, &real).unwrap();
    let LayerData::Raster(r2) = &changed.data else { panic!() };
    assert_eq!(r2.nodata, None);
    assert!(changed.profile.bands[0].min.unwrap() < -1000.0, "now −9999 × 0.1 − 100 is in the range");
    let line = &changed.profile.assumptions.iter().find(|x| x.kind == AssumptionKind::RasterNoData).unwrap().line;
    assert!(line.contains("real value"), "{line}");
}

/// Review finding (S1): changing the text encoding re-reads the file in that encoding.
#[test]
fn v5_text_encoding_can_be_changed() {
    let l = open("V5/places_gbk.shp").unwrap().remove(0);
    let utf8 = lm_core::model::Choice::new("utf8", "Unicode (UTF-8)");
    let changed = reread_encoding(&l, &utf8).unwrap();
    let LayerData::Vector(ds) = &changed.data else { panic!() };
    let name = ds.field_index("name").unwrap();
    assert_ne!(ds.value(0, name), &rustgis_engine::data::Value::Text("人民广场".into()));
    let back = reread_encoding(&changed, &lm_core::model::Choice::new("gbk", "Chinese (GBK)")).unwrap();
    let LayerData::Vector(ds) = &back.data else { panic!() };
    assert_eq!(ds.value(0, name), &rustgis_engine::data::Value::Text("人民广场".into()));
}

/// Writes `text` to a fresh temporary CSV and imports it.
fn import_text(file: &str, text: &str) -> ImportedLayer {
    let dir = std::env::temp_dir().join(format!("lm-numstyle-{}-{file}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(file);
    std::fs::write(&path, text).unwrap();
    let l = import_path(&path, &Ctx::detached(Ticket::default())).unwrap().remove(0);
    let _ = std::fs::remove_dir_all(dir);
    l
}

fn column(l: &ImportedLayer, name: &str) -> Vec<rustgis_engine::data::Value> {
    let LayerData::Vector(ds) = &l.data else { panic!() };
    let i = ds.field_index(name).unwrap();
    (0..ds.features.len()).map(|r| ds.value(r, i).clone()).collect()
}

#[test]
fn csv_thousands_separators_are_read_as_thousands() {
    // Regression (S1): "12,500" was read as 12.5.
    use rustgis_engine::data::Value;
    let l = import_text(
        "towns.csv",
        "name,population,lon,lat\nA,\"12,500\",121.40,31.20\nB,\"3,400\",121.41,31.21\nC,\"1,250,000\",121.42,31.22\nD,980,121.43,31.23\n",
    );
    assert_eq!(column(&l, "population"), vec![Value::Int(12_500), Value::Int(3_400), Value::Int(1_250_000), Value::Int(980)]);
    // Only d,ddd values and a comma delimiter: not guessed, kept as written.
    let l = import_text("towns2.csv", "name,population,lon,lat\nA,\"12,500\",121.40,31.20\nB,\"3,400\",121.41,31.21\n");
    assert_eq!(column(&l, "population"), vec![Value::Text("12,500".into()), Value::Text("3,400".into())]);
    // With semicolons, decimal commas.
    let l = import_text("werte.csv", "name;wert;lon;lat\nA;12,500;121,40;31,20\nB;3,400;121,41;31,21\n");
    assert_eq!(column(&l, "wert"), vec![Value::Float(12.5), Value::Float(3.4)]);
    assert_eq!(l.profile.kind, LayerKind::Points);
}

/// Review finding (S1): the header counts located points, not rows with no location.
#[test]
fn t1_counts_located_points() {
    let l = open("T1/schools.csv").unwrap().remove(0);
    assert_eq!(l.profile.count + l.profile.exclusions.len() as u64, 248, "{}", l.profile.subtitle());
    assert_eq!(l.profile.subtitle(), format!("{} points · schools.csv", l.profile.count));
}

/// Review finding (S2): nothing is dropped silently: extra CSV cells and GeoJSON ids are kept.
#[test]
fn ragged_rows_and_feature_ids_are_kept() {
    let csv = import_text("ragged.csv", "name,lon,lat\nA,121.4,31.2,extra\nB,121.5,31.3\n");
    let LayerData::Vector(ds) = &csv.data else { panic!() };
    assert!(ds.field_index("column 4").is_some(), "{:?}", ds.fields.iter().map(|f| &f.name).collect::<Vec<_>>());
    let dir = std::env::temp_dir().join(format!("lm-gj-id-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("ids.geojson");
    std::fs::write(&p, r#"{"type":"FeatureCollection","features":[{"type":"Feature","id":"a1","geometry":{"type":"Point","coordinates":[1,2]},"properties":{"n":1}}]}"#).unwrap();
    let l = import_path(&p, &Ctx::detached(Ticket::default())).unwrap().remove(0);
    let LayerData::Vector(ds) = &l.data else { panic!() };
    let i = ds.field_index("id").unwrap();
    assert_eq!(ds.value(0, i), &rustgis_engine::data::Value::Text("a1".into()));
    let _ = std::fs::remove_dir_all(dir);
}

/// Review finding (S3): a CSV whose first 64 KiB are ASCII but which is GBK further on.
#[test]
fn gbk_after_an_ascii_head_is_read_as_gbk() {
    let mut text = String::from("name,lon,lat\n");
    for i in 0..4000 {
        text.push_str(&format!("site{i},121.4,31.2\n"));
    }
    text.push_str("人民广场,121.47,31.23\n");
    let (bytes, _, _) = encoding_rs::GBK.encode(&text);
    let dir = std::env::temp_dir().join(format!("lm-gbk-tail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("tail.csv");
    std::fs::write(&p, &bytes).unwrap();
    let l = import_path(&p, &Ctx::detached(Ticket::default())).unwrap().remove(0);
    assert_eq!(l.profile.encoding.as_deref(), Some("GBK"));
    let LayerData::Vector(ds) = &l.data else { panic!() };
    assert_eq!(ds.value(4000, 0), &rustgis_engine::data::Value::Text("人民广场".into()));
    let _ = std::fs::remove_dir_all(dir);
}

/// Fuzzing (Gate 2) found a `.shp` alone whose header promises more than the file holds: the
/// reader returned the same error forever and the import never finished.
#[test]
fn a_truncated_shapefile_without_its_index_finishes() {
    let dir = std::env::temp_dir().join(format!("lm-import-trunc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut shp = std::fs::read(fixtures().join("V3/sites.shp")).unwrap();
    shp[24..28].copy_from_slice(&1_000_000u32.to_be_bytes());
    shp.truncate(shp.len() - 6);
    let p = dir.join("cut.shp");
    std::fs::write(&p, &shp).unwrap();
    let job = start(p, || {});
    let t = std::time::Instant::now();
    let result = loop {
        if let Some(ImportEvent::Done(r)) = job.poll().into_iter().find(|e| matches!(e, ImportEvent::Done(_))) {
            break r;
        }
        assert!(t.elapsed().as_secs() < 10, "the import never finished");
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let l = result.unwrap().remove(0);
    assert_eq!(l.profile.count, 24, "the shapes before the cut");
    assert!(l.profile.notices.iter().any(|n| n.contains("couldn't be read")), "{:?}", l.profile.notices);
    let _ = std::fs::remove_dir_all(dir);
}
