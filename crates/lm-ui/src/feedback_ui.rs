//! Help › Send Feedback… (docs/lm/feedback.md): a small sheet with Idea · Problem · Other, the
//! message, an optional title and reply address, and "Include app version and macOS version".
//! Send posts it through the feedback service, which puts it on GitHub Discussions; the person
//! needs no GitHub account. The sheet says before sending that the message will be public and the
//! address won't. The request runs on a background thread (`lm_account::feedback::send_now`);
//! the sheet shows "Sending…", then the discussion's link, or what to do when it failed, with the
//! text kept.
//!
//! Keys: ⌘↩ sends, Esc cancels (taken before the shell's own shortcuts see them, so Esc doesn't
//! also clear the selection behind the sheet). A click outside doesn't close it: typing is never
//! lost by accident.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use egui::{RichText, Ui, vec2};
use egui_phosphor::regular as icon;

use lm_account::feedback::{self as fb, Draft, FeedbackError, Kind, Problem};
use lm_core::{i18n, tl};

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, TITLE, Theme};

const WIDTH: f32 = 460.0;
const MESSAGE_ID: &str = "lm-feedback-message";

/// The sheet while it is open.
pub(crate) struct Sheet {
    pub(crate) draft: Draft,
    pub(crate) phase: Phase,
    /// What to change, or why sending failed, translated; shown above the buttons.
    pub(crate) error: Option<String>,
    /// Put the cursor in the message on the next frame.
    focus: bool,
    /// Esc and ⌘↩, taken from the input at the start of the frame ([`keys`]).
    escape: bool,
    send: bool,
}

pub(crate) enum Phase {
    Editing,
    Sending(Receiver<Result<String, FeedbackError>>),
    /// Posted: the discussion's address.
    Sent(String),
}

impl Sheet {
    pub(crate) fn new() -> Sheet {
        Sheet { draft: Draft::default(), phase: Phase::Editing, error: None, focus: true, escape: false, send: false }
    }

    fn sending(&self) -> bool {
        matches!(self.phase, Phase::Sending(_))
    }
}

/// Opens the sheet (Help › Send Feedback…); an open one stays as it is.
pub(crate) fn open(shell: &mut Shell) {
    if shell.feedback.is_none() {
        shell.feedback = Some(Sheet::new());
    }
}

/// Takes Esc and ⌘↩ for the sheet while it is open: called before the shell's shortcuts.
pub(crate) fn keys(shell: &mut Shell, ctx: &egui::Context) {
    let Some(s) = shell.feedback.as_mut() else { return };
    let (escape, send) = ctx.input_mut(|i| {
        (i.consume_key(egui::Modifiers::NONE, egui::Key::Escape), i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
    });
    s.escape |= escape;
    s.send |= send;
}

/// What `e` says, in the interface language.
fn error_text(e: &FeedbackError) -> String {
    let (template, values) = e.words();
    words(template, &values)
}

fn problem_text(p: Problem, d: &Draft) -> String {
    let (template, values) = p.words(d);
    words(template, &values)
}

fn words(template: &str, values: &[(&str, String)]) -> String {
    // Counts in the language's own style ("4.000" in German).
    let shown: Vec<(&str, String)> = values
        .iter()
        .map(|(k, v)| match (*k, v.parse::<u64>()) {
            ("n" | "max", Ok(n)) => (*k, i18n::count(n)),
            _ => (*k, v.clone()),
        })
        .collect();
    let mut args: Vec<(&str, &str)> = shown.iter().map(|(k, v)| (*k, v.as_str())).collect();
    args.push(("app", lm_core::brand::shown(false)));
    i18n::fmt(template, &args)
}

/// Starts sending after checking the draft (the server's bounds, so most problems are caught here).
fn start(s: &mut Sheet, ctx: &egui::Context, language: &str) {
    if let Err(p) = fb::check(&s.draft) {
        s.error = Some(problem_text(p, &s.draft));
        return;
    }
    s.error = None;
    let (tx, rx) = channel();
    let draft = s.draft.clone();
    let language = language.to_string();
    let wake = ctx.clone();
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let _ = tx.send(fb::send_now(&draft, &language));
        wake.request_repaint();
    });
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (draft, language, wake);
        let _ = tx.send(Err(FeedbackError::NotConfigured));
    }
    s.phase = Phase::Sending(rx);
}

/// Picks up the background thread's answer.
fn poll(s: &mut Sheet) {
    let Phase::Sending(rx) = &s.phase else { return };
    match rx.try_recv() {
        Ok(Ok(url)) => s.phase = Phase::Sent(url),
        Ok(Err(e)) => {
            log::info!("feedback: not sent: {e}");
            s.error = Some(error_text(&e));
            s.phase = Phase::Editing;
        }
        Err(TryRecvError::Disconnected) => {
            s.error = Some(error_text(&FeedbackError::Upstream));
            s.phase = Phase::Editing;
        }
        Err(TryRecvError::Empty) => {}
    }
}

/// Draws the sheet while it is open.
pub(crate) fn sheet(shell: &mut Shell, ctx: &egui::Context) {
    let Some(mut s) = shell.feedback.take() else { return };
    let t = *shell.theme();
    let language = shell.language().code();
    let was_sending = s.sending();
    poll(&mut s);
    let mut close = std::mem::take(&mut s.escape);
    let mut send = std::mem::take(&mut s.send) && matches!(s.phase, Phase::Editing);
    let mut view: Option<String> = None;
    // A Modal: the map behind is dimmed and can't be clicked; its own close-on-click-outside is
    // not used (see the module notes).
    egui::Modal::new(egui::Id::new("lm-feedback")).show(ctx, |ui| {
        ui.set_width(WIDTH);
        match &s.phase {
            Phase::Sent(url) => {
                let (v, done) = sent(ui, &t, url);
                if v {
                    view = Some(url.clone());
                }
                close |= done;
            }
            _ => {
                let (go, cancel) = form(ui, &t, &mut s);
                send |= go;
                close |= cancel;
            }
        }
    });
    if send && !s.sending() {
        start(&mut s, ctx, language);
    }
    // Screen readers hear the outcome.
    let id = egui::Id::new("lm-feedback-status");
    match &s.phase {
        Phase::Sending(_) if !was_sending => crate::a11y::live(ctx, id, tl!("Sending…")),
        Phase::Sent(_) if was_sending => crate::a11y::live(ctx, id, tl!("Thank you! Your feedback is posted.")),
        Phase::Editing if was_sending => {
            if let Some(e) = &s.error {
                crate::a11y::live(ctx, id, e);
            }
        }
        _ => {}
    }
    if let Some(url) = view {
        #[cfg(not(target_arch = "wasm32"))]
        if let Err(e) = lm_account::browser::open(&url) {
            log::warn!("feedback: {e}");
        }
        #[cfg(target_arch = "wasm32")]
        let _ = url;
    }
    if !close {
        shell.feedback = Some(s);
    }
}

/// The form; returns (Send pressed, Cancel pressed).
fn form(ui: &mut Ui, t: &Theme, s: &mut Sheet) -> (bool, bool) {
    let sending = s.sending();
    let mut send = false;
    let mut cancel = false;
    ui.label(RichText::new(tl!("Send Feedback")).font(theme::semibold(TITLE)));
    ui.add_space(2.0);
    ui.label(
        RichText::new(i18n::fmt("Tell us what would make {app} better, or what went wrong.", &[("app", lm_core::brand::shown(false))]))
            .color(t.text_secondary),
    );
    ui.add_space(12.0);
    ui.add_enabled_ui(!sending, |ui| {
        crate::form::with_visible_controls(ui, t, |ui| {
            let labels: Vec<&str> = Kind::ALL.iter().map(|k| tl!(k.label())).collect();
            let selected = Kind::ALL.iter().position(|k| *k == s.draft.kind).unwrap_or(0);
            if let Some(k) = crate::form::segmented(ui, t, selected, &labels, WIDTH).and_then(|i| Kind::ALL.get(i)) {
                s.draft.kind = *k;
            }
            ui.add_space(10.0);
            caption(ui, t, tl!("Message"));
            let hint = match s.draft.kind {
                Kind::Idea => i18n::fmt("What would you like {app} to do?", &[("app", lm_core::brand::shown(false))]),
                Kind::Problem => tl!("What happened, and what did you expect?").to_string(),
                Kind::Other => tl!("What's on your mind?").to_string(),
            };
            let message = egui::TextEdit::multiline(&mut s.draft.message)
                .hint_text(hint)
                .desired_rows(6)
                .desired_width(f32::INFINITY)
                .id(egui::Id::new(MESSAGE_ID));
            let r = egui::ScrollArea::vertical().id_salt("lm-feedback-scroll").max_height(160.0).show(ui, |ui| ui.add(message)).inner;
            crate::a11y::label(&r, tl!("Message"));
            if std::mem::take(&mut s.focus) {
                r.request_focus();
            }
            let n = fb::chars(&fb::clean(&s.draft.message));
            if n > fb::MAX_MESSAGE - 500 {
                let over = n > fb::MAX_MESSAGE;
                let text =
                    i18n::fmt("{n} of {max} characters", &[("n", &i18n::count(n as u64)), ("max", &i18n::count(fb::MAX_MESSAGE as u64))]);
                ui.label(RichText::new(text).size(CAPTION).color(if over { t.critical } else { t.text_secondary }));
            }
            ui.add_space(8.0);
            caption(ui, t, tl!("Title (optional)"));
            let title = egui::TextEdit::singleline(&mut s.draft.title)
                .hint_text(tl!("A short summary"))
                .desired_width(f32::INFINITY)
                .char_limit(fb::MAX_TITLE)
                .id(egui::Id::new("lm-feedback-title"));
            let r = ui.add(title);
            crate::a11y::label(&r, tl!("Title (optional)"));
            ui.add_space(8.0);
            caption(ui, t, tl!("Email (optional)"));
            let email = egui::TextEdit::singleline(&mut s.draft.email)
                .hint_text("name@example.com") // i18n: an example address
                .desired_width(f32::INFINITY)
                .char_limit(fb::MAX_EMAIL)
                .id(egui::Id::new("lm-feedback-email"));
            let r = ui.add(email);
            crate::a11y::label(&r, tl!("Email (optional)"));
            let note = tl!("Only used to reply to you. Never shown publicly.");
            crate::a11y::describe(&r, note);
            ui.label(RichText::new(note).size(CAPTION).color(t.text_secondary));
            ui.add_space(8.0);
            let label =
                if cfg!(windows) { tl!("Include app version and Windows version") } else { tl!("Include app version and macOS version") };
            crate::form::checkbox(ui, t, &mut s.draft.include_diagnostics, label);
        });
    });
    ui.add_space(10.0);
    ui.horizontal_top(|ui| {
        ui.label(RichText::new(icon::GLOBE_SIMPLE).font(theme::icon_font(15.0)).color(t.text_secondary));
        ui.add(
            egui::Label::new(
                RichText::new(tl!("Your feedback will be posted publicly on GitHub Discussions, without your email."))
                    .size(CAPTION)
                    .color(t.text_secondary),
            )
            .wrap(),
        );
    });
    if let Some(e) = &s.error {
        ui.add_space(8.0);
        ui.add(egui::Label::new(RichText::new(e).color(t.critical)).wrap());
    }
    ui.add_space(12.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let words = if sending { tl!("Sending…") } else { tl!("Send") };
        let b = egui::Button::new(RichText::new(words).color(t.on_accent)).fill(t.accent).min_size(vec2(88.0, 30.0));
        let r = ui.add_enabled(!sending, b);
        crate::a11y::describe(&r, &crate::keys::shortcut("↩"));
        if r.clicked() {
            send = true;
        }
        if ui.add(egui::Button::new(tl!("Cancel")).min_size(vec2(80.0, 30.0))).clicked() {
            cancel = true;
        }
        if sending {
            ui.add(egui::Spinner::new().size(16.0));
        }
    });
    (send, cancel)
}

/// The success state; returns (View on GitHub pressed, Done pressed).
fn sent(ui: &mut Ui, t: &Theme, url: &str) -> (bool, bool) {
    let mut view = false;
    let mut done = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::CHECK_CIRCLE).font(theme::icon_font(28.0)).color(t.accent));
        ui.label(RichText::new(tl!("Thank you! Your feedback is posted.")).font(theme::semibold(TITLE)));
    });
    ui.add_space(6.0);
    ui.add(
        egui::Label::new(RichText::new(tl!("It's public on GitHub Discussions, where you can follow replies.")).color(t.text_secondary))
            .wrap(),
    );
    ui.add_space(4.0);
    ui.add(egui::Label::new(RichText::new(url).size(CAPTION).color(t.text_secondary)).truncate());
    ui.add_space(14.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let b = egui::Button::new(RichText::new(tl!("View on GitHub")).color(t.on_accent)).fill(t.accent).min_size(vec2(112.0, 30.0));
        let r = ui.add(b);
        if r.clicked() {
            view = true;
        }
        if ui.add(egui::Button::new(tl!("Done")).min_size(vec2(80.0, 30.0))).clicked() {
            done = true;
        }
    });
    (view, done)
}

fn caption(ui: &mut Ui, t: &Theme, text: &str) {
    ui.label(RichText::new(text).size(BODY).color(t.text_primary));
    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screenshots::{WORLD, harness_phase};
    use egui_kittest::kittest::Queryable as _;

    fn with_sheet(setup: impl Fn(&mut Sheet) + 'static) -> egui_kittest::Harness<'static, Option<Shell>> {
        harness_phase(1, false, WORLD, move |s| {
            let mut sheet = Sheet::new();
            setup(&mut sheet);
            s.feedback = Some(sheet);
        })
    }

    #[test]
    fn errors_and_problems_read_in_the_interface_language() {
        lm_core::i18n::set_current(lm_core::i18n::Lang::EN);
        assert!(error_text(&FeedbackError::Offline).starts_with("NimbleMap is working offline"));
        let d = Draft { message: "x".repeat(4100), ..Draft::default() };
        assert_eq!(problem_text(Problem::MessageTooLong, &d), "The message is 4,100 characters long. Shorten it to 4,000 or fewer.");
        if let Some(de) = lm_core::i18n::Lang::from_code("de") {
            lm_core::i18n::set_current(de);
            let text = problem_text(Problem::MessageTooLong, &d);
            assert!(text.contains("4.100") && text.contains("4.000"), "{text}");
            lm_core::i18n::set_current(lm_core::i18n::Lang::EN);
        }
    }

    #[test]
    fn a_short_message_is_caught_before_sending() {
        let ctx = egui::Context::default();
        let mut s = Sheet::new();
        s.draft.message = "hi".into();
        start(&mut s, &ctx, "en");
        assert!(matches!(s.phase, Phase::Editing));
        assert_eq!(s.error.as_deref(), Some("Write a few words about your idea or problem."));
    }

    #[test]
    fn the_answer_moves_the_sheet_on() {
        let (tx, rx) = channel();
        let mut s = Sheet::new();
        s.phase = Phase::Sending(rx);
        poll(&mut s);
        assert!(s.sending());
        let url = "https://github.com/kangning-huang/NimbleMap/discussions/1".to_string();
        let _ = tx.send(Ok(url.clone()));
        poll(&mut s);
        assert!(matches!(&s.phase, Phase::Sent(u) if *u == url));
        // A failure keeps the text and says what to do.
        let (tx, rx) = channel();
        s.phase = Phase::Sending(rx);
        s.draft.message = "My long message".into();
        let _ = tx.send(Err(FeedbackError::RateLimited));
        poll(&mut s);
        assert!(matches!(s.phase, Phase::Editing));
        assert_eq!(s.draft.message, "My long message");
        assert!(s.error.as_deref().is_some_and(|e| e.contains("tomorrow")));
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn feedback_sheet() {
        let mut h = with_sheet(|s| {
            s.draft.kind = Kind::Problem;
            s.draft.message = "The time slider skips March when I press →.\nIt happens with the storms example.".into();
            s.draft.email = "kim@example.com".into();
        });
        for label in ["Idea", "Problem", "Other", "Send", "Cancel", "Include app version and macOS version"] {
            assert!(h.query_by_label(label).is_some(), "{label}");
        }
        h.snapshot("feedback_sheet");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn feedback_sheet_error() {
        let mut h = with_sheet(|s| {
            s.draft.message = "Please add a dark basemap with labels.".into();
            s.error = Some(error_text(&FeedbackError::Network("dns".into())));
        });
        h.snapshot("feedback_sheet_error");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn feedback_sheet_sent() {
        let mut h = with_sheet(|s| s.phase = Phase::Sent("https://github.com/kangning-huang/NimbleMap/discussions/42".into()));
        assert!(h.query_by_label("View on GitHub").is_some());
        h.snapshot("feedback_sheet_sent");
    }

    #[test]
    fn escape_closes_and_command_return_sends() {
        let mut h = with_sheet(|s| s.draft.message = "hi".into());
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
        assert!(h.state().as_ref().is_some_and(|s| s.feedback.is_none()), "Esc closes the sheet");
        // ⌘↩ sends: a message that's too short is stopped with its reason, nothing goes out.
        let mut h = with_sheet(|s| s.draft.message = "hi".into());
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
        h.run_steps(2);
        let shell = h.state().as_ref();
        let sheet = shell.and_then(|s| s.feedback.as_ref());
        assert!(sheet.is_some_and(|s| matches!(s.phase, Phase::Editing) && s.error.is_some()));
        // The message didn't get a line break from the key.
        assert_eq!(sheet.map(|s| s.draft.message.as_str()), Some("hi"));
    }
}
