//! Companion choice, eligibility reasons and determinism.

use lm_actions::{Context, FieldSummary, Ineligible, OpenLayer, Options, Selection, check, suggest};
use lm_core::model::{Confidence, FieldRole, LayerId, LayerKind, MeasureKind};

fn points(id: u64, name: &str, shelf: usize, seq: u64) -> OpenLayer {
    OpenLayer::new(LayerId(id), name, LayerKind::Points, shelf, seq).with_fields(vec![
        FieldSummary::new("type", FieldRole::Category, Confidence::Likely),
        FieldSummary::new("enrollment", FieldRole::Measure(MeasureKind::Count), Confidence::Certain),
    ])
}

fn areas(id: u64, name: &str, shelf: usize, seq: u64) -> OpenLayer {
    OpenLayer::new(LayerId(id), name, LayerKind::Areas, shelf, seq)
}

fn raster(id: u64, name: &str, shelf: usize, seq: u64) -> OpenLayer {
    OpenLayer::new(LayerId(id), name, LayerKind::Raster, shelf, seq)
}

fn count_in(ctx: &Context) -> (Option<LayerId>, String) {
    match check("count_in", ctx, Options::developer(4)) {
        Ok(s) => (s.companion, s.label),
        Err(e) => (None, e.message()),
    }
}

#[test]
fn most_recently_added_visible_layer_is_the_companion() {
    let ctx = Context::new(
        Selection::Layer { layer: LayerId(1) },
        vec![points(1, "Schools", 0, 1), areas(2, "Districts", 1, 2), areas(3, "Parks", 2, 3)],
    );
    assert_eq!(count_in(&ctx), (Some(LayerId(3)), "Count Schools in each Park".to_string()));
}

#[test]
fn hidden_layers_are_never_chosen() {
    let ctx = Context::new(
        Selection::Layer { layer: LayerId(1) },
        vec![points(1, "Schools", 0, 1), areas(2, "Districts", 1, 2), areas(3, "Parks", 2, 3).hidden()],
    );
    assert_eq!(count_in(&ctx).0, Some(LayerId(2)));

    // Only a hidden area layer: no companion, and row 5 does not match (row 4 does).
    let ctx = Context::new(Selection::Layer { layer: LayerId(1) }, vec![points(1, "Schools", 0, 1), areas(3, "Parks", 1, 3).hidden()]);
    assert_eq!(count_in(&ctx), (None, "Add an area layer to count Schools in each area.".to_string()));
    assert_eq!(suggest(&ctx, Options::developer(6)).rows, vec![4]);
    // Explicitly co-selecting a hidden layer does not make it a companion either.
    let ctx = ctx.with_co_selected(vec![LayerId(3)]);
    assert_eq!(count_in(&ctx).0, None);
}

#[test]
fn ties_go_to_the_higher_layer_on_the_shelf() {
    let ctx = Context::new(
        Selection::Layer { layer: LayerId(1) },
        vec![points(1, "Schools", 0, 1), areas(2, "Districts", 3, 5), areas(3, "Parks", 1, 5)],
    );
    assert_eq!(count_in(&ctx).0, Some(LayerId(3)));
}

#[test]
fn explicit_co_selection_wins() {
    let layers = vec![points(1, "Schools", 0, 1), areas(2, "Districts", 1, 2), areas(3, "Parks", 2, 3), raster(4, "Heat", 3, 4)];
    let ctx = Context::new(Selection::Layer { layer: LayerId(1) }, layers).with_co_selected(vec![LayerId(2)]);
    assert_eq!(count_in(&ctx).0, Some(LayerId(2)));
    // The co-selected area layer also decides the rows: only row 5, though a raster is open.
    let s = suggest(&ctx, Options::developer(6));
    assert_eq!(s.rows, vec![5]);
    assert_eq!(s.top[0].label, "Count Schools in each District");
}

#[test]
fn the_selected_layer_is_not_its_own_companion() {
    let ctx = Context::new(Selection::Layer { layer: LayerId(2) }, vec![areas(2, "Districts", 0, 2)]);
    assert_eq!(count_in(&ctx), (None, "Add a point layer to count points in each of Districts.".to_string()));
}

#[test]
fn reasons_say_what_to_do() {
    // An area layer without number or category fields.
    let ctx = Context::new(Selection::Layer { layer: LayerId(2) }, vec![areas(2, "Districts", 0, 2)]);
    let free = Options::free(3);
    assert_eq!(
        check("style_by_value", &ctx, free),
        Err(Ineligible::Missing("Districts has no number or category field to color by. Set a field's role in Details.".into()))
    );
    let s = suggest(&ctx, free);
    assert!(s.top.is_empty());
    let ids: Vec<&str> = s.unavailable.iter().map(|u| u.action_id.as_str()).collect();
    assert_eq!(ids, ["style_by_value", "filter"]);

    assert_eq!(check("similar_values", &ctx, free), Err(Ineligible::WrongSelection("Click a cell on a raster layer first.".into())));
    assert_eq!(check("count_in", &ctx, free), Err(Ineligible::NotYet { min_phase: 4 }));
    assert_eq!(check("count_in", &ctx, Options::free(6)), Err(Ineligible::AssistantOnly));
    assert_eq!(check("nope", &ctx, free), Err(Ineligible::UnknownAction));
}

#[test]
fn possible_measures_do_not_qualify() {
    let unsure = OpenLayer::new(LayerId(2), "Districts", LayerKind::Areas, 0, 1).with_fields(vec![FieldSummary::new(
        "code",
        FieldRole::Measure(MeasureKind::Unknown),
        Confidence::Possible,
    )]);
    let ctx = Context::new(Selection::Object { layer: LayerId(2) }, vec![unsure]);
    assert!(matches!(check("compare_peers", &ctx, Options::free(3)), Err(Ineligible::Missing(_))));
}

#[test]
fn starred_fields_are_picked_first() {
    let mut layer = OpenLayer::new(LayerId(2), "Districts", LayerKind::Areas, 0, 1).with_fields(vec![
        FieldSummary::new("income", FieldRole::Measure(MeasureKind::Intensive), Confidence::Certain),
        FieldSummary::new("rent", FieldRole::Measure(MeasureKind::Intensive), Confidence::Likely),
    ]);
    let ctx = |l: &OpenLayer| Context::new(Selection::Layer { layer: LayerId(2) }, vec![l.clone()]);
    assert_eq!(check("style_by_value", &ctx(&layer), Options::free(3)).map(|s| s.field), Ok(Some("income".to_string())));
    layer.fields[1].starred = true;
    assert_eq!(check("style_by_value", &ctx(&layer), Options::free(3)).map(|s| s.field), Ok(Some("rent".to_string())));
}

#[test]
fn same_context_same_output_1000_times() {
    let ctx = Context::new(
        Selection::Layer { layer: LayerId(1) },
        vec![points(1, "Schools", 0, 1), areas(2, "Districts", 1, 2), raster(3, "Heat", 2, 3), areas(4, "Parks", 3, 3)],
    );
    for opts in [Options::developer(6), Options::free(3)] {
        let first = suggest(&ctx, opts);
        assert!(!first.top.is_empty());
        for _ in 0..1000 {
            assert_eq!(suggest(&ctx, opts), first);
        }
    }
}

#[test]
fn unknown_selected_layer_gives_nothing() {
    let ctx = Context::new(Selection::Layer { layer: LayerId(9) }, vec![points(1, "Schools", 0, 1)]);
    assert_eq!(suggest(&ctx, Options::developer(6)), Default::default());
}
