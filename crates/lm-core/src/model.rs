//! The data contracts (Architecture › Data contracts), Phase 2 part: layer profiles, inferred
//! properties with their confidence, assumptions, import status and the document's layers.
//!
//! These types are shared by the profiler (`lm-profile`), the import coordinator (`lm-jobs`), the
//! document format (`lm-doc`) and the interface (`lm-ui`). Until Gate 5 a phase may only add
//! fields (Gate protocol › What makes rollback cheap), so every saved struct takes
//! `#[serde(default)]` on fields added later.

use serde::{Deserialize, Serialize};

/// A layer in the document. Never reused within a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayerId(pub u64);

/// How sure the app is about an inferred property.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    Certain,
    Likely,
    Possible,
    Unknown,
}

/// What a wrong guess would change (Assumptions and questions › the grid).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Consequence {
    DisplayOnly,
    ChangesNumbers,
    ChangesLocations,
}

/// How an uncertain interpretation is treated (Assumptions and questions › The four treatments).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Treatment {
    /// Applied and recorded in Details.
    Silent,
    /// Applied and stated in one line with a purple dot and a Change link.
    Visible,
    /// Asked once, with a recommended answer preselected and Not sure.
    Question,
    /// The action is not offered.
    Block,
}

/// Why the app believes something, in words for the Details pane ("Column names lon and lat;
/// 242 of 248 rows parse as coordinates").
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence(pub String);

/// An inferred property: its value, how sure, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Known<T> {
    pub value: T,
    pub confidence: Confidence,
    pub evidence: Evidence,
}

impl<T> Known<T> {
    pub fn certain(value: T, evidence: impl Into<String>) -> Self {
        Known { value, confidence: Confidence::Certain, evidence: Evidence(evidence.into()) }
    }
    pub fn likely(value: T, evidence: impl Into<String>) -> Self {
        Known { value, confidence: Confidence::Likely, evidence: Evidence(evidence.into()) }
    }
    pub fn possible(value: T, evidence: impl Into<String>) -> Self {
        Known { value, confidence: Confidence::Possible, evidence: Evidence(evidence.into()) }
    }
    pub fn unknown(value: T, evidence: impl Into<String>) -> Self {
        Known { value, confidence: Confidence::Unknown, evidence: Evidence(evidence.into()) }
    }
}

/// A coordinate system, as far as LayeredMap needs to name one.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Crs {
    /// Longitude/latitude on WGS 84 ("GPS coordinates").
    Wgs84,
    /// China's GCJ-02 offset (Amap, Tencent).
    Gcj02,
    /// Baidu's BD-09 offset.
    Bd09,
    /// An EPSG code (UTM zones, Gauss–Krüger zones, national grids).
    Epsg(u32),
    /// A coordinate system given as WKT (from a .prj or GeoTIFF keys) that has no EPSG code here.
    Wkt(String),
    /// Not known.
    Unknown,
}

impl Crs {
    /// A short name for interface lines ("GPS coordinates (WGS 84)").
    pub fn label(&self) -> String {
        match self {
            Crs::Wgs84 => "GPS coordinates (WGS 84)".into(),
            Crs::Gcj02 => "Amap/Tencent coordinates (GCJ-02)".into(),
            Crs::Bd09 => "Baidu coordinates (BD-09)".into(),
            Crs::Epsg(c) => format!("EPSG:{c}"),
            Crs::Wkt(_) => "the coordinate system in the file".into(),
            Crs::Unknown => "an unknown coordinate system".into(),
        }
    }
}

/// The kind of a layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayerKind {
    Points,
    Lines,
    Areas,
    Raster,
    /// Rows without usable locations (a CSV opened as a table).
    Table,
}

impl LayerKind {
    /// Singular and plural nouns for counts ("248 points").
    pub fn nouns(self) -> (&'static str, &'static str) {
        match self {
            LayerKind::Points => ("point", "points"),
            LayerKind::Lines => ("line", "lines"),
            LayerKind::Areas => ("area", "areas"),
            LayerKind::Raster => ("cell", "cells"),
            LayerKind::Table => ("row", "rows"),
        }
    }

    /// Stacking rank for new layers (WP2.7): points and lines above areas, areas above rasters.
    /// Higher draws on top.
    pub fn stack_rank(self) -> u8 {
        match self {
            LayerKind::Points | LayerKind::Table => 3,
            LayerKind::Lines => 2,
            LayerKind::Areas => 1,
            LayerKind::Raster => 0,
        }
    }
}

/// An extent in longitude/latitude (WGS 84).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// A source file and its fingerprint: enough to tell later whether it changed or moved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    /// Absolute path when opened.
    pub path: String,
    /// For a file inside an archive or a multi-layer file: the entry or layer name.
    #[serde(default)]
    pub member: Option<String>,
    pub size: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
    /// SHA-256 of the file's bytes, when computed.
    #[serde(default)]
    pub sha256: Option<String>,
}

impl SourceRef {
    /// The file name, for subtitles ("schools.csv").
    pub fn file_name(&self) -> &str {
        self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path)
    }
}

/// A field's storage type as read (raw strings are kept separately by the readers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Integer,
    Float,
    Text,
    Boolean,
    Date,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasureKind {
    /// A measured value to average (temperature).
    Intensive,
    Count,
    Density,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldRole {
    Identifier,
    /// A place or object name (used for object titles and search).
    Name,
    Category,
    Measure(MeasureKind),
    Text,
    Date,
    /// A coordinate column (longitude or latitude, x or y).
    Coordinate,
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldStats {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub median: Option<f64>,
    pub distinct: u64,
    pub nulls: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldProfile {
    pub name: String,
    pub dtype: DataType,
    pub role: Known<FieldRole>,
    pub unit: Known<Option<String>>,
    #[serde(default)]
    pub stats: Option<FieldStats>,
}

/// What a raster cell's value means (Assumption catalog › Raster cell meaning).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellMeaning {
    /// A measured value: averaged, not summed.
    Measured,
    /// A total per cell (population counts): summed.
    Total,
    /// Category codes (land cover).
    Category,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BandProfile {
    /// 1-based, as GIS tools number bands.
    pub index: u32,
    pub name: String,
    pub nodata: Known<Option<f64>>,
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub offset: Option<f64>,
    pub unit: Known<Option<String>>,
    pub cell_meaning: Known<CellMeaning>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

/// One option of an assumption ("WGS 84", "GCJ-02", "Not sure").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    /// Stable machine id ("wgs84", "gcj02", "not_sure", "columns:lon,lat").
    pub id: String,
    /// The words shown in the Change popover.
    pub label: String,
}

impl Choice {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Choice { id: id.into(), label: label.into() }
    }
    pub fn not_sure() -> Self {
        Choice::new("not_sure", "Not sure")
    }
}

/// Which row of the assumption catalog (Assumptions and questions › Assumption catalog).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssumptionKind {
    CsvCoordinateColumns,
    CsvAmbiguousColumns,
    NoCrsLonLat,
    NoCrsProjected,
    ChinaOffsetPossible,
    ChinaOffsetNamed,
    RasterNoData,
    RasterNoUnit,
    RasterKelvin,
    LandsatScale,
    RasterCellMeaning,
    IdentifierField,
    FewValuesCategorical,
    ShapefileGbk,
    ExcelSheet,
    MixedGeometrySplit,
}

/// What an assumption is about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Subject {
    Layer,
    Field(String),
    Band(u32),
}

/// An applied interpretation the user can see and change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assumption {
    pub kind: AssumptionKind,
    pub subject: Subject,
    pub chosen: Choice,
    /// Every option the Change popover offers, `chosen` included, Not sure last where it applies.
    pub alternatives: Vec<Choice>,
    pub confidence: Confidence,
    pub consequence: Consequence,
    pub treatment: Treatment,
    /// The line shown to the user, without the trailing "Change" link ("Located using lon and
    /// lat as GPS coordinates (WGS 84).").
    pub line: String,
    #[serde(default)]
    pub confirmed_by_user: bool,
}

/// A record left out of a layer, for the import report ("Review 6").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exclusion {
    /// 1-based row (CSV line after the header, feature number otherwise).
    pub row: u64,
    pub reason: String,
}

/// An import's final status (WP2.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportStatus {
    Complete,
    CompleteWithExclusions,
    NeedsDecision,
    Canceled,
    Failed(String),
}

/// Everything the import learned about a layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerProfile {
    pub source: SourceRef,
    /// Format name for Details ("CSV", "Shapefile", "GeoTIFF").
    pub format: String,
    pub kind: LayerKind,
    /// Features, rows or cells.
    pub count: u64,
    pub extent_wgs84: Option<Extent>,
    pub crs: Known<Crs>,
    pub fields: Vec<FieldProfile>,
    #[serde(default)]
    pub bands: Vec<BandProfile>,
    pub assumptions: Vec<Assumption>,
    #[serde(default)]
    pub exclusions: Vec<Exclusion>,
    /// Text encoding read, when the format has one ("UTF-8", "GBK").
    #[serde(default)]
    pub encoding: Option<String>,
    /// Notices from the reader in plain words ("No .dbf file: shapes only.").
    #[serde(default)]
    pub notices: Vec<String>,
}

impl LayerProfile {
    /// "248 points · schools.csv"
    pub fn subtitle(&self) -> String {
        let (one, many) = self.kind.nouns();
        let noun = if self.count == 1 { one } else { many };
        format!("{} {} · {}", group_thousands(self.count), noun, self.source.file_name())
    }

    /// The assumptions shown as lines (Visible ones), in catalog order.
    pub fn visible_assumptions(&self) -> impl Iterator<Item = &Assumption> {
        self.assumptions.iter().filter(|a| a.treatment == Treatment::Visible)
    }

    /// Questions still waiting for an answer.
    pub fn open_questions(&self) -> impl Iterator<Item = &Assumption> {
        self.assumptions.iter().filter(|a| a.treatment == Treatment::Question && !a.confirmed_by_user)
    }
}

/// 1248 → "1,248" (Microcopy rules: thousands separators).
pub fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1248), "1,248");
        assert_eq!(group_thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn subtitle_names_the_count_and_file() {
        let p = LayerProfile {
            source: SourceRef { path: "/Users/a/Downloads/schools.csv".into(), member: None, size: 1, mtime: 0, sha256: None },
            format: "CSV".into(),
            kind: LayerKind::Points,
            count: 248,
            extent_wgs84: None,
            crs: Known::likely(Crs::Wgs84, "lon/lat columns"),
            fields: vec![],
            bands: vec![],
            assumptions: vec![],
            exclusions: vec![],
            encoding: None,
            notices: vec![],
        };
        assert_eq!(p.subtitle(), "248 points · schools.csv");
    }

    #[test]
    fn rasters_stack_below_areas_below_points() {
        assert!(LayerKind::Points.stack_rank() > LayerKind::Areas.stack_rank());
        assert!(LayerKind::Lines.stack_rank() > LayerKind::Areas.stack_rank());
        assert!(LayerKind::Areas.stack_rank() > LayerKind::Raster.stack_rank());
    }
}
