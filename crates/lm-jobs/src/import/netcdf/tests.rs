//! NetCDF reading against xarray (fixtures R8, `reference/make_netcdf_fixtures.py`), CF calendars,
//! and refusals.

use std::collections::HashMap;
use std::path::PathBuf;

use super::*;
use crate::import::{Ticket, import_path};

fn r8(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/R8").join(name)
}

fn import(name: &str) -> ImportedLayer {
    import_path(&r8(name), &Ctx::detached(Ticket::default())).unwrap().remove(0)
}

fn rows(file: &str) -> Vec<HashMap<String, String>> {
    let mut rdr = ::csv::Reader::from_path(r8(file)).unwrap();
    let headers = rdr.headers().unwrap().clone();
    rdr.records().map(|r| headers.iter().zip(r.unwrap().iter()).map(|(h, v)| (h.to_string(), v.to_string())).collect()).collect()
}

fn grid(l: &ImportedLayer) -> &GridProfile {
    l.profile.grid.as_ref().unwrap()
}

/// The layer showing `variable` at `level`, and its raster at time step `time`.
fn shown(file: &str, variable: &str, level: usize, time: usize) -> (ImportedLayer, Arc<Raster>) {
    let base = import(file);
    let l = choose_variable(&base, variable, level, time).unwrap();
    let r = slice(&l.profile.source, grid(&l), time).unwrap();
    (l, r)
}

#[test]
fn every_cell_matches_xarray() {
    let mut checked = 0;
    let mut by_slice: HashMap<(String, String, usize, usize), Arc<Raster>> = HashMap::new();
    for row in rows("expected_values.csv") {
        let (file, var) = (row["file"].clone(), row["variable"].clone());
        let t: usize = row["time_index"].parse().unwrap();
        let lv: usize = row["level_index"].parse().unwrap();
        let r = by_slice.entry((file.clone(), var.clone(), lv, t)).or_insert_with(|| shown(&file, &var, lv, t).1).clone();
        let (lon, lat): (f64, f64) = (row["lon"].parse().unwrap(), row["lat"].parse().unwrap());
        let got =
            crate::inspect::raster_value_at(&r, 0, lon, lat).unwrap_or_else(|| panic!("{file} {var} t{t}: ({lon}, {lat}) is outside"));
        match row["value"].as_str() {
            "" => assert!(got.is_none(), "{file} {var} t{t} l{lv} ({lon}, {lat}): {got:?}, xarray has no data"),
            v => {
                let want: f64 = v.parse().unwrap();
                let got = got.unwrap_or_else(|| panic!("{file} {var} t{t} l{lv} ({lon}, {lat}): no data, xarray {want}"));
                assert!((got - want).abs() <= 1e-4 * want.abs().max(1.0), "{file} {var} t{t} l{lv} ({lon}, {lat}): {got} vs xarray {want}");
            }
        }
        checked += 1;
    }
    assert!(checked > 1000, "{checked}");
}

#[test]
fn slice_statistics_match_xarray() {
    for row in rows("expected_slices.csv") {
        let t: usize = row["time_index"].parse().unwrap();
        let lv: usize = row["level_index"].parse().unwrap();
        let (_, r) = shown(&row["file"], &row["variable"], lv, t);
        let s = &r.bands[0].stats;
        let what = format!("{} {} t{t} l{lv}", row["file"], row["variable"]);
        assert_eq!(s.count, row["valid"].parse::<usize>().unwrap(), "{what}");
        for (got, key) in [(s.min, "min"), (s.max, "max"), (s.mean, "mean")] {
            let want: f64 = row[key].parse().unwrap();
            assert!((got - want).abs() <= 1e-4 * want.abs().max(1.0), "{what} {key}: {got} vs {want}");
        }
    }
}

#[test]
fn dates_match_cftime() {
    let mut files: HashMap<String, ImportedLayer> = HashMap::new();
    for row in rows("expected_dates.csv") {
        let l = files.entry(row["file"].clone()).or_insert_with(|| import(&row["file"]));
        let i: usize = row["time_index"].parse().unwrap();
        let a = grid(l).axis().unwrap();
        let d = a.dates[i];
        assert_eq!(format!("{:04}-{:02}-{:02} {:02}:{:02}:00", d.0, d.1, d.2, d.3, d.4), row["date"], "{} step {i}", row["file"]);
    }
}

#[test]
fn classic_file_opens_with_its_variables_and_labels() {
    let l = import("r8_classic.nc");
    let g = grid(&l);
    assert_eq!(l.profile.format, "NetCDF-3");
    let names: Vec<&str> = g.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["t2m", "ta", "orog"]);
    // The default is the first variable with a time dimension.
    assert_eq!(g.variable, "t2m");
    assert_eq!(g.shown().unwrap().label(), "2 m temperature (°C)");
    assert_eq!((g.steps(), g.date_label(0), g.date_label(1)), (3, "June 2023".to_string(), "July 2023".to_string()));
    let ta = g.variables.iter().find(|v| v.name == "ta").unwrap();
    assert_eq!(ta.levels.as_ref().unwrap().values, [850.0, 500.0]);
    assert!(g.variables.iter().find(|v| v.name == "orog").unwrap().time.is_none());
    // South-up rows are turned north up; the grid is regular −180–180.
    let LayerData::Raster(r) = &l.data else { panic!() };
    assert!(r.transform.pixel_height < 0.0);
    assert_eq!((r.width, r.height), (8, 6));
    let e = l.profile.extent_wgs84.unwrap();
    assert_eq!((e.min_x, e.max_x, e.min_y, e.max_y), (-180.0, 180.0, -60.0, 60.0));
    assert_eq!(l.profile.bands[0].unit.value.as_deref(), Some("°C"));
    // The colour range comes from every step (only 3 here).
    assert_eq!(g.sampled, 3);
    assert!(g.lines.is_empty(), "{:?}", g.lines);
    // A level picker's line.
    let (lt, _) = shown("r8_classic.nc", "ta", 1, 0);
    assert_eq!(grid(&lt).lines, ["Level: 500 hPa (2 of 2)."]);
    assert_eq!(grid(&lt).caption(2), "Air temperature (K), 500 hPa, August 2023");
}

#[test]
fn packed_noleap_file_shows_its_assumptions() {
    let l = import("r8_packed_noleap.nc");
    let g = grid(&l);
    assert_eq!(l.profile.format, "NetCDF-4");
    assert_eq!(
        g.lines,
        [
            "Time: noleap calendar (no leap days; every year has 365 days).",
            "Values unpacked with the file's scale_factor (0.01) and add_offset (20)."
        ]
    );
    // Daily steps; the noleap calendar has no 29 February.
    assert_eq!(g.date_label(2), "1 Mar 2004");
    // No scale or offset is left for the actions to apply: the cells are stored unpacked.
    assert_eq!((l.profile.bands[0].scale, l.profile.bands[0].offset), (None, None));
}

#[test]
fn longitudes_past_180_are_shown_as_minus_180_to_180() {
    let l = import("r8_lon360.nc");
    let g = grid(&l);
    assert_eq!(g.lines[0], "Longitudes 0–360 shown as −180–180.");
    assert_eq!(g.lines[1], "Time: 360-day calendar (twelve 30-day months).");
    assert_eq!(g.date_label(3), "2003");
    // 180° is drawn as −180°: the first column's cell straddles the antimeridian.
    let LayerData::Raster(r) = &l.data else { panic!() };
    assert_eq!((r.transform.origin_x, r.width), (-191.25, 16));
    let e = l.profile.extent_wgs84.unwrap();
    assert_eq!((e.min_x, e.max_x), (-180.0, 168.75));
}

/// A click in a world copy (the zoomed-out map repeats the world) reads the cell drawn there,
/// and the column straddling 180° answers on both sides of it.
#[test]
fn values_are_read_in_every_world_copy_and_across_180() {
    let (_, r) = shown("r8_lon360.nc", "pr", 0, 0);
    let at = |lon: f64| crate::inspect::raster_value_at(&r, 0, lon, 10.0).unwrap_or_else(|| panic!("{lon} is outside"));
    for lon in [-150.0, -30.0, 0.0, 90.0, 160.0] {
        for k in [-2.0, -1.0, 1.0, 2.0] {
            assert_eq!(at(lon + k * 360.0), at(lon), "{lon} + {k} worlds");
        }
    }
    assert_eq!(at(175.0), at(-175.0), "the first column straddles 180°");
    assert_eq!(at(535.0), at(-175.0));
    assert_eq!(crate::inspect::wrap_lon(190.0), -170.0);
    assert_eq!(crate::inspect::wrap_lon(-540.0), -180.0);
}

#[test]
fn curvilinear_grids_are_refused_plainly() {
    let e = import_path(&r8("r8_curvilinear.nc"), &Ctx::detached(Ticket::default())).err().unwrap();
    match e {
        ImportError::Refused(m) => assert!(m.contains("curvilinear grid") && m.contains("not been changed"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn slices_are_cached_and_read_lazily() {
    let l = import("r8_packed_noleap.nc");
    let g = grid(&l).clone();
    // Every step was read for the colour range (4 ≤ 12), so all are cached now.
    for t in 0..4 {
        assert!(cached_slice(&l.profile.source, &g, t).is_some(), "step {t}");
    }
    let mut other = g.clone();
    other.variable = "nope".into();
    assert!(cached_slice(&l.profile.source, &other, 0).is_none());
    assert!(slice(&l.profile.source, &other, 0).is_err());
}

#[test]
fn every_cf_calendar_decodes() {
    let d = |v: &[f64], u: &str, c: &str| decode_times(v, u, c).iter().map(|d| d.iso()).collect::<Vec<_>>();
    // Standard (mixed Julian/Gregorian): 1582-10-04 is followed by 1582-10-15.
    assert_eq!(d(&[0.0, 1.0], "days since 1582-10-04", "standard"), ["1582-10-04", "1582-10-15"]);
    assert_eq!(d(&[0.0, 1.0], "days since 1582-10-04", "gregorian"), ["1582-10-04", "1582-10-15"]);
    assert_eq!(d(&[0.0, 1.0], "days since 1582-10-04", "proleptic_gregorian"), ["1582-10-04", "1582-10-05"]);
    // Leap days: 2004 has 29 February except in noleap / 365_day; all_leap has it in 2001.
    assert_eq!(d(&[59.0], "days since 2004-01-01", "standard"), ["2004-02-29"]);
    assert_eq!(d(&[59.0], "days since 2004-01-01", "noleap"), ["2004-03-01"]);
    assert_eq!(d(&[59.0], "days since 2004-01-01", "365_day"), ["2004-03-01"]);
    assert_eq!(d(&[59.0], "days since 2001-01-01", "all_leap"), ["2001-02-29"]);
    assert_eq!(d(&[59.0], "days since 2001-01-01", "366_day"), ["2001-02-29"]);
    // 360_day: 30-day months, a 30 February.
    assert_eq!(d(&[59.0, 360.0], "days since 2001-01-01", "360_day"), ["2001-02-30", "2002-01-01"]);
    // Julian: 1900 is a leap year.
    assert_eq!(d(&[59.0], "days since 1900-01-01", "julian"), ["1900-02-29"]);
    // Hours and seconds since, with a time of day.
    assert_eq!(d(&[1_806_270.0], "hours since 1800-01-01 00:00:0.0", "standard"), ["2006-01-22 06:00"]);
    assert_eq!(d(&[86_400.0 * 365.0], "seconds since 1970-01-01", "standard"), ["1971-01-01"]);
    // An unspecified calendar is the standard one.
    assert_eq!(d(&[31.0], "days since 2023-01-01", ""), ["2023-02-01"]);
    // Whole months and years since an epoch (counted as calendar months).
    assert_eq!(d(&[0.0, 1.0, 13.0], "months since 1960-01-01", "standard"), ["1960-01-01", "1960-02-01", "1961-02-01"]);
    assert_eq!(d(&[2.0], "years since 2000-07-01", "noleap"), ["2002-07-01"]);
    // Unreadable units give no dates (the steps are then numbered).
    assert!(decode_times(&[1.0], "fortnights since 2000-01-01", "standard").is_empty());
    assert!(decode_times(&[1.5], "months since 2000-01-01", "standard").is_empty());
    assert!(decode_times(&[1.0], "days", "standard").is_empty());
    assert!(decode_times(&[1.0], "DAYS SİNCE 2000-01-01", "standard").is_empty());
    assert!(decode_times(&[f64::NAN], "days since 2000-01-01", "standard").is_empty());
}

#[test]
fn axes_wrap_pad_and_resample() {
    // A global 0–360 grid rolls into −180–180.
    let (w, past) = wrap_longitudes(&[0.0, 90.0, 180.0, 270.0]);
    assert!(past || w == [0.0, 90.0, 180.0, 270.0]);
    let (w, past) = wrap_longitudes(&[0.0, 90.0, 190.0, 270.0]);
    assert!(past);
    assert_eq!(w, [0.0, 90.0, -170.0, -90.0]);
    let a = build_axis(&w, false).unwrap();
    assert_eq!(a.start, -170.0);
    // A regional grid across the antimeridian is padded with empty columns.
    let (w, _) = wrap_longitudes(&[170.0, 180.0, 190.0]);
    let a = build_axis(&w, false).unwrap();
    assert_eq!(a.map.first(), Some(&Some(1)));
    assert_eq!(a.map.last(), Some(&Some(0)));
    assert_eq!(a.map.len(), 36);
    // Rows run north to south; slightly uneven (Gaussian) latitudes go to the nearest row.
    let a = build_axis(&[-60.2, -20.0, 20.0, 60.2], true).unwrap();
    assert_eq!(a.map, [Some(3), Some(2), Some(1), Some(0)]);
    assert!(a.off > 0.1 && a.off < 0.5, "{}", a.off);
    // Too uneven, or with gaps in its values: refused.
    assert!(build_axis(&[0.0, 1.0, 1.4, 2.0], false).is_err());
    // A missing cell in an even axis is a gap, not a refusal.
    assert_eq!(build_axis(&[0.0, 1.0, 3.0], false).unwrap().map, [Some(0), Some(1), None, Some(2)]);
    assert!(build_axis(&[0.0, f64::NAN], false).is_err());
    assert!(build_axis(&[], false).is_err());
}

/// The spike (docs/lm/netcdf-spike.md): opens every file in `LM_NETCDF_SPIKE_DIR`, reads the
/// first and last steps and reports the times. `cargo test -p lm-jobs --release --lib netcdf_spike -- --ignored --nocapture`.
#[test]
#[ignore = "needs LM_NETCDF_SPIKE_DIR with real NetCDF files"]
fn netcdf_spike() {
    use std::time::Instant;
    let dir = std::env::var("LM_NETCDF_SPIKE_DIR").expect("LM_NETCDF_SPIKE_DIR");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "nc") && !p.file_name().unwrap().to_string_lossy().starts_with('.'))
        .collect();
    files.sort();
    for f in files {
        let size = std::fs::metadata(&f).unwrap().len();
        let t0 = Instant::now();
        let r = import_path(&f, &Ctx::detached(Ticket::default()));
        let open_s = t0.elapsed().as_secs_f64();
        match r {
            Err(e) => {
                println!("SPIKE {} ({:.1} MB): refused/failed in {open_s:.2} s: {e} [{}]", f.display(), size as f64 / 1e6, e.details())
            }
            Ok(mut layers) => {
                let l = layers.remove(0);
                let g = grid(&l).clone();
                let last = g.steps() - 1;
                let t1 = Instant::now();
                let s = slice(&l.profile.source, &g, last.saturating_sub(1)).unwrap();
                let step_s = t1.elapsed().as_secs_f64();
                let t2 = Instant::now();
                let mut n = 0;
                for v in &g.variables {
                    if v.name != g.variable {
                        let _ = choose_variable(&l, &v.name, 0, 0).map(|_| n += 1);
                    }
                }
                let vars_s = t2.elapsed().as_secs_f64();
                // Step 700 (when there is one), to compare with xarray's mean of the same step.
                if g.steps() > 700 {
                    let s700 = slice(&l.profile.source, &g, 700).unwrap();
                    println!(
                        "SPIKE step 700 ({}): mean {:.6} of {} cells",
                        g.date_label(700),
                        s700.bands[0].stats.mean,
                        s700.bands[0].stats.count
                    );
                }
                println!(
                    "SPIKE {} ({:.1} MB, {}): {} vars, shown {} {}x{}, {} steps {} → {}; open+profile {open_s:.2} s ({} steps sampled); one uncached step {step_s:.3} s; {n} other variables {vars_s:.2} s; lines {:?}; range {:?}; band {}..{}",
                    f.file_name().unwrap().to_string_lossy(),
                    size as f64 / 1e6,
                    l.profile.format,
                    g.variables.len(),
                    g.variable,
                    s.width,
                    s.height,
                    g.steps(),
                    g.date_label(0),
                    g.date_label(last),
                    g.sampled,
                    g.lines,
                    g.range,
                    l.profile.bands[0].min.unwrap_or(f64::NAN),
                    l.profile.bands[0].max.unwrap_or(f64::NAN),
                );
            }
        }
    }
}

/// The owner's heatwave data (JRC Global Drought Observatory, Heat-wave Intensity 2023, CC BY 4.0;
/// `/Volumes/LaCie/data/layeredmap-netcdf-test/jrc_warm_spell/`, not in the repository): NetCDF-4
/// with zlib 9 + shuffle in (92, 1, 180, 360) chunks, a `band` dimension of one, `spatial_ref`,
/// unitless `lat`/`lon` and the title only in the global attributes. Runs when the drive is
/// mounted: `cargo test -p lm-jobs --release --lib jrc_heatwave -- --ignored --nocapture`.
#[test]
#[ignore = "needs the JRC heatwave file on the LaCie drive"]
fn jrc_heatwave_opens_fast_and_plays() {
    use std::time::Instant;
    let path = PathBuf::from("/Volumes/LaCie/data/layeredmap-netcdf-test/jrc_warm_spell/hwi2a_m_gdo_20230101_20231231_d.nc");
    let t0 = Instant::now();
    let l = import_path(&path, &Ctx::detached(Ticket::default())).unwrap().remove(0);
    let open_s = t0.elapsed().as_secs_f64();
    let g = grid(&l).clone();
    let names: Vec<&str> = g.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["hwi2a"], "spatial_ref and band are not variables");
    assert_eq!(g.shown().unwrap().label(), "Heat-wave Intensity (°C)");
    assert!(g.shown().unwrap().levels.is_none(), "the band dimension of one is squeezed");
    assert_eq!((g.steps(), g.date_label(0), g.date_label(364)), (365, "1 Jan 2023".to_string(), "31 Dec 2023".to_string()));
    let LayerData::Raster(r) = &l.data else { panic!() };
    assert_eq!((r.width, r.height), (1440, 720));
    let e = l.profile.extent_wgs84.unwrap();
    assert_eq!((e.min_x, e.max_x, e.min_y, e.max_y), (-180.0, 180.0, -90.0, 90.0));
    // Stepping through the summer, one day at a time, as Play does.
    let t1 = Instant::now();
    let mut valid = 0;
    for day in 180..200 {
        valid += slice(&l.profile.source, &g, day).unwrap().bands[0].stats.count;
    }
    let step_s = t1.elapsed().as_secs_f64() / 20.0;
    // 10 July 2023 against xarray: mean 0.9472581453239732 over 1,036,800 cells.
    assert_eq!(g.date_label(190), "10 Jul 2023");
    let s = slice(&l.profile.source, &g, 190).unwrap();
    assert_eq!(s.bands[0].stats.count, 1_036_800);
    assert!((s.bands[0].stats.mean - 0.947_258_145_3).abs() < 1e-6, "{}", s.bands[0].stats.mean);
    println!(
        "JRC heatwave: open + profile {open_s:.2} s, {step_s:.3} s per new day, {valid} valid cells over 20 days, range {:?}, lines {:?}",
        g.range, g.lines
    );
    assert!(open_s < 10.0 && step_s < 0.5, "too slow: {open_s} s, {step_s} s per day");
}
