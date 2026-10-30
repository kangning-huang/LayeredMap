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
