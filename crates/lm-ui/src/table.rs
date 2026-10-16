//! The table drawer (WP3.6) and the filter builder (WP3.7).
//!
//! The drawer shows one layer's records: virtualized rows, identifier columns pinned left,
//! numbers right-aligned, blanks as "—" so blank and zero stay distinct. Selection is shared
//! with the map both ways. Clicking a header sorts the view, never the data. Filters are
//! sentences ("Show schools where Type is Primary") that become chips; a filtered layer shows a
//! funnel on its shelf row.

use std::sync::Arc;

use egui::{Align2, RichText, Sense, Ui, vec2};
use egui_phosphor::regular as icon;

use lm_core::document::DocLayer;
use lm_core::filter::{Condition, Filter, Test};
use lm_core::model::{DataType, FieldRole, LayerId};
use lm_core::selection::FeatureRef;
use lm_jobs::inspect as data;

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, PANEL_MARGIN, Theme};

/// Drawer height (px).
pub const DRAWER_HEIGHT: f32 = 300.0;
const ROW_H: f32 = 26.0;
const COL_W: f32 = 140.0;

/// The kind of condition a field offers (Filter builder › Conditions by type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Number,
    Date,
    Category,
}

/// A condition being written in the filter builder.
#[derive(Clone, Debug, PartialEq)]
pub struct FilterDraft {
    pub layer: LayerId,
    pub field: String,
    pub kind: FieldKind,
    /// The verb chosen ("is", "above"…).
    pub verb: &'static str,
    pub a: String,
    pub b: String,
    /// Categories ticked for "is any of".
    pub any_of: Vec<String>,
}

fn field_kind(l: &DocLayer, field: &str) -> FieldKind {
    let Some(f) = l.field(field) else { return FieldKind::Text };
    match (l.role(f).value, f.dtype) {
        (FieldRole::Category, _) => FieldKind::Category,
        (FieldRole::Date, _) | (_, DataType::Date) => FieldKind::Date,
        (FieldRole::Measure(_), _) | (_, DataType::Integer | DataType::Float) => FieldKind::Number,
        _ => FieldKind::Text,
    }
}

fn verbs(kind: FieldKind) -> &'static [&'static str] {
    match kind {
        FieldKind::Text => &["is", "is not", "contains", "is empty"],
        FieldKind::Number => &["above", "below", "between", "is empty"],
        FieldKind::Date => &["before", "after", "between"],
        FieldKind::Category => &["is any of"],
    }
}

impl FilterDraft {
    /// A draft for a field (the first category or measure when none is given), preset to the
    /// selected object's value when there is one ("Filter by [Type]").
    pub fn preset(shell: &Shell, l: &DocLayer, field: Option<&str>, row: Option<usize>) -> FilterDraft {
        let field = field
            .map(String::from)
            .or_else(|| l.profile.fields.iter().find(|f| l.role(f).value == FieldRole::Category).map(|f| f.name.clone()))
            .or_else(|| l.profile.fields.iter().find(|f| matches!(l.role(f).value, FieldRole::Measure(_))).map(|f| f.name.clone()))
            .or_else(|| l.profile.fields.first().map(|f| f.name.clone()))
            .unwrap_or_default();
        let kind = field_kind(l, &field);
        let value = row.and_then(|r| {
            let ds = crate::inspect::dataset(shell, l.id)?;
            let fi = data::field_index(&ds, &field)?;
            data::cell_text(&ds, r, fi)
        });
        let mut d = FilterDraft {
            layer: l.id,
            field,
            kind,
            verb: verbs(kind).first().copied().unwrap_or("is"),
            a: String::new(),
            b: String::new(),
            any_of: Vec::new(),
        };
        if let Some(v) = value {
            match kind {
                FieldKind::Category => d.any_of = vec![v],
                _ => d.a = v.replace(',', ""),
            }
        }
        d
    }

    /// The condition, when the draft is complete.
    pub fn condition(&self) -> Option<Condition> {
        let num = |s: &str| s.trim().replace(',', "").parse::<f64>().ok();
        let test = match (self.kind, self.verb) {
            (_, "is empty") => Test::IsEmpty,
            (FieldKind::Category, _) => {
                if self.any_of.is_empty() {
                    return None;
                }
                Test::AnyOf(self.any_of.clone())
            }
            (_, "is") if !self.a.trim().is_empty() => Test::Is(self.a.trim().into()),
            (_, "is not") if !self.a.trim().is_empty() => Test::IsNot(self.a.trim().into()),
            (_, "contains") if !self.a.trim().is_empty() => Test::Contains(self.a.trim().into()),
            (FieldKind::Number, "above") => Test::Above(num(&self.a)?),
            (FieldKind::Number, "below") => Test::Below(num(&self.a)?),
            (FieldKind::Number, "between") => Test::Between(num(&self.a)?, num(&self.b)?),
            (FieldKind::Date, "before") if !self.a.trim().is_empty() => Test::Before(self.a.trim().into()),
            (FieldKind::Date, "after") if !self.a.trim().is_empty() => Test::After(self.a.trim().into()),
            (FieldKind::Date, "between") if !self.a.trim().is_empty() && !self.b.trim().is_empty() => {
                Test::DateBetween(self.a.trim().into(), self.b.trim().into())
            }
            _ => return None,
        };
        Some(Condition { field: self.field.clone(), test })
    }
}

/// The cached row order of the open table.
#[derive(Default)]
pub struct TableView {
    key: String,
    pub rows: Arc<Vec<usize>>,
    pub sort: Option<(usize, bool)>,
    pub search: String,
    /// The selection last scrolled to, so a selection made on the map brings its row into view.
    scrolled_for: Vec<usize>,
    /// Rows being worked out for a key.
    computing: Option<(String, std::sync::mpsc::Receiver<Vec<usize>>)>,
}

impl TableView {
    /// Rows are being worked out.
    pub fn busy(&self) -> bool {
        self.computing.is_some()
    }
}

/// The drawer along the bottom of the map, between the shelf and the inspector.
pub fn drawer(shell: &mut Shell, ui: &mut Ui, map: egui::Rect, left: f32, right: f32) {
    let Some(id) = shell.table_for else { return };
    let Some(l) = shell.ws.doc.layer(id).cloned() else {
        shell.table_for = None;
        return;
    };
    let t = *shell.theme();
    let Some(ds) = crate::inspect::dataset(shell, id) else {
        shell.table_for = None;
        return;
    };
    let rect = egui::Rect::from_min_max(
        egui::pos2(map.left() + left + PANEL_MARGIN, map.bottom() - DRAWER_HEIGHT - PANEL_MARGIN),
        egui::pos2(map.right() - right - PANEL_MARGIN, map.bottom() - PANEL_MARGIN),
    );
    if rect.width() < 240.0 {
        return;
    }
    // The row order for the current filter, sort and search.
    let mask = shell.ws.derived(id).and_then(|d| d.mask.clone());
    let key = format!(
        "{}|{}|{:?}|{:?}|{}|{}",
        id.0,
        l.profile.data_rev,
        l.filter,
        shell.table.sort,
        shell.table.search,
        mask.as_ref().map_or(0, |m| m.len())
    );
    if shell.table.key != key && shell.table.computing.as_ref().is_none_or(|(k, _)| *k != key) {
        // Worked out off the interface thread: sorting a million rows takes a few hundred ms.
        let (tx, rx) = std::sync::mpsc::channel();
        let (ds2, mask2, sort, q) = (ds.clone(), mask.clone(), shell.table.sort, shell.table.search.trim().to_lowercase());
        let wake = shell.ws.wake.clone();
        let spawned = std::thread::Builder::new().name("lm-table".into()).spawn(move || {
            let mut rows = data::table_rows(&ds2, mask2.as_deref().map(|m| m.as_slice()), sort);
            if !q.is_empty() {
                rows.retain(|r| (0..ds2.fields.len()).any(|f| data::cell_text(&ds2, *r, f).is_some_and(|v| v.to_lowercase().contains(&q))));
            }
            let _ = tx.send(rows);
            if let Some(w) = wake {
                w();
            }
        });
        if spawned.is_ok() {
            shell.table.computing = Some((key.clone(), rx));
        }
    }
    if let Some((k, rx)) = &shell.table.computing
        && let Ok(rows) = rx.try_recv()
    {
        shell.table.key = k.clone();
        shell.table.rows = Arc::new(rows);
        shell.table.computing = None;
    }
    // A different layer's rows never show while this one's are worked out.
    if !shell.table.key.starts_with(&format!("{}|", id.0)) {
        shell.table.rows = Arc::new(Vec::new());
    }
    let rows = shell.table.rows.clone();
    let cols = data::table_columns(&l, &ds);
    let selected: Vec<usize> = shell.ws.selection.rows_in(id);
    let mut close = false;
    let mut select: Option<(usize, bool)> = None;
    let mut sort_click: Option<usize> = None;
    let mut remove_condition: Option<usize> = None;
    let mut open_builder = false;
    egui::Area::new(egui::Id::new("lm-table")).order(egui::Order::Middle).fixed_pos(rect.min).show(ui.ctx(), |ui| {
        egui::Frame::new()
            .fill(t.surface)
            .stroke(egui::Stroke::new(1.0, t.border))
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_PANEL))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(rect.width() - 24.0);
                ui.set_height(rect.height() - 24.0);
                // Header.
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&l.display_name).font(theme::semibold(BODY)));
                    let total = ds.features.len() as u64;
                    ui.label(
                        RichText::new(format!(
                            "Showing {} of {}",
                            lm_core::model::group_thousands(rows.len() as u64),
                            lm_core::model::group_thousands(total)
                        ))
                        .color(t.text_secondary),
                    );
                    if let Some(f) = &l.filter {
                        for (k, c) in f.conditions.iter().enumerate() {
                            let chip = egui::Button::new(RichText::new(format!("{}  ×", c.chip())).size(CAPTION))
                                .fill(t.accent_weak)
                                .corner_radius(egui::CornerRadius::same(10));
                            if ui.add(chip).on_hover_text("Remove this filter").clicked() {
                                remove_condition = Some(k);
                            }
                        }
                    }
                    if ui.button(format!("{} Filter", icon::PLUS)).clicked() {
                        open_builder = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new(icon::X).font(theme::icon_font(14.0))).frame(false))
                            .on_hover_text("Close")
                            .clicked()
                        {
                            close = true;
                        }
                        ui.add(egui::TextEdit::singleline(&mut shell.table.search).hint_text("Search").desired_width(160.0));
                    });
                });
                ui.separator();
                // Columns: a header row, then virtualized rows, scrolled together sideways.
                let ncols = cols.len();
                egui::ScrollArea::horizontal().id_salt(("lm-table-h", id.0)).show(ui, |ui| {
                    let width = COL_W * ncols as f32 + 8.0;
                    ui.set_min_width(width);
                    ui.horizontal(|ui| {
                        for &c in &cols {
                            let name = ds.fields.get(c).map(|f| f.name.clone()).unwrap_or_default();
                            let arrow = match shell.table.sort {
                                Some((f, true)) if f == c => " ↑",
                                Some((f, false)) if f == c => " ↓",
                                _ => "",
                            };
                            let (r, resp) = ui.allocate_exact_size(vec2(COL_W, ROW_H), Sense::click());
                            ui.painter().text(
                                r.left_center() + vec2(6.0, 0.0),
                                Align2::LEFT_CENTER,
                                truncate(&format!("{name}{arrow}"), 18),
                                theme::semibold(CAPTION),
                                t.text_primary,
                            );
                            if resp.on_hover_text(format!("Sort by {name}")).clicked() {
                                sort_click = Some(c);
                            }
                        }
                    });
                    let scroll = egui::ScrollArea::vertical().id_salt(("lm-table-v", id.0)).auto_shrink([false, false]);
                    // Bring a selection made on the map into view.
                    let scroll = if selected != shell.table.scrolled_for {
                        shell.table.scrolled_for = selected.clone();
                        match selected.first().and_then(|s| rows.iter().position(|r| r == s)) {
                            Some(pos) => scroll.vertical_scroll_offset((pos as f32 * ROW_H - ROW_H * 3.0).max(0.0)),
                            None => scroll,
                        }
                    } else {
                        scroll
                    };
                    scroll.show_rows(ui, ROW_H, rows.len(), |ui, range| {
                        for i in range {
                            let Some(&r) = rows.get(i) else { continue };
                            let on = selected.contains(&r);
                            let (rr, resp) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
                            let p = ui.painter();
                            if on {
                                p.rect_filled(rr, 4.0, t.accent_weak);
                            } else if resp.hovered() {
                                p.rect_filled(rr, 4.0, t.surface_raised);
                            } else if i % 2 == 1 {
                                p.rect_filled(rr, 0.0, t.surface_raised.gamma_multiply(0.5));
                            }
                            for (k, &c) in cols.iter().enumerate() {
                                let x = rr.left() + k as f32 * COL_W;
                                let numeric = ds.fields.get(c).is_some_and(|f| f.ty.is_numeric());
                                match data::cell_text(&ds, r, c) {
                                    Some(v) => {
                                        let v = truncate(&v, 20);
                                        if numeric {
                                            p.text(
                                                egui::pos2(x + COL_W - 10.0, rr.center().y),
                                                Align2::RIGHT_CENTER,
                                                v,
                                                theme::regular(CAPTION),
                                                t.text_primary,
                                            );
                                        } else {
                                            p.text(
                                                egui::pos2(x + 6.0, rr.center().y),
                                                Align2::LEFT_CENTER,
                                                v,
                                                theme::regular(CAPTION),
                                                t.text_primary,
                                            );
                                        }
                                    }
                                    None => {
                                        let at = if numeric {
                                            egui::pos2(x + COL_W - 10.0, rr.center().y)
                                        } else {
                                            egui::pos2(x + 6.0, rr.center().y)
                                        };
                                        let align = if numeric { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
                                        p.text(at, align, "—", theme::regular(CAPTION), t.text_secondary);
                                    }
                                }
                            }
                            if resp.clicked() {
                                select = Some((r, ui.input(|i| i.modifiers.shift)));
                            }
                        }
                    });
                });
            });
    });
    if close {
        shell.table_for = None;
    }
    if let Some(c) = sort_click {
        shell.table.sort = match shell.table.sort {
            Some((f, true)) if f == c => Some((c, false)),
            Some((f, false)) if f == c => None,
            _ => Some((c, true)),
        };
    }
    if let Some((r, shift)) = select {
        let f = FeatureRef { layer: id, row: r };
        if shift {
            shell.ws.selection.toggle(f);
        } else {
            shell.ws.selection = lm_core::selection::SelectionContext::object(f);
        }
        shell.table.scrolled_for = shell.ws.selection.rows_in(id);
    }
    if let Some(k) = remove_condition
        && let Some(mut f) = l.filter.clone()
    {
        if k < f.conditions.len() {
            f.conditions.remove(k);
        }
        let to = (!f.conditions.is_empty()).then_some(f);
        let tx = shell.ws.doc.set_filter_tx(id, to);
        shell.ws.commit(tx);
    }
    if open_builder && shell.filter_draft.is_none() {
        shell.filter_draft = Some(FilterDraft::preset(shell, &l, None, None));
    }
    builder(shell, ui, rect);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { s.chars().take(max - 1).collect::<String>() + "…" }
}

/// The filter builder: "Show [schools] where [Type] [is] [Primary]", with a live count.
fn builder(shell: &mut Shell, ui: &mut Ui, drawer: egui::Rect) {
    let Some(mut d) = shell.filter_draft.clone() else { return };
    let Some(l) = shell.ws.doc.layer(d.layer).cloned() else {
        shell.filter_draft = None;
        return;
    };
    let Some(ds) = crate::inspect::dataset(shell, d.layer) else { return };
    let t: Theme = *shell.theme();
    let mut apply = false;
    let mut cancel = false;
    egui::Area::new(egui::Id::new("lm-filter-builder"))
        .order(egui::Order::Foreground)
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(drawer.left_top() + vec2(0.0, -8.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(560.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Show");
                    ui.label(RichText::new(data::noun_plural(&l)).strong());
                    ui.label("where");
                    let before = d.field.clone();
                    egui::ComboBox::from_id_salt("lm-filter-field").selected_text(&d.field).show_ui(ui, |ui| {
                        for f in &l.profile.fields {
                            ui.selectable_value(&mut d.field, f.name.clone(), &f.name);
                        }
                    });
                    if d.field != before {
                        d.kind = field_kind(&l, &d.field);
                        d.verb = verbs(d.kind).first().copied().unwrap_or("is");
                        d.a.clear();
                        d.b.clear();
                        d.any_of.clear();
                    }
                    egui::ComboBox::from_id_salt("lm-filter-verb").selected_text(d.verb).show_ui(ui, |ui| {
                        for v in verbs(d.kind) {
                            ui.selectable_value(&mut d.verb, *v, *v);
                        }
                    });
                    match (d.kind, d.verb) {
                        (_, "is empty") => {}
                        (FieldKind::Category, _) => {
                            let fi = data::field_index(&ds, &d.field);
                            let values = fi.map(|i| ds.unique_values(i, 50)).unwrap_or_default();
                            ui.menu_button(if d.any_of.is_empty() { "choose…".to_string() } else { d.any_of.join(", ") }, |ui| {
                                for v in values {
                                    let mut on = d.any_of.contains(&v);
                                    if ui.checkbox(&mut on, &v).changed() {
                                        if on {
                                            d.any_of.push(v.clone());
                                        } else {
                                            d.any_of.retain(|x| *x != v);
                                        }
                                    }
                                }
                            });
                        }
                        (_, "between") => {
                            ui.add(egui::TextEdit::singleline(&mut d.a).desired_width(80.0));
                            ui.label("and");
                            ui.add(egui::TextEdit::singleline(&mut d.b).desired_width(80.0));
                        }
                        _ => {
                            ui.add(egui::TextEdit::singleline(&mut d.a).desired_width(140.0));
                        }
                    }
                });
                // Live count over the whole layer with the existing filter and this condition.
                let cond = d.condition();
                let count = cond.as_ref().map(|c| {
                    let mut f = l.filter.clone().unwrap_or_default();
                    f.conditions.push(c.clone());
                    data::filter_mask(&ds, Some(&f)).map_or(ds.features.len(), |m| m.iter().filter(|k| **k).count())
                });
                ui.horizontal(|ui| {
                    match count {
                        Some(n) => {
                            ui.label(RichText::new(format!("{} match", lm_core::model::group_thousands(n as u64))).color(t.text_secondary))
                        }
                        None => ui.label(RichText::new("Finish the sentence to see how many match.").color(t.text_secondary)),
                    };
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(cond.is_some(), egui::Button::new(RichText::new("Apply").color(t.on_accent)).fill(t.accent))
                            .clicked()
                        {
                            apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            });
        });
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if apply && let Some(c) = d.condition() {
        let mut f: Filter = l.filter.clone().unwrap_or_default();
        f.conditions.push(c);
        let tx = shell.ws.doc.set_filter_tx(d.layer, Some(f));
        shell.ws.commit(tx);
        shell.filter_draft = None;
    } else if cancel {
        shell.filter_draft = None;
    } else {
        shell.filter_draft = Some(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(kind: FieldKind, verb: &'static str, a: &str, b: &str) -> FilterDraft {
        FilterDraft { layer: LayerId(1), field: "f".into(), kind, verb, a: a.into(), b: b.into(), any_of: vec![] }
    }

    #[test]
    fn drafts_become_conditions_only_when_complete() {
        assert_eq!(draft(FieldKind::Number, "above", "500,000", "").condition().map(|c| c.test), Some(Test::Above(500_000.0)));
        assert_eq!(draft(FieldKind::Number, "above", "", "").condition(), None);
        assert_eq!(draft(FieldKind::Number, "between", "1", "").condition(), None);
        assert_eq!(draft(FieldKind::Text, "is empty", "", "").condition().map(|c| c.test), Some(Test::IsEmpty));
        assert_eq!(draft(FieldKind::Category, "is any of", "", "").condition(), None);
        let mut d = draft(FieldKind::Category, "is any of", "", "");
        d.any_of = vec!["Primary".into()];
        assert_eq!(d.condition().map(|c| c.chip()), Some("f is Primary".into()));
    }
}
