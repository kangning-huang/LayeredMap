//! The developer surface for the certified actions (spec › Composer, Preview, Result card).
//! After the tier decision (2026-10-16) analysis actions are never buttons in either tier: the
//! composer appears only with the developer flag (`LM_DEV=1` or a debug build), to test what
//! Pro's assistant will call. The result card and the recipe are the same either way.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{RichText, Ui, vec2};

use lm_actions::Suggested;
use lm_core::document::DocLayer;
use lm_core::model::{LayerId, LayerKind, group_thousands};
use lm_core::recipe::{Answer, ResultCard};
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
        _ => Vec::new(),
    }
}

/// Opens the composer for an analysis suggestion (developer surface), and previews at once when
/// the estimate is under a second.
pub fn open(shell: &mut Shell, main: LayerId, row: Option<usize>, a: &Suggested) {
    if !lm_jobs::actions::ACTIONS.contains(&a.action_id.as_str()) {
        shell.notice = Some((format!("“{}” isn't connected yet.", a.label), 0.0));
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
        label: lm_actions::registry().action("summarize_drawn").map_or_else(|| "Summarize in an area I draw".into(), |s| s.label.clone()),
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
        label: lm_actions::registry().action(action).map_or_else(|| action.to_string(), |s| s.label.clone()),
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
    let main = shell.ws.doc.layer(c.main).ok_or("The layer was removed.")?;
    let kind = main.profile.kind;
    let rows_main = if c.selected_only && !c.selected_rows.is_empty() { Some(c.selected_rows.clone()) } else { None };
    let one = c.row.map(|r| vec![r]);
    let needs_companion = !companion_kinds(&c.action, kind).is_empty();
    let other = match (needs_companion, c.companion) {
        (true, Some(id)) => Some(id),
        (true, None) => {
            let words: Vec<&str> = companion_kinds(&c.action, kind).iter().map(|k| k.nouns().1).collect();
            return Err(format!("Add a layer of {} to answer this.", words.join(" or ")));
        }
        (false, _) => None,
    };
    let input =
        |id: LayerId, rows: Option<Vec<usize>>| shell.ws.plan_input(id, rows).ok_or_else(|| "A layer is still loading.".to_string());
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
        _ => vec![input(c.main, rows_main.clone().or(one.clone()))?],
    };
    let spec = lm_actions::registry().action(&c.action);
    Ok(Plan {
        action: c.action.clone(),
        inputs,
        params: c.params.clone(),
        method: spec.and_then(|s| s.method.clone()).unwrap_or_default(),
        computed_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
        software: format!("LayeredMap {}", env!("CARGO_PKG_VERSION")),
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
                ActionError::Failed(m) => format!("The question could not be answered. Nothing was added to your map. ({m})"),
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
    ui.label(RichText::new("Developer surface · assistant action").size(CAPTION).color(t.text_secondary));
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
            c.companion.and_then(|id| choices.iter().find(|(i, _)| *i == id)).map_or("Choose a layer".to_string(), |(_, n)| n.clone());
        let mut pick = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("With").color(t.text_secondary));
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
            ui.label(RichText::new("Distance").color(t.text_secondary));
            if ui.add(egui::TextEdit::singleline(&mut d).desired_width(90.0)).changed() {
                c.params.insert("distance".into(), d.clone());
                changed = true;
            }
            if ui.checkbox(&mut merge, "Merge overlaps").changed() {
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
        let current = c.params.get("field").cloned().unwrap_or_else(|| "Choose a value".into());
        ui.horizontal(|ui| {
            ui.label(RichText::new("By").color(t.text_secondary));
            egui::ComboBox::from_id_salt("lm-composer-field").selected_text(&current).show_ui(ui, |ui| {
                for f in &fields {
                    if ui.selectable_label(current == *f, f).clicked() {
                        c.params.insert("field".into(), f.clone());
                        changed = true;
                    }
                }
            });
        });
    }
    // Scope line.
    if let Some(c) = shell.composer.as_mut() {
        let (_, many) = l.profile.kind.nouns();
        let n = shell.ws.doc.layer(c.main).map_or(0, |l| l.profile.count);
        if c.selected_rows.len() > 1 {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Scope").color(t.text_secondary));
                if ui.selectable_label(!c.selected_only, format!("All {} {many}", group_thousands(n))).clicked() {
                    c.selected_only = false;
                    changed = true;
                }
                if ui.selectable_label(c.selected_only, format!("{} selected", group_thousands(c.selected_rows.len() as u64))).clicked() {
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
            let words = p.map_or("Working…".to_string(), |p| match p.total {
                Some(total) => format!("{}: {} of {}", p.phase, group_thousands(p.done), group_thousands(total)),
                None => format!("{}…", p.phase),
            });
            ui.label(RichText::new(words).size(CAPTION).color(t.text_secondary));
        });
        // Esc never cancels a job; its own button does (spec › Keyboard).
        if ui.button("Cancel").clicked() {
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
            ui.label(RichText::new("Kept").size(CAPTION).color(t.text_secondary));
            result_card(shell, ui, t, &r.card, &r.recipe.text(), Some(&r));
        }
        if ui.button("Done").clicked() {
            close(shell);
        }
        return;
    }
    match c.result.clone() {
        Some(r) if !c.stale => {
            ui.label(RichText::new(&r.card.summary).font(semibold(BODY)));
            if shell.ws.has_preview() {
                ui.label(RichText::new("Preview — not saved").size(CAPTION).color(t.accent));
            }
            ui.add_space(6.0);
            let mut keep_clicked = false;
            ui.horizontal(|ui| {
                keep_clicked = ui.add(egui::Button::new(RichText::new("Keep result").color(t.on_accent)).fill(t.accent)).clicked();
                if ui.button("Discard").clicked() {
                    close(shell);
                }
            });
            if keep_clicked {
                keep(shell);
                return;
            }
            ui.add_space(8.0);
            result_card(shell, ui, t, &r.card, &r.recipe.text(), None);
        }
        _ => {
            if c.stale {
                ui.label(RichText::new("Preview out of date").size(CAPTION).color(t.warning));
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut preview = false;
            ui.horizontal(|ui| {
                preview = ui
                    .add_sized(vec2((w - 8.0) / 2.0, 30.0), egui::Button::new(RichText::new("Preview").color(t.on_accent)).fill(t.accent))
                    .clicked();
                if ui.add_sized(vec2((w - 8.0) / 2.0, 30.0), egui::Button::new("Cancel")).clicked() {
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
) {
    ui.label(RichText::new(&card.title).font(semibold(theme::HEADLINE)));
    ui.add_space(4.0);
    let all_key = egui::Id::new(("lm-result-all", &card.title));
    match &card.answer {
        Answer::Ranking { rows, unit } => {
            let show_all = ui.data(|d| d.get_temp::<bool>(all_key)).unwrap_or(false);
            let max = rows.iter().filter_map(|r| r.value).fold(0.0_f64, f64::max);
            let w = ui.available_width();
            for r in rows.iter().take(if show_all { usize::MAX } else { 5 }) {
                let (rect, _) = ui.allocate_exact_size(vec2(w, 22.0), egui::Sense::hover());
                let p = ui.painter();
                let label_w = w * 0.42;
                p.text(rect.left_center(), egui::Align2::LEFT_CENTER, middle(&r.label, 22), theme::regular(CAPTION), t.text_primary);
                let bar_max = w - label_w - 64.0;
                if let Some(v) = r.value {
                    let bw = if max > 0.0 { (v / max).clamp(0.0, 1.0) as f32 * bar_max } else { 0.0 };
                    let bar = egui::Rect::from_min_size(egui::pos2(rect.left() + label_w, rect.center().y - 6.0), vec2(bw.max(1.0), 12.0));
                    p.rect_filled(bar, 2.0, t.accent);
                    p.text(
                        egui::pos2(bar.right() + 6.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        number(v),
                        theme::regular(CAPTION),
                        t.text_secondary,
                    );
                } else {
                    p.text(
                        egui::pos2(rect.left() + label_w, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "No data",
                        theme::regular(CAPTION),
                        t.text_secondary,
                    );
                }
            }
            if rows.len() > 5 {
                let text = if show_all { "Show fewer".to_string() } else { format!("Show all {}", group_thousands(rows.len() as u64)) };
                if ui.link(RichText::new(text).size(CAPTION)).clicked() {
                    ui.data_mut(|d| d.insert_temp(all_key, !show_all));
                }
            }
            if !unit.is_empty() {
                ui.label(RichText::new(format!("Values: {unit}")).size(CAPTION).color(t.text_secondary));
            }
        }
        Answer::Number { value, unit, detail } => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(number(*value)).font(semibold(theme::BIG_NUMBER)));
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
    for c in &card.context {
        ui.label(RichText::new(c).size(CAPTION).color(t.text_secondary));
    }
    if let Some(r) = kept {
        let status = r.status(|i| {
            if i.fingerprint == "drawn" {
                return Some(i.fingerprint.clone());
            }
            let layer = shell.ws.doc.layers.iter().find(|l| l.profile.source.file_name() == i.file)?;
            lm_jobs::fingerprint::of_path(std::path::Path::new(&layer.profile.source.path))
        });
        if let Some(s) = status {
            ui.label(RichText::new(s).size(CAPTION).color(t.warning));
        }
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(RichText::new("How this was calculated").size(CAPTION)).id_salt(("lm-recipe", &card.title)).show(
        ui,
        |ui| {
            ui.label(RichText::new(recipe).size(CAPTION).family(egui::FontFamily::Proportional));
            if ui.small_button("Copy").clicked() {
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
            result_card(shell, ui, t, &r.card, &r.recipe.text(), Some(&r));
        });
        ui.add_space(8.0);
    }
}
