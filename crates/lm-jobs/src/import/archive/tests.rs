use super::*;
use crate::import::{LayerData, Ticket, fresh_read};

fn ctx() -> Ctx {
    Ctx::detached(Ticket::default())
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// A fresh, empty temporary folder for one test.
fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lm-archive-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A gzip file (RFC 1952) of `data` with the optional header fields `flags` asks for.
fn gzip(data: &[u8], flags: u8) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, flags, 0, 0, 0, 0, 0, 3];
    if flags & 0x04 != 0 {
        out.extend_from_slice(&[4, 0, b'a', b'b', 2, 0]);
    }
    if flags & 0x08 != 0 {
        out.extend_from_slice(b"inner.csv\0");
    }
    if flags & 0x10 != 0 {
        out.extend_from_slice(b"a comment\0");
    }
    if flags & 0x02 != 0 {
        out.extend_from_slice(&[0x12, 0x34]);
    }
    out.extend(miniz_oxide::deflate::compress_to_vec(data, 6));
    out.extend_from_slice(&0u32.to_le_bytes()); // CRC-32 (not checked)
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

fn zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let e: Vec<(String, Vec<u8>)> = entries.iter().map(|(n, b)| (n.to_string(), b.clone())).collect();
    rustgis_engine::data::zip::write(&e).unwrap()
}

fn write_zip(dir: &Path, name: &str, entries: &[(&str, Vec<u8>)]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, zip(entries)).unwrap();
    p
}

const POINTS: &str = "name,lat,lon\nA,35.6,139.7\nB,34.7,135.5\nC,43.1,141.3\n";
const GEOJSON: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"t1"},"geometry":{"type":"Point","coordinates":[-100.5,40.1]}},{"type":"Feature","properties":{"name":"t2"},"geometry":{"type":"Point","coordinates":[-101.5,41.1]}}]}"#;

#[test]
fn gzip_headers_with_every_optional_field_are_unpacked() {
    let data = POINTS.as_bytes();
    for flags in [0u8, 0x02, 0x04, 0x08, 0x10, 0x1e] {
        assert_eq!(gunzip(&gzip(data, flags), "x.gz", 1 << 20).unwrap(), data, "flags {flags:#x}");
    }
}

#[test]
fn a_cut_short_or_oversized_gzip_is_refused_politely() {
    let gz = gzip(POINTS.repeat(50).as_bytes(), 0x08);
    for cut in [5, 15, gz.len() / 2, gz.len() - 3] {
        let e = gunzip(&gz[..cut], "x.csv.gz", 1 << 20).unwrap_err().to_string();
        assert!(e.contains("damaged or cut short — try downloading it again"), "{cut}: {e}");
    }
    let e = gunzip(&gz, "x.csv.gz", 100).unwrap_err().to_string();
    assert!(e.contains("more than 2 GB"), "{e}");
    assert_eq!(inner_name("gistemp.nc.gz"), "gistemp.nc");
    assert_eq!(inner_name("a.CSV.GZ"), "a.CSV");
    assert_eq!(inner_name("b.tgz"), "b.tar");
}

#[test]
fn a_gz_is_unpacked_once_to_a_stable_name_and_read_as_its_contents() {
    let dir = temp("gz");
    let gz = dir.join("places.csv.gz");
    let bytes = gzip(POINTS.as_bytes(), 0x08);
    std::fs::write(&gz, &bytes).unwrap();
    let cache = dir.join("Unpacked");
    let a = unpack_gz(&gz, &cache).unwrap();
    assert_eq!(a, cache.join(format!("{}-places.csv", hash12(&bytes))));
    assert_eq!(a.file_name().unwrap().to_str().unwrap().split('-').next().unwrap().len(), 12);
    assert_eq!(std::fs::read(&a).unwrap(), POINTS.as_bytes());
    // An existing copy is reused, not written again.
    std::fs::write(&a, "name,lat,lon\nZ,1,2\n").unwrap();
    assert_eq!(unpack_gz(&gz, &cache).unwrap(), a);
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "name,lat,lon\nZ,1,2\n");
    std::fs::write(&a, POINTS).unwrap();
    let layers = import_path(&a, &ctx()).unwrap();
    assert_eq!((layers[0].profile.kind, layers[0].profile.count), (LayerKind::Points, 3));
    // A gzip inside a gzip is refused.
    let twice = dir.join("twice.csv.gz.gz");
    std::fs::write(&twice, gzip(&bytes, 0)).unwrap();
    assert!(unpack_gz(&twice, &cache).unwrap_err().to_string().contains("more than once"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn import_path_opens_a_gz_with_a_notice() {
    let dir = temp("gz-import");
    let gz = dir.join("cities.csv.gz");
    std::fs::write(&gz, gzip(format!("{POINTS}D,26.2,127.7\n").as_bytes(), 0x08)).unwrap();
    let l = import_path(&gz, &ctx()).unwrap().remove(0);
    assert_eq!((l.profile.kind, l.profile.count, l.name.as_str()), (LayerKind::Points, 4, "cities"));
    assert_eq!(l.profile.notices.first().map(String::as_str), Some("Unpacked from cities.csv.gz."));
    assert!(l.profile.source.path.contains("Unpacked"), "{}", l.profile.source.path);
    assert!(Path::new(&l.profile.source.path).is_file(), "the unpacked file stays for later reads");
    let damaged = dir.join("cut.csv.gz");
    std::fs::write(&damaged, &gzip(POINTS.as_bytes(), 0)[..20]).unwrap();
    let e = import_path(&damaged, &ctx()).err().unwrap().to_string();
    assert!(e.contains("damaged or cut short"), "{e}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn member_rules_skip_paperwork_and_keep_one_copy_of_each_layer() {
    for (n, k) in [
        ("a/b.geojson", Some(MemberKind::Vector)),
        ("x.CSV", Some(MemberKind::Vector)),
        ("x.tsv", Some(MemberKind::Vector)),
        ("x.xls", Some(MemberKind::Vector)),
        ("x.gpkg", Some(MemberKind::Vector)),
        ("x.kml", Some(MemberKind::Vector)),
        ("x.gpx", Some(MemberKind::Vector)),
        ("x.tif", Some(MemberKind::File)),
        ("x.TIFF", Some(MemberKind::File)),
        ("x.nc", Some(MemberKind::File)),
        ("x.json", Some(MemberKind::MaybeJson)),
        ("folder/", None),
        ("__MACOSX/x.csv", None),
        ("a/._x.csv", None),
        ("CHANGELOG.txt", None),
        ("meta.xml", None),
        ("doc.pdf", None),
        ("index.html", None),
        ("index.htm", None),
        ("README.md", None),
        ("notes.rtf", None),
        ("notes.docx", None),
        ("map.png", None),
        ("photo.jpg", None),
        ("LICENSE.csv", None),
        ("readme.json", None),
    ] {
        assert_eq!(member_kind(n), k, "{n}");
    }
    assert!(json_is_geo(GEOJSON.as_bytes()));
    assert!(json_is_geo(br#"{ "type" : "Topology", "objects": {}}"#));
    assert!(!json_is_geo(br#"{"name": "settings", "values": [1, 2]}"#));
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    // (a) One file name in two folders: the "utf" folder wins, else the first.
    assert_eq!(dedupe_same_name(&s(&["Shift-JIS/N02.shp", "UTF-8/N02.shp", "Shift-JIS/S.shp"])), s(&["UTF-8/N02.shp", "Shift-JIS/S.shp"]));
    assert_eq!(dedupe_same_name(&s(&["a/x.csv", "b/x.csv"])), s(&["a/x.csv"]));
    assert_eq!(dedupe_same_name(&s(&["a/x.csv", "utf8/X.CSV", "c/x.csv"])), s(&["utf8/X.CSV"]));
    // (b) One stem in several formats: geojson > gpkg > shp > csv/tsv > xlsx/xls > kml > gpx.
    assert_eq!(dedupe_same_stem(&s(&["worldcities.csv", "worldcities.xlsx"])), s(&["worldcities.csv"]));
    assert_eq!(dedupe_same_stem(&s(&["UTF-8/S.geojson", "UTF-8/S.shp"])), s(&["UTF-8/S.geojson"]));
    assert_eq!(dedupe_same_stem(&s(&["t.gpx", "t.kml", "t.xls"])), s(&["t.xls"]));
    assert_eq!(dedupe_same_stem(&s(&["a.gpkg", "a.shp", "b.csv"])), s(&["a.gpkg", "b.csv"]));
    // Rasters and grids aren't the same layer as a table of one stem.
    assert_eq!(dedupe_same_stem(&s(&["dem.csv", "dem.tif"])), s(&["dem.csv", "dem.tif"]));
}

#[test]
fn a_zip_of_geojson_and_metadata_opens_the_geojson() {
    let dir = temp("zip-geojson");
    let z = write_zip(
        &dir,
        "uswtdb.zip",
        &[
            ("uswtdb.geojson", GEOJSON.as_bytes().to_vec()),
            ("uswtdb.xml", b"<meta/>".to_vec()),
            ("CHANGELOG.txt", b"v9".to_vec()),
            ("__MACOSX/._uswtdb.geojson", b"fork".to_vec()),
        ],
    );
    let layers = import_path(&z, &ctx()).unwrap();
    assert_eq!(layers.len(), 1);
    let l = &layers[0];
    assert_eq!((l.name.as_str(), l.profile.kind, l.profile.count), ("uswtdb", LayerKind::Points, 2));
    assert_eq!(l.profile.format, "GeoJSON");
    assert_eq!(l.profile.source.member.as_deref(), Some("uswtdb.geojson"));
    assert_eq!(Path::new(&l.profile.source.path), std::fs::canonicalize(&z).unwrap());
    assert_eq!(l.profile.source.size, std::fs::metadata(&z).unwrap().len());
    // The layer can be read again from its source (reopen, relocate).
    let again = fresh_read(&l.profile.source).unwrap();
    assert_eq!(again.profile.source, l.profile.source);
    assert_eq!(again.profile.count, 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_zip_with_one_layer_in_two_formats_opens_it_once() {
    let dir = temp("zip-worldcities");
    let xlsx = std::fs::read(fixtures().join("T5/survey.xlsx")).unwrap();
    let z = write_zip(
        &dir,
        "worldcities.zip",
        &[("license.txt", b"CC".to_vec()), ("worldcities.csv", POINTS.as_bytes().to_vec()), ("worldcities.xlsx", xlsx.clone())],
    );
    let layers = import_path(&z, &ctx()).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!((layers[0].profile.format.as_str(), layers[0].profile.count), ("CSV", 3));
    assert_eq!(layers[0].profile.source.member.as_deref(), Some("worldcities.csv"));
    // A CSV in a zip can have its columns changed (no file is read again).
    let t = crate::import::relocate_csv(&layers[0], &lm_core::model::Choice::not_sure()).unwrap();
    assert_eq!((t.profile.kind, t.profile.source.member.as_deref()), (LayerKind::Table, Some("worldcities.csv")));

    // A workbook in a zip: its sheet is part of the member, and another sheet can be chosen.
    let z = write_zip(&dir, "survey.zip", &[("data/survey.xlsx", xlsx)]);
    let l = import_path(&z, &ctx()).unwrap().remove(0);
    assert_eq!(l.profile.source.member.as_deref(), Some("data/survey.xlsx › Schools"));
    let s = crate::import::choose_sheet(&l, &lm_core::model::Choice::new("sheet:Summary", "Summary")).unwrap();
    assert_eq!(s.profile.source.member.as_deref(), Some("data/survey.xlsx › Summary"));
    assert_eq!(s.profile.kind, LayerKind::Table);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tables_are_left_out_when_something_has_a_location() {
    let dir = temp("zip-tables");
    let table = |t: &str| format!("id,{t}\n1,a\n2,b\n").into_bytes();
    let z = write_zip(
        &dir,
        "pleiades.zip",
        &[
            ("data/gis/places.csv", POINTS.as_bytes().to_vec()),
            ("data/gis/association_certainty.csv", table("certainty")),
            ("data/gis/name_types.csv", table("type")),
            ("data/gis/README.md", b"# read me".to_vec()),
            ("data/gis/index.html", b"<html/>".to_vec()),
        ],
    );
    let layers = import_path(&z, &ctx()).unwrap();
    assert_eq!(layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["places"]);
    assert!(
        layers[0]
            .profile
            .notices
            .contains(&"Also in the zip: 2 tables without locations (association_certainty.csv, name_types.csv).".to_string()),
        "{:?}",
        layers[0].profile.notices
    );
    // Only tables: they are what's opened.
    let z = write_zip(&dir, "codes.zip", &[("a.csv", table("x")), ("b.tsv", b"id\tv\n1\t2\n".to_vec())]);
    let layers = import_path(&z, &ctx()).unwrap();
    assert_eq!(layers.len(), 2);
    assert!(layers.iter().all(|l| l.profile.kind == LayerKind::Table));
    // A zip with nothing to open says what it holds.
    let z = write_zip(&dir, "docs.zip", &[("a.txt", b"x".to_vec()), ("b.pdf", b"%PDF".to_vec())]);
    let e = import_path(&z, &ctx()).err().unwrap().to_string();
    assert_eq!(e, "docs.zip holds no files NimbleMap opens (it holds a.txt, b.pdf).");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn at_most_thirty_layers_open_from_one_zip() {
    let dir = temp("zip-many");
    let names: Vec<String> = (0..33).map(|i| format!("p{i:02}.csv")).collect();
    let entries: Vec<(&str, Vec<u8>)> = names.iter().map(|n| (n.as_str(), POINTS.as_bytes().to_vec())).collect();
    let z = write_zip(&dir, "many.zip", &entries);
    let layers = import_path(&z, &ctx()).unwrap();
    assert_eq!(layers.len(), ZIP_MAX_LAYERS);
    assert_eq!(layers[0].name, "p00");
    assert!(
        layers[0]
            .profile
            .notices
            .iter()
            .any(|n| n == "3 more layers in the zip were left out: NimbleMap opens at most 30 layers from one zip.")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_raster_in_a_zip_is_kept_in_the_cache_and_opens() {
    let dir = temp("zip-tif");
    let tif = std::fs::read(fixtures().join("R6/scaled.tif")).unwrap();
    let z = write_zip(&dir, "scaled.zip", &[("rasters/scaled.tif", tif.clone()), ("rasters/scaled.tif.aux.xml", b"<x/>".to_vec())]);
    let layers = import_path(&z, &ctx()).unwrap();
    assert_eq!(layers.len(), 1);
    let l = &layers[0];
    assert_eq!(l.profile.kind, LayerKind::Raster);
    assert!(matches!(l.data, LayerData::Raster(_)));
    assert_eq!(std::fs::read(&l.profile.source.path).unwrap(), tif, "the raster's own file, read again later");
    assert_eq!(l.profile.notices.first().map(String::as_str), Some("Unpacked from scaled.zip (rasters/scaled.tif)."));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_shapefile_stored_in_two_encodings_opens_once_from_the_utf8_folder() {
    let dir = temp("zip-japan");
    let part = |e: &str| std::fs::read(fixtures().join(format!("V3/sites.{e}"))).unwrap();
    let mut entries = Vec::new();
    for folder in ["Shift-JIS", "UTF-8"] {
        for e in ["shp", "shx", "dbf", "cpg"] {
            entries.push((format!("{folder}/N02_Station.{e}"), part(e)));
        }
    }
    entries.push(("UTF-8/N02_Station.geojson".into(), GEOJSON.as_bytes().to_vec()));
    entries.push(("KS-META.xml".into(), b"<x/>".to_vec()));
    let refs: Vec<(&str, Vec<u8>)> = entries.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    let z = write_zip(&dir, "N02.zip", &refs);
    let layers = import_path(&z, &ctx()).unwrap();
    assert_eq!(layers.len(), 1, "{:?}", layers.iter().map(|l| l.profile.source.member.clone()).collect::<Vec<_>>());
    assert_eq!(layers[0].profile.source.member.as_deref(), Some("UTF-8/N02_Station.shp"));
    assert_eq!(layers[0].profile.format, "Zipped Shapefile");
    let _ = std::fs::remove_dir_all(dir);
}
