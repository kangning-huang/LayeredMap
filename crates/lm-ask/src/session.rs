//! One request through the assistant, as the harness measured it (tools/lm/puzzles/pro_e2e.py):
//! the model may look values up with `resolve_feature`, then submits the whole plan with
//! `submit_plan`. The plan is checked and run on a copy of the layers (that run is the preview);
//! a plan that fails a check or doesn't run goes back to the model with the reason, at most twice.

use serde_json::{Value as Json, json};

use lm_core::filter::Filter;
use lm_jobs::plan_exec::{self, PlanLayer, Step, StepOutcome};

use crate::{context, resolve, tools};

const MAX_TURNS: usize = 8;
const MAX_REPAIRS: usize = 2;

/// Sends one chat-completions request body and returns the parsed reply.
pub trait Transport {
    fn complete(&self, body: &Json) -> Result<Json, String>;
}

/// What the app knows when the user asks.
pub struct Request {
    pub text: String,
    pub layers: Vec<PlanLayer>,
    /// Each layer's current filter (same order as `layers`).
    pub filters: Vec<Option<Filter>>,
    /// The selected object, as `(layer, field, value)`, when there is one.
    pub selected: Option<(String, String, String)>,
    /// Earlier requests of this conversation that were carried out.
    pub history: Vec<String>,
    /// The messages of an exchange this request answers (a reply to the assistant's question).
    pub previous: Vec<Json>,
    pub model: String,
    /// Built-in reference layers (Pro), named in the context and loaded when a plan uses them.
    pub reference: Option<std::sync::Arc<crate::reference::Reference>>,
}

impl Request {
    /// Built-in layers whose names no open layer uses: (name, kind).
    fn builtin(&self) -> Vec<(String, String)> {
        let Some(r) = &self.reference else { return Vec::new() };
        r.catalogue()
            .into_iter()
            .filter(|b| !self.layers.iter().any(|l| l.name.eq_ignore_ascii_case(&b.name)))
            .map(|b| (b.name, b.kind))
            .collect()
    }

    /// The open layers plus the built-in ones a plan or lookup names.
    fn layers_with(&self, names: &[String]) -> Vec<PlanLayer> {
        let mut layers = self.layers.clone();
        if let Some(r) = &self.reference {
            for (b, _) in self.builtin() {
                if names.iter().any(|n| n.trim().eq_ignore_ascii_case(&b))
                    && let Some(l) = r.layer(&b)
                {
                    layers.push(l);
                }
            }
        }
        layers
    }
}

/// A plan that checked and ran: the preview.
pub struct PlanResult {
    pub steps: Vec<Step>,
    /// The layers after the plan: the open ones (changed in place) and the new ones (`#k`).
    pub layers: Vec<PlanLayer>,
    pub outcomes: Vec<StepOutcome>,
    /// Check messages still open after the repair rounds (the plan ran anyway).
    pub notes: Vec<String>,
}

pub enum Answer {
    Plan(Box<PlanResult>),
    Question { text: String, choices: Vec<String> },
    CannotDo(String),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cost_usd: f64,
    pub calls: u32,
}

pub struct Turn {
    pub answer: Result<Answer, String>,
    /// The exchange without the system message, for a follow-up reply.
    pub messages: Vec<Json>,
    pub usage: Usage,
}

/// `submit_plan`'s steps as plan steps: `action` is the step's name; calculate_field's `first`
/// and `second` are the runner's `a` and `b`.
pub fn steps_from_submit(steps: &Json) -> Vec<Step> {
    let mut out = Vec::new();
    for s in steps.as_array().into_iter().flatten() {
        let Some(obj) = s.as_object() else { continue };
        let mut args = obj.clone();
        let name = args.remove("action").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        for (new, old) in [("first", "a"), ("second", "b")] {
            if let Some(v) = args.remove(new) {
                args.entry(old.to_string()).or_insert(v);
            }
        }
        out.push(Step { name, args: Json::Object(args) });
    }
    out
}

/// A filter replaces the layer's current one (owner's decision, 2026-10-17): a filter that leaves
/// out a condition already on the layer, or one an earlier step of the plan set, almost always
/// means the user's "keep those…" was read as "show all those…".
pub fn lint(steps: &[Step], current: &[(String, Vec<String>)]) -> Vec<String> {
    let mut errs = Vec::new();
    let mut last: Vec<(String, Option<usize>, Vec<String>)> = current.iter().map(|(l, f)| (l.to_lowercase(), None, f.clone())).collect();
    for (i, s) in steps.iter().enumerate() {
        if s.name != "filter" {
            continue;
        }
        let Some(layer) = s.args.get("layer").and_then(Json::as_str) else { continue };
        let Some(conds) = s.args.get("conditions").and_then(Json::as_array) else { continue };
        let fields: Vec<String> = conds.iter().filter_map(|c| c.get("field").and_then(Json::as_str)).map(str::to_string).collect();
        let key = layer.to_lowercase();
        if let Some((_, from, before)) = last.iter().find(|(l, ..)| *l == key)
            && !conds.is_empty()
        {
            let dropped: Vec<&String> = before.iter().filter(|f| !fields.iter().any(|g| g.eq_ignore_ascii_case(f))).collect();
            if !dropped.is_empty() {
                let names = dropped.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                errs.push(match from {
                        None => format!(
                            "Step #{} filters {layer} without {names}, which the filter now on {layer} uses. A filter replaces the current one, so repeat the current conditions in this filter, unless the user asked to show all or to drop them.",
                            i + 1
                        ),
                        Some(j) => format!(
                            "Step #{} filters {layer} again without {names} from step #{}. A filter replaces the earlier one, so repeat the earlier conditions you still want in this filter.",
                            i + 1,
                            j + 1
                        ),
                    });
            }
        }
        last.retain(|(l, ..)| *l != key);
        last.push((key, Some(i), fields));
    }
    errs
}

fn system_message(req: &Request) -> Json {
    let selected = req.selected.as_ref().map(|(l, f, v)| {
        format!("one object of {l} ({f} = {v:?}); for \"this\", \"selected\" or \"here\" pass it as {{layer: {l:?}, field: {f:?}, value: {v:?}}}")
    });
    let mut map = context::map_text(&req.layers, &req.filters, selected.as_deref(), &req.history);
    if let Some(r) = &req.reference {
        let taken: Vec<String> = req.layers.iter().map(|l| l.name.clone()).collect();
        let first = req.layers.iter().filter(|l| context::kind_word(l).is_some()).count() + 1;
        let lines = r.context_lines(first, &taken);
        if !lines.is_empty() {
            map.push_str("\nBuilt-in reference layers (Natural Earth, NOAA ETOPO; not on the map): use one when the request needs countries, provinces, cities, rivers, lakes, coastline, land, islands, regions, seas or elevation and no open layer has them; prefer the user's own layers. Name them in a plan like any layer.\n");
            map.push_str(&lines.join("\n"));
        }
    }
    json!({"role": "system", "content": [
        {"type": "text", "text": tools::SYSTEM, "cache_control": {"type": "ephemeral"}},
        {"type": "text", "text": map},
    ]})
}

/// Messages for filter steps that leave no object, with the stored values that match the text
/// conditions' words in any field of that layer.
fn empty_filters(steps: &[Step], outcomes: &[StepOutcome], layers: &[PlanLayer]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, (s, o)) in steps.iter().zip(outcomes).enumerate() {
        let StepOutcome::Filtered { layer, shown: 0, of, .. } = o else { continue };
        if *of == 0 {
            continue;
        }
        let mut words: Vec<String> = Vec::new();
        for c in s.args.get("conditions").and_then(Json::as_array).into_iter().flatten() {
            let op = c.get("op").and_then(Json::as_str).unwrap_or("");
            if matches!(op, "is" | "any_of" | "contains" | "starts_with" | "ends_with") {
                words.extend(c.get("value").and_then(Json::as_str).map(str::to_string));
                words.extend(c.get("values").and_then(Json::as_array).into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)));
            }
        }
        let mut hints: Vec<String> = Vec::new();
        for w in words.iter().take(6) {
            let r = resolve::resolve_feature(layers, layer, w);
            for c in r["candidates"].as_array().into_iter().flatten().take(2) {
                hints.push(format!("{} = {:?}", c["field"].as_str().unwrap_or(""), c["value"].as_str().unwrap_or("")));
            }
        }
        let mut m = format!("step {}: the filter leaves no {layer} (0 of {of}). Check the field and the values.", i + 1);
        if !hints.is_empty() {
            hints.dedup();
            m.push_str(&format!(" Stored values like the words used: {}.", hints.join("; ")));
        }
        out.push(m);
    }
    out
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Runs one request to an answer.
pub fn ask(t: &dyn Transport, req: &Request, ctl: &lm_jobs::jobs::Control) -> Turn {
    let tool_list = tools::tools(&req.layers, &req.builtin());
    let mut messages: Vec<Json> = req.previous.clone();
    messages.push(json!({"role": "user", "content": req.text}));
    let system = system_message(req);
    let current: Vec<(String, Vec<String>)> = req
        .layers
        .iter()
        .zip(&req.filters)
        .filter_map(|(l, f)| {
            f.as_ref()
                .filter(|f| !f.conditions.is_empty())
                .map(|f| (l.name.clone(), f.conditions.iter().map(|c| c.field.clone()).collect()))
        })
        .collect();
    let mut usage = Usage::default();
    let mut repairs = 0;
    let mut answer: Option<Result<Answer, String>> = None;
    for _ in 0..MAX_TURNS {
        if ctl.is_cancelled() {
            answer = Some(Err("Cancelled.".into()));
            break;
        }
        let mut all = vec![system.clone()];
        all.extend(messages.iter().cloned());
        let body = json!({
            "model": req.model,
            "messages": all,
            "tools": tool_list,
            "tool_choice": "required",
            "temperature": 0,
            "max_tokens": 2500,
            "usage": {"include": true},
        });
        let reply = match t.complete(&body) {
            Ok(r) => r,
            Err(e) => {
                answer = Some(Err(e));
                break;
            }
        };
        usage.calls += 1;
        if let Some(u) = reply.get("usage") {
            usage.prompt_tokens += u["prompt_tokens"].as_u64().unwrap_or(0);
            usage.completion_tokens += u["completion_tokens"].as_u64().unwrap_or(0);
            usage.cost_usd += u["cost"].as_f64().unwrap_or(0.0);
        }
        if let Some(e) = reply.get("error") {
            answer = Some(Err(format!(
                "The assistant service answered with an error: {}",
                e.get("message").and_then(Json::as_str).unwrap_or("unknown")
            )));
            break;
        }
        let msg = reply["choices"][0]["message"].clone();
        let calls = msg.get("tool_calls").and_then(Json::as_array).cloned().unwrap_or_default();
        let text = msg.get("content").and_then(Json::as_str).unwrap_or("").trim().to_string();
        if calls.is_empty() {
            answer = Some(Ok(if text.contains('?') || text.contains('？') {
                Answer::Question { text, choices: Vec::new() }
            } else if text.is_empty() {
                Answer::CannotDo("The assistant gave no plan.".into())
            } else {
                Answer::CannotDo(text)
            }));
            messages.push(json!({"role": "assistant", "content": msg.get("content").cloned().unwrap_or(Json::Null)}));
            break;
        }
        messages.push(json!({"role": "assistant", "content": msg.get("content").cloned().unwrap_or(Json::Null), "tool_calls": calls}));
        let mut turn_errors = false;
        for c in &calls {
            let id = c["id"].as_str().unwrap_or("").to_string();
            let name = c["function"]["name"].as_str().unwrap_or("");
            let args: Json = match c["function"]["arguments"].as_str().map(serde_json::from_str::<Json>) {
                Some(Ok(v)) => v,
                _ => c["function"]["arguments"].clone(),
            };
            let result = match name {
                "resolve_feature" => {
                    let layer = args["layer"].as_str().unwrap_or("");
                    resolve::result_text(&resolve::resolve_feature(
                        &req.layers_with(&[layer.to_string()]),
                        layer,
                        args["text"].as_str().unwrap_or(""),
                    ))
                }
                "submit_plan" if answer.is_none() => {
                    let steps = steps_from_submit(&args["steps"]);
                    match judge(&steps, req, &current, repairs, ctl) {
                        Judged::Done(a) => {
                            answer = Some(a);
                            "Plan sent to the user for approval.".to_string()
                        }
                        Judged::Repair(why) => {
                            turn_errors = true;
                            format!("Plan not accepted. Fix these and call submit_plan again with the whole plan: {}", why.join(" | "))
                        }
                    }
                }
                "submit_plan" => "Already sent.".to_string(),
                _ => "Use submit_plan for plans.".to_string(),
            };
            messages.push(json!({"role": "tool", "tool_call_id": id, "content": result}));
        }
        if turn_errors {
            repairs += 1;
        }
        if answer.is_some() {
            break;
        }
    }
    Turn { answer: answer.unwrap_or_else(|| Err("The assistant didn't finish a plan.".into())), messages, usage }
}

enum Judged {
    Done(Result<Answer, String>),
    Repair(Vec<String>),
}

fn judge(steps: &[Step], req: &Request, current: &[(String, Vec<String>)], repairs: usize, ctl: &lm_jobs::jobs::Control) -> Judged {
    if steps.is_empty() {
        return if repairs < MAX_REPAIRS {
            Judged::Repair(vec!["submit_plan came with no steps.".into()])
        } else {
            Judged::Done(Err("The assistant sent an empty plan.".into()))
        };
    }
    if let Some(m) = steps.iter().find(|s| s.name == "ask_user" || s.name == "cannot_do") {
        if steps.len() > 1 && repairs < MAX_REPAIRS {
            return Judged::Repair(vec!["ask_user or cannot_do must be the only step: either plan the steps or ask/refuse.".into()]);
        }
        let a = &m.args;
        return Judged::Done(Ok(if m.name == "ask_user" {
            Answer::Question {
                text: a["question"].as_str().unwrap_or("").to_string(),
                choices: a["choices"]
                    .as_array()
                    .map(|c| c.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default(),
            }
        } else {
            Answer::CannotDo(a["reason"].as_str().unwrap_or("").to_string())
        }));
    }
    let notes = lint(steps, current);
    if !notes.is_empty() && repairs < MAX_REPAIRS {
        return Judged::Repair(notes);
    }
    let mut names = Vec::new();
    for s in steps {
        crate::reference::names_in(&s.args, &mut names);
    }
    let mut layers = req.layers_with(&names);
    match plan_exec::execute(&mut layers, steps, now(), ctl) {
        Ok(outcomes) => {
            // A filter that leaves nothing almost always means a wrong field or value ("name is
            // Japan" on cities, whose country is in "country"): send it back once with the stored
            // values closest to the words used.
            if repairs < MAX_REPAIRS {
                let empty = empty_filters(steps, &outcomes, &layers);
                if !empty.is_empty() {
                    return Judged::Repair(empty);
                }
            }
            Judged::Done(Ok(Answer::Plan(Box::new(PlanResult { steps: steps.to_vec(), layers, outcomes, notes }))))
        }
        Err(e) if e.message == "cancelled" => Judged::Done(Err("Cancelled.".into())),
        Err(e) if repairs < MAX_REPAIRS => Judged::Repair(vec![format!("step {}: {}", e.step, e.message)]),
        Err(e) => Judged::Done(Err(format!("The plan didn't run: step {}: {}", e.step, e.message))),
    }
}
