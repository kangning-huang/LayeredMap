//! What the relevance engine looks at (Relevance rules, step 1): the selection, the open layers
//! with their kinds and field roles, and the layers selected together with the first.

use lm_core::model::{Confidence, FieldProfile, FieldRole, LayerId, LayerKind};
use lm_core::selection::SelectionContext;

use crate::registry::SelectionKey;

/// A field, reduced to what eligibility needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSummary {
    pub name: String,
    pub role: FieldRole,
    pub confidence: Confidence,
    /// The user starred it ("Show in insight"): preferred when an action picks a field.
    pub starred: bool,
}

impl FieldSummary {
    pub fn new(name: impl Into<String>, role: FieldRole, confidence: Confidence) -> Self {
        FieldSummary { name: name.into(), role, confidence, starred: false }
    }

    pub fn from_profile(f: &FieldProfile) -> Self {
        FieldSummary::new(f.name.clone(), f.role.value, f.role.confidence)
    }

    /// Certain or Likely: the confidence insight and styling rely on.
    pub fn is_sure(&self) -> bool {
        matches!(self.confidence, Confidence::Certain | Confidence::Likely)
    }
}

/// A layer on the shelf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenLayer {
    pub id: LayerId,
    /// The display name used in labels ("Schools").
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    /// Position on the shelf; 0 is the top.
    pub shelf_index: usize,
    /// Order in which layers were added; higher is more recent.
    pub added_seq: u64,
    /// Fields in source order.
    pub fields: Vec<FieldSummary>,
}

impl OpenLayer {
    /// A visible layer without fields.
    pub fn new(id: LayerId, name: impl Into<String>, kind: LayerKind, shelf_index: usize, added_seq: u64) -> Self {
        OpenLayer { id, name: name.into(), kind, visible: true, shelf_index, added_seq, fields: Vec::new() }
    }

    pub fn with_fields(mut self, fields: Vec<FieldSummary>) -> Self {
        self.fields = fields;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    /// The name in the singular, for labels such as "Which District is it in?". A naive English
    /// rule: "Cities" → "City", "Schools" → "School"; names ending in -ss, -us or -is are kept.
    pub fn singular_name(&self) -> String {
        singular(&self.name)
    }
}

pub(crate) fn singular(name: &str) -> String {
    let b = name.as_bytes();
    let ends = |suffix: &str| b.len() >= suffix.len() && b[b.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes());
    // The suffixes are ASCII, so cutting them off keeps a char boundary.
    if b.len() > 3 && ends("ies") {
        let y = if name.ends_with("IES") { "Y" } else { "y" };
        return format!("{}{y}", &name[..name.len() - 3]);
    }
    if b.len() > 1 && ends("s") && !(ends("ss") || ends("us") || ends("is")) {
        return name[..name.len() - 1].to_string();
    }
    name.to_string()
}

/// What is selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Nothing selected.
    Nothing,
    /// One object (point, line or area; a row of a table without locations).
    Object { layer: LayerId },
    /// Several objects of one layer.
    Objects { layer: LayerId, count: usize },
    /// A whole layer (shelf click).
    Layer { layer: LayerId },
    /// One raster cell.
    RasterPixel { layer: LayerId },
    /// An area the user drew.
    DrawnArea,
}

impl Selection {
    /// From the interface's selection (a drawn area is not part of it yet).
    pub fn from_selection_context(s: &SelectionContext) -> Self {
        match (s.active_layer, &s.pixel, s.features.len()) {
            (_, Some(p), _) => Selection::RasterPixel { layer: p.layer },
            (Some(layer), None, 0) => Selection::Layer { layer },
            (Some(layer), None, 1) => Selection::Object { layer },
            (Some(layer), None, count) => Selection::Objects { layer, count },
            (None, None, _) => Selection::Nothing,
        }
    }

    /// The layer the selection belongs to.
    pub fn layer(&self) -> Option<LayerId> {
        match *self {
            Selection::Object { layer }
            | Selection::Objects { layer, .. }
            | Selection::Layer { layer }
            | Selection::RasterPixel { layer } => Some(layer),
            Selection::Nothing | Selection::DrawnArea => None,
        }
    }
}

/// Everything relevance looks at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub selection: Selection,
    /// The open layers, in any order.
    pub layers: Vec<OpenLayer>,
    /// Layers the user selected together with the first on the shelf (the expert path).
    pub co_selected: Vec<LayerId>,
}

impl Context {
    pub fn new(selection: Selection, layers: Vec<OpenLayer>) -> Self {
        Context { selection, layers, co_selected: Vec::new() }
    }

    /// From the interface's selection; its extra shelf layers become the co-selected layers.
    pub fn from_selection_context(s: &SelectionContext, layers: Vec<OpenLayer>) -> Self {
        let selection = Selection::from_selection_context(s);
        let first = selection.layer();
        let co_selected = s.layers.iter().copied().filter(|id| Some(*id) != first).collect();
        Context { selection, layers, co_selected }
    }

    pub fn with_co_selected(mut self, ids: Vec<LayerId>) -> Self {
        self.co_selected = ids;
        self
    }

    pub fn layer(&self, id: LayerId) -> Option<&OpenLayer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// The layer the selection belongs to, if it is open.
    pub fn selected_layer(&self) -> Option<&OpenLayer> {
        self.selection.layer().and_then(|id| self.layer(id))
    }

    /// The selection as the table's "Selected" column; None when it names a layer that is not
    /// open, or a pixel of a layer that is not a raster.
    pub fn selection_key(&self) -> Option<SelectionKey> {
        let kind = || self.selected_layer().map(|l| l.kind);
        Some(match self.selection {
            Selection::Nothing if self.layers.is_empty() => SelectionKey::Empty,
            Selection::Nothing => SelectionKey::Nothing,
            Selection::DrawnArea => SelectionKey::DrawnArea,
            Selection::Objects { .. } => {
                kind()?;
                SelectionKey::Several
            }
            Selection::Object { .. } => match kind()? {
                LayerKind::Points => SelectionKey::OnePoint,
                LayerKind::Lines => SelectionKey::OneLine,
                LayerKind::Areas => SelectionKey::OneArea,
                LayerKind::Raster => SelectionKey::RasterPixel,
                LayerKind::Table => SelectionKey::Table,
            },
            Selection::Layer { .. } => match kind()? {
                LayerKind::Points => SelectionKey::PointLayer,
                LayerKind::Lines => SelectionKey::LineLayer,
                LayerKind::Areas => SelectionKey::AreaLayer,
                LayerKind::Raster => SelectionKey::RasterLayer,
                LayerKind::Table => SelectionKey::Table,
            },
            Selection::RasterPixel { .. } => match kind()? {
                LayerKind::Raster => SelectionKey::RasterPixel,
                _ => return None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lm_core::selection::{FeatureRef, PixelPick};

    #[test]
    fn singular_names() {
        assert_eq!(singular("Schools"), "School");
        assert_eq!(singular("Districts"), "District");
        assert_eq!(singular("Cities"), "City");
        assert_eq!(singular("Heat"), "Heat");
        assert_eq!(singular("Bus"), "Bus");
        assert_eq!(singular("Address"), "Address");
        assert_eq!(singular("s"), "s");
    }

    #[test]
    fn selection_from_the_interface() {
        let l = LayerId(4);
        let f = FeatureRef { layer: l, row: 0 };
        assert_eq!(Selection::from_selection_context(&SelectionContext::default()), Selection::Nothing);
        assert_eq!(Selection::from_selection_context(&SelectionContext::layer(l)), Selection::Layer { layer: l });
        assert_eq!(Selection::from_selection_context(&SelectionContext::object(f)), Selection::Object { layer: l });
        let mut two = SelectionContext::object(f);
        two.toggle(FeatureRef { layer: l, row: 1 });
        assert_eq!(Selection::from_selection_context(&two), Selection::Objects { layer: l, count: 2 });
        let p = SelectionContext::pixel(PixelPick { layer: l, lon: 0.0, lat: 0.0, value: Some(1.0) });
        assert_eq!(Selection::from_selection_context(&p), Selection::RasterPixel { layer: l });

        let mut shelf = SelectionContext::layer(l);
        shelf.layers = vec![l, LayerId(9)];
        let ctx = Context::from_selection_context(&shelf, vec![]);
        assert_eq!(ctx.co_selected, vec![LayerId(9)]);
    }

    #[test]
    fn selection_keys() {
        let pts = OpenLayer::new(LayerId(1), "Schools", LayerKind::Points, 0, 1);
        let ctx = |s| Context::new(s, vec![pts.clone()]);
        assert_eq!(Context::new(Selection::Nothing, vec![]).selection_key(), Some(SelectionKey::Empty));
        assert_eq!(ctx(Selection::Nothing).selection_key(), Some(SelectionKey::Nothing));
        assert_eq!(ctx(Selection::Object { layer: LayerId(1) }).selection_key(), Some(SelectionKey::OnePoint));
        assert_eq!(ctx(Selection::Layer { layer: LayerId(1) }).selection_key(), Some(SelectionKey::PointLayer));
        assert_eq!(ctx(Selection::RasterPixel { layer: LayerId(1) }).selection_key(), None);
        assert_eq!(ctx(Selection::Layer { layer: LayerId(2) }).selection_key(), None);
    }
}
