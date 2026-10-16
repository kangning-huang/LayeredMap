//! The render thread (ADR-001, strategy B2). It owns its own copy of the map; the UI sends the
//! newest view (older requests are dropped) and map changes, and polls for finished frames, so
//! the UI thread never waits on the CPU renderer.

use std::sync::mpsc::{Receiver, Sender, channel};

use rustgis_map::Map;
use rustgis_map::camera::Camera;
use rustgis_render::{PlacedLabel, View};

enum Msg {
    Map(Box<Map>),
    Render { seq: u64, view: View },
}

/// A finished render.
pub struct Frame {
    pub seq: u64,
    pub camera: Camera,
    /// The view it was drawn for, in logical points.
    pub size: (f64, f64),
    pub pixel_size: [usize; 2],
    pub rgba: Vec<u8>,
    pub labels: Vec<PlacedLabel>,
    pub attribution: Vec<String>,
    pub tile_errors: Vec<String>,
    pub tiles_pending: bool,
    pub render_ms: f64,
}

pub struct Worker {
    tx: Sender<Msg>,
    rx: Receiver<Frame>,
    seq: u64,
    map_rev: u64,
    last: Option<(Camera, [u32; 2], u64, u64)>,
    pub in_flight: bool,
}

impl Worker {
    pub fn spawn(map: Map, ctx: egui::Context) -> Worker {
        let (tx, req) = channel::<Msg>();
        let (out, rx) = channel::<Frame>();
        let spawned = std::thread::Builder::new().name("lm-render".into()).spawn(move || {
            let mut map = map;
            while let Ok(first) = req.recv() {
                // Apply every message waiting; render only the newest view.
                let mut render = None;
                for msg in std::iter::once(first).chain(std::iter::from_fn(|| req.try_recv().ok())) {
                    match msg {
                        Msg::Map(m) => map = *m,
                        Msg::Render { seq, view } => render = Some((seq, view)),
                    }
                }
                let Some((seq, view)) = render else { continue };
                let t = web_time::Instant::now();
                if let Some(r) = rustgis_render::render(&mut map, &view) {
                    let frame = Frame {
                        seq,
                        camera: view.camera,
                        size: (view.width, view.height),
                        pixel_size: [r.pixmap.width() as usize, r.pixmap.height() as usize],
                        rgba: r.pixmap.data().to_vec(),
                        labels: r.labels,
                        attribution: r.attribution,
                        tile_errors: r.tile_errors,
                        tiles_pending: r.tiles_pending,
                        render_ms: t.elapsed().as_secs_f64() * 1000.0,
                    };
                    if out.send(frame).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            }
        });
        if let Err(e) = spawned {
            log::error!("could not start the map render thread: {e}");
        }
        Worker { tx, rx, seq: 0, map_rev: 0, last: None, in_flight: false }
    }

    /// Replaces the worker's map (a basemap change, a new layer); the next render shows it.
    pub fn set_map(&mut self, map: Map) {
        self.map_rev += 1;
        self.last = None;
        let _ = self.tx.send(Msg::Map(Box::new(map)));
    }

    /// Asks for `view` unless it was the last thing asked for (same map and tile generation).
    pub fn request(&mut self, view: View, tiles_gen: u64) {
        let (pw, ph) = view.pixel_size();
        let key = (view.camera, [pw, ph], tiles_gen, self.map_rev);
        if self.last == Some(key) {
            return;
        }
        self.seq += 1;
        self.last = Some(key);
        self.in_flight = true;
        let _ = self.tx.send(Msg::Render { seq: self.seq, view });
    }

    /// The newest finished frame, if any arrived since the last poll.
    pub fn poll(&mut self) -> Option<Frame> {
        let newest = self.rx.try_iter().last();
        if newest.as_ref().is_some_and(|f| f.seq == self.seq) {
            self.in_flight = false;
        }
        newest
    }
}
