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
