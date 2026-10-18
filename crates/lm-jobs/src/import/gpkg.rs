//! GeoPackages: each feature table becomes a layer, and the layers of one file form a group
//! (each layer's `source.member` is its table).
//!
//! **Read-only by construction.** The file is read into memory with an ordinary read-only open
//! and decoded with the upstream pure-Rust SQLite reader (`rustgis_data::sqlite`, through
//! `source::gpkg_mem`); SQLite itself never opens it, so no `-wal`, `-shm` or `-journal` file can
//! appear beside it and nothing can write to it. The one case that reader can't see is changes
//! still held in a sidecar (a non-empty `-wal`, or a hot `-journal`, left by a program that has the
//! file open or crashed): then the file and that sidecar are copied into a private temporary
//! folder, SQLite settles the copy there (checkpoint or rollback), and the copy is read. The
//! sidecars beside the source are only ever read.

use std::io::Read;
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;

use lm_core::model::{Assumption, Crs, Exclusion, LayerProfile};
use rustgis_engine::data::raster::{Crs as RCrs, wkt::crs_from_wkt};
use rustgis_engine::data::source::gpkg_mem::{self, GpkgMemSource};
use rustgis_engine::data::source::{FeatureSource, MAX_LIMIT, Request, Shared};
use rustgis_engine::data::sqlite::{Db, Table, Val};
use rustgis_engine::data::{Dataset, Feature};
use rustgis_engine::geom::Coord;

use super::limits::GEOPACKAGE_MAX_BYTES;
use super::progress::Counting;
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, Stage, extension, file_name, status_of, vector_profile};

pub struct GeoPackage;

impl FormatAdapter for GeoPackage {
    fn name(&self) -> &'static str {
        "GeoPackage"
    }

    fn sniff(&self, path: &Path, _head: &[u8]) -> bool {
        extension(path) == "gpkg"
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        if m.len() > GEOPACKAGE_MAX_BYTES {
            return Err(ImportError::Refused(format!(
                "{} is larger than 2 GB, the most LayeredMap reads as a GeoPackage for now. Your file has not been changed.",
                file_name(path)
            )));
        }
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        let bytes = Arc::new(database_bytes(path, meta.size, ctx)?);
        let tables = gpkg_mem::layers(Shared(bytes.clone())).map_err(|e| ImportError::read(&name, e))?;
        if tables.is_empty() {
            return Err(ImportError::Unsupported(format!(
                "{name} holds no layers with shapes (only tables or tiles), which LayeredMap doesn't open yet. Your file has not been changed."
            )));
        }
        let catalog = Catalog::read(&bytes).map_err(|e| ImportError::read(&name, e))?;
        ctx.stage(Stage::Extent)?;
        let mut out = Vec::new();
        for table in &tables {
            ctx.ticket.check()?;
            out.extend(read_table(path, &bytes, &catalog, table, ctx)?);
        }
        ctx.stage(Stage::Profile)?;
        Ok(out)
    }
}

/// The database's bytes: the file as it is, or, when a sidecar holds changes not yet in it, a
/// settled copy (see the module comment).
fn database_bytes(path: &Path, size: u64, ctx: &Ctx) -> Result<Vec<u8>, ImportError> {
    let name = file_name(path);
    ctx.ticket.set_total(size);
    let file = std::fs::File::open(path).map_err(|e| ImportError::read(&name, e))?;
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    Counting::new(file, ctx.ticket.clone())
        .take(GEOPACKAGE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ImportError::read(&name, e))?;
    ctx.ticket.check()?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let pending: Vec<(&str, std::path::PathBuf)> = ["-wal", "-journal"]
            .into_iter()
            .map(|s| (s, sidecar(path, s)))
            .filter(|(_, p)| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0))
            .collect();
        if !pending.is_empty() {
            log::info!("{name}: changes held in {:?}; reading a settled copy", pending.iter().map(|(s, _)| *s).collect::<Vec<_>>());
            return settled::copy(&bytes, &pending).map_err(|e| ImportError::read(&name, e));
        }
    }
    Ok(bytes)
}

/// The sidecar SQLite keeps beside a database: `<file>-wal`, `<file>-shm` or `<file>-journal`.
pub(super) fn sidecar(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    std::path::PathBuf::from(s)
}

#[cfg(not(target_arch = "wasm32"))]
mod settled {
    //! A copy of a database with its pending sidecar applied, made in a private temporary folder
    //! that is removed afterwards.

    use std::path::{Path, PathBuf};

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir() -> std::io::Result<TempDir> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let mut tries = 0;
        loop {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            let salt = std::collections::hash_map::RandomState::new();
            let tag = std::hash::BuildHasher::hash_one(&salt, (nanos, N.fetch_add(1, Ordering::Relaxed)));
            let p = std::env::temp_dir().join(format!("layeredmap-gpkg-{}-{tag:016x}", std::process::id()));
            let mut b = std::fs::DirBuilder::new();
            // Readable by this user only: the copy holds their data.
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
            match b.create(&p) {
                Ok(()) => return Ok(TempDir(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && tries < 8 => tries += 1,
                Err(e) => return Err(e),
            }
        }
    }

    /// Writes `db` and copies of the `pending` sidecars into a private folder, lets SQLite apply
    /// them there (a write-ahead log is checkpointed, a hot journal rolled back), and returns the
    /// settled database's bytes.
    pub(super) fn copy(db: &[u8], pending: &[(&str, PathBuf)]) -> Result<Vec<u8>, String> {
        let dir = temp_dir().map_err(|e| e.to_string())?;
        let copy = dir.0.join("data.gpkg");
        std::fs::write(&copy, db).map_err(|e| e.to_string())?;
        for (suffix, src) in pending {
            let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
            std::fs::write(super::sidecar(&copy, suffix), bytes).map_err(|e| e.to_string())?;
        }
        settle(&copy).map_err(|e| e.to_string())?;
        std::fs::read(&copy).map_err(|e| e.to_string())
    }

    fn settle(copy: &Path) -> rusqlite::Result<()> {
        use rusqlite::{Connection, OpenFlags, config::DbConfig};
        let conn = Connection::open_with_flags(copy, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        // The file is hostile: nothing it defines may call functions.
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
        // The first read rolls back a hot journal; leaving WAL mode checkpoints the log.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))?;
        conn.query_row("PRAGMA journal_mode = DELETE", [], |r| r.get::<_, String>(0))?;
        conn.close().map_err(|(_, e)| e)
    }
}

/// What the GeoPackage's own tables say about each feature table: its geometry column and its
/// declared coordinate system.
struct Catalog {
    schema: Vec<Table>,
    columns: Vec<Vec<(String, Val)>>,
    srs: Vec<Vec<(String, Val)>>,
}

fn get<'a>(row: &'a [(String, Val)], col: &str) -> Option<&'a Val> {
    row.iter().find(|(c, _)| c == col).map(|(_, v)| v)
}

fn text(v: Option<&Val>) -> Option<String> {
    match v? {
        Val::Text(s) => Some(s.clone()),
        _ => None,
    }
}

fn int(v: Option<&Val>) -> Option<i64> {
    match v? {
        Val::Int(i) => Some(*i),
        Val::Real(x) if x.fract() == 0.0 && x.abs() < 9e15 => Some(*x as i64),
        _ => None,
    }
}

/// A table's declared coordinate system.
#[derive(Debug, Default, PartialEq)]
struct Declared {
    srs_id: i64,
    name: Option<String>,
    /// The EPSG code, decided as the upstream reader decides it.
    epsg: Option<u32>,
    definition: Option<String>,
}

impl Catalog {
    fn read(bytes: &Arc<Vec<u8>>) -> Result<Catalog, String> {
        let db = Db::open(Shared(bytes.clone()))?;
        let schema = db.tables()?;
        let rows = |name: &str| -> Result<Vec<Vec<(String, Val)>>, String> {
            let Some(t) = schema.iter().find(|t| t.name.eq_ignore_ascii_case(name)) else { return Ok(Vec::new()) };
            let mut out = Vec::new();
            db.rows(t, |_, r| {
                out.push(t.columns.iter().map(|c| c.name.to_ascii_lowercase()).zip(r.iter().cloned()).collect());
                if out.len() > 100_000 { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
            })?;
            Ok(out)
        };
        let columns = rows("gpkg_geometry_columns")?;
        let srs = rows("gpkg_spatial_ref_sys")?;
        Ok(Catalog { schema, columns, srs })
    }

    fn geometry_column(&self, table: &str) -> Option<String> {
        let row = self.columns.iter().find(|r| text(get(r, "table_name")).is_some_and(|t| t.eq_ignore_ascii_case(table)))?;
        text(get(row, "column_name"))
    }

    fn declared(&self, table: &str) -> Declared {
        let Some(gc) = self.columns.iter().find(|r| text(get(r, "table_name")).is_some_and(|t| t.eq_ignore_ascii_case(table))) else {
            return Declared::default();
        };
        let srs_id = int(get(gc, "srs_id")).unwrap_or(0);
        let row = self.srs.iter().find(|r| int(get(r, "srs_id")) == Some(srs_id));
        let epsg = match row {
            None => u32::try_from(srs_id).ok().filter(|c| matches!(c, 4326 | 3857)),
            Some(r) => match (text(get(r, "organization")), int(get(r, "organization_coordsys_id"))) {
                (Some(org), Some(id)) if org.eq_ignore_ascii_case("epsg") => u32::try_from(id).ok().filter(|c| *c > 0),
                _ => None,
            },
        };
        Declared {
            srs_id,
            name: row.and_then(|r| text(get(r, "srs_name"))),
            epsg,
            definition: row.and_then(|r| text(get(r, "definition"))).filter(|d| !d.trim().eq_ignore_ascii_case("undefined")),
        }
    }

    /// Every row of a table in order: its rowid, and whether it has a shape.
    fn rows(&self, bytes: &Arc<Vec<u8>>, table: &str, geom: &str) -> Result<Vec<(i64, bool)>, String> {
        let db = Db::open(Shared(bytes.clone()))?;
        let t = self.schema.iter().find(|t| t.name.eq_ignore_ascii_case(table)).ok_or_else(|| format!("no table {table}"))?;
        let col = t.columns.iter().position(|c| c.name.eq_ignore_ascii_case(geom));
        let mut out = Vec::new();
        db.rows(t, |rowid, r| {
            let shaped = match col.and_then(|c| r.get(c)) {
                // A blob flagged empty is a valid row with nothing to draw.
                Some(Val::Blob(b)) => !(b.starts_with(b"GP") && b.get(3).is_some_and(|f| f & 0b1_0000 != 0)),
                _ => false,
            };
            out.push((rowid, shaped));
            ControlFlow::Continue(())
        })?;
        Ok(out)
    }
}

/// How a table's coordinates reach longitude/latitude.
enum Placement {
    /// The upstream reader converted them from the declared EPSG code.
    Converted(String),
    /// The declared definition is converted here (an organization other than EPSG).
    FromDefinition(RCrs, String),
    /// Kept as stored: the values decide (no system declared, or one LayeredMap doesn't know).
    Raw { unknown: bool },
}

fn placement(d: &Declared) -> Placement {
    let label = |code: Option<u32>| match (&d.name, code) {
        (Some(n), Some(c)) => format!("{n} (EPSG:{c})"),
        (Some(n), None) => n.clone(),
        (None, Some(c)) => format!("EPSG:{c}"),
        (None, None) => format!("srs_id {}", d.srs_id),
    };
    // As upstream: a code it knows and can convert from is converted while reading.
    if let Some(c) = d.epsg
        && let Some(crs) = RCrs::from_epsg(c)
        && crs.transformer().is_ok()
    {
        return Placement::Converted(label(Some(c)));
    }
    // Anything else was read as stored; a definition this side understands converts it.
    if let Some(def) = &d.definition
        && let Ok(crs) = crs_from_wkt(def)
        && crs.transformer().is_ok()
    {
        return Placement::FromDefinition(crs, label(d.epsg));
    }
    Placement::Raw { unknown: d.srs_id > 0 || d.definition.is_some() }
}

/// Reads one feature table as one layer (or several, when it mixes points, lines and areas).
fn read_table(path: &Path, bytes: &Arc<Vec<u8>>, catalog: &Catalog, table: &str, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
    let name = file_name(path);
    let src = GpkgMemSource::open_shared(Shared(bytes.clone()), Some(table)).map_err(|e| ImportError::read(&name, e))?;
    let batch = src.read(&Request::all()).map_err(|e| ImportError::read(&name, e))?;
    ctx.ticket.check()?;
    if !batch.complete {
        return Err(ImportError::Refused(format!(
            "The layer {table} in {name} has more than {} features, the most LayeredMap reads from one GeoPackage layer for now. Your file has not been changed.",
            lm_core::model::group_thousands(MAX_LIMIT as u64)
        )));
    }
    let info = src.info();
    let declared = catalog.declared(table);
    let place = placement(&declared);
    let transformer = match &place {
        Placement::FromDefinition(crs, _) => crs.transformer().ok(),
        _ => None,
    };
    let mut by_id: std::collections::HashMap<u64, Feature> = batch.features.into_iter().map(|f| (f.id, f)).collect();
    let geom = catalog.geometry_column(table).unwrap_or_default();
    let rows = catalog.rows(bytes, table, &geom).map_err(|e| ImportError::read(&name, e))?;
    let mut ds = Dataset::new(table.to_string(), info.geometry_type, info.fields.clone());
    ds.source = Some(path.display().to_string());
    let mut exclusions = Vec::new();
    for (i, (rowid, shaped)) in rows.iter().enumerate() {
        let row = i as u64 + 1;
        let Some(f) = by_id.remove(&(*rowid as u64)) else {
            let reason = if *shaped { "Shape couldn't be read" } else { "No shape" };
            exclusions.push(Exclusion { row, reason: reason.into() });
            continue;
        };
        let geometry = match (&transformer, f.geometry) {
            (Some(t), Some(g)) => {
                let g = g.map_coords(&|c| t.to_lonlat(c).unwrap_or(Coord::new(f64::NAN, f64::NAN)));
                let mut ok = true;
                g.for_each_coord(|c| ok &= c.is_finite());
                if !ok {
                    exclusions.push(Exclusion { row, reason: "Shape couldn't be converted to GPS coordinates".into() });
                    continue;
                }
                Some(g)
            }
            (_, g) => g,
        };
        ds.push_next(geometry, f.attrs);
    }
    ctx.ticket.check()?;
    let mut notices = Vec::new();
    if let Placement::Raw { unknown: true } = place {
        notices
            .push("The GeoPackage names a coordinate system LayeredMap doesn't know, so the coordinates were read as stored.".to_string());
    }
    let declared_crs = match place {
        Placement::Converted(_) | Placement::FromDefinition(..) => Some(Crs::Wgs84),
        Placement::Raw { .. } => None,
    };
    let parts = super::split_by_kind(ds);
    let multi = parts.len() > 1;
    let kinds: Vec<_> = parts.iter().map(|(k, _)| *k).collect();
    let split = lm_profile::mixed_geometry_split(&kinds);
    let mut out = Vec::new();
    for (i, (_, part)) in parts.into_iter().enumerate() {
        let assumptions: Vec<Assumption> = split.iter().cloned().collect();
        // Excluded records are reported once, on the first layer.
        let excl = if i == 0 { exclusions.clone() } else { Vec::new() };
        let field_names: Vec<String> = part.fields.iter().map(|f| f.name.clone()).collect();
        // Split parts of one table need their own member, so each can be found again.
        let member = if multi {
            format!("{table} ({})", part.name.rsplit('(').next().unwrap_or("").trim_end_matches(')'))
        } else {
            table.to_string()
        };
        let mut profile: LayerProfile = vector_profile(
            path,
            Some(member),
            "GeoPackage",
            &part,
            declared_crs.clone(),
            assumptions,
            excl,
            None,
            notices.clone(),
            &field_names,
        );
        match &place {
            Placement::Converted(label) | Placement::FromDefinition(_, label) if label.contains("EPSG:4326") => {
                profile.crs.evidence = lm_core::model::Evidence(format!("Declared in the GeoPackage: {label}."));
            }
            Placement::Converted(label) | Placement::FromDefinition(_, label) => {
                profile.crs.evidence =
                    lm_core::model::Evidence(format!("Declared in the GeoPackage: {label}. Converted to GPS coordinates."));
            }
            Placement::Raw { .. } => {}
        }
        out.push(ImportedLayer {
            name: part.name.clone(),
            status: status_of(&profile),
            profile,
            data: LayerData::Vector(Arc::new(part)),
            raw: None,
            style: None,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_systems_are_placed_honestly() {
        let d = |srs_id, epsg, definition: Option<&str>| Declared { srs_id, name: None, epsg, definition: definition.map(str::to_string) };
        assert!(matches!(placement(&d(4326, Some(4326), None)), Placement::Converted(_)));
        assert!(matches!(placement(&d(3857, Some(3857), None)), Placement::Converted(_)));
        // Undefined geographic / cartesian: the values decide, with no notice.
        assert!(matches!(placement(&d(0, None, None)), Placement::Raw { unknown: false }));
        assert!(matches!(placement(&d(-1, None, None)), Placement::Raw { unknown: false }));
        // A system it can't read: kept as stored, said so.
        assert!(matches!(placement(&d(100_001, None, Some("nonsense"))), Placement::Raw { unknown: true }));
    }
}
