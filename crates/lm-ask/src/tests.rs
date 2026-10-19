use std::cell::RefCell;
use std::path::Path;

use lm_core::document::{DocLayer, Style};
use lm_core::filter::{Condition, Filter, Test};
use lm_core::model::LayerId;
use lm_jobs::import::{Ctx, import_path};
use lm_jobs::jobs::Control;
use lm_jobs::plan_exec::{PlanLayer, StepOutcome};
use serde_json::{Value as Json, json};

use crate::session::{Transport, lint, steps_from_submit};
use crate::{Answer, Request, ask, context, resolve, tools};

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

fn map() -> Vec<PlanLayer> {
    vec![layer("fixtures/T1/schools.csv", "Schools", 1), layer("fixtures/V1/districts.geojson", "Districts", 2)]
}

/// Answers each call with the next scripted message and records the requests.
struct Script {
    replies: RefCell<Vec<Json>>,
    seen: RefCell<Vec<Json>>,
}

impl Script {
    fn new(replies: Vec<Json>) -> Self {
        Script { replies: RefCell::new(replies), seen: RefCell::new(Vec::new()) }
    }
}

impl Transport for Script {
    fn complete(&self, body: &Json) -> Result<Json, String> {
        self.seen.borrow_mut().push(body.clone());
        let mut r = self.replies.borrow_mut();
        if r.is_empty() {
            return Err("no more replies".into());
        }
        Ok(r.remove(0))
    }
}

fn call(id: &str, name: &str, args: Json) -> Json {
    json!({"choices": [{"message": {"content": null, "tool_calls": [
        {"id": id, "type": "function", "function": {"name": name, "arguments": args.to_string()}}
    ]}}], "usage": {"prompt_tokens": 1000, "completion_tokens": 50, "cost": 0.0012}})
}

fn request(text: &str, layers: Vec<PlanLayer>, filters: Vec<Option<Filter>>) -> Request {
    Request {
        text: text.into(),
        layers,
        filters,
        selected: None,
        history: Vec::new(),
        previous: Vec::new(),
        model: crate::MODEL.into(),
        reference: None,
    }
}

#[test]
fn the_tools_name_the_maps_layers_by_kind() {
    let t = tools::tools(&map(), &[]);
    let s = t.to_string();
    assert!(
        !s.contains("__POINTS__")
            && !s.contains("__AREAS__")
            && !s.contains("__RASTER__")
            && !s.contains("__LINES__")
            && !s.contains("__TABLES__")
    );
    let props = &t[0]["function"]["parameters"]["properties"]["steps"]["items"]["properties"];
    // count_in counts any layer, so "points" lists every point, line and area layer.
    assert_eq!(props["points"]["enum"][0], json!("Schools"));
    assert_eq!(props["points"]["enum"][1], json!("Districts"));
    assert_eq!(props["areas"]["enum"][0], json!("Districts"));
    // No raster on this map: a plain string, never an empty enum.
    assert!(props["raster"].get("enum").is_none());
    assert_eq!(t[1]["function"]["name"], "resolve_feature");
    assert!(tools::SYSTEM.contains("submit_plan"));
}

#[test]
fn the_context_shows_fields_filters_and_counts_but_no_values() {
    let layers = map();
    let f = Filter { conditions: vec![Condition { field: "type".into(), test: Test::Is("High".into()) }] };
    let mut l = layers.clone();
    l[0].rows = Some(vec![0, 1, 2]);
    let s = context::map_text(&l, &[Some(f), None], None, &["Show only the high schools.".into()]);
    assert!(s.contains("1. Schools: points"), "{s}");
    assert!(s.contains("students (integer)") || s.contains("students (number)"), "{s}");
    assert!(s.contains("Filter now: type is High (3 of"), "{s}");
    assert!(s.contains("Earlier requests in this conversation"), "{s}");
    assert!(!s.contains("Shanghai No. 1"), "no feature values: {s}");
}

#[test]
fn resolve_finds_names_in_other_spellings() {
    let layers = map();
    let r = resolve::resolve_feature(&layers, "Districts", "jiading");
    assert_eq!(r["candidates"][0]["value"], "Jiading");
    let r = resolve::resolve_feature(&layers, "Districts", "嘉定");
    assert_eq!(r["candidates"][0]["value"], "嘉定区");
    assert!(resolve::result_text(&resolve::resolve_feature(&layers, "Nowhere", "x")).contains("no layer"));
}

#[test]
fn calculate_fields_first_and_second_are_the_runners_a_and_b() {
    let s = steps_from_submit(
        &json!([{"action": "calculate_field", "layer": "Districts", "first": "population", "op": "divide", "second": "area_km2"}]),
    );
    assert_eq!(s[0].name, "calculate_field");
    assert_eq!(s[0].args["a"], "population");
    assert_eq!(s[0].args["b"], "area_km2");
    assert!(s[0].args.get("action").is_none());
}

#[test]
fn a_filter_that_drops_the_current_condition_goes_back() {
    let steps = steps_from_submit(
        &json!([{"action": "filter", "layer": "Schools", "conditions": [{"field": "students", "op": "above", "value": "1000"}]}]),
    );
    let errs = lint(&steps, &[("Schools".into(), vec!["type".into()])]);
    assert!(errs[0].contains("without type"), "{errs:?}");
    let ok = steps_from_submit(&json!([{"action": "filter", "layer": "Schools", "conditions": [
        {"field": "type", "op": "is", "value": "High"}, {"field": "students", "op": "above", "value": "1000"}]}]));
    assert!(lint(&ok, &[("Schools".into(), vec!["type".into()])]).is_empty());
}

#[test]
fn a_plan_is_looked_up_checked_run_and_previewed() {
    let t = Script::new(vec![
        call("c1", "resolve_feature", json!({"layer": "Schools", "text": "high"})),
        call(
            "c2",
            "submit_plan",
            json!({"steps": [
                {"action": "filter", "layer": "Schools", "conditions": [{"field": "type", "op": "is", "value": "High"}]},
                {"action": "count_in", "points": "Schools", "areas": "Districts"}
            ]}),
        ),
    ]);
    let turn = ask(&t, &request("How many high schools are in each district?", map(), vec![None, None]), &Control::none());
    let Ok(Answer::Plan(p)) = turn.answer else { panic!("a plan") };
    assert!(matches!(p.outcomes[0], StepOutcome::Filtered { shown, of, .. } if shown < of));
    assert!(matches!(p.outcomes[1], StepOutcome::NewLayer { .. }));
    assert_eq!(p.layers.len(), 3);
    assert_eq!(turn.usage.calls, 2);
    assert!((turn.usage.cost_usd - 0.0024).abs() < 1e-9);
    // The lookup's answer went back to the model as the tool result.
    let seen = t.seen.borrow();
    let msgs = seen[1]["messages"].as_array().unwrap();
    assert!(msgs.iter().any(|m| m["role"] == "tool" && m["content"].as_str().unwrap().contains("\"High\"")));
    // The static prompt is marked for caching; the map follows it.
    assert_eq!(msgs[0]["content"][0]["cache_control"]["type"], "ephemeral");
}

#[test]
fn a_plan_that_does_not_run_goes_back_once_with_the_reason() {
    let t = Script::new(vec![
        call(
            "c1",
            "submit_plan",
            json!({"steps": [{"action": "filter", "layer": "Schools", "conditions": [{"field": "kind", "op": "is", "value": "High"}]}]}),
        ),
        call(
            "c2",
            "submit_plan",
            json!({"steps": [{"action": "filter", "layer": "Schools", "conditions": [{"field": "type", "op": "is", "value": "High"}]}]}),
        ),
    ]);
    let turn = ask(&t, &request("Only high schools", map(), vec![None, None]), &Control::none());
    assert!(matches!(turn.answer, Ok(Answer::Plan(_))));
    let seen = t.seen.borrow();
    let last = seen[1]["messages"].as_array().unwrap().last().unwrap().clone();
    assert!(last["content"].as_str().unwrap().contains("no field “kind”"), "{last}");
}

#[test]
fn a_question_ends_the_turn_and_the_reply_continues_it() {
    let t = Script::new(vec![call(
        "c1",
        "submit_plan",
        json!({"steps": [{"action": "ask_user", "question": "Which district?", "choices": ["Jiading", "Pudong"]}]}),
    )]);
    let turn = ask(&t, &request("Schools in the district", map(), vec![None, None]), &Control::none());
    let Ok(Answer::Question { text, choices }) = &turn.answer else { panic!("a question") };
    assert_eq!(text, "Which district?");
    assert_eq!(choices.len(), 2);
    // The reply goes after the question's tool result.
    let t2 = Script::new(vec![call("c2", "submit_plan", json!({"steps": [{"action": "cannot_do", "reason": "test"}]}))]);
    let mut r = request("Jiading", map(), vec![None, None]);
    r.previous = turn.messages.clone();
    let _ = ask(&t2, &r, &Control::none());
    let seen = t2.seen.borrow();
    let msgs = seen[0]["messages"].as_array().unwrap();
    assert_eq!(msgs[msgs.len() - 1]["content"], "Jiading");
    assert_eq!(msgs[msgs.len() - 2]["role"], "tool");
}

/// Network: one real request through OpenRouter with the keychain's key
/// (`cargo test -p lm-ask real_request -- --ignored --nocapture`).
#[test]
#[ignore]
#[cfg(not(target_arch = "wasm32"))]
fn real_request() {
    let key = crate::openrouter::read_key().expect("an OpenRouter key in the keychain");
    let t = crate::openrouter::OpenRouter::new(key);
    let turn = ask(&t, &request("How many high schools are in each district?", map(), vec![None, None]), &Control::none());
    eprintln!("usage {:?}", turn.usage);
    match turn.answer {
        Ok(Answer::Plan(p)) => eprintln!("plan {:?}", p.steps),
        Ok(Answer::Question { text, .. }) => panic!("asked: {text}"),
        Ok(Answer::CannotDo(r)) => panic!("refused: {r}"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn a_filter_that_leaves_nothing_goes_back_with_the_stored_values() {
    // Owner's test: "cities in Japan, China and the United States" filtered the city name.
    let t = Script::new(vec![
        call(
            "c1",
            "submit_plan",
            json!({"steps": [{"action": "filter", "layer": "Districts", "conditions": [{"field": "name_zh", "op": "is", "value": "Jiading"}]}]}),
        ),
        call(
            "c2",
            "submit_plan",
            json!({"steps": [{"action": "filter", "layer": "Districts", "conditions": [{"field": "name_en", "op": "is", "value": "Jiading"}]}]}),
        ),
    ]);
    let turn = ask(&t, &request("Only Jiading", map(), vec![None, None]), &Control::none());
    assert!(matches!(turn.answer, Ok(Answer::Plan(_))));
    let seen = t.seen.borrow();
    let back = seen[1]["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap().to_string();
    assert!(back.contains("leaves no Districts") && back.contains(r#"name_en = "Jiading""#), "{back}");
}

#[test]
fn a_locale_gives_its_region() {
    use crate::reference::region_of_locale;
    assert_eq!(region_of_locale("en_CN").as_deref(), Some("CN"));
    assert_eq!(region_of_locale("zh-Hans-CN").as_deref(), Some("CN"));
    assert_eq!(region_of_locale("en_US@calendar=gregorian").as_deref(), Some("US"));
    assert_eq!(region_of_locale("fr").as_deref(), None);
}

/// Windows: the Country or region setting wins over the regional format; "World" (001) and a
/// missing setting fall back to the format's locale.
#[test]
fn windows_country_or_region_gives_the_region() {
    use crate::reference::region_of_windows;
    assert_eq!(region_of_windows(Some("CN"), Some("en-US")).as_deref(), Some("CN"));
    assert_eq!(region_of_windows(Some("tw"), None).as_deref(), Some("TW"));
    assert_eq!(region_of_windows(Some("001"), Some("zh-CN")).as_deref(), Some("CN"));
    assert_eq!(region_of_windows(None, Some("zh-Hans-HK")).as_deref(), Some("HK"));
    assert_eq!(region_of_windows(None, Some("en")).as_deref(), None);
    assert_eq!(region_of_windows(None, None).as_deref(), None);
}

/// Network and the builder's reference data: only the schools are open, and the provinces come
/// from the built-in layers (`cargo test -p lm-ask builtin_layers -- --ignored --nocapture`).
#[test]
#[ignore]
#[cfg(not(target_arch = "wasm32"))]
fn builtin_layers_answer_without_opening_them() {
    let dir = crate::reference::default_dir().expect("reference data");
    let reference = crate::reference::Reference::open(&dir, Some("CN")).map(std::sync::Arc::new);
    let key = crate::openrouter::read_key().expect("an OpenRouter key");
    let t = crate::openrouter::OpenRouter::new(key);
    let mut r = request("Which province has the most schools?", vec![layer("fixtures/T1/schools.csv", "Schools", 1)], vec![None]);
    r.reference = reference;
    let turn = ask(&t, &r, &Control::none());
    eprintln!("usage {:?}", turn.usage);
    let Ok(Answer::Plan(p)) = turn.answer else { panic!("a plan: {:?}", turn.answer.err()) };
    eprintln!("plan {:?}", p.steps);
    assert!(p.steps.iter().any(|s| s.args.to_string().contains("World states and provinces")), "{:?}", p.steps);
}

/// The builder's reference data: China's view folds Taiwan into China; the default keeps it
/// separate (`cargo test -p lm-ask chinas_view -- --ignored`).
#[test]
#[ignore]
fn chinas_view_folds_taiwan_into_china() {
    let dir = crate::reference::default_dir().expect("reference data");
    let has_twn = |region: Option<&str>| {
        let r = crate::reference::Reference::open(&dir, region).expect("manifest");
        let l = r.layer("World countries").expect("countries");
        let lm_jobs::import::LayerData::Vector(ds) = &l.data else { panic!() };
        let k = ds.field_index("iso_a3").unwrap();
        (ds.features.iter().any(|f| f.attrs[k].to_string() == "TWN"), ds.features.len())
    };
    let (default_twn, n_default) = has_twn(None);
    let (cn_twn, n_cn) = has_twn(Some("CN"));
    assert!(default_twn && !cn_twn, "default {default_twn}, CN {cn_twn}");
    assert!(n_cn < n_default, "{n_cn} vs {n_default}");
}

/// A table without locations is on the assistant's map ("table, 12 rows") and in the tools'
/// `table` enum, never among the map layers (WP6.4's `match_table`).
#[test]
fn tables_are_offered_for_match_table_only() {
    let mut layers = map();
    layers.push(layer("fixtures/J6/ward_people.csv", "Ward people", 3));
    let s = context::map_text(&layers, &[None, None, None], None, &[]);
    assert!(s.contains("3. Ward people: table, 12 rows. Fields: ward (text); people (integer)"), "{s}");
    let t = tools::tools(&layers, &[]);
    let props = &t[0]["function"]["parameters"]["properties"]["steps"]["items"]["properties"];
    assert_eq!(props["table"]["enum"], json!(["Ward people"]));
    assert!(!props["layer"]["enum"].as_array().unwrap().contains(&json!("Ward people")));
    let s = t.to_string();
    assert!(s.contains("match_table") && s.contains("table_field"));
}

/// WP7.6 and Gate 7: through the Ask proxy the model gets exactly the request the direct route
/// sends (the same body, byte for byte as JSON), so it carries no feature values either.
#[test]
fn the_proxy_route_sends_the_same_request_and_no_values() {
    let _net = crate::NET_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let reply = call("c1", "submit_plan", json!({"steps": [{"action": "cannot_do", "reason": "no rivers layer"}]}));
    let req = request("schools near rivers", map(), vec![None, None]);
    let direct = Script::new(vec![reply.clone()]);
    let a = ask(&direct, &req, &Control::default());
    let (url, seen) = crate::proxy::tests::service(vec![(200, reply.to_string())]);
    let b = ask(&crate::proxy::Proxy::new(url, "anon".into(), "token".into()), &req, &Control::default());
    assert!(matches!(a.answer, Ok(Answer::CannotDo(_))) && matches!(b.answer, Ok(Answer::CannotDo(_))));
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let sent: Json = serde_json::from_str(&seen[0].1).unwrap();
    assert_eq!(sent, direct.seen.borrow()[0]);
    let text = sent.to_string();
    assert!(text.contains("Schools") && !text.contains("Shanghai No. 1"), "layer names, never values");
}
