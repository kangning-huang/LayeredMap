//! The Ask box without the window, for the end-to-end evaluation harness (`tools/lm/llm_eval/v3`).
//! A development tool: an example, so it is never part of the shipped app.
//!
//! It opens files the way the app does (`Workspace::open`), builds the request the way the Ask box
//! does (`Workspace::ask_inputs`, the built-in reference layers, `lm_ask::MODEL` unless the job
//! names another model), runs `lm_ask::ask` (the app's prompt, tools, checks, repairs and plan
//! executor), and shows the plan as the Ask box would (`Workspace::preview_plan`). It then writes
//! what the user would see: the answer, every step's outcome, and what the preview puts on the map.
//!
//! It never talks to the network. Each model request goes out on stdout and the reply comes back
//! on stdin, so the caller decides what answers (OpenRouter, a recorded reply, a mock) and keeps
//! the key and the spending ledger:
//!
//! ```text
//! cargo run --release -p lm-ui --example ask_eval      # then, one JSON object per line:
//! → {"op":"case","id":"t01","files":["/data/storms.shp"],"text":"…","keys":[["storm_id"],["name"]]}
//! ← {"op":"request","id":"t01","body":{…chat completions body…}}
//! → {"op":"reply","reply":{…chat completions reply…}}        (or {"op":"reply","error":"…"})
//! ← {"op":"result","id":"t01",…}
//! ```
//!
//! Case fields: `files` (opened in order, as one batch), `text`, `keys` (field lists that name an
//! object; the first list whose fields a layer has is used, else the row number), `model`,
//! `selected` (`[layer, field, value]`), `region` (system region for the countries' point of view;
//! default none), `reference` (false: no built-in layers). Maps are opened once per file list.

use std::io::{BufRead, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lm_ask::session::Transport;
use lm_jobs::import::{Dataset, LayerData};
use lm_jobs::plan_exec::{PlanLayer, StepOutcome};
use lm_ui::workspace::Workspace;
use serde_json::{Value as Json, json};

/// Most keys listed per layer or set.
const MAX_KEYS: usize = 20_000;

fn main() {
    let stdin = std::io::stdin();
    let lines = Mutex::new(stdin.lock().lines());
    let mut maps: Vec<(Vec<String>, Workspace, Json)> = Vec::new();
    // The lock is taken and dropped inside next_line, so the transport can read replies.
    while let Some(line) = next_line(&lines) {
        if line.trim().is_empty() {
            continue;
        }
        let job: Json = match serde_json::from_str(&line) {
            Ok(j) => j,
            Err(e) => {
                emit(&json!({"op": "result", "id": null, "fatal": format!("unreadable job: {e}")}));
                continue;
            }
        };
        match job["op"].as_str() {
            Some("case") => {}
            Some("quit") => break,
            _ => {
                emit(&json!({"op": "result", "id": job["id"], "fatal": "expected op case"}));
                continue;
            }
        }
        let files: Vec<String> = job["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_string)).collect();
        if !maps.iter().any(|(f, ..)| *f == files) {
            let (ws, loaded) = open_map(&files);
            maps.push((files.clone(), ws, loaded));
        }
        let Some((_, ws, loaded)) = maps.iter_mut().find(|(f, ..)| *f == files) else { continue };
        let out = run_case(ws, loaded, &job, &lines);
        emit(&out);
    }
}

fn next_line<L: Iterator<Item = std::io::Result<String>>>(lines: &Mutex<L>) -> Option<String> {
    let mut l = lines.lock().ok()?;
    l.next()?.ok()
}

fn emit(v: &Json) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

/// Opens the files as one batch and waits for them (two minutes at most).
fn open_map(files: &[String]) -> (Workspace, Json) {
    let mut ws = Workspace::default();
    ws.open(files.iter().map(std::path::PathBuf::from).collect(), || {});
    let start = Instant::now();
    while (!ws.imports.iter().all(|r| r.error.is_some()) || !ws.idle()) && start.elapsed() < Duration::from_secs(120) {
        ws.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
    ws.wait_pending();
    let errors: Vec<String> = ws.imports.iter().filter_map(|r| r.error.as_ref().map(|e| format!("{}: {e}", r.path.display()))).collect();
    let layers: Vec<Json> = ws
        .doc
        .layers
        .iter()
        .map(|l| {
            json!({
                "name": l.display_name,
                "kind": format!("{:?}", l.profile.kind),
                "count": l.profile.count,
                "questions": l.profile.open_questions().count(),
            })
        })
        .collect();
    (ws, json!({"layers": layers, "errors": errors, "seconds": start.elapsed().as_secs_f64()}))
}

/// Sends each request out on stdout and reads the reply from stdin.
struct Pipe<'a, L: Iterator<Item = std::io::Result<String>>> {
    id: Json,
    lines: &'a Mutex<L>,
    requests: Mutex<u32>,
}

impl<L: Iterator<Item = std::io::Result<String>>> Transport for Pipe<'_, L> {
    fn complete(&self, body: &Json) -> Result<Json, String> {
        if let Ok(mut n) = self.requests.lock() {
            *n += 1;
        }
        emit(&json!({"op": "request", "id": self.id, "body": body}));
        let line = next_line(self.lines).ok_or_else(|| "no reply (stdin closed)".to_string())?;
        let v: Json = serde_json::from_str(&line).map_err(|e| format!("unreadable reply: {e}"))?;
        match v.get("error").and_then(Json::as_str) {
            Some(e) => Err(e.to_string()),
            None => Ok(v["reply"].clone()),
        }
    }
}

fn reference(job: &Json) -> Option<std::sync::Arc<lm_ask::reference::Reference>> {
    if job["reference"].as_bool() == Some(false) {
        return None;
    }
    let dir = lm_ask::reference::default_dir()?;
    lm_ask::reference::Reference::open(&dir, job["region"].as_str()).map(std::sync::Arc::new)
}

fn run_case<L: Iterator<Item = std::io::Result<String>>>(ws: &mut Workspace, loaded: &Json, job: &Json, lines: &Mutex<L>) -> Json {
    let keys: Vec<Vec<String>> = job["keys"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| k.as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_string)).collect())
        .collect();
    let (layers, filters) = ws.ask_inputs();
    let selected = job["selected"].as_array().and_then(|s| match s.as_slice() {
        [l, f, v] => Some((l.as_str()?.to_string(), f.as_str()?.to_string(), v.as_str()?.to_string())),
        _ => None,
    });
    let req = lm_ask::Request {
        text: job["text"].as_str().unwrap_or_default().to_string(),
        layers: layers.clone(),
        filters,
        selected,
        history: Vec::new(),
        previous: Vec::new(),
        model: job["model"].as_str().unwrap_or(lm_ask::MODEL).to_string(),
        reference: reference(job),
    };
    let pipe = Pipe { id: job["id"].clone(), lines, requests: Mutex::new(0) };
    let start = Instant::now();
    let turn = lm_ask::ask(&pipe, &req, &lm_jobs::jobs::Control::default());
    let seconds = start.elapsed().as_secs_f64();
    ws.preview_plan(None);
    ws.fit = None;
    let mut out = json!({
        "op": "result",
        "id": job["id"],
        "map": loaded,
        "model": req.model,
        "usage": {
            "prompt_tokens": turn.usage.prompt_tokens,
            "completion_tokens": turn.usage.completion_tokens,
            "cost_usd": turn.usage.cost_usd,
            "calls": turn.usage.calls,
        },
        "messages": turn.messages,
        "seconds": seconds,
    });
    let answer = match &turn.answer {
        Err(e) => json!({"kind": "error", "text": e}),
        Ok(lm_ask::Answer::Question { text, choices }) => json!({"kind": "ask_user", "text": text, "choices": choices}),
        Ok(lm_ask::Answer::CannotDo(t)) => json!({"kind": "cannot_do", "text": t}),
        Ok(lm_ask::Answer::Plan(p)) => {
            ws.preview_plan(Some(p));
            out["steps"] = serde_json::to_value(&p.steps).unwrap_or(Json::Null);
            out["notes"] = json!(p.notes);
            out["outcomes"] = Json::Array(p.outcomes.iter().map(|o| outcome(o, &p.layers, &keys)).collect());
            out["preview"] = preview(ws, &p.layers, &keys);
            json!({"kind": "plan"})
        }
    };
    out["answer"] = answer;
    out
}

/// The key of each row: the first key list whose fields the layer has, else the row number.
fn row_keys(ds: &Dataset, rows: &[usize], keys: &[Vec<String>]) -> Vec<String> {
    let spec: Option<Vec<usize>> = keys.iter().find_map(|k| k.iter().map(|f| ds.field_index(f)).collect::<Option<Vec<usize>>>());
    rows.iter()
        .take(MAX_KEYS)
        .map(|r| match &spec {
            Some(ix) => ix.iter().map(|k| ds.value(*r, *k).to_string()).collect::<Vec<_>>().join("|"),
            None => format!("row:{r}"),
        })
        .collect()
}

fn find<'a>(layers: &'a [PlanLayer], name: &str) -> Option<&'a PlanLayer> {
    layers
        .iter()
        .rev()
        .find(|l| l.alias.as_deref() == Some(name))
        .or_else(|| layers.iter().rev().find(|l| l.name.eq_ignore_ascii_case(name)))
}

fn vector(l: &PlanLayer) -> Option<&Dataset> {
    match &l.data {
        LayerData::Vector(ds) => Some(ds),
        LayerData::Raster(_) => None,
    }
}

/// A layer's shown rows, their keys and the values of `fields` (all fields when empty).
fn rows_json(l: &PlanLayer, keys: &[Vec<String>], fields: &[String]) -> Json {
    let Some(ds) = vector(l) else { return json!({"name": l.name, "kind": "raster"}) };
    let rows: Vec<usize> = l.rows.clone().unwrap_or_else(|| (0..ds.features.len()).collect());
    let ks = row_keys(ds, &rows, keys);
    let ix: Vec<(String, usize)> = fields.iter().filter_map(|f| ds.field_index(f).map(|k| (f.clone(), k))).collect();
    let values: Vec<Json> = rows
        .iter()
        .take(MAX_KEYS)
        .map(|r| {
            let m: serde_json::Map<String, Json> = ix.iter().map(|(f, k)| (f.clone(), value(ds.value(*r, *k)))).collect();
            Json::Object(m)
        })
        .collect();
    json!({
        "name": l.name,
        "alias": l.alias,
        "kind": format!("{:?}", l.layer.profile.kind),
        "count": ds.features.len(),
        "shown": rows.len(),
        "fields": ds.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
        "keys": ks,
        "values": if ix.is_empty() { Json::Null } else { Json::Array(values) },
    })
}

fn value(v: &lm_jobs::Value) -> Json {
    use lm_jobs::Value as V;
    match v {
        V::Null => Json::Null,
        V::Int(i) => json!(i),
        V::Float(f) if f.is_finite() => json!(f),
        V::Float(_) => Json::Null,
        V::Text(s) => json!(s),
        V::Bool(b) => json!(b),
    }
}

/// Rows of a layer (by name) with their keys.
fn rows_by_name(layers: &[PlanLayer], name: &str, rows: &[usize], keys: &[Vec<String>]) -> Json {
    match find(layers, name).and_then(|l| vector(l).map(|ds| (l, ds))) {
        Some((_, ds)) => json!({"layer": name, "rows": rows.iter().take(MAX_KEYS).collect::<Vec<_>>(), "keys": row_keys(ds, rows, keys)}),
        None => json!({"layer": name, "rows": rows.iter().take(MAX_KEYS).collect::<Vec<_>>()}),
    }
}

fn card_json(card: &lm_core::recipe::ResultCard, layer: Option<&PlanLayer>, keys: &[Vec<String>]) -> Json {
    let mut c = serde_json::to_value(card).unwrap_or(Json::Null);
    // Ranking rows: each row's key, from the ranked layer.
    if let (lm_core::recipe::Answer::Ranking { rows, .. }, Some(ds)) = (&card.answer, layer.and_then(vector)) {
        let rs: Vec<usize> = rows.iter().map(|r| r.row).collect();
        c["row_keys"] = json!(row_keys(ds, &rs, keys));
    }
    c
}

fn outcome(o: &StepOutcome, layers: &[PlanLayer], keys: &[Vec<String>]) -> Json {
    // The features the step found as its answer (what the preview highlights).
    // The step's answer set (its own first; nearest_distance adds the objects measured from).
    let sets = o.matched();
    let matched = sets.as_ref().and_then(|s| s.first()).map(|(l, rows)| rows_by_name(layers, l, rows, keys));
    let matched_all: Option<Vec<Json>> = sets.map(|s| s.iter().map(|(l, rows)| rows_by_name(layers, l, rows, keys)).collect());
    match o {
        // The keys are those of the rows the layer shows after the whole plan (the same as this
        // step's when no later step filters it again).
        StepOutcome::Filtered { layer, shown, of, filter, .. } => json!({
            "kind": "filtered",
            "layer": layer,
            "shown": shown,
            "of": of,
            "filter": lm_ask::context::filter_text(filter),
            "final_keys": find(layers, layer).and_then(|l| vector(l).map(|ds| {
                let rows: Vec<usize> = l.rows.clone().unwrap_or_else(|| (0..ds.features.len()).collect());
                row_keys(ds, &rows, keys)
            })),
        }),
        StepOutcome::Display => json!({"kind": "display"}),
        StepOutcome::NewLayer { name, alias, card, color_by, .. } => {
            let l = find(layers, alias);
            let fields: Vec<String> = color_by.iter().cloned().collect();
            json!({
                "kind": "new_layer",
                "matched": matched,
                "matched_all": matched_all,
                "name": name,
                "alias": alias,
                "color_by": color_by,
                "card": card_json(card, None, keys),
                "layer_rows": l.map(|l| rows_json(l, keys, &fields)),
            })
        }
        StepOutcome::NewFields { layer, fields, card, .. } => json!({
            "kind": "new_fields",
            "matched": matched,
            "matched_all": matched_all,
            "layer": layer,
            "fields": fields,
            "card": card_json(card, None, keys),
            "layer_rows": find(layers, layer).map(|l| rows_json(l, keys, fields)),
        }),
        StepOutcome::Card { card, recipe, highlight, .. } => {
            // A ranking names its layer in the recipe's inputs; the card's rows index it.
            let ranked = recipe.inputs.first().and_then(|i| find(layers, &i.name));
            json!({
                "kind": "card",
                "card": card_json(card, ranked, keys),
                "highlight": highlight.as_ref().map(|(l, rows)| rows_by_name(layers, l, rows, keys)),
            })
        }
        StepOutcome::Asked(q) => json!({"kind": "asked", "text": q}),
        StepOutcome::Refused(r) => json!({"kind": "refused", "text": r}),
    }
}

/// What the preview shows: the layer drawn over the map, the highlighted rows (and whether their
/// layer is on the map: built-in layers never are), and where the view is asked to go.
fn preview(ws: &Workspace, layers: &[PlanLayer], keys: &[Vec<String>]) -> Json {
    let shown = ws.preview_shown();
    let drawn = |l: &lm_mapview::map::Layer| {
        let ds = &l.dataset;
        let rows: Vec<usize> = (0..ds.features.len()).collect();
        let e = ds.extent();
        json!({
            "name": l.name,
            "count": ds.features.len(),
            "keys": row_keys(ds, &rows, keys),
            "selected": (!l.selection.is_empty()).then(|| row_keys(ds, &l.selection, keys)),
            "extent": [e.min_x, e.min_y, e.max_x, e.max_y],
        })
    };
    let lit = |(id, rows): &(lm_core::model::LayerId, Vec<usize>)| {
        let doc = ws.doc.layer(*id);
        let name = doc.map(|l| l.display_name.clone()).or_else(|| layers.iter().find(|l| l.layer.id == *id).map(|l| l.name.clone()));
        let ds = layers.iter().find(|l| l.layer.id == *id).and_then(vector);
        let mut e = None;
        for r in rows {
            if let Some(g) = ds.and_then(|ds| ds.features.get(*r)).and_then(|f| f.geometry.as_ref()) {
                let ge = g.extent();
                e = Some(match e {
                    None => ge,
                    Some(x) => ge.union(&x),
                });
            }
        }
        json!({
            "layer": name,
            "on_map": doc.is_some(),
            "count": rows.len(),
            "keys": ds.map(|ds| row_keys(ds, rows, keys)),
            "extent": e.map(|e| [e.min_x, e.min_y, e.max_x, e.max_y]),
        })
    };
    let highlights: Vec<Json> = shown.highlights.iter().map(|h| lit(h)).collect();
    json!({
        "layer": shown.layer.map(drawn),
        // The first highlight (as before), and every one: an answer may span layers.
        "highlight": highlights.first().cloned(),
        "highlights": highlights,
        // An answer's objects drawn over the map from layers that aren't on it (built-in World
        // layers), all highlighted, and the connector lines.
        "overlays": shown.overlays.iter().map(|l| drawn(l)).collect::<Vec<_>>(),
        // Shown with the answer as context (the object measured from, connector lines).
        "context": shown.context.iter().map(|h| lit(h)).collect::<Vec<_>>(),
        "context_overlays": shown.context_overlays.iter().map(|l| drawn(l)).collect::<Vec<_>>(),
        "empty": shown.empty,
        "faint": shown.faint.iter().filter_map(|id| ws.doc.layer(*id).map(|l| l.display_name.clone())).collect::<Vec<_>>(),
        // Where the Ask box asks the map to fly (the shell frames it beside the answer).
        "fit": ws.answer_fit.or(ws.fit).map(|e| [e.min_x, e.min_y, e.max_x, e.max_y]),
    })
}
