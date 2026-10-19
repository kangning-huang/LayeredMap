//! Running an assistant plan (Phase 7's plan format, used by the evaluation harness and the
//! puzzles): steps `{name, args}` in order, on a set of named layers. Analysis steps go through
//! [`crate::actions::run`]; `filter` narrows a layer to the rows meeting its conditions (later
//! steps see only those, as in the app); display steps (`color_by`, `zoom_to`, `view_data`) change
//! nothing here; `ask_user` and `cannot_do` end the plan. A step that makes a layer is named
//! `#k` (k = its position, from 1) as well as by its title; a step that adds fields replaces the
//! layer's data in place.
//!
//! Arguments follow the assistant's tools: layers by display name or `#k`; one object as
//! `{layer, field, value}`; filter conditions as `{field, op, value, value2, values}` with ops is,
//! is_not, contains, starts_with, ends_with, is_empty, above and below (strict), at_least and
//! at_most (inclusive), between (inclusive), before, after, any_of; `find_where` conditions as `"raster|test|a|b;…"` with raster names, or a list of
//! `{raster, op, value, value2}` (or filter-shaped `{field: raster, op: above|below|between…}`).

use lm_core::brand::NAME as APP;
use std::collections::BTreeMap;
use std::path::PathBuf;

use lm_analysis::control::Control;
use lm_core::document::DocLayer;
use lm_core::filter::{Condition, Filter, Test};
use lm_core::recipe::{Recipe, ResultCard};
use serde_json::Value as Json;

use crate::actions::{self, ActionError, ActionOutput, Plan, PlanInput};
use crate::import::LayerData;

/// A layer the plan can use.
#[derive(Clone)]
pub struct PlanLayer {
    pub name: String,
    /// Also answers to this name (`#k` for a step's output).
    pub alias: Option<String>,
    pub layer: DocLayer,
    pub data: LayerData,
    /// Rows the current filter shows (None: all).
    pub rows: Option<Vec<usize>>,
    pub path: Option<PathBuf>,
}

/// One step: the action and its arguments.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Step {
    pub name: String,
    #[serde(default)]
    pub args: Json,
}

/// What each step did.
#[derive(Clone, Debug)]
pub enum StepOutcome {
    /// `layer` is the layer's name (a step's output also answers to its `#k`); `filter` is what
    /// the layer is now filtered by, to keep it in the document.
    Filtered {
        layer: String,
        shown: usize,
        of: usize,
        filter: Filter,
    },
    Display,
    /// `alias` is the new layer's `#k`; `color_by` the field the action colours it by.
    NewLayer {
        name: String,
        alias: String,
        card: Box<ResultCard>,
        recipe: Box<Recipe>,
        color_by: Option<String>,
    },
    NewFields {
        layer: String,
        fields: Vec<String>,
        card: Box<ResultCard>,
        recipe: Box<Recipe>,
    },
    Card {
        card: Box<ResultCard>,
        recipe: Box<Recipe>,
        highlight: Option<(String, Vec<usize>)>,
    },
    Asked(String),
    Refused(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanError {
    pub step: usize,
    pub message: String,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "step {}: {}", self.step, self.message)
    }
}

fn err(step: usize, message: impl Into<String>) -> PlanError {
    PlanError { step, message: message.into() }
}

fn find(layers: &[PlanLayer], name: &str) -> Option<usize> {
    let n = name.trim();
    layers.iter().rposition(|l| l.alias.as_deref() == Some(n)).or_else(|| layers.iter().rposition(|l| l.name.eq_ignore_ascii_case(n)))
}

fn text(v: &Json) -> Option<String> {
    match v {
        Json::String(s) => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        Json::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn arg(args: &Json, key: &str) -> Option<String> {
    args.get(key).and_then(text)
}

/// The row of `{layer, field, value}` (case-insensitive text, or the same number).
fn object_row(layers: &[PlanLayer], obj: &Json, step: usize) -> Result<(usize, usize), PlanError> {
    let lname = obj.get("layer").and_then(text).ok_or_else(|| err(step, "an object needs its layer"))?;
    let li = find(layers, &lname).ok_or_else(|| err(step, format!("no layer “{lname}”")))?;
    let LayerData::Vector(ds) = &layers[li].data else { return Err(err(step, format!("{lname} has no objects"))) };
    let shown: Vec<usize> = layers[li].rows.clone().unwrap_or_else(|| (0..ds.features.len()).collect());
    let (Some(field), Some(value)) = (obj.get("field").and_then(text), obj.get("value").and_then(text)) else {
        // No field and value: the only shown object, if there is exactly one.
        return match shown.as_slice() {
            [r] => Ok((li, *r)),
            _ => Err(err(step, format!("which object of {lname}? (give a field and a value)"))),
        };
    };
    let k = ds.field_index(&field).ok_or_else(|| err(step, format!("{lname} has no field “{field}”")))?;
    let want = value.trim().to_lowercase();
    let wnum = want.parse::<f64>().ok();
    shown
        .into_iter()
        .find(|r| {
            let v = &ds.features[*r].attrs[k];
            v.to_string().trim().to_lowercase() == want || (wnum.is_some() && v.as_f64() == wnum)
        })
        .map(|r| (li, r))
        .ok_or_else(|| err(step, format!("no object of {lname} has {field} = {value}")))
}

fn condition(c: &Json, step: usize) -> Result<Condition, PlanError> {
    let field = c.get("field").and_then(text).ok_or_else(|| err(step, "a condition needs a field"))?;
    let op = c.get("op").and_then(text).unwrap_or_else(|| "is".into());
    let v = c.get("value").and_then(text).unwrap_or_default();
    let num = |s: &str| s.trim().replace(',', "").parse::<f64>().map_err(|_| err(step, format!("“{s}” isn't a number")));
    let test = match op.as_str() {
        "is" => Test::Is(v),
        "is_not" => Test::IsNot(v),
        "contains" => Test::Contains(v),
        "starts_with" => Test::StartsWith(v),
        "ends_with" => Test::EndsWith(v),
        "is_empty" => Test::IsEmpty,
        "above" => Test::Above(num(&v)?),
        "below" => Test::Below(num(&v)?),
        // Inclusive bounds ("at least 1,500 m"): the filter's between with an open end, since
        // above and below are strict.
        "at_least" => Test::Between(num(&v)?, f64::INFINITY),
        "at_most" => Test::Between(f64::NEG_INFINITY, num(&v)?),
        "between" => Test::Between(num(&v)?, num(&c.get("value2").and_then(text).unwrap_or_default())?),
        "before" => Test::Before(v),
        "after" => Test::After(v),
        "any_of" => Test::AnyOf(c.get("values").and_then(Json::as_array).map(|a| a.iter().filter_map(text).collect()).unwrap_or_default()),
        other => return Err(err(step, format!("“{other}” isn't a condition"))),
    };
    Ok(Condition { field, test })
}

fn input_of(l: &PlanLayer, rows: Option<Vec<usize>>) -> PlanInput {
    PlanInput { layer: l.layer.clone(), data: l.data.clone(), rows: rows.or_else(|| l.rows.clone()), path: l.path.clone() }
}

/// Runs `steps` on `layers` (which gain the steps' outputs). Stops at the first error, or at
/// `ask_user` / `cannot_do`.
pub fn execute(layers: &mut Vec<PlanLayer>, steps: &[Step], computed_at: i64, ctl: &Control) -> Result<Vec<StepOutcome>, PlanError> {
    let mut out = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        let n = i + 1;
        let a = &s.args;
        let layer_arg = |key: &str, layers: &[PlanLayer]| -> Result<usize, PlanError> {
            let name = arg(a, key).ok_or_else(|| err(n, format!("missing {key}")))?;
            find(layers, &name).ok_or_else(|| err(n, format!("no layer “{name}”")))
        };
        match s.name.as_str() {
            "filter" => {
                let li = layer_arg("layer", layers)?;
                let conds: Vec<Condition> = a
                    .get("conditions")
                    .and_then(Json::as_array)
                    .map(|cs| cs.iter().map(|c| condition(c, n)).collect::<Result<_, _>>())
                    .transpose()?
                    .unwrap_or_default();
                let LayerData::Vector(ds) = &layers[li].data else { return Err(err(n, "only points, lines and areas can be filtered")) };
                for c in &conds {
                    if ds.field_index(&c.field).is_none() {
                        return Err(err(n, format!("{} has no field “{}”", layers[li].name, c.field)));
                    }
                }
                let f = Filter { conditions: conds };
                let total = ds.features.len();
                let rows = crate::inspect::filter_mask(ds, Some(&f))
                    .map(|m| m.iter().enumerate().filter(|(_, k)| **k).map(|(r, _)| r).collect::<Vec<_>>());
                let shown = rows.as_ref().map_or(total, Vec::len);
                layers[li].rows = rows;
                out.push(StepOutcome::Filtered { layer: layers[li].name.clone(), shown, of: total, filter: f });
            }
            "color_by" | "zoom_to" | "view_data" => out.push(StepOutcome::Display),
            "ask_user" => {
                out.push(StepOutcome::Asked(arg(a, "question").unwrap_or_default()));
                return Ok(out);
            }
            "cannot_do" => {
                out.push(StepOutcome::Refused(arg(a, "reason").unwrap_or_default()));
                return Ok(out);
            }
            action if actions::ACTIONS.contains(&action) => {
                let (inputs, params) = plan_inputs(action, a, layers, n)?;
                let plan = Plan {
                    action: action.to_string(),
                    inputs,
                    params,
                    method: lm_actions_method(action),
                    computed_at,
                    software: lm_core::brand::with_version(env!("CARGO_PKG_VERSION")),
                };
                let r = actions::run(&plan, ctl).map_err(|e| match e {
                    ActionError::Cancelled => err(n, "cancelled"),
                    other => err(n, other.to_string()),
                })?;
                let recipe = Box::new(r.recipe);
                match r.output {
                    ActionOutput::NewLayer { layer, color_by } => {
                        let name = layer.name.clone();
                        let id = lm_core::model::LayerId(10_000 + n as u64);
                        let style = lm_core::document::Style::for_profile(&layer.profile, [0, 0, 0, 255]);
                        layers.push(PlanLayer {
                            name: name.clone(),
                            alias: Some(format!("#{n}")),
                            layer: DocLayer::new(id, name.clone(), layer.profile.clone(), style),
                            data: layer.data.clone(),
                            rows: None,
                            path: None,
                        });
                        out.push(StepOutcome::NewLayer { name, alias: format!("#{n}"), card: Box::new(r.card), recipe, color_by });
                    }
                    ActionOutput::NewFields { layer, data, fields } => {
                        let li = layers.iter().position(|l| l.layer.id == layer).ok_or_else(|| err(n, "the layer went away"))?;
                        layers[li].data = LayerData::Vector(data);
                        out.push(StepOutcome::NewFields { layer: layers[li].name.clone(), fields, card: Box::new(r.card), recipe });
                    }
                    ActionOutput::Card => {
                        let hl =
                            r.highlight.and_then(|(id, rows)| layers.iter().find(|l| l.layer.id == id).map(|l| (l.name.clone(), rows)));
                        out.push(StepOutcome::Card { card: Box::new(r.card), recipe, highlight: hl });
                    }
                }
            }
            other => return Err(err(n, format!("“{other}” isn't an action {APP} knows"))),
        }
    }
    Ok(out)
}

/// The recipe method for an action (the runner's tests use fixed text; the app takes it from
/// the registry, which this crate doesn't depend on).
fn lm_actions_method(action: &str) -> String {
    format!("{APP} certified action “{action}”")
}

/// The runner's inputs (in its order) and parameters for an assistant step.
fn plan_inputs(action: &str, a: &Json, layers: &[PlanLayer], n: usize) -> Result<(Vec<PlanInput>, BTreeMap<String, String>), PlanError> {
    let named = |key: &str| -> Result<usize, PlanError> {
        let name = arg(a, key).ok_or_else(|| err(n, format!("missing {key}")))?;
        find(layers, &name).ok_or_else(|| err(n, format!("no layer “{name}”")))
    };
    let object = |key: &str| -> Result<(usize, usize), PlanError> {
        object_row(layers, a.get(key).ok_or_else(|| err(n, format!("missing {key}")))?, n)
    };
    let mut params = BTreeMap::new();
    for k in ["distance", "band", "field", "stat", "a", "op", "b", "name", "fields", "order", "layer_field", "table_field", "repeats"] {
        if let Some(v) = arg(a, k) {
            params.insert(k.to_string(), v);
        }
    }
    if let Some(m) = a.get("merge").and_then(Json::as_bool) {
        params.insert("merge".into(), if m { "yes" } else { "no" }.into());
    }
    let one = |(li, r): (usize, usize)| input_of(&layers[li], Some(vec![r]));
    let inputs = match action {
        "count_in" => vec![input_of(&layers[named("points")?], None), input_of(&layers[named("areas")?], None)],
        "count_here" => vec![input_of(&layers[named("points")?], None), one(object("area")?)],
        "which_container" => vec![one(object("point")?), input_of(&layers[named("areas")?], None)],
        "whats_inside" | "summarize_data_here" => vec![one(object("area")?), input_of(&layers[named("layer")?], None)],
        "buffer" => match a.get("object").filter(|o| !o.is_null()) {
            Some(o) => vec![one(object_row(layers, o, n)?)],
            None => vec![input_of(&layers[named("layer")?], None)],
        },
        "measure_area" => vec![one(object("area")?)],
        "measure_length" => vec![one(object("line")?)],
        "summarize_each" => vec![input_of(&layers[named("raster")?], None), input_of(&layers[named("areas")?], None)],
        "summarize_here" | "summarize_drawn" => vec![input_of(&layers[named("raster")?], None), one(object("area")?)],
        "add_raster_values" => vec![input_of(&layers[named("raster")?], None), input_of(&layers[named("points")?], None)],
        "keep_inside" | "remove_inside" | "join_by_location" => {
            vec![input_of(&layers[named("layer")?], None), input_of(&layers[named("areas")?], None)]
        }
        "nearest_distance" => vec![input_of(&layers[named("layer")?], None), input_of(&layers[named("other")?], None)],
        "match_table" => vec![input_of(&layers[named("layer")?], None), input_of(&layers[named("table")?], None)],
        "find_where" => {
            // Conditions name rasters; the runner wants input positions.
            let mut rasters: Vec<usize> = Vec::new();
            let mut items = Vec::new();
            let mut push = |raster: &str, test: &str, v1: &str, v2: &str| -> Result<(), PlanError> {
                let li = find(layers, raster).ok_or_else(|| err(n, format!("no raster “{raster}”")))?;
                let k = rasters.iter().position(|x| *x == li).unwrap_or_else(|| {
                    rasters.push(li);
                    rasters.len() - 1
                });
                items.push(format!("{k}|{test}|{v1}|{v2}"));
                Ok(())
            };
            match a.get("conditions") {
                Some(Json::Array(cs)) => {
                    for c in cs {
                        // Filter-shaped conditions work too: the raster as `field`, above/below.
                        let raster =
                            c.get("raster").or_else(|| c.get("field")).and_then(text).or_else(|| arg(a, "raster")).unwrap_or_default();
                        let op = match c.get("op").and_then(text).as_deref() {
                            Some("above") | None => "at_least".to_string(),
                            Some("below") => "at_most".to_string(),
                            Some(other) => other.to_string(),
                        };
                        push(
                            &raster,
                            &op,
                            &c.get("value").and_then(text).unwrap_or_default(),
                            &c.get("value2").and_then(text).unwrap_or_default(),
                        )?;
                    }
                }
                Some(Json::String(s)) => {
                    for item in s.split(';').filter(|i| !i.trim().is_empty()) {
                        let p: Vec<&str> = item.split('|').map(str::trim).collect();
                        push(
                            p.first().copied().unwrap_or(""),
                            p.get(1).copied().unwrap_or("at_least"),
                            p.get(2).copied().unwrap_or(""),
                            p.get(3).copied().unwrap_or(""),
                        )?;
                    }
                }
                _ => return Err(err(n, "find_where needs conditions")),
            }
            params.insert("conditions".into(), items.join(";"));
            rasters.into_iter().map(|li| input_of(&layers[li], None)).collect()
        }
        _ => vec![input_of(&layers[named("layer")?], None)],
    };
    Ok((inputs, params))
}

#[cfg(test)]
mod tests;
