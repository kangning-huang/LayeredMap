//! Results that explain themselves (WP4.6): every kept result carries a recipe generated from the
//! plan that made it — inputs with fingerprints, scope, method, decisions, assumptions, coordinate
//! handling, exclusions and the software version. Copy gives plain text; the document keeps the
//! recipe itself (serialized with the document). A source whose fingerprint no longer matches
//! marks the result "Source changed".

use serde::{Deserialize, Serialize};

use crate::model::{Confidence, LayerId};

/// One input to a result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeInput {
    /// The layer's name as shown ("Schools").
    pub name: String,
    /// The file it was read from ("schools.csv").
    pub file: String,
    /// What the source was when the result was computed (`lm_jobs` fingerprints files).
    pub fingerprint: String,
    /// How many records it had, and their noun ("248 points").
    pub count: u64,
    pub noun: String,
}

/// An assumption the result rests on, with how sure LayeredMap was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecipeAssumption {
    pub text: String,
    pub confidence: Confidence,
}

/// The recipe of one result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    /// "Schools in each district".
    pub title: String,
    /// The action that made it (`count_in_areas`) and its parameters, so Change question can
    /// reopen it with the same inputs.
    pub action: String,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
    /// Seconds since the Unix epoch.
    pub computed_at: i64,
    /// "LayeredMap 0.4.0".
    pub software: String,
    pub inputs: Vec<RecipeInput>,
    /// "all schools; all districts".
    pub scope: String,
    /// "point-in-area count; a point on a boundary counts in every area it touches".
    pub method: String,
    /// "overlapping areas each count their points (2 schools counted twice)".
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<RecipeAssumption>,
    /// "geodesic, on the WGS 84 ellipsoid".
    #[serde(default)]
    pub coordinates: Option<String>,
    /// "6 schools with no usable location".
    #[serde(default)]
    pub excluded: Vec<String>,
}

/// "14 Dec 2026" for a Unix time (UTC; the date is what a reader needs).
pub fn short_date(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

impl Recipe {
    /// The plain text Copy gives, in the order a reader checks it.
    pub fn text(&self) -> String {
        let mut out = vec![self.title.clone(), format!("Computed {} with {}", short_date(self.computed_at), self.software)];
        let inputs: Vec<String> =
            self.inputs.iter().map(|i| format!("{} ({}, {} {})", i.name, i.file, crate::model::group_thousands(i.count), i.noun)).collect();
        out.push(format!("Inputs: {}", inputs.join("; ")));
        out.push(format!("Scope: {}", self.scope));
        out.push(format!("Method: {}", self.method));
        if let Some(c) = &self.coordinates {
            out.push(format!("Coordinates: {c}"));
        }
        for d in &self.decisions {
            out.push(format!("Decision: {d}"));
        }
        for a in &self.assumptions {
            out.push(format!("Assumption: {} ({})", a.text, confidence_word(a.confidence)));
        }
        for e in &self.excluded {
            out.push(format!("Excluded: {e}"));
        }
        out.join("\n")
    }

    /// The inputs whose source changed since the result was computed, given each input's
    /// fingerprint now (None: the source can't be read).
    pub fn changed_inputs(&self, now: impl Fn(&RecipeInput) -> Option<String>) -> Vec<&RecipeInput> {
        self.inputs.iter().filter(|i| now(i).as_deref() != Some(i.fingerprint.as_str())).collect()
    }
}

fn confidence_word(c: Confidence) -> &'static str {
    match c {
        Confidence::Certain => "certain",
        Confidence::Likely => "likely",
        Confidence::Possible => "possible",
        Confidence::Unknown => "unknown",
    }
}

/// What a result card shows (spec › Result card), kept with the result so it reads the same
/// after the document is reopened.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResultCard {
    /// "Schools in each district".
    pub title: String,
    /// One line for the composer after a preview: "17 districts · 0–34 schools · 2 with none".
    pub summary: String,
    pub answer: Answer,
    /// Inputs, scope and exclusions, one line each ("Schools, 248; Districts, 17").
    #[serde(default)]
    pub context: Vec<String>,
    /// Things the reader must know ("2 schools are counted in two districts").
    #[serde(default)]
    pub notices: Vec<String>,
}

/// The answer part of a result card.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    #[default]
    None,
    /// A ranking: every row, largest first (the card shows the top 5, then "Show all").
    Ranking { rows: Vec<RankRow>, unit: String },
    /// One number with its unit and a line under it ("over 83% of this district; the rest has
    /// no data").
    Number { value: f64, unit: String, detail: String },
    /// A short list of names ("Huangpu", "Jing'an") with a heading ("In 2 districts").
    List { heading: String, items: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankRow {
    pub label: String,
    /// None: no value (shown as "No data", ranked last).
    pub value: Option<f64>,
    /// The source row, to select it on the map.
    pub row: usize,
}

/// A kept result in the document: the layer it made (if any), its recipe and its version.
/// Update result adds a version; the old one stays readable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResultRecord {
    pub id: u64,
    pub layer: Option<LayerId>,
    pub recipe: Recipe,
    pub version: u32,
    /// The versions this one replaced, oldest first.
    #[serde(default)]
    pub history: Vec<Recipe>,
    #[serde(default)]
    pub card: ResultCard,
    /// The layer the question was about (the composer's layer), where a card-only result is
    /// listed.
    #[serde(default)]
    pub about: Option<LayerId>,
}

impl ResultRecord {
    /// The card's status line: "Source changed" when an input's fingerprint moved.
    pub fn status(&self, now: impl Fn(&RecipeInput) -> Option<String>) -> Option<String> {
        let changed = self.recipe.changed_inputs(now);
        (!changed.is_empty()).then(|| format!("Source changed: {}", changed.iter().map(|i| i.file.as_str()).collect::<Vec<_>>().join(", ")))
    }

    /// A new version from a recomputed recipe; the current one moves into the history.
    pub fn updated(&self, recipe: Recipe) -> ResultRecord {
        let mut history = self.history.clone();
        history.push(self.recipe.clone());
        ResultRecord {
            id: self.id,
            layer: self.layer,
            recipe,
            version: self.version + 1,
            history,
            card: self.card.clone(),
            about: self.about,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schools() -> Recipe {
        Recipe {
            title: "Schools in each district".into(),
            action: "count_in_areas".into(),
            params: Default::default(),
            computed_at: 1_797_206_400, // 14 Dec 2026
            software: "LayeredMap 0.4.0".into(),
            inputs: vec![
                RecipeInput {
                    name: "Schools".into(),
                    file: "schools.csv".into(),
                    fingerprint: "a1".into(),
                    count: 248,
                    noun: "points".into(),
                },
                RecipeInput {
                    name: "Districts".into(),
                    file: "districts.geojson".into(),
                    fingerprint: "b2".into(),
                    count: 17,
                    noun: "areas".into(),
                },
            ],
            scope: "all schools; all districts".into(),
            method: "point-in-area count; a point on a boundary counts in every area it touches".into(),
            decisions: vec!["overlapping areas each count their points (2 schools counted twice)".into()],
            assumptions: vec![RecipeAssumption { text: "schools located using lon/lat as WGS 84".into(), confidence: Confidence::Likely }],
            coordinates: None,
            excluded: vec!["6 schools with no usable location".into()],
        }
    }

    /// The text matches the build plan's example (WP4.6).
    #[test]
    fn the_recipe_reads_as_the_plan_shows() {
        assert_eq!(
            schools().text(),
            "Schools in each district\n\
             Computed 14 Dec 2026 with LayeredMap 0.4.0\n\
             Inputs: Schools (schools.csv, 248 points); Districts (districts.geojson, 17 areas)\n\
             Scope: all schools; all districts\n\
             Method: point-in-area count; a point on a boundary counts in every area it touches\n\
             Decision: overlapping areas each count their points (2 schools counted twice)\n\
             Assumption: schools located using lon/lat as WGS 84 (likely)\n\
             Excluded: 6 schools with no usable location"
        );
    }

    #[test]
    fn a_moved_fingerprint_marks_the_source_changed() {
        let r = ResultRecord {
            id: 1,
            layer: None,
            recipe: schools(),
            version: 1,
            history: Vec::new(),
            card: ResultCard::default(),
            about: None,
        };
        assert_eq!(r.status(|i| Some(i.fingerprint.clone())), None);
        assert_eq!(
            r.status(|i| if i.file == "schools.csv" { Some("zz".into()) } else { Some(i.fingerprint.clone()) }).as_deref(),
            Some("Source changed: schools.csv")
        );
        let r2 = r.updated(schools());
        assert_eq!((r2.version, r2.history.len()), (2, 1));
    }
}
