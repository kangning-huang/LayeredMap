//! Catalog rows "Excel workbook with several sheets" and "GeoJSON with mixed geometry types".

use crate::csv::{CsvLocation, csv_locate};
use crate::{lines, make};
use lm_core::model::{Assumption, AssumptionKind, Choice, Confidence, Consequence, LayerKind, Subject, Treatment};
use rustgis_data::{Dataset, Geometry};

/// One sheet of a workbook: its name, header row and a sample of rows, as raw strings.
#[derive(Clone, Copy, Debug)]
pub struct SheetSample<'a> {
    pub name: &'a str,
    pub headers: &'a [String],
    pub rows: &'a [Vec<String>],
}

/// Which sheet of a workbook to open.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetChoice {
    /// Index into the sheets given.
    pub sheet: usize,
    /// Where that sheet's records are ([`csv_locate`] on it).
    pub location: CsvLocation,
    /// The visible [`AssumptionKind::ExcelSheet`] when the workbook has several sheets.
    pub assumption: Option<Assumption>,
}

/// Picks the sheet to open (catalog: "Excel workbook with several sheets").
///
/// Each sheet goes through [`csv_locate`]. The first sheet with coordinate columns (located,
/// ambiguous or projected) is opened; with none, the first sheet opens as a table. With more
/// than one sheet, a visible "Opened sheet 'Schools'." names the choice: `Likely` when only that
/// sheet has coordinates, `Possible` when several have (the first is taken) or none has.
/// Returns `None` for a workbook without sheets.
pub fn excel_sheet(sheets: &[SheetSample]) -> Option<SheetChoice> {
    let located: Vec<(usize, CsvLocation)> = sheets
        .iter()
        .enumerate()
        .map(|(i, s)| (i, csv_locate(s.headers, s.rows, s.name)))
        .filter(|(_, l)| !matches!(l, CsvLocation::Table { .. }))
        .collect();
    let (sheet, location, confidence) = match located.as_slice() {
        [(i, l)] => (*i, l.clone(), Confidence::Likely),
        [(i, l), ..] => (*i, l.clone(), Confidence::Possible),
        [] => {
            let first = sheets.first()?;
            (0, csv_locate(first.headers, first.rows, first.name), Confidence::Possible)
        }
    };
    let name = sheets.get(sheet).map(|s| s.name).unwrap_or_default();
    let assumption = (sheets.len() > 1).then(|| {
        let choice = |s: &SheetSample| Choice::new(format!("sheet:{}", s.name), s.name);
        make(
            AssumptionKind::ExcelSheet,
            Subject::Layer,
            Choice::new(format!("sheet:{name}"), name),
            sheets.iter().map(choice).collect(),
            confidence,
            Consequence::ChangesLocations,
            Treatment::Visible,
            lines::excel_sheet(name),
        )
    });
    Some(SheetChoice { sheet, location, assumption })
}

/// The kinds of geometry in a dataset with their feature counts, in the order points, lines,
/// areas (Multi* types count as their single-part kind; features without geometry are left
/// out).
pub fn geometry_kinds(ds: &Dataset) -> Vec<(LayerKind, usize)> {
    let mut counts = [0usize; 3];
    for g in ds.features.iter().filter_map(|f| f.geometry.as_ref()) {
        let i = match g {
            Geometry::Point(_) | Geometry::MultiPoint(_) => 0,
            Geometry::Line(_) => 1,
            Geometry::Polygon(_) => 2,
        };
        if let Some(c) = counts.get_mut(i) {
            *c += 1;
        }
    }
    [LayerKind::Points, LayerKind::Lines, LayerKind::Areas].into_iter().zip(counts).filter(|(_, n)| *n > 0).collect()
}

/// The silent [`AssumptionKind::MixedGeometrySplit`] when a file holds more than one kind of
/// geometry (catalog: "GeoJSON with mixed geometry types"): "Split into 3 layers: points,
/// lines, areas". `kinds` is [`geometry_kinds`]'s output (or just the kinds present).
pub fn mixed_geometry_split(kinds: &[LayerKind]) -> Option<Assumption> {
    let mut present: Vec<LayerKind> = Vec::new();
    for k in [LayerKind::Points, LayerKind::Lines, LayerKind::Areas] {
        if kinds.contains(&k) {
            present.push(k);
        }
    }
    if present.len() < 2 {
        return None;
    }
    let words: Vec<&str> = present.iter().map(|k| k.nouns().1).collect();
    Some(make(
        AssumptionKind::MixedGeometrySplit,
        Subject::Layer,
        Choice::new("split", "One layer per kind of shape"),
        vec![Choice::new("split", "One layer per kind of shape")],
        Confidence::Certain,
        Consequence::DisplayOnly,
        Treatment::Silent,
        lines::mixed_split(&words),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_data::{Coord, GeometryType};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn excel_sheet_matches_only_sheet_with_coordinates() {
        let (h1, r1) = (s(&["topic", "value"]), vec![s(&["a", "1"])]);
        let h2 = s(&["name", "longitude", "latitude"]);
        let r2: Vec<Vec<String>> = (0..20).map(|i| s(&["x", &format!("121.{i:02}"), "31.2"])).collect();
        let (h3, r3) = (s(&["item", "cost"]), vec![s(&["b", "2"])]);
        let sheets = [
            SheetSample { name: "Summary", headers: &h1, rows: &r1 },
            SheetSample { name: "Schools", headers: &h2, rows: &r2 },
            SheetSample { name: "Budget", headers: &h3, rows: &r3 },
        ];
        let c = excel_sheet(&sheets).unwrap();
        assert_eq!(c.sheet, 1);
        assert!(matches!(c.location, CsvLocation::Located { .. }));
        let a = c.assumption.unwrap();
        assert_eq!((a.kind, a.treatment, a.confidence), (AssumptionKind::ExcelSheet, Treatment::Visible, Confidence::Likely));
        assert_eq!(a.line, "Opened sheet 'Schools'.");
        assert_eq!(a.alternatives.len(), 3);
    }

    #[test]
    fn excel_sheet_does_not_match_single_sheet_or_no_coordinates() {
        let h = s(&["name", "lon", "lat"]);
        let r = vec![s(&["a", "121.4", "31.2"])];
        let c = excel_sheet(&[SheetSample { name: "Only", headers: &h, rows: &r }]).unwrap();
        assert_eq!(c.assumption, None);
        let (h1, r1) = (s(&["a"]), vec![s(&["1"])]);
        let c =
            excel_sheet(&[SheetSample { name: "A", headers: &h1, rows: &r1 }, SheetSample { name: "B", headers: &h1, rows: &r1 }]).unwrap();
        assert_eq!(c.sheet, 0);
        assert!(matches!(c.location, CsvLocation::Table { .. }));
        assert_ne!(c.assumption.map(|a| a.confidence), Some(Confidence::Likely));
        assert_eq!(excel_sheet(&[]), None);
    }

    #[test]
    fn mixed_geometry_split_matches_mixed_kinds() {
        let mut ds = Dataset::new("m", GeometryType::Point, vec![]);
        let p = Coord::new(0.0, 0.0);
        ds.push_next(Some(Geometry::Point(p)), vec![]);
        ds.push_next(Some(Geometry::Polygon(vec![vec![vec![p, Coord::new(1.0, 0.0), Coord::new(0.0, 1.0), p]]])), vec![]);
        ds.push_next(Some(Geometry::Line(vec![vec![p, Coord::new(1.0, 1.0)]])), vec![]);
        ds.push_next(Some(Geometry::MultiPoint(vec![p])), vec![]);
        let kinds = geometry_kinds(&ds);
        assert_eq!(kinds, vec![(LayerKind::Points, 2), (LayerKind::Lines, 1), (LayerKind::Areas, 1)]);
        let k: Vec<LayerKind> = kinds.iter().map(|(k, _)| *k).collect();
        let a = mixed_geometry_split(&k).unwrap();
        assert_eq!((a.kind, a.treatment), (AssumptionKind::MixedGeometrySplit, Treatment::Silent));
        assert_eq!(a.line, "Split into 3 layers: points, lines, areas");
        assert_eq!(mixed_geometry_split(&[LayerKind::Areas, LayerKind::Points]).unwrap().line, "Split into 2 layers: points, areas");
    }

    #[test]
    fn mixed_geometry_split_does_not_match_one_kind() {
        assert_eq!(mixed_geometry_split(&[LayerKind::Areas]), None);
        assert_eq!(mixed_geometry_split(&[]), None);
        assert_eq!(mixed_geometry_split(&[LayerKind::Raster, LayerKind::Table]), None);
    }
}
