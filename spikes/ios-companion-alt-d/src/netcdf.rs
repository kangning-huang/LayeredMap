//! NetCDF grids for the Alt D spike, read with the pure-Rust `netcdf-reader`.
//!
//! `rustgis-data` can already read NetCDF, but only by shelling out to GDAL's `gdal_translate`
//! (`crates/data/src/raster/gdal.rs`), which does not exist on an iPhone. The desktop reader
//! (`crates/lm-jobs/src/import/netcdf.rs`) is pure Rust but is wired into `lm-core`'s layer and
//! grid profiles, the time slider and the variable dropdown — far more than a throwaway spike can
//! carry. So this is a deliberately small reading of the same file format, cut to what the spike
//! shows: **one variable, its first time step and first level, as one grid of values.**
//!
//! What it keeps from the desktop reader, because leaving these out gives wrong maps rather than
//! missing ones: latitude/longitude found by CF `units` and `standard_name` (not just by name),
//! `scale_factor`/`add_offset` unpacking and `_FillValue`/`missing_value` masking (both done by
//! `read_variable_slice_unpacked_masked`), longitudes of 0–360 wrapped to −180–180, and a grid
//! stored south-to-north turned north up.
//!
//! What it drops: picking a variable, the time axis, levels, curvilinear and projected grids
//! (refused with a plain sentence), and the colour range sampled across time.

use netcdf_reader::{FileStorage, NcFile, NcOpenOptions, NcSliceInfo, NcSliceInfoElem, NcType, NcVariable};
use rustgis_data::raster::{ColorHint, Crs, GeoTransform, Raster};
use std::path::Path;
use std::sync::Arc;

/// Read as NetCDF. The HDF5 signature alone could be any HDF5 file, so the extension decides.
pub const EXTENSIONS: &[&str] = &["nc", "nc4", "cdf", "netcdf"];

/// Longest side of a grid this spike will draw; the phone has to hold it.
const MAX_SIDE: u64 = 20_000;
/// The default fill of NetCDF floats (9.96921e36), drawn as No data even when undeclared.
const DEFAULT_FILL: f64 = 9.9e36;

pub fn is_netcdf(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    EXTENSIONS.contains(&ext.as_str())
}

fn attr_text(v: &NcVariable, name: &str) -> Option<String> {
    v.attribute(name).and_then(|a| a.value.as_string()).map(|s| s.trim().trim_end_matches('\0').to_string()).filter(|s| !s.is_empty())
}

fn numeric(v: &NcVariable) -> bool {
    use NcType as T;
    matches!(v.dtype(), T::Byte | T::Short | T::Int | T::Float | T::Double | T::UByte | T::UShort | T::UInt | T::Int64 | T::UInt64)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Coord {
    Lat,
    Lon,
    Other,
}

/// Latitude or longitude from a 1-D coordinate's CF units, standard name, or name. Files written
/// by GDAL often give `lat`/`lon` no attributes at all, so a bare name counts when the units say
/// degrees or say nothing.
fn classify(v: &NcVariable) -> Coord {
    let units = attr_text(v, "units").unwrap_or_default().to_ascii_lowercase().replace([' ', '-'], "_");
    let std = attr_text(v, "standard_name").unwrap_or_default().to_ascii_lowercase();
    let name = v.name().to_ascii_lowercase();
    let lat_units = matches!(units.as_str(), "degrees_north" | "degree_north" | "degree_n" | "degrees_n" | "degreen" | "degreesn");
    let lon_units = matches!(units.as_str(), "degrees_east" | "degree_east" | "degree_e" | "degrees_e" | "degreee" | "degreese");
    let degrees = units.starts_with("degree") || units.is_empty();
    if lat_units || std == "latitude" || (degrees && matches!(name.as_str(), "lat" | "latitude" | "lats" | "nav_lat")) {
        return Coord::Lat;
    }
    if lon_units || std == "longitude" || (degrees && matches!(name.as_str(), "lon" | "longitude" | "lons" | "long" | "nav_lon")) {
        return Coord::Lon;
    }
    Coord::Other
}

/// The 1-D coordinate variable of a dimension (named like it, along it alone).
fn coordinate<'a>(vars: &'a [NcVariable], dim: &str) -> Option<&'a NcVariable> {
    vars.iter().find(|v| v.name() == dim && v.dimensions().len() == 1 && v.dimensions().first().is_some_and(|d| d.name == dim))
}

/// One axis of the drawn grid: for each output cell, the file's index along that dimension.
struct Axis {
    map: Vec<Option<usize>>,
    start: f64,
    step: f64,
}

/// Lays a coordinate's values on an even grid. Rows run north to south, columns west to east.
/// A grid too uneven to draw that way (two cells landing on one place) is refused rather than
/// silently losing a row.
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
    // The lower median: a regional grid across the antimeridian has one huge gap.
    let step = gaps.get(gaps.len().saturating_sub(1) / 2).copied().unwrap_or(1.0);
    if !(step.is_finite() && step > 0.0) {
        return Err("a coordinate has no spacing".into());
    }
    let first = sorted.first().copied().unwrap_or(0.0);
    let last = sorted.last().copied().unwrap_or(0.0);
    let n = ((last - first).abs() / step).round() as u64 + 1;
    if n > MAX_SIDE {
        return Err(format!("{n} cells along one side is more than this phone will draw"));
    }
    let signed = if descending { -step } else { step };
    let mut map = vec![None; n as usize];
    for (k, &i) in order.iter().enumerate() {
        let v = sorted.get(k).copied().unwrap_or(first);
        let pos = ((v - first) / signed).round().max(0.0) as usize;
        let d = (v - (first + pos as f64 * signed)).abs();
        match map.get_mut(pos) {
            Some(slot) if slot.is_none() => *slot = Some(i),
            _ if d > 1e-9 => return Err("the cells are too unevenly spaced to draw on an even grid".into()),
            // A repeated coordinate (−180 and 180 both given): the first is kept.
            _ => {}
        }
    }
    Ok(Axis { map, start: first, step: signed })
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

const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// How far apart the steps are, from their median spacing — the same four cadences the desktop
/// uses, so a monthly file reads "July 2023" and a yearly one just "2023".
fn labels_for(times: &[f64], units: &str, calendar: &str, steps: usize) -> Vec<String> {
    use netcdf_reader::cf::{CfCalendar, decode_times_exact, parse_time_units};
    let numbered = || (0..steps).map(|i| format!("Step {} of {}", i + 1, steps.max(1))).collect::<Vec<_>>();
    let Ok(time_ref) = parse_time_units(units, CfCalendar::parse(calendar)) else { return numbered() };
    let Ok(dates) = decode_times_exact(times, &time_ref) else { return numbered() };
    if dates.len() != steps {
        return numbered();
    }
    // Median gap in days, so one odd step can't change how the whole axis reads.
    let mut gaps: Vec<f64> = dates
        .windows(2)
        .filter_map(|w| {
            let (a, b) = (w.first()?, w.get(1)?);
            Some((days(b) - days(a)).abs())
        })
        .collect();
    gaps.sort_by(f64::total_cmp);
    let gap = gaps.get(gaps.len() / 2).copied().unwrap_or(1.0);
    dates
        .iter()
        .map(|d| {
            let (y, mo, da, h, mi) = (d.year, d.month, d.day, d.hour, d.minute);
            let month = MONTHS.get(usize::from(mo).saturating_sub(1)).copied().unwrap_or("");
            let short = month.get(..3).unwrap_or("");
            if gap >= 330.0 {
                format!("{y}")
            } else if gap >= 27.0 {
                format!("{month} {y}")
            } else if gap >= 1.0 {
                format!("{da} {short} {y}")
            } else {
                format!("{da} {short} {y} {h:02}:{mi:02}")
            }
        })
        .collect()
}

fn days(d: &netcdf_reader::cf::CfDateTime) -> f64 {
    f64::from(d.year) * 365.25
        + (f64::from(d.month) - 1.0) * 30.44
        + f64::from(d.day)
        + (f64::from(d.hour) + f64::from(d.minute) / 60.0) / 24.0
}

/// What this spike shows from a NetCDF file.
pub struct Opened {
    pub raster: Raster,
    /// `long_name`/`standard_name` of the variable, for the card's heading.
    pub quantity: String,
    /// The variable's CF `units`, which beats guessing a unit from the file name.
    pub unit: String,
    /// Plain sentences about what was assumed, shown under More details.
    pub notes: Vec<String>,
    /// One label per time step ("July 2023"), empty when the file has no time axis.
    pub times: Vec<String>,
    /// Which step this grid is, into `times`.
    pub time: usize,
}

/// Opens a NetCDF file as one grid of values: the first variable on a regular latitude–longitude
/// grid, at its first time step and first level.
pub fn open(path: &Path) -> Result<Opened, String> {
    open_at(path, 0)
}

/// As [`open`], at time step `time` (clamped to the axis). Stepping re-reads one 2-D hyperslab,
/// not the file: a multi-gigabyte file steps in a moment, same as the desktop reader.
pub fn open_at(path: &Path, time: usize) -> Result<Opened, String> {
    let options = NcOpenOptions { chunk_cache_bytes: 16 << 20, ..NcOpenOptions::default() };
    let storage: netcdf_reader::DynStorage = Arc::new(FileStorage::open(path).map_err(|e| e.to_string())?);
    let file = NcFile::from_storage_with_options(storage, options).map_err(|e| e.to_string())?;
    let vars: Vec<NcVariable> = file.variables().map_err(|e| e.to_string())?.to_vec();

    // The first variable whose last two dimensions are latitude and longitude. Files commonly
    // lead with helper variables (bounds, projection), so this is a scan, not "the first".
    let mut refused: Option<String> = None;
    for v in &vars {
        let dims = v.dimensions();
        if dims.len() < 2 || v.is_coordinate_variable() || !numeric(v) {
            continue;
        }
        let kinds: Vec<Coord> = dims.iter().map(|d| coordinate(&vars, &d.name).map_or(Coord::Other, classify)).collect();
        let n = dims.len();
        let lat_first = match (kinds.get(n - 2).copied(), kinds.get(n - 1).copied()) {
            (Some(Coord::Lat), Some(Coord::Lon)) => true,
            (Some(Coord::Lon), Some(Coord::Lat)) => false,
            _ => {
                // A last dimension of 2 is a bounds variable, not data worth mentioning.
                if dims.last().is_some_and(|d| d.size > 2) && refused.is_none() {
                    refused = Some(v.name().to_string());
                }
                continue;
            }
        };
        let (lat_dim, lon_dim) = if lat_first { (dims.get(n - 2), dims.get(n - 1)) } else { (dims.get(n - 1), dims.get(n - 2)) };
        let (Some(lat_dim), Some(lon_dim)) = (lat_dim, lon_dim) else { continue };
        let (Some(lat_cv), Some(lon_cv)) = (coordinate(&vars, &lat_dim.name), coordinate(&vars, &lon_dim.name)) else { continue };
        let lats = file.read_variable_as_f64(lat_cv.name()).map_err(|e| e.to_string())?.iter().copied().collect::<Vec<f64>>();
        let lons = file.read_variable_as_f64(lon_cv.name()).map_err(|e| e.to_string())?.iter().copied().collect::<Vec<f64>>();
        let (lons, lon_360) = wrap_longitudes(&lons);
        let rows = build_axis(&lats, true)?;
        let cols = build_axis(&lons, false)?;
        let (w, h) = (cols.map.len(), rows.map.len());

        // Which leading dimension is time, if any: the one with a "… since …" coordinate.
        let time_at = (0..n.saturating_sub(2)).find(|&k| {
            dims.get(k).and_then(|d| coordinate(&vars, &d.name)).is_some_and(|cv| {
                let units = attr_text(cv, "units").unwrap_or_default().to_ascii_lowercase().replace([' ', '-'], "_");
                let std = attr_text(cv, "standard_name").unwrap_or_default().to_ascii_lowercase();
                units.contains("_since_") || std == "time" || attr_text(cv, "axis").is_some_and(|a| a.eq_ignore_ascii_case("t"))
            })
        });
        let steps = time_at.and_then(|k| dims.get(k)).map_or(1, |d| d.size.max(1) as usize);
        let time = time.min(steps.saturating_sub(1));

        // One hyperslab: every latitude and longitude, the chosen time, the first of anything else.
        let selections = (0..n)
            .map(|k| {
                if k >= n - 2 {
                    NcSliceInfoElem::Slice { start: 0, end: u64::MAX, step: 1 }
                } else if Some(k) == time_at {
                    NcSliceInfoElem::Index(time as u64)
                } else {
                    NcSliceInfoElem::Index(0)
                }
            })
            .collect();
        // Unpacks scale_factor/add_offset and masks _FillValue / missing_value / valid_range.
        let data = file.read_variable_slice_unpacked_masked(v.name(), &NcSliceInfo { selections }).map_err(|e| e.to_string())?;
        let data = data.as_standard_layout();
        let values = data.as_slice().ok_or("a slice isn't contiguous")?;
        let (lat_len, lon_len) = (lats.len(), lons.len());
        if values.len() != lat_len * lon_len {
            return Err(format!("{}: {} cells, expected {}", v.name(), values.len(), lat_len * lon_len));
        }

        let mut out = vec![f32::NAN; w * h];
        for (r, src_r) in rows.map.iter().enumerate() {
            let Some(i) = *src_r else { continue };
            for (c, src_c) in cols.map.iter().enumerate() {
                let Some(j) = *src_c else { continue };
                let at = if lat_first { i * lon_len + j } else { j * lat_len + i };
                if let (Some(value), Some(dst)) = (values.get(at), out.get_mut(r * w + c))
                    && value.is_finite()
                    && value.abs() < DEFAULT_FILL
                {
                    *dst = *value as f32;
                }
            }
        }

        let transform = GeoTransform {
            origin_x: cols.start - cols.step / 2.0,
            origin_y: rows.start - rows.step / 2.0,
            pixel_width: cols.step,
            pixel_height: rows.step,
        };
        let quantity = attr_text(v, "long_name").or_else(|| attr_text(v, "standard_name")).unwrap_or_else(|| v.name().to_string());
        let unit = attr_text(v, "units").unwrap_or_default();
        let mut raster = Raster::new(&quantity, w as u32, h as u32, vec![out], transform, Crs::LonLat, None, ColorHint::Gray, "Float32")
            .map_err(|e| e.to_string())?;
        if let Some(b) = raster.bands.first_mut() {
            b.name = v.name().to_string();
        }

        // The time axis in words, for the slider's label.
        let times = match time_at.and_then(|k| dims.get(k)).and_then(|d| coordinate(&vars, &d.name)) {
            Some(cv) if steps > 1 => {
                let raw = file.read_variable_as_f64(cv.name()).map(|a| a.iter().copied().collect::<Vec<f64>>()).unwrap_or_default();
                let units = attr_text(cv, "units").unwrap_or_default();
                let calendar = attr_text(cv, "calendar").unwrap_or_else(|| "standard".into());
                labels_for(&raw, &units, &calendar, steps)
            }
            _ if steps > 1 => (0..steps).map(|i| format!("Step {} of {}", i + 1, steps)).collect(),
            _ => Vec::new(),
        };

        let mut notes = Vec::new();
        if lon_360 {
            notes.push("Longitudes 0–360 in the file are shown as −180–180.".to_string());
        }
        // Dimensions read at their first index — not the time one, which the slider now moves.
        let fixed: Vec<String> =
            (0..n.saturating_sub(2)).filter(|k| Some(*k) != time_at).filter_map(|k| dims.get(k).map(|d| d.name.clone())).collect();
        if !fixed.is_empty() {
            notes.push(format!("This file also varies by {}; the first of each is shown.", fixed.join(", ")));
        }
        if vars.iter().filter(|o| o.dimensions().len() >= 2 && !o.is_coordinate_variable() && numeric(o)).count() > 1 {
            notes.push(format!("The file holds several measurements; “{}” is shown.", quantity));
        }
        return Ok(Opened { raster, quantity, unit, notes, times, time });
    }

    Err(match refused {
        // Curvilinear and projected grids: 2-D latitude/longitude, or none at all.
        Some(name) => {
            format!("“{name}” isn't on a plain latitude–longitude grid. NimbleMap can't open curved or projected NetCDF grids yet.")
        }
        None => "This NetCDF file has no grid with latitude and longitude in it.".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LM_NC=<a .nc file>: open it and print what the spike would show.
    #[test]
    #[ignore]
    fn opens_a_real_netcdf() {
        let Ok(path) = std::env::var("LM_NC") else { return };
        let o = open(Path::new(&path)).expect("open");
        let b = o.raster.bands.first().expect("band");
        let e = o.raster.extent();
        println!("quantity = {:?}  unit = {:?}", o.quantity, o.unit);
        println!("grid     = {}x{}", o.raster.width, o.raster.height);
        println!("extent   = {:.3},{:.3} .. {:.3},{:.3}", e.min_x, e.min_y, e.max_x, e.max_y);
        println!("range    = {:.3} .. {:.3}", b.stats.min, b.stats.max);
        println!("notes    = {:?}", o.notes);
        println!("steps    = {}", o.times.len());
        if let (Some(a), Some(z)) = (o.times.first(), o.times.last()) {
            println!("time     = {a:?} … {z:?}");
        }
        assert!(o.raster.width > 1 && o.raster.height > 1);
        assert!(b.stats.max > b.stats.min, "the grid has no range");
    }

    /// LM_NC=<a .nc file with a time axis>: stepping must change the cells, not just the label.
    #[test]
    #[ignore]
    fn steps_through_time() {
        let Ok(path) = std::env::var("LM_NC") else { return };
        let p = Path::new(&path);
        let first = open_at(p, 0).expect("step 0");
        if first.times.len() < 2 {
            println!("no time axis in this file; nothing to step");
            return;
        }
        let last = first.times.len() - 1;
        let later = open_at(p, last).expect("last step");
        println!("{} steps: {:?} … {:?}", first.times.len(), first.times.first(), first.times.last());
        println!("step 0    mean {:?}", mean(&first));
        println!("step {last} mean {:?}", mean(&later));

        assert_eq!(first.time, 0);
        assert_eq!(later.time, last, "the last step should be the one asked for");
        assert_eq!(first.raster.width, later.raster.width, "the grid must keep its shape");
        // Same place, different time: the numbers have to move, or the slider does nothing.
        assert_ne!(mean(&first), mean(&later), "two time steps gave identical cells");

        // Past the end clamps rather than failing.
        let beyond = open_at(p, last + 500).expect("clamped");
        assert_eq!(beyond.time, last);
    }

    fn mean(o: &Opened) -> Option<i64> {
        let b = o.raster.bands.first()?;
        let finite: Vec<f32> = b.data.iter().copied().filter(|v| v.is_finite()).collect();
        (!finite.is_empty()).then(|| (f64::from(finite.iter().sum::<f32>()) / finite.len() as f64 * 1000.0) as i64)
    }
}
