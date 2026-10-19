//! Joining a table (WP3.11) in the inspector: the result card on the joined layer, and the one
//! question a table asks when its places aren't clear.

use egui::{CornerRadius, RichText, Stroke, Ui};
use egui_phosphor::regular as icon;

use lm_core::document::DocLayer;
use lm_core::model::LayerId;

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, Theme};
use lm_core::i18n;
use lm_core::{tl, tn};

fn card<R>(ui: &mut Ui, t: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(t.surface_raised)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(theme::RADIUS_CARD))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            crate::form::with_visible_controls(ui, t, add)
        })
        .inner
}

/// "Joined population.csv to districts by name: 16 of 17 districts matched." with what can be
/// done about it.
pub fn report(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let id = l.id;
    let Some(r) = shell.ws.join_report(id).cloned() else { return };
    let mut dismiss = false;
    card(ui, t, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::LINK).font(theme::icon_font(15.0)).color(t.accent));
            ui.add(egui::Label::new(RichText::new(r.line(&l.display_name, l.profile.kind.nouns())).size(BODY)).wrap());
        });
        ui.label(RichText::new(r.how()).size(CAPTION).color(t.text_secondary));
        ui.add(egui::Label::new(RichText::new(r.expected.line(l.profile.kind.nouns())).size(CAPTION).color(t.text_secondary)).wrap());
        // The table left the shelf's top level: say where it went.
        ui.label(
            RichText::new(i18n::fmt(
                "{table} now appears under {layer} in Layers.",
                &[("table", &r.table_name), ("layer", &l.display_name)],
            ))
            .size(CAPTION)
            .color(t.text_secondary),
        );
        if let Some(note) = &r.rows_note {
            ui.label(RichText::new(note).size(CAPTION).color(t.text_secondary));
        }
        if !r.near_misses.is_empty() {
            ui.add_space(4.0);
            let shown: Vec<String> = r.near_misses.iter().take(3).map(|(a, b)| format!("{a} → {b}")).collect();
            let more = if r.near_misses.len() > 3 { "…" } else { "" };
            let n = r.near_misses.len();
            let template =
                if n == 1 { tn!("{n} name looks like a typo: {examples}") } else { tn!("{n} names look like typos: {examples}") };
            let examples = format!("{}{more}", shown.join(", "));
            ui.add(
                egui::Label::new(RichText::new(i18n::fmt(template, &[("n", &n.to_string()), ("examples", &examples)])).size(CAPTION))
                    .wrap(),
            );
            if ui.button(if n == 1 { tl!("Use this match") } else { tl!("Use these matches") }).clicked() {
                shell.ws.accept_near_misses(id);
            }
        }
        let key = egui::Id::new(("lm-join-unmatched", id.0));
        let open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(show) = r.unmatched_label() {
                let label = if open { tl!("Hide the rows not matched").to_string() } else { show };
                if ui.link(RichText::new(label).size(CAPTION)).clicked() {
                    ui.data_mut(|d| d.insert_temp(key, !open));
                }
                ui.label(RichText::new("·").size(CAPTION).color(t.text_secondary));
            }
            if r.styled.is_some() && l.style.color_by.is_some() {
                if ui.link(RichText::new(tl!("Keep the original colors")).size(CAPTION)).clicked() {
                    shell.ws.keep_original_colors(id);
                }
                ui.label(RichText::new("·").size(CAPTION).color(t.text_secondary));
            }
            if ui.link(RichText::new(tl!("Undo")).size(CAPTION)).clicked() {
                shell.command(ui.ctx(), crate::Command::Undo);
            }
            ui.label(RichText::new("·").size(CAPTION).color(t.text_secondary));
            if ui.link(RichText::new(tl!("Done")).size(CAPTION)).clicked() {
                dismiss = true;
            }
        });
        if open {
            for k in r.unmatched.iter().take(20) {
                ui.label(RichText::new(k).size(CAPTION).color(t.text_secondary));
            }
            if r.unmatched.len() > 20 {
                ui.label(
                    RichText::new(i18n::fmt("and {n} more", &[("n", &(r.unmatched.len() - 20).to_string())]))
                        .size(CAPTION)
                        .color(t.text_secondary),
                );
            }
            if r.blank_keys > 0 {
                let template = if r.blank_keys == 1 { tn!("{n} row with no {field}") } else { tn!("{n} rows with no {field}") };
                let text = i18n::fmt(template, &[("n", &r.blank_keys.to_string()), ("field", &r.table_field)]);
                ui.label(RichText::new(text).size(CAPTION).color(t.text_secondary));
            }
        }
    });
    if dismiss {
        shell.ws.dismiss_join_report(id);
    }
    ui.add_space(8.0);
}

/// On a table: the join question, or what is happening with it. Returns whether it drew.
pub fn table_status(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId) -> bool {
    if shell.ws.join_working(id) {
        ui.label(RichText::new(tl!("Looking for the places this table describes…")).color(t.text_secondary));
        return true;
    }
    let Some(q) = shell.ws.join_question(id).cloned() else { return false };
    if q.confirm {
        confirm(shell, ui, t, id, &q);
        return true;
    }
    let key = egui::Id::new(("lm-join-choice", id.0));
    let mut choice = ui.data(|d| d.get_temp::<usize>(key)).unwrap_or(0);
    let mut answer: Option<Option<usize>> = None;
    card(ui, t, |ui| {
        ui.label(RichText::new(i18n::fmt("Which places does {table} describe?", &[("table", &q.table_name)])).font(theme::semibold(BODY)));
        ui.add_space(4.0);
        for (i, o) in q.offers.iter().enumerate() {
            let target = shell.ws.doc.layer(o.target).map(|l| l.display_name.clone()).unwrap_or_default();
            let text = format!("{target}: {}  ·  {}", o.label, o.detail);
            if ui.radio(choice == i, text).clicked() {
                choice = i;
            }
            // What this answer would give, before anything is joined.
            if let Some(e) = &o.expected {
                let nouns = shell.ws.doc.layer(o.target).map_or(("record", "records"), |l| l.profile.kind.nouns());
                ui.horizontal(|ui| {
                    ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
                    ui.add(egui::Label::new(RichText::new(e.line(nouns)).size(CAPTION).color(t.text_secondary)).wrap());
                });
            }
        }
        if ui.radio(choice == q.offers.len(), tl!("None of these: keep it as a table")).clicked() {
            choice = q.offers.len();
        }
        ui.add_space(6.0);
        if ui.add(egui::Button::new(RichText::new(tl!("Use this")).color(t.on_accent)).fill(t.accent)).clicked() {
            answer = Some((choice < q.offers.len()).then_some(choice));
        }
    });
    ui.data_mut(|d| d.insert_temp(key, choice));
    if let Some(a) = answer {
        shell.ws.answer_join(id, a);
    }
    ui.add_space(8.0);
    true
}

/// A clear join whose keys repeat asks first (owner's choice, 2026-10-18): what it would give and
/// how the repeats are handled, then Join or Keep as a table.
fn confirm(shell: &mut Shell, ui: &mut Ui, t: &Theme, id: LayerId, q: &crate::workspace::JoinQuestion) {
    let Some(o) = q.offers.first().cloned() else { return };
    let (target, nouns) =
        shell.ws.doc.layer(o.target).map_or((String::new(), ("record", "records")), |l| (l.display_name.clone(), l.profile.kind.nouns()));
    let mut answer: Option<bool> = None;
    let mut pick = false;
    card(ui, t, |ui| {
        ui.add(
            egui::Label::new(
                RichText::new(i18n::fmt("Join {table} to {target}?", &[("table", &q.table_name), ("target", &target)]))
                    .font(theme::semibold(BODY)),
            )
            .wrap(),
        );
        ui.add_space(2.0);
        ui.label(RichText::new(format!("{}  ·  {}", o.label, o.detail)).size(CAPTION));
        if let Some(e) = &o.expected {
            ui.add(egui::Label::new(RichText::new(e.line(nouns)).size(CAPTION).color(t.text_secondary)).wrap());
            if let Some(h) = e.handling(nouns) {
                ui.add(egui::Label::new(RichText::new(h).size(CAPTION).color(t.text_secondary)).wrap());
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new(tl!("Join")).color(t.on_accent)).fill(t.accent)).clicked() {
                answer = Some(true);
            }
            if ui.button(tl!("Keep as a table")).clicked() {
                answer = Some(false);
            }
        });
        if ui.link(RichText::new(tl!("Choose other columns…")).size(CAPTION)).clicked() {
            pick = true;
        }
    });
    if let Some(a) = answer {
        shell.ws.answer_join(id, a.then_some(0));
    }
    if pick {
        open_picker(shell, id);
    }
    ui.add_space(8.0);
}

/// Opens "Match to a layer" for a table: the column picker in its panel.
pub fn open_picker(shell: &mut Shell, table: LayerId) {
    match shell.ws.match_draft(table) {
        Some(d) => {
            shell.ws.hold_join(table);
            shell.ws.select_layer(table);
            shell.inspector_tab = 0;
            shell.match_draft = Some(d);
        }
        None => shell.notice = Some((tl!("Open a point, line or area layer to match this table to.").into(), 0.0)),
    }
}

fn combo(ui: &mut Ui, id: egui::Id, label: &str, value: &mut String, options: &[(String, String)]) {
    ui.label(RichText::new(label).size(CAPTION));
    let shown = options.iter().find(|(k, _)| k == value).map_or(value.clone(), |(_, v)| v.clone());
    egui::ComboBox::from_id_salt(id).selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
        for (k, v) in options {
            ui.selectable_value(value, k.clone(), v);
        }
    });
}

/// "Match to a layer" (WP6.4): the layer and its key column, the table's key column (the names
/// may differ), what to do with repeats, and a live preview of how many match and the
/// cardinality before anything is joined. Join is one undo step.
pub fn picker(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    let Some(mut d) = shell.match_draft.clone().filter(|d| d.table == l.id) else { return };
    let targets: Vec<(String, String)> = shell
        .ws
        .match_targets()
        .into_iter()
        .filter_map(|id| shell.ws.doc.layer(id).map(|x| (id.0.to_string(), x.display_name.clone())))
        .collect();
    let mut target = d.target.0.to_string();
    let mut join = false;
    let mut cancel = false;
    card(ui, t, |ui| {
        ui.label(RichText::new(i18n::fmt("Match {layer} to a layer", &[("layer", &l.display_name)])).font(theme::semibold(BODY)));
        ui.add_space(4.0);
        combo(ui, egui::Id::new(("lm-match-layer", l.id.0)), tl!("Layer"), &mut target, &targets);
        if let Some(id) = shell.ws.match_targets().into_iter().find(|id| id.0.to_string() == target)
            && id != d.target
        {
            d.target = id;
            // Start from the best pairing for the new layer.
            let tds_fields = shell.ws.field_names(d.table);
            let fields = shell.ws.field_names(id);
            d.layer_field = fields.first().cloned().unwrap_or_default();
            if let Some(best) = shell.ws.match_draft(d.table).filter(|b| b.target == id) {
                d.layer_field = best.layer_field;
                d.table_field = best.table_field;
            } else if !tds_fields.contains(&d.table_field) {
                d.table_field = tds_fields.first().cloned().unwrap_or_default();
            }
        }
        let (target_name, nouns) = shell
            .ws
            .doc
            .layer(d.target)
            .map_or((String::new(), ("record", "records")), |x| (x.display_name.clone(), x.profile.kind.nouns()));
        let lf: Vec<(String, String)> = shell.ws.field_names(d.target).into_iter().map(|f| (f.clone(), f)).collect();
        let tf: Vec<(String, String)> = shell.ws.field_names(d.table).into_iter().map(|f| (f.clone(), f)).collect();
        ui.add_space(2.0);
        combo(
            ui,
            egui::Id::new(("lm-match-lf", l.id.0)),
            &i18n::fmt("Column in {layer}", &[("layer", &target_name)]),
            &mut d.layer_field,
            &lf,
        );
        ui.add_space(2.0);
        combo(
            ui,
            egui::Id::new(("lm-match-tf", l.id.0)),
            &i18n::fmt("Column in {layer}", &[("layer", &l.display_name)]),
            &mut d.table_field,
            &tf,
        );
        ui.add_space(6.0);
        let preview = shell.ws.match_preview(&d);
        match &preview {
            Ok((_, e)) => {
                ui.add(
                    egui::Label::new(
                        RichText::new(i18n::fmt(
                            "{matched} of {total} {nouns} match · {rows_matched} of {rows} rows find a place",
                            &[
                                ("matched", &i18n::count(e.layer_matched as u64)),
                                ("total", &i18n::count(e.layer_rows as u64)),
                                ("nouns", if e.layer_rows == 1 { nouns.0 } else { nouns.1 }),
                                ("rows_matched", &i18n::count(e.table_matched as u64)),
                                ("rows", &i18n::count(e.table_rows as u64)),
                            ],
                        ))
                        .size(CAPTION),
                    )
                    .wrap(),
                );
                ui.add(egui::Label::new(RichText::new(e.line(nouns)).size(CAPTION).color(t.text_secondary)).wrap());
                if let Some(h) = e.handling(nouns) {
                    ui.add(egui::Label::new(RichText::new(h).size(CAPTION).color(t.text_secondary)).wrap());
                }
            }
            Err(why) => {
                ui.add(egui::Label::new(RichText::new(why).size(CAPTION).color(t.warning)).wrap());
            }
        }
        // Several rows for one record: how to make them one.
        let repeats = preview.as_ref().map_or(d.repeats != "auto", |(_, e)| e.table_most > 1);
        if repeats {
            ui.add_space(4.0);
            ui.label(RichText::new(tl!("Several rows for one place")).size(CAPTION));
            ui.horizontal_wrapped(|ui| {
                for (k, v) in [("auto", tn!("Automatic")), ("latest", tn!("Latest year or date")), ("combine", tn!("Combine"))] {
                    if ui.radio(d.repeats == k, RichText::new(tl!(v)).size(CAPTION)).clicked() {
                        d.repeats = k.to_string();
                    }
                }
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let ok = preview.as_ref().is_ok_and(|(_, e)| e.layer_matched > 0);
            if ui.add_enabled(ok, egui::Button::new(RichText::new(tl!("Join")).color(t.on_accent)).fill(t.accent)).clicked() {
                join = true;
            }
            if ui.button(tl!("Cancel")).clicked() {
                cancel = true;
            }
        });
    });
    let done = cancel || (join && shell.ws.match_join(&d));
    shell.match_draft = (!done).then_some(d);
    ui.add_space(8.0);
}
