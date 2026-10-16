//! Source fingerprints: enough to tell later whether a source changed or went missing.
//! Sources are never modified and never copied into the document.

use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use lm_core::model::SourceRef;
use sha2::{Digest, Sha256};

use crate::error::{DocError, Result};

/// What became of a source since it was fingerprinted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceState {
    Unchanged,
    Changed,
    Missing,
}

/// Fingerprints a file: absolute path, size, modification time and, with `hash`, the SHA-256
/// of its bytes (read in chunks, never the whole file at once).
pub fn fingerprint(path: &Path, hash: bool) -> Result<SourceRef> {
    let meta = std::fs::metadata(path).map_err(|e| DocError::read(path, e))?;
    let sha256 = if hash { Some(sha256_file(path)?) } else { None };
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(SourceRef { path: abs.display().to_string(), member: None, size: meta.len(), mtime: mtime_of(&meta), sha256 })
}

/// Compares a source with its fingerprint. Same size and time: unchanged. A different time with
/// the same size and a recorded hash is decided by the hash (a file touched but not edited).
pub fn check_source(src: &SourceRef) -> SourceState {
    let path = Path::new(&src.path);
    let meta = match std::fs::metadata(path) {
        Ok(m) if m.is_file() => m,
        _ => return SourceState::Missing,
    };
    if meta.len() != src.size {
        return SourceState::Changed;
    }
    if mtime_of(&meta) == src.mtime {
        return SourceState::Unchanged;
    }
    match &src.sha256 {
        Some(want) => match sha256_file(path) {
            Ok(got) if got.eq_ignore_ascii_case(want) => SourceState::Unchanged,
            Ok(_) => SourceState::Changed,
            Err(_) => SourceState::Missing,
        },
        None => SourceState::Changed,
    }
}

/// Modification time in whole seconds since the Unix epoch (negative before it; 0 if unknown).
fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    match meta.modified() {
        Ok(t) => match t.duration_since(UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            Err(e) => i64::try_from(e.duration().as_secs()).map(|s| -s).unwrap_or(i64::MIN),
        },
        Err(_) => 0,
    }
}

/// Lower-case hex SHA-256 of a file, streamed in 64 KiB chunks.
fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path).map_err(|e| DocError::read(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(buf.get(..n).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(DocError::read(path, e)),
        }
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use std::time::{Duration, SystemTime};

    #[test]
    fn fingerprint_records_size_time_and_hash() {
        let t = TempDir::new("fp");
        let p = t.0.join("schools.csv");
        std::fs::write(&p, b"abc").unwrap();
        let s = fingerprint(&p, true).unwrap();
        assert_eq!(s.size, 3);
        assert!(s.mtime > 0);
        assert!(Path::new(&s.path).is_absolute());
        assert_eq!(s.file_name(), "schools.csv");
        assert_eq!(s.sha256.as_deref(), Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert_eq!(fingerprint(&p, false).unwrap().sha256, None);
        assert_eq!(check_source(&s), SourceState::Unchanged);
    }

    #[test]
    fn hash_streams_files_larger_than_one_chunk() {
        let t = TempDir::new("big");
        let p = t.0.join("big.bin");
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&p, &bytes).unwrap();
        let want: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(fingerprint(&p, true).unwrap().sha256, Some(want));
    }

    #[test]
    fn detects_changed_and_missing_files() {
        let t = TempDir::new("chg");
        let p = t.0.join("a.geojson");
        std::fs::write(&p, b"one").unwrap();
        let s = fingerprint(&p, false).unwrap();
        std::fs::write(&p, b"three").unwrap();
        assert_eq!(check_source(&s), SourceState::Changed);
        std::fs::remove_file(&p).unwrap();
        assert_eq!(check_source(&s), SourceState::Missing);
        std::fs::create_dir(&p).unwrap();
        assert_eq!(check_source(&s), SourceState::Missing, "a folder in its place is not the file");
    }

    #[test]
    fn a_touched_file_is_unchanged_only_when_the_hash_says_so() {
        let t = TempDir::new("touch");
        let p = t.0.join("a.csv");
        std::fs::write(&p, b"same").unwrap();
        let with = fingerprint(&p, true).unwrap();
        let without = fingerprint(&p, false).unwrap();
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
        drop(f);
        assert_eq!(check_source(&with), SourceState::Unchanged);
        assert_eq!(check_source(&without), SourceState::Changed);
        std::fs::write(&p, b"diff").unwrap();
        // Times have one-second resolution here: move the edit off the fingerprint's second.
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(7200)).unwrap();
        drop(f);
        assert_eq!(check_source(&with), SourceState::Changed, "same size, different bytes");
    }
}
