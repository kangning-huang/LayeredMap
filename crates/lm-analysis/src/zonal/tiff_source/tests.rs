use super::*;
use crate::control::Control;
use crate::zonal::{MemoryBand, ValueRole, summarize};
use rustgis_data::raster::{Crs, GeoTransform};
use std::path::PathBuf;

fn fixture_path(rel: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", rel].iter().collect()
}

/// One of the GDAL-written files in `fixtures/N7/layouts/` (see its README).
struct Layout {
    file: &'static str,
    data_type: &'static str,
    bands: usize,
    /// Tiled (true) or striped.
    tiled: bool,
    value: fn(i64, usize) -> f64,
    nodata: f64,
    utm: bool,
}

const LAYOUTS: &[Layout] = &[
    Layout {
        file: "cog_deflate_f32.tif",
        data_type: "Float32",
        bands: 1,
        tiled: true,
        value: |v, _| v as f64 + 0.25,
        nodata: -9999.0,
        utm: false,
    },
    Layout {
        file: "tiled_lzw_i16.tif",
        data_type: "Int16",
        bands: 1,
        tiled: true,
        value: |v, _| v as f64 - 100.0,
        nodata: -32768.0,
        utm: false,
    },
    Layout { file: "striped_none_u8.tif", data_type: "UInt8", bands: 1, tiled: false, value: |v, _| v as f64, nodata: 255.0, utm: false },
    Layout {
        file: "striped_deflate_u16.tif",
        data_type: "UInt16",
        bands: 1,
        tiled: false,
        value: |v, _| v as f64 * 200.0,
        nodata: 65535.0,
        utm: false,
    },
    Layout {
        file: "tiled_none_f32_utm.tif",
        data_type: "Float32",
        bands: 1,
        tiled: true,
        value: |v, _| v as f64 + 0.5,
        nodata: f64::NAN,
        utm: true,
    },
    Layout {
        file: "planar_tiled_u8_3band.tif",
        data_type: "UInt8",
        bands: 3,
        tiled: true,
        value: |v, b| ((v + 3 * b as i64) % 251) as f64,
        nodata: 255.0,
        utm: false,
    },
    Layout {
        file: "chunky_striped_u16_3band.tif",
        data_type: "UInt16",
        bands: 3,
        tiled: false,
        value: |v, b| (v + 1000 * b as i64) as f64,
        nodata: 65535.0,
        utm: false,
    },
    Layout {
        file: "striped_packbits_i16.tif",
        data_type: "Int16",
        bands: 1,
        tiled: false,
        value: |v, _| v as f64 - 100.0,
        nodata: -32768.0,
        utm: false,
    },
    Layout {
        file: "tiled_zstd_f32.tif",
        data_type: "Float32",
        bands: 1,
        tiled: true,
        value: |v, _| v as f64 + 0.25,
        nodata: -9999.0,
        utm: false,
    },
    Layout {
        file: "bigtiff_tiled_f64.tif",
        data_type: "Float64",
        bands: 1,
        tiled: true,
        value: |v, _| v as f64 / 8.0 - 3.0,
        nodata: -1e30,
        utm: false,
    },
];

const W: u32 = 150;
const H: u32 = 110;

fn expected(l: &Layout, band: usize, c: u32, r: u32) -> Option<f32> {
    let (c, r) = (i64::from(c), i64::from(r));
    if (c * 3 + r * 5) % 97 == 0 {
        return None;
    }
    Some((l.value)((c * 7 + r * 13) % 251, band) as f32)
}

/// A reproducible sequence of windows, many crossing strip and tile edges.
fn windows(n: usize) -> Vec<(u32, u32, u32, u32)> {
    let mut s = 0x5eed_u64;
    let mut next = move |m: u32| {
        s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((s >> 33) % u64::from(m)) as u32
    };
    let mut v = vec![(0, 0, W, H), (W - 1, H - 1, 1, 1), (0, H - 1, W, 1), (W - 1, 0, 1, H)];
    for _ in 0..n {
        let (c0, r0) = (next(W), next(H));
        v.push((c0, r0, 1 + next(W - c0), 1 + next(H - r0)));
    }
    v
}

#[test]
fn every_layout_reads_window_by_window() {
    for l in LAYOUTS {
        let path = fixture_path(&format!("N7/layouts/{}", l.file));
        // The app's whole-file reader gives the same georeferencing.
        // (Where it can: in tiff 0.11.3 it fails on the LZW file and panics on the uncompressed
        // tiled one, which this reader decodes itself.)
        let whole = match std::panic::catch_unwind(|| rustgis_data::raster::read_path(&path)) {
            Ok(Ok(w)) => Some(w),
            Ok(Err(e)) => {
                eprintln!("{}: rustgis_data::raster::read_path fails: {e}", l.file);
                None
            }
            Err(_) => {
                eprintln!("{}: rustgis_data::raster::read_path panics", l.file);
                None
            }
        };
        for band in 0..l.bands {
            for cache in [DEFAULT_CACHE_SAMPLES, 1] {
                let mut src = TiffSource::open(&path, band).unwrap_or_else(|e| panic!("{}: {e}", l.file)).with_cache_samples(cache);
                assert_eq!(src.data_type(), l.data_type, "{}", l.file);
                assert_eq!(src.chunk_size().0 < W, l.tiled, "{}: {:?}", l.file, src.chunk_size());
                let g = src.grid().clone();
                assert_eq!((g.width, g.height), (W, H), "{}", l.file);
                if let Some(whole) = &whole {
                    assert_eq!((g.transform, g.crs), (whole.transform, whole.crs), "{}", l.file);
                }
                let want_t = if l.utm {
                    GeoTransform { origin_x: 350_000.0, origin_y: 3_460_000.0, pixel_width: 30.0, pixel_height: -30.0 }
                } else {
                    GeoTransform { origin_x: 120.5, origin_y: 31.9, pixel_width: 0.002, pixel_height: -0.0025 }
                };
                assert_eq!(g.transform, want_t, "{}", l.file);
                if l.utm {
                    assert_eq!(g.crs.epsg(), 32651);
                } else {
                    assert_eq!(g.crs, Crs::LonLat);
                }
                let nd = g.nodata.unwrap_or_else(|| panic!("{}: no NoData", l.file));
                assert!(nd == l.nodata || (nd.is_nan() && l.nodata.is_nan()), "{}: NoData {nd}", l.file);
                let mut checked = 0usize;
                for (c0, r0, cols, rows) in windows(60) {
                    let mut out = vec![0f32; (cols * rows) as usize];
                    src.read_window(c0, r0, cols, rows, &mut out).unwrap_or_else(|e| panic!("{}: {e}", l.file));
                    for r in 0..rows {
                        for c in 0..cols {
                            let got = out[(r * cols + c) as usize];
                            match expected(l, band, c0 + c, r0 + r) {
                                Some(v) => assert_eq!(got, v, "{} band {band} at ({}, {})", l.file, c0 + c, r0 + r),
                                None => assert!(!g.is_valid(got), "{} band {band} at ({}, {}): {got}", l.file, c0 + c, r0 + r),
                            }
                            checked += 1;
                        }
                    }
                }
                assert!(checked > 2 * (W * H) as usize, "{checked}");
            }
        }
    }
}

#[test]
fn only_the_needed_chunks_are_decoded_and_kept() {
    let path = fixture_path("N7/layouts/tiled_lzw_i16.tif");
    let mut src = TiffSource::open(&path, 0).unwrap();
    assert_eq!(src.chunk_size(), (32, 32));
    let mut out = vec![0f32; 4];
    // A 2 × 2 window across a tile corner touches four tiles…
    src.read_window(31, 31, 2, 2, &mut out).unwrap();
    assert_eq!(src.chunks_decoded(), 4);
    // …and reading inside them again decodes nothing.
    src.read_window(30, 30, 2, 2, &mut out).unwrap();
    assert_eq!(src.chunks_decoded(), 4);
    // With room for one tile, alternating between two tiles decodes each time.
    let mut small = TiffSource::open(&path, 0).unwrap().with_cache_samples(1);
    let mut one = [0f32; 1];
    for k in 0..6 {
        small.read_window(if k % 2 == 0 { 0 } else { 40 }, 0, 1, 1, &mut one).unwrap();
    }
    assert_eq!(small.chunks_decoded(), 6);
}

#[test]
fn bad_files_bands_and_windows_are_errors() {
    let path = fixture_path("N7/layouts/striped_none_u8.tif");
    assert!(matches!(TiffSource::open(&path, 1), Err(ZonalError::Input(_))));
    assert!(matches!(TiffSource::open(&fixture_path("N7/layouts/missing.tif"), 0), Err(ZonalError::Read(_))));
    let mut src = TiffSource::open(&path, 0).unwrap();
    let mut out = vec![0f32; 4];
    assert!(src.read_window(W - 1, 0, 2, 2, &mut out).is_err());
    assert!(src.read_window(0, H, 1, 1, &mut out).is_err());
    assert!(src.read_window(0, 0, 3, 3, &mut out).is_err(), "buffer too small");
    // Junk and a TIFF that isn't georeferenced are refused without a panic.
    let dir = std::env::temp_dir().join(format!("lm-tiff-source-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let junk = dir.join("junk.tif");
    std::fs::write(&junk, b"II*\0\x08\0\0\0\xff\xff junk").unwrap();
    assert!(TiffSource::open(&junk, 0).is_err());
    let plain = dir.join("plain.tif");
    let mut buf = std::io::Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut buf).unwrap().write_image::<tiff::encoder::colortype::Gray8>(2, 2, &[1, 2, 3, 4]).unwrap();
    std::fs::write(&plain, buf.into_inner()).unwrap();
    assert!(matches!(TiffSource::open(&plain, 0), Err(ZonalError::Input(_))));
    // Truncated or corrupted pixel data gives errors (or wrong numbers), never a panic, in every
    // layout.
    for l in LAYOUTS {
        let bytes = std::fs::read(fixture_path(&format!("N7/layouts/{}", l.file))).unwrap();
        for (k, cut) in [bytes.len() * 3 / 4, bytes.len() / 2].into_iter().enumerate() {
            let p = dir.join(format!("cut{k}_{}", l.file));
            std::fs::write(&p, &bytes[..cut]).unwrap();
            // (The last band: a planar file keeps it at the end.)
            if let Ok(mut src) = TiffSource::open(&p, l.bands - 1) {
                let mut out = vec![0f32; (W * H) as usize];
                assert!(src.read_window(0, 0, W, H, &mut out).is_err(), "{}: cut at {cut} read whole", l.file);
            }
        }
        let mut flipped = bytes.clone();
        let n = flipped.len();
        for i in (n / 2..n).step_by(97) {
            flipped[i] ^= 0x5a;
        }
        let p = dir.join(format!("flip_{}", l.file));
        std::fs::write(&p, &flipped).unwrap();
        if let Ok(mut src) = TiffSource::open(&p, 0) {
            let mut out = vec![0f32; (W * H) as usize];
            let _ = src.read_window(0, 0, W, H, &mut out);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_summary_from_the_file_equals_one_from_memory() {
    let path = fixture_path("R3/landcover.tif");
    let text = std::fs::read_to_string(fixture_path("V1/districts.geojson")).unwrap();
    let (ds, _) = rustgis_data::io::read_geojson(&text, "areas").unwrap();
    let geoms: Vec<_> = ds.features.iter().filter_map(|f| f.geometry.clone()).collect();
    let r = rustgis_data::raster::read_path(&path).unwrap();
    for role in [ValueRole::Measured, ValueRole::Category] {
        let mem = summarize(&mut MemoryBand::new(&r, 0).unwrap(), &geoms, role, &Control::none()).unwrap();
        let file = summarize(&mut TiffSource::open(&path, 0).unwrap(), &geoms, role, &Control::none()).unwrap();
        assert_eq!(mem, file);
    }
}
