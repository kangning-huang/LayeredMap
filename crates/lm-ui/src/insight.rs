//! The insight strip and suggested actions of the inspector (Insight cards and suggested actions).
//!
//! Insight rows use only attributes the layer already has; ranks come from indexes built in the
//! background (`lm_jobs::inspect::LayerIndex`), so a card appears at once. Suggested actions come
//! from the action registry (`lm-actions`); the free app shows only display-only actions
//! (Product tiers).

use egui::{RichText, Ui, vec2};

use lm_actions::{Context, FieldSummary, OpenLayer, Options, Selection, Suggested};
use lm_core::document::DocLayer;
use lm_core::model::LayerId;

use crate::Shell;
use crate::theme::{BODY, CAPTION, Theme};

/// The action context for the current selection.
pub fn context(shell: &Shell) -> Context {
    let n = shell.ws.doc.layers.len();
    let layers: Vec<OpenLayer> = shell
        .ws
        .doc
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let fields = l
                .profile
                .fields
                .iter()
                .map(|f| {
                    let k = l.role(f);
                    let mut s = FieldSummary::new(f.name.clone(), k.value, k.confidence);
                    s.starred = l.starred.contains(&f.name);
                    s
                })
                .collect();
            let mut o = OpenLayer::new(l.id, l.display_name.clone(), l.profile.kind, n - 1 - i, l.id.0).with_fields(fields);
            if !l.visible {
                o = o.hidden();
            }
            o
        })
        .collect();
    let sel = &shell.ws.selection;
    let selection = if shell.ws.doc.layers.is_empty() { Selection::Nothing } else { Selection::from_selection_context(sel) };
    Context::new(selection, layers).with_co_selected(sel.layers.clone())
}

/// Up to three suggested actions as full-width buttons, then More actions…, always last.
pub fn actions(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer, row: Option<usize>) {
    let ctx = context(shell);
    // Analysis actions are never buttons (Product tiers): the free app shows display actions only.
    let s = lm_actions::suggest(&ctx, Options::free(shell.flags.phase()));
    // When the row's own actions are all for later phases, the first display actions under More
    // take their place, so a card never has an empty action area.
    let mut top: Vec<Suggested> = s.top.clone();
    if top.is_empty() {
        top = s.more.iter().flat_map(|(_, v)| v.iter().cloned()).take(2).collect();
    }
    let more: Vec<Suggested> =
        s.more.iter().flat_map(|(_, v)| v.iter().cloned()).filter(|m| !top.iter().any(|x| x.action_id == m.action_id)).collect();
    if top.is_empty() && more.is_empty() {
        return;
    }
    ui.add_space(10.0);
    let w = ui.available_width();
    let mut run: Option<Suggested> = None;
    for a in &top {
        if ui.add_sized(vec2(w, 32.0), egui::Button::new(RichText::new(&a.label).size(BODY))).clicked() {
            run = Some(a.clone());
        }
    }
    if !more.is_empty() {
        ui.menu_button(RichText::new("More actions…").color(t.accent), |ui| {
            for a in &more {
                if ui.button(&a.label).clicked() {
                    run = Some(a.clone());
                    ui.close();
                }
            }
        });
    }
    if let Some(a) = run {
        perform(shell, l.id, row, &a);
    }
}

/// Runs a display-only action (none creates new data).
pub fn perform(shell: &mut Shell, id: LayerId, row: Option<usize>, a: &Suggested) {
    let Some(l) = shell.ws.doc.layer(id).cloned() else { return };
    match a.action_id.as_str() {
        "open_file" => shell.request_open(),
        "try_example" => shell.open_examples(),
        "style_by_value" | "raster_colors" => {
            shell.ws.select_layer(id);
            shell.inspector_tab = 1;
            if l.style.color_by.is_none()
                && l.style.size_by.is_none()
                && let Some((_, s)) =
                    lm_jobs::inspect::presets(&l).into_iter().find(|(_, s)| s.color_by.as_ref().is_some_and(|c| !c.categories))
            {
                let tx = shell.ws.doc.set_style_tx(id, s);
                shell.ws.commit(tx);
            }
        }
        "filter" => {
            // Filter by [Type] presets the builder to that field and the object's value.
            shell.table_for = Some(id);
            shell.filter_draft = Some(crate::table::FilterDraft::preset(shell, &l, a.field.as_deref(), row));
        }
        "view_data" => shell.table_for = Some(id),
        "compare_peers" => {
            shell.compare_peers = Some((id, a.field.clone().or_else(|| first_measure(&l))));
        }
        "threshold" => {
            let v = shell.ws.selection.pixel.as_ref().and_then(|p| p.value).or_else(|| shell.raster_range(id).map(|(a, b)| (a + b) / 2.0));
            let mut s = l.style.clone();
            s.above = v;
            let tx = shell.ws.doc.set_style_tx(id, s);
            shell.ws.commit(tx);
            shell.inspector_tab = 1;
        }
        "similar_values" => {
            if let Some(v) = shell.ws.selection.pixel.as_ref().and_then(|p| p.value) {
                let (lo, hi) = shell.raster_range(id).unwrap_or((v - 1.0, v + 1.0));
                let band = ((hi - lo) * 0.05).abs().max(f64::EPSILON);
                let mut s = l.style.clone();
                s.like = Some((v - band, v + band));
                let tx = shell.ws.doc.set_style_tx(id, s);
                shell.ws.commit(tx);
            }
        }
        _ => {}
    }
}

fn first_measure(l: &DocLayer) -> Option<String> {
    l.profile.fields.iter().find(|f| matches!(l.role(f).value, lm_core::model::FieldRole::Measure(_))).map(|f| f.name.clone())
}

/// The insight strip: up to three rows for the selected object.
/// Returns the fields shown.
pub fn strip(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer, ds: &lm_jobs::import::Dataset, row: usize) -> Vec<String> {
    let Some(index) = shell.ws.layer_index(l.id) else {
        ui.label(RichText::new("Working out how this compares…").size(CAPTION).color(t.text_secondary));
        return Vec::new();
    };
    let rows = lm_jobs::inspect::insight_rows(l, ds, &index, row, 3);
    for r in &rows {
        insight_row(ui, t, r);
    }
    rows.into_iter().map(|r| r.field).collect()
}

fn insight_row(ui: &mut Ui, t: &Theme, r: &lm_jobs::inspect::InsightRow) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - 128.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&r.field).color(t.text_secondary));
                ui.label(RichText::new(&r.value).font(crate::theme::semibold(BODY)));
            });
            if let Some(p) = &r.phrase {
                ui.label(RichText::new(p).size(CAPTION));
            }
            if let Some(c) = &r.comparison {
                ui.label(RichText::new(c).size(CAPTION).color(t.text_secondary));
            }
        });
        if let Some(strip) = &r.strip {
            dot_strip(ui, t, strip);
        }
    });
}

/// A 120 px dot strip: every value as a faint tick, the median as a short line, this object as
/// an accent dot.
fn dot_strip(ui: &mut Ui, t: &Theme, s: &lm_jobs::inspect::Strip) {
    let (r, _) = ui.allocate_exact_size(vec2(120.0, 24.0), egui::Sense::hover());
    let p = ui.painter();
    let x = |v: f32| r.left() + 4.0 + v.clamp(0.0, 1.0) * (r.width() - 8.0);
    let tick = t.text_secondary.gamma_multiply(0.35);
    for v in &s.ticks {
        p.line_segment([egui::pos2(x(*v), r.center().y - 4.0), egui::pos2(x(*v), r.center().y + 4.0)], egui::Stroke::new(1.0, tick));
    }
    if let Some(m) = s.median {
        p.line_segment(
            [egui::pos2(x(m), r.center().y - 8.0), egui::pos2(x(m), r.center().y + 8.0)],
            egui::Stroke::new(1.5, t.text_primary),
        );
    }
    if let Some(v) = s.value {
        p.circle(egui::pos2(x(v), r.center().y), 4.5, t.accent, egui::Stroke::new(1.5, t.surface));
    }
}
