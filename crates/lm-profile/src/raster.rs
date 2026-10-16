//! Raster catalog rows: NoData, unit, kelvin, Landsat scale, cell meaning.

use crate::{lines, make, tokens};
use lm_core::model::{Assumption, AssumptionKind, BandProfile, CellMeaning, Choice, Confidence, Consequence, Known, Subject, Treatment};
use std::collections::HashSet;

/// Landsat Collection 2 Level-2 surface temperature (ST_B10): kelvin = DN × scale + offset.
pub const LANDSAT_C2_ST_SCALE: f64 = 0.00341802;
pub const LANDSAT_C2_ST_OFFSET: f64 = 149.0;

/// Share of the candidate NoData cells that must connect to the raster's edge.
const EDGE_SHARE: f64 = 0.9;
/// At most this many distinct whole numbers make a raster of categories.
const MAX_CATEGORIES: usize = 32;

/// One band, as the import coordinator sampled it.
#[derive(Clone, Copy, Debug)]
pub struct BandSample<'a> {
    /// 1-based band number.
    pub index: u32,
    /// The band's description, or "" (GeoTIFF band descriptions, "ST_B10").
    pub name: &'a str,
    /// The file name ("LC09_…_ST_B10.TIF"); searched for name hints with `name`.
    pub file_name: &'a str,
    /// Size of `values` (a downsampled grid: an overview, or every n-th row and column, that
    /// keeps the outermost rows and columns).
    pub width: usize,
    pub height: usize,
    /// The stored values, row-major, before scale and offset.
    pub values: &'a [f32],
    /// NoData declared in the file.
    pub declared_nodata: Option<f64>,
    /// Scale and offset declared in the file.
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    /// Unit declared in the file.
    pub unit: Option<&'a str>,
    /// The stored type is an integer type (UInt16, Int16…).
    pub integer_type: bool,
}

/// "−9999" with a true minus sign; the lowest 32-bit float as "−3.4e38".
pub(crate) fn nodata_label(v: f64) -> String {
    let s = if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else if v.abs() >= 1e15 {
        format!("{v:.1e}")
    } else {
        format!("{v}")
    };
    s.replacen('-', "\u{2212}", 1)
}

fn names(name: &str, file_name: &str) -> String {
    // The file name without its extension.
    let stem = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = stem.rsplit_once('.').map(|(s, _)| s).unwrap_or(stem);
    format!("{name} {stem}").to_lowercase()
}

fn is_st_b10(name: &str, file_name: &str) -> bool {
    let n = names(name, file_name);
    n.contains("st_b10")
        || n.contains("st-b10")
        || tokens(&n).windows(2).any(|w| w.first().is_some_and(|a| a == "st") && w.get(1).is_some_and(|b| b == "b10"))
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted.get(i.min(sorted.len() - 1)).copied()
}

/// NoData for a band (catalog: "Raster without a NoData value").
///
/// A declared value is `Certain`. Without one, −9999 or the lowest 32-bit float is taken as
/// NoData when it is a spike: it is below every other value with a clear gap, and at least 90%
/// of its cells connect (4-neighbour) to the edge of the grid, the way an image collar does.
/// Then a visible [`AssumptionKind::RasterNoData`] ("Treated −9999 as No data.").
pub fn raster_nodata(
    values: &[f32],
    width: usize,
    height: usize,
    declared: Option<f64>,
    band: u32,
) -> (Known<Option<f64>>, Option<Assumption>) {
    if let Some(v) = declared.filter(|v| !v.is_nan()) {
        return (Known::certain(Some(v), "Declared in the file."), None);
    }
    let n = width.saturating_mul(height).min(values.len());
    let grid = values.get(..n).unwrap_or(&[]);
    for candidate in [-9999.0f32, f32::MIN] {
        let count = grid.iter().filter(|v| **v == candidate).count();
        if count == 0 {
            continue;
        }
        let others: Vec<f32> = grid.iter().copied().filter(|v| *v != candidate && v.is_finite()).collect();
        let lo = others.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = others.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if !others.is_empty() {
            let gap = f64::from(lo) - f64::from(candidate);
            let range = f64::from(hi) - f64::from(lo);
            if gap <= 0.0 || gap < 100.0f64.max(0.1 * range).min(1000.0) {
                continue;
            }
        }
        // Flood fill from the edge through candidate cells.
        let mut seen = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        for (i, (s, v)) in seen.iter_mut().zip(grid).enumerate() {
            let (r, c) = (i / width.max(1), i % width.max(1));
            if (r == 0 || c == 0 || r + 1 == height || c + 1 == width) && *v == candidate {
                *s = true;
                stack.push(i);
            }
        }
        let mut connected = 0usize;
        while let Some(i) = stack.pop() {
            connected += 1;
            let (r, c) = (i / width, i % width);
            let mut next = |j: usize| {
                if grid.get(j) == Some(&candidate)
                    && let Some(s) = seen.get_mut(j)
                    && !*s
                {
                    *s = true;
                    stack.push(j);
                }
            };
            if r > 0 {
                next(i - width);
            }
            if r + 1 < height {
                next(i + width);
            }
            if c > 0 {
                next(i - 1);
            }
            if c + 1 < width {
                next(i + 1);
            }
        }
        if (connected as f64) < EDGE_SHARE * count as f64 {
            continue;
        }
        let v = f64::from(candidate);
        let label = nodata_label(v);
        let evidence = format!("No NoData value in the file; {label} fills {count} of {n} sampled cells, along the edges.");
        let a = make(
            AssumptionKind::RasterNoData,
            Subject::Band(band),
            Choice::new(format!("nodata:{v}"), format!("{label} means No data")),
            vec![
                Choice::new(format!("nodata:{v}"), format!("{label} means No data")),
                Choice::new("value", format!("{label} is a real value")),
            ],
            Confidence::Likely,
            Consequence::ChangesNumbers,
            Treatment::Visible,
            lines::raster_nodata(v),
        );
        return (Known::likely(Some(v), evidence), Some(a));
    }
    (Known::possible(None, "No NoData value in the file, and no edge spike of −9999 or the lowest value."), None)
}

/// The unit of a band (catalog: "Raster with no unit"): declared → `Certain`; none → a visible
/// [`AssumptionKind::RasterNoUnit`] shown in the legend ("Unit unknown." + "Add unit").
pub fn raster_unit(unit: Option<&str>, band: u32) -> (Known<Option<String>>, Option<Assumption>) {
    match unit.map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => (Known::certain(Some(u.to_string()), "Declared in the file."), None),
        None => {
            let a = make(
                AssumptionKind::RasterNoUnit,
                Subject::Band(band),
                Choice::new("unknown", "Unknown"),
                vec![Choice::new("unknown", "Unknown"), Choice::new("add_unit", "Add unit…")],
                Confidence::Unknown,
                Consequence::DisplayOnly,
                Treatment::Visible,
                lines::RASTER_NO_UNIT,
            );
            (Known::unknown(None, "No unit in the file."), Some(a))
        }
    }
}

/// Landsat Collection 2 surface temperature stored as raw integers (catalog: "Landsat
/// Collection 2 ST_B10 …"): the name has ST_B10, the type is an integer type, no scale or
/// offset is declared, and the valid values (fill value 0 left out) lie between 10,000 and
/// 65,535. Returns the scale and offset to apply (0.00341802, 149.0) and a visible
/// [`AssumptionKind::LandsatScale`].
pub fn landsat_scale(
    name: &str,
    file_name: &str,
    valid_raw: &[f64],
    scale: Option<f64>,
    offset: Option<f64>,
    integer_type: bool,
    band: u32,
) -> Option<(f64, f64, Assumption)> {
    let declared = scale.is_some_and(|s| s != 1.0) || offset.is_some_and(|o| o != 0.0);
    if declared || !integer_type || !is_st_b10(name, file_name) {
        return None;
    }
    let mut v: Vec<f64> = valid_raw.iter().copied().filter(|v| *v != 0.0 && v.is_finite()).collect();
    v.sort_by(f64::total_cmp);
    let (lo, hi) = (percentile(&v, 0.01)?, v.last().copied()?);
    if lo < 10_000.0 || hi > 65_535.0 {
        return None;
    }
    let a = make(
        AssumptionKind::LandsatScale,
        Subject::Band(band),
        Choice::new("landsat_c2_st", "Landsat Collection 2 scale and offset"),
        vec![Choice::new("landsat_c2_st", "Landsat Collection 2 scale and offset"), Choice::new("raw", "The stored numbers as they are")],
        Confidence::Likely,
        Consequence::ChangesNumbers,
        Treatment::Visible,
        lines::LANDSAT_SCALE,
    );
    Some((LANDSAT_C2_ST_SCALE, LANDSAT_C2_ST_OFFSET, a))
}

/// Surface temperature in kelvin (catalog: "Raster named lst, temp or ST_B10, with values
/// between 250 and 340"): a name token lst / temp… / ST_B10 and decoded valid values whose
/// 1st and 99th percentiles lie in 250–340. A visible [`AssumptionKind::RasterKelvin`].
pub fn raster_kelvin(name: &str, file_name: &str, valid_decoded: &[f64], band: u32) -> Option<Assumption> {
    let n = names(name, file_name);
    let named = is_st_b10(name, file_name) || tokens(&n).iter().any(|t| t == "lst" || t.starts_with("temp"));
    if !named {
        return None;
    }
    let mut v: Vec<f64> = valid_decoded.iter().copied().filter(|v| v.is_finite()).collect();
    v.sort_by(f64::total_cmp);
    let (lo, hi) = (percentile(&v, 0.01)?, percentile(&v, 0.99)?);
    if lo < 250.0 || hi > 340.0 {
        return None;
    }
    Some(make(
        AssumptionKind::RasterKelvin,
        Subject::Band(band),
        Choice::new("kelvin", "Kelvin, shown in °C"),
        vec![Choice::new("kelvin", "Kelvin, shown in °C"), Choice::new("as_stored", "The values as stored")],
        Confidence::Likely,
        Consequence::ChangesNumbers,
        Treatment::Visible,
        lines::RASTER_KELVIN,
    ))
}

/// What a band's cells mean (catalog: "Raster cell meaning"), from the decoded valid values:
///
/// - decimals or negative values → measured (`Likely`, visible);
/// - whole numbers and a name containing pop or count → totals (`Likely`, visible);
/// - at most 32 distinct whole numbers → categories (`Likely`, visible);
/// - otherwise → measured, the safe reading, with `Possible` confidence and
///   [`Treatment::Silent`]: a deferred question for the first summary
///   ([`crate::is_deferred_question`]).
///
/// Every choice list ends with Not sure (summaries show "average cell value" only).
pub fn raster_cell_meaning(name: &str, file_name: &str, valid_decoded: &[f64], band: u32) -> (Known<CellMeaning>, Option<Assumption>) {
    let v: Vec<f64> = valid_decoded.iter().copied().filter(|v| v.is_finite()).collect();
    if v.is_empty() {
        return (Known::unknown(CellMeaning::Unknown, "No valid cells."), None);
    }
    let measured = Choice::new("measured", "Measured values (averaged)");
    let total = Choice::new("total", "Totals per cell (summed)");
    let category = Choice::new("category", "Categories (shares)");
    let all = vec![measured.clone(), total.clone(), category.clone(), Choice::not_sure()];
    let mk = |chosen: Choice, confidence, treatment, line: &str| {
        make(
            AssumptionKind::RasterCellMeaning,
            Subject::Band(band),
            chosen,
            all.clone(),
            confidence,
            Consequence::ChangesNumbers,
            treatment,
            line,
        )
    };
    let integers = v.iter().all(|x| x.fract() == 0.0);
    let negative = v.iter().any(|x| *x < 0.0);
    let n = names(name, file_name);
    if !integers || negative {
        let why = if negative { "Some values are negative." } else { "Values have decimals." };
        return (
            Known::likely(CellMeaning::Measured, why),
            Some(mk(measured, Confidence::Likely, Treatment::Visible, lines::CELL_MEASURED)),
        );
    }
    if n.contains("pop") || n.contains("count") || n.contains("人口") {
        return (
            Known::likely(CellMeaning::Total, "Whole numbers, and the name says population or count."),
            Some(mk(total, Confidence::Likely, Treatment::Visible, lines::CELL_TOTAL)),
        );
    }
    let distinct: HashSet<i64> = v.iter().map(|x| *x as i64).take(100_000).collect();
    if distinct.len() <= MAX_CATEGORIES {
        let why = format!("{} distinct whole numbers.", distinct.len());
        return (
            Known::likely(CellMeaning::Category, why),
            Some(mk(category, Confidence::Likely, Treatment::Visible, lines::CELL_CATEGORY)),
        );
    }
    (
        Known::possible(CellMeaning::Measured, "Whole numbers with many values; the meaning is unconfirmed, so treated as measured."),
        Some(mk(measured, Confidence::Possible, Treatment::Silent, lines::CELL_MEASURED)),
    )
}

/// Runs every raster row for one band and returns its profile and assumptions in catalog
/// order (NoData, unit, kelvin, Landsat scale, cell meaning). A kelvin band gets the unit "K"
/// (`Likely`) instead of "Unit unknown".
pub fn profile_band(s: &BandSample) -> (BandProfile, Vec<Assumption>) {
    let mut out = Vec::new();
    let (nodata, a) = raster_nodata(s.values, s.width, s.height, s.declared_nodata, s.index);
    out.extend(a);
    let nd = nodata.value;
    let valid_raw: Vec<f64> = s.values.iter().map(|v| f64::from(*v)).filter(|v| v.is_finite() && Some(*v) != nd).collect();
    let landsat = landsat_scale(s.name, s.file_name, &valid_raw, s.scale, s.offset, s.integer_type, s.index);
    let (scale, offset) = match &landsat {
        Some((sc, off, _)) => (Some(*sc), Some(*off)),
        None => (s.scale, s.offset),
    };
    let decoded: Vec<f64> = valid_raw.iter().map(|v| v * scale.unwrap_or(1.0) + offset.unwrap_or(0.0)).collect();
    let kelvin = raster_kelvin(s.name, s.file_name, &decoded, s.index);
    let unit = if kelvin.is_some() && s.unit.is_none_or(|u| u.trim().is_empty()) {
        Known::likely(Some("K".to_string()), "Surface temperature values in the kelvin range.")
    } else {
        let (unit, a) = raster_unit(s.unit, s.index);
        out.extend(a);
        unit
    };
    out.extend(kelvin);
    if let Some((_, _, a)) = landsat {
        out.push(a);
    }
    let (cell_meaning, a) = raster_cell_meaning(s.name, s.file_name, &decoded, s.index);
    out.extend(a);
    let min = decoded.iter().copied().reduce(f64::min);
    let max = decoded.iter().copied().reduce(f64::max);
    let name = if s.name.is_empty() { format!("Band {}", s.index) } else { s.name.to_string() };
    (BandProfile { index: s.index, name, nodata, scale, offset, unit, cell_meaning, min, max }, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A w×h grid of `inner` values with a `border`-cell frame of `edge`.
    fn framed(w: usize, h: usize, border: usize, edge: f32, inner: impl Fn(usize, usize) -> f32) -> Vec<f32> {
        (0..w * h)
            .map(|i| {
                let (r, c) = (i / w, i % w);
                if r < border || c < border || r + border >= h || c + border >= w { edge } else { inner(r, c) }
            })
            .collect()
    }

    #[test]
    fn raster_nodata_matches_edge_spike() {
        let g = framed(24, 24, 2, -9999.0, |r, c| (r * 24 + c) as f32);
        let (nd, a) = raster_nodata(&g, 24, 24, None, 1);
        assert_eq!(nd.value, Some(-9999.0));
        let a = a.unwrap();
        assert_eq!(a.kind, AssumptionKind::RasterNoData);
        assert_eq!(a.treatment, Treatment::Visible);
        assert_eq!(a.line, "Treated \u{2212}9999 as No data.");
        // The lowest 32-bit float as a collar.
        let g = framed(10, 10, 1, f32::MIN, |_, _| 3.5);
        let (nd, a) = raster_nodata(&g, 10, 10, None, 1);
        assert_eq!(nd.value, Some(f64::from(f32::MIN)));
        assert_eq!(a.unwrap().line, "Treated \u{2212}3.4e38 as No data.");
    }

    #[test]
    fn raster_nodata_does_not_match_declared_interior_or_no_gap() {
        let g = framed(24, 24, 2, -9999.0, |r, c| (r * 24 + c) as f32);
        let (nd, a) = raster_nodata(&g, 24, 24, Some(0.0), 1);
        assert_eq!((nd.value, nd.confidence, a), (Some(0.0), Confidence::Certain, None));
        // −9999 in a block in the middle only.
        let g: Vec<f32> = (0..100).map(|i| if (44..47).contains(&i) { -9999.0 } else { i as f32 }).collect();
        assert_eq!(raster_nodata(&g, 10, 10, None, 1).1, None);
        // Values that run down to −9999 (no gap): a real value.
        let g = framed(10, 10, 1, -9999.0, |r, c| -9990.0 + (r + c) as f32);
        assert_eq!(raster_nodata(&g, 10, 10, None, 1).1, None);
        // No candidate at all.
        assert_eq!(raster_nodata(&[1.0, 2.0, 3.0, 4.0], 2, 2, None, 1).1, None);
    }

    #[test]
    fn raster_no_unit_matches_missing_unit() {
        let (u, a) = raster_unit(None, 2);
        assert_eq!(u.value, None);
        let a = a.unwrap();
        assert_eq!((a.kind, a.treatment, a.subject.clone()), (AssumptionKind::RasterNoUnit, Treatment::Visible, Subject::Band(2)));
        assert_eq!(a.line, "Unit unknown.");
        assert!(raster_unit(Some("  "), 1).1.is_some());
    }

    #[test]
    fn raster_no_unit_does_not_match_declared_unit() {
        let (u, a) = raster_unit(Some("m"), 1);
        assert_eq!((u.value.as_deref(), a), (Some("m"), None));
    }

    #[test]
    fn raster_kelvin_matches_named_temperature_range() {
        let v: Vec<f64> = (0..100).map(|i| 280.0 + i as f64 * 0.3).collect();
        for (name, file) in [("lst", "x.tif"), ("", "MOD11_LST_Day.tif"), ("Temperature", "a.tif"), ("ST_B10", "b.tif")] {
            let a = raster_kelvin(name, file, &v, 1).unwrap();
            assert_eq!(a.kind, AssumptionKind::RasterKelvin);
            assert_eq!(a.line, "Treated as surface temperature in kelvin; shown in °C.");
        }
    }

    #[test]
    fn raster_kelvin_does_not_match_wrong_name_or_range() {
        let k: Vec<f64> = (0..100).map(|i| 280.0 + i as f64 * 0.3).collect();
        assert!(raster_kelvin("elevation", "dem.tif", &k, 1).is_none());
        let c: Vec<f64> = (0..100).map(|i| 10.0 + i as f64 * 0.3).collect();
        assert!(raster_kelvin("lst", "lst.tif", &c, 1).is_none());
        // "template" is not temperature… but starts with "temp": we accept that false hint, so
        // the range has to rule it out.
        assert!(raster_kelvin("template", "t.tif", &c, 1).is_none());
    }

    #[test]
    fn landsat_scale_matches_raw_st_b10() {
        let v: Vec<f64> = (0..200).map(|i| if i < 10 { 0.0 } else { 40_000.0 + i as f64 * 20.0 }).collect();
        let (s, o, a) = landsat_scale("", "LC09_L2SP_118038_20240801_20240802_02_T1_ST_B10.TIF", &v, None, None, true, 1).unwrap();
        assert_eq!((s, o), (0.00341802, 149.0));
        assert_eq!(a.kind, AssumptionKind::LandsatScale);
        assert_eq!(a.line, "Applied the Landsat Collection 2 surface-temperature scale and offset.");
        // Decoded, it is kelvin.
        let decoded: Vec<f64> = v.iter().filter(|x| **x != 0.0).map(|x| x * s + o).collect();
        assert!(raster_kelvin("", "LC09_ST_B10.TIF", &decoded, 1).is_some());
    }

    #[test]
    fn landsat_scale_does_not_match_scaled_or_other_bands() {
        let v: Vec<f64> = (0..200).map(|i| 40_000.0 + i as f64 * 20.0).collect();
        assert!(landsat_scale("", "LC09_ST_B10.TIF", &v, Some(0.00341802), Some(149.0), true, 1).is_none());
        assert!(landsat_scale("", "LC09_SR_B4.TIF", &v, None, None, true, 1).is_none());
        assert!(landsat_scale("", "LC09_ST_B10.TIF", &v, None, None, false, 1).is_none());
        let small: Vec<f64> = (0..200).map(|i| 290.0 + i as f64 * 0.1).collect();
        assert!(landsat_scale("", "LC09_ST_B10.TIF", &small, None, None, true, 1).is_none());
    }

    #[test]
    fn raster_cell_meaning_matches_each_reading() {
        let (m, a) = raster_cell_meaning("ndvi", "x.tif", &[0.1, 0.5, -0.2], 1);
        assert_eq!(m.value, CellMeaning::Measured);
        let a = a.unwrap();
        assert_eq!((a.kind, a.treatment), (AssumptionKind::RasterCellMeaning, Treatment::Visible));
        assert_eq!(a.line, "Treated as measured values (averaged, not summed).");
        assert_eq!(a.alternatives.last(), Some(&Choice::not_sure()));
        let many: Vec<f64> = (0..500).map(|i| (i * 7) as f64).collect();
        let (m, a) = raster_cell_meaning("", "worldpop_2020.tif", &many, 1);
        assert_eq!((m.value, a.map(|a| a.treatment)), (CellMeaning::Total, Some(Treatment::Visible)));
        let classes: Vec<f64> = (0..500).map(|i| ((i % 9 + 1) * 10) as f64).collect();
        let (m, _) = raster_cell_meaning("", "landcover.tif", &classes, 1);
        assert_eq!(m.value, CellMeaning::Category);
    }

    #[test]
    fn raster_cell_meaning_does_not_claim_without_evidence() {
        // Many whole numbers and no name hint: not likely, so the question waits for the first
        // summary, with the safe reading applied.
        let many: Vec<f64> = (0..500).map(|i| (i * 7) as f64).collect();
        let (m, a) = raster_cell_meaning("band1", "dem.tif", &many, 1);
        assert_eq!((m.value, m.confidence), (CellMeaning::Measured, Confidence::Possible));
        let a = a.unwrap();
        assert_eq!(a.treatment, Treatment::Silent);
        assert!(crate::is_deferred_question(&a));
        // Decimals named "pop" are still measured (a density, not a total).
        let (m, _) = raster_cell_meaning("pop_density", "x.tif", &[1.5, 2.5], 1);
        assert_eq!(m.value, CellMeaning::Measured);
    }

    #[test]
    fn profile_band_runs_all_rows_for_landsat() {
        let w = 20;
        let raw = framed(w, w, 1, 0.0, |r, c| (42_000 + r * 100 + c * 10) as f32);
        let s = BandSample {
            index: 1,
            name: "",
            file_name: "LC09_L2SP_ST_B10.TIF",
            width: w,
            height: w,
            values: &raw,
            declared_nodata: Some(0.0),
            scale: None,
            offset: None,
            unit: None,
            integer_type: true,
        };
        let (b, a) = profile_band(&s);
        let kinds: Vec<AssumptionKind> = a.iter().map(|a| a.kind).collect();
        assert_eq!(kinds, vec![AssumptionKind::RasterKelvin, AssumptionKind::LandsatScale, AssumptionKind::RasterCellMeaning]);
        assert_eq!(b.unit.value.as_deref(), Some("K"));
        assert_eq!(b.scale, Some(LANDSAT_C2_ST_SCALE));
        assert_eq!(b.cell_meaning.value, CellMeaning::Measured);
        assert!(b.min.is_some_and(|m| m > 290.0));
    }
}
