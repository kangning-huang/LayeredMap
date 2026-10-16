//! The feel harness's camera scripts (WP1.6, from the WP0.3 spike): time-based, so a slow frame
//! can't stretch the path, and the same on every machine.

use rustgis_geom::Coord;
use rustgis_map::camera::Camera;

use crate::camera::{res_of, zoom_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    /// A 2,000-point pan at constant speed over 2 s.
    Pan,
    /// Zoom in 6 levels about the centre over 3 s.
    ZoomIn,
    /// Zoom back out 6 levels over 3 s.
    ZoomOut,
    /// A 4 s pinch: zoom oscillating ±1.5 levels while drifting.
    Pinch,
}

pub const ALL: [Script; 4] = [Script::Pan, Script::ZoomIn, Script::ZoomOut, Script::Pinch];

impl Script {
    pub fn code(self) -> &'static str {
        match self {
            Script::Pan => "pan2000",
            Script::ZoomIn => "zoom_in6",
            Script::ZoomOut => "zoom_out6",
            Script::Pinch => "pinch",
        }
    }

    pub fn duration(self) -> f64 {
        match self {
            Script::Pan => 2.0,
            Script::ZoomIn | Script::ZoomOut => 3.0,
            Script::Pinch => 4.0,
        }
    }

    /// Where the script starts, from the home view.
    pub fn start(self, home: Camera) -> Camera {
        match self {
            Script::ZoomOut => Camera { res: res_of(zoom_of(home.res) + 6.0), ..home },
            _ => home,
        }
    }

    /// The camera `t` seconds into the script.
    pub fn camera(self, home: Camera, t: f64) -> Camera {
        // Hold the last camera once the script has ended.
        let t = t.clamp(0.0, self.duration());
        let s = t / self.duration();
        let z0 = zoom_of(home.res);
        match self {
            Script::Pan => Camera { center: Coord::new(home.center.x + 2000.0 * s * home.res, home.center.y), ..home },
            Script::ZoomIn => Camera { res: res_of(z0 + 6.0 * s), ..home },
            Script::ZoomOut => Camera { res: res_of(z0 + 6.0 * (1.0 - s)), ..home },
            Script::Pinch => {
                let z = 1.5 * (std::f64::consts::TAU * t / 2.0).sin();
                let a = std::f64::consts::PI * t;
                Camera {
                    center: Coord::new(home.center.x + 150.0 * home.res * a.cos(), home.center.y + 150.0 * home.res * a.sin()),
                    res: res_of(z0 + z),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_start_where_they_say_and_end_on_time() {
        let home = Camera { center: Coord::new(1.35e7, 3.66e6), res: res_of(11.0) };
        for s in ALL {
            let c0 = s.camera(home, 0.0);
            let st = s.start(home);
            assert!((zoom_of(c0.res) - zoom_of(st.res)).abs() < 1e-9, "{}", s.code());
            let end = s.camera(home, s.duration() + 1.0);
            assert_eq!(end, s.camera(home, s.duration()), "{} holds after its end", s.code());
        }
        assert!((zoom_of(Script::ZoomIn.camera(home, 3.0).res) - 17.0).abs() < 1e-9);
        let pan = Script::Pan.camera(home, 2.0);
        assert!(((pan.center.x - home.center.x) / home.res - 2000.0).abs() < 1e-6);
    }
}
