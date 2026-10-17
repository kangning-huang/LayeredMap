//! Joining a table (WP3.11) in the inspector: the result card on the joined layer, and the one
//! question a table asks when its places aren't clear.

use egui::{CornerRadius, RichText, Stroke, Ui};
use egui_phosphor::regular as icon;

use lm_core::document::DocLayer;
use lm_core::model::LayerId;

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, Theme};

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
        if let Some(note) = &r.rows_note {
            ui.label(RichText::new(note).size(CAPTION).color(t.text_secondary));
        }
        if !r.near_misses.is_empty() {
            ui.add_space(4.0);
            let shown: Vec<String> = r.near_misses.iter().take(3).map(|(a, b)| format!("{a} → {b}")).collect();
            let more = if r.near_misses.len() > 3 { "…" } else { "" };
            let n = r.near_misses.len();
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "{n} {}: {}{more}",
                        if n == 1 { "name looks like a typo" } else { "names look like typos" },
                        shown.join(", ")
                    ))
                    .size(CAPTION),
                )
                .wrap(),
            );
            if ui.button(if n == 1 { "Use this match" } else { "Use these matches" }).clicked() {
                shell.ws.accept_near_misses(id);
            }
        }
        let key = egui::Id::new(("lm-join-unmatched", id.0));
        let open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let u = r.unmatched.len();
            if u > 0 {
                let label = if open { "Hide the rows not matched".to_string() } else { format!("Show the {u} not matched") };
                if ui.link(RichText::new(label).size(CAPTION)).clicked() {
                    ui.data_mut(|d| d.insert_temp(key, !open));
                }
                ui.label(RichText::new("·").size(CAPTION).color(t.text_secondary));
            }
            if r.styled.is_some() && l.style.color_by.is_some() {
                if ui.link(RichText::new("Keep the original colors").size(CAPTION)).clicked() {
                    shell.ws.keep_original_colors(id);
                }
                ui.label(RichText::new("·").size(CAPTION).color(t.text_secondary));
            }
            if ui.link(RichText::new("Undo").size(CAPTION)).clicked() {
                shell.command(ui.ctx(), crate::Command::Undo);
            }
            ui.label(RichText::new("·").size(CAPTION).color(t.text_secondary));
            if ui.link(RichText::new("Done").size(CAPTION)).clicked() {
                dismiss = true;
            }
        });
        if open {
            for k in r.unmatched.iter().take(20) {
                ui.label(RichText::new(k).size(CAPTION).color(t.text_secondary));
            }
            if r.unmatched.len() > 20 {
                ui.label(RichText::new(format!("and {} more", r.unmatched.len() - 20)).size(CAPTION).color(t.text_secondary));
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
        ui.label(RichText::new("Looking for the places this table describes…").color(t.text_secondary));
        return true;
    }
    let Some(q) = shell.ws.join_question(id).cloned() else { return false };
    let key = egui::Id::new(("lm-join-choice", id.0));
    let mut choice = ui.data(|d| d.get_temp::<usize>(key)).unwrap_or(0);
    let mut answer: Option<Option<usize>> = None;
    card(ui, t, |ui| {
        ui.label(RichText::new(format!("Which places does {} describe?", q.table_name)).font(theme::semibold(BODY)));
        ui.add_space(4.0);
        for (i, o) in q.offers.iter().enumerate() {
            let target = shell.ws.doc.layer(o.target).map(|l| l.display_name.clone()).unwrap_or_default();
            let text = format!("{target}: {}  ·  {}", o.label, o.detail);
            if ui.radio(choice == i, text).clicked() {
                choice = i;
            }
        }
        if ui.radio(choice == q.offers.len(), "None of these: keep it as a table").clicked() {
            choice = q.offers.len();
        }
        ui.add_space(6.0);
        if ui.add(egui::Button::new(RichText::new("Use this").color(t.on_accent)).fill(t.accent)).clicked() {
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
