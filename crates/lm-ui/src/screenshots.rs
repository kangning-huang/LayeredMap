//! Screenshot regression (WP1.7): chrome states rendered headlessly and compared with approved
//! images in `tests/snapshots/`. The offline basemap keeps them free of network and timing.
//! Approve changes with `UPDATE_SNAPSHOTS=1 cargo test -p lm-ui screenshots`. The approved images
//! come from macOS (the reference platform); other platforms' GPUs and text rasterisers differ in
//! small ways, so the comparisons run on macOS only.

use egui_kittest::Harness;
use lm_core::phase::Flags;

use crate::{Prefs, Shell, System, ThemePref};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn harness(dark: bool, view: (f64, f64, f64), setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
    harness_phase(1, dark, view, setup)
}

fn harness_phase(phase: u8, dark: bool, view: (f64, f64, f64), setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
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
                let mut s = Shell::new(ui.ctx(), Flags::up_to(phase), prefs.clone(), system);
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
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn first_launch_light() {
    harness(false, WORLD, |_| {}).snapshot("first_launch_light");
}

#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn first_launch_dark() {
    harness(true, WORLD, |_| {}).snapshot("first_launch_dark");
}

#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn basemap_chooser() {
    harness(false, (121.47, 31.23, 6.0), |s| s.basemap_open = true).snapshot("basemap_chooser");
}

#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
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

/// Phase 2: the first-launch welcome card.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase2_first_launch() {
    harness_phase(2, false, WORLD, |_| {}).snapshot("phase2_first_launch");
}

/// Phase 2: two files opened (points with a visible assumption and exclusions, and areas), the
/// points layer selected: the shelf and the layer overview.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase2_layers() {
    let mut h = harness_phase(2, false, (121.47, 31.23, 9.0), |s| {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for (i, f) in ["V1/districts.geojson", "T1/schools.csv"].iter().enumerate() {
            let layers =
                lm_jobs::import::import_path(&dir.join(f), &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
            s.ws.add_imported(layers, 100 + i as u64);
        }
        s.ws.selected = s.ws.doc.layers.last().map(|l| l.id);
        s.ws.new_data = None;
    });
    for _ in 0..200 {
        h.step();
        if h.state().as_ref().is_some_and(|s| !s.map_view().tiles_pending()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(20);
    let n = h.state().as_ref().map_or(0, |s| s.ws.doc.layers.len());
    assert_eq!(n, 2);
    h.snapshot("phase2_layers");
}

/// Scrolling over the layer list scrolls the list, never the map (owner's bug report, 2026-10-16).
#[test]
fn scrolling_over_the_layer_list_does_not_zoom_the_map() {
    let mut h = harness_phase(2, false, (121.47, 31.23, 9.0), |s| {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for (i, f) in ["V1/districts.geojson", "T1/schools.csv", "V2/mixed.geojson"].iter().enumerate() {
            let layers =
                lm_jobs::import::import_path(&dir.join(f), &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
            s.ws.add_imported(layers, 200 + i as u64);
        }
        s.ws.selected = None;
    });
    h.run_steps(10);
    let zoom = |h: &Harness<'static, Option<Shell>>| h.state().as_ref().map_or(0.0, |s| s.map_view().zoom());
    let over_shelf = egui::pos2(120.0, 140.0);
    let over_map = egui::pos2(700.0, 450.0);
    for (pos, should_zoom) in [(over_shelf, false), (over_map, true)] {
        let start = zoom(&h);
        for _ in 0..5 {
            h.input_mut().events.push(egui::Event::PointerMoved(pos));
            h.input_mut().events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, 1.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            h.step();
        }
        h.run_steps(30);
        let moved = (zoom(&h) - start).abs() > 0.01;
        assert_eq!(moved, should_zoom, "at {pos:?}: zoom went from {start} to {}", zoom(&h));
    }
}

fn two_layers() -> Harness<'static, Option<Shell>> {
    let mut h = harness_phase(2, false, (15.0, 30.0, 2.2), |s| {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for (i, f) in ["V1/districts.geojson", "T1/schools.csv"].iter().enumerate() {
            let layers =
                lm_jobs::import::import_path(&dir.join(f), &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
            s.ws.add_imported(layers, 300 + i as u64);
        }
        s.ws.selected = None;
    });
    h.run_steps(10);
    h
}

/// Owner's bug report: Rename (right-click) must finish with Return; Esc keeps the old name.
#[test]
fn renaming_finishes_with_return_and_esc_cancels() {
    let mut h = two_layers();
    let top = h.state().as_ref().and_then(|s| s.ws.doc.layers.last().map(|l| l.id)).unwrap();
    for (key, typed, expect) in [(egui::Key::Enter, "My schools", "My schools"), (egui::Key::Escape, "Nope", "My schools")] {
        if let Some(s) = h.state_mut().as_mut() {
            let name = s.ws.doc.layer(top).map(|l| l.display_name.clone()).unwrap_or_default();
            s.rename = Some((top, name));
            s.rename_started = true;
        }
        h.run_steps(2);
        if let Some(s) = h.state_mut().as_mut()
            && let Some((_, text)) = s.rename.as_mut()
        {
            *text = typed.to_string();
        }
        h.run_steps(1);
        h.input_mut().events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(3);
        let s = h.state().as_ref().unwrap();
        assert!(s.rename.is_none(), "{key:?} ends renaming");
        assert_eq!(s.ws.doc.layer(top).unwrap().display_name, expect, "after {key:?}");
    }
}

/// Owner's request: double-clicking a layer row zooms the map to that layer.
#[test]
fn double_clicking_a_layer_zooms_to_it() {
    let mut h = two_layers();
    let before = h.state().as_ref().map_or(0.0, |s| s.map_view().zoom());
    let row = egui::pos2(150.0, 141.0);
    h.input_mut().events.push(egui::Event::PointerMoved(row));
    h.run_steps(1);
    // Each press and release in one frame, two frames apart: well inside the double-click time.
    for _ in 0..2 {
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: row,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
    }
    h.run_steps(60);
    let s = h.state().as_ref().unwrap();
    assert!(s.rename.is_none(), "double-click doesn't start renaming");
    assert!(s.map_view().zoom() > before + 3.0, "zoomed from {before} to {}", s.map_view().zoom());
}
