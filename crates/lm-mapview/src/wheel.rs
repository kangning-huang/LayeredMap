//! What a scroll event does to the map (WP6.2).
//!
//! macOS tells a trackpad from a mouse: the trackpad scrolls in points (pan), a wheel in lines
//! (zoom by notches), and a pinch arrives as its own zoom event. Windows doesn't: winit reports
//! every `WM_MOUSEWHEEL` in lines, a precision touchpad included. So on Windows:
//!
//! - a whole-line vertical step is a mouse wheel notch: zoom half a level, as on macOS;
//! - a fraction of a line, or any sideways step, is a touchpad's two-finger scroll: pan;
//! - Ctrl with the wheel is a touchpad pinch (Windows sends a pinch to apps without gesture
//!   support as Ctrl + wheel) or Ctrl + mouse wheel: continuous zoom about the pointer;
//! - Shift with a mouse wheel pans sideways, as Windows apps do.
//!
//! A free-spinning or "smooth scrolling" mouse that reports fractions pans instead of zooming;
//! that and the feel of the speeds need a real device (docs/lm/windows.md).

use egui::{Modifiers, MouseWheelUnit, Vec2};

/// Points per line of a touchpad scroll on Windows: egui's own line speed on the desktop.
pub const LINE_POINTS: f32 = 40.0;
/// Zoom levels per line of Ctrl + wheel (a pinch): one mouse notch is half a level, as a plain
/// notch is.
pub const PINCH_LEVELS_PER_LINE: f64 = 0.5;

/// What one scroll event asks of the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wheel {
    /// A mouse wheel notch: an animated half-level zoom, in (+1) or out (−1).
    Notch(f64),
    /// Move the map by this many points.
    Pan(Vec2),
    /// Zoom continuously by this many levels about the pointer.
    Zoom(f64),
    /// Nothing (a sideways notch on macOS, an empty line event).
    Ignore,
}

/// The meaning of a scroll event; `windows` picks the Windows rules (a parameter so both rule
/// sets are tested on either system).
pub fn classify(unit: MouseWheelUnit, delta: Vec2, modifiers: Modifiers, windows: bool) -> Wheel {
    if unit != MouseWheelUnit::Line {
        // A trackpad's two-finger scroll in points pans, as in Apple Maps (an empty one still
        // stops a glide, as before).
        return Wheel::Pan(delta);
    }
    if delta == Vec2::ZERO {
        return Wheel::Ignore;
    }
    if !windows {
        // A purely sideways notch (tilt wheel, ⇧-scroll) doesn't zoom.
        return if delta.y == 0.0 { Wheel::Ignore } else { Wheel::Notch(f64::from(delta.y.signum())) };
    }
    if modifiers.ctrl {
        return if delta.y == 0.0 { Wheel::Ignore } else { Wheel::Zoom(PINCH_LEVELS_PER_LINE * f64::from(delta.y)) };
    }
    if modifiers.shift && delta.x == 0.0 {
        return Wheel::Pan(Vec2::new(delta.y, 0.0) * LINE_POINTS);
    }
    if delta.x == 0.0 && delta.y.fract() == 0.0 {
        return Wheel::Notch(f64::from(delta.y.signum()));
    }
    Wheel::Pan(delta * LINE_POINTS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    const NONE: Modifiers = Modifiers::NONE;
    const LINE: MouseWheelUnit = MouseWheelUnit::Line;

    #[test]
    fn macos_keeps_its_rules() {
        assert_eq!(classify(LINE, vec2(0.0, 1.0), NONE, false), Wheel::Notch(1.0));
        assert_eq!(classify(LINE, vec2(0.0, -0.3), NONE, false), Wheel::Notch(-1.0));
        assert_eq!(classify(LINE, vec2(2.0, 0.0), NONE, false), Wheel::Ignore);
        assert_eq!(classify(MouseWheelUnit::Point, vec2(3.0, -7.5), NONE, false), Wheel::Pan(vec2(3.0, -7.5)));
        assert_eq!(classify(LINE, vec2(0.0, 1.0), Modifiers::CTRL, false), Wheel::Notch(1.0));
    }

    #[test]
    fn windows_mouse_notches_zoom_by_steps() {
        assert_eq!(classify(LINE, vec2(0.0, 1.0), NONE, true), Wheel::Notch(1.0));
        assert_eq!(classify(LINE, vec2(0.0, -2.0), NONE, true), Wheel::Notch(-1.0));
    }

    /// A precision touchpad's two-finger scroll arrives as fractions of a line: it pans, in both
    /// directions, instead of zooming a half level per event.
    #[test]
    fn windows_touchpad_scroll_pans() {
        assert_eq!(classify(LINE, vec2(0.0, 0.25), NONE, true), Wheel::Pan(vec2(0.0, 10.0)));
        assert_eq!(classify(LINE, vec2(-0.5, 0.0), NONE, true), Wheel::Pan(vec2(-20.0, 0.0)));
        // A tilt wheel's whole sideways step pans too.
        assert_eq!(classify(LINE, vec2(1.0, 0.0), NONE, true), Wheel::Pan(vec2(40.0, 0.0)));
    }

    /// A pinch comes as Ctrl + wheel: continuous zoom, the fractions adding up.
    #[test]
    fn windows_pinch_zooms_continuously() {
        assert_eq!(classify(LINE, vec2(0.0, 0.2), Modifiers::CTRL, true), Wheel::Zoom(0.5 * f64::from(0.2_f32)));
        assert_eq!(classify(LINE, vec2(0.0, -1.0), Modifiers::CTRL, true), Wheel::Zoom(-0.5));
        assert_eq!(classify(LINE, vec2(0.3, 0.0), Modifiers::CTRL, true), Wheel::Ignore);
    }

    #[test]
    fn windows_shift_wheel_pans_sideways() {
        assert_eq!(classify(LINE, vec2(0.0, 1.0), Modifiers::SHIFT, true), Wheel::Pan(vec2(40.0, 0.0)));
    }

    #[test]
    fn empty_events_do_nothing() {
        assert_eq!(classify(LINE, Vec2::ZERO, NONE, true), Wheel::Ignore);
        assert_eq!(classify(MouseWheelUnit::Point, Vec2::ZERO, NONE, false), Wheel::Pan(Vec2::ZERO));
    }
}
