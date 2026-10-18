use super::*;
use rustgis_engine::data::GeometryType;
use rustgis_engine::data::geometry::Geometry;
use rustgis_engine::geom::Coord;
use std::collections::HashMap;

fn table(fields: &[(&str, FieldType)], rows: Vec<Vec<Value>>) -> Dataset {
    let mut d = Dataset::new("t", GeometryType::None, fields.iter().map(|(n, t)| Field::new(*n, *t)).collect());
    for r in rows {
        d.push(None, r);
    }
    d
}

fn layer(fields: &[(&str, FieldType)], rows: Vec<Vec<Value>>) -> Dataset {
    let mut d = Dataset::new("l", GeometryType::Point, fields.iter().map(|(n, t)| Field::new(*n, *t)).collect());
    for (i, r) in rows.into_iter().enumerate() {
        d.push(Some(Geometry::Point(Coord::new(i as f64, 0.0))), r);
    }
    d
}

fn t(s: &str) -> Value {
    Value::Text(s.into())
}

fn countries() -> Dataset {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples/countries.geojson");
    let layers = crate::import::import_path(&path, &crate::import::Ctx::detached(crate::import::Ticket::default())).unwrap();
    match &layers[0].data {
        crate::import::LayerData::Vector(ds) => (**ds).clone(),
        _ => panic!(),
    }
}

fn joined_value(j: &Joined, layer: &Dataset, key_field: &str, key: &str, field: &str) -> Value {
    let kf = layer.fields.iter().position(|f| f.name == key_field).unwrap();
    let row = (0..layer.features.len()).find(|r| key_text(layer, *r, kf).as_deref() == Some(key)).unwrap();
    let f = j.data.fields.iter().position(|f| f.name == field).unwrap();
    j.data.value(row, f).clone()
}

#[test]
fn normalization_ladder() {
    assert_eq!(normalize("  Côte d’Ivoire ", Level::Folded).as_deref(), Some("cote d'ivoire"));
    assert_eq!(normalize("Côte d'Ivoire", Level::Stripped).as_deref(), Some("cotedivoire"));
    assert_eq!(normalize("06037", Level::Code).as_deref(), Some("6037"));
    assert_eq!(normalize("6037.0", Level::Code).as_deref(), Some("6037"));
    assert_eq!(normalize("Pudong", Level::Code), None);
    assert_eq!(normalize("Korea, Rep.", Level::Crosswalk(Family::Countries)).as_deref(), Some("KOR"));
    assert_eq!(normalize("410", Level::Crosswalk(Family::Countries)).as_deref(), Some("KOR"));
    assert_eq!(normalize("CA", Level::Crosswalk(Family::UsStates)).as_deref(), Some("06"));
    assert_eq!(normalize("CA", Level::Crosswalk(Family::Countries)).as_deref(), Some("CAN"));
    assert_eq!(normalize("上海", Level::Crosswalk(Family::CnProvinces)).as_deref(), Some("310000"));
    assert_eq!(normalize("Shanghai Municipality", Level::Crosswalk(Family::CnProvinces)), None);
}

/// J1: the bundled countries and a World Bank-style table keyed by ISO3 join exactly, with no
/// question; keyed by World Bank names they join through the crosswalk.
#[test]
fn j1_countries_join_without_a_question() {
    let c = countries();
    let iso = c.fields.iter().position(|f| f.name == "iso_a3").unwrap();
    let codes: Vec<String> = (0..c.features.len()).filter_map(|r| key_text(&c, r, iso)).collect();
    let tb = table(
        &[("Country Code", FieldType::Text), ("gdp_growth", FieldType::Double)],
        codes.iter().enumerate().map(|(i, k)| vec![t(k), Value::Float(i as f64 / 10.0)]).collect(),
    );
    let Decision::Join(p) = decide(&score(&tb, &c)) else { panic!("asked") };
    assert_eq!((c.fields[p.layer_field].name.as_str(), p.level), ("iso_a3", Level::Exact));
    // World Bank spellings.
    let names = [
        "Korea, Rep.",
        "Egypt, Arab Rep.",
        "Russian Federation",
        "Viet Nam",
        "Cote d'Ivoire",
        "Iran, Islamic Rep.",
        "France",
        "Brazil",
        "Japan",
        "Germany",
    ];
    let tb = table(
        &[("Country Name", FieldType::Text), ("value", FieldType::Integer)],
        names.iter().enumerate().map(|(i, n)| vec![t(n), Value::Int(i as i64)]).collect(),
    );
    let Decision::Join(p) = decide(&score(&tb, &c)) else { panic!("asked") };
    assert_eq!(p.level, Level::Crosswalk(Family::Countries));
    assert_eq!(p.matched_keys, names.len());
    let spec = spec_for(&tb, &c, &p, "wb.csv");
    let j = apply(&tb, &c, &spec).unwrap();
    assert_eq!(j.layer_matched, names.len());
    assert!(j.unmatched.is_empty());
    assert_eq!(joined_value(&j, &c, "iso_a3", "KOR", "value"), Value::Int(0));
}

/// J2: county codes that lost their leading zeros match as codes.
#[test]
fn j2_codes_without_leading_zeros() {
    let ly = layer(
        &[("GEOID", FieldType::Text), ("NAME", FieldType::Text)],
        vec![
            vec![t("06037"), t("Los Angeles")],
            vec![t("06059"), t("Orange")],
            vec![t("36061"), t("New York")],
            vec![t("01001"), t("Autauga")],
        ],
    );
    let tb = table(
        &[("fips", FieldType::Integer), ("cases", FieldType::Integer)],
        vec![
            vec![Value::Int(6037), Value::Int(10)],
            vec![Value::Int(6059), Value::Int(20)],
            vec![Value::Int(36061), Value::Int(30)],
            vec![Value::Int(1001), Value::Int(5)],
        ],
    );
    let Decision::Join(p) = decide(&score(&tb, &ly)) else { panic!("asked") };
    assert_eq!(p.level, Level::Code);
    let j = apply(&tb, &ly, &spec_for(&tb, &ly, &p, "cases.csv")).unwrap();
    assert_eq!(joined_value(&j, &ly, "GEOID", "06059", "cases"), Value::Int(20));
    assert_eq!(j.layer_matched, 4);
}

/// J3: one row per district per year: the latest year is used, and the chip says which.
#[test]
fn j3_several_rows_take_the_latest_year() {
    let ly = layer(&[("name", FieldType::Text)], vec![vec![t("Jiading")], vec![t("Pudong")], vec![t("Huangpu")]]);
    let mut rows = Vec::new();
    for (k, n) in ["Jiading", "Pudong", "Huangpu"].iter().enumerate() {
        for y in 2019..=2023 {
            rows.push(vec![t(n), Value::Int(y), Value::Int((k as i64 + 1) * 1000 + y)]);
        }
    }
    let tb = table(&[("district", FieldType::Text), ("year", FieldType::Integer), ("population", FieldType::Integer)], rows);
    let Decision::Join(p) = decide(&score(&tb, &ly)) else { panic!("asked") };
    let j = apply(&tb, &ly, &spec_for(&tb, &ly, &p, "pop.csv")).unwrap();
    assert_eq!(joined_value(&j, &ly, "name", "Pudong", "population"), Value::Int(2000 + 2023));
    assert_eq!(j.rows_note.as_deref(), Some("Showing 2023"));
    assert_eq!(j.most_rows, 5);
}

/// Several rows and no year: counts are summed, other numbers averaged.
#[test]
fn several_rows_without_a_year_are_combined() {
    let ly = layer(&[("name", FieldType::Text)], vec![vec![t("A")], vec![t("B")]]);
    let tb = table(
        &[("name", FieldType::Text), ("count", FieldType::Integer), ("score", FieldType::Double)],
        vec![
            vec![t("A"), Value::Int(2), Value::Float(1.0)],
            vec![t("A"), Value::Int(3), Value::Float(3.0)],
            vec![t("B"), Value::Int(7), Value::Float(5.0)],
        ],
    );
    let Decision::Join(p) = decide(&score(&tb, &ly)) else { panic!("asked") };
    let j = apply(&tb, &ly, &spec_for(&tb, &ly, &p, "t.csv")).unwrap();
    assert_eq!(joined_value(&j, &ly, "name", "A", "count"), Value::Int(5));
    assert_eq!(joined_value(&j, &ly, "name", "A", "score"), Value::Float(2.0));
    assert!(j.rows_note.unwrap().starts_with("Combined per place"));
}

/// J4: two plausible keys that place rows differently, neither clearly ahead: one question.
#[test]
fn j4_two_plausible_keys_ask_once() {
    let names = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel", "India", "Juliet"];
    let ly = layer(
        &[("id", FieldType::Integer), ("name", FieldType::Text)],
        names.iter().enumerate().map(|(i, n)| vec![Value::Int(i as i64 + 1), t(n)]).collect(),
    );
    // The codes would put rows on records 1-6; the labels on records 5-10, crosswise.
    let tb = table(
        &[("code", FieldType::Integer), ("label", FieldType::Text), ("v", FieldType::Integer)],
        (0..10)
            .map(|i| {
                let code = if i < 6 { Value::Int(i + 1) } else { Value::Int(100 + i) };
                let label = if i >= 4 { t(names[(13 - i) as usize % 10]) } else { t(&format!("Zulu {}", i + 1)) };
                vec![code, label, Value::Int(i)]
            })
            .collect(),
    );
    match decide(&score(&tb, &ly)) {
        Decision::Ask(offer) => assert!(offer.len() >= 2 && offer.len() <= 3, "{offer:?}"),
        d => panic!("{d:?}"),
    }
}

/// Typos are never matched on their own; they come back as suggestions, and an accepted one
/// joins.
#[test]
fn near_misses_are_suggested_not_matched() {
    let ly = layer(
        &[("name", FieldType::Text)],
        ["Shanghai", "Beijing", "Guangzhou", "Shenzhen", "Chengdu", "Wuhan", "Nanjing", "Hangzhou", "Xi'an", "Suzhou"]
            .iter()
            .map(|n| vec![t(n)])
            .collect(),
    );
    let tb = table(
        &[("city", FieldType::Text), ("v", FieldType::Integer)],
        ["Shangai", "Beijing", "Guangzhou", "Shenzhen", "Chengdu", "Wuhan", "Nanjing", "Hangzhou", "Xi'an", "Suzhou"]
            .iter()
            .enumerate()
            .map(|(i, n)| vec![t(n), Value::Int(i as i64)])
            .collect(),
    );
    let Decision::Join(p) = decide(&score(&tb, &ly)) else { panic!("asked") };
    let mut spec = spec_for(&tb, &ly, &p, "c.csv");
    let j = apply(&tb, &ly, &spec).unwrap();
    assert_eq!(j.unmatched, vec!["Shangai".to_string()]);
    assert_eq!(j.near_misses, vec![("Shangai".to_string(), "Shanghai".to_string())]);
    spec.aliases = j.near_misses.clone();
    let j = apply(&tb, &ly, &spec).unwrap();
    assert!(j.unmatched.is_empty());
    assert_eq!(joined_value(&j, &ly, "name", "Shanghai", "v"), Value::Int(0));
}

/// Nothing in common: nothing is offered; a category column is never taken for a key.
#[test]
fn unrelated_tables_offer_nothing() {
    let ly = layer(
        &[("name", FieldType::Text), ("kind", FieldType::Text)],
        (0..20).map(|i| vec![t(&format!("place {i}")), t(if i % 2 == 0 { "a" } else { "b" })]).collect(),
    );
    let tb = table(&[("thing", FieldType::Text)], (0..20).map(|i| vec![t(&format!("other {i}"))]).collect());
    assert_eq!(decide(&score(&tb, &ly)), Decision::Nothing);
    let tb = table(&[("kind", FieldType::Text), ("v", FieldType::Integer)], vec![vec![t("a"), Value::Int(1)], vec![t("b"), Value::Int(2)]]);
    assert_eq!(decide(&score(&tb, &ly)), Decision::Nothing);
}

/// Determinism: the same inputs give the same decision every time.
#[test]
fn decisions_are_deterministic() {
    let c = countries();
    let names = ["Korea, Rep.", "France", "Brazil", "Japan"];
    let tb = table(&[("n", FieldType::Text)], names.iter().map(|n| vec![t(n)]).collect());
    let first = decide(&score(&tb, &c));
    for _ in 0..50 {
        assert_eq!(decide(&score(&tb, &c)), first);
    }
}

fn open(rel: &str) -> Dataset {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    let layers = crate::import::import_path(&path, &crate::import::Ctx::detached(crate::import::Ticket::default())).unwrap();
    match &layers[0].data {
        crate::import::LayerData::Vector(ds) => (**ds).clone(),
        _ => panic!("{rel}"),
    }
}

fn decided(table: &str, layer: &str) -> (Dataset, Dataset, Decision) {
    let (t, l) = (open(table), open(layer));
    let d = decide(&score(&t, &l));
    (t, l, d)
}

/// The Gate 3 join fixtures, opened as the app opens them (WP3.11 › Gate 3).
#[test]
fn gate3_join_fixtures() {
    // J1: ISO3 codes join exactly; World Bank names through the crosswalk, all 20.
    let (_, l, d) = decided("fixtures/J1/gdp_growth_iso3.csv", "assets/examples/countries.geojson");
    let Decision::Join(p) = d else { panic!("J1 iso3: {d:?}") };
    assert_eq!((l.fields[p.layer_field].name.as_str(), p.level, p.rate()), ("iso_a3", Level::Exact, 1.0));
    let (t, l, d) = decided("fixtures/J1/population_wb_names.csv", "assets/examples/countries.geojson");
    let Decision::Join(p) = d else { panic!("J1 names: {d:?}") };
    assert_eq!(p.level, Level::Crosswalk(Family::Countries));
    let j = apply(&t, &l, &spec_for(&t, &l, &p, "population_wb_names.csv")).unwrap();
    assert_eq!((j.layer_matched, j.unmatched.len()), (20, 0), "{:?}", j.unmatched);
    // J2: codes without leading zeros.
    let (t, l, d) = decided("fixtures/J2/cases.csv", "fixtures/J2/counties.geojson");
    let Decision::Join(p) = d else { panic!("J2: {d:?}") };
    assert_eq!(p.level, Level::Code);
    assert_eq!(apply(&t, &l, &spec_for(&t, &l, &p, "cases.csv")).unwrap().layer_matched, 6);
    // J3: the latest year, with the chip.
    let (t, l, d) = decided("fixtures/J3/population_by_year.csv", "fixtures/V1/districts.geojson");
    let Decision::Join(p) = d else { panic!("J3: {d:?}") };
    let j = apply(&t, &l, &spec_for(&t, &l, &p, "population_by_year.csv")).unwrap();
    assert_eq!((j.layer_matched, j.rows_note.as_deref()), (17, Some("Showing 2023")));
    // J4: exactly one question.
    let (_, _, d) = decided("fixtures/J4/mixed_keys.csv", "fixtures/J4/places.geojson");
    assert!(matches!(d, Decision::Ask(ref o) if (2..=3).contains(&o.len())), "J4: {d:?}");
    // J5: Chinese names through the province crosswalk; the misspelling is suggested.
    let (t, l, d) = decided("fixtures/J5/population_zh.csv", "fixtures/J5/provinces.geojson");
    let Decision::Join(p) = d else { panic!("J5: {d:?}") };
    assert_eq!(p.level, Level::Crosswalk(Family::CnProvinces));
    let j = apply(&t, &l, &spec_for(&t, &l, &p, "population_zh.csv")).unwrap();
    assert_eq!(j.layer_matched, 7);
    assert_eq!(j.near_misses, vec![("Hubel".to_string(), "Hubei".to_string())]);
}

/// Several layers (owner's request, 2026-10-18): a table joins every layer it clearly matches,
/// the best first; a layer whose field repeats (cities' country) is never a target, so the
/// codes table leaves cities alone.
#[test]
fn a_table_joins_every_layer_it_clearly_matches() {
    let tb = open("fixtures/J1/gdp_growth_iso3.csv");
    let c = open("assets/examples/countries.geojson");
    let cities = open("assets/examples/cities.geojson");
    let Plan::Join(targets) = plan(&tb, &[&cities, &c, &c]) else { panic!("no join") };
    assert_eq!(targets.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [1, 2], "both countries layers, shelf order, not cities");
    // World Bank names against countries and cities: cities' country names repeat, so only
    // countries.
    let tb = open("fixtures/J1/population_wb_names.csv");
    assert!(matches!(plan(&tb, &[&cities, &c]), Plan::Join(ref v) if v.len() == 1 && v[0].0 == 1));
    // Nothing clear anywhere: the one question, about the best layer (J4 as today).
    let (tb, l) = (open("fixtures/J4/mixed_keys.csv"), open("fixtures/J4/places.geojson"));
    assert!(matches!(plan(&tb, &[&cities, &l]), Plan::Ask(1, ref o) if (2..=3).contains(&o.len())));
    let unrelated = table(&[("thing", FieldType::Text)], (0..20).map(|i| vec![t(&format!("other {i}"))]).collect());
    assert_eq!(plan(&unrelated, &[&cities, &l]), Plan::Nothing);
}

/// Separating a joined table gives the layer's data back as it was.
#[test]
fn separating_gives_the_layer_back() {
    let (t, l) = (open("fixtures/J1/gdp_growth_iso3.csv"), open("assets/examples/countries.geojson"));
    let Decision::Join(p) = decide(&score(&t, &l)) else { panic!() };
    let j = apply(&t, &l, &spec_for(&t, &l, &p, "gdp_growth_iso3.csv")).unwrap();
    assert_eq!(separate(&j.data, &j.new_fields), l);
}

fn j6_csv(name: &str) -> Vec<HashMap<String, String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/J6").join(name);
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let head: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect()
}

fn ward_spec(table: &str) -> JoinSpec {
    keyed_spec(table, "ward_code", "ward_code")
}

fn keyed_spec(table: &str, layer_key: &str, table_key: &str) -> JoinSpec {
    JoinSpec {
        table_name: format!("{table}.csv"),
        table_field: table_key.into(),
        layer_field: layer_key.into(),
        level: Level::Exact,
        rows: Rows::One,
        aliases: Vec::new(),
    }
}

/// WP6.4 (docs/lm/actions-p2.md › Match a table to a layer): J6's five pairings against pandas
/// `merge(validate=…)` (`tools/lm/reference_tests/join_validate.py`), one with key columns of
/// different names: the cardinality, the row counts worked out before joining, the unmatched
/// keys and rows, and every record's value. Everything exact.
#[test]
fn j6_matches_pandas_merge_validate() {
    let values = j6_csv("expected_join_values.csv");
    let mut compared = 0;
    for w in j6_csv("expected_join.csv") {
        let (l, t) = (open(&format!("fixtures/J6/{}.geojson", w["layer"])), open(&format!("fixtures/J6/{}.csv", w["table"])));
        let spec = keyed_spec(&w["table"], &w["layer_key"], &w["table_key"]);
        let e = expect(&t, &l, &spec).unwrap();
        let job = format!("{} × {}", w["layer"], w["table"]);
        let n = |k: &str| w[k].parse::<usize>().unwrap();
        assert_eq!(e.cardinality.pandas(), w["cardinality"], "{job}");
        assert_eq!(e.asks_first(), w["cardinality"] != "one_to_one", "{job}");
        assert_eq!(
            (e.layer_rows, e.table_rows, e.pairs, e.merge_rows, e.layer_matched, e.table_matched),
            (n("layer_rows"), n("table_rows"), n("pairs"), n("merge_rows"), n("layer_matched"), n("table_matched")),
            "{job}"
        );
        // Named columns find the same spec (exact keys here).
        assert_eq!(spec_for_fields(&t, &l, &w["table_key"], &w["layer_key"], &spec.table_name), Some(spec.clone()), "{job}");
        let j = apply(&t, &l, &spec).unwrap();
        assert_eq!(j.expected, e, "{job}: the same counts before and after");
        assert_eq!(j.data.features.len(), e.rows(), "{job}: one row per record");
        assert_eq!(j.unmatched.join("|"), w["unmatched_keys"], "{job}");
        let rows: Vec<String> = j.unmatched_rows.iter().map(|r| (r + 1).to_string()).collect();
        assert_eq!(rows.join("|"), w["unmatched_rows"], "{job}");
        assert_eq!(j.blank_keys, n("blank_keys"), "{job}");
        for v in values.iter().filter(|v| v["layer"] == w["layer"] && v["table"] == w["table"]) {
            let k = j.data.fields.iter().position(|f| f.name == v["field"]).unwrap();
            let r = v["row"].parse::<usize>().unwrap() - 1;
            let want = if v["value"].is_empty() { Value::Null } else { Value::Int(v["value"].parse().unwrap()) };
            assert_eq!(j.data.value(r, k), &want, "{job}: row {}", r + 1);
            compared += 1;
        }
    }
    assert_eq!(compared, 64);
}

/// The card's words for each cardinality.
#[test]
fn expected_rows_read_as_words() {
    let l = open("fixtures/J6/wards.geojson");
    let split = open("fixtures/J6/wards_split.geojson");
    let one = open("fixtures/J6/ward_population.csv");
    let years = open("fixtures/J6/ward_population_by_year.csv");
    let nouns = ("ward", "wards");
    let line = |t: &Dataset, l: &Dataset, name: &str| expect(t, l, &ward_spec(name)).unwrap().line(nouns);
    assert_eq!(line(&one, &l, "ward_population"), "One-to-one: no key repeats. The layer keeps its 12 rows.");
    assert_eq!(
        line(&years, &l, "ward_population_by_year"),
        "One-to-many: a key repeats in the table (up to 3 rows). A plain merge would give 24 rows; the layer keeps its 12, one per ward."
    );
    assert_eq!(
        line(&one, &split, "ward_population"),
        "Many-to-one: a key repeats in the layer (up to 2 wards), and each gets the row. The layer keeps its 14 rows."
    );
    assert_eq!(
        line(&years, &split, "ward_population_by_year"),
        "Many-to-many: keys repeat in the table (up to 3 rows) and in the layer (up to 2 wards). A plain merge would give 28 rows; \
         the layer keeps its 14, one per ward."
    );
}

/// Repeats are handled as the card says, and only joins whose keys repeat ask first: J3's
/// table (one row per district per year) is clear but asks; J1's codes join on their own.
#[test]
fn joins_whose_keys_repeat_ask_first() {
    let (t, l) = (open("fixtures/J3/population_by_year.csv"), open("fixtures/V1/districts.geojson"));
    let Plan::Confirm(0, p) = plan(&t, &[&l]) else { panic!("J3 should ask first") };
    let e = expect(&t, &l, &spec_for(&t, &l, &p, "population_by_year.csv")).unwrap();
    assert_eq!(e.cardinality, Cardinality::OneToMany);
    assert_eq!(
        e.handling(("district", "districts")).as_deref(),
        Some("Several rows for one district: the one with the latest year is used.")
    );
    let (t, l) = (open("fixtures/J1/gdp_growth_iso3.csv"), open("assets/examples/countries.geojson"));
    assert!(matches!(plan(&t, &[&l]), Plan::Join(ref v) if v.len() == 1));
    let years = open("fixtures/J6/ward_population_by_year.csv");
    let split = open("fixtures/J6/wards_split.geojson");
    let e = expect(&years, &split, &ward_spec("ward_population_by_year")).unwrap();
    assert_eq!(
        e.handling(("ward", "wards")).as_deref(),
        Some("Several rows for one ward: the one with the latest year is used. Wards that share a key each take its row.")
    );
    let one = open("fixtures/J6/ward_population.csv");
    assert_eq!(expect(&one, &open("fixtures/J6/wards.geojson"), &ward_spec("ward_population")).unwrap().handling(("ward", "wards")), None);
}
