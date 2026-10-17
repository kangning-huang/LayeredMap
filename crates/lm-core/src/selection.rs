//! The selection context (Architecture › Data contracts; WP3.1). Interaction state: never saved
//! and never an undo step.

use crate::model::LayerId;

/// One record of a layer, by its row in the layer's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FeatureRef {
    pub layer: LayerId,
    pub row: usize,
}

/// A raster cell picked on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct PixelPick {
    pub layer: LayerId,
    pub lon: f64,
    pub lat: f64,
    /// The decoded value of the shown band; None for NoData.
    pub value: Option<f64>,
}

/// What is selected. `features` all belong to `active_layer` (⇧-click adds within a layer);
/// `layers` are layers selected together with the active one on the shelf (the expert path for
/// choosing a companion layer).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionContext {
    pub active_layer: Option<LayerId>,
    pub features: Vec<FeatureRef>,
    pub layers: Vec<LayerId>,
    pub pixel: Option<PixelPick>,
}

impl SelectionContext {
    pub fn is_empty(&self) -> bool {
        self.active_layer.is_none() && self.features.is_empty() && self.pixel.is_none()
    }

    /// A whole layer selected (shelf click).
    pub fn layer(id: LayerId) -> Self {
        SelectionContext { active_layer: Some(id), ..Default::default() }
    }

    /// One object selected (map click or table row).
    pub fn object(f: FeatureRef) -> Self {
        SelectionContext { active_layer: Some(f.layer), features: vec![f], ..Default::default() }
    }

    pub fn pixel(p: PixelPick) -> Self {
        SelectionContext { active_layer: Some(p.layer), pixel: Some(p), ..Default::default() }
    }

    /// ⇧-click: toggles `f` in the selection. A record of another layer starts a new selection.
    pub fn toggle(&mut self, f: FeatureRef) {
        if self.active_layer != Some(f.layer) || self.pixel.is_some() {
            *self = SelectionContext::object(f);
            return;
        }
        match self.features.iter().position(|x| *x == f) {
            Some(i) => {
                self.features.remove(i);
            }
            None => self.features.push(f),
        }
    }

    /// The Esc ladder (WP3.1): several objects → none of them but the layer; one object or a
    /// pixel → the layer; the layer → nothing. Returns false when there was nothing to clear.
    pub fn step_back(&mut self) -> bool {
        if !self.features.is_empty() || self.pixel.is_some() {
            self.features.clear();
            self.pixel = None;
            true
        } else if self.active_layer.is_some() {
            *self = SelectionContext::default();
            true
        } else {
            false
        }
    }

    /// Rows selected in `layer`.
    pub fn rows_in(&self, layer: LayerId) -> Vec<usize> {
        self.features.iter().filter(|f| f.layer == layer).map(|f| f.row).collect()
    }

    /// Forgets anything that refers to a layer no longer in the document.
    pub fn retain_layers(&mut self, exists: impl Fn(LayerId) -> bool) {
        if self.active_layer.is_some_and(|l| !exists(l)) {
            *self = SelectionContext::default();
        }
        self.layers.retain(|l| exists(*l));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_click_toggles_within_a_layer() {
        let a = LayerId(1);
        let mut s = SelectionContext::object(FeatureRef { layer: a, row: 3 });
        s.toggle(FeatureRef { layer: a, row: 5 });
        assert_eq!(s.rows_in(a), [3, 5]);
        s.toggle(FeatureRef { layer: a, row: 3 });
        assert_eq!(s.rows_in(a), [5]);
        s.toggle(FeatureRef { layer: LayerId(2), row: 0 });
        assert_eq!(s.active_layer, Some(LayerId(2)));
        assert_eq!(s.features.len(), 1);
    }

    #[test]
    fn esc_steps_back_object_then_layer_then_nothing() {
        let a = LayerId(1);
        let mut s = SelectionContext::object(FeatureRef { layer: a, row: 3 });
        assert!(s.step_back());
        assert_eq!(s, SelectionContext::layer(a));
        assert!(s.step_back());
        assert!(s.is_empty());
        assert!(!s.step_back());
    }
}
