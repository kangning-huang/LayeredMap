#![no_main]
//! The input is a whole `f.xlsx` (a ZIP of XML parts), opened through the import pipeline: the
//! archive limits, then the Excel adapter's sheet and cell reader.

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.xlsx", data)]);
});
