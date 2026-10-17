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

#[cfg(test)]
mod tests {
    use super::number;

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
