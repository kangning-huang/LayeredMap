#![no_main]
//! The input is a whole `f.csv`, opened through the import pipeline.

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.csv", data)]);
});
