//! Running the certified actions (WP4.5) from a plan: the inputs as immutable snapshots, the
//! parameters, the scope. A run calls the certified algorithm in `lm-analysis`, then builds what
//! the interface keeps — a new layer or new fields, the result card and the recipe — without
//! touching the document. Committing (one undo step) is the workspace's job.
//!
//! Inputs come in a fixed order per action:
//!
//! | Action | Inputs |
//! |---|---|
//! | `count_in` | points, areas |
//! | `count_here` | points, the area (one row) |
//! | `which_container` | points (the selected rows), areas |
//! | `whats_inside`, `summarize_data_here` | the area (one row), the layer to look in |
//! | `buffer` | the layer (or its selected rows) |
//! | `measure_area`, `measure_length` | the layer (one row) |
//! | `summarize_each` | raster, areas |
//! | `summarize_here`, `summarize_drawn` | raster, the area (one row) |
//! | `add_raster_values` | raster, points |
//! | `rank_by_value` | the layer; parameter `field` |
//!
//! Parameters: `distance` ("1 km", "500 m"), `merge` ("yes"/"no"), `band` (1-based), `field`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use lm_analysis::control::Control;
use lm_analysis::{buffer, geodesic, overlay, zonal};
use lm_core::document::DocLayer;
use lm_core::model::{CellMeaning, Confidence, LayerId, LayerKind, group_thousands};
use lm_core::recipe::{Answer, RankRow, Recipe, RecipeAssumption, RecipeInput, ResultCard};
use rustgis_engine::data::{Dataset, Field, FieldType, Geometry, Value};
use rustgis_engine::geom::Coord;

use crate::import::{ImportedLayer, LayerData};

/// One input: the layer as it is now, its data, and the rows in scope.
#[derive(Clone)]
pub struct PlanInput {
    pub layer: DocLayer,
    pub data: LayerData,
    /// Rows in scope, ascending (None: all rows).
    pub rows: Option<Vec<usize>>,
    /// The source file, for its fingerprint (None for a drawn area).
    pub path: Option<PathBuf>,
}

impl PlanInput {
    fn name(&self) -> &str {
        &self.layer.display_name
    }

    fn vector(&self) -> Result<&Arc<Dataset>, ActionError> {
        match &self.data {
            LayerData::Vector(ds) => Ok(ds),
            LayerData::Raster(_) => Err(ActionError::Input(format!("{} is a raster; this needs points, lines or areas", self.name()))),
        }
    }

    /// The rows in scope as their own dataset, with the original row of each.
    fn scoped(&self) -> Result<(Arc<Dataset>, Vec<usize>), ActionError> {
        let ds = self.vector()?;
        match &self.rows {
            None => Ok((ds.clone(), (0..ds.features.len()).collect())),
            Some(rows) => {
                let mut sub = Dataset::new(ds.name.clone(), ds.geometry_type, ds.fields.clone());
                let mut kept = Vec::with_capacity(rows.len());
                for &r in rows {
                    if let Some(f) = ds.features.get(r) {
                        sub.features.push(f.clone());
                        kept.push(r);
                    }
                }
                Ok((Arc::new(sub), kept))
            }
        }
    }

    fn count(&self) -> u64 {
        match (&self.rows, &self.data) {
            (Some(r), _) => r.len() as u64,
            (None, LayerData::Vector(ds)) => ds.features.len() as u64,
            (None, LayerData::Raster(r)) => u64::from(r.width) * u64::from(r.height),
        }
    }

    fn nouns(&self) -> (&'static str, &'static str) {
        self.layer.profile.kind.nouns()
    }

    /// The layer's name as a noun when it reads as one ("populated_places" → "populated
    /// places"); None for names like "GUB_Global_2018", which then use the kind's noun.
    fn word(&self) -> Option<String> {
        let w = self.name().replace(['_', '-'], " ").to_lowercase();
        let w = w.split_whitespace().collect::<Vec<_>>().join(" ");
        let ok = !w.is_empty() && w.split(' ').count() <= 3 && w.chars().all(|c| c.is_alphabetic() || c == ' ');
        ok.then_some(w)
    }

    /// "districts", or "areas" for a name that isn't a noun.
    fn plural_noun(&self) -> String {
        self.word().map_or_else(|| self.nouns().1.to_string(), |w| plural(&w))
    }

    fn singular_noun(&self) -> String {
        self.word().map_or_else(|| self.nouns().0.to_string(), |w| singular(&w))
    }

    /// "each district", or "each area of GUB_Global_2018".
    fn each(&self) -> String {
        match self.word() {
            Some(w) => format!("each {}", singular(&w)),
            None => format!("each {} of {}", self.nouns().0, self.name()),
        }
    }

    /// "12 schools" in the layer's own words.
    fn things(&self, n: u64) -> String {
        if self.layer.profile.kind == LayerKind::Raster {
            return format!("{} cells", group_thousands(n));
        }
        if n == 1 { format!("1 {}", self.singular_noun()) } else { format!("{} {}", group_thousands(n), self.plural_noun()) }
    }

    fn title(&self, ds: &Dataset, row: usize) -> String {
        crate::inspect::object_title(&self.layer, ds, row)
    }
}

/// What to run.
#[derive(Clone)]
pub struct Plan {
    /// The action's id in `lm-actions/rules.toml` ("count_in").
    pub action: String,
    pub inputs: Vec<PlanInput>,
    pub params: BTreeMap<String, String>,
    /// The recipe's method line (the action's `method` in the registry).
    pub method: String,
    /// Seconds since the Unix epoch and "LayeredMap 0.4.0", for the recipe.
    pub computed_at: i64,
    pub software: String,
}

impl Plan {
    fn input(&self, i: usize) -> Result<&PlanInput, ActionError> {
        self.inputs.get(i).ok_or_else(|| ActionError::Input("an input is missing".into()))
    }

    fn param(&self, k: &str) -> Option<&str> {
        self.params.get(k).map(String::as_str)
    }
}

/// What a run gives back.
#[derive(Clone)]
pub struct ActionResult {
    /// The undo label ("Count schools in each district").
    pub label: String,
    pub card: ResultCard,
    pub recipe: Recipe,
    pub output: ActionOutput,
    /// Rows to select on the map while the result is shown.
    pub highlight: Option<(LayerId, Vec<usize>)>,
}

#[derive(Clone)]
pub enum ActionOutput {
    /// A card only.
    Card,
    /// A new layer, coloured by `color_by` when set.
    NewLayer { layer: Box<ImportedLayer>, color_by: Option<String> },
    /// The input layer with fields added (`fields`), replacing its data in one undo step.
    NewFields { layer: LayerId, data: Arc<Dataset>, fields: Vec<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActionError {
    Cancelled,
    /// The inputs or parameters don't fit the action; the message says what would.
    Input(String),
    Failed(String),
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionError::Cancelled => write!(f, "Cancelled"),
            ActionError::Input(s) | ActionError::Failed(s) => write!(f, "{s}"),
        }
    }
}

impl From<lm_analysis::control::Cancelled> for ActionError {
    fn from(_: lm_analysis::control::Cancelled) -> Self {
        ActionError::Cancelled
    }
}

impl From<buffer::BufferError> for ActionError {
    fn from(e: buffer::BufferError) -> Self {
        match e {
            buffer::BufferError::Cancelled => ActionError::Cancelled,
            other => ActionError::Input(other.to_string()),
        }
    }
}

impl From<zonal::ZonalError> for ActionError {
    fn from(e: zonal::ZonalError) -> Self {
        match e {
            zonal::ZonalError::Cancelled => ActionError::Cancelled,
            zonal::ZonalError::Input(m) => ActionError::Input(m),
            zonal::ZonalError::Read(m) => ActionError::Failed(format!("The raster couldn't be read: {m}")),
        }
    }
}

/// The actions this module runs.
pub const ACTIONS: &[&str] = &[
    "count_in",
    "count_here",
    "which_container",
    "whats_inside",
    "summarize_data_here",
    "buffer",
    "measure_area",
    "measure_length",
    "summarize_each",
    "summarize_here",
    "summarize_drawn",
    "add_raster_values",
    "rank_by_value",
];

/// Runs a plan. Checks `ctl` throughout; a cancelled run returns [`ActionError::Cancelled`] and
/// leaves nothing behind.
pub fn run(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    match plan.action.as_str() {
        "count_in" => count_in(plan, ctl),
        "count_here" => count_here(plan, ctl),
        "which_container" => which_container(plan, ctl),
        "whats_inside" | "summarize_data_here" => whats_inside(plan, ctl),
        "buffer" => buffer_action(plan, ctl),
        "measure_area" | "measure_length" => measure(plan),
        "summarize_each" => summarize_each(plan, ctl),
        "summarize_here" | "summarize_drawn" => summarize_one(plan, ctl),
        "add_raster_values" => add_raster_values(plan, ctl),
        "rank_by_value" => rank_by_value(plan, ctl),
        other => Err(ActionError::Input(format!("LayeredMap can't run “{other}” yet."))),
    }
}

// ---------------------------------------------------------------------------------------------
// Words

/// "districts" → "district" (enough for layer names; "Area 1" stays as is).
pub fn singular(name: &str) -> String {
    let n = name.trim();
    if let Some(s) = n.strip_suffix("ies").filter(|s| !s.is_empty()) {
        return format!("{s}y");
    }
    for end in ["sses", "shes", "ches", "xes"] {
        if n.ends_with(end) {
            return n[..n.len() - 2].to_string();
        }
    }
    if n.ends_with('s') && !n.ends_with("ss") && n.len() > 1 {
        return n[..n.len() - 1].to_string();
    }
    n.to_string()
}

/// "district" → "districts"; a name that is already plural stays.
pub fn plural(name: &str) -> String {
    let n = name.trim();
    if singular(n) != n {
        return n.to_string();
    }
    if n.chars().last().is_some_and(|c| c.is_ascii_digit()) || !n.chars().last().is_some_and(|c| c.is_ascii_alphabetic()) {
        return n.to_string();
    }
    if n.ends_with('y') && !n.ends_with("ay") && !n.ends_with("ey") && !n.ends_with("oy") {
        return format!("{}ies", &n[..n.len() - 1]);
    }
    if n.ends_with('s') || n.ends_with('x') || n.ends_with("ch") || n.ends_with("sh") {
        return format!("{n}es");
    }
    format!("{n}s")
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// A number for a card: thousands grouped, at most 3 significant decimals.
pub fn number(v: f64) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    let a = v.abs();
    let decimals = if a >= 100.0 || v.fract() == 0.0 {
        0
    } else if a >= 10.0 {
        1
    } else if a >= 1.0 {
        2
    } else {
        3
    };
    let s = format!("{:.*}", decimals, v);
    let (int, frac) = s.split_once('.').map_or((s.as_str(), None), |(i, f)| (i, Some(f)));
    let neg = int.starts_with('-');
    let digits = int.trim_start_matches('-');
    let grouped = digits.parse::<u64>().map(group_thousands).unwrap_or_else(|_| digits.to_string());
    let mut out = if neg { format!("-{grouped}") } else { grouped };
    if let Some(f) = frac {
        let f = f.trim_end_matches('0');
        if !f.is_empty() {
            out.push('.');
            out.push_str(f);
        }
    }
    out
}

/// An area in the unit a reader expects: m² under 0.01 km², km² otherwise.
fn area_words(m2: f64) -> (f64, &'static str) {
    if m2 < 10_000.0 { (m2, "m²") } else { (m2 / 1e6, "km²") }
}

fn length_words(m: f64) -> (f64, &'static str) {
    if m < 1_000.0 { (m, "m") } else { (m / 1e3, "km") }
}

// ---------------------------------------------------------------------------------------------
// Recipe parts

fn recipe_input(i: &PlanInput) -> RecipeInput {
    let fingerprint = i.path.as_deref().and_then(crate::fingerprint::of_path).unwrap_or_else(|| "drawn".into());
    let (one, many) = i.nouns();
    let n = i.count();
    RecipeInput {
        name: i.name().to_string(),
        file: i.layer.profile.source.file_name().to_string(),
        fingerprint,
        count: n,
        noun: if n == 1 { one.into() } else { many.into() },
    }
}

fn assumptions(i: &PlanInput) -> Vec<RecipeAssumption> {
    i.layer
        .profile
        .visible_assumptions()
        .map(|a| RecipeAssumption {
            text: format!("{}: {}", i.name(), lower_first(a.line.trim().trim_end_matches('.'))),
            confidence: if a.confirmed_by_user { Confidence::Certain } else { a.confidence },
        })
        .collect()
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        // Keep acronyms ("GPS …", "WGS 84") as written.
        Some(f) if c.clone().next().is_some_and(|n| n.is_lowercase()) => f.to_lowercase().collect::<String>() + c.as_str(),
        Some(f) => f.to_string() + c.as_str(),
        None => String::new(),
    }
}

fn exclusions(i: &PlanInput) -> Vec<String> {
    let n = i.layer.profile.exclusions.len() as u64;
    if n == 0 { Vec::new() } else { vec![format!("{} with no usable location", i.things(n))] }
}

fn scope_words(i: &PlanInput) -> String {
    if i.layer.profile.kind == LayerKind::Raster {
        return format!("all of {}", i.name());
    }
    let name = i.plural_noun();
    match &i.rows {
        None => format!("all {name}"),
        Some(r) if r.len() == 1 => match &i.data {
            LayerData::Vector(ds) => i.title(ds, r[0]),
            LayerData::Raster(_) => name,
        },
        Some(r) => format!("{} selected", i.things(r.len() as u64)),
    }
}

fn input_line(inputs: &[&PlanInput]) -> String {
    inputs
        .iter()
        .map(|i| match i.layer.profile.kind {
            LayerKind::Raster => i.name().to_string(),
            _ => format!("{}, {}", capitalized(i.name()), group_thousands(i.count())),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn recipe(plan: &Plan, title: &str, inputs: &[&PlanInput], coordinates: &str, decisions: Vec<String>, excluded: Vec<String>) -> Recipe {
    Recipe {
        title: title.to_string(),
        action: plan.action.clone(),
        params: plan.params.clone(),
        computed_at: plan.computed_at,
        software: plan.software.clone(),
        inputs: inputs.iter().map(|i| recipe_input(i)).collect(),
        scope: inputs.iter().map(|i| scope_words(i)).collect::<Vec<_>>().join("; "),
        method: plan.method.clone(),
        decisions,
        assumptions: inputs.iter().flat_map(|i| assumptions(i)).collect(),
        coordinates: (!coordinates.is_empty()).then(|| coordinates.to_string()),
        excluded,
    }
}

const PLANAR: &str = "longitude and latitude as planar coordinates for point-in-area tests (as GeoPandas does)";
const GEODESIC: &str = "geodesic, on the WGS 84 ellipsoid";
const CELL_AREAS: &str = "fractional cell coverage in the raster's coordinates; each cell weighted by its area on the WGS 84 ellipsoid";

fn context(inputs: &[&PlanInput], excluded: &[String]) -> Vec<String> {
    let mut v = vec![input_line(inputs), format!("Scope: {}", inputs.iter().map(|i| scope_words(i)).collect::<Vec<_>>().join("; "))];
    v.extend(excluded.iter().map(|e| format!("Excluded: {e}")));
    v
}

/// The first field name not taken, from `base` ("schools", "schools 2"…).
fn free_field(ds: &Dataset, base: &str) -> String {
    let base = base.trim();
    if ds.field_index(base).is_none() {
        return base.to_string();
    }
    (2..).map(|k| format!("{base} {k}")).find(|n| ds.field_index(n).is_none()).unwrap_or_else(|| base.to_string())
}

fn ranking(rows: Vec<RankRow>) -> Vec<RankRow> {
    let mut rows = rows;
    rows.sort_by(|a, b| match (a.value, b.value) {
        (Some(x), Some(y)) => y.total_cmp(&x).then(a.row.cmp(&b.row)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.row.cmp(&b.row),
    });
    rows
}

fn layer_from(name: &str, format: &str, ds: Dataset) -> Box<ImportedLayer> {
    Box::new(crate::import::derived_layer(name, format, ds))
}

// ---------------------------------------------------------------------------------------------
// Overlay

/// The statistics "Count [points] in each [area]" offers: the count, or a statistic of one of
/// the points' numeric fields (parameters `stat` and `field`).
pub const POINT_STATS: &[(&str, &str)] =
    &[("count", "Count"), ("sum", "Sum"), ("mean", "Mean"), ("median", "Median"), ("min", "Minimum"), ("max", "Maximum")];

/// The statistics a raster summary offers (parameter `stat`; mean by default).
pub const RASTER_STATS: &[(&str, &str)] =
    &[("mean", "Mean"), ("median", "Median"), ("min", "Minimum"), ("max", "Maximum"), ("sum", "Sum"), ("std", "Standard deviation")];

fn stat_label(stats: &[(&str, &str)], id: &str) -> String {
    stats.iter().find(|(k, _)| *k == id).map_or_else(|| id.to_string(), |(_, l)| l.to_string())
}

/// A statistic of plain values: the sum of none is 0; the others are None without values.
/// The median of an even count is the mean of the two middle values.
fn stat_of(stat: &str, vals: &mut [f64]) -> Option<f64> {
    if stat == "sum" {
        return Some(vals.iter().sum());
    }
    if vals.is_empty() {
        return None;
    }
    Some(match stat {
        "mean" => vals.iter().sum::<f64>() / vals.len() as f64,
        "min" => vals.iter().copied().fold(f64::INFINITY, f64::min),
        "max" => vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        _ => {
            vals.sort_by(f64::total_cmp);
            let n = vals.len();
            if n % 2 == 1 { vals[n / 2] } else { (vals[n / 2 - 1] + vals[n / 2]) / 2.0 }
        }
    })
}

/// "−3,841 to 4,738" when the range goes below zero (a dash would read as a minus), else
/// "0–34".
fn range_words(lo: f64, hi: f64) -> String {
    if lo < 0.0 { format!("{} to {}", number(lo), number(hi)) } else { format!("{}–{}", number(lo), number(hi)) }
}

fn count_in(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let (pts_in, areas_in) = (plan.input(0)?, plan.input(1)?);
    let (pts, _) = pts_in.scoped()?;
    let (areas, area_rows) = areas_in.scoped()?;
    let stat = plan.param("stat").unwrap_or("count");
    let value_field = plan.param("field").filter(|_| stat != "count");
    if stat != "count" && value_field.is_none() {
        return Err(ActionError::Input(format!(
            "Choose which value of the {} to take the {} of.",
            pts_in.plural_noun(),
            stat_label(POINT_STATS, stat).to_lowercase()
        )));
    }
    let r = overlay::count_points_in_areas(&pts, &areas, ctl)?;
    let things = pts_in.plural_noun();
    let area_one = areas_in.singular_noun();
    let each = areas_in.each();
    let mut out = (*areas).clone();
    let count_field = free_field(&out, &things);
    let k = out.ensure_field(&count_field, FieldType::Integer);
    for (f, c) in out.features.iter_mut().zip(&r.counts) {
        if let Some(v) = f.attrs.get_mut(k) {
            *v = Value::Int(*c as i64);
        }
    }
    let mut notices = Vec::new();
    // A statistic of the points' values: which area each point is in, then the values per area.
    let (title, values, color_by, unit, summary_what) = match value_field {
        None => {
            let values: Vec<Option<f64>> = r.counts.iter().map(|c| Some(*c as f64)).collect();
            (format!("{} in {each}", capitalized(&things)), values, count_field.clone(), things.clone(), things.clone())
        }
        Some(field) => {
            let vk = pts.field_index(field).ok_or_else(|| ActionError::Input(format!("{} has no field “{field}”.", pts_in.name())))?;
            let which = overlay::which_area(&pts, &areas, ctl)?;
            let mut per: Vec<Vec<f64>> = vec![Vec::new(); areas.features.len()];
            let mut blank = 0u64;
            for (p, inside) in which.areas.iter().enumerate() {
                if inside.is_empty() {
                    continue;
                }
                match pts.features.get(p).and_then(|f| f.attrs.get(vk)).and_then(Value::as_f64) {
                    Some(v) => inside.iter().for_each(|a| per[*a].push(v)),
                    None => blank += 1,
                }
            }
            if blank > 0 {
                notices.push(format!(
                    "{} have no {field} value and are left out of the {}.",
                    pts_in.things(blank),
                    stat_label(POINT_STATS, stat).to_lowercase()
                ));
            }
            let label = stat_label(POINT_STATS, stat);
            let name = free_field(&out, &format!("{} {field}", label.to_lowercase()));
            let kv = out.ensure_field(&name, FieldType::Double);
            let values: Vec<Option<f64>> = per.iter_mut().map(|v| stat_of(stat, v)).collect();
            for (f, v) in out.features.iter_mut().zip(&values) {
                if let Some(slot) = f.attrs.get_mut(kv) {
                    *slot = v.map_or(Value::Null, Value::Float);
                }
            }
            (
                format!("{label} {field} of the {things} in {each}"),
                values,
                name,
                field.to_string(),
                format!("{} {field}", label.to_lowercase()),
            )
        }
    };
    out.name = title.clone();
    let rows: Vec<RankRow> =
        values.iter().enumerate().map(|(i, v)| RankRow { label: areas_in.title(&areas, i), value: *v, row: area_rows[i] }).collect();
    let with: Vec<f64> = values.iter().flatten().copied().collect();
    let none = r.counts.iter().filter(|c| **c == 0).count() as u64;
    let range = match (with.iter().copied().reduce(f64::min), with.iter().copied().reduce(f64::max)) {
        (Some(lo), Some(hi)) => range_words(lo, hi),
        _ => "no values".into(),
    };
    let summary =
        format!("{} · {range} {summary_what} · {} with none", areas_in.things(areas.features.len() as u64), group_thousands(none));
    let mut decisions = Vec::new();
    if let Some(f) = value_field {
        decisions.push(format!("{} of {f} over the {things} in each {area_one}", stat_label(POINT_STATS, stat).to_lowercase()));
    }
    if r.counted_more_than_once > 0 {
        decisions.push(format!(
            "overlapping {} each count their points ({} counted more than once)",
            plural(&area_one),
            pts_in.things(r.counted_more_than_once)
        ));
        notices.push(format!(
            "{} {} counted in more than one {area_one}: overlapping {} each count their points.",
            pts_in.things(r.counted_more_than_once),
            if r.counted_more_than_once == 1 { "is" } else { "are" },
            plural(&area_one)
        ));
    }
    if r.outside_all > 0 {
        notices.push(format!("{} {} in no {area_one}.", pts_in.things(r.outside_all), if r.outside_all == 1 { "is" } else { "are" }));
    }
    let mut excluded = exclusions(pts_in);
    if r.points_without_geometry > 0 && excluded.is_empty() {
        excluded.push(format!("{} with no usable location", pts_in.things(r.points_without_geometry)));
    }
    let inputs = [pts_in, areas_in];
    Ok(ActionResult {
        label: match value_field {
            None => format!("Count {things} in {each}"),
            Some(f) => format!("{} {f} of {things} in {each}", stat_label(POINT_STATS, stat)),
        },
        card: ResultCard {
            title: title.clone(),
            summary,
            answer: Answer::Ranking { rows: ranking(rows), unit },
            context: context(&inputs, &excluded),
            notices,
        },
        recipe: recipe(plan, &title, &inputs, PLANAR, decisions, excluded),
        output: ActionOutput::NewLayer { layer: layer_from(&title, "Result", out), color_by: Some(color_by) },
        highlight: None,
    })
}

/// The single area of an "…here" action: the scoped row's geometry.
fn one_area(i: &PlanInput) -> Result<(Geometry, String), ActionError> {
    let (ds, rows) = i.scoped()?;
    let g = ds.features.first().and_then(|f| f.geometry.clone()).ok_or_else(|| ActionError::Input("Choose an area first.".into()))?;
    if !matches!(g, Geometry::Polygon(_)) {
        return Err(ActionError::Input("Choose an area: this works inside areas, not points or lines.".into()));
    }
    let full = i.vector()?;
    Ok((g, i.title(full, rows[0])))
}

fn count_here(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let (pts_in, area_in) = (plan.input(0)?, plan.input(1)?);
    let (g, area_name) = one_area(area_in)?;
    let (pts, pt_rows) = pts_in.scoped()?;
    let inside = overlay::whats_inside(&g, &pts, ctl)?;
    let n = inside.rows.len() as u64;
    let things = pts_in.plural_noun();
    let title = format!("{} in {area_name}", capitalized(&things));
    let excluded = exclusions(pts_in);
    let inputs = [pts_in, area_in];
    Ok(ActionResult {
        label: format!("Count {things} in {area_name}"),
        card: ResultCard {
            title: title.clone(),
            summary: format!("{} in {area_name}", pts_in.things(n)),
            answer: Answer::Number { value: n as f64, unit: things, detail: format!("inside or on the edge of {area_name}") },
            context: context(&inputs, &excluded),
            notices: Vec::new(),
        },
        recipe: recipe(plan, &title, &inputs, PLANAR, Vec::new(), excluded),
        output: ActionOutput::Card,
        highlight: Some((pts_in.layer.id, inside.rows.iter().map(|r| pt_rows[*r]).collect())),
    })
}

fn which_container(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let (pts_in, areas_in) = (plan.input(0)?, plan.input(1)?);
    let (pts, pt_rows) = pts_in.scoped()?;
    let (areas, area_rows) = areas_in.scoped()?;
    let r = overlay::which_area(&pts, &areas, ctl)?;
    let mut found: Vec<usize> = r.areas.iter().flatten().copied().collect();
    found.sort_unstable();
    found.dedup();
    let area_one = areas_in.singular_noun();
    let subject = if pt_rows.len() == 1 { pts_in.title(pts_in.vector()?, pt_rows[0]) } else { pts_in.things(pt_rows.len() as u64) };
    let title = format!("Which {area_one} {subject} is in");
    let heading = match found.len() {
        0 => format!("In no {area_one}"),
        1 => format!("In 1 {area_one}"),
        n => format!("In {n} {}", plural(&area_one)),
    };
    let items: Vec<String> = found.iter().map(|i| areas_in.title(&areas, *i)).collect();
    let mut notices = Vec::new();
    if pt_rows.len() == 1 && found.len() > 1 {
        notices.push(format!("It lies on a shared boundary, so every {area_one} it touches is listed."));
    }
    let inputs = [pts_in, areas_in];
    Ok(ActionResult {
        label: title.clone(),
        card: ResultCard {
            title: title.clone(),
            summary: if items.is_empty() { heading.clone() } else { format!("{heading}: {}", items.join(", ")) },
            answer: Answer::List { heading, items },
            context: context(&inputs, &[]),
            notices,
        },
        recipe: recipe(plan, &title, &inputs, PLANAR, Vec::new(), Vec::new()),
        output: ActionOutput::Card,
        highlight: Some((areas_in.layer.id, found.iter().map(|i| area_rows[*i]).collect())),
    })
}

fn whats_inside(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let (area_in, layer_in) = (plan.input(0)?, plan.input(1)?);
    let (g, area_name) = one_area(area_in)?;
    let (ds, rows) = layer_in.scoped()?;
    let r = overlay::whats_inside(&g, &ds, ctl)?;
    let n = r.rows.len() as u64;
    let title = format!("{} inside {area_name}", capitalized(&layer_in.plural_noun()));
    let full = layer_in.vector()?;
    let items: Vec<String> = r.rows.iter().take(50).map(|i| layer_in.title(full, rows[*i])).collect();
    let heading = format!("{} inside", capitalized(&layer_in.things(n)));
    let inputs = [area_in, layer_in];
    Ok(ActionResult {
        label: title.clone(),
        card: ResultCard {
            title: title.clone(),
            summary: format!("{} inside {area_name}", layer_in.things(n)),
            answer: Answer::List { heading, items },
            context: context(&inputs, &[]),
            notices: if n > 50 {
                vec![format!("The first 50 are listed; all {} are selected on the map.", group_thousands(n))]
            } else {
                Vec::new()
            },
        },
        recipe: recipe(plan, &title, &inputs, PLANAR, Vec::new(), Vec::new()),
        output: ActionOutput::Card,
        highlight: Some((layer_in.layer.id, r.rows.iter().map(|i| rows[*i]).collect())),
    })
}

// ---------------------------------------------------------------------------------------------
// Distance and measures

/// "1 km", "500 m", "0.5km", "2 mi" → metres and the words as typed.
pub fn parse_distance(s: &str) -> Option<(f64, String)> {
    let s = s.trim().to_lowercase().replace(',', "");
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let value: f64 = s[..split].trim().parse().ok()?;
    let unit = s[split..].trim();
    let (m, unit) = match unit {
        "" | "m" | "meter" | "meters" | "metre" | "metres" => (value, "m"),
        "km" | "kilometer" | "kilometers" | "kilometre" | "kilometres" => (value * 1000.0, "km"),
        "mi" | "mile" | "miles" => (value * 1609.344, "mi"),
        "ft" | "foot" | "feet" => (value * 0.3048, "ft"),
        _ => return None,
    };
    (m.is_finite() && m > 0.0).then(|| (m, format!("{} {unit}", number(value))))
}

fn buffer_action(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let input = plan.input(0)?;
    let (m, words) = parse_distance(plan.param("distance").unwrap_or("1 km"))
        .ok_or_else(|| ActionError::Input("Enter a distance such as 500 m or 1 km.".into()))?;
    let merge = plan.param("merge") != Some("no");
    let (ds, rows) = input.scoped()?;
    let geoms: Vec<Option<Geometry>> = ds.features.iter().map(|f| f.geometry.clone()).collect();
    let out = buffer::buffer_layer(&geoms, &buffer::BufferOptions { distance_m: m, merge }, ctl)?;
    let subject = if rows.len() == 1 { input.title(input.vector()?, rows[0]) } else { input.plural_noun() };
    let title = format!("Within {words} of {subject}");
    let mut layer = Dataset::new(title.clone(), rustgis_engine::data::GeometryType::Polygon, vec![Field::new("name", FieldType::Text)]);
    let mut total_m2 = 0.0;
    match out {
        buffer::BufferOutput::Merged(g) => {
            if let Some(g) = g {
                if let geodesic::Measure::Area(a) = geodesic::measure(&g) {
                    total_m2 = a.area_m2;
                }
                layer.push_next(Some(g), vec![Value::Text(title.clone())]);
            }
        }
        buffer::BufferOutput::PerFeature(gs) => {
            let full = input.vector()?;
            for (i, g) in gs.into_iter().enumerate() {
                if let Some(g) = g {
                    if let geodesic::Measure::Area(a) = geodesic::measure(&g) {
                        total_m2 += a.area_m2;
                    }
                    layer.push_next(Some(g), vec![Value::Text(input.title(full, rows[i]))]);
                }
            }
        }
    }
    if layer.features.is_empty() {
        return Err(ActionError::Input(format!("None of the {} has a location to measure from.", input.plural_noun())));
    }
    let (v, unit) = area_words(total_m2);
    let detail = if merge {
        format!(
            "within {words} in a straight line of {}{}",
            input.things(rows.len() as u64),
            if rows.len() > 1 { ", overlaps merged" } else { "" }
        )
    } else {
        format!("{} areas, one for each; overlaps counted in each", group_thousands(layer.features.len() as u64))
    };
    let decisions = vec![
        format!("distance {words}, straight-line (geodesic), not along roads"),
        if merge { "overlapping areas merged into one".to_string() } else { "one area for each feature".to_string() },
    ];
    let excluded = exclusions(input);
    let inputs = [input];
    Ok(ActionResult {
        label: format!("Show the area within {words}"),
        card: ResultCard {
            title: title.clone(),
            summary: format!("{} {unit} within {words}", number(v)),
            answer: Answer::Number { value: v, unit: unit.into(), detail },
            context: context(&inputs, &excluded),
            notices: vec!["Straight-line distance: not travel distance along roads or paths.".into()],
        },
        recipe: recipe(plan, &title, &inputs, GEODESIC, decisions, excluded),
        output: ActionOutput::NewLayer { layer: layer_from(&title, "Result", layer), color_by: None },
        highlight: None,
    })
}

fn measure(plan: &Plan) -> Result<ActionResult, ActionError> {
    let input = plan.input(0)?;
    let (ds, rows) = input.scoped()?;
    let full = input.vector()?;
    let name = rows.first().map(|r| input.title(full, *r)).unwrap_or_else(|| input.name().to_string());
    let mut area = 0.0;
    let mut perimeter = 0.0;
    let mut length = 0.0;
    for f in &ds.features {
        match f.geometry.as_ref().map(geodesic::measure) {
            Some(geodesic::Measure::Area(a)) => {
                area += a.area_m2;
                perimeter += a.perimeter_m;
            }
            Some(geodesic::Measure::Length(l)) => length += l,
            _ => {}
        }
    }
    let (title, value, unit, detail, summary) = if plan.action == "measure_area" {
        if area <= 0.0 {
            return Err(ActionError::Input("Choose an area to measure.".into()));
        }
        let (v, u) = area_words(area);
        let (p, pu) = length_words(perimeter);
        (format!("Area of {name}"), v, u, format!("perimeter {} {pu}", number(p)), format!("{} {u}", number(v)))
    } else {
        if length <= 0.0 {
            return Err(ActionError::Input("Choose a line to measure.".into()));
        }
        let (v, u) = length_words(length);
        (format!("Length of {name}"), v, u, "along the line, on the ellipsoid".to_string(), format!("{} {u}", number(v)))
    };
    let inputs = [input];
    Ok(ActionResult {
        label: title.clone(),
        card: ResultCard {
            title: title.clone(),
            summary,
            answer: Answer::Number { value, unit: unit.into(), detail },
            context: context(&inputs, &[]),
            notices: Vec::new(),
        },
        recipe: recipe(plan, &title, &inputs, GEODESIC, Vec::new(), Vec::new()),
        output: ActionOutput::Card,
        highlight: None,
    })
}

// ---------------------------------------------------------------------------------------------
// Rasters

struct Band<'a> {
    src: zonal::MemoryBand<'a>,
    role: zonal::ValueRole,
    unit: String,
    decisions: Vec<String>,
}

fn band<'a>(plan: &Plan, input: &'a PlanInput) -> Result<Band<'a>, ActionError> {
    let LayerData::Raster(raster) = &input.data else {
        return Err(ActionError::Input(format!("{} isn't a raster.", input.name())));
    };
    let index = plan.param("band").and_then(|b| b.parse::<usize>().ok()).unwrap_or(1).max(1) - 1;
    let profile = input.layer.profile.bands.get(index);
    let mut src = zonal::MemoryBand::new(raster, index)?;
    if let Some(p) = profile
        && (p.scale.is_some() || p.offset.is_some())
    {
        src = src.with_scale_offset(p.scale.unwrap_or(1.0), p.offset.unwrap_or(0.0));
    }
    let meaning = profile.map(|p| (p.cell_meaning.value, p.cell_meaning.confidence));
    // Totals only for a confirmed count raster (WP4.5); category codes give shares.
    let role = match meaning {
        Some((CellMeaning::Category, _)) => zonal::ValueRole::Category,
        Some((CellMeaning::Total, Confidence::Certain)) => zonal::ValueRole::Count,
        _ => zonal::ValueRole::Measured,
    };
    let unit = profile.and_then(|p| p.unit.value.clone()).unwrap_or_default();
    let mut decisions = Vec::new();
    if raster.bands.len() > 1 {
        decisions.push(format!("band {}", index + 1));
    }
    if let Some((w, h)) = raster.overview {
        decisions.push(format!(
            "computed on a reduced-resolution copy ({} × {} of {} × {} cells)",
            group_thousands(u64::from(raster.width)),
            group_thousands(u64::from(raster.height)),
            group_thousands(u64::from(w)),
            group_thousands(u64::from(h))
        ));
    }
    Ok(Band { src, role, unit, decisions })
}

fn summarize_each(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let (r_in, areas_in) = (plan.input(0)?, plan.input(1)?);
    let mut b = band(plan, r_in)?;
    let (areas, area_rows) = areas_in.scoped()?;
    let geoms: Vec<Geometry> = areas.features.iter().map(|f| f.geometry.clone().unwrap_or(Geometry::Polygon(Vec::new()))).collect();
    let sums = zonal::summarize(&mut b.src, &geoms, b.role, ctl)?;
    let rname = r_in.name().to_string();
    let area_one = areas_in.singular_noun();
    let each = areas_in.each();
    let stat = plan.param("stat").unwrap_or("mean");
    let stat_name = stat_label(RASTER_STATS, stat);
    let title = if b.role == zonal::ValueRole::Category || stat == "mean" {
        format!("{} in {each}", capitalized(&rname))
    } else {
        format!("{stat_name} {rname} in {each}")
    };
    let mut out = (*areas).clone();
    out.name = title.clone();
    let mut rows = Vec::with_capacity(sums.len());
    let color_by = if b.role == zonal::ValueRole::Category {
        // One share field per class present, largest overall first (at most 12).
        let mut classes: BTreeMap<i64, f64> = BTreeMap::new();
        for s in &sums {
            for c in &s.categories {
                *classes.entry(c.value.round() as i64).or_default() += c.area_m2;
            }
        }
        let mut order: Vec<(i64, f64)> = classes.into_iter().collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1));
        order.truncate(12);
        let mut first = None;
        for (class, _) in &order {
            let f = free_field(&out, &format!("{rname} {class} share"));
            let k = out.ensure_field(&f, FieldType::Double);
            for (feat, s) in out.features.iter_mut().zip(&sums) {
                let share = s.categories.iter().find(|c| c.value.round() as i64 == *class).map_or(0.0, |c| c.share * 100.0);
                if let Some(v) = feat.attrs.get_mut(k) {
                    *v = if s.valid_cells > 0 { Value::Float(share) } else { Value::Null };
                }
            }
            first.get_or_insert(f);
        }
        let top = order.first().map(|c| c.0);
        for (i, s) in sums.iter().enumerate() {
            let share = top.and_then(|t| s.categories.iter().find(|c| c.value.round() as i64 == t)).map(|c| c.share * 100.0);
            rows.push(RankRow {
                label: areas_in.title(&areas, i),
                value: if s.valid_cells > 0 { Some(share.unwrap_or(0.0)) } else { None },
                row: area_rows[i],
            });
        }
        first
    } else {
        let pick = |z: &zonal::ZonalSummary, id: &str| match id {
            "median" => z.median,
            "min" => z.min,
            "max" => z.max,
            "sum" => z.sum,
            "std" => z.std_dev,
            _ => z.mean,
        };
        let mut ks = Vec::new();
        for (id, label) in RASTER_STATS {
            let words = if *id == "std" { "std dev".to_string() } else { label.to_lowercase() };
            let f = free_field(&out, &format!("{rname} {words}"));
            ks.push((out.ensure_field(&f, FieldType::Double), f, *id));
        }
        let cov = free_field(&out, &format!("{rname} coverage %"));
        let kc = out.ensure_field(&cov, FieldType::Double);
        let kt = (b.role == zonal::ValueRole::Count).then(|| {
            let f = free_field(&out, &format!("{rname} total"));
            out.ensure_field(&f, FieldType::Double)
        });
        for (feat, s) in out.features.iter_mut().zip(&sums) {
            for (k, _, id) in &ks {
                feat.attrs[*k] = pick(s, id).map_or(Value::Null, Value::Float);
            }
            feat.attrs[kc] = Value::Float(s.coverage * 100.0);
            if let Some(kt) = kt {
                feat.attrs[kt] = s.total.map_or(Value::Null, |t| Value::Float(t.value));
            }
        }
        for (i, s) in sums.iter().enumerate() {
            rows.push(RankRow { label: areas_in.title(&areas, i), value: pick(s, stat), row: area_rows[i] });
        }
        ks.iter().find(|(_, _, id)| *id == stat).or(ks.first()).map(|(_, f, _)| f.clone())
    };
    let with: Vec<f64> = rows.iter().filter_map(|r| r.value).collect();
    let no_data = rows.len() - with.len();
    let range = match (with.iter().copied().reduce(f64::min), with.iter().copied().reduce(f64::max)) {
        (Some(lo), Some(hi)) => {
            format!("{}{}", range_words(lo, hi), if b.unit.is_empty() { String::new() } else { format!(" {}", b.unit) })
        }
        _ => "no data".into(),
    };
    let what = if b.role == zonal::ValueRole::Category { "share of the largest class".to_string() } else { stat_name.to_lowercase() };
    let summary =
        format!("{} · {what} {range} · {} with no data", areas_in.things(areas.features.len() as u64), group_thousands(no_data as u64));
    let partial = sums.iter().filter(|s| s.valid_cells > 0 && s.coverage < 0.995).count();
    let mut notices = Vec::new();
    if partial > 0 {
        notices.push(format!(
            "{} of the {} {} only partly covered by data; their coverage is in the table.",
            group_thousands(partial as u64),
            plural(&area_one),
            if partial == 1 { "is" } else { "are" }
        ));
    }
    // Areas smaller than one cell take their value from one or two cells (WP4.5: say so).
    let tiny = sums.iter().filter(|s| s.valid_cells > 0 && s.valid_fraction_sum < 1.0).count();
    if tiny > 0 {
        notices.push(format!(
            "{} of the {} {} smaller than one raster cell, so {} value comes from one or two cells; a finer raster would describe {} better.",
            group_thousands(tiny as u64),
            plural(&area_one),
            if tiny == 1 { "is" } else { "are" },
            if tiny == 1 { "its" } else { "their" },
            if tiny == 1 { "it" } else { "them" }
        ));
    }
    if stat == "sum" && b.role == zonal::ValueRole::Measured {
        notices.push(format!("A sum of {rname} values depends on the cell size and is rarely meaningful for a measured value; the mean is usually what you want."));
    }
    let mut decisions = b.decisions.clone();
    decisions.push(match (b.role, stat) {
        (zonal::ValueRole::Category, _) => "category codes give shares of each area, never an average".into(),
        (_, "sum") => "sum of cell values, each times the fraction of the cell inside the area (Σ x·f)".into(),
        (_, "median") => "the area-weighted median of the cells inside each area".into(),
        (_, "min" | "max") => format!("the {} value of any cell touching the inside of each area", stat_name.to_lowercase()),
        (_, "std") => "the area-weighted standard deviation of the cells inside each area".into(),
        (zonal::ValueRole::Count, _) => "totals assume each cell's count is spread evenly within the cell (N = Σ n·f)".into(),
        _ => "values are averaged, weighted by each cell's covered area; no total is given for a measured value".into(),
    });
    let inputs = [r_in, areas_in];
    Ok(ActionResult {
        label: if stat == "mean" { format!("Summarize {rname} in {each}") } else { format!("{stat_name} {rname} in {each}") },
        card: ResultCard {
            title: title.clone(),
            summary,
            answer: Answer::Ranking {
                rows: ranking(rows),
                unit: if b.role == zonal::ValueRole::Category { "%".into() } else { b.unit.clone() },
            },
            context: context(&inputs, &[]),
            notices,
        },
        recipe: recipe(plan, &title, &inputs, CELL_AREAS, decisions, Vec::new()),
        output: ActionOutput::NewLayer { layer: layer_from(&title, "Result", out), color_by },
        highlight: None,
    })
}

fn summarize_one(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let (r_in, area_in) = (plan.input(0)?, plan.input(1)?);
    let mut b = band(plan, r_in)?;
    let (g, area_name) = one_area(area_in)?;
    let s = zonal::summarize(&mut b.src, std::slice::from_ref(&g), b.role, ctl)?.into_iter().next().unwrap_or_default();
    let rname = r_in.name().to_string();
    let title = format!("{} in {area_name}", capitalized(&rname));
    let pct = (s.coverage * 100.0).round();
    let cover = if s.valid_cells == 0 {
        format!("no data covers {area_name}")
    } else if pct >= 99.5 {
        format!("over all of {area_name}")
    } else {
        format!("over {}% of {area_name}; the rest has no data", number(pct))
    };
    let (answer, summary) = if b.role == zonal::ValueRole::Category {
        let mut cats = s.categories.clone();
        cats.sort_by(|a, b| b.share.total_cmp(&a.share));
        let items: Vec<String> = cats.iter().take(10).map(|c| format!("{}: {}%", number(c.value), number(c.share * 100.0))).collect();
        let summary = items.first().cloned().unwrap_or_else(|| "no data".into());
        (Answer::List { heading: format!("Shares of {area_name}, {cover}"), items }, summary)
    } else {
        let stat = plan.param("stat").unwrap_or("mean");
        let (value, word) = match stat {
            "median" => (s.median, "median"),
            "min" => (s.min, "minimum"),
            "max" => (s.max, "maximum"),
            "sum" => (s.sum, "sum"),
            "std" => (s.std_dev, "standard deviation"),
            _ => (s.mean, "average"),
        };
        match value.filter(|_| s.mean.is_some()) {
            Some(m) => {
                let mut detail = format!("{word}, {cover}");
                if let (Some(lo), Some(hi)) = (s.min, s.max) {
                    detail.push_str(&format!(" · range {}", range_words(lo, hi)));
                }
                if let Some(t) = s.total {
                    detail.push_str(&format!(" · total {}", number(t.value)));
                }
                (Answer::Number { value: m, unit: b.unit.clone(), detail }, format!("{} {} {word}", number(m), b.unit))
            }
            None => (Answer::Number { value: f64::NAN, unit: b.unit.clone(), detail: cover.clone() }, "no data".into()),
        }
    };
    let inputs = [r_in, area_in];
    Ok(ActionResult {
        label: format!("Summarize {rname} in {area_name}"),
        card: ResultCard {
            title: title.clone(),
            summary: summary.trim().to_string(),
            answer,
            context: context(&inputs, &[]),
            notices: Vec::new(),
        },
        recipe: recipe(plan, &title, &inputs, CELL_AREAS, b.decisions.clone(), Vec::new()),
        output: ActionOutput::Card,
        highlight: None,
    })
}

fn add_raster_values(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let (r_in, pts_in) = (plan.input(0)?, plan.input(1)?);
    let mut b = band(plan, r_in)?;
    let full = pts_in.vector()?;
    let rows: Vec<usize> = pts_in.rows.clone().unwrap_or_else(|| (0..full.features.len()).collect());
    let coords: Vec<Coord> = rows
        .iter()
        .map(|r| match full.features.get(*r).and_then(|f| f.geometry.as_ref()) {
            Some(Geometry::Point(c)) => *c,
            Some(Geometry::MultiPoint(cs)) if !cs.is_empty() => cs[0],
            _ => Coord::new(f64::NAN, f64::NAN),
        })
        .collect();
    let values = zonal::sample_points(&mut b.src, &coords, ctl)?;
    let rname = r_in.name().to_string();
    let mut out = (**full).clone();
    let field = free_field(&out, &rname);
    let k = out.ensure_field(&field, FieldType::Double);
    for (r, v) in rows.iter().zip(&values) {
        if let Some(f) = out.features.get_mut(*r) {
            f.attrs[k] = v.map_or(Value::Null, Value::Float);
        }
    }
    let blank = values.iter().filter(|v| v.is_none()).count() as u64;
    let things = pts_in.plural_noun();
    let title = format!("{} values for each {}", capitalized(&rname), singular(&things));
    let with: Vec<f64> = values.iter().flatten().copied().collect();
    let range = match (with.iter().copied().reduce(f64::min), with.iter().copied().reduce(f64::max)) {
        (Some(lo), Some(hi)) => format!("{}–{}", number(lo), number(hi)),
        _ => "no values".into(),
    };
    let mut notices = Vec::new();
    if blank > 0 {
        notices.push(format!("{} got no value (outside the raster or on no-data cells); they stay blank, not zero.", pts_in.things(blank)));
    }
    let inputs = [r_in, pts_in];
    Ok(ActionResult {
        label: format!("Add {rname} values to {things}"),
        card: ResultCard {
            title: title.clone(),
            summary: format!("{} · {range} · {} blank", pts_in.things(rows.len() as u64), group_thousands(blank)),
            answer: Answer::Number {
                value: with.len() as f64,
                unit: things.clone(),
                detail: format!("now have a “{field}” value ({range})"),
            },
            context: context(&inputs, &[]),
            notices,
        },
        recipe: recipe(
            plan,
            &title,
            &inputs,
            "the cell containing each point, in the raster's coordinates; no interpolation",
            b.decisions.clone(),
            Vec::new(),
        ),
        output: ActionOutput::NewFields { layer: pts_in.layer.id, data: Arc::new(out), fields: vec![field] },
        highlight: None,
    })
}

fn rank_by_value(plan: &Plan, ctl: &Control) -> Result<ActionResult, ActionError> {
    let input = plan.input(0)?;
    let field = plan.param("field").ok_or_else(|| ActionError::Input("Choose a value to rank by.".into()))?;
    let (ds, rows) = input.scoped()?;
    let k = ds.field_index(field).ok_or_else(|| ActionError::Input(format!("{} has no field “{field}”.", input.name())))?;
    let full = input.vector()?;
    let mut ranked = Vec::with_capacity(rows.len());
    for (i, f) in ds.features.iter().enumerate() {
        if i % 4096 == 0 {
            ctl.check()?;
        }
        ranked.push(RankRow { label: input.title(full, rows[i]), value: f.attrs.get(k).and_then(Value::as_f64), row: rows[i] });
    }
    let title = format!("{} ranked by {field}", capitalized(&input.plural_noun()));
    let with = ranked.iter().filter(|r| r.value.is_some()).count();
    let inputs = [input];
    let ranked = ranking(ranked);
    let summary = match ranked.first() {
        Some(RankRow { label, value: Some(v), .. }) => format!("Highest: {label}, {}", number(*v)),
        _ => "No values to rank".into(),
    };
    let mut notices = Vec::new();
    if with < ranked.len() {
        notices.push(format!("{} have no {field} value and are listed last.", input.things((ranked.len() - with) as u64)));
    }
    Ok(ActionResult {
        label: title.clone(),
        card: ResultCard {
            title: title.clone(),
            summary,
            answer: Answer::Ranking { rows: ranked, unit: field.to_string() },
            context: context(&inputs, &[]),
            notices,
        },
        recipe: recipe(plan, &title, &inputs, "", Vec::new(), Vec::new()),
        output: ActionOutput::Card,
        highlight: None,
    })
}

#[cfg(test)]
mod tests;
