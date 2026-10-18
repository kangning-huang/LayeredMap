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

/// Light and Dark keep their own look; Auto and the offline outline follow the theme.
fn look_dark(p: &Provider, dark_ui: bool) -> bool {
    match (p.source, p.look) {
        (Source::Offline, _) | (_, Look::Auto) => dark_ui,
        (Source::VectorArchive, look) => look == Look::Dark,
        // Imagery is dark: the outline beneath (seen while tiles load) is the dark one.
        (Source::Imagery, _) => true,
    }
}

/// Map labels: the English name where the map has one, else the place's own name (owner, Gate 1).
pub const LABEL_LANGUAGE: &str = "en";

/// Sets the map's backdrop and tile layer for `prefs.basemap` and `prefs.strength`.
pub fn apply(map: &mut Map, prefs: &Prefs, dark_ui: bool) {
    // Offline mode draws the offline outline whatever basemap is chosen, so no tile is asked for.
    let p = if lm_net::offline() { provider("offline") } else { provider(&prefs.basemap) };
    let dark = look_dark(&p, dark_ui);
    map.basemap = quiet(dark);
    map.graticule = false;
    // The tile layer is the only web layer LayeredMap's chrome manages; others are kept.
    map.tiles.retain(|t| !t.id.starts_with("lm-basemap"));
    if p.source == Source::VectorArchive {
        // Developer override, for testing a host or simulating being offline (an unreachable address).
        let url = std::env::var("LM_BASEMAP_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| lm_mapview::archive::url(p.url_template));
        let mut t = TileLayer::new(p.name, TileKind::PmTiles, url);
        t.id = format!("lm-basemap-{}", p.id);
        t.attribution = p.attribution.into();
        t.max_zoom = p.max_zoom;
        t.variant = if dark { "dark".into() } else { "light".into() };
        t.label_language = LABEL_LANGUAGE.into();
        // "Background strength": the basemap fades toward the plain ground colour beneath it.
        t.opacity = prefs.strength.clamp(0.4, 1.0);
        map.tiles.insert(0, t);
    }
    if p.source == Source::Imagery {
        let mut img = TileLayer::new(p.name, TileKind::Raster, p.url_template.to_string());
        img.id = format!("lm-basemap-{}", p.id);
        img.attribution = p.attribution.into();
        img.max_zoom = p.max_zoom;
        img.opacity = prefs.strength.clamp(0.4, 1.0);
        map.tiles.insert(0, img);
        // Place names, boundaries and major roads from the vector archive, over the imagery.
        let url = std::env::var("LM_BASEMAP_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| lm_mapview::archive::url(lm_core::basemap::PROTOMAPS_ARCHIVE));
        let mut labels = TileLayer::new("Satellite labels", TileKind::PmTiles, url);
        labels.id = format!("lm-basemap-{}-labels", p.id);
        labels.max_zoom = 15;
        labels.variant = "labels".into();
        labels.label_language = LABEL_LANGUAGE.into();
        map.tiles.insert(0, labels);
    }
}

/// Starts looking for the newest daily build of the online basemap (the app calls this once; tests
/// don't, so they never touch the network). The map switches to it when it is found.
pub fn start_archive_lookup(ctx: &egui::Context) {
    let ctx = ctx.clone();
    lm_mapview::archive::start(move || ctx.request_repaint());
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

/// Pictures of the same place (the Golden Gate) in each look, so the choices compare at a glance:
/// crops of the app's own rendering at 3× the chooser's 96 × 64 tiles (`assets/thumbnails`).
const THUMBNAILS: [(&str, &[u8]); 3] = [
    ("light", include_bytes!("../../../assets/thumbnails/light.png")),
    ("dark", include_bytes!("../../../assets/thumbnails/dark.png")),
    ("satellite", include_bytes!("../../../assets/thumbnails/satellite.png")),
];

/// A thumbnail's texture, decoded once and kept for the session.
fn thumbnail_texture(ctx: &egui::Context, name: &'static str) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("lm-basemap-thumbnail", name));
    if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(tex);
    }
    let bytes = THUMBNAILS.iter().find(|(n, _)| *n == name)?.1;
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let tex = ctx.load_texture(
        format!("basemap-{name}"),
        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

/// A small picture of a provider for the chooser (no network): a real crop of the map for the
/// online looks, a drawn outline for Offline.
pub fn paint_thumbnail(p: &egui::Painter, rect: egui::Rect, provider: &Provider, dark_ui: bool) {
    if provider.look == Look::Auto && provider.source == Source::VectorArchive {
        // Auto: the light picture on the left half, the dark one on the right.
        for (dark, half) in [
            (false, egui::Rect::from_min_max(rect.min, rect.center_bottom())),
            (true, egui::Rect::from_min_max(rect.center_top(), rect.max)),
        ] {
            let forced = Provider { look: if dark { Look::Dark } else { Look::Light }, ..provider.clone() };
            paint_thumbnail(&p.with_clip_rect(half), rect, &forced, dark_ui);
        }
        return;
    }
    let photo = match provider.source {
        Source::Imagery => Some("satellite"),
        Source::VectorArchive if look_dark(provider, dark_ui) => Some("dark"),
        Source::VectorArchive => Some("light"),
        Source::Offline => None,
    };
    if let Some(tex) = photo.and_then(|name| thumbnail_texture(p.ctx(), name)) {
        let r8 = egui::CornerRadius::same(crate::theme::RADIUS_CONTROL);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        p.add(egui::epaint::RectShape::filled(rect, r8, egui::Color32::WHITE).with_texture(tex.id(), uv));
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The basemap tile layer's (variant, label language).
    fn layer(map: &Map) -> Option<(String, String)> {
        map.tiles.iter().find(|t| t.id.starts_with("lm-basemap")).map(|t| (t.variant.clone(), t.label_language.clone()))
    }

    #[test]
    fn the_default_basemap_follows_the_appearance() {
        let prefs = Prefs::default();
        assert_eq!(prefs.basemap, "auto");
        for dark in [false, true] {
            let map = initial_map(&prefs, dark);
            assert_eq!(layer(&map).map(|l| l.0).as_deref(), Some(if dark { "dark" } else { "light" }));
            assert_eq!(map.basemap, quiet(dark));
        }
    }

    #[test]
    fn light_and_dark_keep_their_look_in_either_appearance() {
        for (id, variant) in [("light", "light"), ("dark", "dark")] {
            let prefs = Prefs { basemap: id.into(), ..Prefs::default() };
            for dark_ui in [false, true] {
                assert_eq!(layer(&initial_map(&prefs, dark_ui)).map(|l| l.0).as_deref(), Some(variant), "{id} with dark UI {dark_ui}");
            }
        }
    }

    #[test]
    fn labels_are_english_first() {
        let map = initial_map(&Prefs::default(), false);
        assert_eq!(layer(&map).map(|l| l.1).as_deref(), Some("en"));
    }
}
