#![no_main]
//! The input is a whole `f.gpx`, opened through the import pipeline (GPX adapter).

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.gpx", data)]);
});
