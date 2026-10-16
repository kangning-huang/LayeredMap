//! Import-time profiling: field roles, coordinate-system confidence, NoData, units, China offset
//! heuristics, and the assumptions they generate (docs/lm/spec.md › Assumptions and questions ›
//! Assumption catalog).
//!
//! Everything here is a **pure function** over data the caller has already read: no file or
//! network I/O, no threads, no clocks. The import coordinator (`lm-jobs`) reads a sample of the
//! file, calls these functions, and puts the results into an [`lm_core::model::LayerProfile`].
//!
//! # One function per catalog row
//!
//! | Catalog row | Function | [`AssumptionKind`] | Treatment |
//! |---|---|---|---|
//! | CSV with lon/lat columns | [`csv_locate`] → [`CsvLocation::Located`] | `CsvCoordinateColumns` | Visible |
//! | CSV with ambiguous coordinate columns | [`csv_locate`] → [`CsvLocation::Ambiguous`] | `CsvAmbiguousColumns` | Question |
//! | No CRS, values within ±180/±90 | [`crs_from_values`] | `NoCrsLonLat` | Visible |
//! | No CRS, projected-looking values | [`crs_from_values`], [`csv_locate`] → [`CsvLocation::Projected`] | `NoCrsProjected` | Question |
//! | Points inside mainland China | [`china_offset`] | `ChinaOffsetPossible` | Visible (Compare) |
//! | … plus a name hint | [`china_offset`] | `ChinaOffsetNamed` | Question |
//! | Raster without NoData | [`raster_nodata`] | `RasterNoData` | Visible |
//! | Raster with no unit | [`raster_unit`] | `RasterNoUnit` | Visible (legend) |
//! | Raster in kelvin | [`raster_kelvin`] | `RasterKelvin` | Visible |
//! | Landsat C2 ST_B10 raw integers | [`landsat_scale`] | `LandsatScale` | Visible |
//! | Raster cell meaning | [`raster_cell_meaning`] | `RasterCellMeaning` | Visible if likely, else deferred |
//! | Field looks like an identifier | [`profile_fields`] | `IdentifierField` | Silent |
//! | Numeric field with ≤ 12 values | [`profile_fields`] | `FewValuesCategorical` | Silent |
//! | Shapefile text, no .cpg | [`shapefile_encoding`] | `ShapefileGbk` | Silent |
//! | Excel workbook with several sheets | [`excel_sheet`] | `ExcelSheet` | Visible |
//! | GeoJSON with mixed geometry | [`mixed_geometry_split`] | `MixedGeometrySplit` | Silent |
//!
//! [`profile_band`] runs all raster rows for one band. Every [`Assumption::line`] is the
//! catalog's wording without the trailing link word ("Change", "Compare", "Add unit"); the
//! exact strings are in [`lines`].
//!
//! # Deferred questions
//!
//! "Questions about what values mean come only when an action first needs the answer." When the
//! cell meaning of a raster is not likely from the evidence, [`raster_cell_meaning`] applies the
//! safe interpretation (measured values) with [`Treatment::Silent`] and
//! [`Confidence::Possible`]; [`is_deferred_question`] tells `lm-actions` to ask at the first
//! summary. Such an assumption never shows as an open question at import.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod china;
mod china_boundary;
mod crs;
pub mod crs_candidates;
mod csv;
mod encoding;
mod fields;
mod layout;
mod place;
mod raster;

pub use china::{china_offset, in_mainland_china, mainland_china_share};
pub use crs::{crs_from_values, looks_like_lonlat};
pub use csv::{CoordinatePair, CsvLocation, csv_locate, parse_number};
pub use encoding::{TextEncoding, shapefile_encoding};
pub use fields::{CoordinateColumn, RawTable, profile_fields, profile_raw_table};
pub use layout::{SheetChoice, SheetSample, excel_sheet, geometry_kinds, mixed_geometry_split};
pub use place::{PlaceCandidate, place_candidates};
pub use raster::{
    BandSample, LANDSAT_C2_ST_OFFSET, LANDSAT_C2_ST_SCALE, landsat_scale, profile_band, raster_cell_meaning, raster_kelvin, raster_nodata,
    raster_unit,
};

use lm_core::model::{Assumption, AssumptionKind, Choice, Confidence, Consequence, Subject, Treatment};

/// The lines shown to the user, exactly as the Assumption catalog words them (without the
/// trailing "Change" / "Compare" / "Add unit" link).
pub mod lines {
    /// CSV with lon/lat columns: `Located using {lon} and {lat} as GPS coordinates (WGS 84).`
    pub fn csv_located(lon: &str, lat: &str) -> String {
        format!("Located using {lon} and {lat} as GPS coordinates (WGS 84).")
    }
    pub const CSV_AMBIGUOUS: &str = "Which columns locate these records?";
    pub const NO_CRS_LONLAT: &str = "Assumed GPS coordinates (WGS 84), because the file doesn't say.";
    pub const NO_CRS_PROJECTED: &str = "Where should this data be?";
    pub const CHINA_OFFSET_POSSIBLE: &str =
        "If these came from a Chinese map app (Amap, Tencent or Baidu), they may sit a few hundred meters off.";
    pub const CHINA_OFFSET_GCJ02: &str = "These look like Amap (GCJ-02) coordinates. Convert to GPS coordinates?";
    pub const CHINA_OFFSET_BD09: &str = "These look like Baidu (BD-09) coordinates. Convert to GPS coordinates?";
    /// Raster without NoData: `Treated −9999 as No data.` (with a true minus sign, U+2212).
    pub fn raster_nodata(value: f64) -> String {
        format!("Treated {} as No data.", super::raster::nodata_label(value))
    }
    pub const RASTER_NO_UNIT: &str = "Unit unknown.";
    pub const RASTER_KELVIN: &str = "Treated as surface temperature in kelvin; shown in °C.";
    pub const LANDSAT_SCALE: &str = "Applied the Landsat Collection 2 surface-temperature scale and offset.";
    pub const CELL_MEASURED: &str = "Treated as measured values (averaged, not summed).";
    /// Not in the catalog's wording (it gives only the measured line); same pattern.
    pub const CELL_TOTAL: &str = "Treated as totals per cell (summed, not averaged).";
    /// Not in the catalog's wording (it gives only the measured line); same pattern.
    pub const CELL_CATEGORY: &str = "Treated as categories (shares, not averages).";
    pub const IDENTIFIER: &str = "Treated as an ID.";
    pub const FEW_VALUES: &str = "Colored as categories.";
    pub const SHAPEFILE_GBK: &str = "Read text as Chinese (GBK).";
    /// `Opened sheet 'Schools'.`
    pub fn excel_sheet(name: &str) -> String {
        format!("Opened sheet '{name}'.")
    }
    /// `Split into 3 layers: points, lines, areas` (the catalog has no final period).
    pub fn mixed_split(kinds: &[&str]) -> String {
        format!("Split into {} layers: {}", kinds.len(), kinds.join(", "))
    }
}

/// True when `a` is a question to ask when an action first needs the answer rather than at
/// import (see the crate docs › Deferred questions): applied silently with only `Possible`
/// confidence, not yet confirmed, and offering Not sure.
pub fn is_deferred_question(a: &Assumption) -> bool {
    a.treatment == Treatment::Silent
        && a.confidence >= Confidence::Possible
        && !a.confirmed_by_user
        && a.alternatives.iter().any(|c| c.id == "not_sure")
}

/// Builds an assumption, putting `chosen` first in the alternatives if it is missing.
pub(crate) fn make(
    kind: AssumptionKind,
    subject: Subject,
    chosen: Choice,
    mut alternatives: Vec<Choice>,
    confidence: Confidence,
    consequence: Consequence,
    treatment: Treatment,
    line: impl Into<String>,
) -> Assumption {
    if !alternatives.iter().any(|c| c.id == chosen.id) {
        alternatives.insert(0, chosen.clone());
    }
    Assumption { kind, subject, chosen, alternatives, confidence, consequence, treatment, line: line.into(), confirmed_by_user: false }
}

/// Lower-cased name split into alphanumeric tokens ("Lng_GCJ" → ["lng", "gcj"]); CJK characters
/// count as alphanumeric, so "经度" is one token.
pub(crate) fn tokens(name: &str) -> Vec<String> {
    name.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_use_catalog_wording() {
        assert_eq!(lines::csv_located("lon", "lat"), "Located using lon and lat as GPS coordinates (WGS 84).");
        assert_eq!(lines::raster_nodata(-9999.0), "Treated \u{2212}9999 as No data.");
        assert_eq!(lines::excel_sheet("Schools"), "Opened sheet 'Schools'.");
        assert_eq!(lines::mixed_split(&["points", "lines", "areas"]), "Split into 3 layers: points, lines, areas");
    }

    #[test]
    fn tokens_split_names() {
        assert_eq!(tokens("Lng_GCJ"), vec!["lng", "gcj"]);
        assert_eq!(tokens("经度"), vec!["经度"]);
        assert_eq!(tokens(" point x "), vec!["point", "x"]);
    }
}
