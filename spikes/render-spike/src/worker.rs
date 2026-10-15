//! A render thread that owns its own copy of the map. The UI sends the view it wants; the thread
//! renders only the newest request (older ones are dropped), so the UI thread never waits on the
//! CPU renderer.

use std::sync::mpsc::{Receiver, Sender, channel};

use rustgis_map::Map;
use rustgis_map::camera::Camera;
use rustgis_render::{PlacedLabel, View};

pub struct Request {
    pub seq: u64,
    pub view: View,
}

pub struct Frame {
    pub seq: u64,
    pub camera: Camera,
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
    pub labels: Vec<PlacedLabel>,
    pub attribution: Vec<String>,
    pub render_ms: f64,
    pub tiles_pending: bool,
}

pub struct Worker {
    tx: Sender<Request>,
    rx: Receiver<Frame>,
    pub next_seq: u64,
    /// The last request sent, and whether its frame has come back.
    pub last_sent: Option<(Camera, [u32; 2], u64)>,
    pub in_flight: bool,
}

impl Worker {
    pub fn spawn(mut map: Map, ctx: egui::Context) -> Worker {
        let (tx, req_rx) = channel::<Request>();
        let (frame_tx, rx) = channel::<Frame>();
        std::thread::Builder::new()
            .name("lm-render".into())
            .spawn(move || {
                while let Ok(mut req) = req_rx.recv() {
                    // Only the newest request matters.
                    while let Ok(newer) = req_rx.try_recv() {
                        req = newer;
                    }
                    let t = web_time::Instant::now();
                    if let Some(r) = rustgis_render::render(&mut map, &req.view) {
                        let frame = Frame {
                            seq: req.seq,
                            camera: req.view.camera,
                            size: [r.pixmap.width() as usize, r.pixmap.height() as usize],
                            rgba: r.pixmap.data().to_vec(),
                            labels: r.labels,
                            attribution: r.attribution,
                            render_ms: t.elapsed().as_secs_f64() * 1000.0,
                            tiles_pending: r.tiles_pending,
                        };
                        if frame_tx.send(frame).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                }
            })
            .ok();
        Worker { tx, rx, next_seq: 0, last_sent: None, in_flight: false }
    }

    /// Asks for `view` unless it was the last thing asked for (with the same tile generation).
    pub fn request(&mut self, view: View, tiles_gen: u64) {
        let size = [view.pixel_size().0, view.pixel_size().1];
        if self.last_sent == Some((view.camera, size, tiles_gen)) {
            return;
        }
        self.next_seq += 1;
        self.last_sent = Some((view.camera, size, tiles_gen));
        self.in_flight = true;
        let _ = self.tx.send(Request { seq: self.next_seq, view });
    }

    /// The newest finished frame, if any arrived.
    pub fn poll(&mut self) -> Option<Frame> {
        let mut out = None;
        while let Ok(f) = self.rx.try_recv() {
            out = Some(f);
        }
        if let Some(f) = &out
            && f.seq == self.next_seq
        {
            self.in_flight = false;
        }
        out
    }
}
