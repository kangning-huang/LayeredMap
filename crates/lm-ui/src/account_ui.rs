//! Settings › Account (WP7.4) and the sign-in prompt in the Ask box. Signed out, the pane offers
//! the region's ways to sign in; signed in, it says who, with Sign Out and Delete Account. The
//! work (`lm_account`) runs on a background thread; the pane shows it waiting for the browser.
//! Once a sign-in email is sent, the pane asks for its 6-digit code while the link can still be
//! clicked on this computer: whichever comes first signs in.
//! Accounts are on only when the build has an account config (`lm_account::Config::load`); without
//! one nothing here shows and the Ask box works as before (docs/lm/accounts-design.md).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use egui::{RichText, Ui, vec2};

use lm_account::{Account, AccountError, AskRoute, Method, Provider, Region, User};

use crate::Shell;
use crate::theme::{self, BODY, CAPTION, HEADLINE, TITLE};

const WIDTH: f32 = 420.0;
const BUTTON_W: f32 = 280.0;

/// The account and the pane's state.
pub struct AccountUi {
    pub(crate) account: Option<Arc<Account>>,
    /// The region signed-out sign-in uses (from the system region; the pane can change it).
    pub(crate) region: Region,
    /// Settings is open.
    pub(crate) open: bool,
    pub(crate) email: String,
    busy: Option<Busy>,
    /// The last outcome to show: text and whether it is a problem.
    pub(crate) message: Option<(String, bool)>,
    pub(crate) confirm_delete: bool,
    /// A developer or test-build assistant key is present (read once: it spawns `security`).
    key_present: Option<bool>,
    /// The email was sent: the code it carries can be typed (shown until signed in or dropped).
    pub(crate) email_code: Option<EmailCode>,
    /// A typed code being checked.
    verifying: Option<Receiver<Result<User, AccountError>>>,
}

/// The code-entry state after "Email me a sign-in code".
#[derive(Clone, Debug)]
pub(crate) struct EmailCode {
    pub(crate) address: String,
    pub(crate) region: Region,
    /// What is in the field: digits only, at most six.
    pub(crate) code: String,
    /// The last code's problem ("That code didn't work…").
    pub(crate) error: Option<String>,
    /// The link's wait on this computer ended (10 minutes); the code still works for an hour.
    pub(crate) link_expired: bool,
    /// Put the cursor in the field on the next frame.
    focus: bool,
}

impl EmailCode {
    pub(crate) fn new(address: String, region: Region) -> EmailCode {
        EmailCode { address, region, code: String::new(), error: None, link_expired: false, focus: false }
    }
}

impl Default for AccountUi {
    fn default() -> Self {
        AccountUi {
            account: None,
            region: Region::International,
            open: false,
            email: String::new(),
            busy: None,
            message: None,
            confirm_delete: false,
            key_present: None,
            email_code: None,
            verifying: None,
        }
    }
}

/// What the background thread is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Doing {
    SignIn(Provider),
    EmailLink(String),
    SignOut,
    Delete,
}

struct Busy {
    doing: Doing,
    region: Region,
    rx: Receiver<Event>,
    cancel: Arc<AtomicBool>,
}

/// What the background thread reports.
enum Event {
    /// The sign-in email went out: its code can be typed now.
    Sent,
    Done(Result<Option<User>, AccountError>),
}

impl AccountUi {
    /// An account with a config (the app's, or a test's).
    pub fn new(account: Arc<Account>, region: Region) -> AccountUi {
        AccountUi { account: Some(account), region, ..Default::default() }
    }

    /// Whether this build has accounts at all (Settings… in the menu).
    pub fn available(&self) -> bool {
        self.account.as_ref().is_some_and(|a| a.config().any())
    }

    /// Who is signed in, and where.
    pub fn user(&self) -> Option<(User, Region)> {
        self.account.as_ref().and_then(|a| a.user())
    }

    /// The region in use: the session's when signed in, else the pane's choice.
    fn active_region(&self) -> Region {
        self.user().map_or(self.region, |(_, r)| r)
    }

    fn proxy_configured(&self) -> bool {
        self.account.as_ref().is_some_and(|a| a.config().region(self.active_region()).is_some())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn key_present(&mut self) -> bool {
        *self.key_present.get_or_insert_with(|| lm_ask::openrouter::read_key().is_some())
    }

    #[cfg(target_arch = "wasm32")]
    fn key_present(&mut self) -> bool {
        false
    }

    /// The Ask box's route for the next request. Without an account config this is the old
    /// behaviour (the key, or nothing).
    pub fn route(&mut self) -> AskRoute {
        let proxy = self.proxy_configured();
        let env = std::env::var("LM_ASK_ROUTE").ok();
        let wants_direct = env.as_deref().is_some_and(|v| v.trim().eq_ignore_ascii_case("direct"));
        // The key is looked up only where it can change the answer (it runs `security`).
        let key = if proxy && !wants_direct { false } else { self.key_present() };
        AskRoute::choose(env.as_deref(), key, proxy)
    }

    /// Where the proxy is for the signed-in region: the Ask URL and the anon key.
    pub(crate) fn proxy_target(&self) -> Option<(Arc<Account>, String, String)> {
        let account = self.account.clone()?;
        let (_, region) = account.user()?;
        let rc = account.config().region(region)?;
        let target = (rc.ask_url(), rc.anon_key.clone());
        Some((account, target.0, target.1))
    }

    /// The Ask box shows the sign-in prompt instead of its field: the proxy is the route and
    /// nobody is signed in. Never without an account config, so no key lookup per frame.
    pub fn needs_sign_in(&self) -> bool {
        self.proxy_configured()
            && self.user().is_none()
            && !std::env::var("LM_ASK_ROUTE").ok().is_some_and(|v| v.trim().eq_ignore_ascii_case("direct"))
    }

    pub(crate) fn busy(&self) -> Option<&Doing> {
        self.busy.as_ref().map(|b| &b.doing)
    }

    /// Stops the background sign-in now: its thread ends on its own (the loopback closes within
    /// 50 ms) and whatever it would still say is dropped.
    fn drop_sign_in(&mut self) {
        if let Some(b) = self.busy.take_if(|b| matches!(b.doing, Doing::SignIn(_) | Doing::EmailLink(_))) {
            b.cancel.store(true, Ordering::SeqCst);
        }
    }
}

/// Reads the account config, the stored session (no network) and the system region: the app
/// calls this once at launch in Pro. Without a config nothing changes.
#[cfg(not(target_arch = "wasm32"))]
pub fn start(shell: &mut Shell) {
    let Some(config) = lm_account::Config::load() else { return };
    let store: Arc<dyn lm_account::store::SecretStore> = match lm_account::store::system() {
        Some(s) => s,
        None => Arc::new(lm_account::store::MemoryStore::default()),
    };
    let account = Arc::new(Account::new(config, Arc::new(lm_account::http::GatedHttp), store));
    account.restore();
    let region = Region::for_system(lm_ask::reference::system_region().as_deref());
    shell.account = AccountUi::new(account, region);
}

/// Opens Settings › Account.
pub(crate) fn open(shell: &mut Shell) {
    if shell.account.available() {
        shell.account.open = true;
        shell.account.confirm_delete = false;
    }
}

/// Starts work on a background thread.
#[cfg(not(target_arch = "wasm32"))]
fn spawn(shell: &mut Shell, ctx: &egui::Context, doing: Doing) {
    let Some(account) = shell.account.account.clone() else { return };
    if shell.account.busy.is_some() {
        return;
    }
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    let region = shell.account.region;
    let wake = ctx.clone();
    let job = doing.clone();
    std::thread::spawn(move || {
        let sent = || {
            let _ = tx.send(Event::Sent);
            wake.request_repaint();
        };
        let r = match job {
            Doing::SignIn(p) => sign_in(&account, region, &Method::Provider(p), &c, sent),
            Doing::EmailLink(e) => sign_in(&account, region, &Method::Email(e), &c, sent),
            Doing::SignOut => account.sign_out().map(|_| None),
            Doing::Delete => account.delete_account().map(|_| None),
        };
        let _ = tx.send(Event::Done(r));
        wake.request_repaint();
    });
    shell.account.message = None;
    shell.account.busy = Some(Busy { doing, region, rx, cancel });
}

/// Checks the typed code on a background thread.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_verify(shell: &mut Shell, ctx: &egui::Context) {
    let Some(account) = shell.account.account.clone() else { return };
    let Some(ec) = shell.account.email_code.as_mut() else { return };
    if shell.account.verifying.is_some() {
        return;
    }
    if lm_account::code::normalize(&ec.code).is_none() {
        ec.error = Some(AccountError::CodeFormat.to_string());
        return;
    }
    ec.error = None;
    let (tx, rx) = channel();
    let (region, address, code) = (ec.region, ec.address.clone(), ec.code.clone());
    let wake = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(account.verify_code(region, &address, &code));
        wake.request_repaint();
    });
    shell.account.verifying = Some(rx);
}

#[cfg(target_arch = "wasm32")]
fn spawn_verify(shell: &mut Shell, _ctx: &egui::Context) {
    shell.account.message = Some((AccountError::NotConfigured.to_string(), true));
}

/// "Send a new email": the waiting sign-in stops and a new email goes to the same address.
fn resend(shell: &mut Shell, ctx: &egui::Context) {
    let Some(ec) = shell.account.email_code.take() else { return };
    shell.account.drop_sign_in();
    shell.account.region = ec.region;
    spawn(shell, ctx, Doing::EmailLink(ec.address));
}

/// "Use a different email": back to the ways to sign in, the address ready to edit.
fn different_email(shell: &mut Shell) {
    if let Some(ec) = shell.account.email_code.take() {
        shell.account.email = ec.address;
    }
    shell.account.drop_sign_in();
    shell.account.message = None;
}

#[cfg(target_arch = "wasm32")]
fn spawn(shell: &mut Shell, _ctx: &egui::Context, _doing: Doing) {
    shell.account.message = Some((AccountError::NotConfigured.to_string(), true));
}

#[cfg(not(target_arch = "wasm32"))]
fn sign_in(
    account: &Account,
    region: Region,
    method: &Method,
    cancel: &AtomicBool,
    sent: impl FnOnce(),
) -> Result<Option<User>, AccountError> {
    let s = account.begin(region, method)?;
    match s.url() {
        Some(url) => lm_account::browser::open(url).map_err(AccountError::Listener)?,
        None => sent(),
    }
    account.complete(s, cancel, lm_account::SIGN_IN_TIMEOUT).map(Some)
}

/// The background work's outcome, once it arrives (every frame, so a sign-in that finishes
/// after Settings was closed still lands).
pub(crate) fn poll(shell: &mut Shell) {
    poll_code(shell);
    let r = loop {
        match shell.account.busy.as_ref().map(|b| b.rx.try_recv()) {
            Some(Ok(Event::Sent)) => {
                if let Some(b) = &shell.account.busy
                    && let Doing::EmailLink(address) = &b.doing
                    && !b.cancel.load(Ordering::SeqCst)
                {
                    let (address, region) = (address.clone(), b.region);
                    shell.account.email_code = Some(EmailCode { focus: true, ..EmailCode::new(address, region) });
                }
            }
            Some(Ok(Event::Done(r))) => break r,
            _ => return,
        }
    };
    let Some(b) = shell.account.busy.take() else { return };
    shell.account.confirm_delete = false;
    // The link's wait ended without signing in while the code can still be typed: say so there.
    if let (Doing::EmailLink(_), Err(e), Some(ec)) = (&b.doing, &r, shell.account.email_code.as_mut()) {
        match e {
            AccountError::Cancelled => {}
            AccountError::TimedOut => ec.link_expired = true,
            e => ec.error = Some(e.to_string()),
        }
        return;
    }
    shell.account.message = match (b.doing, r) {
        (_, Err(AccountError::Cancelled)) => None,
        (Doing::SignIn(_) | Doing::EmailLink(_), Ok(_)) => {
            // The Ask box's sign-in message has done its job.
            shell.ask.clear_message();
            shell.account.email_code = None;
            None
        }
        (Doing::SignOut, Ok(_)) => Some(("You're signed out.".into(), false)),
        (Doing::Delete, Ok(_)) => Some(("Your account was deleted.".into(), false)),
        (_, Err(e)) => Some((e.to_string(), true)),
    };
}

/// A typed code's answer, once it arrives.
fn poll_code(shell: &mut Shell) {
    let r = match shell.account.verifying.as_ref().map(|rx| rx.try_recv()) {
        Some(Ok(r)) => r,
        // The check ended without an answer: let the person try again.
        Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => Err(AccountError::Cancelled),
        _ => return,
    };
    shell.account.verifying = None;
    match r {
        Ok(_) => {
            // Signed in; the link's wait stops by itself (lm_account ends it).
            shell.account.email_code = None;
            shell.account.message = None;
            shell.ask.clear_message();
        }
        Err(AccountError::Cancelled) => {}
        Err(e) => {
            if let Some(ec) = shell.account.email_code.as_mut() {
                ec.error = Some(e.to_string());
            }
        }
    }
}

/// Stops a sign-in waiting for the browser (or still sending its email).
fn cancel(shell: &mut Shell) {
    shell.account.drop_sign_in();
}

/// The Settings window, while open.
pub(crate) fn window(shell: &mut Shell, ctx: &egui::Context) {
    if !shell.account.open {
        return;
    }
    let t = *shell.theme();
    let mut close = false;
    let mut doing = None;
    let modal = egui::Modal::new(egui::Id::new("lm-settings")).show(ctx, |ui| {
        ui.set_width(WIDTH);
        ui.label(RichText::new("Settings").font(theme::semibold(TITLE)));
        ui.add_space(4.0);
        ui.label(RichText::new("Account").size(CAPTION).color(t.text_secondary));
        crate::form::divider(ui, &t);
        doing = crate::form::with_visible_controls(ui, &t, |ui| account_pane(shell, ui, &t));
        crate::form::divider(ui, &t);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let done = egui::Button::new(RichText::new("Done").color(t.on_accent)).fill(t.accent).min_size(vec2(80.0, 30.0));
            if ui.add(done).clicked() {
                close = true;
            }
        });
    });
    if modal.should_close() {
        close = true;
    }
    match doing {
        Some(Pane::Start(d)) => spawn(shell, ctx, d),
        Some(Pane::Cancel) => cancel(shell),
        Some(Pane::Verify) => spawn_verify(shell, ctx),
        Some(Pane::Resend) => resend(shell, ctx),
        Some(Pane::DifferentEmail) => different_email(shell),
        None => {}
    }
    if close {
        shell.account.open = false;
        shell.account.confirm_delete = false;
    }
}

enum Pane {
    Start(Doing),
    Cancel,
    /// Check the typed code.
    Verify,
    Resend,
    DifferentEmail,
}

fn account_pane(shell: &mut Shell, ui: &mut Ui, t: &theme::Theme) -> Option<Pane> {
    let mut out = None;
    let offline = lm_net::offline();
    let pro = lm_core::brand::PRO_NAME;
    match shell.account.user() {
        Some((user, region)) => {
            ui.label(RichText::new(user.display()).font(theme::semibold(HEADLINE)));
            let how = lm_account::session::provider_label(&user.provider);
            let how = if user.email.is_some() { format!("Signed in with {how} · {}", region.label()) } else { region.label().to_string() };
            ui.label(RichText::new(how).size(CAPTION).color(t.text_secondary));
            ui.add_space(8.0);
            ui.label(RichText::new(format!("{pro} is on for this account.")).size(BODY));
            ui.add_space(12.0);
            if shell.account.confirm_delete {
                ui.label(
                    RichText::new(format!(
                        "Delete your {} account? {pro} stops working on every computer you use. Your maps and files are not affected.",
                        lm_core::brand::NAME
                    ))
                    .size(BODY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let del = egui::Button::new(RichText::new("Delete Account").color(egui::Color32::WHITE)).fill(t.critical);
                    if ui.add_enabled(!offline && shell.account.busy.is_none(), del).clicked() {
                        out = Some(Pane::Start(Doing::Delete));
                    }
                    if ui.button("Cancel").clicked() {
                        shell.account.confirm_delete = false;
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    if ui.add_enabled(shell.account.busy.is_none(), egui::Button::new("Sign Out")).clicked() {
                        out = Some(Pane::Start(Doing::SignOut));
                    }
                    let del = egui::Button::new(RichText::new("Delete Account…").color(t.critical));
                    if ui.add_enabled(!offline && shell.account.busy.is_none(), del).clicked() {
                        shell.account.confirm_delete = true;
                    }
                });
            }
            if offline {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Working offline: Delete Account needs the network. Sign Out works offline.")
                        .size(CAPTION)
                        .color(t.text_secondary),
                );
            }
        }
        None if shell.account.email_code.is_some() => {
            ui.label(RichText::new(format!("Sign in to use {pro}")).font(theme::semibold(HEADLINE)));
            ui.add_space(6.0);
            out = code_entry(shell, ui, t, offline);
        }
        None => {
            ui.label(RichText::new(format!("Sign in to use {pro}")).font(theme::semibold(HEADLINE)));
            ui.add_space(2.0);
            ui.label(
                RichText::new(format!(
                    "{pro} is free while it's new. Your account only says who you are: your maps and files never leave this computer."
                ))
                .size(CAPTION)
                .color(t.text_secondary),
            );
            ui.add_space(10.0);
            let regions = [Region::International, Region::China];
            let labels: Vec<&str> = regions.iter().map(|r| r.label()).collect();
            let sel = regions.iter().position(|r| *r == shell.account.region).unwrap_or(0);
            let idle = shell.account.busy.is_none();
            crate::form::row(ui, t, "Region", |ui| {
                if let Some(k) = crate::form::segmented(ui, t, sel, &labels, BUTTON_W)
                    && idle
                    && let Some(r) = regions.get(k)
                {
                    shell.account.region = *r;
                    shell.account.message = None;
                }
            });
            ui.add_space(10.0);
            out = sign_in_buttons(shell, ui, t, offline);
        }
    }
    if let Some((text, warning)) = &shell.account.message {
        ui.add_space(8.0);
        ui.label(RichText::new(text).size(CAPTION).color(if *warning { t.critical } else { t.text_secondary }));
    }
    out
}

fn sign_in_buttons(shell: &mut Shell, ui: &mut Ui, t: &theme::Theme, offline: bool) -> Option<Pane> {
    let region = shell.account.region;
    let providers: Vec<Provider> = shell
        .account
        .account
        .as_ref()
        .and_then(|a| a.config().region(region).map(|rc| rc.providers.iter().map(|(p, _)| *p).collect()))
        .unwrap_or_default();
    if providers.is_empty() {
        let line = match region {
            Region::China => "Sign-in for mainland China (WeChat, Alipay, phone number) is coming in a later version.".to_string(),
            Region::International => AccountError::NotConfigured.to_string(),
        };
        ui.label(RichText::new(line).size(BODY).color(t.text_secondary));
        return None;
    }
    if let Some(doing) = shell.account.busy().cloned() {
        let waiting = match &doing {
            Doing::SignIn(_) => "Finish signing in in your browser…".to_string(),
            Doing::EmailLink(e) => format!("Sending an email to {e}…"),
            Doing::SignOut => "Signing out…".into(),
            Doing::Delete => "Deleting your account…".into(),
        };
        let mut out = None;
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new(waiting).size(BODY));
        });
        if matches!(doing, Doing::SignIn(_) | Doing::EmailLink(_)) {
            ui.add_space(6.0);
            if ui.button("Cancel").clicked() {
                out = Some(Pane::Cancel);
            }
        }
        return out;
    }
    let mut out = None;
    for p in providers.iter().filter(|p| **p != Provider::Email) {
        let b = egui::Button::new(RichText::new(p.button()).size(BODY)).min_size(vec2(BUTTON_W, 30.0));
        if ui.add_enabled(!offline, b).clicked() {
            out = Some(Pane::Start(Doing::SignIn(*p)));
        }
        ui.add_space(2.0);
    }
    if providers.contains(&Provider::Email) {
        ui.add_space(6.0);
        let field = egui::TextEdit::singleline(&mut shell.account.email)
            .hint_text("Email address")
            .desired_width(BUTTON_W)
            .id(egui::Id::new("lm-account-email"));
        let r = ui.add_enabled(!offline, field);
        crate::a11y::label(&r, "Email address");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(2.0);
        let b = egui::Button::new(RichText::new(Provider::Email.button()).size(BODY)).min_size(vec2(BUTTON_W, 30.0));
        let ready = !shell.account.email.trim().is_empty();
        if (ui.add_enabled(!offline && ready, b).clicked() || (enter && ready)) && !offline {
            out = Some(Pane::Start(Doing::EmailLink(shell.account.email.trim().to_string())));
        }
    }
    if offline {
        ui.add_space(6.0);
        ui.label(RichText::new(AccountError::Offline.to_string()).size(CAPTION).color(t.text_secondary));
    }
    out
}

/// "We sent an email…": the 6-digit code field with Verify, Send a New Email and Use a Different
/// Email. The link in the email still works on this computer meanwhile.
fn code_entry(shell: &mut Shell, ui: &mut Ui, t: &theme::Theme, offline: bool) -> Option<Pane> {
    let checking = shell.account.verifying.is_some();
    let ec = shell.account.email_code.as_mut()?;
    let mut out = None;
    let here = if cfg!(target_os = "macos") { "this Mac" } else { "this computer" };
    let said = if ec.link_expired {
        format!("We sent an email to {}. Enter the {}-digit code in it.", ec.address, lm_account::code::LEN)
    } else {
        format!("We sent an email to {}. Click the link in it on {here}, or enter the {}-digit code.", ec.address, lm_account::code::LEN)
    };
    ui.label(RichText::new(said).size(BODY));
    ui.add_space(10.0);
    let label = format!("{}-digit code", lm_account::code::LEN);
    ui.horizontal(|ui| {
        let hint = ui.visuals().weak_text_color();
        let field = egui::TextEdit::singleline(&mut ec.code)
            .font(egui::FontId::monospace(22.0))
            .hint_text(RichText::new("000000").font(egui::FontId::monospace(22.0)).color(hint))
            .horizontal_align(egui::Align::Center)
            .desired_width(132.0)
            .margin(vec2(8.0, 5.0))
            .id(egui::Id::new("lm-account-code"));
        let r = ui.add_enabled(!offline && !checking, field);
        crate::a11y::label(&r, &label);
        if r.changed() {
            // Digits only, at most six: a pasted "123 456" or "123-456" becomes 123456.
            ec.code = lm_account::code::field_text(&ec.code);
            ec.error = None;
        }
        if std::mem::take(&mut ec.focus) {
            r.request_focus();
        }
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ready = ec.code.len() == lm_account::code::LEN && !offline && !checking;
        let verify = egui::Button::new(RichText::new("Verify").size(BODY).color(t.on_accent)).fill(t.accent).min_size(vec2(80.0, 34.0));
        if (ui.add_enabled(ready, verify).clicked() || (enter && ready)) && out.is_none() {
            out = Some(Pane::Verify);
        }
        if checking {
            ui.spinner();
        }
    });
    if let Some(err) = &ec.error {
        ui.add_space(4.0);
        let r = ui.label(RichText::new(err).size(CAPTION).color(t.critical));
        crate::a11y::live(ui.ctx(), r.id, err);
    } else if ec.link_expired {
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("The link has stopped working on {here}; the code still works for an hour."))
                .size(CAPTION)
                .color(t.text_secondary),
        );
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(!offline && !checking, egui::Button::new("Send a New Email")).clicked() {
            out = Some(Pane::Resend);
        }
        if ui.add_enabled(!checking, egui::Button::new("Use a Different Email")).clicked() {
            out = Some(Pane::DifferentEmail);
        }
    });
    if offline {
        ui.add_space(6.0);
        ui.label(RichText::new(AccountError::Offline.to_string()).size(CAPTION).color(t.text_secondary));
    }
    out
}

/// The Ask box's prompt in place of its field while signing in is needed.
pub(crate) fn ask_prompt(shell: &mut Shell, ui: &mut Ui, t: &theme::Theme, width: f32) {
    crate::chrome::panel_frame(t).show(ui, |ui| {
        ui.set_width(width - 2.0 * theme::PANEL_PADDING);
        let inner = width - 2.0 * theme::PANEL_PADDING;
        ui.horizontal(|ui| {
            ui.label(RichText::new(egui_phosphor::regular::SPARKLE).font(theme::icon_font(18.0)).color(t.accent));
            // The words wrap beside the button rather than running under it.
            let words = (inner - 24.0 - 96.0 - 2.0 * ui.spacing().item_spacing.x).max(120.0);
            ui.vertical(|ui| {
                ui.set_width(words);
                ui.label(RichText::new(format!("Sign in to use {}", lm_core::brand::PRO_NAME)).font(theme::semibold(BODY)));
                ui.label(
                    RichText::new("Asking about your layers needs a free account. Your maps and files stay on this computer.")
                        .size(CAPTION)
                        .color(t.text_secondary),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let b = egui::Button::new(RichText::new("Sign In…").color(t.on_accent)).fill(t.accent).min_size(vec2(80.0, 28.0));
                if ui.add(b).clicked() {
                    open(shell);
                }
            });
        });
    });
}

#[cfg(test)]
pub(crate) fn test_ui(signed_in: bool, region: Region) -> AccountUi {
    let acc = tests::account(tests::INTL);
    if signed_in {
        acc.adopt(lm_account::Session {
            user: User {
                id: "6f1d2c3b-4a59-4e8f-9a0b-1c2d3e4f5a6b".into(),
                provider: "google".into(),
                email: Some("kim@example.com".into()),
            },
            region: Region::International,
            access: None,
            refresh: "r".into(),
        });
    }
    let mut ui = AccountUi::new(acc, region);
    ui.key_present = Some(false);
    ui
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn account(config: &str) -> Arc<Account> {
        struct NoNet;
        impl lm_account::http::Http for NoNet {
            fn send(&self, _: &str, _: &str, _: &[(&str, String)], _: Option<&str>) -> Result<lm_account::http::Response, AccountError> {
                Err(AccountError::Network("none".into()))
            }
        }
        let c = lm_account::Config::parse(config).unwrap_or_default();
        Arc::new(Account::new(c, Arc::new(NoNet), Arc::new(lm_account::store::MemoryStore::default())))
    }

    pub(crate) const INTL: &str = r#"{"international": {"auth_url": "https://a.example", "anon_key": "k", "proxy_url": "https://a.example/f",
        "providers": ["apple", "google", "x", "email"]}}"#;

    #[test]
    fn without_a_config_the_ask_box_is_as_before() {
        let mut a = AccountUi::default();
        assert!(!a.available() && !a.needs_sign_in());
        a.key_present = Some(true);
        assert_eq!(a.route(), AskRoute::Direct);
        a.key_present = Some(false);
        assert_eq!(a.route(), AskRoute::Unavailable);
    }

    #[test]
    fn with_a_config_signing_in_is_needed_and_the_proxy_is_used() {
        let acc = account(INTL);
        let mut a = AccountUi::new(acc.clone(), Region::International);
        a.key_present = Some(true);
        assert!(a.available());
        if std::env::var_os("LM_ASK_ROUTE").is_none() {
            assert!(a.needs_sign_in());
            assert_eq!(a.route(), AskRoute::Proxy);
        }
        acc.adopt(lm_account::Session {
            user: User { id: "u".into(), provider: "google".into(), email: Some("kim@example.com".into()) },
            region: Region::International,
            access: None,
            refresh: "r".into(),
        });
        assert!(!a.needs_sign_in());
        // China has no service in this config: no prompt, the old route.
        let mut c = AccountUi::new(account(INTL), Region::China);
        c.key_present = Some(false);
        assert!(!c.needs_sign_in());
        assert_eq!(c.route(), AskRoute::Unavailable);
    }
}
