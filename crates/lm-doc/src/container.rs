//! The `.lmap` document (WP5.1; `docs/lm/format-v1.md`): one ZIP file holding
//!
//! - `mimetype`: `application/vnd.layeredmap.map+zip`, stored first and uncompressed, so a file
//!   can be recognised from its first bytes;
//! - `manifest.json`: the document (layers with their styles, filters and answers; kept results
//!   with their recipes; the view), plus a table of source references and of owned results;
//! - `results/`: owned results, the data of layers that no source file can give back (an
//!   analysis result, a drawn area, a layer with joined or computed fields). Vectors are
//!   FlatGeobuf (GeoJSON for tables and features without geometry), rasters GeoTIFF.
//!
//! Sources are referenced, never copied: absolute path, path relative to the document, and the
//! fingerprint (size, modification time, SHA-256 when known). A source that moved together with
//! the document is found again by its relative path when its fingerprint still matches.
//!
//! Saving is atomic: the whole file is written to a temporary file in the same folder, synced,
//! and renamed over the old one, so a crash at any moment leaves the old file or the new one.

use std::io::{BufWriter, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use lm_core::document::{DOCUMENT_VERSION, Document};
use lm_core::model::{LayerId, SourceRef};
use rustgis_data::Dataset;
use rustgis_data::raster::Raster;
use serde::{Deserialize, Serialize};

use crate::error::{DocError, Result};
use crate::manifest::{FORMAT_NAME, MANIFEST_FILE};
use crate::source::{SourceState, check_source};

/// The file extension of a LayeredMap document.
pub const EXTENSION: &str = "lmap";
/// The first entry of every `.lmap` file.
pub const MIMETYPE: &str = "application/vnd.layeredmap.map+zip";
/// The container version of a map written by this build. A map package is container 2
/// (`package::PACKAGE_CONTAINER`); anything newer than that is refused on open.
pub const CONTAINER_VERSION: u32 = 1;
/// A manifest holds references and profiles, not data; anything larger is not ours.
pub(crate) const MAX_MANIFEST_BYTES: u64 = 256 * 1024 * 1024;
/// The largest owned result read back (8 GiB inflated).
const MAX_RESULT_BYTES: u64 = 8 << 30;
/// Temporary files left by a save that never finished are removed once they are this old.
const STALE_TEMP_SECS: u64 = 10 * 60;

/// The data of a layer the document owns.
#[derive(Clone)]
pub enum OwnedData {
    Vector(Arc<Dataset>),
    Raster(Arc<Raster>),
}

/// A source file, as the manifest's `sources` table records it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SourceEntry {
    /// The layers read from it.
    pub(crate) layers: Vec<LayerId>,
    /// Absolute path when saved (in a package: the file's entry, `data/1/schools.csv`).
    pub(crate) path: String,
    /// Path relative to the folder holding the document, `/`-separated.
    #[serde(default)]
    pub(crate) relative: Option<String>,
    pub(crate) size: u64,
    pub(crate) mtime: i64,
    #[serde(default)]
    pub(crate) sha256: Option<String>,
    /// Package only (container 2): the ZIP entries holding the file and its companions, the
    /// file first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) embedded: Vec<String>,
    /// Package only: the dataset's licence when known ("CC BY 4.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) license: Option<String>,
}

/// An owned result, as the manifest's `owned` table records it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct OwnedEntry {
    pub(crate) layer: LayerId,
    /// The ZIP entry holding the data (`results/layer-3.fgb`).
    pub(crate) entry: String,
    /// `FlatGeobuf`, `GeoJSON` or `GeoTIFF`.
    pub(crate) format: String,
    /// The dataset's own name, given back on reading.
    #[serde(default)]
    pub(crate) name: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct LmapManifest {
    pub(crate) format: String,
    pub(crate) container: u32,
    /// Seconds since the Unix epoch.
    #[serde(default)]
    pub(crate) saved_at: i64,
    /// "LayeredMap 0.5.0".
    #[serde(default)]
    pub(crate) software: String,
    #[serde(default)]
    pub(crate) sources: Vec<SourceEntry>,
    #[serde(default)]
    pub(crate) owned: Vec<OwnedEntry>,
    /// Package only (container 2): what packaging changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) package: Option<crate::package::PackageInfo>,
    /// The document's fields, `version` among them (the same as a v0 manifest).
    #[serde(flatten)]
    pub(crate) document: Document,
}

/// What a save left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaveReport {
    /// Owned results that could not be stored (layer, reason). Their layers read their source
    /// again on open, if they have one.
    pub skipped: Vec<(LayerId, String)>,
}

/// An opened document.
pub struct Loaded {
    pub doc: Document,
    /// Owned results by layer.
    pub owned: Vec<(LayerId, OwnedData)>,
    /// Sources found next to the document by their relative path (old path, new path).
    pub relinked: Vec<(String, String)>,
    /// Owned results (or packaged files) that could not be read, in plain words.
    pub notes: Vec<String>,
    /// The file is a map package (container 2): its sources were unpacked from it, and saving
    /// it again keeps it a package.
    pub package: bool,
}

/// Whether a layer's source is a file (LayeredMap's own layers record no size and no time).
pub fn is_file_source(s: &SourceRef) -> bool {
    !(s.size == 0 && s.mtime == 0) && Path::new(&s.path).is_absolute()
}

/// Seconds since the Unix epoch, now.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Saves `doc` and its owned results as a `.lmap` file at `path`, atomically.
pub fn save_lmap(doc: &Document, owned: &[(LayerId, OwnedData)], path: &Path) -> Result<SaveReport> {
    let abs_dir = folder_of(path);
    let mut report = SaveReport::default();
    let entries = encode_owned(doc, owned, &mut report);
    let mut document = doc.clone();
    document.version = DOCUMENT_VERSION;
    let manifest = LmapManifest {
        format: FORMAT_NAME.into(),
        container: CONTAINER_VERSION,
        saved_at: now_secs(),
        software: software(),
        sources: sources_of(doc, &abs_dir),
        owned: entries.iter().map(|(e, _)| e.clone()).collect(),
        package: None,
        document,
    };
    let json =
        serde_json::to_vec_pretty(&manifest).map_err(|e| DocError::write(path, std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    write_container(path, &json, &entries, &[])?;
    Ok(report)
}

/// "LayeredMap 0.5.0".
pub(crate) fn software() -> String {
    format!("LayeredMap {}", env!("CARGO_PKG_VERSION"))
}

/// The absolute folder that holds `path`.
pub(crate) fn folder_of(path: &Path) -> PathBuf {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::path::absolute(&dir).unwrap_or(dir)
}

/// The owned results of layers still in `doc`, encoded as ZIP entries; those that can't be are
/// reported.
pub(crate) fn encode_owned(doc: &Document, owned: &[(LayerId, OwnedData)], report: &mut SaveReport) -> Vec<(OwnedEntry, Vec<u8>)> {
    let mut entries: Vec<(OwnedEntry, Vec<u8>)> = Vec::new();
    for (id, data) in owned {
        if doc.layer(*id).is_none() || entries.iter().any(|(e, _)| e.layer == *id) {
            continue;
        }
        match encode(*id, data) {
            Ok(e) => entries.push(e),
            Err(why) => report.skipped.push((*id, why)),
        }
    }
    entries
}

/// Writes the container atomically: `mimetype`, the manifest `json`, the owned results, then
/// `files` (ZIP entry, file on disk), copied from disk as they are written. The whole file goes
/// to a temporary file in the same folder, synced, and is renamed over `path`.
pub(crate) fn write_container(path: &Path, json: &[u8], entries: &[(OwnedEntry, Vec<u8>)], files: &[(String, PathBuf)]) -> Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    remove_stale_temps(path);
    let tmp = temp_beside(path);
    let written = (|| -> std::io::Result<()> {
        let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        let mut zip = zip::ZipWriter::new(BufWriter::new(file));
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("mimetype", stored).map_err(zip_io)?;
        zip.write_all(MIMETYPE.as_bytes())?;
        zip.start_file(MANIFEST_FILE, deflated).map_err(zip_io)?;
        zip.write_all(json)?;
        for (e, bytes) in entries {
            // FlatGeobuf and GeoTIFF are already compact; Deflate still helps their attributes.
            let opts = deflated.large_file(bytes.len() as u64 >= u64::from(u32::MAX));
            zip.start_file(e.entry.as_str(), opts).map_err(zip_io)?;
            zip.write_all(bytes)?;
        }
        for (entry, src) in files {
            let mut f = std::fs::File::open(src).map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", src.display())))?;
            let len = f.metadata()?.len();
            let opts = deflated.large_file(len >= u64::from(u32::MAX));
            zip.start_file(entry.as_str(), opts).map_err(zip_io)?;
            std::io::copy(&mut f, &mut zip)?;
        }
        let file = zip.finish().map_err(zip_io)?.into_inner().map_err(|e| e.into_error())?;
        file.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(DocError::write(path, e));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(DocError::write(path, e));
    }
    // The rename itself is made durable by syncing the folder.
    if let Ok(d) = std::fs::File::open(&dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Opens a `.lmap` file. Unknown fields are ignored; a newer container or document version is
/// refused.
pub fn load_lmap(path: &Path) -> Result<Loaded> {
    let shown = || path.display().to_string();
    let file = std::fs::File::open(path).map_err(|e| DocError::read(path, e))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|_| DocError::NotADocument { path: shown() })?;
    let bytes = read_entry(&mut zip, MANIFEST_FILE, MAX_MANIFEST_BYTES).map_err(|_| DocError::NotADocument { path: shown() })?;
    let manifest = parse_manifest(&bytes, path)?;
    let mut doc = manifest.document;
    let mut owned = Vec::new();
    let mut notes = Vec::new();
    for e in &manifest.owned {
        let Some(layer) = doc.layer(e.layer) else { continue };
        let name = layer.display_name.clone();
        match read_entry(&mut zip, &e.entry, MAX_RESULT_BYTES).map_err(|e| e.to_string()).and_then(|b| decode(e, &b)) {
            Ok(d) => owned.push((e.layer, d)),
            Err(why) => notes.push(format!("{name} could not be read from this map: {why}")),
        }
    }
    let package = manifest.container >= crate::package::PACKAGE_CONTAINER || manifest.package.is_some();
    if manifest.sources.iter().any(|s| !s.embedded.is_empty()) {
        notes.extend(crate::package::unpack(&mut zip, &bytes, &manifest.sources, &mut doc));
    }
    let base = path.parent().map(|p| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()));
    let relinked = match base {
        Some(base) if !package => relink_moved(&mut doc, &manifest.sources, &base),
        _ => Vec::new(),
    };
    Ok(Loaded { doc, owned, relinked, notes, package })
}

/// Whether the `.lmap` file at `path` is a map package (its manifest says container 2 or
/// carries a package section). False for anything unreadable.
pub fn is_package(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else { return false };
    let Ok(mut zip) = zip::ZipArchive::new(std::io::BufReader::new(file)) else { return false };
    let Ok(bytes) = read_entry(&mut zip, MANIFEST_FILE, MAX_MANIFEST_BYTES) else { return false };
    serde_json::from_slice::<serde_json::Value>(&bytes).is_ok_and(|v| {
        v.get("container").and_then(serde_json::Value::as_u64).is_some_and(|c| c >= u64::from(crate::package::PACKAGE_CONTAINER))
            || v.get("package").is_some_and(|p| !p.is_null())
    })
}

/// Opens any document: a `.lmap` file, a v0 document folder, or a v0 `manifest.json`. A v0
/// document opens as it was saved, with no owned results (`docs/lm/format-v1.md` › Migration).
pub fn open_any(path: &Path) -> Result<Loaded> {
    let v0 = |dir: &Path| {
        crate::manifest::load(dir).map(|doc| Loaded { doc, owned: Vec::new(), relinked: Vec::new(), notes: Vec::new(), package: false })
    };
    if path.is_dir() {
        return v0(path);
    }
    if path.file_name().and_then(|n| n.to_str()) == Some(MANIFEST_FILE) {
        return v0(path.parent().unwrap_or(Path::new(".")));
    }
    load_lmap(path)
}

fn parse_manifest(bytes: &[u8], path: &Path) -> Result<LmapManifest> {
    let shown = || path.display().to_string();
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| DocError::NotADocument { path: shown() })?;
    if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT_NAME) {
        return Err(DocError::NotADocument { path: shown() });
    }
    let container = value.get("container").and_then(|v| v.as_u64()).ok_or(DocError::NotADocument { path: shown() })?;
    if container > u64::from(crate::package::PACKAGE_CONTAINER) {
        return Err(DocError::TooNew { found: container, supported: crate::package::PACKAGE_CONTAINER });
    }
    let version = value.get("version").and_then(|v| v.as_u64()).ok_or(DocError::NotADocument { path: shown() })?;
    if version > u64::from(DOCUMENT_VERSION) {
        return Err(DocError::TooNew { found: version, supported: DOCUMENT_VERSION });
    }
    serde_json::from_value(value).map_err(|e| DocError::Damaged { path: shown(), detail: e.to_string() })
}

pub(crate) fn read_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str, max: u64) -> std::result::Result<Vec<u8>, String> {
    let f = zip.by_name(name).map_err(|e| format!("{name}: {e}"))?;
    if f.size() > max {
        return Err(format!("{name} is too large"));
    }
    let mut out = Vec::with_capacity(usize::try_from(f.size()).unwrap_or(0));
    f.take(max + 1).read_to_end(&mut out).map_err(|e| format!("{name}: {e}"))?;
    if out.len() as u64 > max {
        return Err(format!("{name} is too large"));
    }
    Ok(out)
}

/// One owned result as a ZIP entry.
pub(crate) fn encode(id: LayerId, data: &OwnedData) -> std::result::Result<(OwnedEntry, Vec<u8>), String> {
    match data {
        OwnedData::Vector(ds) => {
            let all_located = ds.geometry_type != rustgis_data::GeometryType::None && ds.features.iter().all(|f| f.geometry.is_some());
            let (format, ext, bytes) = if all_located {
                ("FlatGeobuf", "fgb", rustgis_data::source::fgb::write_in_order(ds).map_err(|e| e.to_string())?)
            } else {
                ("GeoJSON", "geojson", rustgis_data::io::write_bytes(ds, "geojson").map_err(|e| e.to_string())?.0)
            };
            Ok((
                OwnedEntry { layer: id, entry: format!("results/layer-{}.{ext}", id.0), format: format.into(), name: ds.name.clone() },
                bytes,
            ))
        }
        OwnedData::Raster(r) => {
            let bytes = rustgis_data::raster::geotiff::write(r, 0).map_err(|e| e.to_string())?;
            Ok((
                OwnedEntry { layer: id, entry: format!("results/layer-{}.tif", id.0), format: "GeoTIFF".into(), name: r.name.clone() },
                bytes,
            ))
        }
    }
}

fn decode(e: &OwnedEntry, bytes: &[u8]) -> std::result::Result<OwnedData, String> {
    match e.format.as_str() {
        "FlatGeobuf" | "GeoJSON" => {
            let file = if e.format == "FlatGeobuf" { "result.fgb" } else { "result.geojson" };
            let (mut ds, _) = rustgis_data::io::read_bytes(file, bytes).map_err(|e| e.to_string())?;
            ds.name = e.name.clone();
            ds.source = None;
            Ok(OwnedData::Vector(Arc::new(ds)))
        }
        "GeoTIFF" => {
            let mut r = rustgis_data::raster::read_bytes("result.tif", bytes).map_err(|e| e.to_string())?;
            r.name = e.name.clone();
            r.source = None;
            Ok(OwnedData::Raster(Arc::new(r)))
        }
        other => Err(format!("results stored as {other} are not supported by this version")),
    }
}

/// The `sources` table: one row per source file, with the layers read from it.
fn sources_of(doc: &Document, dir: &Path) -> Vec<SourceEntry> {
    let mut out: Vec<SourceEntry> = Vec::new();
    for l in &doc.layers {
        let s = &l.profile.source;
        if !is_file_source(s) {
            continue;
        }
        match out.iter_mut().find(|e| e.path == s.path) {
            Some(e) => e.layers.push(l.id),
            None => out.push(SourceEntry {
                layers: vec![l.id],
                path: s.path.clone(),
                relative: relative_path(dir, Path::new(&s.path)),
                size: s.size,
                mtime: s.mtime,
                sha256: s.sha256.clone(),
                embedded: Vec::new(),
                license: None,
            }),
        }
    }
    out
}

/// Sources missing at their absolute path but present, unchanged, at their path relative to the
/// document: the layers are pointed there. Returns (old path, new path) for each.
fn relink_moved(doc: &mut Document, sources: &[SourceEntry], base: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for s in sources {
        let Some(rel) = &s.relative else { continue };
        let fp = SourceRef { path: s.path.clone(), member: None, size: s.size, mtime: s.mtime, sha256: s.sha256.clone() };
        if check_source(&fp) != SourceState::Missing {
            continue;
        }
        let candidate = normalize(&base.join(rel));
        let moved = SourceRef { path: candidate.display().to_string(), ..fp };
        if check_source(&moved) != SourceState::Unchanged {
            continue;
        }
        for l in doc.layers.iter_mut().filter(|l| l.profile.source.path == s.path) {
            l.profile.source.path = moved.path.clone();
        }
        out.push((s.path.clone(), moved.path.clone()));
    }
    out
}

/// `to` relative to the folder `from`, `/`-separated (`../data/schools.csv`). None when they
/// share no root (another volume on Windows).
pub fn relative_path(from: &Path, to: &Path) -> Option<String> {
    let from_n = normalize(from);
    let from: Vec<Component> = from_n.components().collect();
    let to_n = normalize(to);
    let to: Vec<Component> = to_n.components().collect();
    if from.first() != to.first() {
        return None;
    }
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), from.len() - common).collect();
    parts.extend(to.iter().skip(common).map(|c| c.as_os_str().to_string_lossy().to_string()));
    Some(parts.join("/"))
}

/// Removes `.` and folds `..` lexically (the paths are only compared and joined).
pub(crate) fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `.<name>.saving-<pid>-<n>` beside `path`.
fn temp_beside(path: &Path) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "map.lmap".into());
    path.with_file_name(format!(".{name}.saving-{}-{n}", std::process::id()))
}

/// Removes temporary files an interrupted save left beside `path`, once they are old enough that
/// no save in progress can own them.
fn remove_stale_temps(path: &Path) {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else { return };
    let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
    let prefix = format!(".{}.saving-", name.to_string_lossy());
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if !e.file_name().to_string_lossy().starts_with(&prefix) {
            continue;
        }
        let old =
            e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() >= STALE_TEMP_SECS);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn zip_io(e: zip::result::ZipError) -> std::io::Error {
    match e {
        zip::result::ZipError::Io(e) => e,
        other => std::io::Error::other(other.to_string()),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod tests_support {
    /// Reads a manifest as `load_lmap` does (version checks included).
    pub fn parse(bytes: &[u8]) -> crate::Result<()> {
        super::parse_manifest(bytes, std::path::Path::new("test.lmap")).map(|_| ())
    }
}
