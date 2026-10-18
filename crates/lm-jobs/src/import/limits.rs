//! Safety limits (WP2.8). Archives are checked from their central directory before anything is
//! expanded; sizes are checked before anything is read.

use super::ImportError;

/// Most bytes an archive may expand to.
pub const ZIP_MAX_EXPANDED: u64 = 2 << 30;
/// Highest compression ratio allowed, per entry and for the whole archive.
pub const ZIP_MAX_RATIO: u64 = 100;
/// Most entries in an archive.
pub const ZIP_MAX_ENTRIES: usize = 10_000;
/// A GeoJSON larger than this is refused with a suggestion to convert it.
pub const GEOJSON_MAX_BYTES: u64 = 2 << 30;
/// A GeoPackage larger than this is refused: it is read into memory to be opened without SQLite.
pub const GEOPACKAGE_MAX_BYTES: u64 = 2 << 30;
/// A workbook larger than this is refused: the spreadsheet reader holds it in memory.
pub const WORKBOOK_MAX_BYTES: u64 = 1 << 30;
/// A KML or GPX larger than this is refused: the XML is parsed into memory whole.
pub const XML_MAX_BYTES: u64 = 256 << 20;
/// Most XML nodes (elements, text, attributes aside) read from one KML or GPX.
pub const XML_MAX_NODES: u32 = 20_000_000;
/// A raster side longer than this, without overviews, goes through the preparation step.
pub const RASTER_PREPARE_SIDE: u64 = 8192;
/// A raster side longer than this is refused.
pub const RASTER_MAX_SIDE: u64 = 200_000;

/// One entry of an archive's central directory.
#[derive(Clone, Debug, PartialEq)]
pub struct ZipEntry {
    pub name: String,
    pub compressed: u64,
    pub expanded: u64,
}

fn u16_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?)))
}

fn u32_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?)))
}

/// Lists an archive's entries from its central directory, without expanding anything.
pub fn zip_entries(bytes: &[u8]) -> Result<Vec<ZipEntry>, ImportError> {
    let damaged = || ImportError::Refused("This archive is damaged or isn't a zip file. Your file has not been changed.".into());
    // The end-of-central-directory record sits in the last 64 KiB + 22 bytes.
    let from = bytes.len().saturating_sub(22 + 65_535);
    let eocd = (from..bytes.len().saturating_sub(21)).rev().find(|&i| bytes.get(i..i + 4) == Some(b"PK\x05\x06")).ok_or_else(damaged)?;
    let count = u16_at(bytes, eocd + 10).ok_or_else(damaged)?;
    let cd_size = u32_at(bytes, eocd + 12).ok_or_else(damaged)?;
    let cd_off = u32_at(bytes, eocd + 16).ok_or_else(damaged)?;
    if count == 0xFFFF || cd_off == 0xFFFF_FFFF || cd_size == 0xFFFF_FFFF {
        return Err(ImportError::Refused("This archive uses the Zip64 format, which LayeredMap can't open yet.".into()));
    }
    if count as usize > ZIP_MAX_ENTRIES {
        return Err(too_many(count as usize));
    }
    let mut at = usize::try_from(cd_off).map_err(|_| damaged())?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if bytes.get(at..at + 4) != Some(b"PK\x01\x02") {
            return Err(damaged());
        }
        let compressed = u32_at(bytes, at + 20).ok_or_else(damaged)?;
        let expanded = u32_at(bytes, at + 24).ok_or_else(damaged)?;
        let name_len = u16_at(bytes, at + 28).ok_or_else(damaged)? as usize;
        let extra_len = u16_at(bytes, at + 30).ok_or_else(damaged)? as usize;
        let comment_len = u16_at(bytes, at + 32).ok_or_else(damaged)? as usize;
        let name = String::from_utf8_lossy(bytes.get(at + 46..at + 46 + name_len).ok_or_else(damaged)?).into_owned();
        out.push(ZipEntry { name, compressed, expanded });
        at += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}

fn too_many(n: usize) -> ImportError {
    ImportError::Refused(format!(
        "This archive holds {} files, more than the {} LayeredMap opens. Your file has not been changed.",
        lm_core::model::group_thousands(n as u64),
        lm_core::model::group_thousands(ZIP_MAX_ENTRIES as u64)
    ))
}

/// Checks an archive against the limits: entry count, expanded size, compression ratio, and no
/// entry that would land outside the archive (absolute paths, `..`). Nested archives are not
/// opened, so they can't multiply.
pub fn check_zip(entries: &[ZipEntry]) -> Result<(), ImportError> {
    if entries.len() > ZIP_MAX_ENTRIES {
        return Err(too_many(entries.len()));
    }
    let nested = entries.iter().filter(|e| {
        let n = e.name.to_ascii_lowercase();
        [".zip", ".gz", ".tgz", ".7z", ".rar", ".bz2", ".xz", ".tar"].iter().any(|x| n.ends_with(x))
    });
    if nested.count() > 0 {
        return Err(ImportError::Refused(
            "This archive has other archives inside it. LayeredMap doesn't open archives inside archives, because they can expand without limit; unzip it first. Your file has not been changed."
                .into(),
        ));
    }
    for e in entries {
        let n = e.name.replace('\\', "/");
        if n.starts_with('/') || n.split('/').any(|p| p == "..") || n.get(1..2) == Some(":") {
            return Err(ImportError::Refused(format!(
                "This archive contains a file that points outside the archive (\"{}\"), so it was not opened. Your file has not been changed.",
                e.name
            )));
        }
        if e.expanded > e.compressed.saturating_mul(ZIP_MAX_RATIO).max(1 << 20) {
            return Err(bomb());
        }
    }
    let expanded: u64 = entries.iter().map(|e| e.expanded).sum();
    let compressed: u64 = entries.iter().map(|e| e.compressed).sum();
    if expanded > ZIP_MAX_EXPANDED || expanded > compressed.saturating_mul(ZIP_MAX_RATIO).max(1 << 20) {
        return Err(bomb());
    }
    Ok(())
}

fn bomb() -> ImportError {
    ImportError::Refused(
        "This archive would expand to far more than its size (over 100 times, or more than 2 GB), so it was not opened. Your file has not been changed."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, c: u64, x: u64) -> ZipEntry {
        ZipEntry { name: name.into(), compressed: c, expanded: x }
    }

    #[test]
    fn traversal_and_absolute_paths_are_refused() {
        for bad in ["../evil.shp", "a/../../b.csv", "/etc/passwd", "C:/x.csv", "a\\..\\b"] {
            assert!(check_zip(&[e(bad, 10, 10)]).is_err(), "{bad}");
        }
        assert!(check_zip(&[e("data/x.shp", 10, 10), e("..x/ok.csv", 1, 1)]).is_ok());
        assert!(check_zip(&[e("inner.zip", 10, 10)]).is_err(), "archives inside archives");
    }

    #[test]
    fn compression_ratio_and_size_are_limited() {
        assert!(check_zip(&[e("a.csv", 1 << 20, 50 << 20)]).is_ok());
        assert!(check_zip(&[e("a.csv", 1 << 20, 200 << 20)]).is_err());
        // Small files may compress very well (a 1 MiB floor).
        assert!(check_zip(&[e("a.csv", 100, 900_000)]).is_ok());
        let many: Vec<ZipEntry> = (0..30).map(|i| e(&format!("{i}.csv"), 1 << 30, 90 << 20)).collect();
        assert!(check_zip(&many).is_err(), "over 2 GB in total");
    }

    #[test]
    fn the_generated_bomb_and_traversal_fixtures_are_refused() {
        let dir = std::env::var_os("LM_FIXTURES_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/generated"));
        for (f, word) in [("X1/x1_bomb.zip", "archives inside"), ("X2/x2_traversal.zip", "outside")] {
            let Ok(bytes) = std::fs::read(dir.join(f)) else { continue };
            let t = std::time::Instant::now();
            let err = zip_entries(&bytes).and_then(|es| check_zip(&es)).unwrap_err();
            assert!(err.to_string().contains(word), "{f}: {err}");
            assert!(t.elapsed().as_secs_f64() < 1.0, "X1 must be rejected in under 1 s");
        }
    }
}
