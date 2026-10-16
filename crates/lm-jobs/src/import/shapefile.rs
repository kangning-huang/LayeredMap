//! Shapefiles: the `.shp` with its `.shx`, `.dbf`, `.prj` and `.cpg` siblings, also from inside a
//! ZIP (checked against the archive limits first). A missing `.dbf` gives shapes only, with a
//! notice; a missing `.shx` is rebuilt by reading the shapes in order; the text encoding comes
//! from the `.cpg` or is detected (`lm_profile::shapefile_encoding`).

use std::path::Path;
use std::sync::Arc;

use lm_core::model::{Crs, LayerKind};
use rustgis_engine::data::io::{ShapefileOptions, ShapefileParts, read_shapefile_parts_with};

use super::limits::{check_zip, zip_entries};
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, Stage, extension, file_name, status_of, vector_profile};

pub struct Shapefile;

thread_local! {
    /// A text encoding chosen by the user (Change on the encoding line), used instead of the
    /// `.cpg` or detection while the file is read again on this thread.
    static ENCODING_OVERRIDE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn cpg_or_override(cpg: Option<String>) -> Option<String> {
    ENCODING_OVERRIDE.with(|o| o.borrow().clone()).or(cpg)
}

/// Reads a Shapefile layer again in the encoding the user chose (`gbk`, `utf8`, `windows1252`):
/// the same file and member, nothing else changed. The choice is recorded as confirmed.
pub fn reread_encoding(layer: &ImportedLayer, choice: &lm_core::model::Choice) -> Result<ImportedLayer, ImportError> {
    let new = read_in_encoding(&layer.profile.source, choice)?;
    // Every other choice the layer had (placement, answers) is applied again.
    let mut others = layer.profile.clone();
    others.assumptions.retain(|a| a.kind != lm_core::model::AssumptionKind::ShapefileGbk);
    let mut new = super::reapply_choices(new, &others)?;
    new.name = layer.name.clone();
    Ok(new)
}

/// The source read again in the chosen encoding, with the encoding line stating the choice.
pub fn read_in_encoding(source: &lm_core::model::SourceRef, choice: &lm_core::model::Choice) -> Result<ImportedLayer, ImportError> {
    let label = match choice.id.as_str() {
        "gbk" => "GBK",
        "utf8" => "UTF-8",
        "windows1252" => "1252",
        other => return Err(ImportError::Refused(format!("Unknown text encoding: {other}"))),
    };
    ENCODING_OVERRIDE.with(|o| *o.borrow_mut() = Some(label.to_string()));
    let read = super::fresh_read(source);
    ENCODING_OVERRIDE.with(|o| *o.borrow_mut() = None);
    let mut new = read?;
    let kind = lm_core::model::AssumptionKind::ShapefileGbk;
    match new.profile.assumptions.iter_mut().find(|a| a.kind == kind) {
        Some(a) => {
            a.chosen = choice.clone();
            a.confirmed_by_user = true;
            a.line = format!("Read text as {}.", choice.label);
            if !a.alternatives.contains(choice) {
                a.alternatives.insert(0, choice.clone());
            }
        }
        None => new.profile.assumptions.push(lm_core::model::Assumption {
            kind,
            subject: lm_core::model::Subject::Layer,
            chosen: choice.clone(),
            alternatives: vec![
                lm_core::model::Choice::new("gbk", "Chinese (GBK)"),
                lm_core::model::Choice::new("utf8", "Unicode (UTF-8)"),
                lm_core::model::Choice::new("windows1252", "Western (Windows-1252)"),
            ],
            confidence: lm_core::model::Confidence::Certain,
            consequence: lm_core::model::Consequence::DisplayOnly,
            treatment: lm_core::model::Treatment::Visible,
            line: format!("Read text as {}.", choice.label),
            confirmed_by_user: true,
        }),
    }
    new.profile.encoding = Some(match choice.id.as_str() {
        "gbk" => "GBK".into(),
        "utf8" => "UTF-8".into(),
        _ => "Windows-1252".into(),
    });
    Ok(new)
}

pub struct ZippedShapefile;

/// Formats that are ZIP archives inside but never a zipped Shapefile (office documents, KMZ,
/// e-books, Java and Android packages): the ZIP magic alone doesn't claim them, so they get the
/// plain "can't open .docx files yet" (or their own adapter, as an .xlsx does).
const NOT_SHAPEFILE_ZIPS: &[&str] =
    &["xlsx", "xlsm", "xltx", "docx", "docm", "dotx", "pptx", "pptm", "potx", "ods", "odt", "odp", "kmz", "epub", "jar", "apk", "ipa"];

/// The sibling of `path` with extension `ext`, matched without regard to case.
fn sibling(path: &Path, ext: &str) -> Option<std::path::PathBuf> {
    let stem = path.file_stem()?.to_str()?.to_lowercase();
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.to_lowercase() == stem)
            && p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext))
    })
}

/// Raw bytes of up to 400 records of a `.dbf`, for the encoding detector.
fn dbf_samples(dbf: &[u8]) -> Vec<&[u8]> {
    let u16_at = |at: usize| dbf.get(at..at + 2).map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])));
    let (Some(header), Some(record)) = (u16_at(8), u16_at(10)) else { return Vec::new() };
    if record == 0 {
        return Vec::new();
    }
    (0..400).filter_map(|i| dbf.get(header + i * record..header + (i + 1) * record)).collect()
}

/// Reads one Shapefile from its parts and profiles it.
#[allow(clippy::too_many_arguments)]
fn read_parts(
    path: &Path,
    member: Option<String>,
    name: &str,
    shp: Vec<u8>,
    shx: Option<Vec<u8>>,
    dbf: Option<Vec<u8>>,
    prj: Option<String>,
    cpg: Option<String>,
    ctx: &Ctx,
) -> Result<ImportedLayer, ImportError> {
    let (encoding, enc_assumption) = match &dbf {
        Some(d) => lm_profile::shapefile_encoding(&dbf_samples(d), cpg.as_deref()),
        None => lm_profile::shapefile_encoding(&[], cpg.as_deref()),
    };
    // A .prj that is understood converts to longitude/latitude; otherwise the coordinates stay as
    // stored and their values decide (degrees, or "Where should this data be?").
    let prj_ok = prj.as_deref().map(rustgis_engine::data::raster::wkt::read_wkt).and_then(Result::ok);
    let opts = ShapefileOptions { encoding: Some(encoding.value.encoding()), raw_coordinates: prj_ok.is_none() };
    let mut notices = Vec::new();
    if dbf.is_none() {
        notices.push("No .dbf file: the shapes were opened without attributes.".to_string());
    }
    if shx.is_none() {
        notices.push("No .shx file: the shapes were read in order instead.".to_string());
    }
    if prj.is_some() && prj_ok.is_none() {
        notices
            .push("The .prj file names a coordinate system LayeredMap doesn't know, so the coordinates were read as stored.".to_string());
    }
    let parts = ShapefileParts { shp, shx, dbf, prj: if prj_ok.is_some() { prj } else { None } };
    let (mut ds, report) = read_shapefile_parts_with(parts, name, opts).map_err(|e| ImportError::read(&file_name(path), e))?;
    ctx.ticket.check()?;
    ds.source = Some(path.display().to_string());
    ctx.stage(Stage::Profile)?;
    if report.skipped > 0 {
        notices.push(format!("{} shapes couldn't be read.", lm_core::model::group_thousands(report.skipped as u64)));
    }
    let declared = prj_ok.as_ref().map(|_| Crs::Wgs84);
    let field_names: Vec<String> = ds.fields.iter().map(|f| f.name.clone()).collect();
    let mut profile = vector_profile(
        path,
        member,
        "Shapefile",
        &ds,
        declared,
        enc_assumption.into_iter().collect(),
        Vec::new(),
        Some(encoding.value.label()),
        notices,
        &field_names,
    );
    if let Some((_, Some(prj_name))) = &prj_ok {
        profile.crs.evidence = lm_core::model::Evidence(format!("From the .prj file: {prj_name}. Converted to GPS coordinates."));
    }
    // Features without a shape are listed in the import report.
    profile.exclusions.extend(
        ds.features.iter().enumerate().filter(|(_, f)| f.geometry.is_none() && profile.kind != LayerKind::Table).map(|(i, _)| {
            lm_core::model::Exclusion {
                row: i as u64 + 1,
                reason: if report.unreadable.contains(&i) { "The shape couldn't be read".into() } else { "No shape".into() },
            }
        }),
    );
    let display = ds.name.clone();
    Ok(ImportedLayer { name: display, status: status_of(&profile), profile, data: LayerData::Vector(Arc::new(ds)), raw: None })
}

impl FormatAdapter for Shapefile {
    fn name(&self) -> &'static str {
        "Shapefile"
    }

    fn sniff(&self, path: &Path, _head: &[u8]) -> bool {
        extension(path) == "shp"
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, _meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        let read = |p: Option<std::path::PathBuf>| -> Result<Option<Vec<u8>>, ImportError> {
            match p {
                Some(p) => std::fs::read(&p).map(Some).map_err(|e| ImportError::read(&name, e)),
                None => Ok(None),
            }
        };
        let shp = std::fs::read(path).map_err(|e| ImportError::read(&name, e))?;
        ctx.ticket.check()?;
        let shx = read(sibling(path, "shx"))?;
        let dbf = read(sibling(path, "dbf"))?;
        let text = |b: Option<Vec<u8>>| b.map(|b| String::from_utf8_lossy(&b).into_owned());
        let prj = text(read(sibling(path, "prj"))?);
        let cpg = cpg_or_override(text(read(sibling(path, "cpg"))?));
        ctx.stage(Stage::Extent)?;
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("layer");
        Ok(vec![read_parts(path, None, stem, shp, shx, dbf, prj, cpg, ctx)?])
    }
}

impl FormatAdapter for ZippedShapefile {
    fn name(&self) -> &'static str {
        "Zipped Shapefile"
    }

    fn sniff(&self, path: &Path, head: &[u8]) -> bool {
        let ext = extension(path);
        ext == "zip" || (head.starts_with(b"PK\x03\x04") && !NOT_SHAPEFILE_ZIPS.contains(&ext.as_str()))
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, _meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        // The archive is read into memory to list and expand it: cap it first.
        let size = std::fs::metadata(path).map_err(|e| ImportError::read(&name, e))?.len();
        if size > super::limits::ZIP_MAX_EXPANDED {
            return Err(ImportError::Refused(format!(
                "{name} is larger than 2 GB, the most LayeredMap reads as a zipped Shapefile. Unzip it and open the .shp. Your file has not been changed."
            )));
        }
        let bytes = std::fs::read(path).map_err(|e| ImportError::read(&name, e))?;
        // The limits are checked from the central directory before anything is expanded.
        let entries = zip_entries(&bytes)?;
        check_zip(&entries)?;
        ctx.ticket.check()?;
        let ext = |n: &str| Path::new(n).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let is_fork = |n: &str| n.split('/').any(|p| p == "__MACOSX" || p.starts_with("._"));
        let wanted = |n: &str| !is_fork(n) && matches!(ext(n).as_str(), "shp" | "shx" | "dbf" | "prj" | "cpg");
        let files = rustgis_engine::data::zip::read_matching(&bytes, wanted).map_err(|e| ImportError::read(&name, e))?;
        drop(bytes);
        ctx.stage(Stage::Extent)?;
        let stem_of = |n: &str| n.get(..n.len().saturating_sub(ext(n).len() + 1)).unwrap_or(n).to_lowercase();
        let mut shps: Vec<&String> = files.iter().filter(|(n, _)| ext(n) == "shp").map(|(n, _)| n).collect();
        shps.sort();
        if shps.is_empty() {
            let others: Vec<String> = entries.iter().map(|e| e.name.clone()).filter(|n| !is_fork(n) && !n.ends_with('/')).take(5).collect();
            return Err(ImportError::Unsupported(format!(
                "{name} holds no Shapefile. LayeredMap opens zipped Shapefiles; open other files directly{}.",
                if others.is_empty() { String::new() } else { format!(" (it holds {})", others.join(", ")) }
            )));
        }
        let get = |stem: &str, e: &str| files.iter().find(|(n, _)| ext(n) == e && stem_of(n) == stem).map(|(_, b)| b.clone());
        let mut out = Vec::new();
        for shp_name in shps {
            ctx.ticket.check()?;
            let stem = stem_of(shp_name);
            let layer_name = Path::new(shp_name.as_str()).file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string();
            let text = |b: Option<Vec<u8>>| b.map(|b| String::from_utf8_lossy(&b).into_owned());
            let layer = read_parts(
                path,
                Some(shp_name.clone()),
                &layer_name,
                get(&stem, "shp").unwrap_or_default(),
                get(&stem, "shx"),
                get(&stem, "dbf"),
                text(get(&stem, "prj")),
                cpg_or_override(text(get(&stem, "cpg"))),
                ctx,
            )?;
            out.push(ImportedLayer {
                profile: lm_core::model::LayerProfile { format: "Zipped Shapefile".into(), ..layer.profile },
                ..layer
            });
        }
        Ok(out)
    }
}
