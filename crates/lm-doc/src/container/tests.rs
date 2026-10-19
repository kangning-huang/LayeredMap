use super::*;
use crate::testutil::TempDir;
use lm_core::document::{Classes, ColorBy, Style};
use lm_core::filter::{Condition, Filter, Test};
use lm_core::model::*;
use lm_core::recipe::{Recipe, RecipeInput, ResultCard, ResultRecord};
use rustgis_data::{Coord, Field, FieldType, Geometry, GeometryType, Value};

fn profile(path: &str, kind: LayerKind, size: u64, mtime: i64) -> LayerProfile {
    LayerProfile {
        source: SourceRef { path: path.into(), member: None, size, mtime, sha256: None },
        format: "GeoJSON".into(),
        kind,
        count: 3,
        extent_wgs84: Some(Extent { min_x: 121.0, min_y: 31.0, max_x: 122.0, max_y: 31.5 }),
        crs: Known::certain(Crs::Wgs84, "declared"),
        fields: vec![],
        bands: vec![],
        assumptions: vec![],
        exclusions: vec![],
        encoding: None,
        notices: vec![],
        data_rev: 0,
        joins: Vec::new(),
        grid: None,
    }
}

fn add(d: &mut Document, p: LayerProfile, name: &str) -> LayerId {
    let tx = d.add_layer_tx(p);
    d.apply(&tx).unwrap();
    let l = d.layers.iter_mut().max_by_key(|l| l.id).unwrap();
    l.display_name = name.into();
    l.id
}

fn points() -> Dataset {
    let fields = vec![
        Field::new("name", FieldType::Text),
        Field::new("count", FieldType::Integer),
        Field::new("share", FieldType::Double),
        Field::new("open", FieldType::Boolean),
    ];
    let mut ds = Dataset::new("Schools near parks", GeometryType::Point, fields);
    ds.push(
        Some(Geometry::Point(Coord::new(121.47, 31.23))),
        vec![Value::Text("Ā school".into()), Value::Int(12), Value::Float(0.25), Value::Bool(true)],
    );
    ds.push(Some(Geometry::Point(Coord::new(121.5, 31.2))), vec![Value::Null, Value::Int(-3), Value::Null, Value::Bool(false)]);
    ds.push(Some(Geometry::Point(Coord::new(-0.5, 51.5))), vec![Value::Text("北京".into()), Value::Null, Value::Float(1e-9), Value::Null]);
    ds
}

fn areas() -> Dataset {
    let ring = |x: f64| vec![Coord::new(x, 0.0), Coord::new(x + 1.0, 0.0), Coord::new(x + 1.0, 1.0), Coord::new(x, 0.0)];
    let mut ds = Dataset::new("Area 1", GeometryType::Polygon, vec![Field::new("name", FieldType::Text)]);
    ds.push(Some(Geometry::Polygon(vec![vec![ring(0.0)], vec![ring(5.0)]])), vec![Value::Text("Area 1".into())]);
    ds
}

fn table() -> Dataset {
    let mut ds =
        Dataset::new("Ranking", GeometryType::None, vec![Field::new("district", FieldType::Text), Field::new("n", FieldType::Integer)]);
    ds.push(None, vec![Value::Text("Pudong".into()), Value::Int(31)]);
    ds.push(None, vec![Value::Text("Xuhui".into()), Value::Int(12)]);
    ds
}

fn raster() -> Raster {
    use rustgis_data::raster::{ColorHint, GeoTransform, crs::Crs as RCrs};
    let t = GeoTransform { origin_x: 120.0, origin_y: 32.0, pixel_width: 0.5, pixel_height: -0.5 };
    Raster::new("heat", 3, 2, vec![vec![1.0, 2.0, 3.5, -1.0, 0.0, 30.25]], t, RCrs::LonLat, Some(-1.0), ColorHint::default(), "Float32")
        .unwrap()
}

/// A document with a file source and three owned results (points, areas, a table) and a raster.
fn with_results(dir: &Path) -> (Document, Vec<(LayerId, OwnedData)>) {
    let src = dir.join("data").join("districts.geojson");
    std::fs::create_dir_all(src.parent().unwrap()).unwrap();
    std::fs::write(&src, b"{\"type\":\"FeatureCollection\",\"features\":[]}").unwrap();
    let fp = crate::fingerprint(&src, true).unwrap();
    let mut d = Document::new();
    let mut p = profile(&fp.path, LayerKind::Areas, fp.size, fp.mtime);
    p.source.sha256 = fp.sha256.clone();
    add(&mut d, p, "Districts");
    let a = add(&mut d, profile("Schools near parks", LayerKind::Points, 0, 0), "Schools near parks");
    let b = add(&mut d, profile("Area 1", LayerKind::Areas, 0, 0), "Area 1");
    let c = add(&mut d, profile("/derived/ranking", LayerKind::Table, 0, 0), "Ranking");
    let r = add(&mut d, profile("/derived/heat", LayerKind::Raster, 0, 0), "Heat above 30");
    d.view = Some((121.47, 31.23, 9.0));
    let owned = vec![
        (a, OwnedData::Vector(Arc::new(points()))),
        (b, OwnedData::Vector(Arc::new(areas()))),
        (c, OwnedData::Vector(Arc::new(table()))),
        (r, OwnedData::Raster(Arc::new(raster()))),
    ];
    (d, owned)
}

fn vector(o: &[(LayerId, OwnedData)], id: LayerId) -> Dataset {
    match o.iter().find(|(l, _)| *l == id).map(|(_, d)| d) {
        Some(OwnedData::Vector(ds)) => (**ds).clone(),
        _ => panic!("no vector for {id:?}"),
    }
}

#[test]
fn save_then_open_gives_the_same_document_and_results() {
    let t = TempDir::new("lmap-roundtrip");
    let (d, owned) = with_results(&t.0);
    let path = t.0.join("Heat study.lmap");
    let report = save_lmap(&d, &owned, &path).unwrap();
    assert!(report.skipped.is_empty(), "{report:?}");
    let back = load_lmap(&path).unwrap();
    assert_eq!(back.doc, d);
    assert!(back.notes.is_empty(), "{:?}", back.notes);
    assert!(back.relinked.is_empty());
    for (id, data) in &owned {
        match data {
            OwnedData::Vector(want) => {
                let got = vector(&back.owned, *id);
                assert_eq!(got.fields, want.fields, "{}", want.name);
                assert_eq!(got.features, want.features, "{}", want.name);
                assert_eq!(got.name, want.name);
            }
            OwnedData::Raster(want) => {
                let Some((_, OwnedData::Raster(got))) = back.owned.iter().find(|(l, _)| l == id) else { panic!("raster missing") };
                assert_eq!((got.width, got.height), (want.width, want.height));
                assert_eq!(got.bands[0].data, want.bands[0].data);
                assert_eq!(got.nodata, want.nodata);
                assert_eq!(got.transform, want.transform);
            }
        }
    }
    // Saving the reopened document gives the same document again.
    let again = t.0.join("again.lmap");
    save_lmap(&back.doc, &back.owned, &again).unwrap();
    assert_eq!(load_lmap(&again).unwrap().doc, d);
}

#[test]
fn the_file_is_a_zip_with_mimetype_first_and_the_tables_filled() {
    let t = TempDir::new("lmap-layout");
    let (d, owned) = with_results(&t.0);
    let path = t.0.join("maps").join("study.lmap");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    save_lmap(&d, &owned, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(&bytes[30..38], b"mimetype");
    assert_eq!(&bytes[38..38 + MIMETYPE.len()], MIMETYPE.as_bytes());
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let names: Vec<String> = zip.file_names().map(String::from).collect();
    for n in ["manifest.json", "results/layer-2.fgb", "results/layer-3.fgb", "results/layer-4.geojson", "results/layer-5.tif"] {
        assert!(names.iter().any(|x| x == n), "{n} in {names:?}");
    }
    let m: serde_json::Value = serde_json::from_slice(&read_entry(&mut zip, MANIFEST_FILE, MAX_MANIFEST_BYTES).unwrap()).unwrap();
    assert_eq!(m["format"], "layeredmap");
    assert_eq!(m["container"], 1);
    assert_eq!(m["version"], 0);
    assert_eq!(m["sources"][0]["relative"], "../data/districts.geojson");
    assert_eq!(m["sources"][0]["layers"], serde_json::json!([1]));
    assert_eq!(m["owned"].as_array().unwrap().len(), 4);
    assert_eq!(m["owned"][0]["format"], "FlatGeobuf");
    assert_eq!(m["owned"][2]["format"], "GeoJSON");
    assert_eq!(m["owned"][3]["format"], "GeoTIFF");
    assert!(m["software"].as_str().unwrap().starts_with(&format!("{} ", lm_core::brand::NAME)));
}

#[test]
fn saving_again_replaces_the_file_and_leaves_nothing_behind() {
    let t = TempDir::new("lmap-replace");
    let (mut d, owned) = with_results(&t.0);
    let path = t.0.join("a.lmap");
    save_lmap(&d, &owned, &path).unwrap();
    d.layers[0].display_name = "Renamed".into();
    save_lmap(&d, &owned, &path).unwrap();
    assert_eq!(load_lmap(&path).unwrap().doc.layers[0].display_name, "Renamed");
    let left: Vec<String> = std::fs::read_dir(&t.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert!(left.iter().all(|n| !n.contains(".saving-")), "{left:?}");
}

#[test]
fn a_failed_save_keeps_the_old_file() {
    let t = TempDir::new("lmap-fail");
    let (d, owned) = with_results(&t.0);
    let path = t.0.join("keep.lmap");
    save_lmap(&d, &owned, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    // A save into a folder that doesn't exist fails before anything is renamed.
    assert!(matches!(save_lmap(&d, &owned, &t.0.join("no such folder").join("keep.lmap")), Err(DocError::Write { .. })));
    // A save over a folder fails at the rename, and the temporary file is removed.
    let blocked = t.0.join("blocked.lmap");
    std::fs::create_dir(&blocked).unwrap();
    std::fs::write(blocked.join("x"), b"x").unwrap();
    assert!(save_lmap(&d, &owned, &blocked).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let left: Vec<String> = std::fs::read_dir(&t.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert!(left.iter().all(|n| !n.contains(".saving-")), "{left:?}");
}

#[test]
fn stale_temporary_files_are_removed_and_fresh_ones_kept() {
    let t = TempDir::new("lmap-stale");
    let path = t.0.join("m.lmap");
    let old = t.0.join(".m.lmap.saving-1-0");
    let fresh = t.0.join(".m.lmap.saving-2-0");
    std::fs::write(&old, b"half").unwrap();
    std::fs::write(&fresh, b"half").unwrap();
    let f = std::fs::File::options().write(true).open(&old).unwrap();
    f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600)).unwrap();
    drop(f);
    save_lmap(&Document::new(), &[], &path).unwrap();
    assert!(!old.exists());
    assert!(fresh.exists());
}

#[test]
fn a_source_moved_with_the_document_is_found_by_its_relative_path() {
    let t = TempDir::new("lmap-moved");
    let project = t.0.join("project");
    let (d, owned) = with_results(&project);
    let path = project.join("study.lmap");
    save_lmap(&d, &owned, &path).unwrap();
    let moved = t.0.join("elsewhere");
    std::fs::rename(&project, &moved).unwrap();
    let back = load_lmap(&moved.join("study.lmap")).unwrap();
    let want = std::path::absolute(moved.join("data").join("districts.geojson")).unwrap().display().to_string();
    let src = |doc: &Document| doc.layer(LayerId(1)).unwrap().profile.source.clone();
    assert_eq!(src(&back.doc).path, want);
    assert_eq!(back.relinked.len(), 1);
    assert_eq!(check_source(&src(&back.doc)), SourceState::Unchanged);
    // A different file at the relative path is not taken: the source stays missing.
    std::fs::write(moved.join("data").join("districts.geojson"), b"something else entirely").unwrap();
    std::fs::rename(&moved, t.0.join("third")).unwrap();
    let back = load_lmap(&t.0.join("third").join("study.lmap")).unwrap();
    assert!(back.relinked.is_empty());
    assert_eq!(src(&back.doc).path, src(&d).path);
}

#[test]
fn relative_paths() {
    assert_eq!(relative_path(Path::new("/a/b"), Path::new("/a/b/c.csv")).as_deref(), Some("c.csv"));
    assert_eq!(relative_path(Path::new("/a/b"), Path::new("/a/d/c.csv")).as_deref(), Some("../d/c.csv"));
    assert_eq!(relative_path(Path::new("/a/b/"), Path::new("/x/./y.tif")).as_deref(), Some("../../x/y.tif"));
    assert_eq!(normalize(Path::new("/a/b/../c")), PathBuf::from("/a/c"));
}

#[test]
fn newer_junk_and_damaged_files_are_refused() {
    let t = TempDir::new("lmap-refuse");
    let path = t.0.join("x.lmap");
    std::fs::write(&path, b"not a zip").unwrap();
    assert!(matches!(load_lmap(&path), Err(DocError::NotADocument { .. })));
    let write = |manifest: &serde_json::Value| {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        w.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(&serde_json::to_vec(manifest).unwrap()).unwrap();
        std::fs::write(&path, w.finish().unwrap().into_inner()).unwrap();
    };
    write(&serde_json::json!({"format": "layeredmap", "container": 3, "version": 0, "layers": [], "next_layer_id": 1}));
    let e = load_lmap(&path).err().unwrap();
    assert!(matches!(e, DocError::TooNew { found: 3, supported: 2 }), "{e}");
    write(&serde_json::json!({"format": "layeredmap", "container": 1, "version": 7, "layers": [], "next_layer_id": 1}));
    assert!(matches!(load_lmap(&path), Err(DocError::TooNew { found: 7, .. })));
    write(&serde_json::json!({"format": "other", "container": 1, "version": 0}));
    assert!(matches!(load_lmap(&path), Err(DocError::NotADocument { .. })));
    write(&serde_json::json!({"format": "layeredmap", "container": 1, "version": 0, "layers": "x", "next_layer_id": 1}));
    assert!(matches!(load_lmap(&path), Err(DocError::Damaged { .. })));
    // Unknown fields (a later phase's additions) are ignored.
    write(&serde_json::json!({"format": "layeredmap", "container": 1, "version": 0, "layers": [], "next_layer_id": 1, "notes": [1]}));
    assert_eq!(load_lmap(&path).unwrap().doc, Document::new());
}

#[test]
fn an_unreadable_owned_result_is_a_note_not_a_failure() {
    let t = TempDir::new("lmap-bad-result");
    let (d, _) = with_results(&t.0);
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let m = LmapManifest {
        format: FORMAT_NAME.into(),
        container: 1,
        saved_at: 0,
        software: String::new(),
        sources: vec![],
        owned: vec![OwnedEntry { layer: LayerId(2), entry: "results/layer-2.fgb".into(), format: "FlatGeobuf".into(), name: "x".into() }],
        package: None,
        document: d.clone(),
    };
    w.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
    w.write_all(&serde_json::to_vec(&m).unwrap()).unwrap();
    w.start_file("results/layer-2.fgb", zip::write::SimpleFileOptions::default()).unwrap();
    w.write_all(b"garbage").unwrap();
    let path = t.0.join("bad.lmap");
    std::fs::write(&path, w.finish().unwrap().into_inner()).unwrap();
    let back = load_lmap(&path).unwrap();
    assert_eq!(back.doc, d);
    assert!(back.owned.is_empty());
    assert_eq!(back.notes.len(), 1);
    assert!(back.notes[0].starts_with("Schools near parks could not be read"), "{}", back.notes[0]);
}

// ---------------------------------------------------------------------------------------------
// Fixture documents D1–D3 (Gates 2–4), saved as v0 folders by the developer save of their gate.

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/D")
}

/// What each gate's document holds: Gate 2 layers with answered assumptions and exclusions;
/// Gate 3 adds styles by value, filters, roles, starred fields and a join; Gate 4 adds a drawn
/// area and kept results with recipes.
fn gate_document(gate: u8) -> Document {
    let mut d = Document::new();
    let mut schools = profile("/Users/student/Downloads/schools.csv", LayerKind::Points, 18_204, 1_791_000_000);
    schools.format = "CSV".into();
    schools.count = 248;
    schools.fields = vec![FieldProfile {
        name: "enrolment".into(),
        dtype: DataType::Integer,
        role: Known::likely(FieldRole::Measure(MeasureKind::Count), "integer counts"),
        unit: Known::unknown(None, ""),
        stats: Some(FieldStats { min: Some(1.0), max: Some(900.0), median: Some(300.0), distinct: 200, nulls: 2 }),
    }];
    schools.assumptions = vec![Assumption {
        kind: AssumptionKind::CsvCoordinateColumns,
        subject: Subject::Layer,
        chosen: Choice::new("columns:lon,lat", "lon and lat"),
        alternatives: vec![Choice::new("columns:lon,lat", "lon and lat"), Choice::not_sure()],
        confidence: Confidence::Likely,
        consequence: Consequence::ChangesLocations,
        treatment: Treatment::Visible,
        line: "Located using lon and lat as GPS coordinates (WGS 84).".into(),
        confirmed_by_user: true,
    }];
    schools.exclusions = vec![Exclusion { row: 7, reason: "No coordinates".into() }];
    schools.encoding = Some("UTF-8".into());
    let mut districts = profile("/Users/student/Downloads/districts.shp", LayerKind::Areas, 52_100, 1_791_000_100);
    districts.format = "Shapefile".into();
    districts.count = 16;
    let s = add(&mut d, districts, "Districts");
    let p = add(&mut d, schools, "Schools");
    d.view = Some((121.47, 31.23, 10.5));
    if gate >= 3 {
        let l = d.layers.iter_mut().find(|l| l.id == p).unwrap();
        l.style = Style {
            color_by: Some(ColorBy {
                field: "enrolment".into(),
                categories: false,
                classes: Classes::Groups { count: 5, equal_steps: false },
                palette: "viridis".into(),
                per_km2: false,
                fit_to_view: false,
            }),
            ..l.style.clone()
        };
        l.filter = Some(Filter { conditions: vec![Condition { field: "enrolment".into(), test: Test::Above(100.0) }] });
        l.roles.insert("enrolment".into(), FieldRole::Measure(MeasureKind::Count));
        l.starred = vec!["enrolment".into()];
        let a = d.layers.iter_mut().find(|l| l.id == s).unwrap();
        a.profile.joins = vec![JoinRecord {
            table: "population.csv".into(),
            source: SourceRef {
                path: "/Users/student/Downloads/population.csv".into(),
                member: None,
                size: 900,
                mtime: 1_791_000_200,
                sha256: None,
            },
            fields: vec!["pop_2020".into()],
            spec: "name = district".into(),
        }];
        d.user_ordered = true;
    }
    if gate >= 4 {
        let area = add(&mut d, profile("Area 1", LayerKind::Areas, 0, 0), "Area 1");
        let recipe = Recipe {
            title: "Schools in each district".into(),
            action: "count_in_areas".into(),
            params: Default::default(),
            computed_at: 1_797_206_400,
            // As saved in the Gate 4 fixture (fixtures/D/D3), before the rename to NimbleMap.
            software: "LayeredMap 0.4.0".into(),
            inputs: vec![RecipeInput {
                name: "Schools".into(),
                file: "schools.csv".into(),
                fingerprint: "18204:0123456789abcdef".into(),
                count: 248,
                noun: "points".into(),
            }],
            scope: "all schools; all districts".into(),
            method: "point-in-area count".into(),
            decisions: vec![],
            assumptions: vec![],
            coordinates: Some("geodesic, on the WGS 84 ellipsoid".into()),
            excluded: vec!["6 schools with no usable location".into()],
        };
        d.results.push(ResultRecord {
            id: 1,
            layer: Some(s),
            recipe,
            version: 1,
            history: vec![],
            card: ResultCard::default(),
            about: Some(area),
        });
        d.next_result_id = 2;
    }
    d
}

/// Writes D1–D3 with the v0 developer save (`LM_WRITE_DOC_FIXTURES=1`).
#[test]
fn write_gate_fixture_documents() {
    if std::env::var_os("LM_WRITE_DOC_FIXTURES").is_none() {
        return;
    }
    for (n, gate) in [(1, 2), (2, 3), (3, 4)] {
        crate::manifest::save(&gate_document(gate), &fixtures().join(format!("D{n}"))).unwrap();
    }
}

/// Gate 5: every fixture document opens, saves as `.lmap`, and reopens with an equal state; the
/// v0 documents open as they were saved (format migration).
#[test]
fn fixture_documents_migrate_and_round_trip() {
    let t = TempDir::new("lmap-fixtures");
    let mut seen = 0;
    for (n, gate) in [(1, 2), (2, 3), (3, 4)] {
        let dir = fixtures().join(format!("D{n}"));
        let v0 = open_any(&dir).unwrap_or_else(|e| panic!("D{n}: {e}"));
        assert_eq!(v0.doc, gate_document(gate), "D{n} opens as saved at Gate {gate}");
        assert_eq!(open_any(&dir.join(MANIFEST_FILE)).unwrap().doc, v0.doc);
        let path = t.0.join(format!("D{n}.lmap"));
        save_lmap(&v0.doc, &v0.owned, &path).unwrap();
        assert_eq!(load_lmap(&path).unwrap().doc, v0.doc, "D{n} round trip");
        assert_eq!(open_any(&path).unwrap().doc, v0.doc);
        seen += 1;
    }
    assert_eq!(seen, 3);
}
