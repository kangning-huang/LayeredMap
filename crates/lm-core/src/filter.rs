//! Display filters (WP3.7 Filter builder): "Show [schools] where [Type] [is] [Primary]".
//!
//! A filter is a list of conditions, all of which must hold. It belongs to the document (a
//! filtered layer shows a funnel on its shelf row and insight counts only filtered records), and
//! it never changes the data. Evaluation takes cell values through [`Cell`], so this crate needs
//! no data library.

use serde::{Deserialize, Serialize};

/// A cell value as a filter sees it. Blank and zero stay distinct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cell<'a> {
    Empty,
    Number(f64),
    Text(&'a str),
}

/// One test on a field (Filter builder › Conditions by type).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Test {
    /// Text equals (case-insensitive, trimmed).
    Is(String),
    IsNot(String),
    Contains(String),
    /// Text begins / ends with (trimmed, Unicode lowercase on both sides).
    StartsWith(String),
    EndsWith(String),
    IsEmpty,
    Above(f64),
    Below(f64),
    /// Inclusive at both ends.
    Between(f64, f64),
    /// Dates as written in ISO order (YYYY-MM-DD…), compared as text.
    Before(String),
    After(String),
    /// Dates from the first to the second, inclusive (as written, ISO order).
    DateBetween(String, String),
    /// Category is any of these values.
    AnyOf(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub test: Test,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub conditions: Vec<Condition>,
}

fn text_of(c: Cell<'_>) -> Option<String> {
    match c {
        Cell::Empty => None,
        Cell::Number(v) => Some(crate::filter::number_text(v)),
        Cell::Text(t) => Some(t.trim().to_string()),
    }
}

/// A number as text without a trailing ".0" (category values that are whole numbers).
pub fn number_text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

fn number_of(c: Cell<'_>) -> Option<f64> {
    match c {
        Cell::Number(v) if v.is_finite() => Some(v),
        Cell::Text(t) => t.trim().parse::<f64>().ok().filter(|v| v.is_finite()),
        _ => None,
    }
}

impl Test {
    pub fn matches(&self, c: Cell<'_>) -> bool {
        let empty = match c {
            Cell::Empty => true,
            Cell::Text(t) => t.trim().is_empty(),
            Cell::Number(v) => v.is_nan(),
        };
        match self {
            Test::IsEmpty => empty,
            _ if empty => false,
            Test::Is(v) => text_of(c).is_some_and(|t| t.eq_ignore_ascii_case(v.trim())),
            Test::IsNot(v) => text_of(c).is_some_and(|t| !t.eq_ignore_ascii_case(v.trim())),
            Test::Contains(v) => text_of(c).is_some_and(|t| t.to_lowercase().contains(&v.trim().to_lowercase())),
            Test::StartsWith(v) => text_of(c).is_some_and(|t| t.to_lowercase().starts_with(&v.trim().to_lowercase())),
            Test::EndsWith(v) => text_of(c).is_some_and(|t| t.to_lowercase().ends_with(&v.trim().to_lowercase())),
            Test::Above(x) => number_of(c).is_some_and(|v| v > *x),
            Test::Below(x) => number_of(c).is_some_and(|v| v < *x),
            Test::Between(a, b) => number_of(c).is_some_and(|v| v >= a.min(*b) && v <= a.max(*b)),
            Test::Before(d) => text_of(c).is_some_and(|t| t.as_str() < d.as_str()),
            Test::After(d) => text_of(c).is_some_and(|t| t.as_str() > d.as_str()),
            Test::DateBetween(a, b) => {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                // A date-time on the last day still counts: compare its date part.
                text_of(c).is_some_and(|t| t.as_str() >= lo.as_str() && t.get(..hi.len()).unwrap_or(t.as_str()) <= hi.as_str())
            }
            Test::AnyOf(vs) => text_of(c).is_some_and(|t| vs.iter().any(|v| t.eq_ignore_ascii_case(v.trim()))),
        }
    }

    /// The test in words, for chips ("is Primary", "above 500,000").
    pub fn words(&self) -> String {
        self.words_for("")
    }

    /// [`Test::words`] for a test on the field `field`: a year field's years read "after 2000",
    /// not "2,000" ([`crate::format::field_number`]).
    pub fn words_for(&self, field: &str) -> String {
        let n = |v: f64| crate::format::field_number(field, v);
        match self {
            Test::Is(v) => format!("is {v}"),
            Test::IsNot(v) => format!("is not {v}"),
            Test::Contains(v) => format!("contains “{v}”"),
            Test::StartsWith(v) => format!("starts with “{v}”"),
            Test::EndsWith(v) => format!("ends with “{v}”"),
            Test::IsEmpty => "is empty".into(),
            Test::Above(x) => format!("above {}", n(*x)),
            Test::Below(x) => format!("below {}", n(*x)),
            Test::Between(a, b) => format!("between {} and {}", n(a.min(*b)), n(a.max(*b))),
            Test::Before(d) => format!("before {d}"),
            Test::After(d) => format!("after {d}"),
            Test::DateBetween(a, b) => format!("between {a} and {b}"),
            Test::AnyOf(vs) => match vs.len() {
                0 => "is any of (none)".into(),
                1..=3 => format!("is {}", vs.join(" or ")),
                k => format!("is any of {k} values"),
            },
        }
    }
}

impl Condition {
    /// The chip text: "Type is Primary".
    pub fn chip(&self) -> String {
        format!("{} {}", self.field, self.test.words_for(&self.field))
    }
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    /// Whether a record passes every condition; `cell` gives the record's value of a field.
    pub fn matches<'a>(&self, cell: impl Fn(&str) -> Cell<'a>) -> bool {
        self.conditions.iter().all(|c| c.test.matches(cell(&c.field)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec<'a>(ty: &'a str, pop: Option<f64>) -> impl Fn(&str) -> Cell<'a> {
        move |f: &str| match f {
            "Type" => Cell::Text(ty),
            "population" => pop.map_or(Cell::Empty, Cell::Number),
            _ => Cell::Empty,
        }
    }

    #[test]
    fn conditions_all_hold() {
        let f = Filter {
            conditions: vec![
                Condition { field: "Type".into(), test: Test::Is("primary".into()) },
                Condition { field: "population".into(), test: Test::Above(500_000.0) },
            ],
        };
        assert!(f.matches(rec("Primary", Some(600_000.0))));
        assert!(!f.matches(rec("Primary", Some(500_000.0))), "above is strict");
        assert!(!f.matches(rec("Secondary", Some(600_000.0))));
        assert!(!f.matches(rec("Primary", None)), "a blank is not above anything");
        assert!(Filter::default().matches(rec("x", None)));
    }

    #[test]
    fn blank_and_zero_stay_distinct() {
        assert!(Test::IsEmpty.matches(Cell::Empty));
        assert!(Test::IsEmpty.matches(Cell::Text("  ")));
        assert!(!Test::IsEmpty.matches(Cell::Number(0.0)));
        assert!(Test::Below(1.0).matches(Cell::Number(0.0)));
        assert!(Test::StartsWith("za".into()).matches(Cell::Text(" Zaragoza")));
        assert!(Test::EndsWith("DAN".into()).matches(Cell::Text("Jordan")));
        assert!(!Test::StartsWith("x".into()).matches(Cell::Empty));
        assert!(Test::StartsWith("Ü".into()).matches(Cell::Text("über")));
        assert!(!Test::IsNot("a".into()).matches(Cell::Empty));
    }

    #[test]
    fn between_contains_any_of_and_dates() {
        assert!(Test::Between(10.0, 1.0).matches(Cell::Number(10.0)));
        assert!(Test::Contains("ARK".into()).matches(Cell::Text("Central Park")));
        assert!(Test::AnyOf(vec!["1".into(), "3".into()]).matches(Cell::Number(3.0)));
        assert!(Test::Before("2024-01-01".into()).matches(Cell::Text("2023-12-31")));
        let d = Test::DateBetween("2024-01-01".into(), "2024-01-31".into());
        assert!(d.matches(Cell::Text("2024-01-31T18:00")));
        assert!(!d.matches(Cell::Text("2024-02-01")));
        assert!(Test::Above(5.0).matches(Cell::Text("6.5")));
    }

    #[test]
    fn chips_read_as_sentences() {
        let c = Condition { field: "population".into(), test: Test::Above(500_000.0) };
        assert_eq!(c.chip(), "population above 500,000");
        assert_eq!(Condition { field: "Type".into(), test: Test::Is("Primary".into()) }.chip(), "Type is Primary");
        let y = Condition { field: "season".into(), test: Test::Between(2000.0, 2013.0) };
        assert_eq!(y.chip(), "season between 2000 and 2013");
    }
}
