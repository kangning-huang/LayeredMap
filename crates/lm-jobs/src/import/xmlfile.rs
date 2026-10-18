//! What the XML formats (KML, KMZ, GPX) share: reading the file's text in its declared encoding,
//! parsing it with `roxmltree` (no DTDs, a node limit), and turning the text values of each record
//! into typed columns.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use rustgis_engine::data::{Field, FieldType, Value};

use super::limits::{XML_MAX_BYTES, XML_MAX_NODES};
use super::progress::Counting;
use super::{Ctx, ImportError, file_name};

/// Reads an XML file (at most [`XML_MAX_BYTES`]), counting progress and stopping on cancel.
pub(super) fn read_capped(path: &Path, format: &str, ctx: &Ctx) -> Result<Vec<u8>, ImportError> {
    let name = file_name(path);
    let size = std::fs::metadata(path).map_err(|e| ImportError::read(&name, e))?.len();
    if size > XML_MAX_BYTES {
        return Err(too_large(&name, format));
    }
    ctx.ticket.set_total(size);
    let file = std::fs::File::open(path).map_err(|e| ImportError::read(&name, e))?;
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    Counting::new(file, ctx.ticket.clone()).take(XML_MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| ImportError::read(&name, e))?;
    ctx.ticket.check()?;
    if bytes.len() as u64 > XML_MAX_BYTES {
        return Err(too_large(&name, format));
    }
    Ok(bytes)
}

pub(super) fn too_large(name: &str, format: &str) -> ImportError {
    ImportError::Refused(format!(
        "{name} is larger than 256 MB, the most LayeredMap reads as {format}. Converting it to GeoPackage makes it open quickly. Your file has not been changed."
    ))
}

/// The file's text: UTF-16 by its byte-order mark, the encoding its XML declaration names, or
/// UTF-8. Bytes that aren't valid in that encoding become U+FFFD, with a notice.
pub(super) fn decode(bytes: &[u8]) -> (String, Option<String>) {
    let lossy_notice = || Some("Some text in the file wasn't valid in its encoding; those characters show as �.".to_string());
    if let Some((enc, _)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, _, bad) = enc.decode(bytes);
        return (text.into_owned(), if bad { lossy_notice() } else { None });
    }
    let declared = declared_encoding(bytes).and_then(|l| encoding_rs::Encoding::for_label(l.as_bytes()));
    match declared {
        Some(enc) if enc != encoding_rs::UTF_8 => {
            let (text, _, bad) = enc.decode(bytes);
            (text.into_owned(), if bad { lossy_notice() } else { None })
        }
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => (s.to_string(), None),
            Err(_) => (String::from_utf8_lossy(bytes).into_owned(), lossy_notice()),
        },
    }
}

/// The `encoding="…"` of the XML declaration, if the file starts with one.
fn declared_encoding(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(bytes.get(..bytes.len().min(200))?);
    let decl = head.trim_start_matches('\u{feff}').trim_start();
    let decl = decl.strip_prefix("<?xml")?;
    let decl = &decl[..decl.find("?>")?];
    let at = decl.find("encoding")?;
    let rest = decl[at + "encoding".len()..].trim_start().strip_prefix('=')?.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let rest = &rest[1..];
    Some(rest[..rest.find(quote)?].trim().to_string())
}

/// Parses the text as XML: no DTDs (no entity tricks), at most [`XML_MAX_NODES`] nodes.
pub(super) fn parse<'a>(text: &'a str, name: &str, format: &str) -> Result<roxmltree::Document<'a>, ImportError> {
    let opts = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: XML_MAX_NODES, ..roxmltree::ParsingOptions::default() };
    roxmltree::Document::parse_with_options(text, opts).map_err(|e| {
        let message = match &e {
            roxmltree::Error::DtdDetected => {
                format!("{name} declares its own entities (a DTD), which LayeredMap doesn't read for safety. Your file has not been changed.")
            }
            roxmltree::Error::NodesLimitReached => format!(
                "{name} has more elements than LayeredMap reads from one {format} file. Converting it to GeoPackage makes it open. Your file has not been changed."
            ),
            _ => format!(
                "{name} isn't valid {format} (a problem on line {}). Your file has not been changed.",
                lm_core::model::group_thousands(u64::from(e.pos().row))
            ),
        };
        ImportError::Failed { message, details: e.to_string() }
    })
}

/// An element's local name (namespaces are ignored: KML 2.0–2.2, `gx:`, GPX 1.0 and 1.1).
pub(super) fn is(n: &roxmltree::Node, local: &str) -> bool {
    n.is_element() && n.tag_name().name() == local
}

/// The first child element called `local`.
pub(super) fn child<'a, 'i>(n: &roxmltree::Node<'a, 'i>, local: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| is(c, local))
}

/// An element's text (its text and CDATA children joined), trimmed; `None` when empty.
pub(super) fn text_of(n: &roxmltree::Node) -> Option<String> {
    let s: String = n.children().filter(|c| c.is_text()).filter_map(|c| c.text()).collect();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// The text of the first child element called `local`.
pub(super) fn child_text(n: &roxmltree::Node, local: &str) -> Option<String> {
    child(n, local).as_ref().and_then(text_of)
}

/// Columns gathered from text values, record by record, typed when every value agrees.
#[derive(Default)]
pub(super) struct Columns {
    names: Vec<String>,
    /// Lower-cased name → column (field lookups ignore case).
    index: HashMap<String, usize>,
    rows: Vec<Vec<(usize, String)>>,
}

impl Columns {
    /// Starts a record; values for it follow with [`Columns::set`].
    pub(super) fn start(&mut self) {
        self.rows.push(Vec::new());
    }

    /// Sets `name` on the current record. A name the record already has (in any case) is kept
    /// under `name_2`, so nothing is overwritten.
    pub(super) fn set(&mut self, name: &str, value: String) {
        let mut key = name.trim().to_string();
        if key.is_empty() {
            key = "field".into();
        }
        loop {
            let i = self.column(&key);
            let row = self.rows.last_mut();
            match row {
                Some(r) if r.iter().any(|(c, _)| *c == i) => key = format!("{key}_2"),
                Some(r) => {
                    r.push((i, value));
                    return;
                }
                None => return,
            }
        }
    }

    fn column(&mut self, name: &str) -> usize {
        let lower = name.to_lowercase();
        if let Some(i) = self.index.get(&lower) {
            return *i;
        }
        self.names.push(name.to_string());
        self.index.insert(lower, self.names.len() - 1);
        self.names.len() - 1
    }

    /// The fields and each record's values: a column whose every value is a whole number is
    /// Integer (unless one has a leading zero, as codes do), a number Double, otherwise Text.
    pub(super) fn finish(self) -> (Vec<Field>, Vec<Vec<Value>>) {
        let n = self.names.len();
        let mut types = vec![Some(FieldType::Integer); n];
        for row in &self.rows {
            for (i, v) in row {
                let Some(t) = types.get_mut(*i) else { continue };
                *t = match (*t, number(v)) {
                    (Some(FieldType::Integer), Some(Value::Int(_))) => Some(FieldType::Integer),
                    (Some(FieldType::Integer | FieldType::Double), Some(_)) => Some(FieldType::Double),
                    _ => Some(FieldType::Text),
                };
            }
        }
        let types: Vec<FieldType> = types.into_iter().map(|t| t.unwrap_or(FieldType::Text)).collect();
        let fields = self.names.iter().zip(&types).map(|(name, t)| Field::new(name.clone(), *t)).collect();
        let rows = self
            .rows
            .into_iter()
            .map(|r| {
                let mut out = vec![Value::Null; n];
                for (i, v) in r {
                    let value = match (types.get(i), number(&v)) {
                        (Some(FieldType::Integer), Some(x)) => x,
                        (Some(FieldType::Double), Some(Value::Int(x))) => Value::Float(x as f64),
                        (Some(FieldType::Double), Some(x)) => x,
                        _ => Value::Text(v),
                    };
                    if let Some(slot) = out.get_mut(i) {
                        *slot = value;
                    }
                }
                out
            })
            .collect();
        (fields, rows)
    }
}

/// A value written as a plain number: `12`, `-3.5`, `1e3`; not `007`, `+5`, `.5`, `inf`.
fn number(s: &str) -> Option<Value> {
    let t = s.trim();
    let digits = t.strip_prefix('-').unwrap_or(t);
    if digits.is_empty() || !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') && !digits[1..].starts_with(['.', 'e', 'E']) {
        return None;
    }
    if let Ok(i) = t.parse::<i64>() {
        return Some(Value::Int(i));
    }
    t.parse::<f64>().ok().filter(|f| f.is_finite()).map(Value::Float)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_encoding_is_used() {
        let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><gpx><name>S\xe3o Paulo</name></gpx>";
        let (text, notice) = decode(latin1);
        assert!(text.contains("São Paulo"), "{text}");
        assert!(notice.is_none());
        let (text, notice) = decode(b"<?xml version='1.0'?><a>\xff</a>");
        assert!(text.contains('\u{fffd}'));
        assert!(notice.is_some());
    }

    #[test]
    fn columns_are_typed_and_never_overwritten() {
        let mut c = Columns::default();
        c.start();
        c.set("name", "A".into());
        c.set("Name", "B".into());
        c.set("pop", "12".into());
        c.set("code", "007".into());
        c.start();
        c.set("pop", "12.5".into());
        c.set("code", "8".into());
        let (fields, rows) = c.finish();
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["name", "Name_2", "pop", "code"]);
        assert_eq!(fields[2].ty, FieldType::Double);
        assert_eq!(fields[3].ty, FieldType::Text);
        assert_eq!(rows[0][1], Value::Text("B".into()));
        assert_eq!(rows[0][2], Value::Float(12.0));
        assert_eq!(rows[1][0], Value::Null);
    }
}
