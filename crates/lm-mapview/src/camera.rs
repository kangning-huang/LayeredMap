//! Camera rules (Interaction specification › Map gestures and keys, Motion): fractional zoom,
//! vertical panning clamped at the poles, animated zoom steps, fly-to, inertia.

use rustgis_geom::{Coord, MERCATOR_R, lonlat_to_merc, merc_to_lonlat};
use rustgis_map::camera::Camera;

/// Metres per logical pixel at zoom 0 with 256-pixel tiles (the web-map zoom scale).
pub const RES_Z0: f64 = 2.0 * std::f64::consts::PI * MERCATOR_R / 256.0;
/// The deepest zoom the map allows.
pub const MAX_ZOOM: f64 = 19.0;
/// Half the Web Mercator world's height, metres.
const HALF_WORLD: f64 = std::f64::consts::PI * MERCATOR_R;

pub fn zoom_of(res: f64) -> f64 {
    (RES_Z0 / res).log2()
}

pub fn res_of(zoom: f64) -> f64 {
    RES_Z0 / 2f64.powf(zoom)
}

/// Keeps the camera inside the world for a `w`×`h` view: never zoomed out beyond the world's
/// height, never past MAX_ZOOM, and never panned beyond a pole (the top and bottom edges of the
/// view stop at the edges of the Mercator square). East–west panning is free within ±1.5 worlds.
pub fn clamp(cam: &mut Camera, w: f64, h: f64) {
    if !cam.res.is_finite() || cam.res <= 0.0 {
        cam.res = res_of(2.0);
    }
    let min_res = res_of(MAX_ZOOM);
    // The whole world's height fits the view at the coarsest resolution.
    let max_res = if h > 0.0 { (2.0 * HALF_WORLD / h).max(min_res) } else { res_of(0.0) };
    cam.res = cam.res.clamp(min_res, max_res);
    if !cam.center.is_finite() {
        cam.center = Coord::new(0.0, 0.0);
    }
    let half_h = h / 2.0 * cam.res;
    let lim_y = (HALF_WORLD - half_h).max(0.0);
    cam.center.y = cam.center.y.clamp(-lim_y, lim_y);
    let _ = w;
    cam.center.x = cam.center.x.clamp(-1.5 * HALF_WORLD, 1.5 * HALF_WORLD);
}

/// Moves the map's content by a screen offset (a drag of `dx`, `dy` points moves what is under
/// the pointer with it).
pub fn pan(cam: &mut Camera, dx: f64, dy: f64) {
    cam.center.x -= dx * cam.res;
    cam.center.y += dy * cam.res;
}

/// Sets the zoom to `zoom` keeping the map point under screen point (`sx`, `sy`) fixed.
pub fn zoom_about(cam: &mut Camera, zoom: f64, sx: f64, sy: f64, w: f64, h: f64) {
    let anchor = cam.to_map(sx, sy, w, h);
    cam.res = res_of(zoom.clamp(-1.0, MAX_ZOOM));
    let after = cam.to_map(sx, sy, w, h);
    cam.center.x += anchor.x - after.x;
    cam.center.y += anchor.y - after.y;
}

/// Cubic ease-out (zoom steps).
pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in-out (fly-to).
pub fn ease_in_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// A running camera animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Animation {
    /// A zoom step about a screen point (wheel notch, double-click, buttons, keys): 250 ms.
    Zoom { from: f64, to: f64, start: f64, anchor: (f64, f64) },
    /// A flight to a place: 600–900 ms by distance, zooming out on the way when it is far.
    Fly { from: Camera, to: Camera, start: f64, duration: f64 },
}

pub const ZOOM_STEP_S: f64 = 0.25;

/// A flight from `from` to `lonlat` at `zoom`, for a `w`×`h` view.
pub fn fly(from: Camera, lonlat: Coord, zoom: f64, start: f64, w: f64) -> Animation {
    let to = Camera { center: lonlat_to_merc(lonlat), res: res_of(zoom) };
    // Distance in screens at the coarser of the two zooms: near hops are quick, far ones slower.
    let coarse = from.res.max(to.res);
    let screens = ((to.center.x - from.center.x).hypot(to.center.y - from.center.y) / (coarse * w.max(1.0))).max(0.0);
    let duration = (0.6 + 0.3 * (screens / 4.0).min(1.0)).clamp(0.6, 0.9);
    Animation::Fly { from, to, start, duration }
}

/// The camera an animation gives at time `t`, and whether it has finished.
pub fn step(anim: &Animation, cam: Camera, t: f64, w: f64, h: f64) -> (Camera, bool) {
    match *anim {
        Animation::Zoom { from, to, start, anchor } => {
            let s = ((t - start) / ZOOM_STEP_S).clamp(0.0, 1.0);
            let z = from + (to - from) * ease_out_cubic(s);
            let mut c = cam;
            zoom_about(&mut c, z, anchor.0, anchor.1, w, h);
            (c, s >= 1.0)
        }
        Animation::Fly { from, to, start, duration } => {
            let s = ((t - start) / duration).clamp(0.0, 1.0);
            let e = ease_in_out_cubic(s);
            // Zoom (in log space) with an arc: out by up to two levels at the middle for a long
            // flight, so the destination comes into view before diving in.
            let (z0, z1) = (zoom_of(from.res), zoom_of(to.res));
            let dist_px = (to.center.x - from.center.x).hypot(to.center.y - from.center.y) / from.res.max(to.res);
            let bump = if dist_px > w.max(h) { (dist_px / w.max(1.0)).log2().clamp(0.0, 2.0) } else { 0.0 };
            let z = z0 + (z1 - z0) * e - bump * (std::f64::consts::PI * e).sin();
            let c = Camera {
                center: Coord::new(from.center.x + (to.center.x - from.center.x) * e, from.center.y + (to.center.y - from.center.y) * e),
                res: res_of(z),
            };
            (c, s >= 1.0)
        }
    }
}

/// The map's centre in longitude/latitude.
pub fn center_lonlat(cam: &Camera) -> Coord {
    merc_to_lonlat(cam.center)
}

/// Inertia: the velocity decays exponentially so a fast flick comes to rest in 300–600 ms.
pub const INERTIA_TAU_S: f64 = 0.11;
/// Below this speed (points per second) the map stops.
pub const INERTIA_STOP: f64 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_and_resolution_round_trip() {
        for z in [0.0, 3.5, 11.0, 18.25] {
            assert!((zoom_of(res_of(z)) - z).abs() < 1e-9);
        }
        // Fractional zoom survives clamping.
        let mut c = Camera { center: Coord::new(0.0, 0.0), res: res_of(11.37) };
        clamp(&mut c, 1280.0, 800.0);
        assert!((zoom_of(c.res) - 11.37).abs() < 1e-9);
    }

    #[test]
    fn panning_stops_at_the_poles() {
        let mut c = Camera { center: Coord::new(0.0, 0.0), res: res_of(3.0) };
        pan(&mut c, 0.0, 1e9); // drag far down: the map moves down, the camera north
        clamp(&mut c, 1280.0, 800.0);
        let top = c.center.y + 400.0 * c.res;
        assert!((top - HALF_WORLD).abs() < 1e-6, "the view's top edge sits on the north edge");
        // Zoomed out beyond the world's height: clamped so the world fills the view's height.
        let mut c = Camera { center: Coord::new(0.0, 5e6), res: res_of(-3.0) };
        clamp(&mut c, 1280.0, 800.0);
        assert!((c.res * 800.0 - 2.0 * HALF_WORLD).abs() < 1e-3);
        assert_eq!(c.center.y, 0.0);
    }

    #[test]
    fn zoom_keeps_the_anchor_fixed() {
        let mut c = Camera { center: lonlat_to_merc(Coord::new(121.47, 31.23)), res: res_of(11.0) };
        let before = c.to_map(100.0, 700.0, 1280.0, 800.0);
        zoom_about(&mut c, 13.5, 100.0, 700.0, 1280.0, 800.0);
        let after = c.to_map(100.0, 700.0, 1280.0, 800.0);
        assert!((before.x - after.x).abs() < 1e-6 && (before.y - after.y).abs() < 1e-6);
    }

    #[test]
    fn a_drag_moves_the_content_with_the_pointer() {
        let mut c = Camera { center: Coord::new(0.0, 0.0), res: 10.0 };
        let p = Coord::new(1000.0, 0.0);
        let (x0, _) = c.to_screen(p, 1280.0, 800.0);
        pan(&mut c, 50.0, 0.0);
        let (x1, _) = c.to_screen(p, 1280.0, 800.0);
        assert!((x1 - x0 - 50.0).abs() < 1e-9);
    }

    #[test]
    fn flights_take_600_to_900_ms_and_end_exactly() {
        let from = Camera { center: lonlat_to_merc(Coord::new(121.47, 31.23)), res: res_of(11.0) };
        for (dest, z) in [(Coord::new(121.5, 31.25), 12.0), (Coord::new(-9.14, 38.72), 11.0)] {
            let a = fly(from, dest, z, 0.0, 1280.0);
            let Animation::Fly { duration, to, .. } = a else { panic!() };
            assert!((0.6..=0.9).contains(&duration));
            let (c, done) = step(&a, from, duration, 1280.0, 800.0);
            assert!(done);
            assert!((c.center.x - to.center.x).abs() < 1e-6 && (zoom_of(c.res) - z).abs() < 1e-9);
        }
    }

    #[test]
    fn easing_is_monotone_and_bounded() {
        let mut prev = 0.0;
        for i in 0..=100 {
            let t = f64::from(i) / 100.0;
            let (a, b) = (ease_out_cubic(t), ease_in_out_cubic(t));
            assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
            assert!(b >= prev);
            prev = b;
        }
    }
}
