#![no_main]
//! The input is a whole `f.gpkg` (a SQLite database), opened through the import pipeline: the
//! GeoPackage adapter's table listing, geometry blobs and attribute reader.

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.gpkg", data)]);
});
