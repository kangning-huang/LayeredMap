//! Runs an assistant plan on files (the evaluation harness and the puzzles use it):
//!
//!     cargo run -p lm-jobs --release --example run_plan -- plan.json
//!
//! `plan.json`: `{"layers": [{"name": "Populated places", "path": "…/ne_10m_populated_places.shp"}, …],
//! "steps": [{"name": "filter", "args": {…}}, …]}`. Prints JSON: each step's outcome, then every
//! layer with the objects the plan leaves shown (title, longitude, latitude; at most `limit`, 50
//! by default), or an error naming the step.

use std::path::PathBuf;

use lm_core::document::{DocLayer, Style};
use lm_core::model::LayerId;
use lm_jobs::import::{Ctx, LayerData, import_path};
use lm_jobs::plan_exec::{PlanLayer, Step, StepOutcome, execute};
use serde_json::{Value, json};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: run_plan plan.json");
        std::process::exit(2)
    });
    let spec: Value =
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
            Ok(v) => v,
            Err(e) => {
                println!("{}", json!({"error": format!("can't read {path}: {e}")}));
                std::process::exit(1)
            }
        };
    let limit = spec.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
    let mut layers = Vec::new();
    for (i, l) in spec["layers"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let name = l["name"].as_str().unwrap_or("").to_string();
        let p = PathBuf::from(l["path"].as_str().unwrap_or(""));
        let mut imported = import_path(&p, &Ctx::detached(Default::default())).unwrap_or_default();
        if imported.is_empty() {
            println!("{}", json!({"error": format!("can't open {}", p.display())}));
            std::process::exit(1)
        }
        let li = imported.remove(0);
        let style = Style::for_profile(&li.profile, [0, 0, 0, 255]);
        layers.push(PlanLayer {
            name: name.clone(),
            alias: None,
            layer: DocLayer::new(LayerId(i as u64 + 1), name, li.profile.clone(), style),
            data: li.data,
            rows: None,
            path: Some(p),
        });
    }
    let steps: Vec<Step> = serde_json::from_value(spec["steps"].clone()).unwrap_or_default();
    let started = std::time::Instant::now();
    let result = execute(&mut layers, &steps, 0, &lm_analysis_control());
    let outcomes: Vec<Value> = match &result {
        Ok(o) => o
            .iter()
            .enumerate()
            .map(|(i, s)| match s {
                StepOutcome::Filtered { layer, shown, of } => json!({"filtered": layer, "shown": shown, "of": of}),
                StepOutcome::Display => json!({"display": true}),
                StepOutcome::NewLayer { name, card } => {
                    // The new layer answers to "#k" (k = the step's position, from 1).
                    let alias = format!("#{}", i + 1);
                    let rows = layers.iter().find(|l| l.alias.as_deref() == Some(alias.as_str())).and_then(|l| match &l.data {
                        LayerData::Vector(ds) => Some(ds.features.len()),
                        LayerData::Raster(_) => None,
                    });
                    json!({"new_layer": name, "alias": alias, "rows": rows, "summary": card.summary})
                }
                StepOutcome::NewFields { layer, fields, card } => json!({"new_fields": fields, "layer": layer, "summary": card.summary}),
                StepOutcome::Card { card, highlight } => {
                    json!({"card": card.title, "summary": card.summary, "answer": format!("{:?}", card.answer), "highlight": highlight})
                }
                StepOutcome::Asked(q) => json!({"asked": q}),
                StepOutcome::Refused(r) => json!({"refused": r}),
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    let state: Vec<Value> = layers
        .iter()
        .map(|l| match &l.data {
            LayerData::Vector(ds) => {
                let rows: Vec<usize> = l.rows.clone().unwrap_or_else(|| (0..ds.features.len()).collect());
                let objects: Vec<Value> = rows
                    .iter()
                    .take(limit)
                    .map(|r| {
                        let f = &ds.features[*r];
                        let p = f.geometry.as_ref().and_then(lm_jobs::actions::label_point);
                        let attrs: serde_json::Map<String, Value> = ds.fields.iter().zip(&f.attrs).map(|(fd, v)| (fd.name.clone(), json!(v.to_string()))).collect();
                        json!({"title": lm_jobs::inspect::object_title(&l.layer, ds, *r), "lon": p.map(|c| c.x), "lat": p.map(|c| c.y), "attrs": attrs})
                    })
                    .collect();
                json!({"name": l.name, "alias": l.alias, "shown": rows.len(), "objects": objects})
            }
            LayerData::Raster(_) => json!({"name": l.name, "raster": true}),
        })
        .collect();
    let out = match result {
        Ok(_) => json!({"ok": true, "steps": outcomes, "layers": state, "seconds": started.elapsed().as_secs_f64()}),
        Err(e) => json!({"ok": false, "error": {"step": e.step, "message": e.message}, "layers": state}),
    };
    println!("{out}");
}

fn lm_analysis_control() -> lm_jobs::jobs::Control {
    lm_jobs::jobs::Control::none()
}
