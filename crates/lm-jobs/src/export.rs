//! Data export (WP5.3, P1): a layer's records as GeoJSON or CSV — longitude and latitude columns
//! for points, or well-known text — or a raster band as GeoTIFF. Vector layers are held in WGS 84,
//! so that is what is written. Styles and recipes don't travel in these formats.
//!
//! CSV cells a spreadsheet would run as a formula (starting with =, +, - or @, or a tab or carriage
//! return) are written with a leading apostrophe (fixture X5); numbers are written as numbers.
//! Fields that look personal (phone numbers, email addresses, ID numbers) are found by
//! [`sensitive`] so the dialog can leave them out unless the user includes them.

use rustgis_engine::data::raster::Raster;
use rustgis_engine::data::{Dataset, FieldType, Geometry, Value};

use crate::import::LayerData;

/// What a layer can be written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFormat {
    GeoJson,
    /// CSV with longitude and latitude columns (points only).
    CsvLonLat,
    /// CSV with the shape as well-known text.
    CsvWkt,
    /// The raster band shown, as a GeoTIFF on its own grid.
    GeoTiff,
}

impl DataFormat {
    pub fn extension(self) -> &'static str {
        match self {
            DataFormat::GeoJson => "geojson",
            DataFormat::CsvLonLat | DataFormat::CsvWkt => "csv",
            DataFormat::GeoTiff => "tif",
        }
    }
}

/// A CSV cell's text, made safe for spreadsheets: a leading apostrophe on text a spreadsheet
/// would run as a formula.
pub fn neutralize(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() }
}

/// A CSV cell: neutralized, then quoted when it holds a comma, quote or line break.
fn cell(s: &str) -> String {
    let s = neutralize(s);
    quote(&s)
}

fn quote(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

/// A value as a CSV cell. Numbers are written as numbers (a negative number is not a formula);
/// text is neutralized.
fn value_cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) if f.is_finite() => format!("{f}"),
        Value::Float(_) => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => cell(s),
    }
}

/// A column name not already taken by one of `fields` ("longitude", else "longitude_wgs84").
fn free_name(base: &str, fields: &[String]) -> String {
    if fields.iter().any(|f| f.eq_ignore_ascii_case(base)) { format!("{base}_wgs84") } else { base.to_string() }
}

/// Whether every located record of `ds` is a single point (CSV can then carry longitude and
/// latitude columns).
pub fn all_points(ds: &Dataset) -> bool {
    ds.features.iter().all(|f| match &f.geometry {
        None | Some(Geometry::Point(_)) => true,
        Some(Geometry::MultiPoint(v)) => v.len() <= 1,
        Some(_) => false,
    })
}

fn point_of(g: Option<&Geometry>) -> Option<(f64, f64)> {
    match g? {
        Geometry::Point(c) => Some((c.x, c.y)),
        Geometry::MultiPoint(v) if v.len() == 1 => v.first().map(|c| (c.x, c.y)),
        _ => None,
    }
}

/// The records `rows` of `ds`, with only the fields named in `fields` (in the dataset's order).
fn subset(ds: &Dataset, rows: &[usize], fields: &[String]) -> Dataset {
    let keep: Vec<usize> = ds.fields.iter().enumerate().filter(|(_, f)| fields.contains(&f.name)).map(|(i, _)| i).collect();
    let mut out = ds.clone();
    out.fields = keep.iter().filter_map(|i| ds.fields.get(*i).cloned()).collect();
    out.features = rows
        .iter()
        .filter_map(|r| ds.features.get(*r))
        .map(|f| {
            let mut f = f.clone();
            f.attrs = keep.iter().map(|i| f.attrs.get(*i).cloned().unwrap_or(Value::Null)).collect();
            f
        })
        .collect();
    out
}

/// The records as a GeoJSON FeatureCollection.
pub fn geojson(ds: &Dataset, rows: &[usize], fields: &[String]) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&rustgis_engine::data::io::to_geojson(&subset(ds, rows, fields))).map_err(|e| e.to_string())
}

/// The records as CSV: the location first (longitude and latitude, or `wkt`), then the fields.
pub fn csv(ds: &Dataset, rows: &[usize], fields: &[String], wkt: bool) -> Result<String, String> {
    if !wkt && !all_points(ds) {
        return Err("Longitude and latitude columns work only for points. Use well-known text (WKT) for lines and areas.".into());
    }
    let sub = subset(ds, rows, fields);
    let names: Vec<String> = sub.fields.iter().map(|f| f.name.clone()).collect();
    let mut header: Vec<String> =
        if wkt { vec![free_name("wkt", &names)] } else { vec![free_name("longitude", &names), free_name("latitude", &names)] };
    header.extend(names.iter().cloned());
    let mut out = header.iter().map(|h| cell(h)).collect::<Vec<_>>().join(",");
    out.push('\n');
    for f in &sub.features {
        let mut cells: Vec<String> = if wkt {
            vec![f.geometry.as_ref().map(|g| quote(&rustgis_engine::data::interop::wkt(g))).unwrap_or_default()]
        } else {
            match point_of(f.geometry.as_ref()) {
                Some((x, y)) => vec![format!("{x}"), format!("{y}")],
                None => vec![String::new(), String::new()],
            }
        };
        cells.extend(f.attrs.iter().map(value_cell));
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    Ok(out)
}

/// A raster band as a GeoTIFF, on the raster's own grid and coordinate system (resampling to
/// WGS 84 would change its values).
pub fn geotiff(r: &Raster, band: usize) -> Result<Vec<u8>, String> {
    rustgis_engine::data::raster::geotiff::write(r, band).map_err(|e| e.to_string())
}

/// Writes a layer: vector `rows` (all when `None`) with `fields`, or raster `band` (0-based).
pub fn export(data: &LayerData, rows: Option<&[usize]>, fields: &[String], format: DataFormat, band: usize) -> Result<Vec<u8>, String> {
    match (data, format) {
        (LayerData::Raster(r), DataFormat::GeoTiff) => geotiff(r, band),
        (LayerData::Raster(_), _) => Err("Raster layers are exported as GeoTIFF.".into()),
        (LayerData::Vector(_), DataFormat::GeoTiff) => Err("GeoTIFF is for raster layers. Use GeoJSON or CSV.".into()),
        (LayerData::Vector(ds), f) => {
            let all: Vec<usize>;
            let rows = match rows {
                Some(r) => r,
                None => {
                    all = (0..ds.features.len()).collect();
                    &all
                }
            };
            match f {
                DataFormat::GeoJson => geojson(ds, rows, fields),
                DataFormat::CsvLonLat => csv(ds, rows, fields, false).map(String::into_bytes),
                _ => csv(ds, rows, fields, true).map(String::into_bytes),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fields that look personal

/// A kind of personal information a field seems to hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensitive {
    Phone,
    Email,
    IdNumber,
}

impl Sensitive {
    /// The note beside the unchecked field.
    pub fn note(self) -> &'static str {
        match self {
            Sensitive::Phone => "Looks like phone numbers",
            Sensitive::Email => "Looks like email addresses",
            Sensitive::IdNumber => "Looks like ID numbers",
        }
    }
}

/// The words of a field name: split at anything not a letter or digit and at lower-to-upper
/// changes ("contactTel" → contact, tel).
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_lowercase();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn by_name(name: &str) -> Option<Sensitive> {
    let w = words(name);
    let has = |s: &str| w.iter().any(|x| x == s);
    let joined = w.concat();
    let zh = |subs: &[&str]| subs.iter().any(|s| name.contains(s));
    if has("phone")
        || has("tel")
        || has("telephone")
        || has("mobile")
        || has("cellphone")
        || has("fax")
        || zh(&["电话", "手机", "联系方式"])
    {
        return Some(Sensitive::Phone);
    }
    if has("email") || has("mail") || joined.contains("email") || zh(&["邮箱", "电子邮件"]) {
        return Some(Sensitive::Email);
    }
    let id_words = ["ssn", "passport", "idcard", "idnumber", "idno", "nationalid", "socialsecurity", "identitycard"];
    let pairs = w.windows(2).map(|p| p.concat()).collect::<Vec<_>>();
    if id_words.iter().any(|s| has(s) || pairs.iter().any(|p| p == s) || joined == *s) || zh(&["身份证", "证件号", "学号", "护照"])
    {
        return Some(Sensitive::IdNumber);
    }
    None
}

fn is_email(s: &str) -> bool {
    let s = s.trim();
    match s.split_once('@') {
        Some((a, b)) => !a.is_empty() && b.contains('.') && !b.starts_with('.') && !b.ends_with('.') && !s.contains(char::is_whitespace),
        None => false,
    }
}

/// A Chinese mobile number (11 digits from 13–19), or a number written with phone punctuation
/// or a + (7–15 digits).
fn is_phone(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit() || " -+().".contains(c)) {
        return false;
    }
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    let cn_mobile = digits.len() == 11 && digits.starts_with('1') && digits.as_bytes().get(1).is_some_and(|d| (b'3'..=b'9').contains(d));
    let punctuated = (s.starts_with('+') || s.contains(['-', ' ', '(', ')'])) && (7..=15).contains(&digits.len());
    let digit_groups = s.split(|c: char| !c.is_ascii_digit()).filter(|g| !g.is_empty()).count();
    // Dates ("2024-07-01") and ranges are not phone numbers.
    let date_like = digit_groups == 3 && s.split('-').next().is_some_and(|y| y.len() == 4);
    (cn_mobile && digits.len() == s.chars().filter(|c| !c.is_whitespace() && *c != '-').count()) || (punctuated && !date_like)
}

/// A Chinese resident ID (18 characters: 17 digits and a digit or X, with a plausible birth
/// year), or a US social security number (123-45-6789).
fn is_id_number(s: &str) -> bool {
    let s = s.trim();
    let b = s.as_bytes();
    let cn = b.len() == 18
        && b[..17].iter().all(u8::is_ascii_digit)
        && b.get(17).is_some_and(|c| c.is_ascii_digit() || *c == b'X' || *c == b'x')
        && s.get(6..8).is_some_and(|y| y == "19" || y == "20");
    let ssn = b.len() == 11
        && b.get(3) == Some(&b'-')
        && b.get(6) == Some(&b'-')
        && b.iter().enumerate().all(|(i, c)| i == 3 || i == 6 || c.is_ascii_digit());
    cn || ssn
}

/// What personal information a field seems to hold, from its name or, failing that, from most of
/// its values (up to 200 of them looked at).
pub fn sensitive<'a>(name: &str, values: impl Iterator<Item = &'a Value>) -> Option<Sensitive> {
    if let Some(s) = by_name(name) {
        return Some(s);
    }
    let mut n = 0usize;
    let mut counts = [0usize; 3];
    for v in values {
        let text = match v {
            Value::Text(s) if !s.trim().is_empty() => s.clone(),
            Value::Int(i) => i.to_string(),
            _ => continue,
        };
        n += 1;
        counts[0] += usize::from(is_phone(&text));
        counts[1] += usize::from(is_email(&text));
        counts[2] += usize::from(is_id_number(&text));
        if n >= 200 {
            break;
        }
    }
    if n == 0 {
        return None;
    }
    let most = |c: usize| c * 10 >= n * 6;
    if most(counts[2]) {
        Some(Sensitive::IdNumber)
    } else if most(counts[1]) {
        Some(Sensitive::Email)
    } else if most(counts[0]) {
        Some(Sensitive::Phone)
    } else {
        None
    }
}

/// [`sensitive`] for each field of `ds`, in order.
pub fn sensitive_fields(ds: &Dataset) -> Vec<Option<Sensitive>> {
    ds.fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            // Measures (numbers that aren't whole) can't be phone or ID numbers.
            if f.ty == FieldType::Double {
                return by_name(&f.name);
            }
            sensitive(&f.name, ds.features.iter().filter_map(|feat| feat.attrs.get(i)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{Ctx, import_path};

    fn fixture(rel: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
    }

    fn vector(rel: &str) -> std::sync::Arc<Dataset> {
        let layers = import_path(&fixture(rel), &Ctx::detached(Default::default())).unwrap();
        match &layers[0].data {
            LayerData::Vector(ds) => ds.clone(),
            LayerData::Raster(_) => panic!("{rel} is a raster"),
        }
    }

    fn names(ds: &Dataset) -> Vec<String> {
        ds.fields.iter().map(|f| f.name.clone()).collect()
    }

    /// Fixture X5: every cell a spreadsheet would run is neutralized; numbers are left alone.
    #[test]
    fn x5_cells_that_look_like_formulas_are_neutralized() {
        let ds = vector("X5/x5_formulas.csv");
        let rows: Vec<usize> = (0..ds.features.len()).collect();
        let out = csv(&ds, &rows, &names(&ds), false).unwrap();
        let mut reader = ::csv::ReaderBuilder::new().has_headers(true).from_reader(out.as_bytes());
        let headers = reader.headers().unwrap().clone();
        let mut cells = 0;
        for rec in reader.records() {
            let rec = rec.unwrap();
            for (h, c) in headers.iter().zip(rec.iter()) {
                cells += 1;
                assert!(!c.starts_with(['=', '+', '-', '@']), "{h}: {c:?} would run as a formula");
            }
        }
        assert!(cells >= 25, "{cells} cells checked");
        for want in ["'=HYPERLINK(", "'+cmd", "'-2+3", "'@SUM(1)", "'=1+1", "'+SUM(A1:A2)", "'@me", "plain"] {
            assert!(out.contains(want), "{want} in\n{out}");
        }
        // Negative numbers stay numbers.
        let mut d = (*ds).clone();
        d.features[0].attrs = d.features[0].attrs.iter().map(|_| Value::Float(-3.5)).collect();
        let n = csv(&d, &[0], &names(&d), false).unwrap();
        assert!(n.lines().nth(1).unwrap().contains(",-3.5"), "{n}");
        assert_eq!(neutralize("\tx"), "'\tx");
        assert_eq!(neutralize("ok=1"), "ok=1");
    }

    #[test]
    fn geojson_round_trips_the_chosen_records_and_fields() {
        let ds = vector("V1/districts.geojson");
        let fields: Vec<String> = names(&ds).into_iter().take(2).collect();
        let rows = vec![0, 2, 5];
        let bytes = geojson(&ds, &rows, &fields).unwrap();
        let (back, _) = rustgis_engine::data::io::read_geojson(std::str::from_utf8(&bytes).unwrap(), "back").unwrap();
        assert_eq!(back.features.len(), 3);
        assert_eq!(names(&back), fields);
        for (k, r) in rows.iter().enumerate() {
            assert_eq!(back.features[k].geometry, ds.features[*r].geometry);
            for (i, f) in fields.iter().enumerate() {
                let j = ds.fields.iter().position(|x| &x.name == f).unwrap();
                assert_eq!(back.features[k].attrs[i].to_string(), ds.features[*r].attrs[j].to_string(), "{f}");
            }
        }
    }

    #[test]
    fn csv_round_trips_points_through_import() {
        let ds = vector("T1/schools.csv");
        let located: Vec<usize> = (0..ds.features.len()).filter(|r| ds.features[*r].geometry.is_some()).collect();
        let fields = names(&ds);
        let out = csv(&ds, &located, &fields, false).unwrap();
        let dir = std::env::temp_dir().join(format!("lm-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("schools export.csv");
        std::fs::write(&path, &out).unwrap();
        let back = vector(path.to_str().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back.features.len(), located.len());
        for (k, r) in located.iter().enumerate() {
            let (a, b) = (point_of(back.features[k].geometry.as_ref()).unwrap(), point_of(ds.features[*r].geometry.as_ref()).unwrap());
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "row {r}: {a:?} vs {b:?}");
        }
        // Lines and areas need WKT.
        let areas = vector("V1/districts.geojson");
        assert!(csv(&areas, &[0], &[], false).is_err());
    }

    #[test]
    fn wkt_csv_round_trips_areas() {
        let ds = vector("V1/districts.geojson");
        let out = csv(&ds, &[0, 1], &names(&ds), true).unwrap();
        let mut reader = ::csv::Reader::from_reader(out.as_bytes());
        assert_eq!(reader.headers().unwrap().get(0), Some("wkt"));
        for (k, rec) in reader.records().enumerate() {
            let rec = rec.unwrap();
            let (g, _) = rustgis_engine::data::interop::parse_wkt(rec.get(0).unwrap()).unwrap();
            assert_eq!(g, ds.features[k].geometry);
        }
    }

    #[test]
    fn rasters_export_as_geotiff_when_the_writer_can() {
        let layers = import_path(&fixture("R2/population_count.tif"), &Ctx::detached(Default::default())).unwrap();
        let bytes = export(&layers[0].data, None, &[], DataFormat::GeoTiff, 0).unwrap();
        assert!(bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*"));
        assert!(export(&layers[0].data, None, &[], DataFormat::GeoJson, 0).is_err());
    }

    #[test]
    fn personal_fields_are_found_by_name_or_values() {
        let t = |s: &str| Value::Text(s.into());
        assert_eq!(sensitive("Phone", std::iter::empty()), Some(Sensitive::Phone));
        assert_eq!(sensitive("contactTel", std::iter::empty()), Some(Sensitive::Phone));
        assert_eq!(sensitive("联系电话", std::iter::empty()), Some(Sensitive::Phone));
        assert_eq!(sensitive("E-mail", std::iter::empty()), Some(Sensitive::Email));
        assert_eq!(sensitive("id_number", std::iter::empty()), Some(Sensitive::IdNumber));
        assert_eq!(sensitive("身份证号", std::iter::empty()), Some(Sensitive::IdNumber));
        // A record id, a hotel name and a district code are not personal.
        for name in ["id", "district_id", "hotel", "name", "code"] {
            assert_eq!(sensitive(name, [t("A1"), t("B2")].iter()), None, "{name}");
        }
        let phones = [t("138 0013 8000"), t("+86 21 6888 1234"), t("021-6888-1234"), t("")];
        assert_eq!(sensitive("contact", phones.iter()), Some(Sensitive::Phone));
        let mobiles = [Value::Int(13800138000), Value::Int(15900001111)];
        assert_eq!(sensitive("c", mobiles.iter()), Some(Sensitive::Phone));
        let mails = [t("li.wei@example.edu"), t("a@b.cn")];
        assert_eq!(sensitive("contact", mails.iter()), Some(Sensitive::Email));
        let ids = [t("11010519491231002X"), t("310104200001011234")];
        assert_eq!(sensitive("number", ids.iter()), Some(Sensitive::IdNumber));
        // Dates, counts and plain codes are not.
        let other = [t("2024-07-01"), t("2023-12-31")];
        assert_eq!(sensitive("when", other.iter()), None);
        assert_eq!(sensitive("pop", [Value::Int(1_250_000), Value::Int(870_000)].iter()), None);
        // The schools fixture has no personal fields.
        assert!(sensitive_fields(&vector("T1/schools.csv")).iter().all(Option::is_none));
    }
}
