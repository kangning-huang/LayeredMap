#![no_main]
//! The input is a whole `f.kmz`: the archive checks, then the KML inside.

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.kmz", data)]);
});
