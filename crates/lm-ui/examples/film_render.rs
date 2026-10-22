//! The launch film's offline renderer (developer tool, not part of the app): renders the real
//! NimbleMap interface frame by frame, offscreen (egui + wgpu through egui_kittest), at a fixed
//! size, with the map camera set exactly for every frame and egui's clock set to the frame's time.
//! A frame is saved only once the map has the finished render of that camera: every basemap tile
//! and layer drawn, no lower-zoom stand-in tiles (`MapView::settled`). Timing never depends on how
//! long tiles take to load.
//!
//! ```text
//! cargo run --release -p lm-ui --example film_render -- --render-path shot.json --out frames/
//! ```
//!
//! `shot.json` (written by design/launch-video-v3/path/):
//! ```json
//! { "size": [1409, 845], "ppp": 2.0, "theme": "light", "basemap": "auto", "dev_pro": false,
//!   "files": ["/path/storms.shp"],
//!   "setup": ["esc", "card style_by_value magnitude"],
//!   "frames": [ {"t": 0.0, "cam": [-62.0, 33.0, 4.6], "out": "000000.png"}, … ],
//!   "events": [ {"t": 1.2, "focus": "search"}, {"t": 1.3, "text": "N"}, {"t": 2.0, "key": "Enter"},
//!               {"t": 5.0, "step": "click -76.6,39.95"} ] }
//! ```
//! Events happen in the app as a person or a `--tour` step would make them: `focus` puts the
//! keyboard in a field (`search`, `find`, `ask`), `text` types into it, `key` presses a key
//! (`Enter`, `Escape`), `click_at` clicks a point of the window (`[x, y]` in points; an overlap
//! chooser's row, say), and `step` runs one `--tour` step (`click lon,lat`, `esc`, `layer N`,
//! `tab N`, `card ID [FIELD]`, `time N`, `find TEXT`, `ask TEXT`, `preset N`). Only the camera is
//! the film's: a search result's own flight is replaced by the frame's camera. Each frame's
//! events run before it is drawn. Frames already in `--out` are kept unless `--force`.
//! Prints one line per frame: its name, the steps it took to settle, and the milliseconds.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use egui_kittest::Harness;
use lm_core::phase::Flags;
use lm_mapview::map::camera::Camera;
use lm_mapview::{Coord, lonlat_to_merc, res_of};
use lm_ui::{Command, Prefs, Shell, System, ThemePref};
use serde_json::Value;

#[derive(Default)]
struct Pending {
    commands: Vec<Command>,
    focus: Option<String>,
    cam: Option<(f64, f64, f64)>,
    /// The last camera the app moved to by itself (printed once, for writing the film's path).
    app_camera: Option<String>,
}

struct State {
    shell: Option<Shell>,
}

fn step_command(s: &str) -> Option<Command> {
    let s = s.trim();
    let (word, rest) = s.split_once(' ').unwrap_or((s, ""));
    let nums = |r: &str| r.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).collect::<Vec<f64>>();
    Some(match word {
        "click" | "shift-click" => match nums(rest).as_slice() {
            [lon, lat] => Command::Click(*lon, *lat, word == "shift-click"),
            _ => return None,
        },
        "layer" => Command::SelectLayerAt(rest.trim().parse().ok()?),
        "tab" => Command::InspectorTab(rest.trim().parse().ok()?),
        "preset" => Command::ApplyPreset(rest.trim().parse().ok()?),
        "esc" => Command::Escape,
        "find" => Command::Find(rest.to_string()),
        "card" => {
            let (id, field) = rest.trim().split_once(' ').map_or((rest.trim(), None), |(a, b)| (a, Some(b.trim().to_string())));
            Command::CardAction(id.to_string(), field)
        }
        "time" => Command::SetTime(rest.trim().parse().ok()?),
        "ask" => Command::Ask(rest.trim().to_string()),
        "keep-ask" => Command::KeepAsk,
        "example" => Command::OpenExample(rest.trim().to_string()),
        "basemap" => Command::SetBasemap(rest.trim().to_string()),
        _ => return None,
    })
}

fn cam3(v: &Value) -> Option<(f64, f64, f64)> {
    let a = v.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let path = PathBuf::from(arg("--render-path").expect("--render-path shot.json"));
    let out = PathBuf::from(arg("--out").expect("--out dir"));
    let force = args.iter().any(|a| a == "--force");
    let max_steps: usize = arg("--max-steps").and_then(|v| v.parse().ok()).unwrap_or(6000);
    std::fs::create_dir_all(&out).expect("out dir");
    let shot: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("read shot")).expect("shot json");

    let size = shot["size"].as_array().map_or((1409.0, 845.0), |a| (a[0].as_f64().unwrap_or(1409.0), a[1].as_f64().unwrap_or(845.0)));
    let ppp = shot["ppp"].as_f64().unwrap_or(2.0) as f32;
    let dark = shot["theme"].as_str() == Some("dark");
    let basemap = shot["basemap"].as_str().unwrap_or("auto").to_string();
    let dev_pro = shot["dev_pro"].as_bool().unwrap_or(false);
    let files: Vec<PathBuf> =
        shot["files"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(PathBuf::from)).collect()).unwrap_or_default();
    let setup: Vec<Command> =
        shot["setup"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().and_then(step_command)).collect()).unwrap_or_default();
    let frames = shot["frames"].as_array().cloned().unwrap_or_default();
    let mut events: Vec<(f64, Value)> =
        shot["events"].as_array().map(|a| a.iter().map(|e| (e["t"].as_f64().unwrap_or(0.0), e.clone())).collect()).unwrap_or_default();
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let first_cam = frames.first().and_then(|f| cam3(&f["cam"])).unwrap_or((0.0, 20.0, 2.0));

    let pending = Arc::new(Mutex::new(Pending { cam: Some(first_cam), ..Pending::default() }));
    let prefs = Prefs { theme: if dark { ThemePref::Dark } else { ThemePref::Light }, basemap, view: Some(first_cam), ..Prefs::default() };
    let system = System { dark_mode: dark, reduce_motion: false };
    let flags = Flags::from_env(std::env::var("LM_PHASE").ok().as_deref());
    let p2 = pending.clone();
    let mut h = Harness::builder().with_size(egui::vec2(size.0 as f32, size.1 as f32)).with_pixels_per_point(ppp).wgpu().build_ui_state(
        move |ui, state: &mut State| {
            let ctx = ui.ctx().clone();
            let shell = state.shell.get_or_insert_with(|| {
                let mut s = Shell::new(&ctx, flags, prefs.clone(), system);
                s.dev_pro = dev_pro;
                s.start_account();
                lm_ui::basemap::start_archive_lookup(&ctx);
                s.map_view_mut().set_frame_by_frame(true);
                if !files.is_empty() {
                    s.command(&ctx, Command::Open(files.clone()));
                }
                s
            });
            let (cmds, focus, cam) = {
                let mut p = p2.lock().unwrap();
                (std::mem::take(&mut p.commands), p.focus.take(), p.cam)
            };
            for c in cmds {
                shell.command(&ctx, c);
            }
            if let Some(f) = focus {
                let id = match f.as_str() {
                    "search" => Some(egui::Id::new("lm-search")),
                    "ask" => Some(egui::Id::new("lm-ask-field")),
                    "find" => shell.ws.selected().map(|l| egui::Id::new(("lm-find", l.0))),
                    _ => None,
                };
                if let Some(id) = id {
                    ctx.memory_mut(|m| m.request_focus(id));
                }
            }
            if let Some((lon, lat, z)) = cam {
                // (always: it also ends any flight the app started, even one still at its start)
                let c = Camera { center: lonlat_to_merc(Coord::new(lon, lat)), res: res_of(z) };
                shell.map_view_mut().set_camera(c);
            }
            shell.ui(ui, system);
            if let Some((lon, lat, z)) = cam {
                let c = Camera { center: lonlat_to_merc(Coord::new(lon, lat)), res: res_of(z) };
                if shell.map_view().camera() != c {
                    let m = shell.map_view();
                    let ll = m.center_lonlat();
                    let key = format!("{:.4},{:.4},{:.3}", ll.x, ll.y, m.zoom());
                    let mut p = p2.lock().unwrap();
                    if p.app_camera.as_deref() != Some(key.as_str()) {
                        println!("app-camera {key}");
                        p.app_camera = Some(key);
                    }
                }
            }
            // The film's camera, not a search result's flight: a flight started this frame is
            // replaced before the map draws again.
            if let Some((lon, lat, z)) = cam {
                // (always: it also ends any flight the app started, even one still at its start)
                let c = Camera { center: lonlat_to_merc(Coord::new(lon, lat)), res: res_of(z) };
                shell.map_view_mut().set_camera(c);
            }
        },
        State { shell: None },
    );

    // Start: the window's first frames, files opened and loaded, the setup steps.
    let t0 = frames.first().and_then(|f| f["t"].as_f64()).unwrap_or(0.0) - 2.0;
    // (well before the first frame, so notices about the opened files have gone by then)
    set_time(&mut h, t0 - 120.0);
    settle(&mut h, 20, max_steps, true);
    for (i, c) in setup.into_iter().enumerate() {
        pending.lock().unwrap().commands.push(c);
        set_time(&mut h, t0 - 90.0 + i as f64 * 3.0);
        settle(&mut h, 10, max_steps, true);
    }
    for dt in [30.0, 10.0, 0.0] {
        set_time(&mut h, t0 - dt);
        settle(&mut h, 5, max_steps, true);
    }
    // One frame drawn and thrown away at the first frame's camera: the very first render could
    // miss the panels (egui lays out areas on the frame after they first appear).
    pending.lock().unwrap().cam = frames.first().and_then(|f| cam3(&f["cam"]));
    for _ in 0..2 {
        settle(&mut h, 10, max_steps, true);
        let _ = h.render();
    }

    let mut next_event = 0;
    let started = std::time::Instant::now();
    for f in &frames {
        let t = f["t"].as_f64().unwrap_or(0.0);
        let name = f["out"].as_str().unwrap_or("frame.png").to_string();
        let target = out.join(&name);
        let cam = cam3(&f["cam"]);
        // Events up to this frame, in order.
        let mut typed = false;
        while next_event < events.len() && events[next_event].0 <= t + 1e-9 {
            let e = &events[next_event].1;
            if let Some(s) = e["step"].as_str()
                && let Some(c) = step_command(s)
            {
                pending.lock().unwrap().commands.push(c);
            }
            if let Some(fid) = e["focus"].as_str() {
                pending.lock().unwrap().focus = Some(fid.to_string());
                set_time(&mut h, t);
                h.step();
            }
            if let Some(text) = e["text"].as_str() {
                h.event(egui::Event::Text(text.to_string()));
                typed = true;
            }
            if let Some(at) = e["click_at"].as_array() {
                // A pointer click at a point of the window (points), then the pointer leaves, so
                // no cursor is drawn.
                let pos = egui::pos2(at[0].as_f64().unwrap_or(0.0) as f32, at[1].as_f64().unwrap_or(0.0) as f32);
                h.event(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    h.event(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                set_time(&mut h, t);
                h.step();
                h.remove_cursor();
                typed = true;
            }
            if let Some(k) = e["key"].as_str() {
                let key = egui::Key::from_name(k).expect("key name");
                h.key_press(key);
                typed = true;
            }
            next_event += 1;
        }
        pending.lock().unwrap().cam = cam;
        set_time(&mut h, t);
        if !force && target.exists() && !typed {
            // Keep the UI's state moving through time even for frames already rendered.
            h.step();
            continue;
        }
        let a = std::time::Instant::now();
        let n = settle(&mut h, 3, max_steps, false);
        let img = h.render().expect("render");
        save_rgb(&img, &target);
        println!("{name} steps={n} ms={:.0}", a.elapsed().as_secs_f64() * 1000.0);
    }
    println!("done: {} frames in {:.1} s", frames.len(), started.elapsed().as_secs_f64());
}

/// egui's clock for film time `t`: kept well above zero (the warm-up runs two minutes before
/// the first frame, and a clock at or below zero left the first frames without their panels).
fn set_time(h: &mut Harness<'_, State>, t: f64) {
    h.input_mut().time = Some(t + 1000.0);
}

/// Steps at a fixed clock until the map shows the finished render of the camera and nothing is
/// loading, after at least `min` steps. Returns the steps taken.
fn settle(h: &mut Harness<'_, State>, min: usize, max: usize, wait_layers: bool) -> usize {
    let mut n = 0;
    let mut ok_in_a_row = 0;
    loop {
        h.step();
        n += 1;
        let ok = h.state().shell.as_ref().is_some_and(|s| {
            s.map_view().renders > 0
                && s.map_view().settled()
                && s.map_view().tile_errors().is_empty()
                && !lm_mapview::archive::looking()
                && !s.is_loading()
                && !s.ask_pending()
                && !s.ws.jobs_running()
                && (!wait_layers || s.ws.imports.is_empty())
        });
        ok_in_a_row = if ok { ok_in_a_row + 1 } else { 0 };
        if n >= min && ok_in_a_row >= 2 {
            return n;
        }
        if n >= max {
            let why = h.state().shell.as_ref().map(|s| format!("{:?}", s.map_view().tile_errors())).unwrap_or_default();
            eprintln!("warning: not settled after {n} steps {why}");
            return n;
        }
        std::thread::sleep(std::time::Duration::from_millis(if ok { 0 } else { 3 }));
    }
}

fn save_rgb(img: &image::RgbaImage, path: &Path) {
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let file = std::io::BufWriter::new(std::fs::File::create(path).expect("create png"));
    let enc = image::codecs::png::PngEncoder::new_with_quality(
        file,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Sub,
    );
    rgb.write_with_encoder(enc).expect("write png");
}
