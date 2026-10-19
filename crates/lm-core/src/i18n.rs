//! Interface languages (WP5.5): NimbleMap's thin layer over upstream's translation scheme
//! (`crates/ui-egui/src/i18n`), with the same catalog format and rules. Strings in the code stay
//! English and are the lookup keys; a catalog per language (`i18n/<code>.tsv`) maps them to
//! display text when the interface is drawn. A string without a translation shows in English.
//!
//! Why not upstream's module itself: it lives in `rustgis-ui-egui`, the ribbon front end that
//! NimbleMap doesn't use (the module table keeps `lm-*` off it), and its catalog holds the
//! ribbon's own strings. The format, the `{name}` placeholders, `tl!`, `fmt` and the `auto`
//! preference are the same, so a translator who knows one knows both.
//!
//! What is never translated: field names, layer names, file contents, dataset values, command
//! ids, saved files and the CLI. Only text the interface writes itself goes through here.
//!
//! The language preference is [`AUTO`] by default: the interface follows the system's preferred
//! languages (the first one with a catalog), else English. View › Language picks one by hand.
//! Looking the system up is the app's job (this crate does no I/O): see [`resolve`].
//!
//! # Catalog format
//! UTF-8, one `English<TAB>Translation` pair per line; `#` starts a comment line; `\n` and `\t`
//! inside a field are a line break and a tab. `{name}` placeholders must be kept (they may move).
//! The app's name never appears in a catalog: messages take it as `{app}` from [`crate::brand`].
//!
//! # Adding a string
//! Wrap it in `tl!("…")` where it is drawn, or `i18n::fmt("… {n} …", &[("n", …)])` for a message
//! with values; mark an English string that is drawn later (an id that doubles as a label) with
//! `tn!("…")`. Then add it to every catalog (the `catalogs` test in `lm-ui` lists what's missing).

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The preference value that follows the system.
pub const AUTO: &str = "auto";

/// Which CJK face a language needs (glyph shapes differ by region; hangul isn't in the Chinese
/// face at all).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cjk {
    SimplifiedChinese,
    Japanese,
    Korean,
}

/// How a language writes numbers: the thousands separator and the decimal mark (CLDR).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Numbers {
    pub group: &'static str,
    pub decimal: &'static str,
    /// Group only numbers with at least this many digits before the decimal mark (CLDR's
    /// minimum grouping digits: Spanish writes 1000 but 10 000).
    pub min_group_digits: usize,
}

const COMMA_DOT: Numbers = Numbers { group: ",", decimal: ".", min_group_digits: 4 };
const DOT_COMMA: Numbers = Numbers { group: ".", decimal: ",", min_group_digits: 4 };

pub struct LangInfo {
    /// Lowercase BCP 47 code; also the preference value.
    pub code: &'static str,
    /// The language's name in itself (the menu lists it so).
    pub name: &'static str,
    /// The name in English.
    pub english: &'static str,
    pub cjk: Option<Cjk>,
    pub numbers: Numbers,
    source: &'static str,
    catalog: OnceLock<HashMap<&'static str, &'static str>>,
}

macro_rules! lang {
    ($code:literal, $name:literal, $en:literal, $cjk:expr, $numbers:expr, $file:literal) => {
        LangInfo {
            code: $code,
            name: $name,
            english: $en,
            cjk: $cjk,
            numbers: $numbers,
            source: include_str!($file),
            catalog: OnceLock::new(),
        }
    };
}

/// The registry, in the order the language menu lists it. English first: it is the source
/// language and the fallback. The choice of languages: docs/lm/localization.md.
pub static LANGUAGES: [LangInfo; 11] = [
    LangInfo { code: "en", name: "English", english: "English", cjk: None, numbers: COMMA_DOT, source: "", catalog: OnceLock::new() },
    lang!("zh", "简体中文", "Chinese (Simplified)", Some(Cjk::SimplifiedChinese), COMMA_DOT, "i18n/zh.tsv"),
    lang!("de", "Deutsch", "German", None, DOT_COMMA, "i18n/de.tsv"),
    // Spanish as the RAE's Ortografía recommends, and as the Spanish website writes: a space
    // between groups of three from five digits on (1000, 14 537) and the decimal point.
    lang!("es", "Español", "Spanish", None, Numbers { group: "\u{A0}", decimal: ".", min_group_digits: 5 }, "i18n/es.tsv"),
    lang!("fr", "Français", "French", None, Numbers { group: "\u{202F}", decimal: ",", min_group_digits: 4 }, "i18n/fr.tsv"),
    lang!("it", "Italiano", "Italian", None, DOT_COMMA, "i18n/it.tsv"),
    lang!("nl", "Nederlands", "Dutch", None, DOT_COMMA, "i18n/nl.tsv"),
    lang!("pt", "Português (Brasil)", "Portuguese (Brazil)", None, DOT_COMMA, "i18n/pt.tsv"),
    lang!("ru", "Русский", "Russian", None, Numbers { group: "\u{A0}", decimal: ",", min_group_digits: 4 }, "i18n/ru.tsv"),
    lang!("ja", "日本語", "Japanese", Some(Cjk::Japanese), COMMA_DOT, "i18n/ja.tsv"),
    lang!("ko", "한국어", "Korean", Some(Cjk::Korean), COMMA_DOT, "i18n/ko.tsv"),
];

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(o) => {
                    out.push('\\');
                    out.push(o);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parses a catalog. Malformed lines are skipped and reported.
pub fn parse(source: &str) -> (HashMap<String, String>, Vec<String>) {
    let mut map = HashMap::new();
    let mut errors = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once('\t') {
            Some((en, tr)) if !en.is_empty() && !tr.trim().is_empty() => {
                if map.insert(unescape(en), unescape(tr)).is_some() {
                    errors.push(format!("line {}: {en:?} is listed twice", i + 1));
                }
            }
            _ => errors.push(format!("line {}: expected English<TAB>translation", i + 1)),
        }
    }
    (map, errors)
}

impl LangInfo {
    fn catalog(&self) -> &HashMap<&'static str, &'static str> {
        self.catalog.get_or_init(|| {
            let (map, _errors) = parse(self.source);
            // Leaked once per language for the life of the process, so lookups return 'static.
            map.into_iter().map(|(k, v)| (&*Box::leak(k.into_boxed_str()), &*Box::leak(v.into_boxed_str()))).collect()
        })
    }
}

/// A language the interface can be shown in.
#[derive(Clone, Copy)]
pub struct Lang(&'static LangInfo);

impl PartialEq for Lang {
    fn eq(&self, o: &Self) -> bool {
        self.0.code == o.0.code
    }
}

impl Eq for Lang {}

impl std::fmt::Debug for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lang({})", self.0.code)
    }
}

impl Lang {
    pub const EN: Lang = Lang(&LANGUAGES[0]);

    pub fn code(self) -> &'static str {
        self.0.code
    }
    pub fn name(self) -> &'static str {
        self.0.name
    }
    pub fn english(self) -> &'static str {
        self.0.english
    }
    pub fn cjk(self) -> Option<Cjk> {
        self.0.cjk
    }
    pub fn numbers(self) -> Numbers {
        self.0.numbers
    }
    /// The catalog's source text (for tests and tools).
    pub fn source(self) -> &'static str {
        self.0.source
    }
    pub fn from_code(code: &str) -> Option<Lang> {
        LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(code.trim())).map(Lang)
    }
    pub fn all() -> impl Iterator<Item = Lang> {
        LANGUAGES.iter().map(Lang)
    }
    /// Number of translated strings.
    pub fn len(self) -> usize {
        self.0.catalog().len()
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// The registered language for a locale tag such as `de_DE.UTF-8`, `pt-BR` or `zh-Hans-CN`.
pub fn lang_from_tag(tag: &str) -> Option<Lang> {
    let base = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase();
    if matches!(base.as_str(), "c" | "posix") {
        return Some(Lang::EN);
    }
    let mut parts = base.split('-');
    let primary = parts.next().unwrap_or("");
    // Only Simplified Chinese has a catalog: Traditional-script locales fall back to English
    // rather than get Simplified text.
    if primary == "zh" && parts.any(|p| matches!(p, "hant" | "tw" | "hk" | "mo")) {
        return None;
    }
    Lang::from_code(primary)
}

/// The first of the system's preferred languages that has a catalog.
pub fn first_supported<'a>(tags: impl IntoIterator<Item = &'a str>) -> Option<Lang> {
    tags.into_iter().find_map(lang_from_tag)
}

/// The language for a preference: a code, or [`AUTO`] (or anything unknown) for the first of the
/// system's preferred languages (`system`, most preferred first) that has a catalog, else English.
pub fn resolve<'a>(pref: &str, system: impl IntoIterator<Item = &'a str>) -> Lang {
    Lang::from_code(pref).unwrap_or_else(|| first_supported(system).unwrap_or(Lang::EN))
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::EN) };
}

/// Sets the language the interface is drawn in (on the thread that draws it).
pub fn set_current(lang: Lang) {
    CURRENT.set(lang);
}

pub fn current() -> Lang {
    CURRENT.get()
}

/// `s` in the current language.
pub fn t(s: &str) -> &str {
    tr(current(), s)
}

/// `s` in `lang`; strings without a translation come back unchanged.
pub fn tr(lang: Lang, s: &str) -> &str {
    lang.0.catalog().get(s).copied().unwrap_or(s)
}

/// Translates `template`, then fills its `{name}` placeholders in one pass (values are never
/// re-read as templates, so a layer named `{n}` stays intact).
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    fill(t(template), args)
}

/// Fills `{name}` placeholders without translating.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, after)) = rest.split_once('{') {
        out.push_str(before);
        let Some((name, tail)) = after.split_once('}') else {
            out.push('{');
            out.push_str(after);
            return out;
        };
        match args.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => out.push_str(v),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// The `{name}` placeholders in a string, sorted (for checking catalogs).
pub fn placeholders(s: &str) -> Vec<&str> {
    let mut v: Vec<&str> = s.split('{').skip(1).filter_map(|p| p.split_once('}').map(|x| x.0)).collect();
    v.sort_unstable();
    v
}

/// A number the interface wrote the English way ("1,248.5", "−0.4", "12%") in the current
/// language's style: "1.248,5" in German, "1 248,5" in French. Text that isn't a plain number
/// comes back as it is, so it is safe on any label; English is never changed.
pub fn number(s: &str) -> String {
    localize_number(current(), s)
}

/// [`number`] for a given language.
pub fn localize_number(lang: Lang, s: &str) -> String {
    let n = lang.numbers();
    if n == COMMA_DOT || s.is_empty() {
        return s.to_string();
    }
    // Sign, digits with English grouping, an optional fraction, then anything (a unit, "%").
    let (sign, body) = match s.char_indices().next() {
        Some((_, c @ ('-' | '−' | '+'))) => (&s[..c.len_utf8()], &s[c.len_utf8()..]),
        _ => ("", s),
    };
    let end = body.find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.')).unwrap_or(body.len());
    let (num, tail) = body.split_at(end);
    let (int, frac) = match num.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (num, None),
    };
    let digits: String = int.chars().filter(char::is_ascii_digit).collect();
    let well_formed = !digits.is_empty()
        && frac.is_none_or(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()))
        && (!int.contains(',') || int.split(',').skip(1).all(|g| g.len() == 3));
    if !well_formed {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 4);
    out.push_str(sign);
    if digits.len() >= n.min_group_digits {
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push_str(n.group);
            }
            out.push(c);
        }
    } else {
        out.push_str(&digits);
    }
    if let Some(f) = frac {
        out.push_str(n.decimal);
        out.push_str(f);
    }
    out.push_str(tail);
    out
}

/// A count in the current language's style: 1248 → "1,248", "1.248", "1 248".
pub fn count(n: u64) -> String {
    number(&crate::model::group_thousands(n))
}

/// `s` in the current language: `tl!("Layers")`.
#[macro_export]
macro_rules! tl {
    ($s:expr) => {
        $crate::i18n::t($s)
    };
}

/// Marks an English string as a catalog key without translating it here: for a label that is also
/// an id and is translated where it is drawn (`tl!(label)`).
#[macro_export]
macro_rules! tn {
    ($s:literal) => {
        $s
    };
}

#[cfg(test)]
mod tests;
