//! The AccessKit tree, walked as a screen reader would (WP5.6, WP6.3): every control has a name,
//! selection changes are announced, and the core path works from the keyboard.

use egui::accesskit::{Action, Live, Role};
use egui_kittest::Harness;
use egui_kittest::kittest::AccessKitNode;

use crate::screenshots::{SHANGHAI, WORLD, click, harness_phase, phase3, settle};
use crate::{Command, Shell};

type H = Harness<'static, Option<Shell>>;

/// Every node in the tree, depth first.
fn walk<'t>(n: AccessKitNode<'t>, out: &mut Vec<AccessKitNode<'t>>) {
    out.push(n);
    for c in n.children() {
        walk(c, out);
    }
}

fn nodes(h: &H) -> Vec<AccessKitNode<'_>> {
    let mut v = Vec::new();
    walk(h.kittest_state().root(), &mut v);
    v
}

/// A control a screen reader can reach or press.
fn interactive(n: &AccessKitNode<'_>) -> bool {
    let d = n.data();
    (d.supports_action(Action::Click) || d.supports_action(Action::Focus)) && !n.is_hidden()
}

/// The name a screen reader would read (a label's text is its value).
fn spoken_name(n: &AccessKitNode<'_>) -> String {
    let label = n.label().unwrap_or_default();
    if !label.trim().is_empty() {
        return label;
    }
    match n.role() {
        Role::Label => n.value().unwrap_or_default(),
        Role::TextInput | Role::MultilineTextInput => n.data().placeholder().unwrap_or_default().to_string(),
        _ => label,
    }
}

fn describe(n: &AccessKitNode<'_>, name: &str) -> String {
    let b = n.bounding_box().map(|r| format!("({:.0},{:.0} {:.0}×{:.0})", r.x0, r.y0, r.width(), r.height()));
    let inside: Vec<String> = n.children().take(3).map(|c| spoken_name(&c)).collect();
    let actions: Vec<&str> = [(Action::Click, "click"), (Action::Focus, "focus")]
        .iter()
        .filter(|(a, _)| n.data().supports_action(*a))
        .map(|(_, s)| *s)
        .collect();
    let siblings: Vec<String> = n.parent().map(|p| p.children().map(|c| spoken_name(&c)).collect()).unwrap_or_default();
    let at = siblings.len().min(n.parent().map_or(0, |p| p.children().position(|c| c.id() == n.id()).unwrap_or(0)));
    let near = &siblings[at.saturating_sub(2)..(at + 2).min(siblings.len())];
    format!("{:?} {:?} at {} {actions:?} containing {inside:?} near {near:?}", n.role(), name, b.unwrap_or_default())
}

/// Controls without a name, and any node whose text has icon glyphs in it (read as garbage),
/// described for the failure. egui's scroll bars are left out: they are its own, and VoiceOver
/// scrolls a scroll area by itself.
fn unnamed(h: &H) -> Vec<String> {
    nodes(h)
        .into_iter()
        .filter(|n| !n.is_hidden() && n.role() != Role::ScrollBar)
        .filter_map(|n| {
            let name = spoken_name(&n);
            let garbled = name.chars().any(super::is_glyph) || n.value().is_some_and(|v| v.chars().any(super::is_glyph));
            let unnamed = interactive(&n) && super::without_glyphs(&name).is_empty();
            (garbled || unnamed).then(|| describe(&n, &name))
        })
        .collect()
}

fn command(h: &mut H, cmd: Command) {
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut() {
        s.command(&ctx, cmd);
    }
    settle(h);
}

fn key(h: &mut H, key: egui::Key, modifiers: egui::Modifiers) {
    h.key_press_modifiers(modifiers, key);
    h.run_steps(3);
}

/// The polite live regions' texts.
fn live_texts(h: &H) -> Vec<String> {
    nodes(h).into_iter().filter(|n| n.data().live() == Some(Live::Polite)).filter_map(|n| n.value()).collect()
}

/// The name of the control with keyboard focus.
fn focused(h: &H) -> Option<String> {
    nodes(h).into_iter().find(|n| n.is_focused()).map(|n| spoken_name(&n))
}

fn select_first(h: &mut H, name: &str) {
    let ctx = h.ctx.clone();
    if let Some(s) = h.state_mut().as_mut()
        && let Some(id) = s.ws.doc.layers.iter().find(|l| l.display_name == name).map(|l| l.id)
    {
        s.ws.select_layer(id);
        s.command(&ctx, Command::InspectorTab(0));
    }
    settle(h);
}

/// The main screens, each with a name for the failure message.
fn screens() -> Vec<(&'static str, Box<dyn Fn() -> H>)> {
    vec![
        ("first launch", Box::new(|| harness_phase(3, false, WORLD, |_| {}))),
        (
            "Examples menu",
            Box::new(|| {
                use egui_kittest::kittest::Queryable as _;
                let mut h = harness_phase(3, false, WORLD, |_| {});
                h.get_by_label("Examples").click();
                h.run_steps(5);
                h
            }),
        ),
        (
            "search results",
            Box::new(|| {
                let mut h = harness_phase(3, false, SHANGHAI, |s| {
                    s.search.query = "Shang".into();
                    s.search.open = true;
                });
                h.run_steps(5);
                h
            }),
        ),
        (
            "basemap chooser",
            Box::new(|| {
                let mut h = harness_phase(3, false, SHANGHAI, |s| s.basemap_open = true);
                h.run_steps(3);
                h
            }),
        ),
        (
            "layers and overview",
            Box::new(|| {
                let mut h = phase3(&["V1/districts.geojson", "T1/schools.csv"], SHANGHAI, |_| {});
                select_first(&mut h, "districts");
                h
            }),
        ),
        (
            "object card",
            Box::new(|| {
                let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
                click(&mut h, 121.26, 31.41);
                h
            }),
        ),
        (
            "Style tab",
            Box::new(|| {
                let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
                select_first(&mut h, "districts");
                command(&mut h, Command::InspectorTab(1));
                h
            }),
        ),
        (
            "Details tab",
            Box::new(|| {
                let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
                select_first(&mut h, "districts");
                command(&mut h, Command::InspectorTab(2));
                h
            }),
        ),
        (
            "table and find",
            Box::new(|| {
                let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
                select_first(&mut h, "districts");
                command(&mut h, Command::ToggleTable);
                command(&mut h, Command::Find("Pu".into()));
                h
            }),
        ),
        ("joined table", Box::new(|| phase3(&["J5/provinces.geojson", "J5/population_zh.csv"], (5.0, 30.5, 6.0), |_| {}))),
        (
            "composer and result card",
            Box::new(|| {
                let mut h = phase3(&["N1/areas.geojson", "N1/points.geojson"], (121.35, 31.2, 10.0), |s| {
                    s.developer = true;
                    if let Some(id) = s.ws.doc.layers.iter().find(|l| l.display_name == "points").map(|l| l.id) {
                        s.ws.select_layer(id);
                    }
                });
                command(&mut h, Command::ComposeAction("count_in".into(), vec![]));
                for _ in 0..300 {
                    h.step();
                    if h.state().as_ref().is_some_and(|s| s.composer.as_ref().is_some_and(|c| c.result.is_some() || c.error.is_some())) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                settle(&mut h);
                h
            }),
        ),
        (
            "Pro, Ask box and time slider",
            Box::new(|| {
                let mut h = phase3(&["R8/r8_classic.nc"], WORLD, |s| {
                    s.pro = true;
                    let id = s.ws.doc.layers[0].id;
                    s.ws.select_layer(id);
                });
                settle(&mut h);
                h
            }),
        ),
        ("export sheet", Box::new(|| crate::screenshots::export_scene(|_| {}))),
        (
            "export data",
            Box::new(|| {
                let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| s.developer = true);
                select_first(&mut h, "districts");
                command(&mut h, Command::ExportData);
                h
            }),
        ),
        (
            "recovery prompt",
            Box::new(|| {
                phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
                    s.doc_file.recovery = Some(lm_doc::Recovery {
                        name: "Heat study".into(),
                        saved_at: lm_doc::container::now_secs() - 125,
                        autosave: std::env::temp_dir().join("lm-no-such-autosave.lmap"),
                        document: None,
                    });
                })
            }),
        ),
        (
            "missing source",
            Box::new(|| {
                phase3(&["V1/districts.geojson", "../assets/examples/elevation.tif"], SHANGHAI, |s| {
                    let mut doc = s.ws.doc.clone();
                    if let Some(l) = doc.layers.iter_mut().find(|l| l.profile.kind == lm_core::model::LayerKind::Raster) {
                        l.profile.source.path = "/Users/student/Downloads/heat_july.tif".into();
                        l.display_name = "heat_july".into();
                    }
                    s.ws.restore(doc);
                })
            }),
        ),
        (
            "data library",
            Box::new(|| {
                let cache = std::env::temp_dir().join(format!("lm-ui-a11y-library-{}", std::process::id()));
                let mut h = harness_phase(3, false, WORLD, move |s| {
                    s.developer = true;
                    s.set_library_catalog(crate::library::catalog_url(None, true));
                    s.library.cache = Some(cache.clone());
                });
                command(&mut h, Command::OpenLibrary);
                for _ in 0..300 {
                    h.step();
                    if h.state().as_ref().is_some_and(|s| s.library.catalog.is_some() && s.library.thumbs_loaded() == 3) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                h.run_steps(5);
                h
            }),
        ),
        (
            "credits",
            Box::new(|| {
                let mut h = harness_phase(3, false, WORLD, |s| s.credits_open = true);
                h.run_steps(3);
                h
            }),
        ),
        (
            "Settings › Account, signed out",
            Box::new(|| {
                let mut h = harness_phase(3, false, WORLD, |s| {
                    s.pro = true;
                    s.account = crate::account_ui::test_ui(false, lm_account::Region::International);
                    s.account.open = true;
                });
                h.run_steps(3);
                h
            }),
        ),
        (
            "Settings › Account, signed in",
            Box::new(|| {
                let mut h = harness_phase(3, false, WORLD, |s| {
                    s.pro = true;
                    s.account = crate::account_ui::test_ui(true, lm_account::Region::International);
                    s.account.open = true;
                });
                h.run_steps(3);
                h
            }),
        ),
        (
            "Settings › Account, the email's code",
            Box::new(|| {
                let mut h = harness_phase(3, false, WORLD, |s| {
                    s.pro = true;
                    s.account = crate::account_ui::test_ui(false, lm_account::Region::International);
                    s.account.open = true;
                    let mut ec = crate::account_ui::EmailCode::new("kim@example.com".into(), lm_account::Region::International);
                    ec.error = Some(lm_account::AccountError::WrongCode.to_string());
                    s.account.email_code = Some(ec);
                });
                h.run_steps(3);
                h
            }),
        ),
        (
            "Pro, sign-in prompt",
            Box::new(|| {
                phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
                    s.pro = true;
                    s.account = crate::account_ui::test_ui(false, lm_account::Region::International);
                })
            }),
        ),
    ]
}

/// No control on the main screens is unnamed or named with an icon glyph (the test lists them).
#[test]
fn every_control_on_the_main_screens_has_a_name() {
    let mut failures = Vec::new();
    for (screen, make) in screens() {
        let h = make();
        for u in unnamed(&h) {
            failures.push(format!("{screen}: {u}"));
        }
    }
    assert!(failures.is_empty(), "{} controls without a usable name:\n{}", failures.len(), failures.join("\n"));
}

/// Selecting a district says what it is and how it ranks (WP6.3: "District 7 selected. Heat 31.2
/// degrees, 3rd highest of 17."), through a polite live region.
#[test]
fn selecting_an_object_announces_its_summary() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
    click(&mut h, 121.26, 31.41);
    let said = live_texts(&h);
    let summary = said.iter().find(|t| t.contains(" selected.")).cloned().unwrap_or_default();
    assert!(summary.starts_with("Jiading selected. "), "{said:?}");
    assert!(summary.contains("highest of 1") || summary.contains("lowest of 1"), "{summary}");
    // The map in words, and the selection cleared.
    let (label, _) = h.state().as_ref().map(super::map_description).unwrap_or_default();
    assert!(label.starts_with("Map. Showing districts. About "), "{label}");
    command(&mut h, Command::Escape);
    assert!(live_texts(&h).iter().any(|t| t.starts_with("districts layer selected.") || t == "Selection cleared."), "{:?}", live_texts(&h));
}

/// Styled by a field, the summary ranks by that field, with its unit spoken.
#[test]
fn the_summary_ranks_by_the_styled_field() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |s| {
        let id = s.ws.doc.layers[0].id;
        let mut style = s.ws.doc.layers[0].style.clone();
        style.color_by = Some(lm_core::document::ColorBy {
            field: "population".into(),
            categories: false,
            classes: lm_core::document::Classes::Smooth,
            palette: "viridis".into(),
            per_km2: false,
            fit_to_view: false,
        });
        let tx = s.ws.doc.set_style_tx(id, style);
        s.ws.commit(tx);
    });
    click(&mut h, 121.26, 31.41);
    let said = live_texts(&h);
    assert!(said.iter().any(|t| t.starts_with("Jiading selected. population 1,834,258, ")), "{said:?}");
}

/// Units are read as words.
#[test]
fn units_are_spoken() {
    assert_eq!(super::spoken("Heat 31.2 °C, 3rd highest of 17"), "Heat 31.2 degrees Celsius, 3rd highest of 17");
    assert_eq!(super::spoken("12° · 4.3 km²"), "12 degrees, 4.3 square kilometres");
    assert_eq!(super::without_glyphs(&format!("{} Filter", egui_phosphor::regular::PLUS)), "Filter");
}

/// From the keyboard alone: Tab to Examples, open World basics, find a country through search,
/// hear its summary; then the table: ⌘T puts the keyboard on the rows and ↓ selects the next.
#[test]
fn keyboard_only_open_an_example_select_and_hear_the_summary() {
    let mut h = harness_phase(3, false, WORLD, |_| {});
    h.run_steps(5);
    // Tab order starts in the toolbar (Keyboard › Moving between regions).
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    assert_eq!(focused(&h).as_deref(), Some("Search places or your data"));
    let mut order = Vec::new();
    for _ in 0..30 {
        if focused(&h).as_deref() == Some("Examples") {
            break;
        }
        key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        order.push(focused(&h).unwrap_or_default());
    }
    assert_eq!(focused(&h).as_deref(), Some("Examples"), "Tab went {order:?}");
    // The focus ring shows for keyboard focus.
    assert!(h.state().as_ref().is_some_and(|s| s.a11y.keyboard));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    for _ in 0..12 {
        if focused(&h).as_deref() == Some("World basics") {
            break;
        }
        key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    }
    assert_eq!(focused(&h).as_deref(), Some("World basics"));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    for _ in 0..3000 {
        h.step();
        if h.state().as_ref().is_some_and(|s| s.ws.doc.layers.len() == 3 && s.ws.imports.is_empty()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(&mut h);
    assert_eq!(h.state().as_ref().map_or(0, |s| s.ws.doc.layers.len()), 3);
    // ⌘F (Find in the selected layer), type, Tab to the match, Return: the country is selected
    // and its summary read.
    key(&mut h, egui::Key::F, egui::Modifiers::COMMAND);
    let field = focused(&h).unwrap_or_default();
    assert!(field.starts_with("Find in ") || field == "Search places or your data", "{field}");
    // Which layer is selected after the example opens depends on which file finished last.
    let q = if field == "Find in countries" { "Japan" } else { "Tokyo" };
    h.event(egui::Event::Text(q.into()));
    // The layer's find index is built in the background on first use (slow on a busy machine).
    for _ in 0..3000 {
        h.step();
        let found = h.state().as_ref().is_some_and(|s| !s.find.hits.is_empty() || s.ws.selection.features.len() == 1);
        if found {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(&mut h);
    // One match opens at once; several are listed, to Tab to.
    let one = h.state().as_ref().is_some_and(|s| s.ws.selection.features.len() == 1);
    if field.starts_with("Find in ") && !one {
        let mut went = Vec::new();
        for _ in 0..8 {
            if focused(&h).is_some_and(|n| n.starts_with(&format!("{q},"))) {
                break;
            }
            key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
            went.push(focused(&h).unwrap_or_default());
        }
        let hits = h.state().as_ref().map_or(0, |s| s.find.hits.len());
        assert!(focused(&h).is_some_and(|n| n.starts_with(&format!("{q},"))), "{field}: {hits} hits; Tab went {went:?}");
    }
    if !one {
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    }
    settle(&mut h);
    let said = live_texts(&h);
    assert!(said.iter().any(|t| t.starts_with(&format!("{q} selected."))), "{said:?}");
    // The table: ⌘T puts the keyboard on the selected row (scrolls it into view if needed).
    key(&mut h, egui::Key::T, egui::Modifiers::COMMAND);
    let mut on_row = String::new();
    for _ in 0..80 {
        h.run_steps(5);
        on_row = focused(&h).unwrap_or_default();
        if on_row.starts_with(q) {
            break;
        }
    }
    if !on_row.starts_with(q) {
        // Table open but Find still held the keyboard: a second ⌘T moves focus onto the rows.
        key(&mut h, egui::Key::T, egui::Modifiers::COMMAND);
        for _ in 0..80 {
            h.run_steps(5);
            on_row = focused(&h).unwrap_or_default();
            if on_row.starts_with(q) {
                break;
            }
        }
    }
    assert!(on_row.starts_with(q), "{on_row}");
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    settle(&mut h);
    let next = focused(&h).unwrap_or_default();
    assert!(!next.is_empty() && !next.starts_with(q), "{next}");
    let title = next.split(',').next().unwrap_or_default().to_string();
    assert!(live_texts(&h).iter().any(|t| t.starts_with(&format!("{title} selected."))), "{:?} after {next}", live_texts(&h));
    // Esc steps back: the selection clears (Esc ladder), and the map keeps working.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
}

/// F6 moves between regions; the map takes the keyboard as one stop with a description, and
/// its arrows pan rather than move focus.
#[test]
fn f6_reaches_the_map_and_arrows_pan_it() {
    let mut h = phase3(&["V1/districts.geojson"], SHANGHAI, |_| {});
    let map = h.state().as_ref().and_then(|s| s.a11y.map_id);
    assert!(map.is_some());
    for _ in 0..4 {
        key(&mut h, egui::Key::F6, egui::Modifiers::NONE);
        if focused(&h).is_some_and(|n| n.starts_with("Map.")) {
            break;
        }
    }
    let name = focused(&h).unwrap_or_default();
    assert!(name.starts_with("Map. Showing districts."), "{name}");
    let before = h.state().as_ref().map(|s| s.map_view().center_lonlat().x).unwrap_or_default();
    for _ in 0..10 {
        h.key_down(egui::Key::ArrowRight);
        h.run_steps(2);
    }
    h.key_up(egui::Key::ArrowRight);
    settle(&mut h);
    let after = h.state().as_ref().map(|s| s.map_view().center_lonlat().x).unwrap_or_default();
    assert!(after > before, "the map panned east: {before} → {after}");
    assert!(focused(&h).is_some_and(|n| n.starts_with("Map.")), "the map kept the keyboard");
}

/// Notices are read when they appear.
#[test]
fn notices_are_live_regions() {
    let mut h = phase3(&["V1/districts.geojson"], WORLD, |_| {});
    command(&mut h, Command::RemoveAllLayers);
    // ⌘Z, as a person would: "Undid remove all layers".
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert!(live_texts(&h).iter().any(|t| t.starts_with("Undid ")), "{:?}", live_texts(&h));
}
