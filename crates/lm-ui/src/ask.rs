//! The Ask box (WP7.1, early test build for NimbleMap Pro): one field at the bottom of the map,
//! "Ask about your layers". A request goes to the assistant (`lm_ask`) on a background thread;
//! the plan comes back checked and already run on a copy of the layers. Its new layer shows over
//! the map and its filter as a highlight. The plan and its cards show above the box; **Keep**
//! adds everything as one undo step, **Discard** drops it. A question from the assistant shows
//! with its choices; the reply continues the exchange. The microphone listens with Apple's
//! speech recognizer (`crate::dictation`).

use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use egui::{Align2, RichText, Ui, vec2};
use egui_phosphor::regular as icon;

use lm_ask::session::Transport;
use lm_ask::{Answer, PlanResult, Request, Turn, Usage};
use lm_core::{i18n, tl};
use lm_jobs::jobs::Control;
use lm_jobs::plan_exec::StepOutcome;

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, HEADLINE, semibold};

const MAX_WIDTH: f32 = 620.0;

#[derive(Default)]
pub struct AskState {
    pub text: String,
    pending: Option<Pending>,
    shown: Option<Shown>,
    /// The exchange a reply continues (after the assistant asked a question).
    conversation: Vec<serde_json::Value>,
    /// Requests carried out (kept) in this session, for "those" and "of these".
    history: Vec<String>,
    pub spent_usd: f64,
    focus: bool,
    /// Counts answers: each new one opens at the top, with rankings collapsed.
    answer_no: u64,
    drawn_no: u64,
    /// Listening (the microphone): the helper, and the field's text before speaking began.
    #[cfg(not(target_arch = "wasm32"))]
    listening: Option<(crate::dictation::Dictation, String)>,
}

struct Pending {
    rx: Receiver<Turn>,
    control: Control,
    request: String,
    started: Instant,
}

enum Shown {
    Plan { request: String, plan: Box<PlanResult>, usage: Usage, seconds: f64 },
    Question { text: String, choices: Vec<String> },
    Message { text: String, warning: bool },
}

impl AskState {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// The message shown in place of an answer (setup problems, offline mode), if any.
    pub fn message(&self) -> Option<&str> {
        match &self.shown {
            Some(Shown::Message { text, .. }) => Some(text),
            _ => None,
        }
    }

    /// Esc: stop the request in flight, or drop what is shown.
    pub fn escape(&mut self) -> bool {
        if let Some(p) = self.pending.take() {
            p.control.cancel();
            return true;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.listening.take().is_some() {
            return true;
        }
        self.shown.take().is_some()
    }
}

/// Sends `text` to the assistant.
pub fn submit(shell: &mut Shell, ctx: &egui::Context, text: String) {
    let text = text.trim().to_string();
    if text.is_empty() || shell.ask.pending.is_some() {
        return;
    }
    // Strict offline mode: say so instead of trying (the transport refuses too).
    #[cfg(not(target_arch = "wasm32"))]
    if lm_net::offline() {
        shell.ask.shown = Some(Shown::Message { text: lm_ask::openrouter::OFFLINE.into(), warning: true });
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let key = lm_ask::openrouter::read_key();
    #[cfg(target_arch = "wasm32")]
    let key: Option<String> = None;
    let Some(key) = key else {
        shell.ask.shown =
            Some(Shown::Message { text: tl!("The assistant isn't set up in this build (it has no assistant key).").into(), warning: true });
        return;
    };
    let (layers, filters) = shell.ws.ask_inputs();
    let selected = selected_object(shell);
    let previous =
        if matches!(shell.ask.shown, Some(Shown::Question { .. })) { std::mem::take(&mut shell.ask.conversation) } else { Vec::new() };
    let req = Request {
        text: text.clone(),
        layers,
        filters,
        selected,
        history: shell.ask.history.clone(),
        previous,
        model: lm_ask::MODEL.into(),
        reference: reference(),
    };
    discard(shell);
    let control = Control::default();
    let (tx, rx) = channel();
    let c = control.clone();
    let ctx = ctx.clone();
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let t = lm_ask::openrouter::OpenRouter::new(key);
        let turn = lm_ask::ask(&t as &dyn Transport, &req, &c);
        let _ = tx.send(turn);
        ctx.request_repaint();
    });
    #[cfg(target_arch = "wasm32")]
    let _ = (key, req, tx, c, ctx);
    shell.ask.pending = Some(Pending { rx, control, request: text, started: Instant::now() });
    shell.ask.text.clear();
}

/// Pro's built-in reference layers, opened once, with the borders of this Mac's region.
fn reference() -> Option<std::sync::Arc<lm_ask::reference::Reference>> {
    static REF: std::sync::OnceLock<Option<std::sync::Arc<lm_ask::reference::Reference>>> = std::sync::OnceLock::new();
    REF.get_or_init(|| {
        let dir = lm_ask::reference::default_dir()?;
        let region = lm_ask::reference::system_region();
        lm_ask::reference::Reference::open(&dir, region.as_deref()).map(std::sync::Arc::new)
    })
    .clone()
}

/// The selected object as `(layer, title field, value)`, when one object is selected.
fn selected_object(shell: &Shell) -> Option<(String, String, String)> {
    let f = shell.ws.selection.features.first()?;
    let l = shell.ws.doc.layer(f.layer)?;
    let d = shell.ws.data(f.layer)?;
    let lm_jobs::import::LayerData::Vector(ds) = &d.data else { return None };
    let k = lm_jobs::inspect::title_field(l, ds)?;
    let v = ds.value(f.row, k).to_string();
    (v != "<Null>").then(|| (l.display_name.clone(), ds.fields[k].name.clone(), v))
}

/// The answer, once it arrives.
pub fn poll(shell: &mut Shell) {
    let Some(turn) = shell.ask.pending.as_ref().and_then(|p| p.rx.try_recv().ok()) else { return };
    let Some(p) = shell.ask.pending.take() else { return };
    let seconds = p.started.elapsed().as_secs_f64();
    shell.ask.spent_usd += turn.usage.cost_usd;
    log::info!(
        "ask: {} model call(s), {} + {} tokens, US${:.4}, {seconds:.1} s",
        turn.usage.calls,
        turn.usage.prompt_tokens,
        turn.usage.completion_tokens,
        turn.usage.cost_usd
    );
    if p.control.is_cancelled() {
        return;
    }
    shell.ask.conversation.clear();
    shell.ask.answer_no += 1;
    shell.ask.shown = Some(match turn.answer {
        Ok(Answer::Plan(plan)) => {
            shell.ws.preview_plan(Some(&plan));
            Shown::Plan { request: p.request, plan, usage: turn.usage, seconds }
        }
        Ok(Answer::Question { text, choices }) => {
            shell.ask.conversation = turn.messages;
            shell.ask.focus = true;
            Shown::Question { text, choices }
        }
        // The assistant's own words stay as it wrote them; only the frame around them is translated.
        Ok(Answer::CannotDo(text)) => {
            Shown::Message { text: i18n::fmt("I can't answer that with these layers: {text}", &[("text", &text)]), warning: false }
        }
        Err(e) => Shown::Message { text: e, warning: true },
    });
}

fn discard(shell: &mut Shell) {
    if matches!(shell.ask.shown, Some(Shown::Plan { .. })) {
        shell.ws.preview_plan(None);
    }
    shell.ask.shown = None;
}

/// The microphone (the button, and scripts): starts listening, or stops it.
pub fn dictate(shell: &mut Shell, ctx: &egui::Context) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some((d, _)) = &mut shell.ask.listening {
            d.stop();
            return;
        }
        let wake = ctx.clone();
        match crate::dictation::Dictation::start(move || wake.request_repaint()) {
            Ok(d) => {
                let base = shell.ask.text.trim_end().to_string();
                shell.ask.listening = Some((d, if base.is_empty() { base } else { base + " " }));
            }
            Err(e) => shell.ask.shown = Some(Shown::Message { text: e, warning: true }),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (shell, ctx);
}

/// Words heard so far go into the field; the final words end listening.
#[cfg(not(target_arch = "wasm32"))]
fn hear(shell: &mut Shell) -> bool {
    use crate::dictation::Event;
    let Some((d, base)) = &shell.ask.listening else { return false };
    let mut done = false;
    for e in d.events() {
        match e {
            Event::Ready => {}
            Event::Partial(t) => shell.ask.text = format!("{base}{t}"),
            Event::Final(t) => {
                if !t.is_empty() {
                    shell.ask.text = format!("{base}{t}");
                }
                done = true;
            }
            Event::Error(m) => {
                shell.ask.shown = Some(Shown::Message { text: m, warning: true });
                done = true;
            }
        }
    }
    if done {
        shell.ask.listening = None;
        shell.ask.focus = true;
    }
    shell.ask.listening.is_some()
}

/// Keeps the plan shown (the Keep button, and scripts).
pub fn keep_shown(shell: &mut Shell) {
    keep(shell);
}

fn keep(shell: &mut Shell) {
    let Some(Shown::Plan { request, plan, .. }) = shell.ask.shown.take() else { return };
    // i18n: an undo label, stored with the document's history
    let label = format!("Ask: {}", short(&request, 48));
    if shell.ws.keep_plan(label, &plan) {
        shell.ask.history.push(request);
    } else {
        shell.ask.shown = Some(Shown::Message { text: tl!("The plan couldn't be kept; nothing changed.").into(), warning: true });
        shell.ws.preview_plan(None);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn internal_build() -> bool {
    lm_ask::openrouter::internal_build()
}

#[cfg(target_arch = "wasm32")]
fn internal_build() -> bool {
    false
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>()) }
}

/// One step of a plan in words.
fn step_words(o: &StepOutcome) -> Option<String> {
    match o {
        // The filter's words come from `lm_ask` (English, like the plan itself).
        StepOutcome::Filtered { layer, shown, of, filter } => Some(i18n::fmt(
            "Show only {layer} where {filter}: {shown} of {of}",
            &[
                ("layer", layer),
                ("filter", &lm_ask::context::filter_text(filter)),
                ("shown", &i18n::count(*shown as u64)),
                ("of", &i18n::count(*of as u64)),
            ],
        )),
        StepOutcome::NewLayer { name, .. } => Some(i18n::fmt("New layer: {name}", &[("name", name)])),
        StepOutcome::NewFields { layer, fields, .. } => {
            Some(i18n::fmt("Add {fields} to {layer}", &[("fields", &fields.join(", ")), ("layer", layer)]))
        }
        StepOutcome::Card { card, .. } => Some(card.title.clone()),
        StepOutcome::Display | StepOutcome::Asked(_) | StepOutcome::Refused(_) => None,
    }
}

/// Whether the Ask box shows: in Pro, once a layer is open, when the map leaves room for it.
pub fn shows(shell: &Shell, map: egui::Rect) -> bool {
    // Nothing to ask about until a layer is open; the welcome card has that space.
    if !shell.pro || shell.ws.doc.layers.is_empty() {
        return false;
    }
    let (left, right) = shell.panel_insets();
    let x0 = map.left() + left + theme::PANEL_MARGIN;
    let x1 = map.right() - right - theme::PANEL_MARGIN;
    (x1 - x0).min(MAX_WIDTH) >= 240.0
}

/// The box and what it shows, at the bottom of `map` between the side panels. The time slider
/// (WP6.12), when a visible layer has time, sits between the answer and the field.
pub fn ask_box(shell: &mut Shell, ui: &mut Ui, map: egui::Rect) {
    if !shows(shell, map) {
        return;
    }
    let t = *shell.theme();
    let (left, right) = shell.panel_insets();
    let x0 = map.left() + left + theme::PANEL_MARGIN;
    let x1 = map.right() - right - theme::PANEL_MARGIN;
    let width = (x1 - x0).min(MAX_WIDTH);
    let bottom = egui::pos2((x0 + x1) / 2.0, map.bottom() - 34.0);
    egui::Area::new(egui::Id::new("lm-ask")).order(egui::Order::Foreground).pivot(Align2::CENTER_BOTTOM).fixed_pos(bottom).show(
        ui.ctx(),
        |ui| {
            ui.set_width(width);
            ui.vertical(|ui| {
                if shell.ask.shown.is_some() || shell.ask.pending.is_some() {
                    answer_panel(shell, ui, &t, width, map.height() * 0.55);
                    ui.add_space(8.0);
                }
                if crate::time_ui::time_layer(shell).is_some() {
                    crate::time_ui::controls(shell, ui, width);
                    ui.add_space(8.0);
                }
                field(shell, ui, &t, width);
            });
        },
    );
}

fn field(shell: &mut Shell, ui: &mut Ui, t: &theme::Theme, width: f32) {
    let empty = shell.ws.doc.layers.is_empty();
    let hint = if matches!(shell.ask.shown, Some(Shown::Question { .. })) {
        tl!("Type your answer, or pick one above")
    } else {
        tl!("Ask about your layers")
    };
    let frame = egui::Frame::new()
        .fill(t.surface)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(22))
        .inner_margin(egui::Margin::symmetric(14, 6))
        .shadow(if t.dark {
            egui::epaint::Shadow::NONE
        } else {
            egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: egui::Color32::from_black_alpha(36) }
        });
    frame.show(ui, |ui| {
        ui.set_width(width - 28.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::SPARKLE).font(theme::icon_font(18.0)).color(t.accent));
            let busy = shell.ask.pending.is_some();
            #[cfg(not(target_arch = "wasm32"))]
            let listening = hear(shell);
            #[cfg(target_arch = "wasm32")]
            let listening = false;
            if listening {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
            let hint = if listening { tl!("Listening… speak your request") } else { hint };
            let edit = egui::TextEdit::singleline(&mut shell.ask.text)
                .hint_text(hint)
                .frame(egui::Frame::NONE)
                .font(theme::regular(BODY))
                .desired_width(width - 28.0 - 24.0 - 2.0 * 34.0 - 16.0)
                .id(egui::Id::new("lm-ask-field"));
            let r = ui.add_enabled(!busy && !empty, edit);
            crate::a11y::label(&r, tl!("Ask about your layers"));
            if shell.ask.focus {
                r.request_focus();
                shell.ask.focus = false;
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            // While listening the microphone is red and stops listening when clicked.
            let (glyph, color, tip) = if listening {
                (icon::MICROPHONE, egui::Color32::from_rgb(255, 59, 48), tl!("Stop listening"))
            } else {
                (icon::MICROPHONE, t.text_secondary, tl!("Speak your request"))
            };
            let mic = ui
                .add_enabled(
                    !busy && !empty,
                    egui::Button::new(RichText::new(glyph).font(theme::icon_font(18.0)).color(color)).frame(false),
                )
                .on_hover_text(tip);
            crate::a11y::button(&mic, tip);
            if mic.clicked() {
                dictate(shell, ui.ctx());
            }
            let send = ui
                .add_enabled(
                    !busy && !shell.ask.text.trim().is_empty(),
                    egui::Button::new(RichText::new(icon::ARROW_CIRCLE_UP).font(theme::icon_font(24.0)).color(t.accent)).frame(false),
                )
                .on_hover_text(tl!("Ask (Return)"));
            crate::a11y::button(&send, tl!("Ask"));
            if (enter || send.clicked()) && !shell.ask.text.trim().is_empty() {
                let text = shell.ask.text.clone();
                submit(shell, ui.ctx(), text);
            }
        });
    });
}

fn answer_panel(shell: &mut Shell, ui: &mut Ui, t: &theme::Theme, width: f32, max_h: f32) {
    crate::chrome::panel_frame(t).show(ui, |ui| {
        ui.set_width(width - 2.0 * theme::PANEL_PADDING);
        if let Some(request) = shell.ask.pending.as_ref().map(|p| p.request.clone()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(i18n::fmt("Planning “{request}”…", &[("request", &short(&request, 60))])).color(t.text_secondary));
                crate::a11y::live(ui.ctx(), egui::Id::new("lm-result-live"), tl!("Planning…"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tl!("Cancel")).clicked() {
                        shell.ask.escape();
                    }
                });
            });
            return;
        }
        let mut action = None;
        let mut pick = None;
        let shown = shell.ask.shown.take();
        // A new answer starts collapsed ("Show all" off) and scrolled to its top.
        if shell.ask.drawn_no != shell.ask.answer_no {
            shell.ask.drawn_no = shell.ask.answer_no;
            if let Some(Shown::Plan { plan, .. }) = &shown {
                for o in &plan.outcomes {
                    if let StepOutcome::Card { card, .. } | StepOutcome::NewLayer { card, .. } = o {
                        ui.ctx().data_mut(|d| d.remove::<bool>(egui::Id::new(("lm-result-all", &card.title))));
                    }
                }
            }
        }
        egui::ScrollArea::vertical().id_salt(("lm-ask-answer", shell.ask.answer_no)).max_height(max_h).auto_shrink([false, true]).show(
            ui,
            |ui| match &shown {
                Some(Shown::Plan { request, plan, .. }) => {
                    ui.label(RichText::new(request).font(semibold(HEADLINE)));
                    ui.add_space(6.0);
                    let words: Vec<String> = plan.outcomes.iter().filter_map(step_words).collect();
                    for (i, w) in words.iter().enumerate() {
                        ui.label(RichText::new(format!("{}. {w}", i + 1)).size(BODY));
                    }
                    if plan.layers.iter().any(|l| l.alias.is_some()) {
                        ui.label(RichText::new(tl!("Preview on the map — not saved")).size(CAPTION).color(t.text_secondary));
                    }
                    for n in &plan.notes {
                        ui.label(RichText::new(n).size(CAPTION).color(t.warning));
                    }
                    let cards: Vec<(lm_core::recipe::ResultCard, String)> = plan
                        .outcomes
                        .iter()
                        .filter_map(|o| match o {
                            StepOutcome::Card { card, recipe, .. } => Some(((**card).clone(), recipe.text())),
                            StepOutcome::NewLayer { card, recipe, .. } => Some(((**card).clone(), recipe.text())),
                            _ => None,
                        })
                        .rev() // the last step's card is the answer: first
                        .collect();
                    // The answer is read once when it arrives: the first card's summary.
                    if let Some((card, _)) = cards.first() {
                        let said = crate::a11y::spoken(&format!("{}. {}", card.title, card.summary));
                        crate::a11y::live(ui.ctx(), egui::Id::new("lm-result-live"), &said);
                    }
                    for (card, recipe) in cards {
                        ui.add_space(10.0);
                        crate::analysis::result_card(shell, ui, t, &card, &recipe, None);
                    }
                }
                Some(Shown::Question { text, choices }) => {
                    ui.label(RichText::new(text).size(BODY));
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for c in choices {
                            if ui.button(c).clicked() {
                                pick = Some(c.clone());
                            }
                        }
                    });
                }
                Some(Shown::Message { text, warning }) => {
                    ui.label(RichText::new(text).color(if *warning { t.warning } else { t.text_primary }));
                    ui.add_space(6.0);
                    if ui.button(tl!("OK")).clicked() {
                        action = Some(false);
                    }
                }
                None => {}
            },
        );
        // Keep and Discard stay in view below the scrolling plan and cards.
        if let Some(Shown::Plan { usage, seconds, .. }) = &shown {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new(tl!("Keep")).color(egui::Color32::WHITE)).fill(t.accent)).clicked() {
                    action = Some(true);
                }
                if ui.button(tl!("Discard")).clicked() {
                    action = Some(false);
                }
                // The cost shows only in internal builds (owner's decision, 2026-10-18).
                if shell.developer || internal_build() {
                    // i18n: developer only
                    let caption = format!(
                        "{} model call{} · US${:.4} · {seconds:.1} s",
                        usage.calls,
                        if usage.calls == 1 { "" } else { "s" },
                        usage.cost_usd
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(caption).size(CAPTION).color(t.text_secondary));
                    });
                }
            });
        }
        shell.ask.shown = shown;
        match action {
            Some(true) => keep(shell),
            Some(false) => discard(shell),
            None => {}
        }
        if let Some(c) = pick {
            submit(shell, ui.ctx(), c);
        }
        ui.allocate_space(vec2(0.0, 0.0));
    });
}
