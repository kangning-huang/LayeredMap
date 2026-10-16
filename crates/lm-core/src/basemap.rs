//! Basemap providers (WP1.3). The interface lists providers; swapping one changes only this
//! table, never the UI.

/// How a provider's tiles are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The bundled Natural Earth world outline: no network.
    Offline,
    /// A vector-tile archive (PMTiles) read by byte ranges, drawn in a built-in style variant.
    VectorArchive,
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
    pub source: Source,
    pub look: Look,
}

/// The pinned online archive until the WP0.6 host exists (docs/lm/basemap-hosting.md).
pub const PROTOMAPS_ARCHIVE: &str = "https://build.protomaps.com/20261014.pmtiles";
const OSM_ATTRIBUTION: &str = "© OpenStreetMap contributors · Protomaps";
const NE_ATTRIBUTION: &str = "Natural Earth";

/// The built-in providers, in chooser order: Auto (the default; light or dark with the
/// appearance), Light, Dark, Offline.
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
            source: Source::VectorArchive,
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
