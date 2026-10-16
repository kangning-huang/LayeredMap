#![no_main]
//! The input is the `.dbf` beside V3's valid .shp and .shx, with no .cpg, so the attribute
//! reader and the encoding detector see it.

use lm_fuzz::{V3_SHP, V3_SHX, import};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    import(&[("f.shp", V3_SHP), ("f.shx", V3_SHX), ("f.dbf", data)]);
});
