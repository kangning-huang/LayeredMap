//! The composer for the certified actions (spec › Composer, Preview, Result card): opened by a
//! card's analysis action when Pro is unlocked, and on the developer surface (`LM_DEV=1` or a
//! debug build). While Pro is locked the actions show with a lock and open Settings › Account
//! instead (`account_ui::locked`). The result card and the recipe are the same either way.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{RichText, Ui, vec2};

use lm_actions::Suggested;
use lm_core::document::DocLayer;
use lm_core::model::{LayerId, LayerKind};
use lm_core::recipe::{Answer, ResultCard};
use lm_core::{i18n, tl, tn};
use lm_jobs::actions::{ActionError, ActionOutput, ActionResult, Plan, number};
use lm_jobs::jobs::{ActionKind, InputSummary};

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, Theme, semibold};

/// The phase whose actions the developer surface offers while Phase 4 is being built (the free
/// app stays at the build phase).
pub const DEV_PHASE: u8 = 4;

/// A question being composed: the action, its inputs and parameters, and the latest preview.
pub struct Composer {
    pub action: String,
    /// "Count Schools in each District" (the suggestion's label).
    pub label: String,
    /// The layer whose panel shows the composer.
    pub main: LayerId,
    /// The selected object, for actions about one object.
    pub row: Option<usize>,
    pub companion: Option<LayerId>,
    pub params: BTreeMap<String, String>,
    /// Only the selected objects of the main layer (when several are selected).
    pub selected_only: bool,
    pub selected_rows: Vec<usize>,
    /// The run whose result the composer is waiting for or showing.
    pub generation: Option<u64>,
    pub result: Option<Arc<ActionResult>>,
    pub error: Option<String>,
    /// Parameters changed since the preview and the estimate is too long to rerun alone.
    pub stale: bool,
    /// The key of the plan last run (to rerun automatically when it changes).
    ran_key: String,
    /// A result kept from this composer: its card stays until closed.
    pub kept: Option<u64>,
}

/// The companion kinds an action can take, given the main layer's kind.
fn companion_kinds(action: &str, main: LayerKind) -> Vec<LayerKind> {
    use LayerKind::*;
    match (action, main) {
        ("count_in", Points) => vec![Areas],
        ("count_in", Areas) => vec![Points],
        ("count_here", _) => vec![Points],
        ("which_container", _) => vec![Areas],
        ("whats_inside" | "summarize_data_here", _) => vec![Points, Lines, Areas],
        ("summarize_each", Raster) => vec![Areas],
        ("summarize_each", _) => vec![Raster],
        ("summarize_here" | "summarize_drawn", _) => vec![Raster],
        ("add_raster_values", Raster) => vec![Points],
        ("add_raster_values", _) => vec![Raster],
        ("keep_inside" | "remove_inside" | "join_by_location", _) => vec![Areas],
        ("nearest_distance", _) => vec![Points, Lines, Areas],
        _ => Vec::new(),
    }
}

/// Opens the composer for an analysis suggestion, and previews at once when the estimate is under
/// a second. With Pro locked, Settings › Account opens instead.
pub fn open(shell: &mut Shell, main: LayerId, row: Option<usize>, a: &Suggested) {
    if !shell.analysis_unlocked() {
        crate::account_ui::locked(shell);
        return;
    }
    if !lm_jobs::actions::ACTIONS.contains(&a.action_id.as_str()) {
        shell.notice = Some((i18n::fmt("“{action}” isn't connected yet.", &[("action", &a.label)]), 0.0));
        return;
    }
    let Some(l) = shell.ws.doc.layer(main).cloned() else { return };
    // "Summarize in an area I draw" on a raster: draw first, then summarize there.
    if a.action_id == "summarize_drawn" && l.profile.kind == LayerKind::Raster {
        close(shell);
        shell.after_drawing = Some(main);
        shell.drawing = Some(crate::draw::Drawing::new(crate::draw::Mode::Polygon));
        return;
    }
    let kinds = companion_kinds(&a.action_id, l.profile.kind);
    let companion = a
        .companion
        .or_else(|| shell.ws.doc.layers.iter().rev().find(|o| o.id != main && o.visible && kinds.contains(&o.profile.kind)).map(|o| o.id));
    let mut params = BTreeMap::new();
    if a.action_id == "buffer" {
        params.insert("distance".into(), "1 km".into());
    }
    if a.action_id == "find_where" {
        params.insert("conditions".into(), format!("{}|at_least|", main.0));
    }
    if a.action_id == "rank_by_value" {
        let f = a.field.clone().or_else(|| first_numeric_field(&l));
        if let Some(f) = f {
            params.insert("field".into(), f);
        }
    }
    let selected_rows = shell.ws.selection.rows_in(main);
    shell.ws.cancel_plan();
    shell.composer = Some(Composer {
        action: a.action_id.clone(),
        label: a.label.clone(),
        main,
        row,
        companion,
        params,
        selected_only: selected_rows.len() > 1,
        selected_rows,
        generation: None,
        result: None,
        error: None,
        stale: false,
        ran_key: String::new(),
        kept: None,
    });
    refresh(shell, true);
}

/// After an area is drawn for "Summarize in an area I draw": select it and summarize the raster
/// there.
pub fn area_drawn(shell: &mut Shell, area: LayerId) {
    let Some(raster) = shell.after_drawing.take() else { return };
    shell.ws.selection = lm_core::selection::SelectionContext::object(lm_core::selection::FeatureRef { layer: area, row: 0 });
    let a = Suggested {
        action_id: "summarize_drawn".into(),
        label: lm_actions::registry()
            .action("summarize_drawn")
            .map_or_else(|| tl!("Summarize in an area I draw").to_string(), |s| tl!(&s.label).to_string()),
        group: lm_actions::Group::Measure,
        companion: Some(raster),
        field: None,
    };
    open(shell, area, Some(0), &a);
}

/// Opens the composer for `action` on the selected layer and object (scripts and tests), with
/// parameters, and previews.
pub fn compose_by_id(shell: &mut Shell, action: &str, params: Vec<(String, String)>) {
    let Some(main) = shell.ws.selected() else { return };
    let rows = shell.ws.selection.rows_in(main);
    let row = (rows.len() == 1).then(|| rows[0]);
    let ctx = crate::insight::context(shell);
    let s = lm_actions::suggest(&ctx, lm_actions::Options::developer(DEV_PHASE.max(shell.flags.phase())));
    let found = s.top.iter().chain(s.more.iter().flat_map(|(_, v)| v.iter())).find(|a| a.action_id == action).cloned();
    let a = found.unwrap_or_else(|| Suggested {
        action_id: action.to_string(),
        label: lm_actions::registry().action(action).map_or_else(|| action.to_string(), |s| tl!(&s.label).to_string()),
        group: lm_actions::registry().action(action).map_or(lm_actions::Group::Measure, |s| s.group),
        companion: None,
        field: None,
    });
    open(shell, main, row, &a);
    if let Some(c) = shell.composer.as_mut() {
        for (k, v) in params {
            if k == "with" {
                c.companion = shell.ws.doc.layers.iter().find(|l| l.display_name == v).map(|l| l.id).or(c.companion);
            } else {
                c.params.insert(k, v);
            }
        }
    }
    refresh(shell, true);
}

fn first_numeric_field(l: &DocLayer) -> Option<String> {
    l.profile.fields.iter().find(|f| matches!(l.role(f).value, lm_core::model::FieldRole::Measure(_))).map(|f| f.name.clone())
}

/// The plan for the composer as it stands, or what is missing.
fn plan(shell: &Shell, c: &Composer) -> Result<Plan, String> {
    let main = shell.ws.doc.layer(c.main).ok_or(tl!("The layer was removed."))?;
    let kind = main.profile.kind;
    let rows_main = if c.selected_only && !c.selected_rows.is_empty() { Some(c.selected_rows.clone()) } else { None };
    let one = c.row.map(|r| vec![r]);
    let needs_companion = !companion_kinds(&c.action, kind).is_empty();
    let other = match (needs_companion, c.companion) {
        (true, Some(id)) => Some(id),
        (true, None) => return Err(missing_layer(&companion_kinds(&c.action, kind))),
        (false, _) => None,
    };
    let input =
        |id: LayerId, rows: Option<Vec<usize>>| shell.ws.plan_input(id, rows).ok_or_else(|| tl!("A layer is still loading.").to_string());
    let inputs = match c.action.as_str() {
        "count_in" => {
            let o = other.unwrap_or(c.main);
            let (p, a) = if kind == LayerKind::Points { (c.main, o) } else { (o, c.main) };
            vec![
                input(p, if p == c.main { rows_main.clone() } else { None })?,
                input(a, if a == c.main { rows_main.clone() } else { None })?,
            ]
        }
        "count_here" => vec![input(other.unwrap_or(c.main), None)?, input(c.main, one.clone())?],
        "which_container" => vec![input(c.main, rows_main.clone().or(one.clone()))?, input(other.unwrap_or(c.main), None)?],
        "whats_inside" | "summarize_data_here" => vec![input(c.main, one.clone())?, input(other.unwrap_or(c.main), None)?],
        "buffer" => vec![input(c.main, rows_main.clone().or(one.clone()))?],
        "measure_area" | "measure_length" => vec![input(c.main, one.clone())?],
        "summarize_each" => {
            let o = other.unwrap_or(c.main);
            let (r, a) = if kind == LayerKind::Raster { (c.main, o) } else { (o, c.main) };
            vec![input(r, None)?, input(a, if a == c.main { rows_main.clone() } else { None })?]
        }
        "summarize_here" | "summarize_drawn" => vec![input(other.unwrap_or(c.main), None)?, input(c.main, one.clone())?],
        "add_raster_values" => {
            let o = other.unwrap_or(c.main);
            let (r, p) = if kind == LayerKind::Raster { (c.main, o) } else { (o, c.main) };
            vec![input(r, None)?, input(p, if p == c.main { rows_main.clone() } else { None })?]
        }
        "find_where" => {
            // The conditions name rasters by layer id; the runner wants input positions.
            let mut ids: Vec<LayerId> = Vec::new();
            let mut items = Vec::new();
            for item in c.params.get("conditions").map(String::as_str).unwrap_or("").split(';').filter(|i| !i.trim().is_empty()) {
                let mut parts: Vec<String> = item.split('|').map(str::to_string).collect();
                let id = parts.first().and_then(|p| p.parse::<u64>().ok()).map(LayerId).ok_or(tl!("A condition names no raster."))?;
                let k = ids.iter().position(|x| *x == id).unwrap_or_else(|| {
                    ids.push(id);
                    ids.len() - 1
                });
                parts[0] = k.to_string();
                items.push(parts.join("|"));
            }
            if items.is_empty() {
                return Err(tl!("Add a condition.").into());
            }
            let mut v = Vec::new();
            for id in ids {
                v.push(input(id, None)?);
            }
            let mut p = c.params.clone();
            p.insert("conditions".into(), items.join(";"));
            return finish_plan(c, v, p);
        }
        "keep_inside" | "remove_inside" | "join_by_location" | "nearest_distance" => {
            vec![input(c.main, rows_main.clone())?, input(other.unwrap_or(c.main), None)?]
        }
        _ => vec![input(c.main, rows_main.clone().or(one.clone()))?],
    };
    finish_plan(c, inputs, c.params.clone())
}

/// What to add when the companion layer is missing: one whole sentence per set of kinds.
fn missing_layer(kinds: &[LayerKind]) -> String {
    use LayerKind::*;
    match kinds {
        [Points] => tl!("Add a layer of points to answer this.").to_string(),
        [Lines] => tl!("Add a layer of lines to answer this.").to_string(),
        [Areas] => tl!("Add a layer of areas to answer this.").to_string(),
        [Raster] => tl!("Add a layer of cells to answer this.").to_string(),
        [Points, Lines, Areas] => tl!("Add a layer of points or lines or areas to answer this.").to_string(),
        // No action asks for another set today.
        _ => {
            let words: Vec<&str> = kinds.iter().map(|k| k.nouns().1).collect();
            format!("Add a layer of {} to answer this.", words.join(" or "))
        }
    }
}

/// "All 1,248 points": the scope choice for every object of the layer.
fn all_text(kind: LayerKind, n: u64) -> String {
    let template = match kind {
        LayerKind::Points => tn!("All {n} points"),
        LayerKind::Lines => tn!("All {n} lines"),
        LayerKind::Areas => tn!("All {n} areas"),
        LayerKind::Raster => tn!("All {n} cells"),
        LayerKind::Table => tn!("All {n} rows"),
    };
    i18n::fmt(template, &[("n", &i18n::count(n))])
}

fn finish_plan(c: &Composer, inputs: Vec<lm_jobs::actions::PlanInput>, params: BTreeMap<String, String>) -> Result<Plan, String> {
    let spec = lm_actions::registry().action(&c.action);
    Ok(Plan {
        action: c.action.clone(),
        inputs,
        params,
        method: spec.and_then(|s| s.method.clone()).unwrap_or_default(),
        computed_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
        software: lm_core::brand::with_version(env!("CARGO_PKG_VERSION")),
    })
}

/// The cost estimate for a plan (WP4.3): decides automatic preview.
fn estimate(plan: &Plan) -> lm_jobs::jobs::Estimate {
    let kind = match plan.action.as_str() {
        "count_in" | "count_here" => ActionKind::CountPointsInAreas,
        "which_container" => ActionKind::WhichArea,
        "whats_inside" | "summarize_data_here" => ActionKind::WhatsInside,
        "buffer" => ActionKind::Buffer,
        "measure_area" | "measure_length" => ActionKind::Measure,
        "summarize_each" | "summarize_here" | "summarize_drawn" => ActionKind::Summarize,
        "add_raster_values" => ActionKind::SampleRaster,
        _ => ActionKind::Rank,
    };
    let mut s = InputSummary::default();
    for (i, input) in plan.inputs.iter().enumerate() {
        let (features, vertices, cells) = match &input.data {
            lm_jobs::import::LayerData::Vector(ds) => {
                let n = input.rows.as_ref().map_or(ds.features.len(), Vec::len) as u64;
                // A rough vertex count: 1 per point, 50 per line or area.
                let per = if input.layer.profile.kind == LayerKind::Points { 1 } else { 50 };
                (n, n * per, 0)
            }
            lm_jobs::import::LayerData::Raster(r) => (0, 0, u64::from(r.width) * u64::from(r.height)),
        };
        if i == 0 {
            s.features = features;
            s.vertices = vertices;
        } else {
            s.against_features = features;
            s.against_vertices = vertices;
        }
        s.cells += cells;
    }
    lm_jobs::jobs::estimate(kind, &s)
}

fn plan_key(c: &Composer) -> String {
    format!("{:?}|{:?}|{:?}|{}|{:?}", c.companion, c.params, c.row, c.selected_only, c.selected_rows)
}

/// Reruns the preview when the plan changed and it is quick (or `force`); otherwise marks the
/// preview out of date.
fn refresh(shell: &mut Shell, force: bool) {
    let Some(c) = shell.composer.as_ref() else { return };
    let key = plan_key(c);
    if !force && key == c.ran_key {
        return;
    }
    let p = plan(shell, c);
    let Some(c) = shell.composer.as_mut() else { return };
    match p {
        Err(e) => {
            c.error = Some(e);
            c.result = None;
            c.ran_key = key;
        }
        Ok(p) => {
            let est = estimate(&p);
            if est.auto_preview || force {
                c.error = None;
                c.stale = false;
                c.ran_key = key;
                let generation = shell.ws.run_plan(p);
                if let Some(c) = shell.composer.as_mut() {
                    c.generation = Some(generation);
                }
            } else {
                c.stale = true;
            }
        }
    }
}

/// Each frame: takes a finished run for the composer.
pub fn poll(shell: &mut Shell) {
    let Some((generation, out)) = shell.ws.poll_plan() else { return };
    let Some(c) = shell.composer.as_mut() else { return };
    if c.generation != Some(generation) {
        return;
    }
    match out {
        Ok(r) => {
            shell.ws.set_highlight(r.highlight.clone());
            c.result = Some(r);
            c.error = None;
        }
        Err(e) => {
            c.result = None;
            c.error = Some(match e {
                ActionError::Failed(m) => {
                    i18n::fmt("The question could not be answered. Nothing was added to your map. ({reason})", &[("reason", &m)])
                }
                other => other.to_string(),
            });
        }
    }
}

/// Closes the composer: cancels any run and drops the preview.
pub fn close(shell: &mut Shell) {
    shell.ws.cancel_plan();
    shell.ws.set_highlight(None);
    shell.composer = None;
}

/// Keeps the current preview as a result (one undo step).
pub fn keep(shell: &mut Shell) -> bool {
    let Some(c) = shell.composer.as_ref() else { return false };
    let Some(r) = c.result.clone() else { return false };
    let main = c.main;
    let card_only = matches!(r.output, ActionOutput::Card);
    let Some(id) = shell.ws.keep_result(&r, Some(main)) else { return false };
    if card_only {
        if let Some(c) = shell.composer.as_mut() {
            c.kept = Some(id);
        }
    } else {
        shell.ws.set_highlight(None);
        shell.composer = None;
    }
    true
}

/// The composer, in place of the lower inspector (the card header stays).
pub fn composer(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let Some(c) = shell.composer.as_ref() else { return };
    let kinds = companion_kinds(&c.action, l.profile.kind);
    let action = c.action.clone();
    let caption = if shell.pro() { tl!("Analysis") } else { tl!("Developer surface · assistant action") };
    ui.label(RichText::new(caption).size(CAPTION).color(t.text_secondary));
    ui.add_space(2.0);
    // The sentence with its tokens.
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&c.label).font(semibold(theme::HEADLINE)));
    });
    ui.add_space(6.0);
    let mut changed = false;
    // Companion token.
    if !kinds.is_empty() {
        let choices: Vec<(LayerId, String)> = shell
            .ws
            .doc
            .layers
            .iter()
            .rev()
            .filter(|o| o.id != l.id && kinds.contains(&o.profile.kind))
            .map(|o| (o.id, o.display_name.clone()))
            .collect();
        let current =
            c.companion.and_then(|id| choices.iter().find(|(i, _)| *i == id)).map_or(tl!("Choose a layer").to_string(), |(_, n)| n.clone());
        let mut pick = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new(tl!("With")).color(t.text_secondary));
            egui::ComboBox::from_id_salt("lm-composer-companion").selected_text(current).show_ui(ui, |ui| {
                for (id, name) in &choices {
                    if ui.selectable_label(c.companion == Some(*id), name).clicked() {
                        pick = Some(*id);
                    }
                }
            });
        });
        if let Some(p) = pick
            && let Some(c) = shell.composer.as_mut()
        {
            c.companion = Some(p);
            changed = true;
        }
    }
    // Parameter tokens.
    if action == "buffer"
        && let Some(c) = shell.composer.as_mut()
    {
        let mut d = c.params.get("distance").cloned().unwrap_or_default();
        let mut merge = c.params.get("merge").map(String::as_str) != Some("no");
        ui.horizontal(|ui| {
            ui.label(RichText::new(tl!("Distance")).color(t.text_secondary));
            if ui.add(egui::TextEdit::singleline(&mut d).desired_width(90.0)).changed() {
                c.params.insert("distance".into(), d.clone());
                changed = true;
            }
            if ui.checkbox(&mut merge, tl!("Merge overlaps")).changed() {
                c.params.insert("merge".into(), if merge { "yes" } else { "no" }.into());
                changed = true;
            }
        });
    }
    if action == "rank_by_value" {
        let fields: Vec<String> = l
            .profile
            .fields
            .iter()
            .filter(|f| matches!(l.role(f).value, lm_core::model::FieldRole::Measure(_)))
            .map(|f| f.name.clone())
            .collect();
        let Some(c) = shell.composer.as_mut() else { return };
        let current = c.params.get("field").cloned();
        let shown = current.clone().unwrap_or_else(|| tl!("Choose a value").into());
        ui.horizontal(|ui| {
            ui.label(RichText::new(tl!("By")).color(t.text_secondary));
            egui::ComboBox::from_id_salt("lm-composer-field").selected_text(&shown).show_ui(ui, |ui| {
                for f in &fields {
                    if ui.selectable_label(current.as_deref() == Some(f), f).clicked() {
                        c.params.insert("field".into(), f.clone());
                        changed = true;
                    }
                }
            });
        });
    }
    // "Find areas where…": one row per condition (raster, test, value), and Add condition.
    if action == "find_where"
        && let Some(c) = shell.composer.as_mut()
    {
        let rasters: Vec<(LayerId, String)> = shell
            .ws
            .doc
            .layers
            .iter()
            .rev()
            .filter(|o| o.profile.kind == LayerKind::Raster)
            .map(|o| (o.id, o.display_name.clone()))
            .collect();
        let mut rows: Vec<Vec<String>> = c
            .params
            .get("conditions")
            .map(String::as_str)
            .unwrap_or("")
            .split(';')
            .filter(|i| !i.trim().is_empty())
            .map(|i| i.split('|').map(str::to_string).collect())
            .collect();
        let tests = [("at_least", tn!("≥  at least")), ("at_most", tn!("≤  at most")), ("between", tn!("[ ]  between"))];
        let mut remove = None;
        let mut edited = false;
        for (k, row) in rows.iter_mut().enumerate() {
            row.resize(4, String::new());
            ui.horizontal_wrapped(|ui| {
                let name = rasters
                    .iter()
                    .find(|(id, _)| id.0.to_string() == row[0])
                    .map_or(tl!("Choose a raster").to_string(), |(_, n)| n.clone());
                egui::ComboBox::from_id_salt(("lm-where-raster", k)).selected_text(name).show_ui(ui, |ui| {
                    for (id, n) in &rasters {
                        if ui.selectable_label(id.0.to_string() == row[0], n).clicked() {
                            row[0] = id.0.to_string();
                            edited = true;
                        }
                    }
                });
                let shown = tests.iter().find(|(t, _)| *t == row[1]).map_or(tests[0].1, |(_, w)| *w);
                egui::ComboBox::from_id_salt(("lm-where-test", k)).selected_text(tl!(shown)).show_ui(ui, |ui| {
                    for (t, w) in tests {
                        if ui.selectable_label(row[1] == t, tl!(w)).clicked() {
                            row[1] = t.to_string();
                            edited = true;
                        }
                    }
                });
                if ui.add(egui::TextEdit::singleline(&mut row[2]).desired_width(64.0).hint_text(tl!("a number"))).changed() {
                    edited = true;
                }
                if row[1] == "between" {
                    ui.label(tl!("and"));
                    if ui.add(egui::TextEdit::singleline(&mut row[3]).desired_width(64.0).hint_text(tl!("a number"))).changed() {
                        edited = true;
                    }
                }
                if k > 0 && ui.small_button("×").on_hover_text(tl!("Remove this condition")).clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            rows.remove(k);
            edited = true;
        }
        if ui.link(RichText::new(tl!("+ Add condition")).size(CAPTION)).clicked() {
            let first = rasters.first().map(|(id, _)| id.0.to_string()).unwrap_or_default();
            rows.push(vec![first, "at_least".into(), String::new(), String::new()]);
            edited = true;
        }
        if edited {
            c.params.insert("conditions".into(), rows.iter().map(|r| r.join("|")).collect::<Vec<_>>().join(";"));
            changed = true;
        }
    }
    // "Add a field calculated from two values": first value, operation, second value.
    if action == "calculate_field"
        && let Some(c) = shell.composer.as_mut()
    {
        let numeric: Vec<String> = l
            .profile
            .fields
            .iter()
            .filter(|f| matches!(l.role(f).value, lm_core::model::FieldRole::Measure(_)))
            .map(|f| f.name.clone())
            .collect();
        if !c.params.contains_key("a")
            && let Some(f) = numeric.first()
        {
            c.params.insert("a".into(), f.clone());
        }
        let a_now = c.params.get("a").cloned().unwrap_or_default();
        let op_now = c.params.get("op").cloned().unwrap_or_else(|| "subtract".into());
        let mut b_now = c.params.get("b").cloned().unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("lm-composer-calc-a")
                .selected_text(if a_now.is_empty() { tl!("Choose a value").into() } else { a_now.clone() })
                .show_ui(ui, |ui| {
                    for f in &numeric {
                        if ui.selectable_label(a_now == *f, f).clicked() {
                            c.params.insert("a".into(), f.clone());
                            changed = true;
                        }
                    }
                });
            let sign = |k: &str| lm_jobs::actions::FIELD_OPS.iter().find(|(id, _, _)| *id == k).map_or("−", |(_, _, s)| *s);
            egui::ComboBox::from_id_salt("lm-composer-calc-op").width(44.0).selected_text(sign(&op_now)).show_ui(ui, |ui| {
                for (id, words, sym) in lm_jobs::actions::FIELD_OPS {
                    if ui.selectable_label(op_now == *id, format!("{sym}  {words}")).clicked() {
                        c.params.insert("op".into(), id.to_string());
                        changed = true;
                    }
                }
            });
            // The second value: type a number, or pick a field.
            if ui.add(egui::TextEdit::singleline(&mut b_now).desired_width(90.0).hint_text(tl!("a number"))).changed() {
                c.params.insert("b".into(), b_now.clone());
                changed = true;
            }
            ui.menu_button(RichText::new(tl!("or a field")).size(CAPTION), |ui| {
                for f in &numeric {
                    if ui.button(f).clicked() {
                        c.params.insert("b".into(), f.clone());
                        changed = true;
                        ui.close();
                    }
                }
            });
        });
    }
    // The statistic: raster summaries (mean by default) and counts (a count, or a statistic of
    // one of the points' numeric values).
    let stats: &[(&str, &str)] = match action.as_str() {
        "summarize_each" | "summarize_here" | "summarize_drawn" => lm_jobs::actions::RASTER_STATS,
        "count_in" => lm_jobs::actions::POINT_STATS,
        _ => &[],
    };
    if !stats.is_empty()
        && let Some(c) = shell.composer.as_ref()
    {
        let default = if action == "count_in" { "count" } else { "mean" };
        let current = c.params.get("stat").cloned().unwrap_or_else(|| default.to_string());
        let label = |id: &str| stats.iter().find(|(k, _)| *k == id).map_or(id.to_string(), |(_, l)| l.to_string());
        // The points layer of a count, for the value to take the statistic of.
        let points_layer = if action == "count_in" {
            [Some(c.main), c.companion]
                .into_iter()
                .flatten()
                .find(|id| shell.ws.doc.layer(*id).is_some_and(|l| l.profile.kind == LayerKind::Points))
        } else {
            None
        };
        let numeric: Vec<String> = points_layer
            .and_then(|id| shell.ws.doc.layer(id))
            .map(|pl| {
                pl.profile
                    .fields
                    .iter()
                    .filter(|f| matches!(pl.role(f).value, lm_core::model::FieldRole::Measure(_)))
                    .map(|f| f.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut pick_stat = None;
        let mut pick_field = None;
        let field_now = c.params.get("field").cloned();
        ui.horizontal(|ui| {
            ui.label(RichText::new(tl!("Statistic")).color(t.text_secondary));
            egui::ComboBox::from_id_salt("lm-composer-stat").selected_text(label(&current)).show_ui(ui, |ui| {
                for (id, name) in stats {
                    // A statistic of values needs a numeric field on the points.
                    let usable = action != "count_in" || *id == "count" || !numeric.is_empty();
                    if ui.add_enabled(usable, egui::Button::selectable(current == *id, *name)).clicked() {
                        pick_stat = Some(id.to_string());
                    }
                }
            });
            if action == "count_in" && current != "count" {
                ui.label(RichText::new(tl!("of")).color(t.text_secondary));
                let shown = field_now.clone().unwrap_or_else(|| tl!("Choose a value").into());
                egui::ComboBox::from_id_salt("lm-composer-stat-field").selected_text(shown).show_ui(ui, |ui| {
                    for f in &numeric {
                        if ui.selectable_label(field_now.as_deref() == Some(f), f).clicked() {
                            pick_field = Some(f.clone());
                        }
                    }
                });
            }
        });
        if let Some(c) = shell.composer.as_mut() {
            if let Some(st) = pick_stat {
                if st != "count"
                    && action == "count_in"
                    && c.params.get("field").is_none_or(|f| !numeric.contains(f))
                    && let Some(f) = numeric.first()
                {
                    c.params.insert("field".into(), f.clone());
                }
                c.params.insert("stat".into(), st);
                changed = true;
            }
            if let Some(f) = pick_field {
                c.params.insert("field".into(), f);
                changed = true;
            }
        }
    }
    // Scope line.
    if let Some(c) = shell.composer.as_mut() {
        let n = shell.ws.doc.layer(c.main).map_or(0, |l| l.profile.count);
        if c.selected_rows.len() > 1 {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tl!("Scope")).color(t.text_secondary));
                if ui.selectable_label(!c.selected_only, all_text(l.profile.kind, n)).clicked() {
                    c.selected_only = false;
                    changed = true;
                }
                if ui
                    .selectable_label(c.selected_only, i18n::fmt("{n} selected", &[("n", &i18n::count(c.selected_rows.len() as u64))]))
                    .clicked()
                {
                    c.selected_only = true;
                    changed = true;
                }
            });
        }
    }
    if changed {
        refresh(shell, false);
    }
    ui.add_space(8.0);
    let Some(c) = shell.composer.as_ref() else { return };
    let running = shell.ws.plan_running();
    let w = ui.available_width();
    if running {
        let p = shell.ws.plan_progress();
        ui.horizontal(|ui| {
            ui.spinner();
            let words = p.map_or(tl!("Working…").to_string(), |p| match p.total {
                Some(total) => i18n::fmt(
                    "{phase}: {done} of {total}",
                    &[("phase", &p.phase), ("done", &i18n::count(p.done)), ("total", &i18n::count(total))],
                ),
                None => i18n::fmt("{phase}…", &[("phase", &p.phase)]),
            });
            ui.label(RichText::new(words).size(CAPTION).color(t.text_secondary));
        });
        // Esc never cancels a job; its own button does (spec › Keyboard).
        if ui.button(tl!("Cancel")).clicked() {
            shell.ws.cancel_plan();
            if let Some(c) = shell.composer.as_mut() {
                c.generation = None;
                c.ran_key.clear();
            }
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        return;
    }
    if let Some(e) = &c.error {
        ui.label(RichText::new(e).color(t.warning));
        ui.add_space(6.0);
    }
    if let Some(id) = c.kept {
        let record = shell.ws.doc.results.iter().find(|r| r.id == id).cloned();
        if let Some(r) = record {
            ui.label(RichText::new(tl!("Kept")).size(CAPTION).color(t.text_secondary));
            crate::a11y::live(
                ui.ctx(),
                egui::Id::new("lm-result-live"),
                &crate::a11y::spoken(&i18n::fmt("Kept: {title}. {summary}", &[("title", &r.card.title), ("summary", &r.card.summary)])),
            );
            result_card(shell, ui, t, &r.card, &r.recipe.text(), Some(&r), false);
        }
        if ui.button(tl!("Done")).clicked() {
            close(shell);
        }
        return;
    }
    match c.result.clone() {
        Some(r) if !c.stale => {
            ui.label(RichText::new(&r.card.summary).font(semibold(BODY)));
            crate::a11y::live(
                ui.ctx(),
                egui::Id::new("lm-result-live"),
                &crate::a11y::spoken(&i18n::fmt("Preview: {summary}", &[("summary", &r.card.summary)])),
            );
            if shell.ws.has_preview() {
                ui.label(RichText::new(tl!("Preview — not saved")).size(CAPTION).color(t.accent));
            }
            ui.add_space(6.0);
            let mut keep_clicked = false;
            ui.horizontal(|ui| {
                keep_clicked = ui.add(egui::Button::new(RichText::new(tl!("Keep result")).color(t.on_accent)).fill(t.accent)).clicked();
                if ui.button(tl!("Discard")).clicked() {
                    close(shell);
                }
            });
            if keep_clicked {
                keep(shell);
                return;
            }
            ui.add_space(8.0);
            result_card(shell, ui, t, &r.card, &r.recipe.text(), None, false);
        }
        _ => {
            if c.stale {
                ui.label(RichText::new(tl!("Preview out of date")).size(CAPTION).color(t.warning));
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut preview = false;
            ui.horizontal(|ui| {
                preview = ui
                    .add_sized(
                        vec2((w - 8.0) / 2.0, 30.0),
                        egui::Button::new(RichText::new(tl!("Preview")).color(t.on_accent)).fill(t.accent),
                    )
                    .clicked();
                if ui.add_sized(vec2((w - 8.0) / 2.0, 30.0), egui::Button::new(tl!("Cancel"))).clicked() {
                    close(shell);
                }
            });
            if preview || enter {
                refresh(shell, true);
            }
        }
    }
}

/// A result card (spec › Result card): title, answer, context, notices, buttons, recipe.
pub fn result_card(
    shell: &mut Shell,
    ui: &mut Ui,
    t: &Theme,
    card: &ResultCard,
    recipe: &str,
    kept: Option<&lm_core::recipe::ResultRecord>,
    in_ask: bool,
) {
    ui.label(RichText::new(&card.title).font(semibold(theme::HEADLINE)));
    ui.add_space(4.0);
    let all_key = egui::Id::new(("lm-result-all", &card.title));
    match &card.answer {
        Answer::Ranking { rows, unit } => {
            let show_all = ui.data(|d| d.get_temp::<bool>(all_key)).unwrap_or(false);
            // Bar length by size, so negative values (cold temperatures) still show.
            let max = rows.iter().filter_map(|r| r.value.map(f64::abs)).fold(0.0_f64, f64::max);
            let w = ui.available_width();
            for (k, r) in rows.iter().take(if show_all { usize::MAX } else { 5 }).enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), egui::Sense::hover());
                // Drawn, so read out as text: "1. Pudong: 34".
                let value = r.value.map_or_else(|| tl!("No data").to_string(), |v| i18n::number(&number(v)));
                let rank = (k + 1).to_string();
                let spoken = i18n::fmt(
                    "{rank}. {label}: {value} {unit}",
                    &[("rank", &rank), ("label", &r.label), ("value", &value), ("unit", unit)],
                );
                crate::a11y::text(&resp, &crate::a11y::spoken(&spoken));
                let p = ui.painter();
                let label_w = w * 0.42;
                // The label is clipped to its column so it never runs into the bar.
                let label_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + label_w - 8.0, rect.max.y));
                p.with_clip_rect(label_rect).text(
                    rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    middle(&r.label, 40),
                    theme::regular(CAPTION),
                    t.text_primary,
                );
                let bar_max = w - label_w - 64.0;
                if let Some(v) = r.value {
                    let bw = if max > 0.0 { (v.abs() / max).clamp(0.0, 1.0) as f32 * bar_max } else { 0.0 };
                    let bar = egui::Rect::from_min_size(egui::pos2(rect.left() + label_w, rect.center().y - 6.0), vec2(bw.max(1.0), 12.0));
                    p.rect_filled(bar, 2.0, t.accent);
                    p.text(
                        egui::pos2(bar.right() + 6.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        // A ranking by a year field reads "2013" (its unit is the field's name).
                        if lm_core::format::is_year(unit, v) { lm_core::format::year(v) } else { i18n::number(&number(v)) },
                        theme::regular(CAPTION),
                        t.text_secondary,
                    );
                } else {
                    p.text(
                        egui::pos2(rect.left() + label_w, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        tl!("No data"),
                        theme::regular(CAPTION),
                        t.text_secondary,
                    );
                }
            }
            if rows.len() > 5 {
                let text = if show_all {
                    tl!("Show fewer").to_string()
                } else {
                    i18n::fmt("Show all {n}", &[("n", &i18n::count(rows.len() as u64))])
                };
                if ui.link(RichText::new(text).size(CAPTION)).clicked() {
                    ui.data_mut(|d| d.insert_temp(all_key, !show_all));
                }
            }
            if !unit.is_empty() {
                ui.label(RichText::new(i18n::fmt("Values: {unit}", &[("unit", unit)])).size(CAPTION).color(t.text_secondary));
            }
        }
        Answer::Number { value, unit, detail } => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::number(&number(*value))).font(semibold(theme::BIG_NUMBER)));
                ui.label(RichText::new(unit).font(semibold(theme::HEADLINE)).color(t.text_secondary));
            });
            ui.label(RichText::new(detail).color(t.text_secondary));
        }
        Answer::List { heading, items } => {
            ui.label(RichText::new(heading).font(semibold(BODY)));
            for i in items {
                ui.label(i);
            }
        }
        Answer::None => {}
    }
    for n in &card.notices {
        ui.add_space(4.0);
        ui.label(RichText::new(n).size(CAPTION).color(t.warning));
    }
    ui.add_space(6.0);
    // In an Ask answer the inputs and scope lines are bookkeeping: the step list already says
    // what was filtered, so they go under "How this was calculated" (exclusions stay in view).
    let (shown_context, in_recipe): (Vec<&String>, Vec<&String>) = card.context.iter().partition(|c| !in_ask || c.starts_with("Excluded:"));
    for c in shown_context {
        ui.label(RichText::new(c).size(CAPTION).color(t.text_secondary));
    }
    if let Some(r) = kept {
        let status = shell.ws.result_status(r);
        if let Some(s) = status {
            ui.label(RichText::new(s).size(CAPTION).color(t.warning));
        }
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(RichText::new(tl!("How this was calculated")).size(CAPTION)).id_salt(("lm-recipe", &card.title)).show(
        ui,
        |ui| {
            for c in &in_recipe {
                ui.label(RichText::new(*c).size(CAPTION).color(t.text_secondary));
            }
            ui.label(RichText::new(recipe).size(CAPTION).family(egui::FontFamily::Proportional));
            if ui.small_button(tl!("Copy")).clicked() {
                ui.ctx().copy_text(recipe.to_string());
            }
        },
    );
}

fn middle(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let head: String = s.chars().take(n - 1).collect();
    format!("{head}…")
}

/// Kept results on a layer's overview: the card of the result that made the layer, and cards
/// about the layer.
pub fn kept_cards(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let mut records: Vec<lm_core::recipe::ResultRecord> = shell.ws.result_for_layer(l.id).cloned().into_iter().collect();
    records.extend(shell.ws.cards_about(l.id).into_iter().cloned());
    for r in records {
        egui::Frame::new().fill(t.surface_raised).corner_radius(theme::RADIUS_CARD).inner_margin(12.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            result_card(shell, ui, t, &r.card, &r.recipe.text(), Some(&r), false);
        });
        ui.add_space(8.0);
    }
}
