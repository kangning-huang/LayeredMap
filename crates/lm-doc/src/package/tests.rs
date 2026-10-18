use std::sync::Arc;

use super::*;
use crate::container::{MAX_MANIFEST_BYTES, load_lmap, read_entry, save_lmap};
use crate::manifest::MANIFEST_FILE;
use crate::source::{SourceState, check_source};
use crate::testutil::TempDir;
use lm_core::document::{Classes, ColorBy, Style};
use lm_core::model::*;
use rustgis_data::{Coord, Field, FieldType, Geometry, GeometryType, Value};

fn profile(path: &str, kind: LayerKind, size: u64, mtime: i64, fields: &[&str]) -> LayerProfile {
    LayerProfile {
        source: SourceRef { path: path.into(), member: None, size, mtime, sha256: None },
        format: "CSV".into(),
        kind,
        count: 2,
        extent_wgs84: None,
        crs: Known::certain(Crs::Wgs84, "declared"),
        fields: fields
            .iter()
            .map(|n| FieldProfile {
                name: (*n).into(),
                dtype: DataType::Text,
                role: Known::certain(FieldRole::Text, "test"),
                unit: Known::certain(None, "test"),
                stats: None,
            })
            .collect(),
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

/// A file source with its fingerprint (hashed), as a layer records it.
fn source(path: &Path, kind: LayerKind, fields: &[&str]) -> LayerProfile {
    let fp = crate::fingerprint(path, true).unwrap();
    let mut p = profile(&fp.path, kind, fp.size, fp.mtime, fields);
    p.source.sha256 = fp.sha256;
    p
}

fn contacts() -> Dataset {
    let fields =
        vec![Field::new("school", FieldType::Text), Field::new("phone", FieldType::Text), Field::new("pupils", FieldType::Integer)];
    let mut ds = Dataset::new("contacts", GeometryType::Point, fields);
    ds.push(
        Some(Geometry::Point(Coord::new(121.4, 31.2))),
        vec![Value::Text("A".into()), Value::Text("138 0013 8000".into()), Value::Int(300)],
    );
    ds.push(
        Some(Geometry::Point(Coord::new(121.5, 31.3))),
        vec![Value::Text("B".into()), Value::Text("021-6888-1234".into()), Value::Int(120)],
    );
    ds
}

/// A user's folder with a CSV, a Shapefile's parts and a file with personal data, and a map of
/// them: a styled layer, two layers from one file, and a result of LayeredMap's own.
struct Scene {
    home: TempDir,
    doc: Document,
    owned: Vec<(LayerId, OwnedData)>,
    csv: PathBuf,
    shp: PathBuf,
    people: LayerId,
    styled: LayerId,
}

fn scene(tag: &str) -> Scene {
    let home = TempDir::new(tag);
    let data = home.0.join("Users").join("ana").join("Downloads");
    std::fs::create_dir_all(&data).unwrap();
    let csv = data.join("schools.csv");
    std::fs::write(&csv, "name,lon,lat\nA,121.4,31.2\nB,121.5,31.3\n").unwrap();
    let shp = data.join("districts.shp");
    for (ext, body) in
        [("shp", &b"shp-bytes"[..]), ("shx", b"shx"), ("dbf", b"dbf-bytes"), ("prj", b"GEOGCS[\"WGS 84\"]"), ("cpg", b"UTF-8")]
    {
        std::fs::write(shp.with_extension(ext), body).unwrap();
    }
    // Not a companion: another file in the same folder.
    std::fs::write(data.join("unrelated.txt"), b"private notes").unwrap();
    let people_csv = data.join("contacts.csv");
    std::fs::write(&people_csv, "school,phone,pupils\nA,138 0013 8000,300\nB,021-6888-1234,120\n").unwrap();

    let mut d = Document::new();
    let styled = add(&mut d, source(&csv, LayerKind::Points, &["name"]), "Schools");
    let style = Style {
        color_by: Some(ColorBy {
            field: "name".into(),
            categories: true,
            classes: Classes::Smooth,
            palette: "viridis".into(),
            per_km2: false,
            fit_to_view: false,
        }),
        ..Style::default()
    };
    let tx = d.set_style_tx(styled, style).unwrap();
    d.apply(&tx).unwrap();
    add(&mut d, source(&csv, LayerKind::Points, &["name"]), "Schools again");
    add(&mut d, source(&shp, LayerKind::Areas, &[]), "Districts");
    let people = add(&mut d, source(&people_csv, LayerKind::Points, &["school", "phone", "pupils"]), "Contacts");
    d.layers.iter_mut().find(|l| l.id == people).unwrap().starred = vec!["phone".into(), "pupils".into()];
    let made = add(&mut d, profile("Buffer 500 m", LayerKind::Points, 0, 0, &[]), "Buffer 500 m");
    d.view = Some((121.47, 31.23, 9.0));
    let owned = vec![(made, OwnedData::Vector(Arc::new(contacts()))), (people, OwnedData::Vector(Arc::new(contacts())))];
    // Only the result is owned in the map itself; Contacts is passed for its dropped field.
    Scene { home, doc: d, owned, csv, shp, people, styled }
}

fn manifest_of(path: &Path) -> (serde_json::Value, Vec<String>) {
    let file = std::fs::File::open(path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names = zip.file_names().map(String::from).collect();
    let bytes = read_entry(&mut zip, MANIFEST_FILE, MAX_MANIFEST_BYTES).unwrap();
    (serde_json::from_slice(&bytes).unwrap(), names)
}

#[test]
fn companions_are_the_parts_with_the_same_name() {
    let s = scene("pkg-companions");
    let got: Vec<String> = companions(&s.shp).iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
    assert_eq!(got, ["districts.shp", "districts.cpg", "districts.dbf", "districts.prj", "districts.shx"]);
    assert_eq!(companions(&s.csv).len(), 1);
    // A folder brings its files, not macOS's metadata files.
    let dir = s.home.0.join("gdb");
    std::fs::create_dir_all(dir.join("inner")).unwrap();
    std::fs::write(dir.join("a.table"), b"a").unwrap();
    std::fs::write(dir.join("inner").join("b.table"), b"b").unwrap();
    std::fs::write(dir.join("._a.table"), b"x").unwrap();
    assert_eq!(companions(&dir).len(), 2);
}

#[test]
fn review_lists_sources_sizes_and_the_paths_it_removes() {
    let s = scene("pkg-review");
    let owned = [s.doc.layers.last().unwrap().id];
    let r = review(&s.doc, &owned, &PackageOptions::default());
    let names: Vec<&str> = r.sources.iter().map(PackedSource::file_name).collect();
    assert_eq!(names, ["districts.shp", "schools.csv", "contacts.csv"], "in layer order, bottom first");
    assert_eq!(r.sources[1].layers.len(), 2, "one file, read by two layers, is carried once");
    assert_eq!(r.sources[0].files.len(), 5);
    assert_eq!(r.sources[0].size, 9 + 3 + 9 + 16 + 5);
    let csv = s.csv.display().to_string();
    assert!(r.stripped.iter().any(|x| x.from == csv && x.to == "data/2/schools.csv"), "{:?}", r.stripped);
    // Leaving a field out of Contacts stores it as a result instead: its file isn't carried.
    let opts = PackageOptions { dropped: vec![(s.people, vec!["phone".into()])], ..Default::default() };
    let r = review(&s.doc, &owned, &opts);
    assert_eq!(r.sources.len(), 2);
    assert!(r.stripped.iter().any(|x| x.to == "contacts.csv"), "the dropped layer keeps only its file name: {:?}", r.stripped);
}

#[test]
fn absolute_paths_are_recognised() {
    for p in [
        "/Users/ana/Downloads/a.csv",
        "/home/li/data/b.tif",
        "C:\\Users\\Ana\\c.shp",
        "D:/maps/d.gpkg",
        "\\\\server\\share\\e.csv",
        "/Users/ana/My Data/x.csv",
    ] {
        assert!(looks_absolute(p), "{p}");
    }
    for p in ["schools.csv", "data/1/schools.csv", "km/h", "/", "/x", "Pudong / Xuhui", " /a/b", "https://example.com/a/b"] {
        assert!(!looks_absolute(p), "{p}");
    }
    let mut v = serde_json::json!({ "a": "/Users/ana/x/y.csv", "b": ["C:\\Users\\Ana\\z.tif", 3], "c": { "d": "plain" } });
    let mut out = Vec::new();
    strip_paths(&mut v, &mut out);
    assert_eq!(v, serde_json::json!({ "a": "y.csv", "b": ["z.tif", 3], "c": { "d": "plain" } }));
    assert_eq!(out.len(), 2);
}

#[test]
fn unsafe_entry_names_are_not_unpacked() {
    assert_eq!(safe_parts("data/1/schools.csv"), Some(PathBuf::from("schools.csv")));
    assert_eq!(safe_parts("data/12/gdb/a.table"), Some(PathBuf::from("gdb/a.table")));
    for bad in
        ["data/1/../../etc/passwd", "data/x/a.csv", "results/a.csv", "data/1/", "data/1//a", "data/1/C:\\a", "/data/1/a", "data/1/./a"]
    {
        assert_eq!(safe_parts(bad), None, "{bad}");
    }
}

#[test]
fn private_fields_are_dropped_from_the_layer_and_its_data() {
    let ds = drop_fields(&contacts(), &["phone".into()]);
    assert_eq!(ds.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["school", "pupils"]);
    assert_eq!(ds.features[1].attrs, vec![Value::Text("B".into()), Value::Int(120)]);
}

/// The package carries every source, opens with the original files moved away, and gives the
/// same layers, styles and results back; nothing in it names the user's folders; the phone field
/// left out is gone from the layer and its data.
#[test]
fn a_package_opens_elsewhere_with_the_same_map() {
    let s = scene("pkg-roundtrip");
    let out = TempDir::new("pkg-roundtrip-out");
    let path = out.0.join("Heat study.lmap");
    let made = s.doc.layers.last().unwrap().id;
    let opts = PackageOptions {
        dropped: vec![(s.people, vec!["phone".into()])],
        licenses: vec![(s.csv.display().to_string(), "CC BY 4.0".into())],
    };
    let report = save_package(&s.doc, &s.owned, &opts, &path).unwrap();
    assert!(report.save.skipped.is_empty() && report.missing.is_empty(), "{report:?}");
    assert!(report.bytes > 0);

    let (m, names) = manifest_of(&path);
    assert_eq!(m["container"], 2);
    for n in [
        "data/2/schools.csv",
        "data/1/districts.shp",
        "data/1/districts.dbf",
        "data/1/districts.prj",
        "results/layer-4.fgb",
        "results/layer-5.fgb",
    ] {
        assert!(names.iter().any(|x| x == n), "{n} in {names:?}");
    }
    assert!(!names.iter().any(|n| n.contains("contacts.csv") || n.contains("unrelated")), "{names:?}");
    assert_eq!(m["sources"][1]["license"], "CC BY 4.0");
    assert_eq!(m["sources"][1]["embedded"][0], "data/2/schools.csv");
    assert_eq!(m["package"]["dropped"][0]["fields"], serde_json::json!(["phone"]));
    let raw = String::from_utf8_lossy(&serde_json::to_vec(&m).unwrap()).to_string();
    assert!(!raw.contains("/Users/") && !raw.contains(&s.home.0.display().to_string()), "an absolute path leaked: {raw}");
    assert!(!raw.contains("138 0013 8000"));

    // The original folder goes away: the package alone gives the map back.
    let original = s.doc.clone();
    drop(s.home);
    let back = load_lmap(&path).unwrap();
    assert!(back.package, "opens as a package");
    assert!(back.notes.is_empty(), "{:?}", back.notes);
    assert_eq!(back.doc.layers.len(), original.layers.len());
    for (a, b) in back.doc.layers.iter().zip(&original.layers) {
        assert_eq!((a.id, &a.display_name, a.visible, &a.style, &a.filter), (b.id, &b.display_name, b.visible, &b.style, &b.filter));
    }
    assert_eq!(back.doc.results, original.results);
    assert_eq!(back.doc.view, original.view);
    let schools = back.doc.layer(s.styled).unwrap();
    assert!(schools.style.color_by.is_some());
    let unpacked = Path::new(&schools.profile.source.path);
    assert!(unpacked.starts_with(unpack_dir()), "{}", unpacked.display());
    assert_eq!(std::fs::read_to_string(unpacked).unwrap(), "name,lon,lat\nA,121.4,31.2\nB,121.5,31.3\n");
    assert_eq!(check_source(&schools.profile.source), SourceState::Unchanged, "same bytes and time as packaged");
    let named = |n: &str| back.doc.layers.iter().find(|l| l.display_name == n).unwrap();
    assert_eq!(named("Schools again").profile.source.path, schools.profile.source.path, "two layers, one file");
    let districts = Path::new(&named("Districts").profile.source.path);
    assert!(districts.with_extension("dbf").is_file() && districts.with_extension("prj").is_file());
    // Contacts: stored as a result without the phone field.
    let contacts = back.doc.layer(s.people).unwrap();
    assert_eq!(contacts.profile.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["school", "pupils"]);
    assert_eq!(contacts.starred, ["pupils"]);
    assert_eq!(contacts.profile.source.path, "contacts.csv");
    let Some((_, OwnedData::Vector(ds))) = back.owned.iter().find(|(id, _)| *id == s.people) else { panic!("contacts' data") };
    assert!(ds.fields.iter().all(|f| f.name != "phone"));
    assert!(back.owned.iter().any(|(id, _)| *id == made));

    // Opening it again reuses the unpacked files; saving it as a package again works from them.
    let again = load_lmap(&path).unwrap();
    assert_eq!(again.doc.layer(s.styled).unwrap().profile.source.path, schools.profile.source.path);
    let path2 = out.0.join("again.lmap");
    save_package(&back.doc, &back.owned, &PackageOptions::default(), &path2).unwrap();
    let (m2, _) = manifest_of(&path2);
    let raw2 = serde_json::to_string(&m2).unwrap();
    assert!(!raw2.contains(&unpack_dir().display().to_string()), "the unpack folder doesn't leak either");
    assert_eq!(load_lmap(&path2).unwrap().doc.layers.len(), original.layers.len());
}

/// A package is container 2: a container-1 reader refuses it; a plain map stays container 1.
#[test]
fn packages_are_versioned_and_plain_maps_are_not() {
    let s = scene("pkg-version");
    let out = TempDir::new("pkg-version-out");
    let plain = out.0.join("plain.lmap");
    save_lmap(&s.doc, &s.owned[..1], &plain).unwrap();
    assert_eq!(manifest_of(&plain).0["container"], 1);
    assert!(!crate::is_package(&plain));
    let pkg = out.0.join("pkg.lmap");
    save_package(&s.doc, &s.owned[..1], &PackageOptions::default(), &pkg).unwrap();
    assert!(crate::is_package(&pkg));
    assert!(!load_lmap(&plain).unwrap().package);
    // A container this build doesn't know is refused.
    let mut m = manifest_of(&pkg).0;
    m["container"] = serde_json::json!(3);
    let err = crate::container::tests_support::parse(&serde_json::to_vec(&m).unwrap());
    assert!(matches!(err, Err(DocError::TooNew { found: 3, .. })), "{err:?}");
}

/// A source that can't be read when packaging travels as a reference only, and is reported.
#[test]
fn a_missing_source_is_reported() {
    let s = scene("pkg-missing");
    std::fs::remove_file(&s.csv).unwrap();
    let out = TempDir::new("pkg-missing-out");
    let report = save_package(&s.doc, &s.owned[..1], &PackageOptions::default(), &out.0.join("m.lmap")).unwrap();
    assert_eq!(report.missing, ["schools.csv"]);
}
