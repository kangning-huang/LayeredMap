//! The system prompt and tools, exported from the measured harness
//! (`tools/lm/assistant/export_assets.py`), with the map's layer names filled in.

use serde_json::Value as Json;

use lm_jobs::plan_exec::PlanLayer;

/// Rules, worked examples and the puzzle-era rules, as tested (docs/lm/llm-eval.md, puzzles).
pub const SYSTEM: &str = include_str!("../assets/system.txt");
const TOOLS: &str = include_str!("../assets/tools.json");

const PLACEHOLDERS: [(&str, &str); 4] =
    [("__POINTS__", "points"), ("__LINES__", "lines"), ("__AREAS__", "areas"), ("__RASTER__", "raster")];

/// The tools for these layers: every enum's placeholders replaced by the names of the layers of
/// that kind. An enum left empty becomes a plain string (some providers reject empty enums).
/// `extra`: built-in layers (name, kind) the plan may name too.
pub fn tools(layers: &[PlanLayer], extra: &[(String, String)]) -> Json {
    let mut t: Json = serde_json::from_str(TOOLS).expect("tools.json is valid JSON (export_assets.py)");
    let names = |kind: &str| -> Vec<String> {
        layers
            .iter()
            .filter(|l| crate::context::kind_word(l) == Some(kind))
            .map(|l| l.name.clone())
            .chain(extra.iter().filter(|(_, k)| k == kind).map(|(n, _)| n.clone()))
            .collect()
    };
    let by_kind: Vec<(&str, Vec<String>)> = PLACEHOLDERS.iter().map(|(p, k)| (*p, names(k))).collect();
    fill(&mut t, &by_kind);
    t
}

fn fill(v: &mut Json, by_kind: &[(&str, Vec<String>)]) {
    match v {
        Json::Object(map) => {
            if let Some(Json::Array(items)) = map.get("enum")
                && items.iter().any(|x| x.as_str().is_some_and(|s| s.starts_with("__") && s.ends_with("__")))
            {
                let mut out = Vec::new();
                for x in items {
                    match by_kind.iter().find(|(p, _)| x.as_str() == Some(p)) {
                        Some((_, names)) => out.extend(names.iter().cloned().map(Json::String)),
                        None => out.push(x.clone()),
                    }
                }
                if out.is_empty() {
                    map.remove("enum");
                } else {
                    map.insert("enum".into(), Json::Array(out));
                }
            }
            for (_, x) in map.iter_mut() {
                fill(x, by_kind);
            }
        }
        Json::Array(items) => items.iter_mut().for_each(|x| fill(x, by_kind)),
        _ => {}
    }
}
