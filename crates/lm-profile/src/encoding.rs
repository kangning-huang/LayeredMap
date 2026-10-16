//! Catalog row "Shapefile text with no .cpg encoding file".

use crate::{lines, make};
use lm_core::model::{Assumption, AssumptionKind, Choice, Confidence, Consequence, Known, Subject, Treatment};

/// A text encoding for attribute text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    /// GBK (decoded as GB18030, its superset).
    Gbk,
    /// Western European (Windows-1252, the usual meaning of "Latin-1" in .dbf files).
    Windows1252,
    /// Any other encoding a `.cpg` names, by its WHATWG label ("Shift_JIS", "Big5").
    Named(String),
}

impl TextEncoding {
    /// Name for Details ("UTF-8", "GBK").
    pub fn label(&self) -> String {
        match self {
            TextEncoding::Utf8 => "UTF-8".into(),
            TextEncoding::Gbk => "GBK".into(),
            TextEncoding::Windows1252 => "Windows-1252".into(),
            TextEncoding::Named(n) => n.clone(),
        }
    }

    /// The `encoding_rs` encoding to decode with.
    pub fn encoding(&self) -> &'static encoding_rs::Encoding {
        match self {
            TextEncoding::Utf8 => encoding_rs::UTF_8,
            TextEncoding::Gbk => encoding_rs::GB18030,
            TextEncoding::Windows1252 => encoding_rs::WINDOWS_1252,
            TextEncoding::Named(n) => encoding_rs::Encoding::for_label(n.as_bytes()).unwrap_or(encoding_rs::UTF_8),
        }
    }

    /// Decodes bytes (malformed sequences become U+FFFD; a BOM is not stripped).
    pub fn decode(&self, bytes: &[u8]) -> String {
        self.encoding().decode_without_bom_handling(bytes).0.into_owned()
    }

    /// Reads a `.cpg` file's contents ("UTF-8", "65001", "GBK", "936", "ANSI 1252", "Big5").
    pub fn from_cpg(cpg: &str) -> Option<TextEncoding> {
        let t = cpg.trim().trim_start_matches('\u{feff}').trim();
        let l = t.to_ascii_lowercase();
        let l = l.strip_prefix("ansi").map(str::trim).unwrap_or(&l).to_string();
        match l.as_str() {
            "" => None,
            "utf-8" | "utf8" | "65001" => Some(TextEncoding::Utf8),
            "gbk" | "936" | "cp936" | "gb2312" | "gb18030" | "54936" => Some(TextEncoding::Gbk),
            "1252" | "cp1252" | "windows-1252" | "latin1" | "iso-8859-1" | "iso8859-1" | "88591" => Some(TextEncoding::Windows1252),
            other => {
                let label = if other.chars().all(|c| c.is_ascii_digit()) { format!("windows-{other}") } else { other.to_string() };
                encoding_rs::Encoding::for_label(label.as_bytes()).map(|e| TextEncoding::Named(e.name().to_string()))
            }
        }
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{3000}'..='\u{303F}' | '\u{FF00}'..='\u{FFEF}')
}

/// A hanzi among GB2312's level 1: the 3,755 most common characters (GBK lead byte 0xB0–0xD7).
fn is_common_hanzi(c: char) -> bool {
    let mut buf = [0u8; 4];
    let (bytes, _, unmappable) = encoding_rs::GBK.encode(c.encode_utf8(&mut buf));
    !unmappable && bytes.len() == 2 && bytes.first().is_some_and(|b| (0xB0..=0xD7).contains(b))
}

/// Share of the decoded characters that are CJK, and of the hanzi that are common.
const GBK_CJK_SHARE: f64 = 0.9;
const GBK_COMMON_SHARE: f64 = 0.8;

/// True when the bytes read plausibly as Western text in Windows-1252: most non-ASCII
/// characters are Latin letters (À–ÿ), and most stand alone between ASCII characters ("São",
/// "Zürich"). GBK text has its non-ASCII bytes in pairs and runs.
fn plausible_windows1252(non_ascii: &[&[u8]]) -> bool {
    let (mut total, mut letters, mut alone) = (0usize, 0usize, 0usize);
    for s in non_ascii {
        let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(s);
        let chars: Vec<char> = text.chars().collect();
        for (i, c) in chars.iter().enumerate().filter(|(_, c)| !c.is_ascii()) {
            total += 1;
            if matches!(c, '\u{C0}'..='\u{FF}') && !matches!(c, '×' | '÷') {
                letters += 1;
            }
            let ascii_at = |j: Option<usize>| j.and_then(|j| chars.get(j)).is_none_or(char::is_ascii);
            if ascii_at(i.checked_sub(1)) && ascii_at(Some(i + 1)) {
                alone += 1;
            }
        }
    }
    total > 0 && letters as f64 >= 0.8 * total as f64 && alone as f64 >= 0.5 * total as f64
}

fn choices(first: &str) -> Vec<Choice> {
    let mut all = vec![
        Choice::new("gbk", "Chinese (GBK)"),
        Choice::new("utf8", "Unicode (UTF-8)"),
        Choice::new("windows1252", "Western (Windows-1252)"),
    ];
    if let Some(i) = all.iter().position(|c| c.id == first) {
        let c = all.remove(i);
        all.insert(0, c);
    }
    all
}

/// The encoding of a Shapefile's attribute text.
///
/// - `samples`: raw bytes of text cells from the `.dbf` (a few hundred cells are plenty; trailing
///   spaces may be left in).
/// - `cpg`: the `.cpg` file's contents, if there is one.
///
/// A readable `.cpg` decides (`Certain`, no assumption). Without one: text that is valid UTF-8
/// (plain ASCII included) is UTF-8 (`Likely`). Otherwise the text is decoded as GBK:
///
/// - **Strong evidence** (no decoding errors; at least 90% of the non-ASCII characters are CJK;
///   at least 80% of the hanzi are among GB2312 level 1, the 3,755 common ones; and the bytes
///   don't read plausibly as Western text): GBK, `Likely`, the silent
///   [`AssumptionKind::ShapefileGbk`] for Details (the catalog row).
/// - **Weaker evidence** (decodes cleanly with at least as many CJK as other characters, and
///   doesn't read as Western text): GBK is still the best guess, `Possible`, and the same
///   assumption is made *visible*, since a wrong guess garbles every label.
/// - Otherwise Windows-1252, `Possible`, with a visible `ShapefileGbk` assumption choosing
///   Windows-1252 ("Read text as Western (Windows-1252)." + Change; GBK and UTF-8 offered).
pub fn shapefile_encoding(samples: &[&[u8]], cpg: Option<&str>) -> (Known<TextEncoding>, Option<Assumption>) {
    if let Some(e) = cpg.and_then(TextEncoding::from_cpg) {
        return (Known::certain(e, "Named by the .cpg file."), None);
    }
    let non_ascii: Vec<&[u8]> = samples.iter().copied().filter(|s| !s.is_ascii()).collect();
    if non_ascii.iter().all(|s| std::str::from_utf8(s).is_ok()) {
        let why = if non_ascii.is_empty() { "The text is plain ASCII." } else { "The text is valid UTF-8." };
        return (Known::likely(TextEncoding::Utf8, why), None);
    }
    let (mut errors, mut cjk, mut other, mut hanzi, mut common) = (false, 0usize, 0usize, 0usize, 0usize);
    for s in &non_ascii {
        let (text, had_errors) = encoding_rs::GBK.decode_without_bom_handling(s);
        errors |= had_errors;
        for c in text.chars().filter(|c| !c.is_ascii()) {
            if is_cjk(c) {
                cjk += 1;
                if ('\u{4E00}'..='\u{9FFF}').contains(&c) {
                    hanzi += 1;
                    common += usize::from(is_common_hanzi(c));
                }
            } else {
                other += 1;
            }
        }
    }
    let western = plausible_windows1252(&non_ascii);
    let gbk_line = |confidence, treatment| {
        make(
            AssumptionKind::ShapefileGbk,
            Subject::Layer,
            Choice::new("gbk", "Chinese (GBK)"),
            choices("gbk"),
            confidence,
            Consequence::DisplayOnly,
            treatment,
            lines::SHAPEFILE_GBK,
        )
    };
    let strong = !errors
        && cjk as f64 >= GBK_CJK_SHARE * (cjk + other) as f64
        && hanzi > 0
        && common as f64 >= GBK_COMMON_SHARE * hanzi as f64
        && !western;
    if strong {
        let why =
            format!("No .cpg file; the text is not UTF-8, and as GBK it reads as {cjk} Chinese characters, {common} of {hanzi} common.");
        return (Known::likely(TextEncoding::Gbk, why), Some(gbk_line(Confidence::Likely, Treatment::Silent)));
    }
    if !errors && cjk > 0 && cjk >= other && !western {
        let why = format!(
            "No .cpg file; the text is not UTF-8, and as GBK it reads as {cjk} Chinese characters, but only {common} of {hanzi} are common ones."
        );
        return (Known::possible(TextEncoding::Gbk, why), Some(gbk_line(Confidence::Possible, Treatment::Visible)));
    }
    let a = make(
        AssumptionKind::ShapefileGbk,
        Subject::Layer,
        Choice::new("windows1252", "Western (Windows-1252)"),
        choices("windows1252"),
        Confidence::Possible,
        Consequence::DisplayOnly,
        Treatment::Visible,
        lines::SHAPEFILE_WINDOWS1252,
    );
    (Known::possible(TextEncoding::Windows1252, "No .cpg file; the text is neither UTF-8 nor clearly Chinese (GBK)."), Some(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gbk(s: &str) -> Vec<u8> {
        encoding_rs::GBK.encode(s).0.into_owned()
    }

    #[test]
    fn shapefile_gbk_matches_gbk_text_without_cpg() {
        let cells = [gbk("人民广场"), gbk("黄浦区"), b"Park".to_vec()];
        let refs: Vec<&[u8]> = cells.iter().map(Vec::as_slice).collect();
        let (enc, a) = shapefile_encoding(&refs, None);
        assert_eq!(enc.value, TextEncoding::Gbk);
        let a = a.unwrap();
        assert_eq!(a.kind, AssumptionKind::ShapefileGbk);
        assert_eq!(a.treatment, Treatment::Silent);
        assert_eq!(a.line, "Read text as Chinese (GBK).");
        assert_eq!(enc.value.decode(&cells[0]), "人民广场");
    }

    #[test]
    fn shapefile_gbk_does_not_match_utf8_cpg_or_latin1() {
        let utf8 = ["人民广场".as_bytes(), b"abc"];
        assert_eq!(shapefile_encoding(&utf8, None), (Known::likely(TextEncoding::Utf8, "The text is valid UTF-8."), None));
        let g = gbk("人民广场");
        let (enc, a) = shapefile_encoding(&[g.as_slice()], Some("GBK"));
        assert_eq!((enc.value, enc.confidence, a), (TextEncoding::Gbk, Confidence::Certain, None));
        let (enc, a) = shapefile_encoding(&[g.as_slice()], Some("UTF-8"));
        assert_eq!((enc.value, a), (TextEncoding::Utf8, None));
        // Western text in Windows-1252: "Café Müller".
        let latin = encoding_rs::WINDOWS_1252.encode("Café Müller").0.into_owned();
        let (enc, a) = shapefile_encoding(&[latin.as_slice()], None);
        assert_eq!((enc.value, a.map(|a| a.chosen.id)), (TextEncoding::Windows1252, Some("windows1252".to_string())));
    }

    fn w1252(s: &str) -> Vec<u8> {
        encoding_rs::WINDOWS_1252.encode(s).0.into_owned()
    }

    #[test]
    fn western_text_without_cpg_is_not_read_as_gbk() {
        // Regression (S2): "São Paulo" in Windows-1252 decodes cleanly as GBK ("S鉶 Paulo").
        for words in [&["São Paulo", "Rio de Janeiro"][..], &["Montréal", "Québec"], &["Zürich", "Genève"], &["Ñuñoa"]] {
            let cells: Vec<Vec<u8>> = words.iter().map(|w| w1252(w)).collect();
            let refs: Vec<&[u8]> = cells.iter().map(Vec::as_slice).collect();
            let (enc, a) = shapefile_encoding(&refs, None);
            assert_eq!(enc.value, TextEncoding::Windows1252, "{words:?}");
            assert_eq!(enc.value.decode(&cells[0]), words[0]);
            let a = a.unwrap();
            assert_eq!((a.kind, a.treatment, a.chosen.id.as_str()), (AssumptionKind::ShapefileGbk, Treatment::Visible, "windows1252"));
            assert_eq!(a.line, "Read text as Western (Windows-1252).");
            let ids: Vec<&str> = a.alternatives.iter().map(|c| c.id.as_str()).collect();
            assert_eq!(ids, vec!["windows1252", "gbk", "utf8"]);
        }
    }

    #[test]
    fn rare_hanzi_are_gbk_but_visible() {
        // GBK-decodable, but outside the 3,755 common characters: still the best guess, shown.
        let cells = [gbk("鑫淼"), gbk("焱")];
        let refs: Vec<&[u8]> = cells.iter().map(Vec::as_slice).collect();
        let (enc, a) = shapefile_encoding(&refs, None);
        assert_eq!((enc.value, enc.confidence), (TextEncoding::Gbk, Confidence::Possible));
        let a = a.unwrap();
        assert_eq!((a.kind, a.treatment, a.confidence), (AssumptionKind::ShapefileGbk, Treatment::Visible, Confidence::Possible));
        assert_eq!(a.line, "Read text as Chinese (GBK).");
    }

    #[test]
    fn cpg_labels() {
        assert_eq!(TextEncoding::from_cpg("ANSI 1252"), Some(TextEncoding::Windows1252));
        assert_eq!(TextEncoding::from_cpg("936\n"), Some(TextEncoding::Gbk));
        assert_eq!(TextEncoding::from_cpg("utf8"), Some(TextEncoding::Utf8));
        assert_eq!(TextEncoding::from_cpg("Big5"), Some(TextEncoding::Named("Big5".into())));
        assert_eq!(TextEncoding::from_cpg("  "), None);
        assert_eq!(TextEncoding::from_cpg("nonsense"), None);
    }
}
