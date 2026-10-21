//! Offline place search (WP1.5): the bundled GeoNames city index and an as-you-type matcher.
//!
//! The index is `assets/geonames/cities.tsv`, built by `tools/lm/build_city_index.py` (data from
//! GeoNames, geonames.org, CC BY 4.0). This module does no I/O: the caller passes the TSV text in
//! (the app embeds it with `include_str!`).
//!
//! # Matching
//! The query and each place's `name` and `ascii` name are *folded*: lower-cased, Latin accents
//! removed (é→e, ü→u, ß→ss, ø→o, ł→l …; Latin-1, Latin Extended-A and the Vietnamese horned
//! letters), hyphens and underscores read as spaces, apostrophes dropped, runs of spaces collapsed.
//! So "sao paulo" finds São Paulo and "xian" finds Xi'an. The Chinese name (`zh`) and a query in
//! Chinese are both folded to simplified characters ([`crate::zh_fold`]), so 纽约 finds New York
//! City, whose GeoNames name is the traditional 紐約, and 紐約 still does. A place matches in the
//! best of these classes, best first:
//!
//! 1. **Exact**: the folded name equals the query, or the Chinese name equals it, or equals a
//!    query of two or more characters plus one administrative suffix (成都 → 成都市).
//! 2. **Prefix**: the folded name, or the Chinese name, starts with the query.
//! 3. **Word prefix**: a later word of the folded name starts with the query ("york" finds New
//!    York City, "paulo" finds São Paulo). It ranks below whole-name prefixes so that typing the
//!    start of a name is never pushed down by places that merely contain the word.
//! 4. **Typo**: for queries of 4–7 characters, Damerau–Levenshtein (optimal string alignment)
//!    distance ≤ 1, and ≤ 2 for 8 or more, between the query and a prefix of the folded name of
//!    the query's length (or one shorter or longer, so a dropped or doubled letter also counts).
//!    Distance 1 ranks above distance 2.
//!
//! Within a class, larger population first, then smaller GeoNames id, so results are stable.

use std::cmp::{Ordering, Reverse};

/// One city from the bundled index.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// GeoNames id; breaks ranking ties.
    pub id: u64,
    /// Official name in UTF-8, e.g. "São Paulo".
    pub name: String,
    /// ASCII spelling, e.g. "Sao Paulo".
    pub ascii: String,
    /// Simplified-Chinese name, e.g. "成都市"; empty when GeoNames has none.
    pub zh: String,
    /// First-level region (state, province); may be empty.
    pub region: String,
    pub country: String,
    /// ISO 3166-1 alpha-2.
    pub country_code: String,
    pub lat: f64,
    pub lon: f64,
    pub population: u64,
}

/// How a place matched the query. Earlier variants rank higher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchKind {
    Exact,
    Prefix,
    WordPrefix,
    /// Matched with this many edits (1 or 2).
    Typo(u8),
}

/// One search result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit<'a> {
    pub place: &'a Place,
    /// Position of the place in [`PlaceIndex::places`].
    pub index: usize,
    pub kind: MatchKind,
}

/// Precomputed search keys for one place.
#[derive(Debug, Clone)]
struct Keys {
    name: Vec<char>,
    /// The Chinese name in simplified characters (empty when there is none).
    zh: String,
    /// The folded ASCII name, only when it differs from the folded name.
    ascii: Option<Vec<char>>,
}

/// The parsed city index, ready to search.
#[derive(Debug, Clone)]
pub struct PlaceIndex {
    places: Vec<Place>,
    keys: Vec<Keys>,
    skipped: usize,
}

/// Administrative suffixes that may follow a Chinese name for an exact match.
const ZH_SUFFIXES: [char; 6] = ['市', '区', '县', '镇', '旗', '盟'];

/// Longest query that is matched; longer input is truncated (no place name is this long).
const MAX_QUERY: usize = 64;

impl PlaceIndex {
    /// Parses the TSV written by `tools/lm/build_city_index.py`: eleven tab-separated columns,
    /// geonameid, name, asciiname, zh, admin1, country, country code, lat, lon, population,
    /// feature code. Malformed rows are skipped and counted ([`PlaceIndex::skipped`]). Fails only
    /// when no row is usable.
    pub fn parse(tsv: &str) -> Result<PlaceIndex, String> {
        let mut places = Vec::new();
        let mut skipped = 0;
        for line in tsv.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            match parse_row(line) {
                Some(p) => places.push(p),
                None => skipped += 1,
            }
        }
        if places.is_empty() {
            return Err(format!("place index has no usable rows ({skipped} malformed)"));
        }
        let keys = places
            .iter()
            .map(|p| {
                let name: Vec<char> = fold(&p.name).chars().collect();
                let ascii: Vec<char> = fold(&p.ascii).chars().collect();
                let zh = crate::zh_fold::to_simplified(&p.zh);
                Keys { ascii: (ascii != name && !ascii.is_empty()).then_some(ascii), name, zh }
            })
            .collect();
        Ok(PlaceIndex { places, keys, skipped })
    }

    /// All places, in file order (population descending).
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    pub fn len(&self) -> usize {
        self.places.len()
    }

    pub fn is_empty(&self) -> bool {
        self.places.is_empty()
    }

    /// Number of malformed rows that [`PlaceIndex::parse`] skipped.
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// Up to `limit` places matching `query`, best first (see the module docs for the rules).
    /// An empty or blank query matches nothing.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit<'_>> {
        let folded = fold(query);
        let q: Vec<char> = folded.chars().take(MAX_QUERY).collect();
        let raw: String = query.trim().chars().take(MAX_QUERY).collect();
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }
        let max_edits: u8 = match q.len() {
            0..=3 => 0,
            4..=7 => 1,
            _ => 2,
        };
        let has_han = raw.chars().any(is_han);
        let raw = if has_han { crate::zh_fold::to_simplified(&raw) } else { raw };
        let mut buf = vec![0u8; 3 * (q.len() + 1)];
        let mut hits: Vec<(MatchKind, usize)> = Vec::new();
        for (i, keys) in self.keys.iter().enumerate() {
            let mut best = None;
            if has_han && !keys.zh.is_empty() {
                best = match_zh(&keys.zh, &raw);
            }
            if best != Some(MatchKind::Exact) {
                for key in std::iter::once(&keys.name).chain(keys.ascii.as_ref()) {
                    if let Some(k) = match_latin(key, &q, max_edits, &mut buf) {
                        best = Some(best.map_or(k, |b: MatchKind| b.min(k)));
                    }
                }
            }
            if let Some(k) = best {
                hits.push((k, i));
            }
        }
        let rank = |&(k, i): &(MatchKind, usize)| (k, Reverse(self.places[i].population), self.places[i].id);
        if hits.len() > limit {
            hits.select_nth_unstable_by_key(limit - 1, rank);
            hits.truncate(limit);
        }
        hits.sort_unstable_by_key(rank);
        hits.into_iter().map(|(kind, index)| Hit { place: &self.places[index], index, kind }).collect()
    }
}

/// A web-map zoom level that frames a place of this population: `17 − log10(population)`,
/// clamped to 10…13. So 10 million and more → 10, 1 million → 11, 100,000 → 12, and 15,000
/// (the index's floor) → about 12.8; unknown (0) → 13.
pub fn zoom_for_population(pop: u64) -> f64 {
    let p = pop.max(1) as f64;
    (17.0 - p.log10()).clamp(10.0, 13.0)
}

fn parse_row(line: &str) -> Option<Place> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() != 11 {
        return None;
    }
    let id = f[0].trim().parse::<u64>().ok()?;
    let name = f[1].trim();
    if name.is_empty() {
        return None;
    }
    let lat = f[7].trim().parse::<f64>().ok().filter(|v| v.is_finite() && (-90.0..=90.0).contains(v))?;
    let lon = f[8].trim().parse::<f64>().ok().filter(|v| v.is_finite() && (-180.0..=180.0).contains(v))?;
    let population = f[9].trim().parse::<u64>().ok()?;
    Some(Place {
        id,
        name: name.to_string(),
        ascii: f[2].trim().to_string(),
        zh: f[3].trim().to_string(),
        region: f[4].trim().to_string(),
        country: f[5].trim().to_string(),
        country_code: f[6].trim().to_string(),
        lat,
        lon,
        population,
    })
}

fn match_zh(zh: &str, query: &str) -> Option<MatchKind> {
    let rest = zh.strip_prefix(query)?;
    let mut chars = rest.chars();
    match (chars.next(), chars.next()) {
        (None, _) => Some(MatchKind::Exact),
        // 成都 is exactly 成都市, but a lone 成 is not exactly 成县: one character is too little.
        (Some(c), None) if ZH_SUFFIXES.contains(&c) && query.chars().count() >= 2 => Some(MatchKind::Exact),
        _ => Some(MatchKind::Prefix),
    }
}

fn match_latin(key: &[char], q: &[char], max_edits: u8, buf: &mut [u8]) -> Option<MatchKind> {
    if key.starts_with(q) {
        return Some(if key.len() == q.len() { MatchKind::Exact } else { MatchKind::Prefix });
    }
    // A later word: the query must start right after a space.
    if key.len() > q.len() && key.windows(q.len() + 1).any(|w| w[0] == ' ' && &w[1..] == q) {
        return Some(MatchKind::WordPrefix);
    }
    if max_edits > 0
        && let Some(d) = prefix_distance(q, key, max_edits, buf)
    {
        return Some(MatchKind::Typo(d));
    }
    None
}

/// The smallest optimal-string-alignment distance between `q` and a prefix of `key` whose length
/// is within one of `q`'s, if it is ≤ `max`. Columns run over `key`, so the loop stops as soon as
/// every cell of a column exceeds `max` (no later column can then come back under it). `buf` holds
/// three columns, `3 × (q.len() + 1)` cells, reused across calls so a search allocates once.
fn prefix_distance(q: &[char], key: &[char], max: u8, buf: &mut [u8]) -> Option<u8> {
    let n = q.len();
    let w = n + 1;
    if buf.len() < 3 * w {
        return None;
    }
    for (i, cell) in buf[..w].iter_mut().enumerate() {
        *cell = u8::try_from(i).unwrap_or(u8::MAX);
    }
    let last = key.len().min(n + 1);
    // Cheap rejection: every distinct query letter missing from the compared prefix costs at least
    // one edit. Letters are hashed into 64 bits; a collision only weakens the filter.
    let mask = |cs: &[char]| cs.iter().fold(0u64, |m, &c| m | 1 << (u32::from(c) % 64));
    if (mask(q) & !mask(&key[..last])).count_ones() > u32::from(max) {
        return None;
    }
    let mut best: Option<u8> = None;
    for j in 1..=last {
        let kc = key[j - 1];
        let (cur, prev, prev2) = ((j % 3) * w, ((j + 2) % 3) * w, ((j + 1) % 3) * w);
        buf[cur] = u8::try_from(j).unwrap_or(u8::MAX);
        let mut col_min = buf[cur];
        for i in 1..=n {
            let cost = u8::from(q[i - 1] != kc);
            let mut v =
                (buf[prev + i - 1].saturating_add(cost)).min(buf[prev + i].saturating_add(1)).min(buf[cur + i - 1].saturating_add(1));
            if i > 1 && j > 1 && q[i - 1] == key[j - 2] && q[i - 2] == kc {
                v = v.min(buf[prev2 + i - 2].saturating_add(1));
            }
            buf[cur + i] = v;
            col_min = col_min.min(v);
        }
        let d = buf[cur + n];
        if j + 1 >= n && d <= max {
            best = Some(best.map_or(d, |b| b.min(d)));
        }
        if col_min > max {
            break;
        }
    }
    best
}

fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

/// Lower-cases, strips Latin diacritics, reads `-` and `_` as spaces, drops apostrophes and
/// collapses whitespace. Non-Latin text passes through lower-cased.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true; // suppress leading spaces
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            '\'' | '\u{2019}' | '\u{02BC}' | '`' | '\u{00B4}' | '\u{02BB}' => continue,
            c if c.is_whitespace() || c == '-' || c == '_' || c == '\u{2010}' || c == '\u{2013}' => {
                if !space {
                    out.push(' ');
                    space = true;
                }
                continue;
            }
            '\u{0300}'..='\u{036F}' => continue, // combining marks (decomposed input)
            _ => {}
        }
        space = false;
        match fold_char(c) {
            Folded::One(f) => out.push(f),
            Folded::Two(a, b) => {
                out.push(a);
                out.push(b);
            }
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

enum Folded {
    One(char),
    Two(char, char),
}

/// Folds one lower-case letter of Latin-1 Supplement, Latin Extended-A, and a few of
/// Latin Extended-B (Vietnamese ơ/ư, Romanian ș/ț, pinyin ǎ/ǐ/ǒ/ǔ) to its base letter(s).
fn fold_char(c: char) -> Folded {
    use Folded::{One, Two};
    let base = match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' | 'ǎ' => 'a',
        'æ' => return Two('a', 'e'),
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' | 'ð' => 'd',
        'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' | 'ǐ' => 'i',
        'ĳ' => return Two('i', 'j'),
        'ĵ' => 'j',
        'ķ' | 'ĸ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => 'n',
        'ò'..='ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'ơ' | 'ǒ' => 'o',
        'œ' => return Two('o', 'e'),
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' | 'ſ' => 's',
        'ß' => return Two('s', 's'),
        'ţ' | 'ť' | 'ŧ' | 'ț' => 't',
        'þ' => return Two('t', 'h'),
        'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ư' | 'ǔ' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        other => other,
    };
    One(base)
}

impl Hit<'_> {
    /// Compares two hits by rank (best first).
    pub fn rank_cmp(&self, other: &Hit<'_>) -> Ordering {
        (self.kind, Reverse(self.place.population), self.place.id).cmp(&(other.kind, Reverse(other.place.population), other.place.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    const TSV: &str = include_str!("../../../assets/geonames/cities.tsv");

    fn index() -> &'static PlaceIndex {
        static INDEX: OnceLock<PlaceIndex> = OnceLock::new();
        INDEX.get_or_init(|| PlaceIndex::parse(TSV).unwrap())
    }

    fn first(q: &str) -> &'static Place {
        index().search(q, 10).first().unwrap_or_else(|| panic!("no result for {q:?}")).place
    }

    #[test]
    fn parses_the_bundled_index() {
        let idx = index();
        assert!(idx.len() > 30_000, "{} places", idx.len());
        assert_eq!(idx.skipped(), 0);
    }

    #[test]
    fn shanghai() {
        let p = first("Shanghai");
        assert_eq!((p.name.as_str(), p.country_code.as_str()), ("Shanghai", "CN"));
        assert_eq!(index().search("Shanghai", 1)[0].kind, MatchKind::Exact);
        assert_eq!(first("shang").name, "Shanghai");
    }

    #[test]
    fn chinese_name_prefix() {
        let hits = index().search("成都", 5);
        assert_eq!(hits[0].place.name, "Chengdu");
        assert_eq!(hits[0].place.zh, "成都市");
        assert_eq!(hits[0].kind, MatchKind::Exact);
        assert_eq!(first("成").name, "Chengdu");
        assert_eq!(first("上海").name, "Shanghai");
        assert_eq!(first("东京").name, "Tokyo");
    }

    /// GeoNames gives some places traditional-character names (紐約): a simplified query finds
    /// them, the traditional query still does, and the stored name is shown as it is.
    #[test]
    fn simplified_queries_find_traditional_names() {
        let nyc = first("纽约");
        assert_eq!((nyc.name.as_str(), nyc.zh.as_str()), ("New York City", "紐約"));
        assert_eq!(index().search("纽约", 1)[0].kind, MatchKind::Exact);
        assert_eq!(first("紐約").name, "New York City");
        assert_eq!(first("达喀尔").name, "Dakar");
        assert_eq!(first("達喀爾").name, "Dakar");
        assert_eq!(first("布宜诺斯艾利斯").name, "Buenos Aires");
        assert_eq!(first("斋浦尔").name, "Jaipur");
    }

    /// Folding leaves the names that were already simplified where they were.
    #[test]
    fn simplified_names_are_unchanged() {
        assert_eq!(first("东京").name, "Tokyo");
        assert_eq!(first("東京").name, "Tokyo");
        assert_eq!(first("巴塞罗那").name, "Barcelona");
        assert_eq!(first("成都").zh, "成都市");
    }

    /// Latin queries never take the Chinese path: their results are as before.
    #[test]
    fn english_search_is_unchanged() {
        for (q, name) in [
            ("New York", "New York City"),
            ("Tokyo", "Tokyo"),
            ("Barcelona", "Barcelona"),
            ("new york", "New York City"),
            ("sao paulo", "São Paulo"),
            ("Dakar", "Dakar"),
            ("Buenos", "Buenos Aires"),
            ("Jaipur", "Jaipur"),
        ] {
            assert_eq!(first(q).name, name, "{q}");
        }
    }

    #[test]
    fn lisbon() {
        // GeoNames' official name is the English "Lisbon"; "Lisboa" is only an alternate name.
        let p = first("Lisbon");
        assert_eq!((p.name.as_str(), p.country_code.as_str()), ("Lisbon", "PT"));
        assert_eq!(p.region, "Lisbon");
        assert_eq!(p.country, "Portugal");
    }

    #[test]
    fn typos() {
        let hits = index().search("Shangahi", 5);
        assert_eq!(hits[0].place.name, "Shanghai");
        assert_eq!(hits[0].kind, MatchKind::Typo(1));
        assert_eq!(first("Lisbin").name, "Lisbon");
        assert_eq!(first("Chengud").name, "Chengdu");
        assert_eq!(first("Shaghai").name, "Shanghai"); // dropped letter
        // A real prefix outranks a typo: "Shanhai" is the start of Shanhaiguan.
        assert_eq!(first("Shanhai").name, "Shanhaiguan");
        assert_eq!(first("Shanghaai").name, "Shanghai"); // doubled letter
        assert_eq!(first("Barcelnoa").name, "Barcelona");
        // Three-letter queries get no typo tolerance.
        assert!(index().search("qxz", 5).is_empty());
    }

    #[test]
    fn diacritics_fold() {
        let p = first("sao paulo");
        assert_eq!((p.name.as_str(), p.country_code.as_str()), ("São Paulo", "BR"));
        assert_eq!(first("São Paulo").name, "São Paulo");
        assert_eq!(first("zurich").name, "Zürich");
        assert_eq!(first("ZÜRICH").name, "Zürich");
        assert_eq!(fold("  Winston-Salem  "), "winston salem");
        assert_eq!(fold("Xi'an"), "xian");
        assert_eq!(fold("Łódź Straße Œuvre"), "lodz strasse oeuvre");
        assert_eq!(fold("Sa\u{0303}o"), "sao");
    }

    #[test]
    fn word_prefix_ranks_below_name_prefix() {
        let hits = index().search("york", 20);
        assert_eq!(hits[0].place.name, "York");
        assert_eq!(hits[0].kind, MatchKind::Exact);
        let nyc = hits.iter().position(|h| h.place.name == "New York City").unwrap();
        assert_eq!(hits[nyc].kind, MatchKind::WordPrefix);
        assert!(hits[..nyc].iter().all(|h| h.kind < MatchKind::WordPrefix));
        assert_eq!(first("new york").name, "New York City");
        // "paulo": Paulo Afonso (whole-name prefix) first, then São Paulo (word prefix).
        let hits = index().search("paulo", 10);
        assert_eq!(hits[0].place.name, "Paulo Afonso");
        assert!(hits.iter().any(|h| h.place.name == "São Paulo" && h.kind == MatchKind::WordPrefix));
    }

    #[test]
    fn ranking_is_sorted_and_deterministic() {
        for q in ["san", "lon", "par", "york", "成", "berlni", "a"] {
            let a = index().search(q, 50);
            let b = index().search(q, 50);
            assert_eq!(a, b);
            assert!(a.windows(2).all(|w| w[0].rank_cmp(&w[1]) == Ordering::Less), "{q}");
        }
        assert_eq!(first("lon").name, "London");
        assert_eq!(index().search("a", 7).len(), 7);
    }

    #[test]
    fn empty_query_matches_nothing() {
        assert!(index().search("", 10).is_empty());
        assert!(index().search("   ", 10).is_empty());
        assert!(index().search("-'", 10).is_empty());
        assert!(index().search("Paris", 0).is_empty());
    }

    #[test]
    fn malformed_rows_are_skipped() {
        let tsv = "\
1\tGood\tGood\t\tRegion\tCountry\tGC\t10.0\t20.0\t5000\tPPL
not-a-number\tBad\tBad\t\tR\tC\tXX\t1\t2\t3\tPPL
2\tShort row\tShort
3\tBadLat\tBadLat\t\tR\tC\tXX\t95.0\t2\t3\tPPL
4\tBadPop\tBadPop\t\tR\tC\tXX\t1\t2\t-3\tPPL
5\t\tEmpty\t\tR\tC\tXX\t1\t2\t3\tPPL

6\tAlso good\tAlso good\t好\tR\tC\tAG\t-1.5\t-2.5\t10\tPPL\r
";
        let idx = PlaceIndex::parse(tsv).unwrap();
        assert_eq!(idx.len(), 2);
        assert_eq!(idx.skipped(), 5);
        assert_eq!(idx.places()[1].country_code, "AG");
        assert_eq!(idx.search("好", 5)[0].place.name, "Also good");
        assert!(PlaceIndex::parse("").is_err());
        assert!(PlaceIndex::parse("garbage\nmore garbage").is_err());
    }

    #[test]
    fn zoom_from_population() {
        assert_eq!(zoom_for_population(25_000_000), 10.0);
        assert!((zoom_for_population(1_000_000) - 11.0).abs() < 1e-9);
        assert!((zoom_for_population(100_000) - 12.0).abs() < 1e-9);
        let z = zoom_for_population(15_000);
        assert!(z > 12.5 && z <= 13.0, "{z}");
        assert_eq!(zoom_for_population(0), 13.0);
        assert!(zoom_for_population(2_000_000) < zoom_for_population(200_000));
    }

    #[test]
    fn search_is_fast() {
        let idx = index();
        let queries = ["s", "sh", "sha", "shan", "shang", "shangh", "shangha", "shanghai", "lisb", "成都", "sao pa", "barcelnoa"];
        let t = std::time::Instant::now();
        let mut n = 0;
        for i in 0..100 {
            n += idx.search(queries[i % queries.len()], 8).len();
        }
        let total = t.elapsed();
        assert!(n > 0);
        eprintln!("place search: {:?} per query over {} places", total / 100, idx.len());
        // Plan: under 2 ms per keystroke on an M4 in release (measured ≈ 0.7 ms). Test builds
        // (opt-level 1, other tests running in parallel) get a generous bound so CI is not flaky.
        let bound_ms = if cfg!(debug_assertions) { 1000 } else { 200 };
        assert!(total.as_millis() < bound_ms, "100 queries took {total:?}");
    }
}
