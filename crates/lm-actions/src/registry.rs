//! The action registry: every action's spec and the table rows, read from `rules.toml`.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The rules file, compiled in.
pub const RULES_TOML: &str = include_str!("../rules.toml");

/// What is selected, reduced to the spec table's "Selected" column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionKey {
    /// Nothing selected and no data open.
    Empty,
    /// Nothing selected, data open (no table row).
    Nothing,
    OnePoint,
    OneLine,
    OneArea,
    PointLayer,
    LineLayer,
    AreaLayer,
    RasterLayer,
    RasterPixel,
    /// A table without locations, or one of its rows.
    Table,
    DrawnArea,
    /// Several objects of one layer (no table row).
    Several,
}

/// The More actions groups, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
pub enum Group {
    Understand,
    Compare,
    Measure,
    #[serde(rename = "Change look")]
    ChangeLook,
    Keep,
}

impl Group {
    /// Display order of the More actions groups.
    pub const ALL: [Group; 5] = [Group::Understand, Group::Compare, Group::Measure, Group::ChangeLook, Group::Keep];

    /// The group heading.
    pub fn label(self) -> &'static str {
        match self {
            Group::Understand => "Understand",
            Group::Compare => "Compare",
            Group::Measure => "Measure",
            Group::ChangeLook => "Change look",
            Group::Keep => "Keep",
        }
    }
}

/// Where an action may appear (Product tiers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// Display-only: a suggested-action button on the card, in the free app.
    Card,
    /// Analysis: never a button in either tier. Called by Pro's assistant; shown only behind the
    /// developer flag.
    Assistant,
}

/// The kind of second layer an action works with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionKind {
    Points,
    Lines,
    Areas,
    Raster,
    /// Any visible layer with locations.
    Any,
    /// Any visible point, line or area layer (not a raster).
    Vector,
    /// A visible layer with an identifier column named like one of the selected table's.
    SharedCode,
}

/// What an action needs from the selected layer's fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Need {
    /// A field whose role is Measure, at Certain or Likely confidence.
    MeasureField,
    /// Such a measure field, or a Category field (Certain or Likely).
    StyleField,
    /// A Category field (Certain or Likely).
    CategoryField,
    /// Two columns whose role is Coordinate.
    CoordinateFields,
}

/// A requirement with the reason shown when it fails, which tells the user what to do.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub need: Need,
    pub reason: String,
}

/// The second layer an action needs, with the reason shown when none is open.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionSpec {
    pub kind: CompanionKind,
    pub reason: String,
}

/// Overrides for some selections ("Color or size by a value" for point layers).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub on: Vec<SelectionKey>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub requires: Option<Vec<Requirement>>,
    #[serde(default)]
    pub companion: Option<CompanionSpec>,
}

/// How a result is previewed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewPolicy {
    /// Display-only actions apply at once and can be undone.
    NotNeeded,
    /// Analysis: previewed automatically when the estimated cost is under a second, otherwise
    /// on request, with a warning past 30 seconds (WP4.3).
    Auto,
}

/// What an action produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputContract {
    /// Changes only what the map shows, or opens data; creates no new data.
    NoNewData,
    /// A new layer with a result card ("Schools in each district").
    NewLayer,
    /// New fields on the selected layer, one undo step ("Heat" on each school).
    NewFields,
    /// A result card only (a measurement, a ranking, which area).
    Card,
    /// A filtered view of an existing layer ("What's inside?").
    FilteredView,
}

/// One action (Architecture › Data contracts › ActionSpec).
#[derive(Clone, Debug, PartialEq)]
pub struct ActionSpec {
    pub id: String,
    /// Translation key: `action.<id>`.
    pub label_key: String,
    /// The label template, with tokens such as `[layer]` and `[Heat]`.
    pub label: String,
    /// Technical words that find the action in search ("buffer").
    pub aliases: Vec<String>,
    pub group: Group,
    /// The selections the action works on.
    pub inputs: Vec<SelectionKey>,
    /// Shown when the selection is not one of `inputs`.
    pub input_reason: String,
    pub requires: Vec<Requirement>,
    pub companion: Option<CompanionSpec>,
    pub variants: Vec<Variant>,
    /// Choices the plan makes, which the recipe records ("distance, in meters or kilometers").
    pub decisions: Vec<String>,
    /// The certified algorithm that computes it (`overlay::count_in_areas`), for analysis.
    pub algorithm: Option<String>,
    /// The method sentence the recipe uses, for analysis.
    pub method: Option<String>,
    pub preview: PreviewPolicy,
    pub output: OutputContract,
    /// The first phase whose build offers the action.
    pub min_phase: u8,
    pub surface: Surface,
    /// Offered under More actions and to the assistant only, never in a row's top three, so no
    /// row of the spec's table needs to name it.
    pub more_only: bool,
}

impl ActionSpec {
    fn variant(&self, key: SelectionKey) -> Option<&Variant> {
        self.variants.iter().find(|v| v.on.contains(&key))
    }

    /// Whether the action works on this selection.
    pub fn accepts(&self, key: SelectionKey) -> bool {
        self.inputs.contains(&key)
    }

    /// The label template for this selection.
    pub fn label_for(&self, key: SelectionKey) -> &str {
        self.variant(key).and_then(|v| v.label.as_deref()).unwrap_or(&self.label)
    }

    /// The requirements for this selection.
    pub fn requires_for(&self, key: SelectionKey) -> &[Requirement] {
        self.variant(key).and_then(|v| v.requires.as_deref()).unwrap_or(&self.requires)
    }

    /// The companion layer needed for this selection, if any.
    pub fn companion_for(&self, key: SelectionKey) -> Option<&CompanionSpec> {
        self.variant(key).and_then(|v| v.companion.as_ref()).or(self.companion.as_ref())
    }

    /// Whether the build phase and the developer flag let the action appear.
    pub fn shown(&self, phase: u8, show_assistant_actions: bool) -> bool {
        self.min_phase <= phase && (self.surface == Surface::Card || show_assistant_actions)
    }

    /// Case-insensitive match against the label (without tokens) and aliases.
    pub fn matches_search(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return false;
        }
        std::iter::once(&self.label).chain(self.aliases.iter()).any(|s| s.to_lowercase().contains(&q))
    }
}

/// What the assistant is told about an action it may call (Product tiers › Pro): its inputs,
/// what it needs, the decisions a plan fills in, and what it makes. Descriptions only; the
/// action runs locally.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActionDescription {
    pub id: String,
    pub label: String,
    pub inputs: Vec<SelectionKey>,
    pub needs: Vec<String>,
    pub companion: Option<String>,
    pub decisions: Vec<String>,
    pub output: OutputContract,
    pub method: Option<String>,
    pub preview: PreviewPolicy,
}

impl ActionSpec {
    /// The machine-readable description (serialize it as JSON for the assistant).
    pub fn describe(&self) -> ActionDescription {
        ActionDescription {
            id: self.id.clone(),
            label: self.label.clone(),
            inputs: self.inputs.clone(),
            needs: self.requires.iter().map(|r| r.reason.clone()).collect(),
            companion: self.companion.as_ref().map(|c| c.reason.clone()),
            decisions: self.decisions.clone(),
            output: self.output,
            method: self.method.clone(),
            preview: self.preview,
        }
    }
}

/// The spec table's "Other open layer" column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtherLayer {
    /// Matches only when no row naming a layer kind matched.
    None,
    /// Always matches ("—" and "any" in the spec).
    Any,
    Points,
    Lines,
    Areas,
    Raster,
}

impl OtherLayer {
    /// The companion kind a row needs, for rows that name one.
    pub fn kind(self) -> Option<CompanionKind> {
        match self {
            OtherLayer::None | OtherLayer::Any => None,
            OtherLayer::Points => Some(CompanionKind::Points),
            OtherLayer::Lines => Some(CompanionKind::Lines),
            OtherLayer::Areas => Some(CompanionKind::Areas),
            OtherLayer::Raster => Some(CompanionKind::Raster),
        }
    }
}

/// One row of the table: a selection, the other open layer, and the actions in order.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub n: u8,
    pub selected: Vec<SelectionKey>,
    pub other: OtherLayer,
    pub actions: Vec<String>,
}

/// Why `rules.toml` was rejected.
#[derive(Debug, thiserror::Error)]
pub enum RulesError {
    #[error("rules.toml does not parse: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("action {0:?} is defined twice")]
    DuplicateAction(String),
    #[error("action {id:?} has an empty {field}")]
    Empty { id: String, field: &'static str },
    #[error("analysis action {id:?} needs {field} (Phase 4)")]
    AnalysisContract { id: String, field: &'static str },
    #[error("action {id:?} has min_phase {min_phase}; phases run 0 to 7")]
    BadPhase { id: String, min_phase: u8 },
    #[error("row {row}: unknown action {action:?}")]
    UnknownAction { row: u8, action: String },
    #[error("row {row}: action {action:?} does not accept the selection {key:?}")]
    InputMismatch { row: u8, action: String, key: SelectionKey },
    #[error("rows must be numbered 1, 2, 3, … in order; expected {expected}, found {found}")]
    RowOrder { expected: u8, found: u8 },
    #[error("row {0} has no selection or no actions")]
    EmptyRow(u8),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRules {
    #[serde(default)]
    action: Vec<RawAction>,
    #[serde(default)]
    row: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAction {
    id: String,
    label: String,
    #[serde(default)]
    aliases: Vec<String>,
    group: Group,
    min_phase: u8,
    surface: Surface,
    #[serde(default)]
    more_only: bool,
    inputs: Vec<SelectionKey>,
    input_reason: String,
    #[serde(default)]
    requires: Vec<Requirement>,
    #[serde(default)]
    companion: Option<CompanionSpec>,
    #[serde(default)]
    variant: Vec<Variant>,
    #[serde(default)]
    output: Option<OutputContract>,
    #[serde(default)]
    algorithm: Option<String>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    decisions: Vec<String>,
}

impl From<RawAction> for ActionSpec {
    fn from(a: RawAction) -> Self {
        let (preview, output) = match a.surface {
            Surface::Card => (PreviewPolicy::NotNeeded, OutputContract::NoNewData),
            Surface::Assistant => (PreviewPolicy::Auto, a.output.unwrap_or(OutputContract::Card)),
        };
        ActionSpec {
            label_key: format!("action.{}", a.id),
            id: a.id,
            label: a.label,
            aliases: a.aliases,
            group: a.group,
            inputs: a.inputs,
            input_reason: a.input_reason,
            requires: a.requires,
            companion: a.companion,
            variants: a.variant,
            decisions: a.decisions,
            algorithm: a.algorithm,
            method: a.method,
            preview,
            output,
            min_phase: a.min_phase,
            surface: a.surface,
            more_only: a.more_only,
        }
    }
}

/// Every action and the table rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registry {
    actions: Vec<ActionSpec>,
    rows: Vec<Row>,
}

impl Registry {
    /// No actions and no rows: what the app falls back to if the rules file is broken.
    pub fn empty() -> Self {
        Registry::default()
    }

    /// Parses and checks a rules file.
    pub fn from_toml(text: &str) -> Result<Self, RulesError> {
        let raw: RawRules = toml::from_str(text)?;
        // Analysis actions state what they make, how, and the recipe's method sentence.
        for a in raw.action.iter().filter(|a| a.surface == Surface::Assistant) {
            for (field, missing) in [
                ("an output", a.output.is_none()),
                ("an algorithm", a.algorithm.is_none()),
                ("a method", a.method.as_deref().is_none_or(|m| m.trim().is_empty())),
            ] {
                if missing {
                    return Err(RulesError::AnalysisContract { id: a.id.clone(), field });
                }
            }
        }
        let actions: Vec<ActionSpec> = raw.action.into_iter().map(ActionSpec::from).collect();
        for (i, a) in actions.iter().enumerate() {
            if actions[..i].iter().any(|b| b.id == a.id) {
                return Err(RulesError::DuplicateAction(a.id.clone()));
            }
            for (field, empty) in [
                ("id", a.id.trim().is_empty()),
                ("label", a.label.trim().is_empty()),
                ("inputs list", a.inputs.is_empty()),
                ("input_reason", a.input_reason.trim().is_empty()),
                ("requirement reason", a.requires.iter().any(|r| r.reason.trim().is_empty())),
                ("companion reason", a.companion.as_ref().is_some_and(|c| c.reason.trim().is_empty())),
            ] {
                if empty {
                    return Err(RulesError::Empty { id: a.id.clone(), field });
                }
            }
            if a.min_phase > 7 {
                return Err(RulesError::BadPhase { id: a.id.clone(), min_phase: a.min_phase });
            }
        }
        for (i, row) in raw.row.iter().enumerate() {
            let expected = u8::try_from(i + 1).unwrap_or(u8::MAX);
            if row.n != expected {
                return Err(RulesError::RowOrder { expected, found: row.n });
            }
            if row.selected.is_empty() || row.actions.is_empty() {
                return Err(RulesError::EmptyRow(row.n));
            }
            for id in &row.actions {
                let Some(a) = actions.iter().find(|a| &a.id == id) else {
                    return Err(RulesError::UnknownAction { row: row.n, action: id.clone() });
                };
                if let Some(&key) = row.selected.iter().find(|k| !a.accepts(**k)) {
                    return Err(RulesError::InputMismatch { row: row.n, action: id.clone(), key });
                }
            }
        }
        Ok(Registry { actions, rows: raw.row })
    }

    /// All actions, in file order.
    pub fn actions(&self) -> &[ActionSpec] {
        &self.actions
    }

    /// The table rows, in order.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn action(&self, id: &str) -> Option<&ActionSpec> {
        self.actions.iter().find(|a| a.id == id)
    }

    /// Actions whose label or aliases contain `query` ("buffer" finds "Show an area within a
    /// distance"), in file order.
    pub fn search(&self, query: &str) -> Vec<&ActionSpec> {
        self.actions.iter().filter(|a| a.matches_search(query)).collect()
    }
}

/// The registry built from the compiled-in `rules.toml`, once. If the file were broken the app
/// logs it and offers no actions (a unit test guarantees it parses).
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| match Registry::from_toml(RULES_TOML) {
        Ok(r) => r,
        Err(e) => {
            log::error!("lm-actions: {e}; no actions will be offered");
            Registry::empty()
        }
    })
}

#[cfg(test)]
mod tests {

    /// Every analysis action states its output, its certified algorithm and its recipe method,
    /// and is previewed by cost (WP4.1, WP4.6).
    #[test]
    fn analysis_actions_state_their_contracts() {
        let r = registry();
        let analysis: Vec<&ActionSpec> = r.actions().iter().filter(|a| a.surface == Surface::Assistant).collect();
        assert!(analysis.len() >= 14, "{}", analysis.len());
        for a in &analysis {
            assert_eq!(a.preview, PreviewPolicy::Auto, "{}", a.id);
            assert!(a.algorithm.is_some() && a.method.is_some(), "{}", a.id);
            assert_ne!(a.output, OutputContract::NoNewData, "{}", a.id);
        }
        let count = r.action("count_in").map(ActionSpec::describe);
        let count = count.as_ref();
        assert_eq!(count.map(|d| d.output), Some(OutputContract::NewLayer));
        assert_eq!(count.map(|d| d.inputs.clone()), Some(vec![SelectionKey::PointLayer, SelectionKey::AreaLayer]));
        assert!(count.is_some_and(|d| d.method.as_deref().is_some_and(|m| m.contains("boundary"))));
    }

    #[test]
    fn an_analysis_action_without_a_method_is_refused() {
        let toml = r#"
[[action]]
id = "x"
label = "X"
group = "Measure"
min_phase = 4
surface = "assistant"
output = "card"
algorithm = "a::b"
inputs = ["one_point"]
input_reason = "Pick a point."
"#;
        assert!(matches!(Registry::from_toml(toml), Err(RulesError::AnalysisContract { field: "a method", .. })));
    }

    use super::*;

    #[test]
    fn rules_file_parses() {
        let r = Registry::from_toml(RULES_TOML);
        assert!(r.is_ok(), "{:?}", r.err());
        assert!(!registry().actions().is_empty());
        assert_eq!(registry().rows().len(), 18);
    }

    #[test]
    fn every_action_has_label_group_and_phase() {
        for a in registry().actions() {
            assert!(!a.label.trim().is_empty(), "{}", a.id);
            assert!(Group::ALL.contains(&a.group), "{}", a.id);
            assert!((2..=7).contains(&a.min_phase), "{} min_phase {}", a.id, a.min_phase);
            assert_eq!(a.label_key, format!("action.{}", a.id));
        }
    }

    #[test]
    fn every_action_is_used_by_a_row() {
        let r = registry();
        for a in r.actions() {
            assert!(a.more_only || r.rows().iter().any(|row| row.actions.contains(&a.id)), "{} is in no row", a.id);
        }
    }

    #[test]
    fn phases_and_surfaces_follow_the_tiers() {
        let r = registry();
        let phase = |id: &str| r.action(id).map(|a| (a.min_phase, a.surface));
        for id in ["style_by_value", "filter", "compare_peers", "raster_colors", "threshold", "similar_values"] {
            assert_eq!(phase(id), Some((3, Surface::Card)), "{id}");
        }
        assert_eq!(phase("open_file"), Some((2, Surface::Card)));
        assert_eq!(phase("try_example"), Some((2, Surface::Card)));
        assert_eq!(phase("browse_library"), Some((5, Surface::Card)));
        assert_eq!(phase("match_table"), Some((3, Surface::Card)));
        for a in r.actions() {
            if a.surface == Surface::Assistant {
                assert_eq!(a.min_phase, 4, "{}", a.id);
            }
        }
    }

    #[test]
    fn search_finds_technical_words() {
        let ids: Vec<&str> = registry().search("Buffer").iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["buffer"]);
        assert!(registry().search("  ").is_empty());
    }

    #[test]
    fn broken_rules_are_rejected() {
        assert!(matches!(Registry::from_toml("[[action]]\nid = 3"), Err(RulesError::Parse(_))));
        let unknown = "[[row]]\nn = 1\nselected = [\"empty\"]\nother = \"none\"\nactions = [\"nope\"]";
        assert!(matches!(Registry::from_toml(unknown), Err(RulesError::UnknownAction { .. })));
        let a = "[[action]]\nid = \"a\"\nlabel = \"A\"\ngroup = \"Keep\"\nmin_phase = 3\nsurface = \"card\"\ninputs = [\"empty\"]\ninput_reason = \"r\"\n";
        let twice = format!("{a}{a}");
        assert!(matches!(Registry::from_toml(&twice), Err(RulesError::DuplicateAction(_))));
        let mismatch = format!("{a}[[row]]\nn = 1\nselected = [\"one_point\"]\nother = \"none\"\nactions = [\"a\"]");
        assert!(matches!(Registry::from_toml(&mismatch), Err(RulesError::InputMismatch { .. })));
        let order = format!("{a}[[row]]\nn = 2\nselected = [\"empty\"]\nother = \"none\"\nactions = [\"a\"]");
        assert!(matches!(Registry::from_toml(&order), Err(RulesError::RowOrder { .. })));
        assert!(matches!(Registry::from_toml(""), Ok(r) if r == Registry::empty()));
    }
}
