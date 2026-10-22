//! The files the importers read: the disk, plus files held in memory.
//!
//! The browser build (apps/nimblemap-web) has no file system: a file the user drops or picks, or
//! an example fetched from the site, is [`put`] here under a path such as `/dropped/roads.shp`,
//! and opened with the usual `Command::Open(path)`. The importers ask this module instead of
//! `std::fs`, so the same pipeline (detection, profiling, questions) runs on both. On the desktop
//! nothing is ever put here, and every call goes straight to `std::fs`.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

fn files() -> &'static Mutex<BTreeMap<PathBuf, Arc<[u8]>>> {
    static FILES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<[u8]>>>> = OnceLock::new();
    FILES.get_or_init(Default::default)
}

fn held(path: &Path) -> Option<Arc<[u8]>> {
    files().lock().unwrap_or_else(PoisonError::into_inner).get(path).cloned()
}

/// Holds `bytes` as the file at `path` (replacing what was there).
pub fn put(path: impl Into<PathBuf>, bytes: Vec<u8>) {
    files().lock().unwrap_or_else(PoisonError::into_inner).insert(path.into(), Arc::from(bytes));
}

/// Forgets a file held in memory.
pub fn remove(path: &Path) {
    files().lock().unwrap_or_else(PoisonError::into_inner).remove(path);
}

/// Whether `path` is held in memory.
pub fn is_held(path: &Path) -> bool {
    files().lock().unwrap_or_else(PoisonError::into_inner).contains_key(path)
}

/// A file's size and kind.
#[derive(Debug)]
pub enum Metadata {
    Disk(std::fs::Metadata),
    Memory(u64),
}

impl Metadata {
    pub fn len(&self) -> u64 {
        match self {
            Metadata::Disk(m) => m.len(),
            Metadata::Memory(n) => *n,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn is_dir(&self) -> bool {
        matches!(self, Metadata::Disk(m) if m.is_dir())
    }
    pub fn is_file(&self) -> bool {
        match self {
            Metadata::Disk(m) => m.is_file(),
            Metadata::Memory(_) => true,
        }
    }
    /// When the file was last changed (unknown for a file in memory).
    pub fn modified(&self) -> std::io::Result<std::time::SystemTime> {
        match self {
            Metadata::Disk(m) => m.modified(),
            Metadata::Memory(_) => Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "a file in memory")),
        }
    }
}

pub fn metadata(path: &Path) -> std::io::Result<Metadata> {
    match held(path) {
        Some(b) => Ok(Metadata::Memory(b.len() as u64)),
        None => std::fs::metadata(path).map(Metadata::Disk),
    }
}

/// An open file: on disk or in memory.
#[derive(Debug)]
pub enum File {
    Disk(std::fs::File),
    Memory(std::io::Cursor<Arc<[u8]>>),
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            File::Disk(f) => f.read(buf),
            File::Memory(c) => c.read(buf),
        }
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            File::Disk(f) => f.seek(pos),
            File::Memory(c) => c.seek(pos),
        }
    }
}

pub fn open(path: &Path) -> std::io::Result<File> {
    match held(path) {
        Some(b) => Ok(File::Memory(std::io::Cursor::new(b))),
        None => std::fs::File::open(path).map(File::Disk),
    }
}

pub fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    match held(path) {
        Some(b) => Ok(b.to_vec()),
        None => std::fs::read(path),
    }
}

/// The bytes of a file held in memory, without copying (None for a file on disk).
pub fn shared(path: &Path) -> Option<Arc<[u8]>> {
    held(path)
}

/// The files in a folder: those held in memory under it, or the folder's entries on disk.
pub fn read_dir(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mem: Vec<PathBuf> =
        files().lock().unwrap_or_else(PoisonError::into_inner).keys().filter(|p| p.parent() == Some(dir)).cloned().collect();
    if !mem.is_empty() {
        return Ok(mem);
    }
    Ok(std::fs::read_dir(dir)?.flatten().map(|e| e.path()).collect())
}

pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    if is_held(path) { Ok(path.to_path_buf()) } else { std::fs::canonicalize(path) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_files_read_like_disk_files() {
        let p = PathBuf::from("/lm-vfs-test/a.csv");
        put(&p, b"x,y\n1,2\n".to_vec());
        assert_eq!(metadata(&p).unwrap().len(), 8);
        assert!(metadata(&p).unwrap().is_file());
        let mut s = String::new();
        let mut f = open(&p).unwrap();
        f.seek(SeekFrom::Start(4)).unwrap();
        f.read_to_string(&mut s).unwrap();
        assert_eq!(s, "1,2\n");
        assert_eq!(read_dir(Path::new("/lm-vfs-test")).unwrap(), vec![p.clone()]);
        remove(&p);
        assert!(metadata(&p).is_err());
    }
}
