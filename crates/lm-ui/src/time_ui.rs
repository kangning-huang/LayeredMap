//! The time slider (WP6.12): shown at the bottom of the map, between the side panels, while a
//! visible layer has a time axis (a NetCDF grid). Back and forward step once; Play steps about
//! twice a second (looping, and waiting for each step's cells); the track drags; ← and → step
//! while the map has the keyboard. In Pro it sits just above the Ask field.
//!
//! The slider drives one layer: the selected one when it has time, else the topmost visible
//! layer with time. Its step is document state ([`lm_core::document::DocLayer::time`]); steps
//! taken one after another fold into one undo step.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;

use lm_core::model::LayerId;

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, Theme};

/// Steps per second while playing.
pub const PLAY_RATE: f64 = 2.0;
/// The widest the bar gets (the Ask field's width).
pub const MAX_WIDTH: f32 = 620.0;
/// The bar's height.
pub const HEIGHT: f32 = 48.0;

/// Play's state: the layer it plays and when the next step is due (seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playing {
    pub layer: LayerId,
    pub next_at: f64,
}

/// The layer the slider drives: the selected layer when it is visible and has time, else the
/// topmost visible layer with time.
pub fn time_layer(shell: &Shell) -> Option<LayerId> {
    let has_time = |l: &lm_core::document::DocLayer| l.visible && l.profile.grid.as_ref().is_some_and(|g| g.has_time());
    if let Some(l) = shell.ws.selected().and_then(|id| shell.ws.doc.layer(id)).filter(|l| has_time(l)) {
        return Some(l.id);
    }
    shell.ws.doc.layers.iter().rev().find(|l| has_time(l)).map(|l| l.id)
}

/// Each frame, before the map: ← and → step the time layer when nothing else has the keyboard
/// (they no longer pan the map left and right then; ⌥← and ⌥→ still do), and Play advances.
pub fn frame(shell: &mut Shell, ctx: &egui::Context) {
    let Some(id) = time_layer(shell) else {
        shell.playing = None;
        return;
    };
    if ctx.memory(|m| m.focused().is_none()) {
        let (left, right) = ctx.input_mut(|i| {
            if i.modifiers.is_none() {
                (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
            } else {
                (false, false)
            }
        });
        if left || right {
            shell.playing = None;
            shell.ws.step_time(id, if right { 1 } else { -1 }, false);
        }
    }
    let Some(p) = shell.playing else { return };
    if p.layer != id {
        shell.playing = None;
        return;
    }
    let now = ctx.input(|i| i.time);
    let steps = shell.ws.doc.layer(id).and_then(|l| l.profile.grid.as_ref()).map_or(1, |g| g.steps());
    let time = shell.ws.doc.layer(id).map_or(0, |l| l.time);
    // Read the next step ahead, so playing doesn't wait on the file.
    shell.ws.prefetch(id, (time + 1) % steps.max(1));
    if now >= p.next_at && shell.ws.time_ready(id) {
        shell.ws.step_time(id, 1, true);
        shell.playing = Some(Playing { layer: id, next_at: now + 1.0 / PLAY_RATE });
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(50));
}

/// The bar on its own, when the Ask box isn't shown: bottom centre of the map between the side
/// panels.
pub fn bar(shell: &mut Shell, ui: &mut Ui, map: Rect) {
    if time_layer(shell).is_none() {
        return;
    }
    let (left, right) = shell.panel_insets();
    let x0 = map.left() + left + theme::PANEL_MARGIN;
    let x1 = map.right() - right - theme::PANEL_MARGIN;
    let width = (x1 - x0).min(MAX_WIDTH);
    if width < 240.0 {
        return;
    }
    let bottom = pos2((x0 + x1) / 2.0, map.bottom() - 34.0);
    egui::Area::new(egui::Id::new("lm-time")).order(egui::Order::Foreground).pivot(Align2::CENTER_BOTTOM).fixed_pos(bottom).show(
        ui.ctx(),
        |ui| {
            ui.set_width(width);
            controls(shell, ui, width);
        },
    );
}

/// A round button with an icon (back, Play, forward): 32 × 32, the icon in the text colour,
/// a soft fill on hover, the accent's weak fill while `on`.
fn round_button(ui: &mut Ui, t: &Theme, glyph: &str, tip: &str, on: bool, enabled: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if on {
        p.circle_filled(r.center(), 16.0, t.accent_weak);
    } else if enabled && resp.hovered() {
        p.circle_filled(r.center(), 16.0, t.surface_raised);
    }
    let color = if enabled { t.text_primary } else { t.text_secondary.gamma_multiply(0.6) };
    p.text(r.center(), Align2::CENTER_CENTER, glyph, theme::icon_font(18.0), color);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
    resp.on_hover_text(tip)
}

/// The bar's contents: back, Play, forward, the track, and the step's date with the variable.
pub fn controls(shell: &mut Shell, ui: &mut Ui, width: f32) {
    let t = *shell.theme();
    let Some(id) = time_layer(shell) else { return };
    let Some(l) = shell.ws.doc.layer(id).cloned() else { return };
    let Some(g) = l.profile.grid.clone() else { return };
    let steps = g.steps();
    let time = l.time.min(steps.saturating_sub(1));
    let frame = egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(theme::RADIUS_PANEL))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .shadow(if t.dark {
            egui::epaint::Shadow::NONE
        } else {
            egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(36) }
        });
    let playing = shell.playing.is_some_and(|p| p.layer == id);
    frame.show(ui, |ui| {
        ui.set_width(width - 16.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if round_button(ui, &t, icon::CARET_LEFT, "Previous step (←)", false, time > 0).clicked() {
                shell.playing = None;
                shell.ws.step_time(id, -1, false);
            }
            let (glyph, tip) = if playing { (icon::PAUSE, "Pause") } else { (icon::PLAY, "Play") };
            if round_button(ui, &t, glyph, tip, playing, true).clicked() {
                let now = ui.input(|i| i.time);
                shell.playing = if playing { None } else { Some(Playing { layer: id, next_at: now + 1.0 / PLAY_RATE }) };
                if playing {
                    shell.ws.end_gesture();
                }
            }
            if round_button(ui, &t, icon::CARET_RIGHT, "Next step (→)", false, time + 1 < steps).clicked() {
                shell.playing = None;
                shell.ws.step_time(id, 1, false);
            }
            ui.add_space(10.0);
            // The date and variable take a fixed column at the right; the track fills the rest.
            let label_w = 150.0;
            let track_w = (ui.available_width() - label_w - 12.0).max(60.0);
            if let Some(to) = track(ui, &t, track_w, time, steps) {
                shell.playing = None;
                shell.ws.set_time(id, to);
            }
            ui.add_space(12.0);
            let (r, _) = ui.allocate_exact_size(vec2(label_w, 36.0), Sense::hover());
            let date = g.date_label(time);
            let p = ui.painter();
            p.text(pos2(r.left(), r.top() + 1.0), Align2::LEFT_TOP, &date, theme::semibold(BODY), t.text_primary);
            let caption = format!(
                "{} · {} of {}",
                short(&l.display_name, 14),
                lm_core::model::group_thousands(time as u64 + 1),
                lm_core::model::group_thousands(steps as u64)
            );
            p.text(pos2(r.left(), r.bottom() - 1.0), Align2::LEFT_BOTTOM, caption, theme::regular(CAPTION), t.text_secondary);
        });
    });
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>()) }
}

/// The track: a 4 px rail, accent up to the thumb, a 16 px thumb; ticks for up to 60 steps.
/// A click or drag picks the nearest step. Returns the step picked.
fn track(ui: &mut Ui, t: &Theme, width: f32, time: usize, steps: usize) -> Option<usize> {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click_and_drag());
    let inset = 8.0;
    let (x0, x1) = (r.left() + inset, r.right() - inset);
    let y = r.center().y;
    let n = steps.max(2) - 1;
    let x_of = |i: usize| x0 + (x1 - x0) * i as f32 / n as f32;
    let p = ui.painter();
    let rail = Rect::from_min_max(pos2(x0, y - 2.0), pos2(x1, y + 2.0));
    p.rect_filled(rail, CornerRadius::same(2), t.border);
    let at = x_of(time);
    p.rect_filled(Rect::from_min_max(rail.min, pos2(at, rail.max.y)), CornerRadius::same(2), t.accent);
    if steps <= 60 {
        for i in 0..steps {
            let x = x_of(i);
            let c = if i <= time { t.accent } else { t.control_stroke };
            p.line_segment([pos2(x, y + 6.0), pos2(x, y + 9.0)], Stroke::new(1.0, c));
        }
    }
    let hovered = resp.hovered() || resp.dragged();
    if !t.dark {
        p.circle_filled(pos2(at, y + 1.0), 8.5, Color32::from_black_alpha(30));
    }
    p.circle(pos2(at, y), 8.0, Color32::WHITE, Stroke::new(1.0, if hovered { t.accent } else { t.control_stroke }));
    resp.widget_info(|| egui::WidgetInfo::slider(true, time as f64 + 1.0, "Time step"));
    let pointer = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged());
    let picked = pointer.map(|pos| (((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0) * n as f32).round() as usize);
    picked.filter(|s| *s != time && *s < steps)
}
