//! Document state (Architecture › State model): the saved store. Layers with their source
//! references, profiles and styles, the saved view, and the layer-id counter.
//!
//! Every change goes through a [`Transaction`] of reversible [`Op`]s, applied with
//! [`Document::apply`]; the undo stack (`crate::undo`) records transactions, so one user action
//! is one undo step. Until Gate 5 a phase may only add fields, so fields added after v0 take
//! `#[serde(default)]`.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::filter::Filter;
use crate::model::{AssumptionKind, Choice, FieldProfile, FieldRole, Known, LayerId, LayerKind, LayerProfile, Subject};

/// The document format version written by this build. A newer number is refused on load.
pub const DOCUMENT_VERSION: u32 = 0;

/// A colour, red–green–blue–alpha, 0–255 each.
pub type Rgba = [u8; 4];

/// The categorical palette (Visual design system › Data palettes): Okabe–Ito, in the order new
/// layers receive them. Blue first; yellow and black last because they read poorly as a first
/// colour on the light and dark basemaps.
pub const OKABE_ITO: [Rgba; 8] = [
    [0x00, 0x72, 0xB2, 0xFF], // blue
    [0xE6, 0x9F, 0x00, 0xFF], // orange
    [0x00, 0x9E, 0x73, 0xFF], // bluish green
    [0xD5, 0x5E, 0x00, 0xFF], // vermillion
    [0xCC, 0x79, 0xA7, 0xFF], // reddish purple
    [0x56, 0xB4, 0xE9, 0xFF], // sky blue
    [0xF0, 0xE4, 0x42, 0xFF], // yellow
    [0x00, 0x00, 0x00, 0xFF], // black
];

/// White, for point edges.
pub const WHITE: Rgba = [0xFF, 0xFF, 0xFF, 0xFF];

/// Above this many points, points draw at 4 px with no edge (Default symbols).
pub const DENSE_POINTS: u64 = 10_000;

/// How colours spread over a field's values (Style panel › Classes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Classes {
    /// Continuous: each value takes its own colour along the palette.
    Smooth,
    /// Classed into `count` groups by quantiles (the default "Groups"), or equal steps.
    Groups { count: u8, equal_steps: bool },
}

/// Colour by a field (WP3.5): by value along a sequential palette, or by category.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorBy {
    pub field: String,
    /// Categories (Okabe–Ito, 7 largest plus Other) rather than a range of values.
    pub categories: bool,
    pub classes: Classes,
    /// Sequential palette for values: "viridis", "cividis", "heat", "blues".
    pub palette: String,
    /// Divide by the object's geodesic area in km² (the count guard's density field).
    #[serde(default)]
    pub per_km2: bool,
    /// The range comes from the objects in view rather than the whole layer (legend shows an
    /// indicator while on).
    #[serde(default)]
    pub fit_to_view: bool,
}

/// Size points by a field (WP3.5): circle area proportional to the value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SizeBy {
    pub field: String,
    /// Diameter in px of the largest value.
    pub max_size: f32,
}

/// Labels on the map from a field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelStyle {
    pub field: String,
    /// Text size in px.
    pub size: f32,
}

/// How a layer is drawn (Visual design system › Default symbols; WP3.5 Style panel).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    /// Point and area fill (lines use `stroke`).
    pub fill: Rgba,
    /// Line colour, point edge, area outline.
    pub stroke: Rgba,
    /// Point edge or area outline width, in px. 0 means no edge.
    pub stroke_width: f32,
    /// Whole-layer opacity, 0–1.
    pub opacity: f32,
    /// Fill opacity for areas, 0–1 (35% by default).
    pub fill_opacity: f32,
    /// Point diameter in px.
    pub point_size: f32,
    /// Line width in px.
    pub line_width: f32,
    /// Raster colour ramp name ("viridis", "cividis", "heat", "blues").
    pub ramp: String,
    /// Raster stretch (min, max) in cell values; None stretches to the band's range.
    #[serde(default)]
    pub stretch: Option<(f64, f64)>,
    /// Colour by a field (vector layers).
    #[serde(default)]
    pub color_by: Option<ColorBy>,
    /// Size points by a field.
    #[serde(default)]
    pub size_by: Option<SizeBy>,
    #[serde(default)]
    pub labels: Option<LabelStyle>,
    /// Raster band shown, 1-based (0 means the first).
    #[serde(default)]
    pub band: u32,
    /// "Show only values above…": raster cells at or below this are not drawn.
    #[serde(default)]
    pub above: Option<f64>,
    /// "Show where values are like this": raster cells inside this range are highlighted.
    #[serde(default)]
    pub like: Option<(f64, f64)>,
}

impl Default for Style {
    fn default() -> Self {
        Style::for_kind(LayerKind::Points, OKABE_ITO[0], 0)
    }
}

impl Style {
    /// The default symbol for a layer kind (Visual design system › Default symbols), in `color`.
    /// `count` decides the dense-points symbol.
    pub fn for_kind(kind: LayerKind, color: Rgba, count: u64) -> Style {
        let base = Style {
            fill: color,
            stroke: color,
            stroke_width: 0.0,
            opacity: 1.0,
            fill_opacity: 1.0,
            point_size: 7.0,
            line_width: 2.0,
            ramp: "viridis".into(),
            stretch: None,
            color_by: None,
            size_by: None,
            labels: None,
            band: 0,
            above: None,
            like: None,
        };
        match kind {
            LayerKind::Points | LayerKind::Table => {
                if count > DENSE_POINTS {
                    Style { point_size: 4.0, stroke_width: 0.0, ..base }
                } else {
                    Style { stroke: WHITE, stroke_width: 1.5, ..base }
                }
            }
            LayerKind::Lines => base,
            LayerKind::Areas => Style { stroke: darker(color), stroke_width: 1.0, fill_opacity: 0.35, ..base },
            LayerKind::Raster => base,
        }
    }

    /// The default style for a layer from its profile: the kind's symbol, and for rasters the
    /// first band's range as the stretch.
    pub fn for_profile(profile: &LayerProfile, color: Rgba) -> Style {
        let mut s = Style::for_kind(profile.kind, color, profile.count);
        if profile.kind == LayerKind::Raster
            && let Some(b) = profile.bands.first()
            && let (Some(lo), Some(hi)) = (b.min, b.max)
            && lo.is_finite()
            && hi.is_finite()
            && lo < hi
        {
            s.stretch = Some((lo, hi));
        }
        s
    }

    /// The colour that identifies the layer in the palette (lines: stroke; others: fill).
    pub fn key_color(&self, kind: LayerKind) -> Rgba {
        if kind == LayerKind::Lines { self.stroke } else { self.fill }
    }
}

/// A darker shade of a colour (area outlines): 70% of each channel.
pub fn darker(c: Rgba) -> Rgba {
    let d = |v: u8| ((f32::from(v) * 0.7).round()) as u8;
    [d(c[0]), d(c[1]), d(c[2]), c[3]]
}

/// A layer in the document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocLayer {
    pub id: LayerId,
    pub display_name: String,
    pub visible: bool,
    pub profile: LayerProfile,
    pub style: Style,
    /// A display filter: only matching records are drawn and counted in insight (WP3.7).
    #[serde(default)]
    pub filter: Option<Filter>,
    /// Field roles the user changed (WP3.3), by field name. They win over the inferred role and
    /// survive re-reads of the source.
    #[serde(default)]
    pub roles: BTreeMap<String, FieldRole>,
    /// Fields the user starred ("Show in insight"), in the order starred.
    #[serde(default)]
    pub starred: Vec<String>,
}

impl DocLayer {
    pub fn new(id: LayerId, display_name: String, profile: LayerProfile, style: Style) -> DocLayer {
        DocLayer { id, display_name, visible: true, profile, style, filter: None, roles: BTreeMap::new(), starred: Vec::new() }
    }

    /// A field's role, with the user's choice applied (certain, "Set by you").
    pub fn role(&self, field: &FieldProfile) -> Known<FieldRole> {
        match self.roles.get(&field.name) {
            Some(r) => Known::certain(*r, "Set by you."),
            None => field.role.clone(),
        }
    }

    /// The field profile with this name.
    pub fn field(&self, name: &str) -> Option<&FieldProfile> {
        self.profile.fields.iter().find(|f| f.name == name)
    }
}

/// The saved document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// The format version (see [`DOCUMENT_VERSION`]).
    pub version: u32,
    /// Bottom-to-top draw order.
    pub layers: Vec<DocLayer>,
    /// The saved view: (center longitude, center latitude, zoom).
    #[serde(default)]
    pub view: Option<(f64, f64, f64)>,
    /// The next layer id to hand out. Ids are never reused within a document.
    pub next_layer_id: u64,
    /// True once the user has reordered layers (WP2.7): new layers then go on top and nothing
    /// is re-sorted.
    #[serde(default)]
    pub user_ordered: bool,
}

impl Default for Document {
    fn default() -> Self {
        Document { version: DOCUMENT_VERSION, layers: Vec::new(), view: None, next_layer_id: 1, user_ordered: false }
    }
}

/// One reversible change. Each op carries what it needs to be undone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Op {
    AddLayer {
        index: usize,
        layer: Box<DocLayer>,
    },
    RemoveLayer {
        index: usize,
        layer: Box<DocLayer>,
    },
    Rename {
        id: LayerId,
        from: String,
        to: String,
    },
    SetVisible {
        id: LayerId,
        from: bool,
        to: bool,
    },
    /// Remove the layer at `from_index`, then insert it at `to_index` (in the shortened list).
    Move {
        from_index: usize,
        to_index: usize,
    },
    SetUserOrdered {
        from: bool,
        to: bool,
    },
    /// Change the chosen option of the assumption with this kind and subject.
    SetAssumption {
        id: LayerId,
        kind: AssumptionKind,
        subject: Subject,
        from: Choice,
        to: Choice,
        #[serde(default)]
        from_confirmed: bool,
        #[serde(default)]
        to_confirmed: bool,
    },
    SetStyle {
        id: LayerId,
        from: Box<Style>,
        to: Box<Style>,
    },
    SetProfile {
        id: LayerId,
        from: Box<LayerProfile>,
        to: Box<LayerProfile>,
    },
    SetFilter {
        id: LayerId,
        from: Option<Filter>,
        to: Option<Filter>,
    },
    /// A field's role set by the user (`None`: back to the inferred role).
    SetRole {
        id: LayerId,
        field: String,
        from: Option<FieldRole>,
        to: Option<FieldRole>,
    },
    /// Star or unstar a field; `from`/`to` are its position in the starred list (None: not
    /// starred), so undo puts it back where it was.
    SetStarred {
        id: LayerId,
        field: String,
        from: Option<usize>,
        to: Option<usize>,
    },
}

impl Op {
    /// The op that undoes this one.
    pub fn inverse(&self) -> Op {
        match self {
            Op::AddLayer { index, layer } => Op::RemoveLayer { index: *index, layer: layer.clone() },
            Op::RemoveLayer { index, layer } => Op::AddLayer { index: *index, layer: layer.clone() },
            Op::Rename { id, from, to } => Op::Rename { id: *id, from: to.clone(), to: from.clone() },
            Op::SetVisible { id, from, to } => Op::SetVisible { id: *id, from: *to, to: *from },
            Op::Move { from_index, to_index } => Op::Move { from_index: *to_index, to_index: *from_index },
            Op::SetUserOrdered { from, to } => Op::SetUserOrdered { from: *to, to: *from },
            Op::SetAssumption { id, kind, subject, from, to, from_confirmed, to_confirmed } => Op::SetAssumption {
                id: *id,
                kind: *kind,
                subject: subject.clone(),
                from: to.clone(),
                to: from.clone(),
                from_confirmed: *to_confirmed,
                to_confirmed: *from_confirmed,
            },
            Op::SetStyle { id, from, to } => Op::SetStyle { id: *id, from: to.clone(), to: from.clone() },
            Op::SetProfile { id, from, to } => Op::SetProfile { id: *id, from: to.clone(), to: from.clone() },
            Op::SetFilter { id, from, to } => Op::SetFilter { id: *id, from: to.clone(), to: from.clone() },
            Op::SetRole { id, field, from, to } => Op::SetRole { id: *id, field: field.clone(), from: *to, to: *from },
            Op::SetStarred { id, field, from, to } => Op::SetStarred { id: *id, field: field.clone(), from: *to, to: *from },
        }
    }
}

/// One user action: a label for the Undo tooltip ("Rename layer") and its ops, applied in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub label: String,
    pub ops: Vec<Op>,
}

impl Transaction {
    pub fn new(label: impl Into<String>, ops: Vec<Op>) -> Self {
        Transaction { label: label.into(), ops }
    }

    /// The transaction that undoes this one: inverse ops in reverse order, same label.
    pub fn inverse(&self) -> Transaction {
        Transaction { label: self.label.clone(), ops: self.ops.iter().rev().map(Op::inverse).collect() }
    }
}

/// Why a transaction could not be applied. The document is left unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyError {
    NoSuchLayer(LayerId),
    IndexOutOfRange {
        index: usize,
        len: usize,
    },
    DuplicateLayer(LayerId),
    /// RemoveLayer found a different layer at its index.
    LayerMismatch {
        index: usize,
        expected: LayerId,
    },
    NoSuchAssumption {
        id: LayerId,
        kind: AssumptionKind,
    },
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::NoSuchLayer(id) => write!(f, "layer {} is not in the document", id.0),
            ApplyError::IndexOutOfRange { index, len } => {
                write!(f, "layer position {index} is outside the list of {len} layers")
            }
            ApplyError::DuplicateLayer(id) => write!(f, "layer {} is already in the document", id.0),
            ApplyError::LayerMismatch { index, expected } => {
                write!(f, "expected layer {} at position {index}", expected.0)
            }
            ApplyError::NoSuchAssumption { id, kind } => {
                write!(f, "layer {} has no assumption {kind:?}", id.0)
            }
        }
    }
}

impl std::error::Error for ApplyError {}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    /// Position of a layer in draw order.
    pub fn index_of(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn layer(&self, id: LayerId) -> Option<&DocLayer> {
        self.layers.iter().find(|l| l.id == id)
    }

    fn layer_mut(&mut self, id: LayerId) -> Result<&mut DocLayer, ApplyError> {
        self.layers.iter_mut().find(|l| l.id == id).ok_or(ApplyError::NoSuchLayer(id))
    }

    /// Hands out a fresh layer id. Not an undoable change: ids are never reused, even after the
    /// layer's addition is undone.
    pub fn allocate_layer_id(&mut self) -> LayerId {
        let id = LayerId(self.next_layer_id.max(1));
        self.next_layer_id = id.0.saturating_add(1);
        id
    }

    /// Where a new layer of this kind goes (WP2.7). Until the user reorders, by
    /// [`LayerKind::stack_rank`]: on top of the layers of its rank or lower, below higher ranks.
    /// After the first reorder, on top.
    pub fn insert_index(&self, kind: LayerKind) -> usize {
        if self.user_ordered {
            return self.layers.len();
        }
        let rank = kind.stack_rank();
        self.layers.iter().position(|l| l.profile.kind.stack_rank() > rank).unwrap_or(self.layers.len())
    }

    /// The next palette colour: the first Okabe–Ito colour no layer uses yet, else cycling.
    pub fn next_color(&self) -> Rgba {
        let used: Vec<Rgba> =
            self.layers.iter().filter(|l| l.profile.kind != LayerKind::Raster).map(|l| l.style.key_color(l.profile.kind)).collect();
        OKABE_ITO
            .iter()
            .copied()
            .find(|c| !used.contains(c))
            .unwrap_or_else(|| OKABE_ITO.get(used.len() % OKABE_ITO.len()).copied().unwrap_or(OKABE_ITO[0]))
    }

    /// A transaction adding a layer for `profile`: a fresh id, the file name as display name,
    /// the default style in the next palette colour, placed by the stacking rule.
    pub fn add_layer_tx(&mut self, profile: LayerProfile) -> Transaction {
        let id = self.allocate_layer_id();
        let name = profile.source.file_name();
        let display_name = match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => name.to_string(),
        };
        let style = Style::for_profile(&profile, self.next_color());
        let index = self.insert_index(profile.kind);
        let layer = DocLayer::new(id, display_name, profile, style);
        Transaction::new("Add layer", vec![Op::AddLayer { index, layer: Box::new(layer) }])
    }

    pub fn remove_layer_tx(&self, id: LayerId) -> Option<Transaction> {
        let index = self.index_of(id)?;
        let layer = self.layers.get(index)?.clone();
        Some(Transaction::new("Remove layer", vec![Op::RemoveLayer { index, layer: Box::new(layer) }]))
    }

    pub fn rename_tx(&self, id: LayerId, to: impl Into<String>) -> Option<Transaction> {
        let from = self.layer(id)?.display_name.clone();
        Some(Transaction::new("Rename layer", vec![Op::Rename { id, from, to: to.into() }]))
    }

    pub fn set_visible_tx(&self, id: LayerId, to: bool) -> Option<Transaction> {
        let from = self.layer(id)?.visible;
        let label = if to { "Show layer" } else { "Hide layer" };
        Some(Transaction::new(label, vec![Op::SetVisible { id, from, to }]))
    }

    /// A user reorder: moves the layer and marks the document user-ordered (both undone together).
    pub fn move_tx(&self, from_index: usize, to_index: usize) -> Option<Transaction> {
        if from_index >= self.layers.len() || to_index >= self.layers.len() {
            return None;
        }
        let mut ops = Vec::with_capacity(2);
        if !self.user_ordered {
            ops.push(Op::SetUserOrdered { from: false, to: true });
        }
        ops.push(Op::Move { from_index, to_index });
        Some(Transaction::new("Move layer", ops))
    }

    pub fn set_style_tx(&self, id: LayerId, to: Style) -> Option<Transaction> {
        let from = self.layer(id)?.style.clone();
        Some(Transaction::new("Change style", vec![Op::SetStyle { id, from: Box::new(from), to: Box::new(to) }]))
    }

    pub fn set_filter_tx(&self, id: LayerId, to: Option<Filter>) -> Option<Transaction> {
        let from = self.layer(id)?.filter.clone();
        let label = if to.is_some() { "Filter layer" } else { "Clear filter" };
        Some(Transaction::new(label, vec![Op::SetFilter { id, from, to }]))
    }

    /// The user sets a field's role (`None` returns to the inferred role).
    pub fn set_role_tx(&self, id: LayerId, field: &str, to: Option<FieldRole>) -> Option<Transaction> {
        let l = self.layer(id)?;
        l.field(field)?;
        let from = l.roles.get(field).copied();
        Some(Transaction::new("Change field role", vec![Op::SetRole { id, field: field.into(), from, to }]))
    }

    pub fn set_starred_tx(&self, id: LayerId, field: &str, to: bool) -> Option<Transaction> {
        let l = self.layer(id)?;
        l.field(field)?;
        let from = l.starred.iter().position(|s| s == field);
        let label = if to { "Show in insight" } else { "Remove from insight" };
        let to = match (to, from) {
            (true, Some(i)) => Some(i),
            (true, None) => Some(l.starred.len()),
            (false, _) => None,
        };
        Some(Transaction::new(label, vec![Op::SetStarred { id, field: field.into(), from, to }]))
    }

    pub fn set_profile_tx(&self, id: LayerId, to: LayerProfile) -> Option<Transaction> {
        let from = self.layer(id)?.profile.clone();
        Some(Transaction::new("Update layer", vec![Op::SetProfile { id, from: Box::new(from), to: Box::new(to) }]))
    }

    /// The user picks `to` for an assumption (Change popover or question card); it becomes
    /// confirmed by the user.
    pub fn set_assumption_tx(&self, id: LayerId, kind: AssumptionKind, subject: Subject, to: Choice) -> Option<Transaction> {
        let a = self.layer(id)?.profile.assumptions.iter().find(|a| a.kind == kind && a.subject == subject)?;
        Some(Transaction::new(
            "Change assumption",
            vec![Op::SetAssumption {
                id,
                kind,
                subject,
                from: a.chosen.clone(),
                to,
                from_confirmed: a.confirmed_by_user,
                to_confirmed: true,
            }],
        ))
    }

    /// Applies every op of `tx` in order. All or nothing: if an op fails, the ops already
    /// applied are reversed and the document is unchanged.
    pub fn apply(&mut self, tx: &Transaction) -> Result<(), ApplyError> {
        for (done, op) in tx.ops.iter().enumerate() {
            if let Err(e) = self.apply_op(op) {
                for prev in tx.ops.iter().take(done).rev() {
                    // Reversing an op that just applied cannot fail; ignore defensively.
                    let _ = self.apply_op(&prev.inverse());
                }
                return Err(e);
            }
        }
        Ok(())
    }

    fn apply_op(&mut self, op: &Op) -> Result<(), ApplyError> {
        let len = self.layers.len();
        match op {
            Op::AddLayer { index, layer } => {
                if *index > len {
                    return Err(ApplyError::IndexOutOfRange { index: *index, len });
                }
                if self.index_of(layer.id).is_some() {
                    return Err(ApplyError::DuplicateLayer(layer.id));
                }
                self.next_layer_id = self.next_layer_id.max(layer.id.0.saturating_add(1));
                self.layers.insert(*index, (**layer).clone());
            }
            Op::RemoveLayer { index, layer } => match self.layers.get(*index) {
                None => return Err(ApplyError::IndexOutOfRange { index: *index, len }),
                Some(l) if l.id != layer.id => return Err(ApplyError::LayerMismatch { index: *index, expected: layer.id }),
                Some(_) => {
                    self.layers.remove(*index);
                }
            },
            Op::Rename { id, to, .. } => self.layer_mut(*id)?.display_name = to.clone(),
            Op::SetVisible { id, to, .. } => self.layer_mut(*id)?.visible = *to,
            Op::Move { from_index, to_index } => {
                if *from_index >= len {
                    return Err(ApplyError::IndexOutOfRange { index: *from_index, len });
                }
                if *to_index >= len {
                    return Err(ApplyError::IndexOutOfRange { index: *to_index, len });
                }
                let l = self.layers.remove(*from_index);
                self.layers.insert(*to_index, l);
            }
            Op::SetUserOrdered { to, .. } => self.user_ordered = *to,
            Op::SetAssumption { id, kind, subject, to, to_confirmed, .. } => {
                let layer = self.layer_mut(*id)?;
                let a = layer
                    .profile
                    .assumptions
                    .iter_mut()
                    .find(|a| a.kind == *kind && a.subject == *subject)
                    .ok_or(ApplyError::NoSuchAssumption { id: *id, kind: *kind })?;
                a.chosen = to.clone();
                a.confirmed_by_user = *to_confirmed;
            }
            Op::SetStyle { id, to, .. } => self.layer_mut(*id)?.style = (**to).clone(),
            Op::SetProfile { id, to, .. } => self.layer_mut(*id)?.profile = (**to).clone(),
            Op::SetFilter { id, to, .. } => self.layer_mut(*id)?.filter = to.clone(),
            Op::SetRole { id, field, to, .. } => {
                let l = self.layer_mut(*id)?;
                match to {
                    Some(r) => {
                        l.roles.insert(field.clone(), *r);
                    }
                    None => {
                        l.roles.remove(field);
                    }
                }
            }
            Op::SetStarred { id, field, to, .. } => {
                let l = self.layer_mut(*id)?;
                l.starred.retain(|s| s != field);
                if let Some(i) = to {
                    let i = (*i).min(l.starred.len());
                    l.starred.insert(i, field.clone());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::*;

    pub(crate) fn profile(file: &str, kind: LayerKind) -> LayerProfile {
        LayerProfile {
            source: SourceRef { path: format!("/data/{file}"), member: None, size: 10, mtime: 5, sha256: None },
            format: "CSV".into(),
            kind,
            count: 248,
            extent_wgs84: None,
            crs: Known::likely(Crs::Wgs84, "lon/lat columns"),
            fields: vec![],
            bands: vec![],
            assumptions: vec![Assumption {
                kind: AssumptionKind::NoCrsLonLat,
                subject: Subject::Layer,
                chosen: Choice::new("wgs84", "WGS 84"),
                alternatives: vec![Choice::new("wgs84", "WGS 84"), Choice::new("gcj02", "GCJ-02"), Choice::not_sure()],
                confidence: Confidence::Likely,
                consequence: Consequence::ChangesLocations,
                treatment: Treatment::Visible,
                line: "Located as GPS coordinates (WGS 84).".into(),
                confirmed_by_user: false,
            }],
            exclusions: vec![],
            encoding: None,
            notices: vec![],
            data_rev: 0,
        }
    }

    /// A document with an area, a raster and a points layer.
    pub(crate) fn sample() -> Document {
        let mut d = Document::new();
        for (f, k) in [("districts.shp", LayerKind::Areas), ("dem.tif", LayerKind::Raster), ("schools.csv", LayerKind::Points)] {
            let tx = d.add_layer_tx(profile(f, k));
            d.apply(&tx).unwrap();
        }
        d
    }

    fn kinds(d: &Document) -> Vec<LayerKind> {
        d.layers.iter().map(|l| l.profile.kind).collect()
    }

    fn round_trip(d: &mut Document, tx: &Transaction) {
        let before = d.clone();
        d.apply(tx).unwrap();
        assert_ne!(*d, before, "{} changed nothing", tx.label);
        d.apply(&tx.inverse()).unwrap();
        assert_eq!(*d, before, "undoing {} did not restore the document", tx.label);
    }

    #[test]
    fn every_op_round_trips() {
        let mut d = sample();
        let id = d.layers[0].id;
        let tx = d.add_layer_tx(profile("roads.geojson", LayerKind::Lines));
        round_trip(&mut d, &tx);
        let tx = d.remove_layer_tx(id).unwrap();
        round_trip(&mut d, &tx);
        let tx = d.rename_tx(id, "Districts").unwrap();
        round_trip(&mut d, &tx);
        let tx = d.set_visible_tx(id, false).unwrap();
        round_trip(&mut d, &tx);
        round_trip(&mut d, &Transaction::new("Move", vec![Op::Move { from_index: 0, to_index: 2 }]));
        round_trip(&mut d, &Transaction::new("Order", vec![Op::SetUserOrdered { from: false, to: true }]));
        let tx = d.set_assumption_tx(id, AssumptionKind::NoCrsLonLat, Subject::Layer, Choice::new("gcj02", "GCJ-02")).unwrap();
        round_trip(&mut d, &tx);
        let mut s = d.layers[0].style.clone();
        s.opacity = 0.5;
        let tx = d.set_style_tx(id, s).unwrap();
        round_trip(&mut d, &tx);
        let mut p = d.layers[0].profile.clone();
        p.crs = Known::certain(Crs::Epsg(32650), "from .prj");
        let tx = d.set_profile_tx(id, p).unwrap();
        round_trip(&mut d, &tx);
        let tx = d.move_tx(2, 0).unwrap();
        round_trip(&mut d, &tx);
    }

    #[test]
    fn layers_stack_by_kind_until_the_user_reorders() {
        let d = sample();
        // Added: areas, raster, points → raster at bottom, then areas, points on top.
        assert_eq!(kinds(&d), [LayerKind::Raster, LayerKind::Areas, LayerKind::Points]);
        let mut d2 = d.clone();
        let tx = d2.add_layer_tx(profile("roads.geojson", LayerKind::Lines));
        d2.apply(&tx).unwrap();
        assert_eq!(kinds(&d2), [LayerKind::Raster, LayerKind::Areas, LayerKind::Lines, LayerKind::Points]);
        let tx = d2.add_layer_tx(profile("dem2.tif", LayerKind::Raster));
        d2.apply(&tx).unwrap();
        assert_eq!(d2.layers[1].profile.kind, LayerKind::Raster, "second raster on top of the first, below areas");

        // After a reorder, new layers go on top and nothing is re-sorted.
        let mut d3 = d.clone();
        let tx = d3.move_tx(2, 0).unwrap();
        d3.apply(&tx).unwrap();
        assert!(d3.user_ordered);
        assert_eq!(kinds(&d3), [LayerKind::Points, LayerKind::Raster, LayerKind::Areas]);
        let tx = d3.add_layer_tx(profile("dem2.tif", LayerKind::Raster));
        d3.apply(&tx).unwrap();
        assert_eq!(kinds(&d3), [LayerKind::Points, LayerKind::Raster, LayerKind::Areas, LayerKind::Raster]);
    }

    #[test]
    fn undoing_a_reorder_restores_the_stacking_rule() {
        let mut d = sample();
        let tx = d.move_tx(2, 0).unwrap();
        round_trip(&mut d, &tx);
        assert!(!d.user_ordered);
    }

    #[test]
    fn new_layers_get_distinct_palette_colours_and_kind_symbols() {
        let d = sample();
        let mut colors: Vec<Rgba> =
            d.layers.iter().filter(|l| l.profile.kind != LayerKind::Raster).map(|l| l.style.key_color(l.profile.kind)).collect();
        colors.dedup();
        assert_eq!(colors.len(), 2);
        let areas = d.layers.iter().find(|l| l.profile.kind == LayerKind::Areas).unwrap();
        assert_eq!(areas.style.fill_opacity, 0.35);
        assert_eq!(areas.style.stroke, darker(areas.style.fill));
        let pts = Style::for_kind(LayerKind::Points, OKABE_ITO[0], 248);
        assert_eq!((pts.point_size, pts.stroke, pts.stroke_width), (7.0, WHITE, 1.5));
        let dense = Style::for_kind(LayerKind::Points, OKABE_ITO[0], 20_000);
        assert_eq!((dense.point_size, dense.stroke_width), (4.0, 0.0));
        assert_eq!(Style::for_kind(LayerKind::Lines, OKABE_ITO[0], 1).line_width, 2.0);
        // Nine non-raster layers: all eight colours used, then cycling.
        let mut d = Document::new();
        for i in 0..9 {
            let tx = d.add_layer_tx(profile(&format!("p{i}.csv"), LayerKind::Points));
            d.apply(&tx).unwrap();
        }
        let mut all: Vec<Rgba> = d.layers.iter().map(|l| l.style.fill).collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 8);
    }

    #[test]
    fn raster_style_stretches_to_the_band_range() {
        let mut p = profile("dem.tif", LayerKind::Raster);
        p.bands = vec![BandProfile {
            index: 1,
            name: "elevation".into(),
            nodata: Known::certain(None, ""),
            scale: None,
            offset: None,
            unit: Known::unknown(None, ""),
            cell_meaning: Known::unknown(CellMeaning::Unknown, ""),
            min: Some(-10.0),
            max: Some(4000.0),
        }];
        let s = Style::for_profile(&p, OKABE_ITO[0]);
        assert_eq!((s.ramp.as_str(), s.stretch), ("viridis", Some((-10.0, 4000.0))));
    }

    #[test]
    fn layer_ids_are_never_reused() {
        let mut d = sample();
        let tx = d.add_layer_tx(profile("a.csv", LayerKind::Points));
        let first = match &tx.ops[0] {
            Op::AddLayer { layer, .. } => layer.id,
            _ => panic!("not an add"),
        };
        d.apply(&tx).unwrap();
        d.apply(&tx.inverse()).unwrap();
        let tx2 = d.add_layer_tx(profile("b.csv", LayerKind::Points));
        let second = match &tx2.ops[0] {
            Op::AddLayer { layer, .. } => layer.id,
            _ => panic!("not an add"),
        };
        assert_ne!(first, second);
        assert_eq!(d.layers[0].display_name, "dem");
    }

    #[test]
    fn a_failing_op_leaves_the_document_unchanged() {
        let mut d = sample();
        let before = d.clone();
        let id = d.layers[0].id;
        let tx = Transaction::new(
            "Bad",
            vec![Op::Rename { id, from: "dem".into(), to: "x".into() }, Op::SetVisible { id: LayerId(999), from: true, to: false }],
        );
        assert_eq!(d.apply(&tx), Err(ApplyError::NoSuchLayer(LayerId(999))));
        assert_eq!(d, before);
        let bad = Transaction::new("Bad", vec![Op::Move { from_index: 0, to_index: 7 }]);
        assert!(d.apply(&bad).is_err());
        let wrong = Transaction::new("Bad", vec![Op::RemoveLayer { index: 1, layer: Box::new(d.layers[0].clone()) }]);
        assert!(matches!(d.apply(&wrong), Err(ApplyError::LayerMismatch { .. })));
        assert_eq!(d, before);
    }
}
