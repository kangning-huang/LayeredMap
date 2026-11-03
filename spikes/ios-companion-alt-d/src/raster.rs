//! Rasters for the Alt C spike: read with `rustgis-data`, resample onto a Web Mercator grid (what
//! MapLibre's image source stretches linearly), color, and write a PNG plus a grid of values for
//! "tap to read the value". Swift gets an envelope with the bounds and the legend range.

use rustgis_data::Coord;
use rustgis_data::raster::{self, ColorHint, Raster};
use serde_json::json;
use std::path::Path;

/// Longest side of the picture MapLibre draws. Enough for a phone screen; keeps memory small.
const MAX_SIDE: u32 = 1024;
const MAX_LAT: f64 = 85.05;

/// Viridis, low → high (the app's "Standard" ramp).
const VIRIDIS: [[u8; 3]; 5] = [[0x44, 0x01, 0x54], [0x3B, 0x52, 0x8B], [0x21, 0x91, 0x8C], [0x5E, 0xC9, 0x62], [0xFD, 0xE7, 0x25]];

fn merc_y(lat: f64) -> f64 {
    let lat = lat.clamp(-MAX_LAT, MAX_LAT).to_radians();
    (std::f64::consts::FRAC_PI_4 + lat / 2.0).tan().ln()
}

fn lat_of(y: f64) -> f64 {
    y.sinh().atan().to_degrees()
}

fn ramp(t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0) * (VIRIDIS.len() - 1) as f64;
    let i = (t.floor() as usize).min(VIRIDIS.len() - 2);
    let f = t - i as f64;
    let (a, b) = (VIRIDIS[i], VIRIDIS[i + 1]);
    [0, 1, 2].map(|k| (a[k] as f64 + (b[k] as f64 - a[k] as f64) * f).round() as u8)
}

/// How the pixels are shown: a picture (a scanned map, a photo) or measured values on a ramp.
fn is_picture(r: &Raster) -> bool {
    match &r.color {
        ColorHint::Rgb { .. } | ColorHint::Palette(_) => true,
        // One byte band of grey: a scan, shown in grey rather than as a value ramp.
        ColorHint::Gray | ColorHint::GrayAlpha => {
            r.data_type == "UInt8" && r.band_count() <= 2 && r.bands.first().is_some_and(|b| b.stats.max - b.stats.min > 100.0)
        }
    }
}

/// Open a raster, write `<out_stem>.png` (and `<out_stem>.values` for value rasters), and describe it:
/// `{"name","look":"picture|values","bounds":[w,s,e,n],"png","values","grid":[w,h],"low","high",
/// "min","max","bands","data_type","cell_m"}`.
pub fn open_raster_envelope(path: &str, out_stem: &str) -> Result<String, String> {
    // NetCDF goes through the spike's own pure-Rust reader: `rustgis-data` reads it only by
    // shelling out to GDAL, which does not exist on an iPhone. Everything after this is shared.
    let p = Path::new(path);
    let (r, quantity, unit, notes) = if crate::netcdf::is_netcdf(p) {
        let o = crate::netcdf::open(p)?;
        (o.raster, Some(o.quantity), Some(o.unit), o.notes)
    } else {
        (raster::read_path(p).map_err(|e| e.to_string())?, None, None, Vec::new())
    };
    let e = r.extent();
    let (west, east) = (e.min_x.max(-180.0), e.max_x.min(180.0));
    let (south, north) = (e.min_y.max(-MAX_LAT), e.max_y.min(MAX_LAT));
    if !(east > west && north > south) {
        return Err("the raster's location couldn't be worked out".into());
    }
    let (yn, ys) = (merc_y(north), merc_y(south));
    let span_x = (east - west).to_radians();
    let span_y = yn - ys;
    let longest = r.width.max(r.height).clamp(64, MAX_SIDE) as f64;
    let (w, h) =
        if span_x >= span_y { (longest, (longest * span_y / span_x).max(1.0)) } else { ((longest * span_x / span_y).max(1.0), longest) };
    let (w, h) = (w.round() as u32, h.round() as u32);

    let picture = is_picture(&r);
    let band0 = r.bands.first().ok_or("the raster has no bands")?;
    let (low, high) = (band0.stats.percentile(2.0), band0.stats.percentile(98.0));
    let (low, high) = if high > low { (low, high) } else { (band0.stats.min, band0.stats.max.max(band0.stats.min + 1e-9)) };
    // Byte bands are shown as they are; others are stretched 2–98 %.
    let stretch = |band: usize, v: f32| -> u8 {
        if r.data_type == "UInt8" {
            return v.clamp(0.0, 255.0) as u8;
        }
        let s = r.bands.get(band).map(|b| (b.stats.percentile(2.0), b.stats.percentile(98.0))).unwrap_or((low, high));
        (((v as f64 - s.0) / (s.1 - s.0).max(1e-9)).clamp(0.0, 1.0) * 255.0) as u8
    };

    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut values: Vec<f32> = if picture { Vec::new() } else { vec![f32::NAN; (w * h) as usize] };
    for j in 0..h {
        let lat = lat_of(yn - (j as f64 + 0.5) / h as f64 * span_y);
        for i in 0..w {
            let lon = west + (i as f64 + 0.5) / w as f64 * (east - west);
            let Some((col, row)) = r.pixel_at(Coord::new(lon, lat)) else { continue };
            let px = ((j * w + i) * 4) as usize;
            let Some(out) = rgba.get_mut(px..px + 4) else { continue };
            if picture {
                let color: Option<[u8; 4]> = match &r.color {
                    ColorHint::Rgb { alpha } => {
                        let rgb = [0, 1, 2].map(|b| r.value(b, col, row));
                        match rgb {
                            [Some(a), Some(b), Some(c)] => {
                                let al = if *alpha { r.value(3, col, row).map(|v| stretch(3, v)).unwrap_or(255) } else { 255 };
                                Some([stretch(0, a), stretch(1, b), stretch(2, c), al])
                            }
                            _ => None,
                        }
                    }
                    ColorHint::Palette(table) => r.value(0, col, row).and_then(|v| table.get(v.max(0.0) as usize).copied()),
                    ColorHint::Gray | ColorHint::GrayAlpha => r.value(0, col, row).map(|v| {
                        let g = stretch(0, v);
                        [g, g, g, 255]
                    }),
                };
                if let Some(c) = color {
                    out.copy_from_slice(&c);
                }
            } else if let Some(v) = r.value(0, col, row) {
                let c = ramp((v as f64 - low) / (high - low));
                out.copy_from_slice(&[c[0], c[1], c[2], 255]);
                if let Some(slot) = values.get_mut((j * w + i) as usize) {
                    *slot = v;
                }
            }
        }
    }

    let png = format!("{out_stem}.png");
    image::save_buffer(&png, &rgba, w, h, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?;
    let values_path = if picture {
        None
    } else {
        let path = format!("{out_stem}.values");
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Some(path)
    };
    let (cx, cy) = r.cell_size_m();
    Ok(json!({
        "name": r.name,
        "look": if picture { "picture" } else { "values" },
        "bounds": [west, south, east, north],
        "png": png,
        "values": values_path,
        "grid": [w, h],
        "low": low,
        "high": high,
        "min": band0.stats.min,
        "max": band0.stats.max,
        "bands": r.band_count(),
        "data_type": r.data_type,
        "cell_m": (cx + cy) / 2.0,
        // NetCDF only: the file says what it measures and in what unit, so Swift does not have to
        // guess them from the file name.
        "quantity": quantity,
        "unit": unit,
        "notes": notes,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_data::raster::{Crs, GeoTransform};

    /// LM_RASTER_RAW=<dir with rasters.json> LM_RASTER_OUT=<dir>: write the persona study's
    /// synthetic GeoTIFFs (LM-tmp/personas/scripts/make_rasters.py), then open each one here.
    #[test]
    #[ignore]
    fn make_persona_rasters() {
        let (Ok(raw), Ok(out)) = (std::env::var("LM_RASTER_RAW"), std::env::var("LM_RASTER_OUT")) else { return };
        let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{raw}/rasters.json")).unwrap()).unwrap();
        for (key, target) in [("walt", "West Riding map 1852.tif"), ("priya", "lisbon_elevation.tif")] {
            let m = &meta[key];
            let (w, h) = (m["w"].as_u64().unwrap() as u32, m["h"].as_u64().unwrap() as u32);
            let bytes = std::fs::read(format!("{raw}/{}", m["file"].as_str().unwrap())).unwrap();
            let transform = GeoTransform {
                origin_x: m["origin"][0].as_f64().unwrap(),
                origin_y: m["origin"][1].as_f64().unwrap(),
                pixel_width: m["pixel"][0].as_f64().unwrap(),
                pixel_height: m["pixel"][1].as_f64().unwrap(),
            };
            let crs = Crs::from_epsg(m["epsg"].as_u64().unwrap() as u32).unwrap();
            let r = if key == "walt" {
                let n = (w * h) as usize;
                let bands = (0..3).map(|b| (0..n).map(|i| bytes[i * 3 + b] as f32).collect()).collect();
                Raster::new(m["name"].as_str().unwrap(), w, h, bands, transform, crs, None, ColorHint::Rgb { alpha: false }, "UInt8")
                    .unwrap()
            } else {
                let band: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
                Raster::new(m["name"].as_str().unwrap(), w, h, vec![band], transform, crs, m["nodata"].as_f64(), ColorHint::Gray, "Float32")
                    .unwrap()
            };
            let tif = format!("{out}/{target}");
            std::fs::write(&tif, rustgis_data::raster::geotiff::write(&r, 0).unwrap()).unwrap();
            let env = open_raster_envelope(&tif, &format!("{out}/check-{key}")).unwrap();
            println!("{target}: {env}");
        }
    }
}
