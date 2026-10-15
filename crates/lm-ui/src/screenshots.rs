//! Screenshot regression (WP1.7): chrome states rendered headlessly and compared with approved
//! images in `tests/snapshots/`. The offline basemap keeps them free of network and timing.
//! Approve changes with `UPDATE_SNAPSHOTS=1 cargo test -p lm-ui screenshots`.

use egui_kittest::Harness;
use lm_core::phase::Flags;

use crate::{Prefs, Shell, System, ThemePref};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn harness(dark: bool, view: (f64, f64, f64), setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
    let prefs = Prefs {
        theme: if dark { ThemePref::Dark } else { ThemePref::Light },
        basemap: "offline".into(),
        view: Some(view),
        ..Prefs::default()
    };
    let system = System { dark_mode: dark, reduce_motion: true };
    let mut h = Harness::builder().with_size(SIZE).with_pixels_per_point(1.0).wgpu().build_ui_state(
        move |ui, state: &mut Option<Shell>| {
            let shell = state.get_or_insert_with(|| {
                let mut s = Shell::new(ui.ctx(), Flags::up_to(1), prefs.clone(), system);
                setup(&mut s);
                s
            });
            shell.ui(ui, system);
        },
        None,
    );
    // Run until the map's first frames have arrived and finished fading in.
    for _ in 0..400 {
        h.step();
        let settled = h.state_mut().as_mut().is_some_and(|s| s.map_view().renders > 0 && !s.map_view().tiles_pending() && s.search.ready());
        if settled {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(20);
    h
}

const WORLD: (f64, f64, f64) = (15.0, 30.0, 2.2);

#[test]
fn first_launch_light() {
    harness(false, WORLD, |_| {}).snapshot("first_launch_light");
}

#[test]
fn first_launch_dark() {
    harness(true, WORLD, |_| {}).snapshot("first_launch_dark");
}

#[test]
fn basemap_chooser() {
    harness(false, (121.47, 31.23, 6.0), |s| s.basemap_open = true).snapshot("basemap_chooser");
}

#[test]
fn search_results() {
    let mut h = harness(false, (121.47, 31.23, 6.0), |s| {
        s.search.query = "Shang".into();
        s.search.open = true;
    });
    h.run_steps(5);
    // The results list names Shanghai first.
    let first = h.state().as_ref().and_then(|s| s.search.rows.first().map(|r| r.title.clone())).unwrap_or_default();
    assert!(first.starts_with("Shanghai"), "{first}");
    h.snapshot("search_results");
}
