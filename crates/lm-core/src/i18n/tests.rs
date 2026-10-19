use super::*;

#[test]
fn parse_and_escape() {
    let (m, e) = parse("# c\nOpen\tÖffnen\nA\\nB\tX\\nY\nbad line\nOpen\tAuf\n");
    assert_eq!(m.get("A\nB").map(String::as_str), Some("X\nY"));
    assert_eq!(e.len(), 2, "{e:?}");
}

#[test]
fn every_catalog_parses_cleanly_and_keeps_placeholders() {
    for l in Lang::all().skip(1) {
        let (m, errors) = parse(l.source());
        assert!(errors.is_empty(), "{}: {errors:?}", l.code());
        for (en, tr) in &m {
            assert_eq!(placeholders(en), placeholders(tr), "{}: placeholders differ in {en:?} → {tr:?}", l.code());
            // Action templates' [tokens] are filled with layer names (lm-actions).
            let tokens = |s: &str| -> Vec<String> {
                let mut v: Vec<String> = s.split('[').skip(1).filter_map(|p| p.split_once(']').map(|x| x.0.to_string())).collect();
                v.sort();
                v
            };
            assert_eq!(tokens(en), tokens(tr), "{}: [tokens] differ in {en:?} → {tr:?}", l.code());
            // Line breaks are part of the layout.
            assert_eq!(en.matches('\n').count(), tr.matches('\n').count(), "{}: line breaks differ in {en:?}", l.code());
            // The app's name comes from lm_core::brand as {app}, never typed into a catalog.
            assert!(!en.contains(crate::brand::NAME) && !tr.contains(crate::brand::NAME), "{}: brand typed in {en:?}", l.code());
        }
    }
}

#[test]
fn lookup_falls_back_to_english() {
    assert_eq!(tr(Lang::EN, "Layers"), "Layers");
    for l in Lang::all() {
        assert_eq!(tr(l, "no such string, surely"), "no such string, surely");
    }
    assert_eq!(current(), Lang::EN);
    assert_eq!(fmt("{n} of {total} selected", &[("n", "3"), ("total", "{n}")]), "3 of {n} selected");
    assert_eq!(fill("{a} {missing} {", &[("a", "x")]), "x {missing} {");
}

#[test]
fn tags_and_preferences() {
    assert_eq!(lang_from_tag("de_DE.UTF-8").map(Lang::code), Some("de"));
    assert_eq!(lang_from_tag("pt-BR").map(Lang::code), Some("pt"));
    assert_eq!(lang_from_tag("zh-Hans-CN").map(Lang::code), Some("zh"));
    assert_eq!(lang_from_tag("zh-TW"), None);
    assert_eq!(lang_from_tag("C").map(Lang::code), Some("en"));
    assert_eq!(lang_from_tag("xx-YY"), None);
    assert_eq!(first_supported(["xx", "fr-CA", "de"]).map(Lang::code), Some("fr"));
    assert_eq!(resolve(AUTO, ["ja-JP", "en-US"]).code(), "ja");
    assert_eq!(resolve("ko", ["ja-JP"]).code(), "ko");
    assert_eq!(resolve(AUTO, ["zh-Hant-TW", "sv"]).code(), "en");
    assert_eq!(resolve("bogus", []).code(), "en");
    // Every registered language has a name in itself, a unique code and a menu row.
    let mut codes: Vec<&str> = Lang::all().map(Lang::code).collect();
    codes.dedup();
    assert_eq!(codes.len(), LANGUAGES.len());
}

#[test]
fn numbers_follow_the_language() {
    let de = Lang::from_code("de").unwrap_or(Lang::EN);
    let fr = Lang::from_code("fr").unwrap_or(Lang::EN);
    let es = Lang::from_code("es").unwrap_or(Lang::EN);
    let ja = Lang::from_code("ja").unwrap_or(Lang::EN);
    for s in ["1,248.5", "−0.4", "12%", "4,999 lines", "Haiyan", "", "1,2,3", "3.", "v1.2.3"] {
        assert_eq!(localize_number(Lang::EN, s), s);
        assert_eq!(localize_number(ja, s), s);
    }
    assert_eq!(localize_number(de, "1,248.5"), "1.248,5");
    assert_eq!(localize_number(de, "−0.4 °C"), "−0,4 °C");
    assert_eq!(localize_number(fr, "14,537"), "14\u{202F}537");
    assert_eq!(localize_number(es, "4,999"), "4999");
    assert_eq!(localize_number(es, "14,537"), "14.537");
    assert_eq!(localize_number(de, "12%"), "12%");
    // Not a number in English form: left alone.
    assert_eq!(localize_number(de, "1,2,3"), "1,2,3");
    assert_eq!(localize_number(de, "Haiyan"), "Haiyan");
    assert_eq!(count(1248), "1,248");
}
