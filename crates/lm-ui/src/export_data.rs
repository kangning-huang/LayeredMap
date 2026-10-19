//! The data export dialog (WP5.3, P1): choose a layer, which records (all, the filtered ones or
//! the selected ones) and which fields, and a format — GeoJSON, CSV with longitude and latitude
//! (points) or well-known text, or GeoTIFF for a raster — in WGS 84. Fields that look personal
//! start unchecked, with a note. The dialog says what doesn't travel: styles and recipes.

use egui::{RichText, Ui, vec2};
use lm_core::model::{LayerId, LayerKind};
use lm_core::{i18n, tl};
use lm_jobs::export::{DataFormat, sensitive_fields};
use lm_jobs::import::LayerData;

use crate::theme::{self, CAPTION};
use crate::{FileRequest, Shell, form};

/// Which records are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    All,
    Filtered,
    Selected,
}

/// One field and whether it is written.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldChoice {
    pub name: String,
    pub on: bool,
    /// Why it starts unchecked ("Looks like phone numbers").
    pub note: Option<&'static str>,
}

/// The dialog's state.
#[derive(Clone, Debug, PartialEq)]
pub struct DataDialog {
    pub layer: LayerId,
    pub scope: Scope,
    pub format: DataFormat,
    pub fields: Vec<FieldChoice>,
    pub error: Option<String>,
}

impl DataDialog {
    /// The dialog for `layer` (the selected layer, else the top one); `None` with no layers.
    pub fn new(shell: &Shell, layer: Option<LayerId>) -> Option<DataDialog> {
        let id = layer.or_else(|| shell.ws.selected()).or_else(|| shell.ws.doc.layers.last().map(|l| l.id))?;
        let mut d = DataDialog { layer: id, scope: Scope::All, format: DataFormat::GeoJson, fields: Vec::new(), error: None };
        d.choose_layer(shell, id);
        Some(d)
    }

    /// Switches to another layer: its fields (personal-looking ones unchecked), a fitting
    /// format, and the filtered or selected records when there are any.
    pub fn choose_layer(&mut self, shell: &Shell, id: LayerId) {
        self.layer = id;
        self.error = None;
        let kind = shell.ws.doc.layer(id).map(|l| l.profile.kind);
        self.fields = match shell.ws.data(id).map(|d| &d.data) {
            Some(LayerData::Vector(ds)) => ds
                .fields
                .iter()
                .zip(sensitive_fields(ds))
                .map(|(f, s)| FieldChoice { name: f.name.clone(), on: s.is_none(), note: s.map(|s| s.note()) })
                .collect(),
            _ => Vec::new(),
        };
        self.format = if kind == Some(LayerKind::Raster) { DataFormat::GeoTiff } else { DataFormat::GeoJson };
        self.scope = if !shell.ws.selection.rows_in(id).is_empty() {
            Scope::Selected
        } else if shell.ws.doc.layer(id).is_some_and(|l| l.filter.is_some()) {
            Scope::Filtered
        } else {
            Scope::All
        };
    }

    /// The records written, in layer order (`None`: all of them).
    pub fn rows(&self, shell: &Shell) -> Option<Vec<usize>> {
        match self.scope {
            Scope::All => None,
            Scope::Selected => {
                let mut r = shell.ws.selection.rows_in(self.layer);
                r.sort_unstable();
                r.dedup();
                Some(r)
            }
            Scope::Filtered => {
                let mask = shell.ws.derived(self.layer).and_then(|d| d.mask.clone())?;
                Some(mask.iter().enumerate().filter(|(_, keep)| **keep).map(|(i, _)| i).collect())
            }
        }
    }

    /// The file's bytes and a suggested name.
    pub fn write(&self, shell: &Shell) -> Result<(Vec<u8>, String), String> {
        let l = shell.ws.doc.layer(self.layer).ok_or(tl!("The layer is gone."))?;
        let data = shell.ws.data(self.layer).ok_or(tl!("The layer's data is still loading."))?;
        let fields: Vec<String> = self.fields.iter().filter(|f| f.on).map(|f| f.name.clone()).collect();
        let band = match &data.data {
            LayerData::Raster(r) => lm_jobs::inspect::style_band(r, &l.style),
            LayerData::Vector(_) => 0,
        };
        let rows = self.rows(shell);
        let bytes = lm_jobs::export::export(&data.data, rows.as_deref(), &fields, self.format, band)?;
        // The suggested file name is in the interface's language; the extension isn't.
        let name = l.display_name.replace(['/', ':', '\\'], "-");
        let name = match self.scope {
            Scope::All => name,
            Scope::Filtered => i18n::fmt("{name} (filtered)", &[("name", &name)]),
            Scope::Selected => i18n::fmt("{name} (selected)", &[("name", &name)]),
        };
        Ok((bytes, format!("{name}.{}", self.format.extension())))
    }
}

/// Draws the dialog while it is open.
pub fn dialog(shell: &mut Shell, ctx: &egui::Context) {
    let Some(mut d) = shell.data_export.take() else { return };
    let t = *shell.theme();
    let mut close = false;
    let mut export = false;
    let modal = egui::Modal::new(egui::Id::new("lm-export-data")).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.label(RichText::new(tl!("Export data")).font(theme::semibold(theme::TITLE)));
        ui.add_space(12.0);
        form::with_visible_controls(ui, &t, |ui| body(shell, ui, &t, &mut d));
        ui.add_space(12.0);
        if let Some(e) = &d.error {
            ui.label(RichText::new(e).color(t.critical));
            ui.add_space(6.0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let ok = egui::Button::new(RichText::new(tl!("Export…")).color(t.on_accent)).fill(t.accent).min_size(vec2(88.0, 30.0));
            if ui.add(ok).clicked() {
                export = true;
            }
            if ui.add(egui::Button::new(tl!("Cancel")).min_size(vec2(80.0, 30.0))).clicked() {
                close = true;
            }
        });
    });
    if modal.should_close() {
        close = true;
    }
    if export {
        match d.write(shell) {
            Ok((bytes, name)) => {
                shell.push_file_request(FileRequest::SaveBytes { title: tl!("Export data").into(), suggested_name: name, bytes });
                close = true;
            }
            Err(e) => d.error = Some(e),
        }
    }
    if !close {
        shell.data_export = Some(d);
    }
}

fn body(shell: &Shell, ui: &mut Ui, t: &theme::Theme, d: &mut DataDialog) {
    let layers: Vec<(LayerId, String, LayerKind)> =
        shell.ws.doc.layers.iter().rev().map(|l| (l.id, l.display_name.clone(), l.profile.kind)).collect();
    let current = layers.iter().find(|l| l.0 == d.layer).map(|l| l.1.clone()).unwrap_or_default();
    let mut chosen = None;
    form::row(ui, t, tl!("Layer"), |ui| {
        egui::ComboBox::from_id_salt("lm-export-layer").selected_text(current).width(ui.available_width()).show_ui(ui, |ui| {
            for (id, name, _) in &layers {
                if ui.selectable_label(*id == d.layer, name).clicked() {
                    chosen = Some(*id);
                }
            }
        });
    });
    if let Some(id) = chosen.filter(|id| *id != d.layer) {
        d.choose_layer(shell, id);
    }
    let kind = layers.iter().find(|l| l.0 == d.layer).map_or(LayerKind::Points, |l| l.2);
    let raster = kind == LayerKind::Raster;
    let total = shell.ws.doc.layer(d.layer).map_or(0, |l| l.profile.count);
    if !raster {
        let filtered = shell.ws.doc.layer(d.layer).is_some_and(|l| l.filter.is_some());
        let shown = shell.ws.derived(d.layer).map_or(total as usize, |dv| dv.shown);
        let selected = shell.ws.selection.rows_in(d.layer).len();
        let n = i18n::count;
        // A count only where there is something to count.
        let labels = [
            i18n::fmt("All {n}", &[("n", &n(total))]),
            if filtered { i18n::fmt("Filtered {n}", &[("n", &n(shown as u64))]) } else { tl!("Filtered").into() },
            if selected > 0 { i18n::fmt("Selected {n}", &[("n", &n(selected as u64))]) } else { tl!("Selected").into() },
        ];
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let sel = match d.scope {
            Scope::All => 0,
            Scope::Filtered => 1,
            Scope::Selected => 2,
        };
        form::row(ui, t, tl!("Records"), |ui| {
            if let Some(k) = form::segmented(ui, t, sel, &refs, ui.available_width()) {
                match k {
                    1 if filtered => d.scope = Scope::Filtered,
                    2 if selected > 0 => d.scope = Scope::Selected,
                    0 => d.scope = Scope::All,
                    _ => {}
                }
            }
        });
        if !filtered || selected == 0 {
            let why = match (filtered, selected > 0) {
                (false, false) => tl!("The layer has no filter and nothing is selected."),
                (false, true) => tl!("The layer has no filter."),
                _ => tl!("Nothing is selected in this layer."),
            };
            form::indented(ui, |ui| ui.label(RichText::new(why).size(CAPTION).color(t.text_secondary)));
        }
        let points = shell.ws.data(d.layer).is_some_and(|data| match &data.data {
            LayerData::Vector(ds) => lm_jobs::export::all_points(ds),
            LayerData::Raster(_) => false,
        });
        let sel = match d.format {
            DataFormat::GeoJson => 0,
            _ => 1,
        };
        form::row(ui, t, tl!("Format"), |ui| {
            if let Some(k) = form::segmented(ui, t, sel, &["GeoJSON", "CSV"], ui.available_width()) {
                d.format = if k == 0 {
                    DataFormat::GeoJson
                } else if points {
                    DataFormat::CsvLonLat
                } else {
                    DataFormat::CsvWkt
                };
            }
        });
        if matches!(d.format, DataFormat::CsvLonLat | DataFormat::CsvWkt) {
            let sel = usize::from(d.format == DataFormat::CsvWkt);
            form::row(ui, t, tl!("Location"), |ui| {
                if let Some(k) = form::segmented(ui, t, sel, &[tl!("Longitude, latitude"), "WKT"], ui.available_width()) {
                    d.format = if k == 0 && points { DataFormat::CsvLonLat } else { DataFormat::CsvWkt };
                }
            });
            if !points {
                form::indented(ui, |ui| {
                    ui.label(
                        RichText::new(tl!("Lines and areas are written as well-known text (WKT).")).size(CAPTION).color(t.text_secondary),
                    )
                });
            }
        }
        form::divider(ui, t);
        ui.horizontal(|ui| {
            form::heading(ui, tl!("Fields"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.link(tl!("None")).clicked() {
                    d.fields.iter_mut().for_each(|f| f.on = false);
                }
                ui.label(RichText::new("·").color(t.text_secondary));
                if ui.link(tl!("All")).clicked() {
                    d.fields.iter_mut().for_each(|f| f.on = true);
                }
            });
        });
        egui::ScrollArea::vertical().max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
            for f in d.fields.iter_mut() {
                ui.horizontal(|ui| {
                    form::checkbox(ui, t, &mut f.on, &f.name);
                    if let Some(note) = f.note {
                        ui.label(
                            RichText::new(i18n::fmt("{note}: left out unless you check it", &[("note", note)]))
                                .size(CAPTION)
                                .color(t.assumption),
                        );
                    }
                });
            }
        });
    } else {
        form::row(ui, t, tl!("Format"), |ui| ui.label("GeoTIFF")); // i18n: format name
        form::indented(ui, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(tl!(
                        "The band shown, on the raster's own grid and coordinate system: resampling to WGS 84 would change its values."
                    ))
                    .size(CAPTION)
                    .color(t.text_secondary),
                )
                .wrap(),
            )
        });
    }
    form::divider(ui, t);
    if !raster {
        ui.label(RichText::new(tl!("Coordinates are written in WGS 84 (longitude, latitude).")).color(t.text_secondary));
    }
    ui.add(
        egui::Label::new(
            RichText::new(i18n::fmt(
                "Styles and recipes stay in {app}: these formats carry only locations and values.",
                &[("app", lm_core::brand::shown(false))],
            ))
            .color(t.text_secondary),
        )
        .wrap(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_pick_all_filtered_or_selected_records() {
        let ctx = egui::Context::default();
        let mut s = Shell::new(
            &ctx,
            lm_core::phase::Flags::default(),
            crate::Prefs { basemap: "offline".into(), ..Default::default() },
            crate::System::default(),
        );
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/V1/districts.geojson");
        let layers = lm_jobs::import::import_path(&path, &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        s.ws.add_imported(layers, 1);
        let id = s.ws.doc.layers[0].id;
        let mut d = DataDialog::new(&s, None).unwrap();
        assert_eq!((d.layer, d.scope, d.format), (id, Scope::All, DataFormat::GeoJson));
        assert_eq!(d.rows(&s), None);
        s.ws.selection = lm_core::selection::SelectionContext::object(lm_core::selection::FeatureRef { layer: id, row: 3 });
        d.choose_layer(&s, id);
        assert_eq!(d.scope, Scope::Selected);
        assert_eq!(d.rows(&s), Some(vec![3]));
        let (bytes, name) = d.write(&s).unwrap();
        assert_eq!(name, format!("{} (selected).geojson", s.ws.doc.layers[0].display_name));
        let fc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(fc["features"].as_array().map(Vec::len), Some(1));
    }
}
