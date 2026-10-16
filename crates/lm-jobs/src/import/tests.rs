use std::path::PathBuf;

use lm_core::model::{AssumptionKind, ImportStatus, LayerKind, Treatment};

use super::*;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Result<Vec<ImportedLayer>, ImportError> {
    import_path(&fixtures().join(rel), &Ctx::detached(Ticket::default()))
}

fn has(l: &ImportedLayer, k: AssumptionKind) -> bool {
    l.profile.assumptions.iter().any(|a| a.kind == k)
}

fn sha(path: &std::path::Path) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(std::fs::read(path).unwrap()).to_vec()
}

#[test]
fn t1_csv_lands_with_a_visible_assumption_and_six_exclusions() {
    let l = open("T1/schools.csv").unwrap();
    let l = &l[0];
    assert_eq!(l.profile.kind, LayerKind::Points);
    assert_eq!(l.profile.count, 248);
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
                || !matches!(ext.as_str(), "csv" | "geojson" | "shp" | "zip" | "tif")
            {
                continue;
            }
            let siblings: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
            let before: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
            let r = import_path(&f, &Ctx::detached(Ticket::default()));
            let after: Vec<Vec<u8>> = siblings.iter().map(|p| sha(p)).collect();
            assert_eq!(before, after, "{} changed a source file", f.display());
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
    let generated = fixtures().join("generated");
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

/// The large stress fixtures (Gate 2): run with
/// `cargo test -p lm-jobs --release large_fixtures -- --ignored --nocapture`.
#[test]
#[ignore = "needs the generated fixtures (several GB); run by hand or by cargo xtask stress"]
fn large_fixtures() {
    let generated = fixtures().join("generated");
    let find = |dir: &str, ext: &str| {
        std::fs::read_dir(generated.join(dir))
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == ext) && !p.file_name().and_then(|n| n.to_str()).unwrap_or("").starts_with('.'))
    };
    // T3: a 1,000,000-row CSV opens.
    if let Some(p) = find("T3", "csv") {
        let t = std::time::Instant::now();
        let l = import_path(&p, &Ctx::detached(Ticket::default())).unwrap().remove(0);
        eprintln!("T3: {} rows, {:?}, {:?}", l.profile.count, l.profile.kind, t.elapsed());
        assert!(l.profile.count >= 1_000_000);
    }
    // R4: the 2 GB COG previews within 3 seconds.
    if let Some(p) = find("R4", "tif") {
        let t = std::time::Instant::now();
        let l = import_path(&p, &Ctx::detached(Ticket::default())).unwrap().remove(0);
        let secs = t.elapsed().as_secs_f64();
        eprintln!("R4: {:.2} s, notices {:?}", secs, l.profile.notices);
        assert!(secs < 3.0, "R4 took {secs:.2} s");
    }
    // R5: the 4 GB TIFF without overviews can be canceled during preparation.
    if let Some(p) = find("R5", "tif") {
        let job = start(p, || {});
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let read = job.ticket.bytes();
        let t = std::time::Instant::now();
        job.cancel();
        loop {
            if let Some(ImportEvent::Done(r)) = job.poll().into_iter().find(|e| matches!(e, ImportEvent::Done(_))) {
                eprintln!("R5: canceled after {} of {} bytes; stopped {:?} after cancel", read.0, read.1, t.elapsed());
                assert_eq!(r.err(), Some(ImportError::Canceled));
                assert!(t.elapsed().as_secs_f64() < 2.0);
                break;
            }
            assert!(t.elapsed().as_secs() < 10, "R5 did not stop after cancel");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
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
