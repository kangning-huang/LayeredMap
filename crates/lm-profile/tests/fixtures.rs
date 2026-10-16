//! The catalog functions on the hand-made fixtures (`fixtures/<ID>/`), compared with what their
//! `expected.json` states: assumptions that should fire (ids mapped to catalog rows below) and
//! field roles. An `expected.json` lists the assumptions that must be shown; the catalog may add
//! others (a China offset notice, "Unit unknown"), which these tests allow.
//!
//! Expected ids → catalog rows: `crs_wgs84_assumed` → CsvCoordinateColumns (CSV) or
//! NoCrsLonLat (Shapefile); `crs_unknown_projected` → NoCrsProjected; `encoding_detected` →
//! ShapefileGbk (Shapefile; there is no catalog row for CSV encodings); `sheet_chosen` →
//! ExcelSheet; `offset_system_*` → ChinaOffsetNamed; `nodata_assumed` → RasterNoData.
//! `decimal_comma` has no catalog row.

use lm_core::model::{
    Assumption, AssumptionKind, CellMeaning, Confidence, Crs, Extent, FieldProfile, FieldRole, Known, LayerKind, Treatment,
};
use lm_profile::*;
use rustgis_data::{Dataset, Value};
use serde_json::Value as Json;
use std::path::PathBuf;

fn dir(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(id)
}

fn expected(id: &str) -> Json {
    let text = std::fs::read_to_string(dir(id).join("expected.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// A small CSV reader (quotes, any delimiter) independent of the code under test.
fn split_csv(text: &str, delim: char) -> (Vec<String>, Vec<Vec<String>>) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (mut cells, mut cur, mut quoted) = (Vec::new(), String::new(), false);
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' if quoted && chars.peek() == Some(&'"') => {
                    cur.push('"');
                    chars.next();
                }
                '"' => quoted = !quoted,
                c if c == delim && !quoted => cells.push(std::mem::take(&mut cur)),
                c => cur.push(c),
            }
        }
        cells.push(cur);
        rows.push(cells);
    }
    let headers = rows.remove(0);
    (headers, rows)
}

fn read_csv(id: &str, file: &str) -> (Vec<String>, Vec<Vec<String>>) {
    split_csv(&std::fs::read_to_string(dir(id).join(file)).unwrap(), ',')
}

/// The expected role of a field, as a `FieldRole` and whether the fixture marks it ambiguous.
fn expected_role(s: &str) -> (FieldRole, bool) {
    let ambiguous = s.ends_with('?');
    let role = match s.trim_end_matches('?') {
        "id" | "code" => FieldRole::Identifier,
        "name" => FieldRole::Name,
        "category" => FieldRole::Category,
        "date" => FieldRole::Date,
        "longitude" | "latitude" => FieldRole::Coordinate,
        "measure" => FieldRole::Measure(lm_core::model::MeasureKind::Unknown),
        other => panic!("unknown role {other}"),
    };
    (role, ambiguous)
}

fn check_fields(id: &str, fields: &[FieldProfile]) {
    let exp = expected(id);
    let Some(map) = exp.get("fields").and_then(Json::as_object) else { return };
    for (name, role) in map {
        let (want, ambiguous) = expected_role(role.as_str().unwrap());
        let got = fields.iter().find(|f| &f.name == name).unwrap_or_else(|| panic!("{id}: no field {name}"));
        let same = match (want, got.role.value) {
            (FieldRole::Measure(_), FieldRole::Measure(_)) => true,
            (a, b) => a == b,
        };
        assert!(same, "{id}: field {name}: expected {role}, got {:?} ({})", got.role.value, got.role.evidence.0);
        if ambiguous {
            assert!(got.role.confidence >= Confidence::Possible, "{id}: {name} should be uncertain");
        }
    }
}

fn kinds(a: &[Assumption]) -> Vec<AssumptionKind> {
    a.iter().map(|a| a.kind).collect()
}

fn shown(a: &[Assumption]) -> Vec<AssumptionKind> {
    a.iter().filter(|a| matches!(a.treatment, Treatment::Visible | Treatment::Question)).map(|a| a.kind).collect()
}

fn expected_ids(id: &str) -> Vec<String> {
    expected(id)
        .get("assumptions")
        .and_then(Json::as_array)
        .map(|a| a.iter().filter_map(|x| x.get("id").and_then(Json::as_str).map(String::from)).collect())
        .unwrap_or_default()
}

fn raw_extent(ds: &Dataset) -> Option<Extent> {
    let e = ds.extent();
    (e.min_x <= e.max_x).then_some(Extent { min_x: e.min_x, min_y: e.min_y, max_x: e.max_x, max_y: e.max_y })
}

fn points(ds: &Dataset) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for g in ds.features.iter().filter_map(|f| f.geometry.as_ref()) {
        g.for_each_coord(|c| out.push((c.x, c.y)));
    }
    out
}

fn csv_points(loc: &CsvLocation, rows: &[Vec<String>]) -> Vec<(f64, f64)> {
    let CsvLocation::Located { pair, .. } = loc else { return vec![] };
    rows.iter().filter_map(|r| parse_number(r.get(pair.x_index)?).zip(parse_number(r.get(pair.y_index)?))).collect()
}

/// Everything a CSV import produces: location, field profiles and assumptions.
fn profile_csv(file: &str, headers: &[String], rows: &[Vec<String>]) -> (CsvLocation, Vec<FieldProfile>, Vec<Assumption>) {
    let loc = csv_locate(headers, rows, file);
    let (fields, mut a) = profile_raw_table(RawTable { headers, rows }, &loc.coordinate_columns());
    let mut all: Vec<Assumption> = loc.assumption().cloned().into_iter().collect();
    if matches!(loc, CsvLocation::Located { .. }) {
        let crs = Known::likely(Crs::Wgs84, "lon/lat columns");
        all.extend(china_offset(&csv_points(&loc, rows), &crs, file, headers));
    }
    all.append(&mut a);
    (loc, fields, all)
}

#[test]
fn v1_districts_declared_crs_no_assumptions() {
    let (ds, _) = rustgis_data::io::read_path(&dir("V1").join("districts.geojson")).unwrap();
    let (fields, a) = profile_fields(&ds, None, &[]);
    check_fields("V1", &fields);
    // GeoJSON declares WGS 84 (RFC 7946).
    let (crs, crs_a) = crs_from_values(raw_extent(&ds), Some(Crs::Wgs84));
    assert!(crs_a.is_empty());
    assert_eq!(china_offset(&points(&ds), &crs, "districts.geojson", &[]), None);
    let k: Vec<LayerKind> = geometry_kinds(&ds).iter().map(|(k, _)| *k).collect();
    assert_eq!(mixed_geometry_split(&k), None);
    assert!(shown(&a).is_empty());
    assert!(expected_ids("V1").is_empty());
    // population: 16 values, 1 missing.
    let pop = fields.iter().find(|f| f.name == "population").and_then(|f| f.stats.clone()).unwrap();
    assert_eq!(pop.nulls, 1);
}

#[test]
fn v2_mixed_geometry_split_into_three_layers() {
    let (ds, _) = rustgis_data::io::read_path(&dir("V2").join("mixed.geojson")).unwrap();
    let kinds_found = geometry_kinds(&ds);
    assert_eq!(kinds_found, vec![(LayerKind::Points, 4), (LayerKind::Lines, 3), (LayerKind::Areas, 3)]);
    assert_eq!(expected("V2")["split_into_layers"], 3);
    let k: Vec<LayerKind> = kinds_found.iter().map(|(k, _)| *k).collect();
    let a = mixed_geometry_split(&k).unwrap();
    assert_eq!(a.line, "Split into 3 layers: points, lines, areas");
    assert_eq!(a.treatment, Treatment::Silent);
    let (fields, _) = profile_fields(&ds, None, &[]);
    check_fields("V2", &fields);
}

#[test]
fn v3_shapefile_without_prj_in_degrees_assumes_wgs84() {
    let (ds, _) = rustgis_data::io::read_path(&dir("V3").join("sites.shp")).unwrap();
    let (crs, a) = crs_from_values(raw_extent(&ds), None);
    assert_eq!(crs.value, Crs::Wgs84);
    assert_eq!(kinds(&a), vec![AssumptionKind::NoCrsLonLat]);
    assert_eq!(a[0].treatment, Treatment::Visible);
    assert_eq!(expected_ids("V3"), vec!["crs_wgs84_assumed"]);
    // Shanghai, assumed WGS 84: the catalog also shows the China offset notice.
    let offset = china_offset(&points(&ds), &crs, "sites.shp", &[]).unwrap();
    assert_eq!(offset.kind, AssumptionKind::ChinaOffsetPossible);
    let (fields, _) = profile_fields(&ds, None, &[]);
    check_fields("V3", &fields);
}

#[test]
fn v4_shapefile_without_prj_projected_asks_where() {
    let (ds, _) = rustgis_data::io::read_path(&dir("V4").join("districts_gk.shp")).unwrap();
    let (crs, a) = crs_from_values(raw_extent(&ds), None);
    assert_eq!(crs.value, Crs::Unknown);
    assert_eq!(kinds(&a), vec![AssumptionKind::NoCrsProjected]);
    assert_eq!(a[0].line, "Where should this data be?");
    assert_eq!(expected_ids("V4"), vec!["crs_unknown_projected"]);
    let exp = expected("V4");
    let b: Vec<f64> = exp["projected_bounds"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let e = raw_extent(&ds).unwrap();
    assert!((e.min_x - b[0]).abs() < 1.0 && (e.max_y - b[3]).abs() < 1.0);
    let (fields, _) = profile_fields(&ds, None, &[]);
    check_fields("V4", &fields);
}

/// Text cells of a dBASE file, as raw bytes (header length at 8, record length at 10, field
/// descriptors of 32 bytes from 32 until 0x0D).
fn dbf_text_cells(bytes: &[u8]) -> Vec<Vec<u8>> {
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    let (n, header, record) = (u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize, u16_at(8), u16_at(10));
    let mut fields = Vec::new();
    let (mut at, mut offset) = (32, 1);
    while bytes[at] != 0x0D {
        let (ty, len) = (bytes[at + 11], bytes[at + 16] as usize);
        fields.push((ty, offset, len));
        offset += len;
        at += 32;
    }
    let mut out = Vec::new();
    for r in 0..n {
        let rec = &bytes[header + r * record..header + (r + 1) * record];
        for (ty, off, len) in &fields {
            if *ty == b'C' {
                let cell = &rec[*off..off + len];
                let end = cell.iter().rposition(|b| *b != b' ').map_or(0, |p| p + 1);
                out.push(cell[..end].to_vec());
            }
        }
    }
    out
}

#[test]
fn v5_gbk_dbf_without_cpg_reads_as_gbk() {
    let dbf = std::fs::read(dir("V5").join("places_gbk.dbf")).unwrap();
    let cells = dbf_text_cells(&dbf);
    let refs: Vec<&[u8]> = cells.iter().map(Vec::as_slice).collect();
    let (enc, a) = shapefile_encoding(&refs, None);
    assert_eq!(enc.value, TextEncoding::Gbk);
    let exp = expected("V5");
    assert!(exp["encoding"]["accept"].as_array().unwrap().iter().any(|v| v == "GBK"));
    let a = a.unwrap();
    assert_eq!(a.kind, AssumptionKind::ShapefileGbk);
    assert_eq!(a.line, "Read text as Chinese (GBK).");
    let names: Vec<String> = cells.iter().map(|c| enc.value.decode(c)).collect();
    assert!(names.iter().any(|n| n == exp["first_row"]["name"].as_str().unwrap()), "{names:?}");
    // Field roles, on the text decoded as GBK.
    let (mut ds, _) = rustgis_data::io::read_path(&dir("V5").join("places_gbk.shp")).unwrap();
    let text_fields: Vec<usize> =
        ds.fields.iter().enumerate().filter(|(_, f)| f.ty == rustgis_data::FieldType::Text).map(|(i, _)| i).collect();
    let mut it = cells.into_iter();
    for f in &mut ds.features {
        for &i in &text_fields {
            f.attrs[i] = Value::Text(enc.value.decode(&it.next().unwrap()));
        }
    }
    let (fields, _) = profile_fields(&ds, None, &[]);
    check_fields("V5", &fields);
}

#[test]
fn v9_zipped_shapefile_declares_its_crs() {
    let bytes = std::fs::read(dir("V9").join("roads_bundle.zip")).unwrap();
    let layers = rustgis_data::io::read_layers("roads_bundle.zip", &bytes).unwrap();
    let ds = &layers[0].dataset;
    assert_eq!(geometry_kinds(ds), vec![(LayerKind::Lines, 6)]);
    let (_, a) = crs_from_values(raw_extent(ds), Some(Crs::Epsg(4326)));
    assert!(a.is_empty());
    assert!(expected_ids("V9").is_empty());
}

#[test]
fn v10_edge_cases_profile_without_crashing() {
    let (ds, _) = rustgis_data::io::read_path(&dir("V10").join("edge_cases.geojson")).unwrap();
    let (fields, _) = profile_fields(&ds, None, &[]);
    assert!(fields.iter().any(|f| f.name == "case"));
    let _ = geometry_kinds(&ds);
}

#[test]
fn t1_lon_lat_csv_located_with_six_blank_rows() {
    let (h, rows) = read_csv("T1", "schools.csv");
    let (loc, fields, a) = profile_csv("schools.csv", &h, &rows);
    let CsvLocation::Located { pair, assumption } = &loc else { panic!("{loc:?}") };
    let exp = expected("T1");
    assert_eq!((pair.x.as_str(), pair.y.as_str()), (exp["coordinates"]["x"].as_str().unwrap(), exp["coordinates"]["y"].as_str().unwrap()));
    assert_eq!(assumption.line, "Located using lon and lat as GPS coordinates (WGS 84).");
    let excluded: Vec<u64> =
        exp["import_report"]["excluded_rows_1based_data"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    assert_eq!(pair.blank_rows, excluded);
    assert_eq!(pair.rows_in_range as u64, exp["import_report"]["located"].as_u64().unwrap());
    assert_eq!(expected_ids("T1"), vec!["crs_wgs84_assumed"]);
    assert!(shown(&a).contains(&AssumptionKind::CsvCoordinateColumns));
    check_fields("T1", &fields);
}

#[test]
fn t2_ambiguous_columns_ask_with_both_pairs() {
    let (h, rows) = read_csv("T2", "deliveries.csv");
    let (loc, fields, a) = profile_csv("deliveries.csv", &h, &rows);
    let CsvLocation::Ambiguous { candidates, assumption, .. } = &loc else { panic!("{loc:?}") };
    let exp = expected("T2");
    let want: Vec<&str> =
        exp["question_card"]["candidate_pairs"].as_array().unwrap().iter().map(|p| p["label"].as_str().unwrap()).collect();
    let got: Vec<String> = candidates.iter().map(CoordinatePair::label).collect();
    assert_eq!(got, want);
    assert_eq!(assumption.treatment, Treatment::Question);
    assert_eq!(assumption.alternatives.last().map(|c| c.id.as_str()), Some("not_sure"));
    let rows_of = |key: &str| -> Vec<u64> { exp["swapped_rows"][key].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect() };
    assert_eq!(candidates[0].swapped_rows, rows_of("x/y"));
    assert_eq!(candidates[1].swapped_rows, rows_of("lng2/lat2"));
    assert_eq!(kinds(&a).first(), Some(&AssumptionKind::CsvAmbiguousColumns));
    check_fields("T2", &fields);
}

#[test]
fn t4_gbk_semicolons_decimal_commas_codes_and_dates() {
    let bytes = std::fs::read(dir("T4").join("stations_gbk.csv")).unwrap();
    let (text, _, errors) = encoding_rs::GBK.decode(&bytes);
    assert!(!errors);
    let (h, rows) = split_csv(&text, ';');
    let (loc, fields, a) = profile_csv("stations_gbk.csv", &h, &rows);
    let CsvLocation::Located { pair, .. } = &loc else { panic!("{loc:?}") };
    assert_eq!((pair.x.as_str(), pair.y.as_str()), ("经度", "纬度"));
    assert!(shown(&a).contains(&AssumptionKind::CsvCoordinateColumns));
    check_fields("T4", &fields);
    let code = fields.iter().find(|f| f.name == "编码").unwrap();
    assert_eq!(code.dtype, lm_core::model::DataType::Text);
    let amount = fields.iter().find(|f| f.name == "金额").unwrap();
    assert_eq!(amount.stats.as_ref().map(|s| (s.min, s.max)), Some((Some(0.5), Some(999_999.99))));
    assert!(rows.iter().any(|r| r[4] == "1234,50") && csv_points(&loc, &rows).first() == Some(&(121.473701, 31.230416)));
}

#[test]
fn t5_workbook_opens_the_only_sheet_with_coordinates() {
    let bytes = std::fs::read(dir("T5").join("survey.xlsx")).unwrap();
    let names = rustgis_data::sheets::sheet_names(&bytes).unwrap();
    let mut grids: Vec<(String, Vec<String>, Vec<Vec<String>>)> = Vec::new();
    for n in &names {
        let (ds, _) = rustgis_data::sheets::read(&bytes, "survey", Some(n), &Default::default()).unwrap();
        let headers: Vec<String> = ds.fields.iter().map(|f| f.name.clone()).collect();
        let rows: Vec<Vec<String>> = ds
            .features
            .iter()
            .map(|f| {
                f.attrs
                    .iter()
                    .map(|v| match v {
                        Value::Null => String::new(),
                        Value::Int(i) => i.to_string(),
                        Value::Float(x) => x.to_string(),
                        Value::Text(t) => t.clone(),
                        Value::Bool(b) => b.to_string(),
                    })
                    .collect()
            })
            .collect();
        grids.push((n.clone(), headers, rows));
    }
    let sheets: Vec<SheetSample> = grids.iter().map(|(n, h, r)| SheetSample { name: n, headers: h, rows: r }).collect();
    let choice = excel_sheet(&sheets).unwrap();
    let exp = expected("T5");
    let want = exp["chosen_sheet"].as_str().unwrap();
    assert_eq!(names[choice.sheet], want);
    let a = choice.assumption.unwrap();
    assert_eq!((a.kind, a.treatment), (AssumptionKind::ExcelSheet, Treatment::Visible));
    assert_eq!(a.line, format!("Opened sheet '{want}'."));
    assert!(matches!(choice.location, CsvLocation::Located { .. }));
    let (_, h, r) = &grids[choice.sheet];
    let (_, fields, _) = profile_csv("survey.xlsx", h, r);
    check_fields("T5", &fields);
}

#[test]
fn c1_amap_name_suggests_gcj02() {
    let (h, rows) = read_csv("C1", "amap_pois.csv");
    let (loc, fields, a) = profile_csv("amap_pois.csv", &h, &rows);
    assert!(matches!(loc, CsvLocation::Located { .. }));
    let q = a.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetNamed).unwrap();
    assert_eq!(q.treatment, Treatment::Question);
    assert_eq!(q.chosen.id, "gcj02");
    assert_eq!(q.line, "These look like Amap (GCJ-02) coordinates. Convert to GPS coordinates?");
    let ids: Vec<&str> = q.alternatives.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["gcj02", "wgs84", "bd09", "not_sure"]);
    assert_eq!(expected("C1")["offset_question"]["suggested"], "GCJ-02");
    check_fields("C1", &fields);
}

#[test]
fn c3_baidu_name_suggests_bd09() {
    let (h, rows) = read_csv("C3", "baidu_pois.csv");
    let (_, fields, a) = profile_csv("baidu_pois.csv", &h, &rows);
    let q = a.iter().find(|a| a.kind == AssumptionKind::ChinaOffsetNamed).unwrap();
    assert_eq!(q.chosen.id, "bd09");
    assert_eq!(q.line, "These look like Baidu (BD-09) coordinates. Convert to GPS coordinates?");
    assert_eq!(expected("C3")["offset_question"]["suggested"], "BD-09");
    check_fields("C3", &fields);
}

fn band(id: &str, file: &str, scale: Option<f64>, offset: Option<f64>) -> (lm_core::model::BandProfile, Vec<Assumption>) {
    let r = rustgis_data::raster::read_path(&dir(id).join(file)).unwrap();
    let b = &r.bands[0];
    let integer = !r.data_type.to_ascii_lowercase().contains("float");
    let s = BandSample {
        index: 1,
        name: "",
        file_name: file,
        width: r.width as usize,
        height: r.height as usize,
        values: &b.data,
        declared_nodata: r.nodata,
        scale,
        offset,
        unit: None,
        integer_type: integer,
    };
    profile_band(&s)
}

#[test]
fn r3_landcover_is_categories() {
    let (b, a) = band("R3", "landcover.tif", None, None);
    assert_eq!(expected("R3")["value_role"], "category");
    assert_eq!(b.cell_meaning.value, CellMeaning::Category);
    assert_eq!(b.nodata.value, Some(0.0));
    assert!(a.iter().all(|a| a.kind != AssumptionKind::RasterNoData));
}

#[test]
fn r6_minus_9999_edges_become_nodata() {
    let exp = expected("R6");
    let (b, a) = band("R6", "scaled.tif", exp["scale"].as_f64(), exp["offset"].as_f64());
    let nd = a.iter().find(|a| a.kind == AssumptionKind::RasterNoData).unwrap();
    assert_eq!(nd.treatment, Treatment::Visible);
    assert_eq!(nd.line, "Treated \u{2212}9999 as No data.");
    assert_eq!(b.nodata.value, exp["assumptions"][0]["raw_value"].as_f64());
    let range: Vec<f64> = exp["valid_decoded_range"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!((b.min.unwrap() - range[0]).abs() < 1e-3 && (b.max.unwrap() - range[1]).abs() < 1e-3, "{:?} {:?}", b.min, b.max);
    assert_eq!(b.cell_meaning.value, CellMeaning::Measured);
}
