use std::path::{Path, PathBuf};

use lm_core::document::{DocLayer, Style};
use lm_core::model::LayerId;
use lm_core::recipe::Answer;

use super::*;
use crate::import::{Ctx, import_path};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn input(rel: &str, id: u64) -> PlanInput {
    let path = fixtures().join(rel);
    let mut layers = import_path(&path, &Ctx::detached(Default::default())).unwrap_or_default();
    assert!(!layers.is_empty(), "{rel} did not open");
    let l = layers.remove(0);
    let style = Style::for_profile(&l.profile, [0, 0, 0, 255]);
    PlanInput { layer: DocLayer::new(LayerId(id), l.name.clone(), l.profile.clone(), style), data: l.data, rows: None, path: Some(path) }
}

fn plan(action: &str, inputs: Vec<PlanInput>, params: &[(&str, &str)]) -> Plan {
    Plan {
        action: action.into(),
        inputs,
        params: params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        method: "test method".into(),
        computed_at: 1_797_206_400,
        software: "LayeredMap 0.4.0".into(),
    }
}

fn csv(rel: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(fixtures().join(rel)).unwrap_or_default();
    text.lines().skip(1).map(|l| l.split(',').map(str::to_string).collect()).collect()
}

fn new_layer(r: &ActionResult) -> &ImportedLayer {
    match &r.output {
        ActionOutput::NewLayer { layer, .. } => layer,
        _ => panic!("expected a new layer"),
    }
}

fn int_column(ds: &Dataset, key: &str, value: &str) -> Vec<(String, i64)> {
    let (k, v) = (ds.field_index(key).unwrap_or(0), ds.field_index(value).unwrap_or(0));
    ds.features.iter().map(|f| (f.attrs[k].to_string(), f.attrs[v].as_f64().unwrap_or(-1.0) as i64)).collect()
}

/// N1: the counts match GeoPandas exactly, the overlap is reported, and the recipe says so.
#[test]
fn n1_counts_match_the_reference_and_explain_the_overlap() {
    let r = run(&plan("count_in", vec![input("N1/points.geojson", 1), input("N1/areas.geojson", 2)], &[]), &Control::none()).unwrap();
    let LayerData::Vector(ds) = &new_layer(&r).data else { panic!("vector") };
    let got = int_column(ds, "area_id", "points");
    let want: Vec<(String, i64)> =
        csv("N1/expected_count_in_areas.csv").into_iter().map(|r| (r[0].clone(), r[1].parse().unwrap_or(-1))).collect();
    assert_eq!(got, want);
    assert!(r.card.notices.iter().any(|n| n.contains("more than one")), "{:?}", r.card.notices);
    assert!(r.recipe.text().contains("Decision: overlapping areas each count their points"), "{}", r.recipe.text());
    assert_eq!(r.card.title, "Points in each area");
    let ActionOutput::NewLayer { color_by, .. } = &r.output else { panic!() };
    assert_eq!(color_by.as_deref(), Some("points"));
    match &r.card.answer {
        Answer::Ranking { rows, .. } => {
            assert_eq!(rows.len(), 4);
            assert_eq!(rows[0].value, Some(10.0));
            assert_eq!(rows[3].value, Some(0.0));
        }
        other => panic!("{other:?}"),
    }
}

/// T1 schools in V1 districts: all 17 rows as GeoPandas counts them.
#[test]
fn t1_schools_in_v1_districts() {
    let r = run(&plan("count_in", vec![input("T1/schools.csv", 1), input("V1/districts.geojson", 2)], &[]), &Control::none()).unwrap();
    let LayerData::Vector(ds) = &new_layer(&r).data else { panic!("vector") };
    let want = csv("T1/expected_count_in_areas.csv");
    assert_eq!(ds.features.len(), want.len());
    let id_field = ds.fields.iter().position(|f| {
        ds.features.iter().zip(&want).all(|(ft, w)| {
            ft.attrs.get(ds.fields.iter().position(|g| g.name == f.name).unwrap_or(0)).is_some_and(|v| v.to_string() == w[0])
        })
    });
    let k_id = id_field.expect("an id field matching the reference");
    let k = ds.field_index("schools").expect("count field");
    for (f, w) in ds.features.iter().zip(&want) {
        assert_eq!(f.attrs[k_id].to_string(), w[0]);
        assert_eq!(f.attrs[k].as_f64(), w[1].parse::<f64>().ok(), "{}", w[0]);
    }
    // The excluded schools (no usable location) are in the recipe.
    assert!(r.recipe.excluded.iter().any(|e| e.contains("no usable location")), "{:?}", r.recipe.excluded);
}

/// A point on a shared boundary lists both areas.
#[test]
fn which_area_lists_both_sides_of_a_boundary() {
    let mut pts = input("N1/points.geojson", 1);
    let LayerData::Vector(ds) = &pts.data else { panic!() };
    let k = ds.field_index("point_id").unwrap_or(0);
    let row = ds.features.iter().position(|f| f.attrs[k].to_string() == "P04").expect("P04");
    pts.rows = Some(vec![row]);
    let r = run(&plan("which_container", vec![pts, input("N1/areas.geojson", 2)], &[]), &Control::none()).unwrap();
    match &r.card.answer {
        Answer::List { heading, items } => {
            assert_eq!(heading, "In 2 areas");
            assert_eq!(items.len(), 2);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(r.highlight.as_ref().map(|h| h.1.len()), Some(2));
}

/// A 1 km buffer of one point is a geodesic circle: π km² within 0.1%.
#[test]
fn a_point_buffer_has_the_area_of_its_circle() {
    let mut pts = input("N1/points.geojson", 1);
    pts.rows = Some(vec![0]);
    let r = run(&plan("buffer", vec![pts], &[("distance", "1 km")]), &Control::none()).unwrap();
    let Answer::Number { value, unit, .. } = &r.card.answer else { panic!() };
    assert_eq!(unit, "km²");
    assert!((value / std::f64::consts::PI - 1.0).abs() < 1e-3, "{value}");
    assert!(r.card.title.starts_with("Within 1 km of "), "{}", r.card.title);
    assert!(r.recipe.text().contains("straight-line"));
}

/// R6: the values added to each point are rasterio's, and a blank stays blank.
#[test]
fn r6_values_added_to_points_match_rasterio() {
    let r = run(&plan("add_raster_values", vec![input("R6/scaled.tif", 1), input("R6/points.csv", 2)], &[]), &Control::none()).unwrap();
    let ActionOutput::NewFields { data, fields, .. } = &r.output else { panic!("new fields") };
    let k = data.field_index(&fields[0]).expect("field");
    let id = data.field_index("point_id").expect("point_id");
    for w in csv("R6/expected_raster_sample.csv") {
        let f = data.features.iter().find(|f| f.attrs[id].to_string() == w[0]).expect("point");
        match w[4].parse::<f64>() {
            Ok(v) => assert!((f.attrs[k].as_f64().unwrap_or(f64::NAN) - v).abs() < 1e-9, "{}", w[0]),
            Err(_) => assert!(f.attrs[k].as_f64().is_none(), "{} should be blank", w[0]),
        }
    }
}

#[test]
fn areas_are_measured_on_the_ellipsoid_and_ranked() {
    let mut areas = input("N1/areas.geojson", 2);
    areas.rows = Some(vec![0]);
    let r = run(&plan("measure_area", vec![areas], &[]), &Control::none()).unwrap();
    let Answer::Number { value, unit, detail } = &r.card.answer else { panic!() };
    assert_eq!(unit, "km²");
    assert!(*value > 1.0 && detail.starts_with("perimeter"), "{value} {detail}");

    let r = run(&plan("rank_by_value", vec![input("V1/districts.geojson", 3)], &[("field", "population")]), &Control::none()).unwrap();
    let Answer::Ranking { rows, .. } = &r.card.answer else { panic!() };
    assert_eq!(rows.len(), 17);
    assert!(rows.windows(2).all(|w| w[0].value >= w[1].value));
}

#[test]
fn a_cancelled_run_returns_nothing() {
    let ctl = Control::none();
    ctl.cancel();
    let r = run(&plan("count_in", vec![input("N1/points.geojson", 1), input("N1/areas.geojson", 2)], &[]), &ctl);
    assert!(matches!(r, Err(ActionError::Cancelled)));
}

#[test]
fn wrong_inputs_say_what_would_work() {
    let r = run(&plan("measure_area", vec![input("N1/points.geojson", 1)], &[]), &Control::none());
    assert!(matches!(r, Err(ActionError::Input(m)) if m.contains("area")));
    let r = run(&plan("buffer", vec![input("N1/points.geojson", 1)], &[("distance", "far")]), &Control::none());
    assert!(matches!(r, Err(ActionError::Input(m)) if m.contains("distance")));
}

#[test]
fn words() {
    assert_eq!(singular("districts"), "district");
    assert_eq!(singular("cities"), "city");
    assert_eq!(singular("Area 1"), "Area 1");
    assert_eq!(plural("school"), "schools");
    assert_eq!(plural("schools"), "schools");
    assert_eq!(plural("city"), "cities");
    assert_eq!(parse_distance("500 m").map(|d| d.0), Some(500.0));
    assert_eq!(parse_distance("1.5km").map(|d| d.1), Some("1.5 km".to_string()));
    assert_eq!(parse_distance("-1 km"), None);
    assert_eq!(number(1234567.0), "1,234,567");
    assert_eq!(number(1.23456), "1.23");
    assert_eq!(number(0.123456), "0.123");
    assert_eq!(number(21.26), "21.3");
}
