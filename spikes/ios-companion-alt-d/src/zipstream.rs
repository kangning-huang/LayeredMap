//! A zip read from its file, a member at a time, straight to disk.
//!
//! `rustgis_data::zip` takes the whole archive as bytes and inflates a member into memory: for
//! Geofabrik's Taiwan extract that was 263 MB of archive plus each layer's files held at once,
//! and iOS ended the app inside the reader. Here only the central directory is read into
//! memory; a member is inflated in 256 KB pieces into its own file.

use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZFlush, MZStatus};
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// The most one member may expand to (an archive bomb is refused, not unpacked).
const MAX_MEMBER: u64 = 2 << 30;
const CHUNK: usize = 256 << 10;

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    method: u16,
    compressed: u64,
    expanded: u64,
    local_offset: u64,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}

fn bad() -> String {
    "not a zip archive (or a damaged one)".to_string()
}

/// The archive's entries, from its central directory.
pub fn entries(file: &mut File) -> Result<Vec<Entry>, String> {
    let len = file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let tail_len = len.min(65_557);
    file.seek(SeekFrom::Start(len - tail_len)).map_err(|e| e.to_string())?;
    let mut tail = vec![0u8; tail_len as usize];
    file.read_exact(&mut tail).map_err(|e| e.to_string())?;
    let eocd = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| tail.get(i..i + 4) == Some(&[0x50, 0x4B, 0x05, 0x06]))
        .ok_or_else(bad)?;
    let count = u16_at(&tail, eocd + 10).ok_or_else(bad)?;
    let cd_size = u32_at(&tail, eocd + 12).ok_or_else(bad)?;
    let cd_offset = u32_at(&tail, eocd + 16).ok_or_else(bad)?;
    if count == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF {
        return Err("this archive is larger than 4 GB (ZIP64), which the iPhone can't read; unzip it on a computer first".into());
    }
    file.seek(SeekFrom::Start(u64::from(cd_offset))).map_err(|e| e.to_string())?;
    let mut cd = vec![0u8; cd_size as usize];
    file.read_exact(&mut cd).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(count as usize);
    let mut p = 0usize;
    for _ in 0..count {
        if u32_at(&cd, p) != Some(0x0201_4B50) {
            return Err(bad());
        }
        let method = u16_at(&cd, p + 10).ok_or_else(bad)?;
        let compressed = u64::from(u32_at(&cd, p + 20).ok_or_else(bad)?);
        let expanded = u64::from(u32_at(&cd, p + 24).ok_or_else(bad)?);
        let name_len = usize::from(u16_at(&cd, p + 28).ok_or_else(bad)?);
        let extra_len = usize::from(u16_at(&cd, p + 30).ok_or_else(bad)?);
        let comment_len = usize::from(u16_at(&cd, p + 32).ok_or_else(bad)?);
        let local_offset = u64::from(u32_at(&cd, p + 42).ok_or_else(bad)?);
        let name_bytes = cd.get(p + 46..p + 46 + name_len).ok_or_else(bad)?;
        // UTF-8 when flagged; otherwise CP437/locale bytes, read leniently.
        let name = String::from_utf8_lossy(name_bytes).replace('\\', "/");
        out.push(Entry { name, method, compressed, expanded, local_offset });
        p += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}

/// Inflates `e` from the archive into `dest`.
pub fn extract(file: &mut File, e: &Entry, dest: &Path) -> Result<(), String> {
    if e.expanded > MAX_MEMBER || (e.compressed > 0 && e.expanded / e.compressed.max(1) > 1000 && e.expanded > (64 << 20)) {
        return Err(format!("{} would expand to far more than its size, so it was not opened", e.name));
    }
    file.seek(SeekFrom::Start(e.local_offset)).map_err(|err| err.to_string())?;
    let mut head = [0u8; 30];
    file.read_exact(&mut head).map_err(|err| err.to_string())?;
    if u32_at(&head, 0) != Some(0x0403_4B50) {
        return Err(bad());
    }
    let skip = u64::from(u16_at(&head, 26).ok_or_else(bad)?) + u64::from(u16_at(&head, 28).ok_or_else(bad)?);
    file.seek(SeekFrom::Current(skip as i64)).map_err(|err| err.to_string())?;
    let mut out = BufWriter::new(File::create(dest).map_err(|err| err.to_string())?);
    let mut left = e.compressed;
    let mut input = vec![0u8; CHUNK];
    match e.method {
        0 => {
            while left > 0 {
                let n = left.min(CHUNK as u64) as usize;
                file.read_exact(&mut input[..n]).map_err(|err| err.to_string())?;
                out.write_all(&input[..n]).map_err(|err| err.to_string())?;
                left -= n as u64;
            }
        }
        8 => {
            let mut state = InflateState::new_boxed(DataFormat::Raw);
            let mut output = vec![0u8; CHUNK];
            let mut written = 0u64;
            let mut done = false;
            while !done {
                let n = left.min(CHUNK as u64) as usize;
                file.read_exact(&mut input[..n]).map_err(|err| err.to_string())?;
                left -= n as u64;
                let mut slice = &input[..n];
                loop {
                    // Never `Finish`: it reports a buffer error whenever the output fills first.
                    // The stream says when it has ended.
                    let r = inflate(&mut state, slice, &mut output, MZFlush::None);
                    out.write_all(&output[..r.bytes_written]).map_err(|err| err.to_string())?;
                    written += r.bytes_written as u64;
                    if written > MAX_MEMBER {
                        return Err(format!("{} would expand past 2 GB, so it was not opened", e.name));
                    }
                    slice = &slice[r.bytes_consumed..];
                    match r.status {
                        Ok(MZStatus::StreamEnd) => {
                            done = true;
                            break;
                        }
                        Ok(_) => {}
                        Err(_) if r.bytes_written > 0 || r.bytes_consumed > 0 => {}
                        Err(_) => return Err(format!("{} is damaged or cut short in the archive", e.name)),
                    }
                    if slice.is_empty() && r.bytes_written < output.len() {
                        break;
                    }
                }
                if left == 0 && !done {
                    return Err(format!("{} is damaged or cut short in the archive", e.name));
                }
            }
        }
        m => return Err(format!("{} is compressed in a way the iPhone can't read (method {m})", e.name)),
    }
    out.flush().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_rustgis_writes() {
        let dir = std::env::temp_dir().join(format!("lm-zipstream-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let big: Vec<u8> = (0..300_000u32).flat_map(|i| (i % 251).to_le_bytes()).collect();
        let zip = rustgis_data::zip::write(&[("a/one.txt".into(), b"hello".to_vec()), ("two.bin".into(), big.clone())]).expect("zip");
        let path = dir.join("t.zip");
        std::fs::write(&path, zip).expect("write");
        let mut f = File::open(&path).expect("open");
        let es = entries(&mut f).expect("entries");
        assert_eq!(es.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["a/one.txt", "two.bin"]);
        for e in &es {
            let out = dir.join(e.name.replace('/', "_"));
            extract(&mut f, e, &out).expect("extract");
        }
        assert_eq!(std::fs::read(dir.join("a_one.txt")).expect("read"), b"hello");
        assert_eq!(std::fs::read(dir.join("two.bin")).expect("read"), big);
    }
}
