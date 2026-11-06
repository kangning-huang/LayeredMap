//! A layer written to disk for the app, instead of handed over as one GeoJSON string.
//!
//! The old hand-over held a layer five times at once: the reader's dataset, a JSON value of it,
//! that value as text, Swift's parse of the text (an object per value, a dictionary per row) and
//! the map's own copy. On a phone — about 3.3 GB for the app — 154,400 white storks ended it.
//! Now each layer is four files, streamed as they are written, and Swift keeps only compact
//! columns:
//!
//! - `<stem>.geojson` — for MapLibre: each feature's shape and its index (`_lm_i`), nothing else.
//!   Every feature is written as `{"type":"Feature","geometry":…,"properties":{"_lm_i":N}}`, so the
//!   app can splice one more property in before the closing `}}` (the field a layer is coloured by).
//! - `<stem>.idx` — where each feature is in the GeoJSON (u64 offset, u32 length, little-endian),
//!   so one feature's shape can be read back (the outline drawn when it is tapped).
//! - `<stem>.geo` — eight f64 per feature: centre lon/lat, bounds (min lon, min lat, max lon, max
//!   lat), area km², length km; NaN where not applicable. Same formulas as the app's GeoMeasure.
//! - `<stem>.cols` — the attribute table by column: numbers as f64 (NaN = empty), anything else as
//!   text with each distinct value stored once (a bird's name, not 154,400 copies of it).

use rustgis_data::{Coord, Dataset, Geometry, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

const EARTH_KM: f64 = 6371.0088;

pub struct Written {
    pub map: PathBuf,
    pub idx: PathBuf,
    pub geo: PathBuf,
    pub cols: PathBuf,
}

fn io(e: std::io::Error) -> String {
    e.to_string()
}

fn num(out: &mut impl Write, v: f64) -> std::io::Result<()> {
    if v.is_finite() { write!(out, "{v}") } else { out.write_all(b"null") }
}

fn coord(out: &mut impl Write, c: &Coord) -> std::io::Result<()> {
    out.write_all(b"[")?;
    num(out, c.x)?;
    out.write_all(b",")?;
    num(out, c.y)?;
    out.write_all(b"]")
}

fn coords(out: &mut impl Write, cs: &[Coord]) -> std::io::Result<()> {
    out.write_all(b"[")?;
    for (i, c) in cs.iter().enumerate() {
        if i > 0 {
            out.write_all(b",")?;
        }
        coord(out, c)?;
    }
    out.write_all(b"]")
}

fn rings(out: &mut impl Write, rs: &[Vec<Coord>]) -> std::io::Result<()> {
    out.write_all(b"[")?;
    for (i, r) in rs.iter().enumerate() {
        if i > 0 {
            out.write_all(b",")?;
        }
        coords(out, r)?;
    }
    out.write_all(b"]")
}

/// GeoJSON geometry, as `to_geojson` writes it (one part → Line/Polygon, several → Multi…).
fn geometry(out: &mut impl Write, g: Option<&Geometry>) -> std::io::Result<()> {
    match g {
        None => out.write_all(b"null"),
        Some(Geometry::Point(c)) => {
            out.write_all(br#"{"type":"Point","coordinates":"#)?;
            coord(out, c)?;
            out.write_all(b"}")
        }
        Some(Geometry::MultiPoint(cs)) => {
            out.write_all(br#"{"type":"MultiPoint","coordinates":"#)?;
            coords(out, cs)?;
            out.write_all(b"}")
        }
        Some(Geometry::Line(parts)) if parts.len() == 1 => {
            out.write_all(br#"{"type":"LineString","coordinates":"#)?;
            coords(out, &parts[0])?;
            out.write_all(b"}")
        }
        Some(Geometry::Line(parts)) => {
            out.write_all(br#"{"type":"MultiLineString","coordinates":"#)?;
            rings(out, parts)?;
            out.write_all(b"}")
        }
        Some(Geometry::Polygon(polys)) if polys.len() == 1 => {
            out.write_all(br#"{"type":"Polygon","coordinates":"#)?;
            rings(out, &polys[0])?;
            out.write_all(b"}")
        }
        Some(Geometry::Polygon(polys)) => {
            out.write_all(br#"{"type":"MultiPolygon","coordinates":["#)?;
            for (i, p) in polys.iter().enumerate() {
                if i > 0 {
                    out.write_all(b",")?;
                }
                rings(out, p)?;
            }
            out.write_all(b"]}")
        }
    }
}

fn rad(d: f64) -> f64 {
    d.to_radians()
}

fn length(line: &[Coord]) -> f64 {
    line.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let h = (rad(b.y - a.y) / 2.0).sin().powi(2) + rad(a.y).cos() * rad(b.y).cos() * (rad(b.x - a.x) / 2.0).sin().powi(2);
            2.0 * EARTH_KM * h.sqrt().min(1.0).asin()
        })
        .sum()
}

fn ring_area(r: &[Coord]) -> f64 {
    if r.len() < 3 {
        return 0.0;
    }
    let mut total = 0.0;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[(i + 1) % r.len()]);
        total += rad(b.x - a.x) * (2.0 + rad(a.y).sin() + rad(b.y).sin());
    }
    (total * EARTH_KM * EARTH_KM / 2.0).abs()
}

fn polygon_area(rs: &[Vec<Coord>]) -> f64 {
    let Some(outer) = rs.first() else { return 0.0 };
    (ring_area(outer) - rs.iter().skip(1).map(|h| ring_area(h)).sum::<f64>()).max(0.0)
}

#[derive(Clone, Copy)]
struct Bounds([f64; 4]);

impl Bounds {
    fn empty() -> Bounds {
        Bounds([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY])
    }
    fn add(&mut self, c: &Coord) {
        if c.x.is_finite() && c.y.is_finite() {
            self.0 = [self.0[0].min(c.x), self.0[1].min(c.y), self.0[2].max(c.x), self.0[3].max(c.y)];
        }
    }
    fn valid(&self) -> bool {
        self.0[0] <= self.0[2] && self.0[1] <= self.0[3]
    }
    fn center(&self) -> (f64, f64) {
        ((self.0[0] + self.0[2]) / 2.0, (self.0[1] + self.0[3]) / 2.0)
    }
}

/// Centre lon/lat, bounds, area km², length km — as the app's `GeoMeasure` measures them.
fn measure(g: Option<&Geometry>) -> [f64; 8] {
    let nan = f64::NAN;
    let mut b = Bounds::empty();
    let (mut area, mut len) = (nan, nan);
    let mut center = None;
    match g {
        None => {}
        Some(Geometry::Point(c)) => b.add(c),
        Some(Geometry::MultiPoint(cs)) => cs.iter().for_each(|c| b.add(c)),
        Some(Geometry::Line(parts)) => {
            let mut total = 0.0;
            for p in parts {
                p.iter().for_each(|c| b.add(c));
                total += length(p);
            }
            len = total;
        }
        Some(Geometry::Polygon(polys)) => {
            let (mut total, mut biggest, mut biggest_b) = (0.0, 0.0, Bounds::empty());
            for p in polys {
                let mut pb = Bounds::empty();
                for c in p.first().into_iter().flatten() {
                    pb.add(c);
                    b.add(c);
                }
                let a = polygon_area(p);
                total += a;
                if a > biggest {
                    biggest = a;
                    biggest_b = pb;
                }
            }
            area = total;
            if polys.len() > 1 && biggest_b.valid() {
                center = Some(biggest_b.center());
            }
        }
    }
    if !b.valid() {
        return [nan; 8];
    }
    let (cx, cy) = center.unwrap_or_else(|| b.center());
    [cx, cy, b.0[0], b.0[1], b.0[2], b.0[3], area, len]
}

fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Text(s) => Some(s.clone()),
        Value::Bool(b) => Some(if *b { "Yes" } else { "No" }.to_string()),
    }
}

fn put_str(out: &mut impl Write, s: &str) -> std::io::Result<()> {
    out.write_all(&(s.len() as u32).to_le_bytes())?;
    out.write_all(s.as_bytes())
}

/// Writes the four files for `ds` into `dir` as `<stem>.*`.
pub fn write_layer(ds: &Dataset, dir: &Path, stem: &str) -> Result<Written, String> {
    std::fs::create_dir_all(dir).map_err(io)?;
    let w = Written {
        map: dir.join(format!("{stem}.geojson")),
        idx: dir.join(format!("{stem}.idx")),
        geo: dir.join(format!("{stem}.geo")),
        cols: dir.join(format!("{stem}.cols")),
    };

    // The map's GeoJSON and where each feature sits in it.
    let mut map = BufWriter::with_capacity(1 << 20, File::create(&w.map).map_err(io)?);
    let mut idx = BufWriter::new(File::create(&w.idx).map_err(io)?);
    let mut geo = BufWriter::new(File::create(&w.geo).map_err(io)?);
    let head = br#"{"type":"FeatureCollection","features":["#;
    map.write_all(head).map_err(io)?;
    let mut at = head.len() as u64;
    let mut feature = Vec::with_capacity(4096);
    for (i, f) in ds.features.iter().enumerate() {
        if i > 0 {
            map.write_all(b",").map_err(io)?;
            at += 1;
        }
        feature.clear();
        feature.extend_from_slice(br#"{"type":"Feature","geometry":"#);
        geometry(&mut feature, f.geometry.as_ref()).map_err(io)?;
        write!(feature, r#","properties":{{"_lm_i":{i}}}}}"#).map_err(io)?;
        map.write_all(&feature).map_err(io)?;
        idx.write_all(&at.to_le_bytes()).map_err(io)?;
        idx.write_all(&(feature.len() as u32).to_le_bytes()).map_err(io)?;
        at += feature.len() as u64;
        for v in measure(f.geometry.as_ref()) {
            geo.write_all(&v.to_le_bytes()).map_err(io)?;
        }
    }
    map.write_all(b"]}").map_err(io)?;
    map.flush().map_err(io)?;
    idx.flush().map_err(io)?;
    geo.flush().map_err(io)?;

    // The attribute table, a column at a time.
    let mut cols = BufWriter::with_capacity(1 << 20, File::create(&w.cols).map_err(io)?);
    let rows = ds.features.len();
    cols.write_all(b"LMC1").map_err(io)?;
    cols.write_all(&(rows as u32).to_le_bytes()).map_err(io)?;
    cols.write_all(&(ds.fields.len() as u32).to_le_bytes()).map_err(io)?;
    for (j, field) in ds.fields.iter().enumerate() {
        put_str(&mut cols, &field.name).map_err(io)?;
        let cell = |r: usize| ds.features.get(r).and_then(|f| f.attrs.get(j)).unwrap_or(&Value::Null);
        let numeric = (0..rows).all(|r| matches!(cell(r), Value::Null | Value::Int(_) | Value::Float(_)));
        if numeric {
            cols.write_all(&[0]).map_err(io)?;
            for r in 0..rows {
                let v = match cell(r) {
                    Value::Int(i) => *i as f64,
                    Value::Float(f) => *f,
                    _ => f64::NAN,
                };
                cols.write_all(&v.to_le_bytes()).map_err(io)?;
            }
        } else {
            cols.write_all(&[1]).map_err(io)?;
            let mut dict: Vec<String> = Vec::new();
            let mut lookup: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
            let mut codes: Vec<u32> = Vec::with_capacity(rows);
            for r in 0..rows {
                codes.push(match text_of(cell(r)) {
                    None => u32::MAX,
                    Some(s) => *lookup.entry(s.clone()).or_insert_with(|| {
                        dict.push(s);
                        (dict.len() - 1) as u32
                    }),
                });
            }
            cols.write_all(&(dict.len() as u32).to_le_bytes()).map_err(io)?;
            for s in &dict {
                put_str(&mut cols, s).map_err(io)?;
            }
            for c in codes {
                cols.write_all(&c.to_le_bytes()).map_err(io)?;
            }
        }
    }
    cols.flush().map_err(io)?;
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_data::{Field, FieldType, GeometryType};

    #[test]
    fn writes_spliceable_features_and_columns() {
        let mut ds = Dataset::new("t", GeometryType::Polygon, vec![Field::new("name", FieldType::Text), Field::new("n", FieldType::Double)]);
        let sq = vec![vec![Coord::new(0.0, 0.0), Coord::new(1.0, 0.0), Coord::new(1.0, 1.0), Coord::new(0.0, 1.0), Coord::new(0.0, 0.0)]];
        ds.push(Some(Geometry::Polygon(vec![sq])), vec![Value::Text("a".into()), Value::Float(2.5)]);
        ds.push(None, vec![Value::Text("a".into()), Value::Null]);
        let dir = std::env::temp_dir().join(format!("lm-store-{}", std::process::id()));
        let w = write_layer(&ds, &dir, "x").expect("write");
        let map = std::fs::read(&w.map).expect("map");
        let v: serde_json::Value = serde_json::from_slice(&map).expect("valid GeoJSON");
        assert_eq!(v["features"][1]["properties"]["_lm_i"], 1);
        // The index finds each feature, and each ends in "}}" so a property can be spliced in.
        let idx = std::fs::read(&w.idx).expect("idx");
        let off = u64::from_le_bytes(idx[12..20].try_into().expect("8")) as usize;
        let len = u32::from_le_bytes(idx[20..24].try_into().expect("4")) as usize;
        let one = &map[off..off + len];
        assert!(one.ends_with(b"}}"));
        let f: serde_json::Value = serde_json::from_slice(one).expect("one feature");
        assert!(f["geometry"].is_null());
        // A 1° square at the equator: about 12,364 km².
        let geo = std::fs::read(&w.geo).expect("geo");
        let area = f64::from_le_bytes(geo[48..56].try_into().expect("8"));
        assert!((area - 12_364.0).abs() < 50.0, "{area}");
        let cols = std::fs::read(&w.cols).expect("cols");
        assert_eq!(&cols[..4], b"LMC1");
    }
}
