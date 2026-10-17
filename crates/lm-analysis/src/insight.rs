//! Attribute-only insight: where an object stands among its peers (spec › Insight cards and
//! suggested actions › Insight rows), plus the number formatting every insight string shares.
//!
//! Everything here uses values the layer already has: no question, no computation the user waits
//! for. Each measure field gets a [`RankIndex`] (its finite values, sorted) in the background at
//! import; a rank is then two binary searches, so the card appears instantly on click
//! (spec › Insight rows › Speed).
//!
//! `validation/gate-3/check_insight.py` re-implements the phrases below in pandas so the owner can
//! check the app's numbers in Gate 3 step 2. **Any change to a rule here must be made there too**;
//! `validation/gate-3/insight_golden.json` holds cases both implementations must pass.
//!
//! # Rules (decisions the spec leaves open)
//!
//! **Missing values.** `None`, NaN and ±∞ are missing. They are left out of `n` and counted in
//! [`RankIndex::missing`] (spec › Insight rows › Missing values).
//!
//! **Rank counts.** For a value `x`: `higher` = values strictly above `x`, `lower` = strictly
//! below, `equal` = the rest (`x` itself is normally one of them). Ranks are competition ranks:
//! position from the top is `higher + 1`, from the bottom `lower + 1`.
//!
//! **Upper half** means `higher <= lower` (the object is at least as close to the top as to the
//! bottom; the exact middle counts as upper). Upper half: "3rd highest of 17"; lower half:
//! "4th lowest of 17". Position 1 drops the ordinal: "highest of 17", "lowest of 17". When two or
//! more values equal `x` the phrase starts with "tied ": "tied 3rd highest of 17".
//!
//! **Small and large layers.** `n <= 2`: no rank phrase and no median line. `n >= 1,000`: a
//! percentile instead. The percentage is an integer, rounded down, so it is never overstated:
//! upper half "higher than P% of 1,248 schools" with `P = ⌊100·lower/n⌋`, lower half
//! "lower than Q% of 1,248 schools" with `Q = ⌊100·higher/n⌋`. Because the object itself is never
//! strictly below itself, `P <= 99`: no "higher than 100%". The extremes read "highest of 1,248
//! schools" / "lowest of …" (with "tied " when shared). If heavy ties make the percentage 0, the
//! phrase becomes "tied with E% of 1,248 schools" (`E = ⌊100·equal/n⌋`).
//!
//! **Denominator.** Both conditions are stated when both apply, filtered first:
//! "of 12", "of 12 filtered", "of 16 with values", "of 12 filtered with values"; in percentile
//! form "of 1,248 schools", "of 1,248 filtered schools", "of 1,248 schools with values",
//! "of 1,248 filtered schools with values". "with values" appears when the index (over the
//! filtered records, if filtered) saw at least one missing value.
//!
//! **Median.** Over the `n` finite values; for even `n` the mean of the middle two.
//!
//! **Numbers** ([`format_number`]): thousands separators ("1,248"), a true minus sign (U+2212),
//! and these decimals:
//! - A field's decimals `fd` are inferred at index time ([`RankIndex::decimals`]): the most
//!   decimals any value needs in its shortest round-trip form, capped at 6.
//! - A value shows `min(fd, max(2, s))` decimals, where `s` is the number of decimals that give
//!   3 significant digits (`|v| >= 100` → 0, `>= 10` → 1, `>= 1` → 2, `0.1..1` → 3, `0.01..0.1`
//!   → 4, …). So values keep the field's own precision up to 2 decimals, and small values keep up
//!   to 3 significant digits, never more decimals than the field has.
//! - Medians and differences (which can need a decimal more, e.g. the mean of 1,248 and 1,253)
//!   use the fewest decimals `d` in `fd..=max(fd, 2)` that show them exactly (to 1e-9 relative),
//!   else `max(fd, 2)`, then the value rule with `d` in place of `fd`.
//! - Without field decimals (`None`) a number shows 3 significant digits with trailing zeros
//!   trimmed.
//! - Rounding is the exact binary value rounded half to even, which Rust's `{:.N}` and Python's
//!   `f"{x:.Nf}"` both do.
//!
//! **Median line.** "at the median" when `|v − median| <= 1e-9·max(1, |median|)`; otherwise
//! "+2.1 °C above the median" / "−0.4 °C below the median" (the unit when there is one). If the
//! difference rounds to zero at the chosen decimals it is shown with 3 significant digits instead.
//!
//! **Units** follow the number after a space ("31.2 °C", "4.3 km²"), except "%", "‰", a bare "°",
//! "′" and "″", which attach directly ("45%", "12°").

use std::collections::BTreeMap;

/// The true minus sign used in every insight string.
pub const MINUS: char = '\u{2212}';
/// Rank phrases are omitted at or below this many values (spec: "Two or fewer objects: no rank").
pub const MIN_RANKED: usize = 3;
/// From this many values on, a percentile replaces the ordinal rank.
pub const PERCENTILE_FROM: usize = 1_000;
/// The dot strip's width in pixels; also the default number of ticks (spec › Insight rows).
pub const DOT_STRIP_PX: usize = 120;
/// Bins in a field summary histogram (spec › Inspector: layer overview).
pub const HISTOGRAM_BINS: usize = 20;
/// The most decimals inferred for a field.
pub const MAX_FIELD_DECIMALS: u8 = 6;
/// Shown for a missing value (spec › Insight rows › Missing values).
pub const NO_VALUE: &str = "No value";
/// The words insight text must never use (spec › Insight rows › Forbidden words in insight).
pub const FORBIDDEN_WORDS: [&str; 6] = ["significant", "cause", "because", "explains", "better", "worse"];

/// Relative tolerance for "exact" decimals and "at the median".
const EXACT_REL: f64 = 1e-9;

// ---------------------------------------------------------------------------------------------
// Number formatting
// ---------------------------------------------------------------------------------------------

/// Decimals giving 3 significant digits for a magnitude `a >= 0` (see the module notes).
/// Uses comparisons with literal powers of ten, not `log10`, so Python gets the same answer.
fn sig3_decimals(a: f64) -> u8 {
    if a.is_nan() || a <= 0.0 || a.is_infinite() {
        return 0;
    }
    if a >= 100.0 {
        return 0;
    }
    if a >= 10.0 {
        return 1;
    }
    if a >= 1.0 {
        return 2;
    }
    const P: [f64; 10] = [1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10];
    for (k, p) in P.iter().enumerate() {
        if a >= *p {
            return 3 + k as u8;
        }
    }
    13
}

/// Inserts thousands separators into the integer part of a plain decimal string ("1248.5").
fn group_thousands(s: &str) -> String {
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s, None),
    };
    let mut out = String::with_capacity(s.len() + int.len() / 3);
    let len = int.len();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

fn trim_zeros(s: String) -> String {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

fn has_nonzero_digit(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit() && c != '0')
}

/// Formats a number for insight text (see the module notes, **Numbers**). `field_decimals` is
/// the field's inferred decimals ([`RankIndex::decimals`]); `None` means 3 significant digits,
/// trailing zeros trimmed. Non-finite input gives "No value".
///
/// `format_number(1248.0, Some(0))` → "1,248"; `format_number(31.25, Some(2))` → "31.25";
/// `format_number(-0.4, Some(1))` → "−0.4"; `format_number(0.012345, None)` → "0.0123".
pub fn format_number(v: f64, field_decimals: Option<u8>) -> String {
    if !v.is_finite() {
        return NO_VALUE.to_string();
    }
    let a = v.abs();
    let sig = sig3_decimals(a);
    let (d, trim) = match field_decimals {
        Some(fd) => (fd.min(sig.max(2)), false),
        None => (sig, true),
    };
    let mut s = format!("{:.*}", d as usize, a);
    if trim {
        s = trim_zeros(s);
    }
    let body = group_thousands(&s);
    if v < 0.0 && has_nonzero_digit(&s) { format!("{MINUS}{body}") } else { body }
}

/// A count with thousands separators: "1,248".
pub fn format_count(n: usize) -> String {
    group_thousands(&n.to_string())
}

/// English ordinal with thousands separators: 1st, 2nd, 3rd, 4th, 11th, 12th, 13th, 21st,
/// 101st, 111th, "1,002nd".
pub fn ordinal(n: usize) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{}{suffix}", format_count(n))
}

/// Appends a unit to a formatted number: a space before it, except for "%", "‰", a bare "°",
/// "′" and "″". An empty or missing unit leaves the number alone.
pub fn with_unit(number: &str, unit: Option<&str>) -> String {
    match unit.map(str::trim) {
        None | Some("") => number.to_string(),
        Some(u) if u.starts_with('%') || u.starts_with('‰') || u == "°" || u == "′" || u == "″" => format!("{number}{u}"),
        Some(u) => format!("{number} {u}"),
    }
}

/// A value with its unit, using the field's decimals: "31.2 °C". Missing → "No value".
pub fn format_value(v: Option<f64>, field_decimals: Option<u8>, unit: Option<&str>) -> String {
    match v {
        Some(x) if x.is_finite() => with_unit(&format_number(x, field_decimals), unit),
        _ => NO_VALUE.to_string(),
    }
}

/// Decimals in the shortest round-trip form of `v` (0 for whole numbers), capped at
/// [`MAX_FIELD_DECIMALS`]. Python: `-Decimal(repr(v)).as_tuple().exponent`.
pub fn value_decimals(v: f64) -> u8 {
    if !v.is_finite() || v.fract() == 0.0 {
        return 0;
    }
    let s = format!("{}", v.abs());
    let d = s.split_once('.').map_or(0, |(_, f)| f.len());
    d.min(MAX_FIELD_DECIMALS as usize) as u8
}

/// Decimals for a derived number (median, difference): the fewest in `fd..=max(fd, 2)` that show
/// it exactly, else `max(fd, 2)`.
pub fn derived_decimals(x: f64, fd: u8) -> u8 {
    let hi = fd.max(2);
    for d in fd..=hi {
        let r: f64 = format!("{:.*}", d as usize, x).parse().unwrap_or(f64::NAN);
        if (r - x).abs() <= EXACT_REL * x.abs().max(1.0) {
            return d;
        }
    }
    hi
}

/// A median or other derived number formatted with [`derived_decimals`].
pub fn format_derived(x: f64, fd: u8) -> String {
    format_number(x, Some(derived_decimals(x, fd)))
}

// ---------------------------------------------------------------------------------------------
// Rank index
// ---------------------------------------------------------------------------------------------

/// A measure field's finite values, sorted, for O(log n) ranks (spec › Insight rows › Speed).
///
/// Build it on a background thread at import: sorting 1,000,000 values takes tens of
/// milliseconds. It is `Send + Sync` and immutable once built.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RankIndex {
    sorted: Vec<f64>,
    missing: usize,
    filtered: bool,
    decimals: u8,
}

/// Where one value stands in a [`RankIndex`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rank {
    /// Values strictly above.
    pub higher: usize,
    /// Values strictly below.
    pub lower: usize,
    /// Values equal (normally including the object itself).
    pub equal: usize,
    /// Values present (missing ones excluded).
    pub n: usize,
    /// Missing values among the records the index covers.
    pub missing: usize,
    /// The index covers only the filtered records.
    pub filtered: bool,
}

impl Rank {
    /// True in the upper half: `higher <= lower`.
    pub fn is_upper_half(&self) -> bool {
        self.higher <= self.lower
    }
}

impl RankIndex {
    /// Builds the index over all of a layer's records. `None`, NaN and ±∞ count as missing.
    pub fn new(values: impl IntoIterator<Item = Option<f64>>) -> Self {
        let iter = values.into_iter();
        let mut sorted = Vec::with_capacity(iter.size_hint().0);
        let mut missing = 0usize;
        let mut decimals = 0u8;
        for v in iter {
            match v {
                Some(x) if x.is_finite() => {
                    // -0.0 and 0.0 are the same value.
                    let x = if x == 0.0 { 0.0 } else { x };
                    if decimals < MAX_FIELD_DECIMALS && x.fract() != 0.0 {
                        decimals = decimals.max(value_decimals(x));
                    }
                    sorted.push(x);
                }
                _ => missing += 1,
            }
        }
        sorted.sort_unstable_by(f64::total_cmp);
        RankIndex { sorted, missing, filtered: false, decimals }
    }

    /// Marks the index as built over a filter's records, so phrases say "of 12 filtered".
    pub fn with_filtered(mut self, filtered: bool) -> Self {
        self.filtered = filtered;
        self
    }

    /// Values present (`n`).
    pub fn len(&self) -> usize {
        self.sorted.len()
    }

    /// No values present.
    pub fn is_empty(&self) -> bool {
        self.sorted.is_empty()
    }

    /// Missing values seen while building.
    pub fn missing(&self) -> usize {
        self.missing
    }

    /// Built over filtered records.
    pub fn is_filtered(&self) -> bool {
        self.filtered
    }

    /// The field's decimals (see the module notes, **Numbers**).
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// The finite values, ascending.
    pub fn sorted(&self) -> &[f64] {
        &self.sorted
    }

    /// Smallest and largest value.
    pub fn range(&self) -> Option<(f64, f64)> {
        Some((*self.sorted.first()?, *self.sorted.last()?))
    }

    /// Where `x` stands. `None` when `x` is not finite or the index is empty.
    pub fn rank(&self, x: f64) -> Option<Rank> {
        if !x.is_finite() || self.sorted.is_empty() {
            return None;
        }
        let n = self.sorted.len();
        let lower = self.sorted.partition_point(|v| *v < x);
        let not_higher = self.sorted.partition_point(|v| *v <= x);
        Some(Rank { higher: n - not_higher, lower, equal: not_higher - lower, n, missing: self.missing, filtered: self.filtered })
    }

    /// Fraction of values strictly below `x` (0..1).
    pub fn percentile(&self, x: f64) -> Option<f64> {
        let r = self.rank(x)?;
        Some(r.lower as f64 / r.n as f64)
    }

    /// Median; for even `n` the mean of the middle two.
    pub fn median(&self) -> Option<f64> {
        let n = self.sorted.len();
        if n == 0 {
            return None;
        }
        let hi = *self.sorted.get(n / 2)?;
        if n % 2 == 1 { Some(hi) } else { Some((*self.sorted.get(n / 2 - 1)? + hi) / 2.0) }
    }

    /// Positions for the dot strip, in 0..=1 across the value range (spec › Insight rows ›
    /// Measure row). At most `max_ticks` ticks: every value when `n <= max_ticks`, otherwise the
    /// values at ranks `round(i·(n−1)/(max_ticks−1))`, so the ticks follow the distribution.
    /// Repeated positions are dropped. When all values are equal every position is 0.5.
    pub fn dot_strip(&self, value: Option<f64>, max_ticks: usize) -> Option<DotStrip> {
        let (lo, hi) = self.range()?;
        let span = hi - lo;
        let pos = |v: f64| -> f32 { if span > 0.0 { ((v - lo) / span).clamp(0.0, 1.0) as f32 } else { 0.5 } };
        let n = self.sorted.len();
        let mut ticks: Vec<f32> = if n <= max_ticks {
            self.sorted.iter().map(|v| pos(*v)).collect()
        } else if max_ticks == 1 {
            vec![pos(self.median()?)]
        } else {
            (0..max_ticks)
                .filter_map(|i| {
                    let k = ((i as f64) * ((n - 1) as f64) / ((max_ticks - 1) as f64)).round() as usize;
                    self.sorted.get(k).map(|v| pos(*v))
                })
                .collect()
        };
        ticks.dedup();
        Some(DotStrip { value: value.filter(|v| v.is_finite()).map(pos), median: pos(self.median()?), ticks })
    }

    /// A histogram over the value range with `bins` equal-width bins (spec › Inspector: layer
    /// overview › Field summary uses [`HISTOGRAM_BINS`]). Bin `i` holds `[edge_i, edge_i+1)`; the
    /// last bin also holds the maximum. When all values are equal they go in the middle bin.
    pub fn histogram(&self, bins: usize) -> Option<Histogram> {
        let (lo, hi) = self.range()?;
        let bins = bins.max(1);
        let n = self.sorted.len();
        let mut counts = vec![0usize; bins];
        if hi > lo {
            let w = hi - lo;
            let mut start = 0usize;
            for (i, c) in counts.iter_mut().enumerate() {
                let end = if i + 1 == bins {
                    n
                } else {
                    let edge = lo + w * ((i + 1) as f64) / (bins as f64);
                    self.sorted.partition_point(|v| *v < edge)
                };
                *c = end.saturating_sub(start);
                start = end.max(start);
            }
        } else if let Some(c) = counts.get_mut(bins / 2) {
            *c = n;
        }
        Some(Histogram { min: lo, max: hi, counts })
    }
}

/// Dot strip geometry, positions in 0..=1 (left = smallest value).
#[derive(Clone, Debug, PartialEq)]
pub struct DotStrip {
    /// The object's value (the accent dot); `None` when it has no value.
    pub value: Option<f32>,
    /// The median (a short line).
    pub median: f32,
    /// Faint ticks, ascending, without repeats.
    pub ticks: Vec<f32>,
}

/// Equal-width histogram counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub min: f64,
    pub max: f64,
    pub counts: Vec<usize>,
}

// ---------------------------------------------------------------------------------------------
// Phrases
// ---------------------------------------------------------------------------------------------

/// "of 12", "of 12 filtered", "of 16 with values", "of 12 filtered with values".
fn denominator(r: &Rank) -> String {
    let mut s = format!("of {}", format_count(r.n));
    if r.filtered {
        s.push_str(" filtered");
    }
    if r.missing > 0 {
        s.push_str(" with values");
    }
    s
}

/// "of 1,248 schools", "of 1,248 filtered schools", "of 1,248 schools with values".
fn denominator_noun(r: &Rank, noun_plural: &str) -> String {
    let mut s = format!("of {}", format_count(r.n));
    if r.filtered {
        s.push_str(" filtered");
    }
    let noun = noun_plural.trim();
    if !noun.is_empty() {
        s.push(' ');
        s.push_str(noun);
    }
    if r.missing > 0 {
        s.push_str(" with values");
    }
    s
}

/// The percentile part shared by layers of 1,000+ and raster cells: "higher than 82% ",
/// "lower than 40% " or "tied with 98% ", followed by `of`.
fn percent_phrase(r: &Rank, of: &str) -> String {
    let n = r.n.max(1);
    let (word, pct) = if r.is_upper_half() { ("higher than", 100 * r.lower / n) } else { ("lower than", 100 * r.higher / n) };
    if pct == 0 { format!("tied with {}% {of}", 100 * r.equal / n) } else { format!("{word} {pct}% {of}") }
}

/// The rank phrase (see the module notes): "3rd highest of 17", "tied 4th lowest of 16 with
/// values", "higher than 82% of 1,248 schools". `None` for two or fewer values.
/// `noun_plural` names the layer's objects ("schools") and is used only from 1,000 values on.
pub fn rank_phrase(r: &Rank, noun_plural: &str) -> Option<String> {
    if r.n < MIN_RANKED {
        return None;
    }
    let tied = if r.equal >= 2 { "tied " } else { "" };
    if r.n >= PERCENTILE_FROM {
        let of = denominator_noun(r, noun_plural);
        if r.higher == 0 {
            return Some(format!("{tied}highest {of}"));
        }
        if r.lower == 0 {
            return Some(format!("{tied}lowest {of}"));
        }
        return Some(percent_phrase(r, &of));
    }
    let (pos, word) = if r.is_upper_half() { (r.higher + 1, "highest") } else { (r.lower + 1, "lowest") };
    let ord = if pos == 1 { String::new() } else { format!("{} ", ordinal(pos)) };
    Some(format!("{tied}{ord}{word} {}", denominator(r)))
}

/// The median comparison: "+2.1 °C above the median", "−0.4 below the median", "at the median".
/// `fd` is the field's decimals.
pub fn median_line(value: f64, median: f64, fd: u8, unit: Option<&str>) -> Option<String> {
    if !value.is_finite() || !median.is_finite() {
        return None;
    }
    let diff = value - median;
    if diff.abs() <= EXACT_REL * median.abs().max(1.0) {
        return Some("at the median".to_string());
    }
    let mut s = format_derived(diff.abs(), fd);
    if !has_nonzero_digit(&s) {
        s = format_number(diff.abs(), None);
    }
    let (sign, word) = if diff > 0.0 { ('+', "above") } else { (MINUS, "below") };
    Some(format!("{sign}{} {word} the median", with_unit(&s, unit)))
}

/// One measure row of the insight strip (spec › Insight rows › Measure row).
#[derive(Clone, Debug, PartialEq)]
pub struct MeasureRow {
    /// The value with its unit, or "No value".
    pub value: String,
    /// "3rd highest of 17"; `None` with no value or two or fewer values.
    pub rank: Option<String>,
    /// "+2.1 °C above the median"; `None` when `rank` would be.
    pub median: Option<String>,
    /// The 120 px dot strip; `None` when the field has no values at all.
    pub strip: Option<DotStrip>,
}

/// Builds a measure row for one object. `noun_plural` names the layer's objects ("schools").
pub fn measure_row(index: &RankIndex, value: Option<f64>, unit: Option<&str>, noun_plural: &str) -> MeasureRow {
    let fd = index.decimals();
    let v = value.filter(|x| x.is_finite());
    let rank = v.and_then(|x| index.rank(x));
    let ranked = rank.is_some_and(|r| r.n >= MIN_RANKED);
    MeasureRow {
        value: format_value(v, Some(fd), unit),
        rank: rank.and_then(|r| rank_phrase(&r, noun_plural)),
        median: if ranked { v.zip(index.median()).and_then(|(x, m)| median_line(x, m, fd, unit)) } else { None },
        strip: index.dot_strip(v, DOT_STRIP_PX),
    }
}

/// A raster pixel's comparison (spec › Insight rows › Raster pixel): "higher than 82% of this
/// layer's cells", with " (estimated from a sample)" while `estimated`. `rank` comes from a
/// [`RankIndex`] over the cell values or a sample of them. `None` when there are no values.
pub fn raster_phrase(r: &Rank, estimated: bool) -> Option<String> {
    if r.n == 0 {
        return None;
    }
    let mut s = percent_phrase(r, "of this layer's cells");
    if estimated {
        s.push_str(" (estimated from a sample)");
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------------------------

/// Counts of a category field's values (spec › Insight rows › Category row). Missing and blank
/// (whitespace-only) values are left out of `n` and counted as missing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryCounts {
    counts: BTreeMap<String, usize>,
    n: usize,
    missing: usize,
    filtered: bool,
}

impl CategoryCounts {
    /// Counts the values of a layer's records.
    pub fn new<S: AsRef<str>>(values: impl IntoIterator<Item = Option<S>>) -> Self {
        let mut c = CategoryCounts::default();
        for v in values {
            match v {
                Some(s) if !s.as_ref().trim().is_empty() => {
                    *c.counts.entry(s.as_ref().to_string()).or_insert(0) += 1;
                    c.n += 1;
                }
                _ => c.missing += 1,
            }
        }
        c
    }

    /// Marks the counts as covering a filter's records.
    pub fn with_filtered(mut self, filtered: bool) -> Self {
        self.filtered = filtered;
        self
    }

    /// Records with a value.
    pub fn len(&self) -> usize {
        self.n
    }

    /// No records with a value.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Missing or blank records.
    pub fn missing(&self) -> usize {
        self.missing
    }

    /// How many records have this category.
    pub fn count(&self, category: &str) -> usize {
        self.counts.get(category).copied().unwrap_or(0)
    }

    /// Categories with their counts, most common first (ties alphabetical).
    pub fn ranked(&self) -> Vec<(&str, usize)> {
        let mut v: Vec<(&str, usize)> = self.counts.iter().map(|(k, c)| (k.as_str(), *c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        v
    }

    /// "Residential · 41% of districts are Residential". The share is rounded down;
    /// a nonzero share under 1% reads "fewer than 1% of". Missing values: "of districts with
    /// values"; filtered: "of filtered districts". With two or fewer values only the category is
    /// shown; a missing category gives "No value".
    pub fn category_row(&self, category: Option<&str>, noun_plural: &str) -> String {
        let Some(cat) = category.filter(|c| !c.trim().is_empty()) else { return NO_VALUE.to_string() };
        if self.n < MIN_RANKED {
            return cat.to_string();
        }
        let count = self.count(cat);
        let pct = 100 * count / self.n;
        let share = if pct == 0 && count > 0 { "fewer than 1%".to_string() } else { format!("{pct}%") };
        let mut who = String::new();
        if self.filtered {
            who.push_str("filtered ");
        }
        let noun = noun_plural.trim();
        who.push_str(if noun.is_empty() { "objects" } else { noun });
        if self.missing > 0 {
            who.push_str(" with values");
        }
        format!("{cat} · {share} of {who} are {cat}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    fn idx(v: &[f64]) -> RankIndex {
        RankIndex::new(v.iter().map(|x| Some(*x)))
    }

    fn phrase(values: &[f64], x: f64) -> Option<String> {
        rank_phrase(&idx(values).rank(x).unwrap(), "schools")
    }

    #[test]
    fn index_is_send_sync() {
        fn ok<T: Send + Sync>() {}
        ok::<RankIndex>();
        ok::<CategoryCounts>();
    }

    #[test]
    fn ordinals() {
        let want = [
            (1, "1st"),
            (2, "2nd"),
            (3, "3rd"),
            (4, "4th"),
            (11, "11th"),
            (12, "12th"),
            (13, "13th"),
            (21, "21st"),
            (22, "22nd"),
            (23, "23rd"),
            (101, "101st"),
            (111, "111th"),
            (112, "112th"),
            (1002, "1,002nd"),
        ];
        for (n, s) in want {
            assert_eq!(ordinal(n), s);
        }
    }

    #[test]
    fn numbers() {
        assert_eq!(format_number(1248.0, Some(0)), "1,248");
        assert_eq!(format_number(1_234_567.891, Some(3)), "1,234,567.89");
        assert_eq!(format_number(31.2, Some(1)), "31.2");
        assert_eq!(format_number(31.0, Some(1)), "31.0");
        assert_eq!(format_number(-0.4, Some(1)), "\u{2212}0.4");
        assert_eq!(format_number(-0.04, Some(1)), "0.0");
        assert_eq!(format_number(0.0012, Some(4)), "0.0012");
        assert_eq!(format_number(0.123456, Some(6)), "0.123");
        assert_eq!(format_number(31.23456, Some(6)), "31.23");
        assert_eq!(format_number(0.012345, None), "0.0123");
        assert_eq!(format_number(2.10, None), "2.1");
        assert_eq!(format_number(1248.4, None), "1,248");
        assert_eq!(format_number(100.0, None), "100");
        assert_eq!(format_number(0.0, None), "0");
        assert_eq!(format_number(f64::NAN, None), NO_VALUE);
        // Exact binary ties round half to even, as in Python.
        assert_eq!(format_number(0.125, Some(2)), "0.12");
        assert_eq!(format_number(2.5, Some(0)), "2");
        assert_eq!(format_number(3.5, Some(0)), "4");
        assert_eq!(format_number(31.25, Some(1)), "31.2");
        assert_eq!(format_count(1_000_000), "1,000,000");
        assert_eq!(format_count(999), "999");
        assert_eq!(with_unit("31.2", Some("°C")), "31.2 °C");
        assert_eq!(with_unit("45", Some("%")), "45%");
        assert_eq!(with_unit("12", Some("°")), "12°");
        assert_eq!(with_unit("12", Some("")), "12");
    }

    #[test]
    fn decimals_inferred() {
        assert_eq!(idx(&[1.0, 2.0, 3.0]).decimals(), 0);
        assert_eq!(idx(&[1.0, 2.5, 3.25]).decimals(), 2);
        assert_eq!(idx(&[0.1 + 0.2]).decimals(), MAX_FIELD_DECIMALS);
        assert_eq!(value_decimals(1e-7), MAX_FIELD_DECIMALS);
        assert_eq!(value_decimals(31.2), 1);
        assert_eq!(derived_decimals(1250.5, 0), 1);
        assert_eq!(derived_decimals(2.1999999999999993, 1), 1);
        assert_eq!(derived_decimals(1.0 / 3.0, 0), 2);
    }

    #[test]
    fn spec_rank_phrases() {
        let v: Vec<f64> = (1..=17).map(f64::from).collect();
        assert_eq!(phrase(&v, 15.0).as_deref(), Some("3rd highest of 17"));
        assert_eq!(phrase(&v, 4.0).as_deref(), Some("4th lowest of 17"));
        assert_eq!(phrase(&v, 17.0).as_deref(), Some("highest of 17"));
        assert_eq!(phrase(&v, 1.0).as_deref(), Some("lowest of 17"));
        // The middle (8 above, 8 below) counts as upper half.
        assert_eq!(phrase(&v, 9.0).as_deref(), Some("9th highest of 17"));
        // Ties.
        let t = [1.0, 2.0, 3.0, 5.0, 5.0, 9.0, 10.0];
        assert_eq!(phrase(&t, 5.0).as_deref(), Some("tied 3rd highest of 7"));
        let t = [1.0, 1.0, 3.0, 5.0, 6.0];
        assert_eq!(phrase(&t, 1.0).as_deref(), Some("tied lowest of 5"));
        // Two or fewer: none.
        assert_eq!(phrase(&[1.0, 2.0], 2.0), None);
    }

    #[test]
    fn denominators() {
        let mut vals: Vec<Option<f64>> = (1..=16).map(|i| Some(f64::from(i))).collect();
        vals.push(None);
        let i = RankIndex::new(vals.clone());
        assert_eq!(rank_phrase(&i.rank(14.0).unwrap(), "").as_deref(), Some("3rd highest of 16 with values"));
        let f = RankIndex::new((1..=12).map(|i| Some(f64::from(i)))).with_filtered(true);
        assert_eq!(rank_phrase(&f.rank(1.0).unwrap(), "").as_deref(), Some("lowest of 12 filtered"));
        let fm = RankIndex::new(vals).with_filtered(true);
        assert_eq!(rank_phrase(&fm.rank(4.0).unwrap(), "").as_deref(), Some("4th lowest of 16 filtered with values"));
    }

    #[test]
    fn percentiles() {
        let v: Vec<f64> = (0..1248).map(f64::from).collect();
        // 1,024 strictly lower → ⌊102400/1248⌋ = 82.
        assert_eq!(phrase(&v, 1024.0).as_deref(), Some("higher than 82% of 1,248 schools"));
        assert_eq!(phrase(&v, 100.0).as_deref(), Some("lower than 91% of 1,248 schools"));
        assert_eq!(phrase(&v, 1247.0).as_deref(), Some("highest of 1,248 schools"));
        assert_eq!(phrase(&v, 1246.0).as_deref(), Some("higher than 99% of 1,248 schools"));
        assert_eq!(phrase(&v, 0.0).as_deref(), Some("lowest of 1,248 schools"));
        // Ties at the top never claim 100%.
        let mut t = vec![5.0; 10];
        t.extend((0..1238).map(f64::from).map(|x| x / 1000.0));
        assert_eq!(phrase(&t, 5.0).as_deref(), Some("tied highest of 1,248 schools"));
        // Heavy ties: "tied with".
        let mut h = vec![1.0; 990];
        h.extend([0.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
        assert_eq!(phrase(&h, 1.0).as_deref(), Some("tied with 99% of 1,000 schools"));
        // With missing values and filtered.
        let mut m: Vec<Option<f64>> = v.iter().map(|x| Some(*x)).collect();
        m.push(Some(f64::NAN));
        let i = RankIndex::new(m).with_filtered(true);
        assert_eq!(
            rank_phrase(&i.rank(1024.0).unwrap(), "schools").as_deref(),
            Some("higher than 82% of 1,248 filtered schools with values")
        );
    }

    #[test]
    fn median_lines() {
        assert_eq!(idx(&[3.0, 1.0, 2.0]).median(), Some(2.0));
        assert_eq!(idx(&[4.0, 1.0, 2.0, 3.0]).median(), Some(2.5));
        assert_eq!(median_line(31.2, 29.1, 1, Some("°C")).as_deref(), Some("+2.1 °C above the median"));
        assert_eq!(median_line(28.7, 29.1, 1, None).as_deref(), Some("\u{2212}0.4 below the median"));
        assert_eq!(median_line(29.1, 29.1, 1, Some("°C")).as_deref(), Some("at the median"));
        assert_eq!(median_line(1248.0, 1250.5, 0, None).as_deref(), Some("\u{2212}2.5 below the median"));
        // Rounds to zero at the field's decimals → 3 significant digits.
        assert_eq!(median_line(10.0, 10.001, 0, None).as_deref(), Some("\u{2212}0.001 below the median"));
        assert_eq!(median_line(10.0, 9.99, 1, Some("%")).as_deref(), Some("+0.01% above the median"));
    }

    #[test]
    fn measure_rows() {
        let vals = [Some(28.7), Some(31.2), None, Some(29.1), Some(30.0), Some(27.5)];
        let i = RankIndex::new(vals);
        let r = measure_row(&i, Some(31.2), Some("°C"), "districts");
        assert_eq!(r.value, "31.2 °C");
        assert_eq!(r.rank.as_deref(), Some("highest of 5 with values"));
        assert_eq!(r.median.as_deref(), Some("+2.1 °C above the median"));
        let s = r.strip.unwrap();
        assert_eq!(s.value, Some(1.0));
        assert_eq!(s.ticks.len(), 5);
        let none = measure_row(&i, None, Some("°C"), "districts");
        assert_eq!(none.value, NO_VALUE);
        assert_eq!((none.rank, none.median), (None, None));
        assert_eq!(none.strip.unwrap().value, None);
        let two = RankIndex::new([Some(1.0), Some(2.0)]);
        let r = measure_row(&two, Some(2.0), None, "x");
        assert_eq!((r.value.as_str(), r.rank, r.median), ("2", None, None));
    }

    #[test]
    fn dot_strip_subsamples() {
        let v: Vec<f64> = (0..10_000).map(f64::from).collect();
        let s = idx(&v).dot_strip(Some(2500.0), DOT_STRIP_PX).unwrap();
        assert_eq!(s.ticks.len(), DOT_STRIP_PX);
        assert_eq!(s.ticks.first(), Some(&0.0));
        assert_eq!(s.ticks.last(), Some(&1.0));
        assert!((s.value.unwrap() - 0.25).abs() < 1e-3);
        assert!((s.median - 0.5).abs() < 1e-3);
        assert!(s.ticks.windows(2).all(|w| w[0] < w[1]));
        let flat = idx(&[3.0, 3.0, 3.0]).dot_strip(Some(3.0), 120).unwrap();
        assert_eq!((flat.value, flat.median, flat.ticks), (Some(0.5), 0.5, vec![0.5]));
        assert_eq!(RankIndex::default().dot_strip(None, 120), None);
    }

    #[test]
    fn histograms() {
        let v: Vec<f64> = (0..=100).map(f64::from).collect();
        let h = idx(&v).histogram(HISTOGRAM_BINS).unwrap();
        assert_eq!(h.counts.len(), 20);
        assert_eq!(h.counts.iter().sum::<usize>(), 101);
        assert_eq!(h.counts[0], 5);
        assert_eq!(h.counts[19], 6); // 95..=100
        let flat = idx(&[2.0, 2.0]).histogram(20).unwrap();
        assert_eq!(flat.counts[10], 2);
        // Naive check on uneven data.
        let d = [0.1, 0.1, 0.35, 0.9, 1.7, 2.0, 2.0, 5.5, 9.99, 10.0];
        let h = idx(&d).histogram(20).unwrap();
        let w = (10.0 - 0.1) / 20.0;
        let mut naive = [0usize; 20];
        for x in d {
            let b = (((x - 0.1) / w).floor() as usize).min(19);
            naive[b] += 1;
        }
        assert_eq!(h.counts, naive);
    }

    #[test]
    fn raster() {
        let v: Vec<f64> = (0..100).map(f64::from).collect();
        let i = idx(&v);
        assert_eq!(raster_phrase(&i.rank(82.0).unwrap(), false).as_deref(), Some("higher than 82% of this layer's cells"));
        assert_eq!(
            raster_phrase(&i.rank(82.0).unwrap(), true).as_deref(),
            Some("higher than 82% of this layer's cells (estimated from a sample)")
        );
        assert_eq!(raster_phrase(&i.rank(10.0).unwrap(), false).as_deref(), Some("lower than 89% of this layer's cells"));
        assert_eq!(raster_phrase(&idx(&[1.0; 5]).rank(1.0).unwrap(), false).as_deref(), Some("tied with 100% of this layer's cells"));
    }

    #[test]
    fn categories() {
        let mut vals: Vec<Option<&str>> = vec![Some("Residential"); 41];
        vals.extend(vec![Some("Industrial"); 59]);
        let c = CategoryCounts::new(vals.clone());
        assert_eq!(c.category_row(Some("Residential"), "districts"), "Residential · 41% of districts are Residential");
        vals.push(None);
        vals.push(Some("  "));
        let c = CategoryCounts::new(vals.clone()).with_filtered(true);
        assert_eq!(c.missing(), 2);
        assert_eq!(c.category_row(Some("Industrial"), "districts"), "Industrial · 59% of filtered districts with values are Industrial");
        assert_eq!(c.category_row(None, "districts"), NO_VALUE);
        assert_eq!(c.ranked()[0], ("Industrial", 59));
        let mut rare: Vec<Option<&str>> = vec![Some("A"); 300];
        rare.push(Some("B"));
        assert_eq!(CategoryCounts::new(rare).category_row(Some("B"), "parcels"), "B · fewer than 1% of parcels are B");
        assert_eq!(CategoryCounts::new([Some("A"), Some("B")]).category_row(Some("A"), "x"), "A");
    }

    /// Every phrase family, over many shapes of data, avoids the forbidden words.
    #[test]
    fn no_forbidden_words() {
        let mut out: Vec<String> = Vec::new();
        for n in [3usize, 5, 17, 999, 1000, 1248] {
            let mut v: Vec<Option<f64>> = (0..n).map(|i| Some((i % 7) as f64 * 1.5)).collect();
            v.push(None);
            for filtered in [false, true] {
                let i = RankIndex::new(v.clone()).with_filtered(filtered);
                for x in [0.0, 1.5, 4.5, 9.0] {
                    let r = measure_row(&i, Some(x), Some("°C"), "schools");
                    out.extend([Some(r.value), r.rank, r.median].into_iter().flatten());
                    if let Some(rk) = i.rank(x) {
                        out.extend(raster_phrase(&rk, true));
                    }
                }
            }
        }
        let c = CategoryCounts::new([Some("A"), Some("B"), Some("B"), None]);
        out.push(c.category_row(Some("A"), "districts"));
        out.push(format_value(None, None, None));
        assert!(out.len() > 100);
        for s in &out {
            let lower = s.to_lowercase();
            for w in FORBIDDEN_WORDS {
                assert!(!lower.contains(w), "{s:?} contains {w:?}");
            }
            assert!(!s.contains('!'), "{s:?}");
            assert!(!s.contains('-'), "{s:?} must use the true minus sign");
        }
    }

    #[test]
    fn million_values_build_quickly() {
        let v: Vec<Option<f64>> = (0..1_000_000u64).map(|i| Some(((i * 2_654_435_761) % 1_000_003) as f64 / 10.0)).collect();
        let t = std::time::Instant::now();
        let i = RankIndex::new(v);
        let built = t.elapsed();
        assert_eq!(i.len(), 1_000_000);
        assert_eq!(i.decimals(), 1);
        let r = i.rank(50_000.0).unwrap();
        assert_eq!(r.higher + r.lower + r.equal, 1_000_000);
        // Generous bound for unoptimised CI builds.
        assert!(built.as_secs_f64() < 5.0, "{built:?}");
    }

    // ----- Golden cases shared with validation/gate-3/check_insight.py ------------------------

    fn golden_path() -> PathBuf {
        [env!("CARGO_MANIFEST_DIR"), "..", "..", "validation", "gate-3", "insight_golden.json"].iter().collect()
    }

    fn script_path() -> PathBuf {
        [env!("CARGO_MANIFEST_DIR"), "..", "..", "validation", "gate-3", "check_insight.py"].iter().collect()
    }

    /// Golden values: numbers, null, or the strings "NaN" / "Infinity" / "-Infinity".
    fn golden_value(v: &serde_json::Value) -> Option<f64> {
        match v {
            serde_json::Value::Number(x) => x.as_f64(),
            serde_json::Value::String(s) if s == "NaN" => Some(f64::NAN),
            serde_json::Value::String(s) if s == "Infinity" => Some(f64::INFINITY),
            serde_json::Value::String(s) if s == "-Infinity" => Some(f64::NEG_INFINITY),
            _ => None,
        }
    }

    #[test]
    fn golden_cases() {
        let text = std::fs::read_to_string(golden_path()).unwrap();
        let g: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut checked = 0;
        for c in g["numbers"].as_array().unwrap() {
            let v = c["v"].as_f64().unwrap();
            let d = c.get("decimals").and_then(serde_json::Value::as_u64).map(|d| d as u8);
            assert_eq!(format_number(v, d), c["expect"].as_str().unwrap(), "{c}");
            checked += 1;
        }
        for c in g["rows"].as_array().unwrap() {
            let vals: Vec<Option<f64>> = c["values"].as_array().unwrap().iter().map(golden_value).collect();
            let i = RankIndex::new(vals).with_filtered(c.get("filtered").and_then(serde_json::Value::as_bool).unwrap_or(false));
            let x = c.get("value").and_then(golden_value);
            let unit = c.get("unit").and_then(serde_json::Value::as_str);
            let noun = c.get("noun").and_then(serde_json::Value::as_str).unwrap_or("objects");
            let row = measure_row(&i, x, unit, noun);
            let name = c["name"].as_str().unwrap_or("?");
            assert_eq!(row.value, c["expect_value"].as_str().unwrap(), "{name}");
            assert_eq!(row.rank.as_deref(), c["expect_rank"].as_str(), "{name}");
            assert_eq!(row.median.as_deref(), c["expect_median"].as_str(), "{name}");
            checked += 1;
        }
        for c in g["categories"].as_array().unwrap() {
            let vals: Vec<Option<&str>> = c["values"].as_array().unwrap().iter().map(serde_json::Value::as_str).collect();
            let cc = CategoryCounts::new(vals).with_filtered(c.get("filtered").and_then(serde_json::Value::as_bool).unwrap_or(false));
            let name = c["name"].as_str().unwrap_or("?");
            assert_eq!(cc.category_row(c["value"].as_str(), c["noun"].as_str().unwrap_or("")), c["expect"].as_str().unwrap(), "{name}");
            checked += 1;
        }
        assert!(checked >= 30, "{checked}");
    }

    /// A Python with pandas, or `None` (the cross-checks are then skipped).
    fn python() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = std::env::var_os("LM_PYTHON") {
            candidates.push(p.into());
        }
        if let Some(h) = std::env::var_os("HOME") {
            candidates.push(PathBuf::from(h).join("miniforge3/envs/layeredmap-ref/bin/python3"));
        }
        candidates.push("python3".into());
        candidates.into_iter().find(|p| {
            std::process::Command::new(p)
                .args(["-c", "import pandas"])
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    }

    #[test]
    fn python_passes_golden_cases() {
        let Some(py) = python() else {
            eprintln!("skipped: no python3 with pandas");
            return;
        };
        let out = std::process::Command::new(py).arg(script_path()).args(["--selftest"]).arg(golden_path()).output().unwrap();
        assert!(out.status.success(), "{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }

    /// Random layers written to CSV and read by the pandas script: every row's value, rank phrase
    /// and median line must match the Rust output exactly.
    #[test]
    fn python_matches_rust_on_random_layers() {
        let Some(py) = python() else {
            eprintln!("skipped: no python3 with pandas");
            return;
        };
        let dir = std::env::temp_dir().join(format!("lm-insight-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Deterministic LCG.
        let mut s: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = move || {
            s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (s >> 33) as f64 / (1u64 << 31) as f64
        };
        for (case, (n, decimals, scale)) in
            [(3usize, 0i32, 10.0), (7, 1, 40.0), (17, 1, 40.0), (40, 2, 1.0), (60, 3, 0.01), (250, 0, 1e6), (1500, 1, 100.0)]
                .into_iter()
                .enumerate()
        {
            let p = 10f64.powi(decimals);
            let mut raw: Vec<String> = Vec::new();
            for k in 0..n {
                let r = next();
                if r < 0.08 && case != 0 {
                    raw.push(String::new());
                } else if r < 0.3 && k > 0 {
                    let prev = raw[k - 1].clone();
                    raw.push(prev); // ties
                } else {
                    let v = ((next() - 0.3) * scale * p).round() / p;
                    raw.push(format!("{v}"));
                }
            }
            let csv: String = std::iter::once("id,v".to_string())
                .chain(raw.iter().enumerate().map(|(k, v)| format!("{k},{v}")))
                .collect::<Vec<_>>()
                .join("\n");
            let path = dir.join(format!("case{case}.csv"));
            std::fs::write(&path, csv + "\n").unwrap();
            let vals: Vec<Option<f64>> = raw.iter().map(|v| v.parse::<f64>().ok()).collect();
            let index = RankIndex::new(vals.clone());
            let out = std::process::Command::new(&py)
                .arg(script_path())
                .arg(&path)
                .arg("v")
                .args(["--all", "--unit", "°C", "--noun", "schools"])
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            let text = String::from_utf8(out.stdout).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            assert_eq!(lines.len(), n, "case {case}");
            for (k, line) in lines.iter().enumerate() {
                let row = measure_row(&index, vals[k], Some("°C"), "schools");
                let want = format!("{}\t{}\t{}", row.value, row.rank.unwrap_or_default(), row.median.unwrap_or_default());
                assert_eq!(*line, want, "case {case} row {k} raw {:?}", raw[k]);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Property tests ----------------------------------------------------------------------

    fn naive_rank(vals: &[Option<f64>], x: f64) -> (usize, usize, usize, usize, usize) {
        let fin: Vec<f64> = vals.iter().flatten().copied().filter(|v| v.is_finite()).collect();
        let mut higher = 0;
        let mut lower = 0;
        let mut equal = 0;
        for v in &fin {
            if *v > x {
                higher += 1
            } else if *v < x {
                lower += 1
            } else {
                equal += 1
            }
        }
        (higher, lower, equal, fin.len(), vals.len() - fin.len())
    }

    fn arb_value() -> impl Strategy<Value = Option<f64>> {
        prop_oneof![
            6 => (-20i32..20).prop_map(|i| Some(f64::from(i) / 2.0)),
            2 => (-1e6f64..1e6).prop_map(Some),
            1 => Just(None),
            1 => Just(Some(f64::NAN)),
            1 => Just(Some(f64::INFINITY)),
            1 => Just(Some(-0.0)),
        ]
    }

    proptest! {
        #[test]
        fn ranks_match_naive_sort(vals in proptest::collection::vec(arb_value(), 0..200), probe in -12i32..12) {
            let index = RankIndex::new(vals.clone());
            let mut probes: Vec<f64> = vals.iter().flatten().copied().filter(|v| v.is_finite()).collect();
            probes.push(f64::from(probe) / 2.0);
            for x in probes {
                let (h, l, e, n, m) = naive_rank(&vals, x);
                let r = index.rank(x);
                if n == 0 {
                    prop_assert!(r.is_none());
                    continue;
                }
                let r = r.unwrap();
                prop_assert_eq!((r.higher, r.lower, r.equal, r.n, r.missing), (h, l, e, n, m));
                // Competition rank from a naive descending sort.
                let mut desc: Vec<f64> = vals.iter().flatten().copied().filter(|v| v.is_finite()).collect();
                desc.sort_by(|a, b| b.partial_cmp(a).unwrap());
                if let Some(pos) = desc.iter().position(|v| *v == x) {
                    prop_assert_eq!(r.higher, pos);
                }
                // Percentile consistent with counts, never 100% for a member.
                let p = index.percentile(x).unwrap();
                prop_assert!((p - l as f64 / n as f64).abs() < 1e-12);
                if e > 0 {
                    prop_assert!(100 * r.lower / r.n <= 99);
                }
                if let Some(s) = rank_phrase(&r, "schools") {
                    prop_assert!(!s.contains("100%"));
                }
            }
        }

        #[test]
        fn median_matches_naive(vals in proptest::collection::vec(arb_value(), 0..200)) {
            let mut fin: Vec<f64> = vals.iter().flatten().copied().filter(|v| v.is_finite()).collect();
            fin.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let want = match fin.len() {
                0 => None,
                n if n % 2 == 1 => Some(fin[n / 2]),
                n => Some((fin[n / 2 - 1] + fin[n / 2]) / 2.0),
            };
            prop_assert_eq!(RankIndex::new(vals).median(), want);
        }

        #[test]
        fn histogram_counts_everything(vals in proptest::collection::vec(arb_value(), 1..200), bins in 1usize..40) {
            let index = RankIndex::new(vals);
            if let Some(h) = index.histogram(bins) {
                prop_assert_eq!(h.counts.len(), bins);
                prop_assert_eq!(h.counts.iter().sum::<usize>(), index.len());
            }
        }

        #[test]
        fn formatting_round_trips(v in -1e9f64..1e9, d in 0u8..7) {
            let s = format_number(v, Some(d));
            let back: f64 = s.replace(',', "").replace(MINUS, "-").parse().unwrap();
            let shown = d.min(sig3_decimals(v.abs()).max(2));
            prop_assert!((back - v).abs() <= 0.5 * 10f64.powi(-i32::from(shown)) + 1e-9 * v.abs());
        }
    }
}
