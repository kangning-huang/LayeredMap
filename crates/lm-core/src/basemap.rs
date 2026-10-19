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
    /// The name shown in the basemap chooser (an English catalog key, translated where it is
    /// drawn).
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
/// NASA Blue Marble Next Generation through NASA GIBS: a cloud-free true-colour mosaic of the
/// world, keyless, tiles to zoom 8 (about 600 m per pixel; deeper zooms scale those tiles up).
/// NASA imagery is not subject to copyright in the United States; NASA asks to be credited, and
/// nothing may suggest NASA endorses the app. It replaces Sentinel-2 cloudless by EOX, whose
/// licence is non-commercial (owner's decision, 2026-10-18).
pub const BLUE_MARBLE_TILES: &str =
    "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/BlueMarble_NextGeneration/default/GoogleMapsCompatible_Level8/{z}/{y}/{x}.jpeg";
const BLUE_MARBLE_ATTRIBUTION: &str = "NASA Blue Marble (NASA Earth Observatory, GIBS)";

/// The built-in providers, in chooser order: Auto (the default; light or dark with the
/// appearance), Light, Dark, Satellite, Offline.
pub fn providers() -> Vec<Provider> {
    vec![
        Provider {
            id: "auto",
            name: crate::tn!("Auto"),
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
            name: crate::tn!("Light"),
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
            name: crate::tn!("Dark"),
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
            name: crate::tn!("Satellite"),
            url_template: BLUE_MARBLE_TILES,
            attribution: BLUE_MARBLE_ATTRIBUTION,
            max_zoom: 8,
            offline_allowed: false,
            export_allowed: true,
            export_terms: "",
            source: Source::Imagery,
            look: Look::Dark,
        },
        Provider {
            id: "offline",
            name: crate::tn!("Offline"),
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
            name: crate::tn!("Offline"),
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

/// The OpenStreetMap credit the vector archive's own layers carry (also in the satellite
/// basemap's labels and its close-up map).
pub const OSM_CREDIT: &str = OSM_ATTRIBUTION;

/// What a credit names, for telling repeats apart: "© OpenStreetMap", "&copy; OpenStreetMap
/// contributors" and "OpenStreetMap contributors" all name OpenStreetMap.
fn credit_key(c: &str) -> String {
    let mut k = c.to_lowercase().replace("&copy;", "").replace('©', "").replace("(c)", "");
    for tail in ["contributors", "contributor"] {
        if let Some(stripped) = k.trim_end().strip_suffix(tail) {
            k = stripped.to_string();
        }
    }
    k.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The credits `parts` as one list: each split at " · ", repeats dropped (by [`credit_key`]),
/// keeping the fuller wording of a repeat ("© OpenStreetMap contributors" over "© OpenStreetMap")
/// in the place it first appeared.
pub fn merge_credits<'a>(parts: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<(String, String)> = Vec::new();
    for c in parts.into_iter().flat_map(|p| p.split(" · ")).map(str::trim).filter(|c| !c.is_empty()) {
        let key = credit_key(c);
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some((_, kept)) if c.len() > kept.len() => *kept = c.to_string(),
            Some(_) => {}
            None => out.push((key, c.to_string())),
        }
    }
    out.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_that_say_the_same_thing_are_listed_once() {
        let m = merge_credits(["NASA Blue Marble (NASA Earth Observatory, GIBS)", "© OpenStreetMap", OSM_CREDIT, "Natural Earth"]);
        assert_eq!(m, ["NASA Blue Marble (NASA Earth Observatory, GIBS)", "© OpenStreetMap contributors", "Protomaps", "Natural Earth"]);
        assert_eq!(merge_credits(["&copy; OpenStreetMap contributors", "© OpenStreetMap"]), ["&copy; OpenStreetMap contributors"]);
        assert!(merge_credits(["", " · "]).is_empty());
    }

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
