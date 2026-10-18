//! Plain number formatting for interface text (Microcopy rules: thousands separators).

/// 500000 → "500,000"; 12.5 → "12.5"; 0.333333 → "0.33"; −3 → "−3" (true minus sign).
/// Whole numbers print without decimals; others with at most two, trailing zeros trimmed.
pub fn number(v: f64) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    let neg = v < 0.0;
    let a = v.abs();
    let fixed = format!("{a:.2}");
    let (int, frac) = fixed.split_once('.').unwrap_or((fixed.as_str(), ""));
    let frac = frac.trim_end_matches('0');
    let int_n: u64 = int.parse().unwrap_or(0);
    let mut s = crate::model::group_thousands(int_n);
    if !frac.is_empty() {
        s.push('.');
        s.push_str(frac);
    }
    if neg && s.chars().any(|c| c.is_ascii_digit() && c != '0') {
        s.insert(0, '\u{2212}');
    }
    s
}

/// The years a year field may hold: anything from 1000 to 2100 reads as a year.
pub const YEAR_RANGE: std::ops::RangeInclusive<f64> = 1000.0..=2100.0;

/// Whether a field's name says it holds years: "year", "gdp_year", "YearBuilt", "season",
/// "yr", "fiscal_yr", "yyyy", "年份". Words inside other words don't count ("yield", "layer").
pub fn is_year_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("year")
        || n.contains('年')
        || n.split(|c: char| !c.is_ascii_alphanumeric())
            .flat_map(split_camel_lower)
            .any(|w| matches!(w.as_str(), "yr" | "yrs" | "yyyy" | "season" | "seasons"))
}

/// Splits a lowercased word at digits ("season2" → "season"), so "season_2020" and "yr2" count.
fn split_camel_lower(w: &str) -> Vec<String> {
    w.split(|c: char| c.is_ascii_digit()).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// Whether `v` of a field called `name` is a year: the name says so and the value lies in
/// [`YEAR_RANGE`]. Every surface that prints a field's value asks this, so they all agree.
pub fn is_year(name: &str, v: f64) -> bool {
    v.is_finite() && YEAR_RANGE.contains(&v) && is_year_name(name)
}

/// A year as written: "2013", never "2,013" (a fractional class break keeps its decimals:
/// "1993.5").
pub fn year(v: f64) -> String {
    number(v).replace(',', "")
}

/// A value of the field `name`: [`year`] for a year ([`is_year`]), else [`number`].
pub fn field_number(name: &str, v: f64) -> String {
    if is_year(name, v) { year(v) } else { number(v) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years_read_without_separators() {
        for n in ["year", "gdp_year", "YearBuilt", "season", "SEASON", "fiscal_yr", "yyyy", "建成年份", "season_2"] {
            assert!(is_year_name(n), "{n}");
        }
        for n in ["yield", "layer", "seasonal_rain", "population", "years_of_school_pct"] {
            // "years_of_school_pct" says year: its values decide (2.5 is no year).
            if n != "years_of_school_pct" {
                assert!(!is_year_name(n), "{n}");
            }
        }
        assert_eq!(field_number("season", 2013.0), "2013");
        assert_eq!(field_number("year", 1980.0), "1980");
        assert_eq!(field_number("year", 1993.5), "1993.5");
        assert_eq!(field_number("years_of_school_pct", 12.0), "12");
        assert_eq!(field_number("year", 12_000.0), "12,000");
        assert_eq!(field_number("population", 2013.0), "2,013");
        assert!(!is_year("year", f64::NAN));
    }

    #[test]
    fn numbers_read_plainly() {
        assert_eq!(number(500_000.0), "500,000");
        assert_eq!(number(12.5), "12.5");
        assert_eq!(number(1.0 / 3.0), "0.33");
        assert_eq!(number(-3.0), "\u{2212}3");
        assert_eq!(number(-0.001), "0");
        assert_eq!(number(1234.567), "1,234.57");
        assert_eq!(number(f64::NAN), "—");
    }
}
