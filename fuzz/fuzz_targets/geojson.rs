#![no_main]
//! The input is a whole `f.geojson`, opened through the import pipeline.

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.geojson", data)]);
});
