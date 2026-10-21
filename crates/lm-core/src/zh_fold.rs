//! Traditional → simplified Chinese, character by character, for search: GeoNames gives some
//! places their Chinese name in traditional characters (紐約 for New York City, 達喀爾 for
//! Dakar), so a query typed in simplified characters (纽约) would miss them. Place search folds
//! both the query and the stored names with [`to_simplified`] before comparing.
//!
//! The table is `assets/zh/ts-characters.txt` (3,221 pairs), made by `tools/lm/build_ts_table.py`
//! from OpenCC's `TSCharacters.txt` (Apache License 2.0; see ATTRIBUTION.md). Characters with no
//! entry, and every non-Han character, are left as they are.

use std::sync::OnceLock;

const TABLE: &str = include_str!("../../../assets/zh/ts-characters.txt");

/// The pairs, sorted by the traditional character.
fn table() -> &'static [(char, char)] {
    static PAIRS: OnceLock<Vec<(char, char)>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        let mut v: Vec<(char, char)> = TABLE
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let mut c = l.chars();
                let (t, tab, s) = (c.next()?, c.next()?, c.next()?);
                (tab == '\t' && c.next().is_none()).then_some((t, s))
            })
            .collect();
        v.sort_unstable_by_key(|p| p.0);
        v.dedup_by_key(|p| p.0);
        v
    })
}

/// The simplified form of one character (itself when it has none).
pub fn simplify_char(c: char) -> char {
    if c.is_ascii() {
        return c;
    }
    let t = table();
    t.binary_search_by_key(&c, |p| p.0).map_or(c, |i| t.get(i).map_or(c, |p| p.1))
}

/// `s` with every traditional character replaced by its simplified form.
pub fn to_simplified(s: &str) -> String {
    s.chars().map(simplify_char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traditional_names_fold_to_simplified() {
        assert_eq!(to_simplified("紐約"), "纽约");
        assert_eq!(to_simplified("達喀爾"), "达喀尔");
        assert_eq!(to_simplified("布宜諾斯艾利斯"), "布宜诺斯艾利斯");
        assert_eq!(to_simplified("齋浦爾"), "斋浦尔");
    }

    #[test]
    fn simplified_and_other_text_is_unchanged() {
        for s in ["纽约", "东京", "巴塞罗那", "成都市", "New York", "São Paulo", "東京 ok"] {
            let out = to_simplified(s);
            if s == "東京 ok" {
                assert_eq!(out, "东京 ok");
            } else {
                assert_eq!(out, s);
            }
        }
        assert!(table().len() > 3000);
    }
}
