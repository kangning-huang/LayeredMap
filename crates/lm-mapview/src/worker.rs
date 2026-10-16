//! The render thread (ADR-001, strategy B2). It owns its own copy of the map; the UI sends the
//! newest view (older requests are dropped) and map changes, and polls for finished frames, so
//! the UI thread never waits on the CPU renderer.

use std::sync::mpsc::{Receiver, Sender, channel};

use rustgis_map::Map;
use rustgis_map::camera::Camera;
use rustgis_render::{PlacedLabel, SelectionStyle, View};

/// An edit applied to the render thread's copy of the map, the same as on the UI's.
pub type Edit = Box<dyn FnOnce(&mut Map) + Send>;

enum Msg {
    Edit(Edit),
    Render { seq: u64, view: View, selection: SelectionStyle },
}

/// What the render thread sends back: a frame, or word that a render produced nothing (so the
/// UI doesn't wait for it forever).
enum Out {
    Frame(Frame),
    Nothing(u64),
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
    rx: Receiver<Out>,
    seq: u64,
    map_rev: u64,
    last: Option<(Camera, [u32; 2], u64, u64, SelectionStyle)>,
    pub in_flight: bool,
    /// The render thread has stopped (it panicked); the view restarts it.
    pub dead: bool,
}

impl Worker {
    pub fn spawn(map: Map, ctx: egui::Context) -> Worker {
        let (tx, req) = channel::<Msg>();
        let (out, rx) = channel::<Out>();
        let spawned = std::thread::Builder::new().name("lm-render".into()).spawn(move || {
            let mut map = map;
            while let Ok(first) = req.recv() {
                // Apply every message waiting; render only the newest view.
                let mut render = None;
                for msg in std::iter::once(first).chain(std::iter::from_fn(|| req.try_recv().ok())) {
                    match msg {
                        Msg::Edit(f) => f(&mut map),
                        Msg::Render { seq, view, selection } => render = Some((seq, view, selection)),
                    }
                }
                let Some((seq, view, selection)) = render else { continue };
                let t = web_time::Instant::now();
                let Some(r) = rustgis_render::render_styled(&mut map, &view, selection) else {
                    if out.send(Out::Nothing(seq)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                    continue;
                };
                {
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
                    if out.send(Out::Frame(frame)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            }
        });
        if let Err(e) = spawned {
            log::error!("could not start the map render thread: {e}");
        }
        Worker { tx, rx, seq: 0, map_rev: 0, last: None, in_flight: false, dead: false }
    }

    /// Applies an edit to the worker's map (a basemap change, a new layer); the next render shows
    /// it.
    pub fn edit(&mut self, f: Edit) {
        self.map_rev += 1;
        self.last = None;
        if self.tx.send(Msg::Edit(f)).is_err() {
            self.dead = true;
        }
    }

    /// Asks for `view` unless it was the last thing asked for (same map, tile generation and
    /// selection style).
    pub fn request(&mut self, view: View, selection: SelectionStyle, tiles_gen: u64) {
        let (pw, ph) = view.pixel_size();
        let key = (view.camera, [pw, ph], tiles_gen, self.map_rev, selection);
        if self.last == Some(key) {
            return;
        }
        self.seq += 1;
        self.last = Some(key);
        self.in_flight = true;
        if self.tx.send(Msg::Render { seq: self.seq, view, selection }).is_err() {
            self.dead = true;
            self.in_flight = false;
        }
    }

    /// The newest finished frame, if any arrived since the last poll.
    pub fn poll(&mut self) -> Option<Frame> {
        let mut newest = None;
        loop {
            match self.rx.try_recv() {
                Ok(Out::Frame(f)) => {
                    if f.seq == self.seq {
                        self.in_flight = false;
                    }
                    newest = Some(f);
                }
                Ok(Out::Nothing(seq)) => {
                    if seq == self.seq {
                        self.in_flight = false;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.dead = true;
                    self.in_flight = false;
                    break;
                }
            }
        }
        newest
    }
}
