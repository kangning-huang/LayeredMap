#![no_main]
//! The first byte picks the companions, the rest is the `.shp`: 0 alone (no .shx, no .dbf: the
//! index is rebuilt, shapes only), 1 with V3's .shx and .dbf (a mismatched index), 2 with V3's
//! .dbf only, 3 as a `.zip` (the archive limits and the zipped-Shapefile reader).

use lm_fuzz::{V3_DBF, V3_SHX, import};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((mode, rest)) = data.split_first() else { return };
    match mode % 4 {
        0 => import(&[("f.shp", rest)]),
        1 => import(&[("f.shp", rest), ("f.shx", V3_SHX), ("f.dbf", V3_DBF)]),
        2 => import(&[("f.shp", rest), ("f.dbf", V3_DBF)]),
        _ => import(&[("f.zip", rest)]),
    }
});
