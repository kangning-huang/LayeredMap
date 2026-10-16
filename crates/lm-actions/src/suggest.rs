//! Relevance (Insight cards and suggested actions › Relevance rules): match the table rows,
//! drop actions whose inputs, requirements or phase fail, merge two matching rows, and put the
//! rest under More actions. Pure and deterministic; recomputing only on change is the UI's job.

use std::cmp::Reverse;

use lm_core::model::{FieldRole, LayerId, LayerKind};

use crate::context::{Context, FieldSummary, OpenLayer};
use crate::registry::{ActionSpec, CompanionKind, Group, Need, Registry, Row, SelectionKey, registry};

/// Which actions may appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// The build phase: actions with a higher `min_phase` are not offered.
    pub phase: u8,
    /// The developer flag: also show assistant (analysis) actions, which are never buttons in
    /// either tier.
    pub show_assistant_actions: bool,
}

impl Options {
    /// The free app: display-only actions up to `phase`.
    pub fn free(phase: u8) -> Self {
        Options { phase, show_assistant_actions: false }
    }

    /// The developer surface: every action up to `phase`.
    pub fn developer(phase: u8) -> Self {
        Options { phase, show_assistant_actions: true }
    }
}

/// An action ready to show, with its tokens filled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggested {
    pub action_id: String,
    /// "Count Schools in each District".
    pub label: String,
    pub group: Group,
    /// The second layer, shown as a swappable token.
    pub companion: Option<LayerId>,
    /// The field the action starts with ([Type], the field to color by, the shared [code]).
    pub field: Option<String>,
}

/// Why an action is not offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ineligible {
    UnknownAction,
    /// A later phase's build offers it.
    NotYet {
        min_phase: u8,
    },
    /// An analysis action, and the developer flag is off.
    AssistantOnly,
    /// It does not work on this selection; the text says what to select.
    WrongSelection(String),
    /// A field or second layer is missing; the text says what to do.
    Missing(String),
}

impl Ineligible {
    /// In words, for a tooltip or the developer surface.
    pub fn message(&self) -> String {
        match self {
            Ineligible::UnknownAction => "This action does not exist.".to_string(),
            Ineligible::NotYet { min_phase } => format!("Arrives in Phase {min_phase}."),
            Ineligible::AssistantOnly => "Available through the assistant.".to_string(),
            Ineligible::WrongSelection(s) | Ineligible::Missing(s) => s.clone(),
        }
    }
}

/// An action that fits the selection but misses a field or layer, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unavailable {
    pub action_id: String,
    pub reason: Ineligible,
}

/// The actions for a selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Suggestions {
    /// Up to three, in the table's order (merged when two rows match).
    pub top: Vec<Suggested>,
    /// Everything else eligible, by group in `Group::ALL` order; empty groups left out.
    pub more: Vec<(Group, Vec<Suggested>)>,
    /// Actions that fit the selection and the phase but miss a field or layer.
    pub unavailable: Vec<Unavailable>,
    /// The table rows that matched, in merge order (for tests and the developer surface).
    pub rows: Vec<u8>,
}

/// Suggestions from the compiled-in rules.
pub fn suggest(ctx: &Context, opts: Options) -> Suggestions {
    registry().suggest(ctx, opts)
}

/// Whether one action can run on this context, from the compiled-in rules.
pub fn check(action_id: &str, ctx: &Context, opts: Options) -> Result<Suggested, Ineligible> {
    registry().check(action_id, ctx, opts)
}

/// A chosen companion layer, and the shared code column for `SharedCode`.
struct Companion<'c> {
    layer: &'c OpenLayer,
    code: Option<String>,
}

fn sort_key(l: &OpenLayer) -> (Reverse<u64>, usize, LayerId) {
    (Reverse(l.added_seq), l.shelf_index, l.id)
}

/// Choosing the companion layer: a co-selected layer of the right kind first, otherwise the most
/// recently added visible one; ties go to the higher layer on the shelf. Hidden layers never.
fn pick_companion(ctx: &Context, kind: CompanionKind) -> Option<Companion<'_>> {
    let selected = ctx.selected_layer();
    let selected_id = ctx.selection.layer();
    let candidates: Vec<Companion<'_>> = ctx
        .layers
        .iter()
        .filter(|l| l.visible && Some(l.id) != selected_id)
        .filter_map(|l| {
            let fits = match kind {
                CompanionKind::Points => l.kind == LayerKind::Points,
                CompanionKind::Lines => l.kind == LayerKind::Lines,
                CompanionKind::Areas => l.kind == LayerKind::Areas,
                CompanionKind::Raster => l.kind == LayerKind::Raster,
                CompanionKind::Any => l.kind != LayerKind::Table,
                CompanionKind::SharedCode => {
                    let code = selected.filter(|_| l.kind != LayerKind::Table).and_then(|s| shared_code(s, l));
                    return code.map(|code| Companion { layer: l, code: Some(code) });
                }
            };
            fits.then_some(Companion { layer: l, code: None })
        })
        .collect();
    let explicit = candidates.iter().any(|c| ctx.co_selected.contains(&c.layer.id));
    candidates.into_iter().filter(|c| !explicit || ctx.co_selected.contains(&c.layer.id)).min_by_key(|c| sort_key(c.layer))
}

/// The first identifier column of `table` that `layer` also has (names compared ignoring case).
fn shared_code(table: &OpenLayer, layer: &OpenLayer) -> Option<String> {
    let ids = |l: &OpenLayer| {
        l.fields.iter().filter(|f| f.role == FieldRole::Identifier).map(|f| f.name.trim().to_lowercase()).collect::<Vec<_>>()
    };
    let theirs = ids(layer);
    table
        .fields
        .iter()
        .filter(|f| f.role == FieldRole::Identifier)
        .find(|f| theirs.contains(&f.name.trim().to_lowercase()))
        .map(|f| f.name.clone())
}

/// The first field passing `pred`, starred fields first, then source order.
fn pick_field(fields: &[FieldSummary], pred: impl Fn(&FieldSummary) -> bool) -> Option<String> {
    let mut it = fields.iter().filter(|f| pred(f));
    let first = it.clone().find(|f| f.starred);
    first.or_else(|| it.next()).map(|f| f.name.clone())
}

/// Whether `layer` meets `need`: Some(field it picked, if any), or None.
fn meets(need: Need, layer: Option<&OpenLayer>) -> Option<Option<String>> {
    let fields = &layer?.fields;
    let measure = |f: &FieldSummary| matches!(f.role, FieldRole::Measure(_)) && f.is_sure();
    let category = |f: &FieldSummary| f.role == FieldRole::Category && f.is_sure();
    match need {
        Need::MeasureField => pick_field(fields, measure).map(Some),
        Need::CategoryField => pick_field(fields, category).map(Some),
        Need::StyleField => pick_field(fields, measure).or_else(|| pick_field(fields, category)).map(Some),
        Need::CoordinateFields => (fields.iter().filter(|f| f.role == FieldRole::Coordinate).count() >= 2).then_some(None),
    }
}

/// Values for the label tokens.
struct Tokens<'c> {
    layer: Option<&'c OpenLayer>,
    companion: Option<&'c OpenLayer>,
    field: Option<String>,
    code: Option<String>,
}

impl Tokens<'_> {
    fn of_kind(&self, kind: LayerKind) -> Option<&OpenLayer> {
        self.layer.filter(|l| l.kind == kind).or(self.companion.filter(|l| l.kind == kind))
    }

    /// Replaces the tokens it has values for; others stay as written.
    fn fill(&self, template: &str) -> String {
        if !template.contains('[') {
            return template.to_string();
        }
        let name = |l: Option<&OpenLayer>| l.map(|l| l.name.clone());
        let one = |l: Option<&OpenLayer>| l.map(OpenLayer::singular_name);
        let pairs = [
            ("[layer]", name(self.layer)),
            ("[other]", name(self.companion)),
            ("[Schools]", name(self.of_kind(LayerKind::Points))),
            ("[School]", one(self.of_kind(LayerKind::Points))),
            ("[Districts]", name(self.of_kind(LayerKind::Areas))),
            ("[District]", one(self.of_kind(LayerKind::Areas))),
            ("[Lines]", name(self.of_kind(LayerKind::Lines))),
            ("[Line]", one(self.of_kind(LayerKind::Lines))),
            ("[Heat]", name(self.of_kind(LayerKind::Raster))),
            ("[Type]", self.field.clone()),
            ("[field]", self.field.clone()),
            ("[code]", self.code.clone()),
        ];
        let mut out = template.to_string();
        for (token, value) in pairs {
            if let Some(v) = value {
                out = out.replace(token, &v);
            }
        }
        out
    }
}

impl Registry {
    /// Suggestions for a context: pure and deterministic.
    pub fn suggest(&self, ctx: &Context, opts: Options) -> Suggestions {
        let Some(key) = ctx.selection_key() else {
            return Suggestions::default();
        };
        let rows = self.matching_rows(ctx, key);
        let lists: Vec<Vec<Suggested>> = rows
            .iter()
            .map(|r| r.actions.iter().filter_map(|id| self.action(id)).filter_map(|a| self.evaluate(a, key, ctx, opts).ok()).collect())
            .collect();
        let merged = merge(&lists);
        let top: Vec<Suggested> = merged.iter().take(3).cloned().collect();

        let mut more: Vec<(usize, usize, Suggested)> = Vec::new();
        let mut unavailable = Vec::new();
        for (i, a) in self.actions().iter().enumerate() {
            if !a.accepts(key) || !a.shown(opts.phase, opts.show_assistant_actions) || top.iter().any(|s| s.action_id == a.id) {
                continue;
            }
            match self.evaluate(a, key, ctx, opts) {
                Ok(s) => {
                    let in_rows = merged.iter().position(|m| m.action_id == a.id).unwrap_or(usize::MAX);
                    more.push((in_rows, i, s));
                }
                Err(reason @ Ineligible::Missing(_)) => unavailable.push(Unavailable { action_id: a.id.clone(), reason }),
                Err(_) => {}
            }
        }
        more.sort_by_key(|(in_rows, i, _)| (*in_rows, *i));
        let more = Group::ALL
            .iter()
            .filter_map(|&g| {
                let items: Vec<Suggested> = more.iter().filter(|(_, _, s)| s.group == g).map(|(_, _, s)| s.clone()).collect();
                (!items.is_empty()).then_some((g, items))
            })
            .collect();
        Suggestions { top, more, unavailable, rows: rows.iter().map(|r| r.n).collect() }
    }

    /// Whether one action can run on this context, with its filled label or the reason it can't.
    pub fn check(&self, action_id: &str, ctx: &Context, opts: Options) -> Result<Suggested, Ineligible> {
        let a = self.action(action_id).ok_or(Ineligible::UnknownAction)?;
        match ctx.selection_key() {
            Some(key) => self.evaluate(a, key, ctx, opts),
            None => Err(Ineligible::WrongSelection(a.input_reason.clone())),
        }
    }

    /// The rows for this context, in merge order: rows naming a layer kind, by how recently
    /// their companion was added; then "any" rows; then "none" rows if no kind row matched. When
    /// a co-selected layer is the companion of some row, only such rows count.
    fn matching_rows(&self, ctx: &Context, key: SelectionKey) -> Vec<&Row> {
        let rows: Vec<&Row> = self.rows().iter().filter(|r| r.selected.contains(&key)).collect();
        let mut with_companion: Vec<(&Row, &OpenLayer)> =
            rows.iter().filter_map(|r| r.other.kind().and_then(|k| pick_companion(ctx, k)).map(|c| (*r, c.layer))).collect();
        if with_companion.iter().any(|(_, l)| ctx.co_selected.contains(&l.id)) {
            with_companion.retain(|(_, l)| ctx.co_selected.contains(&l.id));
        }
        with_companion.sort_by_key(|(r, l)| (sort_key(l), r.n));
        let none_rows = with_companion.is_empty();
        let mut out: Vec<&Row> = with_companion.into_iter().map(|(r, _)| r).collect();
        out.extend(rows.iter().filter(|r| r.other == crate::registry::OtherLayer::Any));
        if none_rows {
            out.extend(rows.iter().filter(|r| r.other == crate::registry::OtherLayer::None));
        }
        out
    }

    fn evaluate(&self, a: &ActionSpec, key: SelectionKey, ctx: &Context, opts: Options) -> Result<Suggested, Ineligible> {
        if a.min_phase > opts.phase {
            return Err(Ineligible::NotYet { min_phase: a.min_phase });
        }
        if !a.shown(opts.phase, opts.show_assistant_actions) {
            return Err(Ineligible::AssistantOnly);
        }
        let mut tokens = Tokens { layer: ctx.selected_layer(), companion: None, field: None, code: None };
        if !a.accepts(key) {
            return Err(Ineligible::WrongSelection(tokens.fill(&a.input_reason)));
        }
        for req in a.requires_for(key) {
            match meets(req.need, tokens.layer) {
                Some(field) => tokens.field = tokens.field.take().or(field),
                None => return Err(Ineligible::Missing(tokens.fill(&req.reason))),
            }
        }
        if let Some(c) = a.companion_for(key) {
            match pick_companion(ctx, c.kind) {
                Some(found) => {
                    tokens.companion = Some(found.layer);
                    tokens.code = found.code;
                }
                None => return Err(Ineligible::Missing(tokens.fill(&c.reason))),
            }
        }
        Ok(Suggested {
            action_id: a.id.clone(),
            label: tokens.fill(a.label_for(key)),
            group: a.group,
            companion: tokens.companion.map(|l| l.id),
            field: tokens.field.clone().or(tokens.code.clone()),
        })
    }
}

/// Merging rows (lists already filtered, in merge order): the first action of each row, then
/// the first action the first row shares with another, then the rest in row order. Duplicates
/// dropped. One row gives its own order.
fn merge(lists: &[Vec<Suggested>]) -> Vec<Suggested> {
    let mut out: Vec<Suggested> = Vec::new();
    let mut push = |s: &Suggested| {
        if !out.iter().any(|o| o.action_id == s.action_id) {
            out.push(s.clone());
        }
    };
    for l in lists {
        if let Some(s) = l.first() {
            push(s);
        }
    }
    if let Some((first, others)) = lists.split_first() {
        let shared = first.iter().find(|s| others.iter().any(|o| o.iter().any(|t| t.action_id == s.action_id)));
        if let Some(s) = shared {
            push(s);
        }
    }
    for l in lists {
        for s in l {
            push(s);
        }
    }
    out
}
