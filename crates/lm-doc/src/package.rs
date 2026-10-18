//! Portable map packages (WP6.5; `docs/lm/format-v1.md` › Map packages): one `.lmap` file that
//! carries the map **and its data**, so it opens on another computer.
//!
//! A package is the ordinary `.lmap` container with the source files inside it, under `data/`
//! (`data/1/schools.csv`, a Shapefile's parts beside it), and the manifest's `sources` rows
//! pointing at them (`embedded`). Its `container` is 2: a build that reads only container 1 would
//! open its layers with no data, so it refuses the file instead.
//!
//! Packaging also cleans what would leak: absolute paths (which name the user's account and
//! folders) are replaced by the entry inside the package or by the bare file name, and fields the
//! user chose to drop (names, emails, phone or ID numbers) are removed — those layers are stored
//! as owned results without the fields, instead of their source file.
//!
//! Opening a package unpacks its files into a folder of the app's own
//! ([`set_unpack_dir`]; one folder per package, by the manifest's hash, reused when opened again)
//! and points the layers there, with the files' recorded modification times, so the map reads
//! exactly as it was packaged and its kept results still match their recipes.

use std::io::{Read, Seek};
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

use lm_core::document::{DOCUMENT_VERSION, Document};
use lm_core::model::LayerId;
use rustgis_data::Dataset;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::container::{
    LmapManifest, OwnedData, SaveReport, SourceEntry, encode_owned, is_file_source, now_secs, software, write_container,
};
use crate::error::{DocError, Result};
use crate::manifest::FORMAT_NAME;

/// The container version of a map package.
pub const PACKAGE_CONTAINER: u32 = 2;
/// The folder inside the ZIP that holds packaged files.
pub const DATA_DIR: &str = "data";
/// The largest packaged file unpacked (64 GiB).
const MAX_FILE_BYTES: u64 = 64 << 30;
/// The most files one source folder may bring (a folder-based format).
const MAX_FOLDER_FILES: usize = 10_000;
/// Files that travel with a source of the same name: a Shapefile's parts, projection and code
/// page files, world files, raster metadata, overviews and spatial indexes.
const COMPANIONS: &[&str] = &[
    "shp", "shx", "dbf", "prj", "cpg", "qix", "sbn", "sbx", "fix", "qmd", "xml", "aux.xml", "tfw", "tifw", "tiffw", "wld", "jgw", "pgw",
    "ovr", "hdr", "aux", "rrd", "vat.dbf",
];

static UNPACK_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Sets the folder packages are unpacked into (the app: beside its autosaves).
pub fn set_unpack_dir(dir: PathBuf) {
    if let Ok(mut d) = UNPACK_DIR.write() {
        *d = Some(dir);
    }
}

/// The folder packages are unpacked into: the one set, else a folder in the system's temporary
/// folder.
pub fn unpack_dir() -> PathBuf {
    UNPACK_DIR.read().ok().and_then(|d| d.clone()).unwrap_or_else(|| std::env::temp_dir().join("LayeredMap packages"))
}

/// What packaging changed, recorded in the manifest's `package` section.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Fields left out, by layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped: Vec<DroppedFields>,
    /// How many absolute paths were replaced.
    #[serde(default)]
    pub paths_removed: usize,
}

/// Fields of one layer left out of a package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DroppedFields {
    pub layer: LayerId,
    pub fields: Vec<String>,
}

/// A source file a package will carry.
#[derive(Clone, Debug, PartialEq)]
pub struct PackedSource {
    /// The source as the map references it.
    pub path: String,
    /// The files carried: the source first, then its companions (or a folder's files).
    pub files: Vec<PathBuf>,
    /// Their bytes together.
    pub size: u64,
    /// The layers read from it.
    pub layers: Vec<LayerId>,
    /// Whether the source can be read now.
    pub found: bool,
}

impl PackedSource {
    /// "schools.csv".
    pub fn file_name(&self) -> &str {
        self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path)
    }
}

/// An absolute path that won't be written, and what is written instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stripped {
    pub from: String,
    pub to: String,
}

/// The user's choices for a package.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackageOptions {
    /// Fields to leave out, by layer. Such a layer is stored as an owned result without them, so
    /// its data must be among the owned data passed in.
    pub dropped: Vec<(LayerId, Vec<String>)>,
    /// Licences of sources, by their path, written into the manifest ("CC BY 4.0").
    pub licenses: Vec<(String, String)>,
}

/// What a package will hold and what it leaves out: shown before it is written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Review {
    pub sources: Vec<PackedSource>,
    /// Absolute paths replaced, each once.
    pub stripped: Vec<Stripped>,
}

/// What writing a package did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackageReport {
    /// Owned results that could not be stored.
    pub save: SaveReport,
    pub stripped: Vec<Stripped>,
    /// Sources that couldn't be read, so their layers travel without data (file names).
    pub missing: Vec<String>,
    /// The package's size in bytes.
    pub bytes: u64,
}

/// The files that travel with the source at `path`: the file itself, then its companions of the
/// same name in its folder (or, for a folder, the files inside it).
pub fn companions(path: &Path) -> Vec<PathBuf> {
    if path.is_dir() {
        let mut out = Vec::new();
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            entries.sort();
            for p in entries {
                if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("._") || n == ".DS_Store") {
                    continue;
                }
                if p.is_dir() {
                    stack.push(p);
                } else if p.is_file() && out.len() < MAX_FOLDER_FILES {
                    out.push(p);
                }
            }
        }
        out.sort();
        return out;
    }
    let mut out = vec![path.to_path_buf()];
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else { return out };
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s).to_lowercase();
    let lname = name.to_lowercase();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut extra: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.as_path() != path && p.is_file())
        .filter(|p| {
            let Some(n) = p.file_name().and_then(|n| n.to_str()).map(str::to_lowercase) else { return false };
            if n.starts_with("._") {
                return false;
            }
            COMPANIONS.iter().any(|ext| n == format!("{stem}.{ext}") || n == format!("{lname}.{ext}"))
        })
        .collect();
    extra.sort();
    out.extend(extra);
    out
}

/// The sources a package of `doc` carries: every file source of a layer not in `skip` (layers
/// stored as owned results need no source).
pub fn sources(doc: &Document, skip: &[LayerId]) -> Vec<PackedSource> {
    let mut out: Vec<PackedSource> = Vec::new();
    for l in &doc.layers {
        let s = &l.profile.source;
        if !is_file_source(s) || skip.contains(&l.id) {
            continue;
        }
        if let Some(p) = out.iter_mut().find(|p| p.path == s.path) {
            p.layers.push(l.id);
            continue;
        }
        let path = Path::new(&s.path);
        let found = path.exists();
        let files = if found { companions(path) } else { Vec::new() };
        let size = files.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum();
        out.push(PackedSource { path: s.path.clone(), files, size, layers: vec![l.id], found });
    }
    out
}

/// The ZIP entry for `file`, the `k`-th source's file (1-based), whose main file is `main`.
fn entry_for(k: usize, main: &Path, file: &Path) -> String {
    let rel: Vec<String> = if main.is_dir() {
        let top = main.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "data".into());
        let inner =
            file.strip_prefix(main).map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<String>>());
        std::iter::once(top).chain(inner.unwrap_or_default()).collect()
    } else {
        vec![file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into())]
    };
    let rel: Vec<String> = rel.into_iter().map(|c| c.replace(['/', '\\', ':'], "_")).collect();
    format!("{DATA_DIR}/{k}/{}", rel.join("/"))
}

/// A string that is an absolute path: `/…/…`, `C:\…` or `\\server\…`.
fn looks_absolute(s: &str) -> bool {
    if s.len() > 4096 || s.contains(['\n', '\r']) || s.trim() != s {
        return false;
    }
    let b = s.as_bytes();
    let unix = s.starts_with('/') && !s.starts_with("//") && s[1..].contains('/');
    let drive = b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
    let unc = s.starts_with("\\\\");
    unix || drive || unc
}

/// The last part of a path.
fn base_name(s: &str) -> String {
    s.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("").to_string()
}

/// Replaces every absolute path among `v`'s strings by its file name, collecting what changed.
fn strip_paths(v: &mut serde_json::Value, out: &mut Vec<Stripped>) {
    match v {
        serde_json::Value::String(s) if looks_absolute(s) => {
            let to = base_name(s);
            if !out.iter().any(|x| x.from == *s) {
                out.push(Stripped { from: s.clone(), to: to.clone() });
            }
            *s = to;
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| strip_paths(x, out)),
        serde_json::Value::Object(m) => m.values_mut().for_each(|x| strip_paths(x, out)),
        _ => {}
    }
}

/// `ds` without the fields `drop`.
pub fn drop_fields(ds: &Dataset, drop: &[String]) -> Dataset {
    let keep: Vec<usize> = ds.fields.iter().enumerate().filter(|(_, f)| !drop.contains(&f.name)).map(|(i, _)| i).collect();
    let mut out = ds.clone();
    out.fields = keep.iter().filter_map(|i| ds.fields.get(*i).cloned()).collect();
    for f in &mut out.features {
        let attrs = std::mem::take(&mut f.attrs);
        f.attrs = keep.iter().map(|i| attrs.get(*i).cloned().unwrap_or(rustgis_data::Value::Null)).collect();
    }
    out
}

/// About how many bytes `data` takes as an owned result, before compression: 16 per vertex plus
/// the attribute values for vectors, 4 per cell for rasters.
pub fn estimate_owned(data: &OwnedData) -> u64 {
    match data {
        OwnedData::Vector(ds) => {
            let attrs = |v: &rustgis_data::Value| match v {
                rustgis_data::Value::Text(t) => t.len() as u64 + 4,
                rustgis_data::Value::Null => 0,
                _ => 8,
            };
            1024 + ds
                .features
                .iter()
                .map(|f| 16 + f.geometry.as_ref().map_or(0, |g| 16 * g.coord_count() as u64) + f.attrs.iter().map(attrs).sum::<u64>())
                .sum::<u64>()
        }
        OwnedData::Raster(r) => 1024 + r.width as u64 * r.height as u64 * 4,
    }
}

/// The package's document and tables: `doc` with its sources pointed into the package, dropped
/// fields removed from their layers, and every absolute path replaced.
struct Prepared {
    doc: Document,
    sources: Vec<SourceEntry>,
    /// (ZIP entry, file on disk) for every packaged file.
    files: Vec<(String, PathBuf)>,
    stripped: Vec<Stripped>,
    packed: Vec<PackedSource>,
    info: PackageInfo,
}

fn prepare(doc: &Document, owned: &[LayerId], opts: &PackageOptions) -> Prepared {
    let dropped: Vec<&(LayerId, Vec<String>)> = opts.dropped.iter().filter(|(_, f)| !f.is_empty()).collect();
    let mut skip: Vec<LayerId> = owned.to_vec();
    skip.extend(dropped.iter().map(|(id, _)| *id));
    let packed = sources(doc, &skip);
    let mut out = doc.clone();
    out.version = DOCUMENT_VERSION;
    let mut stripped = Vec::new();
    let mut files = Vec::new();
    let mut table = Vec::new();
    for (i, p) in packed.iter().enumerate().filter(|(_, p)| p.found) {
        let main = Path::new(&p.path);
        let entries: Vec<(String, PathBuf)> = p.files.iter().map(|f| (entry_for(i + 1, main, f), f.clone())).collect();
        let Some(first) = entries.first().map(|(e, _)| e.clone()) else { continue };
        let first =
            if main.is_dir() { format!("{DATA_DIR}/{}/{}", i + 1, base_name(&p.path).replace(['/', '\\', ':'], "_")) } else { first };
        for l in out.layers.iter_mut().filter(|l| p.layers.contains(&l.id)) {
            l.profile.source.path = first.clone();
        }
        let src = doc.layer(p.layers[0]).map(|l| l.profile.source.clone());
        stripped.push(Stripped { from: p.path.clone(), to: first.clone() });
        table.push(SourceEntry {
            layers: p.layers.clone(),
            path: first.clone(),
            relative: None,
            size: src.as_ref().map_or(0, |s| s.size),
            mtime: src.as_ref().map_or(0, |s| s.mtime),
            sha256: src.and_then(|s| s.sha256),
            embedded: entries.iter().map(|(e, _)| e.clone()).collect(),
            license: opts.licenses.iter().find(|(path, _)| *path == p.path).map(|(_, l)| l.clone()),
        });
        files.extend(entries);
    }
    for (id, fields) in &dropped {
        if let Some(l) = out.layers.iter_mut().find(|l| l.id == *id) {
            l.profile.fields.retain(|f| !fields.contains(&f.name));
            l.roles.retain(|k, _| !fields.contains(k));
            l.starred.retain(|k| !fields.contains(k));
        }
    }
    let info = PackageInfo {
        dropped: dropped.iter().map(|(layer, fields)| DroppedFields { layer: *layer, fields: fields.clone() }).collect(),
        paths_removed: 0,
    };
    Prepared { doc: out, sources: table, files, stripped, packed, info }
}

/// What a package of `doc` would carry and which paths it would remove, before it is written.
/// `owned` are the layers stored as owned results.
pub fn review(doc: &Document, owned: &[LayerId], opts: &PackageOptions) -> Review {
    let p = prepare(doc, owned, opts);
    let mut stripped = p.stripped;
    if let Ok(mut v) = serde_json::to_value(&p.doc) {
        strip_paths(&mut v, &mut stripped);
    }
    Review { sources: p.packed, stripped }
}

/// Writes `doc` as a map package at `path`, atomically: its owned results, its source files and
/// their companions, with the fields in `opts.dropped` left out and every absolute path removed.
pub fn save_package(doc: &Document, owned: &[(LayerId, OwnedData)], opts: &PackageOptions, path: &Path) -> Result<PackageReport> {
    let mut owned: Vec<(LayerId, OwnedData)> = owned.to_vec();
    for (id, fields) in opts.dropped.iter().filter(|(_, f)| !f.is_empty()) {
        match owned.iter_mut().find(|(o, _)| o == id) {
            Some((_, OwnedData::Vector(ds))) => *ds = std::sync::Arc::new(drop_fields(ds, fields)),
            Some((_, OwnedData::Raster(_))) => {}
            None => {
                let name = doc.layer(*id).map_or_else(|| format!("layer {}", id.0), |l| l.display_name.clone());
                return Err(DocError::write(
                    path,
                    std::io::Error::other(format!("{name} is still loading, so its fields can't be left out")),
                ));
            }
        }
    }
    let owned_ids: Vec<LayerId> = owned.iter().map(|(id, _)| *id).collect();
    let mut p = prepare(doc, &owned_ids, opts);
    // A layer stored as a result keeps only its source's file name.
    for l in p.doc.layers.iter_mut().filter(|l| owned_ids.contains(&l.id) && is_file_source(&l.profile.source)) {
        let from = l.profile.source.path.clone();
        l.profile.source.path = base_name(&from);
        l.profile.source.size = 0;
        l.profile.source.mtime = 0;
        l.profile.source.sha256 = None;
    }
    let mut report =
        PackageReport { missing: p.packed.iter().filter(|s| !s.found).map(|s| s.file_name().to_string()).collect(), ..Default::default() };
    let entries = encode_owned(&p.doc, &owned, &mut report.save);
    let manifest = LmapManifest {
        format: FORMAT_NAME.into(),
        container: PACKAGE_CONTAINER,
        saved_at: now_secs(),
        software: software(),
        sources: p.sources,
        owned: entries.iter().map(|(e, _)| e.clone()).collect(),
        package: Some(p.info),
        document: p.doc,
    };
    let invalid = |e: serde_json::Error| DocError::write(path, std::io::Error::new(std::io::ErrorKind::InvalidData, e));
    let mut value = serde_json::to_value(&manifest).map_err(invalid)?;
    let mut stripped = std::mem::take(&mut p.stripped);
    strip_paths(&mut value, &mut stripped);
    if let Some(info) = value.get_mut("package").and_then(|v| v.as_object_mut()) {
        info.insert("paths_removed".into(), serde_json::Value::from(stripped.len()));
    }
    let json = serde_json::to_vec_pretty(&value).map_err(invalid)?;
    write_container(path, &json, &entries, &p.files)?;
    report.stripped = stripped;
    report.bytes = std::fs::metadata(path).map_or(0, |m| m.len());
    Ok(report)
}

/// An entry name that is safe to unpack: `data/<n>/<parts…>`, each part a plain name. Returns the
/// parts after `data/<n>/`.
fn safe_parts(entry: &str) -> Option<PathBuf> {
    let mut it = entry.split('/');
    if it.next()? != DATA_DIR || !it.next()?.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut out = PathBuf::new();
    for part in it {
        let ok = !part.is_empty()
            && part != "."
            && part != ".."
            && !part.contains(['\\', ':', '\0'])
            && Path::new(part).components().all(|c| matches!(c, Component::Normal(_)));
        if !ok {
            return None;
        }
        out.push(part);
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Copies one entry to `target` (through a temporary file beside it) unless a file of its size is
/// already there.
fn unpack_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, entry: &str, target: &Path) -> std::result::Result<(), String> {
    let f = zip.by_name(entry).map_err(|e| format!("{entry}: {e}"))?;
    let size = f.size();
    if size > MAX_FILE_BYTES {
        return Err(format!("{entry} is too large"));
    }
    if std::fs::metadata(target).is_ok_and(|m| m.is_file() && m.len() == size) {
        return Ok(());
    }
    let dir = target.parent().ok_or("no folder")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = target.with_file_name(format!(
        ".{}.part-{}",
        target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        std::process::id()
    ));
    let copied = (|| -> std::io::Result<u64> {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        let n = std::io::copy(&mut f.take(size + 1), &mut out)?;
        std::io::Write::flush(&mut out)?;
        Ok(n)
    })();
    match copied {
        Ok(n) if n == size => std::fs::rename(&tmp, target).map_err(|e| e.to_string()),
        Ok(_) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("{entry} is damaged"))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e.to_string())
        }
    }
}

/// Unpacks a package's files into [`unpack_dir`] and points the layers at them. Returns notes
/// for files that couldn't be unpacked (their layers then show the missing-source card).
pub(crate) fn unpack<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    manifest: &[u8],
    sources: &[SourceEntry],
    doc: &mut Document,
) -> Vec<String> {
    let hash: String = Sha256::digest(manifest).iter().take(8).map(|b| format!("{b:02x}")).collect();
    let root = unpack_dir().join(hash);
    let mut notes = Vec::new();
    for s in sources.iter().filter(|s| !s.embedded.is_empty()) {
        let name = base_name(&s.path);
        let mut first: Option<PathBuf> = None;
        let mut failed = None;
        for (k, entry) in s.embedded.iter().enumerate() {
            let Some(parts) = safe_parts(entry) else {
                failed = Some(format!("{entry} isn't a valid name"));
                continue;
            };
            let num = entry.split('/').nth(1).unwrap_or("0");
            let target = root.join(num).join(&parts);
            if let Err(e) = unpack_entry(zip, entry, &target) {
                failed = Some(e);
                continue;
            }
            if k == 0 {
                first = Some(target);
            }
        }
        // A folder source: the layers point at its top folder.
        let main = match (safe_parts(&s.path), s.path.split('/').nth(1)) {
            (Some(parts), Some(num)) if root.join(num).join(&parts).is_dir() => Some(root.join(num).join(parts)),
            _ => first,
        };
        if let Some(why) = failed {
            notes.push(format!("{name} could not be unpacked from this package: {why}"));
        }
        let Some(main) = main else { continue };
        // The recorded modification time, so the file reads as unchanged.
        if s.mtime > 0
            && main.is_file()
            && let Ok(f) = std::fs::File::options().write(true).open(&main)
        {
            let _ = f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s.mtime.unsigned_abs()));
        }
        let shown = std::path::absolute(&main).unwrap_or(main).display().to_string();
        for l in doc.layers.iter_mut().filter(|l| l.profile.source.path == s.path || s.layers.contains(&l.id)) {
            l.profile.source.path = shown.clone();
        }
    }
    notes
}

#[cfg(test)]
mod tests;
