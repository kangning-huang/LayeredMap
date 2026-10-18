//! Basemap providers (WP1.3). The interface lists providers; swapping one changes only this
//! table, never the UI.

/// How a provider's tiles are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The bundled Natural Earth world outline: no network.
    Offline,
    /// A vector-tile archive (PMTiles) read by byte ranges, drawn in a built-in style variant.
    VectorArchive,
    /// Raster image tiles (satellite imagery), with place names and boundaries from the vector
    /// archive drawn over them.
    Imagery,
}

/// The light or dark look of a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Follows the interface's appearance (light or dark), as Apple Maps does.
    Auto,
    Light,
    Dark,
}

/// A basemap provider: the plan's record (`id, name, url_template, attribution, max_zoom,
/// offline_allowed, export_allowed`) plus how it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Provider {
    pub id: &'static str,
    /// The name shown in the basemap chooser.
    pub name: &'static str,
    /// The tile address: a PMTiles archive for [`Source::VectorArchive`], empty when offline.
    pub url_template: &'static str,
    pub attribution: &'static str,
    pub max_zoom: u8,
    /// Usable with no network.
    pub offline_allowed: bool,
    /// Its terms allow it in exported images (otherwise export substitutes the offline map).
    pub export_allowed: bool,
    /// What the provider's terms ask of an exported image beyond its credit (shown in the export
    /// sheet; empty when nothing).
    pub export_terms: &'static str,
    pub source: Source,
    pub look: Look,
}

/// The pinned online archive until the WP0.6 host exists (docs/lm/basemap-hosting.md).
pub const PROTOMAPS_ARCHIVE: &str = "https://build.protomaps.com/20261014.pmtiles";
const OSM_ATTRIBUTION: &str = "© OpenStreetMap contributors · Protomaps";
const NE_ATTRIBUTION: &str = "Natural Earth";
/// Sentinel-2 cloudless by EOX: a cloud-free 10 m mosaic of the world, keyless. The 2024 edition
/// is CC BY-NC-SA 4.0 (non-commercial; the 2016 edition is CC BY 4.0), so a paid tier needs
/// another source (docs/lm/decisions.md, 2026-10-17).
pub const SENTINEL2_TILES: &str = "https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless-2024_3857/default/GoogleMapsCompatible/{z}/{y}/{x}.jpg";
const SENTINEL2_ATTRIBUTION: &str = "Sentinel-2 cloudless by EOX (Copernicus Sentinel data 2024)";
/// The 2024 edition's licence (CC BY-NC-SA 4.0) carries over to images made from it.
const SENTINEL2_TERMS: &str = "The satellite imagery is licensed CC BY-NC-SA 4.0: use this image for non-commercial purposes only, and share it under the same licence.";

/// The built-in providers, in chooser order: Auto (the default; light or dark with the
/// appearance), Light, Dark, Satellite, Offline.
pub fn providers() -> Vec<Provider> {
    vec![
        Provider {
            id: "auto",
            name: "Auto",
            url_template: PROTOMAPS_ARCHIVE,
            attribution: OSM_ATTRIBUTION,
            max_zoom: 15,
            offline_allowed: false,
            export_allowed: true,
            export_terms: "",
            source: Source::VectorArchive,
            look: Look::Auto,
        },
        Provider {
            id: "light",
            name: "Light",
            url_template: PROTOMAPS_ARCHIVE,
            attribution: OSM_ATTRIBUTION,
            max_zoom: 15,
            offline_allowed: false,
            export_allowed: true,
            export_terms: "",
            source: Source::VectorArchive,
            look: Look::Light,
        },
        Provider {
            id: "dark",
            name: "Dark",
            url_template: PROTOMAPS_ARCHIVE,
            attribution: OSM_ATTRIBUTION,
            max_zoom: 15,
            offline_allowed: false,
            export_allowed: true,
            export_terms: "",
            source: Source::VectorArchive,
            look: Look::Dark,
        },
        Provider {
            id: "satellite",
            name: "Satellite",
            url_template: SENTINEL2_TILES,
            attribution: SENTINEL2_ATTRIBUTION,
            max_zoom: 14,
            offline_allowed: false,
            export_allowed: true,
            export_terms: SENTINEL2_TERMS,
            source: Source::Imagery,
            look: Look::Dark,
        },
        Provider {
            id: "offline",
            name: "Offline",
            url_template: "",
            attribution: NE_ATTRIBUTION,
            max_zoom: 8,
            offline_allowed: true,
            export_allowed: true,
            export_terms: "",
            source: Source::Offline,
            look: Look::Light,
        },
    ]
}

/// The provider with `id`, or the default (Auto).
pub fn provider(id: &str) -> Provider {
    let all = providers();
    let found = all.iter().find(|p| p.id == id).cloned();
    match found {
        Some(p) => p,
        None => all.into_iter().next().unwrap_or(Provider {
            id: "offline",
            name: "Offline",
            url_template: "",
            attribution: NE_ATTRIBUTION,
            max_zoom: 8,
            offline_allowed: true,
            export_allowed: true,
            export_terms: "",
            source: Source::Offline,
            look: Look::Light,
        }),
    }
}

/// Above this zoom the offline basemap has nothing more to show (Natural Earth 1:50m).
pub const OFFLINE_MAX_ZOOM: f64 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_is_the_default_and_ids_are_unique() {
        let p = providers();
        assert_eq!(p[0].id, "auto");
        assert_eq!(p[0].look, Look::Auto);
        let mut ids: Vec<&str> = p.iter().map(|p| p.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), p.len());
        assert_eq!(provider("nope").id, "auto");
        assert_eq!(provider("offline").source, Source::Offline);
    }

    #[test]
    fn only_offline_needs_no_network() {
        for p in providers() {
            assert_eq!(p.offline_allowed, p.source == Source::Offline, "{}", p.id);
            assert!(!p.attribution.is_empty());
        }
    }
}
