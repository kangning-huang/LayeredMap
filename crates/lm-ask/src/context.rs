//! The map as the assistant sees it: each layer's name, kind, size and fields with their types,
//! the filter on it with its count, and what is selected. Never feature values (WP7.2); the one
//! exception is [`crate::resolve`], which answers for the user's own words.

use lm_core::filter::{Filter, Test};
use lm_core::model::LayerKind;
use lm_jobs::Value;
use lm_jobs::import::LayerData;
use lm_jobs::plan_exec::PlanLayer;

/// Most fields listed per layer; the rest are counted.
const MAX_FIELDS: usize = 80;

/// The kind word the tools use ("points", "lines", "areas", "raster"); tables have none.
pub fn kind_word(l: &PlanLayer) -> Option<&'static str> {
    match l.layer.profile.kind {
        LayerKind::Points => Some("points"),
        LayerKind::Lines => Some("lines"),
        LayerKind::Areas => Some("areas"),
        LayerKind::Raster => Some("raster"),
        LayerKind::Table => None,
    }
}

/// How many objects the layer has, and how many its filter shows.
pub fn counts(l: &PlanLayer) -> (usize, usize) {
    match &l.data {
        LayerData::Vector(ds) => {
            let n = ds.features.len();
            (n, l.rows.as_ref().map_or(n, Vec::len))
        }
        LayerData::Raster(_) => (0, 0),
    }
}

fn field_type(ds: &lm_jobs::import::Dataset, k: usize) -> &'static str {
    // The declared type, but a text column of numbers (a CSV) reads as numbers.
    use lm_jobs::import::FieldType;
    match ds.fields[k].ty {
        FieldType::Integer => "integer",
        FieldType::Double => "number",
        FieldType::Boolean => "yes/no",
        FieldType::Text => {
            let mut seen = 0;
            for f in ds.features.iter().take(200) {
                match f.attrs.get(k) {
                    Some(Value::Null) | None => {}
                    Some(v) if v.as_f64().is_some() => seen += 1,
                    Some(_) => return "text",
                }
            }
            if seen > 0 { "number" } else { "text" }
        }
    }
}

/// One condition in words ("POP_MAX at least 1000000").
pub fn condition_text(field: &str, t: &Test) -> String {
    let n = lm_core::filter::number_text;
    let rest = match t {
        Test::Is(v) => format!("is {v}"),
        Test::IsNot(v) => format!("is not {v}"),
        Test::Contains(v) => format!("contains {v}"),
        Test::StartsWith(v) => format!("starts with {v}"),
        Test::EndsWith(v) => format!("ends with {v}"),
        Test::IsEmpty => "is empty".into(),
        Test::Above(v) => format!("above {}", n(*v)),
        Test::Below(v) => format!("below {}", n(*v)),
        Test::Between(a, b) if b.is_infinite() => format!("at least {}", n(*a)),
        Test::Between(a, b) if a.is_infinite() => format!("at most {}", n(*b)),
        Test::Between(a, b) => format!("between {} and {}", n(*a), n(*b)),
        Test::Before(v) => format!("before {v}"),
        Test::After(v) => format!("after {v}"),
        Test::DateBetween(a, b) => format!("from {a} to {b}"),
        Test::AnyOf(vs) => format!("any of {}", vs.join(", ")),
    };
    format!("{field} {rest}")
}

pub fn filter_text(f: &Filter) -> String {
    f.conditions.iter().map(|c| condition_text(&c.field, &c.test)).collect::<Vec<_>>().join("; ")
}

/// One line per layer. `filters` gives each layer's current filter (by position in `layers`).
pub fn layer_line(i: usize, l: &PlanLayer, filter: Option<&Filter>) -> Option<String> {
    let kind = kind_word(l)?;
    match &l.data {
        LayerData::Raster(_) => {
            let bands = l.layer.profile.bands.len().max(1);
            let unit = l.layer.profile.fields.first().and_then(|f| f.unit.value.clone());
            Some(format!(
                "{i}. {}: raster, {bands} band{}{}",
                l.name,
                if bands == 1 { "" } else { "s" },
                unit.map(|u| format!(", {u}")).unwrap_or_default()
            ))
        }
        LayerData::Vector(ds) => {
            let (n, shown) = counts(l);
            let units = |name: &str| {
                l.layer.profile.fields.iter().find(|f| f.name == name).and_then(|f| f.unit.value.clone()).filter(|u| !u.is_empty())
            };
            let mut fields: Vec<String> = ds
                .fields
                .iter()
                .enumerate()
                .take(MAX_FIELDS)
                .map(|(k, f)| match units(&f.name) {
                    Some(u) => format!("{} ({}, {u})", f.name, field_type(ds, k)),
                    None => format!("{} ({})", f.name, field_type(ds, k)),
                })
                .collect();
            if ds.fields.len() > MAX_FIELDS {
                fields.push(format!("and {} more", ds.fields.len() - MAX_FIELDS));
            }
            let made = if l.alias.is_some() { " (made by an earlier request)" } else { "" };
            let mut s = format!("{i}. {}: {kind}, {}{made}. Fields: {}", l.name, thousands(n), fields.join("; "));
            if let Some(f) = filter.filter(|f| !f.conditions.is_empty()) {
                s.push_str(&format!(". Filter now: {} ({} of {} shown)", filter_text(f), thousands(shown), thousands(n)));
            }
            Some(s)
        }
    }
}

/// The map context: open layers, what is selected, the earlier requests of this conversation.
pub fn map_text(layers: &[PlanLayer], filters: &[Option<Filter>], selected: Option<&str>, history: &[String]) -> String {
    let mut lines = vec!["The user's map. Open layers:".to_string()];
    let mut i = 0;
    for (k, l) in layers.iter().enumerate() {
        if let Some(line) = layer_line(i + 1, l, filters.get(k).and_then(Option::as_ref)) {
            i += 1;
            lines.push(line);
        }
    }
    lines.push(match selected {
        Some(s) => format!("Selected: {s}"),
        None => "Selected: nothing".into(),
    });
    if !history.is_empty() {
        lines.push("Earlier requests in this conversation (done):".into());
        lines.extend(history.iter().map(|h| format!("- {h}")));
    }
    lines.join("\n")
}

pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
