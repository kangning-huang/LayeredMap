#![no_main]
//! The input is a whole `f.kml`, opened through the import pipeline (KML adapter).

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.kml", data)]);
});
