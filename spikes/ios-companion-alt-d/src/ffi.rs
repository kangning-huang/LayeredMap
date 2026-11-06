//! C ABI used by the SwiftUI spike. All `unsafe` stays in this module.

use crate::decode_geojson_text;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::slice;

pub const LM_DECODE_OK: i32 = 0;
pub const LM_DECODE_EMPTY: i32 = 1;
pub const LM_DECODE_INVALID_UTF8: i32 = 2;
pub const LM_DECODE_PARSE: i32 = 3;
pub const LM_DECODE_IO: i32 = 4;
pub const LM_DECODE_NULL_ARG: i32 = 5;

fn set_out_string(out: *mut *mut c_char, s: &str) -> Result<(), i32> {
    if out.is_null() {
        return Err(LM_DECODE_NULL_ARG);
    }
    let c = CString::new(s).map_err(|_| LM_DECODE_PARSE)?;
    // SAFETY: out is non-null; we hand ownership of the CString buffer to the caller.
    unsafe {
        *out = c.into_raw();
    }
    Ok(())
}

fn set_feature_count(out: *mut usize, n: usize) {
    if !out.is_null() {
        // SAFETY: caller-provided optional out pointer; null means ignore.
        unsafe {
            *out = n;
        }
    }
}

fn clear_out(out: *mut *mut c_char) {
    if !out.is_null() {
        // SAFETY: initialize out to null on error paths when pointer is provided.
        unsafe {
            *out = ptr::null_mut();
        }
    }
}

/// Decode GeoJSON bytes → FeatureCollection JSON C string.
///
/// # Safety
/// `bytes` must be valid for `len` reads when non-null. `out_json` must be writable when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lm_decode_geojson(bytes: *const u8, len: usize, out_json: *mut *mut c_char, out_feature_count: *mut usize) -> i32 {
    if out_json.is_null() {
        return LM_DECODE_NULL_ARG;
    }
    clear_out(out_json);
    set_feature_count(out_feature_count, 0);

    if bytes.is_null() || len == 0 {
        let _ = set_out_string(out_json, "empty input");
        return LM_DECODE_EMPTY;
    }

    // SAFETY: caller guarantees bytes is valid for len.
    let slice = unsafe { slice::from_raw_parts(bytes, len) };
    let text = match std::str::from_utf8(slice) {
        Ok(t) => t,
        Err(_) => {
            let _ = set_out_string(out_json, "input is not UTF-8");
            return LM_DECODE_INVALID_UTF8;
        }
    };

    match decode_geojson_text(text, "url") {
        Ok((json, n)) => {
            set_feature_count(out_feature_count, n);
            match set_out_string(out_json, &json) {
                Ok(()) => LM_DECODE_OK,
                Err(code) => code,
            }
        }
        Err(msg) => {
            let _ = set_out_string(out_json, &msg);
            LM_DECODE_PARSE
        }
    }
}

/// Decode GeoJSON from a filesystem path.
///
/// # Safety
/// `path` must be a valid NUL-terminated C string when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lm_decode_geojson_path(path: *const c_char, out_json: *mut *mut c_char, out_feature_count: *mut usize) -> i32 {
    if out_json.is_null() || path.is_null() {
        return LM_DECODE_NULL_ARG;
    }
    clear_out(out_json);
    set_feature_count(out_feature_count, 0);

    // SAFETY: path is a non-null C string from the caller.
    let c_path = unsafe { CStr::from_ptr(path) };
    let path_str = match c_path.to_str() {
        Ok(s) => s,
        Err(_) => {
            let _ = set_out_string(out_json, "path is not UTF-8");
            return LM_DECODE_INVALID_UTF8;
        }
    };

    let text = match std::fs::read_to_string(path_str) {
        Ok(t) => t,
        Err(e) => {
            let _ = set_out_string(out_json, &e.to_string());
            return LM_DECODE_IO;
        }
    };

    if text.is_empty() {
        let _ = set_out_string(out_json, "empty file");
        return LM_DECODE_EMPTY;
    }

    match decode_geojson_text(&text, path_str) {
        Ok((json, n)) => {
            set_feature_count(out_feature_count, n);
            match set_out_string(out_json, &json) {
                Ok(()) => LM_DECODE_OK,
                Err(code) => code,
            }
        }
        Err(msg) => {
            let _ = set_out_string(out_json, &msg);
            LM_DECODE_PARSE
        }
    }
}

/// Open any supported vector file by path; on success `out_json` is the envelope of
/// [`open_path_envelope`], on error a plain message.
///
/// # Safety
/// `path` must be a valid NUL-terminated C string when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lm_open_path(path: *const c_char, out_json: *mut *mut c_char, out_feature_count: *mut usize) -> i32 {
    // SAFETY: forwarded as given.
    unsafe { lm_open_path_at(path, 0, 0.0, 0.0, 0, out_json, out_feature_count) }
}

/// [`lm_open_path`] for data that may not be in longitude/latitude: `has_near` != 0 gives where to
/// guess its grid near (`near_lon`, `near_lat`: the map's centre or the person's location), and
/// `epsg` != 0 is a grid the person chose. See [`crate::placement`].
///
/// # Safety
/// `path` must be a valid NUL-terminated C string when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lm_open_path_at(
    path: *const c_char,
    has_near: i32,
    near_lon: f64,
    near_lat: f64,
    epsg: u32,
    out_json: *mut *mut c_char,
    out_feature_count: *mut usize,
) -> i32 {
    if out_json.is_null() || path.is_null() {
        return LM_DECODE_NULL_ARG;
    }
    clear_out(out_json);
    set_feature_count(out_feature_count, 0);

    // SAFETY: path is a non-null C string from the caller.
    let c_path = unsafe { CStr::from_ptr(path) };
    let Ok(path_str) = c_path.to_str() else {
        let _ = set_out_string(out_json, "path is not UTF-8");
        return LM_DECODE_INVALID_UTF8;
    };

    let near = (has_near != 0).then_some((near_lon, near_lat));
    let chosen = u16::try_from(epsg).ok().filter(|c| *c != 0);
    match crate::open_path_envelope_at(path_str, near, chosen) {
        Ok((json, n)) => {
            set_feature_count(out_feature_count, n);
            match set_out_string(out_json, &json) {
                Ok(()) => LM_DECODE_OK,
                Err(code) => code,
            }
        }
        Err(msg) => {
            let _ = set_out_string(out_json, &msg);
            LM_DECODE_PARSE
        }
    }
}

/// Every layer in a file (one per layer of a `.zip`): `out_json` is a JSON array of the
/// envelopes of [`crate::open_path_layers`]; arguments as [`lm_open_path_at`]. With `out_dir`
/// (may be NULL), each layer is written there as files and its envelope names them.
///
/// # Safety
/// `path` must be a valid NUL-terminated C string when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lm_open_path_all(
    path: *const c_char,
    has_near: i32,
    near_lon: f64,
    near_lat: f64,
    epsg: u32,
    out_dir: *const c_char,
    out_json: *mut *mut c_char,
    out_feature_count: *mut usize,
) -> i32 {
    if out_json.is_null() || path.is_null() {
        return LM_DECODE_NULL_ARG;
    }
    clear_out(out_json);
    set_feature_count(out_feature_count, 0);
    // SAFETY: path is a non-null C string from the caller.
    let c_path = unsafe { CStr::from_ptr(path) };
    let Ok(path_str) = c_path.to_str() else {
        let _ = set_out_string(out_json, "path is not UTF-8");
        return LM_DECODE_INVALID_UTF8;
    };
    let near = (has_near != 0).then_some((near_lon, near_lat));
    let chosen = u16::try_from(epsg).ok().filter(|c| *c != 0);
    // SAFETY: out_dir is NULL or a valid C string from the caller.
    let dir = (!out_dir.is_null()).then(|| unsafe { CStr::from_ptr(out_dir) }.to_str().ok()).flatten().map(std::path::Path::new);
    match crate::open_path_layers(path_str, near, chosen, dir) {
        Ok((json, n)) => {
            set_feature_count(out_feature_count, n);
            match set_out_string(out_json, &json) {
                Ok(()) => LM_DECODE_OK,
                Err(code) => code,
            }
        }
        Err(msg) => {
            let _ = set_out_string(out_json, &msg);
            LM_DECODE_PARSE
        }
    }
}

/// Open a raster (GeoTIFF, ASCII grid) and write its picture next to `out_stem` (see
/// [`crate::raster::open_raster_envelope`]).
///
/// # Safety
/// `path` and `out_stem` must be valid NUL-terminated C strings when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lm_open_raster(path: *const c_char, out_stem: *const c_char, out_json: *mut *mut c_char) -> i32 {
    if out_json.is_null() || path.is_null() || out_stem.is_null() {
        return LM_DECODE_NULL_ARG;
    }
    clear_out(out_json);
    // SAFETY: both are non-null C strings from the caller.
    let (p, o) = unsafe { (CStr::from_ptr(path), CStr::from_ptr(out_stem)) };
    let (Ok(p), Ok(o)) = (p.to_str(), o.to_str()) else {
        let _ = set_out_string(out_json, "path is not UTF-8");
        return LM_DECODE_INVALID_UTF8;
    };
    let result =
        std::panic::catch_unwind(|| crate::raster::open_raster_envelope(p, o)).unwrap_or_else(|_| Err("couldn't read this raster".into()));
    match result {
        Ok(json) => match set_out_string(out_json, &json) {
            Ok(()) => LM_DECODE_OK,
            Err(code) => code,
        },
        Err(msg) => {
            let _ = set_out_string(out_json, &msg);
            LM_DECODE_PARSE
        }
    }
}

/// Re-read a NetCDF grid at another time step, keeping the colour range so the ramp means the
/// same thing at every step. Pass `low >= high` to let it work the range out itself.
///
/// # Safety
/// `path` and `out_stem` must be non-null C strings; `out_json` must be a valid writable pointer.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn lm_open_raster_step(
    path: *const c_char,
    out_stem: *const c_char,
    time: i32,
    low: f64,
    high: f64,
    ramp: *const c_char,
    west: f64,
    south: f64,
    east: f64,
    north: f64,
    out_json: *mut *mut c_char,
) -> i32 {
    if out_json.is_null() || path.is_null() || out_stem.is_null() {
        return LM_DECODE_NULL_ARG;
    }
    clear_out(out_json);
    // SAFETY: both are non-null C strings from the caller.
    let (p, o) = unsafe { (CStr::from_ptr(path), CStr::from_ptr(out_stem)) };
    let (Ok(p), Ok(o)) = (p.to_str(), o.to_str()) else {
        let _ = set_out_string(out_json, "path is not UTF-8");
        return LM_DECODE_INVALID_UTF8;
    };
    let range = (high > low && low.is_finite() && high.is_finite()).then_some((low, high));
    let step = time.max(0) as usize;
    // SAFETY: a non-null C string from the caller, or absent for the default ramp.
    let ramp = if ramp.is_null() { "viridis" } else { unsafe { CStr::from_ptr(ramp) }.to_str().unwrap_or("viridis") };
    // A window of zero or nonsense size means "the whole thing".
    let window =
        (east > west && north > south && [west, south, east, north].iter().all(|v| v.is_finite())).then_some((west, south, east, north));
    let result = std::panic::catch_unwind(|| crate::raster::open_raster_envelope_at(p, o, step, range, ramp, window))
        .unwrap_or_else(|_| Err("couldn't read this raster".into()));
    match result {
        Ok(json) => match set_out_string(out_json, &json) {
            Ok(()) => LM_DECODE_OK,
            Err(code) => code,
        },
        Err(msg) => {
            let _ = set_out_string(out_json, &msg);
            LM_DECODE_PARSE
        }
    }
}

/// Free a string returned by this library.
///
/// # Safety
/// `s` must be null or a pointer previously returned by this crate's APIs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lm_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    // SAFETY: s was created with CString::into_raw in this crate.
    unsafe {
        drop(CString::from_raw(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn ffi_roundtrip_point() {
        let text = br#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[2.0,1.0]}}
        ]}"#;
        let mut out: *mut c_char = ptr::null_mut();
        let mut n: usize = 0;
        // SAFETY: test-owned buffer and out pointers.
        let status = unsafe { lm_decode_geojson(text.as_ptr(), text.len(), &mut out, &mut n) };
        assert_eq!(status, LM_DECODE_OK);
        assert_eq!(n, 1);
        assert!(!out.is_null());
        // SAFETY: out is a CString from the API.
        let s = unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned();
        assert!(s.contains("FeatureCollection"));
        unsafe { lm_string_free(out) };
    }
}
