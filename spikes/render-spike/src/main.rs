//! WP0.3 rendering spike (throwaway): can RustGIS's renderer, inside egui, feel like a consumer
//! map with an online vector basemap plus fixture F1 (100,000 points, 10,000 polygons)?
//!
//! Four drawing strategies, switchable live (keys 1–4) and measured by `--bench`:
//! - **A** redraw everything every frame, on the UI thread;
//! - **B** RustGIS's own approach: keep the last frame as a texture, move and scale it during a
//!   gesture, redraw (on the UI thread) once the gesture settles;
//! - **B2** as B, but the CPU renderer runs on a worker thread and always renders the newest
//!   camera, so the UI thread never waits and the texture sharpens during long gestures;
//! - **C** B2 for the basemap only; the data layers are drawn on the GPU every frame (wgpu,
//!   through an egui paint callback), so data is always sharp and exactly placed.
//!
//! Usage:
//!   lm-render-spike                    interactive (trackpad, mouse, keys; P toggles the probes)
//!   lm-render-spike --bench [--runs 5] [--strategies A,B,B2,C] [--out DIR]
//! Options: --fixtures DIR (default fixtures/generated), --tiles URL|none, --vsync.

mod gpu;
mod worker;

use std::path::PathBuf;

use eframe::egui_wgpu;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, pos2, vec2};
use rustgis_geom::{Coord, lonlat_to_merc};
use rustgis_map::camera::Camera;
use rustgis_map::symbology::{Marker, Renderer, Symbol};
use rustgis_map::tiles::{TileKind, TileLayer};
use rustgis_map::{Basemap, Map};
use rustgis_render::{PlacedLabel, View};

const DEFAULT_TILES: &str = "https://build.protomaps.com/20261014.pmtiles";
const OKABE_BLUE: [u8; 3] = [0x00, 0x72, 0xB2];
const OKABE_ORANGE: [u8; 3] = [0xE6, 0x9F, 0x00];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Strategy {
    A,
    B,
    B2,
    C,
}

impl Strategy {
    fn code(self) -> &'static str {
        match self {
            Strategy::A => "A",
            Strategy::B => "B",
            Strategy::B2 => "B2",
            Strategy::C => "C",
        }
    }
    fn parse(s: &str) -> Option<Strategy> {
        match s.trim().to_ascii_uppercase().as_str() {
            "A" => Some(Strategy::A),
            "B" => Some(Strategy::B),
            "B2" => Some(Strategy::B2),
            "C" => Some(Strategy::C),
            _ => None,
        }
    }
}

/// The scripted camera paths of WP0.3.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Script {
    /// A 2,000 px pan at constant speed.
    Pan,
    /// Zoom in 6 levels about the centre.
    ZoomIn,
    /// Zoom back out 6 levels.
    ZoomOut,
    /// A continuous pinch: zoom oscillating ±1.5 levels while drifting.
    Pinch,
}

const SCRIPTS: [Script; 4] = [Script::Pan, Script::ZoomIn, Script::ZoomOut, Script::Pinch];

impl Script {
    fn code(self) -> &'static str {
        match self {
            Script::Pan => "pan2000",
            Script::ZoomIn => "zoom_in6",
            Script::ZoomOut => "zoom_out6",
            Script::Pinch => "pinch",
        }
    }
    fn duration(self) -> f64 {
        match self {
            Script::Pan => 2.0,
            Script::ZoomIn | Script::ZoomOut => 3.0,
            Script::Pinch => 4.0,
        }
    }
    fn start(self, home: Camera) -> Camera {
        match self {
            Script::ZoomOut => Camera { res: home.res / 64.0, ..home },
            _ => home,
        }
    }
    fn camera(self, home: Camera, t: f64) -> Camera {
        let s = (t / self.duration()).clamp(0.0, 1.0);
        match self {
            Script::Pan => Camera { center: Coord::new(home.center.x + 2000.0 * s * home.res, home.center.y), ..home },
            Script::ZoomIn => Camera { res: home.res / 2f64.powf(6.0 * s), ..home },
            Script::ZoomOut => Camera { res: home.res / 2f64.powf(6.0 * (1.0 - s)), ..home },
            Script::Pinch => {
                let z = 1.5 * (std::f64::consts::TAU * t / 2.0).sin();
                let a = std::f64::consts::PI * t;
                Camera {
                    center: Coord::new(home.center.x + 150.0 * home.res * a.cos(), home.center.y + 150.0 * home.res * a.sin()),
                    res: home.res / 2f64.powf(z),
                }
            }
        }
    }
}

struct FrameRec {
    strategy: Strategy,
    script: Script,
    run: usize,
    phase: &'static str,
    t_ms: f64,
    dt_ms: f64,
    ui_ms: f64,
    cpu_ms: f64,
    rendered: bool,
    render_ms: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Settle,
    Run,
    After,
}

struct Bench {
    plan: Vec<(Strategy, Script, usize)>,
    idx: usize,
    phase: Phase,
    phase_start: f64,
    stable_since: Option<f64>,
    last_gen: u64,
    records: Vec<FrameRec>,
    out: PathBuf,
}

struct Texture {
    handle: egui::TextureHandle,
    camera: Camera,
    labels: Vec<PlacedLabel>,
}

struct App {
    strategy: Strategy,
    /// The full map (basemap tiles + data) for the UI-thread strategies A and B.
    map: Map,
    worker_full: worker::Worker,
    worker_base: worker::Worker,
    gpu_origin: Option<Coord>,
    camera: Camera,
    home: Camera,
    tex: Option<Texture>,
    sync_key: Option<(Camera, [u32; 2], u64)>,
    sync_pending: bool,
    attribution: Vec<String>,
    last_input: f64,
    last_frame: Option<f64>,
    velocity: egui::Vec2,
    zoom_anim: Option<(f64, f64, f64, Pos2)>, // (from log2 res, to log2 res, start time, anchor)
    recent: Vec<f64>,
    bench: Option<Bench>,
    probes: bool,
    probe_text: String,
    probe_pick: usize,
    last_render_ms: f64,
    vsync: bool,
}

fn now(ctx: &egui::Context) -> f64 {
    ctx.input(|i| i.time)
}

fn style_layers(map: &mut Map) {
    for l in &mut map.layers {
        let g = l.dataset.geometry_type;
        l.renderer = Renderer::Single(match g {
            rustgis_data::GeometryType::Point => Symbol {
                fill: rustgis_map::color::Rgba::rgb(OKABE_BLUE[0], OKABE_BLUE[1], OKABE_BLUE[2]),
                stroke: rustgis_map::color::Rgba::new(0, 0, 0, 0),
                stroke_width: 0.0,
                size: 4.0,
                marker: Marker::Circle,
            },
            _ => {
                let c = rustgis_map::color::Rgba::rgb(OKABE_ORANGE[0], OKABE_ORANGE[1], OKABE_ORANGE[2]);
                Symbol { fill: c.with_alpha(89), stroke: c.shade(0.6), stroke_width: 0.75, size: 0.0, marker: Marker::Circle }
            }
        });
    }
}

fn load_fonts(ctx: &egui::Context) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut defs = egui::FontDefinitions::default();
    let mut add = |name: &str, file: &str, first: bool| {
        if let Ok(bytes) = std::fs::read(dir.join(file)) {
            defs.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                let list = defs.families.entry(fam).or_default();
                if first {
                    list.insert(0, name.into());
                } else {
                    list.push(name.into());
                }
            }
        }
    };
    add("Inter", "Inter-Regular.ttf", true);
    add("NotoSansSC", "NotoSansSC-Regular.otf", false);
    ctx.set_fonts(defs);
}

impl App {
    fn view(&self, size: egui::Vec2, ppp: f32, cam: Camera) -> View {
        View { width: f64::from(size.x), height: f64::from(size.y), pixels_per_point: f64::from(ppp), camera: cam, dark: false }
    }

    fn upload(&mut self, ctx: &egui::Context, size: [usize; 2], rgba: &[u8], camera: Camera, labels: Vec<PlacedLabel>) {
        let img = egui::ColorImage::from_rgba_premultiplied(size, rgba);
        match &mut self.tex {
            Some(t) => {
                t.handle.set(img, egui::TextureOptions::LINEAR);
                t.camera = camera;
                t.labels = labels;
            }
            None => self.tex = Some(Texture { handle: ctx.load_texture("map", img, egui::TextureOptions::LINEAR), camera, labels }),
        }
    }

    /// Brings the map texture up to date as the strategy allows. Returns whether a render
    /// finished this frame, and how long it took.
    fn refresh(&mut self, ctx: &egui::Context, size: egui::Vec2, ppp: f32, interacting: bool) -> (bool, f64) {
        let tile_gen = rustgis_tiles::cache::generation();
        let view = self.view(size, ppp, self.camera);
        let px = [view.pixel_size().0, view.pixel_size().1];
        match self.strategy {
            Strategy::A | Strategy::B => {
                let key = (self.camera, px, tile_gen);
                let stale = self.sync_key != Some(key);
                let may = self.strategy == Strategy::A || !interacting || self.tex.is_none();
                if stale && may {
                    let t = web_time::Instant::now();
                    if let Some(r) = rustgis_render::render(&mut self.map, &view) {
                        let ms = t.elapsed().as_secs_f64() * 1000.0;
                        let size = [r.pixmap.width() as usize, r.pixmap.height() as usize];
                        self.attribution = r.attribution.clone();
                        self.sync_pending = r.tiles_pending;
                        self.upload(ctx, size, r.pixmap.data(), self.camera, r.labels);
                        self.sync_key = Some(key);
                        self.last_render_ms = ms;
                        return (true, ms);
                    }
                }
                (false, 0.0)
            }
            Strategy::B2 | Strategy::C => {
                let w = if self.strategy == Strategy::B2 { &mut self.worker_full } else { &mut self.worker_base };
                w.request(view, tile_gen);
                if let Some(f) = w.poll() {
                    self.attribution = f.attribution.clone();
                    self.sync_pending = f.tiles_pending;
                    self.last_render_ms = f.render_ms;
                    let (size, camera, ms) = (f.size, f.camera, f.render_ms);
                    self.upload(ctx, size, &f.rgba, camera, f.labels);
                    return (true, ms);
                }
                (false, 0.0)
            }
        }
    }

    fn busy(&self) -> bool {
        match self.strategy {
            Strategy::A | Strategy::B => self.sync_pending,
            Strategy::B2 => self.worker_full.in_flight || self.sync_pending,
            Strategy::C => self.worker_base.in_flight || self.sync_pending,
        }
    }

    fn set_strategy(&mut self, s: Strategy) {
        if s != self.strategy {
            self.strategy = s;
            self.sync_key = None;
            self.worker_full.last_sent = None;
            self.worker_base.last_sent = None;
        }
    }

    /// `Camera::pan_px` moves the map's content by the given offset, so drags, trackpad scrolls
    /// (whose deltas are content motion on macOS) and inertia pass their deltas straight through.
    ///
    /// User input: drag to pan (with inertia), trackpad scroll to pan, wheel and pinch to zoom,
    /// double-click to zoom in, keys. Returns whether the camera is moving.
    fn input(&mut self, ui: &mut egui::Ui, resp: &egui::Response, rect: Rect, t: f64, dt: f64) -> bool {
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        let mut moving = false;
        if resp.dragged() {
            let d = resp.drag_delta();
            self.camera.pan_px(f64::from(d.x), f64::from(d.y));
            if dt > 0.0 {
                let v = d / dt as f32;
                self.velocity = self.velocity * 0.6 + v * 0.4;
            }
            self.zoom_anim = None;
            moving = true;
        } else if resp.drag_stopped() {
            // keep velocity for inertia
        } else if self.velocity.length() > 5.0 {
            let d = self.velocity * dt as f32;
            self.camera.pan_px(f64::from(d.x), f64::from(d.y));
            // Exponential decay: from a fast flick to rest in roughly 0.4 s.
            self.velocity *= (-dt / 0.11).exp() as f32;
            moving = true;
        } else {
            self.velocity = egui::Vec2::ZERO;
        }
        let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
        let events = ui.input(|i| i.events.clone());
        for e in &events {
            match e {
                egui::Event::MouseWheel { unit, delta, .. } if pointer.is_some() => {
                    let p = pointer.unwrap_or(rect.center());
                    if *unit == egui::MouseWheelUnit::Line {
                        // A mouse wheel: 0.5 zoom levels per notch, animated over 250 ms.
                        let cur = self.zoom_anim.map_or(self.camera.res.log2(), |(_, to, _, _)| to);
                        self.zoom_anim = Some((self.camera.res.log2(), cur - 0.5 * f64::from(delta.y.signum()), t, p));
                    } else {
                        // A trackpad two-finger scroll pans, as in Apple Maps.
                        self.camera.pan_px(f64::from(delta.x), f64::from(delta.y));
                        self.velocity = egui::Vec2::ZERO;
                    }
                    moving = true;
                }
                egui::Event::Zoom(f) if pointer.is_some() => {
                    let p = pointer.unwrap_or(rect.center()) - rect.min;
                    self.camera.zoom_at(f64::from(*f), f64::from(p.x), f64::from(p.y), w, h);
                    self.zoom_anim = None;
                    moving = true;
                }
                _ => {}
            }
        }
        if resp.double_clicked()
            && let Some(p) = pointer
        {
            let alt = ui.input(|i| i.modifiers.alt);
            let cur = self.camera.res.log2();
            self.zoom_anim = Some((cur, if alt { cur + 1.0 } else { cur - 1.0 }, t, p));
        }
        let keys = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                i.key_pressed(egui::Key::Minus),
            )
        });
        if !self.probes_focused(ui) {
            let (l, r, u, d, zin, zout) = keys;
            let (qx, qy) = (w / 4.0, h / 4.0);
            if l {
                self.camera.pan_px(qx, 0.0);
            }
            if r {
                self.camera.pan_px(-qx, 0.0);
            }
            if u {
                self.camera.pan_px(0.0, qy);
            }
            if d {
                self.camera.pan_px(0.0, -qy);
            }
            if zin || zout {
                let cur = self.camera.res.log2();
                self.zoom_anim = Some((cur, if zin { cur - 1.0 } else { cur + 1.0 }, t, rect.center()));
            }
            moving |= l || r || u || d;
        }
        if let Some((from, to, t0, anchor)) = self.zoom_anim {
            let s = ((t - t0) / 0.25).clamp(0.0, 1.0);
            let e = 1.0 - (1.0 - s).powi(3); // ease-out cubic
            let target = 2f64.powf(from + (to - from) * e);
            let a = anchor - rect.min;
            self.camera.zoom_at(self.camera.res / target, f64::from(a.x), f64::from(a.y), w, h);
            if s >= 1.0 {
                self.zoom_anim = None;
            }
            moving = true;
        }
        moving
    }

    fn probes_focused(&self, ui: &egui::Ui) -> bool {
        ui.ctx().memory(|m| m.focused().is_some())
    }

    fn paint_map(&self, ui: &egui::Ui, rect: Rect, ppp: f32) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0xE8, 0xEC, 0xEF));
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        let to_screen = |x: f64, y: f64| {
            let (sx, sy) = self.camera.to_screen(Coord::new(x, y), w, h);
            pos2(rect.min.x + sx as f32, rect.min.y + sy as f32)
        };
        let mut zooming = false;
        if let Some(tex) = &self.tex {
            let e0 = tex.camera.extent(w, h);
            let r = Rect::from_min_max(to_screen(e0.min_x, e0.max_y), to_screen(e0.max_x, e0.min_y));
            painter.image(tex.handle.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            zooming = (tex.camera.res / self.camera.res - 1.0).abs() > 0.002;
            if !zooming {
                let (dx, dy) = (r.min.x - rect.min.x, r.min.y - rect.min.y);
                for l in &tex.labels {
                    draw_label(&painter, l, pos2(rect.min.x + l.x + dx, rect.min.y + l.y + dy));
                }
            }
        }
        if self.strategy == Strategy::C
            && let Some(origin) = self.gpu_origin
        {
            let cb = gpu::DrawData {
                center_rel: [(self.camera.center.x - origin.x) as f32, (self.camera.center.y - origin.y) as f32],
                viewport_px: [rect.width() * ppp, rect.height() * ppp],
                ppp,
                points_per_m: (1.0 / self.camera.res) as f32,
                point_pt: 4.0,
                point_color: gpu::premul(OKABE_BLUE, 1.0),
                fill_color: gpu::premul(OKABE_ORANGE, 0.35),
                line_color: gpu::premul([0x8A, 0x5F, 0x00], 0.9),
            };
            painter.add(egui_wgpu::Callback::new_paint_callback(rect, cb));
        }
        let _ = zooming;
        // Attribution, always visible (bottom right, 12 px).
        let credit = if self.attribution.is_empty() {
            "© OpenStreetMap contributors · Protomaps".to_string()
        } else {
            self.attribution.join(" · ")
        };
        let at = rect.right_bottom() - vec2(6.0, 4.0);
        let galley = painter.layout_no_wrap(credit, FontId::proportional(12.0), Color32::from_rgb(0x5F, 0x66, 0x70));
        let bg = Rect::from_min_size(at - galley.size(), galley.size()).expand(3.0);
        painter.rect_filled(bg, 3.0, Color32::from_white_alpha(200));
        painter.galley(bg.min + vec2(3.0, 3.0), galley, Color32::BLACK);
    }

    fn overlay(&self, ui: &egui::Ui, rect: Rect) {
        let mut sorted = self.recent.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95 = sorted.get(((sorted.len() as f64) * 0.95) as usize).copied().unwrap_or(0.0);
        let fps = if sorted.is_empty() { 0.0 } else { 1000.0 * sorted.len() as f64 / sorted.iter().sum::<f64>() };
        let text = format!(
            "Strategy {}  (1 A · 2 B · 3 B2 · 4 C)\n{fps:.0} fps · p95 {p95:.1} ms · last render {:.0} ms\nzoom {:.1} · {}  · P probes · H home",
            self.strategy.code(),
            self.last_render_ms,
            (156_543.034 / self.camera.res).log2(),
            if self.vsync { "vsync" } else { "no vsync" },
        );
        let painter = ui.painter_at(rect);
        let galley = painter.layout_no_wrap(text, FontId::monospace(12.0), Color32::WHITE);
        let r = Rect::from_min_size(rect.min + vec2(12.0, 12.0), galley.size()).expand(6.0);
        painter.rect_filled(r, 6.0, Color32::from_black_alpha(170));
        painter.galley(r.min + vec2(6.0, 6.0), galley, Color32::WHITE);
    }

    /// WP0.4/WP0.5 probes: Pinyin input, VoiceOver reachability, fonts and tabular figures.
    fn probe_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Probes").default_pos(pos2(900.0, 80.0)).default_width(340.0).show(ctx, |ui| {
            ui.label("Pinyin input: type 上海市徐汇区, then Xuhui");
            ui.add(egui::TextEdit::singleline(&mut self.probe_text).hint_text("Search places or your data").desired_width(300.0));
            ui.label(format!("Committed: {}", self.probe_text));
            ui.separator();
            ui.label("VoiceOver (⌘F5): five buttons, a text field above, a list below");
            ui.horizontal_wrapped(|ui| {
                for b in ["Open file", "Layers", "Undo", "Share", "Zoom to all data"] {
                    let _ = ui.button(b);
                }
            });
            for (i, d) in ["黄浦区 Huangpu", "徐汇区 Xuhui", "长宁区 Changning", "静安区 Jing'an", "浦东新区 Pudong"].iter().enumerate()
            {
                if ui.selectable_label(self.probe_pick == i, *d).clicked() {
                    self.probe_pick = i;
                }
            }
            ui.separator();
            ui.label("Fonts (Inter with tabular figures, Noto Sans SC):");
            for size in [12.0, 14.0, 20.0, 28.0] {
                ui.label(egui::RichText::new("徐汇区 Xuhui 31.2 °C").size(size));
            }
            for n in ["1,248 schools", "  111.1 km²", "  888.8 km²", "−2.1 °C"] {
                ui.label(egui::RichText::new(n).size(14.0));
            }
        });
    }
}

fn draw_label(p: &egui::Painter, l: &PlacedLabel, at: Pos2) {
    let font = FontId::proportional(l.size * 1.33);
    let align = if l.align < -0.25 {
        Align2::LEFT_CENTER
    } else if l.align > 0.25 {
        Align2::RIGHT_CENTER
    } else {
        Align2::CENTER_CENTER
    };
    let c = |c: rustgis_map::color::Rgba| Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3]);
    if let Some(h) = l.halo {
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            p.text(at + vec2(dx, dy), align, &l.text, font.clone(), c(h));
        }
    }
    p.text(at, align, &l.text, font, c(l.color));
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

impl Bench {
    fn write(&self) -> std::io::Result<()> {
        use std::io::Write;
        std::fs::create_dir_all(&self.out)?;
        let mut f = std::io::BufWriter::new(std::fs::File::create(self.out.join("frames.csv"))?);
        writeln!(f, "strategy,script,run,phase,t_ms,dt_ms,ui_ms,cpu_ms,rendered,render_ms")?;
        for r in &self.records {
            writeln!(
                f,
                "{},{},{},{},{:.2},{:.3},{:.3},{:.3},{},{:.2}",
                r.strategy.code(),
                r.script.code(),
                r.run,
                r.phase,
                r.t_ms,
                r.dt_ms,
                r.ui_ms,
                r.cpu_ms,
                u8::from(r.rendered),
                r.render_ms
            )?;
        }
        let mut s = std::io::BufWriter::new(std::fs::File::create(self.out.join("summary.csv"))?);
        writeln!(s, "strategy,script,run,frames,p50_ms,p95_ms,p99_ms,max_ms,over_16_7,over_33,max_ui_ms,ui_stalls_over_50")?;
        let mut keys: Vec<(Strategy, Script, usize)> = Vec::new();
        for r in &self.records {
            if !keys.contains(&(r.strategy, r.script, r.run)) {
                keys.push((r.strategy, r.script, r.run));
            }
        }
        for (st, sc, run) in keys {
            let rows: Vec<&FrameRec> =
                self.records.iter().filter(|r| r.strategy == st && r.script == sc && r.run == run && r.phase != "first").collect();
            let mut dts: Vec<f64> = rows.iter().map(|r| r.dt_ms).collect();
            dts.sort_by(|a, b| a.total_cmp(b));
            let max_ui = rows.iter().map(|r| r.ui_ms).fold(0.0, f64::max);
            writeln!(
                s,
                "{},{},{},{},{:.2},{:.2},{:.2},{:.2},{},{},{:.1},{}",
                st.code(),
                sc.code(),
                run,
                dts.len(),
                percentile(&dts, 0.5),
                percentile(&dts, 0.95),
                percentile(&dts, 0.99),
                dts.last().copied().unwrap_or(0.0),
                dts.iter().filter(|d| **d > 16.7).count(),
                dts.iter().filter(|d| **d > 33.0).count(),
                max_ui,
                rows.iter().filter(|r| r.ui_ms > 50.0).count()
            )?;
        }
        Ok(())
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let t_ui = web_time::Instant::now();
        let ctx = ui.ctx().clone();
        let t = now(&ctx);
        let dt = self.last_frame.map_or(0.0, |l| t - l);
        self.last_frame = Some(t);
        let ppp = ctx.pixels_per_point();

        // Keys that aren't map navigation.
        if self.bench.is_none() && !self.probes_focused(ui) {
            let (pick, probes, home) = ui.input(|i| {
                let pick = [
                    (egui::Key::Num1, Strategy::A),
                    (egui::Key::Num2, Strategy::B),
                    (egui::Key::Num3, Strategy::B2),
                    (egui::Key::Num4, Strategy::C),
                ]
                .into_iter()
                .find(|(k, _)| i.key_pressed(*k))
                .map(|(_, s)| s);
                (pick, i.key_pressed(egui::Key::P), i.key_pressed(egui::Key::H))
            });
            if let Some(s) = pick {
                self.set_strategy(s);
            }
            if probes {
                self.probes = !self.probes;
            }
            if home {
                self.camera = self.home;
            }
        }

        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        if w < 4.0 || h < 4.0 {
            return;
        }

        // Camera: the bench script, or the user.
        let mut interacting;
        let mut record_phase: Option<&'static str> = None;
        // The plan entry this frame belongs to, taken before the state machine can advance it.
        let entry = self.bench.as_ref().map(|b| (b.plan[b.idx], b.phase_start));
        let mut bench_done = false;
        if let Some(st) = self.bench.as_ref().map(|b| b.plan[b.idx].0) {
            self.set_strategy(st);
        }
        let busy = self.busy();
        let tex_camera = self.tex.as_ref().map(|x| x.camera);
        if let Some(b) = &mut self.bench {
            let (_, sc, run) = b.plan[b.idx];
            let elapsed = t - b.phase_start;
            match b.phase {
                Phase::Settle => {
                    self.camera = sc.start(self.home);
                    interacting = false;
                    let tile_gen = rustgis_tiles::cache::generation();
                    let quiet = !busy && tile_gen == b.last_gen && tex_camera == Some(sc.start(self.home));
                    b.last_gen = tile_gen;
                    if quiet {
                        let since = *b.stable_since.get_or_insert(t);
                        if t - since > 0.6 || elapsed > 20.0 {
                            b.phase = Phase::Run;
                            b.phase_start = t;
                            b.stable_since = None;
                        }
                    } else {
                        b.stable_since = None;
                        if elapsed > 20.0 {
                            b.phase = Phase::Run;
                            b.phase_start = t;
                        }
                    }
                }
                Phase::Run => {
                    self.camera = sc.camera(self.home, elapsed);
                    interacting = true;
                    if run > 0 {
                        // The first frame's interval spans the last settle frame: kept in
                        // frames.csv, left out of the statistics.
                        record_phase = Some(
                            if b.records
                                .last()
                                .is_some_and(|r| r.phase != "after" && r.run == run && r.script == sc && r.strategy == self.strategy)
                            {
                                "run"
                            } else {
                                "first"
                            },
                        );
                    }
                    if elapsed >= sc.duration() {
                        b.phase = Phase::After;
                        b.phase_start = t;
                    }
                }
                Phase::After => {
                    interacting = false;
                    if run > 0 {
                        record_phase = Some("after");
                    }
                    if elapsed >= 0.6 {
                        b.idx += 1;
                        b.phase = Phase::Settle;
                        b.phase_start = t;
                        if b.idx >= b.plan.len() {
                            bench_done = true;
                        }
                    }
                }
            }
        } else {
            let moving = self.input(ui, &resp, rect, t, dt);
            if moving {
                self.last_input = t;
            }
            interacting = moving || t - self.last_input < 0.18;
        }
        self.camera.clamp();
        if self.bench.is_none() && resp.dragged() {
            interacting = true;
        }

        let (rendered, render_ms) = self.refresh(&ctx, rect.size(), ppp, interacting);
        self.paint_map(ui, rect, ppp);
        self.overlay(ui, rect);
        if self.probes {
            self.probe_window(&ctx);
        }

        if dt > 0.0 {
            self.recent.push(dt * 1000.0);
            if self.recent.len() > 120 {
                self.recent.remove(0);
            }
        }
        let ui_ms = t_ui.elapsed().as_secs_f64() * 1000.0;
        if let (Some(phase), Some(b), Some(((st, sc, run), phase_start))) = (record_phase, &mut self.bench, entry) {
            b.records.push(FrameRec {
                strategy: st,
                script: sc,
                run,
                phase,
                t_ms: (t - phase_start) * 1000.0,
                dt_ms: dt * 1000.0,
                ui_ms,
                cpu_ms: f64::from(frame.info().cpu_usage.unwrap_or(0.0)) * 1000.0,
                rendered,
                render_ms,
            });
        }
        if ui_ms > 50.0 && self.bench.is_none() {
            log::warn!("UI thread stall: {ui_ms:.0} ms");
        }
        if bench_done {
            if let Some(b) = &self.bench {
                match b.write() {
                    Ok(()) => println!("bench: wrote {}", b.out.display()),
                    Err(e) => eprintln!("bench: {e}"),
                }
            }
            self.bench = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.bench.is_some() || interacting || self.busy() {
            ctx.request_repaint();
        }
    }
}

struct Args {
    bench: bool,
    runs: usize,
    strategies: Vec<Strategy>,
    out: PathBuf,
    fixtures: PathBuf,
    tiles: Option<String>,
    vsync: bool,
    size: (f32, f32),
    /// `f2`: the 1,000,000 points of F2 instead of F1's 100,000.
    points: String,
    /// A raster to add under the data (stress: R4, R5), timed.
    raster: Option<PathBuf>,
}

fn args() -> Result<Args, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut a = Args {
        bench: false,
        runs: 5,
        strategies: vec![Strategy::A, Strategy::B, Strategy::B2, Strategy::C],
        out: root.join("docs/lm/spike"),
        fixtures: root.join("fixtures/generated"),
        tiles: Some(DEFAULT_TILES.into()),
        vsync: false,
        size: (1280.0, 800.0),
        points: "f1".into(),
        raster: None,
    };
    let mut it = std::env::args().skip(1);
    let mut interactive_vsync = true;
    while let Some(x) = it.next() {
        match x.as_str() {
            "--bench" => a.bench = true,
            "--runs" => a.runs = it.next().and_then(|v| v.parse().ok()).ok_or("--runs N")?,
            "--strategies" => {
                a.strategies = it.next().ok_or("--strategies A,B,B2,C")?.split(',').filter_map(Strategy::parse).collect();
            }
            "--out" => a.out = PathBuf::from(it.next().ok_or("--out DIR")?),
            "--fixtures" => a.fixtures = PathBuf::from(it.next().ok_or("--fixtures DIR")?),
            "--tiles" => {
                let v = it.next().ok_or("--tiles URL|none")?;
                a.tiles = (v != "none").then_some(v);
            }
            "--points" => a.points = it.next().ok_or("--points f1|f2")?.to_ascii_lowercase(),
            "--raster" => a.raster = Some(PathBuf::from(it.next().ok_or("--raster PATH")?)),
            "--vsync" => interactive_vsync = true,
            "--no-vsync" => interactive_vsync = false,
            "--size" => {
                let v = it.next().ok_or("--size WxH")?;
                let (x, y) = v.split_once('x').ok_or("--size WxH")?;
                a.size = (x.parse().map_err(|_| "--size")?, y.parse().map_err(|_| "--size")?);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    // Measurements run without vsync so frame time is the real cost, not the display's cadence.
    a.vsync = if a.bench { false } else { interactive_vsync };
    Ok(a)
}

fn main() -> Result<(), String> {
    let a = args()?;
    let t = std::time::Instant::now();
    let read = |name: &str| -> Result<rustgis_data::Dataset, String> {
        let p = a.fixtures.join("F1").join(name);
        rustgis_data::io::read_path(&p)
            .map(|(ds, _)| ds)
            .map_err(|e| format!("{}: {e} (run `cargo xtask fixtures --only F1`)", p.display()))
    };
    let points = if a.points == "f2" {
        let p = a.fixtures.join("F2/f2_points.csv");
        rustgis_data::io::read_path(&p).map(|(ds, _)| ds).map_err(|e| format!("{}: {e}", p.display()))?
    } else {
        read("f1_points.geojson")?
    };
    let polygons = read("f1_polygons.geojson")?;
    println!("F1 loaded: {} points, {} polygons in {:.1} s", points.features.len(), polygons.features.len(), t.elapsed().as_secs_f64());

    let mut base = Map::new("spike");
    base.basemap = Basemap::Paper;
    base.graticule = false;
    if let Some(url) = &a.tiles {
        let mut tl = TileLayer::new("Protomaps", TileKind::PmTiles, url.clone());
        tl.attribution = "© OpenStreetMap contributors · Protomaps".into();
        base.tiles.push(tl);
    }
    let mut full = base.clone();
    full.add_dataset(polygons.clone());
    full.add_dataset(points.clone());
    // Index 0 draws on top: points above polygons.
    full.layers.sort_by_key(|l| u8::from(l.dataset.geometry_type != rustgis_data::GeometryType::Point));
    style_layers(&mut full);

    if let Some(path) = &a.raster {
        let t = std::time::Instant::now();
        let r = rustgis_data::raster::read_path(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let read_s = t.elapsed().as_secs_f64();
        let layer = rustgis_map::RasterLayer::from_path(r, path.display().to_string());
        base.add_raster("raster", layer.clone());
        full.add_raster("raster", layer);
        // Rasters draw under the vector data (index 0 is on top).
        full.layers.sort_by_key(|l| u8::from(l.raster.is_some()));
        let mut probe = base.clone();
        probe.tiles.clear();
        let view = View {
            width: 1280.0,
            height: 800.0,
            pixels_per_point: 2.0,
            camera: Camera { center: lonlat_to_merc(Coord::new(121.47, 31.23)), res: 76.437 },
            dark: false,
        };
        let t2 = std::time::Instant::now();
        let ok = rustgis_render::render(&mut probe, &view).is_some();
        println!(
            "raster {}: read {:.2} s; first render (2560×1600, no tiles) {:.0} ms{}",
            path.display(),
            read_s,
            t2.elapsed().as_secs_f64() * 1000.0,
            if ok { "" } else { " (failed)" }
        );
    }
    let home_ll = Coord::new(121.47, 31.23);
    let home = Camera { center: lonlat_to_merc(home_ll), res: 76.437 }; // about zoom 11
    let origin = home.center;
    let mesh = gpu::Mesh::build(origin, &[&points, &polygons]);
    println!(
        "GPU mesh: {} points, {} fill triangles, {} outline segments",
        mesh.points.len(),
        mesh.fill_indices.len() / 3,
        mesh.lines.len() / 2
    );

    let mut plan = Vec::new();
    if a.bench {
        // Warm-up pass (run 0, not recorded): fetches the tiles every script passes through.
        for sc in SCRIPTS {
            plan.push((Strategy::B2, sc, 0));
        }
        for run in 1..=a.runs {
            for &st in &a.strategies {
                for sc in SCRIPTS {
                    plan.push((st, sc, run));
                }
            }
        }
    }

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("LayeredMap — WP0.3 rendering spike").with_inner_size([a.size.0, a.size.1]),
        ..Default::default()
    };
    options.wgpu_options.surface.present_mode =
        if a.vsync { eframe::egui_wgpu::wgpu::PresentMode::AutoVsync } else { eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync };
    let bench_out = a.out.clone();
    let vsync = a.vsync;
    eframe::run_native(
        "lm-render-spike",
        options,
        Box::new(move |cc| {
            load_fonts(&cc.egui_ctx);
            let gpu_origin = cc.wgpu_render_state.as_ref().map(|rs| {
                let data = gpu::GpuData::new(rs, &mesh);
                rs.renderer.write().callback_resources.insert(data);
                origin
            });
            let ctx = cc.egui_ctx.clone();
            let waker = ctx.clone();
            rustgis_tiles::cache::set_waker(move || waker.request_repaint());
            Ok(Box::new(App {
                strategy: if a.bench { Strategy::B2 } else { Strategy::C },
                worker_full: worker::Worker::spawn(full.clone(), ctx.clone()),
                worker_base: worker::Worker::spawn(base.clone(), ctx),
                map: full,
                gpu_origin,
                camera: home,
                home,
                tex: None,
                sync_key: None,
                sync_pending: false,
                attribution: Vec::new(),
                last_input: -10.0,
                last_frame: None,
                velocity: egui::Vec2::ZERO,
                zoom_anim: None,
                recent: Vec::new(),
                bench: a.bench.then(|| Bench {
                    plan,
                    idx: 0,
                    phase: Phase::Settle,
                    phase_start: 0.0,
                    stable_since: None,
                    last_gen: 0,
                    records: Vec::new(),
                    out: bench_out,
                }),
                probes: false,
                probe_text: String::new(),
                probe_pick: 0,
                last_render_ms: 0.0,
                vsync,
            }))
        }),
    )
    .map_err(|e| e.to_string())
}
