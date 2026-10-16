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

/// What GDAL records about a band in a GeoTIFF's metadata tag (42112): scale and offset (decoded =
/// raw × scale + offset), unit and description.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BandMeta {
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
    pub description: Option<String>,
}

/// Parses GDAL's metadata XML (`<Item name="SCALE" sample="0" role="scale">0.1</Item>`) into one
/// entry per band (sample 0 is band 1).
pub fn parse_gdal_metadata(xml: &str, bands: usize) -> Vec<BandMeta> {
    let mut out = vec![BandMeta::default(); bands];
    let attr = |tag: &str, name: &str| -> Option<String> {
        let key = format!("{name}=\"");
        let i = tag.find(&key)? + key.len();
        let j = tag.get(i..)?.find('"')? + i;
        tag.get(i..j).map(str::to_string)
    };
    let mut rest = xml;
    while let Some(start) = rest.find("<Item") {
        let after = rest.get(start..).unwrap_or_default();
        let Some(close) = after.find('>') else { break };
        let Some(end) = after.find("</Item>") else { break };
        let tag = after.get(..close).unwrap_or_default();
        let value = after.get(close + 1..end).unwrap_or_default().trim();
        let value = value.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
        rest = after.get(end + 7..).unwrap_or_default();
        let Some(sample) = attr(tag, "sample").and_then(|s| s.parse::<usize>().ok()) else { continue };
        let Some(m) = out.get_mut(sample) else { continue };
        let role = attr(tag, "role").unwrap_or_default();
        let name = attr(tag, "name").unwrap_or_default();
        match (role.as_str(), name.to_ascii_uppercase().as_str()) {
            ("scale", _) | (_, "SCALE") => m.scale = value.parse().ok().filter(|v: &f64| v.is_finite() && *v != 0.0),
            ("offset", _) | (_, "OFFSET") => m.offset = value.parse().ok().filter(|v: &f64| v.is_finite()),
            ("unittype", _) => m.unit = Some(value.to_string()).filter(|u| !u.is_empty()),
            ("description", _) => m.description = Some(value.to_string()).filter(|d| !d.is_empty()),
            _ => {}
        }
    }
    out
}

/// The GDAL metadata of a GeoTIFF's first image, if it has any.
fn read_gdal_metadata(path: &Path, bands: usize) -> Vec<BandMeta> {
    let read = || -> Option<String> {
        let f = std::fs::File::open(path).ok()?;
        let mut dec = tiff::decoder::Decoder::new(std::io::BufReader::new(f)).ok()?;
        dec.get_tag_ascii_string(tiff::tags::Tag::Unknown(42112)).ok()
    };
    read().map(|xml| parse_gdal_metadata(&xml, bands)).unwrap_or_else(|| vec![BandMeta::default(); bands])
}

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
        // Scale and offset must be applied to every number shown (raw values would be wrong).
        let metas = read_gdal_metadata(path, r.bands.len());
        let (pw, ph) = (r.width as usize, r.height as usize);
        let mut bands = Vec::new();
        let mut assumptions = Vec::new();
        for (i, b) in r.bands.iter().enumerate() {
            let sample = lm_profile::BandSample {
                index: i as u32 + 1,
                name: metas.get(i).and_then(|m| m.description.as_deref()).unwrap_or(&b.name),
                file_name: &name,
                width: pw,
                height: ph,
                values: &b.data,
                declared_nodata: r.nodata,
                scale: metas.get(i).and_then(|m| m.scale),
                offset: metas.get(i).and_then(|m| m.offset),
                unit: metas.get(i).and_then(|m| m.unit.as_deref()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdal_metadata_gives_scale_offset_unit_per_band() {
        let xml = r#"<GDALMetadata>
  <Item name="OFFSET" sample="0" role="offset">-100</Item>
  <Item name="SCALE" sample="0" role="scale">0.1</Item>
  <Item name="DESCRIPTION" sample="1" role="description">Surface temperature</Item>
  <Item name="UNITTYPE" sample="1" role="unittype">K</Item>
</GDALMetadata>"#;
        let m = parse_gdal_metadata(xml, 2);
        assert_eq!(m[0].scale, Some(0.1));
        assert_eq!(m[0].offset, Some(-100.0));
        assert_eq!(m[1].unit.as_deref(), Some("K"));
        assert_eq!(m[1].description.as_deref(), Some("Surface temperature"));
        assert_eq!(parse_gdal_metadata("", 1), vec![BandMeta::default()]);
    }
}
