//! Interface text and its catalogs (WP5.5; `lm_core::i18n`).
//!
//! - Every string the code marks for translation (`tl!("…")`, `tn!("…")`, `i18n::fmt("…", …)`)
//!   is a key; a catalog holds only keys the code still uses, and (once a language is complete)
//!   every one of them.
//! - English text handed straight to a widget, without going through translation, is flagged.
//!   A line that must stay as it is (a format name, a unit, a developer-only label) says so with
//!   an `// i18n: …` comment giving the reason.
//!
//!     cargo test -p lm-ui --test catalogs

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use lm_core::i18n::{self, Lang};

/// The source folders whose interface text goes through the catalogs.
const SOURCES: &[&str] = &[
    "crates/lm-ui/src",
    "apps/layeredmap/src",
    "crates/lm-actions/src",
    "crates/lm-core/src",
    "crates/lm-account/src",
    "crates/lm-ask/src",
];

/// Calls that mark a translatable literal: the literal is their first argument.
const MARKERS: &[&str] = &["tl!(", "tn!(", "i18n::fmt("];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.starts_with("._") {
            continue;
        }
        if p.is_dir() {
            rust_files(&p, out);
        } else if name.ends_with(".rs") && name != "tests.rs" {
            out.push(p);
        }
    }
}

/// The file's code before its `#[cfg(test)]` module (tests may say anything).
fn shipped(text: &str) -> &str {
    text.find("#[cfg(test)]\nmod ").map_or(text, |i| &text[..i])
}

/// A Rust string literal starting at `s` (just after the opening quote): its value and length.
fn literal(s: &str) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut it = s.char_indices();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Some((out, i + 1)),
            '\\' => match it.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                '\'' => out.push('\''),
                '0' => out.push('\0'),
                // A line continuation: the break and the next line's indent vanish.
                '\n' => {
                    let rest = &s[i + 2..];
                    let skip = rest.len() - rest.trim_start().len();
                    for _ in 0..skip {
                        it.next();
                    }
                }
                'u' => {
                    let rest = &s[i + 2..];
                    let close = rest.find('}')?;
                    let ch = u32::from_str_radix(&rest[1..close], 16).ok().and_then(char::from_u32)?;
                    out.push(ch);
                    for _ in 0..=close {
                        it.next();
                    }
                }
                o => {
                    out.push('\\');
                    out.push(o);
                }
            },
            c => out.push(c),
        }
    }
    None
}

/// Every key the code marks, with the first place it appears.
fn keys() -> BTreeMap<String, String> {
    let mut files = Vec::new();
    for dir in SOURCES {
        rust_files(&root().join(dir), &mut files);
    }
    let mut keys = BTreeMap::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        let code = shipped(&text);
        for m in MARKERS {
            let mut at = 0;
            while let Some(i) = code[at..].find(m) {
                let start = at + i + m.len();
                at = start;
                // Not in a comment (the docs quote `tl!("…")`).
                let line_start = code[..start].rfind('\n').map_or(0, |n| n + 1);
                if code[line_start..start].contains("//") {
                    continue;
                }
                let rest = code[start..].trim_start();
                let Some(body) = rest.strip_prefix('"') else { continue };
                if let Some((value, _)) = literal(body) {
                    let line = code[..start].matches('\n').count() + 1;
                    keys.entry(value).or_insert_with(|| format!("{}:{line}", f.display()));
                }
            }
        }
    }
    // Action labels: rules.toml's `label` templates, translated before their [tokens] are filled
    // (lm-actions `suggest`).
    let rules = root().join("crates/lm-actions/rules.toml");
    let text = std::fs::read_to_string(&rules).unwrap_or_default();
    for (n, line) in text.lines().enumerate() {
        if let Some(body) = line.trim_start().strip_prefix("label = \"")
            && let Some((value, _)) = literal(body)
        {
            keys.entry(value).or_insert_with(|| format!("{}:{}", rules.display(), n + 1));
        }
    }
    keys
}

/// Catalog keys escaped the way a catalog writes them.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t")
}

#[test]
fn catalogs_hold_only_keys_the_code_uses() {
    let keys = keys();
    assert!(keys.len() > 50, "found only {} keys: is the scanner broken?", keys.len());
    let mut report = Vec::new();
    for lang in Lang::all().skip(1) {
        let (catalog, errors) = i18n::parse(lang.source());
        assert!(errors.is_empty(), "{}: {errors:?}", lang.code());
        let stale: BTreeSet<&String> = catalog.keys().filter(|k| !keys.contains_key(*k)).collect();
        assert!(
            stale.is_empty(),
            "{}.tsv has keys the code no longer uses:\n{}",
            lang.code(),
            stale.iter().map(|k| escape(k)).collect::<Vec<_>>().join("\n")
        );
        let missing = keys.keys().filter(|k| !catalog.contains_key(*k)).count();
        report.push(format!("{} {}/{}", lang.code(), keys.len() - missing, keys.len()));
    }
    println!("translated: {}", report.join(", "));
}

/// Every language the menu lists is complete: a new string needs a line in every catalog.
#[test]
fn every_catalog_translates_every_key() {
    let keys = keys();
    for lang in Lang::all().skip(1) {
        let (catalog, _) = i18n::parse(lang.source());
        let missing: Vec<String> =
            keys.iter().filter(|(k, _)| !catalog.contains_key(*k)).map(|(k, at)| format!("{}\t({at})", escape(k))).collect();
        assert!(missing.is_empty(), "{}.tsv lacks {} strings:\n{}", lang.code(), missing.len(), missing.join("\n"));
    }
}

/// Widget calls whose first argument is the text shown.
const WIDGETS: &[&str] = &[
    ".label(\"",
    ".button(\"",
    ".heading(\"",
    ".small_button(\"",
    "RichText::new(\"",
    "Button::new(\"",
    "Label::new(\"",
    "on_hover_text(\"",
    "on_disabled_hover_text(\"",
    ".hint_text(\"",
    "a11y::button(&resp, \"",
    "MenuItem::with_id(",
    "CheckMenuItem::with_id(",
    "Submenu::new(\"",
    "Submenu::with_items(\"",
    ".set_title(\"",
    "format!(\"",
];

/// English shown without translation: `file:line: text`.
fn untranslated() -> Vec<String> {
    let mut files = Vec::new();
    for dir in &SOURCES[..2] {
        // lm-actions and lm-core only mark keys; the widgets are in the interface crates.
        rust_files(&root().join(dir), &mut files);
    }
    let mut hits = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        for (n, line) in shipped(&text).lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if line.contains("// i18n:") || code.trim_start().starts_with("log::") || code.contains("log::") {
                continue;
            }
            for w in WIDGETS {
                let Some(i) = code.find(w) else { continue };
                let mut rest = &code[i + w.len()..];
                // Menu items: the text is the second argument, after the id.
                if w.ends_with("with_id(") {
                    let Some(j) = rest.find(", ") else { continue };
                    rest = &rest[j + 2..];
                    let Some(r) = rest.strip_prefix('"') else { continue };
                    rest = r;
                }
                // format! builds text for many things; only flag it straight into a widget.
                if *w == "format!(\"" && !WIDGETS[..WIDGETS.len() - 1].iter().any(|v| code[..i].ends_with(v.trim_end_matches('"'))) {
                    continue;
                }
                let Some((value, _)) = literal(rest) else { continue };
                if value.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2 {
                    hits.push(format!("{}:{}: {}", f.strip_prefix(root()).unwrap_or(&f).display(), n + 1, value));
                }
            }
        }
    }
    hits
}

#[test]
fn english_shown_in_the_interface_goes_through_translation() {
    let hits = untranslated();
    assert!(
        hits.is_empty(),
        "{} strings are drawn without tl!/i18n::fmt (or mark the line `// i18n: reason`):\n{}",
        hits.len(),
        hits.join("\n")
    );
}

#[test]
fn the_scanner_reads_rust_literals() {
    assert_eq!(literal(r#"a\nb\"c" rest"#), Some(("a\nb\"c".to_string(), 8)));
    assert_eq!(literal("one \\\n        two\" x").map(|l| l.0), Some("one two".to_string()));
    assert_eq!(literal(r#"\u{2212}1""#).map(|l| l.0), Some("−1".to_string()));
    assert_eq!(literal("unterminated"), None);
}
