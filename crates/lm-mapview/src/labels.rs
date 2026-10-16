//! Labels on screen, each with its own fade. A new frame's labels are matched to the ones already
//! shown (same text, nearly the same place): a label that stays stays at full strength, and only
//! labels that appear or disappear fade. Crossfading the whole set instead made every label dim
//! and return on each re-render, which flashed while tiles streamed in after a jump.

use rustgis_geom::Coord;
use rustgis_map::camera::Camera;
use rustgis_render::PlacedLabel;

/// Two placements of the same text closer than this (screen points) are the same label.
const SAME_LABEL_PT: f64 = 24.0;

pub struct Live {
    pub label: PlacedLabel,
    /// Where the label is anchored, in map coordinates: it moves with the camera, its text
    /// doesn't scale.
    pub at: Coord,
    pub alpha: f32,
    /// Fading in (true) or out (false).
    pub shown: bool,
}

#[derive(Default)]
pub struct Labels {
    pub live: Vec<Live>,
}

impl Labels {
    /// Takes a new frame's labels, drawn for `cam` at `size` (logical points). With `instant`
    /// (the first frame) they appear at full strength.
    pub fn update(&mut self, labels: Vec<PlacedLabel>, cam: &Camera, size: (f64, f64), instant: bool) {
        let mut old = std::mem::take(&mut self.live);
        let near2 = (SAME_LABEL_PT * cam.res).powi(2);
        let mut live = Vec::with_capacity(labels.len() + old.len());
        for label in labels {
            let at = cam.to_map(f64::from(label.x), f64::from(label.y), size.0, size.1);
            // The nearest old placement of the same text, if close enough.
            let found = old
                .iter()
                .enumerate()
                .filter(|(_, o)| o.label.text == label.text)
                .map(|(i, o)| (i, (o.at.x - at.x).powi(2) + (o.at.y - at.y).powi(2)))
                .filter(|(_, d2)| *d2 <= near2)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            let alpha = match found {
                Some(i) => old.swap_remove(i).alpha,
                None if instant => 1.0,
                None => 0.0,
            };
            live.push(Live { label, at, alpha, shown: true });
        }
        // Labels the new frame dropped fade out from wherever they were.
        live.extend(old.into_iter().filter(|o| o.alpha > 0.0).map(|o| Live { shown: false, ..o }));
        self.live = live;
    }

    /// Advances the fades by `dt` seconds (a full fade takes `fade_s`). Returns whether any label
    /// is still fading.
    pub fn step(&mut self, dt: f64, fade_s: f64) -> bool {
        let d = if fade_s > 0.0 { (dt / fade_s) as f32 } else { 1.0 };
        for l in &mut self.live {
            l.alpha = if l.shown { (l.alpha + d).min(1.0) } else { (l.alpha - d).max(0.0) };
        }
        self.live.retain(|l| l.shown || l.alpha > 0.0);
        self.live.iter().any(|l| l.alpha < 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(text: &str, x: f32, y: f32) -> PlacedLabel {
        PlacedLabel {
            text: text.into(),
            x,
            y,
            size: 12.0,
            color: rustgis_map::color::Rgba([0, 0, 0, 255]),
            halo: None,
            bold: false,
            icon: None,
            align: 0.0,
        }
    }

    fn cam() -> Camera {
        Camera { center: Coord::new(0.0, 0.0), res: 10.0 }
    }

    #[test]
    fn a_label_that_stays_never_dims() {
        let mut s = Labels::default();
        s.update(vec![label("London", 100.0, 100.0), label("Ealing", 50.0, 120.0)], &cam(), (800.0, 600.0), true);
        // Ten re-renders as tiles arrive, each a little different: London stays put.
        for i in 0..10 {
            let extra = format!("Town {i}");
            s.update(vec![label("London", 100.5, 99.5), label(&extra, 300.0, 300.0)], &cam(), (800.0, 600.0), false);
            s.step(0.016, 0.15);
            let london = s.live.iter().find(|l| l.label.text == "London" && l.shown).map(|l| l.alpha);
            assert_eq!(london, Some(1.0), "re-render {i}");
        }
    }

    #[test]
    fn new_labels_fade_in_and_dropped_ones_fade_out() {
        let mut s = Labels::default();
        s.update(vec![label("Old", 100.0, 100.0)], &cam(), (800.0, 600.0), true);
        s.update(vec![label("New", 400.0, 300.0)], &cam(), (800.0, 600.0), false);
        let a = |s: &Labels, t: &str| s.live.iter().find(|l| l.label.text == t).map(|l| l.alpha);
        assert_eq!(a(&s, "New"), Some(0.0));
        assert_eq!(a(&s, "Old"), Some(1.0));
        assert!(s.step(0.075, 0.15));
        assert!((a(&s, "New").unwrap_or_default() - 0.5).abs() < 1e-6);
        assert!((a(&s, "Old").unwrap_or_default() - 0.5).abs() < 1e-6);
        assert!(!s.step(0.1, 0.15));
        assert_eq!(a(&s, "Old"), None, "gone once faded out");
        assert_eq!(a(&s, "New"), Some(1.0));
    }

    #[test]
    fn the_same_text_far_away_is_a_different_label() {
        let mut s = Labels::default();
        s.update(vec![label("Springfield", 100.0, 100.0)], &cam(), (800.0, 600.0), true);
        s.update(vec![label("Springfield", 600.0, 500.0)], &cam(), (800.0, 600.0), false);
        let shown: Vec<f32> = s.live.iter().filter(|l| l.shown).map(|l| l.alpha).collect();
        assert_eq!(shown, vec![0.0]);
    }

    #[test]
    fn labels_are_anchored_in_map_coordinates() {
        let mut s = Labels::default();
        // Drawn for a camera 1,000 m east: the same place must match.
        s.update(vec![label("A", 400.0, 300.0)], &cam(), (800.0, 600.0), true);
        let east = Camera { center: Coord::new(1000.0, 0.0), res: 10.0 };
        s.update(vec![label("A", 300.0, 300.0)], &east, (800.0, 600.0), false);
        assert_eq!(s.live.len(), 1);
        assert_eq!(s.live.first().map(|l| l.alpha), Some(1.0));
    }
}
