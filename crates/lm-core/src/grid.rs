//! Gridded files with variables and a time axis (WP6.12, NetCDF): what the document keeps about
//! them, and the words the interface uses for them ("2 m temperature (°C)", "July 2023").
//!
//! A NetCDF file is one raster layer. Its [`GridProfile`] lists the variables on the latitude–
//! longitude grid (long name, unit, time axis, extra level dimension), the decoded dates of each
//! time axis, and which variable and level are shown. Which time step is shown is the layer's
//! own state ([`crate::document::DocLayer::time`]), not the profile's: stepping through time
//! never re-reads the profile.

use serde::{Deserialize, Serialize};

/// A date on a file's own calendar (CF calendars: a 360_day file has 30 February), down to the
/// minute: `[year, month, day, hour, minute]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GridDate(pub i32, pub u8, pub u8, pub u8, pub u8);

impl GridDate {
    pub fn year(self) -> i32 {
        self.0
    }
    pub fn month(self) -> u8 {
        self.1
    }
    pub fn day(self) -> u8 {
        self.2
    }

    /// "2023-07-16" (with " 06:00" when the time of day isn't midnight).
    pub fn iso(self) -> String {
        let d = format!("{:04}-{:02}-{:02}", self.0, self.1, self.2);
        if self.3 == 0 && self.4 == 0 { d } else { format!("{d} {:02}:{:02}", self.3, self.4) }
    }

    /// Approximate days since year 0, for the spacing between steps (any calendar).
    fn approx_days(self) -> f64 {
        f64::from(self.0) * 365.25
            + (f64::from(self.1) - 1.0) * 30.44
            + f64::from(self.2)
            + (f64::from(self.3) + f64::from(self.4) / 60.0) / 24.0
    }
}

/// A time axis: its dimension, calendar and dates, one per step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridTimeAxis {
    /// The dimension's name in the file ("time").
    pub name: String,
    /// The CF calendar as written ("standard", "noleap", "360_day"…).
    pub calendar: String,
    /// One per step; empty when the axis couldn't be decoded (steps are then numbered).
    #[serde(default)]
    pub dates: Vec<GridDate>,
    /// How many steps the axis has.
    pub len: usize,
}

/// A variable's extra dimension (depth, pressure level), with its values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridLevels {
    /// The dimension's name ("level", "depth").
    pub name: String,
    #[serde(default)]
    pub long_name: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    /// One per level (the coordinate's values, or 1, 2, 3… when it has none).
    pub values: Vec<f64>,
}

/// One variable on the latitude–longitude grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridVariable {
    /// The name in the file ("t2m").
    pub name: String,
    /// `long_name` (or `standard_name`) from the file ("2 m temperature").
    #[serde(default)]
    pub long_name: Option<String>,
    /// `units` from the file ("degC").
    #[serde(default)]
    pub unit: Option<String>,
    /// The time axis it runs along (a [`GridTimeAxis::name`]); None for a variable without time.
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub levels: Option<GridLevels>,
}

impl GridVariable {
    /// "2 m temperature (°C)": the long name (else the name) and the unit as people write it.
    pub fn label(&self) -> String {
        let name = self.long_name.as_deref().filter(|n| !n.trim().is_empty()).unwrap_or(&self.name);
        match self.unit.as_deref().map(unit_text).filter(|u| !u.is_empty()) {
            Some(u) => format!("{name} ({u})"),
            None => name.to_string(),
        }
    }

    /// "850 hPa" for a level value.
    pub fn level_label(&self, index: usize) -> String {
        let Some(l) = &self.levels else { return String::new() };
        let v = l.values.get(index).copied().unwrap_or(index as f64 + 1.0);
        let n = number(v);
        match l.unit.as_deref().map(unit_text).filter(|u| !u.is_empty() && *u != "1") {
            Some(u) => format!("{n} {u}"),
            None => format!("{} {n}", l.long_name.as_deref().unwrap_or(&l.name)),
        }
    }
}

/// What the document keeps about a gridded file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridProfile {
    /// The variables on the latitude–longitude grid, in file order.
    pub variables: Vec<GridVariable>,
    /// The shown variable's name.
    pub variable: String,
    /// The shown level of that variable (0 for the first).
    #[serde(default)]
    pub level: usize,
    #[serde(default)]
    pub times: Vec<GridTimeAxis>,
    /// Interpretations applied to the shown variable, shown as visible assumptions
    /// ("Longitudes 0–360 shown as −180–180.", "Time: noleap calendar (no leap days).").
    #[serde(default)]
    pub lines: Vec<String>,
    /// The colour range the variable opens with, the same for every time step: the 2nd to 98th
    /// percentile of the cells of [`GridProfile::sampled`] evenly spaced steps.
    #[serde(default)]
    pub range: Option<(f64, f64)>,
    /// How many time steps the range was taken from.
    #[serde(default)]
    pub sampled: usize,
}

impl GridProfile {
    /// The shown variable.
    pub fn shown(&self) -> Option<&GridVariable> {
        self.variables.iter().find(|v| v.name == self.variable)
    }

    /// The time axis of a variable.
    pub fn axis_of(&self, v: &GridVariable) -> Option<&GridTimeAxis> {
        let name = v.time.as_deref()?;
        self.times.iter().find(|t| t.name == name)
    }

    /// The shown variable's time axis (None when it has no time).
    pub fn axis(&self) -> Option<&GridTimeAxis> {
        self.axis_of(self.shown()?)
    }

    /// How many time steps the shown variable has (1 without time).
    pub fn steps(&self) -> usize {
        self.axis().map_or(1, |a| a.len.max(1))
    }

    /// Whether the shown variable has more than one time step (the time slider shows).
    pub fn has_time(&self) -> bool {
        self.steps() > 1
    }

    /// "July 2023" for step `i` of the shown variable (empty without time).
    pub fn date_label(&self, i: usize) -> String {
        self.axis().map(|a| axis_label(a, i)).unwrap_or_default()
    }

    /// "2 m temperature (°C), July 2023" (and the level when there is one): what a recipe or a
    /// caption names.
    pub fn caption(&self, time: usize) -> String {
        let Some(v) = self.shown() else { return String::new() };
        let mut parts = vec![v.label()];
        if v.levels.as_ref().is_some_and(|l| l.values.len() > 1) {
            parts.push(v.level_label(self.level));
        }
        let d = self.date_label(time);
        if !d.is_empty() {
            parts.push(d);
        }
        parts.join(", ")
    }

    /// "January 1950 to December 2023" for the shown variable (None without time).
    pub fn time_range(&self) -> Option<String> {
        let a = self.axis()?;
        if a.len < 2 {
            return None;
        }
        Some(format!("{} to {} ({} steps)", axis_label(a, 0), axis_label(a, a.len - 1), a.len))
    }
}

/// How far apart a time axis's steps are, which decides how their dates are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    /// Less than a day apart: "16 Jul 2023 06:00".
    SubDaily,
    /// Days apart: "16 Jul 2023".
    Daily,
    /// About a month or a season apart: "July 2023".
    Monthly,
    /// A year or more apart: "2023".
    Yearly,
}

/// The cadence of `dates`, from the median spacing between steps. One date alone: daily, or
/// sub-daily when it has a time of day.
pub fn cadence(dates: &[GridDate]) -> Cadence {
    let mut gaps: Vec<f64> = dates
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some((b.approx_days() - a.approx_days()).abs()),
            _ => None,
        })
        .filter(|g| *g > 0.0)
        .collect();
    if gaps.is_empty() {
        return match dates.first() {
            Some(d) if d.3 != 0 || d.4 != 0 => Cadence::SubDaily,
            _ => Cadence::Daily,
        };
    }
    gaps.sort_by(f64::total_cmp);
    let g = gaps.get(gaps.len() / 2).copied().unwrap_or(0.0);
    // Every step on the same time of day within a month: monthly; a year apart: yearly.
    if g < 0.99 {
        Cadence::SubDaily
    } else if g < 27.0 {
        Cadence::Daily
    } else if g < 300.0 {
        Cadence::Monthly
    } else {
        Cadence::Yearly
    }
}

const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

fn month_name(m: u8) -> &'static str {
    MONTHS.get(usize::from(m).saturating_sub(1)).copied().unwrap_or("")
}

fn month_short(m: u8) -> &'static str {
    month_name(m).get(..3).unwrap_or("")
}

/// A date written for a cadence: "2023", "July 2023", "16 Jul 2023", "16 Jul 2023 06:00".
/// Years before 1 are written as astronomers do (0, −1…), with no era.
pub fn format_date(d: GridDate, c: Cadence) -> String {
    match c {
        Cadence::Yearly => d.0.to_string(),
        Cadence::Monthly => format!("{} {}", month_name(d.1), d.0),
        Cadence::Daily => format!("{} {} {}", d.2, month_short(d.1), d.0),
        Cadence::SubDaily => format!("{} {} {} {:02}:{:02}", d.2, month_short(d.1), d.0, d.3, d.4),
    }
}

/// Step `i` of an axis in words: its date for the axis's cadence, or "Step 3 of 12" when the
/// dates couldn't be read.
pub fn axis_label(a: &GridTimeAxis, i: usize) -> String {
    match a.dates.get(i) {
        Some(d) => format_date(*d, cadence(&a.dates)),
        None => format!("Step {} of {}", i + 1, a.len.max(1)),
    }
}

/// A CF unit as people write it: "degC" → "°C", "degrees_Celsius" → "°C", "degK" → "K",
/// "percent" → "%"; anything else as written.
pub fn unit_text(u: &str) -> String {
    let t = u.trim();
    match t {
        "degC" | "deg_C" | "degree_C" | "degrees_C" | "degreeC" | "degrees_Celsius" | "degree_Celsius" | "celsius" | "Celsius" | "C" => {
            "°C".into()
        }
        "degK" | "deg_K" | "degree_K" | "degrees_K" | "kelvin" | "Kelvin" => "K".into(),
        "degF" | "degrees_F" | "degree_F" | "degrees_Fahrenheit" => "°F".into(),
        "percent" | "%" => "%".into(),
        _ => t.to_string(),
    }
}

/// A level or coordinate value with no needless decimals ("850", "2.5").
fn number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(dates: Vec<GridDate>) -> GridTimeAxis {
        GridTimeAxis { name: "time".into(), calendar: "standard".into(), len: dates.len(), dates }
    }

    #[test]
    fn monthly_steps_read_as_month_and_year() {
        // Monthly means stamped on the 1st (OISST) or mid-month (CMIP): both monthly.
        let first = axis((1..=3).map(|m| GridDate(2023, m, 1, 0, 0)).collect());
        assert_eq!(axis_label(&first, 0), "January 2023");
        assert_eq!(axis_label(&first, 2), "March 2023");
        let mid = axis(vec![GridDate(2000, 1, 16, 12, 0), GridDate(2000, 2, 15, 0, 0), GridDate(2000, 3, 16, 12, 0)]);
        assert_eq!(cadence(&mid.dates), Cadence::Monthly);
        assert_eq!(axis_label(&mid, 1), "February 2000");
        // A 360_day calendar's months are 30 days apart; seasons are monthly too.
        let d360 = axis((1..=4).map(|m| GridDate(2001, m, 16, 0, 0)).collect());
        assert_eq!(axis_label(&d360, 3), "April 2001");
        let seasons = axis(vec![GridDate(2001, 1, 15, 0, 0), GridDate(2001, 4, 15, 0, 0), GridDate(2001, 7, 15, 0, 0)]);
        assert_eq!(cadence(&seasons.dates), Cadence::Monthly);
    }

    #[test]
    fn daily_yearly_and_sub_daily_steps() {
        let daily = axis(vec![GridDate(2004, 2, 27, 0, 0), GridDate(2004, 2, 28, 0, 0), GridDate(2004, 3, 1, 0, 0)]);
        assert_eq!(axis_label(&daily, 2), "1 Mar 2004");
        let yearly = axis((2000..2004).map(|y| GridDate(y, 1, 1, 0, 0)).collect());
        assert_eq!(axis_label(&yearly, 3), "2003");
        let six_hourly = axis(vec![GridDate(2023, 7, 16, 0, 0), GridDate(2023, 7, 16, 6, 0), GridDate(2023, 7, 16, 12, 0)]);
        assert_eq!(axis_label(&six_hourly, 1), "16 Jul 2023 06:00");
        // One step: daily, or sub-daily with a time of day.
        assert_eq!(axis_label(&axis(vec![GridDate(2000, 5, 16, 0, 0)]), 0), "16 May 2000");
        assert_eq!(axis_label(&axis(vec![GridDate(2000, 5, 16, 12, 0)]), 0), "16 May 2000 12:00");
        // Year 0 of a model calendar, and an axis whose dates couldn't be read.
        assert_eq!(format_date(GridDate(0, 1, 1, 0, 0), Cadence::Yearly), "0");
        let unread = GridTimeAxis { name: "time".into(), calendar: "standard".into(), dates: vec![], len: 12 };
        assert_eq!(axis_label(&unread, 2), "Step 3 of 12");
    }

    #[test]
    fn variables_read_with_their_units() {
        let v = GridVariable {
            name: "t2m".into(),
            long_name: Some("2 m temperature".into()),
            unit: Some("degC".into()),
            time: Some("time".into()),
            levels: None,
        };
        assert_eq!(v.label(), "2 m temperature (°C)");
        let bare = GridVariable { name: "sst".into(), long_name: None, unit: None, time: None, levels: None };
        assert_eq!(bare.label(), "sst");
        let ta = GridVariable {
            levels: Some(GridLevels { name: "level".into(), long_name: None, unit: Some("hPa".into()), values: vec![850.0, 500.0] }),
            ..v.clone()
        };
        assert_eq!(ta.level_label(1), "500 hPa");
        let depth = GridVariable {
            levels: Some(GridLevels { name: "depth".into(), long_name: Some("depth".into()), unit: None, values: vec![2.5] }),
            ..v
        };
        assert_eq!(depth.level_label(0), "depth 2.5");
        assert_eq!(unit_text("degrees_Celsius"), "°C");
        assert_eq!(unit_text("kg m-2 s-1"), "kg m-2 s-1");
    }

    #[test]
    fn captions_name_the_variable_level_and_date() {
        let g = GridProfile {
            variables: vec![GridVariable {
                name: "ta".into(),
                long_name: Some("Air temperature".into()),
                unit: Some("K".into()),
                time: Some("time".into()),
                levels: Some(GridLevels { name: "level".into(), long_name: None, unit: Some("hPa".into()), values: vec![850.0, 500.0] }),
            }],
            variable: "ta".into(),
            level: 1,
            times: vec![axis((6..=8).map(|m| GridDate(2023, m, 1, 0, 0)).collect())],
            lines: vec![],
            range: None,
            sampled: 0,
        };
        assert_eq!(g.caption(1), "Air temperature (K), 500 hPa, July 2023");
        assert_eq!(g.steps(), 3);
        assert!(g.has_time());
        assert_eq!(g.time_range().as_deref(), Some("June 2023 to August 2023 (3 steps)"));
    }
}
