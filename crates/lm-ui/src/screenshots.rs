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
    harness_sized(SIZE, 1.0, phase, dark, view, setup)
}

fn harness_sized(
    size: egui::Vec2,
    ppp: f32,
    phase: u8,
    dark: bool,
    view: (f64, f64, f64),
    setup: impl Fn(&mut Shell) + 'static,
) -> Harness<'static, Option<Shell>> {
    let prefs = Prefs {
        theme: if dark { ThemePref::Dark } else { ThemePref::Light },
        basemap: "offline".into(),
        view: Some(view),
        ..Prefs::default()
    };
    let system = System { dark_mode: dark, reduce_motion: true };
    let mut h = Harness::builder().with_size(size).with_pixels_per_point(ppp).wgpu().build_ui_state(
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
        if let Some(id) = s.ws.doc.layers.last().map(|l| l.id) {
            s.ws.select_layer(id);
        }
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
        s.ws.clear_selection();
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
        s.ws.clear_selection();
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

/// Phase 3: a harness with fixtures opened and every background job (map layers, hit indexes,
/// insight indexes, styles) finished.
fn phase3(files: &'static [&'static str], view: (f64, f64, f64), setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
    let mut h = harness_phase(3, false, view, move |s| {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for (i, f) in files.iter().enumerate() {
            let layers =
                lm_jobs::import::import_path(&dir.join(f), &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
            s.ws.add_imported(layers, 300 + i as u64);
        }
        s.ws.clear_selection();
        s.ws.new_data = None;
        setup(s);
    });
    settle(&mut h);
    h
}

/// Steps until the workspace has no background work and the map has drawn it.
fn settle(h: &mut Harness<'static, Option<Shell>>) {
    for _ in 0..600 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.ws.idle() && !s.table.busy() && !s.map_view().tiles_pending()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(30);
}

fn click(h: &mut Harness<'static, Option<Shell>>, lon: f64, lat: f64) {
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&egui::Context::default(), crate::Command::Click(lon, lat, false));
    }
    settle(h);
}

const SHANGHAI: (f64, f64, f64) = (121.5, 31.2, 9.6);

/// An object card: a district clicked, with its insight rows and facts.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase3_object_card() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
    click(&mut h, 121.26, 31.41);
    let rows = h.state().as_ref().map(|s| s.ws.selection.features.len()).unwrap_or(0);
    assert_eq!(rows, 1, "one district selected");
    h.snapshot("phase3_object_card");
}

/// No value: the district whose population is blank says "No value" and leaves the rank out.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase3_no_value() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
    click(&mut h, 121.71, 31.26);
    h.snapshot("phase3_no_value");
}

/// The overlap chooser: a click on a school inside a district finds both.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase3_overlap_chooser() {
    let mut h = phase3(&["V1/districts.geojson", "T1/schools.csv"], SHANGHAI, |_| {});
    // The first located school.
    let at = h.state().as_ref().and_then(|s| {
        let id = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Points)?.id;
        let ds = crate::inspect::dataset(s, id)?;
        (0..ds.features.len())
            .find_map(|r| lm_jobs::inspect::row_extent(&ds, r))
            .map(|e| ((e.min_x + e.max_x) / 2.0, (e.min_y + e.max_y) / 2.0))
    });
    let (lon, lat) = at.expect("a located school");
    click(&mut h, lon, lat);
    let n = h.state().as_ref().and_then(|s| s.chooser.as_ref().map(|c| c.1.len())).unwrap_or(0);
    assert!(n >= 2, "school and district under the click, got {n}");
    h.snapshot("phase3_overlap_chooser");
}

/// A filtered layer: the overview names the filter, the shelf row shows a funnel, the table says
/// how many of how many.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase3_filtered() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
        let id = s.ws.doc.layers[0].id;
        let f = lm_core::filter::Filter {
            conditions: vec![lm_core::filter::Condition { field: "population".into(), test: lm_core::filter::Test::Above(1_000_000.0) }],
        };
        let tx = s.ws.doc.set_filter_tx(id, Some(f));
        s.ws.commit(tx);
        s.ws.select_layer(id);
        s.table_for = Some(id);
    });
    settle(&mut h);
    let shown = h.state().as_ref().and_then(|s| s.ws.derived(s.ws.doc.layers[0].id).map(|d| d.shown)).unwrap_or(0);
    assert_eq!(shown, 10, "districts above 1,000,000");
    h.snapshot("phase3_filtered");
}

/// The Style tab with districts coloured by population in groups, and its legend.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase3_style_tab() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
        let id = s.ws.doc.layers[0].id;
        s.ws.select_layer(id);
        s.inspector_tab = 1;
        s.command(&egui::Context::default(), crate::Command::ApplyPreset(1));
    });
    settle(&mut h);
    h.snapshot("phase3_style_tab");
}

/// Details lists every field with wrapped evidence; the inspector keeps its width however long
/// the evidence is, frame after frame (owner's Phase 3 recording: it used to grow off the window).
#[test]
fn phase3_details_tab() {
    let mut h = phase3(&["../assets/examples/populated_places.geojson"], (100.0, 35.0, 3.0), |s| {
        // The same source text on every machine.
        s.ws.doc.layers[0].profile.source.path = "examples/populated_places.geojson".into();
        let id = s.ws.doc.layers[0].id;
        s.ws.select_layer(id);
        s.inspector_tab = 2;
    });
    settle(&mut h);
    let width = |h: &Harness<'static, Option<Shell>>| h.ctx.memory(|m| m.area_rect(egui::Id::new("lm-inspector"))).map(|r| r.width());
    let first = width(&h).expect("the inspector is open");
    for _ in 0..10 {
        h.run();
        assert_eq!(width(&h), Some(first));
    }
    assert!(first <= crate::panels::INSPECTOR_WIDTH + 2.0, "{first}"); // the frame's 1 px border on each side
    h.snapshot("phase3_details_tab");
}

/// Find in a layer (WP3.10): "pu" selects every matching district on the map and lists
/// them with the matched field in bold.
#[test]
fn phase3_find() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
        let id = s.ws.doc.layers[0].id;
        s.ws.select_layer(id);
        s.command(&egui::Context::default(), crate::Command::Find("pu".into()));
    });
    settle(&mut h);
    let (n, selected) = h.state().as_ref().map(|s| (s.find.hits.len(), s.ws.selection.features.len())).unwrap();
    assert!(n > 0 && selected == n, "{n} found, {selected} selected");
    h.snapshot("phase3_find");
}

/// Selection is shared: an object selected (as a table row click does) is highlighted on the map,
/// and a map click selects the object the table then shows.
#[test]
fn selection_is_shared_between_table_and_map() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
    let id = h.state().as_ref().map(|s| s.ws.doc.layers[0].id).unwrap();
    if let Some(s) = h.state_mut().as_mut() {
        s.ws.selection = lm_core::selection::SelectionContext::object(lm_core::selection::FeatureRef { layer: id, row: 3 });
    }
    settle(&mut h);
    let on_map = |h: &Harness<'static, Option<Shell>>| -> Vec<usize> {
        h.state()
            .as_ref()
            .map(|s| s.map_view().map().layers.iter().filter(|l| l.id >= 1_000_000).flat_map(|l| l.selection.clone()).collect())
            .unwrap_or_default()
    };
    assert_eq!(on_map(&h), [3]);
    click(&mut h, 121.26, 31.41);
    let sel = h.state().as_ref().map(|s| s.ws.selection.rows_in(id)).unwrap_or_default();
    assert_eq!(sel.len(), 1);
    assert_eq!(on_map(&h), sel);
    // Esc steps back: the object, then the layer.
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&egui::Context::default(), crate::Command::Escape);
    }
    settle(&mut h);
    assert!(on_map(&h).is_empty());
    assert_eq!(h.state().as_ref().and_then(|s| s.ws.selected()), Some(id));
}

/// The Style tab in its main states, rendered tall at 1.5x (the test GPU allows 2048 px textures) into `$LM_GALLERY` for design reviews
/// (`LM_GALLERY=dir cargo test -p lm-ui style_gallery -- --ignored`). Not a regression test.
#[test]
#[ignore]
fn style_gallery() {
    let Some(dir) = std::env::var_os("LM_GALLERY").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    type Setup = fn(&mut Shell);
    let scenes: [(&str, &'static str, bool, Setup); 6] = [
        ("areas-single", "countries.geojson", false, |_| {}),
        ("areas-by-value", "countries.geojson", false, |s| s.command(&egui::Context::default(), crate::Command::ApplyPreset(2))),
        ("areas-by-value-dark", "countries.geojson", true, |s| s.command(&egui::Context::default(), crate::Command::ApplyPreset(2))),
        ("areas-by-category", "countries.geojson", false, |s| s.command(&egui::Context::default(), crate::Command::ApplyPreset(1))),
        ("points-size", "populated_places.geojson", false, |s| s.command(&egui::Context::default(), crate::Command::ApplyPreset(3))),
        ("raster", "elevation.tif", false, |_| {}),
    ];
    for (name, file, dark, setup) in scenes {
        let mut h = harness_sized(egui::vec2(1280.0, 1300.0), 1.5, 3, dark, (100.0, 35.0, 3.0), move |s| {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples").join(file);
            let layers = lm_jobs::import::import_path(&path, &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
            s.ws.add_imported(layers, 300);
            s.ws.new_data = None;
            let id = s.ws.doc.layers[0].id;
            s.ws.select_layer(id);
            s.inspector_tab = 1;
            setup(s);
        });
        settle(&mut h);
        h.render().unwrap().save(dir.join(format!("{name}.png"))).unwrap();
    }
}
