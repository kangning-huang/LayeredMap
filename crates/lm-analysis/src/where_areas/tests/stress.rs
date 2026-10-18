//! Speed and cancel latency on global grids (release mode):
//!
//! `cargo test -p lm-analysis --release --lib where_areas::tests::stress -- --ignored --nocapture`

use super::*;
use std::time::{Duration, Instant};

/// A global grid of `w × h` cells with a smooth, noisy field (`seed` varies it).
fn global(w: usize, h: usize, seed: u64) -> Raster {
    let mut data = field(w, h, seed);
    // Some NoData (oceans, say): a band of rows and a block.
    for r in h / 2..h / 2 + h / 40 {
        for c in 0..w / 3 {
            data[r * w + c] = ND;
        }
    }
    let cell = 360.0 / w as f64;
    lonlat(w as u32, h as u32, -180.0, 90.0, cell, data)
}

fn four(rs: &[Raster]) -> Vec<Condition<'_>> {
    let tests = [Test::AtLeast(0.2), Test::AtMost(1.2), Test::Between(-0.5, 1.5), Test::AtLeast(-0.8)];
    rs.iter().zip(tests).map(|(r, t)| Condition { band: MemoryBand::new(r, 0).unwrap(), test: t }).collect()
}

fn timed(w: usize, h: usize) -> (WhereResult, f64) {
    let rs: Vec<Raster> = (0..4).map(|k| global(w, h, 11 + k)).collect();
    let mut c = four(&rs);
    let t = Instant::now();
    let res = find_where(&mut c, &Control::none()).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let parts: usize = res.areas.iter().map(|a| if let Geometry::Polygon(p) = &a.geometry { p.len() } else { 0 }).sum();
    let vertices: usize =
        res.areas.iter().map(|a| if let Geometry::Polygon(p) = &a.geometry { p.iter().flatten().map(Vec::len).sum() } else { 0 }).sum();
    eprintln!(
        "{w} × {h}, 4 conditions: {:.2} s; {} areas, {parts} parts, {vertices} vertices, {} of {} cells in",
        secs,
        res.areas.len(),
        res.cells_in,
        res.grid_cells
    );
    (res, secs)
}

#[test]
#[ignore]
fn ten_minute_global_grid_with_four_conditions() {
    let (res, secs) = timed(2160, 1080);
    assert!(res.areas.len() > 100);
    if !cfg!(debug_assertions) {
        assert!(secs < 3.0, "{secs:.2} s");
    }
}

#[test]
#[ignore]
fn five_minute_global_grid_with_four_conditions() {
    let (res, _) = timed(4320, 2160);
    assert!(res.areas.len() > 100);
    for a in res.areas.iter().step_by(97) {
        assert!(to_geo(&a.geometry).is_valid());
    }
}

#[test]
#[ignore]
fn a_cancel_lands_quickly_on_a_big_grid() {
    let rs: Vec<Raster> = (0..4).map(|k| global(4320, 2160, 21 + k)).collect();
    let ctl = Control::none();
    let c2 = ctl.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        c2.cancel();
        Instant::now()
    });
    let mut c = four(&rs);
    let r = find_where(&mut c, &ctl);
    let stopped = Instant::now();
    let at = canceller.join().unwrap();
    let latency = stopped.saturating_duration_since(at);
    eprintln!("cancel latency: {:.1} ms ({:?})", latency.as_secs_f64() * 1e3, r.as_ref().map(|r| r.areas.len()));
    if r.is_err() {
        assert_eq!(r, Err(ZonalError::Cancelled));
        assert!(latency < Duration::from_millis(200), "{latency:?}");
    }
}
