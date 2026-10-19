//! Share › Map package… (WP6.5): one `.lmap` file that carries the map and its data, so it opens
//! on another computer (`lm_doc::package`). Before anything is written the dialog shows what the
//! package holds and what it leaves out:
//!
//! - its size, estimated from the files and results it carries;
//! - each source with its licence — the examples' and the data library's are known; anything
//!   else reads "Unknown — check before sharing";
//! - the folder paths it removes (they name the user's account and folders);
//! - fields that look personal (people's names, emails, phone and ID numbers), each checked to be
//!   left out unless the map's style or filter uses it.
//!
//! The user's own map is untouched: packaging is an export, not a save.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use egui::{RichText, Ui, vec2};
use egui_phosphor::regular as icon;
use lm_core::model::LayerId;
use lm_doc::package::{PackageOptions, PackageReport, Review};
use lm_jobs::export::sensitive_fields;
use lm_jobs::import::LayerData;

use crate::library::size_text;
use crate::theme::{self, CAPTION, Theme};
use crate::{FileRequest, Shell, form};

/// "Unknown — check before sharing".
pub const UNKNOWN_LICENSE: &str = "Unknown — check before sharing";

/// One source the package carries.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceRow {
    pub path: String,
    pub file: String,
    pub size: u64,
    /// The layers read from it, by name.
    pub layers: Vec<String>,
    /// `None`: not known.
    pub license: Option<String>,
    /// The file can't be read now: its layers travel without data.
    pub missing: bool,
}

/// A field that looks personal.
#[derive(Clone, Debug, PartialEq)]
pub struct PrivateField {
    pub layer: LayerId,
    pub layer_name: String,
    pub field: String,
    /// "Looks like phone numbers".
    pub note: &'static str,
    /// Left out of the package.
    pub drop: bool,
    /// Why it can't be left out ("The map is colored by it").
    pub locked: Option<&'static str>,
}

/// The dialog's state.
pub struct PackageDialog {
    pub sources: Vec<SourceRow>,
    pub fields: Vec<PrivateField>,
    /// Layers whose data the package stores itself (results, joined layers), with their size
    /// estimates.
    owned: Vec<(LayerId, u64)>,
    /// Every layer's data size as a result, for layers whose fields get left out.
    as_result: Vec<(LayerId, u64)>,
    pub review: Review,
    /// Layers the package stores itself (results, drawn areas, layers losing fields), by name.
    pub stored: Vec<String>,
    pub error: Option<String>,
}

/// A package being written in the background, and where.
pub struct PackageJob {
    pub path: PathBuf,
    rx: Receiver<Result<PackageReport, String>>,
}

/// What the package will be written from once the user picks where.
pub struct PackageInputs {
    doc: lm_core::document::Document,
    owned: Vec<(LayerId, lm_doc::OwnedData)>,
    opts: PackageOptions,
    name: String,
}

fn to_owned(d: &LayerData) -> lm_doc::OwnedData {
    match d {
        LayerData::Vector(ds) => lm_doc::OwnedData::Vector(ds.clone()),
        LayerData::Raster(r) => lm_doc::OwnedData::Raster(r.clone()),
    }
}

/// The licence of the source at `path`, when NimbleMap knows it: an example's, or a data
/// library dataset's (from the catalog, when it has been read).
pub fn license_of(shell: &Shell, path: &Path) -> Option<String> {
    if let Some(l) = crate::examples::license_of(path) {
        return Some(l.to_string());
    }
    let datasets = shell.library.cache().join("datasets");
    let id = path.strip_prefix(&datasets).ok()?.components().next()?.as_os_str().to_str()?.to_string();
    Some(match shell.library.catalog.as_ref().and_then(|c| c.get(&id)) {
        Some(d) => d.license.name.clone(),
        None => "From the data library: see its licence there".into(),
    })
}

/// Why a field can't be left out of a layer: the style or the filter uses it.
fn used_by(l: &lm_core::document::DocLayer, field: &str) -> Option<&'static str> {
    let s = &l.style;
    if s.color_by.as_ref().is_some_and(|c| c.field == field) {
        return Some("The map is colored by it");
    }
    if s.size_by.as_ref().is_some_and(|c| c.field == field) {
        return Some("The map sizes points by it");
    }
    if s.labels.as_ref().is_some_and(|c| c.field == field) {
        return Some("The map labels by it");
    }
    if l.filter.as_ref().is_some_and(|f| f.conditions.iter().any(|c| c.field == field)) {
        return Some("The filter uses it");
    }
    None
}

impl PackageDialog {
    /// The dialog for the map the shell shows.
    pub fn new(shell: &Shell) -> PackageDialog {
        let owned: Vec<(LayerId, u64)> =
            crate::saving::owned(&shell.ws).iter().map(|(id, d)| (*id, lm_doc::package::estimate_owned(d))).collect();
        let mut fields = Vec::new();
        let mut as_result = Vec::new();
        for l in shell.ws.doc.layers.iter().rev() {
            let Some(data) = shell.ws.data(l.id) else { continue };
            as_result.push((l.id, lm_doc::package::estimate_owned(&to_owned(&data.data))));
            let LayerData::Vector(ds) = &data.data else { continue };
            for (f, s) in ds.fields.iter().zip(sensitive_fields(ds)) {
                let Some(s) = s else { continue };
                let locked = used_by(l, &f.name);
                fields.push(PrivateField {
                    layer: l.id,
                    layer_name: l.display_name.clone(),
                    field: f.name.clone(),
                    note: s.note(),
                    drop: locked.is_none(),
                    locked,
                });
            }
        }
        let mut d =
            PackageDialog { sources: Vec::new(), fields, owned, as_result, review: Review::default(), stored: Vec::new(), error: None };
        d.refresh(shell);
        d
    }

    /// The fields to leave out, by layer.
    pub fn dropped(&self) -> Vec<(LayerId, Vec<String>)> {
        let mut out: Vec<(LayerId, Vec<String>)> = Vec::new();
        for f in self.fields.iter().filter(|f| f.drop && f.locked.is_none()) {
            match out.iter_mut().find(|(id, _)| *id == f.layer) {
                Some((_, v)) => v.push(f.field.clone()),
                None => out.push((f.layer, vec![f.field.clone()])),
            }
        }
        out
    }

    fn options(&self) -> PackageOptions {
        let licenses = self.sources.iter().filter_map(|s| s.license.clone().map(|l| (s.path.clone(), l))).collect();
        PackageOptions { dropped: self.dropped(), licenses }
    }

    /// Recomputes what the package carries (after a field choice changed).
    pub fn refresh(&mut self, shell: &Shell) {
        let owned: Vec<LayerId> = self.owned.iter().map(|(id, _)| *id).collect();
        self.review = lm_doc::package::review(&shell.ws.doc, &owned, &self.options());
        let name = |id: &LayerId| shell.ws.doc.layer(*id).map_or_else(String::new, |l| l.display_name.clone());
        let dropped: Vec<LayerId> = self.dropped().iter().map(|(id, _)| *id).collect();
        self.stored =
            shell
                .ws
                .doc
                .layers
                .iter()
                .rev()
                .filter(|l| owned.contains(&l.id) || dropped.contains(&l.id))
                .map(|l| {
                    if dropped.contains(&l.id) {
                        format!("{} (without the fields left out)", l.display_name)
                    } else {
                        l.display_name.clone()
                    }
                })
                .collect();
        self.sources = self
            .review
            .sources
            .iter()
            .map(|p| SourceRow {
                path: p.path.clone(),
                file: p.file_name().to_string(),
                size: p.size,
                layers: p.layers.iter().map(name).collect(),
                license: license_of(shell, Path::new(&p.path)),
                missing: !p.found,
            })
            .collect();
    }

    /// The package's size before compression: its files, and the data it stores itself.
    pub fn estimate(&self) -> u64 {
        let files: u64 = self.sources.iter().map(|s| s.size).sum();
        let dropped: Vec<LayerId> =
            self.dropped().iter().map(|(id, _)| *id).filter(|id| !self.owned.iter().any(|(o, _)| o == id)).collect();
        let results: u64 = self.owned.iter().map(|(_, n)| n).sum::<u64>()
            + self.as_result.iter().filter(|(id, _)| dropped.contains(id)).map(|(_, n)| n).sum::<u64>();
        files + results + 16 * 1024
    }

    /// What the package is written from: the map now, the data the package stores itself (with
    /// the layers that lose fields), and the choices.
    pub(crate) fn inputs(&self, shell: &Shell) -> Result<PackageInputs, String> {
        let mut owned = crate::saving::owned(&shell.ws);
        for (id, _) in self.dropped() {
            if owned.iter().any(|(o, _)| *o == id) {
                continue;
            }
            let data = shell.ws.data(id).ok_or("A layer is still loading. Try again in a moment.")?;
            owned.push((id, to_owned(&data.data)));
        }
        let mut doc = shell.ws.doc.clone();
        let c = shell.map.center_lonlat();
        doc.view = Some((c.x, c.y, shell.map.zoom()));
        Ok(PackageInputs { doc, owned, opts: self.options(), name: shell.doc_file.name() })
    }
}

/// Starts writing the package to `path` off the interface thread.
pub(crate) fn write_to(shell: &mut Shell, path: PathBuf, t: f64) {
    let Some(inputs) = shell.package_inputs.take() else { return };
    let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(lm_doc::EXTENSION)) {
        path
    } else {
        path.with_extension(lm_doc::EXTENSION)
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let target = path.clone();
    let wake = shell.ws.wake.clone();
    let spawned = std::thread::Builder::new().name("lm-package".into()).spawn(move || {
        let r = lm_doc::package::save_package(&inputs.doc, &inputs.owned, &inputs.opts, &target).map_err(|e| e.to_string());
        let _ = tx.send(r);
        if let Some(w) = wake {
            w();
        }
    });
    match spawned {
        Ok(_) => {
            shell.package_job = Some(PackageJob { path, rx });
            shell.notice = Some(("Packaging the map and its data…".into(), t));
        }
        Err(e) => shell.notice = Some((format!("The package couldn't be made: {e}"), t)),
    }
}

/// Each frame: a finished package says so.
pub(crate) fn poll(shell: &mut Shell, t: f64) {
    let Some(job) = &shell.package_job else { return };
    let r = match job.rx.try_recv() {
        Ok(r) => r,
        Err(std::sync::mpsc::TryRecvError::Empty) => return,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("packaging stopped unexpectedly".into()),
    };
    let name = job.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    shell.package_job = None;
    let msg = match r {
        Ok(rep) if !rep.missing.is_empty() => {
            format!("Saved {name} ({}), without {}: it couldn't be found.", size_text(rep.bytes), rep.missing.join(", "))
        }
        Ok(rep) => match rep.save.skipped.first() {
            Some((id, why)) => {
                let layer = shell.ws.doc.layer(*id).map_or_else(String::new, |l| l.display_name.clone());
                format!("Saved {name}, but {layer} couldn't be kept in it ({why}).")
            }
            None => format!("Saved {name} ({}): the map and its data, ready to share.", size_text(rep.bytes)),
        },
        Err(e) => format!("The package couldn't be made. {e}"),
    };
    shell.notice = Some((msg, t));
}

/// Draws the dialog while it is open.
pub fn dialog(shell: &mut Shell, ctx: &egui::Context) {
    let Some(mut d) = shell.package_dialog.take() else { return };
    let t = *shell.theme();
    let mut close = false;
    let mut save = false;
    let mut changed = false;
    let modal = egui::Modal::new(egui::Id::new("lm-package")).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.label(RichText::new("Share as a map package").font(theme::semibold(theme::TITLE)));
        ui.add_space(4.0);
        ui.label(RichText::new("One file with the map and its data, so it opens on another computer.").color(t.text_secondary));
        ui.add_space(12.0);
        let max_h = (ctx.content_rect().height() - 220.0).max(200.0);
        egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
            form::with_visible_controls(ui, &t, |ui| changed = body(ui, &t, &mut d));
        });
        ui.add_space(12.0);
        if let Some(e) = &d.error {
            ui.label(RichText::new(e).color(t.critical));
            ui.add_space(6.0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let ok = egui::Button::new(RichText::new("Save Package…").color(t.on_accent)).fill(t.accent).min_size(vec2(112.0, 30.0));
            if ui.add(ok).clicked() {
                save = true;
            }
            if ui.add(egui::Button::new("Cancel").min_size(vec2(80.0, 30.0))).clicked() {
                close = true;
            }
        });
    });
    if modal.should_close() {
        close = true;
    }
    if changed {
        d.refresh(shell);
    }
    if save {
        match d.inputs(shell) {
            Ok(inputs) => {
                let suggested_name = format!("{} (package).{}", inputs.name, lm_doc::EXTENSION);
                shell.package_inputs = Some(inputs);
                shell.push_file_request(FileRequest::SavePackage { suggested_name });
                close = true;
            }
            Err(e) => d.error = Some(e),
        }
    }
    if !close {
        shell.package_dialog = Some(d);
    }
}

/// The dialog's body; true when a field choice changed.
fn body(ui: &mut Ui, t: &Theme, d: &mut PackageDialog) -> bool {
    let mut changed = false;
    form::row(ui, t, "Size", |ui| ui.label(RichText::new(format!("About {}", size_text(d.estimate()))).strong()));
    form::indented(ui, |ui| {
        ui.label(RichText::new("Before compression; text files shrink a lot in the package.").size(CAPTION).color(t.text_secondary))
    });
    form::divider(ui, t);
    form::heading(ui, "Data it includes");
    if d.sources.is_empty() {
        ui.label(RichText::new("No source files: the map's layers are all stored in it already.").color(t.text_secondary));
    }
    if !d.stored.is_empty() {
        let text = format!("Stored in the package itself: {}.", d.stored.join(", "));
        ui.add(egui::Label::new(RichText::new(text).size(CAPTION).color(t.text_secondary)).wrap());
        ui.add_space(4.0);
    }
    for s in &d.sources {
        ui.horizontal_top(|ui| {
            let glyph = if s.missing { icon::WARNING } else { icon::FILE };
            let tint = if s.missing { t.warning } else { t.text_secondary };
            ui.label(RichText::new(glyph).font(theme::icon_font(16.0)).color(tint));
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&s.file).strong());
                    let size = if s.missing { "can't be found, so it's left out".to_string() } else { size_text(s.size) };
                    ui.label(RichText::new(size).size(CAPTION).color(t.text_secondary));
                });
                let layers = s.layers.join(", ");
                ui.label(RichText::new(format!("Layers: {layers}")).size(CAPTION).color(t.text_secondary));
                match &s.license {
                    Some(l) => ui.label(RichText::new(format!("Licence: {l}")).size(CAPTION).color(t.text_secondary)),
                    None => ui.label(RichText::new(format!("Licence: {UNKNOWN_LICENSE}")).size(CAPTION).color(t.assumption)),
                };
            });
        });
        ui.add_space(4.0);
    }
    form::divider(ui, t);
    form::heading(ui, "Folder paths");
    let paths: Vec<&lm_doc::package::Stripped> = d.review.stripped.iter().collect();
    if paths.is_empty() {
        ui.label(RichText::new("The map names no folders on this computer.").color(t.text_secondary));
    } else {
        ui.add(
            egui::Label::new(
                RichText::new("Paths name your account and folders, so they are left out; files keep only their names.")
                    .color(t.text_secondary),
            )
            .wrap(),
        );
        for p in paths.iter().take(8) {
            ui.add(egui::Label::new(RichText::new(format!("{}  →  {}", p.from, p.to)).size(CAPTION).color(t.text_secondary)).truncate());
        }
        if paths.len() > 8 {
            ui.label(RichText::new(format!("and {} more", paths.len() - 8)).size(CAPTION).color(t.text_secondary));
        }
    }
    form::divider(ui, t);
    form::heading(ui, "Fields that look personal");
    if d.fields.is_empty() {
        ui.label(RichText::new("None found: no names, emails, phone or ID numbers.").color(t.text_secondary));
    } else {
        ui.add(
            egui::Label::new(RichText::new("Checked fields are left out of the package. Your own map keeps them.").color(t.text_secondary))
                .wrap(),
        );
        for f in d.fields.iter_mut() {
            ui.horizontal(|ui| {
                let label = format!("Leave out {} ({})", f.field, f.layer_name);
                ui.add_enabled_ui(f.locked.is_none(), |ui| {
                    if form::checkbox(ui, t, &mut f.drop, &label).changed() {
                        changed = true;
                    }
                });
                let note = match f.locked {
                    Some(why) => format!("{}. {why}, so it stays.", f.note),
                    None => f.note.to_string(),
                };
                ui.label(RichText::new(note).size(CAPTION).color(t.assumption));
            });
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell() -> Shell {
        let ctx = egui::Context::default();
        Shell::new(
            &ctx,
            lm_core::phase::Flags::default(),
            crate::Prefs { basemap: "offline".into(), ..Default::default() },
            crate::System::default(),
        )
    }

    #[test]
    fn the_dialog_lists_sources_licences_paths_and_personal_fields() {
        let mut s = shell();
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples");
        let ctx = lm_jobs::import::Ctx::detached(Default::default());
        s.ws.add_imported(lm_jobs::import::import_path(&dir.join("volcanoes.csv"), &ctx).unwrap(), 1);
        let tmp = std::env::temp_dir().join(format!("lm-package-dialog-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let people = tmp.join("pupils.csv");
        std::fs::write(
            &people,
            "school,full_name,phone,lon,lat\nA,Li Wei,138 0013 8000,121.4,31.2\nB,Wang Fang,139 0013 8000,121.5,31.3\n",
        )
        .unwrap();
        s.ws.add_imported(lm_jobs::import::import_path(&people, &ctx).unwrap(), 2);
        let mut d = PackageDialog::new(&s);
        let found: Vec<(&str, &str)> = d.fields.iter().map(|f| (f.field.as_str(), f.note)).collect();
        assert_eq!(found, [("full_name", "Looks like people's names"), ("phone", "Looks like phone numbers")]);
        assert!(d.fields.iter().all(|f| f.drop), "personal fields start left out");
        // Leaving fields out stores that layer itself, so its file isn't carried, and its path
        // shrinks to the file name.
        let files: Vec<&str> = d.sources.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(files, ["volcanoes.csv"]);
        let people_path = people.display().to_string();
        let same = |a: &str, b: &str| std::fs::canonicalize(a).ok() == std::fs::canonicalize(b).ok();
        assert!(d.review.stripped.iter().any(|p| same(&p.from, &people_path) && p.to == "pupils.csv"), "{:?}", d.review.stripped);
        assert!(d.sources[0].license.as_deref().is_some_and(|l| l.contains("NOAA")), "{:?}", d.sources[0]);
        let with_dropped = d.estimate();
        // Keeping them: the file travels, its licence unknown.
        d.fields.iter_mut().for_each(|f| f.drop = false);
        d.refresh(&s);
        let files: Vec<&str> = d.sources.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(files, ["volcanoes.csv", "pupils.csv"]);
        assert_eq!(d.sources[1].license, None, "a user's own file: unknown");
        assert!(d.review.stripped.iter().any(|p| same(&p.from, &people_path) && p.to == "data/2/pupils.csv"), "{:?}", d.review.stripped);
        assert!(d.estimate() >= d.sources.iter().map(|r| r.size).sum::<u64>());
        assert!(with_dropped > 0);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
