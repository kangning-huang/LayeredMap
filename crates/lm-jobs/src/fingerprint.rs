//! Source fingerprints for recipes (WP4.6): what a file was when a result was computed, so a
//! changed source marks the result "Source changed". The fingerprint is the file's length and a
//! hash of its first and last mebibyte: cheap for a 2 GB raster, and it changes when the content
//! does in practice. The hash is FNV-1a 64, written out here so fingerprints saved in documents
//! stay comparable across Rust releases (the standard hasher makes no such promise).

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const CHUNK: u64 = 1 << 20;

fn fnv1a(mut h: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// "len:hash" for a file, or None if it can't be read. A folder (a Shapefile set, a zipped
/// GeoPackage unpacked) is fingerprinted by its files' names and fingerprints, in order.
pub fn of_path(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.is_dir() {
        let mut names: Vec<_> = std::fs::read_dir(path).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        names.sort();
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for p in names {
            let name = p.file_name()?.to_string_lossy().to_string();
            if name.starts_with("._") {
                continue;
            }
            h = fnv1a(h, name.as_bytes());
            h = fnv1a(h, of_path(&p).unwrap_or_default().as_bytes());
        }
        return Some(format!("dir:{h:016x}"));
    }
    let len = meta.len();
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; CHUNK.min(len) as usize];
    f.read_exact(&mut buf).ok()?;
    let mut h = fnv1a(0xcbf2_9ce4_8422_2325, &buf);
    if len > CHUNK {
        let tail = CHUNK.min(len - CHUNK);
        f.seek(SeekFrom::End(-(tail as i64))).ok()?;
        let mut t = vec![0u8; tail as usize];
        f.read_exact(&mut t).ok()?;
        h = fnv1a(h, &t);
    }
    Some(format!("{len}:{h:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_same_fingerprint_and_a_change_shows() {
        let dir = std::env::temp_dir().join(format!("lm-fp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.csv");
        std::fs::write(&a, b"name,v\nx,1\n").unwrap();
        let f1 = of_path(&a).unwrap();
        assert_eq!(of_path(&a).unwrap(), f1);
        std::fs::write(&a, b"name,v\nx,2\n").unwrap();
        assert_ne!(of_path(&a).unwrap(), f1);
        assert!(of_path(&dir.join("missing.csv")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The hash is fixed: a document saved today compares equal after a toolchain update.
    #[test]
    fn the_hash_is_stable() {
        assert_eq!(fnv1a(0xcbf2_9ce4_8422_2325, b"LayeredMap"), 0x46ed_6640_72bf_f677);
    }
}
