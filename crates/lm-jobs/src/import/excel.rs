//! Excel workbooks (`.xlsx`, `.xlsm`, `.xls`), read with the upstream spreadsheet reader
//! (calamine). Each sheet's header and first rows are sampled to choose the sheet (catalog:
//! "Excel workbook with several sheets", `lm_profile::excel_sheet`); the chosen sheet then goes
//! the CSV way: cells kept as written, located by `lm_profile::csv_locate`, typed beside them.

use lm_core::brand::NAME as APP;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use lm_core::model::{Assumption, AssumptionKind, Choice};
use lm_profile::SheetSample;
use rustgis_engine::data::sheets;

use super::csv::{TableOrigin, build};
use super::limits::WORKBOOK_MAX_BYTES;
use super::progress::Counting;
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, Metadata, RawRows, Stage, extension, file_name};

pub struct Excel;

/// File extensions read as workbooks.
pub const EXTENSIONS: &[&str] = &["xlsx", "xlsm", "xls"];

/// Rows of each sheet looked at to choose the sheet.
const SAMPLE_ROWS: usize = 1000;

impl FormatAdapter for Excel {
    fn name(&self) -> &'static str {
        "Excel"
    }

    fn sniff(&self, path: &Path, _head: &[u8]) -> bool {
        EXTENSIONS.contains(&extension(path).as_str())
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        if m.len() > WORKBOOK_MAX_BYTES {
            return Err(ImportError::Refused(format!(
                "{} is larger than 1 GB, the most {APP} reads as a workbook. Saving the sheet as CSV makes it open. Your file has not been changed.",
                file_name(path)
            )));
        }
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        let bytes = read_bytes(path, meta.size, ctx)?;
        let names = sheets::sheet_names(&bytes).map_err(|e| ImportError::read(&name, e))?;
        // Each sheet's header and a sample of its rows; a sheet that can't be read (empty, too
        // large) isn't offered.
        let mut samples: Vec<sheets::Grid> = Vec::new();
        for n in &names {
            ctx.ticket.check()?;
            match sheets::grid(&bytes, Some(n)) {
                Ok(mut g) => {
                    g.rows.truncate(SAMPLE_ROWS);
                    samples.push(g);
                }
                Err(e) => log::info!("{name}: sheet '{n}' passed over: {e}"),
            }
        }
        ctx.stage(Stage::Extent)?;
        let offered: Vec<SheetSample> = samples.iter().map(|g| SheetSample { name: &g.pick, headers: &g.headers, rows: &g.rows }).collect();
        let Some(choice) = lm_profile::excel_sheet(&offered) else {
            return Err(ImportError::Failed {
                message: format!("{name} has no sheet with anything in it. Nothing was added to your map."),
                details: format!("sheets: {}", names.join(", ")),
            });
        };
        let sheet = offered.get(choice.sheet).map(|s| s.name.to_string()).unwrap_or_default();
        drop(offered);
        drop(samples);
        // The chosen sheet is read whole.
        let grid = sheets::grid(&bytes, Some(&sheet)).map_err(|e| ImportError::read(&name, e))?;
        drop(bytes);
        ctx.stage(Stage::Profile)?;
        Ok(vec![open_sheet(path, grid, choice.assumption)?])
    }
}

/// Reads a workbook whole (calamine reads from memory), counting the bytes for progress and
/// stopping on cancel.
fn read_bytes(path: &Path, size: u64, ctx: &Ctx) -> Result<Vec<u8>, ImportError> {
    let name = file_name(path);
    ctx.ticket.set_total(size);
    let file = std::fs::File::open(path).map_err(|e| ImportError::read(&name, e))?;
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    Counting::new(file, ctx.ticket.clone())
        .take(WORKBOOK_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ImportError::read(&name, e))?;
    ctx.ticket.check()?;
    Ok(bytes)
}

/// Builds the layer of one sheet the CSV way. `sheet_choice` (the ExcelSheet assumption) is
/// listed first among its assumptions.
fn open_sheet(path: &Path, grid: sheets::Grid, sheet_choice: Option<Assumption>) -> Result<ImportedLayer, ImportError> {
    let sheets::Grid { pick, headers, rows, .. } = grid;
    let raw = RawRows { headers: super::csv::dedupe_headers(headers), rows, delimiter: None };
    let location = lm_profile::csv_locate(&raw.headers, &raw.rows, &pick);
    let origin = TableOrigin {
        path,
        member: Some(pick),
        name: path.file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string(),
        format: "Excel".into(),
        encoding: None,
        notices: Vec::new(),
        assumptions: sheet_choice.into_iter().collect(),
    };
    let layer = build(origin, &raw, location)?;
    Ok(ImportedLayer { raw: Some(Arc::new(raw)), ..layer })
}

/// Opens another sheet of an Excel layer's workbook (`sheet:<name>`, one of the ExcelSheet
/// assumption's alternatives), as a new layer with the same name; the choice is recorded on that
/// assumption. The workbook is read again; it is never written.
pub fn choose_sheet(layer: &ImportedLayer, choice: &Choice) -> Result<ImportedLayer, ImportError> {
    let Some(sheet) = choice.id.strip_prefix("sheet:") else {
        return Err(ImportError::Refused("Choose one of the workbook's sheets.".into()));
    };
    let path = Path::new(&layer.profile.source.path);
    let name = file_name(path);
    let size = std::fs::metadata(path)
        .map_err(|e| ImportError::Failed {
            message: format!("{name} is no longer where it was. Nothing was changed."),
            details: e.to_string(),
        })?
        .len();
    let bytes = read_bytes(path, size, &Ctx::detached(super::Ticket::default()))?;
    let grid = sheets::grid(&bytes, Some(sheet)).map_err(|e| ImportError::read(&name, e))?;
    let assumption = layer.profile.assumptions.iter().find(|a| a.kind == AssumptionKind::ExcelSheet).cloned().map(|mut a| {
        a.chosen = choice.clone();
        a.confirmed_by_user = true;
        a.line = lm_profile::lines::excel_sheet(&grid.pick);
        if !a.alternatives.contains(choice) {
            a.alternatives.push(choice.clone());
        }
        a
    });
    let mut out = open_sheet(path, grid, assumption)?;
    out.name = layer.name.clone();
    Ok(out)
}
