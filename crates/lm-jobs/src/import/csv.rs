//! Delimited text (CSV, TSV): encoding and delimiter sniffed from the first 64 KiB, rows streamed,
//! every cell kept as written (leading zeros, dates and decimal commas stay as they are) with the
//! inferred types beside them. Where the records are comes from `lm_profile::csv_locate`.

use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use lm_core::model::{Assumption, AssumptionKind, Choice, Crs, Exclusion, Known, LayerKind, LayerProfile};
use lm_profile::{CoordinatePair, CsvLocation, NumberStyle, RawTable};
use rustgis_engine::data::geometry::Geometry;
use rustgis_engine::data::{Dataset, Field, FieldType, GeometryType, Value};
use rustgis_engine::geom::Coord;

use super::progress::{Counting, Decoding};
use super::{
    Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, RawRows, Stage, extension, file_name, source_ref, status_of,
};

pub struct Delimited;

/// How much of the file is looked at to sniff the encoding and delimiter.
const HEAD: usize = 64 << 10;

impl FormatAdapter for Delimited {
    fn name(&self) -> &'static str {
        "CSV"
    }

    fn sniff(&self, path: &Path, head: &[u8]) -> bool {
        match extension(path).as_str() {
            "csv" | "tsv" | "tab" => true,
            "txt" | "" => rustgis_engine::data::io::sniff_extension(head) == Some("csv") || looks_delimited_gbk(head),
            _ => false,
        }
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        let open = || std::fs::File::open(path).map_err(|e| ImportError::read(&name, e));
        let mut head = Vec::with_capacity(HEAD);
        open()?.take(HEAD as u64).read_to_end(&mut head).map_err(|e| ImportError::read(&name, e))?;
        let (mut encoding, bom) = detect_encoding(&head);
        let head_text = encoding.decode_without_bom_handling(head.get(bom..).unwrap_or_default()).0;
        let delimiter =
            if matches!(extension(path).as_str(), "tsv" | "tab") { b'\t' } else { rustgis_engine::data::io::sniff_delimiter(&head_text) };
        // A file that looked like UTF-8 in its first 64 KiB but isn't further on is read again
        // as Chinese (GBK) rather than failing or losing characters.
        let (headers, rows) = match read_rows(path, &name, encoding, bom, delimiter, meta.size, ctx) {
            Err(ImportError::Failed { details, .. })
                if encoding == encoding_rs::UTF_8 && details.to_ascii_lowercase().contains("utf-8") =>
            {
                encoding = encoding_rs::GB18030;
                read_rows(path, &name, encoding, bom, delimiter, meta.size, ctx)?
            }
            other => other?,
        };
        // Rows longer than the header keep their extra cells, in numbered columns.
        let mut headers = headers.unwrap_or_default();
        let widest = rows.iter().map(Vec::len).max().unwrap_or(0);
        if widest > headers.len() {
            headers.resize(widest, String::new());
        }
        let headers = dedupe_headers(headers);
        if headers.is_empty() {
            return Err(ImportError::Failed {
                message: format!("{name} is empty. Nothing was added to your map."),
                details: "no header row".into(),
            });
        }
        ctx.stage(Stage::Extent)?;
        let raw = RawRows { headers, rows, delimiter: Some(char::from(delimiter)) };
        let mut notices = Vec::new();
        if encoding == encoding_rs::WINDOWS_1252 {
            notices.push("The text isn't UTF-8 or Chinese (GBK); it was read as Western (Windows-1252).".into());
        }
        let enc_label = if encoding == encoding_rs::GB18030 { "GBK".to_string() } else { encoding.name().to_string() };
        let location = lm_profile::csv_locate_delimited(&raw.headers, &raw.rows, &name, raw.delimiter);
        ctx.stage(Stage::Profile)?;
        let origin = TableOrigin {
            path,
            member: None,
            name: path.file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string(),
            format: "CSV".into(),
            encoding: Some(enc_label),
            notices,
            assumptions: Vec::new(),
        };
        let layer = build(origin, &raw, location)?;
        Ok(vec![ImportedLayer { raw: Some(Arc::new(raw)), ..layer }])
    }
}

/// Reads every row of a delimited file as raw cells: the header row, then the data rows.
fn read_rows(
    path: &Path,
    name: &str,
    encoding: &'static encoding_rs::Encoding,
    bom: usize,
    delimiter: u8,
    size: u64,
    ctx: &Ctx,
) -> Result<(Option<Vec<String>>, Vec<Vec<String>>), ImportError> {
    ctx.ticket.set_total(size);
    let f = std::fs::File::open(path).map_err(|e| ImportError::read(name, e))?;
    let mut file = Counting::new(std::io::BufReader::new(f), ctx.ticket.clone());
    let mut skip = vec![0u8; bom];
    file.read_exact(&mut skip).map_err(|e| ImportError::read(name, e))?;
    let reader: Box<dyn Read> = if encoding == encoding_rs::UTF_8 { Box::new(file) } else { Box::new(Decoding::new(file, encoding)) };
    let mut csv = ::csv::ReaderBuilder::new().delimiter(delimiter).flexible(true).has_headers(false).from_reader(reader);
    let mut headers: Option<Vec<String>> = None;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut record = ::csv::StringRecord::new();
    loop {
        match csv.read_record(&mut record) {
            Ok(false) => break,
            Ok(true) => {
                let cells: Vec<String> = record.iter().map(str::to_string).collect();
                match &headers {
                    None => headers = Some(cells),
                    Some(_) if cells.iter().all(|c| c.trim().is_empty()) => {}
                    Some(_) => rows.push(cells),
                }
                if rows.len().is_multiple_of(50_000) {
                    ctx.ticket.check()?;
                }
            }
            Err(e) => return Err(ImportError::read(name, e)),
        }
    }
    Ok((headers, rows))
}

/// GBK text a byte sniffer would miss (it isn't UTF-8): two lines split alike by a separator.
fn looks_delimited_gbk(head: &[u8]) -> bool {
    let (text, errors) = encoding_rs::GBK.decode_without_bom_handling(head);
    if errors {
        return false;
    }
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let (Some(a), Some(b)) = (lines.next(), lines.next()) else { return false };
    [',', ';', '\t', '|'].iter().any(|&c| a.matches(c).count() > 0 && a.matches(c).count() == b.matches(c).count())
}

/// The text encoding of a file from its first bytes, and the length of its byte-order mark.
fn detect_encoding(head: &[u8]) -> (&'static encoding_rs::Encoding, usize) {
    if head.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (encoding_rs::UTF_8, 3);
    }
    match std::str::from_utf8(head) {
        Ok(_) => return (encoding_rs::UTF_8, 0),
        // A character cut at the end of the sample is fine.
        Err(e) if e.error_len().is_none() => return (encoding_rs::UTF_8, 0),
        Err(_) => {}
    }
    let mut d = encoding_rs::GBK.new_decoder_without_bom_handling();
    let mut s = String::with_capacity(head.len() * 2);
    let (r, _) = d.decode_to_string_without_replacement(head, &mut s, false);
    if matches!(r, encoding_rs::DecoderResult::InputEmpty) {
        // GB18030 decodes everything GBK does (and its four-byte forms).
        return (encoding_rs::GB18030, 0);
    }
    (encoding_rs::WINDOWS_1252, 0)
}

/// Blank or repeated header names get a number, so every field can be told apart.
pub(super) fn dedupe_headers(h: Vec<String>) -> Vec<String> {
    use std::collections::{HashMap, HashSet};
    let mut out: Vec<String> = Vec::with_capacity(h.len());
    // Names taken so far, and for each base name the next number to try: names are only ever
    // added, so the numbers below it are taken. (Searching `out` for every candidate was cubic
    // in a header of repeated names; fuzz finding `csv/timeout-5530a57f`.)
    let mut taken: HashSet<String> = HashSet::with_capacity(h.len());
    let mut next: HashMap<String, usize> = HashMap::new();
    for (i, name) in h.into_iter().enumerate() {
        let base = name.trim().trim_start_matches('\u{feff}').to_string();
        let base = if base.is_empty() { format!("column {}", i + 1) } else { base };
        let mut n = base.clone();
        let k = next.entry(base.clone()).or_insert(2);
        while taken.contains(&n) {
            n = format!("{base} {k}");
            *k += 1;
        }
        taken.insert(n.clone());
        out.push(n);
    }
    out
}

/// The number style of column `col` ([`lm_profile::number_style`]: what a comma means there).
fn column_style(raw: &RawRows, col: usize) -> NumberStyle {
    lm_profile::number_style(raw.rows.iter().map(|r| r.get(col).map_or("", String::as_str)), raw.delimiter).value
}

/// A whole number as written in `style` ("12,500" with thousands separators).
fn integer(c: &str, style: NumberStyle) -> Option<i64> {
    if style == NumberStyle::Thousands && c.contains(',') && !c.contains('.') {
        lm_profile::parse_number_with(c, style)?;
        return c.replace(',', "").parse().ok();
    }
    c.parse().ok()
}

/// A column's inferred type from its cells as written, in its number style. Leading zeros keep
/// a column as text, and so do commas whose meaning can't be told.
fn infer_type(raw: &RawRows, col: usize, style: NumberStyle) -> FieldType {
    let mut ty = None::<FieldType>;
    for r in &raw.rows {
        let Some(c) = r.get(col).map(|c| c.trim()).filter(|c| !c.is_empty()) else { continue };
        let this = if c.len() > 1 && c.starts_with('0') && !c.starts_with("0.") && !c.starts_with("0,") {
            FieldType::Text
        } else if integer(c, style).is_some() {
            FieldType::Integer
        } else if lm_profile::parse_number_with(c, style).is_some() {
            FieldType::Double
        } else if c.eq_ignore_ascii_case("true") || c.eq_ignore_ascii_case("false") {
            FieldType::Boolean
        } else {
            FieldType::Text
        };
        ty = Some(match (ty, this) {
            (None, t) => t,
            (Some(a), b) if a == b => a,
            (Some(FieldType::Integer), FieldType::Double) | (Some(FieldType::Double), FieldType::Integer) => FieldType::Double,
            _ => FieldType::Text,
        });
        if ty == Some(FieldType::Text) {
            break;
        }
    }
    ty.unwrap_or(FieldType::Text)
}

fn typed(cell: &str, ty: FieldType, style: NumberStyle) -> Value {
    let c = cell.trim();
    if c.is_empty() {
        return Value::Null;
    }
    match ty {
        FieldType::Integer => integer(c, style).map(Value::Int).unwrap_or_else(|| Value::Text(cell.to_string())),
        FieldType::Double => lm_profile::parse_number_with(c, style).map(Value::Float).unwrap_or_else(|| Value::Text(cell.to_string())),
        FieldType::Boolean => Value::Bool(c.eq_ignore_ascii_case("true")),
        _ => Value::Text(cell.to_string()),
    }
}

/// Why a row has no usable location, or its point.
fn locate_row(row: &[String], pair: &CoordinatePair, styles: &[NumberStyle], check_range: bool) -> Result<Coord, String> {
    let style = |i: usize| styles.get(i).copied().unwrap_or(NumberStyle::Point);
    let cell = |i: usize| row.get(i).map_or("", |c| c.trim());
    let (xs, ys) = (cell(pair.x_index), cell(pair.y_index));
    if xs.is_empty() || ys.is_empty() {
        let blank = if xs.is_empty() { &pair.x } else { &pair.y };
        return Err(format!("No location: {blank} is blank"));
    }
    let x = lm_profile::parse_number_with(xs, style(pair.x_index))
        .ok_or_else(|| format!("No location: {} is not a number (\"{xs}\")", pair.x))?;
    let y = lm_profile::parse_number_with(ys, style(pair.y_index))
        .ok_or_else(|| format!("No location: {} is not a number (\"{ys}\")", pair.y))?;
    if check_range && !((-180.0..=180.0).contains(&x) && (-90.0..=90.0).contains(&y)) {
        return Err(format!("No location: {x}, {y} is outside longitude −180 to 180 or latitude −90 to 90"));
    }
    Ok(Coord::new(x, y))
}

/// Where a table of raw cells came from: a CSV file, or one sheet of a workbook.
pub(super) struct TableOrigin<'a> {
    pub path: &'a Path,
    /// The sheet of a workbook.
    pub member: Option<String>,
    /// The layer's display name.
    pub name: String,
    /// "CSV" or "Excel".
    pub format: String,
    pub encoding: Option<String>,
    pub notices: Vec<String>,
    /// Assumptions made before the records were located (which sheet), listed first.
    pub assumptions: Vec<Assumption>,
}

/// Builds the layer from raw cells and a location decision (CSV and Excel share it).
pub(super) fn build(origin: TableOrigin, raw: &RawRows, location: CsvLocation) -> Result<ImportedLayer, ImportError> {
    let (pair, lonlat) = match &location {
        CsvLocation::Located { pair, .. } => (Some(pair.clone()), true),
        CsvLocation::Ambiguous { candidates, recommended, .. } => (candidates.get(*recommended).cloned(), true),
        // Projected values are kept as written until "Where should this data be?" is answered.
        CsvLocation::Projected { pair, .. } => (Some(pair.clone()), false),
        CsvLocation::Table { .. } => (None, false),
    };
    build_with(origin, raw, location, pair, lonlat)
}

/// [`build`] with the coordinate pair given: `pair` is used for the geometry (`None` opens the
/// rows as a table); `lonlat` says whether its values are degrees.
fn build_with(
    origin: TableOrigin,
    raw: &RawRows,
    location: CsvLocation,
    pair: Option<CoordinatePair>,
    lonlat: bool,
) -> Result<ImportedLayer, ImportError> {
    let TableOrigin { path, member, name, format, encoding, notices, assumptions: before } = origin;
    let styles: Vec<NumberStyle> = (0..raw.headers.len()).map(|i| column_style(raw, i)).collect();
    let types: Vec<FieldType> = styles.iter().enumerate().map(|(i, s)| infer_type(raw, i, *s)).collect();
    let fields: Vec<Field> = raw.headers.iter().zip(&types).map(|(h, t)| Field::new(h.clone(), *t)).collect();
    let mut ds = Dataset::new(name.clone(), if pair.is_some() { GeometryType::Point } else { GeometryType::None }, fields);
    ds.source = Some(path.display().to_string());
    let mut exclusions = Vec::new();
    for (i, row) in raw.rows.iter().enumerate() {
        let attrs: Vec<Value> =
            types.iter().zip(&styles).enumerate().map(|(c, (t, s))| row.get(c).map_or(Value::Null, |cell| typed(cell, *t, *s))).collect();
        let geom = match &pair {
            Some(p) => match locate_row(row, p, &styles, lonlat) {
                Ok(c) => Some(Geometry::Point(c)),
                Err(reason) => {
                    exclusions.push(Exclusion { row: i as u64 + 1, reason });
                    None
                }
            },
            None => None,
        };
        ds.push_next(geom, attrs);
    }
    let mut assumptions: Vec<Assumption> = before;
    assumptions.extend(location.assumption().into_iter().cloned());
    let raw_table = RawTable { headers: &raw.headers, rows: &raw.rows, delimiter: raw.delimiter };
    let (field_profiles, field_assumptions) = lm_profile::profile_fields(&ds, Some(raw_table), &location.coordinate_columns());
    assumptions.extend(field_assumptions);
    let extent = super::extent_of(&ds);
    let kind = if pair.is_some() && lonlat && extent.is_some() { LayerKind::Points } else { LayerKind::Table };
    let crs = if !lonlat {
        if pair.is_some() {
            Known::unknown(Crs::Unknown, "The values aren't degrees and the file doesn't say.")
        } else {
            Known::unknown(Crs::Unknown, "No coordinate columns.")
        }
    } else {
        // csv_locate's line already says "as GPS coordinates (WGS 84)".
        Known::likely(Crs::Wgs84, "Coordinate columns with values within ±180 / ±90.")
    };
    if lonlat && pair.is_some() {
        // A sheet's name hints at the source as much as its file's.
        let file = match &member {
            Some(sheet) => format!("{} {sheet}", file_name(path)),
            None => file_name(path),
        };
        if let Some(a) = lm_profile::china_offset(&super::sample_points(&ds, 2000), &crs, &file, &raw.headers) {
            assumptions.push(a);
        }
    }
    let mut notices = notices;
    if let CsvLocation::Table { reason } = &location {
        notices.push(reason.clone());
    }
    let profile = LayerProfile {
        source: source_ref(path, member),
        format,
        kind,
        count: super::located_count(&ds, kind),
        extent_wgs84: if kind == LayerKind::Points { extent } else { None },
        crs,
        fields: field_profiles,
        bands: Vec::new(),
        assumptions,
        // A table's rows aren't excluded: they open as rows.
        exclusions: if kind == LayerKind::Table && pair.is_none() { Vec::new() } else { exclusions },
        encoding,
        notices,
        data_rev: 0,
    };
    Ok(ImportedLayer { name, status: status_of(&profile), profile, data: LayerData::Vector(Arc::new(ds)), raw: None })
}

/// Rebuilds a CSV (or Excel sheet) layer with the columns the user chose (`columns:x,y`), or as a
/// table (Not sure). Its format, sheet and sheet choice are kept.
pub fn relocate(layer: &ImportedLayer, raw: &Arc<RawRows>, choice: &Choice) -> Result<ImportedLayer, ImportError> {
    let path = Path::new(&layer.profile.source.path);
    let mut location = lm_profile::csv_locate_delimited(&raw.headers, &raw.rows, layer.profile.source.file_name(), raw.delimiter);
    let chosen_pair = choice.id.strip_prefix("columns:").and_then(|cols| {
        let (x, y) = cols.split_once(',')?;
        let xi = raw.headers.iter().position(|h| h == x)?;
        let yi = raw.headers.iter().position(|h| h == y)?;
        Some((x.to_string(), y.to_string(), xi, yi))
    });
    let (pair, lonlat) = match chosen_pair {
        Some((x, y, x_index, y_index)) => {
            let p = CoordinatePair {
                x,
                y,
                x_index,
                y_index,
                swapped: false,
                rows_nonblank: 0,
                rows_numeric: 0,
                rows_in_range: 0,
                blank_rows: vec![],
                swapped_rows: vec![],
                out_of_range_rows: vec![],
                extent: None,
            };
            (Some(p), true)
        }
        None => (None, false),
    };
    // Record the answer on the question (or visible line) it settles.
    if let Some(a) = match &mut location {
        CsvLocation::Located { assumption, .. } | CsvLocation::Ambiguous { assumption, .. } | CsvLocation::Projected { assumption, .. } => {
            Some(assumption)
        }
        CsvLocation::Table { .. } => None,
    } {
        a.chosen = choice.clone();
        a.confirmed_by_user = true;
        if !a.alternatives.contains(choice) {
            a.alternatives.insert(0, choice.clone());
        }
    }
    let origin = TableOrigin {
        path,
        member: layer.profile.source.member.clone(),
        name: layer.name.clone(),
        format: layer.profile.format.clone(),
        encoding: layer.profile.encoding.clone(),
        // The import's notices stay (encoding, preview size…).
        notices: layer.profile.notices.clone(),
        assumptions: layer.profile.assumptions.iter().filter(|a| a.kind == AssumptionKind::ExcelSheet).cloned().collect(),
    };
    let mut out = build_with(origin, raw, location, pair, lonlat)?;
    let mut seen = std::collections::HashSet::new();
    out.profile.notices.retain(|n| seen.insert(n.clone()));
    out.raw = Some(raw.clone());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_are_told_apart() {
        assert_eq!(detect_encoding(b"\xEF\xBB\xBFa,b\n1,2"), (encoding_rs::UTF_8, 3));
        assert_eq!(detect_encoding("名称,经度\n人民广场,121.4".as_bytes()), (encoding_rs::UTF_8, 0));
        let (gbk, _, _) = encoding_rs::GBK.encode("名称;经度\n人民广场;121,4");
        assert_eq!(detect_encoding(&gbk).0, encoding_rs::GB18030);
        assert_eq!(detect_encoding(b"caf\xE9;1\n").0, encoding_rs::WINDOWS_1252);
    }

    #[test]
    fn leading_zeros_keep_a_column_as_text() {
        let raw = RawRows {
            headers: vec!["code".into(), "n".into(), "x".into()],
            rows: vec![vec!["007".into(), "3".into(), "1,5".into()], vec!["120".into(), "4".into(), "2".into()]],
            delimiter: Some(','),
        };
        let ty = |i: usize| infer_type(&raw, i, column_style(&raw, i));
        assert_eq!(ty(0), FieldType::Text);
        assert_eq!(ty(1), FieldType::Integer);
        assert_eq!(ty(2), FieldType::Double);
        assert_eq!(typed("007", FieldType::Text, NumberStyle::Point), Value::Text("007".into()));
    }

    #[test]
    fn number_styles_type_columns() {
        let col = |cells: &[&str], delimiter| RawRows {
            headers: vec!["n".into()],
            rows: cells.iter().map(|c| vec![c.to_string()]).collect(),
            delimiter,
        };
        let raw = col(&["12,500", "1,250,000", "980"], Some(','));
        let style = column_style(&raw, 0);
        assert_eq!((style, infer_type(&raw, 0, style)), (NumberStyle::Thousands, FieldType::Integer));
        assert_eq!(typed("1,250,000", FieldType::Integer, style), Value::Int(1_250_000));
        let raw = col(&["12,500.5", "3,400"], Some(','));
        let style = column_style(&raw, 0);
        assert_eq!(infer_type(&raw, 0, style), FieldType::Double);
        assert_eq!(typed("3,400", FieldType::Double, style), Value::Float(3400.0));
        let raw = col(&["12,500", "3,400"], Some(','));
        assert_eq!(infer_type(&raw, 0, column_style(&raw, 0)), FieldType::Text);
        let raw = col(&["12,500", "3,400"], Some(';'));
        let style = column_style(&raw, 0);
        assert_eq!((style, infer_type(&raw, 0, style)), (NumberStyle::DecimalComma, FieldType::Double));
        assert_eq!(typed("12,500", FieldType::Double, style), Value::Float(12.5));
    }

    #[test]
    fn repeated_and_blank_headers_are_numbered() {
        assert_eq!(dedupe_headers(vec!["a".into(), "".into(), "a".into()]), vec!["a", "column 2", "a 2"]);
        // A name already taken by a numbered one is skipped over, as before.
        let h = ["a", "a 2", "a", "a", " a 3 "].map(String::from).to_vec();
        assert_eq!(dedupe_headers(h), vec!["a", "a 2", "a 3", "a 4", "a 3 2"]);
    }

    /// Fuzz finding (Gate 2, `csv/timeout-5530a57f`): thousands of columns. Numbering a header
    /// of repeated names searched every name taken for every candidate (cubic).
    #[test]
    fn thousands_of_repeated_headers_are_numbered_quickly() {
        let t = std::time::Instant::now();
        let out = dedupe_headers(vec!["x".to_string(); 20_000]);
        assert!(t.elapsed().as_secs_f64() < 1.0, "{:?}", t.elapsed());
        assert_eq!((out[0].as_str(), out[1].as_str(), out[19_999].as_str()), ("x", "x 2", "x 20000"));
    }
}
