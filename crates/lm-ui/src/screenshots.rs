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

pub(crate) fn harness_phase(
    phase: u8,
    dark: bool,
    view: (f64, f64, f64),
    setup: impl Fn(&mut Shell) + 'static,
) -> Harness<'static, Option<Shell>> {
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

pub(crate) const WORLD: (f64, f64, f64) = (15.0, 30.0, 2.2);

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

/// The layer's right-click menu, each item with its symbol; then "Remove all" empties the map in
/// one step that Undo reverses.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase3_layer_menu_and_remove_all() {
    use egui_kittest::kittest::Queryable as _;
    let mut h = harness_phase(3, false, (121.47, 31.23, 9.0), |s| {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for (i, f) in ["V1/districts.geojson", "T1/schools.csv"].iter().enumerate() {
            let layers =
                lm_jobs::import::import_path(&dir.join(f), &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
            s.ws.add_imported(layers, 100 + i as u64);
        }
        s.ws.new_data = None;
    });
    h.run_steps(20);
    let row = egui::pos2(110.0, 181.0);
    h.input_mut().events.push(egui::Event::PointerMoved(row));
    h.run_steps(2);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: row,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(2);
    }
    h.run_steps(10);
    h.snapshot("phase3_layer_menu");
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(4);
    h.get_by_label("Remove all").click();
    h.run_steps(4);
    assert_eq!(h.state().as_ref().map_or(99, |s| s.ws.doc.layers.len()), 0);
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::Undo);
    }
    h.run_steps(4);
    assert_eq!(h.state().as_ref().map_or(0, |s| s.ws.doc.layers.len()), 2);
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
pub(crate) fn phase3(
    files: &'static [&'static str],
    view: (f64, f64, f64),
    setup: impl Fn(&mut Shell) + 'static,
) -> Harness<'static, Option<Shell>> {
    phase3_sized(SIZE, 1.0, files, view, setup)
}

/// [`phase3`] in a window of `size` points at `ppp` pixels per point.
fn phase3_sized(
    size: egui::Vec2,
    ppp: f32,
    files: &'static [&'static str],
    view: (f64, f64, f64),
    setup: impl Fn(&mut Shell) + 'static,
) -> Harness<'static, Option<Shell>> {
    let mut h = harness_sized(size, ppp, 3, false, view, move |s| {
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
pub(crate) fn settle(h: &mut Harness<'static, Option<Shell>>) {
    for _ in 0..600 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.ws.idle() && !s.table.busy() && !s.map_view().tiles_pending()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(30);
}

pub(crate) fn click(h: &mut Harness<'static, Option<Shell>>, lon: f64, lat: f64) {
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&egui::Context::default(), crate::Command::Click(lon, lat, false));
    }
    settle(h);
}

pub(crate) const SHANGHAI: (f64, f64, f64) = (121.5, 31.2, 9.6);

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
    let mut h = phase3(&["../assets/examples/cities.geojson"], (100.0, 35.0, 3.0), |s| {
        // The same source text on every machine.
        s.ws.doc.layers[0].profile.source.path = "examples/cities.geojson".into();
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

/// Joining a table (WP3.11): provinces and a table keyed by Chinese names join with no
/// question, colour the layer, and show the result card with the typo suggestion. The join is
/// one undo step: Undo brings the table back and the layer's fields as they were.
#[test]
fn phase3_join_card() {
    let mut h = phase3(&["J5/provinces.geojson", "J5/population_zh.csv"], (5.0, 30.5, 6.0), |_| {});
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.doc.layers.len(), 1, "the table left the shelf");
    let l = &s.ws.doc.layers[0];
    assert!(l.profile.fields.iter().any(|f| f.name == "population_millions"));
    assert!(l.style.color_by.is_some());
    let r = s.ws.join_report(l.id).expect("a join card");
    assert_eq!((r.matched, r.near_misses.len()), (7, 1));
    h.snapshot("phase3_join_card");
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&egui::Context::default(), crate::Command::Undo);
    }
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.doc.layers.len(), 2);
    let l = s.ws.doc.layers.iter().find(|l| l.profile.kind != lm_core::model::LayerKind::Table).unwrap();
    assert!(!l.profile.fields.iter().any(|f| f.name == "population_millions"));
    assert!(l.style.color_by.is_none());
}

/// A joined table stays on the shelf, indented under its layer (owner's request, 2026-10-18):
/// ISO3 codes join countries, not cities (their country field repeats), and the table shows as
/// a child row of countries, expanded right after the join, with how many countries matched.
#[test]
fn phase3_joined_sublayer() {
    use egui_kittest::kittest::Queryable as _;
    let mut h =
        phase3(&["../assets/examples/cities.geojson", "../assets/examples/countries.geojson", "J1/gdp_growth_iso3.csv"], WORLD, |_| {});
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.doc.layers.len(), 2, "the table left the top level");
    let countries = s.ws.doc.layers.iter().find(|l| l.display_name == "countries").unwrap().id;
    let cities = s.ws.doc.layers.iter().find(|l| l.display_name == "cities").unwrap().id;
    assert!(s.ws.sublayers(cities).is_empty());
    let subs = s.ws.sublayers(countries);
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].table, "gdp_growth_iso3.csv");
    assert!(s.ws.sublayers_open(countries));
    h.snapshot("phase3_joined_sublayer");
    // A click on the child row selects countries and shows the join's card; its menu's Unjoin
    // puts the table back on the shelf, and Undo joins it again.
    let row = egui::pos2(170.0, 226.0);
    h.input_mut().events.push(egui::Event::PointerMoved(row));
    h.run_steps(2);
    for (button, pressed) in [(egui::PointerButton::Primary, true), (egui::PointerButton::Primary, false)] {
        h.input_mut().events.push(egui::Event::PointerButton { pos: row, button, pressed, modifiers: egui::Modifiers::NONE });
        h.run_steps(2);
    }
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.selected(), Some(countries));
    assert_eq!(s.join_focus, Some((countries, "gdp_growth_iso3.csv".to_string())));
    assert_eq!(s.inspector_tab, 0, "the join's card is on Overview");
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: row,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(2);
    }
    h.run_steps(4);
    h.get_by_label_contains("Unjoin").click();
    h.run_steps(4);
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.doc.layers.len(), 3, "the table is back");
    assert!(s.ws.sublayers(countries).is_empty());
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::Undo);
    }
    h.run_steps(4);
    let s = h.state().as_ref().unwrap();
    assert_eq!((s.ws.doc.layers.len(), s.ws.sublayers(countries).len()), (2, 1));
}

/// A table whose places aren't clear asks one question (J4).
#[test]
fn phase3_join_question() {
    let mut h = phase3(&["J4/places.geojson", "J4/mixed_keys.csv"], (6.0, 0.5, 6.0), |_| {});
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    let table = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Table).expect("the table stays");
    assert!(s.ws.join_question(table.id).is_some_and(|q| q.offers.len() >= 2));
    h.snapshot("phase3_join_question");
}

/// A clear join whose keys repeat asks first (owner's choice, 2026-10-18): J3, one row per
/// district per year, shows the row counts and how the repeats are handled, with Join and Keep
/// as a table.
#[test]
fn phase3_join_confirm() {
    let mut h = phase3(&["V1/districts.geojson", "J3/population_by_year.csv"], SHANGHAI, |_| {});
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    let table = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Table).expect("the table waits");
    assert!(s.ws.join_question(table.id).is_some_and(|q| q.confirm));
    h.snapshot("phase3_join_confirm");
}

/// "Match to a layer" (WP6.4): the column picker on a table whose key column has another name
/// than the layer's (J6: `ward` ↔ `ward_code`), with the live preview.
#[test]
fn phase3_match_picker() {
    let mut h = phase3(&["J6/wards.geojson", "J6/ward_people.csv"], (121.37, 31.02, 9.0), |s| {
        let table = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Table).unwrap().id;
        // As if opened from the table's menu before the automatic join ran: the picker holds it.
        crate::join_ui::open_picker(s, table);
    });
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    let d = s.match_draft.as_ref().expect("the picker is open");
    assert_eq!((d.layer_field.as_str(), d.table_field.as_str()), ("ward_code", "ward"));
    h.snapshot("phase3_match_picker");
}

/// Drawing an area (WP4.4): the outline so far and the banner; a finished outline becomes the
/// layer "Area 1" in WGS 84.
#[test]
fn phase4_drawing() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
        s.ws.clear_selection();
        let mut d = crate::draw::Drawing::new(crate::draw::Mode::Polygon);
        for p in [(121.30, 31.30), (121.45, 31.33), (121.50, 31.20)] {
            d.click(p, |a, b| a == b);
        }
        s.drawing = Some(d);
    });
    settle(&mut h);
    h.snapshot("phase4_drawing");
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&egui::Context::default(), crate::Command::Escape);
        assert!(s.drawing.is_none(), "Esc cancels");
        s.command(&egui::Context::default(), crate::Command::DrawArea(vec![(121.30, 31.30), (121.45, 31.33), (121.50, 31.20)]));
    }
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    let area = s.ws.doc.layers.iter().find(|l| l.display_name == "Area 1").expect("Area 1");
    assert_eq!(area.profile.kind, lm_core::model::LayerKind::Areas);
    assert_eq!(area.profile.crs.value, lm_core::model::Crs::Wgs84);
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
        ("points-size", "cities.geojson", false, |s| s.command(&egui::Context::default(), crate::Command::ApplyPreset(3))),
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

/// The interface in every language (WP5.5): the welcome card, an object card and the Style tab,
/// rendered into `$LM_GALLERY` for translation reviews
/// (`LM_GALLERY=dir cargo test -p lm-ui language_gallery -- --ignored`). Not a regression test:
/// Japanese and Korean use the system's fonts.
#[test]
#[ignore]
fn language_gallery() {
    let Some(dir) = std::env::var_os("LM_GALLERY").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for lang in lm_core::i18n::Lang::all() {
        let code = lang.code();
        let mut h = harness_phase(2, false, WORLD, move |s| s.prefs.language = code.into());
        h.run_steps(10);
        h.render().unwrap().save(dir.join(format!("{code}-welcome.png"))).unwrap();
        let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, move |s| s.prefs.language = code.into());
        click(&mut h, 121.26, 31.41);
        h.render().unwrap().save(dir.join(format!("{code}-card.png"))).unwrap();
        let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, move |s| {
            s.prefs.language = code.into();
            let id = s.ws.doc.layers[0].id;
            s.ws.select_layer(id);
            s.inspector_tab = 1;
            s.command(&egui::Context::default(), crate::Command::ApplyPreset(1));
        });
        settle(&mut h);
        h.render().unwrap().save(dir.join(format!("{code}-style.png"))).unwrap();
    }
}

/// Phase 4's developer surface: the composer previews "Count points in each area", Keep adds
/// the result layer and its record in one undo step, and Undo removes both.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase4_composer_preview_keep_undo() {
    let mut h = phase3(&["N1/areas.geojson", "N1/points.geojson"], (121.35, 31.2, 10.0), |s| {
        s.developer = true;
        let id = s.ws.doc.layers.iter().find(|l| l.display_name == "points").map(|l| l.id);
        if let Some(id) = id {
            s.ws.select_layer(id);
        }
    });
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::ComposeAction("count_in".into(), vec![]));
    }
    for _ in 0..300 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.composer.as_ref().is_some_and(|c| c.result.is_some() || c.error.is_some())) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(&mut h);
    let err = h.state().as_ref().and_then(|s| s.composer.as_ref().and_then(|c| c.error.clone()));
    assert_eq!(err, None);
    h.snapshot("phase4_composer");
    let before = h.state().as_ref().map_or(0, |s| s.ws.doc.layers.len());
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::KeepResult);
    }
    settle(&mut h);
    h.snapshot("phase4_result_card");
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.doc.layers.len(), before + 1);
    assert_eq!(s.ws.doc.results.len(), 1);
    assert!(s.composer.is_none());
    assert!(s.ws.doc.layers.iter().any(|l| l.display_name == "Points in each area"));
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::Undo);
    }
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    assert_eq!((s.ws.doc.layers.len(), s.ws.doc.results.len()), (before, 0));
}

/// "between" keeps "[a] and [b]" together: when the sentence wraps, both boxes move as one.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase3_filter_between() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
        let l = s.ws.doc.layers[0].clone();
        s.table_for = Some(l.id);
        let mut d = crate::table::FilterDraft::preset(s, &l, Some("population"), None);
        d.verb = "between";
        s.filter_draft = Some(d);
    });
    settle(&mut h);
    h.snapshot("phase3_filter_between");
}

/// Developer surface on the bundled examples: which assistant actions each selection offers
/// (printed for the testing guide).
#[test]
#[ignore = "prints the developer surface's actions for the testing guide"]
fn developer_actions_on_examples() {
    let _h = phase3(
        &["../assets/examples/countries.geojson", "../assets/examples/cities.geojson", "../assets/examples/elevation.tif"],
        WORLD,
        |s| {
            s.developer = true;
            let ids: Vec<_> = s.ws.doc.layers.iter().map(|l| (l.id, l.display_name.clone(), l.profile.kind)).collect();
            for (id, name, kind) in ids {
                for row in [None, Some(0usize)] {
                    match row {
                        None => s.ws.select_layer(id),
                        Some(r) if kind != lm_core::model::LayerKind::Raster => {
                            s.ws.selection =
                                lm_core::selection::SelectionContext::object(lm_core::selection::FeatureRef { layer: id, row: r })
                        }
                        _ => continue,
                    }
                    let ctx = crate::insight::context(s);
                    let sug = lm_actions::suggest(&ctx, lm_actions::Options::developer(4));
                    let top: Vec<String> = sug.top.iter().map(|a| format!("{} [{}]", a.label, a.action_id)).collect();
                    let more: Vec<String> =
                        sug.more.iter().flat_map(|(_, v)| v.iter().map(|a| format!("{} [{}]", a.label, a.action_id))).collect();
                    println!("{name} {:?}: top {:?}\n    more {:?}", row.map(|_| "object"), top, more);
                }
            }
        },
    );
}

/// "Summarize in an area I draw": drawing on a raster's behalf opens the composer on the new
/// area with the raster as the second layer, and the summary arrives as a number.
#[test]
fn phase4_summarize_in_a_drawn_area() {
    let mut h = phase3(&["../assets/examples/elevation.tif"], WORLD, |s| {
        s.developer = true;
        s.after_drawing = s.ws.doc.layers.first().map(|l| l.id);
    });
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::DrawArea(vec![(100.0, 25.0), (110.0, 25.0), (110.0, 35.0), (100.0, 35.0)]));
    }
    for _ in 0..300 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.composer.as_ref().is_some_and(|c| c.result.is_some() || c.error.is_some())) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let s = h.state().as_ref().unwrap();
    let c = s.composer.as_ref().expect("composer");
    assert_eq!(c.error, None);
    assert_eq!(c.action, "summarize_drawn");
    let r = c.result.as_ref().expect("result");
    assert!(matches!(r.card.answer, lm_core::recipe::Answer::Number { value, .. } if value.is_finite()), "{:?}", r.card.answer);
}

/// Signed in to an account with Pro (the one app's unlocked state, no override): the toolbar
/// shows the PRO badge, and a layer's panel offers the analysis actions as working buttons.
/// Signed out, the same actions show locked, with a line saying how to unlock them; clicking one
/// opens Settings › Account. A build that can't sign in shows none of them.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn pro_offers_analysis_actions() {
    use egui_kittest::kittest::Queryable as _;
    // One harness per case, so their snapshot results are merged into one (egui_kittest).
    let mut results = egui_kittest::SnapshotResults::new();
    // (account: None = no account config, Some(signed in)) → what the panel shows
    for account in [None, Some(false), Some(true)] {
        let mut h = phase3(&["../assets/examples/countries.geojson", "../assets/examples/cities.geojson"], WORLD, move |s| {
            if let Some(signed_in) = account {
                s.account = crate::account_ui::test_ui(signed_in, lm_account::Region::International);
            }
            let id = s.ws.doc.layers.iter().find(|l| l.display_name == "countries").map(|l| l.id);
            if let Some(id) = id {
                s.ws.select_layer(id);
            }
        });
        settle(&mut h);
        let working = h.query_by_label("Count cities in each country").is_some();
        let locked = h.query_by_label("Count cities in each country (locked)").is_some();
        let line = h.query_by_label_contains("Sign in to use analysis tools").is_some();
        match account {
            None => assert!(!working && !locked && !line, "no account config: no analysis actions"),
            Some(false) => {
                assert!(locked && !working && line, "signed out: locked, with the way to unlock");
                h.snapshot("locked_layer_panel");
                // A locked action opens Settings › Account instead of the composer.
                h.get_by_label("Count cities in each country (locked)").click();
                h.run_steps(5);
                let s = h.state().as_ref().unwrap();
                assert!(s.account.open && s.composer.is_none());
            }
            Some(true) => {
                assert!(working && !locked && !line, "Pro unlocked: working buttons");
                assert!(h.state().as_ref().is_some_and(|s| s.pro() && s.pro_access() == lm_account::Access::Unlocked));
                h.snapshot("pro_layer_panel");
            }
        }
        results.extend_harness(&mut h);
    }
}

/// Pro: "Find areas where elevation is at least 2,000 m" previews areas and keeps them as a layer.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn pro_find_where_on_the_elevation_example() {
    let mut h = phase3(&["../assets/examples/elevation.tif"], WORLD, |s| {
        s.account = crate::account_ui::test_ui(true, lm_account::Region::International);
        let id = s.ws.doc.layers[0].id;
        s.ws.select_layer(id);
    });
    let ctx = h.ctx.clone();
    let id = h.state().as_ref().map(|s| s.ws.doc.layers[0].id.0).unwrap();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::ComposeAction("find_where".into(), vec![("conditions".into(), format!("{id}|at_least|2000"))]));
    }
    for _ in 0..400 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.composer.as_ref().is_some_and(|c| c.result.is_some() || c.error.is_some())) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(&mut h);
    assert_eq!(h.state().as_ref().and_then(|s| s.composer.as_ref().and_then(|c| c.error.clone())), None);
    h.snapshot("pro_find_where");
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::KeepResult);
    }
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.doc.layers.len(), 2);
    assert!(s.ws.doc.layers.iter().any(|l| l.display_name.starts_with("Where elevation")));
}

/// Pro's Ask box answers "How many Category 5 storms have passed within 100 km of Tokyo?"
/// (fixtures/A1): the preview highlights only the 2 tracks that answer, outlines the 100 km area
/// and frames it; the filtered-out storms are hidden as Keep will hide them. Keep selects the 2.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn pro_ask_answer_preview() {
    let mut h = phase3(&["A1/storms.geojson", "A1/cities.geojson"], (145.0, 30.0, 4.0), |s| {
        s.account = crate::account_ui::test_ui(true, lm_account::Region::International);
        s.ws.clear_selection();
    });
    let request = "How many Category 5 storms have passed within 100 km of Tokyo?";
    if let Some(s) = h.state_mut().as_mut() {
        let steps: Vec<lm_jobs::plan_exec::Step> = serde_json::from_value(serde_json::json!([
            {"name": "filter", "args": {"layer": "storms", "conditions": [{"field": "category", "op": "is", "value": "5"}]}},
            {"name": "buffer", "args": {"object": {"layer": "cities", "field": "name", "value": "Tokyo"}, "distance": "100 km"}},
            {"name": "count_in", "args": {"points": "storms", "areas": "#2"}}
        ]))
        .unwrap();
        let (mut layers, _) = s.ws.ask_inputs();
        let outcomes = lm_jobs::plan_exec::execute(&mut layers, &steps, 0, &lm_jobs::jobs::Control::none()).unwrap();
        crate::ask::show_plan(s, request, lm_ask::PlanResult { steps, layers, outcomes, notes: Vec::new() });
    }
    settle(&mut h);
    h.snapshot("pro_ask_answer_preview");
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::KeepAsk);
    }
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    let storms = s.ws.doc.layers.iter().find(|l| l.display_name == "storms").map(|l| l.id).unwrap();
    assert_eq!(s.ws.selection.rows_in(storms), vec![0, 3]);
    assert_eq!(s.ws.doc.layers.len(), 4);
}

/// Phase 5: the launch after a crash offers the autosave (WP5.2).
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase5_recovery_prompt() {
    use egui_kittest::kittest::Queryable as _;
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
        s.doc_file.recovery = Some(lm_doc::Recovery {
            name: "Heat study".into(),
            saved_at: lm_doc::container::now_secs() - 125,
            autosave: std::env::temp_dir().join("lm-no-such-autosave.lmap"),
            document: None,
        });
    });
    settle(&mut h);
    assert!(h.query_by_label_contains("NimbleMap quit unexpectedly. Restore 'Heat study' from 2 minutes ago?").is_some());
    h.snapshot("phase5_recovery_prompt");
    // Discard closes the prompt and keeps the map as it is.
    h.get_by_label("Discard").click();
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    assert!(s.doc_file.recovery.is_none());
    assert_eq!(s.ws.doc.layers.len(), 1);
}

/// A reopened map whose raster moved: heat_july.tif is missing, districts are there.
fn with_missing_source(setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
    let mut h = phase3(&["V1/districts.geojson", "../assets/examples/elevation.tif"], SHANGHAI, move |s| {
        let mut doc = s.ws.doc.clone();
        if let Some(l) = doc.layers.iter_mut().find(|l| l.profile.kind == lm_core::model::LayerKind::Raster) {
            l.profile.source.path = "/Users/student/Downloads/heat_july.tif".into();
            l.display_name = "heat_july".into();
        }
        s.ws.restore(doc);
        setup(s);
    });
    settle(&mut h);
    h
}

/// Phase 5: the missing-source card on the layer's overview (Error and status copy).
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase5_missing_source() {
    use egui_kittest::kittest::Queryable as _;
    let mut h = with_missing_source(|_| {});
    let s = h.state().as_ref().unwrap();
    assert_eq!(s.ws.missing.len(), 1);
    assert_eq!(s.ws.selected(), s.ws.missing.first().copied());
    assert!(h.query_by_label_contains("Can't find heat_july.tif. It was last seen in Downloads.").is_some());
    assert!(h.query_by_label("Locate…").is_some());
    h.snapshot("phase5_missing_source");
    // Locate… asks the app for the file.
    h.get_by_label("Locate…").click();
    h.run_steps(3);
    let reqs = h.state_mut().as_mut().unwrap().take_file_requests();
    assert!(matches!(reqs.as_slice(), [crate::FileRequest::Locate { file_name, .. }] if file_name == "heat_july.tif"), "{reqs:?}");
}

/// Phase 5: the chosen file doesn't match the fingerprint, so the card asks first.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn phase5_relink_question() {
    use egui_kittest::kittest::Queryable as _;
    let mut h = with_missing_source(|_| {});
    let elevation = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples/elevation.tif");
    let tmp = std::env::temp_dir().join(format!("lm-relink-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let moved = tmp.join("heat_july.tif");
    let mut bytes = std::fs::read(&elevation).unwrap();
    bytes.extend_from_slice(b"different");
    std::fs::write(&moved, &bytes).unwrap();
    let ctx = h.ctx.clone();
    let id = h.state().as_ref().and_then(|s| s.ws.missing.first().copied()).unwrap();
    h.state_mut().as_mut().unwrap().command(&ctx, crate::Command::Relink(id, moved.clone()));
    settle(&mut h);
    assert!(h.query_by_label_contains("This file looks different (its size changed). Use it anyway?").is_some());
    h.snapshot("phase5_relink_question");
    // Cancel keeps the layer missing.
    h.get_by_label("Cancel").click();
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    assert!(s.doc_file.relink.is_none());
    assert_eq!(s.ws.missing, vec![id]);
    let _ = std::fs::remove_dir_all(tmp);
}

// ---------------------------------------------------------------------------------------------
// Export (WP5.3)

/// Steps until the map image sheet has drawn the map for its settings.
pub(crate) fn wait_for_export(h: &mut Harness<'static, Option<Shell>>) {
    for _ in 0..3000 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.export_sheet.as_ref().is_some_and(|e| !e.busy())) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(5);
}

/// Districts coloured by population in groups, with the schools over them; the developer flag
/// shows export ahead of its phase.
pub(crate) fn export_scene(setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
    export_scene_sized(SIZE, 1.0, setup)
}

/// [`export_scene`] in a window of `size` points at `ppp` pixels per point.
fn export_scene_sized(size: egui::Vec2, ppp: f32, setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
    let mut h = phase3_sized(size, ppp, &["V1/districts.geojson", "T1/schools.csv"], SHANGHAI, move |s| {
        s.developer = true;
        if let Some(id) = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Areas).map(|l| l.id) {
            s.ws.select_layer(id);
            s.command(&egui::Context::default(), crate::Command::ApplyPreset(1));
            s.ws.clear_selection();
        }
        setup(s);
    });
    settle(&mut h);
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::ExportMapImage);
    }
    wait_for_export(&mut h);
    h
}

/// How many pixels in `area` (points on the sheet) are dark, like text.
fn dark_pixels(pm: &lm_mapview::export::Pixmap, ppp: f32, a: lm_mapview::export::Area) -> usize {
    let (x0, y0, x1, y1) = ((a.x * ppp) as u32, (a.y * ppp) as u32, (a.right() * ppp) as u32, (a.bottom() * ppp) as u32);
    let mut dark = 0usize;
    for y in y0..y1.min(pm.height()) {
        for x in x0..x1.min(pm.width()) {
            if lm_mapview::export::pixel(pm, x, y).is_some_and(|c| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]) < 3 * 120) {
                dark += 1;
            }
        }
    }
    dark
}

/// Gate 5 export pixel tests: each preset comes out at exactly its size, with a legend and the
/// credits in it, and every colour in the legend is a colour the map draws.
#[test]
fn exported_images_have_exact_sizes_a_legend_credits_and_the_maps_colors() {
    use lm_mapview::export::{Entry, Swatch};
    let mut h = export_scene(|_| {});
    let ctx = h.ctx.clone();
    for preset in [crate::export::Preset::Slide, crate::export::Preset::Square, crate::export::Preset::A4] {
        if let Some(s) = h.state_mut().as_mut()
            && let Some(e) = s.export_sheet.as_mut()
        {
            e.preset = preset;
            e.title = "Schools and districts".into();
        }
        wait_for_export(&mut h);
        let s = h.state().as_ref().unwrap();
        let e = s.export_sheet.as_ref().unwrap();
        let sheet = e.sheet(s);
        let (pm, lay) = e.image(s).expect("the image");
        assert_eq!((pm.width(), pm.height()), preset.pixels(), "{preset:?}");
        // `LM_EXPORT_OUT=dir` keeps the images for a look.
        // The same sheet as PDF (WP6.5): one page of the preset's paper, the title as text.
        let pdf = e.pdf(s).expect("the map is drawn").expect("the PDF");
        assert!(pdf.starts_with(b"%PDF-") && pdf.windows(9).any(|w| w == b"/Identity"), "{preset:?}");
        if let Some(dir) = std::env::var_os("LM_EXPORT_OUT").map(std::path::PathBuf::from) {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{preset:?}.png")), lm_mapview::export::encode_png(&pm).unwrap()).unwrap();
            std::fs::write(dir.join(format!("{preset:?}.pdf")), &pdf).unwrap();
        }
        // The credits, in the footer's right part, and never empty.
        assert!(sheet.attribution.contains("Natural Earth"), "{}", sheet.attribution);
        let f = lay.footer;
        let text = (sheet.ppp * sheet.ppp * 20.0) as usize;
        assert!(
            dark_pixels(&pm, sheet.ppp, lm_mapview::export::Area::new(f.w * 0.75, f.y, f.w * 0.25 - 8.0, f.h)) > text,
            "{preset:?} credits"
        );
        assert!(dark_pixels(&pm, sheet.ppp, lay.title.unwrap()) > text, "{preset:?} title");
        // The legend: inside the map, with text in it.
        let legend = lay.legend.expect("a legend");
        assert!(lay.map.contains(legend.x, legend.y) && lay.map.contains(legend.right(), legend.bottom()));
        assert!(dark_pixels(&pm, sheet.ppp, legend) > 4 * text, "{preset:?} legend text");
        // Each class swatch's colour, read from the image, is drawn on the map.
        let block = sheet.legend.iter().find(|b| b.title.starts_with("districts")).expect("the districts' legend");
        let mut y = legend.y + 12.0 + 16.0 + 4.0;
        let mut checked = 0;
        let map_px = |x: u32, y: u32| {
            lay.map.contains(x as f32 / sheet.ppp, y as f32 / sheet.ppp) && !legend.contains(x as f32 / sheet.ppp, y as f32 / sheet.ppp)
        };
        for entry in &block.entries {
            let Entry::Item { swatch, label } = entry else { continue };
            if let Swatch::Area { .. } = swatch {
                let at = (((legend.x + 12.0 + 7.0) * sheet.ppp) as u32, ((y + 10.0) * sheet.ppp) as u32);
                let c = lm_mapview::export::pixel(&pm, at.0, at.1).unwrap();
                let close = |p: [u8; 4]| (0..3).all(|k| (i32::from(p[k]) - i32::from(c[k])).abs() <= 3);
                let mut on_map = 0;
                for py in (0..pm.height()).step_by(2) {
                    for px in (0..pm.width()).step_by(2) {
                        if map_px(px, py) && lm_mapview::export::pixel(&pm, px, py).is_some_and(close) {
                            on_map += 1;
                        }
                    }
                }
                assert!(on_map > 20, "{preset:?}: legend colour {c:?} for {label} found at {on_map} map pixels");
                checked += 1;
            }
            y += 20.0;
        }
        assert!(checked >= 4, "{checked} classes checked");
        // Shanghai, about 100 km across: a north arrow with its N and dart, clear of the legend.
        let north = lay.north.expect("a north arrow");
        assert!(sheet.north_arrow && sheet.graticule.is_none() && sheet.scale.is_some());
        assert!(dark_pixels(&pm, sheet.ppp, north) as f32 > 40.0 * sheet.ppp * sheet.ppp, "{preset:?} north arrow");
        assert!(
            north.x + north.w <= legend.x
                || legend.x + legend.w <= north.x
                || north.y + north.h <= legend.y
                || legend.y + legend.h <= north.y
        );
        let _ = &ctx;
    }
}

/// Automatic layout: the map fits the visible data (the shown objects of a filtered layer); the
/// overrides switch the north arrow off and the lat/lon frame on, and the image follows.
#[test]
fn export_layout_fits_the_data_and_follows_the_overrides() {
    use lm_mapview::export::{Area, Show};
    let mut h = export_scene(|s| {
        let id = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Areas).map(|l| l.id).unwrap();
        let f = lm_core::filter::Filter {
            conditions: vec![lm_core::filter::Condition { field: "population".into(), test: lm_core::filter::Test::Above(1_000_000.0) }],
        };
        let tx = s.ws.doc.set_filter_tx(id, Some(f));
        s.ws.commit(tx);
        // The schools are hidden: only the ten districts frame the map.
        if let Some(l) = s.ws.doc.layers.iter_mut().find(|l| l.profile.kind != lm_core::model::LayerKind::Areas) {
            l.visible = false;
        }
    });
    let s = h.state().as_ref().unwrap();
    let e = s.export_sheet.as_ref().unwrap();
    assert_eq!(e.fit, crate::export::Fit::Data);
    let plan = e.plan();
    let (w, hh) = (f64::from(plan.area.w), f64::from(plan.area.h));
    let id = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Areas).map(|l| l.id).unwrap();
    let ds = crate::inspect::dataset(s, id).unwrap();
    let mask = s.ws.derived(id).and_then(|d| d.mask.clone()).expect("the filter's mask");
    let (mut lo, mut hi) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
    for (row, on) in mask.iter().enumerate() {
        let Some(b) = lm_jobs::inspect::row_extent(&ds, row).filter(|_| *on) else { continue };
        for (lon, lat) in [(b.min_x, b.min_y), (b.max_x, b.max_y)] {
            let (x, y) = plan.camera.to_screen(lm_mapview::lonlat_to_merc(lm_mapview::Coord::new(lon, lat)), w, hh);
            assert!(x >= 0.0 && x <= w && y >= 0.0 && y <= hh, "shown district {row} at {x},{y}");
            lo = (lo.0.min(x), lo.1.min(y));
            hi = (hi.0.max(x), hi.1.max(y));
        }
    }
    // Fitted, not merely contained: the shown districts fill one side but for the 5% margins.
    let fill = ((hi.0 - lo.0) / w).max((hi.1 - lo.1) / hh);
    assert!(fill > 0.88, "fills {fill}");
    // Overrides: no arrow, the frame on, with grid lines.
    let s = h.state_mut().as_mut().unwrap();
    if let Some(e) = s.export_sheet.as_mut() {
        e.overrides.north_arrow = Show::Off;
        e.overrides.labels = Show::On;
        e.grid = true;
    }
    wait_for_export(&mut h);
    let s = h.state().as_ref().unwrap();
    let e = s.export_sheet.as_ref().unwrap();
    let sheet = e.sheet(s);
    let (pm, lay) = e.image(s).expect("the image");
    assert!(lay.north.is_none() && sheet.graticule.is_some_and(|g| g.grid));
    let m = lay.map;
    let fr = lm_mapview::export::FRAME_TB;
    let top = Area::new(m.x, m.y - fr + 1.0, m.w, fr - 3.0);
    assert!(dark_pixels(&pm, sheet.ppp, top) > (sheet.ppp * sheet.ppp * 40.0) as usize, "longitude labels above the map");
    // Current view: the camera is the one the user saw, refitted.
    let s = h.state_mut().as_mut().unwrap();
    if let Some(e) = s.export_sheet.as_mut() {
        e.fit = crate::export::Fit::View;
    }
    let e = s.export_sheet.as_ref().unwrap();
    assert_ne!(e.plan().camera, plan.camera);
}

/// Export warnings: a colour scale without a legend, a filtered layer ("10 of 17"); previews
/// left out; a basemap whose terms forbid export is replaced by the offline map, in the preview.
#[test]
fn export_warnings_name_what_the_image_leaves_out() {
    let mut h = export_scene(|s| {
        let id = s.ws.doc.layers.iter().find(|l| l.profile.kind == lm_core::model::LayerKind::Areas).map(|l| l.id).unwrap();
        let f = lm_core::filter::Filter {
            conditions: vec![lm_core::filter::Condition { field: "population".into(), test: lm_core::filter::Test::Above(1_000_000.0) }],
        };
        let tx = s.ws.doc.set_filter_tx(id, Some(f));
        s.ws.commit(tx);
    });
    let s = h.state_mut().as_mut().unwrap();
    let mut e = s.export_sheet.take().unwrap();
    let w = e.warnings(s).join("\n");
    assert!(w.contains("districts is filtered: the image shows 10 of 17 areas."), "{w}");
    for l in e.legend.iter_mut() {
        l.1 = false;
    }
    let w = e.warnings(s).join("\n");
    assert!(w.contains("districts is colored by value but has no legend."), "{w}");
    assert!(!w.contains("schools is colored"), "single-colour layers need no legend: {w}");
    // The Satellite basemap (NASA Blue Marble) carries no extra terms, only its credit.
    s.prefs.basemap = "satellite".into();
    s.basemap_changed();
    let sat = crate::export::ImageSheet::new(s);
    assert!(!sat.warnings(s).iter().any(|w| w.contains("licensed")), "{:?}", sat.warnings(s));
    // Credits name only the layers drawn at the export's zoom, OpenStreetMap once: the imagery
    // and its labels out to zoom 9.5, the close-up map (no imagery) past it.
    let credits = sat.attribution();
    assert_eq!(credits.matches("OpenStreetMap").count(), 1, "{credits}");
    let zoom = lm_mapview::zoom_of(sat.plan().camera.res);
    assert_eq!(credits.contains("NASA Blue Marble"), zoom <= crate::basemap::IMAGERY_MAX_ZOOM, "zoom {zoom}: {credits}");
    s.prefs.basemap = "offline".into();
    s.basemap_changed();
    s.export_sheet = Some(e);
}

#[test]
fn previews_are_left_out_of_exports() {
    let mut map = lm_mapview::map::Map::new("t");
    let ds = lm_jobs::import::Dataset::new("p", lm_jobs::GeometryType::Point, Vec::new());
    for id in [5, 999_998, 999_999, 1_000_001] {
        let mut l = lm_mapview::map::Layer::new(id, ds.clone());
        l.selection = vec![0];
        map.layers.push(l);
    }
    let out = crate::workspace::map_for_export(&map);
    assert_eq!(out.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![5, 1_000_001]);
    assert!(out.layers.iter().all(|l| l.selection.is_empty()));
}

/// The map image sheet: the preview at the slide's exact proportions, with the legend bottom
/// left, the scale bar, title, caption and credits; the settings at the right.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn export_sheet() {
    let mut h = export_scene(|_| {});
    if let Some(e) = h.state_mut().as_mut().and_then(|s| s.export_sheet.as_mut()) {
        e.title = "Schools and districts of Shanghai".into();
    }
    wait_for_export(&mut h);
    h.run_steps(5);
    h.snapshot("export_sheet");
}

/// The map image sheet at the scale of a country: cities across the contiguous United States, Key
/// West to Bellingham (the current view is Shanghai; Fit to data frames the cities). About 6,000 km across and over
/// 25° of latitude: latitude and longitude on the frame, no north arrow, no scale bar.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn export_sheet_us() {
    let dir = std::env::temp_dir().join(format!("lm-export-us-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("us_cities.csv");
    std::fs::write(
        &csv,
        "name,population,lon,lat\n\
         New York,8336817,-74.006,40.713\nLos Angeles,3979576,-118.244,34.052\nChicago,2693976,-87.630,41.878\n\
         Houston,2320268,-95.370,29.760\nPhoenix,1680992,-112.074,33.448\nPhiladelphia,1584064,-75.165,39.953\n\
         San Antonio,1547253,-98.494,29.424\nSan Diego,1423851,-117.161,32.716\nDallas,1343573,-96.797,32.777\n\
         Seattle,753675,-122.332,47.606\nDenver,727211,-104.990,39.739\nBoston,692600,-71.059,42.360\n\
         Miami,467963,-80.192,25.762\nMinneapolis,429954,-93.265,44.978\nPortland,654741,-122.676,45.523\n\
         Atlanta,506811,-84.388,33.749\nSalt Lake City,200567,-111.891,40.761\nKansas City,495327,-94.579,39.100\n\
         Bellingham,92314,-122.479,48.752\nKey West,26444,-81.780,24.555\nBangor,31753,-68.772,44.801\n",
    )
    .unwrap();
    let path = csv.clone();
    let mut h = harness_phase(3, false, SHANGHAI, move |s| {
        s.developer = true;
        let layers = lm_jobs::import::import_path(&path, &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
        s.ws.add_imported(layers, 500);
        s.ws.clear_selection();
        s.ws.new_data = None;
    });
    settle(&mut h);
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::ExportMapImage);
        if let Some(e) = s.export_sheet.as_mut() {
            e.title = "Cities of the contiguous United States".into();
        }
    }
    wait_for_export(&mut h);
    let s = h.state().as_ref().unwrap();
    let plan = s.export_sheet.as_ref().unwrap().plan();
    assert!(plan.width_km > lm_mapview::export::ARROW_MAX_KM, "{}", plan.width_km);
    // Labels and a scale bar, no north arrow (owner's decision, 2026-10-18: 28° and 7,000 km).
    assert_eq!(plan.furniture, lm_mapview::export::Furniture { north_arrow: false, scale_bar: true, labels: true });
    if let Some(out) = std::env::var_os("LM_EXPORT_OUT").map(std::path::PathBuf::from)
        && let Some((pm, _)) = s.export_sheet.as_ref().and_then(|e| e.image(s))
    {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("us.png"), lm_mapview::export::encode_png(&pm).unwrap()).unwrap();
    }
    h.run_steps(5);
    h.snapshot("export_sheet_us");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The data dialog for a points layer with personal fields: phone and email start unchecked,
/// each with its note.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn export_data_dialog() {
    let dir = std::env::temp_dir().join(format!("lm-export-dialog-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("contacts.csv");
    std::fs::write(
        &csv,
        "name,phone,email,students,lon,lat\n\
         Fudan Primary,021-6564-2222,office@fudan-primary.example,640,121.50,31.30\n\
         Xuhui Middle,+86 21 6438 1111,info@xuhui-middle.example,1210,121.43,31.19\n\
         Pudong High,138 0013 8000,admin@pudong-high.example,980,121.55,31.22\n",
    )
    .unwrap();
    let path = csv.clone();
    let mut h = harness_phase(3, false, SHANGHAI, move |s| {
        s.developer = true;
        let layers = lm_jobs::import::import_path(&path, &lm_jobs::import::Ctx::detached(Default::default())).unwrap_or_default();
        s.ws.add_imported(layers, 400);
        s.ws.clear_selection();
        s.ws.new_data = None;
    });
    settle(&mut h);
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::ExportData);
    }
    h.run_steps(10);
    let fields = h.state().as_ref().and_then(|s| s.data_export.as_ref().map(|d| d.fields.clone())).unwrap();
    let on = |n: &str| fields.iter().find(|f| f.name == n).map(|f| (f.on, f.note));
    assert_eq!(on("phone"), Some((false, Some("Looks like phone numbers"))));
    assert_eq!(on("email"), Some((false, Some("Looks like email addresses"))));
    assert_eq!(on("name"), Some((true, None)));
    h.snapshot("export_data_dialog");
    let _ = std::fs::remove_dir_all(&dir);
}

/// WP6.5: the map package dialog shows its size estimate, the sources with their licences (an
/// example's known, a user's file unknown), the folder paths it removes and the personal fields,
/// checked to be left out.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn package_dialog() {
    // A fixed folder: the dialog shows the paths it removes. The example is copied into an
    // `examples` folder there, where its licence is recognised.
    let dir = std::path::PathBuf::from("/tmp/lm-ui-package-shot");
    let _ = std::fs::create_dir_all(dir.join("examples"));
    let airports = dir.join("examples").join("airports.csv");
    std::fs::copy(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples/airports.csv"), &airports).unwrap();
    let csv = dir.join("pupils.csv");
    std::fs::write(
        &csv,
        "school,full_name,phone,lon,lat\n\
         Fudan Primary,Li Wei,021-6564-2222,121.50,31.30\n\
         Xuhui Middle,Wang Fang,+86 21 6438 1111,121.43,31.19\n\
         Pudong High,Zhang Min,138 0013 8000,121.55,31.22\n",
    )
    .unwrap();
    let path = csv.clone();
    let mut h = harness_phase(3, false, SHANGHAI, move |s| {
        s.developer = true;
        let airports = airports.clone();
        let ctx = lm_jobs::import::Ctx::detached(Default::default());
        s.ws.add_imported(lm_jobs::import::import_path(&airports, &ctx).unwrap_or_default(), 400);
        s.ws.add_imported(lm_jobs::import::import_path(&path, &ctx).unwrap_or_default(), 401);
        s.ws.clear_selection();
        s.ws.new_data = None;
    });
    settle(&mut h);
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::ExportPackage);
    }
    h.run_steps(10);
    let d = h.state().as_ref().and_then(|s| s.package_dialog.as_ref().map(|d| (d.sources.clone(), d.fields.clone()))).unwrap();
    assert_eq!(d.0.iter().map(|r| r.file.as_str()).collect::<Vec<_>>(), ["airports.csv"]);
    assert!(d.1.iter().all(|f| f.drop) && d.1.len() == 2);
    h.snapshot("package_dialog");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fresh library cache for a test.
fn library_cache(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("lm-ui-library-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// WP5.4: the welcome card offers the data library when a catalog is set (developer surface
/// until Phase 5).
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn library_welcome_card() {
    let cache = library_cache("welcome");
    let c = cache.clone();
    let mut h = harness_phase(3, false, WORLD, move |s| {
        s.developer = true;
        s.set_library_catalog(crate::library::catalog_url(None, true));
        s.library.cache = Some(c.clone());
    });
    h.run_steps(5);
    assert!(h.state().as_ref().is_some_and(|s| s.library_available()));
    h.snapshot("library_welcome_card");
    let _ = std::fs::remove_dir_all(cache);
}

/// WP5.4: the library panel over the local test catalog (thumbnails, titles in both languages,
/// size and licence); then Add to map downloads into the cache, opens the file through the normal
/// import pipeline and closes the panel.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn library_panel() {
    let cache = library_cache("panel");
    let c = cache.clone();
    let mut h = harness_phase(3, false, WORLD, move |s| {
        s.developer = true;
        s.set_library_catalog(crate::library::catalog_url(None, true));
        s.library.cache = Some(c.clone());
    });
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::OpenLibrary);
    }
    for _ in 0..300 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.library.catalog.is_some() && s.library.thumbs_loaded() == 3) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(10);
    let n = h.state().as_ref().and_then(|s| s.library.catalog.as_ref().map(|c| c.datasets.len()));
    assert_eq!(n, Some(3));
    h.snapshot("library_panel");

    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, crate::Command::AddFromLibrary("shanghai-districts-test".into()));
    }
    for _ in 0..500 {
        h.step();
        if h.state().as_ref().is_some_and(|s| !s.ws.doc.layers.is_empty() && s.ws.idle()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let s = h.state().as_ref().expect("shell");
    assert_eq!(s.ws.doc.layers.len(), 1);
    let l = &s.ws.doc.layers[0];
    assert_eq!(l.profile.kind, lm_core::model::LayerKind::Areas);
    assert_eq!(l.profile.count, 17);
    assert!(cache.join("datasets/shanghai-districts-test/districts.geojson").is_file());
    assert!(!s.library.open, "the panel steps aside once the layer is on its way");
    let _ = std::fs::remove_dir_all(cache);
}

/// The welcome card's Examples menu lists the example datasets (owner's request, 2026-10-18);
/// choosing one opens its files.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn welcome_examples_menu() {
    use egui_kittest::kittest::Queryable as _;
    let mut h = harness_phase(3, false, WORLD, |_| {});
    h.run_steps(5);
    h.get_by_label_contains("Examples").click();
    h.run_steps(5);
    assert!(h.query_by_label("Typhoons and hurricanes").is_some());
    assert!(h.query_by_label("Walled cities of China, 1866").is_some());
    h.snapshot("welcome_examples_menu");
    h.get_by_label("Typhoons and hurricanes").click();
    h.run_steps(3);
    let s = h.state().as_ref().unwrap();
    assert!(
        s.ws.imports.iter().any(|i| i.name().contains("storm tracks")) || s.ws.doc.layers.iter().any(|l| l.display_name == "storm tracks")
    );
}

/// WP6.12: a NetCDF grid with time: the time slider at the bottom of the map ("July 2023", step
/// 2 of 3), and the variable pop-up in the layer's row.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn netcdf_time_slider() {
    use egui_kittest::kittest::Queryable as _;
    let mut h = phase3(&["R8/r8_classic.nc"], WORLD, |s| {
        let id = s.ws.doc.layers[0].id;
        s.ws.set_time(id, 1);
    });
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    let id = s.ws.doc.layers[0].id;
    assert_eq!((s.ws.doc.layers[0].time, s.ws.shown_time(id)), (1, 1));
    assert!(h.query_by_label("Play").is_some() && h.query_by_label("Variable shown").is_some());
    h.snapshot("netcdf_time_slider");
    // → steps forward while the map has the keyboard; ← steps back.
    h.key_press(egui::Key::ArrowRight);
    settle(&mut h);
    assert_eq!(h.state().as_ref().unwrap().ws.doc.layers[0].time, 2);
    h.key_press(egui::Key::ArrowLeft);
    h.key_press(egui::Key::ArrowLeft);
    settle(&mut h);
    assert_eq!(h.state().as_ref().unwrap().ws.doc.layers[0].time, 0);
    // Play steps about twice a second and loops; Pause stops.
    h.get_by_label("Play").click();
    h.run_steps(2);
    assert!(h.state().as_ref().unwrap().playing.is_some());
    h.get_by_label("Pause").click();
    h.run_steps(2);
    assert!(h.state().as_ref().unwrap().playing.is_none());
}

/// WP6.12: the variable pop-up open, listing the grid's variables with their units, the shown one
/// checked; picking one redraws the layer as one undo step.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn netcdf_variable_dropdown() {
    use egui_kittest::kittest::Queryable as _;
    let mut h = phase3(&["R8/r8_classic.nc"], WORLD, |_| {});
    h.get_by_label("Variable shown").click();
    h.run_steps(4);
    h.snapshot("netcdf_variable_dropdown");
    h.get_by_label("Air temperature (K)").click();
    settle(&mut h);
    let s = h.state().as_ref().unwrap();
    let g = s.ws.doc.layers[0].profile.grid.as_ref().unwrap();
    assert_eq!(g.variable, "ta");
    assert_eq!(s.ws.undo.undo_label(), Some("Show Air temperature in r8_classic"));
    // A variable with levels gets the level pop-up under it.
    assert!(h.query_by_label("Level shown").is_some());
}

/// WP6.12 in Pro: the time slider sits just above the Ask field, never over it.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn netcdf_time_slider_above_ask() {
    let mut h = phase3(&["R8/r8_lon360.nc"], WORLD, |s| s.account = crate::account_ui::test_ui(true, lm_account::Region::International));
    settle(&mut h);
    h.snapshot("netcdf_time_slider_pro");
}

/// A global grid zoomed out until the world repeats: it is drawn in every copy of the world in
/// view, filling its column across 180° (a 0–360° file), not once beside bare basemap. A click on
/// a copy picks the cell drawn there, under its own longitude.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
fn raster_world_copies() {
    let mut h = phase3(&["R8/r8_lon360.nc"], (180.0, 10.0, 1.4), |_| {});
    settle(&mut h);
    h.snapshot("raster_world_copies");
    click(&mut h, 160.0 + 360.0, 30.0);
    let s = h.state().as_ref().unwrap();
    let p = s.ws.selection.pixel.as_ref().expect("a cell picked in the copy east");
    assert_eq!((p.lon, p.lat), (160.0, 30.0));
    let id = s.ws.doc.layers[0].id;
    assert_eq!(s.ws.raster_pick(lm_mapview::Coord::new(160.0, 30.0)), Some((id, p.value)));
}

/// Windows display scaling (WP6.2): the screens a student meets first, at 125 % and 150 %, in
/// the window a 1920×1080 laptop gives at each (1536×864 and 1280×720 points), and at 125 % on
/// a 1366×768 laptop, the smallest common screen (1093×614 points less the taskbar). egui lays
/// out in points, so these check that nothing assumed whole pixels: text stays sharp, the map
/// fills its area, and panels, cards and the export sheet fit. Rendered with the bundled fonts,
/// so the images are the same on macOS, where they are compared.
mod scaling {
    use super::*;

    /// A 1920×1080 screen at 125 %.
    const AT_125: (egui::Vec2, f32) = (egui::vec2(1536.0, 864.0), 1.25);
    /// A 1920×1080 screen at 150 %.
    const AT_150: (egui::Vec2, f32) = (egui::vec2(1280.0, 720.0), 1.5);
    /// A 1366×768 screen at 125 %, less a 48 px taskbar and the title bar: below the window's
    /// usual minimum, which Windows lowers to fit (apps/layeredmap main.rs).
    const SMALL_125: (egui::Vec2, f32) = (egui::vec2(1093.0, 560.0), 1.25);

    fn first_launch((size, ppp): (egui::Vec2, f32), name: &str) {
        let mut h = harness_sized(size, ppp, lm_core::phase::BUILD_PHASE, false, WORLD, |_| {});
        let px = h.ctx.pixels_per_point();
        assert_eq!(px, ppp);
        h.snapshot(name);
    }

    fn object_card((size, ppp): (egui::Vec2, f32), name: &str) {
        let mut h = phase3_sized(size, ppp, &["V1/districts.geojson"], SHANGHAI, |_| {});
        click(&mut h, 121.26, 31.41);
        let rows = h.state().as_ref().map(|s| s.ws.selection.features.len()).unwrap_or(0);
        assert_eq!(rows, 1, "one district selected");
        h.snapshot(name);
    }

    fn export_sheet((size, ppp): (egui::Vec2, f32), name: &str) {
        let mut h = export_scene_sized(size, ppp, |_| {});
        if let Some(e) = h.state_mut().as_mut().and_then(|s| s.export_sheet.as_mut()) {
            e.title = "Schools and districts of Shanghai".into();
        }
        wait_for_export(&mut h);
        h.run_steps(5);
        h.snapshot(name);
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn first_launch_at_125() {
        first_launch(AT_125, "scale125_first_launch");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn first_launch_at_150() {
        first_launch(AT_150, "scale150_first_launch");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn object_card_at_125() {
        object_card(AT_125, "scale125_object_card");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn object_card_at_150() {
        object_card(AT_150, "scale150_object_card");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn object_card_on_a_small_screen_at_125() {
        object_card(SMALL_125, "scale125_small_object_card");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn export_sheet_at_125() {
        export_sheet(AT_125, "scale125_export_sheet");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn export_sheet_at_150() {
        export_sheet(AT_150, "scale150_export_sheet");
    }

    /// Without comparing pixels (so on Windows CI too): at 125 % and 150 % the map is drawn at
    /// the screen's own resolution, whole pixels covering the map's area in points.
    #[test]
    fn the_map_renders_at_the_screens_resolution() {
        for (size, ppp) in [AT_125, AT_150, SMALL_125] {
            let h = harness_sized(size, ppp, 1, false, WORLD, |_| {});
            let s = h.state().as_ref().unwrap();
            assert!(s.map_view().renders > 0, "the map drew at {ppp}");
            let rect = s.map_view().rect().unwrap();
            let want = [(rect.width() * ppp).round() as usize, (rect.height() * ppp).round() as usize];
            assert_eq!(s.map_view().frame_pixel_size(), Some(want), "at {ppp}");
        }
    }
}

/// Settings › Account (WP7.4), as its states look: signed out with the international ways to
/// sign in, the email sent (its 6-digit code, and a wrong one), signed in, Delete Account's
/// confirmation, and mainland China (not built yet).
mod account {
    use super::*;
    use lm_account::Region;

    fn settings(signed_in: bool, region: Region, setup: impl Fn(&mut Shell) + 'static) -> Harness<'static, Option<Shell>> {
        let mut h = harness(false, WORLD, move |s| {
            s.account = crate::account_ui::test_ui(signed_in, region);
            s.account.open = true;
            setup(s);
        });
        h.run_steps(5);
        h
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn account_signed_out() {
        let mut h = settings(false, Region::International, |_| {});
        use egui_kittest::kittest::Queryable as _;
        for b in ["Continue with Apple", "Continue with Google", "Continue with X", "Email me a sign-in code"] {
            assert!(h.query_by_label(b).is_some(), "{b}");
        }
        h.snapshot("account_signed_out");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn account_signed_in() {
        let mut h = settings(true, Region::International, |_| {});
        use egui_kittest::kittest::Queryable as _;
        assert!(h.query_by_label("kim@example.com").is_some());
        assert!(h.query_by_label("Sign Out").is_some() && h.query_by_label("Delete Account…").is_some());
        h.snapshot("account_signed_in");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn account_delete_confirmation() {
        settings(true, Region::International, |s| s.account.confirm_delete = true).snapshot("account_delete_confirm");
    }

    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn account_mainland_china() {
        settings(false, Region::China, |_| {}).snapshot("account_china");
    }

    fn email_sent(s: &mut Shell) {
        s.account.email_code = Some(crate::account_ui::EmailCode::new("kim@example.com".into(), Region::International));
    }

    /// After "Email me a sign-in code": the link works on this Mac, or the 6-digit code is typed.
    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn account_email_code() {
        let mut h = settings(false, Region::International, email_sent);
        use egui_kittest::kittest::Queryable as _;
        assert!(h.query_by_label_contains("We sent an email to kim@example.com. Click the link in it on this Mac").is_some());
        for b in ["6-digit code", "Verify", "Send a New Email", "Use a Different Email"] {
            assert!(h.query_by_label(b).is_some(), "{b}");
        }
        h.snapshot("account_email_code");
    }

    /// A code the service refused: the plain message under the field, the digits kept to fix.
    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn account_wrong_code() {
        let mut h = settings(false, Region::International, |s| {
            email_sent(s);
            if let Some(ec) = s.account.email_code.as_mut() {
                ec.code = "123465".into();
                ec.error = Some(lm_account::AccountError::WrongCode.to_string());
            }
        });
        use egui_kittest::kittest::Queryable as _;
        assert!(h.query_by_label("That code didn't work. Check it, or send a new email.").is_some());
        h.snapshot("account_wrong_code");
    }

    /// Typing (or pasting) "123 456" keeps the six digits, Return checks them, and the answer lands
    /// under the field (this test's account has no network). "Use a Different Email" goes back to
    /// the address, ready to edit.
    #[test]
    fn the_code_field_takes_digits_and_return_checks_them() {
        let mut h = settings(false, Region::International, email_sent);
        use egui_kittest::kittest::Queryable as _;
        h.get_by_label("6-digit code").focus();
        h.run_steps(2);
        h.get_by_label("6-digit code").type_text("123 456 7x");
        h.run_steps(2);
        let code =
            |h: &Harness<'static, Option<Shell>>| h.state().as_ref().and_then(|s| s.account.email_code.as_ref().map(|e| e.code.clone()));
        assert_eq!(code(&h).as_deref(), Some("123456"));
        h.key_press(egui::Key::Enter);
        for _ in 0..100 {
            h.run_steps(1);
            if h.state().as_ref().and_then(|s| s.account.email_code.as_ref()).is_some_and(|e| e.error.is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let err = h.state().as_ref().and_then(|s| s.account.email_code.as_ref()).and_then(|e| e.error.clone()).unwrap_or_default();
        assert!(err.starts_with("Couldn't reach the sign-in service"), "{err}");
        h.get_by_label("Use a Different Email").click();
        h.run_steps(3);
        let s = h.state().as_ref().unwrap();
        assert!(s.account.email_code.is_none());
        assert_eq!(s.account.email, "kim@example.com");
    }

    /// The Ask box while Pro is locked, signed out: the sign-in prompt takes the field's place (and
    /// with it the microphone).
    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn ask_sign_in_prompt() {
        let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
            s.account = crate::account_ui::test_ui(false, Region::International);
        });
        use egui_kittest::kittest::Queryable as _;
        assert!(h.query_by_label("Sign In…").is_some());
        assert!(h.query_by_label("Ask about your layers").is_none(), "no field while signed out");
        assert!(h.query_by_label("Speak your request").is_none(), "no microphone while signed out");
        assert!(!h.state().as_ref().is_some_and(|s| s.pro()));
        h.snapshot("ask_sign_in_prompt");
        // Sign In… opens Settings › Account.
        h.get_by_label("Sign In…").click();
        h.run_steps(5);
        assert!(h.state().as_ref().is_some_and(|s| s.account.open));
    }

    /// Signed in to an account without Pro (once Pro needs payment): the Ask box says so plainly
    /// and opens Settings › Account; asking from a script is refused the same way.
    #[test]
    #[cfg_attr(not(target_os = "macos"), ignore = "approved images are from macOS")]
    fn ask_not_pro_prompt() {
        let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
            s.account = crate::account_ui::test_ui(true, Region::International);
            if let Some(a) = &s.account.account {
                a.note_ask_answer(false);
            }
        });
        use egui_kittest::kittest::Queryable as _;
        assert_eq!(h.state().as_ref().map(|s| s.pro_access()), Some(lm_account::Access::NotPro));
        assert!(h.query_by_label("NimbleMap Pro isn't on for this account.").is_some());
        assert!(h.query_by_label("Ask about your layers").is_none());
        h.snapshot("ask_not_pro_prompt");
        let ctx = h.ctx.clone();
        if let Some(s) = h.state_mut().as_mut() {
            s.command(&ctx, crate::Command::Ask("schools near rivers".into()));
            assert_eq!(s.ask.message(), Some("NimbleMap Pro isn't on for this account."));
            assert!(!s.ask.busy(), "nothing was sent");
        }
        // The refusal shows in the Ask box above the prompt, which moves Settings… (the box
        // settles in its new place on its own, ask::ask_box): let it, before clicking.
        h.run();
        h.get_by_label("Settings…").click();
        h.run_steps(5);
        assert!(h.state().as_ref().is_some_and(|s| s.account.open));
    }
}
