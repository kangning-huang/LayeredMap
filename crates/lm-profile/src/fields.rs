//! Field roles and statistics, and the catalog rows "Field looks like an identifier" and
//! "Numeric field with 12 or fewer distinct values".

use crate::csv::parse_number;
use crate::{lines, make, tokens};
use lm_core::model::{
    Assumption, AssumptionKind, Choice, Confidence, Consequence, DataType, FieldProfile, FieldRole, FieldStats, Known, MeasureKind,
    Subject, Treatment,
};
use rustgis_data::{Dataset, FieldType, Value};
use std::collections::HashSet;

/// At most this many distinct numeric values are colored as categories.
const FEW_VALUES: usize = 12;

/// Raw cell strings as written in the file (CSV, a sheet), kept beside the typed dataset so that
/// leading zeros, decimal commas and dates are judged as written.
#[derive(Clone, Copy, Debug)]
pub struct RawTable<'a> {
    pub headers: &'a [String],
    /// Data rows in file order, each split into cells.
    pub rows: &'a [Vec<String>],
}

/// A column that locates the records (from [`crate::CsvLocation::coordinate_columns`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoordinateColumn {
    pub name: String,
    pub confidence: Confidence,
}

/// One cell as the profiler sees it.
#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Null,
    Number {
        value: f64,
        integer: bool,
    },
    /// Text; `digits_with_leading_zero` for codes like "001234".
    Text(String),
    Bool(bool),
}

fn cell_from_raw(s: &str) -> Cell {
    let t = s.trim();
    if t.is_empty() {
        return Cell::Null;
    }
    // Codes keep their leading zeros: "001234" is text, "0" and "0.5" are numbers.
    let digits = t.trim_start_matches('-');
    if digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0.") && !digits.starts_with("0,") {
        return Cell::Text(t.to_string());
    }
    match parse_number(t) {
        Some(v) => Cell::Number { value: v, integer: v.fract() == 0.0 && !t.contains(['.', ',', 'e', 'E']) },
        None => Cell::Text(t.to_string()),
    }
}

fn cell_from_value(v: &Value, ty: FieldType) -> Cell {
    match v {
        Value::Null => Cell::Null,
        Value::Int(i) => Cell::Number { value: *i as f64, integer: true },
        Value::Float(f) if f.is_finite() => Cell::Number { value: *f, integer: ty == FieldType::Integer && f.fract() == 0.0 },
        Value::Float(_) => Cell::Null,
        Value::Bool(b) => Cell::Bool(*b),
        Value::Text(s) if s.trim().is_empty() => Cell::Null,
        Value::Text(s) => Cell::Text(s.clone()),
    }
}

/// A date as written: `YYYY-MM-DD`, `YYYY/MM/DD`, `YYYY.MM.DD`, optionally followed by a time.
fn is_date(s: &str) -> bool {
    let d = s.trim().split([' ', 'T']).next().unwrap_or("");
    let parts: Vec<&str> = d.split(['-', '/', '.']).collect();
    let [y, m, day] = parts.as_slice() else { return false };
    let num = |p: &str, len: std::ops::RangeInclusive<usize>| len.contains(&p.len()) && p.chars().all(|c| c.is_ascii_digit());
    if !(num(y, 4..=4) && num(m, 1..=2) && num(day, 1..=2)) {
        return false;
    }
    let (m, day): (u32, u32) = (m.parse().unwrap_or(0), day.parse().unwrap_or(0));
    (1..=12).contains(&m) && (1..=31).contains(&day)
}

/// The name ends in id / code / adcode / fips (as a word, or after a lower-case letter in
/// camelCase: "district_id", "DistrictID", "adcode", "FIPS", "编码").
fn id_like_name(name: &str) -> bool {
    let toks = tokens(name);
    let Some(last) = toks.last() else { return false };
    let words = ["id", "code", "adcode", "fips", "fid", "gid", "oid", "objectid", "uid", "geoid", "编码", "代码", "编号"];
    if words.contains(&last.as_str()) || ["编码", "代码", "编号"].iter().any(|w| last.ends_with(w)) {
        return true;
    }
    // camelCase: "DistrictID", "zipCode", "countyFips".
    let trimmed = name.trim();
    ["ID", "Id", "Code", "CODE", "Fips", "FIPS"]
        .iter()
        .any(|suffix| trimmed.strip_suffix(suffix).and_then(|head| head.chars().last()).is_some_and(|c| c.is_lowercase()))
}

/// A name/title column: "name", "NAME", "name_en", "school_name", "名称", "名字", "地名".
fn name_like(name: &str) -> bool {
    let toks = tokens(name);
    toks.first().is_some_and(|t| t == "name")
        || toks.last().is_some_and(|t| t == "name")
        || ["名称", "名字", "地名"].iter().any(|w| name.contains(w))
}

fn measure_kind(name: &str, integers: bool) -> MeasureKind {
    let l = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| l.contains(w));
    if has(&["density", "per_km", "per km", "/km", "密度"]) {
        MeasureKind::Density
    } else if integers && has(&["pop", "count", "number", "total", "students", "人口", "数量", "人数"]) {
        MeasureKind::Count
    } else if has(&["temp", "rate", "pct", "percent", "ratio", "mean", "avg", "average", "率", "温度"]) {
        MeasureKind::Intensive
    } else {
        MeasureKind::Unknown
    }
}

/// A unit read from the name: `area_km2` → km², `share_pct` → %, `height (m)` → m.
fn unit_from_name(name: &str) -> Option<String> {
    let t = name.trim();
    if let (Some(open), Some(close)) = (t.rfind(['(', '[']), t.rfind([')', ']']))
        && close > open + 1
    {
        return t.get(open + 1..close).map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    }
    let last = tokens(t).pop()?;
    Some(
        match last.as_str() {
            "km2" | "sqkm" => "km²",
            "m2" | "sqm" => "m²",
            "ha" => "ha",
            "pct" | "percent" => "%",
            "km" => "km",
            _ => return None,
        }
        .to_string(),
    )
}

fn median(sorted: &[f64]) -> Option<f64> {
    let n = sorted.len();
    if n == 0 {
        return None;
    }
    let mid = n / 2;
    if n % 2 == 1 { sorted.get(mid).copied() } else { Some((sorted.get(mid - 1)? + sorted.get(mid)?) / 2.0) }
}

/// What the profiler found for one column.
struct Column<'a> {
    name: &'a str,
    cells: Vec<Cell>,
    declared: Option<FieldType>,
}

fn profile_column(col: &Column, coords: &[CoordinateColumn], name_taken: &mut bool) -> (FieldProfile, Vec<Assumption>) {
    let n_rows = col.cells.len();
    let values: Vec<&Cell> = col.cells.iter().filter(|c| **c != Cell::Null).collect();
    let nulls = (n_rows - values.len()) as u64;
    let numbers: Vec<(f64, bool)> =
        values.iter().filter_map(|c| if let Cell::Number { value, integer } = c { Some((*value, *integer)) } else { None }).collect();
    let texts: Vec<&str> = values.iter().filter_map(|c| if let Cell::Text(t) = c { Some(t.as_str()) } else { None }).collect();
    let bools = values.iter().filter(|c| matches!(c, Cell::Bool(_))).count();
    let all_numeric = !values.is_empty() && numbers.len() == values.len();
    let all_integers = all_numeric && numbers.iter().all(|(_, i)| *i);
    let dates = texts.iter().filter(|t| is_date(t)).count();
    let all_dates = !texts.is_empty() && texts.len() == values.len() && dates as f64 >= 0.95 * texts.len() as f64;

    let distinct: usize = {
        let mut seen: HashSet<String> = HashSet::new();
        for c in &values {
            seen.insert(match c {
                Cell::Number { value, .. } => format!("n{value}"),
                Cell::Text(t) => format!("t{}", t.trim()),
                Cell::Bool(b) => format!("b{b}"),
                Cell::Null => String::new(),
            });
        }
        seen.len()
    };
    let mut sorted: Vec<f64> = numbers.iter().map(|(v, _)| *v).collect();
    sorted.sort_by(f64::total_cmp);
    let stats = FieldStats {
        min: if all_numeric { sorted.first().copied() } else { None },
        max: if all_numeric { sorted.last().copied() } else { None },
        median: if all_numeric { median(&sorted) } else { None },
        distinct: distinct as u64,
        nulls,
    };
    let dtype = match col.declared {
        Some(FieldType::Boolean) => DataType::Boolean,
        _ if bools > 0 && bools == values.len() => DataType::Boolean,
        _ if all_dates => DataType::Date,
        _ if all_integers => DataType::Integer,
        _ if all_numeric => DataType::Float,
        Some(FieldType::Integer) if values.is_empty() => DataType::Integer,
        Some(FieldType::Double) if values.is_empty() => DataType::Float,
        _ => DataType::Text,
    };

    let leading_zeros = !texts.is_empty()
        && texts.iter().all(|t| t.chars().all(|c| c.is_ascii_digit()))
        && texts.iter().any(|t| t.len() > 1 && t.starts_with('0'));
    let unique = n_rows >= 2 && nulls == 0 && distinct == values.len();
    // Integers count as ID evidence only when they are dense (1, 2, 3… or 1001, 1002…): unique
    // totals such as populations are measures, not IDs.
    let dense_integers =
        all_integers && sorted.first().zip(sorted.last()).is_some_and(|(lo, hi)| hi - lo + 1.0 <= 2.0 * values.len() as f64);
    let id_name = id_like_name(col.name);

    let mut assumptions = Vec::new();
    let subject = Subject::Field(col.name.to_string());
    let (role, evidence): (Known<FieldRole>, String);
    if let Some(c) = coords.iter().find(|c| c.name == col.name) {
        evidence = "Locates the records.".into();
        role = Known { value: FieldRole::Coordinate, confidence: c.confidence, evidence: Default::default() };
    } else if unique && (id_name || leading_zeros || dense_integers) {
        let why = [
            (id_name, "the name ends in id/code"),
            (leading_zeros, "values have leading zeros"),
            (dense_integers, "values are consecutive whole numbers"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, w)| *w)
        .collect::<Vec<_>>()
        .join(", ");
        evidence = format!("Unique in every row; {why}.");
        role = Known::likely(FieldRole::Identifier, "");
        assumptions.push(make(
            AssumptionKind::IdentifierField,
            subject.clone(),
            Choice::new("id", "An ID"),
            vec![Choice::new("id", "An ID"), Choice::new("value", "A value")],
            Confidence::Likely,
            Consequence::DisplayOnly,
            Treatment::Silent,
            lines::IDENTIFIER,
        ));
    } else if values.is_empty() {
        evidence = "No values.".into();
        role = Known::unknown(FieldRole::Unknown, "");
    } else if dtype == DataType::Date {
        evidence = format!("{dates} of {} values are dates.", values.len());
        role = Known::likely(FieldRole::Date, "");
    } else if dtype == DataType::Boolean {
        evidence = "Yes/no values.".into();
        role = Known::likely(FieldRole::Category, "");
    } else if all_numeric {
        // Few distinct whole numbers that repeat: categories (codes, ratings).
        if all_integers && distinct <= FEW_VALUES && distinct * 2 <= values.len() {
            evidence = format!("{distinct} distinct whole numbers in {} values.", values.len());
            role = Known::likely(FieldRole::Category, "");
            assumptions.push(make(
                AssumptionKind::FewValuesCategorical,
                subject.clone(),
                Choice::new("categories", "Categories"),
                vec![Choice::new("categories", "Categories"), Choice::new("numbers", "A range of numbers")],
                Confidence::Likely,
                Consequence::DisplayOnly,
                Treatment::Silent,
                lines::FEW_VALUES,
            ));
        } else {
            evidence = format!("Numbers ({distinct} distinct).");
            role = Known::likely(FieldRole::Measure(measure_kind(col.name, all_integers)), "");
        }
    } else if name_like(col.name) {
        let first = !*name_taken;
        *name_taken = true;
        evidence = if first { "Named like a name.".into() } else { "Named like a name (another name column comes first).".into() };
        role = Known::likely(FieldRole::Name, "");
    } else if distinct <= 2 || (distinct < values.len() && (distinct <= FEW_VALUES || distinct * 2 <= values.len())) {
        evidence = format!("{distinct} distinct values in {} rows.", values.len());
        role = Known::likely(FieldRole::Category, "");
    } else {
        evidence = format!("Text with {distinct} distinct values.");
        role = Known::likely(FieldRole::Text, "");
    }
    let role = Known { evidence: lm_core::model::Evidence(evidence), ..role };
    let unit = match unit_from_name(col.name) {
        Some(u) => Known::likely(Some(u), "From the field name."),
        None => Known::unknown(None, "No unit in the file."),
    };
    (FieldProfile { name: col.name.to_string(), dtype, role, unit, stats: Some(stats) }, assumptions)
}

fn profile_columns(cols: &[Column], coords: &[CoordinateColumn]) -> (Vec<FieldProfile>, Vec<Assumption>) {
    let mut name_taken = false;
    let mut fields = Vec::with_capacity(cols.len());
    let mut assumptions = Vec::new();
    for c in cols {
        let (f, a) = profile_column(c, coords, &mut name_taken);
        fields.push(f);
        assumptions.extend(a);
    }
    (fields, assumptions)
}

/// Profiles every field of a dataset: role (with confidence and evidence), storage type, a unit
/// read from the name, and statistics (min, max, median, distinct count, null count).
///
/// - `raw`: the file's raw cell strings (CSV, sheets). A field whose name is a header there is
///   judged on its raw strings, so codes keep leading zeros ("001234" is text), decimal commas
///   parse, and dates are recognised as written. Other fields use the dataset's typed values.
/// - `coordinates`: columns that locate the records ([`crate::CsvLocation::coordinate_columns`]).
///
/// Roles, in order of precedence:
/// 1. **Coordinate**: listed in `coordinates`.
/// 2. **Identifier** (silent [`AssumptionKind::IdentifierField`]): no blanks, unique in every
///    row, and a name ending in id/code/adcode/fips, or leading zeros, or consecutive whole
///    numbers (unique but sparse integers, like populations, are measures).
/// 3. **Date**: at least 95% of the text values are dates as written.
/// 4. **Category** for numbers (silent [`AssumptionKind::FewValuesCategorical`]): whole numbers
///    with at most 12 distinct values, each appearing twice on average.
/// 5. **Measure**: other numbers; the [`MeasureKind`] from the name (pop/count → Count…).
/// 6. **Name**: text named name / NAME / name_en / 名称 (every such column; the first is the
///    object title).
/// 7. **Category** for text: repeating values (at most 12 distinct, or each twice on average).
/// 8. **Text** otherwise.
///
/// Returns the field profiles in dataset order and the assumptions (silent ones, for Details).
pub fn profile_fields(ds: &Dataset, raw: Option<RawTable>, coordinates: &[CoordinateColumn]) -> (Vec<FieldProfile>, Vec<Assumption>) {
    let cols: Vec<Column> = ds
        .fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let raw_col = raw.and_then(|r| r.headers.iter().position(|h| h.trim() == f.name.trim()).map(|j| (r, j)));
            let cells = match raw_col {
                Some((r, j)) => r.rows.iter().map(|row| cell_from_raw(row.get(j).map(String::as_str).unwrap_or(""))).collect(),
                None => (0..ds.features.len()).map(|row| cell_from_value(ds.value(row, i), f.ty)).collect(),
            };
            Column { name: &f.name, cells, declared: if raw_col.is_some() { None } else { Some(f.ty) } }
        })
        .collect();
    profile_columns(&cols, coordinates)
}

/// [`profile_fields`] for a table known only by its raw strings (a CSV or sheet sample).
pub fn profile_raw_table(raw: RawTable, coordinates: &[CoordinateColumn]) -> (Vec<FieldProfile>, Vec<Assumption>) {
    let cols: Vec<Column> = raw
        .headers
        .iter()
        .enumerate()
        .map(|(j, h)| Column {
            name: h.as_str(),
            cells: raw.rows.iter().map(|row| cell_from_raw(row.get(j).map(String::as_str).unwrap_or(""))).collect(),
            declared: None,
        })
        .collect();
    profile_columns(&cols, coordinates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_data::{Field, GeometryType};

    fn ds(fields: &[(&str, FieldType)], rows: Vec<Vec<Value>>) -> Dataset {
        let mut d = Dataset::new("t", GeometryType::None, fields.iter().map(|(n, t)| Field::new(*n, *t)).collect());
        for r in rows {
            d.push_next(None, r);
        }
        d
    }
    fn role(fields: &[FieldProfile], name: &str) -> FieldRole {
        fields.iter().find(|f| f.name == name).map(|f| f.role.value).unwrap_or(FieldRole::Unknown)
    }
    fn kinds(a: &[Assumption]) -> Vec<(AssumptionKind, Subject)> {
        a.iter().map(|a| (a.kind, a.subject.clone())).collect()
    }

    #[test]
    fn identifier_matches_unique_ids() {
        let d = ds(
            &[("district_id", FieldType::Text), ("seq", FieldType::Integer), ("FIPS", FieldType::Integer), ("DistrictID", FieldType::Text)],
            (1..=20)
                .map(|i| vec![Value::Text(format!("D{i:02}")), Value::Int(i), Value::Int(i * 1000 + 7), Value::Text(format!("X{i}"))])
                .collect(),
        );
        let (f, a) = profile_fields(&d, None, &[]);
        for n in ["district_id", "seq", "FIPS", "DistrictID"] {
            assert_eq!(role(&f, n), FieldRole::Identifier, "{n}");
        }
        assert!(
            a.iter()
                .all(|a| a.kind == AssumptionKind::IdentifierField && a.treatment == Treatment::Silent && a.line == "Treated as an ID.")
        );
        assert_eq!(a.len(), 4);
        // Leading zeros, from raw strings.
        let headers = vec!["编码".to_string()];
        let rows: Vec<Vec<String>> = ["001234", "000056", "120001"].iter().map(|s| vec![s.to_string()]).collect();
        let (f, a) = profile_raw_table(RawTable { headers: &headers, rows: &rows }, &[]);
        assert_eq!(f[0].role.value, FieldRole::Identifier);
        assert_eq!(f[0].dtype, DataType::Text);
        assert_eq!(kinds(&a), vec![(AssumptionKind::IdentifierField, Subject::Field("编码".into()))]);
    }

    #[test]
    fn identifier_does_not_match_repeats_blanks_or_sparse_integers() {
        let d = ds(
            &[("code", FieldType::Text), ("population", FieldType::Integer), ("paid", FieldType::Integer), ("valid", FieldType::Text)],
            (1..=20)
                .map(|i| {
                    vec![
                        Value::Text(format!("C{}", i % 5)),
                        Value::Int(i * 7919 + 100_000),
                        if i == 3 { Value::Null } else { Value::Int(i) },
                        Value::Text(format!("v{i}")),
                    ]
                })
                .collect(),
        );
        let (f, a) = profile_fields(&d, None, &[]);
        assert_ne!(role(&f, "code"), FieldRole::Identifier);
        assert_eq!(role(&f, "population"), FieldRole::Measure(MeasureKind::Count));
        assert_ne!(role(&f, "paid"), FieldRole::Identifier);
        assert_ne!(role(&f, "valid"), FieldRole::Identifier);
        assert!(a.iter().all(|a| a.kind != AssumptionKind::IdentifierField));
    }

    #[test]
    fn few_values_categorical_matches_repeating_small_integers() {
        let d = ds(&[("rating", FieldType::Integer)], (0..40).map(|i| vec![Value::Int(i % 5 + 1)]).collect());
        let (f, a) = profile_fields(&d, None, &[]);
        assert_eq!(f[0].role.value, FieldRole::Category);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].kind, AssumptionKind::FewValuesCategorical);
        assert_eq!(a[0].treatment, Treatment::Silent);
        assert_eq!(a[0].line, "Colored as categories.");
    }

    #[test]
    fn few_values_categorical_does_not_match_many_values_or_decimals() {
        let d = ds(
            &[("students", FieldType::Integer), ("amount", FieldType::Double)],
            (0..40).map(|i| vec![Value::Int(100 + i * 3), Value::Float(1.5 + (i % 4) as f64)]).collect(),
        );
        let (f, a) = profile_fields(&d, None, &[]);
        assert_eq!(f[0].role.value, FieldRole::Measure(MeasureKind::Count));
        assert!(matches!(f[1].role.value, FieldRole::Measure(_)));
        assert!(a.iter().all(|a| a.kind != AssumptionKind::FewValuesCategorical));
        // Ten distinct values in ten rows (T4's amounts) are a measure, not categories.
        let d = ds(&[("金额", FieldType::Integer)], (0..10).map(|i| vec![Value::Int(i * 37 + 5)]).collect());
        let (f, _) = profile_fields(&d, None, &[]);
        assert!(matches!(f[0].role.value, FieldRole::Measure(_)));
    }

    #[test]
    fn roles_names_categories_dates_coordinates_and_stats() {
        let headers: Vec<String> = ["name", "type", "日期", "lon", "lat", "note", "名称"].iter().map(|s| s.to_string()).collect();
        let rows: Vec<Vec<String>> = (0..12)
            .map(|i| {
                vec![
                    format!("School {i}"),
                    ["Primary", "Middle", "High"][i % 3].to_string(),
                    format!("2024-03-{:02}", i + 1),
                    format!("121.{i}"),
                    format!("31.{i}"),
                    if i == 4 { String::new() } else { format!("free text number {i}") },
                    format!("站{i}"),
                ]
            })
            .collect();
        let coords = vec![
            CoordinateColumn { name: "lon".into(), confidence: Confidence::Likely },
            CoordinateColumn { name: "lat".into(), confidence: Confidence::Likely },
        ];
        let (f, a) = profile_raw_table(RawTable { headers: &headers, rows: &rows }, &coords);
        assert_eq!(role(&f, "name"), FieldRole::Name);
        assert_eq!(role(&f, "名称"), FieldRole::Name);
        assert_eq!(role(&f, "type"), FieldRole::Category);
        assert_eq!(role(&f, "日期"), FieldRole::Date);
        assert_eq!(f[2].dtype, DataType::Date);
        assert_eq!(role(&f, "lon"), FieldRole::Coordinate);
        assert_eq!(role(&f, "note"), FieldRole::Text);
        assert!(a.is_empty());
        let lon = f.iter().find(|f| f.name == "lon").and_then(|f| f.stats.clone()).unwrap();
        assert_eq!((lon.min, lon.max, lon.distinct, lon.nulls), (Some(121.0), Some(121.9), 11, 0));
        // "121.10" and "121.1" are the same number: 11 distinct values in 12 rows.
        let note = f.iter().find(|f| f.name == "note").and_then(|f| f.stats.clone()).unwrap();
        assert_eq!(note.nulls, 1);
    }

    #[test]
    fn stats_median_and_units() {
        let d = ds(&[("area_km2", FieldType::Double)], [4.0, 1.0, 3.0, 2.0].iter().map(|v| vec![Value::Float(*v)]).collect());
        let (f, _) = profile_fields(&d, None, &[]);
        let s = f[0].stats.clone().unwrap();
        assert_eq!((s.min, s.max, s.median, s.distinct), (Some(1.0), Some(4.0), Some(2.5), 4));
        assert_eq!(f[0].unit.value.as_deref(), Some("km²"));
    }

    #[test]
    fn dates_are_checked() {
        assert!(is_date("2024-02-29"));
        assert!(is_date("2024/3/5 10:00"));
        assert!(!is_date("2024-13-01"));
        assert!(!is_date("20240301"));
        assert!(!is_date("1.2.3"));
    }
}
