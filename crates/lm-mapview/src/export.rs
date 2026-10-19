//! Map images at an exact output size (WP5.3). The map is drawn offscreen by RustGIS's CPU
//! renderer at the export's own pixel size (never a screenshot of the window), waiting for its
//! tiles; then the sheet — title, legend, scale bar, north arrow, latitude/longitude frame, caption
//! and credits — is drawn around it with the app's fonts. [`plan`] lays the map out automatically:
//! it frames the data and picks the furniture by the map's ground width. The export sheet's
//! preview is the same drawing at a smaller scale, so what the preview shows is what is saved.
//!
//! Everything is laid out in points; `ppp` (pixels per point) turns points into the output's
//! pixels. The output is PNG, or PDF ([`compose_pdf`], WP6.5): the same drawing on a page of the
//! sheet's paper size, the map an embedded image and the rest vector shapes and selectable text.

use ab_glyph::{Font, FontArc, FontRef, GlyphId, PxScale, ScaleFont};
use rustgis_geom::Extent;
use rustgis_map::Map;
use rustgis_map::camera::Camera;
pub use rustgis_render::Pixmap;
use rustgis_render::View;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapPaint, Rect, Stroke, Transform};

mod pdf;

/// Space between the map's edge and a card on it (points).
pub const MARGIN: f32 = 16.0;
/// Text size of the caption, credits, legend and scale bar (points).
pub const TEXT: f32 = 12.0;
const LINE: f32 = 16.0;
/// The title's size, and the band it sits in above the map.
pub const TITLE: f32 = 22.0;
pub const TITLE_BAND: f32 = 60.0;
/// The band under the map: up to two lines of caption with the credits.
pub const FOOTER: f32 = 52.0;
/// Side padding of the title and footer bands.
const SIDE: f32 = 24.0;
const PRIMARY: [u8; 4] = [0x1B, 0x1D, 0x21, 0xFF];
const SECONDARY: [u8; 4] = [0x5F, 0x66, 0x70, 0xFF];
const BORDER: [u8; 4] = [0xE3, 0xE5, 0xE8, 0xFF];
const CARD: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xF0];
/// The legend's ramp width (points).
const RAMP_W: f32 = 160.0;
/// Missing data: light gray with a diagonal hatch (Data palettes › Missing data).
pub const NO_DATA: [u8; 4] = [0xD0, 0xD3, 0xD8, 0xFF];

/// A rectangle in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Area {
        Area { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
    /// In pixels at `ppp`, rounded outward to whole pixels.
    fn px(&self, ppp: f32) -> Option<Rect> {
        Rect::from_xywh((self.x * ppp).round(), (self.y * ppp).round(), (self.w * ppp).round().max(1.0), (self.h * ppp).round().max(1.0))
    }
}

/// The app's fonts for the sheet (Inter Regular and SemiBold, then Noto Sans SC for characters
/// Inter lacks).
#[derive(Clone, Copy)]
pub struct Fonts {
    pub regular: &'static [u8],
    pub semibold: &'static [u8],
    pub fallback: &'static [u8],
}

/// How a legend entry's symbol is drawn: as it is on the map.
#[derive(Clone, Debug, PartialEq)]
pub enum Swatch {
    /// An area's fill (with the map's opacity, drawn over the map's ground) and outline.
    Area {
        fill: [u8; 4],
        stroke: [u8; 4],
    },
    Point {
        fill: [u8; 4],
        stroke: [u8; 4],
        diameter: f32,
    },
    Line {
        color: [u8; 4],
        width: f32,
    },
    /// "No data": gray, hatched.
    NoData,
}

/// One legend row.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Item {
        swatch: Swatch,
        label: String,
    },
    /// A continuous ramp from low (left) to high (right), with its end values.
    Ramp {
        stops: Vec<[u8; 4]>,
        low: String,
        high: String,
    },
}

/// One layer's part of the legend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LegendBlock {
    pub title: String,
    pub entries: Vec<Entry>,
}

/// What the exported image shows, and at which size.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    /// The output's size in pixels.
    pub width: u32,
    pub height: u32,
    /// Pixels per point.
    pub ppp: f32,
    /// Empty: no title band.
    pub title: String,
    pub caption: String,
    /// The map's credits (never empty: the basemap's at least).
    pub attribution: String,
    pub legend: Vec<LegendBlock>,
    /// Where the user dragged the legend (its top-left corner, in points); `None` places it.
    pub legend_at: Option<(f32, f32)>,
    /// The scale bar's length in points and its label.
    pub scale: Option<(f32, String)>,
    /// The map's ground colour, under area swatches (their fills are translucent on the map too).
    pub ground: [u8; 4],
    pub north_arrow: bool,
    /// Latitude and longitude on a frame around the map; `None`: the map reaches the edges.
    pub graticule: Option<Graticule>,
}

/// The latitude and longitude frame: the map's camera (to place the ticks) and whether faint grid
/// lines cross the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Graticule {
    pub camera: Camera,
    pub grid: bool,
}

impl Sheet {
    /// The output's size in points.
    pub fn size_pt(&self) -> (f32, f32) {
        (self.width as f32 / self.ppp, self.height as f32 / self.ppp)
    }

    /// The same sheet `k` times as many pixels across (a preview: `k` < 1). The layout, in
    /// points, stays the same.
    pub fn at_scale(&self, k: f32) -> Sheet {
        let width = ((self.width as f32 * k).round() as u32).max(1);
        let k = width as f32 / self.width as f32;
        Sheet { width, height: ((self.height as f32 * k).round() as u32).max(1), ppp: self.ppp * k, ..self.clone() }
    }
}

/// Where everything sits on the sheet, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    pub title: Option<Area>,
    pub map: Area,
    pub footer: Area,
    pub legend: Option<Area>,
    pub scale: Option<Area>,
    pub north: Option<Area>,
}

/// The map's part of a sheet `size` points large, with or without a title band, and `framed`
/// (inset for the latitude and longitude labels around it) or reaching the sheet's edges.
pub fn map_area(size: (f32, f32), has_title: bool, framed: bool) -> Area {
    let top = if has_title { TITLE_BAND } else { 0.0 };
    let (sx, sy) = if framed { (FRAME_SIDE, FRAME_TB) } else { (0.0, 0.0) };
    Area::new(sx, top + sy, (size.0 - 2.0 * sx).max(1.0), (size.1 - top - FOOTER - 2.0 * sy).max(1.0))
}

/// The map area in whole pixels at `ppp`: the size the map is rendered at.
pub fn map_pixels(area: Area, ppp: f32) -> (u32, u32) {
    (((area.w * ppp).round() as u32).max(1), ((area.h * ppp).round() as u32).max(1))
}

/// The camera that shows the view the user saw (`center` and `res` over `seen` points) whole in
/// an area `area` points large.
pub fn fit_camera(center: rustgis_geom::Coord, res: f64, seen: (f32, f32), area: (f32, f32)) -> Camera {
    let k = (f64::from(seen.0) / f64::from(area.0.max(1.0))).max(f64::from(seen.1) / f64::from(area.1.max(1.0)));
    let k = if k.is_finite() && k > 0.0 { k } else { 1.0 };
    Camera { center, res: res * k }
}

// ---------------------------------------------------------------------------------------------
// Automatic layout (WP5.3 › Automatic layout): what frames the map, and which furniture it gets

/// Up to this ground width across the map's centre (km) — a city, a province — the map gets a north
/// arrow and a scale bar: one scale holds across it to within a few per cent.
pub const ARROW_MAX_KM: f64 = 1_500.0;
/// Above this width (km) — a continent, the world — only latitude and longitude on the frame: no
/// one scale bar is right (Mercator scale doubles between the equator and 60°), and on a web map
/// north is up everywhere, so an arrow adds nothing.
pub const LABELS_ONLY_KM: f64 = 7_000.0;
/// Between the two widths (a country such as China or the United States), latitude and longitude
/// on the frame, plus a scale bar only when the map spans less latitude than this (degrees). Over
/// it, Mercator scale differs by about 1.4× from the map's bottom edge to its top, so one bar
/// misleads. The test is strict: a map spanning 28° or more gets no bar. 28° and 7,000 km (owner's
/// decision, 2026-10-18) keep a scale bar on a map of the contiguous United States (about 27° and
/// 6,300 km with the margin).
pub const SCALE_MAX_LAT_SPAN: f64 = 28.0;
/// The margin around the data when the map fits it, as a share of the data's size on each side.
pub const FIT_MARGIN: f64 = 0.05;
/// The frame labels' text size, and the margins a framed map keeps for them: above and below
/// (longitudes) and at the sides (latitudes, up to "85.25°N" wide).
pub const FRAME_TEXT: f32 = 10.0;
pub const FRAME_TB: f32 = 20.0;
pub const FRAME_SIDE: f32 = 44.0;
/// The tick marks' length, pointing into the map from the frame (points).
const TICK: f32 = 4.0;
/// The frame's neatline and ticks: a soft near-black, quieter than the text.
const NEATLINE: [u8; 4] = [0x3A, 0x3F, 0x47, 0xFF];
/// Grid lines across the map: faint, so they never compete with the data.
const GRID: [u8; 4] = [0x1B, 0x1D, 0x21, 0x38];
/// The north arrow's card (points).
pub const ARROW_CARD: (f32, f32) = (28.0, 52.0);
/// The intervals the frame labels may use (degrees).
/// 60° keeps a world map to six longitudes (owner's decision, 2026-10-18).
pub const STEPS: [f64; 10] = [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0];
/// Half the Web Mercator world's width, metres.
const HALF_WORLD: f64 = std::f64::consts::PI * rustgis_geom::MERCATOR_R;

/// An element of the map's furniture: as the rules say, always, or never.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Show {
    #[default]
    Auto,
    On,
    Off,
}

impl Show {
    pub const ALL: [Show; 3] = [Show::Auto, Show::On, Show::Off];

    pub fn label(self) -> &'static str {
        match self {
            Show::Auto => "Auto",
            Show::On => "On",
            Show::Off => "Off",
        }
    }

    /// Whether the element is drawn, the rules saying `auto`.
    pub fn resolve(self, auto: bool) -> bool {
        match self {
            Show::Auto => auto,
            Show::On => true,
            Show::Off => false,
        }
    }
}

/// Which furniture the map carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Furniture {
    pub north_arrow: bool,
    pub scale_bar: bool,
    /// Latitude and longitude labels on a frame around the map.
    pub labels: bool,
}

/// The user's choices over the rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    pub north_arrow: Show,
    pub scale_bar: Show,
    pub labels: Show,
}

impl Overrides {
    pub fn apply(&self, auto: Furniture) -> Furniture {
        Furniture {
            north_arrow: self.north_arrow.resolve(auto.north_arrow),
            scale_bar: self.scale_bar.resolve(auto.scale_bar),
            labels: self.labels.resolve(auto.labels),
        }
    }
}

/// The furniture the rules give a map `width_km` across its centre and spanning `lat_span`
/// degrees of latitude.
pub fn auto_furniture(width_km: f64, lat_span: f64) -> Furniture {
    if width_km <= ARROW_MAX_KM {
        Furniture { north_arrow: true, scale_bar: true, labels: false }
    } else if width_km <= LABELS_ONLY_KM {
        Furniture { north_arrow: false, scale_bar: lat_span < SCALE_MAX_LAT_SPAN, labels: true }
    } else {
        Furniture { north_arrow: false, scale_bar: false, labels: true }
    }
}

/// The ground distance (km) across a map `w` points wide, along its centre's latitude: the
/// Mercator resolution there shrunk by cos(latitude).
pub fn ground_width_km(camera: Camera, w: f32) -> f64 {
    let lat = rustgis_geom::merc_to_lonlat(camera.center).y.clamp(-rustgis_geom::MAX_LAT, rustgis_geom::MAX_LAT);
    f64::from(w) * camera.res * lat.to_radians().cos() / 1000.0
}

/// The longitudes at a `w` × `h` point map's left and right edges, kept within ±180°.
pub fn lon_range(camera: Camera, size: (f32, f32)) -> (f64, f64) {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    let lon = |sx: f64| rustgis_geom::merc_to_lonlat(camera.to_map(sx, h / 2.0, w, h)).x.clamp(-180.0, 180.0);
    (lon(0.0), lon(w))
}

/// The latitudes at a `w` × `h` point map's bottom and top edges.
pub fn lat_range(camera: Camera, size: (f32, f32)) -> (f64, f64) {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    let lat =
        |sy: f64| rustgis_geom::merc_to_lonlat(camera.to_map(w / 2.0, sy, w, h)).y.clamp(-rustgis_geom::MAX_LAT, rustgis_geom::MAX_LAT);
    (lat(h), lat(0.0))
}

/// The lon/lat extent of the visible data: the union of `boxes` (lon/lat), kept within the world.
/// The extent is kept between −180° and 180° (an exported map is at most one world wide), so data
/// crossing the antimeridian (spanning both ends) gets the world's whole width, with its own
/// latitudes.
pub fn data_extent(boxes: impl IntoIterator<Item = Extent>) -> Option<Extent> {
    let mut all = Extent::EMPTY;
    for b in boxes {
        if [b.min_x, b.min_y, b.max_x, b.max_y].iter().all(|v| v.is_finite()) && b.min_x <= b.max_x && b.min_y <= b.max_y {
            all = all.union(&b);
        }
    }
    if all.is_empty() {
        return None;
    }
    let lat = rustgis_geom::MAX_LAT;
    Some(Extent::new(
        all.min_x.clamp(-180.0, 180.0),
        all.min_y.clamp(-lat, lat),
        all.max_x.clamp(-180.0, 180.0),
        all.max_y.clamp(-lat, lat),
    ))
}

/// The camera that shows the lon/lat extent `e` whole in an area `area` points large, with
/// [`FIT_MARGIN`] around it: a point or a tiny area stops at street level (zoom 16), and the map
/// is never more than one world across, nor past a pole.
pub fn fit_extent(e: &Extent, area: (f32, f32)) -> Camera {
    let (w, h) = (f64::from(area.0.max(1.0)), f64::from(area.1.max(1.0)));
    let m = rustgis_geom::extent_to_merc(e);
    let pad = 1.0 + 2.0 * FIT_MARGIN;
    let res = (m.width() * pad / w).max(m.height() * pad / h);
    let street = crate::camera::res_of(16.0);
    let res = if res.is_finite() { res.max(street) } else { street };
    let mut cam = Camera { center: m.center(), res: res.min(2.0 * HALF_WORLD / w) };
    if !cam.center.is_finite() {
        cam.center = rustgis_geom::Coord::new(0.0, 0.0);
    }
    crate::camera::clamp(&mut cam, w, h);
    let lim_x = (HALF_WORLD - w / 2.0 * cam.res).max(0.0);
    cam.center.x = cam.center.x.clamp(-lim_x, lim_x);
    cam
}

/// What frames the exported map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Framing {
    /// The visible data's lon/lat extent ([`data_extent`]), with a margin.
    Data(Extent),
    /// The view the user saw: `center` and `res` over `seen` points.
    View { center: rustgis_geom::Coord, res: f64, seen: (f32, f32) },
}

impl Framing {
    /// The camera for a map area `area` points large.
    pub fn camera(&self, area: (f32, f32)) -> Camera {
        match *self {
            Framing::Data(e) => fit_extent(&e, area),
            Framing::View { center, res, seen } => fit_camera(center, res, seen, area),
        }
    }
}

/// The map's place on the sheet, its camera and its furniture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub area: Area,
    pub camera: Camera,
    pub furniture: Furniture,
    /// What the rules alone give.
    pub auto: Furniture,
    /// The ground width across the centre (km) and the latitude span (degrees) the rules read.
    pub width_km: f64,
    pub lat_span: f64,
}

/// Plans the map on a sheet `size` points large: framed by `framing`, its furniture chosen by the
/// rules ([`auto_furniture`]) unless `overrides` say otherwise. The rules read the map as it would
/// be without a frame, so taking the labels' margin never flips the choice.
pub fn plan(size: (f32, f32), has_title: bool, framing: &Framing, overrides: Overrides) -> Plan {
    let open = map_area(size, has_title, false);
    let cam = framing.camera((open.w, open.h));
    let width_km = ground_width_km(cam, open.w);
    let (south, north) = lat_range(cam, (open.w, open.h));
    let auto = auto_furniture(width_km, north - south);
    let furniture = overrides.apply(auto);
    let area = map_area(size, has_title, furniture.labels);
    let camera = if furniture.labels { framing.camera((area.w, area.h)) } else { cam };
    Plan { area, camera, furniture, auto, width_km, lat_span: north - south }
}

/// The multiples of `step` from `lo` to `hi`.
pub fn ticks(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    if step.is_nan() || step <= 0.0 || !lo.is_finite() || !hi.is_finite() || hi < lo {
        return Vec::new();
    }
    let (k0, k1) = ((lo / step - 1e-9).ceil() as i64, (hi / step + 1e-9).floor() as i64);
    // Rounded to the step's decimals, so 0.1 × 3 reads 0.3, and never −0.
    (k0..=k1.min(k0 + 10_000)).map(|k| ((k as f64 * step) * 1e6).round() / 1e6 + 0.0).collect()
}

/// The label interval for a side spanning `lo` to `hi` degrees: the finest of [`STEPS`] giving it
/// 3–6 labels, else the finest giving at most 6, else the coarsest (30°: the world's 360° gets 13).
pub fn graticule_step(lo: f64, hi: f64) -> f64 {
    let n = |s: f64| ticks(lo, hi, s).len();
    STEPS
        .iter()
        .copied()
        .find(|s| (3..=6).contains(&n(*s)))
        .or_else(|| STEPS.iter().copied().find(|s| n(*s) <= 6))
        .unwrap_or(STEPS[STEPS.len() - 1])
}

/// A frame label: "30°N", "0°", "120°E", "15°S", "180°", with the decimals `step` needs.
pub fn degree_label(v: f64, step: f64, lat: bool) -> String {
    let decimals = if step >= 1.0 - 1e-9 {
        0
    } else if ((step * 10.0) - (step * 10.0).round()).abs() < 1e-6 {
        1
    } else {
        2
    };
    let a = v.abs();
    let num = format!("{a:.decimals$}");
    let zero = a < 0.5 * 10f64.powi(-(decimals as i32));
    if zero {
        return "0°".into();
    }
    if !lat && (a - 180.0).abs() < 1e-9 {
        return "180°".into();
    }
    let hemi = match (lat, v > 0.0) {
        (true, true) => 'N',
        (true, false) => 'S',
        (false, true) => 'E',
        (false, false) => 'W',
    };
    format!("{num}°{hemi}")
}

/// Where the frame's ticks fall, in points from the map area's top-left corner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameTicks {
    pub lon_step: f64,
    pub lat_step: f64,
    /// Longitudes and their x along the top and bottom edges.
    pub lons: Vec<(f64, f32)>,
    /// Latitudes and their y along the sides: Mercator, so not evenly spaced.
    pub lats: Vec<(f64, f32)>,
}

/// The ticks for a map `size` points large seen through `camera`; ticks within a point of a corner
/// are left out (they would sit on the neatline's corner).
pub fn frame_ticks(camera: Camera, size: (f32, f32)) -> FrameTicks {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    let (west, east) = lon_range(camera, size);
    let (south, north) = lat_range(camera, size);
    let (lon_step, lat_step) = (graticule_step(west, east), graticule_step(south, north));
    let screen = |lon: f64, lat: f64| camera.to_screen(rustgis_geom::lonlat_to_merc(rustgis_geom::Coord::new(lon, lat)), w, h);
    let lons = ticks(west, east, lon_step)
        .into_iter()
        .map(|lon| (lon, screen(lon, 0.0).0 as f32))
        .filter(|(_, x)| *x > 1.0 && *x < size.0 - 1.0)
        .collect();
    let lats = ticks(south, north, lat_step)
        .into_iter()
        .map(|lat| (lat, screen(0.0, lat).1 as f32))
        .filter(|(_, y)| *y > 1.0 && *y < size.1 - 1.0)
        .collect();
    FrameTicks { lon_step, lat_step, lons, lats }
}

// ---------------------------------------------------------------------------------------------
// Where the data is

/// Which parts of the map area hold data (a coarse grid over it), so the legend keeps clear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Occupancy {
    cols: usize,
    rows: usize,
    cells: Vec<bool>,
}

impl Occupancy {
    /// The share (0–1) of `r` (in points, relative to the map area `map`) that holds data.
    pub fn share(&self, map: Area, r: Area) -> f32 {
        if self.cells.is_empty() || map.w <= 0.0 || map.h <= 0.0 {
            return 0.0;
        }
        let cx = |x: f32| (((x - map.x) / map.w) * self.cols as f32).clamp(0.0, self.cols as f32);
        let cy = |y: f32| (((y - map.y) / map.h) * self.rows as f32).clamp(0.0, self.rows as f32);
        let (c0, c1) = (cx(r.x).floor() as usize, cx(r.right()).ceil() as usize);
        let (r0, r1) = (cy(r.y).floor() as usize, cy(r.bottom()).ceil() as usize);
        let (mut n, mut hit) = (0usize, 0usize);
        for row in r0..r1.min(self.rows) {
            for col in c0..c1.min(self.cols) {
                n += 1;
                hit += usize::from(self.cells.get(row * self.cols + col).copied().unwrap_or(false));
            }
        }
        if n == 0 { 0.0 } else { hit as f32 / n as f32 }
    }

    /// Cells across and down.
    pub fn grid(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }
}

/// Where `map`'s data layers draw, at `camera` over an area `size` points large: the layers drawn
/// small on a blank ground with no tiles, every cell not left blank holding data.
pub fn occupancy(map: &mut Map, camera: Camera, size: (f32, f32)) -> Occupancy {
    const COLS: usize = 48;
    let ppp = (192.0 / size.0.max(1.0)).min(1.0);
    let tiles = std::mem::take(&mut map.tiles);
    let basemap = std::mem::replace(&mut map.basemap, rustgis_map::Basemap::Blank);
    let saved = map.camera;
    map.camera = camera;
    let view = View { width: f64::from(size.0), height: f64::from(size.1), pixels_per_point: f64::from(ppp), camera, dark: false };
    let drawn = rustgis_render::render(map, &view);
    map.tiles = tiles;
    map.basemap = basemap;
    map.camera = saved;
    let Some(r) = drawn else { return Occupancy::default() };
    let (w, h) = (r.pixmap.width() as usize, r.pixmap.height() as usize);
    let rows = ((COLS as f32 * size.1 / size.0.max(1.0)).round() as usize).clamp(1, 4 * COLS);
    let mut cells = vec![false; COLS * rows];
    for (i, px) in r.pixmap.pixels().iter().enumerate() {
        let c = px.demultiply();
        if c.red() < 0xF6 || c.green() < 0xF6 || c.blue() < 0xF6 {
            let (x, y) = (i % w, i / w);
            let (col, row) = (x * COLS / w.max(1), y * rows / h.max(1));
            if let Some(cell) = cells.get_mut(row.min(rows - 1) * COLS + col.min(COLS - 1)) {
                *cell = true;
            }
        }
    }
    Occupancy { cols: COLS, rows, cells }
}

// ---------------------------------------------------------------------------------------------
// The map, offscreen

/// The map drawn for an export.
pub struct MapImage {
    pub pixmap: Pixmap,
    /// The credits the map's tile layers asked for.
    pub attribution: Vec<String>,
    /// Some tiles couldn't be had in time (or at all): the image shows less detail.
    pub tiles_missing: bool,
    pub occupancy: Occupancy,
}

/// Draws `map` offscreen at `camera` into an image `pixels` large at `ppp` pixels per point,
/// waiting (up to a minute) for basemap tiles. Blocking: call it off the interface thread.
/// Selection highlights are not drawn.
pub fn render_map(mut map: Map, camera: Camera, pixels: (u32, u32), ppp: f32) -> Option<MapImage> {
    for l in &mut map.layers {
        l.selection.clear();
    }
    map.camera = camera;
    let size = (pixels.0 as f32 / ppp, pixels.1 as f32 / ppp);
    let view = View { width: f64::from(size.0), height: f64::from(size.1), pixels_per_point: f64::from(ppp), camera, dark: false };
    let r = rustgis_render::render_export(&mut map, &view)?;
    let occupancy = occupancy(&mut map, camera, size);
    Some(MapImage { pixmap: r.pixmap, attribution: r.attribution, tiles_missing: r.tiles_pending || !r.tile_errors.is_empty(), occupancy })
}

/// `pm` reduced to `w` × `h` by averaging (the preview's copy of the map).
pub fn downsample(pm: &Pixmap, w: u32, h: u32) -> Option<Pixmap> {
    let mut out = Pixmap::new(w.max(1), h.max(1))?;
    let (sw, sh) = (pm.width() as usize, pm.height() as usize);
    let (dw, dh) = (out.width() as usize, out.height() as usize);
    let src = pm.data();
    let dst = out.data_mut();
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh / dh).max(y * sh / dh + 1).min(sh));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw / dw).max(x * sw / dw + 1).min(sw));
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (sy * sw + sx) * 4;
                    for (k, a) in acc.iter_mut().enumerate() {
                        *a += u32::from(src.get(i + k).copied().unwrap_or(0));
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)).max(1) as u32;
            let o = (y * dw + x) * 4;
            for (k, a) in acc.iter().enumerate() {
                if let Some(d) = dst.get_mut(o + k) {
                    *d = (a / n) as u8;
                }
            }
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// Text

struct Face {
    regular: Option<FontArc>,
    semibold: Option<FontArc>,
    fallback: Option<FontArc>,
}

/// One glyph of a line, placed: which of the sheet's fonts (0 regular, 1 semibold, 2 fallback),
/// the glyph, its pen position on the baseline (pixels) and the character it shows.
#[derive(Clone, Copy, Debug)]
struct Placed {
    slot: usize,
    id: GlyphId,
    x: f32,
    y: f32,
    ch: char,
}

impl Face {
    fn new(f: &Fonts) -> Face {
        let load = |b: &'static [u8]| FontRef::try_from_slice(b).ok().map(FontArc::new);
        Face { regular: load(f.regular), semibold: load(f.semibold), fallback: load(f.fallback) }
    }

    /// The font in `slot` (0 regular, 1 semibold, 2 fallback).
    fn font(&self, slot: usize) -> Option<&FontArc> {
        match slot {
            0 => self.regular.as_ref(),
            1 => self.semibold.as_ref(),
            _ => self.fallback.as_ref(),
        }
    }

    fn primary_slot(&self, bold: bool) -> Option<usize> {
        if bold && self.semibold.is_some() { Some(1) } else { self.regular.as_ref().map(|_| 0) }
    }

    fn primary(&self, bold: bool) -> Option<&FontArc> {
        self.primary_slot(bold).and_then(|s| self.font(s))
    }

    /// The font slot and glyph for `c`: the primary font's, else the fallback's, else the
    /// primary's missing-glyph box.
    fn pick(&self, c: char, bold: bool) -> Option<(usize, &FontArc, GlyphId)> {
        let slot = self.primary_slot(bold)?;
        let first = self.font(slot)?;
        let g = first.glyph_id(c);
        if g.0 != 0 {
            return Some((slot, first, g));
        }
        if let Some(f) = &self.fallback {
            let g = f.glyph_id(c);
            if g.0 != 0 {
                return Some((2, f, g));
            }
        }
        Some((slot, first, g))
    }

    /// The scale giving `font` an em `px` pixels tall.
    fn scale(font: &FontArc, px: f32) -> PxScale {
        let h = font.height_unscaled();
        let em = font.units_per_em().unwrap_or(h);
        PxScale::from(px * h / em.max(1.0))
    }

    fn width(&self, text: &str, px: f32, bold: bool) -> f32 {
        let mut w = 0.0;
        let mut prev: Option<(usize, GlyphId)> = None;
        for c in text.chars() {
            let Some((slot, f, g)) = self.pick(c, bold) else { return text.chars().count() as f32 * px * 0.55 };
            let sf = f.as_scaled(Self::scale(f, px));
            if let Some((ps, pg)) = prev
                && ps == slot
            {
                w += sf.kern(pg, g);
            }
            w += sf.h_advance(g);
            prev = Some((slot, g));
        }
        w
    }

    /// The glyphs of one line with its left end at `x` and its middle at `y` (pixels).
    fn place(&self, text: &str, x: f32, y: f32, px: f32, bold: bool) -> Vec<Placed> {
        let Some(first) = self.primary(bold) else { return Vec::new() };
        let sf = first.as_scaled(Self::scale(first, px));
        let base = y + (sf.ascent() + sf.descent()) / 2.0;
        let mut pen = x;
        let mut prev: Option<(usize, GlyphId)> = None;
        let mut out = Vec::with_capacity(text.len());
        for c in text.chars() {
            let Some((slot, f, g)) = self.pick(c, bold) else { break };
            let scaled = f.as_scaled(Self::scale(f, px));
            if let Some((ps, pg)) = prev
                && ps == slot
            {
                pen += scaled.kern(pg, g);
            }
            out.push(Placed { slot, id: g, x: pen, y: base, ch: c });
            pen += scaled.h_advance(g);
            prev = Some((slot, g));
        }
        out
    }

    /// Draws one line with its left end at `x` and its middle at `y` (pixels).
    fn draw(&self, cv: &mut dyn Canvas, text: &str, x: f32, y: f32, px: f32, color: [u8; 4], bold: bool) {
        let glyphs = self.place(text, x, y, px, bold);
        if !glyphs.is_empty() {
            cv.glyphs(self, &glyphs, px, color);
        }
    }

    /// `text` cut to fit `max` pixels, with an ellipsis when cut.
    fn fit(&self, text: &str, px: f32, bold: bool, max: f32) -> String {
        if self.width(text, px, bold) <= max {
            return text.to_string();
        }
        let mut s: Vec<char> = text.chars().collect();
        while !s.is_empty() {
            s.pop();
            let t: String = s.iter().collect::<String>().trim_end().to_string() + "…";
            if self.width(&t, px, bold) <= max {
                return t;
            }
        }
        String::new()
    }

    /// `text` wrapped to lines at most `max` pixels wide (at spaces; anywhere in text without
    /// them, such as Chinese), at most `lines` of them, the last one cut with an ellipsis.
    fn wrap(&self, text: &str, px: f32, bold: bool, max: f32, lines: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut line = String::new();
        let mut rest = text.trim().to_string();
        while !rest.is_empty() {
            // The next piece: a word with its following space, or one character of text that
            // doesn't use spaces.
            let first = rest.chars().next().unwrap_or(' ');
            let piece: String = if first.is_ascii() || first.is_alphabetic() && first.len_utf8() < 3 {
                let end = rest.find(' ').map_or(rest.len(), |i| i + 1);
                rest[..end].to_string()
            } else {
                first.to_string()
            };
            rest = rest[piece.len()..].to_string();
            let candidate = format!("{line}{piece}");
            if self.width(candidate.trim_end(), px, bold) <= max || line.is_empty() {
                line = candidate;
            } else {
                out.push(line.trim_end().to_string());
                line = piece;
            }
        }
        if !line.trim().is_empty() {
            out.push(line.trim_end().to_string());
        }
        if out.len() > lines {
            let tail = out[lines - 1..].join(" ");
            out.truncate(lines - 1);
            out.push(self.fit(&tail, px, bold, max));
        }
        out.into_iter().map(|l| self.fit(&l, px, bold, max)).collect()
    }
}

/// Blends `color` with coverage `cov` into one pixel.
fn blend(pm: &mut Pixmap, x: i32, y: i32, c: [u8; 4], cov: f32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    if x < 0 || y < 0 || x >= w || y >= h || cov <= 0.0 {
        return;
    }
    let Some(px) = pm.pixels_mut().get_mut((y * w + x) as usize) else { return };
    let a = f32::from(c[3]) / 255.0 * cov.min(1.0);
    let inv = 1.0 - a;
    let ch = |src: u8, dst: u8| (f32::from(src) * a + f32::from(dst) * inv).round() as u8;
    let al = (a * 255.0 + f32::from(px.alpha()) * inv).round().min(255.0) as u8;
    let (r, g, b) = (ch(c[0], px.red()), ch(c[1], px.green()), ch(c[2], px.blue()));
    if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(r.min(al), g.min(al), b.min(al), al) {
        *px = p;
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing

/// What the sheet is drawn on, in the output's pixels: the pixels themselves (PNG, the preview) or
/// a PDF page (`pdf`), where the same drawing becomes vector shapes and text around the map's
/// image.
trait Canvas {
    fn fill_path(&mut self, path: &tiny_skia::Path, c: [u8; 4]);
    fn stroke_path(&mut self, path: &tiny_skia::Path, c: [u8; 4], stroke: &Stroke);
    fn fill_rect(&mut self, r: Rect, c: [u8; 4]);
    /// One line of text, placed by [`Face::place`], `px` pixels to the em.
    fn glyphs(&mut self, face: &Face, glyphs: &[Placed], px: f32, c: [u8; 4]);
    /// `pm` (the map) scaled into `r`.
    fn image(&mut self, pm: &Pixmap, r: Rect);
}

impl Canvas for Pixmap {
    fn fill_path(&mut self, path: &tiny_skia::Path, c: [u8; 4]) {
        Pixmap::fill_path(self, path, &paint(c), FillRule::Winding, Transform::identity(), None);
    }

    fn stroke_path(&mut self, path: &tiny_skia::Path, c: [u8; 4], stroke: &Stroke) {
        Pixmap::stroke_path(self, path, &paint(c), stroke, Transform::identity(), None);
    }

    fn fill_rect(&mut self, r: Rect, c: [u8; 4]) {
        Pixmap::fill_rect(self, r, &paint(c), Transform::identity(), None);
    }

    fn glyphs(&mut self, face: &Face, glyphs: &[Placed], px: f32, c: [u8; 4]) {
        for p in glyphs {
            let Some(f) = face.font(p.slot) else { continue };
            let glyph = p.id.with_scale_and_position(Face::scale(f, px), ab_glyph::point(p.x, p.y));
            if let Some(og) = f.outline_glyph(glyph) {
                let bb = og.px_bounds();
                og.draw(|gx, gy, cov| blend(self, bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32, c, cov));
            }
        }
    }

    fn image(&mut self, pm: &Pixmap, r: Rect) {
        let (sx, sy) = (r.width() / pm.width() as f32, r.height() / pm.height() as f32);
        let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..PixmapPaint::default() };
        self.draw_pixmap(0, 0, pm.as_ref(), &paint, Transform::from_row(sx, 0.0, 0.0, sy, r.left(), r.top()), None);
    }
}

fn paint(c: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c[0], c[1], c[2], c[3]));
    p.anti_alias = true;
    p
}

fn rounded(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let k = radius.min(r.width() / 2.0).min(r.height() / 2.0);
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    let mut pb = PathBuilder::new();
    pb.move_to(l + k, t);
    pb.line_to(rt - k, t);
    pb.quad_to(rt, t, rt, t + k);
    pb.line_to(rt, b - k);
    pb.quad_to(rt, b, rt - k, b);
    pb.line_to(l + k, b);
    pb.quad_to(l, b, l, b - k);
    pb.line_to(l, t + k);
    pb.quad_to(l, t, l + k, t);
    pb.close();
    pb.finish()
}

/// A card on the map: white, a hairline edge, 8 pt corners.
fn card(pm: &mut dyn Canvas, a: Area, ppp: f32) {
    let Some(r) = a.px(ppp) else { return };
    if let Some(path) = rounded(r, 8.0 * ppp) {
        pm.fill_path(&path, CARD);
        pm.stroke_path(&path, BORDER, &Stroke { width: ppp.max(1.0), ..Stroke::default() });
    }
}

fn fill_rect(pm: &mut dyn Canvas, x: f32, y: f32, w: f32, h: f32, c: [u8; 4]) {
    if let Some(r) = Rect::from_xywh(x, y, w.max(0.5), h.max(0.5)) {
        pm.fill_rect(r, c);
    }
}

fn line(pm: &mut dyn Canvas, a: (f32, f32), b: (f32, f32), width: f32, c: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.0, a.1);
    pb.line_to(b.0, b.1);
    if let Some(p) = pb.finish() {
        pm.stroke_path(&p, c, &Stroke { width, ..Stroke::default() });
    }
}

/// A legend swatch, 14 points square, its middle at (`x` + 7, `y`) in points.
fn swatch(pm: &mut dyn Canvas, s: &Swatch, x: f32, y: f32, ppp: f32, ground: [u8; 4]) {
    let (x, y, k) = (x * ppp, y * ppp, ppp);
    let side = 14.0 * k;
    let (left, top) = (x, y - side / 2.0);
    match s {
        Swatch::Area { fill, stroke } => {
            fill_rect(pm, left, top, side, side, ground);
            fill_rect(pm, left, top, side, side, *fill);
            if let Some(r) = Rect::from_xywh(left + 0.5 * k, top + 0.5 * k, side - k, side - k) {
                let path = PathBuilder::from_rect(r);
                pm.stroke_path(&path, *stroke, &Stroke { width: k, ..Stroke::default() });
            }
        }
        Swatch::Point { fill, stroke, diameter } => {
            let d = diameter.clamp(4.0, 16.0) * k;
            if let Some(c) = PathBuilder::from_circle(x + side / 2.0, y, d / 2.0) {
                pm.fill_path(&c, *fill);
                pm.stroke_path(&c, *stroke, &Stroke { width: 1.5 * k, ..Stroke::default() });
            }
        }
        Swatch::Line { color, width } => line(pm, (left, y), (left + side, y), width.clamp(1.0, 4.0) * k, *color),
        Swatch::NoData => {
            fill_rect(pm, left, top, side, side, NO_DATA);
            // The diagonal hatch, clipped to the square.
            let step = 4.0 * k;
            let mut c = step;
            while c < 2.0 * side {
                let (u0, u1) = ((c - side).max(0.0), c.min(side));
                line(pm, (left + u0, top + side - (c - u0)), (left + u1, top + side - (c - u1)), k, [0x9A, 0x9A, 0x9A, 0xFF]);
                c += step;
            }
        }
    }
}

/// The legend's size in points.
fn legend_size(face: &Face, blocks: &[LegendBlock], max_w: f32) -> (f32, f32) {
    let px = TEXT;
    let mut w: f32 = 120.0;
    let mut h = 12.0;
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            h += 10.0;
        }
        if !b.title.is_empty() {
            w = w.max(face.width(&b.title, px, true));
            h += LINE + 4.0;
        }
        for e in &b.entries {
            match e {
                Entry::Item { label, .. } => {
                    w = w.max(22.0 + face.width(label, px, false));
                    h += 20.0;
                }
                Entry::Ramp { .. } => {
                    w = w.max(RAMP_W);
                    h += 12.0 + LINE + 4.0;
                }
            }
        }
    }
    ((w + 24.0).min(max_w), h + 12.0)
}

fn draw_legend(pm: &mut dyn Canvas, face: &Face, blocks: &[LegendBlock], a: Area, ppp: f32, ground: [u8; 4]) {
    card(pm, a, ppp);
    let px = TEXT * ppp;
    let inner = a.w - 24.0;
    let (x, mut y) = (a.x + 12.0, a.y + 12.0);
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            y += 10.0;
        }
        if !b.title.is_empty() {
            let t = face.fit(&b.title, px, true, inner * ppp);
            face.draw(pm, &t, x * ppp, (y + LINE / 2.0) * ppp, px, PRIMARY, true);
            y += LINE + 4.0;
        }
        for e in &b.entries {
            match e {
                Entry::Item { swatch: s, label } => {
                    swatch(pm, s, x, y + 10.0, ppp, ground);
                    let t = face.fit(label, px, false, (inner - 22.0) * ppp);
                    face.draw(pm, &t, (x + 22.0) * ppp, (y + 10.0) * ppp, px, PRIMARY, false);
                    y += 20.0;
                }
                Entry::Ramp { stops, low, high } => {
                    let n = stops.len().max(1);
                    let w = RAMP_W.min(inner) / n as f32;
                    for (k, c) in stops.iter().enumerate() {
                        fill_rect(pm, (x + k as f32 * w) * ppp, (y + 1.0) * ppp, (w * ppp).ceil() + 0.5, 10.0 * ppp, *c);
                    }
                    y += 12.0;
                    face.draw(pm, low, x * ppp, (y + LINE / 2.0) * ppp, px, SECONDARY, false);
                    let hw = face.width(high, px, false);
                    face.draw(pm, high, (x + RAMP_W.min(inner)) * ppp - hw, (y + LINE / 2.0) * ppp, px, SECONDARY, false);
                    y += LINE + 4.0;
                }
            }
        }
    }
}

/// The scale bar card's size for a bar `len` points long.
fn scale_size(len: f32) -> (f32, f32) {
    (len + 12.0, 30.0)
}

/// Lays the sheet out: the bands, the map, the scale bar at the map's bottom left, and the legend
/// where it was dragged, else bottom left above the scale bar unless data is there (then the
/// corner with the least data).
pub fn layout(sheet: &Sheet, fonts: &Fonts, occupancy: Option<&Occupancy>) -> Layout {
    let face = Face::new(fonts);
    layout_with(sheet, &face, occupancy)
}

fn layout_with(sheet: &Sheet, face: &Face, occupancy: Option<&Occupancy>) -> Layout {
    let size = sheet.size_pt();
    let has_title = !sheet.title.trim().is_empty();
    let map = map_area(size, has_title, sheet.graticule.is_some());
    let title = has_title.then(|| Area::new(0.0, 0.0, size.0, TITLE_BAND));
    let footer = Area::new(0.0, size.1 - FOOTER, size.0, FOOTER);
    let scale = sheet.scale.as_ref().map(|(len, _)| {
        let (w, h) = scale_size(*len);
        Area::new(map.x + MARGIN, map.bottom() - MARGIN - h, w, h)
    });
    let legend = (!sheet.legend.is_empty()).then(|| {
        let (w, h) = legend_size(face, &sheet.legend, (map.w * 0.45).max(140.0));
        let clamp =
            |x: f32, y: f32| Area::new(x.clamp(map.x, (map.right() - w).max(map.x)), y.clamp(map.y, (map.bottom() - h).max(map.y)), w, h);
        if let Some((x, y)) = sheet.legend_at {
            return clamp(x, y);
        }
        let above_scale = scale.map_or(map.bottom() - MARGIN, |s| s.y - 8.0);
        let candidates = [
            clamp(map.x + MARGIN, above_scale - h),
            clamp(map.x + MARGIN, map.y + MARGIN),
            clamp(map.right() - MARGIN - w, map.bottom() - MARGIN - h),
            clamp(map.right() - MARGIN - w, map.y + MARGIN),
        ];
        clear_corner(&candidates, map, occupancy)
    });
    // The north arrow: top right, else another corner clear of the legend and the scale bar
    // (preferring one without data).
    let north = sheet.north_arrow.then(|| {
        let (w, h) = ARROW_CARD;
        let corners = [
            Area::new(map.right() - MARGIN - w, map.y + MARGIN, w, h),
            Area::new(map.x + MARGIN, map.y + MARGIN, w, h),
            Area::new(map.right() - MARGIN - w, map.bottom() - MARGIN - h, w, h),
            Area::new(map.x + MARGIN, map.bottom() - MARGIN - h, w, h),
        ];
        let free: Vec<Area> = corners.iter().copied().filter(|c| ![legend, scale].iter().flatten().any(|o| overlaps(c, o))).collect();
        if free.is_empty() { corners[0] } else { clear_corner(&free, map, occupancy) }
    });
    Layout { title, map, footer, legend, scale, north }
}

fn overlaps(a: &Area, b: &Area) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// The first of `candidates` with almost no data, else the least covered (the first unless
/// another is clearly better).
fn clear_corner(candidates: &[Area], map: Area, occupancy: Option<&Occupancy>) -> Area {
    let Some(occ) = occupancy else { return candidates[0] };
    candidates.iter().find(|c| occ.share(map, **c) < 0.03).copied().unwrap_or_else(|| {
        let mut best = candidates[0];
        for c in &candidates[1..] {
            if occ.share(map, *c) < occ.share(map, best) - 0.05 {
                best = *c;
            }
        }
        best
    })
}

/// The footer's lines: caption lines and the credits, each with its baseline row (0 or 1).
fn footer_lines(face: &Face, sheet: &Sheet, width: f32) -> (Vec<String>, String, usize) {
    let px = TEXT;
    let inner = width - 2.0 * SIDE;
    let credit_w = face.width(&sheet.attribution, px, false);
    // Credits that would leave the caption less than half the width get a line of their own.
    if credit_w > inner * 0.5 {
        let credits = face.fit(&sheet.attribution, px, false, inner);
        let caption = face.wrap(&sheet.caption, px, false, inner, 1);
        (caption, credits, 1)
    } else {
        let caption = face.wrap(&sheet.caption, px, false, inner - credit_w - 24.0, 2);
        let row = caption.len().saturating_sub(1);
        (caption, sheet.attribution.clone(), row)
    }
}

/// Draws the whole sheet: `map` (the map area's pixels at the sheet's scale) and everything
/// around and on it.
pub fn compose(map: &Pixmap, sheet: &Sheet, fonts: &Fonts, occupancy: Option<&Occupancy>) -> Option<(Pixmap, Layout)> {
    let face = Face::new(fonts);
    let lay = layout_with(sheet, &face, occupancy);
    let mut pm = Pixmap::new(sheet.width, sheet.height)?;
    pm.fill(Color::WHITE);
    draw_sheet(&mut pm, map, sheet, &face, &lay);
    Some((pm, lay))
}

/// The sheet as a one-page PDF `page_mm` (width, height) large: the map as an image at the
/// sheet's full resolution, and everything around and on it — title, caption, credits, legend,
/// scale bar, north arrow, the latitude and longitude frame — as vector shapes and selectable
/// text in the sheet's fonts (subsets embedded, Chinese included). `title` is the document's
/// title in the PDF's properties.
pub fn compose_pdf(
    map: &Pixmap,
    sheet: &Sheet,
    fonts: &Fonts,
    occupancy: Option<&Occupancy>,
    page_mm: (f64, f64),
    title: &str,
) -> Result<Vec<u8>, String> {
    let face = Face::new(fonts);
    let lay = layout_with(sheet, &face, occupancy);
    let mut page = pdf::PdfCanvas::new(sheet.width, sheet.height, page_mm);
    page.fill_rect(
        Rect::from_xywh(0.0, 0.0, sheet.width as f32, sheet.height as f32).ok_or("The sheet has no size.")?,
        [255, 255, 255, 255],
    );
    draw_sheet(&mut page, map, sheet, &face, &lay);
    page.finish(&face, fonts, title)
}

/// Draws the sheet on `pm` (cleared to white): `map` (the map area's pixels at the sheet's scale)
/// and everything around and on it.
fn draw_sheet(pm: &mut dyn Canvas, map: &Pixmap, sheet: &Sheet, face: &Face, lay: &Layout) {
    let lay = *lay;
    let ppp = sheet.ppp;
    // The map, scaled to its area when its pixel size differs by rounding.
    if let Some(r) = lay.map.px(ppp) {
        pm.image(map, r);
    }
    if let Some(t) = lay.title {
        let px = TITLE * ppp;
        let text = face.fit(sheet.title.trim(), px, true, (t.w - 2.0 * SIDE) * ppp);
        face.draw(pm, &text, SIDE * ppp, (t.y + t.h / 2.0 + 2.0) * ppp, px, PRIMARY, true);
        // A framed map has its neatline: the band's rule would only double it.
        if sheet.graticule.is_none() {
            line(pm, (0.0, t.bottom() * ppp), (t.w * ppp, t.bottom() * ppp), ppp.max(1.0), BORDER);
        }
    }
    // Footer: caption at the left, credits at the right of the last caption line (or a line of
    // their own when long).
    let f = lay.footer;
    if sheet.graticule.is_none() {
        line(pm, (0.0, f.y * ppp), (f.w * ppp, f.y * ppp), ppp.max(1.0), BORDER);
    }
    if let Some(g) = sheet.graticule.filter(|g| g.grid) {
        draw_grid(pm, &g, lay.map, ppp);
    }
    let (caption, credits, credit_row) = footer_lines(face, sheet, f.w);
    let rows = caption.len().max(credit_row + 1).max(1);
    let top = f.y + (f.h - rows as f32 * LINE) / 2.0;
    let px = TEXT * ppp;
    for (i, l) in caption.iter().enumerate() {
        face.draw(pm, l, SIDE * ppp, (top + (i as f32 + 0.5) * LINE) * ppp, px, SECONDARY, false);
    }
    let cw = face.width(&credits, px, false);
    face.draw(pm, &credits, (f.w - SIDE) * ppp - cw, (top + (credit_row as f32 + 0.5) * LINE) * ppp, px, SECONDARY, false);
    // Scale bar: a line with end ticks and its distance, as on screen.
    if let (Some(a), Some((len, label))) = (lay.scale, sheet.scale.as_ref()) {
        card(pm, a, ppp);
        let (x0, y) = ((a.x + 6.0) * ppp, (a.bottom() - 6.0) * ppp);
        let x1 = x0 + len * ppp;
        let w = 1.5 * ppp;
        line(pm, (x0, y), (x1, y), w, PRIMARY);
        line(pm, (x0, y + w / 2.0), (x0, y - 5.0 * ppp), w, PRIMARY);
        line(pm, (x1, y + w / 2.0), (x1, y - 5.0 * ppp), w, PRIMARY);
        face.draw(pm, label, x0, y - 13.0 * ppp, px, PRIMARY, false);
    }
    if let Some(a) = lay.legend {
        draw_legend(pm, face, &sheet.legend, a, ppp, sheet.ground);
    }
    if let Some(a) = lay.north {
        draw_north(pm, face, a, ppp);
    }
    if let Some(g) = &sheet.graticule {
        draw_frame(pm, face, g, lay.map, ppp, sheet.size_pt());
    }
}

/// Faint grid lines across the map at the frame's ticks.
fn draw_grid(pm: &mut dyn Canvas, g: &Graticule, map: Area, ppp: f32) {
    let t = frame_ticks(g.camera, (map.w, map.h));
    let w = (0.5 * ppp).max(1.0);
    for (_, x) in &t.lons {
        let x = (map.x + x) * ppp;
        line(pm, (x, map.y * ppp), (x, map.bottom() * ppp), w, GRID);
    }
    for (_, y) in &t.lats {
        let y = (map.y + y) * ppp;
        line(pm, (map.x * ppp, y), (map.right() * ppp, y), w, GRID);
    }
}

/// The frame: a thin neatline around the map, ticks pointing in, longitudes above and below,
/// latitudes at the sides, each label left out where it would crowd its neighbour or leave the
/// sheet.
fn draw_frame(pm: &mut dyn Canvas, face: &Face, g: &Graticule, map: Area, ppp: f32, sheet: (f32, f32)) {
    let t = frame_ticks(g.camera, (map.w, map.h));
    let lw = (0.75 * ppp).max(1.0);
    if let Some(r) = Rect::from_ltrb(map.x * ppp, map.y * ppp, map.right() * ppp, map.bottom() * ppp) {
        pm.stroke_path(&PathBuilder::from_rect(r), NEATLINE, &Stroke { width: lw, ..Stroke::default() });
    }
    let px = FRAME_TEXT * ppp;
    let gap = 5.0;
    let mut last_right = f32::NEG_INFINITY;
    for (lon, x) in &t.lons {
        let x = map.x + x;
        line(pm, (x * ppp, map.y * ppp), (x * ppp, (map.y + TICK) * ppp), lw, NEATLINE);
        line(pm, (x * ppp, map.bottom() * ppp), (x * ppp, (map.bottom() - TICK) * ppp), lw, NEATLINE);
        let text = degree_label(*lon, t.lon_step, false);
        let tw = face.width(&text, FRAME_TEXT, false);
        let left = x - tw / 2.0;
        if left < last_right + 8.0 || left < 2.0 || left + tw > sheet.0 - 2.0 {
            continue;
        }
        last_right = left + tw;
        face.draw(pm, &text, left * ppp, (map.y - FRAME_TB / 2.0) * ppp, px, SECONDARY, false);
        face.draw(pm, &text, left * ppp, (map.bottom() + FRAME_TB / 2.0) * ppp, px, SECONDARY, false);
    }
    let mut last_y = f32::NEG_INFINITY;
    // Top to bottom, so crowding is judged in reading order.
    let mut lats = t.lats.clone();
    lats.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (lat, y) in &lats {
        let y = map.y + y;
        line(pm, (map.x * ppp, y * ppp), ((map.x + TICK) * ppp, y * ppp), lw, NEATLINE);
        line(pm, (map.right() * ppp, y * ppp), ((map.right() - TICK) * ppp, y * ppp), lw, NEATLINE);
        if y < last_y + FRAME_TEXT + 4.0 {
            continue;
        }
        last_y = y;
        let text = degree_label(*lat, t.lat_step, true);
        let tw = face.width(&text, FRAME_TEXT, false);
        face.draw(pm, &text, (map.x - gap - tw).max(1.0) * ppp, y * ppp, px, SECONDARY, false);
        face.draw(pm, &text, (map.right() + gap) * ppp, y * ppp, px, SECONDARY, false);
    }
}

/// The north arrow on its card: an "N" over a slender split dart, its west half solid and its east
/// half open, as on a printed map.
fn draw_north(pm: &mut dyn Canvas, face: &Face, a: Area, ppp: f32) {
    card(pm, a, ppp);
    let cx = a.x + a.w / 2.0;
    let px = 11.0 * ppp;
    let nw = face.width("N", px, true);
    face.draw(pm, "N", cx * ppp - nw / 2.0, (a.y + 13.0) * ppp, px, PRIMARY, true);
    let (tip, base, notch, half) = (a.y + 23.0, a.bottom() - 7.0, a.bottom() - 12.5, 7.0);
    let pt = |x: f32, y: f32| (x * ppp, y * ppp);
    let shape = |pts: &[(f32, f32)]| {
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        pb.close();
        pb.finish()
    };
    let (t, l, n, r) = (pt(cx, tip), pt(cx - half, base), pt(cx, notch), pt(cx + half, base));
    if let Some(east) = shape(&[t, r, n]) {
        pm.fill_path(&east, [0xFF, 0xFF, 0xFF, 0xFF]);
    }
    if let Some(west) = shape(&[t, n, l]) {
        pm.fill_path(&west, PRIMARY);
    }
    if let Some(outline) = shape(&[t, r, n, l]) {
        let stroke = Stroke { width: ppp.max(1.0), line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
        pm.stroke_path(&outline, PRIMARY, &stroke);
    }
}

// ---------------------------------------------------------------------------------------------
// Output

/// The image as PNG bytes.
pub fn encode_png(pm: &Pixmap) -> Result<Vec<u8>, String> {
    pm.encode_png().map_err(|e| e.to_string())
}

/// The image for an egui texture.
pub fn color_image(pm: &Pixmap) -> egui::ColorImage {
    egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data())
}

/// The colour of one pixel (unpremultiplied RGBA); `None` outside the image.
pub fn pixel(pm: &Pixmap, x: u32, y: u32) -> Option<[u8; 4]> {
    let c = pm.pixel(x, y)?.demultiply();
    Some([c.red(), c.green(), c.blue(), c.alpha()])
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
    const INTER_SB: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
    const NOTO: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-Regular.otf");
    const FONTS: Fonts = Fonts { regular: INTER, semibold: INTER_SB, fallback: NOTO };

    fn sheet(width: u32, height: u32, ppp: f32) -> Sheet {
        Sheet {
            width,
            height,
            ppp,
            title: "Heat study".into(),
            caption: "Data: Districts (districts.geojson). Made with NimbleMap.".into(),
            attribution: "Natural Earth".into(),
            legend: vec![LegendBlock {
                title: "Population".into(),
                entries: vec![
                    Entry::Item {
                        swatch: Swatch::Area { fill: [200, 30, 30, 255], stroke: [140, 20, 20, 255] },
                        label: "Over 1,000,000".into(),
                    },
                    Entry::Item { swatch: Swatch::NoData, label: "No data".into() },
                ],
            }],
            legend_at: None,
            scale: Some((100.0, "10 km".into())),
            ground: [0xF6, 0xF5, 0xF1, 0xFF],
            north_arrow: false,
            graticule: None,
        }
    }

    fn blank(w: u32, h: u32) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::from_rgba8(0xF6, 0xF5, 0xF1, 0xFF));
        p
    }

    #[test]
    fn the_layout_keeps_bands_and_cards_inside() {
        let s = sheet(1920, 1080, 1.5);
        let lay = layout(&s, &FONTS, None);
        let (w, h) = s.size_pt();
        assert_eq!(lay.title.map(|t| t.h), Some(TITLE_BAND));
        assert!((lay.map.bottom() + FOOTER - h).abs() < 1e-3);
        let legend = lay.legend.unwrap();
        let scale = lay.scale.unwrap();
        // Legend bottom left, above the scale bar; both inside the map.
        assert!(legend.x < w / 2.0 && legend.bottom() <= scale.y);
        assert!(lay.map.contains(legend.x, legend.y) && lay.map.contains(scale.right(), scale.bottom()));
        // No title: the map starts at the top.
        let lay = layout(&Sheet { title: "  ".into(), ..s }, &FONTS, None);
        assert!(lay.title.is_none() && lay.map.y == 0.0);
    }

    #[test]
    fn the_legend_moves_off_the_data_and_stays_where_dragged() {
        let s = sheet(1080, 1080, 1.5);
        let map = map_area(s.size_pt(), true, false);
        // Data along the whole left half.
        let cols = 48;
        let rows = 48;
        let cells = (0..rows * cols).map(|i| i % cols < cols / 2).collect();
        let occ = Occupancy { cols, rows, cells };
        let lay = layout(&s, &FONTS, Some(&occ));
        let legend = lay.legend.unwrap();
        assert!(legend.x > map.w / 2.0, "moved right: {legend:?}");
        assert!(occ.share(map, legend) < 0.03);
        let dragged = layout(&Sheet { legend_at: Some((40.0, 200.0)), ..s.clone() }, &FONTS, Some(&occ)).legend.unwrap();
        assert_eq!((dragged.x, dragged.y), (40.0, 200.0));
        // Dragged too far: kept inside the map.
        let far = layout(&Sheet { legend_at: Some((5000.0, -50.0)), ..s }, &FONTS, None).legend.unwrap();
        assert!(far.right() <= map.right() + 1e-3 && far.y >= map.y);
    }

    #[test]
    fn composing_gives_the_exact_size_with_a_title_and_credits() {
        let s = sheet(1920, 1080, 1.5);
        let lay = layout(&s, &FONTS, None);
        let (mw, mh) = map_pixels(lay.map, s.ppp);
        let (pm, lay) = compose(&blank(mw, mh), &s, &FONTS, None).unwrap();
        assert_eq!((pm.width(), pm.height()), (1920, 1080));
        // Dark text in the title band and in the footer's right end (the credits).
        let dark_in = |a: Area| {
            let r = a.px(s.ppp).unwrap();
            (r.top() as u32..r.bottom() as u32)
                .flat_map(|y| (r.left() as u32..r.right() as u32).map(move |x| (x, y)))
                .filter(|(x, y)| pixel(&pm, *x, *y).is_some_and(|c| c[0] < 160))
                .count()
        };
        assert!(dark_in(lay.title.unwrap()) > 100);
        let f = lay.footer;
        assert!(dark_in(Area::new(f.w * 0.8, f.y, f.w * 0.2, f.h)) > 20, "credits at the right of the footer");
        // The legend's area swatch: the fill over the ground, exactly.
        let legend = lay.legend.unwrap();
        let (sx, sy) = (((legend.x + 12.0 + 7.0) * s.ppp) as u32, ((legend.y + 12.0 + LINE + 4.0 + 10.0) * s.ppp) as u32);
        assert_eq!(pixel(&pm, sx, sy), Some([200, 30, 30, 255]));
    }

    #[test]
    fn the_preview_is_the_same_layout_smaller() {
        let s = sheet(2480, 3508, 2480.0 / 793.7);
        let p = s.at_scale(0.2);
        assert_eq!(p.width, 496);
        let (a, b) = (layout(&s, &FONTS, None), layout(&p, &FONTS, None));
        let close = |x: f32, y: f32| (x - y).abs() < 1.0;
        assert!(close(a.map.h, b.map.h) && close(a.legend.unwrap().y, b.legend.unwrap().y));
        let small = downsample(&blank(400, 300), 40, 30).unwrap();
        assert_eq!(pixel(&small, 10, 10), Some([0xF6, 0xF5, 0xF1, 0xFF]));
    }

    #[test]
    fn long_captions_wrap_and_long_credits_get_their_own_line() {
        let face = Face::new(&FONTS);
        let long =
            "Data: Districts (districts.geojson); Heat (summer surface temperature, 2024); Schools (schools.csv). Made with NimbleMap.";
        let lines = face.wrap(long, TEXT, false, 300.0, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| face.width(l, TEXT, false) <= 300.0));
        assert!(lines[1].ends_with('…'));
        let zh = face.wrap("数据：上海市各区人口与夏季地表温度，由某某大学提供。使用 NimbleMap 制作。", TEXT, false, 120.0, 3);
        assert!(zh.len() >= 2);
        let s = Sheet { attribution: "NASA Blue Marble (NASA Earth Observatory, GIBS) · Natural Earth".into(), ..sheet(1080, 1080, 1.5) };
        let (_, _, row) = footer_lines(&face, &s, 400.0);
        assert_eq!(row, 1);
    }

    #[test]
    fn a_map_is_drawn_offscreen_at_the_asked_size() {
        let mut map = Map::new("t");
        map.basemap = rustgis_map::Basemap::Paper;
        let cam = Camera { center: rustgis_geom::lonlat_to_merc(rustgis_geom::Coord::new(121.47, 31.23)), res: 300.0 };
        let img = render_map(map, cam, (900, 600), 1.5).unwrap();
        assert_eq!((img.pixmap.width(), img.pixmap.height()), (900, 600));
        assert!(!img.tiles_missing);
        // No data layers: nothing occupied.
        assert_eq!(img.occupancy.share(Area::new(0.0, 0.0, 600.0, 400.0), Area::new(0.0, 0.0, 600.0, 400.0)), 0.0);
    }

    /// Walled cities of China, 1866 (1,789 city walls, most 1–3 km across) exported for the whole
    /// country: every wall still shows, at least as a dot, though each is under a pixel wide.
    #[test]
    fn walled_cities_survive_a_whole_china_export() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/examples/walled cities 1866.geojson");
        let text = std::fs::read_to_string(path).unwrap();
        let (ds, _) = rustgis_render::rustgis_data::io::read_geojson(&text, "walls").unwrap();
        let centres: Vec<rustgis_geom::Coord> = ds.features.iter().filter_map(|f| f.geometry.as_ref()?.label_point()).collect();
        assert!(centres.len() > 1700);
        let mut map = Map::new("t");
        map.basemap = rustgis_map::Basemap::Paper;
        map.graticule = false;
        let id = map.add_dataset(ds);
        let red = rustgis_map::Rgba::rgb(230, 20, 20);
        if let Some(l) = map.layer_mut(id) {
            l.renderer = rustgis_map::Renderer::Single(rustgis_map::Symbol {
                fill: red.with_alpha(115),
                stroke: red,
                stroke_width: 1.0,
                size: 0.0,
                marker: Default::default(),
            });
        }
        // The slide preset (1920 × 1080 at 1.5 pixels per point), fitted to China's walls.
        let p = plan((1280.0, 720.0), false, &Framing::Data(Extent::new(73.0, 18.0, 135.0, 53.0)), Overrides::default());
        let ppp = 1.5;
        let pixels = map_pixels(p.area, ppp);
        let img = render_map(map, p.camera, pixels, ppp).unwrap();
        let (w, h) = (f64::from(pixels.0) / f64::from(ppp), f64::from(pixels.1) / f64::from(ppp));
        let reddish = |x: i64, y: i64| {
            let px = u32::try_from(x).ok().zip(u32::try_from(y).ok()).and_then(|(x, y)| img.pixmap.pixel(x, y));
            px.is_some_and(|q| i32::from(q.red()) - i32::from(q.green()) > 40)
        };
        let shown = centres
            .iter()
            .filter(|c| {
                let (sx, sy) = p.camera.to_screen(rustgis_geom::lonlat_to_merc(**c), w, h);
                let (px, py) = ((sx * f64::from(ppp)).round() as i64, (sy * f64::from(ppp)).round() as i64);
                (-1..=1).any(|dx| (-1..=1).any(|dy| reddish(px + dx, py + dy)))
            })
            .count();
        assert!(shown * 100 >= centres.len() * 99, "{shown} of {} walls drawn", centres.len());
    }

    #[test]
    fn fitting_shows_the_whole_view() {
        let c = rustgis_geom::Coord::new(0.0, 0.0);
        // A wide screen view into a tall page: the page zooms out by the width ratio.
        let cam = fit_camera(c, 100.0, (1200.0, 700.0), (600.0, 900.0));
        assert!((cam.res - 200.0).abs() < 1e-9);
    }

    fn ll(lon: f64, lat: f64) -> rustgis_geom::Coord {
        rustgis_geom::Coord::new(lon, lat)
    }

    /// The camera fitting `e` into the slide's open map area, and the rules' reading of it.
    fn slide_plan(e: Extent) -> Plan {
        plan((1280.0, 720.0), true, &Framing::Data(e), Overrides::default())
    }

    #[test]
    fn the_rules_pick_furniture_by_ground_width() {
        // The table itself.
        assert_eq!(auto_furniture(120.0, 1.0), Furniture { north_arrow: true, scale_bar: true, labels: false });
        assert_eq!(auto_furniture(ARROW_MAX_KM, 12.0), Furniture { north_arrow: true, scale_bar: true, labels: false });
        assert_eq!(auto_furniture(3_000.0, 20.0), Furniture { north_arrow: false, scale_bar: true, labels: true });
        assert_eq!(auto_furniture(3_000.0, SCALE_MAX_LAT_SPAN), Furniture { north_arrow: false, scale_bar: false, labels: true });
        assert_eq!(auto_furniture(LABELS_ONLY_KM + 1.0, 10.0), Furniture { north_arrow: false, scale_bar: false, labels: true });
        // Shanghai: about 120 km across.
        let p = slide_plan(Extent::new(120.85, 30.67, 122.2, 31.88));
        assert!(p.width_km < 400.0, "{}", p.width_km);
        assert_eq!(p.furniture, Furniture { north_arrow: true, scale_bar: true, labels: false });
        assert_eq!(p.area, map_area((1280.0, 720.0), true, false));
        // The contiguous United States (24.5°N–49.4°N): about 27° of latitude with the margin and
        // 6,300 km across, under both limits, so labels and a scale bar (owner's decision).
        let us = slide_plan(Extent::new(-124.8, 24.5, -66.9, 49.4));
        assert!(us.width_km > ARROW_MAX_KM && us.width_km <= LABELS_ONLY_KM, "{}", us.width_km);
        assert!(us.lat_span < SCALE_MAX_LAT_SPAN, "{}", us.lat_span);
        assert_eq!(us.furniture, Furniture { north_arrow: false, scale_bar: true, labels: true });
        assert_eq!(us.area, map_area((1280.0, 720.0), true, true), "framed for its labels");
        // The same width with less latitude (a band 15° tall) keeps its scale bar.
        let band = slide_plan(Extent::new(-110.0, 30.0, -80.0, 42.0));
        assert!(band.width_km > ARROW_MAX_KM && band.width_km <= LABELS_ONLY_KM, "{}", band.width_km);
        assert_eq!(band.furniture, Furniture { north_arrow: false, scale_bar: true, labels: true });
        // The world: labels only.
        let w = slide_plan(Extent::new(-180.0, -60.0, 180.0, 80.0));
        assert!(w.width_km > LABELS_ONLY_KM);
        assert_eq!(w.furniture, Furniture { north_arrow: false, scale_bar: false, labels: true });
        // Overrides win over the rules.
        let o = Overrides { north_arrow: Show::Off, scale_bar: Show::Auto, labels: Show::On };
        let p = plan((1280.0, 720.0), true, &Framing::Data(Extent::new(120.85, 30.67, 122.2, 31.88)), o);
        assert_eq!(p.furniture, Furniture { north_arrow: false, scale_bar: true, labels: true });
        assert_eq!(p.auto, Furniture { north_arrow: true, scale_bar: true, labels: false });
    }

    #[test]
    fn fitting_the_data_contains_every_feature_with_a_margin() {
        let boxes = [Extent::new(121.2, 31.0, 121.3, 31.1), Extent::new(121.9, 31.5, 121.9, 31.5), Extent::new(120.9, 30.7, 121.0, 30.8)];
        let e = data_extent(boxes).unwrap();
        assert_eq!((e.min_x, e.min_y, e.max_x, e.max_y), (120.9, 30.7, 121.9, 31.5));
        for area in [(1280.0, 608.0), (794.0, 1011.0), (720.0, 608.0)] {
            let cam = fit_extent(&e, area);
            let (w, h) = (f64::from(area.0), f64::from(area.1));
            for b in &boxes {
                for c in [ll(b.min_x, b.min_y), ll(b.max_x, b.max_y)] {
                    let (x, y) = cam.to_screen(rustgis_geom::lonlat_to_merc(c), w, h);
                    assert!(x >= 0.0 && x <= w && y >= 0.0 && y <= h, "{c:?} at {x},{y} outside {area:?}");
                }
            }
            // The tighter side keeps a 5% margin each way: the data fills 1 / 1.1 of it.
            let m = rustgis_geom::extent_to_merc(&e);
            let fill = (m.width() / (cam.res * w)).max(m.height() / (cam.res * h));
            assert!((fill - 1.0 / 1.1).abs() < 1e-6, "{fill}");
        }
        // Nothing to fit.
        assert!(data_extent([Extent::EMPTY]).is_none());
        // Across the antimeridian (Fiji, the Aleutians): the world's width, the data's latitudes.
        let e = data_extent([Extent::new(172.0, 51.0, 180.0, 55.0), Extent::new(-180.0, 50.0, -165.0, 54.0)]).unwrap();
        assert_eq!((e.min_x, e.max_x, e.min_y, e.max_y), (-180.0, 180.0, 50.0, 55.0));
        let cam = fit_extent(&e, (1280.0, 608.0));
        assert!(cam.center.x.abs() < 1e-6 && (cam.res * 1280.0 - 2.0 * HALF_WORLD).abs() < 1.0, "one world across");
        // The whole world, poles and all, in a wide slide: one world across, never past a pole.
        let cam = fit_extent(&Extent::new(-180.0, -90.0, 180.0, 90.0), (1280.0, 608.0));
        let (s, n) = lat_range(cam, (1280.0, 608.0));
        assert!(n <= rustgis_geom::MAX_LAT + 1e-6 && s >= -rustgis_geom::MAX_LAT - 1e-6);
        assert!(cam.res * 1280.0 <= 2.0 * HALF_WORLD + 1.0);
        // A lone point stops at street level.
        let cam = fit_extent(&Extent::new(121.5, 31.2, 121.5, 31.2), (720.0, 600.0));
        assert!((cam.res - crate::camera::res_of(16.0)).abs() < 1e-9);
    }

    #[test]
    fn the_interval_gives_three_to_six_labels_a_side() {
        assert_eq!(ticks(-0.05, 0.31, 0.1), vec![0.0, 0.1, 0.2, 0.3]);
        assert_eq!(graticule_step(120.85, 122.2), 0.25);
        assert_eq!(graticule_step(30.6, 31.9), 0.25);
        assert_eq!(graticule_step(-130.0, -62.0), 15.0);
        assert_eq!(graticule_step(22.0, 52.0), 5.0);
        assert_eq!(graticule_step(-180.0, 180.0), 60.0, "the world: every 60°, six longitudes");
        assert_eq!(ticks(-180.0, 180.0, 60.0).len(), 7, "-180 to 180 inclusive");
        assert_eq!(graticule_step(10.0, 10.05), 0.1, "too small for three: the finest");
        // Over a sweep of spans, every pick but the extremes gives 3–6.
        for k in 1..400 {
            let span = 0.35 * 1.02f64.powi(k);
            if span > 180.0 {
                break;
            }
            let lo = -7.3 + k as f64 * 0.37;
            let n = ticks(lo, lo + span, graticule_step(lo, lo + span)).len();
            assert!((2..=6).contains(&n), "span {span} from {lo}: {n} labels");
        }
    }

    #[test]
    fn degree_labels_read_like_an_atlas() {
        assert_eq!(degree_label(30.0, 10.0, true), "30°N");
        assert_eq!(degree_label(-15.0, 5.0, true), "15°S");
        assert_eq!(degree_label(0.0, 5.0, true), "0°");
        assert_eq!(degree_label(120.0, 30.0, false), "120°E");
        assert_eq!(degree_label(-75.0, 15.0, false), "75°W");
        assert_eq!(degree_label(180.0, 30.0, false), "180°");
        assert_eq!(degree_label(-180.0, 30.0, false), "180°");
        assert_eq!(degree_label(121.25, 0.25, false), "121.25°E");
        assert_eq!(degree_label(31.5, 0.5, true), "31.5°N");
        assert_eq!(degree_label(-0.0, 0.1, true), "0°");
        assert_eq!(degree_label(0.30000000000000004, 0.1, false), "0.3°E");
    }

    #[test]
    fn latitude_ticks_follow_mercator_not_a_straight_line() {
        let cam = fit_extent(&Extent::new(-124.8, 24.5, -66.9, 49.4), (1192.0, 568.0));
        let size = (1192.0, 568.0);
        let t = frame_ticks(cam, size);
        assert!(t.lats.len() >= 3 && t.lons.len() >= 3, "{t:?}");
        for (lat, y) in &t.lats {
            let (_, sy) = cam.to_screen(rustgis_geom::lonlat_to_merc(ll(-100.0, *lat)), 1192.0, 568.0);
            assert!((f64::from(*y) - sy).abs() < 1e-3, "{lat}: {y} vs {sy}");
        }
        for (lon, x) in &t.lons {
            let (sx, _) = cam.to_screen(rustgis_geom::lonlat_to_merc(ll(*lon, 37.0)), 1192.0, 568.0);
            assert!((f64::from(*x) - sx).abs() < 1e-3);
        }
        // Equal steps of latitude are further apart going north.
        let gaps: Vec<f32> = t.lats.windows(2).map(|w| (w[0].1 - w[1].1).abs()).collect();
        assert!(gaps.windows(2).all(|g| g[1] > g[0]), "{gaps:?}");
        // And not where a linear spread would put them.
        let (s, n) = lat_range(cam, size);
        let (lat, y) = t.lats[1];
        let linear = (n - lat) / (n - s) * 568.0;
        assert!((f64::from(y) - linear).abs() > 2.0, "{y} vs linear {linear}");
    }

    /// Pixels darker than mid-grey in `a` (points).
    fn dark(pm: &Pixmap, ppp: f32, a: Area) -> usize {
        let r = a.px(ppp).unwrap();
        (r.top() as u32..r.bottom() as u32)
            .flat_map(|y| (r.left() as u32..r.right() as u32).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(pm, *x, *y).is_some_and(|c| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]) < 3 * 120))
            .count()
    }

    #[test]
    fn the_north_arrow_and_frame_labels_are_drawn_when_asked() {
        for (w, h, ppp) in [(1920, 1080, 1.5), (2480, 3508, 2480.0 / 793.7)] {
            let s = Sheet { north_arrow: true, ..sheet(w, h, ppp) };
            let lay = layout(&s, &FONTS, None);
            let (mw, mh) = map_pixels(lay.map, ppp);
            let (pm, lay) = compose(&blank(mw, mh), &s, &FONTS, None).unwrap();
            let north = lay.north.expect("a north arrow");
            // Top right of the map, clear of the legend (bottom left), with its N and dart.
            assert!(north.x > lay.map.w / 2.0 && north.y < lay.map.y + lay.map.h / 2.0, "{north:?}");
            assert!(!overlaps(&north, &lay.legend.unwrap()));
            assert!(dark(&pm, ppp, north) as f32 > 40.0 * ppp * ppp, "the arrow is drawn: {}", dark(&pm, ppp, north));
            // Off: no arrow, and the corner is plain map.
            let (pm2, lay2) = compose(&blank(mw, mh), &sheet(w, h, ppp), &FONTS, None).unwrap();
            assert!(lay2.north.is_none());
            assert_eq!(dark(&pm2, ppp, north), 0);
            // The frame: the map inset, labels above and at the left, nothing over the map.
            let cam = fit_extent(&Extent::new(-124.8, 24.5, -66.9, 49.4), (1000.0, 600.0));
            let s = Sheet { graticule: Some(Graticule { camera: cam, grid: false }), ..sheet(w, h, ppp) };
            let lay = layout(&s, &FONTS, None);
            assert_eq!(lay.map, map_area(s.size_pt(), true, true));
            let (mw, mh) = map_pixels(lay.map, ppp);
            let (pm, lay) = compose(&blank(mw, mh), &s, &FONTS, None).unwrap();
            let m = lay.map;
            let top = Area::new(m.x, m.y - FRAME_TB + 1.0, m.w, FRAME_TB - 3.0);
            let left = Area::new(1.0, m.y + 2.0, FRAME_SIDE - 3.0, m.h - 4.0);
            let text = (ppp * ppp * 30.0) as usize;
            assert!(dark(&pm, ppp, top) > text, "longitudes above: {}", dark(&pm, ppp, top));
            assert!(dark(&pm, ppp, left) > text, "latitudes at the left");
            // Grid lines are off by default: the map's middle stays clear.
            let mid = Area::new(m.x + m.w * 0.55, m.y + 30.0, m.w * 0.4, m.h * 0.3);
            let grid = |pm: &Pixmap| {
                let r = mid.px(ppp).unwrap();
                (r.top() as u32..r.bottom() as u32)
                    .flat_map(|y| (r.left() as u32..r.right() as u32).map(move |x| (x, y)))
                    .filter(|(x, y)| pixel(pm, *x, *y).is_some_and(|c| c[0] < 0xF0))
                    .count()
            };
            assert_eq!(grid(&pm), 0);
            let s = Sheet { graticule: Some(Graticule { camera: cam, grid: true }), ..s };
            let (pm, _) = compose(&blank(mw, mh), &s, &FONTS, None).unwrap();
            assert!(grid(&pm) > 0, "grid lines when asked");
        }
    }

    #[test]
    fn png_comes_out() {
        let pm = blank(30, 20);
        let png = encode_png(&pm).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }

    /// A map with something in every pixel: a gradient with a checker over it.
    fn painted(w: u32, h: u32) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        for (i, px) in p.pixels_mut().iter_mut().enumerate() {
            let (x, y) = (i as u32 % w, i as u32 / w);
            let check = if (x / 7 + y / 5) % 2 == 0 { 40 } else { 0 };
            *px = tiny_skia::PremultipliedColorU8::from_rgba((x * 255 / w) as u8, (y * 255 / h) as u8, 200 - check, 255).unwrap();
        }
        p
    }

    /// The PDF's one page: its size in points, its text and its images (width, height, RGB).
    fn read_pdf(bytes: &[u8]) -> ((f32, f32), String, Vec<(i64, i64, Vec<u8>)>) {
        let doc = lopdf::Document::load_mem(bytes).expect("the PDF parses");
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 1);
        let (&n, &id) = pages.iter().next().unwrap();
        let page = doc.get_dictionary(id).unwrap();
        let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
        let num = |o: &lopdf::Object| o.as_float().or_else(|_| o.as_i64().map(|v| v as f32)).unwrap();
        let size = (num(&mb[2]) - num(&mb[0]), num(&mb[3]) - num(&mb[1]));
        let text = doc.extract_text(&[n]).unwrap();
        let images = doc
            .get_page_images(id)
            .unwrap()
            .iter()
            .map(|img| {
                let stream = doc.get_object(img.id).unwrap().as_stream().unwrap();
                (img.width, img.height, stream.decompressed_content().unwrap())
            })
            .collect();
        (size, text, images)
    }

    /// WP6.5: the PDF is the PNG's sheet on paper — the page is the preset's paper size, the
    /// title, caption, credits and legend are text (Chinese included), and the embedded map is the
    /// PNG's map pixel for pixel.
    #[test]
    fn the_pdf_is_the_png_sheet_on_paper() {
        // Square, nothing over the map, so the whole map area can be compared.
        let mut s = sheet(1080, 1080, 1.5);
        s.title = "Heat study 城市热岛".into();
        s.legend.clear();
        s.scale = None;
        let lay = layout(&s, &FONTS, None);
        let (mw, mh) = map_pixels(lay.map, s.ppp);
        let map = painted(mw, mh);
        let (png, _) = compose(&map, &s, &FONTS, None).unwrap();
        let pdf = compose_pdf(&map, &s, &FONTS, None, (190.5, 190.5), &s.title).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let ((w, h), text, images) = read_pdf(&pdf);
        assert!((w - 540.0).abs() < 0.05 && (h - 540.0).abs() < 0.05, "7.5 in square: {w} × {h}");
        assert!(text.contains("Heat study") && text.contains("城市热岛"), "{text}");
        assert!(text.contains("Natural Earth") && text.contains("Made with NimbleMap"), "{text}");
        assert_eq!(images.len(), 1);
        let (iw, ih, rgb) = &images[0];
        assert_eq!((*iw, *ih), (i64::from(mw), i64::from(mh)));
        let r = lay.map.px(s.ppp).unwrap();
        let (x0, y0) = (r.left() as u32, r.top() as u32);
        // The PNG's bilinear draw softens the map's outermost pixels against the white sheet; the
        // rest is the map itself.
        let mut worst = 0i32;
        for y in 1..mh - 1 {
            for x in 1..mw - 1 {
                let p = pixel(&png, x0 + x, y0 + y).unwrap();
                let i = ((y * mw + x) * 3) as usize;
                for k in 0..3 {
                    worst = worst.max((i32::from(p[k]) - i32::from(rgb[i + k])).abs());
                }
            }
        }
        assert!(worst <= 1, "the embedded map differs from the PNG's by up to {worst}");
    }

    /// A4 comes out as A4 (210 × 297 mm), with every piece of furniture, and its fonts embedded
    /// as small subsets: Inter for Latin, Noto Sans SC (CFF) for Chinese.
    #[test]
    fn an_a4_pdf_has_the_paper_size_furniture_and_font_subsets() {
        let mut s = sheet(2480, 3508, 300.0 / 96.0);
        s.title = "Districts 区县".into();
        s.north_arrow = true;
        s.legend[0]
            .entries
            .push(Entry::Ramp { stops: vec![[0, 0, 255, 255], [255, 0, 0, 255]], low: "1 °C".into(), high: "9 °C".into() });
        let lay = layout(&s, &FONTS, None);
        let (mw, mh) = map_pixels(lay.map, s.ppp);
        let pdf = compose_pdf(&blank(mw, mh), &s, &FONTS, None, (210.0, 297.0), "Districts").unwrap();
        // `LM_EXPORT_OUT=dir` keeps it for a look.
        if let Some(dir) = std::env::var_os("LM_EXPORT_OUT").map(std::path::PathBuf::from) {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("a4-furniture.pdf"), &pdf).unwrap();
        }
        let ((w, h), text, images) = read_pdf(&pdf);
        assert!((w - 595.28).abs() < 0.05 && (h - 841.89).abs() < 0.05, "A4: {w} × {h}");
        for want in ["Districts", "区县", "Population", "Over 1,000,000", "No data", "10 km", "N", "9 °C"] {
            assert!(text.contains(want), "{want} in {text}");
        }
        assert_eq!(images.len(), 1);
        let raw = String::from_utf8_lossy(&pdf);
        assert!(raw.contains("/Identity-H") && raw.contains("/ToUnicode") && raw.contains("/CIDFontType0C"));
        assert!(raw.contains("+Inter-Regular") && raw.contains("+Inter-SemiBold") && raw.contains("+NotoSansSC"));
        // Subsets, not the 8 MB Chinese font: the whole page stays small.
        assert!(pdf.len() < 600_000, "{} bytes", pdf.len());
    }

    /// The frame's labels and the slide's size.
    #[test]
    fn a_slide_pdf_carries_the_frame_labels() {
        let mut s = sheet(1920, 1080, 1.5);
        let cam = Camera { center: rustgis_geom::lonlat_to_merc(rustgis_geom::Coord::new(105.0, 35.0)), res: 6_000.0 };
        s.graticule = Some(Graticule { camera: cam, grid: true });
        s.scale = None;
        let lay = layout(&s, &FONTS, None);
        let (mw, mh) = map_pixels(lay.map, s.ppp);
        let pdf = compose_pdf(&blank(mw, mh), &s, &FONTS, None, (338.667, 190.5), "").unwrap();
        let ((w, h), text, _) = read_pdf(&pdf);
        assert!((w - 960.0).abs() < 0.05 && (h - 540.0).abs() < 0.05, "13.33 × 7.5 in: {w} × {h}");
        let t = frame_ticks(cam, (lay.map.w, lay.map.h));
        let first = degree_label(t.lons[0].0, t.lon_step, false);
        assert!(text.contains(&first), "{first} in {text}");
    }
}
