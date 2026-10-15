//! Frame statistics for the debug overlay (⌥⌘F): live fps and 95th-percentile frame time.

use std::collections::VecDeque;

/// The last few seconds of frame intervals.
#[derive(Default)]
pub struct FrameStats {
    intervals_ms: VecDeque<f64>,
    last: Option<f64>,
}

const KEEP: usize = 240;

impl FrameStats {
    /// Records a frame drawn at time `t` (seconds). Gaps over a second (the app was idle, which
    /// egui is when nothing moves) are not frames the user waited for, and are left out.
    pub fn frame(&mut self, t: f64) {
        if let Some(l) = self.last {
            let dt = (t - l) * 1000.0;
            if dt > 0.0 && dt < 1000.0 {
                self.intervals_ms.push_back(dt);
                if self.intervals_ms.len() > KEEP {
                    self.intervals_ms.pop_front();
                }
            }
        }
        self.last = Some(t);
    }

    pub fn fps(&self) -> f64 {
        let n = self.intervals_ms.len();
        let sum: f64 = self.intervals_ms.iter().sum();
        if n == 0 || sum <= 0.0 { 0.0 } else { 1000.0 * n as f64 / sum }
    }

    /// The 95th-percentile frame interval, milliseconds.
    pub fn p95_ms(&self) -> f64 {
        let mut v: Vec<f64> = self.intervals_ms.iter().copied().collect();
        if v.is_empty() {
            return 0.0;
        }
        v.sort_by(f64::total_cmp);
        let i = ((v.len() - 1) as f64 * 0.95).round() as usize;
        v.get(i).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_sixty_fps() {
        let mut s = FrameStats::default();
        for i in 0..120 {
            s.frame(f64::from(i) / 60.0);
        }
        assert!((s.fps() - 60.0).abs() < 0.01);
        assert!((s.p95_ms() - 16.667).abs() < 0.01);
    }

    #[test]
    fn idle_gaps_are_not_frames() {
        let mut s = FrameStats::default();
        s.frame(0.0);
        s.frame(5.0);
        assert_eq!(s.fps(), 0.0);
    }
}
