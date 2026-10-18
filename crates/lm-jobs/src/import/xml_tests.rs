//! KML, KMZ and GPX (WP6.7) against fixtures V11 and V12: layers, fields, styles and exclusions
//! as LayeredMap defines them, and features, vertices and bounds against GDAL's reading recorded
//! in each `expected.json` (`reference/make_kml_gpx_fixtures.py`).

use lm_core::model::{Crs, ImportStatus, LayerKind};
use rustgis_engine::data::Value;
use serde_json::Value as Json;

use super::tests::{fixtures, sha};
use super::*;

fn open(rel: &str) -> Result<Vec<ImportedLayer>, ImportError> {
    import_path(&fixtures().join(rel), &Ctx::detached(Ticket::default()))
}

fn expected(id: &str) -> Json {
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

fn ds(l: &ImportedLayer) -> &Dataset {
    match &l.data {
        LayerData::Vector(d) => d,
        LayerData::Raster(_) => panic!("{} is a raster", l.name),
    }
}

fn values(l: &ImportedLayer, field: &str) -> Vec<Value> {
    let d = ds(l);
    let i = d.field_index(field).unwrap_or_else(|| panic!("{}: no field {field}", l.name));
    d.features.iter().map(|f| f.attrs[i].clone()).collect()
}

/// Vertices, bounds and the sums of the longitudes and latitudes of a set of layers' located
/// features.
fn vertices_and_bounds(layers: &[&ImportedLayer]) -> (u64, [f64; 4], [f64; 2]) {
    let mut n = 0;
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let mut sum = [0.0, 0.0];
    for l in layers {
        for g in ds(l).features.iter().filter_map(|f| f.geometry.as_ref()) {
            g.for_each_coord(|c| {
                n += 1;
                b = [b[0].min(c.x), b[1].min(c.y), b[2].max(c.x), b[3].max(c.y)];
                sum = [sum[0] + c.x, sum[1] + c.y];
            });
        }
    }
    (n, b, sum)
}

/// Each expected layer (name, kind, count, style), and per GDAL layer the vertices, bounds,
/// coordinate sums and (where LayeredMap didn't split it) the feature count of the LayeredMap
/// layers of that group.
fn check_against_gdal(id: &str, file: &str, layers: &[ImportedLayer]) {
    let exp = expected(id);
    let ours = exp["layeredmap"][file]["layers"].as_array().unwrap();
    assert_eq!(layers.len(), ours.len(), "{file}: {:?}", layers.iter().map(|l| &l.name).collect::<Vec<_>>());
    let mut groups: Vec<(&str, Vec<&ImportedLayer>)> = Vec::new();
    for e in ours {
        let name = e["name"].as_str().unwrap();
        let l = layers.iter().find(|l| l.name == name).unwrap_or_else(|| panic!("{file}: no layer {name}"));
        assert_eq!(kind_word(l.profile.kind), e["geometry"].as_str().unwrap(), "{name}");
        assert_eq!(l.profile.count, e["count"].as_u64().unwrap(), "{name}");
        assert_eq!(l.profile.exclusions.len() as u64, e["excluded"].as_u64().unwrap_or(0), "{name}: {:?}", l.profile.exclusions);
        assert_eq!(l.profile.crs.value, Crs::Wgs84, "{name}");
        assert_eq!(l.profile.crs.confidence, lm_core::model::Confidence::Certain, "{name}: declared by the format");
        match (&e["style"], &l.style) {
            (Json::Null, None) => {}
            (Json::Null, Some(s)) => panic!("{name}: unexpected style {s:?}"),
            (want, got) => {
                let s = got.as_ref().unwrap_or_else(|| panic!("{name}: no style"));
                let rgba = |v: &Json| -> [u8; 4] { serde_json::from_value(v.clone()).unwrap() };
                if !want["fill"].is_null() {
                    assert_eq!(s.fill, rgba(&want["fill"]), "{name} fill");
                }
                if !want["stroke"].is_null() {
                    assert_eq!(s.stroke, rgba(&want["stroke"]), "{name} stroke");
                }
                for (key, got) in [("line_width", s.line_width), ("stroke_width", s.stroke_width), ("fill_opacity", s.fill_opacity)] {
                    if let Some(w) = want[key].as_f64() {
                        assert!((f64::from(got) - w).abs() < 1e-6, "{name} {key}: {got} vs {w}");
                    }
                }
            }
        }
        if let Some(fields) = e["fields"].as_array() {
            let have: Vec<&str> = ds(l).fields.iter().map(|f| f.name.as_str()).collect();
            for f in fields {
                assert!(have.contains(&f.as_str().unwrap()), "{name}: field {f} not in {have:?}");
            }
        }
        let group = e["group"].as_str().unwrap();
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, v)) => v.push(l),
            None => groups.push((group, vec![l])),
        }
    }
    let gdal: Vec<&Json> = exp["gdal"][file].as_array().unwrap().iter().filter(|g| g["count"].as_u64().unwrap() > 0).collect();
    assert_eq!(gdal.len(), groups.len(), "{file}: one group per GDAL layer with features");
    for g in gdal {
        let name = g["layer"].as_str().unwrap();
        let (_, mine) = groups.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{file}: GDAL layer {name} has no match"));
        let (n, b, sum) = vertices_and_bounds(mine);
        let want_sum: Vec<f64> = g["sum_xy"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        for (a, w) in sum.iter().zip(&want_sum) {
            assert!((a - w).abs() < 1e-6, "{file} {name}: coordinate sums {sum:?} vs {want_sum:?}");
        }
        assert_eq!(n, g["vertices"].as_u64().unwrap(), "{file} {name}: vertices");
        let want: Vec<f64> = g["bounds"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        for (a, w) in b.iter().zip(&want) {
            assert!((a - w).abs() < 1e-9, "{file} {name}: bounds {b:?} vs {want:?}");
        }
        if mine.len() == 1 {
            assert_eq!(mine[0].profile.count, g["count"].as_u64().unwrap(), "{file} {name}: features");
        }
    }
}

#[test]
fn v11_kml_opens_one_layer_per_folder_with_its_styles() {
    let layers = open("V11/places.kml").unwrap();
    check_against_gdal("V11", "places.kml", &layers);
    for l in &layers {
        assert_eq!(l.profile.format, "KML");
        // Each layer of the file can be found again by its member.
        assert_eq!(l.profile.source.member.as_deref(), Some(l.name.as_str()));
    }
    let by = |n: &str| layers.iter().find(|l| l.name == n).unwrap();
    // Fields from name, description (CDATA), ExtendedData/Data and SchemaData.
    let schools = by("Schools");
    assert_eq!(values(schools, "pupils")[..3], [Value::Int(420), Value::Int(385), Value::Int(512)]);
    assert_eq!(values(schools, "district")[0], Value::Text("Jing'an".into()));
    assert_eq!(values(schools, "description")[0], Value::Text("<b>Open</b> 8:00-16:00".into()));
    assert_eq!(schools.profile.exclusions[0].reason, "No shape");
    assert_eq!(schools.status, ImportStatus::CompleteWithExclusions);
    // Time spans become fields.
    assert_eq!(values(by("Rivers"), "begin")[0], Value::Text("2024-05-01".into()));
    // The hole stays a hole; the MultiGeometry of two areas is one feature of two parts.
    let parks = ds(by("Parks"));
    let parts = |i: usize| match parks.features[i].geometry.as_ref().unwrap() {
        rustgis_engine::data::geometry::Geometry::Polygon(p) => p.iter().map(Vec::len).collect::<Vec<_>>(),
        g => panic!("{g:?}"),
    };
    assert_eq!(parts(1), [2], "outer ring and one hole");
    assert_eq!(parts(2), [1, 1], "two areas");
    // Lines that differ in style keep each style in a field, with a notice.
    let survey = by("Survey (lines)");
    assert_eq!(values(survey, "style"), [Value::Text("riverLine".into()), Value::Text("line #FF0000".into())]);
    assert!(survey.profile.notices.iter().any(|n| n.contains("2 different styles")), "{:?}", survey.profile.notices);
    // The network link and the ground overlay are named once, on the first layer.
    let first = &layers[0].profile.notices;
    assert!(first.iter().any(|n| n.contains("network link") && n.contains("ground overlay")), "{first:?}");
    assert!(layers[1..].iter().all(|l| !l.profile.notices.iter().any(|n| n.contains("network link"))));
}

#[test]
fn v11_kmz_reads_its_doc_kml() {
    let layers = open("V11/trail.kmz").unwrap();
    check_against_gdal("V11", "trail.kmz", &layers);
    assert!(layers.iter().all(|l| l.profile.format == "KMZ"));
    let track = layers.iter().find(|l| l.profile.kind == LayerKind::Lines).unwrap();
    assert_eq!(values(track, "name")[0], Value::Text("Morning walk".into()));
}

#[test]
fn v12_gpx_opens_waypoints_tracks_and_routes() {
    let layers = open("V12/hike.gpx").unwrap();
    check_against_gdal("V12", "hike.gpx", &layers);
    let exp = expected("V12");
    let first = &exp["layeredmap"]["hike.gpx"]["layers"][1]["first"];
    let tracks = layers.iter().find(|l| l.name == "hike (tracks)").unwrap();
    for key in ["start_time", "end_time"] {
        assert_eq!(values(tracks, key)[0], Value::Text(first[key].as_str().unwrap().into()), "{key}");
    }
    for key in ["min_ele", "max_ele"] {
        assert_eq!(values(tracks, key)[0].as_f64(), first[key].as_f64(), "{key}");
    }
    // One feature per track: its two segments are the parts of one line.
    match ds(tracks).features[0].geometry.as_ref().unwrap() {
        rustgis_engine::data::geometry::Geometry::Line(parts) => assert_eq!(parts.len() as u64, first["parts"].as_u64().unwrap()),
        g => panic!("{g:?}"),
    }
    // The second track has no times: empty, not invented.
    assert_eq!(values(tracks, "start_time")[1], Value::Null);
    let waypoints = layers.iter().find(|l| l.name == "hike (waypoints)").unwrap();
    assert_eq!(values(waypoints, "ele")[..2], [Value::Float(1034.5), Value::Float(1620.0)]);
    assert_eq!(values(waypoints, "time")[2], Value::Null);
    for l in &layers {
        assert_eq!(l.profile.format, "GPX");
        assert_eq!(l.status, ImportStatus::Complete, "{}: {:?}", l.name, l.profile.assumptions);
        assert!(l.style.is_none());
    }
}

#[test]
fn v12_old_gpx_is_read_in_its_declared_encoding() {
    let layers = open("V12/old_garmin.gpx").unwrap();
    check_against_gdal("V12", "old_garmin.gpx", &layers);
    let names: Vec<Value> = expected("V12")["layeredmap"]["old_garmin.gpx"]["layers"][0]["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| Value::Text(n.as_str().unwrap().into()))
        .collect();
    assert_eq!(values(&layers[0], "name"), names);
    assert!(layers[0].profile.notices.is_empty(), "{:?}", layers[0].profile.notices);
}

#[test]
fn malformed_kml_and_gpx_fail_plainly_or_list_what_was_left_out() {
    let err = |f: &str| open(f).err().unwrap_or_else(|| panic!("{f} opened")).to_string();
    assert!(err("V11/malformed/truncated.kml").contains("isn't valid KML (a problem on line"), "{}", err("V11/malformed/truncated.kml"));
    assert!(err("V11/malformed/not_kml.kml").contains("isn't a KML file"));
    let links = err("V11/malformed/only_links.kml");
    assert!(links.contains("holds no places") && links.contains("2 links to other files") && links.contains("ground overlay"), "{links}");
    assert!(err("V11/malformed/dtd.kml").contains("(a DTD)"));
    assert!(err("V11/malformed/no_kml.kmz").contains("holds no KML document"));
    assert!(err("V12/malformed/truncated.gpx").contains("isn't valid GPX"));

    let bad = open("V11/malformed/bad_coords.kml").unwrap();
    assert_eq!(bad.len(), 1, "{:?}", bad.iter().map(|l| &l.name).collect::<Vec<_>>());
    assert_eq!(bad[0].profile.count, 1);
    let reasons: Vec<&str> = bad[0].profile.exclusions.iter().map(|e| e.reason.as_str()).collect();
    assert_eq!(
        reasons,
        [
            "Coordinates that aren't numbers",
            "Coordinates outside longitude ±180 and latitude ±90",
            "An area with fewer than three corners",
            "A line with fewer than two points",
            "A 3D model, which LayeredMap doesn't show",
        ]
    );
    // Rows are the records' positions in the layer (the good point is row 1).
    assert_eq!(bad[0].profile.exclusions.iter().map(|e| e.row).collect::<Vec<_>>(), [2, 3, 4, 5, 6]);

    let gpx = open("V12/malformed/bad_points.gpx").unwrap();
    let by = |n: &str| gpx.iter().find(|l| l.name == n).unwrap_or_else(|| panic!("no {n}"));
    let w = by("bad_points (waypoints)");
    assert_eq!((w.profile.count, w.profile.exclusions.len()), (1, 2));
    assert_eq!(w.profile.exclusions[0].reason, "No position (lat and lon)");
    // The only route has one point: no routes layer, and the first layer says why.
    assert!(gpx.iter().all(|l| !l.name.contains("routes")));
    assert!(
        gpx[0].profile.notices.iter().any(|n| n == "1 route couldn't be drawn and was left out (fewer than two points with a position)."),
        "{:?}",
        gpx[0].profile.notices
    );
    let t = by("bad_points (tracks)");
    assert_eq!(t.profile.count, 1);
    assert_eq!(vertices_and_bounds(&[t]).0, 2);
    assert!(t.profile.notices.iter().any(|n| n.contains("1 point without a usable position was left out")), "{:?}", t.profile.notices);
}

#[test]
fn too_many_folders_become_one_layer_per_kind_with_a_folder_field() {
    let dir = std::env::temp_dir().join(format!("lm-kml-folders-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut kml = String::from("<kml><Document>");
    for i in 0..(kml::MAX_FOLDER_LAYERS + 5) {
        kml.push_str(&format!(
            "<Folder><name>F{i}</name><Placemark><name>p{i}</name><Point><coordinates>{i},1</coordinates></Point></Placemark></Folder>"
        ));
    }
    kml.push_str("</Document></kml>");
    let p = dir.join("many.kml");
    std::fs::write(&p, kml).unwrap();
    let layers = import_path(&p, &Ctx::detached(Ticket::default())).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].profile.count as usize, kml::MAX_FOLDER_LAYERS + 5);
    assert_eq!(values(&layers[0], "folder")[3], Value::Text("F3".into()));
    assert!(layers[0].profile.notices.iter().any(|n| n.contains("25 folders")), "{:?}", layers[0].profile.notices);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn kml_kmz_and_gpx_are_detected_and_left_unchanged() {
    for (f, adapter) in [("V11/places.kml", "KML"), ("V11/trail.kmz", "KMZ"), ("V12/hike.gpx", "GPX")] {
        let p = fixtures().join(f);
        let head = std::fs::read(&p).unwrap();
        assert_eq!(pick_adapter(&p, &head[..head.len().min(4096)], true).map(|a| a.name()).ok(), Some(adapter), "{f}");
        let before = sha(&p);
        open(f).unwrap();
        assert_eq!(before, sha(&p), "{f} changed");
    }
    // An .xml is taken by what it holds.
    assert_eq!(pick_adapter(std::path::Path::new("export.xml"), b"<?xml version=\"1.0\"?><gpx", true).map(|a| a.name()).ok(), Some("GPX"));
    assert_eq!(pick_adapter(std::path::Path::new("export.xml"), b"<kml xmlns=", true).map(|a| a.name()).ok(), Some("KML"));
}

#[test]
fn canceled_kml_kmz_and_gpx_imports_return_nothing() {
    for f in ["V11/places.kml", "V11/trail.kmz", "V12/hike.gpx"] {
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
fn a_kml_layer_is_found_again_by_its_member() {
    let layers = open("V11/places.kml").unwrap();
    let parks = layers.iter().find(|l| l.name == "Parks").unwrap();
    let again = fresh_read(&parks.profile.source).unwrap();
    assert_eq!(again.name, "Parks");
    assert_eq!(again.profile.count, parks.profile.count);
    assert_eq!(again.style, parks.style);
}
