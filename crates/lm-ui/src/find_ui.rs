//! Find in a layer (WP3.10): one search field at the top of the layer inspector, styled like the
//! toolbar search. Typing selects the matching records on the map; the panel lists the first
//! eight with the matched field in bold. Once typing pauses the map zooms to the matches, and
//! choosing one in the list zooms to it; Return zooms to all again; Esc clears.

use std::sync::Arc;

use egui::{Align2, CornerRadius, Rect, RichText, Sense, Ui, pos2, vec2};
use egui_phosphor::regular as icon;

use lm_core::document::DocLayer;
use lm_core::model::LayerId;
use lm_core::selection::{FeatureRef, SelectionContext};
use lm_jobs::find::{FindHit, match_range};
use lm_jobs::inspect as data;

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, RADIUS_CONTROL, Theme};

/// The most matches selected on the map at once (the list and the count cover all of them).
const MAX_SELECTED: usize = 10_000;
/// Matches listed in the panel.
const LISTED: usize = 8;
/// How long typing has to pause before the map zooms to the matches, so it doesn't jump about
/// with every letter.
const ZOOM_AFTER: f64 = 0.6;

#[derive(Default)]
pub struct FindState {
    pub layer: Option<LayerId>,
    pub query: String,
    pub hits: Arc<Vec<FindHit>>,
    key: String,
    /// Focus the field on the next frame (⌘F, a test).
    pub focus: bool,
    /// A match whose card is open, with a way back to the list.
    pub open_row: Option<usize>,
    /// When to zoom to the matches (the time the last change to them settles).
    zoom_at: Option<f64>,
}

impl FindState {
    /// Whether a search is running on `id`.
    pub fn active_for(&self, id: LayerId) -> bool {
        self.layer == Some(id) && !self.query.trim().is_empty()
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.hits = Arc::new(Vec::new());
        self.key.clear();
        self.open_row = None;
        self.zoom_at = None;
    }
}

/// The search field. Updates the matches and the map's selection as the text changes.
pub fn bar(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let id = l.id;
    if shell.find.layer != Some(id) {
        shell.find = FindState { layer: Some(id), focus: shell.find.focus, ..FindState::default() };
    }
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
    let field_id = egui::Id::new(("lm-find", id.0));
    let has_text = !shell.find.query.is_empty();
    let edit = egui::TextEdit::singleline(&mut shell.find.query)
        .id(field_id)
        .hint_text(format!("Find in {}", l.display_name))
        .desired_width(rect.width() - 40.0)
        .margin(egui::Margin { left: 30, right: if has_text { 96 } else { 10 }, top: 7, bottom: 7 });
    let resp = ui.put(rect, edit);
    let p = ui.painter();
    p.text(
        pos2(rect.left() + 11.0, rect.center().y),
        Align2::LEFT_CENTER,
        icon::MAGNIFYING_GLASS,
        theme::icon_font(15.0),
        t.text_secondary,
    );
    if shell.find.focus {
        resp.request_focus();
        shell.find.focus = false;
    }
    // The matches, worked out again when the text, the data or the filter changes.
    let q = shell.find.query.trim().to_string();
    let mut indexing = false;
    if q.is_empty() {
        if !shell.find.key.is_empty() {
            shell.find.clear();
            shell.ws.selection = SelectionContext::layer(id);
        }
    } else if let Some(ix) = shell.ws.find_index(id) {
        let mask = shell.ws.derived(id).and_then(|d| d.mask.clone());
        let key = format!("{q}|{}|{}|{}", l.profile.data_rev, mask.as_ref().map_or(0, |m| m.iter().filter(|v| **v).count()), ix.rows());
        if shell.find.key != key {
            let hits = ix.find(&q, mask.as_deref().map(|m| m.as_slice()));
            shell.ws.selection = SelectionContext {
                active_layer: Some(id),
                features: hits.iter().take(MAX_SELECTED).map(|h| FeatureRef { layer: id, row: h.row }).collect(),
                ..Default::default()
            };
            shell.find.hits = Arc::new(hits);
            shell.find.key = key;
            shell.find.open_row = None;
            shell.find.zoom_at = (!shell.find.hits.is_empty()).then(|| ui.input(|i| i.time) + ZOOM_AFTER);
        }
    } else {
        indexing = true;
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    }
    if let Some(at) = shell.find.zoom_at {
        let now = ui.input(|i| i.time);
        if now >= at {
            shell.find.zoom_at = None;
            zoom_to_hits(shell, id);
        } else {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(at - now));
        }
    }
    // The count and the clear button at the right of the field.
    if has_text {
        let x_rect = Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(20.0, 20.0));
        let x = ui.interact(x_rect, field_id.with("clear"), Sense::click()).on_hover_text("Clear");
        crate::a11y::button(&x, "Clear the search");
        let p = ui.painter();
        p.circle_filled(x_rect.center(), 8.0, if x.hovered() { t.text_secondary } else { t.control_stroke });
        p.text(x_rect.center(), Align2::CENTER_CENTER, icon::X, theme::icon_font(10.0), t.surface);
        let n = shell.find.hits.len();
        let count = if indexing { "…".to_string() } else { format!("{} found", lm_core::model::group_thousands(n as u64)) };
        p.text(pos2(x_rect.left() - 6.0, rect.center().y), Align2::RIGHT_CENTER, count, theme::regular(CAPTION), t.text_secondary);
        if x.clicked() {
            shell.find.clear();
            shell.ws.selection = SelectionContext::layer(id);
            resp.request_focus();
        }
    }
    let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    if (resp.has_focus() || resp.lost_focus()) && enter && !shell.find.hits.is_empty() {
        zoom_to_hits(shell, id);
    }
    if resp.has_focus() && esc {
        shell.find.clear();
        shell.ws.selection = SelectionContext::layer(id);
        resp.surrender_focus();
    }
}

fn zoom_to_hits(shell: &mut Shell, id: LayerId) {
    let Some(ds) = crate::inspect::dataset(shell, id) else { return };
    let mut ext: Option<lm_core::model::Extent> = None;
    for h in shell.find.hits.iter().take(MAX_SELECTED) {
        if let Some(e) = data::row_extent(&ds, h.row) {
            ext = Some(match ext {
                None => e,
                Some(a) => lm_core::model::Extent {
                    min_x: a.min_x.min(e.min_x),
                    min_y: a.min_y.min(e.min_y),
                    max_x: a.max_x.max(e.max_x),
                    max_y: a.max_y.max(e.max_y),
                },
            });
        }
    }
    if ext.is_some() {
        shell.zoom_to = ext;
    }
}

/// The panel body while a search runs: the first matches, or one match's card.
pub fn results(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let id = l.id;
    let Some(ds) = crate::inspect::dataset(shell, id) else { return };
    let hits = shell.find.hits.clone();
    let q = shell.find.query.trim().to_string();
    if let Some(row) = shell.find.open_row {
        if ui.link(format!("‹ All {} found", lm_core::model::group_thousands(hits.len() as u64))).clicked() {
            shell.find.open_row = None;
            shell.find.key.clear();
            return;
        }
        ui.add_space(4.0);
        ui.label(RichText::new(data::object_title(l, &ds, row)).font(theme::semibold(theme::HEADLINE)));
        ui.add_space(2.0);
        crate::inspect::object_body(shell, ui, t, l, row);
        return;
    }
    if hits.is_empty() {
        if shell.ws.find_index(id).is_some() {
            let (_, many) = l.profile.kind.nouns();
            ui.add_space(6.0);
            ui.label(RichText::new(format!("No {many} contain “{q}”.")).color(t.text_secondary));
        }
        return;
    }
    ui.add_space(4.0);
    for h in hits.iter().take(LISTED) {
        let title = data::object_title(l, &ds, h.row);
        let field = ds.fields.get(h.field).map(|f| f.name.clone()).unwrap_or_default();
        let value = data::cell_text(&ds, h.row, h.field).unwrap_or_default();
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
        crate::a11y::button(&resp, format!("{title}, {field}: {value}"));
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(RADIUS_CONTROL), t.accent_weak);
        }
        ui.painter().text(r.left_top() + vec2(8.0, 5.0), Align2::LEFT_TOP, &title, theme::regular(BODY), t.text_primary);
        // "land_use: Residential" with the match in bold.
        let mut job = egui::text::LayoutJob::default();
        let plain = |c| egui::TextFormat { font_id: theme::regular(CAPTION), color: c, ..Default::default() };
        let bold = egui::TextFormat { font_id: theme::semibold(CAPTION), color: t.text_primary, ..Default::default() };
        job.append(&format!("{field}: "), 0.0, plain(t.text_secondary));
        match match_range(&value, &q) {
            Some(m) => {
                job.append(&value[..m.start], 0.0, plain(t.text_secondary));
                job.append(&value[m.clone()], 0.0, bold);
                job.append(&value[m.end..], 0.0, plain(t.text_secondary));
            }
            None => job.append(&value, 0.0, plain(t.text_secondary)),
        }
        job.wrap.max_width = r.width() - 16.0;
        job.wrap.max_rows = 1;
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(r.left_bottom() + vec2(8.0, -5.0 - galley.size().y), galley, t.text_primary);
        if resp.clicked() {
            shell.find.open_row = Some(h.row);
            shell.find.zoom_at = None;
            shell.ws.selection = SelectionContext::object(FeatureRef { layer: id, row: h.row });
            if let Some(e) = data::row_extent(&ds, h.row) {
                shell.zoom_to = Some(e);
            }
        }
    }
    let n = hits.len();
    ui.add_space(6.0);
    let more =
        if n > LISTED { format!("{} more on the map · ", lm_core::model::group_thousands((n - LISTED) as u64)) } else { String::new() };
    ui.horizontal(|ui| {
        ui.label(RichText::new(more).size(CAPTION).color(t.text_secondary));
        if ui.link(RichText::new(if n == 1 { "Zoom to it" } else { "Zoom to all" }).size(CAPTION).color(t.accent)).clicked() {
            zoom_to_hits(shell, id);
        }
    });
}
