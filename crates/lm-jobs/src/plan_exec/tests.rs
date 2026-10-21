use std::path::Path;

use lm_core::document::{DocLayer, Style};
use lm_core::model::LayerId;
use serde_json::json;

use super::*;
use crate::import::{Ctx, import_path};

fn layer(rel: &str, name: &str, id: u64) -> PlanLayer {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    let mut ls = import_path(&path, &Ctx::detached(Default::default())).unwrap_or_default();
    let l = ls.remove(0);
    let style = Style::for_profile(&l.profile, [0, 0, 0, 255]);
    PlanLayer {
        name: name.into(),
        alias: None,
        layer: DocLayer::new(LayerId(id), name.into(), l.profile.clone(), style),
        data: l.data,
        rows: None,
        path: Some(path),
    }
}

fn steps(v: serde_json::Value) -> Vec<Step> {
    serde_json::from_value(v).unwrap()
}

/// A two-step plan: filter the schools to high schools, count them per district, then rank.
#[test]
fn a_plan_filters_counts_and_ranks_using_step_outputs() {
    let mut layers = vec![layer("fixtures/T1/schools.csv", "Schools", 1), layer("fixtures/V1/districts.geojson", "Districts", 2)];
    let plan = steps(json!([
        {"name": "filter", "args": {"layer": "Schools", "conditions": [{"field": "type", "op": "is", "value": "High"}]}},
        {"name": "count_in", "args": {"points": "Schools", "areas": "Districts"}},
        {"name": "rank_by_value", "args": {"layer": "#2", "field": "schools"}},
        {"name": "zoom_to", "args": {"layer": "#2"}}
    ]));
    let out = execute(&mut layers, &plan, 0, &Control::none()).unwrap();
    assert!(matches!(&out[0], StepOutcome::Filtered { shown, of, .. } if shown < of));
    assert!(matches!(&out[1], StepOutcome::NewLayer { .. }));
    assert!(matches!(&out[2], StepOutcome::Card { .. }));
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[2].alias.as_deref(), Some("#2"));
}

#[test]
fn objects_are_found_by_field_value_and_errors_name_the_step() {
    let mut layers = vec![layer("fixtures/N1/points.geojson", "Points", 1), layer("fixtures/N1/areas.geojson", "Areas", 2)];
    let ok = execute(&mut layers, &steps(json!([{"name": "which_container", "args": {"point": {"layer": "Points", "field": "point_id", "value": "p04"}, "areas": "Areas"}}])), 0, &Control::none()).unwrap();
    assert!(matches!(&ok[0], StepOutcome::Card { highlight: Some((_, rows)), .. } if rows.len() == 2));
    let e = execute(&mut layers, &steps(json!([{"name": "filter", "args": {"layer": "Nope", "conditions": []}}])), 0, &Control::none())
        .unwrap_err();
    assert_eq!(e.step, 1);
    let e = execute(
        &mut layers,
        &steps(
            json!([{"name": "buffer", "args": {"object": {"layer": "Points", "field": "point_id", "value": "zzz"}, "distance": "1 km"}}]),
        ),
        0,
        &Control::none(),
    )
    .unwrap_err();
    assert!(e.message.contains("no object"), "{e}");
    let asked = execute(
        &mut layers,
        &steps(json!([{"name": "ask_user", "args": {"question": "How far?"}}, {"name": "buffer", "args": {}}])),
        0,
        &Control::none(),
    )
    .unwrap();
    assert_eq!(asked.len(), 1);
}

/// "At least" and "at most" include the bound; above and below don't (puzzle clues say "at
/// least 1,500 m"). 2,455 students is the value of three schools in the fixture.
#[test]
fn at_least_and_at_most_include_the_bound() {
    let shown = |op: &str| {
        let mut layers = vec![layer("fixtures/T1/schools.csv", "Schools", 1)];
        let plan = steps(
            json!([{"name": "filter", "args": {"layer": "Schools", "conditions": [{"field": "students", "op": op, "value": "2,455"}]}}]),
        );
        match execute(&mut layers, &plan, 0, &Control::none()).unwrap().remove(0) {
            StepOutcome::Filtered { shown, of, .. } => (shown, of),
            other => panic!("{other:?}"),
        }
    };
    let (above, of) = shown("above");
    let (at_least, _) = shown("at_least");
    let (below, _) = shown("below");
    let (at_most, _) = shown("at_most");
    assert_eq!(at_least, above + 3);
    assert_eq!(at_most, below + 3);
    assert_eq!(at_least + below, of);
    assert_eq!(at_most + above, of);
}

/// The assistant's `match_table` step (WP6.4): a table's columns join a layer by two named
/// columns of different names, in place; a later filter can use the new field.
#[test]
fn an_assistant_plan_matches_a_table_by_named_columns() {
    let mut layers = vec![layer("fixtures/J6/wards.geojson", "Wards", 1), layer("fixtures/J6/ward_people.csv", "Ward people", 2)];
    let plan = steps(json!([
        {"name": "match_table", "args": {"layer": "Wards", "table": "Ward people", "layer_field": "ward_code", "table_field": "ward"}},
        {"name": "filter", "args": {"layer": "Wards", "conditions": [{"field": "people", "op": "at_least", "value": "3000"}]}}
    ]));
    let out = execute(&mut layers, &plan, 0, &Control::none()).unwrap();
    let StepOutcome::NewFields { layer, fields, card, .. } = &out[0] else { panic!("{:?}", out[0]) };
    assert_eq!((layer.as_str(), fields.as_slice()), ("Wards", ["people".to_string()].as_slice()));
    assert!(card.title.contains("ward ↔ ward_code"), "{}", card.title);
    assert!(matches!(&out[1], StepOutcome::Filtered { shown: 7, .. }), "{:?}", out[1]);
}

/// fixtures/A1: five storm tracks around Tokyo (category 5 except row 2): rows 0 (through Tokyo)
/// and 3 (about 50 km east) pass within 100 km; row 2 passes too but is category 3; rows 1 and 4
/// stay far. And one city, Tokyo.
fn tokyo_map() -> Vec<PlanLayer> {
    vec![layer("fixtures/A1/storms.geojson", "Storms", 1), layer("fixtures/A1/cities.geojson", "Cities", 2)]
}

fn tokyo_steps() -> Vec<Step> {
    steps(json!([
        {"name": "filter", "args": {"layer": "Storms", "conditions": [{"field": "strength", "op": "is", "value": "Category 5"}]}},
        {"name": "buffer", "args": {"object": {"layer": "Cities", "field": "name", "value": "Tokyo"}, "distance": "100 km"}},
        {"name": "count_in", "args": {"points": "Storms", "areas": "#2"}}
    ]))
}

/// The launch-video bug: "How many Category 5 storms have passed within 100 km of Tokyo?". The
/// answer's features are the tracks count_in counted in the one area, not the step-1 filter's.
#[test]
fn count_in_over_one_area_reports_the_counted_features_as_the_answer() {
    let mut layers = tokyo_map();
    let out = execute(&mut layers, &tokyo_steps(), 0, &Control::none()).unwrap();
    assert!(matches!(&out[0], StepOutcome::Filtered { shown: 4, of: 5, .. }));
    assert_eq!(out[2].matched(), Some(vec![("Storms".to_string(), vec![0, 3])]));
    assert_eq!(out[2].made_fields(), vec!["storms".to_string()]);
    // One area: the answer is a number with what was counted where, and being outside the one
    // area is no notice.
    let StepOutcome::NewLayer { card, .. } = &out[2] else { panic!() };
    let lm_core::recipe::Answer::Number { value, unit, detail } = &card.answer else { panic!("{:?}", card.answer) };
    assert_eq!((*value, unit.as_str(), detail.as_str()), (2.0, "storms", "Category 5 storms within 100 km of Tokyo"));
    assert!(card.notices.is_empty(), "{:?}", card.notices);
    // Over many areas the counts are a ranking, not a set of features.
    let mut layers = vec![layer("fixtures/T1/schools.csv", "Schools", 1), layer("fixtures/V1/districts.geojson", "Districts", 2)];
    let out = execute(
        &mut layers,
        &steps(json!([{"name": "count_in", "args": {"points": "Schools", "areas": "Districts"}}])),
        0,
        &Control::none(),
    )
    .unwrap();
    assert!(out[0].matched().is_none());
}

/// whats_inside and join_by_location over one area give their features too.
#[test]
fn whats_inside_and_join_by_location_report_their_features() {
    let mut layers = tokyo_map();
    let mut plan = tokyo_steps();
    plan[2] = steps(json!([{"name": "whats_inside", "args": {"area": {"layer": "#2"}, "layer": "Storms"}}])).remove(0);
    let out = execute(&mut layers, &plan, 0, &Control::none()).unwrap();
    assert_eq!(out[2].matched(), Some(vec![("Storms".to_string(), vec![0, 3])]));
    let mut layers = tokyo_map();
    plan[2] = steps(json!([{"name": "join_by_location", "args": {"layer": "Cities", "areas": "#2"}}])).remove(0);
    let out = execute(&mut layers, &plan, 0, &Control::none()).unwrap();
    assert_eq!(out[2].matched(), Some(vec![("Cities".to_string(), vec![0])]));
}

/// Fields a step made can be named reliably by later steps: "#k" is step k's first made field,
/// and a guessed "km to nearest …" is the one distance field made earlier. A wrong name fails
/// with the exact names of the fields earlier steps made.
#[test]
fn later_steps_name_made_fields_by_step_or_by_the_distance_name() {
    let distance_then = |field: &str| {
        let mut plan = tokyo_steps();
        plan.truncate(2);
        plan.extend(steps(json!([
            {"name": "nearest_distance", "args": {"layer": "Storms", "other": "#2"}},
            {"name": "filter", "args": {"layer": "Storms", "conditions": [
                {"field": "strength", "op": "is", "value": "Category 5"}, {"field": field, "op": "at_most", "value": "0"}]}}
        ])));
        let mut layers = tokyo_map();
        execute(&mut layers, &plan, 0, &Control::none())
    };
    for field in ["#3", "km to nearest buffer", "km to nearest area"] {
        let out = distance_then(field).unwrap_or_else(|e| panic!("{field}: {e}"));
        let StepOutcome::Filtered { shown, filter, .. } = &out[3] else { panic!("{:?}", out[3]) };
        assert_eq!(*shown, 2, "{field}");
        assert_eq!(filter.conditions[1].field, "km to nearest area", "{field}: the filter keeps the real name");
    }
    let e = distance_then("distance to Tokyo").unwrap_err();
    assert_eq!(e.step, 4);
    assert!(e.message.contains("step 3 added “km to nearest area”, “nearest area” to Storms"), "{}", e.message);
    assert!(e.message.contains("\"#3\""), "{}", e.message);
    // "#k" of a step that made no field stays as written and fails the same way.
    let e = distance_then("#2").unwrap_err();
    assert!(e.message.contains("no field “#2”"), "{}", e.message);
    // count_in's count is "#k" too.
    let mut plan = tokyo_steps();
    plan.extend(steps(json!([{"name": "rank_by_value", "args": {"layer": "#3", "field": "#3"}}])));
    let mut layers = tokyo_map();
    assert!(execute(&mut layers, &plan, 0, &Control::none()).is_ok());
}

/// The real model plan from the launch video buffered Tokyo with merge false, so the area kept
/// the city's own name: the card read "Lines in Tokyo". A buffer's area always says it is one,
/// and a count, a list or a "how many here" in it reads "within 100 km of Tokyo".
#[test]
fn an_area_made_by_a_buffer_is_described_as_the_buffer_whatever_it_is_called() {
    for merge in [false, true] {
        let mut plan = tokyo_steps();
        plan[1].args["merge"] = json!(merge);
        let mut layers = tokyo_map();
        let out = execute(&mut layers, &plan, 0, &Control::none()).unwrap();
        let StepOutcome::NewLayer { name, .. } = &out[1] else { panic!() };
        assert_eq!(name, "Within 100 km of Tokyo", "the buffer layer's own name");
        let StepOutcome::NewLayer { name, card, .. } = &out[2] else { panic!() };
        assert_eq!(name, "Storms within 100 km of Tokyo", "merge {merge}");
        assert_eq!(card.title, "Storms within 100 km of Tokyo");
        let lm_core::recipe::Answer::Number { detail, .. } = &card.answer else { panic!("{:?}", card.answer) };
        assert_eq!(detail, "Category 5 storms within 100 km of Tokyo");
        for (action, args, title) in [
            ("whats_inside", json!({"area": {"layer": "#2"}, "layer": "Storms"}), "Storms within 100 km of Tokyo"),
            ("count_here", json!({"points": "Storms", "area": {"layer": "#2"}}), "Storms within 100 km of Tokyo"),
        ] {
            let mut p = plan.clone();
            p[2] = Step { name: action.into(), args };
            let mut layers = tokyo_map();
            let out = execute(&mut layers, &p, 0, &Control::none()).unwrap();
            let StepOutcome::Card { card, .. } = &out[2] else { panic!() };
            assert_eq!(card.title, title, "{action}, merge {merge}");
            assert!(!card.summary.contains("in Tokyo") && !card.summary.contains("inside Tokyo"), "{}", card.summary);
        }
    }
    // One area per object keeps each place's name, and the distance in its own field.
    let mut plan = tokyo_steps();
    plan[1].args["merge"] = json!(false);
    plan.truncate(2);
    let mut layers = tokyo_map();
    execute(&mut layers, &plan, 0, &Control::none()).unwrap();
    let LayerData::Vector(ds) = &layers[2].data else { panic!() };
    assert_eq!(ds.features[0].attrs[0].to_string(), "Tokyo");
    assert_eq!(ds.value(0, ds.field_index("within").unwrap()).to_string(), "100 km");
}

/// Harness v3 bug 3 (n01, "Which volcano is closest to Tokyo?"): nearest_distance from one object
/// reports the nearest object it found and the object it measured from, with a line between.
#[test]
fn n01_nearest_distance_reports_the_nearest_object_and_the_source() {
    let mut layers = tokyo_map();
    let plan = steps(json!([
        {"name": "filter", "args": {"layer": "Cities", "conditions": [{"field": "name", "op": "is", "value": "Tokyo"}]}},
        {"name": "nearest_distance", "args": {"layer": "Cities", "other": "Storms"}}
    ]));
    let out = execute(&mut layers, &plan, 0, &Control::none()).unwrap();
    // Weak (row 2) passes about 5 km from Tokyo: the nearest storm.
    assert_eq!(out[1].matched(), Some(vec![("Storms".to_string(), vec![2]), ("Cities".to_string(), vec![0])]));
    let StepOutcome::NewFields { connectors, .. } = &out[1] else { panic!() };
    assert_eq!(connectors.len(), 1);
    assert_eq!(connectors[0][0], (139.69, 35.69));
    // Measured from many objects, the distances are a ranking, not an answer set.
    let mut layers = tokyo_map();
    let out = execute(
        &mut layers,
        &steps(json!([{"name": "nearest_distance", "args": {"layer": "Storms", "other": "Cities"}}])),
        0,
        &Control::none(),
    )
    .unwrap();
    assert!(out[0].matched().is_some(), "5 storms is still a few");
}

/// Harness v3 bug 7 (k13): nearest_other measures to the other objects the filter shows, as its
/// tool description says; with one object shown, to every other object of the layer.
#[test]
fn k13_nearest_other_measures_to_the_shown_objects() {
    let km = |filter: serde_json::Value| {
        let mut layers = tokyo_map();
        let mut plan = Vec::new();
        if !filter.is_null() {
            plan.push(Step { name: "filter".into(), args: json!({"layer": "Storms", "conditions": [filter]}) });
        }
        plan.push(Step { name: "nearest_other".into(), args: json!({"layer": "Storms"}) });
        execute(&mut layers, &plan, 0, &Control::none()).unwrap();
        let LayerData::Vector(ds) = &layers[0].data else { panic!() };
        let k = ds.field_index("km to nearest other storm").unwrap();
        let n = ds.field_index("nearest other storm").unwrap();
        (ds.features[3].attrs[k].as_f64(), ds.features[3].attrs[n].to_string())
    };
    // All five shown: Near (row 3) crosses Abby and Weak (a tie goes to the lower row).
    assert_eq!(km(serde_json::Value::Null), (Some(0.0), "Abby".to_string()));
    // Near, Far and Wide shown: Near's nearest other is one of them, not the hidden Abby.
    let (d, name) = km(json!({"field": "name", "op": "any_of", "values": ["Near", "Far", "Wide"]}));
    assert!(d.is_some_and(|d| d > 50.0) && name == "Wide", "{d:?} {name}");
    // One shown: measured to every other storm.
    let (d, name) = km(json!({"field": "name", "op": "is", "value": "Near"}));
    assert_eq!((d, name.as_str()), (Some(0.0), "Abby"));
}

/// Harness v3 n01/n04 with the model's plan (measure from every volcano, then rank by the
/// distance): a ranking's answer is its top object, so the preview has something to show.
#[test]
fn n04_a_ranking_reports_its_top_object() {
    let mut layers = tokyo_map();
    let plan = steps(json!([
        {"name": "nearest_distance", "args": {"layer": "Storms", "other": "Cities"}},
        {"name": "rank_by_value", "args": {"layer": "Storms", "field": "#1", "order": "lowest"}}
    ]));
    let out = execute(&mut layers, &plan, 0, &Control::none()).unwrap();
    // Weak passes closest to Tokyo (about 5 km).
    let m = out[1].matched().unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].0, "Storms");
    assert_eq!(m[0].1, vec![2]);
}
