//! The data library (WP5.4): a catalog of course datasets hosted next to the basemap, a download
//! cache, and the background work behind the library panel. Format: `docs/lm/library-catalog.md`.
//!
//! - [`parse`] reads a catalog and checks every entry; a bad entry is skipped, not fatal.
//! - [`load_catalog`] reads the catalog off the interface thread, keeps a copy in the cache (so the
//!   library still opens without a network), and then fetches the thumbnails.
//! - [`download`] fetches a dataset's files into the cache with progress and Cancel, checks each
//!   file's SHA-256, and hands back the file to open through the normal import pipeline. A dataset
//!   already in the cache opens without the network.
//! - Every network request goes through one function, [`open_url`], so the strict offline switch
//!   can gate the whole library in one place.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::jobs::Wake;

#[cfg(test)]
mod tests;

/// The `format` every catalog names.
pub const FORMAT: &str = "layeredmap-library";
/// The catalog version this build reads. A newer version is refused with a plain message.
pub const VERSION: u32 = 1;
/// The production catalog, once the owner has chosen the host (Gate 0 / WP5.7). Until then the
/// app uses `LM_LIBRARY_URL`, or the local test catalog in developer builds.
pub const DEFAULT_CATALOG_URL: Option<&str> = None;
/// Thumbnails larger than this are ignored (they are small PNGs).
const MAX_THUMBNAIL: u64 = 2 * 1024 * 1024;
/// Catalogs larger than this are refused.
const MAX_CATALOG: u64 = 4 * 1024 * 1024;
/// Bytes read between progress reports and cancel checks.
const CHUNK: usize = 64 * 1024;

/// English and Chinese text (both required: the library is for users in China too).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub en: String,
    pub zh: String,
}

impl Text {
    /// The text for a language code ("zh", "zh-Hans", "en-GB"…): Chinese for any `zh`, else English.
    pub fn get(&self, lang: &str) -> &str {
        if lang.starts_with("zh") { &self.zh } else { &self.en }
    }
}

/// What a dataset holds, as the panel describes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Points,
    Lines,
    Areas,
    Raster,
    Table,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Points => "Points",
            Kind::Lines => "Lines",
            Kind::Areas => "Areas",
            Kind::Raster => "Grid",
            Kind::Table => "Table",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct License {
    /// SPDX identifier where one exists (`CC-BY-4.0`, `ODbL-1.0`), else a short id
    /// (`public-domain`).
    pub id: String,
    /// What the card shows ("CC BY 4.0", "Public domain").
    pub name: String,
    /// The licence page.
    pub url: String,
}

/// One file of a dataset. A Shapefile lists each of its parts; the first file is the one opened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataFile {
    /// The file's name in the cache (no folders).
    pub name: String,
    /// Where to download it, tried in order (a China-reachable mirror can come first).
    pub urls: Vec<String>,
    /// Bytes.
    pub size: u64,
    /// Lowercase hexadecimal SHA-256 of the file.
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub id: String,
    pub title: Text,
    pub description: Text,
    pub kind: Kind,
    /// Longitude/latitude bounds: west, south, east, north.
    pub extent: [f64; 4],
    #[serde(alias = "licence")]
    pub license: License,
    /// The provider's page for the data.
    pub source: String,
    /// How to cite it, as the provider asks.
    pub citation: String,
    /// The data's date or version ("2022", "2024-06", "v5.1.2").
    pub date: String,
    /// The credit to show on the map and exports, when the licence asks for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    /// A small PNG (3:2, about 400 × 266).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    pub files: Vec<DataFile>,
}

impl Dataset {
    /// All files' bytes together.
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Whether every word of `query` appears in the titles, description, kind or licence.
    pub fn matches(&self, query: &str) -> bool {
        let hay = format!(
            "{} {} {} {} {} {} {}",
            self.title.en,
            self.title.zh,
            self.description.en,
            self.description.zh,
            self.kind.label(),
            self.license.name,
            self.id
        )
        .to_lowercase();
        query.to_lowercase().split_whitespace().all(|w| hay.contains(w))
    }
}

/// A checked catalog.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Catalog {
    pub updated: Option<String>,
    pub datasets: Vec<Dataset>,
    /// Entries left out, with why (for the log).
    pub skipped: Vec<String>,
}

impl Catalog {
    pub fn get(&self, id: &str) -> Option<&Dataset> {
        self.datasets.iter().find(|d| d.id == id)
    }
}

/// Why something in the library didn't work, in words for the user.
#[derive(Clone, Debug, PartialEq)]
pub enum LibraryError {
    /// The strict offline switch is on.
    Offline,
    /// The host could not be reached (the detail goes to the log).
    Network(String),
    /// The host answered that the file isn't there.
    NotFound(String),
    /// A downloaded file didn't match its SHA-256.
    Checksum(String),
    /// A download was larger than the catalog says.
    TooBig(String),
    /// The catalog could not be read.
    BadCatalog(String),
    /// The cache could not be written.
    Disk(String),
    Cancelled,
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibraryError::Offline => write!(f, "LayeredMap is set to work offline, so the data library can't download anything."),
            LibraryError::Network(_) => write!(f, "Couldn't reach the data library. Check your internet connection and try again."),
            LibraryError::NotFound(name) => write!(f, "The data library no longer has {name}. Try again later."),
            LibraryError::Checksum(name) => write!(
                f,
                "The downloaded copy of {name} didn't match the library's checksum, so it wasn't opened. Try again; if it happens again, the library's copy may be damaged."
            ),
            LibraryError::TooBig(name) => write!(f, "{name} was larger than the library says, so the download was stopped."),
            LibraryError::BadCatalog(_) => write!(f, "The data library's list couldn't be read. Try again later."),
            LibraryError::Disk(e) => write!(f, "Couldn't save the download on this Mac: {e}"),
            LibraryError::Cancelled => write!(f, "Download cancelled."),
        }
    }
}

impl LibraryError {
    /// The technical detail, for the log.
    pub fn details(&self) -> String {
        match self {
            LibraryError::Network(d) | LibraryError::BadCatalog(d) => d.clone(),
            other => other.to_string(),
        }
    }
}

fn disk(e: std::io::Error) -> LibraryError {
    LibraryError::Disk(e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Parsing

/// Reads a catalog. `base` is the catalog's own URL: relative file and thumbnail URLs are
/// resolved against it. A catalog of another format, or a newer version, is refused; a single
/// bad entry is skipped (listed in [`Catalog::skipped`]).
pub fn parse(json: &str, base: &str) -> Result<Catalog, LibraryError> {
    #[derive(Deserialize)]
    struct Raw {
        format: String,
        version: u32,
        #[serde(default)]
        updated: Option<String>,
        datasets: Vec<serde_json::Value>,
    }
    let raw: Raw = serde_json::from_str(json).map_err(|e| LibraryError::BadCatalog(e.to_string()))?;
    if raw.format != FORMAT {
        return Err(LibraryError::BadCatalog(format!("format is {:?}, not {FORMAT:?}", raw.format)));
    }
    if raw.version > VERSION {
        return Err(LibraryError::BadCatalog(format!("version {} needs a newer LayeredMap", raw.version)));
    }
    let mut out = Catalog { updated: raw.updated, ..Catalog::default() };
    let mut ids = HashSet::new();
    for (i, v) in raw.datasets.into_iter().enumerate() {
        let label = v.get("id").and_then(|s| s.as_str()).map_or_else(|| format!("entry {}", i + 1), str::to_string);
        let checked = serde_json::from_value::<Dataset>(v).map_err(|e| e.to_string()).and_then(|d| check(d, base));
        match checked {
            Ok(d) if !ids.insert(d.id.clone()) => out.skipped.push(format!("{label}: the id is used twice")),
            Ok(d) => out.datasets.push(d),
            Err(e) => out.skipped.push(format!("{label}: {e}")),
        }
    }
    for s in &out.skipped {
        log::warn!("data library: skipped {s}");
    }
    Ok(out)
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A file name that stays inside its cache folder.
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && !name.ends_with(".part")
        && !name.ends_with(".sha256")
        && !name.contains(['/', '\\', ':', '\0'])
}

fn check(mut d: Dataset, base: &str) -> Result<Dataset, String> {
    if !safe_id(&d.id) {
        return Err("the id must be 1–64 lowercase letters, digits or hyphens".into());
    }
    for (what, t) in [("title", &d.title), ("description", &d.description)] {
        if t.en.trim().is_empty() || t.zh.trim().is_empty() {
            return Err(format!("the {what} needs both English and Chinese"));
        }
    }
    let [w, s, e, n] = d.extent;
    if !(d.extent.iter().all(|v| v.is_finite()) && w <= e && s <= n && (-180.0..=180.0).contains(&w) && (-90.0..=90.0).contains(&s)) {
        return Err("the extent must be west, south, east, north in degrees".into());
    }
    if d.license.id.trim().is_empty() || d.license.name.trim().is_empty() || d.license.url.trim().is_empty() {
        return Err("the licence needs an id, a name and a URL".into());
    }
    if d.citation.trim().is_empty() || d.source.trim().is_empty() {
        return Err("a source and a citation are required".into());
    }
    if d.files.is_empty() {
        return Err("no files".into());
    }
    let mut names = HashSet::new();
    for f in &mut d.files {
        if !safe_name(&f.name) {
            return Err(format!("{:?} is not a plain file name", f.name));
        }
        if !names.insert(f.name.to_lowercase()) {
            return Err(format!("{} is listed twice", f.name));
        }
        f.sha256 = f.sha256.to_ascii_lowercase();
        if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{}: the SHA-256 must be 64 hexadecimal digits", f.name));
        }
        if f.urls.is_empty() {
            return Err(format!("{}: no download URL", f.name));
        }
        for u in &mut f.urls {
            *u = resolve(base, u);
        }
    }
    d.thumbnail = d.thumbnail.as_deref().map(|t| resolve(base, t));
    Ok(d)
}

/// `url` as an absolute URL: as written when it has a scheme, else relative to `base` (the
/// catalog's URL; `..` steps up a folder).
pub fn resolve(base: &str, url: &str) -> String {
    if url.contains("://") {
        return url.to_string();
    }
    let (root, path) = match base.find("://") {
        Some(i) => {
            let after = &base[i + 3..];
            let host_end = after.find('/').map_or(base.len(), |j| i + 3 + j);
            (&base[..host_end], &base[host_end..])
        }
        None => ("", base),
    };
    let mut parts: Vec<&str> = if url.starts_with('/') {
        Vec::new()
    } else {
        let dir = path.rfind('/').map_or("", |j| &path[..j]);
        dir.split('/').filter(|p| !p.is_empty()).collect()
    };
    for p in url.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    format!("{root}/{}", parts.join("/"))
}

// ---------------------------------------------------------------------------------------------
// The network

/// A body being read, with its length when the host gave one.
pub struct Body {
    pub reader: Box<dyn Read + Send>,
    pub len: Option<u64>,
}

/// The local path a `file://` URL (or a plain path) names, if it is one.
pub fn local_path(url: &str) -> Option<PathBuf> {
    if let Some(p) = url.strip_prefix("file://") {
        // file:///Users/… (empty host) or file://localhost/Users/…
        let p = p.strip_prefix("localhost").unwrap_or(p);
        return Some(PathBuf::from(percent_decode(p)));
    }
    (!url.contains("://")).then(|| PathBuf::from(url))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// **The only function in the library that reaches the network** (catalog, thumbnails and
/// datasets all come through here), so the strict offline switch gates the library at this one
/// place. `file://` URLs and plain paths are read from disk (the local test catalog, a catalog on
/// a USB stick) and are never gated.
pub fn open_url(url: &str) -> Result<Body, LibraryError> {
    if let Some(path) = local_path(url) {
        let file = std::fs::File::open(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => LibraryError::NotFound(file_label(url)),
            _ => LibraryError::Network(format!("{}: {e}", path.display())),
        })?;
        let len = file.metadata().ok().map(|m| m.len());
        return Ok(Body { reader: Box::new(file), len });
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(LibraryError::Network(format!("{url}: not an http, https or file URL")));
    }
    // The strict offline switch (WP6.6): every library request asks the one network gate.
    if lm_net::check(lm_net::Purpose::Library).is_err() {
        return Err(LibraryError::Offline);
    }
    http_get(url)
}

#[cfg(not(target_arch = "wasm32"))]
fn http_get(url: &str) -> Result<Body, LibraryError> {
    use std::time::Duration;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(20)))
        .user_agent(concat!("LayeredMap/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    match agent.get(url).call() {
        Ok(resp) => {
            let len = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
            Ok(Body { reader: Box::new(resp.into_body().into_reader()), len })
        }
        Err(ureq::Error::StatusCode(404 | 410)) => Err(LibraryError::NotFound(file_label(url))),
        Err(e) => Err(LibraryError::Network(format!("{url}: {e}"))),
    }
}

#[cfg(target_arch = "wasm32")]
fn http_get(url: &str) -> Result<Body, LibraryError> {
    Err(LibraryError::Network(format!("{url}: the web build has no data library")))
}

/// The last part of a URL, for messages.
fn file_label(url: &str) -> String {
    url.rsplit('/').find(|s| !s.is_empty()).unwrap_or(url).to_string()
}

fn read_all(url: &str, max: u64) -> Result<Vec<u8>, LibraryError> {
    let body = open_url(url)?;
    let mut out = Vec::new();
    body.reader.take(max + 1).read_to_end(&mut out).map_err(|e| LibraryError::Network(format!("{url}: {e}")))?;
    if out.len() as u64 > max {
        return Err(LibraryError::TooBig(file_label(url)));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// The cache

/// The library's cache: `~/Library/Caches/LayeredMap/library` on macOS,
/// `%LOCALAPPDATA%\LayeredMap\Cache\library` on Windows, the XDG cache elsewhere.
pub fn cache_dir() -> PathBuf {
    match lm_core::platform::current_user_dir(lm_core::platform::Folder::Caches) {
        Some(d) => d.join("library"),
        None => std::env::temp_dir().join("layeredmap-library"),
    }
}

fn short_hash(s: &str) -> String {
    let d = Sha256::digest(s.as_bytes());
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn catalog_copy(cache: &Path, url: &str) -> PathBuf {
    cache.join("catalogs").join(format!("{}.json", short_hash(url)))
}

fn thumbnail_copy(cache: &Path, url: &str) -> PathBuf {
    cache.join("thumbnails").join(format!("{}.png", short_hash(url)))
}

/// Where a dataset's files are kept.
pub fn dataset_dir(cache: &Path, id: &str) -> PathBuf {
    cache.join("datasets").join(id)
}

fn marker(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".sha256");
    PathBuf::from(s)
}

/// Whether the cache holds this file as verified: the right size, and the checksum recorded when
/// it was downloaded and checked matches the catalog's.
fn cached(path: &Path, f: &DataFile) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == f.size)
        && std::fs::read_to_string(marker(path)).is_ok_and(|s| s.trim() == f.sha256)
}

/// Whether every file of the dataset is in the cache (it opens without the network).
pub fn is_cached(cache: &Path, d: &Dataset) -> bool {
    let dir = dataset_dir(cache, &d.id);
    d.files.iter().all(|f| cached(&dir.join(&f.name), f))
}

/// Writes `bytes` to `path` through a temporary file, so a reader never sees half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

// ---------------------------------------------------------------------------------------------
// Downloading

/// Progress and cancel shared between a download and whoever watches it.
#[derive(Debug, Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub cancel: AtomicBool,
}

/// Fetches a dataset into the cache (skipping files already there and verified) and returns the
/// file to open. Runs on the caller's thread: [`download`] runs it in the background.
/// `tick` is called after each chunk.
pub fn fetch_dataset(d: &Dataset, cache: &Path, progress: &Progress, tick: &dyn Fn()) -> Result<PathBuf, LibraryError> {
    let dir = dataset_dir(cache, &d.id);
    std::fs::create_dir_all(&dir).map_err(disk)?;
    let mut before = 0;
    for f in &d.files {
        let path = dir.join(&f.name);
        if !cached(&path, f) {
            let mut last = None;
            for url in &f.urls {
                // Each attempt counts from the start of this file.
                progress.done.store(before, Ordering::Relaxed);
                match fetch_file(url, f, &path, progress, tick) {
                    Ok(()) => {
                        last = None;
                        break;
                    }
                    // Nothing a mirror can fix: stop at once.
                    Err(e @ (LibraryError::Cancelled | LibraryError::Disk(_) | LibraryError::Offline)) => return Err(e),
                    Err(e) => {
                        log::warn!("data library: {url}: {}", e.details());
                        last = Some(e);
                    }
                }
            }
            if let Some(e) = last {
                return Err(e);
            }
        }
        before += f.size;
        progress.done.store(before, Ordering::Relaxed);
        tick();
    }
    Ok(dir.join(&d.files[0].name))
}

/// Downloads one file to `<path>.part`, hashing as it goes, and moves it into place only when the
/// checksum matches. Whatever happens, no `.part` file is left behind.
fn fetch_file(url: &str, f: &DataFile, path: &Path, progress: &Progress, tick: &dyn Fn()) -> Result<(), LibraryError> {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let result = (|| {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(LibraryError::Cancelled);
        }
        let body = open_url(url)?;
        let mut reader = body.reader;
        let mut out = std::fs::File::create(&part).map_err(disk)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        let mut got: u64 = 0;
        loop {
            if progress.cancel.load(Ordering::Relaxed) {
                return Err(LibraryError::Cancelled);
            }
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(LibraryError::Network(format!("{url}: {e}"))),
            };
            got += n as u64;
            if got > f.size {
                return Err(LibraryError::TooBig(f.name.clone()));
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(disk)?;
            progress.done.fetch_add(n as u64, Ordering::Relaxed);
            tick();
        }
        out.flush().map_err(disk)?;
        drop(out);
        let sum: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if sum != f.sha256 {
            log::warn!("data library: {} from {url} has SHA-256 {sum}, the catalog says {}", f.name, f.sha256);
            return Err(LibraryError::Checksum(f.name.clone()));
        }
        // The marker is removed first and written last, so a crash in between leaves the file
        // unverified (downloaded again), never trusted with the wrong contents.
        let _ = std::fs::remove_file(marker(path));
        std::fs::rename(&part, path).map_err(disk)?;
        write_atomic(&marker(path), f.sha256.as_bytes()).map_err(disk)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// A dataset download running in the background. Dropping it cancels the download.
pub struct Download {
    pub id: String,
    pub total: u64,
    progress: Arc<Progress>,
    rx: Receiver<Result<PathBuf, LibraryError>>,
    outcome_taken: bool,
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download").field("id", &self.id).field("total", &self.total).finish()
    }
}

impl Download {
    /// Bytes done so far.
    pub fn done(&self) -> u64 {
        self.progress.done.load(Ordering::Relaxed).min(self.total)
    }

    /// Done as a fraction, 0–1.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 { 0.0 } else { (self.done() as f64 / self.total as f64) as f32 }
    }

    /// Stops the download at its next chunk; its partial file is removed. From now on
    /// [`Download::poll`] gives `Cancelled`.
    pub fn cancel(&self) {
        self.progress.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.progress.cancel.load(Ordering::Relaxed)
    }

    /// The outcome once, when the download has ended: the file to open, or why not. Never blocks.
    pub fn poll(&mut self) -> Option<Result<PathBuf, LibraryError>> {
        if self.outcome_taken {
            return None;
        }
        let r = match self.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) if self.is_cancelled() => Err(LibraryError::Cancelled),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(LibraryError::Disk("the download stopped unexpectedly".into())),
        };
        self.outcome_taken = true;
        Some(if self.is_cancelled() { Err(LibraryError::Cancelled) } else { r })
    }

    /// Waits for the outcome (tests; never on the interface thread).
    pub fn wait(&mut self, timeout: std::time::Duration) -> Option<Result<PathBuf, LibraryError>> {
        match self.rx.recv_timeout(timeout) {
            Ok(r) => {
                self.outcome_taken = true;
                Some(if self.is_cancelled() { Err(LibraryError::Cancelled) } else { r })
            }
            Err(_) => None,
        }
    }
}

impl Drop for Download {
    fn drop(&mut self) {
        if !self.outcome_taken {
            self.cancel();
        }
    }
}

/// Starts fetching a dataset into `cache` on a background thread; `wake` is called on progress
/// (at most every 50 ms) and when it ends.
pub fn download(d: &Dataset, cache: PathBuf, wake: Wake) -> Download {
    let (tx, rx) = channel();
    let progress = Arc::new(Progress::default());
    let job = Download { id: d.id.clone(), total: d.size(), progress: progress.clone(), rx, outcome_taken: false };
    let d = d.clone();
    let spawned = std::thread::Builder::new().name("lm-library-download".into()).spawn(move || {
        let last = std::sync::Mutex::new(std::time::Instant::now());
        let tick = || {
            let mut l = last.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if l.elapsed() >= std::time::Duration::from_millis(50) {
                *l = std::time::Instant::now();
                wake();
            }
        };
        let r = fetch_dataset(&d, &cache, &progress, &tick);
        match &r {
            Ok(p) => log::info!("data library: {} ready at {}", d.id, p.display()),
            Err(e) => log::warn!("data library: {}: {}", d.id, e.details()),
        }
        let _ = tx.send(r);
        wake();
    });
    if let Err(e) = spawned {
        log::error!("could not start the library download: {e}");
    }
    job
}

// ---------------------------------------------------------------------------------------------
// The catalog in the background

/// News from [`load_catalog`].
#[derive(Debug)]
pub enum CatalogEvent {
    /// The catalog, with the ids of the datasets already in the cache. `saved_copy` is true when
    /// the network failed and the copy kept from an earlier visit is shown.
    Loaded { catalog: Catalog, saved_copy: bool, cached: HashSet<String> },
    /// No catalog: neither the host nor a saved copy.
    Failed(LibraryError),
    /// A dataset's thumbnail (PNG bytes).
    Thumbnail { id: String, png: Vec<u8> },
}

/// The catalog being read. Dropping it stops fetching thumbnails.
pub struct CatalogJob {
    rx: Receiver<CatalogEvent>,
    stop: Arc<AtomicBool>,
}

impl CatalogJob {
    /// The events since the last poll. Never blocks.
    pub fn poll(&self) -> Vec<CatalogEvent> {
        self.rx.try_iter().collect()
    }

    /// Waits for the next event (tests).
    pub fn wait(&self, timeout: std::time::Duration) -> Option<CatalogEvent> {
        self.rx.recv_timeout(timeout).ok()
    }
}

impl Drop for CatalogJob {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Reads the catalog at `url` on a background thread: from the host (saving a copy in `cache`),
/// or from the saved copy when the host can't be reached; then each thumbnail, from the cache or
/// the host.
pub fn load_catalog(url: String, cache: PathBuf, wake: Wake) -> CatalogJob {
    let (tx, rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let job = CatalogJob { rx, stop: stop.clone() };
    let spawned = std::thread::Builder::new().name("lm-library-catalog".into()).spawn(move || {
        let send = |e: CatalogEvent| {
            let _ = tx.send(e);
            wake();
        };
        let (catalog, saved_copy) = match read_catalog(&url, &cache) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("data library: {url}: {}", e.details());
                send(CatalogEvent::Failed(e));
                return;
            }
        };
        let cached_ids = catalog.datasets.iter().filter(|d| is_cached(&cache, d)).map(|d| d.id.clone()).collect();
        let thumbs: Vec<(String, String)> = catalog.datasets.iter().filter_map(|d| Some((d.id.clone(), d.thumbnail.clone()?))).collect();
        send(CatalogEvent::Loaded { catalog, saved_copy, cached: cached_ids });
        for (id, t) in thumbs {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let copy = thumbnail_copy(&cache, &t);
            let png = match std::fs::read(&copy) {
                Ok(b) => Ok(b),
                Err(_) => read_all(&t, MAX_THUMBNAIL).inspect(|b| {
                    if let Err(e) = write_atomic(&copy, b) {
                        log::warn!("data library: could not keep the thumbnail {}: {e}", copy.display());
                    }
                }),
            };
            match png {
                Ok(png) => send(CatalogEvent::Thumbnail { id, png }),
                Err(e) => log::warn!("data library: thumbnail {t}: {}", e.details()),
            }
        }
    });
    if let Err(e) = spawned {
        log::error!("could not start reading the data library: {e}");
    }
    job
}

/// The catalog from the host (a copy is saved), else the saved copy.
fn read_catalog(url: &str, cache: &Path) -> Result<(Catalog, bool), LibraryError> {
    let copy = catalog_copy(cache, url);
    let fresh = read_all(url, MAX_CATALOG).and_then(|b| {
        let text = String::from_utf8(b).map_err(|e| LibraryError::BadCatalog(e.to_string()))?;
        let c = parse(&text, url)?;
        if let Err(e) = write_atomic(&copy, text.as_bytes()) {
            log::warn!("data library: could not keep a copy of the catalog: {e}");
        }
        Ok(c)
    });
    match fresh {
        Ok(c) => Ok((c, false)),
        Err(e) => match std::fs::read_to_string(&copy).ok().and_then(|t| parse(&t, url).ok()) {
            Some(c) => {
                log::info!("data library: showing the saved catalog ({})", e.details());
                Ok((c, true))
            }
            None => Err(e),
        },
    }
}
