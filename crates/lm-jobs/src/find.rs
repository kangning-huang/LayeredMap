//! Find in a layer (WP3.10): every field of every record, matched as people read it, case- and
//! accent-insensitively ("cote" finds "Côte d'Ivoire"); numbers as they are displayed, with or
//! without separators ("1250" and "1,250" both find 1,250). The index holds each cell's folded
//! text, built once per data revision off the interface thread, so a search over 1,000,000
//! records is a scan of short strings.

use rustgis_engine::data::Dataset;

use crate::inspect::cell_text;

/// Lowercase with the common Latin diacritics removed: "Côte d'Ivoire" → "cote d'ivoire".
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        for l in c.to_lowercase() {
            match l {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => out.push('a'),
                'ç' | 'ć' | 'č' | 'ĉ' => out.push('c'),
                'ď' | 'đ' => out.push('d'),
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => out.push('e'),
                'ğ' | 'ģ' => out.push('g'),
                'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => out.push('i'),
                'ķ' => out.push('k'),
                'ł' | 'ļ' | 'ľ' => out.push('l'),
                'ñ' | 'ń' | 'ň' | 'ņ' => out.push('n'),
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => out.push('o'),
                'ŕ' | 'ř' => out.push('r'),
                'ś' | 'š' | 'ş' | 'ș' => out.push('s'),
                'ť' | 'ţ' | 'ț' => out.push('t'),
                'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => out.push('u'),
                'ý' | 'ÿ' => out.push('y'),
                'ź' | 'ż' | 'ž' => out.push('z'),
                'ß' => out.push_str("ss"),
                'æ' => out.push_str("ae"),
                'œ' => out.push_str("oe"),
                '’' | '‘' | '`' => out.push('\''),
                other => out.push(other),
            }
        }
    }
    out
}

/// The folded text of every cell, by field then row. Blank cells are empty strings.
#[derive(Debug, Default)]
pub struct FindIndex {
    pub fields: Vec<String>,
    cols: Vec<Vec<Box<str>>>,
    rows: usize,
}

/// One match: the record and the first field (in field order) that holds the text.
#[derive(Clone, Debug, PartialEq)]
pub struct FindHit {
    pub row: usize,
    pub field: usize,
    /// How well it matched: 0 a whole value, 1 the start of a word, 2 inside a word.
    pub quality: u8,
}

impl FindIndex {
    pub fn build(ds: &Dataset) -> FindIndex {
        let rows = ds.features.len();
        let cols = (0..ds.fields.len())
            .map(|f| {
                (0..rows)
                    .map(|r| {
                        let Some(t) = cell_text(ds, r, f) else { return Box::<str>::from("") };
                        let mut s = fold(&t);
                        // Numbers also match without their separators ("1250" for "1,250").
                        if s.contains(',') && s.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c == '\u{2212}') {
                            s = format!("{s}\u{1}{}", s.replace(',', ""));
                        }
                        s.into_boxed_str()
                    })
                    .collect()
            })
            .collect();
        FindIndex { fields: ds.fields.iter().map(|f| f.name.clone()).collect(), cols, rows }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Every record holding `query` in any field, best matches first, then in record order.
    /// Records `visible` marks false (hidden by a filter) are never matched.
    pub fn find(&self, query: &str, visible: Option<&[bool]>) -> Vec<FindHit> {
        let q = fold(query.trim());
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for r in 0..self.rows {
            if visible.is_some_and(|v| !v.get(r).copied().unwrap_or(true)) {
                continue;
            }
            let mut best: Option<FindHit> = None;
            for (f, col) in self.cols.iter().enumerate() {
                let cell = &col[r];
                let Some(at) = cell.find(&q) else { continue };
                let quality = if cell.split('\u{1}').any(|part| part == q) {
                    0
                } else if at == 0 || cell[..at].ends_with(|c: char| !c.is_alphanumeric()) {
                    1
                } else {
                    2
                };
                if best.as_ref().is_none_or(|b| quality < b.quality) {
                    best = Some(FindHit { row: r, field: f, quality });
                }
                if quality == 0 {
                    break;
                }
            }
            hits.extend(best);
        }
        hits.sort_by_key(|h| (h.quality, h.row));
        hits
    }
}

/// Where `query` sits in `text` (byte range in `text`), compared folded, for drawing the match
/// in bold. Folding keeps one character for one in all but a few letters (ß, æ, œ); when the
/// lengths differ the whole text is returned unmarked.
pub fn match_range(text: &str, query: &str) -> Option<std::ops::Range<usize>> {
    let q = fold(query.trim());
    if q.is_empty() {
        return None;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let folded: Vec<String> = chars.iter().map(|(_, c)| fold(&c.to_string())).collect();
    if folded.iter().any(|f| f.chars().count() != 1) {
        return None;
    }
    let flat: String = folded.concat();
    let start_char = flat.find(&q).map(|b| flat[..b].chars().count())?;
    let end_char = start_char + q.chars().count();
    let start = chars.get(start_char)?.0;
    let end = chars.get(end_char).map_or(text.len(), |(b, _)| *b);
    Some(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_engine::data::geometry::Geometry;
    use rustgis_engine::data::{Field, FieldType, GeometryType, Value};
    use rustgis_engine::geom::Coord;

    fn ds() -> Dataset {
        let fields = vec![
            Field::new("name", FieldType::Text),
            Field::new("land_use", FieldType::Text),
            Field::new("pop", FieldType::Integer),
            Field::new("year", FieldType::Integer),
        ];
        let rows: [(&str, &str, i64, i64); 4] = [
            ("Côte d'Ivoire", "Residential", 1250, 2019),
            ("Huangpu", "Commercial", 662_030, 2020),
            ("Pudong", "Residential", 5_681_512, 2019),
            ("Chinatown", "Mixed", 0, 2018),
        ];
        let mut d = Dataset::new("t", GeometryType::Point, fields);
        for (n, l, p, y) in rows {
            d.push(
                Some(Geometry::Point(Coord::new(0.0, 0.0))),
                vec![Value::Text(n.into()), Value::Text(l.into()), Value::Int(p), Value::Int(y)],
            );
        }
        d
    }

    #[test]
    fn folds_accents_and_case() {
        assert_eq!(fold("Côte d’Ivoire"), "cote d'ivoire");
        assert_eq!(fold("ŁÓDŹ"), "lodz");
        assert_eq!(fold("Straße"), "strasse");
    }

    #[test]
    fn every_field_is_searched() {
        let ix = FindIndex::build(&ds());
        let rows = |q: &str| ix.find(q, None).iter().map(|h| h.row).collect::<Vec<_>>();
        assert_eq!(rows("cote"), vec![0]);
        assert_eq!(rows("residential"), vec![0, 2]);
        assert_eq!(rows("2019"), vec![0, 2]);
        assert_eq!(rows("1250"), vec![0]);
        assert_eq!(rows("1,250"), vec![0]);
        // Starting a word ranks above inside one: Pudong, then Huangpu.
        assert_eq!(rows("pu"), vec![2, 1]);
        assert!(rows("zzz").is_empty());
        assert!(rows("  ").is_empty());
    }

    #[test]
    fn whole_values_rank_first_and_filters_hide() {
        let ix = FindIndex::build(&ds());
        // "chin" starts a word in Chinatown; "pudong" is a whole value.
        let h = ix.find("pudong", None);
        assert_eq!((h[0].row, h[0].field, h[0].quality), (2, 0, 0));
        assert_eq!(ix.find("residential", Some(&[false, true, true, true])).iter().map(|h| h.row).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn the_match_is_found_in_the_original_text() {
        assert_eq!(match_range("Côte d'Ivoire", "cote"), Some(0..5));
        assert_eq!(match_range("Residential", "dent"), Some(4..8));
        assert_eq!(match_range("Straße", "strasse"), None);
    }
}
