//! Joining a table to places (WP3.11). Pure and deterministic: given a table without geometry
//! and a layer, find the column they share, decide whether the answer is clear enough to join
//! without a question, and produce the layer's data with the table's fields added.
//!
//! Keys are compared on a ladder of normalizations, loosest last, and each join records the
//! level it needed: exact text; case-folded with accents and runs of space removed; with
//! punctuation removed too; codes without leading zeros ("06037" = "6037"); and bundled
//! crosswalks (countries, US states, Chinese provinces). Near-misses are never matched on their
//! own; they are offered afterwards.

mod crosswalk;

use std::collections::{BTreeMap, HashMap, HashSet};

use rustgis_engine::data::{Dataset, Field, FieldType, Value};
use serde::{Deserialize, Serialize};

use crate::find::fold;

pub use crosswalk::Family;

/// How loosely keys were compared, tightest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    Exact,
    Folded,
    Stripped,
    Code,
    /// Through one family of standard names and codes (never several at once: "CA" is
    /// California and Canada).
    Crosswalk(Family),
}

impl Level {
    pub const ALL: [Level; 7] = [
        Level::Exact,
        Level::Folded,
        Level::Stripped,
        Level::Code,
        Level::Crosswalk(Family::Countries),
        Level::Crosswalk(Family::UsStates),
        Level::Crosswalk(Family::CnProvinces),
    ];

    /// How the card says it matched ("by name", then this).
    pub fn describe(self) -> &'static str {
        match self {
            Level::Exact => "exactly",
            Level::Folded => "ignoring case and accents",
            Level::Stripped => "ignoring case, accents and punctuation",
            Level::Code => "as codes, ignoring leading zeros",
            Level::Crosswalk(Family::Countries) => "through standard country names and codes",
            Level::Crosswalk(Family::UsStates) => "through US state names, abbreviations and codes",
            Level::Crosswalk(Family::CnProvinces) => "through Chinese province names",
        }
    }
}

/// A cell as key text: numbers as plain digits (no separators), whole floats without ".0".
pub fn key_text(ds: &Dataset, row: usize, field: usize) -> Option<String> {
    match ds.value(row, field) {
        Value::Null => None,
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 => Some(format!("{f:.0}")),
        Value::Float(f) if f.is_finite() => Some(f.to_string()),
        Value::Float(_) => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Text(t) => {
            let t = t.trim();
            (!t.is_empty()).then(|| t.to_string())
        }
    }
}

fn folded(s: &str) -> String {
    fold(s).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn stripped(s: &str) -> String {
    fold(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

fn code(s: &str) -> Option<String> {
    let t = s.trim();
    let t = t.strip_suffix(".0").unwrap_or(t);
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let z = t.trim_start_matches('0');
    Some(if z.is_empty() { "0".into() } else { z.into() })
}

/// The key `s` becomes at `level` (None: it has no key at that level).
pub fn normalize(s: &str, level: Level) -> Option<String> {
    let k = match level {
        Level::Exact => s.trim().to_string(),
        Level::Folded => folded(s),
        Level::Stripped => stripped(s),
        Level::Code => code(s)?,
        Level::Crosswalk(f) => crosswalk::canonical(f, s)?,
    };
    (!k.is_empty()).then_some(k)
}

/// Fields that could hold keys: text or whole numbers, filled in at least 90% of rows.
pub fn key_fields(ds: &Dataset) -> Vec<usize> {
    let n = ds.features.len().max(1);
    (0..ds.fields.len())
        .filter(|&f| {
            let mut filled = 0usize;
            let mut fractional = false;
            for r in 0..ds.features.len() {
                match ds.value(r, f) {
                    Value::Null => {}
                    Value::Float(x) if x.fract() != 0.0 => fractional = true,
                    Value::Text(t) if t.trim().is_empty() => {}
                    Value::Bool(_) => fractional = true,
                    _ => filled += 1,
                }
            }
            !fractional && filled * 10 >= n * 9
        })
        .collect()
}

/// One way of pairing a table column with a layer field, scored.
#[derive(Clone, Debug, PartialEq)]
pub struct Pairing {
    pub table_field: usize,
    pub layer_field: usize,
    pub level: Level,
    /// Distinct table keys found in the layer, of all distinct table keys.
    pub matched_keys: usize,
    pub table_keys: usize,
    /// Layer records that get data.
    pub layer_matched: usize,
    pub layer_rows: usize,
    /// Two runs of small whole numbers, and the table's column isn't named like a key: 1–10
    /// matches any numbering, so a weak pairing is offered but never joined on its own.
    pub weak: bool,
    /// Which layer row each table row lands on.
    assignment: Vec<Option<usize>>,
}

impl Pairing {
    /// The share of the table's keys that found a place.
    pub fn rate(&self) -> f64 {
        if self.table_keys == 0 { 0.0 } else { self.matched_keys as f64 / self.table_keys as f64 }
    }
}

/// The layer's keys at `level`: key → row, for keys that are unique in the layer.
fn layer_keys(layer: &Dataset, field: usize, level: Level) -> Option<HashMap<String, usize>> {
    let mut map: HashMap<String, usize> = HashMap::new();
    let mut dup: HashSet<String> = HashSet::new();
    let mut filled = 0usize;
    for r in 0..layer.features.len() {
        let Some(k) = key_text(layer, r, field).and_then(|t| normalize(&t, level)) else { continue };
        filled += 1;
        if map.insert(k.clone(), r).is_some() {
            dup.insert(k);
        }
    }
    for k in &dup {
        map.remove(k);
    }
    // A field whose values repeat (a category, a region) is not a key.
    (filled > 0 && map.len() * 10 >= filled * 9).then_some(map)
}

fn pair(table: &Dataset, layer: &Dataset, tf: usize, lf: usize, level: Level) -> Option<Pairing> {
    let keys = layer_keys(layer, lf, level)?;
    let mut table_keys: HashSet<String> = HashSet::new();
    let mut matched: HashSet<String> = HashSet::new();
    let mut hit_rows: HashSet<usize> = HashSet::new();
    let mut assignment = Vec::with_capacity(table.features.len());
    // Keys are counted as written: a key with no form at this level still counts against the
    // pairing (small numbers are not country codes just because a few of them are).
    for r in 0..table.features.len() {
        let raw = key_text(table, r, tf);
        let k = raw.as_ref().and_then(|t| normalize(t, level));
        let row = k.as_ref().and_then(|k| keys.get(k).copied());
        if let Some(raw) = raw {
            if let Some(lr) = row {
                matched.insert(raw.clone());
                hit_rows.insert(lr);
            }
            table_keys.insert(raw);
        }
        assignment.push(row);
    }
    Some(Pairing {
        table_field: tf,
        layer_field: lf,
        level,
        matched_keys: matched.len(),
        table_keys: table_keys.len(),
        layer_matched: hit_rows.len(),
        layer_rows: layer.features.len(),
        weak: dense_integers(table, tf) && dense_integers(layer, lf) && !key_like(&table.fields[tf].name),
        assignment,
    })
}

/// A field named like a key: id, code, key, FIPS, GEOID, ISO…
fn key_like(name: &str) -> bool {
    let n = name.to_lowercase();
    let words: Vec<&str> = n.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    ["id", "code", "key", "fips", "geoid", "iso", "iso3", "iso2", "no", "num", "number", "adcode", "gid", "uid", "ref"]
        .iter()
        .any(|k| words.contains(k))
        || n.ends_with("id")
        || n.ends_with("code")
        || n.contains("fips")
        || n.contains("geoid")
        || n.starts_with("iso")
}

/// Whole numbers that run densely from near zero (0–9, 1–17): a numbering, not an identity.
fn dense_integers(ds: &Dataset, f: usize) -> bool {
    let mut v: Vec<i64> = Vec::new();
    for r in 0..ds.features.len() {
        match ds.value(r, f) {
            Value::Int(i) => v.push(*i),
            Value::Float(x) if x.fract() == 0.0 => v.push(*x as i64),
            Value::Null => {}
            _ => return false,
        }
    }
    if v.is_empty() {
        return false;
    }
    v.sort_unstable();
    v.dedup();
    let (lo, hi) = (v[0], v[v.len() - 1]);
    (0..=10).contains(&lo) && (hi - lo + 1) as usize <= v.len() * 3 / 2 + 2
}

/// Every pairing of a table key column with a layer key field at the level that matches most
/// (the tightest such level), best first.
pub fn score(table: &Dataset, layer: &Dataset) -> Vec<Pairing> {
    let mut out = Vec::new();
    for tf in key_fields(table) {
        for lf in key_fields(layer) {
            let mut best: Option<Pairing> = None;
            for level in Level::ALL {
                let Some(p) = pair(table, layer, tf, lf, level) else { continue };
                if best.as_ref().is_none_or(|b| p.matched_keys > b.matched_keys) {
                    let full = p.matched_keys == p.table_keys;
                    best = Some(p);
                    if full {
                        break;
                    }
                }
            }
            if let Some(b) = best.filter(|b| b.matched_keys > 0) {
                out.push(b);
            }
        }
    }
    let similar = |p: &Pairing| name_overlap(&table.fields[p.table_field].name, &layer.fields[p.layer_field].name);
    out.sort_by(|a, b| {
        b.rate()
            .total_cmp(&a.rate())
            .then(b.layer_matched.cmp(&a.layer_matched))
            .then(similar(b).cmp(&similar(a)))
            .then(a.level.cmp(&b.level))
            .then(a.table_field.cmp(&b.table_field))
            .then(a.layer_field.cmp(&b.layer_field))
    });
    out
}

/// Words two field names share ("Country Name" and "name" share one): among equal pairings, the
/// one whose names agree reads best on the card.
fn name_overlap(a: &str, b: &str) -> usize {
    let words = |s: &str| {
        s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 1).map(str::to_string).collect::<HashSet<String>>()
    };
    words(a).intersection(&words(b)).count()
}

/// Whether two pairings put the table's rows in the same places (90% of the rows either
/// places agree): two name fields that mean the same are not rival answers.
fn agree(a: &Pairing, b: &Pairing) -> bool {
    let (mut either, mut same) = (0usize, 0usize);
    for (x, y) in a.assignment.iter().zip(&b.assignment) {
        if x.is_some() || y.is_some() {
            either += 1;
            if x == y {
                same += 1;
            }
        }
    }
    either == 0 || same * 10 >= either * 9
}

/// What to do with a table and a layer.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// The answer is clear: join without a question.
    Join(Pairing),
    /// Ask once, offering up to three pairings that place the rows differently.
    Ask(Vec<Pairing>),
    /// Nothing matches well enough to offer.
    Nothing,
}

/// The rule: join when the best pairing matches at least 80% of the table's keys, is not weak,
/// and every pairing that would place rows differently is at least 20 points behind; ask when
/// the best matches at least 30%; otherwise there is nothing to offer.
pub fn decide(pairings: &[Pairing]) -> Decision {
    let Some(best) = pairings.first() else { return Decision::Nothing };
    let rivals: Vec<&Pairing> = pairings.iter().skip(1).filter(|p| !agree(best, p)).collect();
    let runner_up = rivals.first().map_or(0.0, |p| p.rate());
    if !best.weak && best.rate() >= 0.8 && best.rate() - runner_up >= 0.2 {
        return Decision::Join(best.clone());
    }
    if best.rate() < 0.3 {
        return Decision::Nothing;
    }
    let mut offer: Vec<Pairing> = vec![best.clone()];
    for p in pairings.iter().skip(1) {
        if offer.len() == 3 {
            break;
        }
        if p.rate() >= 0.3 && !offer.iter().any(|o| agree(o, p)) {
            offer.push(p.clone());
        }
    }
    Decision::Ask(offer)
}

/// What to do with a table and every open layer.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// Join to each of these layers (by index), the best match first: each one's decision is a
    /// clear join on its own.
    Join(Vec<(usize, Pairing)>),
    /// No layer is clear: ask once about the best layer's pairings.
    Ask(usize, Vec<Pairing>),
    Nothing,
}

/// The rule over several layers (shelf order, top first): the table joins every layer whose
/// decision is a clear join, with no question; only when none is clear does it ask, about the
/// layer that matches best. Ties go to the layer higher on the shelf. The rule for each layer
/// is `decide`'s, unchanged: a layer whose key field repeats (cities with a country field) is
/// never a target, so a table of country codes joins countries and leaves cities alone.
pub fn plan(table: &Dataset, layers: &[&Dataset]) -> Plan {
    let mut joins: Vec<(usize, Pairing)> = Vec::new();
    let mut ask: Option<(usize, Vec<Pairing>)> = None;
    for (i, l) in layers.iter().enumerate() {
        match decide(&score(table, l)) {
            Decision::Join(p) => joins.push((i, p)),
            Decision::Ask(ps) => {
                if ask.as_ref().is_none_or(|(_, best)| ps[0].rate() > best[0].rate()) {
                    ask = Some((i, ps));
                }
            }
            Decision::Nothing => {}
        }
    }
    if !joins.is_empty() {
        // Stable: equal rates keep shelf order.
        joins.sort_by(|a, b| b.1.rate().total_cmp(&a.1.rate()));
        return Plan::Join(joins);
    }
    match ask {
        Some((i, ps)) => Plan::Ask(i, ps),
        None => Plan::Nothing,
    }
}

/// The layer's data without fields a join added (separating a joined table).
pub fn separate(layer: &Dataset, fields: &[String]) -> Dataset {
    let keep: Vec<usize> = (0..layer.fields.len()).filter(|&f| !fields.contains(&layer.fields[f].name)).collect();
    let mut data = layer.clone();
    data.fields = keep.iter().map(|&f| layer.fields[f].clone()).collect();
    for feat in &mut data.features {
        feat.attrs = keep.iter().map(|&f| feat.attrs.get(f).cloned().unwrap_or(Value::Null)).collect();
    }
    data
}

/// What to do when the table has several rows for one place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Rows {
    /// One row per place.
    One,
    /// The row with the latest value in this field (a year or date).
    Latest { field: String },
    /// Counts summed, other numbers averaged, text from the first row.
    Combine,
}

/// A join as the document records it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JoinSpec {
    /// The table's name as the card shows it ("population.csv").
    pub table_name: String,
    pub table_field: String,
    pub layer_field: String,
    pub level: Level,
    pub rows: Rows,
    /// Near-misses the user accepted: table key → layer key (as written).
    #[serde(default)]
    pub aliases: Vec<(String, String)>,
}

impl JoinSpec {
    /// The spec as the document keeps it (`JoinRecord::spec`).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Option<JoinSpec> {
        serde_json::from_str(s).ok()
    }
}

/// The outcome of a join.
#[derive(Clone, Debug)]
pub struct Joined {
    /// The layer's data with the table's fields after its own.
    pub data: Dataset,
    /// The names of the fields added.
    pub new_fields: Vec<String>,
    /// Layer records that got data, of all.
    pub layer_matched: usize,
    /// Table keys (as written) that found no place.
    pub unmatched: Vec<String>,
    /// Near-misses for unmatched keys: (table key, the layer key it looks like).
    pub near_misses: Vec<(String, String)>,
    /// What the chip says about several rows per place, if anything.
    pub rows_note: Option<String>,
    /// Records per place in the table, at most.
    pub most_rows: usize,
}

/// A field whose values are years or dates: by name, or whole numbers from 1800 to 2200.
fn time_field(table: &Dataset, skip: usize) -> Option<usize> {
    (0..table.fields.len()).filter(|&f| f != skip).find(|&f| {
        let name = table.fields[f].name.to_lowercase();
        let named = ["year", "date", "yr", "period", "time"].iter().any(|w| name == *w || name.contains(w));
        let years = (0..table.features.len()).all(|r| match table.value(r, f) {
            Value::Int(i) => (1800..=2200).contains(i),
            Value::Null => true,
            _ => false,
        });
        named || (years && table.features.len() > 1)
    })
}

/// Sortable time text: numbers by value, text as written (ISO dates sort as text).
fn time_key(v: &Value) -> Option<(f64, String)> {
    match v {
        Value::Int(i) => Some((*i as f64, String::new())),
        Value::Float(f) if f.is_finite() => Some((*f, String::new())),
        Value::Text(t) if !t.trim().is_empty() => Some((t.trim().parse::<f64>().unwrap_or(f64::NEG_INFINITY), t.trim().to_string())),
        _ => None,
    }
}

fn is_count_name(name: &str) -> bool {
    let n = name.to_lowercase();
    ["count", "pop", "total", "number", "num_", "n_", "sum", "cases", "households", "people", "visits"].iter().any(|w| n.contains(w))
}

/// The table's rows each layer record takes, after the rule for several rows per place.
fn rows_per_record(table: &Dataset, assignment: &[Option<usize>], tf: usize) -> (Rows, BTreeMap<usize, Vec<usize>>) {
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (tr, lr) in assignment.iter().enumerate() {
        if let Some(lr) = lr {
            groups.entry(*lr).or_default().push(tr);
        }
    }
    if groups.values().all(|g| g.len() == 1) {
        return (Rows::One, groups);
    }
    if let Some(t) = time_field(table, tf) {
        return (Rows::Latest { field: table.fields[t].name.clone() }, groups);
    }
    (Rows::Combine, groups)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(ca != cb));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Joins `table` to `layer` as `spec` says.
pub fn apply(table: &Dataset, layer: &Dataset, spec: &JoinSpec) -> Option<Joined> {
    let tf = table.fields.iter().position(|f| f.name == spec.table_field)?;
    let lf = layer.fields.iter().position(|f| f.name == spec.layer_field)?;
    let mut keys = layer_keys(layer, lf, spec.level).unwrap_or_default();
    // Accepted near-misses point a table key at a layer key.
    let alias: HashMap<String, String> = spec
        .aliases
        .iter()
        .filter_map(|(t, l)| Some((normalize(t, spec.level).or_else(|| Some(folded(t)))?, normalize(l, spec.level)?)))
        .collect();
    for (t, l) in &alias {
        if let Some(&r) = keys.get(l) {
            keys.insert(t.clone(), r);
        }
    }
    let mut assignment = Vec::with_capacity(table.features.len());
    let mut unmatched: Vec<String> = Vec::new();
    let mut seen_unmatched = HashSet::new();
    for r in 0..table.features.len() {
        let raw = key_text(table, r, tf);
        let k = raw.as_ref().and_then(|t| normalize(t, spec.level).or_else(|| Some(folded(t))));
        let row = k.as_ref().and_then(|k| keys.get(k).copied());
        if row.is_none()
            && let Some(raw) = raw
            && seen_unmatched.insert(raw.clone())
        {
            unmatched.push(raw);
        }
        assignment.push(row);
    }
    let (rule, groups) = match &spec.rows {
        Rows::One => rows_per_record(table, &assignment, tf),
        _ => {
            let (_, g) = rows_per_record(table, &assignment, tf);
            (spec.rows.clone(), g)
        }
    };
    let most_rows = groups.values().map(Vec::len).max().unwrap_or(0);
    // New fields: every table field but the key, renamed if the layer has the name already.
    let added: Vec<usize> = (0..table.fields.len()).filter(|&f| f != tf).collect();
    let stem = spec.table_name.rsplit_once('.').map_or(spec.table_name.as_str(), |(s, _)| s).to_string();
    let mut data = layer.clone();
    let mut new_fields = Vec::new();
    for &f in &added {
        let src = &table.fields[f];
        let mut name = src.name.clone();
        if data.fields.iter().any(|x| x.name == name) {
            name = format!("{} ({stem})", src.name);
        }
        data.fields.push(Field { name: name.clone(), ty: src.ty, alias: src.alias.clone() });
        new_fields.push(name);
    }
    let latest = match &rule {
        Rows::Latest { field } => table.fields.iter().position(|f| &f.name == field),
        _ => None,
    };
    let mut latest_values: HashSet<String> = HashSet::new();
    for (lr, feat) in data.features.iter_mut().enumerate() {
        let group = groups.get(&lr);
        for &f in &added {
            let v = match group {
                None => Value::Null,
                Some(g) if g.len() == 1 => table.value(g[0], f).clone(),
                Some(g) => match (&rule, latest) {
                    (Rows::Latest { .. }, Some(t)) => {
                        let pick = g.iter().copied().max_by(|a, b| {
                            let ka = time_key(table.value(*a, t)).unwrap_or((f64::NEG_INFINITY, String::new()));
                            let kb = time_key(table.value(*b, t)).unwrap_or((f64::NEG_INFINITY, String::new()));
                            ka.0.total_cmp(&kb.0).then(ka.1.cmp(&kb.1))
                        });
                        pick.map_or(Value::Null, |p| table.value(p, f).clone())
                    }
                    _ => combine(table, g, f),
                },
            };
            feat.attrs.push(v);
        }
        if let (Some(g), Some(t)) = (group, latest)
            && let Some(p) = g.iter().copied().max_by(|a, b| {
                let ka = time_key(table.value(*a, t)).unwrap_or((f64::NEG_INFINITY, String::new()));
                let kb = time_key(table.value(*b, t)).unwrap_or((f64::NEG_INFINITY, String::new()));
                ka.0.total_cmp(&kb.0).then(ka.1.cmp(&kb.1))
            })
            && let Some(v) = key_text(table, p, t)
        {
            latest_values.insert(v);
        }
    }
    let rows_note = match &rule {
        Rows::One => None,
        Rows::Latest { field } => Some(if latest_values.len() == 1 {
            format!("Showing {}", latest_values.into_iter().next().unwrap_or_default())
        } else {
            format!("Showing the latest {field} for each place")
        }),
        Rows::Combine => Some(format!("Combined per place, up to {most_rows} rows each: counts summed, other numbers averaged")),
    };
    // Near-misses: unmatched keys within two edits of a layer key nothing else took.
    let taken: HashSet<usize> = groups.keys().copied().collect();
    let free: Vec<(String, String)> =
        (0..layer.features.len()).filter(|r| !taken.contains(r)).filter_map(|r| key_text(layer, r, lf).map(|t| (folded(&t), t))).collect();
    let mut near_misses = Vec::new();
    for u in &unmatched {
        let fu = folded(u);
        if fu.chars().count() < 4 {
            continue;
        }
        let best = free.iter().map(|(f, t)| (edit_distance(&fu, f), t)).filter(|(d, _)| *d <= 2).min_by_key(|(d, _)| *d);
        if let Some((_, t)) = best {
            near_misses.push((u.clone(), t.clone()));
        }
    }
    Some(Joined { data, new_fields, layer_matched: groups.len(), unmatched, near_misses, rows_note, most_rows })
}

/// Several table rows for one place, as one value: counts summed, other numbers averaged, text
/// from the first row.
fn combine(table: &Dataset, rows: &[usize], f: usize) -> Value {
    let nums: Vec<f64> = rows.iter().filter_map(|r| table.value(*r, f).as_f64()).collect();
    let numeric = matches!(table.fields[f].ty, FieldType::Integer | FieldType::Double);
    if numeric && !nums.is_empty() {
        let sum: f64 = nums.iter().sum();
        if is_count_name(&table.fields[f].name) {
            return if table.fields[f].ty == FieldType::Integer { Value::Int(sum as i64) } else { Value::Float(sum) };
        }
        return Value::Float(sum / nums.len() as f64);
    }
    rows.iter().map(|r| table.value(*r, f).clone()).find(|v| !matches!(v, Value::Null)).unwrap_or(Value::Null)
}

/// The spec for a decided pairing.
pub fn spec_for(table: &Dataset, layer: &Dataset, p: &Pairing, table_name: &str) -> JoinSpec {
    JoinSpec {
        table_name: table_name.to_string(),
        table_field: table.fields[p.table_field].name.clone(),
        layer_field: layer.fields[p.layer_field].name.clone(),
        level: p.level,
        rows: Rows::One,
        aliases: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
