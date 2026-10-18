//! `resolve_feature(layer, text)`: how a layer stores a name or category the user wrote (another
//! spelling, an abbreviation, part of a name). The only values the assistant ever sees, and only
//! the few that match the user's words (WP7.2, owner's decision 2026-10-17).

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use lm_jobs::Value;
use lm_jobs::import::LayerData;
use lm_jobs::plan_exec::PlanLayer;

const LIMIT: usize = 5;
/// Rows read per layer at most (a lookup must stay instant).
const MAX_ROWS: usize = 300_000;

fn norm(s: &str) -> String {
    let s: String = s.trim().chars().map(fold).collect::<String>().to_lowercase();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Latin letters without their accents (Foz do Iguaçu, Zürich, São Paulo).
fn fold(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' => 'a',
        'ç' | 'Ç' | 'č' | 'Č' | 'ć' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ě' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' | 'ń' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' => 'y',
        'š' | 'Š' | 'ś' => 's',
        'ž' | 'Ž' | 'ź' | 'ż' => 'z',
        'ł' | 'Ł' => 'l',
        _ => c,
    }
}

fn bigrams(s: &str) -> Vec<(char, char)> {
    let c: Vec<char> = s.chars().collect();
    c.windows(2).map(|w| (w[0], w[1])).collect()
}

/// How well a stored value matches the user's words, 0–1.
fn score(query: &str, stored: &str) -> f64 {
    if query == stored {
        return 1.0;
    }
    if query.chars().count() >= 2 && (stored.starts_with(query) || query.starts_with(stored) && stored.chars().count() >= 3) {
        return 0.9;
    }
    if query.chars().count() >= 3 && stored.contains(query) {
        return 0.8;
    }
    let (a, b) = (bigrams(query), bigrams(stored));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut rest = b.clone();
    let mut same = 0;
    for g in &a {
        if let Some(p) = rest.iter().position(|x| x == g) {
            rest.swap_remove(p);
            same += 1;
        }
    }
    0.85 * (2.0 * same as f64) / (a.len() + b.len()) as f64
}

/// The tool's answer: `{layer, text, candidates: [{field, value, objects}]}` or `{error}`.
pub fn resolve_feature(layers: &[PlanLayer], layer: &str, text: &str) -> Json {
    let Some(l) = layers.iter().rev().find(|l| l.name.eq_ignore_ascii_case(layer.trim())) else {
        return json!({"error": format!("There is no layer named {layer:?}.")});
    };
    let LayerData::Vector(ds) = &l.data else {
        return json!({"error": format!("{} is a raster; it has no names or categories to look up.", l.name)});
    };
    let q = norm(text);
    if q.is_empty() {
        return json!({"layer": l.name, "text": text, "candidates": []});
    }
    let text_fields: Vec<usize> = (0..ds.fields.len()).filter(|&k| ds.fields[k].ty == lm_jobs::import::FieldType::Text).collect();
    // (field, stored value) -> (score, objects)
    let mut hits: BTreeMap<(usize, String), (f64, usize)> = BTreeMap::new();
    let mut seen: BTreeMap<(usize, String), f64> = BTreeMap::new();
    for f in ds.features.iter().take(MAX_ROWS) {
        for &k in &text_fields {
            let Some(Value::Text(v)) = f.attrs.get(k) else { continue };
            if v.trim().is_empty() || v.parse::<f64>().is_ok() {
                continue;
            }
            let key = (k, v.clone());
            if let Some(e) = hits.get_mut(&key) {
                e.1 += 1;
                continue;
            }
            let s = *seen.entry(key.clone()).or_insert_with(|| score(&q, &norm(v)));
            if s >= 0.6 {
                hits.insert(key, (s, 1));
            }
        }
    }
    let top = hits.values().map(|h| h.0).fold(0.0, f64::max);
    let mut c: Vec<_> = hits.into_iter().filter(|(_, h)| h.0 >= (top - 0.15).max(0.6)).collect();
    c.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let candidates: Vec<Json> =
        c.into_iter().take(LIMIT).map(|((k, v), (_, n))| json!({"field": ds.fields[k].name, "value": v, "objects": n})).collect();
    json!({"layer": l.name, "text": text, "candidates": candidates})
}

/// The tool result as the model reads it.
pub fn result_text(r: &Json) -> String {
    if let Some(e) = r.get("error").and_then(Json::as_str) {
        return e.to_string();
    }
    let layer = r.get("layer").and_then(Json::as_str).unwrap_or("");
    let c = r.get("candidates").and_then(Json::as_array).cloned().unwrap_or_default();
    if c.is_empty() {
        return format!(
            "No stored value in {layer} matches {:?}. If it isn't in this layer, ask_user or cannot_do.",
            r["text"].as_str().unwrap_or("")
        );
    }
    let list: Vec<String> = c
        .iter()
        .map(|x| {
            let n = x["objects"].as_u64().unwrap_or(0);
            format!(
                "{} = {:?} ({n} object{})",
                x["field"].as_str().unwrap_or(""),
                x["value"].as_str().unwrap_or(""),
                if n == 1 { "" } else { "s" }
            )
        })
        .collect();
    format!("Stored in {layer}: {}. Use the value exactly as stored.", list.join("; "))
}
