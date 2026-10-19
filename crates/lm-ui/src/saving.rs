//! Keeping maps (WP5.1, WP5.2): Save, Save As and Open of `.lmap` files, the document's name and
//! Edited state for the window title, autosave and the recovery prompt, and relinking a source
//! that moved (the missing-source card).
//!
//! The shell never opens dialogs: Save As and Locate… ask the app through `FileRequest`s.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use egui::{Align2, CornerRadius, RichText, Stroke, Ui};
use egui_phosphor::regular as icon;
use lm_core::document::{DocLayer, Document, Transaction};
use lm_core::model::{LayerId, SourceRef};
use lm_core::recipe::ResultRecord;
use lm_core::{i18n, tl, tn};
use lm_jobs::import::LayerData;

use crate::theme::{self, CAPTION, Theme};
use crate::workspace::Workspace;
use crate::{FileRequest, Shell};

/// Autosave runs this often while there are unsaved changes (seconds).
pub const AUTOSAVE_SECS: f64 = 30.0;

/// The document's file, its saved state and its autosave.
#[derive(Default)]
pub struct DocFile {
    /// The user's file, once saved or opened.
    pub path: Option<PathBuf>,
    /// The file is a map package (WP6.5): saving it again writes a package, with the data.
    pub package: bool,
    /// The document as last saved or opened: what Edited compares with.
    saved: Document,
    /// Set after a restore from an autosave: edited whatever the comparison says.
    restored: bool,
    edited: bool,
    checked_at: f64,
    /// The recovery journal (set by the app; harness runs have none).
    journal: Option<lm_doc::Journal>,
    /// The launch after a crash: what to offer to restore.
    pub recovery: Option<lm_doc::Recovery>,
    /// The document as last autosaved, and when.
    autosaved: Option<Document>,
    autosave_at: f64,
    autosaving: Option<Receiver<Result<(), String>>>,
    was_focused: bool,
    /// A relink waiting for the user's yes (the file looks different).
    pub relink: Option<RelinkAsk>,
    /// Closing asks first when there are unsaved changes (set by the app).
    pub confirm_close: bool,
    /// The window asked to close and "Save changes?" is showing.
    pub closing: bool,
    /// Closing may go ahead (the changes were saved, or the user chose Don't Save).
    close_ok: bool,
    /// A save started by "Save changes?" closes the window once it succeeds.
    close_after_save: bool,
}

/// "This file looks different (its size changed). Use it anyway?"
#[derive(Clone, Debug, PartialEq)]
pub struct RelinkAsk {
    pub layer: LayerId,
    /// The chosen file's fingerprint.
    pub source: SourceRef,
    /// "its size changed" (English; translated where drawn).
    pub reason: &'static str,
}

impl RelinkAsk {
    pub fn question(&self) -> String {
        i18n::fmt("This file looks different ({reason}). Use it anyway?", &[("reason", tl!(self.reason))])
    }
}

impl DocFile {
    /// "Heat study", or "Untitled" before the first save.
    pub fn name(&self) -> String {
        self.path.as_deref().and_then(|p| p.file_stem()).map_or_else(|| tl!("Untitled").to_string(), |s| s.to_string_lossy().to_string())
    }

    /// Whether the document has changes its file doesn't have.
    pub fn edited(&self) -> bool {
        self.edited
    }
}

/// Why a chosen file doesn't match a source's fingerprint, if it doesn't.
pub fn mismatch(old: &SourceRef, new: &SourceRef) -> Option<&'static str> {
    if old.size != new.size {
        return Some(tn!("its size changed"));
    }
    match (&old.sha256, &new.sha256) {
        (Some(a), Some(b)) if !a.eq_ignore_ascii_case(b) => Some(tn!("its contents changed")),
        (Some(_), Some(_)) => None,
        _ if old.mtime != new.mtime => Some(tn!("its modification date changed")),
        _ => None,
    }
}

/// The layers whose data no source file can give back, with that data: NimbleMap's own layers
/// (results, drawn areas), layers with joined tables, and layers a kept result added fields to.
pub fn owned(ws: &Workspace) -> Vec<(LayerId, lm_doc::OwnedData)> {
    ws.doc
        .layers
        .iter()
        .filter(|l| {
            !lm_doc::is_file_source(&l.profile.source)
                || !l.profile.joins.is_empty()
                || ws.doc.results.iter().any(|r| r.layer == Some(l.id))
        })
        .filter_map(|l| {
            let data = match &ws.data(l.id)?.data {
                LayerData::Vector(ds) => lm_doc::OwnedData::Vector(ds.clone()),
                LayerData::Raster(r) => lm_doc::OwnedData::Raster(r.clone()),
            };
            Some((l.id, data))
        })
        .collect()
}

impl Workspace {
    /// Points every layer read from `old_path` at the file `fresh` describes, as one undo step,
    /// and reads it for the layers that have no data (WP5.2). Returns false if nothing changed.
    pub fn relink(&mut self, old_path: &str, fresh: &SourceRef) -> bool {
        let layers: Vec<DocLayer> = self.doc.layers.iter().filter(|l| l.profile.source.path == old_path).cloned().collect();
        let mut ops = Vec::new();
        for l in &layers {
            let mut p = l.profile.clone();
            p.source = SourceRef { member: p.source.member.clone(), ..fresh.clone() };
            match self.doc.set_profile_tx(l.id, p) {
                Some(tx) => {
                    // Each op is applied as it's gathered so the next one starts from it.
                    if self.doc.apply(&tx).is_err() {
                        return false;
                    }
                    ops.extend(tx.ops);
                }
                None => continue,
            }
        }
        if ops.is_empty() {
            return false;
        }
        // The undo label stays English, like the document's other transaction labels.
        let tx = Transaction::new(format!("Relink {}", fresh.file_name()), ops);
        let _ = self.doc.apply(&tx.inverse());
        if !self.commit(Some(tx)) {
            return false;
        }
        let ids: Vec<LayerId> = layers.iter().map(|l| l.id).collect();
        self.missing.retain(|id| !ids.contains(id));
        let to_read: Vec<DocLayer> =
            ids.iter().filter(|id| self.data(**id).is_none()).filter_map(|id| self.doc.layer(*id).cloned()).collect();
        if !to_read.is_empty() {
            self.read_saved(fresh.path.clone(), to_read);
        }
        true
    }

    /// A kept result's status line: "Source changed: schools.csv" when an input's file no longer
    /// matches the fingerprint in the recipe (or can't be found).
    pub fn result_status(&self, r: &ResultRecord) -> Option<String> {
        r.status(|i| {
            if i.fingerprint == "drawn" {
                return Some(i.fingerprint.clone());
            }
            let layer = self
                .doc
                .layers
                .iter()
                .find(|l| l.profile.source.file_name() == i.file)
                .or_else(|| self.doc.layers.iter().find(|l| l.display_name == i.name && lm_doc::is_file_source(&l.profile.source)))?;
            lm_jobs::fingerprint::of_path(Path::new(&layer.profile.source.path))
        })
    }
}

/// Starts the recovery journal in `dir`; after a crash the prompt offers the autosave.
pub(crate) fn start(shell: &mut Shell, dir: &Path) {
    match lm_doc::Journal::begin(dir) {
        Ok((j, r)) => {
            // Opened packages unpack beside the autosaves, so a restored autosave finds them.
            if let Some(parent) = dir.parent() {
                lm_doc::package::set_unpack_dir(parent.join("Packages"));
            }
            shell.doc_file.journal = Some(j);
            shell.doc_file.recovery = r;
            shell.doc_file.confirm_close = true;
        }
        Err(e) => log::warn!("autosave unavailable: {e}"),
    }
}

/// The document with the current view, as it is saved.
fn current(shell: &mut Shell) -> Document {
    let c = shell.map.center_lonlat();
    shell.ws.doc.view = Some((c.x, c.y, shell.map.zoom()));
    shell.ws.doc.clone()
}

/// ⌘S: saves to the document's file, or asks where (Save As) the first time.
pub(crate) fn save(shell: &mut Shell, t: f64) {
    // A save the user starts never closes the window (only "Save changes?" → Save does).
    shell.doc_file.close_after_save = false;
    save_or_ask(shell, t);
}

fn save_or_ask(shell: &mut Shell, t: f64) {
    match shell.doc_file.path.clone() {
        Some(p) => save_to(shell, p, t),
        None => ask_where(shell),
    }
}

/// ⇧⌘S: asks the app for a file to save to.
pub(crate) fn save_as(shell: &mut Shell) {
    shell.doc_file.close_after_save = false;
    ask_where(shell);
}

fn ask_where(shell: &mut Shell) {
    let suggested_name = format!("{}.{}", shell.doc_file.name(), lm_doc::EXTENSION);
    shell.file_requests.push(FileRequest::SaveAs { suggested_name });
}

/// Saves the document as a `.lmap` file at `path` (the extension is added if missing).
pub(crate) fn save_to(shell: &mut Shell, path: PathBuf, t: f64) {
    let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(lm_doc::EXTENSION)) {
        path
    } else {
        let mut s = path.into_os_string();
        s.push(".");
        s.push(lm_doc::EXTENSION);
        PathBuf::from(s)
    };
    let doc = current(shell);
    let owned = owned(&shell.ws);
    let written = if shell.doc_file.package {
        // A package stays a package: its data goes back in, with the licences known.
        let licenses = lm_doc::package::sources(&doc, &owned.iter().map(|(id, _)| *id).collect::<Vec<_>>())
            .iter()
            .filter_map(|p| crate::package::license_of(shell, Path::new(&p.path)).map(|l| (p.path.clone(), l)))
            .collect();
        let opts = lm_doc::package::PackageOptions { dropped: Vec::new(), licenses };
        lm_doc::package::save_package(&doc, &owned, &opts, &path).map(|r| r.save)
    } else {
        lm_doc::save_lmap(&doc, &owned, &path)
    };
    match written {
        Ok(report) => {
            let f = &mut shell.doc_file;
            f.path = Some(path);
            f.saved = doc;
            f.restored = false;
            f.edited = false;
            if let Some(j) = &mut f.journal
                && let Err(e) = j.nothing_unsaved()
            {
                log::warn!("journal: {e}");
            }
            if let Some((id, why)) = report.skipped.first() {
                let name = shell.ws.doc.layer(*id).map_or_else(String::new, |l| l.display_name.clone());
                shell.notice =
                    Some((i18n::fmt("Saved, but {name} couldn't be kept in the map file ({why}).", &[("name", &name), ("why", why)]), t));
            }
            if shell.doc_file.close_after_save {
                shell.doc_file.close_ok = true;
            }
        }
        Err(e) => {
            shell.doc_file.close_after_save = false;
            shell.notice = Some((e.to_string(), t));
        }
    }
}

/// Opens a document: a `.lmap` file or a v0 document folder. It replaces the current one.
pub(crate) fn open(shell: &mut Shell, ctx: &egui::Context, path: &Path, t: f64) {
    match lm_doc::open_any(path) {
        Ok(loaded) => {
            let is_lmap = path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case(lm_doc::EXTENSION));
            let package = loaded.package;
            install(shell, ctx, loaded, t);
            shell.doc_file.path = is_lmap.then(|| path.to_path_buf());
            shell.doc_file.package = is_lmap && package;
            shell.doc_file.restored = !is_lmap;
        }
        Err(e) => shell.notice = Some((e.to_string(), t)),
    }
}

/// Puts an opened document in place of the current one.
fn install(shell: &mut Shell, ctx: &egui::Context, loaded: lm_doc::Loaded, t: f64) {
    let view = loaded.doc.view;
    let owned: Vec<(LayerId, LayerData)> = loaded
        .owned
        .into_iter()
        .map(|(id, d)| {
            (
                id,
                match d {
                    lm_doc::OwnedData::Vector(ds) => LayerData::Vector(ds),
                    lm_doc::OwnedData::Raster(r) => LayerData::Raster(r),
                },
            )
        })
        .collect();
    shell.ws = Workspace::with_waker(ctx);
    shell.ws.restore_owned(loaded.doc.clone(), owned);
    shell.find.clear();
    shell.table_for = None;
    shell.composer = None;
    let f = &mut shell.doc_file;
    f.saved = loaded.doc;
    f.edited = false;
    f.relink = None;
    f.autosaved = None;
    if let Some(n) = loaded.notes.into_iter().next() {
        shell.notice = Some((n, t));
    }
    if let Some((lon, lat, zoom)) = view {
        shell.map.fly_to(lm_mapview::Coord::new(lon, lat), zoom, t, shell.last_map_size, true);
    }
}

/// Restore or Discard on the recovery prompt.
pub(crate) fn answer_recovery(shell: &mut Shell, ctx: &egui::Context, restore: bool, t: f64) {
    let Some(r) = shell.doc_file.recovery.take() else { return };
    if restore {
        match lm_doc::load_lmap(&r.autosave) {
            Ok(loaded) => {
                install(shell, ctx, loaded, t);
                shell.doc_file.path = r.document.clone();
                // The autosave is a plain map; the user's file says whether it was a package.
                shell.doc_file.package = r.document.as_deref().is_some_and(lm_doc::is_package);
                shell.doc_file.restored = true;
                shell.doc_file.edited = true;
            }
            Err(e) => shell.notice = Some((i18n::fmt("The autosaved map could not be restored. {error}", &[("error", &e.to_string())]), t)),
        }
    }
    if let Some(j) = &mut shell.doc_file.journal
        && let Err(e) = j.answered()
    {
        log::warn!("journal: {e}");
    }
}

/// Locate…: the user picked `path` for a missing layer's source. A file that matches the
/// fingerprint is used at once; one that doesn't asks first.
pub(crate) fn relink(shell: &mut Shell, id: LayerId, path: &Path, t: f64) {
    let Some(old) = shell.ws.doc.layer(id).map(|l| l.profile.source.clone()) else { return };
    let fresh = match lm_doc::fingerprint(path, old.sha256.is_some()) {
        Ok(f) => f,
        Err(e) => {
            shell.notice = Some((e.to_string(), t));
            return;
        }
    };
    match mismatch(&old, &fresh) {
        None => {
            shell.ws.relink(&old.path, &fresh);
        }
        Some(reason) => shell.doc_file.relink = Some(RelinkAsk { layer: id, source: fresh, reason }),
    }
}

/// The answer to "Use it anyway?". Yes relinks; the kept results made from the old file then
/// show "Source changed", since the new file no longer matches their recipes.
pub(crate) fn answer_relink(shell: &mut Shell, yes: bool, t: f64) {
    let Some(ask) = shell.doc_file.relink.take() else { return };
    if !yes {
        return;
    }
    let Some(old) = shell.ws.doc.layer(ask.layer).map(|l| l.profile.source.path.clone()) else { return };
    if shell.ws.relink(&old, &ask.source) {
        let n = shell.ws.doc.results.iter().filter(|r| shell.ws.result_status(r).is_some()).count();
        if n > 0 {
            let template = if n == 1 {
                tn!("Using {file}. 1 result made from the old file now say Source changed.")
            } else {
                tn!("Using {file}. {n} results made from the old file now say Source changed.")
            };
            shell.notice = Some((i18n::fmt(template, &[("file", ask.source.file_name()), ("n", &n.to_string())]), t));
        }
    }
}

/// Removes a missing layer (the card's Remove layer), as one undo step.
fn remove_layer(shell: &mut Shell, id: LayerId) {
    if let Some(tx) = shell.ws.doc.remove_layer_tx(id) {
        shell.ws.commit(Some(tx));
        shell.ws.missing.retain(|m| *m != id);
        shell.ws.clear_selection();
    }
}

/// Each frame: Edited, autosave every 30 s and on losing focus, finished autosaves.
pub(crate) fn frame(shell: &mut Shell, ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    let f = &mut shell.doc_file;
    if now - f.checked_at >= 0.25 || now < f.checked_at {
        f.checked_at = now;
        f.edited = f.restored || shell.ws.doc != f.saved;
    }
    if let Some(rx) = &f.autosaving {
        match rx.try_recv() {
            Ok(res) => {
                f.autosaving = None;
                match res {
                    Ok(()) => {
                        let name = f.name();
                        if let Some(j) = &mut f.journal
                            && let Err(e) = j.autosaved(&name, f.path.as_deref())
                        {
                            log::warn!("journal: {e}");
                        }
                    }
                    Err(e) => log::warn!("autosave: {e}"),
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => f.autosaving = None,
        }
    }
    if f.journal.is_none() {
        return;
    }
    if !f.edited {
        f.autosaved = None;
        if let Some(j) = &mut f.journal
            && let Err(e) = j.nothing_unsaved()
        {
            log::warn!("journal: {e}");
        }
        return;
    }
    let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
    let lost_focus = f.was_focused && !focused;
    f.was_focused = focused;
    let due = now - f.autosave_at >= AUTOSAVE_SECS || now < f.autosave_at;
    if due || lost_focus {
        if f.autosaving.is_none() && f.autosaved.as_ref() != Some(&shell.ws.doc) {
            autosave(shell, now);
        } else {
            // Nothing new since the last autosave (or one is being written): look again later.
            f.autosave_at = now;
        }
    } else {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(AUTOSAVE_SECS - (now - f.autosave_at)));
    }
}

/// Writes the autosave off the interface thread.
fn autosave(shell: &mut Shell, now: f64) {
    let Some(path) = shell.doc_file.journal.as_ref().map(|j| j.autosave_path()) else { return };
    let doc = current(shell);
    let owned = owned(&shell.ws);
    let (tx, rx) = std::sync::mpsc::channel();
    let snapshot = shell.ws.doc.clone();
    let spawned = std::thread::Builder::new().name("lm-autosave".into()).spawn(move || {
        let _ = tx.send(lm_doc::save_lmap(&doc, &owned, &path).map(|_| ()).map_err(|e| e.to_string()));
    });
    let f = &mut shell.doc_file;
    f.autosave_at = now;
    if spawned.is_ok() {
        f.autosaving = Some(rx);
        f.autosaved = Some(snapshot);
    }
}

/// Whether the window may close now. With unsaved changes it asks first (Save changes?).
pub(crate) fn may_close(shell: &mut Shell) -> bool {
    let f = &mut shell.doc_file;
    if f.close_ok || !f.confirm_close || !f.edited || shell.ws.doc.layers.is_empty() {
        return true;
    }
    f.closing = true;
    false
}

/// A clean quit: the journal is marked clean and the autosave removed.
pub(crate) fn shutdown(shell: &mut Shell) {
    if let Some(rx) = shell.doc_file.autosaving.take() {
        let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
    }
    if let Some(j) = &mut shell.doc_file.journal
        && let Err(e) = j.end_clean()
    {
        log::warn!("journal: {e}");
    }
}

fn accent_button(ui: &mut Ui, t: &Theme, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).color(t.on_accent)).fill(t.accent))
}

/// A centred prompt over the map with a dimmed backdrop: a line of text and buttons.
fn prompt<R>(ui: &Ui, t: &Theme, id: &str, map: egui::Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    let ctx = ui.ctx();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new((id, "dim"))));
    painter.rect_filled(map, CornerRadius::ZERO, egui::Color32::from_black_alpha(if t.dark { 110 } else { 60 }));
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .pivot(Align2::CENTER_CENTER)
        .fixed_pos(map.center())
        .show(ctx, |ui| {
            crate::chrome::panel_frame(t)
                .show(ui, |ui| {
                    ui.set_width(380.0);
                    add(ui)
                })
                .inner
        })
        .inner
}

/// "NimbleMap quit unexpectedly. Restore 'Heat study' from 2 minutes ago?" Restore · Discard.
pub(crate) fn recovery_prompt(shell: &mut Shell, ui: &Ui, map: egui::Rect) {
    let Some(r) = shell.doc_file.recovery.clone() else { return };
    let t = *shell.theme();
    let text = r.question(shell.app_name(), lm_doc::container::now_secs());
    let answer = prompt(ui, &t, "lm-recovery", map, |ui| {
        let mut answer = None;
        ui.horizontal_top(|ui| {
            ui.label(RichText::new(icon::CLOCK_COUNTER_CLOCKWISE).font(theme::icon_font(22.0)).color(t.accent));
            ui.add(egui::Label::new(RichText::new(&text).font(theme::semibold(theme::BODY))).wrap());
        });
        ui.add_space(4.0);
        ui.label(RichText::new(tl!("Your own file has not been changed.")).size(CAPTION).color(t.text_secondary));
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if accent_button(ui, &t, tl!("Restore")).clicked() {
                answer = Some(true);
            }
            if ui.button(tl!("Discard")).clicked() {
                answer = Some(false);
            }
        });
        answer
    });
    if let Some(restore) = answer {
        let now = ui.input(|i| i.time);
        answer_recovery(shell, &ui.ctx().clone(), restore, now);
    }
}

/// "Do you want to save the changes you made to 'Heat study'?" Save · Don't Save · Cancel.
pub(crate) fn close_prompt(shell: &mut Shell, ui: &Ui, map: egui::Rect) {
    if !shell.doc_file.closing {
        return;
    }
    let t = *shell.theme();
    let text = i18n::fmt("Do you want to save the changes you made to '{name}'?", &[("name", &shell.doc_file.name())]);
    let answer = prompt(ui, &t, "lm-close", map, |ui| {
        let mut answer = None;
        ui.add(egui::Label::new(RichText::new(&text).font(theme::semibold(theme::BODY))).wrap());
        ui.label(RichText::new(tl!("Your changes will be lost if you don't save them.")).size(CAPTION).color(t.text_secondary));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(tl!("Don't Save")).clicked() {
                answer = Some(0);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if accent_button(ui, &t, tl!("Save")).clicked() {
                    answer = Some(2);
                }
                if ui.button(tl!("Cancel")).clicked() {
                    answer = Some(1);
                }
            });
        });
        answer
    });
    let Some(a) = answer else { return };
    let now = ui.input(|i| i.time);
    shell.doc_file.closing = false;
    match a {
        0 => shell.doc_file.close_ok = true,
        2 => {
            shell.doc_file.close_after_save = true;
            save_or_ask(shell, now);
        }
        _ => {}
    }
    if a != 1 && shell.doc_file.close_ok {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// After "Save changes?" → Save → Save As, the window closes once the save succeeded.
pub(crate) fn close_if_saved(shell: &mut Shell, ctx: &egui::Context) {
    let f = &mut shell.doc_file;
    if f.close_after_save && f.close_ok {
        f.close_after_save = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The missing-source card (Error and status copy): "Can't find heat_july.tif. It was last seen
/// in Downloads." Locate… · Remove layer. After Locate…, a file that looks different asks first.
pub(crate) fn missing_card(shell: &mut Shell, ui: &mut Ui, t: &Theme, l: &DocLayer) {
    if !shell.ws.missing.contains(&l.id) {
        return;
    }
    let ask = shell.doc_file.relink.clone().filter(|a| a.layer == l.id);
    let mut action = None;
    egui::Frame::new()
        .fill(t.surface_raised)
        .stroke(Stroke::new(1.0, t.warning.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(theme::RADIUS_CARD))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let line = match &ask {
                Some(a) => a.question(),
                None => crate::workspace::missing_line(&l.profile.source),
            };
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(icon::WARNING).font(theme::icon_font(14.0)).color(t.warning));
                ui.add(egui::Label::new(RichText::new(line).font(theme::semibold(theme::BODY))).wrap());
            });
            if let Some(a) = &ask {
                ui.label(RichText::new(a.source.file_name()).size(CAPTION).color(t.text_secondary));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| match &ask {
                Some(_) => {
                    if accent_button(ui, t, tl!("Use It")).clicked() {
                        action = Some(2);
                    }
                    if ui.button(tl!("Cancel")).clicked() {
                        action = Some(3);
                    }
                }
                None => {
                    if accent_button(ui, t, tl!("Locate…")).clicked() {
                        action = Some(0);
                    }
                    if ui.button(tl!("Remove layer")).clicked() {
                        action = Some(1);
                    }
                }
            });
        });
    ui.add_space(8.0);
    let now = ui.input(|i| i.time);
    match action {
        Some(0) => shell.file_requests.push(FileRequest::Locate { layer: l.id, file_name: l.profile.source.file_name().to_string() }),
        Some(1) => remove_layer(shell, l.id),
        Some(2) => answer_relink(shell, true, now),
        Some(3) => answer_relink(shell, false, now),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lm_core::recipe::{Recipe, RecipeInput};

    fn src(path: &str, size: u64, mtime: i64, sha: Option<&str>) -> SourceRef {
        SourceRef { path: path.into(), member: None, size, mtime, sha256: sha.map(String::from) }
    }

    #[test]
    fn mismatch_names_what_changed() {
        let a = src("/a/x.csv", 10, 100, Some("aa"));
        assert_eq!(mismatch(&a, &src("/b/x.csv", 11, 100, Some("aa"))), Some("its size changed"));
        assert_eq!(mismatch(&a, &src("/b/x.csv", 10, 200, Some("bb"))), Some("its contents changed"));
        assert_eq!(mismatch(&a, &src("/b/x.csv", 10, 200, Some("AA"))), None, "touched, same bytes");
        let b = src("/a/x.csv", 10, 100, None);
        assert_eq!(mismatch(&b, &src("/b/x.csv", 10, 200, None)), Some("its modification date changed"));
        assert_eq!(mismatch(&b, &src("/b/x.csv", 10, 100, None)), None);
        let ask = RelinkAsk { layer: LayerId(1), source: a, reason: "its size changed" };
        assert_eq!(ask.question(), "This file looks different (its size changed). Use it anyway?");
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lm-ui-relink-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn wait(ws: &mut Workspace) {
        for _ in 0..500 {
            ws.poll();
            if ws.idle() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("background work did not finish");
    }

    const CSV: &str = "name,lon,lat,enrolment\nA,121.4,31.2,300\nB,121.5,31.3,120\n";

    /// A document with schools.csv and a kept result made from it, saved, the file moved away.
    fn reopened_with_moved_source(tag: &str) -> (PathBuf, Workspace, PathBuf) {
        let dir = temp(tag);
        let csv = dir.join("Downloads").join("schools.csv");
        std::fs::create_dir_all(csv.parent().unwrap()).unwrap();
        std::fs::write(&csv, CSV).unwrap();
        let mut ws = Workspace::default();
        let layers = lm_jobs::import::import_path(&csv, &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        ws.add_imported(layers, 1);
        let id = ws.doc.layers[0].id;
        let recipe = Recipe {
            title: "Schools above 200".into(),
            action: "filter".into(),
            params: Default::default(),
            computed_at: 0,
            software: lm_core::brand::NAME.into(),
            inputs: vec![RecipeInput {
                name: "schools".into(),
                file: "schools.csv".into(),
                fingerprint: lm_jobs::fingerprint::of_path(&csv).unwrap(),
                count: 2,
                noun: "points".into(),
            }],
            scope: String::new(),
            method: String::new(),
            decisions: vec![],
            assumptions: vec![],
            coordinates: None,
            excluded: vec![],
        };
        ws.doc.results.push(ResultRecord {
            id: 1,
            layer: None,
            recipe,
            version: 1,
            history: vec![],
            card: Default::default(),
            about: Some(id),
        });
        assert_eq!(ws.result_status(&ws.doc.results[0]), None);
        let path = dir.join("study.lmap");
        lm_doc::save_lmap(&ws.doc, &owned(&ws), &path).unwrap();
        let moved = dir.join("Moved").join("schools.csv");
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::rename(&csv, &moved).unwrap();
        let mut back = Workspace::default();
        let loaded = lm_doc::load_lmap(&path).unwrap();
        back.restore_owned(loaded.doc, Vec::new());
        wait(&mut back);
        assert_eq!(back.missing, vec![id]);
        assert_eq!(back.selected(), Some(id));
        assert!(back.data(id).is_none());
        assert_eq!(
            crate::workspace::missing_line(&back.doc.layers[0].profile.source),
            "Can't find schools.csv. It was last seen in Downloads."
        );
        (dir, back, moved)
    }

    #[test]
    fn relinking_to_the_same_file_restores_the_layer() {
        let (dir, mut ws, moved) = reopened_with_moved_source("same");
        let old = ws.doc.layers[0].profile.source.clone();
        let fresh = lm_doc::fingerprint(&moved, old.sha256.is_some()).unwrap();
        assert_eq!(mismatch(&old, &fresh), None);
        assert!(ws.relink(&old.path, &fresh));
        wait(&mut ws);
        let id = ws.doc.layers[0].id;
        assert!(ws.missing.is_empty());
        assert!(ws.data(id).is_some());
        assert_eq!(ws.doc.layers[0].profile.source.path, fresh.path);
        assert_eq!(ws.result_status(&ws.doc.results[0]), None, "same file: the result still stands");
        // Relinking is one undo step.
        assert_eq!(ws.undo(), Some("Relink schools.csv".into()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn relinking_to_a_different_file_marks_results_source_changed() {
        let (dir, mut ws, moved) = reopened_with_moved_source("diff");
        std::fs::write(&moved, format!("{CSV}C,121.6,31.4,80\n")).unwrap();
        let old = ws.doc.layers[0].profile.source.clone();
        let fresh = lm_doc::fingerprint(&moved, old.sha256.is_some()).unwrap();
        assert_eq!(mismatch(&old, &fresh), Some("its size changed"));
        assert!(ws.relink(&old.path, &fresh));
        wait(&mut ws);
        let id = ws.doc.layers[0].id;
        assert!(ws.data(id).is_some());
        assert_eq!(ws.data(id).map(|d| d.profile.count), Some(ws.doc.layers[0].profile.count));
        assert_eq!(ws.result_status(&ws.doc.results[0]).as_deref(), Some("Source changed: schools.csv"));
        let _ = std::fs::remove_dir_all(dir);
    }

    fn shell(ctx: &egui::Context) -> Shell {
        Shell::new(
            ctx,
            lm_core::phase::Flags::default(),
            crate::Prefs { basemap: "offline".into(), ..Default::default() },
            crate::System::default(),
        )
    }

    /// WP6.5: a map package opens on "another computer" — the original files moved away — with
    /// the same layers, styles and results; undo, Edited and autosave keep working on it, and
    /// saving it keeps it a package.
    #[test]
    fn a_package_reopens_elsewhere_with_the_same_map() {
        let dir = temp("package");
        let csv = dir.join("Downloads").join("schools.csv");
        std::fs::create_dir_all(csv.parent().unwrap()).unwrap();
        std::fs::write(&csv, CSV).unwrap();
        let ctx = egui::Context::default();
        let mut s = shell(&ctx);
        let ctx_i = lm_jobs::import::Ctx::detached(Default::default());
        s.ws.add_imported(lm_jobs::import::import_path(&csv, &ctx_i).unwrap(), 1);
        let schools = s.ws.doc.layers[0].id;
        let mut style = s.ws.doc.layers[0].style.clone();
        style.fill = [200, 30, 30, 255];
        style.point_size = 11.0;
        let tx = s.ws.doc.set_style_tx(schools, style).unwrap();
        assert!(s.ws.commit(Some(tx)));
        let area = lm_jobs::import::drawn_area("Area 1", &[(121.0, 31.0), (122.0, 31.0), (122.0, 32.0)]).unwrap();
        s.ws.add_imported(vec![area], 2);
        let fp = lm_jobs::fingerprint::of_path(&csv).unwrap();
        s.ws.doc.results.push(ResultRecord {
            id: 1,
            layer: None,
            recipe: Recipe {
                title: "Schools above 200".into(),
                action: "filter".into(),
                params: Default::default(),
                computed_at: 0,
                software: lm_core::brand::NAME.into(),
                inputs: vec![RecipeInput {
                    name: "schools".into(),
                    file: "schools.csv".into(),
                    fingerprint: fp,
                    count: 2,
                    noun: "points".into(),
                }],
                scope: String::new(),
                method: String::new(),
                decisions: vec![],
                assumptions: vec![],
                coordinates: None,
                excluded: vec![],
            },
            version: 1,
            history: vec![],
            card: Default::default(),
            about: Some(schools),
        });
        wait(&mut s.ws);
        let before = s.ws.doc.clone();

        // Share › Map package…, written in the background.
        let d = crate::package::PackageDialog::new(&s);
        assert_eq!(d.sources.len(), 1);
        s.package_inputs = Some(d.inputs(&s).unwrap());
        let path = dir.join("Out").join("Schools (package).lmap");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        crate::package::write_to(&mut s, path.clone(), 0.0);
        for _ in 0..500 {
            crate::package::poll(&mut s, 0.0);
            if s.package_job.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(s.package_job.is_none() && path.is_file(), "{:?}", s.notice);
        assert_eq!(s.ws.doc, before, "packaging leaves the user's map as it was");

        // "Another computer": the user's folder is gone.
        std::fs::remove_dir_all(dir.join("Downloads")).unwrap();
        let mut other = shell(&ctx);
        open(&mut other, &ctx, &path, 0.0);
        wait(&mut other.ws);
        assert!(other.doc_file.package && !other.doc_file.edited());
        assert!(other.ws.missing.is_empty(), "{:?}", other.notice);
        assert_eq!(other.ws.doc.layers.len(), before.layers.len());
        for (a, b) in other.ws.doc.layers.iter().zip(&before.layers) {
            assert_eq!((&a.display_name, &a.style, a.profile.count), (&b.display_name, &b.style, b.profile.count));
            assert_eq!(other.ws.data(a.id).map(|d| d.profile.count), s.ws.data(b.id).map(|d| d.profile.count), "{}", a.display_name);
        }
        assert_eq!(other.ws.doc.results, before.results);
        assert_eq!(other.ws.result_status(&other.ws.doc.results[0]), None, "the packaged file matches the recipe");

        // Undo and Edited work on the opened package.
        let mut style = other.ws.doc.layer(schools).unwrap().style.clone();
        style.point_size = 4.0;
        let tx = other.ws.doc.set_style_tx(schools, style).unwrap();
        assert!(other.ws.commit(Some(tx)));
        assert!(other.ws.undo().is_some());
        assert_eq!(other.ws.doc.layer(schools).unwrap().style.point_size, 11.0);

        // An autosave of it is a plain map pointing at the unpacked files, and opens whole.
        let autosave = dir.join("autosave.lmap");
        lm_doc::save_lmap(&other.ws.doc, &owned(&other.ws), &autosave).unwrap();
        let mut restored = Workspace::default();
        let loaded = lm_doc::load_lmap(&autosave).unwrap();
        assert!(!loaded.package);
        restored.restore_owned(loaded.doc, Vec::new());
        wait(&mut restored);
        assert!(restored.data(schools).is_some());

        // ⌘S keeps it a package.
        let again = dir.join("Out").join("again.lmap");
        save_to(&mut other, again.clone(), 0.0);
        assert!(lm_doc::is_package(&again), "{:?}", other.notice);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn owned_layers_are_saved_with_their_data_and_reopen_without_files() {
        let dir = temp("owned");
        let mut ws = Workspace::default();
        let area = lm_jobs::import::drawn_area("Area 1", &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]).unwrap();
        ws.add_imported(vec![area], 1);
        let id = ws.doc.layers[0].id;
        let o = owned(&ws);
        assert_eq!(o.len(), 1);
        let path = dir.join("area.lmap");
        lm_doc::save_lmap(&ws.doc, &o, &path).unwrap();
        let loaded = lm_doc::load_lmap(&path).unwrap();
        let data: Vec<(LayerId, LayerData)> = loaded
            .owned
            .into_iter()
            .map(|(i, d)| match d {
                lm_doc::OwnedData::Vector(ds) => (i, LayerData::Vector(ds)),
                lm_doc::OwnedData::Raster(r) => (i, LayerData::Raster(r)),
            })
            .collect();
        let mut back = Workspace::default();
        back.restore_owned(loaded.doc, data);
        wait(&mut back);
        assert!(back.missing.is_empty());
        let (Some(LayerData::Vector(a)), Some(LayerData::Vector(b))) = (ws.data(id).map(|d| &d.data), back.data(id).map(|d| &d.data))
        else {
            panic!("no data")
        };
        assert_eq!(a.features, b.features);
        assert_eq!(back.doc, ws.doc);
        let _ = std::fs::remove_dir_all(dir);
    }
}
