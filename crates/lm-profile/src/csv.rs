//! Catalog rows "CSV with lon/lat columns" and "CSV with ambiguous coordinate columns" (and, for
//! CSVs with easting/northing-looking values, "No coordinate system; projected-looking values").

use crate::crs::projected_question;
use crate::{lines, make, tokens};
use lm_core::model::{Assumption, AssumptionKind, Choice, Confidence, Consequence, Extent, Known, Subject, Treatment};

/// Share of non-blank sample rows that must parse as in-range coordinates for a pair to locate
/// the records without a question (catalog: "at least 95% of rows parse").
const PARSE_SHARE: f64 = 0.95;

/// A pair of columns that may hold the records' locations, with what the sample showed.
#[derive(Clone, Debug, PartialEq)]
pub struct CoordinatePair {
    /// The longitude (x) column's header, as written.
    pub x: String,
    /// The latitude (y) column's header, as written.
    pub y: String,
    pub x_index: usize,
    pub y_index: usize,
    /// True when this pair reads the named columns the other way round (the "lat" column as x).
    pub swapped: bool,
    /// Sample rows where both cells are non-blank.
    pub rows_nonblank: usize,
    /// Of those, rows where both parse as numbers.
    pub rows_numeric: usize,
    /// Of those, rows within ±180 / ±90.
    pub rows_in_range: usize,
    /// Sample rows (1-based data rows) with one or both cells blank.
    pub blank_rows: Vec<u64>,
    /// Sample rows (1-based) whose values fit only the other way round (a latitude above 90
    /// that would be a valid longitude, with a longitude that would be a valid latitude).
    pub swapped_rows: Vec<u64>,
    /// Sample rows (1-based) that parse but fit neither way.
    pub out_of_range_rows: Vec<u64>,
    /// Extent of the in-range rows (degrees), or of all numeric rows for a projected pair.
    pub extent: Option<Extent>,
}

impl CoordinatePair {
    /// Stable choice id: `columns:{x},{y}`.
    pub fn choice_id(&self) -> String {
        format!("columns:{},{}", self.x, self.y)
    }
    /// Label in the question card and the Change popover: `x / y`.
    pub fn label(&self) -> String {
        format!("{} / {}", self.x, self.y)
    }
    pub fn choice(&self) -> Choice {
        Choice::new(self.choice_id(), self.label())
    }
    fn share(&self, n: usize) -> f64 {
        if self.rows_nonblank == 0 { 0.0 } else { n as f64 / self.rows_nonblank as f64 }
    }
    /// Share of non-blank rows within ±180 / ±90.
    pub fn in_range_share(&self) -> f64 {
        self.share(self.rows_in_range)
    }
    /// Share of non-blank rows that parse as numbers.
    pub fn numeric_share(&self) -> f64 {
        self.share(self.rows_numeric)
    }
    fn reversed(&self) -> CoordinatePair {
        let mut p = self.clone();
        std::mem::swap(&mut p.x, &mut p.y);
        std::mem::swap(&mut p.x_index, &mut p.y_index);
        p.swapped = !self.swapped;
        p
    }
}

/// Where the records of a CSV (or a sheet) are, as far as the header and a sample of rows tell.
#[derive(Clone, Debug, PartialEq)]
pub enum CsvLocation {
    /// One pair of columns clearly locates the records (a visible
    /// [`AssumptionKind::CsvCoordinateColumns`]).
    Located { pair: CoordinatePair, assumption: Assumption },
    /// Several candidate pairs, or rows whose latitude is above 90: the
    /// [`AssumptionKind::CsvAmbiguousColumns`] question at import. `candidates[recommended]`
    /// is preselected; each candidate is one choice, then Not sure (open as a table).
    Ambiguous { candidates: Vec<CoordinatePair>, recommended: usize, assumption: Assumption },
    /// A pair of coordinate columns whose values are not degrees (350,000 / 3,450,000): the
    /// [`AssumptionKind::NoCrsProjected`] question at import.
    Projected { pair: CoordinatePair, assumption: Assumption },
    /// No coordinate columns: open as a table ("Find location…" stays available).
    Table { reason: String },
}

impl CsvLocation {
    /// The assumption to attach to the layer, if any.
    pub fn assumption(&self) -> Option<&Assumption> {
        match self {
            CsvLocation::Located { assumption, .. }
            | CsvLocation::Ambiguous { assumption, .. }
            | CsvLocation::Projected { assumption, .. } => Some(assumption),
            CsvLocation::Table { .. } => None,
        }
    }

    /// The columns to profile as [`lm_core::model::FieldRole::Coordinate`], with how sure:
    /// `Likely` for a located pair, `Possible` for every column of every candidate otherwise.
    pub fn coordinate_columns(&self) -> Vec<crate::CoordinateColumn> {
        let col = |name: &str, confidence| crate::CoordinateColumn { name: name.to_string(), confidence };
        match self {
            CsvLocation::Located { pair, .. } => vec![col(&pair.x, Confidence::Likely), col(&pair.y, Confidence::Likely)],
            CsvLocation::Projected { pair, .. } => vec![col(&pair.x, Confidence::Possible), col(&pair.y, Confidence::Possible)],
            CsvLocation::Ambiguous { candidates, .. } => {
                let mut out: Vec<crate::CoordinateColumn> = Vec::new();
                for p in candidates {
                    for n in [&p.x, &p.y] {
                        if !out.iter().any(|c| &c.name == n) {
                            out.push(col(n, Confidence::Possible));
                        }
                    }
                }
                out
            }
            CsvLocation::Table { .. } => vec![],
        }
    }
}

/// How a column writes its numbers: what a comma means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberStyle {
    /// A decimal point; a comma is not part of a number (cells with commas stay text).
    Point,
    /// A decimal point, with commas grouping thousands (`12,500`, `1,250,000.5`).
    Thousands,
    /// A decimal comma (`121,473701`, `1234,50`); no grouping.
    DecimalComma,
}

/// What one cell's commas say about the column's style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommaCell {
    /// No comma.
    None,
    /// `^-?\d{1,3}(,\d{3})+(\.\d+)?$` with two or more groups or a decimal point: thousands.
    Thousands,
    /// `^-?\d{1,3},\d{3}$`: thousands or a decimal comma; can't tell alone.
    Either,
    /// `^-?\d+,\d+$` otherwise (not exactly three digits after the comma, or more than three
    /// before it): a decimal comma.
    Decimal,
    /// Anything else with a comma ("1,2,3", "1.234,5"): not a number in any style.
    Other,
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn comma_cell(cell: &str) -> CommaCell {
    let s = cell.trim();
    if !s.contains(',') {
        return CommaCell::None;
    }
    let body = s.strip_prefix('-').unwrap_or(s);
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    if frac.is_some_and(|f| !all_digits(f)) {
        return CommaCell::Other;
    }
    let groups: Vec<&str> = int.split(',').collect();
    let grouped =
        groups.first().is_some_and(|g| all_digits(g) && g.len() <= 3) && groups.iter().skip(1).all(|g| g.len() == 3 && all_digits(g));
    if grouped {
        return if groups.len() == 2 && frac.is_none() { CommaCell::Either } else { CommaCell::Thousands };
    }
    match (groups.as_slice(), frac) {
        ([a, b], None) if all_digits(a) && all_digits(b) => CommaCell::Decimal,
        _ => CommaCell::Other,
    }
}

/// The number style of one column, decided from all its cells as written.
///
/// - No cell has a comma → [`NumberStyle::Point`] (`Certain`).
/// - Every comma cell is grouped like `12,500` / `1,250,000` / `12,500.50`, and at least one has
///   two groups or a decimal point → [`NumberStyle::Thousands`] (`Likely`).
/// - Some comma cell has other than exactly three digits after its comma (`121,473701`, `12,5`)
///   or more than three before it, and none is grouped unmistakably →
///   [`NumberStyle::DecimalComma`] (`Likely`).
/// - Every comma cell is `d,ddd` (one to three digits, a comma, three digits) and nothing else
///   tells: with a `;` delimiter (`delimiter == Some(';')`, the European convention) →
///   [`NumberStyle::DecimalComma`] (`Likely`); otherwise **not guessed**:
///   [`NumberStyle::Point`] with `Unknown` confidence, so those cells stay text.
/// - Both unmistakable kinds in one column → `Point`, `Unknown` (they stay text).
pub fn number_style<'a>(cells: impl IntoIterator<Item = &'a str>, delimiter: Option<char>) -> Known<NumberStyle> {
    let (mut thousands, mut either, mut decimal) = (0usize, 0usize, 0usize);
    let mut example: Option<String> = None;
    for c in cells {
        match comma_cell(c) {
            CommaCell::Thousands => thousands += 1,
            CommaCell::Either => {
                either += 1;
                if example.is_none() {
                    example = Some(c.trim().to_string());
                }
            }
            CommaCell::Decimal => decimal += 1,
            CommaCell::None | CommaCell::Other => {}
        }
    }
    match (thousands > 0, decimal > 0) {
        (true, true) => Known::unknown(
            NumberStyle::Point,
            "Commas are used both as thousands separators and as decimal commas; cells with commas are kept as text.",
        ),
        (true, false) => Known::likely(NumberStyle::Thousands, "Commas group thousands (as in 1,250,000)."),
        (false, true) => {
            Known::likely(NumberStyle::DecimalComma, "Commas are decimal commas (digits after the comma aren't groups of three).")
        }
        (false, false) if either == 0 => Known::certain(NumberStyle::Point, "No commas in the numbers."),
        (false, false) if delimiter == Some(';') => {
            Known::likely(NumberStyle::DecimalComma, "Commas read as decimal commas, because the columns are separated by semicolons.")
        }
        (false, false) => Known::unknown(
            NumberStyle::Point,
            format!(
                "Values such as {} could use a thousands separator or a decimal comma, and nothing in the file tells which; kept as text.",
                example.unwrap_or_default()
            ),
        ),
    }
}

/// Parses a number as written in a cell of a column in `style` ([`number_style`]). Blank, or not
/// a number in that style: `None`.
pub fn parse_number_with(cell: &str, style: NumberStyle) -> Option<f64> {
    let s = cell.trim();
    if s.is_empty() {
        return None;
    }
    let v = if !s.contains(',') {
        s.parse::<f64>().ok()
    } else {
        match (style, comma_cell(s)) {
            (NumberStyle::Thousands, CommaCell::Thousands | CommaCell::Either) => s.replace(',', "").parse::<f64>().ok(),
            (NumberStyle::DecimalComma, CommaCell::Decimal | CommaCell::Either) => s.replace(',', ".").parse::<f64>().ok(),
            _ => None,
        }
    };
    v.filter(|v| v.is_finite())
}

/// Parses a single cell whose style is unmistakable on its own: `121.47`, ` -3 `, `121,473701`
/// and `1234,50` (decimal commas), `1,250,000` and `1,234.5` (thousands). A cell like `12,500`
/// could be either and gives `None`: decide per column with [`number_style`] and
/// [`parse_number_with`].
pub fn parse_number(cell: &str) -> Option<f64> {
    let style = number_style([cell], None);
    if style.confidence == Confidence::Unknown {
        return None;
    }
    parse_number_with(cell, style.value)
}

/// The number style of column `i` of `rows`.
pub(crate) fn column_style(rows: &[Vec<String>], i: usize, delimiter: Option<char>) -> NumberStyle {
    number_style(rows.iter().map(|r| r.get(i).map(String::as_str).unwrap_or("")), delimiter).value
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Family {
    /// lon/lng/longitude/经度 with lat/latitude/纬度.
    LonLat,
    /// x/y, xcoord/ycoord.
    Xy,
    /// easting/northing: projected by name.
    EastNorth,
}

/// A header that names a coordinate: its axis, family, and the rest of the name ("2" for
/// "lng2", "gcj" for "Lng_GCJ"), which pairs it with its partner.
#[derive(Clone, Debug)]
struct CoordName {
    index: usize,
    axis: Axis,
    family: Family,
    key: String,
}

fn axis_word(token: &str) -> Option<(Axis, Family)> {
    match token {
        "lon" | "lng" | "long" | "longitude" | "longitud" | "longitudine" => Some((Axis::X, Family::LonLat)),
        "lat" | "latitude" | "latitud" | "latitudine" => Some((Axis::Y, Family::LonLat)),
        "x" | "xcoord" | "xcoordinate" => Some((Axis::X, Family::Xy)),
        "y" | "ycoord" | "ycoordinate" => Some((Axis::Y, Family::Xy)),
        "easting" | "east" => Some((Axis::X, Family::EastNorth)),
        "northing" | "north" => Some((Axis::Y, Family::EastNorth)),
        _ => None,
    }
}

fn coord_name(index: usize, header: &str) -> Option<CoordName> {
    let lower = header.trim().to_lowercase();
    for (word, axis) in [("经度", Axis::X), ("纬度", Axis::Y)] {
        if lower.contains(word) {
            return Some(CoordName { index, axis, family: Family::LonLat, key: lower.replace(word, "") });
        }
    }
    let toks = tokens(&lower);
    let mut found: Option<(Axis, Family)> = None;
    let mut rest: Vec<String> = Vec::new();
    for t in &toks {
        // "lng2" → "lng" + "2".
        let stem = t.trim_end_matches(|c: char| c.is_ascii_digit());
        let digits = &t[stem.len()..];
        match axis_word(stem) {
            Some(w) if found.is_none() => {
                found = Some(w);
                if !digits.is_empty() {
                    rest.push(digits.to_string());
                }
            }
            _ => rest.push(t.clone()),
        }
    }
    let (axis, family) = found?;
    // Common qualifiers that do not tell pairs apart.
    rest.retain(|t| !matches!(t.as_str(), "dd" | "deg" | "degrees" | "decimal" | "wgs84" | "coord" | "coordinate"));
    Some(CoordName { index, axis, family, key: rest.join("_") })
}

/// The pairs of columns named like coordinates: by family and the rest of the name, then (for
/// leftovers) the single unpaired x with the single unpaired y.
fn named_pairs(headers: &[String]) -> Vec<(usize, usize, Family)> {
    let names: Vec<CoordName> = headers.iter().enumerate().filter_map(|(i, h)| coord_name(i, h)).collect();
    let mut pairs = Vec::new();
    let mut used = vec![false; names.len()];
    // The y columns by family and key, in header order: each x takes the first one still free
    // (looked up, not searched for: a header of thousands of coordinate-like names was
    // quadratic).
    let mut ys: std::collections::HashMap<(Family, &str), std::collections::VecDeque<usize>> = std::collections::HashMap::new();
    for (j, b) in names.iter().enumerate().filter(|(_, b)| b.axis == Axis::Y) {
        ys.entry((b.family, b.key.as_str())).or_default().push_back(j);
    }
    for (i, a) in names.iter().enumerate() {
        if a.axis != Axis::X {
            continue;
        }
        if let Some(j) = ys.get_mut(&(a.family, a.key.as_str())).and_then(std::collections::VecDeque::pop_front) {
            used[i] = true;
            used[j] = true;
            if let Some(b) = names.get(j) {
                pairs.push((a.index, b.index, a.family));
            }
        }
    }
    let left = |axis| names.iter().enumerate().filter(|(i, n)| !used[*i] && n.axis == axis).map(|(_, n)| n).collect::<Vec<_>>();
    if let ([x], [y]) = (left(Axis::X).as_slice(), left(Axis::Y).as_slice()) {
        let family = if x.family == y.family { x.family } else { Family::Xy };
        pairs.push((x.index, y.index, family));
    }
    pairs
}

fn evaluate(headers: &[String], rows: &[Vec<String>], xi: usize, yi: usize, delimiter: Option<char>) -> CoordinatePair {
    let (sx, sy) = (column_style(rows, xi, delimiter), column_style(rows, yi, delimiter));
    let name = |i: usize| headers.get(i).map(|h| h.trim().to_string()).unwrap_or_default();
    let mut p = CoordinatePair {
        x: name(xi),
        y: name(yi),
        x_index: xi,
        y_index: yi,
        swapped: false,
        rows_nonblank: 0,
        rows_numeric: 0,
        rows_in_range: 0,
        blank_rows: vec![],
        swapped_rows: vec![],
        out_of_range_rows: vec![],
        extent: None,
    };
    let mut ext: Option<Extent> = None;
    let mut grow = |x: f64, y: f64| {
        let e = ext.get_or_insert(Extent { min_x: x, min_y: y, max_x: x, max_y: y });
        e.min_x = e.min_x.min(x);
        e.min_y = e.min_y.min(y);
        e.max_x = e.max_x.max(x);
        e.max_y = e.max_y.max(y);
    };
    for (r, row) in rows.iter().enumerate() {
        let row_no = r as u64 + 1;
        let cell = |i: usize| row.get(i).map(|s| s.trim()).unwrap_or("");
        let (cx, cy) = (cell(xi), cell(yi));
        if cx.is_empty() && cy.is_empty() && row.iter().all(|c| c.trim().is_empty()) {
            continue; // an empty line
        }
        if cx.is_empty() || cy.is_empty() {
            p.blank_rows.push(row_no);
            continue;
        }
        p.rows_nonblank += 1;
        let Some((x, y)) = parse_number_with(cx, sx).zip(parse_number_with(cy, sy)) else { continue };
        p.rows_numeric += 1;
        let fits = |lon: f64, lat: f64| (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat);
        if fits(x, y) {
            p.rows_in_range += 1;
            grow(x, y);
        } else if fits(y, x) {
            p.swapped_rows.push(row_no);
        } else {
            p.out_of_range_rows.push(row_no);
        }
    }
    p.extent = ext;
    p
}

/// Extent of all numeric rows of a pair (for projected values).
fn numeric_extent(rows: &[Vec<String>], xi: usize, yi: usize, delimiter: Option<char>) -> Option<Extent> {
    let (sx, sy) = (column_style(rows, xi, delimiter), column_style(rows, yi, delimiter));
    let mut ext: Option<Extent> = None;
    for row in rows {
        let cell = |i: usize| row.get(i).map(String::as_str).unwrap_or("");
        if let Some((x, y)) = parse_number_with(cell(xi), sx).zip(parse_number_with(cell(yi), sy)) {
            let e = ext.get_or_insert(Extent { min_x: x, min_y: y, max_x: x, max_y: y });
            e.min_x = e.min_x.min(x);
            e.min_y = e.min_y.min(y);
            e.max_x = e.max_x.max(x);
            e.max_y = e.max_y.max(y);
        }
    }
    ext
}

/// Finds the columns that locate the records of a CSV (or a spreadsheet sheet).
///
/// - `headers`: the header row, as written.
/// - `sample_rows`: data rows (the first few thousand are plenty), split into cells, raw.
/// - `file_name`: for evidence only (China offset hints are [`crate::china_offset`]'s job).
///
/// Columns are found by name, case-insensitively, by token: lon / lng / long / longitude / x /
/// 经度 with lat / latitude / y / 纬度, also inside longer names ("Lng_GCJ", "pickup_lat",
/// "lng2"); easting / northing count as projected names. Columns pair up by the rest of their
/// names ("lng2" with "lat2"). Each pair is checked on the sample: rows where both cells are
/// non-blank must parse (in each column's [`number_style`]) within ±180 / ±90 for at least 95% of rows.
///
/// - Exactly one such pair and no row that only fits swapped → [`CsvLocation::Located`].
/// - Two or more plausible pairs, or a pair with latitudes above 90 that fit swapped →
///   [`CsvLocation::Ambiguous`] (a swapped reading of the columns is offered as a candidate).
/// - No degree pair, but a named pair that parses with values beyond degrees →
///   [`CsvLocation::Projected`].
/// - Otherwise → [`CsvLocation::Table`].
pub fn csv_locate(headers: &[String], sample_rows: &[Vec<String>], file_name: &str) -> CsvLocation {
    csv_locate_delimited(headers, sample_rows, file_name, None)
}

/// [`csv_locate`] for a delimited file whose delimiter is known: a `;` delimiter reads `d,ddd`
/// values as decimal commas ([`number_style`]). `None` for a sheet.
pub fn csv_locate_delimited(headers: &[String], sample_rows: &[Vec<String>], file_name: &str, delimiter: Option<char>) -> CsvLocation {
    let named = named_pairs(headers);
    if named.is_empty() {
        return CsvLocation::Table { reason: format!("{file_name}: no columns named like longitude and latitude.") };
    }
    let evaluated: Vec<(CoordinatePair, Family)> =
        named.iter().map(|&(x, y, f)| (evaluate(headers, sample_rows, x, y, delimiter), f)).collect();
    // Plausible as degrees: parses, and fits one way or the other in at least 95% of rows.
    let plausible: Vec<&CoordinatePair> = evaluated
        .iter()
        .filter(|(p, f)| *f != Family::EastNorth && p.rows_nonblank > 0)
        .map(|(p, _)| p)
        .filter(|p| p.numeric_share() >= PARSE_SHARE && p.share(p.rows_in_range + p.swapped_rows.len()) >= PARSE_SHARE)
        .collect();
    match plausible.as_slice() {
        [p] if p.swapped_rows.is_empty() && p.in_range_share() >= PARSE_SHARE => {
            let pair = (*p).clone();
            let assumption = located_assumption(&pair, sample_rows.len());
            CsvLocation::Located { pair, assumption }
        }
        [p] => {
            // Latitudes above 90: offer the columns as named and swapped.
            let as_named = (*p).clone();
            let mut swapped = evaluate(headers, sample_rows, p.y_index, p.x_index, delimiter);
            swapped.swapped = true;
            let recommended = usize::from(swapped.rows_in_range > as_named.rows_in_range);
            let candidates = vec![as_named, swapped];
            let assumption = ambiguous_question(&candidates, recommended);
            CsvLocation::Ambiguous { candidates, recommended, assumption }
        }
        [] => {
            let projected = evaluated.iter().map(|(p, _)| p).find(|p| {
                p.rows_nonblank > 0
                    && p.numeric_share() >= PARSE_SHARE
                    && p.rows_in_range + p.swapped_rows.len() < p.rows_numeric
                    && numeric_extent(sample_rows, p.x_index, p.y_index, delimiter)
                        .is_some_and(|e| e.max_x.abs().max(e.min_x.abs()) > 1000.0 || e.max_y.abs().max(e.min_y.abs()) > 1000.0)
            });
            match projected {
                Some(p) => {
                    let mut pair = p.clone();
                    pair.extent = numeric_extent(sample_rows, p.x_index, p.y_index, delimiter);
                    CsvLocation::Projected { pair, assumption: projected_question() }
                }
                None => CsvLocation::Table {
                    reason: format!(
                        "{file_name}: columns {} don't hold coordinates in enough rows.",
                        evaluated.iter().map(|(p, _)| p.label()).collect::<Vec<_>>().join(", ")
                    ),
                },
            }
        }
        many => {
            let mut candidates: Vec<CoordinatePair> = many.iter().map(|p| (*p).clone()).collect();
            // Stable order: by column position.
            candidates.sort_by_key(|p| (p.x_index.min(p.y_index), p.x_index));
            let recommended = candidates
                .iter()
                .enumerate()
                .max_by(|(ia, a), (ib, b)| a.rows_in_range.cmp(&b.rows_in_range).then(ib.cmp(ia)))
                .map(|(i, _)| i)
                .unwrap_or(0);
            let assumption = ambiguous_question(&candidates, recommended);
            CsvLocation::Ambiguous { candidates, recommended, assumption }
        }
    }
}

fn located_assumption(pair: &CoordinatePair, _rows: usize) -> Assumption {
    let swapped = pair.reversed();
    make(
        AssumptionKind::CsvCoordinateColumns,
        Subject::Layer,
        pair.choice(),
        vec![pair.choice(), swapped.choice(), Choice::not_sure()],
        Confidence::Likely,
        Consequence::ChangesLocations,
        Treatment::Visible,
        lines::csv_located(&pair.x, &pair.y),
    )
}

fn ambiguous_question(candidates: &[CoordinatePair], recommended: usize) -> Assumption {
    let mut alternatives: Vec<Choice> = candidates.iter().map(CoordinatePair::choice).collect();
    alternatives.push(Choice::not_sure());
    let chosen = candidates.get(recommended).map(CoordinatePair::choice).unwrap_or_else(Choice::not_sure);
    make(
        AssumptionKind::CsvAmbiguousColumns,
        Subject::Layer,
        chosen,
        alternatives,
        Confidence::Possible,
        Consequence::ChangesLocations,
        Treatment::Question,
        lines::CSV_AMBIGUOUS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    fn rows(v: &[&[&str]]) -> Vec<Vec<String>> {
        v.iter().map(|r| s(r)).collect()
    }
    fn shanghai(n: usize) -> Vec<Vec<String>> {
        (0..n).map(|i| s(&["a", &format!("{:.4}", 121.2 + i as f64 * 0.01), &format!("{:.4}", 31.0 + i as f64 * 0.01)])).collect()
    }

    #[test]
    fn csv_coordinate_columns_matches_header_variants() {
        let variants: &[(&str, &str)] = &[
            ("lon", "lat"),
            ("LON", "LAT"),
            ("lng", "lat"),
            ("Long", "Lat"),
            ("LONG", "LAT"),
            ("longitude", "latitude"),
            ("Longitude", "Latitude"),
            ("x", "y"),
            ("X", "Y"),
            ("经度", "纬度"),
            ("Lng_GCJ", "Lat_GCJ"),
            ("lon_dd", "lat_dd"),
            ("pickup_lng", "pickup_lat"),
        ];
        for (x, y) in variants {
            let headers = s(&["name", x, y]);
            match csv_locate(&headers, &shanghai(20), "f.csv") {
                CsvLocation::Located { pair, assumption } => {
                    assert_eq!((pair.x.as_str(), pair.y.as_str()), (*x, *y));
                    assert_eq!(assumption.kind, AssumptionKind::CsvCoordinateColumns);
                    assert_eq!(assumption.treatment, Treatment::Visible);
                    assert_eq!(assumption.line, format!("Located using {x} and {y} as GPS coordinates (WGS 84)."));
                    assert_eq!(assumption.alternatives.last(), Some(&Choice::not_sure()));
                }
                other => panic!("{x}/{y}: {other:?}"),
            }
        }
    }

    #[test]
    fn csv_coordinate_columns_found_when_lat_comes_first() {
        let headers = s(&["lat", "lon"]);
        let r: Vec<Vec<String>> = shanghai(10).into_iter().map(|r| vec![r[2].clone(), r[1].clone()]).collect();
        let CsvLocation::Located { pair, .. } = csv_locate(&headers, &r, "f.csv") else { panic!() };
        assert_eq!((pair.x_index, pair.y_index), (1, 0));
    }

    #[test]
    fn csv_coordinate_columns_tolerates_few_blanks_and_decimal_commas() {
        let headers = s(&["经度", "纬度"]);
        let mut r: Vec<Vec<String>> = (0..30).map(|_| s(&["121,473701", "31,230416"])).collect();
        r.push(s(&["", ""]));
        r.push(s(&["121,5", ""]));
        let CsvLocation::Located { pair, .. } = csv_locate(&headers, &r, "f.csv") else { panic!() };
        assert_eq!(pair.blank_rows, vec![32]);
        assert_eq!(pair.rows_in_range, 30);
    }

    #[test]
    fn csv_coordinate_columns_does_not_match_without_names_or_parse() {
        // No coordinate names.
        let headers = s(&["name", "a", "b"]);
        assert!(matches!(csv_locate(&headers, &shanghai(10), "f.csv"), CsvLocation::Table { .. }));
        // Names match but under 95% of rows parse.
        let headers = s(&["name", "lon", "lat"]);
        let mut r = shanghai(10);
        r.push(s(&["b", "n/a", "n/a"]));
        assert!(matches!(csv_locate(&headers, &r, "f.csv"), CsvLocation::Table { .. }));
        // "longname" is not a coordinate column.
        let headers = s(&["longname", "latency"]);
        assert!(matches!(csv_locate(&headers, &rows(&[&["1", "2"]]), "f.csv"), CsvLocation::Table { .. }));
    }

    #[test]
    fn csv_ambiguous_columns_matches_two_pairs() {
        let headers = s(&["id", "x", "y", "lat2", "lng2"]);
        let r: Vec<Vec<String>> = (0..20)
            .map(|i| {
                let d = i as f64 * 0.01;
                s(&[
                    &i.to_string(),
                    &format!("{}", 121.3 + d),
                    &format!("{}", 31.1 + d),
                    &format!("{}", 31.2 + d),
                    &format!("{}", 121.4 + d),
                ])
            })
            .collect();
        let CsvLocation::Ambiguous { candidates, assumption, .. } = csv_locate(&headers, &r, "f.csv") else { panic!() };
        let labels: Vec<String> = candidates.iter().map(CoordinatePair::label).collect();
        assert_eq!(labels, vec!["x / y", "lng2 / lat2"]);
        assert_eq!(assumption.kind, AssumptionKind::CsvAmbiguousColumns);
        assert_eq!(assumption.treatment, Treatment::Question);
        assert_eq!(assumption.line, "Which columns locate these records?");
        let ids: Vec<&str> = assumption.alternatives.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["columns:x,y", "columns:lng2,lat2", "not_sure"]);
    }

    #[test]
    fn csv_ambiguous_columns_matches_latitudes_above_90() {
        // Columns named the wrong way round.
        let headers = s(&["lon", "lat"]);
        let r: Vec<Vec<String>> = (0..10).map(|i| s(&[&format!("{}", 31.0 + i as f64 * 0.01), "121.5"])).collect();
        let CsvLocation::Ambiguous { candidates, recommended, assumption } = csv_locate(&headers, &r, "f.csv") else { panic!() };
        assert_eq!(candidates.len(), 2);
        assert!(candidates[recommended].swapped);
        assert_eq!(assumption.chosen.id, "columns:lat,lon");
        // A few swapped rows also ask.
        let headers = s(&["lon", "lat"]);
        let mut r = shanghai(20).into_iter().map(|r| vec![r[1].clone(), r[2].clone()]).collect::<Vec<_>>();
        r[4] = s(&["31.1", "121.4"]);
        let CsvLocation::Ambiguous { candidates, .. } = csv_locate(&headers, &r, "f.csv") else { panic!() };
        assert_eq!(candidates[0].swapped_rows, vec![5]);
    }

    #[test]
    fn csv_ambiguous_columns_does_not_match_one_clean_pair() {
        let headers = s(&["name", "lon", "lat", "x2"]);
        let r: Vec<Vec<String>> = shanghai(10)
            .into_iter()
            .map(|mut r| {
                r.push("abc".into());
                r
            })
            .collect();
        assert!(matches!(csv_locate(&headers, &r, "f.csv"), CsvLocation::Located { .. }));
    }

    #[test]
    fn csv_projected_values_ask_where() {
        let headers = s(&["id", "x", "y"]);
        let r: Vec<Vec<String>> =
            (0..10).map(|i| s(&[&i.to_string(), &format!("{}", 350_000 + i * 10), &format!("{}", 3_450_000 + i * 10)])).collect();
        let CsvLocation::Projected { pair, assumption } = csv_locate(&headers, &r, "f.csv") else { panic!() };
        assert_eq!(assumption.kind, AssumptionKind::NoCrsProjected);
        assert_eq!(assumption.line, "Where should this data be?");
        assert_eq!(pair.extent.map(|e| e.min_x), Some(350_000.0));
        let headers = s(&["easting", "northing"]);
        let r = rows(&[&["614006.6", "3420504.5"], &["672100.3", "3487815.1"]]);
        assert!(matches!(csv_locate(&headers, &r, "f.csv"), CsvLocation::Projected { .. }));
    }

    #[test]
    fn parse_number_accepts_commas_only_when_unambiguous() {
        assert_eq!(parse_number(" 121.5 "), Some(121.5));
        assert_eq!(parse_number("1234,50"), Some(1234.5));
        assert_eq!(parse_number("121,473701"), Some(121.473701));
        assert_eq!(parse_number("1,234.5"), Some(1234.5));
        assert_eq!(parse_number("1,250,000"), Some(1_250_000.0));
        // Regression (S1): "12,500" was 12.5. Alone it could be either, so it is not a number.
        assert_eq!(parse_number("12,500"), None);
        assert_eq!(parse_number("1.234,5"), None);
        assert_eq!(parse_number("1,2,3"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("NaN"), None);
    }

    fn style(cells: &[&str], delimiter: Option<char>) -> (NumberStyle, Confidence) {
        let k = number_style(cells.iter().copied(), delimiter);
        (k.value, k.confidence)
    }

    #[test]
    fn number_style_is_decided_per_column() {
        use NumberStyle::*;
        assert_eq!(style(&["12", "3.5", ""], None), (Point, Confidence::Certain));
        assert_eq!(style(&["12,500", "3,400", "1,250,000"], None), (Thousands, Confidence::Likely));
        assert_eq!(style(&["12,500.5", "980"], None), (Thousands, Confidence::Likely));
        assert_eq!(style(&["121,473701", "121,5"], None), (DecimalComma, Confidence::Likely));
        assert_eq!(style(&["12,5", "3,400"], None), (DecimalComma, Confidence::Likely));
        assert_eq!(style(&["1234,567"], None), (DecimalComma, Confidence::Likely));
        // Truly ambiguous: not guessed, unless the delimiter is a semicolon.
        assert_eq!(style(&["12,500", "3,400"], None), (Point, Confidence::Unknown));
        assert_eq!(style(&["12,500", "3,400"], Some(',')), (Point, Confidence::Unknown));
        assert_eq!(style(&["12,500", "3,400"], Some(';')), (DecimalComma, Confidence::Likely));
        // Unmistakable groups beat the semicolon.
        assert_eq!(style(&["12,500", "1,250,000"], Some(';')), (Thousands, Confidence::Likely));
        // Contradicting cells: not guessed.
        assert_eq!(style(&["1,250,000", "12,5"], None).1, Confidence::Unknown);
        assert_eq!(parse_number_with("12,500", Thousands), Some(12_500.0));
        assert_eq!(parse_number_with("-1,250,000.25", Thousands), Some(-1_250_000.25));
        assert_eq!(parse_number_with("12,5", Thousands), None);
        assert_eq!(parse_number_with("12,500", DecimalComma), Some(12.5));
        assert_eq!(parse_number_with("1,250,000", DecimalComma), None);
        assert_eq!(parse_number_with("12,500", Point), None);
        assert_eq!(parse_number_with("7", Point), Some(7.0));
    }

    #[test]
    fn csv_coordinates_with_semicolons_and_ambiguous_decimal_commas() {
        // "121,473" alone could be thousands; in a `;` file it is a decimal comma.
        let headers = s(&["lon", "lat"]);
        let r: Vec<Vec<String>> = (0..10).map(|i| s(&[&format!("121,4{i}3"), &format!("31,2{i}0")])).collect();
        assert!(matches!(csv_locate(&headers, &r, "f.csv"), CsvLocation::Table { .. }));
        let CsvLocation::Located { pair, .. } = csv_locate_delimited(&headers, &r, "f.csv", Some(';')) else { panic!() };
        assert_eq!(pair.extent.map(|e| e.min_x), Some(121.403));
    }
}
