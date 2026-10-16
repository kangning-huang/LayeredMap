//! The map widget: camera, gestures, inertia, tile placeholders and fades (later: hit testing,
//! selection highlight). The only crate below `lm-ui` that may depend on egui, and the only
//! direct line from the interface to RustGIS (its drawing path).
//!
//! Drawing follows ADR-001 (strategy B2): RustGIS's CPU renderer runs on a worker thread and
//! always renders the newest camera; meanwhile the last frame is moved and scaled so every
//! gesture answers within one frame. A new frame crossfades in over 150 ms; labels fade one by one
//! (only those that appear or disappear, see `labels`).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod camera;
mod labels;
pub mod script;
pub mod stats;
mod worker;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Ui, pos2, vec2};
use rustgis_geom::extent_to_merc;
pub use rustgis_geom::{Coord, Extent, lonlat_to_merc};
/// The map model, for hosts that configure basemaps and layers.
pub use rustgis_map as map;
pub use rustgis_map::Map;
use rustgis_map::camera::Camera;
use rustgis_render::{PlacedLabel, View};

use camera::{Animation, INERTIA_STOP, INERTIA_TAU_S};
pub use camera::{MAX_ZOOM, res_of, zoom_of};
pub use stats::FrameStats;

/// How long a new frame (and its labels) takes to fade in.
pub const FADE_S: f64 = 0.15;
/// With Reduce Motion, flights jump; the new frame still fades, briefly.
pub const REDUCED_FADE_S: f64 = 0.1;

/// Registers a font the renderer may fall back on for label text its interface font can't draw
/// (LayeredMap's bundled Noto Sans SC, so Chinese labels aren't dropped). Returns false for an
/// unreadable font.
pub fn add_label_font(bytes: Vec<u8>) -> bool {
    rustgis_render::fonts::add_fallback(bytes, 0)
}

/// What the host tells the map each frame.
#[derive(Clone, Copy, Debug)]
pub struct ViewOptions {
    /// Painted under the map before the first frame arrives, and around a frame that is being
    /// moved: the basemap's own ground colour, so the map is never blank.
    pub backdrop: Color32,
    /// The system's Reduce Motion setting.
    pub reduce_motion: bool,
    /// Keyboard navigation applies (the map, not a text field, has the keyboard).
    pub keyboard: bool,
}

/// What happened in the map this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct MapOutput {
    /// The camera moved (gesture, inertia or animation).
    pub moving: bool,
    /// A click (not a drag) on the map, in longitude/latitude.
    pub clicked: Option<Coord>,
}

struct Shown {
    tex: egui::TextureHandle,
    camera: Camera,
    size: (f64, f64),
    /// When it started fading in (seconds).
    since: f64,
}

/// The map widget.
pub struct MapView {
    map: Map,
    worker: worker::Worker,
    camera: Camera,
    current: Option<Shown>,
    previous: Option<Shown>,
    labels: labels::Labels,
    /// A spare texture handle to upload into, so frames don't allocate textures.
    spare: Option<egui::TextureHandle>,
    anim: Option<Animation>,
    velocity: egui::Vec2,
    last_t: Option<f64>,
    stats: FrameStats,
    attribution: Vec<String>,
    tile_errors: Vec<String>,
    tiles_pending: bool,
    last_render_ms: f64,
    fade_s: f64,
    /// Renders finished, for the performance harness's counters.
    pub renders: u64,
    /// Times the render thread was restarted.
    restarts: u32,
}

impl MapView {
    /// A map view of `map`, rendering on its own thread.
    pub fn new(map: Map, ctx: &egui::Context) -> MapView {
        let camera = map.camera;
        let waker = ctx.clone();
        rustgis_tiles::cache::set_waker(move || waker.request_repaint());
        let source_waker = ctx.clone();
        rustgis_map::source::set_waker(move || source_waker.request_repaint());
        MapView {
            worker: worker::Worker::spawn(map.clone(), ctx.clone()),
            map,
            camera,
            current: None,
            previous: None,
            labels: labels::Labels::default(),
            spare: None,
            anim: None,
            velocity: egui::Vec2::ZERO,
            last_t: None,
            stats: FrameStats::default(),
            attribution: Vec::new(),
            tile_errors: Vec::new(),
            tiles_pending: false,
            last_render_ms: 0.0,
            fade_s: FADE_S,
            renders: 0,
            restarts: 0,
        }
    }

    /// The map being shown.
    pub fn map(&self) -> &Map {
        &self.map
    }

    /// Changes the map (basemap, tile layers, data layers); the next frame shows the change. The
    /// same edit is applied to the render thread's copy, so the map isn't copied whole.
    pub fn edit_map(&mut self, f: impl Fn(&mut Map) + Send + 'static) {
        f(&mut self.map);
        self.worker.edit(Box::new(f));
    }

    pub fn camera(&self) -> Camera {
        self.camera
    }

    pub fn set_camera(&mut self, c: Camera) {
        self.camera = c;
        self.anim = None;
    }

    /// The fractional web-map zoom level.
    pub fn zoom(&self) -> f64 {
        zoom_of(self.camera.res)
    }

    pub fn center_lonlat(&self) -> Coord {
        camera::center_lonlat(&self.camera)
    }

    /// Zooms by `levels` about the view's centre, animated (zoom buttons, + and −).
    pub fn zoom_by(&mut self, levels: f64, t: f64, size: egui::Vec2) {
        let from = self.zoom();
        let base = match self.anim {
            Some(Animation::Zoom { to, .. }) => to,
            _ => from,
        };
        self.velocity = egui::Vec2::ZERO;
        let to = camera::clamp_zoom(base + levels, f64::from(size.y));
        self.anim = Some(Animation::Zoom { from, to, start: t, anchor: (f64::from(size.x) / 2.0, f64::from(size.y) / 2.0) });
    }

    /// Flies to a place (search results): 600–900 ms, or a jump with Reduce Motion.
    pub fn fly_to(&mut self, lonlat: Coord, zoom: f64, t: f64, size: egui::Vec2, reduce_motion: bool) {
        self.velocity = egui::Vec2::ZERO;
        if reduce_motion {
            self.camera = Camera { center: lonlat_to_merc(lonlat), res: res_of(zoom) };
            self.anim = None;
            self.fade_s = REDUCED_FADE_S;
        } else {
            self.anim = Some(camera::fly(self.camera, lonlat, zoom, t, f64::from(size.x)));
        }
    }

    /// Fits a longitude/latitude extent (⌘0, zoom to layer), with a margin.
    pub fn zoom_to_extent(&mut self, e: &Extent, size: egui::Vec2) {
        let m = extent_to_merc(e);
        let (w, h) = (f64::from(size.x).max(1.0), f64::from(size.y).max(1.0));
        let res = (m.width() / (w * 0.85)).max(m.height() / (h * 0.85));
        self.velocity = egui::Vec2::ZERO;
        self.anim = None;
        self.camera = Camera { center: m.center(), res: if res.is_finite() && res > 0.0 { res } else { res_of(2.0) } };
    }

    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    /// The tile layers' credits, from the last frame.
    pub fn attribution(&self) -> &[String] {
        &self.attribution
    }

    /// Why tiles couldn't be drawn in the last frame (empty when all is well).
    pub fn tile_errors(&self) -> &[String] {
        &self.tile_errors
    }

    pub fn tiles_pending(&self) -> bool {
        self.tiles_pending || self.worker.in_flight
    }

    pub fn last_render_ms(&self) -> f64 {
        self.last_render_ms
    }

    /// Shows the map in `rect`: handles input, renders off-thread, paints.
    pub fn show(&mut self, ui: &mut Ui, rect: Rect, opts: ViewOptions) -> MapOutput {
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let t = ui.input(|i| i.time);
        let dt = self.last_t.map_or(0.0, |l| (t - l).clamp(0.0, 0.1));
        self.last_t = Some(t);
        self.stats.frame(t);
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        let mut out = MapOutput::default();
        if w < 4.0 || h < 4.0 {
            return out;
        }
        out.moving = self.input(ui, &resp, rect, t, dt, opts);
        if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let m = self.camera.to_map(f64::from(p.x - rect.min.x), f64::from(p.y - rect.min.y), w, h);
            out.clicked = self.map.to_lonlat(m);
        }
        camera::clamp(&mut self.camera, w, h);

        // Ask the worker for the newest view; take what it finished.
        let ppp = ui.ctx().pixels_per_point();
        let view = View { width: w, height: h, pixels_per_point: f64::from(ppp), camera: self.camera, dark: false };
        if self.worker.dead && self.restarts < 3 {
            // The render thread stopped (a panic in the renderer): start a fresh one on the map.
            log::error!("the map render thread stopped; restarting it");
            self.restarts += 1;
            self.worker = worker::Worker::spawn(self.map.clone(), ui.ctx().clone());
        }
        self.worker.request(view, rustgis_tiles::cache::generation());
        // While a crossfade runs, a newer frame waits for it to finish (at most 150 ms) unless the
        // camera is moving: swapping mid-fade would drop the half-faded frame and pop.
        let fading = self.current.as_ref().is_some_and(|c| t - c.since < self.fade_s);
        if (!fading || out.moving)
            && let Some(f) = self.worker.poll()
        {
            self.take(ui.ctx(), f, t);
        }
        let labels_fading = self.labels.step(dt, self.fade_s);

        self.paint(ui, rect, t, opts);
        let fading = self.current.as_ref().is_some_and(|c| t - c.since < self.fade_s);
        if out.moving || fading || labels_fading || self.anim.is_some() || self.velocity.length() > 0.0 || self.worker.in_flight {
            ui.ctx().request_repaint();
        }
        if !fading {
            self.previous = None;
            self.fade_s = FADE_S;
        }
        out
    }

    fn take(&mut self, ctx: &egui::Context, f: worker::Frame, t: f64) {
        self.renders += 1;
        self.attribution = f.attribution;
        self.tile_errors = f.tile_errors;
        self.tiles_pending = f.tiles_pending;
        self.last_render_ms = f.render_ms;
        let img = egui::ColorImage::from_rgba_premultiplied(f.pixel_size, &f.rgba);
        let tex = match self.previous.take().map(|p| p.tex).or_else(|| self.spare.take()) {
            Some(mut h) => {
                h.set(img, egui::TextureOptions::LINEAR);
                h
            }
            None => ctx.load_texture("lm-map", img, egui::TextureOptions::LINEAR),
        };
        self.labels.update(f.labels, &f.camera, f.size, self.current.is_none());
        let shown = Shown { tex, camera: f.camera, size: f.size, since: t };
        // The frame on screen fades out under the new one; the one before it is recycled.
        if let Some(old) = self.current.replace(shown)
            && let Some(prev) = self.previous.replace(old)
        {
            self.spare = Some(prev.tex);
        }
    }

    /// Where a frame drawn for `cam` at `size` sits on screen now.
    fn placed(&self, rect: Rect, cam: &Camera, size: (f64, f64)) -> Rect {
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        let e = cam.extent(size.0, size.1);
        let to = |x: f64, y: f64| {
            let (sx, sy) = self.camera.to_screen(Coord::new(x, y), w, h);
            pos2(rect.min.x + sx as f32, rect.min.y + sy as f32)
        };
        Rect::from_min_max(to(e.min_x, e.max_y), to(e.max_x, e.min_y))
    }

    fn paint(&self, ui: &Ui, rect: Rect, t: f64, opts: ViewOptions) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, opts.backdrop);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let a = self.current.as_ref().map_or(1.0, |c| ((t - c.since) / self.fade_s).clamp(0.0, 1.0)) as f32;
        if let Some(p) = &self.previous {
            painter.image(p.tex.id(), self.placed(rect, &p.camera, p.size), uv, Color32::WHITE);
        }
        if let Some(c) = &self.current {
            let alpha = if self.previous.is_some() { a } else { 1.0 };
            painter.image(c.tex.id(), self.placed(rect, &c.camera, c.size), uv, Color32::WHITE.gamma_multiply(alpha));
        }
        // Labels are anchored to the map (positions follow the camera, text doesn't scale), so
        // they never vanish mid-gesture.
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        for l in &self.labels.live {
            if l.alpha <= 0.01 {
                continue;
            }
            let (sx, sy) = self.camera.to_screen(l.at, w, h);
            let at = pos2(rect.min.x + sx as f32, rect.min.y + sy as f32);
            if rect.expand(200.0).contains(at) {
                draw_label(&painter, &l.label, at, l.alpha);
            }
        }
    }

    /// Gestures and keys (Interaction specification › Map gestures and keys). Returns whether
    /// the camera moved.
    fn input(&mut self, ui: &Ui, resp: &egui::Response, rect: Rect, t: f64, dt: f64, opts: ViewOptions) -> bool {
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        let mut moved = false;
        let stop_anim = |me: &mut MapView| {
            me.anim = None;
        };
        // Drag: the map follows the pointer; release keeps its speed (inertia).
        if resp.dragged() {
            let d = resp.drag_delta();
            camera::pan(&mut self.camera, f64::from(d.x), f64::from(d.y));
            if dt > 0.0 {
                self.velocity = self.velocity * 0.5 + (d / dt as f32) * 0.5;
            }
            stop_anim(self);
            moved = true;
        } else if self.velocity.length() > INERTIA_STOP as f32 && !resp.drag_stopped() {
            let d = self.velocity * dt as f32;
            camera::pan(&mut self.camera, f64::from(d.x), f64::from(d.y));
            self.velocity *= (-dt / INERTIA_TAU_S).exp() as f32;
            moved = true;
        } else if !resp.drag_stopped() {
            self.velocity = egui::Vec2::ZERO;
        }
        if resp.drag_stopped() && ui.input(|i| i.pointer.velocity().length()) < 30.0 {
            // A drag that ends at rest doesn't glide.
            self.velocity = egui::Vec2::ZERO;
        }
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
        if let Some(p) = hover {
            let local = p - rect.min;
            for e in ui.input(|i| i.events.clone()) {
                match e {
                    // A purely sideways notch (tilt wheel, ⇧-scroll) doesn't zoom.
                    egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta, .. } if delta.y == 0.0 => {}
                    egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta, .. } => {
                        // A mouse wheel: half a zoom level per notch, animated over 250 ms.
                        let from = self.zoom();
                        let base = match self.anim {
                            Some(Animation::Zoom { to, .. }) => to,
                            _ => from,
                        };
                        let to = camera::clamp_zoom(base + 0.5 * f64::from(delta.y.signum()), h);
                        self.anim = Some(Animation::Zoom { from, to, start: t, anchor: (f64::from(local.x), f64::from(local.y)) });
                        self.velocity = egui::Vec2::ZERO;
                        moved = true;
                    }
                    egui::Event::MouseWheel { delta, .. } => {
                        // A trackpad's two-finger scroll pans, as in Apple Maps.
                        camera::pan(&mut self.camera, f64::from(delta.x), f64::from(delta.y));
                        self.velocity = egui::Vec2::ZERO;
                        stop_anim(self);
                        moved = true;
                    }
                    egui::Event::Zoom(f) => {
                        // Pinch: continuous, about the pinch centre, never snapped.
                        let z = self.zoom() + f64::from(f).log2();
                        camera::zoom_about(&mut self.camera, z, f64::from(local.x), f64::from(local.y), w, h);
                        stop_anim(self);
                        moved = true;
                    }
                    _ => {}
                }
            }
        }
        if resp.double_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            // Zoom one level about the click (⌥ zooms out).
            let local = p - rect.min;
            let from = self.zoom();
            let to = camera::clamp_zoom(if ui.input(|i| i.modifiers.alt) { from - 1.0 } else { from + 1.0 }, h);
            self.anim = Some(Animation::Zoom { from, to, start: t, anchor: (f64::from(local.x), f64::from(local.y)) });
            self.velocity = egui::Vec2::ZERO;
        }
        if opts.keyboard {
            let (l, r, u, d, zin, zout) = ui.input(|i| {
                let k = |key| i.key_pressed(key) && !i.modifiers.command;
                (
                    k(egui::Key::ArrowLeft),
                    k(egui::Key::ArrowRight),
                    k(egui::Key::ArrowUp),
                    k(egui::Key::ArrowDown),
                    k(egui::Key::Plus) || k(egui::Key::Equals),
                    k(egui::Key::Minus),
                )
            });
            // Arrows pan a quarter of the view, towards the arrow.
            let (qx, qy) = (w / 4.0, h / 4.0);
            let (dx, dy) = (
                if l {
                    qx
                } else if r {
                    -qx
                } else {
                    0.0
                },
                if u {
                    qy
                } else if d {
                    -qy
                } else {
                    0.0
                },
            );
            if dx != 0.0 || dy != 0.0 {
                camera::pan(&mut self.camera, dx, dy);
                stop_anim(self);
                moved = true;
            }
            if zin || zout {
                self.zoom_by(if zin { 1.0 } else { -1.0 }, t, rect.size());
            }
        }
        if let Some(a) = self.anim {
            let (c, done) = camera::step(&a, self.camera, t, w, h);
            self.camera = c;
            if done {
                self.anim = None;
            }
            moved = true;
        }
        moved
    }
}

fn draw_label(p: &egui::Painter, l: &PlacedLabel, at: Pos2, alpha: f32) {
    let font = FontId::proportional(l.size * 1.33);
    let align = if l.align < -0.25 {
        Align2::LEFT_CENTER
    } else if l.align > 0.25 {
        Align2::RIGHT_CENTER
    } else {
        Align2::CENTER_CENTER
    };
    let c = |c: rustgis_map::color::Rgba| Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3]).gamma_multiply(alpha);
    if let Some(h) = l.halo {
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.7, -0.7), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7)] {
            p.text(at + vec2(dx, dy), align, &l.text, font.clone(), c(h));
        }
    }
    p.text(at, align, &l.text, font, c(l.color));
}
