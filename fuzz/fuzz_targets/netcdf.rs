#![no_main]
//! The input is a whole `f.nc` (NetCDF-3 or NetCDF-4), opened through the import pipeline
//! (NetCDF adapter): the header, the coordinates, the time axis and the first slices.

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    lm_fuzz::import(&[("f.nc", data)]);
});
