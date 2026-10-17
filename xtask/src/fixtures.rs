//! `cargo xtask fixtures [--out DIR] [--only F1,X1,…] [--large]`: writes the generated test
//! fixtures (build plan › Test fixtures and stress tests).
//!
//! Every file is a pure function of its ID: a fixed seed, no clock, no network, so two runs
//! produce identical bytes (and `fixtures/generated/CHECKSUMS.sha256`-style comparisons work).
//! The writers here (GeoJSON, CSV, ZIP, GeoTIFF/BigTIFF) are deliberately independent of the
//! RustGIS readers the fixtures test.
//!
//! The output directory is `$LM_FIXTURES_DIR`, else `fixtures/generated` (ignored by git). The
//! large rasters (R4 2 GB, R5 4 GB) are written only with `--large` or `--only`. N7's 3,000 areas
//! (for the R4 summary stress test) are small and always written.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Shanghai, where the classroom pilot's data lives: lon/lat bounds of the generated layers.
const SHANGHAI: [f64; 4] = [120.85, 30.65, 122.0, 31.85];

/// SplitMix64: a tiny, well-mixed, reproducible generator.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
    /// Standard normal (Box–Muller).
    fn normal(&mut self) -> f64 {
        let u = self.f().max(1e-12);
        let v = self.f();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

const CATEGORIES: [&str; 8] = ["Residential", "Commercial", "Industrial", "Park", "School", "Hospital", "Transit", "Water"];

/// Metres per degree of longitude and latitude near `lat`.
fn m_per_deg(lat: f64) -> (f64, f64) {
    (111_320.0 * lat.to_radians().cos(), 110_574.0)
}

/// Clustered points over a bounding box: 30 Gaussian clusters plus a uniform sprinkle.
fn clustered_points(rng: &mut Rng, n: usize, b: [f64; 4]) -> Vec<(f64, f64)> {
    let centers: Vec<(f64, f64, f64)> = (0..30).map(|_| (rng.range(b[0], b[2]), rng.range(b[1], b[3]), rng.range(0.01, 0.08))).collect();
    (0..n)
        .map(|i| {
            if i % 5 == 0 {
                (rng.range(b[0], b[2]), rng.range(b[1], b[3]))
            } else {
                let (cx, cy, s) = centers[rng.below(centers.len() as u64) as usize];
                ((cx + rng.normal() * s).clamp(b[0], b[2]), (cy + rng.normal() * s).clamp(b[1], b[3]))
            }
        })
        .collect()
}

fn coord(x: f64, y: f64) -> String {
    format!("[{x:.6},{y:.6}]")
}

/// A GeoJSON FeatureCollection writer that streams one feature per line.
struct GeoJson {
    w: BufWriter<File>,
    first: bool,
}

impl GeoJson {
    fn create(path: &Path) -> Result<GeoJson, String> {
        let mut w = BufWriter::new(File::create(path).map_err(|e| format!("{}: {e}", path.display()))?);
        w.write_all(b"{\"type\":\"FeatureCollection\",\"features\":[\n").map_err(|e| e.to_string())?;
        Ok(GeoJson { w, first: true })
    }
    fn feature(&mut self, geometry: &str, properties: &str) -> Result<(), String> {
        let sep = if self.first { "" } else { ",\n" };
        self.first = false;
        write!(self.w, "{sep}{{\"type\":\"Feature\",\"properties\":{{{properties}}},\"geometry\":{geometry}}}").map_err(|e| e.to_string())
    }
    fn finish(mut self) -> Result<(), String> {
        self.w.write_all(b"\n]}\n").and_then(|_| self.w.flush()).map_err(|e| e.to_string())
    }
}

fn point_props(rng: &mut Rng, i: usize) -> String {
    let cat = CATEGORIES[rng.below(8) as usize];
    format!(
        "\"id\":{i},\"name\":\"P{i}\",\"category\":\"{cat}\",\"value\":{:.3},\"count\":{}",
        20.0 + 15.0 * rng.f() + 3.0 * rng.normal(),
        rng.below(500)
    )
}

/// An irregular star-shaped ring of `n` vertices around (cx, cy), closed.
fn star_ring(rng: &mut Rng, cx: f64, cy: f64, radius_m: f64, n: usize) -> Vec<(f64, f64)> {
    let (mx, my) = m_per_deg(cy);
    let mut ring: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            let r = radius_m * rng.range(0.55, 1.0);
            (cx + r * a.cos() / mx, cy + r * a.sin() / my)
        })
        .collect();
    if let Some(&p) = ring.first() {
        ring.push(p);
    }
    ring
}

fn ring_json(ring: &[(f64, f64)]) -> String {
    let pts: Vec<String> = ring.iter().map(|&(x, y)| coord(x, y)).collect();
    format!("[{}]", pts.join(","))
}

/// F1: the map-feel load. 100,000 clustered points and 10,000 polygons (under 1,000,000
/// vertices in all) over Shanghai.
fn f1(out: &Path) -> Result<String, String> {
    let dir = out.join("F1");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut rng = Rng::new(0xF1);
    let mut gj = GeoJson::create(&dir.join("f1_points.geojson"))?;
    for (i, (x, y)) in clustered_points(&mut rng, 100_000, SHANGHAI).into_iter().enumerate() {
        let props = point_props(&mut rng, i);
        gj.feature(&format!("{{\"type\":\"Point\",\"coordinates\":{}}}", coord(x, y)), &props)?;
    }
    gj.finish()?;
    let mut gj = GeoJson::create(&dir.join("f1_polygons.geojson"))?;
    let mut vertices = 0usize;
    for i in 0..10_000 {
        let n = 20 + rng.below(141) as usize; // 20..=160, mean 90
        let (cx, cy, radius) = (rng.range(SHANGHAI[0], SHANGHAI[2]), rng.range(SHANGHAI[1], SHANGHAI[3]), rng.range(200.0, 1500.0));
        let ring = star_ring(&mut rng, cx, cy, radius, n);
        vertices += ring.len();
        let props = format!(
            "\"id\":{i},\"name\":\"Area {i}\",\"category\":\"{}\",\"value\":{:.3}",
            CATEGORIES[rng.below(8) as usize],
            25.0 + 8.0 * rng.normal()
        );
        gj.feature(&format!("{{\"type\":\"Polygon\",\"coordinates\":[{}]}}", ring_json(&ring)), &props)?;
    }
    gj.finish()?;
    if vertices > 1_000_000 {
        return Err(format!("F1 has {vertices} polygon vertices; the plan caps it at 1,000,000"));
    }
    Ok(format!("F1: 100,000 points, 10,000 polygons, {vertices} polygon vertices"))
}

/// A CSV of `n` clustered points with lon/lat and five attributes.
fn points_csv(path: &Path, seed: u64, n: usize) -> Result<(), String> {
    let mut rng = Rng::new(seed);
    let mut w = BufWriter::new(File::create(path).map_err(|e| format!("{}: {e}", path.display()))?);
    let pts = clustered_points(&mut rng, n, SHANGHAI);
    writeln!(w, "id,lon,lat,category,value,count,code,label").map_err(|e| e.to_string())?;
    for (i, (x, y)) in pts.into_iter().enumerate() {
        writeln!(
            w,
            "{i},{x:.6},{y:.6},{},{:.3},{},{:06},Point {i}",
            CATEGORIES[rng.below(8) as usize],
            20.0 + 15.0 * rng.f(),
            rng.below(500),
            rng.below(1_000_000)
        )
        .map_err(|e| e.to_string())?;
    }
    w.flush().map_err(|e| e.to_string())
}

/// F2: 1,000,000 points with 5 attribute fields (CSV with lon/lat).
fn f2(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("F2")).map_err(|e| e.to_string())?;
    points_csv(&out.join("F2/f2_points.csv"), 0xF2, 1_000_000)?;
    Ok("F2: 1,000,000 points".into())
}

/// T3: a 1,000,000-row CSV (a different seed from F2, so the two are not the same file).
fn t3(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("T3")).map_err(|e| e.to_string())?;
    points_csv(&out.join("T3/t3_rows.csv"), 0x73, 1_000_000)?;
    Ok("T3: 1,000,000 rows".into())
}

/// V7: 100,000 polygons, a jittered hexagon-ish grid over Shanghai.
fn v7(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("V7")).map_err(|e| e.to_string())?;
    let mut rng = Rng::new(0x77);
    let mut gj = GeoJson::create(&out.join("V7/v7_polygons.geojson"))?;
    let (cols, rows) = (400usize, 250usize);
    let dx = (SHANGHAI[2] - SHANGHAI[0]) / cols as f64;
    let dy = (SHANGHAI[3] - SHANGHAI[1]) / rows as f64;
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            let (x0, y0) = (SHANGHAI[0] + c as f64 * dx, SHANGHAI[1] + r as f64 * dy);
            let j = |rng: &mut Rng| rng.range(-0.08, 0.08);
            let ring = [
                (x0 + dx * j(&mut rng), y0 + dy * j(&mut rng)),
                (x0 + dx * (1.0 + j(&mut rng)), y0 + dy * j(&mut rng)),
                (x0 + dx * (1.0 + j(&mut rng)), y0 + dy * (1.0 + j(&mut rng))),
                (x0 + dx * (0.5 + j(&mut rng)), y0 + dy * (1.1 + j(&mut rng))),
                (x0 + dx * j(&mut rng), y0 + dy * (1.0 + j(&mut rng))),
            ];
            let mut closed = ring.to_vec();
            closed.push(ring[0]);
            let props = format!(
                "\"id\":{i},\"code\":\"{:06}\",\"category\":\"{}\",\"value\":{:.3}",
                i,
                CATEGORIES[rng.below(8) as usize],
                100.0 * rng.f()
            );
            gj.feature(&format!("{{\"type\":\"Polygon\",\"coordinates\":[{}]}}", ring_json(&closed)), &props)?;
        }
    }
    gj.finish()?;
    Ok("V7: 100,000 polygons".into())
}

/// V8: 1,000 points with 200 fields of mixed types.
fn v8(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("V8")).map_err(|e| e.to_string())?;
    let mut rng = Rng::new(0x78);
    let mut gj = GeoJson::create(&out.join("V8/v8_wide.geojson"))?;
    for (i, (x, y)) in clustered_points(&mut rng, 1_000, SHANGHAI).into_iter().enumerate() {
        let mut props = vec![format!("\"id\":{i}")];
        for f in 1..200 {
            props.push(match f % 4 {
                0 => format!("\"text_{f:03}\":\"v{}\"", rng.below(50)),
                1 => format!("\"measure_{f:03}\":{:.3}", 1000.0 * rng.f()),
                2 => format!("\"count_{f:03}\":{}", rng.below(10_000)),
                _ => format!("\"cat_{f:03}\":\"{}\"", CATEGORIES[rng.below(8) as usize]),
            });
        }
        gj.feature(&format!("{{\"type\":\"Point\",\"coordinates\":{}}}", coord(x, y)), &props.join(","))?;
    }
    gj.finish()?;
    Ok("V8: 1,000 points × 200 fields".into())
}

// ---- ZIP (for X1, X2) ----------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A ZIP archive in memory: (name, uncompressed bytes) entries, deflated.
fn zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let comp = miniz_oxide::deflate::compress_to_vec(data, 9);
        let crc = crc32(data);
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend(sig.to_le_bytes());
            if central {
                h.extend(20u16.to_le_bytes()); // made by
            }
            h.extend(20u16.to_le_bytes()); // version needed
            h.extend(0u16.to_le_bytes()); // flags
            h.extend(8u16.to_le_bytes()); // deflate
            h.extend(0u32.to_le_bytes()); // time, date
            h.extend(crc.to_le_bytes());
            h.extend((comp.len() as u32).to_le_bytes());
            h.extend((data.len() as u32).to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes()); // extra
            if central {
                h.extend(0u16.to_le_bytes()); // comment
                h.extend(0u16.to_le_bytes()); // disk
                h.extend(0u16.to_le_bytes()); // internal attrs
                h.extend(0u32.to_le_bytes()); // external attrs
                h.extend(offset.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend(&comp);
        central.extend(header(0x0201_4b50, true));
    }
    let cd_offset = out.len() as u32;
    out.extend(&central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(cd_offset.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

/// X1: a nested ZIP bomb. Each level holds 16 copies of the level below; the innermost entry is
/// 256 MiB of zeros, so full expansion is 16³ × 256 MiB = 1 TiB from a few kilobytes.
fn x1(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("X1")).map_err(|e| e.to_string())?;
    // Compress 256 MiB of zeros without holding it: deflate is linear here, so build it from a
    // 1 MiB block compressed once per level is not possible; instead compress in one go (256 MiB
    // of RAM, briefly) — acceptable for a fixture generator.
    let inner = zip(&[("zeros.bin", vec![0u8; 256 << 20])]);
    let mut level = inner;
    for depth in 0..3 {
        let names: Vec<String> = (0..16).map(|i| format!("level{depth}_{i:02}.zip")).collect();
        let entries: Vec<(&str, Vec<u8>)> = names.iter().map(|n| (n.as_str(), level.clone())).collect();
        level = zip(&entries);
    }
    std::fs::write(out.join("X1/x1_bomb.zip"), &level).map_err(|e| e.to_string())?;
    Ok(format!("X1: nested ZIP bomb, {} bytes on disk, 1 TiB expanded", level.len()))
}

/// X2: a ZIP whose entries climb out of the extraction folder or name absolute paths.
fn x2(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("X2")).map_err(|e| e.to_string())?;
    let shp = b"not really a shapefile".to_vec();
    let z = zip(&[
        ("../../evil.shp", shp.clone()),
        ("ok/../../../etc/evil.dbf", shp.clone()),
        ("/tmp/absolute.shx", shp.clone()),
        ("..\\windows\\evil.prj", shp),
    ]);
    std::fs::write(out.join("X2/x2_traversal.zip"), z).map_err(|e| e.to_string())?;
    Ok("X2: path-traversal ZIP".into())
}

/// X3: hostile GeoJSON — extreme nesting, a huge coordinate array, NaN and Infinity.
fn x3(out: &Path) -> Result<String, String> {
    let dir = out.join("X3");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let depth = 100_000;
    let nested = format!(
        "{{\"type\":\"Feature\",\"properties\":{{}},\"geometry\":{{\"type\":\"Polygon\",\"coordinates\":{}0{}}}}}",
        "[".repeat(depth),
        "]".repeat(depth)
    );
    std::fs::write(dir.join("x3_nesting.geojson"), nested).map_err(|e| e.to_string())?;
    let mut w = BufWriter::new(File::create(dir.join("x3_huge_array.geojson")).map_err(|e| e.to_string())?);
    w.write_all(b"{\"type\":\"Feature\",\"properties\":{},\"geometry\":{\"type\":\"LineString\",\"coordinates\":[")
        .map_err(|e| e.to_string())?;
    for i in 0..20_000_000u64 {
        let sep = if i == 0 { "" } else { "," };
        write!(w, "{sep}[{},{}]", i % 360, i % 90).map_err(|e| e.to_string())?;
    }
    w.write_all(b"]}}\n").and_then(|_| w.flush()).map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("x3_nan.geojson"),
        "{\"type\":\"FeatureCollection\",\"features\":[\
         {\"type\":\"Feature\",\"properties\":{\"v\":NaN},\"geometry\":{\"type\":\"Point\",\"coordinates\":[NaN,NaN]}},\
         {\"type\":\"Feature\",\"properties\":{},\"geometry\":{\"type\":\"Point\",\"coordinates\":[Infinity,-Infinity]}},\
         {\"type\":\"Feature\",\"properties\":{},\"geometry\":{\"type\":\"Point\",\"coordinates\":[1e400,1e400]}},\
         {\"type\":\"Feature\",\"properties\":{},\"geometry\":{\"type\":\"Point\",\"coordinates\":[121.5,31.2]}}]}\n",
    )
    .map_err(|e| e.to_string())?;
    Ok("X3: nesting, huge array, NaN coordinates".into())
}

// ---- TIFF (for X4, R4, R5, R7) -------------------------------------------------------------

#[derive(Clone, Copy)]
enum Layout {
    Strips { rows: u32 },
    Tiles { size: u32 },
}

/// One image (full resolution or an overview) of a float32 single-band GeoTIFF.
struct Image {
    width: u32,
    height: u32,
    layout: Layout,
    overview: bool,
}

/// Writes a single-band float32 GeoTIFF (BigTIFF when `big`) in EPSG:4326, with optional
/// overviews: the full image first in the IFD chain, overviews after it, and — as a COG lays them
/// out — every IFD at the start of the file and the smallest overview's pixels first.
/// `value(x, y, level)` gives the pixel at column `x`, row `y` of level `level` (0 = full).
fn write_geotiff(
    path: &Path,
    big: bool,
    images: &[Image],
    origin: (f64, f64),
    pixel: (f64, f64),
    nodata: Option<&str>,
    value: &dyn Fn(u32, u32, usize) -> f32,
) -> Result<u64, String> {
    let f = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut w = BufWriter::with_capacity(1 << 22, f);
    let off = |v: u64| -> Vec<u8> { if big { v.to_le_bytes().to_vec() } else { (v as u32).to_le_bytes().to_vec() } };
    // Chunk (strip or tile) sizes per image.
    let chunks = |im: &Image| -> Vec<(u32, u32, u32, u32)> {
        // (x0, y0, w, h) in pixels; tiles are padded to full size, strips are not.
        match im.layout {
            Layout::Strips { rows } => {
                (0..im.height.div_ceil(rows)).map(|s| (0, s * rows, im.width, rows.min(im.height - s * rows))).collect()
            }
            Layout::Tiles { size } => {
                let mut v = Vec::new();
                for ty in 0..im.height.div_ceil(size) {
                    for tx in 0..im.width.div_ceil(size) {
                        v.push((tx * size, ty * size, size, size));
                    }
                }
                v
            }
        }
    };
    // Lay out the IFDs first (sizes are fixed), then the pixel data, smallest image first.
    let entry_size: u64 = if big { 20 } else { 12 };
    let header_size: u64 = if big { 16 } else { 8 };
    let geokeys: Vec<u16> = vec![1, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326];
    let ifd_meta: Vec<(usize, u64)> = images
        .iter()
        .enumerate()
        .map(|(i, im)| {
            let layout_tags = match im.layout {
                Layout::Strips { .. } => 3,
                Layout::Tiles { .. } => 4,
            };
            let n_entries: u64 = 9 + layout_tags + if i == 0 { 3 + u64::from(nodata.is_some()) } else { 0 };
            let n_chunks = chunks(im).len() as u64;
            let val = if big { 8 } else { 4 };
            // IFD itself + out-of-line arrays (offsets, counts, and for image 0 the geo tags).
            let geo = if i == 0 { 24 + 48 + geokeys.len() as u64 * 2 + nodata.map_or(0, |s| s.len() as u64 + 1) } else { 0 };
            (n_entries as usize, (if big { 8 } else { 2 }) + n_entries * entry_size + val + 2 * n_chunks * val + geo + 8)
        })
        .collect();
    let mut ifd_pos = Vec::new();
    let mut p = header_size;
    for (_, size) in &ifd_meta {
        ifd_pos.push(p);
        p += size + (p + size) % 2;
    }
    let data_start = p;
    // Pixel data positions: smallest image (last) first.
    let mut chunk_offsets: Vec<Vec<(u64, u64)>> = vec![Vec::new(); images.len()];
    let mut d = data_start;
    for (i, im) in images.iter().enumerate().rev() {
        for (_, _, cw, ch) in chunks(im) {
            let bytes = u64::from(cw) * u64::from(ch) * 4;
            chunk_offsets[i].push((d, bytes));
            d += bytes;
        }
    }
    // Header.
    if big {
        w.write_all(b"II").and_then(|_| w.write_all(&43u16.to_le_bytes())).map_err(|e| e.to_string())?;
        w.write_all(&8u16.to_le_bytes()).and_then(|_| w.write_all(&0u16.to_le_bytes())).map_err(|e| e.to_string())?;
    } else {
        w.write_all(b"II").and_then(|_| w.write_all(&42u16.to_le_bytes())).map_err(|e| e.to_string())?;
    }
    w.write_all(&off(ifd_pos[0])).map_err(|e| e.to_string())?;
    for (i, im) in images.iter().enumerate() {
        let base = ifd_pos[i];
        w.seek(SeekFrom::Start(base)).map_err(|e| e.to_string())?;
        let (n_entries, _) = ifd_meta[i];
        let val = if big { 8u64 } else { 4 };
        let n_chunks = chunk_offsets[i].len() as u64;
        let after_ifd = base + (if big { 8 } else { 2 }) + n_entries as u64 * entry_size + val;
        let offs_at = after_ifd;
        let counts_at = offs_at + n_chunks * val;
        let geo_at = counts_at + n_chunks * val;
        // (tag, type, count, value-or-offset)
        let long_t: u16 = if big { 16 } else { 4 };
        let mut entries: Vec<(u16, u16, u64, u64)> = vec![
            (254, 4, 1, u64::from(im.overview)),
            (256, 4, 1, u64::from(im.width)),
            (257, 4, 1, u64::from(im.height)),
            (258, 3, 1, 32),
            (259, 3, 1, 1),
            (262, 3, 1, 1),
            (277, 3, 1, 1),
            (284, 3, 1, 1),
            (339, 3, 1, 3),
        ];
        match im.layout {
            Layout::Strips { rows } => {
                entries.push((273, long_t, n_chunks, if n_chunks == 1 { chunk_offsets[i][0].0 } else { offs_at }));
                entries.push((278, 4, 1, u64::from(rows)));
                entries.push((279, long_t, n_chunks, if n_chunks == 1 { chunk_offsets[i][0].1 } else { counts_at }));
            }
            Layout::Tiles { size } => {
                entries.push((322, 4, 1, u64::from(size)));
                entries.push((323, 4, 1, u64::from(size)));
                entries.push((324, long_t, n_chunks, if n_chunks == 1 { chunk_offsets[i][0].0 } else { offs_at }));
                entries.push((325, long_t, n_chunks, if n_chunks == 1 { chunk_offsets[i][0].1 } else { counts_at }));
            }
        }
        if i == 0 {
            entries.push((33550, 12, 3, geo_at));
            entries.push((33922, 12, 6, geo_at + 24));
            entries.push((34735, 3, geokeys.len() as u64, geo_at + 72));
            if let Some(s) = nodata {
                entries.push((42113, 2, s.len() as u64 + 1, geo_at + 72 + geokeys.len() as u64 * 2));
            }
        }
        entries.sort_by_key(|e| e.0);
        if entries.len() != n_entries {
            return Err(format!("internal: IFD {i} has {} entries, sized for {n_entries}", entries.len()));
        }
        if big {
            w.write_all(&(entries.len() as u64).to_le_bytes()).map_err(|e| e.to_string())?;
        } else {
            w.write_all(&(entries.len() as u16).to_le_bytes()).map_err(|e| e.to_string())?;
        }
        for (tag, typ, count, v) in &entries {
            w.write_all(&tag.to_le_bytes()).and_then(|_| w.write_all(&typ.to_le_bytes())).map_err(|e| e.to_string())?;
            w.write_all(&off(*count)).map_err(|e| e.to_string())?;
            // SHORT values sit left-justified in the value field.
            let mut field = vec![0u8; val as usize];
            let bytes: Vec<u8> = match (tag, nodata) {
                // A value that fits the field is stored in it, not at an offset (TIFF 6 §2).
                (42113, Some(s)) if *count <= val => s.as_bytes().to_vec(),
                _ if *typ == 3 && *count == 1 => (*v as u16).to_le_bytes().to_vec(),
                _ => off(*v),
            };
            field[..bytes.len().min(val as usize)].copy_from_slice(&bytes[..bytes.len().min(val as usize)]);
            w.write_all(&field).map_err(|e| e.to_string())?;
        }
        let next = ifd_pos.get(i + 1).copied().unwrap_or(0);
        w.write_all(&off(next)).map_err(|e| e.to_string())?;
        w.seek(SeekFrom::Start(offs_at)).map_err(|e| e.to_string())?;
        if n_chunks > 1 {
            for (o, _) in &chunk_offsets[i] {
                w.write_all(&off(*o)).map_err(|e| e.to_string())?;
            }
            for (_, c) in &chunk_offsets[i] {
                w.write_all(&off(*c)).map_err(|e| e.to_string())?;
            }
        }
        if i == 0 {
            w.seek(SeekFrom::Start(geo_at)).map_err(|e| e.to_string())?;
            for v in [pixel.0, pixel.1, 0.0] {
                w.write_all(&v.to_le_bytes()).map_err(|e| e.to_string())?;
            }
            for v in [0.0, 0.0, 0.0, origin.0, origin.1, 0.0] {
                w.write_all(&v.to_le_bytes()).map_err(|e| e.to_string())?;
            }
            for k in &geokeys {
                w.write_all(&k.to_le_bytes()).map_err(|e| e.to_string())?;
            }
            if let Some(s) = nodata {
                w.write_all(s.as_bytes()).and_then(|_| w.write_all(&[0])).map_err(|e| e.to_string())?;
            }
        }
    }
    // Pixels, smallest image first, each chunk row by row.
    w.seek(SeekFrom::Start(data_start)).map_err(|e| e.to_string())?;
    let mut row = Vec::new();
    for (i, im) in images.iter().enumerate().rev() {
        for (x0, y0, cw, ch) in chunks(im) {
            for y in y0..y0 + ch {
                row.clear();
                for x in x0..x0 + cw {
                    let v = if x < im.width && y < im.height { value(x, y, i) } else { 0.0 };
                    row.extend_from_slice(&v.to_le_bytes());
                }
                w.write_all(&row).map_err(|e| e.to_string())?;
            }
        }
    }
    w.flush().map_err(|e| e.to_string())?;
    Ok(d)
}

/// A smooth synthetic "surface temperature" field in °C over a unit square.
fn field(u: f64, v: f64) -> f32 {
    (30.0 + 4.0 * (u * 9.0).sin() * (v * 7.0).cos() + 2.0 * ((u + v) * 23.0).sin() - 3.0 * v) as f32
}

/// R7: a geographic raster from 10° S to 70° N (unequal cell areas), 1,600 × 1,600.
fn r7(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("R7")).map_err(|e| e.to_string())?;
    let (w, h) = (1600u32, 1600u32);
    let im = [Image { width: w, height: h, layout: Layout::Strips { rows: 16 }, overview: false }];
    let px = 80.0 / f64::from(h);
    let bytes = write_geotiff(&out.join("R7/r7_wide_latitude.tif"), false, &im, (100.0, 70.0), (px, px), None, &|x, y, _| {
        field(f64::from(x) / f64::from(w), f64::from(y) / f64::from(h))
    })?;
    Ok(format!("R7: {w}×{h} float32, 10°S–70°N, {bytes} bytes"))
}

/// R4: a ~2 GB cloud-optimised GeoTIFF over Shanghai with overviews (512 px tiles).
fn r4(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("R4")).map_err(|e| e.to_string())?;
    let side = 23_040u32; // 23,040² × 4 B ≈ 2.12 GB at full resolution
    let mut images = Vec::new();
    let mut s = side;
    let mut first = true;
    while s >= 256 {
        images.push(Image { width: s, height: s, layout: Layout::Tiles { size: 512 }, overview: !first });
        first = false;
        s /= 2;
    }
    let deg = (SHANGHAI[2] - SHANGHAI[0]).max(SHANGHAI[3] - SHANGHAI[1]);
    let sizes: Vec<u32> = images.iter().map(|i| i.width).collect();
    let bytes = write_geotiff(
        &out.join("R4/r4_cog.tif"),
        true,
        &images,
        (SHANGHAI[0], SHANGHAI[3]),
        (deg / f64::from(side), deg / f64::from(side)),
        Some("-9999"),
        &|x, y, l| {
            let n = f64::from(sizes[l]);
            field(f64::from(x) / n, f64::from(y) / n)
        },
    )?;
    Ok(format!("R4: {side}² COG with {} overviews, {bytes} bytes", images.len() - 1))
}

/// R4's extent: (west, north) and side in degrees (the raster is square).
fn r4_extent() -> (f64, f64, f64) {
    (SHANGHAI[0], SHANGHAI[3], (SHANGHAI[2] - SHANGHAI[0]).max(SHANGHAI[3] - SHANGHAI[1]))
}

/// N7: 3,000 areas over R4 for the summary stress test, a 60 × 50 grid of irregular star
/// polygons (16–40 vertices) that overlap their neighbours a little. Every 7th has a hole, every
/// 11th is a multipolygon (a small second part beside it), and the western column and northern
/// row reach past the raster's edge. Ids `P0000`–`P2999`, row by row from the north-west.
fn n7(out: &Path) -> Result<String, String> {
    let dir = out.join("N7");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let (west, north, side) = r4_extent();
    let (across, down) = (60usize, 50usize);
    let (cw, ch) = (side / across as f64, side / down as f64);
    let mut rng = Rng::new(0x4E37);
    let mut gj = GeoJson::create(&dir.join("n7_polygons.geojson"))?;
    let mut vertices = 0usize;
    for j in 0..down {
        for i in 0..across {
            let k = j * across + i;
            let mut cx = west + (i as f64 + 0.5 + rng.range(-0.2, 0.2)) * cw;
            let mut cy = north - (j as f64 + 0.5 + rng.range(-0.2, 0.2)) * ch;
            if i == 0 {
                cx -= 0.4 * cw;
            }
            if j == 0 {
                cy += 0.4 * ch;
            }
            let (rx, ry) = (0.62 * cw, 0.62 * ch);
            let n = 16 + rng.below(25) as usize;
            let mut outer: Vec<(f64, f64)> = (0..n)
                .map(|v| {
                    let a = std::f64::consts::TAU * (v as f64 + rng.range(-0.3, 0.3)) / n as f64;
                    let r = rng.range(0.6, 1.0);
                    (cx + r * rx * a.cos(), cy + r * ry * a.sin())
                })
                .collect();
            outer.push(outer[0]);
            vertices += outer.len();
            let mut rings = vec![ring_json(&outer)];
            if k % 7 == 3 {
                // A hole well inside the smallest radius (0.6), wound the other way.
                let mut hole: Vec<(f64, f64)> = (0..10)
                    .map(|v| {
                        let a = -std::f64::consts::TAU * v as f64 / 10.0;
                        (cx + 0.25 * rx * a.cos(), cy + 0.25 * ry * a.sin())
                    })
                    .collect();
                hole.push(hole[0]);
                vertices += hole.len();
                rings.push(ring_json(&hole));
            }
            let geometry = if k % 11 == 5 {
                // A second part just outside the first's largest radius (1.0).
                let (sx, sy) = (cx + 1.25 * rx, cy);
                let mut part: Vec<(f64, f64)> = (0..8)
                    .map(|v| {
                        let a = std::f64::consts::TAU * v as f64 / 8.0;
                        (sx + 0.15 * rx * a.cos(), sy + 0.15 * ry * a.sin())
                    })
                    .collect();
                part.push(part[0]);
                vertices += part.len();
                format!("{{\"type\":\"MultiPolygon\",\"coordinates\":[[{}],[{}]]}}", rings.join(","), ring_json(&part))
            } else {
                format!("{{\"type\":\"Polygon\",\"coordinates\":[{}]}}", rings.join(","))
            };
            gj.feature(&geometry, &format!("\"area_id\":\"P{k:04}\""))?;
        }
    }
    gj.finish()?;
    Ok(format!("N7: {} areas over R4, {vertices} vertices", across * down))
}

/// R5: a ~4 GB striped BigTIFF without overviews.
fn r5(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("R5")).map_err(|e| e.to_string())?;
    let side = 32_768u32; // 32,768² × 4 B = 4 GiB
    let im = [Image { width: side, height: side, layout: Layout::Strips { rows: 1 }, overview: false }];
    let deg = (SHANGHAI[2] - SHANGHAI[0]).max(SHANGHAI[3] - SHANGHAI[1]);
    let bytes = write_geotiff(
        &out.join("R5/r5_striped.tif"),
        true,
        &im,
        (SHANGHAI[0], SHANGHAI[3]),
        (deg / f64::from(side), deg / f64::from(side)),
        None,
        &|x, y, _| field(f64::from(x) / f64::from(side), f64::from(y) / f64::from(side)),
    )?;
    Ok(format!("R5: {side}² striped BigTIFF, no overviews, {bytes} bytes"))
}

/// X4: a GeoTIFF whose header claims 1,000,000,000 × 1,000,000,000 pixels, with almost no data.
fn x4(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("X4")).map_err(|e| e.to_string())?;
    let mut b = Vec::new();
    b.extend(b"II");
    b.extend(42u16.to_le_bytes());
    b.extend(8u32.to_le_bytes());
    let entries: [(u16, u16, u32, u32); 9] = [
        (256, 4, 1, 1_000_000_000),
        (257, 4, 1, 1_000_000_000),
        (258, 3, 1, 32),
        (259, 3, 1, 1),
        (262, 3, 1, 1),
        (273, 4, 1, 200),
        (277, 3, 1, 1),
        (278, 4, 1, 1_000_000_000),
        (279, 4, 1, 16),
    ];
    b.extend((entries.len() as u16).to_le_bytes());
    for (tag, typ, count, v) in entries {
        b.extend(tag.to_le_bytes());
        b.extend(typ.to_le_bytes());
        b.extend(count.to_le_bytes());
        if typ == 3 {
            b.extend((v as u16).to_le_bytes());
            b.extend([0, 0]);
        } else {
            b.extend(v.to_le_bytes());
        }
    }
    b.extend(0u32.to_le_bytes());
    b.resize(216, 0);
    std::fs::write(out.join("X4/x4_billion.tif"), b).map_err(|e| e.to_string())?;
    Ok("X4: 1e9 × 1e9 header".into())
}

/// X5: a CSV whose cells begin with =, +, - or @ (spreadsheet formula injection on export).
fn x5(out: &Path) -> Result<String, String> {
    std::fs::create_dir_all(out.join("X5")).map_err(|e| e.to_string())?;
    let text = "name,lon,lat,note\n\
                \"=HYPERLINK(\"\"http://example.invalid\"\",\"\"click\"\")\",121.47,31.23,=1+1\n\
                +cmd,121.48,31.24,+SUM(A1:A2)\n\
                -2+3,121.49,31.25,-1\n\
                @SUM(1),121.50,31.26,@me\n\
                plain,121.51,31.27,ok\n";
    std::fs::write(out.join("X5/x5_formulas.csv"), text).map_err(|e| e.to_string())?;
    Ok("X5: formula-injection CSV".into())
}

type Gen = fn(&Path) -> Result<String, String>;

const ALL: &[(&str, Gen, bool)] = &[
    ("F1", f1, false),
    ("F2", f2, false),
    ("V7", v7, false),
    ("V8", v8, false),
    ("T3", t3, false),
    ("R7", r7, false),
    ("N7", n7, false),
    ("X1", x1, false),
    ("X2", x2, false),
    ("X3", x3, false),
    ("X4", x4, false),
    ("X5", x5, false),
    ("R4", r4, true),
    ("R5", r5, true),
];

pub fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let mut out: PathBuf = std::env::var_os("LM_FIXTURES_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("fixtures/generated"));
    let mut only: Option<Vec<String>> = None;
    let mut large = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = PathBuf::from(it.next().ok_or("--out needs a directory")?),
            "--only" => only = Some(it.next().ok_or("--only needs IDs")?.split(',').map(|s| s.trim().to_uppercase()).collect()),
            "--large" => large = true,
            other => return Err(format!("fixtures: unknown argument {other}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    for (id, generate, is_large) in ALL {
        let wanted = match &only {
            Some(list) => list.iter().any(|x| x == id),
            None => !is_large || large,
        };
        if !wanted {
            continue;
        }
        let t = std::time::Instant::now();
        let msg = generate(&out)?;
        println!("{msg} ({:.1} s)", t.elapsed().as_secs_f64());
    }
    println!("fixtures written to {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn generators_are_deterministic() {
        let dir = std::env::temp_dir().join(format!("lm-fixtures-{}", std::process::id()));
        let (a, b) = (dir.join("a"), dir.join("b"));
        for d in [&a, &b] {
            std::fs::create_dir_all(d).unwrap();
            x2(d).unwrap();
            x4(d).unwrap();
            x5(d).unwrap();
            v8(d).unwrap();
            n7(d).unwrap();
        }
        for f in ["X2/x2_traversal.zip", "X4/x4_billion.tif", "X5/x5_formulas.csv", "V8/v8_wide.geojson", "N7/n7_polygons.geojson"] {
            assert_eq!(std::fs::read(a.join(f)).unwrap(), std::fs::read(b.join(f)).unwrap(), "{f}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn small_geotiff_reads_back() {
        let dir = std::env::temp_dir().join(format!("lm-fixtures-tiff-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, big, layout) in [("s.tif", false, Layout::Strips { rows: 3 }), ("t.tif", true, Layout::Tiles { size: 16 })] {
            let im = [Image { width: 40, height: 30, layout, overview: false }, Image { width: 20, height: 15, layout, overview: true }];
            let p = dir.join(name);
            write_geotiff(&p, big, &im, (120.0, 32.0), (0.01, 0.01), Some("-9999"), &|x, y, l| (x + 100 * y) as f32 + 0.5 * l as f32)
                .unwrap();
            let mut dec = tiff::decoder::Decoder::new(std::io::BufReader::new(File::open(&p).unwrap())).unwrap();
            assert_eq!(dec.dimensions().unwrap(), (40, 30));
            let tiff::decoder::DecodingResult::F32(px) = dec.read_image().unwrap() else { panic!("not f32") };
            assert_eq!(px[0], 0.0);
            assert_eq!(px[39 + 40 * 29], (39 + 100 * 29) as f32);
            let scale = dec.get_tag_f64_vec(tiff::tags::Tag::ModelPixelScaleTag).unwrap();
            assert_eq!(scale, [0.01, 0.01, 0.0]);
            assert_eq!(dec.get_tag_ascii_string(tiff::tags::Tag::GdalNodata).unwrap(), "-9999");
            assert!(dec.more_images());
            dec.next_image().unwrap();
            assert_eq!(dec.dimensions().unwrap(), (20, 15));
            let tiff::decoder::DecodingResult::F32(px) = dec.read_image().unwrap() else { panic!("not f32") };
            assert_eq!(px[1], 1.5);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
