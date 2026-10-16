//! GeoTIFF and Cloud-Optimized GeoTIFF. The header gives the size and whether overviews exist
//! before any pixels are read; a COG previews from its overviews, and a large TIFF without them is
//! read sparsely (the "Preparing a faster preview…" step), cancelable at any point because the
//! file is read through a [`Counting`] reader.

use std::path::Path;
use std::sync::Arc;

use lm_core::model::{Crs, Extent, Known, LayerKind, LayerProfile};

use super::limits::{RASTER_MAX_SIDE, RASTER_PREPARE_SIDE};
use super::progress::Counting;
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, Stage, extension, file_name, source_ref, status_of};

pub struct GeoTiff;

impl FormatAdapter for GeoTiff {
    fn name(&self) -> &'static str {
        "GeoTIFF"
    }

    fn sniff(&self, path: &Path, head: &[u8]) -> bool {
        matches!(extension(path).as_str(), "tif" | "tiff") || matches!(head, [b'I', b'I', 42 | 43, 0, ..] | [b'M', b'M', 0, 42 | 43, ..])
    }

    fn is_raster(&self) -> bool {
        true
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let name = file_name(path);
        let size = std::fs::metadata(path).map_err(|e| ImportError::read(&name, e))?.len();
        let f = std::fs::File::open(path).map_err(|e| ImportError::read(&name, e))?;
        let mut dec = tiff::decoder::Decoder::new(std::io::BufReader::new(f)).map_err(|e| ImportError::read(&name, e))?;
        let (w, h) = dec.dimensions().map_err(|e| ImportError::read(&name, e))?;
        let (w, h) = (u64::from(w), u64::from(h));
        if w > RASTER_MAX_SIDE || h > RASTER_MAX_SIDE {
            return Err(ImportError::Refused(format!(
                "{name} says it is {} × {} pixels, more than the {} pixels per side LayeredMap opens. Your file has not been changed.",
                lm_core::model::group_thousands(w),
                lm_core::model::group_thousands(h),
                lm_core::model::group_thousands(RASTER_MAX_SIDE)
            )));
        }
        let has_overviews = dec.more_images();
        Ok(Metadata { size, pixels: Some((w, h)), has_overviews, ..Metadata::default() })
    }

    fn read_window(&self, path: &Path, meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        let (w, h) = meta.pixels.unwrap_or((0, 0));
        if (w > RASTER_PREPARE_SIDE || h > RASTER_PREPARE_SIDE) && !meta.has_overviews {
            ctx.preparing();
        }
        ctx.ticket.set_total(meta.size);
        let f = std::fs::File::open(path).map_err(|e| ImportError::read(&name, e))?;
        let reader = std::io::BufReader::with_capacity(1 << 20, Counting::new(f, ctx.ticket.clone()));
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("raster").to_string();
        let mut r = rustgis_engine::data::raster::geotiff::read(reader, &stem).map_err(|e| ImportError::read(&name, e))?;
        ctx.ticket.check()?;
        r.source = Some(path.display().to_string());
        ctx.stage(Stage::Extent)?;
        let e = r.extent();
        let extent =
            (e.min_x.is_finite() && e.max_x > e.min_x).then_some(Extent { min_x: e.min_x, min_y: e.min_y, max_x: e.max_x, max_y: e.max_y });
        ctx.stage(Stage::Profile)?;
        let integer_type = r.data_type.contains("Int");
        let (pw, ph) = (r.width as usize, r.height as usize);
        let mut bands = Vec::new();
        let mut assumptions = Vec::new();
        for (i, b) in r.bands.iter().enumerate() {
            let sample = lm_profile::BandSample {
                index: i as u32 + 1,
                name: &b.name,
                file_name: &name,
                width: pw,
                height: ph,
                values: &b.data,
                declared_nodata: r.nodata,
                scale: None,
                offset: None,
                unit: None,
                integer_type,
            };
            let (band, a) = lm_profile::profile_band(&sample);
            bands.push(band);
            assumptions.extend(a);
        }
        let crs = match r.crs {
            rustgis_engine::data::raster::Crs::LonLat => Known::certain(Crs::Wgs84, "From the GeoTIFF's coordinate keys."),
            c => Known::certain(Crs::Epsg(u32::from(c.epsg())), "From the GeoTIFF's coordinate keys; shown in GPS coordinates."),
        };
        let mut notices = Vec::new();
        if let Some((fw, fh)) = r.overview {
            notices.push(format!(
                "Shown from a {} × {} pixel preview of the {} × {} image.",
                lm_core::model::group_thousands(u64::from(r.width)),
                lm_core::model::group_thousands(u64::from(r.height)),
                lm_core::model::group_thousands(u64::from(fw)),
                lm_core::model::group_thousands(u64::from(fh))
            ));
        }
        let profile = LayerProfile {
            source: source_ref(path, None),
            format: "GeoTIFF".into(),
            kind: LayerKind::Raster,
            count: w.saturating_mul(h),
            extent_wgs84: extent,
            crs,
            fields: Vec::new(),
            bands,
            assumptions,
            exclusions: Vec::new(),
            encoding: None,
            notices,
            data_rev: 0,
        };
        Ok(vec![ImportedLayer { name: stem, status: status_of(&profile), profile, data: LayerData::Raster(Arc::new(r)), raw: None }])
    }
}
