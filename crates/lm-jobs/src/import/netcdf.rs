//! NetCDF-3 and NetCDF-4 grids (WP6.12), read with the pure-Rust `netcdf-reader`. A file is one
//! raster layer: its variables on a regular latitude–longitude grid (two dimensions or more),
//! with their long names and units, CF dates on each time axis, and an extra level dimension
//! (depth, pressure) shown at its first level until another is picked.
//!
//! Only the header and the coordinate variables are read when a file opens; cells are read one
//! 2-D slice (one time step, one level) at a time and the last few are kept ([`slice`]), so a
//! multi-gigabyte file opens in a moment. `_FillValue`, `missing_value` and the valid range
//! become No data, `scale_factor`/`add_offset` are applied, longitudes of 0–360 are shown as
//! −180–180 and a grid stored south to north is turned north up. Curvilinear and projected grids
//! are refused with a plain message (a documented limitation).
//!
//! The file is read through positional reads (never memory-mapped), so a file that changes
//! while it is open gives an error, not a crash.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use lm_core::grid::{GridDate, GridLevels, GridProfile, GridTimeAxis, GridVariable, unit_text};
use lm_core::model::{BandProfile, Crs, Extent, Known, LayerKind, LayerProfile, SourceRef, group_thousands};
use netcdf_reader::{FileStorage, NcFile, NcFormat, NcOpenOptions, NcSliceInfo, NcSliceInfoElem, NcVariable};
use rustgis_engine::data::raster::{ColorHint, Crs as RasterCrs, GeoTransform, Raster};

use super::limits::{NETCDF_MAX_CELLS, RASTER_MAX_SIDE};
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, Stage, extension, file_name, source_ref, status_of};

pub struct NetCdf;

/// File extensions read as NetCDF (the HDF5 signature alone could be any HDF5 file).
const EXTENSIONS: &[&str] = &["nc", "nc4", "cdf", "netcdf"];

/// How many time steps the colour range is taken from, evenly spaced.
pub const RANGE_SAMPLE_STEPS: usize = 12;

/// The default fill value of NetCDF floats (9.96921e36), drawn as No data even when a file
/// doesn't declare it: unwritten cells hold it.
const DEFAULT_FILL: f64 = 9.9e36;

/// What a dimension of a grid variable is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Time,
    Level,
    /// Another dimension, read at its first index.
    Fixed,
    /// A dimension of length one.
    Squeezed,
    Lat,
    Lon,
}

/// One axis of the drawn grid: for each output row (or column), the file's index, or None
/// where the file has no cell (a regional grid across the antimeridian is padded).
#[derive(Clone, Debug, PartialEq)]
struct Axis {
    map: Vec<Option<usize>>,
    /// The first output cell's centre, and the signed step between centres, in degrees.
    start: f64,
    step: f64,
    /// The worst distance between a cell's place in the file and where it is drawn, in degrees.
    off: f64,
}

/// The drawn grid of a pair of latitude and longitude dimensions.
#[derive(Clone, Debug, PartialEq)]
struct Geom {
    lat_len: usize,
    lon_len: usize,
    rows: Axis,
    cols: Axis,
    /// The file's longitudes reach past 180°.
    lon_360: bool,
}

impl Geom {
    fn width(&self) -> usize {
        self.cols.map.len()
    }
    fn height(&self) -> usize {
        self.rows.map.len()
    }
    fn transform(&self) -> GeoTransform {
        GeoTransform {
            origin_x: self.cols.start - self.cols.step / 2.0,
            origin_y: self.rows.start - self.rows.step / 2.0,
            pixel_width: self.cols.step,
            pixel_height: self.rows.step,
        }
    }
}

/// How one grid variable is laid out in the file.
#[derive(Clone, Debug)]
struct Layout {
    name: String,
    roles: Vec<Role>,
    /// Names of the dimensions read at their first index.
    fixed: Vec<String>,
    geom: Arc<Geom>,
    /// The latitude dimension comes before the longitude one.
    lat_first: bool,
    scale: Option<f64>,
    offset: Option<f64>,
    /// How many time steps one stored chunk holds, learned on the first read (1 for NetCDF-3
    /// and for files chunked a step at a time).
    time_chunk: OnceLock<usize>,
}

/// An opened file: its reader and everything learned from its header.
struct Opened {
    file: NcFile,
    format: &'static str,
    /// The variables and time axes; `variable` is the default (the first with time).
    grid: GridProfile,
    layouts: Vec<Layout>,
    notices: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Reading the header

fn attr_text(v: &NcVariable, name: &str) -> Option<String> {
    v.attribute(name).and_then(|a| a.value.as_string()).map(|s| s.trim().trim_end_matches('\0').to_string()).filter(|s| !s.is_empty())
}

fn attr_num(v: &NcVariable, name: &str) -> Option<f64> {
    v.attribute(name).and_then(|a| a.value.as_f64()).filter(|x| x.is_finite())
}

fn numeric(v: &NcVariable) -> bool {
    use netcdf_reader::NcType as T;
    matches!(v.dtype(), T::Byte | T::Short | T::Int | T::Float | T::Double | T::UByte | T::UShort | T::UInt | T::Int64 | T::UInt64)
}

/// Latitude or longitude, from a 1-D coordinate variable's units, standard name or name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Coord {
    Lat,
    Lon,
    Time,
    Other,
}

fn classify(v: &NcVariable) -> Coord {
    let units = attr_text(v, "units").unwrap_or_default().to_ascii_lowercase().replace([' ', '-'], "_");
    let std = attr_text(v, "standard_name").unwrap_or_default().to_ascii_lowercase();
    let name = v.name().to_ascii_lowercase();
    let lat_units = matches!(units.as_str(), "degrees_north" | "degree_north" | "degree_n" | "degrees_n" | "degreen" | "degreesn");
    let lon_units = matches!(units.as_str(), "degrees_east" | "degree_east" | "degree_e" | "degrees_e" | "degreee" | "degreese");
    // A name alone counts when the units say degrees or say nothing (GDAL-written files often
    // give `lat` and `lon` no attributes at all).
    let degrees = units.starts_with("degree") || units.is_empty();
    if lat_units || std == "latitude" || (degrees && matches!(name.as_str(), "lat" | "latitude" | "lats" | "nav_lat")) {
        return Coord::Lat;
    }
    if lon_units || std == "longitude" || (degrees && matches!(name.as_str(), "lon" | "longitude" | "lons" | "long" | "nav_lon")) {
        return Coord::Lon;
    }
    if units.contains("_since_") || std == "time" || attr_text(v, "axis").is_some_and(|a| a.eq_ignore_ascii_case("t")) {
        return Coord::Time;
    }
    Coord::Other
}

/// The 1-D coordinate variable of a dimension (a variable named like it, along it alone).
fn coordinate<'a>(vars: &'a [NcVariable], dim: &str) -> Option<&'a NcVariable> {
    vars.iter().find(|v| v.name() == dim && v.dimensions().len() == 1 && v.dimensions().first().is_some_and(|d| d.name == dim))
}

fn read_f64(file: &NcFile, name: &str) -> Result<Vec<f64>, String> {
    file.read_variable_as_f64(name).map(|a| a.iter().copied().collect()).map_err(|e| e.to_string())
}

/// Builds one axis of the drawn grid from a coordinate's values. Longitudes are wrapped into
/// −180–180 first. Rows run north to south (`descending`), columns west to east. Cells go to the
/// nearest place on an even grid with the coordinate's typical spacing; an axis too uneven for
/// that (two cells landing on one place) is refused.
fn build_axis(values: &[f64], descending: bool) -> Result<Axis, String> {
    if values.is_empty() || values.iter().any(|v| !v.is_finite()) {
        return Err("a coordinate has missing values".into());
    }
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    if descending {
        order.reverse();
    }
    let sorted: Vec<f64> = order.iter().map(|&i| values[i]).collect();
    let mut gaps: Vec<f64> =
        sorted.windows(2).filter_map(|w| w.get(1).zip(w.first()).map(|(b, a)| (b - a).abs())).filter(|g| *g > 1e-9).collect();
    gaps.sort_by(f64::total_cmp);
    // The lower median: across the antimeridian a regional grid has one huge gap.
    let step = gaps.get(gaps.len().saturating_sub(1) / 2).copied().unwrap_or(1.0);
    if !(step.is_finite() && step > 0.0) {
        return Err("a coordinate has no spacing".into());
    }
    let first = sorted.first().copied().unwrap_or(0.0);
    let last = sorted.last().copied().unwrap_or(0.0);
    let n = ((last - first).abs() / step).round() as u64 + 1;
    if n > RASTER_MAX_SIDE {
        return Err(format!("{} cells along one side", group_thousands(n)));
    }
    let signed = if descending { -step } else { step };
    let mut map = vec![None; n as usize];
    let mut off: f64 = 0.0;
    for (k, &i) in order.iter().enumerate() {
        let v = sorted.get(k).copied().unwrap_or(first);
        let pos = ((v - first) / signed).round().max(0.0) as usize;
        let d = (v - (first + pos as f64 * signed)).abs();
        off = off.max(d);
        match map.get_mut(pos) {
            Some(slot) if slot.is_none() => *slot = Some(i),
            // Two cells on one place of the even grid: one would be lost.
            _ if d > 1e-9 => return Err("the cells are too unevenly spaced to draw on an even grid".into()),
            // A repeated coordinate (−180 and 180 both given): the first is kept.
            _ => {}
        }
    }
    Ok(Axis { map, start: first, step: signed, off })
}

/// Longitudes as −180–180, and whether any were past 180°.
fn wrap_longitudes(values: &[f64]) -> (Vec<f64>, bool) {
    let past = values.iter().any(|v| *v > 180.0 + 1e-9);
    let below = values.iter().any(|v| *v < -180.0 - 1e-9);
    if !past && !below {
        return (values.to_vec(), false);
    }
    (values.iter().map(|v| (v + 180.0).rem_euclid(360.0) - 180.0).collect(), past)
}

/// Decodes a time axis to dates on its calendar: CF units through `netcdf-reader`, and whole
/// "months since" or "years since" (common, though CF discourages them) by counting calendar
/// months. Empty when the units can't be read.
pub fn decode_times(values: &[f64], units: &str, calendar: &str) -> Vec<GridDate> {
    use netcdf_reader::cf::{CfCalendar, decode_times_exact, parse_time_units};
    // The reader slices the units by byte positions of their lower-cased form: only ASCII is safe.
    if !units.is_ascii() || !calendar.is_ascii() || values.iter().any(|v| !v.is_finite()) {
        return Vec::new();
    }
    let cal = CfCalendar::parse(calendar);
    if let Ok(tref) = parse_time_units(units, cal)
        && let Ok(d) = decode_times_exact(values, &tref)
    {
        return d.into_iter().map(|t| GridDate(t.year, t.month, t.day, t.hour, t.minute)).collect();
    }
    // Whole months or years since an epoch.
    let lower = units.trim().to_ascii_lowercase();
    let Some((unit, epoch)) = lower.split_once(" since ") else { return Vec::new() };
    let months_per = match unit.trim() {
        "month" | "months" | "mon" => 1i64,
        "year" | "years" | "yr" => 12,
        _ => return Vec::new(),
    };
    let date = epoch.split([' ', 'T']).next().unwrap_or("");
    let mut parts = date.split('-').filter(|p| !p.is_empty());
    let (Some(y), Some(m)) = (parts.next().and_then(|p| p.parse::<i64>().ok()), parts.next().and_then(|p| p.parse::<i64>().ok())) else {
        return Vec::new();
    };
    let d = parts.next().and_then(|p| p.parse::<u8>().ok()).unwrap_or(1);
    let mut out = Vec::with_capacity(values.len());
    for v in values {
        if (v - v.round()).abs() > 1e-6 || v.abs() > 1e7 {
            return Vec::new();
        }
        let total = y * 12 + (m - 1) + (v.round() as i64) * months_per;
        let Ok(year) = i32::try_from(total.div_euclid(12)) else { return Vec::new() };
        out.push(GridDate(year, (total.rem_euclid(12) + 1) as u8, d, 0, 0));
    }
    out
}

/// Reads a file's header: its grid variables and time axes. Errors are the user-facing refusals.
fn read_header(file: NcFile, name: &str) -> Result<Opened, ImportError> {
    let format = match file.format() {
        NcFormat::Classic | NcFormat::Offset64 | NcFormat::Cdf5 => "NetCDF-3",
        NcFormat::Nc4 | NcFormat::Nc4Classic => "NetCDF-4",
    };
    let vars: Vec<NcVariable> = file.variables().map_err(|e| ImportError::read(name, e))?.to_vec();
    let mut geoms: Vec<((String, String), Arc<Geom>)> = Vec::new();
    let mut times: Vec<GridTimeAxis> = Vec::new();
    let mut variables = Vec::new();
    let mut layouts = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut geom_error: Option<String> = None;
    for v in &vars {
        let dims = v.dimensions();
        if dims.len() < 2 || v.is_coordinate_variable() || !numeric(v) {
            continue;
        }
        let kinds: Vec<Coord> = dims.iter().map(|d| coordinate(&vars, &d.name).map_or(Coord::Other, classify)).collect();
        let n = dims.len();
        let last_two = (kinds.get(n - 2).copied(), kinds.get(n - 1).copied());
        let lat_first = match last_two {
            (Some(Coord::Lat), Some(Coord::Lon)) => true,
            (Some(Coord::Lon), Some(Coord::Lat)) => false,
            _ => {
                // Bounds and other helper variables (a last dimension of 2) are not data.
                if dims.last().is_some_and(|d| d.size > 2) && attr_text(v, "coordinates").is_none() {
                    skipped.push(v.name().to_string());
                }
                continue;
            }
        };
        let (lat_dim, lon_dim) = if lat_first {
            (dims.get(n - 2).map(|d| d.name.clone()), dims.get(n - 1).map(|d| d.name.clone()))
        } else {
            (dims.get(n - 1).map(|d| d.name.clone()), dims.get(n - 2).map(|d| d.name.clone()))
        };
        let (Some(lat_dim), Some(lon_dim)) = (lat_dim, lon_dim) else { continue };
        let key = (lat_dim.clone(), lon_dim.clone());
        let geom = match geoms.iter().find(|(k, _)| *k == key) {
            Some((_, g)) => g.clone(),
            None => {
                let built = (|| -> Result<Geom, String> {
                    let lat = read_f64(&file, &lat_dim)?;
                    let lon = read_f64(&file, &lon_dim)?;
                    let (lon_w, lon_360) = wrap_longitudes(&lon);
                    let rows = build_axis(&lat, true)?;
                    let cols = build_axis(&lon_w, false)?;
                    let cells = (rows.map.len() as u64).saturating_mul(cols.map.len() as u64);
                    if cells > NETCDF_MAX_CELLS {
                        return Err(format!("{} cells in one time step", group_thousands(cells)));
                    }
                    Ok(Geom { lat_len: lat.len(), lon_len: lon.len(), rows, cols, lon_360 })
                })();
                match built {
                    Ok(g) => {
                        let g = Arc::new(g);
                        geoms.push((key, g.clone()));
                        g
                    }
                    Err(e) => {
                        geom_error.get_or_insert(e);
                        continue;
                    }
                }
            }
        };
        let mut roles = Vec::with_capacity(n);
        let mut fixed = Vec::new();
        let mut time_dim = None;
        let mut level: Option<GridLevels> = None;
        for (k, d) in dims.iter().enumerate().take(n - 2) {
            let kind = kinds.get(k).copied().unwrap_or(Coord::Other);
            if kind == Coord::Time && time_dim.is_none() {
                time_dim = Some(d.name.clone());
                roles.push(Role::Time);
            } else if d.size <= 1 {
                // A dimension of one (GDAL's `band`) is squeezed: nothing to pick, nothing to say.
                roles.push(Role::Squeezed);
            } else if level.is_none() {
                let cv = coordinate(&vars, &d.name);
                let values = cv.and_then(|c| read_f64(&file, c.name()).ok()).filter(|v| v.len() as u64 == d.size);
                level = Some(GridLevels {
                    name: d.name.clone(),
                    long_name: cv.and_then(|c| attr_text(c, "long_name")),
                    unit: cv.and_then(|c| attr_text(c, "units")),
                    values: values.unwrap_or_else(|| (1..=d.size.min(1_000_000)).map(|i| i as f64).collect()),
                });
                roles.push(Role::Level);
            } else {
                fixed.push(d.name.clone());
                roles.push(Role::Fixed);
            }
        }
        roles.extend(if lat_first { [Role::Lat, Role::Lon] } else { [Role::Lon, Role::Lat] });
        if let Some(t) = &time_dim
            && !times.iter().any(|a| a.name == *t)
        {
            let len = dims.iter().find(|d| d.name == *t).map_or(0, |d| d.size as usize);
            let cv = coordinate(&vars, t);
            let dates = match cv {
                Some(c) => {
                    let units = attr_text(c, "units").unwrap_or_default();
                    let calendar = attr_text(c, "calendar").unwrap_or_else(|| "standard".into());
                    let vals = read_f64(&file, c.name()).unwrap_or_default();
                    let d = decode_times(&vals, &units, &calendar);
                    if d.len() == len { d } else { Vec::new() }
                }
                None => Vec::new(),
            };
            let calendar = cv.and_then(|c| attr_text(c, "calendar")).unwrap_or_else(|| "standard".into()).to_ascii_lowercase();
            times.push(GridTimeAxis { name: t.clone(), calendar, dates, len });
        }
        variables.push(GridVariable {
            name: v.name().to_string(),
            long_name: attr_text(v, "long_name").or_else(|| attr_text(v, "standard_name")),
            unit: attr_text(v, "units"),
            time: time_dim,
            levels: level,
        });
        layouts.push(Layout {
            name: v.name().to_string(),
            roles,
            fixed,
            geom,
            lat_first,
            time_chunk: OnceLock::new(),
            scale: attr_num(v, "scale_factor"),
            offset: attr_num(v, "add_offset"),
        });
    }
    // One product per file (GDAL / JRC style): a lone variable with no long name or units takes
    // the file's title ("04_TITLE", "title") and unit ("07_DATA_UOM": "degree Celsius [°C]").
    if let [v] = variables.as_mut_slice() {
        let globals: Vec<(String, String)> = file
            .global_attributes()
            .map(|a| a.iter().filter_map(|a| a.value.as_string().map(|s| (a.name.to_ascii_lowercase(), s.trim().to_string()))).collect())
            .unwrap_or_default();
        let global = |key: &str| {
            globals.iter().find(|(k, s)| !s.is_empty() && (k == key || k.ends_with(&format!("_{key}")))).map(|(_, s)| s.clone())
        };
        if v.long_name.is_none() {
            v.long_name = global("title");
        }
        if v.unit.is_none() {
            v.unit = global("data_uom").or_else(|| global("units")).map(|u| match (u.find('['), u.rfind(']')) {
                (Some(a), Some(b)) if b > a + 1 => u.get(a + 1..b).unwrap_or(&u).trim().to_string(),
                _ => u,
            });
        }
    }
    if variables.is_empty() {
        let curvilinear = vars.iter().any(|v| v.dimensions().len() >= 2 && matches!(classify(v), Coord::Lat | Coord::Lon));
        let projected = vars.iter().any(|v| attr_text(v, "grid_mapping").is_some())
            || vars.iter().any(|v| attr_text(v, "standard_name").is_some_and(|s| s.starts_with("projection_")));
        let msg = if curvilinear {
            format!(
                "{name} uses a curvilinear grid (latitude and longitude that vary in two directions), which {} can't draw yet. Your file has not been changed.",
                lm_core::brand::NAME
            )
        } else if projected {
            format!(
                "{name} uses a projected grid (x and y instead of latitude and longitude), which {} can't draw yet. Your file has not been changed.",
                lm_core::brand::NAME
            )
        } else if let Some(e) = geom_error {
            format!("{name} can't be drawn: {e}. Your file has not been changed.")
        } else {
            format!("{name} has no variable on a latitude–longitude grid, so there is nothing to draw. Your file has not been changed.")
        };
        return Err(ImportError::Refused(msg));
    }
    // The default: the first variable that runs through time.
    let default = variables
        .iter()
        .find(|v| v.time.as_ref().is_some_and(|t| times.iter().any(|a| a.name == *t && a.len > 1)))
        .or_else(|| variables.iter().find(|v| v.time.is_some()))
        .or(variables.first())
        .map(|v| v.name.clone())
        .unwrap_or_default();
    let mut notices = Vec::new();
    if !skipped.is_empty() {
        skipped.truncate(6);
        notices.push(format!("Not on the latitude–longitude grid, so not offered: {}.", skipped.join(", ")));
    }
    let grid = GridProfile { variables, variable: default, level: 0, times, lines: Vec::new(), range: None, sampled: 0 };
    Ok(Opened { file, format, grid, layouts, notices })
}

// ---------------------------------------------------------------------------------------------
// Caches: open files and recent slices

/// Which file (path, size, modification time).
type FileKey = (PathBuf, u64, i64);

fn file_key(path: &Path) -> FileKey {
    let meta = std::fs::metadata(path).ok();
    let mtime = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    (path.to_path_buf(), meta.map_or(0, |m| m.len()), mtime)
}

/// Open files kept for their next slice (the newest last).
const OPEN_FILES: usize = 4;

fn open_files() -> &'static Mutex<Vec<(FileKey, Arc<Opened>)>> {
    static OPEN: OnceLock<Mutex<Vec<(FileKey, Arc<Opened>)>>> = OnceLock::new();
    OPEN.get_or_init(|| Mutex::new(Vec::new()))
}

/// Opens a file (or takes it from the cache) and reads its header.
fn open(path: &Path) -> Result<Arc<Opened>, ImportError> {
    let key = file_key(path);
    if let Some(o) = open_files().lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key).map(|(_, o)| o.clone()) {
        return Ok(o);
    }
    let name = file_name(path);
    let storage = FileStorage::open(path).map_err(|e| ImportError::read(&name, e))?;
    let options = NcOpenOptions { chunk_cache_bytes: 32 << 20, ..NcOpenOptions::default() };
    let file = NcFile::from_storage_with_options(Arc::new(storage), options).map_err(|e| ImportError::read(&name, e))?;
    let opened = Arc::new(read_header(file, &name)?);
    let mut cache = open_files().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(k, _)| k.0 != key.0);
    cache.push((key, opened.clone()));
    if cache.len() > OPEN_FILES {
        cache.remove(0);
    }
    Ok(opened)
}

/// One slice: file, size, modification time, variable, level, time step.
type SliceKey = (String, u64, i64, String, usize, usize);

/// Slices kept, newest last, up to this many bytes of cells (and at least two).
const SLICE_BUDGET: usize = 384 << 20;

fn slices() -> &'static Mutex<Vec<(SliceKey, Arc<Raster>)>> {
    static SLICES: OnceLock<Mutex<Vec<(SliceKey, Arc<Raster>)>>> = OnceLock::new();
    SLICES.get_or_init(|| Mutex::new(Vec::new()))
}

fn slice_key(source: &SourceRef, variable: &str, level: usize, time: usize) -> SliceKey {
    (source.path.clone(), source.size, source.mtime, variable.to_string(), level, time)
}

fn remember(key: SliceKey, r: Arc<Raster>) {
    let mut cache = slices().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(k, _)| *k != key);
    cache.push((key, r));
    let bytes = |c: &Vec<(SliceKey, Arc<Raster>)>| c.iter().map(|(_, r)| r.width as usize * r.height as usize * 4).sum::<usize>();
    while cache.len() > 2 && bytes(&cache) > SLICE_BUDGET {
        cache.remove(0);
    }
}

/// A slice already read, without touching the file (the interface thread may call this).
pub fn cached_slice(source: &SourceRef, grid: &GridProfile, time: usize) -> Option<Arc<Raster>> {
    let key = slice_key(source, &grid.variable, grid.level, time);
    let mut cache = slices().lock().unwrap_or_else(|e| e.into_inner());
    let pos = cache.iter().position(|(k, _)| *k == key)?;
    // Most recently used goes last.
    let entry = cache.remove(pos);
    let r = entry.1.clone();
    cache.push(entry);
    Some(r)
}

/// The 2-D slice of a grid's shown variable and level at time step `time`, read from the file
/// unless it was read recently. Blocking: call it off the interface thread.
pub fn slice(source: &SourceRef, grid: &GridProfile, time: usize) -> Result<Arc<Raster>, ImportError> {
    if let Some(r) = cached_slice(source, grid, time) {
        return Ok(r);
    }
    let path = Path::new(&source.path);
    let o = open(path)?;
    // When a stored chunk holds many time steps (the JRC heatwave files keep 92 days in one),
    // reading one step means inflating the whole chunk: read the steps around it in the same
    // pass and keep them, so the next steps (Play, →) are already there.
    let steps = grid.steps();
    let layout = o.layouts.iter().find(|l| l.name == grid.variable);
    let chunk = layout.map_or(1, |l| time_chunk(&o, l));
    let cells = layout.map_or(1, |l| l.geom.width() * l.geom.height()).max(1);
    let block = chunk.min(BLOCK_BYTES / (cells * 4)).clamp(1, BLOCK_STEPS);
    if block > 1 && steps > 1 {
        let start = time - time % block;
        let n = block.min(steps - start);
        let rasters = read_block(&o, path, &grid.variable, grid.level, start, n)?;
        let mut wanted = None;
        for (k, r) in rasters.into_iter().enumerate() {
            let r = Arc::new(r);
            if start + k == time {
                wanted = Some(r.clone());
            }
            remember(slice_key(source, &grid.variable, grid.level, start + k), r);
        }
        // The step asked for goes last, so it is the newest in the cache.
        if let Some(r) = &wanted {
            remember(slice_key(source, &grid.variable, grid.level, time), r.clone());
        }
        return wanted.ok_or_else(|| ImportError::read(&file_name(path), "a time step is missing from its block"));
    }
    let r = Arc::new(read_slice(&o, path, &grid.variable, grid.level, time)?);
    remember(slice_key(source, &grid.variable, grid.level, time), r.clone());
    Ok(r)
}

/// Most bytes and steps read together from a chunk that holds many time steps.
const BLOCK_BYTES: usize = 128 << 20;
const BLOCK_STEPS: usize = 32;

/// How many time steps a variable's stored chunks hold along time (1 when they hold one, for
/// NetCDF-3, or when it can't be told).
fn time_chunk(o: &Opened, layout: &Layout) -> usize {
    *layout.time_chunk.get_or_init(|| {
        let Some(t) = layout.roles.iter().position(|r| *r == Role::Time) else { return 1 };
        if !matches!(o.format, "NetCDF-4") {
            return 1;
        }
        o.file
            .iter_variable_chunks(&layout.name)
            .ok()
            .and_then(|mut it| it.next())
            .and_then(Result::ok)
            .and_then(|c| c.shape().get(t).copied())
            .map_or(1, |n| n.clamp(1, 100_000) as usize)
    })
}

/// Reads one slice from the file and lays it out north up, west to east.
fn read_slice(o: &Opened, path: &Path, variable: &str, level: usize, time: usize) -> Result<Raster, ImportError> {
    read_block(o, path, variable, level, time, 1)?.pop().ok_or_else(|| ImportError::read(&file_name(path), "no cells were read"))
}

/// Reads `n` consecutive time steps from `start` (one hyperslab) as rasters, north up.
fn read_block(o: &Opened, path: &Path, variable: &str, level: usize, start: usize, n: usize) -> Result<Vec<Raster>, ImportError> {
    let name = file_name(path);
    let layout =
        o.layouts.iter().find(|l| l.name == variable).ok_or_else(|| ImportError::Refused(format!("{name} no longer holds {variable}.")))?;
    let has_time = layout.roles.contains(&Role::Time);
    let n = if has_time { n.max(1) } else { 1 };
    let selections = layout
        .roles
        .iter()
        .map(|r| match r {
            Role::Time if n == 1 => NcSliceInfoElem::Index(start as u64),
            Role::Time => NcSliceInfoElem::Slice { start: start as u64, end: (start + n) as u64, step: 1 },
            Role::Level => NcSliceInfoElem::Index(level as u64),
            Role::Fixed | Role::Squeezed => NcSliceInfoElem::Index(0),
            Role::Lat | Role::Lon => NcSliceInfoElem::Slice { start: 0, end: u64::MAX, step: 1 },
        })
        .collect();
    let data =
        o.file.read_variable_slice_unpacked_masked(variable, &NcSliceInfo { selections }).map_err(|e| ImportError::read(&name, e))?;
    let data = data.as_standard_layout();
    let values = data.as_slice().ok_or_else(|| ImportError::read(&name, "a slice isn't contiguous"))?;
    let g = &layout.geom;
    let per = g.lat_len * g.lon_len;
    if values.len() != per * n {
        return Err(ImportError::read(&name, format!("{variable}: {} cells, expected {}", values.len(), per * n)));
    }
    values.chunks(per.max(1)).map(|v| lay_out(v, layout, path, variable)).collect()
}

/// One step's cells, in the file's order, laid out north up and west to east.
fn lay_out(values: &[f64], layout: &Layout, path: &Path, variable: &str) -> Result<Raster, ImportError> {
    let name = file_name(path);
    let g = &layout.geom;
    let (w, h) = (g.width(), g.height());
    let mut out = vec![f32::NAN; w * h];
    for (r, src_r) in g.rows.map.iter().enumerate() {
        let Some(i) = *src_r else { continue };
        for (c, src_c) in g.cols.map.iter().enumerate() {
            let Some(j) = *src_c else { continue };
            let at = if layout.lat_first { i * g.lon_len + j } else { j * g.lat_len + i };
            if let (Some(v), Some(dst)) = (values.get(at), out.get_mut(r * w + c))
                && v.is_finite()
                && v.abs() < DEFAULT_FILL
            {
                *dst = *v as f32;
            }
        }
    }
    let mut raster =
        Raster::new(variable, w as u32, h as u32, vec![out], g.transform(), RasterCrs::LonLat, None, ColorHint::Gray, "Float32")
            .map_err(|e| ImportError::read(&name, e))?;
    if let Some(b) = raster.bands.first_mut() {
        b.name = variable.to_string();
    }
    raster.source = Some(path.display().to_string());
    Ok(raster)
}

// ---------------------------------------------------------------------------------------------
// The layer

/// The visible assumption lines for a variable at a level.
fn lines_for(o: &Opened, layout: &Layout, v: &GridVariable, level: usize) -> Vec<String> {
    let mut out = Vec::new();
    let g = &layout.geom;
    if g.lon_360 {
        out.push("Longitudes 0–360 shown as −180–180.".into());
    }
    let axis = v.time.as_ref().and_then(|t| o.grid.times.iter().find(|a| a.name == *t));
    if let Some(a) = axis {
        if a.dates.is_empty() && a.len > 1 {
            out.push("The time axis couldn't be read, so steps are numbered.".into());
        } else {
            match a.calendar.as_str() {
                "noleap" | "365_day" => out.push("Time: noleap calendar (no leap days; every year has 365 days).".into()),
                "360_day" => out.push("Time: 360-day calendar (twelve 30-day months).".into()),
                "all_leap" | "366_day" => out.push("Time: all-leap calendar (every year has 366 days).".into()),
                "julian" => out.push("Time: Julian calendar.".into()),
                _ => {}
            }
        }
    }
    if let Some(l) = &v.levels
        && l.values.len() > 1
    {
        out.push(format!("Level: {} ({} of {}).", v.level_label(level), level + 1, l.values.len()));
    }
    for d in &layout.fixed {
        out.push(format!("Shown at the first {d}; other {d} values aren't offered yet."));
    }
    let uneven = |a: &Axis| a.off > a.step.abs() * 0.01;
    if uneven(&g.rows) || uneven(&g.cols) {
        let which = if uneven(&g.rows) { "Latitudes" } else { "Longitudes" };
        let off = g.rows.off.max(g.cols.off);
        out.push(format!(
            "{which} are unevenly spaced; cells are drawn on an even grid, at most {}° from their place in the file.",
            short(off)
        ));
    }
    if layout.scale.is_some_and(|v| v != 1.0) || layout.offset.is_some_and(|v| v != 0.0) {
        out.push(format!(
            "Values unpacked with the file's scale_factor ({}) and add_offset ({}).",
            short(layout.scale.unwrap_or(1.0)),
            short(layout.offset.unwrap_or(0.0))
        ));
    }
    out
}

/// A short number for a line ("0.01", "273.15").
fn short(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// The value range of a variable over up to [`RANGE_SAMPLE_STEPS`] evenly spaced time steps:
/// (min, max), the 2nd and 98th percentiles of the cells, a sample of cell values, and how many
/// steps were read.
fn sample_range(
    o: &Opened,
    path: &Path,
    source: &SourceRef,
    grid: &GridProfile,
    ctx: Option<&Ctx>,
) -> Result<(Option<(f64, f64)>, Option<(f64, f64)>, Vec<f64>, usize), ImportError> {
    let steps = grid.steps();
    // One step per stored chunk at most when a chunk holds many steps: each step read from such
    // a chunk inflates all of it (4 steps for a year of daily data in 92-day chunks).
    let chunk = o.layouts.iter().find(|l| l.name == grid.variable).map_or(1, |l| time_chunk(o, l));
    let k = steps.clamp(1, RANGE_SAMPLE_STEPS).min(steps.div_ceil(chunk.max(1)).max(2));
    let picks: Vec<usize> = if k == 1 { vec![0] } else { (0..k).map(|i| i * (steps - 1) / (k - 1)).collect() };
    let mut sample: Vec<f64> = Vec::new();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &t in &picks {
        if let Some(c) = ctx {
            c.ticket.check()?;
        }
        let r = match cached_slice(source, grid, t) {
            Some(r) => r,
            None => {
                let r = Arc::new(read_slice(o, path, &grid.variable, grid.level, t)?);
                remember(slice_key(source, &grid.variable, grid.level, t), r.clone());
                r
            }
        };
        let Some(b) = r.bands.first() else { continue };
        let stride = (b.data.len() / 100_000).max(1);
        for (i, v) in b.data.iter().enumerate() {
            if !v.is_finite() {
                continue;
            }
            let x = f64::from(*v);
            lo = lo.min(x);
            hi = hi.max(x);
            if i % stride == 0 {
                sample.push(x);
            }
        }
    }
    let full = (lo.is_finite() && hi.is_finite()).then_some((lo, hi));
    sample.sort_by(f64::total_cmp);
    let pct = |p: f64| sample.get(((sample.len().saturating_sub(1)) as f64 * p).round() as usize).copied();
    let window = match (pct(0.02), pct(0.98)) {
        (Some(a), Some(b)) if b > a => Some((a, b)),
        _ => full.filter(|(a, b)| b > a),
    };
    Ok((full, window, sample, picks.len()))
}

/// The layer for one variable and level of an opened file, showing time step `time`.
fn layer_for(o: &Opened, path: &Path, variable: &str, level: usize, time: usize, ctx: Option<&Ctx>) -> Result<ImportedLayer, ImportError> {
    let name = file_name(path);
    let source = source_ref(path, None);
    let layout =
        o.layouts.iter().find(|l| l.name == variable).ok_or_else(|| ImportError::Refused(format!("{name} no longer holds {variable}.")))?;
    let var = o
        .grid
        .variables
        .iter()
        .find(|v| v.name == variable)
        .cloned()
        .ok_or_else(|| ImportError::Refused(format!("{name} no longer holds {variable}.")))?;
    let levels = var.levels.as_ref().map_or(1, |l| l.values.len().max(1));
    let mut grid = o.grid.clone();
    grid.variable = variable.to_string();
    grid.level = level.min(levels - 1);
    let time = time.min(grid.steps() - 1);
    if let Some(c) = ctx {
        c.stage(Stage::Profile)?;
    }
    let (full, window, sample, sampled) = sample_range(o, path, &source, &grid, ctx)?;
    grid.lines = lines_for(o, layout, &var, grid.level);
    grid.range = window;
    grid.sampled = sampled;
    let label = var.label();
    let (cell_meaning, meaning) = lm_profile::raster_cell_meaning(&label, &name, &sample, 1);
    let unit = var.unit.as_deref().map(unit_text).filter(|u| !u.is_empty());
    let band = BandProfile {
        index: 1,
        name: label,
        nodata: Known::certain(None, "Cells the file marks as missing (_FillValue, missing_value, outside the valid range) are No data."),
        // The cells are stored unpacked: no scale or offset is left to apply.
        scale: None,
        offset: None,
        unit: match &unit {
            Some(u) => Known::certain(Some(u.clone()), "From the variable's units attribute."),
            None => Known::unknown(None, "The variable has no units attribute."),
        },
        cell_meaning,
        min: full.map(|(a, _)| a),
        max: full.map(|(_, b)| b),
    };
    let raster = match cached_slice(&source, &grid, time) {
        Some(r) => r,
        None => {
            let r = Arc::new(read_slice(o, path, &grid.variable, grid.level, time)?);
            remember(slice_key(&source, &grid.variable, grid.level, time), r.clone());
            r
        }
    };
    let e = raster.extent();
    let extent =
        (e.min_x.is_finite() && e.max_x > e.min_x).then_some(Extent { min_x: e.min_x, min_y: e.min_y, max_x: e.max_x, max_y: e.max_y });
    let g = &layout.geom;
    let mut notices = o.notices.clone();
    notices.insert(
        0,
        format!(
            "{} file with {} variable{} on a latitude–longitude grid of {} × {} cells.",
            o.format,
            o.grid.variables.len(),
            if o.grid.variables.len() == 1 { "" } else { "s" },
            group_thousands(g.width() as u64),
            group_thousands(g.height() as u64)
        ),
    );
    let profile = LayerProfile {
        source,
        format: o.format.into(),
        kind: LayerKind::Raster,
        count: (g.width() as u64).saturating_mul(g.height() as u64),
        extent_wgs84: extent,
        crs: Known::certain(Crs::Wgs84, "Latitude and longitude from the file's coordinate variables."),
        fields: Vec::new(),
        bands: vec![band],
        assumptions: meaning.into_iter().collect(),
        exclusions: Vec::new(),
        encoding: None,
        notices,
        data_rev: 0,
        joins: Vec::new(),
        grid: Some(grid),
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("grid").to_string();
    // One colour range for every time step, from the sampled steps (editable in Style).
    let mut style = lm_core::document::Style {
        stretch: default_stretch(&profile),
        ..lm_core::document::Style::for_profile(&profile, lm_core::document::OKABE_ITO[0])
    };
    // Heat and temperature grids (a heatwave index, air temperature) open in the heat ramp.
    if profile.bands.first().is_some_and(|b| {
        let n = b.name.to_lowercase();
        n.contains("heat") || n.contains("temperature")
    }) {
        style.ramp = "heat".into();
    }
    Ok(ImportedLayer { name: stem, status: status_of(&profile), profile, data: LayerData::Raster(raster), raw: None, style: Some(style) })
}

/// The colour range a grid layer opens with, the same for every time step: the 2nd to 98th
/// percentile of the cells of the sampled steps.
pub fn default_stretch(profile: &LayerProfile) -> Option<(f64, f64)> {
    profile.grid.as_ref().and_then(|g| g.range).or_else(|| profile.bands.first().and_then(|b| b.min.zip(b.max)).filter(|(a, b)| b > a))
}

/// Shows another variable (or level) of a grid layer, at time step `time`: the layer's name is
/// kept; its band, range, assumption lines and cells are the new variable's. The file is read,
/// never written.
pub fn choose_variable(layer: &ImportedLayer, variable: &str, level: usize, time: usize) -> Result<ImportedLayer, ImportError> {
    let path = PathBuf::from(&layer.profile.source.path);
    let o = open(&path)?;
    let mut out = layer_for(&o, &path, variable, level, time, None)?;
    out.name = layer.name.clone();
    out.profile.data_rev = layer.profile.data_rev;
    out.profile.source.sha256 = layer.profile.source.sha256.clone();
    Ok(out)
}

impl FormatAdapter for NetCdf {
    fn name(&self) -> &'static str {
        "NetCDF"
    }

    fn sniff(&self, path: &Path, head: &[u8]) -> bool {
        EXTENSIONS.contains(&extension(path).as_str()) || matches!(head, [b'C', b'D', b'F', 1 | 2 | 5, ..])
    }

    fn is_raster(&self) -> bool {
        true
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let size = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?.len();
        let o = open(path)?;
        let pixels = o.layouts.iter().find(|l| l.name == o.grid.variable).map(|l| (l.geom.width() as u64, l.geom.height() as u64));
        Ok(Metadata { size, pixels, ..Metadata::default() })
    }

    fn read_window(&self, path: &Path, _meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let o = open(path)?;
        ctx.stage(Stage::Extent)?;
        let variable = o.grid.variable.clone();
        let layer = layer_for(&o, path, &variable, 0, 0, Some(ctx))?;
        Ok(vec![layer])
    }
}

#[cfg(test)]
mod tests;
