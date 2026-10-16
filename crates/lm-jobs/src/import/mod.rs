//! The import coordinator (WP2.1): one pipeline for every way in (Open, a drop on the window or
//! the Dock icon, Open With). Each file is imported on its own thread, through the stages
//! detect → read metadata → extent → profile → first draw → ready, each reported to its shelf
//! row. A canceled import leaves nothing behind; source files are only ever read.
//!
//! Formats sit behind [`FormatAdapter`] (WP2.2): GeoJSON, Shapefile (also zipped), CSV/TSV and
//! GeoTIFF/COG (P0), GeoPackage and Excel (P1, behind [`P1_FORMATS`]), wrapping the upstream `rustgis-data` readers where they fit and streaming where
//! they don't (large GeoJSON and CSV are scanned as streams, never read whole to learn a name or
//! bounds).

mod csv;
mod excel;
mod geojson;
mod gpkg;
pub mod limits;
pub mod progress;
mod raster;
mod shapefile;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use lm_core::model::{Assumption, Choice, Crs, Exclusion, Extent, ImportStatus, Known, LayerKind, LayerProfile, SourceRef};
use rustgis_engine::data::raster::Raster;

pub use lm_profile::PlaceCandidate;
pub use progress::Ticket;
pub use rustgis_engine::data::Dataset;

/// Why an import didn't produce layers. The message is shown as written (Error and status copy).
#[derive(Clone, Debug, PartialEq)]
pub enum ImportError {
    Canceled,
    /// Refused on purpose (a limit, an unsafe archive), with the user-facing message.
    Refused(String),
    /// The format isn't supported ("LayeredMap can't open .dwg files yet.").
    Unsupported(String),
    /// Reading failed: the user-facing message, and the technical one for Details.
    Failed {
        message: String,
        details: String,
    },
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Canceled => write!(f, "Canceled."),
            ImportError::Refused(m) | ImportError::Unsupported(m) => write!(f, "{m}"),
            ImportError::Failed { message, .. } => write!(f, "{message}"),
        }
    }
}

impl ImportError {
    /// A read failure: canceled if the reader stopped because of a cancel, otherwise failed.
    pub fn read(file: &str, e: impl std::fmt::Display) -> ImportError {
        let details = e.to_string();
        if progress::is_cancel_message(&details) {
            return ImportError::Canceled;
        }
        ImportError::Failed { message: format!("{file} could not be read. Your file has not been changed."), details }
    }

    /// Technical details for the Details pane.
    pub fn details(&self) -> String {
        match self {
            ImportError::Failed { details, .. } => details.clone(),
            other => other.to_string(),
        }
    }
}

/// A layer's data.
#[derive(Clone)]
pub enum LayerData {
    Vector(Arc<Dataset>),
    Raster(Arc<Raster>),
}

/// Raw cells of a delimited file, kept so a changed assumption (other coordinate columns) can be
/// applied without reading the file again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawRows {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// The delimiter of a delimited file (decides what `12,500` means with
    /// `lm_profile::number_style`); `None` for a sheet.
    pub delimiter: Option<char>,
}

/// What a finished import hands to the document.
#[derive(Clone)]
pub struct ImportedLayer {
    /// The display name ("schools").
    pub name: String,
    pub profile: LayerProfile,
    pub data: LayerData,
    pub status: ImportStatus,
    /// For a CSV: its raw cells (to rerun location with other columns).
    pub raw: Option<Arc<RawRows>>,
}

/// The stages, in order (WP2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Waiting,
    Detect,
    Metadata,
    Extent,
    Profile,
    FirstDraw,
    Ready,
}

impl Stage {
    /// Progress text for the shelf row.
    pub fn label(self) -> &'static str {
        match self {
            Stage::Waiting => "Waiting…",
            Stage::Detect => "Opening…",
            Stage::Metadata => "Reading",
            Stage::Extent => "Finding where it is…",
            Stage::Profile => "Looking at the values…",
            Stage::FirstDraw => "Drawing…",
            Stage::Ready => "Ready",
        }
    }
}

/// A message from an import's thread.
pub enum ImportEvent {
    Stage(Stage),
    /// A large raster without overviews is being prepared ("Preparing a faster preview…").
    Preparing,
    Done(Result<Vec<ImportedLayer>, ImportError>),
}

/// A running import.
pub struct ImportJob {
    pub path: PathBuf,
    pub ticket: Ticket,
    rx: Receiver<ImportEvent>,
}

impl ImportJob {
    pub fn cancel(&self) {
        self.ticket.cancel();
    }

    /// Events since the last poll.
    pub fn poll(&self) -> Vec<ImportEvent> {
        self.rx.try_iter().collect()
    }
}

/// At most this many files are read at once; the rest wait their turn.
const PARALLEL: usize = 4;

fn slots() -> &'static (Mutex<usize>, Condvar) {
    static SLOTS: std::sync::OnceLock<(Mutex<usize>, Condvar)> = std::sync::OnceLock::new();
    SLOTS.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

struct Slot;

impl Slot {
    fn take(ticket: &Ticket) -> Option<Slot> {
        let (m, cv) = slots();
        let mut n = m.lock().unwrap_or_else(|e| e.into_inner());
        while *n >= PARALLEL {
            if ticket.is_canceled() {
                return None;
            }
            n = cv.wait_timeout(n, std::time::Duration::from_millis(100)).map(|r| r.0).unwrap_or_else(|e| e.into_inner().0);
        }
        *n += 1;
        Some(Slot)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let (m, cv) = slots();
        let mut n = m.lock().unwrap_or_else(|e| e.into_inner());
        *n = n.saturating_sub(1);
        cv.notify_one();
    }
}

/// Starts importing `path` on its own thread. `wake` is called whenever there is news (the
/// interface repaints).
pub fn start(path: PathBuf, wake: impl Fn() + Send + Sync + 'static) -> ImportJob {
    let (tx, rx) = channel();
    let ticket = Ticket::default();
    let job = ImportJob { path: path.clone(), ticket: ticket.clone(), rx };
    let wake = Arc::new(wake);
    let w = wake.clone();
    let spawned = std::thread::Builder::new().name("lm-import".into()).spawn(move || {
        let send = |e: ImportEvent| {
            let _ = tx.send(e);
            w();
        };
        send(ImportEvent::Stage(Stage::Waiting));
        let result = match Slot::take(&ticket) {
            None => Err(ImportError::Canceled),
            Some(_slot) => {
                let ctx = Ctx { ticket: ticket.clone(), tx: tx.clone(), wake: w.clone() };
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| import_path(&path, &ctx))).unwrap_or_else(|_| {
                    Err(ImportError::Failed {
                        message: format!("{} could not be read. Your file has not been changed.", file_name(&path)),
                        details: "the reader stopped unexpectedly (a panic)".into(),
                    })
                })
            }
        };
        // A cancel wins over a result that arrived at the same time: nothing is left behind.
        let result = if ticket.is_canceled() { Err(ImportError::Canceled) } else { result };
        send(ImportEvent::Done(result));
    });
    if let Err(e) = spawned {
        log::error!("could not start an import thread: {e}");
        wake();
    }
    job
}

/// What an import's stages share.
pub struct Ctx {
    pub ticket: Ticket,
    tx: Sender<ImportEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Ctx {
    fn stage(&self, s: Stage) -> Result<(), ImportError> {
        self.ticket.check()?;
        let _ = self.tx.send(ImportEvent::Stage(s));
        (self.wake)();
        Ok(())
    }

    fn preparing(&self) {
        let _ = self.tx.send(ImportEvent::Preparing);
        (self.wake)();
    }

    /// A context for tests and synchronous callers: events go nowhere.
    pub fn detached(ticket: Ticket) -> Ctx {
        let (tx, _) = channel();
        Ctx { ticket, tx, wake: Arc::new(|| {}) }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("This file").to_string()
}

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// A file format LayeredMap opens (WP2.2).
pub trait FormatAdapter: Sync {
    /// Format name for Details ("CSV").
    fn name(&self) -> &'static str;
    /// Whether this adapter reads the file, from its name and first bytes.
    fn sniff(&self, path: &Path, head: &[u8]) -> bool;
    /// Cheap facts from the file's header (size, declared coordinate system, bounds if stored),
    /// without reading the records.
    fn open_metadata(&self, path: &Path, ctx: &Ctx) -> Result<Metadata, ImportError>;
    /// Reads the records (vector formats), streaming where the format allows.
    fn stream_features(&self, path: &Path, meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let _ = (path, meta, ctx);
        Err(ImportError::Unsupported(format!("{} files hold no features.", self.name())))
    }
    /// Reads pixels for display (raster formats): an overview, or the whole image if small.
    fn read_window(&self, path: &Path, meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let _ = (path, meta, ctx);
        Err(ImportError::Unsupported(format!("{} files hold no pixels.", self.name())))
    }
    fn is_raster(&self) -> bool {
        false
    }
}

/// Facts from a file's header.
#[derive(Clone, Debug, Default)]
pub struct Metadata {
    pub size: u64,
    pub mtime: i64,
    /// Raster size in pixels.
    pub pixels: Option<(u64, u64)>,
    pub has_overviews: bool,
}

/// The P1 formats of WP2.2 (GeoPackage, Excel) are on. Cutting rule (plan, Phase 2 › Rollback):
/// a P1 format that fails at the gate is switched off here and moved to Phase 6 (WP6.7); that
/// does not fail the gate. Off, its files are refused as any unsupported format is ("LayeredMap
/// can't open .gpkg files yet.").
pub const P1_FORMATS: bool = true;

/// File extensions of the P1 formats.
const P1_EXTENSIONS: &[&str] = &["gpkg", "xlsx", "xlsm", "xls"];

/// Every adapter, in sniffing order (the P1 ones only when `p1`). GeoPackage and Excel come
/// before the zipped Shapefile, which claims anything starting like a ZIP (as an `.xlsx` does).
fn adapters(p1: bool) -> Vec<&'static dyn FormatAdapter> {
    let mut v: Vec<&'static dyn FormatAdapter> = vec![&geojson::GeoJson, &csv::Delimited, &shapefile::Shapefile];
    if p1 {
        v.push(&gpkg::GeoPackage);
        v.push(&excel::Excel);
    }
    v.push(&shapefile::ZippedShapefile);
    v.push(&raster::GeoTiff);
    v
}

/// The adapter that reads `path`, or why there is none.
fn pick_adapter(path: &Path, head: &[u8], p1: bool) -> Result<&'static dyn FormatAdapter, ImportError> {
    let ext = extension(path);
    let switched_off = !p1 && P1_EXTENSIONS.contains(&ext.as_str());
    match adapters(p1).into_iter().find(|a| a.sniff(path, head)).filter(|_| !switched_off) {
        Some(a) => Ok(a),
        None if ext.is_empty() => {
            Err(ImportError::Unsupported(format!("LayeredMap can't open {}: it isn't a format it reads yet.", file_name(path))))
        }
        None => Err(ImportError::Unsupported(format!("LayeredMap can't open .{ext} files yet."))),
    }
}

/// Imports one file synchronously (the job thread, tests and the fixture snapshots call this).
pub fn import_path(path: &Path, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
    ctx.stage(Stage::Detect)?;
    let name = file_name(path);
    let meta = std::fs::metadata(path).map_err(|e| ImportError::Failed {
        message: if e.kind() == std::io::ErrorKind::NotFound {
            format!("{name} is no longer where it was. Nothing was added to your map.")
        } else {
            format!("{name} could not be opened. Your file has not been changed.")
        },
        details: e.to_string(),
    })?;
    if meta.is_dir() {
        return Err(ImportError::Unsupported(format!("{name} is a folder. Open the files inside it instead.")));
    }
    let mut head = vec![0u8; 4096];
    let n = {
        use std::io::Read;
        let mut f = std::fs::File::open(path).map_err(|e| ImportError::read(&name, e))?;
        f.read(&mut head).map_err(|e| ImportError::read(&name, e))?
    };
    head.truncate(n);
    let adapter = pick_adapter(path, &head, P1_FORMATS)?;
    ctx.stage(Stage::Metadata)?;
    let m = adapter.open_metadata(path, ctx)?;
    let layers = if adapter.is_raster() { adapter.read_window(path, &m, ctx)? } else { adapter.stream_features(path, &m, ctx)? };
    ctx.stage(Stage::FirstDraw)?;
    Ok(layers)
}

/// The source record for a file (path, size, modification time); the hash is computed by the
/// document when it saves.
fn source_ref(path: &Path, member: Option<String>) -> SourceRef {
    let meta = std::fs::metadata(path).ok();
    let mtime = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    SourceRef { path: abs.display().to_string(), member, size: meta.map_or(0, |m| m.len()), mtime, sha256: None }
}

/// A dataset's extent in longitude/latitude, when it has any geometry.
fn extent_of(ds: &Dataset) -> Option<Extent> {
    let e = ds.extent();
    (e.min_x.is_finite() && e.max_x >= e.min_x).then_some(Extent { min_x: e.min_x, min_y: e.min_y, max_x: e.max_x, max_y: e.max_y })
}

/// The kind of a vector dataset by its geometries.
fn kind_of(ds: &Dataset) -> LayerKind {
    let kinds = lm_profile::geometry_kinds(ds);
    kinds.iter().max_by_key(|(_, n)| *n).map_or(LayerKind::Table, |(k, _)| *k)
}

/// Points of a dataset (first vertex of each geometry), at most `n`, for the China heuristics.
fn sample_points(ds: &Dataset, n: usize) -> Vec<(f64, f64)> {
    let step = (ds.features.len() / n.max(1)).max(1);
    ds.features
        .iter()
        .step_by(step)
        .filter_map(|f| {
            let mut first = None;
            f.geometry.as_ref()?.for_each_coord(|c| {
                if first.is_none() {
                    first = Some((c.x, c.y));
                }
            });
            first
        })
        .take(n)
        .collect()
}

/// The status a layer's profile implies.
fn status_of(profile: &LayerProfile) -> ImportStatus {
    if profile.open_questions().next().is_some() {
        ImportStatus::NeedsDecision
    } else if profile.exclusions.is_empty() {
        ImportStatus::Complete
    } else {
        ImportStatus::CompleteWithExclusions
    }
}

/// Builds the profile of a vector layer read in longitude/latitude (or raw coordinates when the
/// file declares none), with every assumption the catalog gives it.
#[allow(clippy::too_many_arguments)]
fn vector_profile(
    path: &Path,
    member: Option<String>,
    format: &str,
    ds: &Dataset,
    declared: Option<Crs>,
    mut assumptions: Vec<Assumption>,
    exclusions: Vec<Exclusion>,
    encoding: Option<String>,
    notices: Vec<String>,
    column_names: &[String],
) -> LayerProfile {
    let extent = extent_of(ds);
    let (crs, crs_assumptions) = lm_profile::crs_from_values(extent, declared);
    assumptions.extend(crs_assumptions);
    let name_for_hints = member.clone().unwrap_or_else(|| file_name(path));
    if let Some(a) = lm_profile::china_offset(&sample_points(ds, 2000), &crs, &name_for_hints, column_names) {
        assumptions.push(a);
    }
    let (fields, field_assumptions) = lm_profile::profile_fields(ds, None, &[]);
    assumptions.extend(field_assumptions);
    let kind = kind_of(ds);
    LayerProfile {
        source: source_ref(path, member),
        format: format.into(),
        kind,
        count: ds.features.len() as u64,
        extent_wgs84: if matches!(crs.value, Crs::Wgs84 | Crs::Gcj02 | Crs::Bd09) { extent } else { None },
        crs,
        fields,
        bands: Vec::new(),
        assumptions,
        exclusions,
        encoding,
        notices,
        data_rev: 0,
    }
}

/// Splits a dataset with mixed geometry into points, lines and areas (catalog: "Split into 3
/// layers"). Features without geometry stay with the first part. Returns the parts and kinds.
fn split_by_kind(ds: Dataset) -> Vec<(LayerKind, Dataset)> {
    use rustgis_engine::data::{GeometryType, geometry::Geometry};
    let kinds = lm_profile::geometry_kinds(&ds);
    if kinds.len() <= 1 {
        let k = kinds.first().map_or(LayerKind::Table, |(k, _)| *k);
        return vec![(k, ds)];
    }
    let kind_of_geom = |g: &Geometry| match g {
        Geometry::Point(_) | Geometry::MultiPoint(_) => LayerKind::Points,
        Geometry::Line(_) => LayerKind::Lines,
        _ => LayerKind::Areas,
    };
    let mut parts: Vec<(LayerKind, Dataset)> = [LayerKind::Points, LayerKind::Lines, LayerKind::Areas]
        .into_iter()
        .filter(|k| kinds.iter().any(|(x, _)| x == k))
        .map(|k| {
            let (suffix, gt) = match k {
                LayerKind::Points => ("points", GeometryType::Point),
                LayerKind::Lines => ("lines", GeometryType::Polyline),
                _ => ("areas", GeometryType::Polygon),
            };
            let mut d = Dataset::new(format!("{} ({suffix})", ds.name), gt, ds.fields.clone());
            d.source = ds.source.clone();
            (k, d)
        })
        .collect();
    for f in ds.features {
        let k = f.geometry.as_ref().map(kind_of_geom);
        let idx = k.and_then(|k| parts.iter().position(|(x, _)| *x == k)).unwrap_or(0);
        if let Some((_, d)) = parts.get_mut(idx) {
            d.features.push(f);
        }
    }
    parts
}

/// Applies a changed assumption to a CSV layer from its raw cells (WP2.3: "reruns only the
/// affected stages"): other coordinate columns, or Not sure (open as a table).
pub fn relocate_csv(layer: &ImportedLayer, choice: &Choice) -> Result<ImportedLayer, ImportError> {
    let raw = layer.raw.as_ref().ok_or_else(|| ImportError::Refused("This layer can't be located again from its cells.".into()))?;
    csv::relocate(layer, raw, choice)
}

/// "Where should this data be?" (WP2.4): the coordinate systems that put a layer read with raw
/// coordinates near `place` (longitude, latitude), nearest first.
pub fn place_candidates(layer: &ImportedLayer, place: (f64, f64)) -> Vec<lm_profile::PlaceCandidate> {
    let LayerData::Vector(ds) = &layer.data else { return Vec::new() };
    let e = ds.extent();
    if !e.min_x.is_finite() {
        return Vec::new();
    }
    let center = ((e.min_x + e.max_x) / 2.0, (e.min_y + e.max_y) / 2.0);
    lm_profile::place_candidates(center, place, |code, (x, y)| {
        let t = rustgis_engine::data::raster::Crs::Epsg(code).transformer().ok()?;
        t.to_lonlat(rustgis_engine::geom::Coord::new(x, y)).map(|c| (c.x, c.y))
    })
}

/// Opens another sheet of an Excel layer's workbook (the ExcelSheet assumption's Change:
/// `sheet:<name>`), keeping the layer's name; the workbook is read again, never written.
pub fn choose_sheet(layer: &ImportedLayer, choice: &Choice) -> Result<ImportedLayer, ImportError> {
    excel::choose_sheet(layer, choice)
}

pub use raster::rechoose_raster;
pub use shapefile::reread_encoding;

/// The name of a coordinate system in the candidate table ("CGCS2000 / 3-degree Gauss-Kruger CM
/// 120E"), or "EPSG:n".
pub fn crs_name(epsg: u16) -> String {
    lm_profile::crs_candidates::CANDIDATES
        .iter()
        .find(|(c, _, _)| *c == epsg)
        .map_or_else(|| format!("EPSG:{epsg}"), |(_, n, _)| (*n).to_string())
}

/// Reprojects a layer read with raw coordinates from the coordinate system the user chose
/// ("Where should this data be?"), as a new layer; the source file is untouched.
pub fn reproject(layer: &ImportedLayer, epsg: u32) -> Result<ImportedLayer, ImportError> {
    use rustgis_engine::data::raster::Crs as RCrs;
    let LayerData::Vector(ds) = &layer.data else {
        return Err(ImportError::Refused("Only vector layers can be placed this way.".into()));
    };
    let crs = RCrs::parse(&format!("EPSG:{epsg}"))
        .map_err(|e| ImportError::Failed { message: format!("EPSG:{epsg} isn't known."), details: e.to_string() })?;
    let t = crs
        .transformer()
        .map_err(|e| ImportError::Failed { message: format!("EPSG:{epsg} can't be converted."), details: e.to_string() })?;
    let mut out = (**ds).clone();
    for f in &mut out.features {
        if let Some(g) = f.geometry.take() {
            let g = g.map_coords(&|c| t.to_lonlat(c).unwrap_or(rustgis_engine::geom::Coord::new(f64::NAN, f64::NAN)));
            let mut ok = true;
            g.for_each_coord(|c| ok &= c.is_finite());
            f.geometry = ok.then_some(g);
        }
    }
    let mut profile = layer.profile.clone();
    let name = lm_profile::crs_candidates::CANDIDATES
        .iter()
        .find(|(c, _, _)| u32::from(*c) == epsg)
        .map_or_else(|| format!("EPSG:{epsg}"), |(_, n, _)| (*n).to_string());
    profile.crs = Known::certain(Crs::Epsg(epsg), format!("Chosen by you: {name} (EPSG:{epsg})."));
    profile.extent_wgs84 = extent_of(&out);
    profile.kind = kind_of(&out);
    // The choice becomes a visible assumption that names the system (WP2.4 step 5).
    for a in profile.assumptions.iter_mut().filter(|a| a.kind == lm_core::model::AssumptionKind::NoCrsProjected) {
        let choice = Choice::new(format!("epsg:{epsg}"), name.clone());
        if !a.alternatives.contains(&choice) {
            a.alternatives.insert(0, choice.clone());
        }
        a.chosen = choice;
        a.confirmed_by_user = true;
        a.treatment = lm_core::model::Treatment::Visible;
        a.line = format!(
            "Placed using {name} (EPSG:{epsg}), which you chose. The datum can't be checked from the file, so positions may be off by up to about 100 m."
        );
    }
    Ok(ImportedLayer { data: LayerData::Vector(Arc::new(out)), status: status_of(&profile), profile, ..layer.clone() })
}

/// The excluded records of a layer as CSV (WP2.6 "Export problem list"): row, reason, then the
/// record's own cells when the source is a CSV. Cells that a spreadsheet would run as a formula
/// (starting with =, +, - or @) are prefixed with an apostrophe (fixture X5).
pub fn problem_list_csv(layer: &ImportedLayer) -> String {
    fn cell(s: &str) -> String {
        let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() };
        if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
    }
    let mut out = String::new();
    let mut header = vec!["row".to_string(), "reason".to_string()];
    if let Some(raw) = &layer.raw {
        header.extend(raw.headers.iter().cloned());
    }
    out.push_str(&header.iter().map(|h| cell(h)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for e in &layer.profile.exclusions {
        let mut cells = vec![e.row.to_string(), e.reason.clone()];
        if let Some(r) = layer.raw.as_ref().and_then(|raw| raw.rows.get(e.row.saturating_sub(1) as usize)) {
            cells.extend(r.iter().cloned());
        }
        out.push_str(&cells.iter().map(|c| cell(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

/// Converts a dataset's coordinates from GCJ-02 (or BD-09) to WGS 84 as a new dataset, with its
/// recipe ("Converted from GCJ-02 to WGS 84"). The source is never modified (WP2.5).
pub fn convert_offset(ds: &Dataset, from_bd09: bool) -> Result<(Dataset, String), ImportError> {
    use lm_analysis::china::{OffsetSystem, convert_dataset};
    let from = if from_bd09 { OffsetSystem::Bd09 } else { OffsetSystem::Gcj02 };
    let c = convert_dataset(ds, from, OffsetSystem::Wgs84).map_err(|e| ImportError::Failed {
        message: "The points couldn't be converted. Nothing was changed.".into(),
        details: format!("{e:?}"),
    })?;
    Ok((c.dataset, c.recipe))
}

#[cfg(test)]
mod tests;
