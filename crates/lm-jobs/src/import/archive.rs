//! Data packed in an archive: a gzipped file (`.nc.gz`, `.csv.gz`) and a zip that holds files
//! other than a Shapefile (a GeoJSON with its metadata, a folder of CSVs, a GBIF download).
//!
//! A `.gz` is unpacked once into the cache (`Unpacked/<hash>-<name>`) and that file is imported:
//! a NetCDF grid reads its time steps from it later, so it must stay where it is.
//!
//! A zip's members are chosen by the rules the iPhone app follows too: folders, macOS forks and
//! files that aren't data (text, metadata, documents, images) are skipped; the same layer stored
//! twice (one name in two folders, one stem in two formats) is opened once; each member is read
//! with the normal importer. When some members have locations, plain tables are left out (named
//! in a notice). Vector members keep the zip as their source with the member's path as
//! `source.member`; rasters and grids are unpacked into the cache like a `.gz`, because they are
//! read again from their file.

use lm_core::brand::NAME as APP;
use std::path::{Path, PathBuf};

use lm_core::model::LayerKind;
use sha2::Digest;

use super::limits::{ZIP_MAX_EXPANDED, ZipEntry};
use super::{Ctx, ImportError, ImportedLayer, extension, file_name, import_path, source_ref};

/// Most layers opened from one zip.
pub const ZIP_MAX_LAYERS: usize = 30;

/// Where unpacked files are kept: the app cache's `Unpacked` folder.
pub fn unpacked_dir() -> PathBuf {
    // The browser has no folders (and no temporary folder to ask for): copies are held in memory.
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from("/unpacked");
    }
    match lm_core::platform::current_user_dir(lm_core::platform::Folder::Caches) {
        Some(d) => d.join("Unpacked"),
        None => std::env::temp_dir().join("layeredmap-unpacked"),
    }
}

/// A number no other write in this process uses (temporary names).
fn unique() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    #[cfg(not(target_arch = "wasm32"))]
    return format!("{}-{n}", std::process::id());
    #[cfg(target_arch = "wasm32")]
    return n.to_string();
}

/// The first 12 hex digits of the SHA-256 of `bytes`.
fn hash12(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes).iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// `bytes` kept as `<dir>/<hash>-<name>`, reusing a copy already there; written through a
/// temporary file so a half-written copy is never reused. When the folder can't be written (the
/// browser), the copy is held in memory under that path instead.
fn keep(dir: &Path, key: &str, name: &str, bytes: impl FnOnce() -> Result<Vec<u8>, ImportError>) -> Result<PathBuf, ImportError> {
    let target = dir.join(format!("{key}-{name}"));
    if crate::vfs::metadata(&target).is_ok_and(|m| m.is_file()) {
        return Ok(target);
    }
    let bytes = bytes()?;
    let tmp = dir.join(format!(".{key}-{}.part", unique()));
    let written = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&tmp, &bytes)).and_then(|()| std::fs::rename(&tmp, &target));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        log::warn!("could not keep the unpacked {name} in {}: {e}", dir.display());
        crate::vfs::put(&target, bytes);
    }
    Ok(target)
}

fn damaged(name: &str) -> ImportError {
    ImportError::Refused(format!("{name} is damaged or cut short — try downloading it again. Your file has not been changed."))
}

fn too_large(name: &str) -> ImportError {
    ImportError::Refused(format!(
        "{name} would unpack to more than 2 GB, the most {APP} opens from a compressed file. Your file has not been changed."
    ))
}

fn nested() -> ImportError {
    ImportError::Refused(format!(
        "This file is compressed more than once. {APP} doesn't open archives inside archives, because they can expand without limit; unpack it first. Your file has not been changed."
    ))
}

/// The contents of a gzip file (RFC 1952): its header (with the optional extra field, name,
/// comment and header checksum) skipped, the DEFLATE data inflated to at most `limit` bytes, and
/// the length in the trailer checked.
pub fn gunzip(bytes: &[u8], name: &str, limit: usize) -> Result<Vec<u8>, ImportError> {
    const FHCRC: u8 = 0x02;
    const FEXTRA: u8 = 0x04;
    const FNAME: u8 = 0x08;
    const FCOMMENT: u8 = 0x10;
    if bytes.len() < 18 || bytes[0] != 0x1f || bytes[1] != 0x8b {
        return Err(damaged(name));
    }
    if bytes[2] != 8 {
        return Err(ImportError::Unsupported(format!("{name} is compressed in a way {APP} can't unpack yet.")));
    }
    let flags = bytes[3];
    let mut at = 10usize;
    if flags & FEXTRA != 0 {
        let xlen = bytes.get(at..at + 2).map(|b| usize::from(u16::from_le_bytes([b[0], b[1]]))).ok_or_else(|| damaged(name))?;
        at += 2 + xlen;
    }
    for flag in [FNAME, FCOMMENT] {
        if flags & flag != 0 {
            let end = bytes.get(at..).and_then(|b| b.iter().position(|&c| c == 0)).ok_or_else(|| damaged(name))?;
            at += end + 1;
        }
    }
    if flags & FHCRC != 0 {
        at += 2;
    }
    let body = bytes.get(at..bytes.len().saturating_sub(8)).filter(|b| !b.is_empty()).ok_or_else(|| damaged(name))?;
    let out = miniz_oxide::inflate::decompress_to_vec_with_limit(body, limit).map_err(|e| match e.status {
        miniz_oxide::inflate::TINFLStatus::HasMoreOutput => too_large(name),
        _ => damaged(name),
    })?;
    let isize = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().map_err(|_| damaged(name))?);
    if isize != out.len() as u32 {
        return Err(damaged(name));
    }
    Ok(out)
}

/// The name a gzipped file had before it was compressed: `x.nc.gz` → `x.nc`, `x.tgz` → `x.tar`.
fn inner_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tgz") {
        format!("{}.tar", &name[..name.len() - 4])
    } else if let Some(n) = [".gzip", ".gz"].iter().find(|x| lower.ends_with(*x)).map(|x| name.len() - x.len()).filter(|&n| n > 0) {
        name[..n].to_string()
    } else {
        format!("{name}.unpacked")
    }
}

/// Unpacks the gzip file at `path` into `dir` (once: an earlier copy is reused) and returns the
/// unpacked file's path.
pub fn unpack_gz(path: &Path, dir: &Path) -> Result<PathBuf, ImportError> {
    let name = file_name(path);
    let size = crate::vfs::metadata(path).map_err(|e| ImportError::read(&name, e))?.len();
    if size > ZIP_MAX_EXPANDED {
        return Err(too_large(&name));
    }
    let bytes = crate::vfs::read(path).map_err(|e| ImportError::read(&name, e))?;
    let target = keep(dir, &hash12(&bytes), &inner_name(&name), || {
        let out = gunzip(&bytes, &name, ZIP_MAX_EXPANDED as usize)?;
        if out.starts_with(&[0x1f, 0x8b]) {
            return Err(nested());
        }
        Ok(out)
    })?;
    Ok(target)
}

/// Layers read from an unpacked copy: named without the copy's `<hash>-` prefix, and saying
/// where they came from.
fn unpacked_from(layers: &mut [ImportedLayer], copy: &Path, note: String) {
    let prefix = file_name(copy).split_once('-').map(|(h, _)| format!("{h}-")).unwrap_or_default();
    for l in layers {
        if let Some(n) = l.name.strip_prefix(&prefix).filter(|n| !n.is_empty()) {
            l.name = n.to_string();
        }
        l.profile.notices.insert(0, note.clone());
    }
}

/// Imports a gzipped file: unpacked into the cache, then read as the file inside.
pub fn import_gz(path: &Path, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
    let target = unpack_gz(path, &unpacked_dir())?;
    ctx.ticket.check()?;
    let mut head = [0u8; 2];
    let n = {
        use std::io::Read;
        let mut f = crate::vfs::open(&target).map_err(|e| ImportError::read(&file_name(&target), e))?;
        f.read(&mut head).map_err(|e| ImportError::read(&file_name(&target), e))?
    };
    if n == 2 && head == [0x1f, 0x8b] {
        return Err(nested());
    }
    let mut layers = import_path(&target, ctx)?;
    unpacked_from(&mut layers, &target, format!("Unpacked from {}.", file_name(path)));
    Ok(layers)
}

// ---- Zip members -------------------------------------------------------------------------

/// What a zip member is to the importer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    /// Read with the vector importers (kept in memory while read).
    Vector,
    /// A raster or grid: unpacked into the cache, as it is read again from its file.
    File,
    /// A `.json`: data only if it holds GeoJSON or TopoJSON.
    MaybeJson,
}

/// Files never taken for data, whatever their extension (`LICENSE.csv`, `README.json`).
fn is_paperwork(file: &str) -> bool {
    let stem = file.split('.').next().unwrap_or("").to_ascii_lowercase();
    ["license", "licence", "readme", "copying", "changelog", "notice", "citation"]
        .iter()
        .any(|w| stem == *w || stem.starts_with(&format!("{w}_")) || stem.starts_with(&format!("{w}-")))
}

/// A macOS fork or metadata folder (`__MACOSX/`, `._x.csv`).
pub fn is_fork(name: &str) -> bool {
    name.split('/').any(|p| p == "__MACOSX" || p.starts_with("._"))
}

/// Whether the member `name` is data, and how it is read. Folders, forks, paperwork and every
/// other extension (text, metadata, documents, images other than GeoTIFF) are not.
pub fn member_kind(name: &str) -> Option<MemberKind> {
    let n = name.replace('\\', "/");
    if n.ends_with('/') || is_fork(&n) {
        return None;
    }
    let file = n.rsplit('/').next().unwrap_or(&n);
    if is_paperwork(file) {
        return None;
    }
    match extension(Path::new(file)).as_str() {
        "geojson" | "csv" | "tsv" | "xlsx" | "xls" | "kml" | "gpx" | "gpkg" | "shp" => Some(MemberKind::Vector),
        "tif" | "tiff" | "nc" => Some(MemberKind::File),
        "json" => Some(MemberKind::MaybeJson),
        _ => None,
    }
}

/// Whether the start of a `.json` is GeoJSON or TopoJSON.
pub fn json_is_geo(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains("\"type\":\"Topology\"")
        || [
            "FeatureCollection",
            "Feature",
            "Point",
            "MultiPoint",
            "LineString",
            "MultiLineString",
            "Polygon",
            "MultiPolygon",
            "GeometryCollection",
        ]
        .iter()
        .any(|t| compact.contains(&format!("\"type\":\"{t}\"")))
}

fn file_part(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// Rule (a): one file name in several folders (`Shift-JIS/x.shp`, `UTF-8/x.shp`) is one layer
/// stored twice. The copy in a folder whose name contains "utf" is kept, else the first. The
/// order of the rest is kept.
pub fn dedupe_same_name(names: &[String]) -> Vec<String> {
    let utf = |n: &str| {
        let parts: Vec<&str> = n.split('/').collect();
        parts[..parts.len().saturating_sub(1)].iter().any(|p| p.to_ascii_lowercase().contains("utf"))
    };
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let key = file_part(n).to_lowercase();
        match out.iter().position(|o| file_part(o).to_lowercase() == key) {
            Some(i) if utf(n) && !utf(&out[i]) => out[i] = n.clone(),
            Some(_) => {}
            None => out.push(n.clone()),
        }
    }
    out
}

/// Rule (b)'s order: lower is preferred. `None` for formats the rule doesn't compare (rasters,
/// grids: a `.tif` and a `.csv` of one stem are different data).
fn format_rank(name: &str) -> Option<u8> {
    Some(match extension(Path::new(name)).as_str() {
        "geojson" | "json" => 0,
        "gpkg" => 1,
        "shp" => 2,
        "csv" | "tsv" => 3,
        "xlsx" | "xls" => 4,
        "kml" => 5,
        "gpx" => 6,
        _ => return None,
    })
}

/// Rule (b): one stem in several formats (`worldcities.csv`, `worldcities.xlsx`) is one layer
/// stored twice; the copy in the preferred format is kept (GeoJSON, GeoPackage, Shapefile,
/// CSV/TSV, Excel, KML, GPX). The order of the rest is kept.
pub fn dedupe_same_stem(names: &[String]) -> Vec<String> {
    let stem = |n: &str| {
        let f = file_part(n);
        f.rsplit_once('.').map_or(f, |(s, _)| s).to_lowercase()
    };
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let Some(rank) = format_rank(n) else {
            out.push(n.clone());
            continue;
        };
        match out.iter().position(|o| format_rank(o).is_some() && stem(o) == stem(n)) {
            Some(i) if format_rank(&out[i]).is_some_and(|r| rank < r) => out[i] = n.clone(),
            Some(_) => {}
            None => out.push(n.clone()),
        }
    }
    out
}

/// A number of names for a notice: the first three, then "…".
fn some_names(names: &[String]) -> String {
    let mut s = names.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if names.len() > 3 {
        s.push_str(", …");
    }
    s
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", lm_core::model::group_thousands(n as u64), if n == 1 { one } else { many })
}

/// A private in-memory path for a member while it is read (never on disk).
fn scratch_path(file: &str) -> PathBuf {
    PathBuf::from("/layeredmap-zip-member").join(unique()).join(file)
}

/// The bytes of one member of the zip `bytes`.
fn member_bytes(bytes: &[u8], member: &str, zip_name: &str) -> Result<Vec<u8>, ImportError> {
    let mut found = rustgis_engine::data::zip::read_matching(bytes, |n| n == member).map_err(|e| ImportError::read(zip_name, e))?;
    found.pop().map(|(_, b)| b).ok_or_else(|| ImportError::Refused(format!("{zip_name} no longer holds {member}.")))
}

/// The bytes of the member `member` of the zip at `path` (Excel's other sheets are read again
/// from them).
pub fn read_member(path: &Path, member: &str) -> Result<Vec<u8>, ImportError> {
    let name = file_name(path);
    let bytes = crate::vfs::read(path).map_err(|e| ImportError::read(&name, e))?;
    let entries = super::limits::zip_entries(&bytes)?;
    super::limits::check_zip(&entries)?;
    member_bytes(&bytes, member, &name)
}

/// Separates a zip member's path from a part inside it (a workbook's sheet, a GeoPackage table)
/// in `source.member`.
pub const PART: &str = " › ";

/// The members of the zip at `path` that are read, in order, after the rules: data members only,
/// `.json` members only when they hold GeoJSON or TopoJSON (`json_head` gives a member's start),
/// rule (a), then rule (b).
pub fn chosen_members(entries: &[ZipEntry], mut json_head: impl FnMut(&str) -> Option<Vec<u8>>) -> Vec<String> {
    let mut names: Vec<String> = entries
        .iter()
        .filter(|e| match member_kind(&e.name) {
            Some(MemberKind::MaybeJson) => json_head(&e.name).is_some_and(|h| json_is_geo(&h)),
            Some(_) => true,
            None => false,
        })
        .map(|e| e.name.clone())
        .collect();
    names.sort();
    dedupe_same_stem(&dedupe_same_name(&names))
}

/// Opens the data members of a zip that holds no Shapefile (`bytes` is the whole archive, its
/// `entries` already checked against the limits).
pub fn open_members(path: &Path, bytes: &[u8], entries: &[ZipEntry], ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
    let zip_name = file_name(path);
    let mut json: Vec<(String, Vec<u8>)> = Vec::new();
    let members = chosen_members(entries, |m| {
        let b = member_bytes(bytes, m, &zip_name).ok()?;
        let head = b[..b.len().min(4096)].to_vec();
        json.push((m.to_string(), b));
        Some(head)
    });
    if members.is_empty() {
        let others: Vec<String> = entries.iter().map(|e| e.name.clone()).filter(|n| !is_fork(n) && !n.ends_with('/')).take(5).collect();
        return Err(ImportError::Unsupported(format!(
            "{zip_name} holds no files {APP} opens{}.",
            if others.is_empty() { String::new() } else { format!(" (it holds {})", others.join(", ")) }
        )));
    }
    let mut zip_key: Option<String> = None;
    let mut layers: Vec<ImportedLayer> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    let mut first_error: Option<ImportError> = None;
    for member in &members {
        ctx.ticket.check()?;
        let file = file_part(member).to_string();
        let content = match json.iter().position(|(n, _)| n == member) {
            Some(i) => json.swap_remove(i).1,
            None => member_bytes(bytes, member, &zip_name)?,
        };
        let read = if member_kind(member) == Some(MemberKind::File) {
            // Read again from its file later (tiles, time steps): kept in the cache.
            let key = zip_key.get_or_insert_with(|| hash12(bytes)).clone();
            keep(&unpacked_dir(), &key, &file, || Ok(content)).and_then(|p| {
                let mut ls = import_path(&p, ctx)?;
                unpacked_from(&mut ls, &p, format!("Unpacked from {zip_name} ({member})."));
                Ok(ls)
            })
        } else {
            let scratch = scratch_path(&file);
            crate::vfs::put(&scratch, content);
            let read = import_path(&scratch, ctx);
            crate::vfs::remove(&scratch);
            read.map(|mut ls| {
                for l in &mut ls {
                    let inner = l.profile.source.member.take();
                    let id = match inner {
                        Some(part) => format!("{member}{PART}{part}"),
                        None => member.clone(),
                    };
                    l.profile.source = source_ref(path, Some(id));
                }
                ls
            })
        };
        match read {
            Ok(ls) => layers.extend(ls),
            Err(ImportError::Canceled) => return Err(ImportError::Canceled),
            Err(e) => {
                log::info!("{zip_name}: {member} not opened: {}", e.details());
                unreadable.push(file);
                first_error.get_or_insert(e);
            }
        }
    }
    if layers.is_empty() {
        return Err(first_error.unwrap_or_else(|| ImportError::Unsupported(format!("{zip_name} holds no files {APP} opens."))));
    }
    let mut notices = Vec::new();
    // Tables are left out when something has a location: they'd crowd the map's layer list.
    if layers.iter().any(|l| l.profile.kind != LayerKind::Table) {
        let tables: Vec<String> = layers
            .iter()
            .filter(|l| l.profile.kind == LayerKind::Table)
            .map(|l| {
                l.profile
                    .source
                    .member
                    .as_deref()
                    .map_or_else(|| l.name.clone(), |m| file_part(m.split(PART).next().unwrap_or(m)).to_string())
            })
            .collect();
        if !tables.is_empty() {
            layers.retain(|l| l.profile.kind != LayerKind::Table);
            notices.push(format!(
                "Also in the zip: {} without locations ({}).",
                plural(tables.len(), "table", "tables"),
                some_names(&tables)
            ));
        }
    }
    if layers.len() > ZIP_MAX_LAYERS {
        let left = layers.len() - ZIP_MAX_LAYERS;
        layers.truncate(ZIP_MAX_LAYERS);
        notices.push(format!(
            "{} more {} in the zip {} left out: {APP} opens at most {ZIP_MAX_LAYERS} layers from one zip.",
            lm_core::model::group_thousands(left as u64),
            if left == 1 { "layer" } else { "layers" },
            if left == 1 { "was" } else { "were" }
        ));
    }
    if !unreadable.is_empty() {
        notices.push(format!("Also in the zip, but couldn't be read: {}.", some_names(&unreadable)));
    }
    if let Some(first) = layers.first_mut() {
        first.profile.notices.extend(notices);
    }
    Ok(layers)
}

#[cfg(test)]
mod tests;
