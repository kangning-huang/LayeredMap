//! Help › Export Diagnostics… (WP5.7): the report the app assembled is shown in full, read-only,
//! before anything is saved. "Save…" asks where to write it as a text file the user can send
//! themselves; nothing is ever sent by the app.

use egui::RichText;

use crate::{FileRequest, Shell};

/// Shows `report` for review (the app calls this through `Command::ReviewDiagnostics`).
pub(crate) fn open(shell: &mut Shell, report: String) {
    shell.diagnostics = Some(report);
}

/// The suggested file name: "NimbleMap diagnostics 2026-10-18.txt".
pub fn file_name(app: &str, today: &str) -> String {
    format!("{app} diagnostics {today}.txt")
}

/// Today's date (UTC) as YYYY-MM-DD.
fn today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    let d = lm_mapview::archive::ymd(days);
    format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..8])
}

/// The review window, while a report is open.
pub(crate) fn window(shell: &mut Shell, ctx: &egui::Context) {
    let Some(report) = shell.diagnostics.clone() else { return };
    let mut open = true;
    let mut save = false;
    let mut cancel = false;
    egui::Window::new("Export Diagnostics")
        .id(egui::Id::new("lm-diagnostics"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([620.0, 480.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                "This is everything the diagnostics file will contain. Nothing is sent: save it, \
                 read it, and send it yourself if you want help.",
            );
            ui.label(
                RichText::new("It holds no data values or file contents; file and folder names may appear in error messages.")
                    .size(crate::theme::CAPTION)
                    .color(shell.theme().text_secondary),
            );
            ui.add_space(6.0);
            let avail = (ui.available_height() - 40.0).max(120.0);
            egui::ScrollArea::vertical().max_height(avail).show(ui, |ui| {
                let mut text = report.as_str();
                ui.add(egui::TextEdit::multiline(&mut text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Save…").clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if save {
        shell.file_requests.push(FileRequest::SaveText {
            title: "Export Diagnostics".into(),
            suggested_name: file_name(shell.app_name(), &today()),
            contents: report,
        });
        shell.diagnostics = None;
    } else if cancel || !open {
        shell.diagnostics = None;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_file_is_named_for_the_app_and_day() {
        assert_eq!(super::file_name(lm_core::brand::PRO_NAME, "2026-10-18"), "NimbleMap Pro diagnostics 2026-10-18.txt");
        let t = super::today();
        assert_eq!((t.len(), &t[4..5], &t[7..8]), (10, "-", "-"));
    }
}
