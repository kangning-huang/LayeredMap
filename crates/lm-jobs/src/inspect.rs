//! Inspect, select and style (Phase 3): everything the inspector, the table drawer and the Style
//! panel need from a layer's data. The interface never reads data itself (Architecture › Rules),
//! so it asks here: cell text, object titles and facts, filter masks, sorted table views, colour
//! and size classes for the Style panel with the legend they imply, and searching values.
//!
//! Numbers never come from the renderer: every count, range and class here is computed from the
//! data.

use std::collections::HashMap;

use lm_core::document::{Classes, DocLayer, OKABE_ITO, Rgba};
use lm_core::filter::{Cell, Filter};
use lm_core::model::{Extent, FieldRole, LayerKind};
use lm_core::palette;
use rustgis_engine::data::{Dataset, Value};

/// A field's position in the dataset by its exact name (falling back to any case).
pub fn field_index(ds: &Dataset, name: &str) -> Option<usize> {
    ds.fields.iter().position(|f| f.name == name).or_else(|| ds.field_index(name))
}

/// A cell as a filter sees it.
pub fn cell(ds: &Dataset, row: usize, field: usize) -> Cell<'_> {
    match ds.value(row, field) {
        Value::Null => Cell::Empty,
        Value::Int(i) => Cell::Number(*i as f64),
        Value::Float(f) if f.is_nan() => Cell::Empty,
        Value::Float(f) => Cell::Number(*f),
        Value::Bool(b) => Cell::Text(if *b { "Yes" } else { "No" }),
        Value::Text(t) if t.trim().is_empty() => Cell::Empty,
        Value::Text(t) => Cell::Text(t.as_str()),
    }
}

/// A cell's text for cards and the table; None for a blank (shown as "—", so blank and zero
/// stay distinct).
pub fn cell_text(ds: &Dataset, row: usize, field: usize) -> Option<String> {
    // A year reads as "2019", never "2,019".
    if ds.fields.get(field).is_some_and(|f| is_year_name(&f.name))
        && let Some(v) = cell_number(ds, row, field)
        && v.fract() == 0.0
        && (1000.0..=9999.0).contains(&v)
    {
        return Some(format!("{v:.0}"));
    }
    match ds.value(row, field) {
        Value::Null => None,
        Value::Int(i) => {
            let s = lm_core::model::group_thousands(i.unsigned_abs());
            Some(if *i < 0 { format!("\u{2212}{s}") } else { s })
        }
        Value::Float(f) if !f.is_finite() => None,
        Value::Float(f) => Some(number_text(*f)),
        Value::Bool(b) => Some(if *b { "Yes".into() } else { "No".into() }),
        Value::Text(t) if t.trim().is_empty() => None,
        Value::Text(t) => Some(t.clone()),
    }
}

/// Whether a field name says it holds years: "year", "gdp_year", "YearBuilt", "yr", "fiscal_yr".
pub fn is_year_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("year") || n.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "yr" || w == "yyyy")
}

/// A number for cards: thousands separators, up to the precision that matters (at most 4
/// significant decimals for small values, 2 decimals otherwise).
pub fn number_text(v: f64) -> String {
    if v != 0.0 && v.abs() < 0.01 {
        let s = format!("{:.4}", v);
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        return if v < 0.0 { s.replacen('-', "\u{2212}", 1) } else { s };
    }
    lm_core::format::number(v)
}

/// A numeric value of a cell (numbers and numeric text).
pub fn cell_number(ds: &Dataset, row: usize, field: usize) -> Option<f64> {
    match ds.value(row, field) {
        Value::Bool(_) => None,
        v => v.as_f64(),
    }
}

/// Which records pass the layer's filter (all when there is none).
pub fn filter_mask(ds: &Dataset, filter: Option<&Filter>) -> Option<Vec<bool>> {
    let f = filter.filter(|f| !f.is_empty())?;
    let idx: Vec<(String, Option<usize>)> = f.conditions.iter().map(|c| (c.field.clone(), field_index(ds, &c.field))).collect();
    Some(
        (0..ds.features.len())
            .map(|r| {
                f.matches(|name| match idx.iter().find(|(n, _)| n == name).and_then(|(_, i)| *i) {
                    Some(i) => cell(ds, r, i),
                    None => Cell::Empty,
                })
            })
            .collect(),
    )
}

/// The field that names objects (WP3.2): the first field whose role is Name (fields called name,
/// 名称, title or NAME_EN), otherwise the most unique short text field (when at least half its
/// values are distinct; a category such as "Hospital" doesn't name an object).
/// How much a line layer piles up on itself: on a grid of about 200 × 200 cells over its
/// extent, the 90th percentile of how many different lines cross a cell that any line crosses.
/// Storm tracks reach dozens; rivers and roads stay near 1–2. `None` for no lines.
pub fn line_overlap(ds: &Dataset) -> Option<f64> {
    use rustgis_engine::data::Geometry;
    let mut e = Extent { min_x: f64::INFINITY, min_y: f64::INFINITY, max_x: f64::NEG_INFINITY, max_y: f64::NEG_INFINITY };
    for f in &ds.features {
        if let Some(Geometry::Line(parts)) = &f.geometry {
            for c in parts.iter().flatten().filter(|c| c.x.is_finite() && c.y.is_finite()) {
                e.min_x = e.min_x.min(c.x);
                e.min_y = e.min_y.min(c.y);
                e.max_x = e.max_x.max(c.x);
                e.max_y = e.max_y.max(c.y);
            }
        }
    }
    if !(e.min_x < e.max_x || e.min_y < e.max_y) {
        return None;
    }
    let size = (e.max_x - e.min_x).max(e.max_y - e.min_y) / 200.0;
    let (w, h) = (((e.max_x - e.min_x) / size).ceil() as usize + 1, ((e.max_y - e.min_y) / size).ceil() as usize + 1);
    let mut count = vec![0u32; w * h];
    let mut last = vec![u32::MAX; w * h];
    let mut mark = |row: u32, x: f64, y: f64| {
        let (i, j) = (((x - e.min_x) / size) as usize, ((y - e.min_y) / size) as usize);
        let k = j.min(h - 1) * w + i.min(w - 1);
        if last[k] != row {
            last[k] = row;
            count[k] += 1;
        }
    };
    for (row, f) in ds.features.iter().enumerate() {
        let Some(Geometry::Line(parts)) = &f.geometry else { continue };
        for part in parts {
            for seg in part.windows(2) {
                let (a, b) = (seg[0], seg[1]);
                if !(a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite()) {
                    continue;
                }
                // Sample every half cell along the segment.
                let n = (((b.x - a.x).hypot(b.y - a.y) / (size * 0.5)).ceil() as usize).clamp(1, 10_000);
                for t in 0..=n {
                    let u = t as f64 / n as f64;
                    mark(row as u32, a.x + (b.x - a.x) * u, a.y + (b.y - a.y) * u);
                }
            }
        }
    }
    let mut hit: Vec<u32> = count.into_iter().filter(|c| *c > 0).collect();
    if hit.is_empty() {
        return None;
    }
    hit.sort_unstable();
    Some(f64::from(hit[(hit.len() - 1) * 9 / 10]))
}

/// The default look of a line layer that piles up (storm tracks): fainter and thinner, so
/// overlaps build up into a picture of where lines are dense. `None`: keep the default.
pub fn dense_lines_style(overlap: f64) -> Option<(u8, f32)> {
    // Rivers and coastlines score 3–5 on a world grid (neighbouring features sharing a cell);
    // storm tracks score about 40.
    if overlap < 8.0 {
        return None;
    }
    // About three overlapping lines make full strength; never fainter than 10%.
    let alpha = (3.0 / overlap).clamp(0.1, 0.5);
    Some(((alpha * 255.0).round() as u8, 1.0))
}

pub fn title_field(layer: &DocLayer, ds: &Dataset) -> Option<usize> {
    let roles: Vec<(usize, FieldRole)> =
        layer.profile.fields.iter().filter_map(|f| field_index(ds, &f.name).map(|i| (i, layer.role(f).value))).collect();
    if let Some((i, _)) = roles.iter().find(|(_, r)| *r == FieldRole::Name) {
        return Some(*i);
    }
    let title_names = ["name", "名称", "title", "name_en"];
    if let Some(i) = ds.fields.iter().position(|f| title_names.iter().any(|n| f.name.eq_ignore_ascii_case(n))) {
        return Some(i);
    }
    // Most unique short text: distinct values among the first 5,000 rows, average length ≤ 40.
    let n = ds.features.len().min(5_000);
    let mut best: Option<(usize, usize)> = None;
    for (i, role) in &roles {
        if !matches!(role, FieldRole::Text | FieldRole::Category) {
            continue;
        }
        let mut seen = std::collections::HashSet::new();
        let mut len = 0usize;
        let mut count = 0usize;
        for r in 0..n {
            if let Value::Text(t) = ds.value(r, *i)
                && !t.trim().is_empty()
            {
                len += t.chars().count();
                count += 1;
                seen.insert(t.as_str());
            }
        }
        // A name tells objects apart: most values distinct, and short.
        if count == 0 || len / count > 40 || seen.len() * 2 < count {
            continue;
        }
        if best.is_none_or(|(_, d)| seen.len() > d) {
            best = Some((*i, seen.len()));
        }
    }
    best.map(|(i, _)| i)
}

/// An object's title: its name, or the layer's name and the record number ("Districts 7").
pub fn object_title(layer: &DocLayer, ds: &Dataset, row: usize) -> String {
    title_field(layer, ds)
        .and_then(|i| cell_text(ds, row, i))
        .unwrap_or_else(|| format!("{} {}", layer.display_name, lm_core::model::group_thousands(row as u64 + 1)))
}

/// One fact on an object card.
#[derive(Clone, Debug, PartialEq)]
pub struct Fact {
    pub field: String,
    /// None for a blank ("No value").
    pub value: Option<String>,
    pub unit: Option<String>,
}

/// Longest text shown as a fact; longer text is skipped (until "Show all fields").
pub const LONG_TEXT: usize = 80;

/// The facts list (Inspector: object card › Facts): up to `max` fields in source order,
/// skipping identifiers, coordinate columns, the title field and long text; `all` lists every
/// field. Returns the facts and the number of fields there are.
pub fn facts(layer: &DocLayer, ds: &Dataset, row: usize, max: usize, all: bool) -> (Vec<Fact>, usize) {
    let title = title_field(layer, ds);
    let mut out = Vec::new();
    for (i, f) in ds.fields.iter().enumerate() {
        let profile = layer.field(&f.name);
        let role = profile.map(|p| layer.role(p).value).unwrap_or(FieldRole::Unknown);
        let value = cell_text(ds, row, i);
        if !all {
            if out.len() >= max {
                break;
            }
            if Some(i) == title || matches!(role, FieldRole::Identifier | FieldRole::Coordinate) {
                continue;
            }
            if value.as_ref().is_some_and(|v| v.chars().count() > LONG_TEXT) {
                continue;
            }
        }
        let unit = profile.and_then(|p| p.unit.value.clone());
        out.push(Fact { field: f.name.clone(), value, unit });
    }
    (out, ds.fields.len())
}

/// A table view's row order (Table drawer: clicking a header sorts the view, never the data).
/// Rows outside the filter are left out. Blanks sort last either way.
pub fn table_rows(ds: &Dataset, mask: Option<&[bool]>, sort: Option<(usize, bool)>) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..ds.features.len()).filter(|r| mask.is_none_or(|m| m.get(*r).copied().unwrap_or(true))).collect();
    if let Some((field, ascending)) = sort {
        let numeric = ds.fields.get(field).is_some_and(|f| f.ty.is_numeric());
        if numeric {
            rows.sort_by(|a, b| {
                let (x, y) = (cell_number(ds, *a, field), cell_number(ds, *b, field));
                match (x, y) {
                    (Some(x), Some(y)) => {
                        let o = x.total_cmp(&y);
                        if ascending { o } else { o.reverse() }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        } else {
            rows.sort_by(|a, b| {
                let (x, y) = (cell_text(ds, *a, field), cell_text(ds, *b, field));
                match (x, y) {
                    (Some(x), Some(y)) => {
                        let o = x.to_lowercase().cmp(&y.to_lowercase());
                        if ascending { o } else { o.reverse() }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
    }
    rows
}

/// Columns in table order: identifier columns pinned left, then the rest in source order.
pub fn table_columns(layer: &DocLayer, ds: &Dataset) -> Vec<usize> {
    let is_id =
        |i: usize| ds.fields.get(i).and_then(|f| layer.field(&f.name)).is_some_and(|p| layer.role(p).value == FieldRole::Identifier);
    let mut cols: Vec<usize> = (0..ds.fields.len()).filter(|i| is_id(*i)).collect();
    cols.extend((0..ds.fields.len()).filter(|i| !is_id(*i)));
    cols
}

/// A search hit in the user's data (WP3.9).
#[derive(Clone, Debug, PartialEq)]
pub struct DataHit {
    pub row: usize,
    /// The matching value.
    pub text: String,
    pub field: String,
}

/// Values in a layer's first five text fields that contain `query` (case-insensitive), at most
/// `limit`, whole-value matches first.
pub fn search_values(layer: &DocLayer, ds: &Dataset, query: &str, limit: usize) -> Vec<DataHit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let text_fields: Vec<usize> = ds
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            layer.field(&f.name).is_some_and(|p| matches!(layer.role(p).value, FieldRole::Name | FieldRole::Text | FieldRole::Category))
        })
        .map(|(i, _)| i)
        .take(5)
        .collect();
    let mut exact = Vec::new();
    let mut partial = Vec::new();
    'rows: for r in 0..ds.features.len() {
        for &i in &text_fields {
            if let Value::Text(t) = ds.value(r, i) {
                let l = t.to_lowercase();
                let hit = DataHit { row: r, text: t.clone(), field: ds.fields.get(i).map(|f| f.name.clone()).unwrap_or_default() };
                if l.trim() == q {
                    exact.push(hit);
                    if exact.len() >= limit {
                        break 'rows;
                    }
                    continue 'rows;
                } else if l.contains(&q) && partial.len() < limit {
                    partial.push(hit);
                    continue 'rows;
                }
            }
        }
    }
    exact.extend(partial);
    exact.truncate(limit);
    exact
}

// ---------------------------------------------------------------------------------------------
// Style classes and legends (WP3.5)

/// One legend entry.
#[derive(Clone, Debug, PartialEq)]
pub enum LegendItem {
    /// A swatch and its label ("Residential", "12.0 – 15.3").
    Swatch { color: Rgba, label: String },
    /// A continuous ramp from low to high.
    Ramp { palette: String, low: String, high: String },
    /// A point size and the value it stands for.
    Size { diameter: f32, label: String },
    /// "No data", drawn hatched.
    NoData { count: u64 },
}

/// The legend generated from a style (never typed).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Legend {
    /// "July mean temperature (°C)".
    pub title: Option<String>,
    pub items: Vec<LegendItem>,
    /// The range comes from the objects in view ("Fit range to view" is on).
    pub fitted_to_view: bool,
}

/// What colour and size each row takes: an index per row into `colors` × `sizes` combinations.
/// `index[row]` is a combination number; `combos[n]` is (colour, diameter or 0 for the style's).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowClasses {
    pub index: Vec<u16>,
    pub combos: Vec<(Rgba, f32)>,
    pub legend: Legend,
    /// The value→colour rule, so a value can be checked against the legend.
    pub scale: Option<ColorScale>,
}

/// How values map to colours.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorScale {
    /// Continuous between `lo` and `hi`.
    Smooth { palette: String, lo: f64, hi: f64 },
    /// Classes with ascending upper bounds (the last is the maximum).
    Groups { breaks: Vec<f64>, colors: Vec<Rgba> },
    /// Category value → colour; anything else takes `other`.
    Categories { colors: Vec<(String, Rgba)>, other: Option<Rgba> },
}

/// Steps of the smooth palette (indices 1..=SMOOTH_STEPS; 0 is No data).
const SMOOTH_STEPS: usize = 255;
/// Grey for "Other" categories.
pub const OTHER: Rgba = [0x9A, 0x9E, 0xA6, 0xFF];

impl ColorScale {
    /// The colour a value takes; None for a value with no colour (No data).
    pub fn color_of(&self, v: Option<f64>, text: Option<&str>) -> Option<Rgba> {
        match self {
            ColorScale::Smooth { palette, lo, hi } => {
                let v = v?;
                let t = if hi > lo { (v - lo) / (hi - lo) } else { 0.5 };
                let step = (t.clamp(0.0, 1.0) * (SMOOTH_STEPS - 1) as f64).round() / (SMOOTH_STEPS - 1) as f64;
                Some(palette::sample(palette, step))
            }
            ColorScale::Groups { breaks, colors } => {
                let v = v?;
                let k = breaks.iter().position(|b| v <= *b).unwrap_or(breaks.len().saturating_sub(1));
                colors.get(k).copied()
            }
            ColorScale::Categories { colors, other } => {
                let t = text?;
                colors.iter().find(|(c, _)| c == t).map(|(_, col)| *col).or(*other)
            }
        }
    }
}

/// Upper bounds of `n` quantile classes over sorted values (duplicates removed, so fewer
/// classes when values repeat).
pub fn quantile_breaks(sorted: &[f64], n: usize) -> Vec<f64> {
    if sorted.is_empty() || n == 0 {
        return Vec::new();
    }
    let mut b: Vec<f64> = (1..=n)
        .map(|k| {
            let pos = ((k as f64 / n as f64) * sorted.len() as f64).ceil() as usize;
            sorted.get(pos.saturating_sub(1).min(sorted.len() - 1)).copied().unwrap_or(0.0)
        })
        .collect();
    b.dedup();
    b
}

/// Upper bounds of `n` equal steps between the minimum and maximum.
pub fn equal_breaks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if n == 0 || !lo.is_finite() || !hi.is_finite() {
        return Vec::new();
    }
    if hi <= lo {
        return vec![hi];
    }
    (1..=n).map(|k| if k == n { hi } else { lo + (hi - lo) * k as f64 / n as f64 }).collect()
}

/// The values a style colours by, per row: the field's numbers, or numbers per km² when the
/// count guard's density is on (`area_km2` gives an object's geodesic area).
pub fn style_values(ds: &Dataset, field: usize, per_km2: bool, area_km2: &dyn Fn(usize) -> Option<f64>) -> Vec<Option<f64>> {
    (0..ds.features.len())
        .map(|r| {
            let v = cell_number(ds, r, field)?;
            if per_km2 { area_km2(r).filter(|a| *a > 0.0).map(|a| v / a) } else { Some(v) }
        })
        .collect()
}

fn label_of(field: &str, unit: Option<&str>, per_km2: bool) -> String {
    match (unit, per_km2) {
        (_, true) => format!("{field} per km²"),
        (Some(u), false) if !u.is_empty() => format!("{field} ({u})"),
        _ => field.to_string(),
    }
}

/// Works out each row's colour and size for a layer's style, and the legend. None when the
/// style is a single colour (nothing per row).
///
/// - `mask`: rows the filter keeps; others don't count toward ranges or classes.
/// - `in_view`: rows in view, when "Fit range to view" is on.
/// - `area_km2`: geodesic area of a row, for per-km² densities.
pub fn row_classes(
    layer: &DocLayer,
    ds: &Dataset,
    mask: Option<&[bool]>,
    in_view: Option<&dyn Fn(usize) -> bool>,
    area_km2: &dyn Fn(usize) -> Option<f64>,
) -> Option<RowClasses> {
    let style = &layer.style;
    let n = ds.features.len();
    let counted = |r: usize| mask.is_none_or(|m| m.get(r).copied().unwrap_or(true));
    let mut legend = Legend::default();
    // Colours.
    let mut color_of_row: Vec<Option<Rgba>> = vec![None; n];
    let mut scale = None;
    let mut no_data = 0u64;
    if let Some(cb) = &style.color_by
        && let Some(fi) = field_index(ds, &cb.field)
    {
        let unit = layer.field(&cb.field).and_then(|p| p.unit.value.clone());
        legend.title = Some(label_of(&cb.field, unit.as_deref(), cb.per_km2));
        if cb.categories {
            let mut counts: HashMap<String, u64> = HashMap::new();
            for r in (0..n).filter(|r| counted(*r)) {
                if let Some(t) = cell_text(ds, r, fi) {
                    *counts.entry(t).or_default() += 1;
                }
            }
            let mut cats: Vec<(String, u64)> = counts.into_iter().collect();
            cats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let shown = if cats.len() > OKABE_ITO.len() { OKABE_ITO.len() - 1 } else { cats.len() };
            let colors: Vec<(String, Rgba)> =
                cats.iter().take(shown).enumerate().map(|(k, (c, _))| (c.clone(), OKABE_ITO.get(k).copied().unwrap_or(OTHER))).collect();
            let other = (cats.len() > shown).then_some(OTHER);
            for (c, col) in &colors {
                legend.items.push(LegendItem::Swatch { color: *col, label: c.clone() });
            }
            if let Some(o) = other {
                let k = cats.len() - shown;
                legend.items.push(LegendItem::Swatch { color: o, label: format!("Other ({k} values)") });
            }
            let s = ColorScale::Categories { colors, other };
            for (r, slot) in color_of_row.iter_mut().enumerate() {
                let t = cell_text(ds, r, fi);
                *slot = s.color_of(None, t.as_deref());
                if slot.is_none() && counted(r) {
                    no_data += 1;
                }
            }
            scale = Some(s);
        } else {
            let values = style_values(ds, fi, cb.per_km2, area_km2);
            let mut sorted: Vec<f64> =
                values.iter().enumerate().filter(|(r, _)| counted(*r) && in_view.is_none_or(|f| f(*r))).filter_map(|(_, v)| *v).collect();
            sorted.sort_by(f64::total_cmp);
            legend.fitted_to_view = cb.fit_to_view && in_view.is_some();
            if let (Some(lo), Some(hi)) = (sorted.first().copied(), sorted.last().copied()) {
                let s = match cb.classes {
                    Classes::Smooth => {
                        legend.items.push(LegendItem::Ramp { palette: cb.palette.clone(), low: number_text(lo), high: number_text(hi) });
                        ColorScale::Smooth { palette: cb.palette.clone(), lo, hi }
                    }
                    Classes::Groups { count, equal_steps } => {
                        let k = usize::from(count.clamp(2, 7));
                        let breaks = if equal_steps { equal_breaks(lo, hi, k) } else { quantile_breaks(&sorted, k) };
                        let colors = palette::classes(&cb.palette, breaks.len());
                        let mut prev = lo;
                        for (b, c) in breaks.iter().zip(&colors) {
                            legend
                                .items
                                .push(LegendItem::Swatch { color: *c, label: format!("{} – {}", number_text(prev), number_text(*b)) });
                            prev = *b;
                        }
                        ColorScale::Groups { breaks, colors }
                    }
                };
                for (r, slot) in color_of_row.iter_mut().enumerate() {
                    *slot = s.color_of(values.get(r).copied().flatten(), None);
                }
                scale = Some(s);
            }
            no_data = values.iter().enumerate().filter(|(r, v)| counted(*r) && v.is_none()).count() as u64;
        }
        if no_data > 0 {
            legend.items.push(LegendItem::NoData { count: no_data });
        }
    }
    // Sizes (points only): circle area proportional to the value.
    let mut size_of_row: Vec<f32> = vec![0.0; n];
    if layer.profile.kind == LayerKind::Points
        && let Some(sb) = &style.size_by
        && let Some(fi) = field_index(ds, &sb.field)
    {
        let vmax = (0..n).filter(|r| counted(*r)).filter_map(|r| cell_number(ds, r, fi)).filter(|v| *v > 0.0).fold(0.0f64, f64::max);
        if vmax > 0.0 {
            let max = sb.max_size.max(4.0);
            let diameter = |v: f64| -> f32 { ((v.max(0.0) / vmax).sqrt() as f32 * max).max(2.0) };
            // Quantized to half pixels so the symbol table stays small.
            for (r, slot) in size_of_row.iter_mut().enumerate() {
                if let Some(v) = cell_number(ds, r, fi) {
                    *slot = (diameter(v) * 2.0).round() / 2.0;
                }
            }
            if legend.title.is_none() {
                let unit = layer.field(&sb.field).and_then(|p| p.unit.value.clone());
                legend.title = Some(label_of(&sb.field, unit.as_deref(), false));
            }
            for v in [vmax, nice(vmax / 2.0), nice(vmax / 10.0)] {
                legend.items.push(LegendItem::Size { diameter: diameter(v), label: number_text(v) });
            }
        }
    }
    if style.color_by.is_none() && style.size_by.is_none() {
        return None;
    }
    // Combine into a table of (colour, size) and an index per row.
    let base = style.fill;
    let mut combos: Vec<(Rgba, f32)> = Vec::new();
    let mut lookup: HashMap<([u8; 4], u32), u16> = HashMap::new();
    let mut index = Vec::with_capacity(n);
    for r in 0..n {
        let color = match (&style.color_by, color_of_row.get(r).copied().flatten()) {
            (Some(_), Some(c)) => c,
            (Some(_), None) => palette::NO_DATA,
            (None, _) => base,
        };
        let size = size_of_row.get(r).copied().unwrap_or(0.0);
        let key = (color, size.to_bits());
        let i = match lookup.get(&key) {
            Some(i) => *i,
            None => {
                let i = u16::try_from(combos.len()).unwrap_or(u16::MAX);
                if combos.len() < usize::from(u16::MAX) {
                    combos.push((color, size));
                    lookup.insert(key, i);
                }
                i
            }
        };
        index.push(i);
    }
    Some(RowClasses { index, combos, legend, scale })
}

/// [`row_classes`] with the layer's own geodesic areas, and "Fit range to view" limited to the
/// rows touching `view`.
pub fn classes_for(layer: &DocLayer, ds: &Dataset, mask: Option<&[bool]>, view: Option<&Extent>) -> Option<RowClasses> {
    let in_view = view.map(|e| move |r: usize| row_in_extent(ds, r, e));
    let in_view_ref: Option<&dyn Fn(usize) -> bool> = in_view.as_ref().map(|f| f as &dyn Fn(usize) -> bool);
    row_classes(layer, ds, mask, in_view_ref, &|r| area_km2(ds, r))
}

/// An object's geodesic area in km² (areas only).
pub fn area_km2(ds: &Dataset, row: usize) -> Option<f64> {
    let g = ds.features.get(row)?.geometry.as_ref()?;
    lm_analysis::geodesic::geometry_area_m2(g).map(|a| a / 1e6)
}

/// The extent of one record, for Zoom to (a point gets a small box around it).
pub fn row_extent(ds: &Dataset, row: usize) -> Option<Extent> {
    let e = ds.features.get(row)?.geometry.as_ref()?.extent();
    if !e.min_x.is_finite() || !e.max_x.is_finite() {
        return None;
    }
    let pad = if e.max_x - e.min_x < 1e-6 && e.max_y - e.min_y < 1e-6 { 0.002 } else { 0.0 };
    Some(Extent { min_x: e.min_x - pad, min_y: e.min_y - pad, max_x: e.max_x + pad, max_y: e.max_y + pad })
}

/// An object's size for its card subtitle: geodesic area for areas, length for lines.
pub fn measure_text(ds: &Dataset, row: usize, imperial: bool) -> Option<String> {
    use lm_analysis::geodesic::{self, Measure, Units};
    let units = if imperial { Units::Imperial } else { Units::Metric };
    match geodesic::measure(ds.features.get(row)?.geometry.as_ref()?) {
        Measure::None => None,
        Measure::Length(m) => Some(geodesic::format_length(m, units)),
        Measure::Area(a) => Some(geodesic::format_area(a.area_m2, units)),
    }
}

// ---------------------------------------------------------------------------------------------
// Insight (Insight cards and suggested actions › Insight rows)

use lm_analysis::insight::{CategoryCounts, RankIndex};

/// Rank indexes for a layer's measure fields and counts for its category fields, built in the
/// background when the data, filter or field roles change, so a rank is a binary search.
#[derive(Debug, Default)]
pub struct LayerIndex {
    pub measures: HashMap<String, RankIndex>,
    pub categories: HashMap<String, CategoryCounts>,
    /// Records counted (the filter's matches, or all).
    pub counted: usize,
    pub filtered: bool,
}

/// Fields that qualify for insight: measures at Certain or Likely confidence, and categories.
fn insight_field(layer: &DocLayer, f: &lm_core::model::FieldProfile) -> Option<bool> {
    use lm_core::model::Confidence;
    let k = layer.role(f);
    let sure = matches!(k.confidence, Confidence::Certain | Confidence::Likely);
    match k.value {
        FieldRole::Measure(_) if sure => Some(true),
        FieldRole::Category => Some(false),
        _ => None,
    }
}

/// Builds the indexes over the records `mask` keeps.
pub fn build_index(layer: &DocLayer, ds: &Dataset, mask: Option<&[bool]>) -> LayerIndex {
    let keep = |r: usize| mask.is_none_or(|m| m.get(r).copied().unwrap_or(true));
    let filtered = mask.is_some();
    let mut out = LayerIndex { filtered, counted: (0..ds.features.len()).filter(|r| keep(*r)).count(), ..Default::default() };
    for f in &layer.profile.fields {
        let Some(measure) = insight_field(layer, f) else { continue };
        let Some(fi) = field_index(ds, &f.name) else { continue };
        let rows = (0..ds.features.len()).filter(|r| keep(*r));
        if measure {
            out.measures.insert(f.name.clone(), RankIndex::new(rows.map(|r| cell_number(ds, r, fi))).with_filtered(filtered));
        } else {
            out.categories.insert(f.name.clone(), CategoryCounts::new(rows.map(|r| cell_text(ds, r, fi))).with_filtered(filtered));
        }
    }
    out
}

/// A dot strip in 0–1 positions.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub value: Option<f32>,
    pub median: Option<f32>,
    pub ticks: Vec<f32>,
}

/// One insight row as the card shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct InsightRow {
    pub field: String,
    /// The value with its unit, the category, or "No value".
    pub value: String,
    /// "3rd highest of 17", "41% of districts are Residential".
    pub phrase: Option<String>,
    /// "+2.1 °C above the median".
    pub comparison: Option<String>,
    pub strip: Option<Strip>,
}

/// The noun for a layer's objects in phrases: its name when that reads as words ("schools"),
/// otherwise its kind ("points").
pub fn noun_plural(layer: &DocLayer) -> String {
    let n = layer.display_name.trim();
    if !n.is_empty() && n.chars().all(|c| c.is_alphabetic() || c == ' ') && n.chars().count() <= 24 {
        n.to_lowercase()
    } else {
        layer.profile.kind.nouns().1.to_string()
    }
}

/// The fields shown in insight, in order: the styling field, starred fields, then other
/// measures in source order. Identifiers never qualify.
pub fn insight_fields(layer: &DocLayer, index: &LayerIndex, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |name: &str, out: &mut Vec<String>| {
        if out.len() < max && !out.iter().any(|o| o == name) && (index.measures.contains_key(name) || index.categories.contains_key(name)) {
            out.push(name.to_string());
        }
    };
    if let Some(cb) = &layer.style.color_by {
        push(&cb.field, &mut out);
    }
    if let Some(sb) = &layer.style.size_by {
        push(&sb.field, &mut out);
    }
    for s in &layer.starred {
        push(s, &mut out);
    }
    for f in &layer.profile.fields {
        if index.measures.contains_key(&f.name) {
            push(&f.name, &mut out);
        }
    }
    out
}

/// Up to `max` insight rows for one object.
pub fn insight_rows(layer: &DocLayer, ds: &Dataset, index: &LayerIndex, row: usize, max: usize) -> Vec<InsightRow> {
    let noun = noun_plural(layer);
    insight_fields(layer, index, max)
        .into_iter()
        .filter_map(|name| {
            let fi = field_index(ds, &name)?;
            if let Some(ix) = index.measures.get(&name) {
                let unit = layer.field(&name).and_then(|p| p.unit.value.clone());
                let m = lm_analysis::insight::measure_row(ix, cell_number(ds, row, fi), unit.as_deref(), &noun);
                Some(InsightRow {
                    field: name,
                    value: m.value,
                    phrase: m.rank,
                    comparison: m.median,
                    strip: m.strip.map(|s| Strip { value: s.value, median: Some(s.median), ticks: s.ticks }),
                })
            } else {
                let counts = index.categories.get(&name)?;
                let value = cell_text(ds, row, fi);
                let line = counts.category_row(value.as_deref(), &noun);
                let (value, phrase) = match line.split_once(" · ") {
                    Some((v, p)) => (v.to_string(), Some(p.to_string())),
                    None => (line, None),
                };
                Some(InsightRow { field: name, value, phrase, comparison: None, strip: None })
            }
        })
        .collect()
}

/// A ranking of every object by one field, highest first (Compare with other [Districts]):
/// (row, title, value text, value) with blanks left out.
pub fn ranking(layer: &DocLayer, ds: &Dataset, field: &str, mask: Option<&[bool]>) -> Vec<(usize, String, String, f64)> {
    let Some(fi) = field_index(ds, field) else { return Vec::new() };
    let title = title_field(layer, ds);
    let unit = layer.field(field).and_then(|p| p.unit.value.clone());
    let mut v: Vec<(usize, f64)> = (0..ds.features.len())
        .filter(|r| mask.is_none_or(|m| m.get(*r).copied().unwrap_or(true)))
        .filter_map(|r| cell_number(ds, r, fi).map(|x| (r, x)))
        .collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter()
        .map(|(r, x)| {
            let name = title
                .and_then(|i| cell_text(ds, r, i))
                .unwrap_or_else(|| format!("{} {}", layer.display_name, lm_core::model::group_thousands(r as u64 + 1)));
            (r, name, lm_analysis::insight::with_unit(&number_text(x), unit.as_deref()), x)
        })
        .collect()
}

/// A field's 20-bin histogram and range text for the layer overview's field summary.
pub fn field_summary(index: &LayerIndex, field: &str, unit: Option<&str>) -> Option<(String, Vec<u64>)> {
    let ix = index.measures.get(field)?;
    let (lo, hi) = ix.range()?;
    let h = ix.histogram(lm_analysis::insight::HISTOGRAM_BINS)?;
    let fd = Some(ix.decimals());
    let range = format!(
        "{} to {}",
        lm_analysis::insight::format_number(lo, fd),
        lm_analysis::insight::with_unit(&lm_analysis::insight::format_number(hi, fd), unit)
    );
    Some((range, h.counts.iter().map(|c| *c as u64).collect()))
}

/// A raster cell's insight: its value with the unit, and how it compares with the layer's cells
/// (estimated from the preview's sample of cells).
pub fn pixel_insight(r: &Raster, band: usize, value: f64, unit: Option<&str>) -> (String, Option<String>) {
    const SAMPLE: usize = 200_000;
    let sample = rustgis_engine::map::raster::sorted_sample(r, band, SAMPLE);
    // A preview read from an overview, or every n-th cell of a large band, is a sample.
    let estimated = r.overview.is_some() || r.bands.get(band).is_some_and(|b| b.data.len() > SAMPLE);
    let ix = RankIndex::new(sample.into_iter().map(Some));
    let v = lm_analysis::insight::format_value(Some(value), Some(ix.decimals()), unit);
    let phrase = ix.rank(value).and_then(|rk| lm_analysis::insight::raster_phrase(&rk, estimated));
    (v, phrase)
}

// ---------------------------------------------------------------------------------------------
// Raster display (WP3.5 › Raster)

use rustgis_engine::data::raster::Raster;
use rustgis_engine::map::raster::{Normalizer, Stretch};

/// The default raster stretch: the 2nd to 98th percentile of cells.
pub const DEFAULT_STRETCH: Stretch = Stretch::PercentClip { low: 2.0, high: 2.0 };

/// The 0-based band a style shows.
pub fn style_band(r: &Raster, style: &lm_core::document::Style) -> usize {
    (style.band.max(1) as usize - 1).min(r.bands.len().saturating_sub(1))
}

/// The value of a band at a longitude/latitude: `Some(None)` for a NoData cell inside the
/// raster, `None` outside it.
pub fn raster_value_at(r: &Raster, band: usize, lon: f64, lat: f64) -> Option<Option<f64>> {
    let (col, row) = r.pixel_at(rustgis_engine::geom::Coord::new(lon, lat))?;
    Some(r.value(band, col, row).map(f64::from))
}

/// A band's whole range (minimum, maximum).
pub fn raster_full_range(r: &Raster, band: usize) -> Option<(f64, f64)> {
    let s = &r.bands.get(band)?.stats;
    (s.min.is_finite() && s.max.is_finite() && s.min < s.max).then_some((s.min, s.max))
}

/// The value range the colours spread over: the style's own, or 2–98% of cells — computed the
/// same way the renderer does, so the legend matches the map.
pub fn raster_window(r: &Raster, style: &lm_core::document::Style) -> Option<(f64, f64)> {
    if let Some(w) = style.stretch {
        return Some(w);
    }
    let b = style_band(r, style);
    let s = &r.bands.get(b)?.stats;
    let n = Normalizer::new(DEFAULT_STRETCH, s, &r.data_type);
    Some((n.lo, n.hi))
}

/// How the renderer should draw a single-band raster for a style: the stretch it computes its
/// range with, and 256 ramp stops over that range that carry the palette inside the style's
/// window, hide values at or below "Show only values above…", and dim values outside "Show
/// where values are like this".
pub fn raster_ramp(r: &Raster, style: &lm_core::document::Style) -> Option<(Stretch, Vec<(f64, Rgba)>)> {
    let b = style_band(r, style);
    let s = &r.bands.get(b)?.stats;
    let (stretch, (rlo, rhi)) = match style.stretch {
        // The renderer's range is the whole band; the window is drawn inside it.
        Some(_) => (Stretch::MinMax, (s.min, s.max)),
        None => {
            let n = Normalizer::new(DEFAULT_STRETCH, s, &r.data_type);
            (DEFAULT_STRETCH, (n.lo, n.hi))
        }
    };
    let (wlo, whi) = raster_window(r, style)?;
    let stops = (0..256)
        .map(|i| {
            let x = i as f64 / 255.0;
            let v = rlo + x * (rhi - rlo);
            let t = if whi > wlo { (v - wlo) / (whi - wlo) } else { 0.5 };
            let mut c = palette::sample(&style.ramp, t);
            if style.above.is_some_and(|a| v <= a) {
                c[3] = 0;
            }
            if let Some((p, q)) = style.like
                && !(v >= p.min(q) && v <= p.max(q))
            {
                c = [0x9A, 0x9E, 0xA6, 0x50];
            }
            (x, c)
        })
        .collect();
    Some((stretch, stops))
}

/// A round number near `v` (two significant figures) for size legends.
fn nice(v: f64) -> f64 {
    if v <= 0.0 || !v.is_finite() {
        return 0.0;
    }
    let mag = 10f64.powf(v.log10().floor() - 1.0);
    (v / mag).round() * mag
}

/// The layer kind's style presets (WP3.5 › Presets) available for this layer's fields, as
/// (label, style). The first is always the single colour.
pub fn presets(layer: &DocLayer) -> Vec<(String, lm_core::document::Style)> {
    use lm_core::document::{ColorBy, LabelStyle, SizeBy};
    let base = lm_core::document::Style { color_by: None, size_by: None, labels: None, ..layer.style.clone() };
    let mut out = vec![("Single color".to_string(), base.clone())];
    let fields = &layer.profile.fields;
    let role = |f: &lm_core::model::FieldProfile| layer.role(f);
    let good = |c: lm_core::model::Confidence| matches!(c, lm_core::model::Confidence::Certain | lm_core::model::Confidence::Likely);
    let measure = fields
        .iter()
        .find(|f| layer.starred.contains(&f.name) && matches!(role(f).value, FieldRole::Measure(_)))
        .or_else(|| fields.iter().find(|f| matches!(role(f).value, FieldRole::Measure(_)) && good(role(f).confidence)));
    let category = fields.iter().find(|f| role(f).value == FieldRole::Category);
    let name = fields.iter().find(|f| role(f).value == FieldRole::Name);
    if let Some(c) = category {
        let s = lm_core::document::Style {
            color_by: Some(ColorBy {
                field: c.name.clone(),
                categories: true,
                classes: Classes::Smooth,
                palette: "viridis".into(),
                per_km2: false,
                fit_to_view: false,
            }),
            ..base.clone()
        };
        out.push((format!("By {}", c.name), s));
    }
    if let Some(m) = measure {
        let palette = if palette::is_temperature(m.unit.value.as_deref()) {
            "heat"
        } else if matches!(role(m).value, FieldRole::Measure(lm_core::model::MeasureKind::Count)) {
            "blues"
        } else {
            "viridis"
        };
        let by = ColorBy {
            field: m.name.clone(),
            categories: false,
            classes: Classes::Groups { count: 5, equal_steps: false },
            palette: palette.into(),
            per_km2: false,
            fit_to_view: false,
        };
        out.push((format!("By {}", m.name), lm_core::document::Style { color_by: Some(by.clone()), ..base.clone() }));
        if layer.profile.kind == LayerKind::Points {
            out.push((
                format!("Size by {}", m.name),
                lm_core::document::Style { size_by: Some(SizeBy { field: m.name.clone(), max_size: 24.0 }), ..base.clone() },
            ));
        }
        if palette::is_temperature(m.unit.value.as_deref()) {
            out.push((
                "Heat".to_string(),
                lm_core::document::Style {
                    color_by: Some(ColorBy { classes: Classes::Smooth, palette: "heat".into(), ..by }),
                    ..base.clone()
                },
            ));
        }
    }
    if let Some(nf) = name {
        out.push((
            "Labels on".to_string(),
            lm_core::document::Style { labels: Some(LabelStyle { field: nf.name.clone(), size: 12.0 }), ..layer.style.clone() },
        ));
    }
    out.truncate(6);
    out
}

/// Whether the count guard applies (WP3.5): areas coloured by a Count field, not yet per km².
pub fn count_guard(layer: &DocLayer) -> bool {
    layer.profile.kind == LayerKind::Areas
        && layer.style.color_by.as_ref().is_some_and(|cb| {
            !cb.categories
                && !cb.per_km2
                && layer
                    .field(&cb.field)
                    .is_some_and(|f| matches!(layer.role(f).value, FieldRole::Measure(lm_core::model::MeasureKind::Count)))
        })
}

/// Whether a row's geometry touches an extent (for "Fit range to view").
pub fn row_in_extent(ds: &Dataset, row: usize, e: &Extent) -> bool {
    ds.features.get(row).and_then(|f| f.geometry.as_ref()).is_some_and(|g| {
        let b = g.extent();
        !(b.max_x < e.min_x || b.min_x > e.max_x || b.max_y < e.min_y || b.min_y > e.max_y)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lm_core::document::{ColorBy, Style};
    use lm_core::filter::{Condition, Test};
    use lm_core::model::*;
    use rustgis_engine::data::geometry::Geometry;
    use rustgis_engine::data::{Field, FieldType, GeometryType};
    use rustgis_engine::geom::Coord;

    fn field(name: &str, role: FieldRole, unit: Option<&str>) -> FieldProfile {
        FieldProfile {
            name: name.into(),
            dtype: DataType::Float,
            role: Known::likely(role, ""),
            unit: Known::likely(unit.map(String::from), ""),
            stats: None,
        }
    }

    fn districts() -> (DocLayer, Dataset) {
        let mut ds = Dataset::new(
            "d",
            GeometryType::Point,
            vec![
                Field::new("id", FieldType::Integer),
                Field::new("name", FieldType::Text),
                Field::new("temp", FieldType::Double),
                Field::new("type", FieldType::Text),
                Field::new("pop", FieldType::Integer),
            ],
        );
        let rows: [(i64, &str, Option<f64>, &str, i64); 6] = [
            (1, "Alpha", Some(30.1), "Residential", 120_000),
            (2, "Beta", Some(31.5), "Industrial", 600_000),
            (3, "Gamma", None, "Residential", 50_000),
            (4, "Delta", Some(29.0), "Residential", 800_000),
            (5, "", Some(33.2), "Park", 1_000),
            (6, "Zeta", Some(31.5), "Industrial", 0),
        ];
        for (k, (id, name, t, ty, pop)) in rows.iter().enumerate() {
            ds.push(
                Some(Geometry::Point(Coord::new(k as f64, 0.0))),
                vec![
                    Value::Int(*id),
                    Value::Text((*name).into()),
                    t.map_or(Value::Null, Value::Float),
                    Value::Text((*ty).into()),
                    Value::Int(*pop),
                ],
            );
        }
        let profile = LayerProfile {
            source: SourceRef { path: "/d/districts.csv".into(), member: None, size: 1, mtime: 0, sha256: None },
            format: "CSV".into(),
            kind: LayerKind::Points,
            count: 6,
            extent_wgs84: None,
            crs: Known::likely(Crs::Wgs84, ""),
            fields: vec![
                field("id", FieldRole::Identifier, None),
                field("name", FieldRole::Name, None),
                field("temp", FieldRole::Measure(MeasureKind::Intensive), Some("°C")),
                field("type", FieldRole::Category, None),
                field("pop", FieldRole::Measure(MeasureKind::Count), None),
            ],
            bands: vec![],
            assumptions: vec![],
            exclusions: vec![],
            encoding: None,
            notices: vec![],
            data_rev: 0,
            joins: Vec::new(),
            grid: None,
        };
        let style = Style::for_profile(&profile, OKABE_ITO[0]);
        (DocLayer::new(LayerId(1), "Districts".into(), profile, style), ds)
    }

    #[test]
    fn titles_use_the_name_field_or_layer_and_number() {
        let (l, ds) = districts();
        assert_eq!(object_title(&l, &ds, 1), "Beta");
        assert_eq!(object_title(&l, &ds, 4), "Districts 5", "a blank name falls back");
    }

    #[test]
    fn facts_skip_ids_and_the_title_and_keep_blanks_distinct() {
        let (l, ds) = districts();
        let (f, total) = facts(&l, &ds, 2, 6, false);
        assert_eq!(total, 5);
        let names: Vec<&str> = f.iter().map(|f| f.field.as_str()).collect();
        assert_eq!(names, ["temp", "type", "pop"]);
        assert_eq!(f[0].value, None, "blank temperature is No value");
        assert_eq!(f[0].unit.as_deref(), Some("°C"));
        assert_eq!(f[2].value.as_deref(), Some("50,000"));
        let (all, _) = facts(&l, &ds, 5, 6, true);
        assert_eq!(all.len(), 5);
        assert_eq!(all[4].value.as_deref(), Some("0"), "zero is not blank");
    }

    #[test]
    fn filters_mask_rows() {
        let (_, ds) = districts();
        let f = Filter { conditions: vec![Condition { field: "pop".into(), test: Test::Above(500_000.0) }] };
        assert_eq!(filter_mask(&ds, Some(&f)), Some(vec![false, true, false, true, false, false]));
        assert_eq!(filter_mask(&ds, None), None);
    }

    #[test]
    fn table_sorts_views_with_blanks_last() {
        let (l, ds) = districts();
        assert_eq!(table_rows(&ds, None, Some((2, true))), [3, 0, 1, 5, 4, 2]);
        assert_eq!(table_rows(&ds, None, Some((2, false))), [4, 1, 5, 0, 3, 2]);
        let mask = [true, false, true, true, true, true];
        assert_eq!(table_rows(&ds, Some(&mask), None), [0, 2, 3, 4, 5]);
        assert_eq!(table_columns(&l, &ds)[0], 0, "identifier pinned left");
    }

    #[test]
    fn groups_match_the_legend() {
        let (mut l, ds) = districts();
        l.style.color_by = Some(ColorBy {
            field: "temp".into(),
            categories: false,
            classes: Classes::Groups { count: 5, equal_steps: false },
            palette: "heat".into(),
            per_km2: false,
            fit_to_view: false,
        });
        let rc = row_classes(&l, &ds, None, None, &|_| None).unwrap();
        let legend_colors: Vec<Rgba> =
            rc.legend.items.iter().filter_map(|i| if let LegendItem::Swatch { color, .. } = i { Some(*color) } else { None }).collect();
        // Every coloured row's colour appears in the legend; the blank row is No data.
        for (r, i) in rc.index.iter().enumerate() {
            let (c, _) = rc.combos[usize::from(*i)];
            if r == 2 {
                assert_eq!(c, palette::NO_DATA);
            } else {
                assert!(legend_colors.contains(&c), "row {r}");
            }
        }
        assert!(rc.legend.items.contains(&LegendItem::NoData { count: 1 }));
        assert_eq!(rc.legend.title.as_deref(), Some("temp (°C)"));
    }

    #[test]
    fn categories_keep_seven_and_other() {
        let mut ds = Dataset::new("c", GeometryType::Point, vec![Field::new("k", FieldType::Text)]);
        for k in 0..20 {
            for _ in 0..(20 - k) {
                ds.push(Some(Geometry::Point(Coord::new(0.0, 0.0))), vec![Value::Text(format!("v{k}"))]);
            }
        }
        let (mut l, _) = districts();
        l.profile.fields = vec![field("k", FieldRole::Category, None)];
        l.style.color_by = Some(ColorBy {
            field: "k".into(),
            categories: true,
            classes: Classes::Smooth,
            palette: "viridis".into(),
            per_km2: false,
            fit_to_view: false,
        });
        let rc = row_classes(&l, &ds, None, None, &|_| None).unwrap();
        assert_eq!(rc.legend.items.len(), 8);
        assert_eq!(rc.legend.items[7], LegendItem::Swatch { color: OTHER, label: "Other (13 values)".into() });
    }

    #[test]
    fn sizes_are_area_proportional() {
        let (mut l, ds) = districts();
        l.style.size_by = Some(lm_core::document::SizeBy { field: "pop".into(), max_size: 24.0 });
        let rc = row_classes(&l, &ds, None, None, &|_| None).unwrap();
        let size = |r: usize| rc.combos[usize::from(rc.index[r])].1;
        assert_eq!(size(3), 24.0);
        // 200,000 / 800,000 = 1/4 of the area → half the diameter.
        let a = size(1) / size(3);
        assert!((a - (600_000f32 / 800_000.0).sqrt()).abs() < 0.03, "{a}");
        assert!(rc.legend.items.iter().filter(|i| matches!(i, LegendItem::Size { .. })).count() == 3);
    }

    #[test]
    fn quantiles_and_equal_steps() {
        let v: Vec<f64> = (1..=10).map(f64::from).collect();
        assert_eq!(quantile_breaks(&v, 5), [2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(equal_breaks(0.0, 10.0, 5), [2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(quantile_breaks(&[1.0, 1.0, 1.0], 5), [1.0]);
    }

    #[test]
    fn presets_follow_the_fields() {
        let (l, _) = districts();
        let names: Vec<String> = presets(&l).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["Single color", "By type", "By temp", "Size by temp", "Heat", "Labels on"]);
    }

    #[test]
    fn values_search_finds_whole_matches_first() {
        let (l, ds) = districts();
        let hits = search_values(&l, &ds, "eta", 10);
        assert_eq!(hits.iter().map(|h| h.row).collect::<Vec<_>>(), [1, 5]);
        let hits = search_values(&l, &ds, "residential", 10);
        assert_eq!(hits.len(), 3);
    }
}

#[cfg(test)]
mod example_tests {
    use super::*;

    /// The bundled countries: colouring by a measure gives one legend swatch per class.
    #[test]
    fn countries_color_by_gdp() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples/countries.geojson");
        let layers = crate::import::import_path(&path, &crate::import::Ctx::detached(crate::import::Ticket::default())).unwrap();
        let l0 = &layers[0];
        let crate::import::LayerData::Vector(ds) = &l0.data else { panic!() };
        let mut doc = lm_core::document::Document::new();
        let tx = doc.add_layer_tx(l0.profile.clone());
        doc.apply(&tx).unwrap();
        let mut layer = doc.layers[0].clone();
        let presets = presets(&layer);
        let (name, s) = presets.iter().find(|(n, _)| n.contains("gdp")).cloned().unwrap();
        layer.style = s;
        let rc = classes_for(&layer, ds, None, None).unwrap();
        eprintln!("{name}: {:?}", rc.legend);
        assert!(rc.legend.items.len() >= 3);
    }

    /// Years read as "2019", never "2,019"; other numbers keep their separators.
    #[test]
    fn years_have_no_thousands_separator() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples/countries.geojson");
        let layers = crate::import::import_path(&path, &crate::import::Ctx::detached(crate::import::Ticket::default())).unwrap();
        let crate::import::LayerData::Vector(ds) = &layers[0].data else { panic!() };
        let year = field_index(ds, "gdp_year").unwrap();
        let pop = field_index(ds, "pop_est").unwrap();
        let t = cell_text(ds, 0, year).unwrap();
        assert!(t.len() == 4 && !t.contains(','), "{t}");
        assert!(cell_text(ds, 0, pop).unwrap().contains(','));
        assert!(is_year_name("YearBuilt") && is_year_name("fiscal_yr") && !is_year_name("layer") && !is_year_name("yield"));
    }
}

#[cfg(test)]
mod v1_tests {
    use super::*;

    /// Fixture V1's reference ranks (`fixtures/V1/expected_insight.csv`, from pandas): every rank,
    /// tie, median and phrase on the card agrees. The card says "highest" for first place and
    /// names the denominator ("of 16 with values") where the reference phrase doesn't.
    #[test]
    fn v1_population_ranks_match_the_reference() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/V1");
        let layers =
            crate::import::import_path(&root.join("districts.geojson"), &crate::import::Ctx::detached(crate::import::Ticket::default()))
                .unwrap();
        let crate::import::LayerData::Vector(ds) = &layers[0].data else { panic!() };
        let mut doc = lm_core::document::Document::new();
        let tx = doc.add_layer_tx(layers[0].profile.clone());
        doc.apply(&tx).unwrap();
        let layer = doc.layers[0].clone();
        let index = build_index(&layer, ds, None);
        let ix = index.measures.get("population").expect("population is a measure");
        assert_eq!(ix.len(), 16);
        assert_eq!(ix.median(), Some(1_140_872.0));
        let csv = std::fs::read_to_string(root.join("expected_insight.csv")).unwrap();
        let idf = field_index(ds, "district_id").unwrap();
        let pf = field_index(ds, "population").unwrap();
        for line in csv.lines().skip(1) {
            let c: Vec<&str> = line.split(',').collect();
            let row = (0..ds.features.len()).find(|r| cell_text(ds, *r, idf).as_deref() == Some(c[0])).unwrap();
            let v = cell_number(ds, row, pf);
            if c[1].is_empty() {
                assert_eq!(v, None, "{}", c[0]);
                continue;
            }
            let rank = ix.rank(v.unwrap()).unwrap();
            assert_eq!(rank.higher + 1, c[3].parse::<usize>().unwrap(), "{} from top", c[0]);
            assert_eq!(rank.lower + 1, c[4].parse::<usize>().unwrap(), "{} from bottom", c[0]);
            assert_eq!(rank.equal, c[5].parse::<usize>().unwrap(), "{} ties", c[0]);
            let rows = insight_rows(&layer, ds, &index, row, 3);
            let pop = rows.iter().find(|r| r.field == "population").unwrap();
            let expected = c[8].replace("1st highest", "highest").replace("1st lowest", "lowest") + " with values";
            assert_eq!(pop.phrase.as_deref(), Some(expected.as_str()), "{}", c[0]);
        }
    }
}

#[cfg(test)]
mod stress {
    use super::*;

    fn open(rel: &str) -> (DocLayer, std::sync::Arc<Dataset>) {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/generated").join(rel);
        let layers = crate::import::import_path(&p, &crate::import::Ctx::detached(crate::import::Ticket::default())).unwrap();
        let crate::import::LayerData::Vector(ds) = &layers[0].data else { panic!() };
        let mut doc = lm_core::document::Document::new();
        let tx = doc.add_layer_tx(layers[0].profile.clone());
        doc.apply(&tx).unwrap();
        (doc.layers[0].clone(), ds.clone())
    }

    /// Gate 3 stress numbers (run in release: `cargo test --release -p lm-jobs stress -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn gate3_stress() {
        for rel in ["V7/v7_polygons.geojson", "F2/f2_points.csv", "F1/f1_points.geojson"] {
            let (mut l, ds) = open(rel);
            let n = ds.features.len();
            let t = std::time::Instant::now();
            let index = build_index(&l, &ds, None);
            let build = t.elapsed();
            let t = std::time::Instant::now();
            let rows = insight_rows(&l, &ds, &index, n / 2, 3);
            let card = t.elapsed();
            let t = std::time::Instant::now();
            let (facts, _) = facts(&l, &ds, n / 2, 6, false);
            let title = object_title(&l, &ds, n / 2);
            let card2 = t.elapsed();
            let measure = l.profile.fields.iter().find(|f| index.measures.contains_key(&f.name)).map(|f| f.name.clone());
            let fi = measure.as_deref().and_then(|m| field_index(&ds, m));
            let t = std::time::Instant::now();
            let sorted = table_rows(&ds, None, fi.map(|f| (f, false)));
            let sort = t.elapsed();
            if let Some((_, s)) =
                presets(&l).into_iter().find(|(name, _)| name.starts_with("By ") && measure.as_deref().is_some_and(|m| name.ends_with(m)))
            {
                l.style = s;
            }
            let t = std::time::Instant::now();
            let rc = classes_for(&l, &ds, None, None);
            let style = t.elapsed();
            eprintln!(
                "{rel}: {n} rows; index {build:?}; insight {card:?} ({} rows); facts+title {card2:?} ({} facts, {title}); sort {sort:?} ({}); style by value {style:?} ({} symbols)",
                rows.len(),
                facts.len(),
                sorted.len(),
                rc.map_or(0, |c| c.combos.len())
            );
        }
    }
}

#[cfg(test)]
mod overlap_tests {
    use super::*;
    use crate::import::{Ctx, LayerData, import_path};

    fn overlap(path: &str) -> Option<f64> {
        let ls = import_path(std::path::Path::new(path), &Ctx::detached(Default::default())).ok()?;
        let LayerData::Vector(ds) = &ls.first()?.data else { return None };
        line_overlap(ds)
    }

    /// Lines that don't pile up keep the default look; a bundle of crossing tracks fades.
    #[test]
    fn crossing_tracks_fade_and_a_road_does_not() {
        use rustgis_engine::data::{Coord, Geometry, GeometryType};
        let mut tracks = Dataset::new("tracks", GeometryType::Polyline, vec![]);
        for i in 0..40 {
            let y = f64::from(i) * 0.01;
            tracks.push_next(Some(Geometry::Line(vec![vec![Coord::new(0.0, y), Coord::new(10.0, 1.0 - y)]])), vec![]);
        }
        let o = line_overlap(&tracks).unwrap();
        assert!(o >= 8.0, "{o}");
        assert!(dense_lines_style(o).is_some_and(|(a, w)| a < 64 && w == 1.0));
        let mut road = Dataset::new("road", GeometryType::Polyline, vec![]);
        road.push_next(Some(Geometry::Line(vec![vec![Coord::new(0.0, 0.0), Coord::new(10.0, 3.0)]])), vec![]);
        assert_eq!(dense_lines_style(line_overlap(&road).unwrap()), None);
    }

    /// The owner's data, when the drive is mounted (`cargo test -p lm-jobs overlap_of_real -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn overlap_of_real_layers() {
        for p in [
            "/Volumes/LaCie/data/layeredmap-example-survey/work/storms_1980_2026.gpkg",
            "/Volumes/LaCie/data/layeredmap-reference/rivers.shp",
            "/Volumes/LaCie/data/layeredmap-reference/coastline.shp",
        ] {
            let o = overlap(p);
            eprintln!("{p}: overlap {o:?} → {:?}", o.and_then(dense_lines_style));
        }
    }
}
