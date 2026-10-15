//! Basemaps (WP1.3): turns a provider (lm-core) and the theme into the map's backdrop and tile
//! layer. The offline Natural Earth outline is always drawn underneath, so a missing tile shows
//! the coarse world instead of a blank (Map feel › Missing tiles).

use lm_core::basemap::{Look, Provider, Source, provider};
use lm_mapview::map::{Basemap, Map, TileKind, TileLayer};
use lm_mapview::{Coord, Extent, res_of};

use crate::{Prefs, ThemePref};

/// The whole world, for "zoom to all data" with no data.
pub const WORLD: Extent = Extent { min_x: -180.0, min_y: -60.0, max_x: 180.0, max_y: 75.0 };

/// Whether the interface (and the offline basemap) is dark.
pub fn dark_for(pref: ThemePref, system_dark: bool) -> bool {
    match pref {
        ThemePref::System => system_dark,
        ThemePref::Light => false,
        ThemePref::Dark => true,
    }
}

/// The offline outline's look: "Quiet" light or dark (RustGIS's Paper and Midnight palettes).
fn quiet(dark: bool) -> Basemap {
    if dark { Basemap::Midnight } else { Basemap::Paper }
}

/// The online provider's look follows its own name; the offline one follows the theme.
fn look_dark(p: &Provider, dark_ui: bool) -> bool {
    match p.source {
        Source::Offline => dark_ui,
        Source::VectorArchive => p.look == Look::Dark,
    }
}

/// Sets the map's backdrop and tile layer for `prefs.basemap` and `prefs.strength`.
pub fn apply(map: &mut Map, prefs: &Prefs, dark_ui: bool) {
    let p = provider(&prefs.basemap);
    let dark = look_dark(&p, dark_ui);
    map.basemap = quiet(dark);
    map.graticule = false;
    // The tile layer is the only web layer LayeredMap's chrome manages; others are kept.
    map.tiles.retain(|t| !t.id.starts_with("lm-basemap"));
    if p.source == Source::VectorArchive {
        let mut t = TileLayer::new(p.name, TileKind::PmTiles, p.url_template);
        t.id = format!("lm-basemap-{}", p.id);
        t.attribution = p.attribution.into();
        t.max_zoom = p.max_zoom;
        t.variant = if dark { "dark".into() } else { "light".into() };
        // Each place in its own language: Chinese in China, as people there read their maps.
        t.label_language = "local".into();
        // "Background strength": the basemap fades toward the plain ground colour beneath it.
        t.opacity = prefs.strength.clamp(0.4, 1.0);
        map.tiles.insert(0, t);
    }
}

/// The starting map: the saved view, or the whole world on first launch.
pub fn initial_map(prefs: &Prefs, dark_ui: bool) -> Map {
    let mut map = Map::new("LayeredMap");
    apply(&mut map, prefs, dark_ui);
    map.camera = match prefs.view {
        Some((lon, lat, zoom)) if lon.is_finite() && lat.is_finite() && zoom.is_finite() => {
            lm_mapview::map::Camera { center: lm_mapview::lonlat_to_merc(Coord::new(lon, lat)), res: res_of(zoom) }
        }
        _ => lm_mapview::map::Camera { center: lm_mapview::lonlat_to_merc(Coord::new(15.0, 30.0)), res: res_of(2.2) },
    };
    map
}

/// The ground colour painted before the first frame: the basemap's water.
pub fn backdrop(map: &Map) -> egui::Color32 {
    let (water, _, _, _) = map.basemap.palette();
    egui::Color32::from_rgb(water.0[0], water.0[1], water.0[2])
}

/// A small picture of a provider for the chooser (no network): ground, water, two roads.
pub fn paint_thumbnail(p: &egui::Painter, rect: egui::Rect, provider: &Provider, dark_ui: bool) {
    let dark = look_dark(provider, dark_ui);
    let (water, land, line, _) = quiet(dark).palette();
    let c = |r: lm_mapview::map::Rgba| egui::Color32::from_rgb(r.0[0], r.0[1], r.0[2]);
    let r8 = egui::CornerRadius::same(crate::theme::RADIUS_CONTROL);
    p.rect_filled(rect, r8, c(land));
    let w = rect.width();
    let water_rect = egui::Rect::from_min_max(rect.left_bottom() + egui::vec2(0.0, -rect.height() * 0.38), rect.right_bottom());
    p.rect_filled(water_rect, egui::CornerRadius { nw: 0, ne: 0, sw: r8.sw, se: r8.se }, c(water));
    if provider.source == Source::VectorArchive {
        let road = if dark { egui::Color32::from_rgb(0x4A, 0x4E, 0x55) } else { egui::Color32::from_rgb(0xC9, 0xCD, 0xD3) };
        p.line_segment(
            [rect.left_top() + egui::vec2(w * 0.1, rect.height() * 0.15), rect.left_top() + egui::vec2(w * 0.85, rect.height() * 0.55)],
            egui::Stroke::new(2.5, road),
        );
        p.line_segment(
            [rect.left_top() + egui::vec2(w * 0.55, rect.height() * 0.05), rect.left_top() + egui::vec2(w * 0.35, rect.height() * 0.6)],
            egui::Stroke::new(1.5, road),
        );
    } else {
        p.line_segment([water_rect.left_top(), water_rect.right_top()], egui::Stroke::new(1.0, c(line)));
    }
}
