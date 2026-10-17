//! "Summarize [raster] in each [area]" and "Add [raster] values to each [point]" (spec › Phase 4 ›
//! WP4.5 Certified actions).
//!
//! **Weights.** Each raster cell `i` gets the weight `a_i = f_i · A_i`, where `f_i` is the
//! fraction of the cell's area inside the polygon, measured in the raster's own coordinates with
//! the polygon's edges as straight lines there (exactly what exactextract computes), and `A_i` is
//! the cell's area on the WGS 84 ellipsoid. NoData cells carry no weight. Then
//!
//! * mean `x̄ = Σ x_i a_i / Σ a_i` over valid cells; minimum and maximum over valid cells with
//!   `f_i > 0` (fractions at or below [`MIN_FRACTION`] count as zero);
//! * median: the area-weighted median, the smallest valid value whose cumulative weight `Σ a_i`
//!   (values in ascending order) reaches half the total; standard deviation
//!   `√(Σ a_i (x_i − x̄)² / Σ a_i)`, both over the same weights as the mean;
//! * sum `Σ x_i f_i` (exactextract's `sum`: each cell's value times the fraction inside), offered
//!   for any raster but meaningful only for counts;
//! * count raster total `N = Σ n_i f_i` (people spread evenly within each cell); density raster
//!   total `N = Σ d_i a_i / unit` (unit = the density's area unit in m², 10⁶ for "per km²");
//! * category raster shares: `Σ a_i` per class over `Σ a_i` of all valid cells (never a mean of
//!   class codes);
//! * coverage `c = Σ_valid a_i / max(A_P, Σ_all a_i)`, where `A_P` is the polygon's area on the
//!   ellipsoid (edges straight in the raster's coordinates, as the grid sees them) and `Σ_all`
//!   includes NoData cells. Weighting a planar fraction by a cell's ellipsoidal area is not the
//!   exact ellipsoidal area of the covered part (a sliver in the poleward half of a 1° cell is
//!   over-weighted by up to ~1 %), so `Σ_all a_i` can exceed `A_P` slightly; taking the larger as
//!   the denominator keeps `c ≤ 1` always, and equals `Σ_valid a_i / A_P` (the reference's
//!   formula) whenever the grid does not over-measure the polygon.
//!
//! **Coverage algorithm.** Exact, by signed-area accumulation (the scanline method used by
//! font rasterisers such as font-rs, and equivalent to exactextract's cell traversal): each edge,
//! split at every row boundary, adds to each cell it crosses the exact trapezoid area right of it
//! (times its direction), and a running sum along the row turns those into the winding-number
//! integral over each cell, which is the covered fraction for a valid polygon (holes and parts
//! are oriented so that the winding number is 1 inside, 0 outside). The cost is `O(edges·rows
//! crossed + cells in the bounding box)`, with no supersampling. Edges left or right of the
//! raster are projected onto its side (which preserves the winding to their right), so a
//! polygon larger than the raster costs only the raster's part.
//!
//! **Streaming.** A polygon is processed in strips of at most [`STRIP_CELLS`] cells of its
//! bounding box clipped to the raster; each strip reads only its window through
//! [`RasterSource::read_window`], so memory stays bounded (about 20 MB per strip) whatever the
//! raster's size, and a 2 GB raster never has to fit in memory. The control is checked after
//! every strip read and every 4,096 edges, and progress is reported per area. [`TiffSource`]
//! reads a GeoTIFF on disk this way, decoding only the strips or tiles each window touches.
//!
//! **Cell area.** Area on the ellipsoid of a region of longitude/latitude is
//! `(b²/2) ∬ dλ dq(φ)` with `q` the authalic function
//! `q(φ) = sinφ/(1−e² sin²φ) + atanh(e sinφ)/e`. For a geographic raster a cell is a rectangle in
//! `(λ, q)`, so `A_i = (b²/2) · Δλ · |q(φ₁) − q(φ₂)|`, exact (the same closed form the N3 hand
//! calculation uses; relative rounding error ≈ 1e-16 / Δφ, under 1e-10 for 0.0001° cells). For a
//! projected raster each cell's four corners are converted to longitude/latitude and the cell is
//! taken as the quadrilateral that is straight in `(λ, q)`, `A_i = (b²/2)·|shoelace|`; for cells
//! of a few kilometres or less the difference from the true area of the projected square is far
//! below 1 ppm. Cells whose corners can't be converted get no weight.
//!
//! **Polygon area `A_P`.** `(b²/2)|∮ q dλ|` per ring (holes subtract), with each edge straight
//! in the raster's coordinates: for a geographic raster by 5-point Gauss–Legendre on pieces of
//! at most 0.25° of latitude (error ≪ 1e-12); for a projected raster by the trapezoid rule in
//! `(λ, q)` on the edge densified to about one cell per piece (consistent with the cell areas).
//!
//! **Points.** The value of the cell containing the point (`floor` of its pixel position, as
//! rasterio's `sample`), with scale and offset applied; NoData, NaN, points outside the raster
//! and points that can't be placed on it give `None`, never zero. No interpolation.
//!
//! Antimeridian-crossing rasters are not handled specially: polygons are used in the raster's
//! longitude range as given.

use std::collections::BTreeMap;

pub mod tiff_source;
pub use tiff_source::TiffSource;

use crate::control::{Cancelled, Control};
use rustgis_data::Geometry;
use rustgis_data::raster::{Crs, GeoTransform, Raster, Transformer};
use rustgis_geom::Coord;

/// WGS 84 semi-major axis (m) and flattening.
const WGS84_A: f64 = 6_378_137.0;
const WGS84_F: f64 = 1.0 / 298.257_223_563;

/// Most cells (bounding box × rows) processed at once for one polygon.
pub const STRIP_CELLS: usize = 1 << 20;

/// Coverage fractions at or below this count as zero (rounding noise along shared edges).
pub const MIN_FRACTION: f64 = 1e-12;

const PHASE_SUMMARY: &str = "Summarizing values in areas";
const PHASE_SAMPLE: &str = "Adding values to points";

/// Why a summary or sample did not finish.
#[derive(Clone, Debug, PartialEq)]
pub enum ZonalError {
    Cancelled,
    /// The raster couldn't be read.
    Read(String),
    /// The raster or an area can't be used.
    Input(String),
}

impl From<Cancelled> for ZonalError {
    fn from(_: Cancelled) -> Self {
        ZonalError::Cancelled
    }
}

impl std::fmt::Display for ZonalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZonalError::Cancelled => write!(f, "cancelled"),
            ZonalError::Read(e) => write!(f, "the raster couldn't be read: {e}"),
            ZonalError::Input(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ZonalError {}

/// What a raster band is: one band's grid, georeferencing and value encoding.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub width: u32,
    pub height: u32,
    pub transform: GeoTransform,
    pub crs: Crs,
    /// Raw values equal to this (compared in `f32`, as stored) are NoData; NaN always is.
    pub nodata: Option<f64>,
    /// Value = raw × scale + offset.
    pub scale: f64,
    pub offset: f64,
}

impl Grid {
    /// Whether a raw value is a measurement.
    pub fn is_valid(&self, raw: f32) -> bool {
        raw.is_finite() && self.nodata.is_none_or(|d| raw != d as f32)
    }

    /// A raw value with scale and offset applied.
    pub fn value(&self, raw: f32) -> f64 {
        f64::from(raw) * self.scale + self.offset
    }

    fn validate(&self) -> Result<(), ZonalError> {
        if self.width == 0 || self.height == 0 {
            return Err(ZonalError::Input("the raster has no cells".into()));
        }
        if !self.transform.is_valid() {
            return Err(ZonalError::Input("the raster has no usable georeferencing".into()));
        }
        if !self.scale.is_finite() || !self.offset.is_finite() {
            return Err(ZonalError::Input("the raster's scale or offset isn't a number".into()));
        }
        Ok(())
    }
}

/// One raster band that can be read a window at a time, so a summary never needs the whole
/// raster in memory.
pub trait RasterSource {
    fn grid(&self) -> &Grid;
    /// Fills `out` (`cols × rows`, row by row) with the raw values of the window whose top-left
    /// cell is (`col0`, `row0`). The window always lies inside the grid.
    fn read_window(&mut self, col0: u32, row0: u32, cols: u32, rows: u32, out: &mut [f32]) -> Result<(), ZonalError>;
}

/// One band of a raster held in memory.
pub struct MemoryBand<'a> {
    raster: &'a Raster,
    band: usize,
    grid: Grid,
}

impl<'a> MemoryBand<'a> {
    /// Band `band` (0-based) with no scale or offset.
    pub fn new(raster: &'a Raster, band: usize) -> Result<MemoryBand<'a>, ZonalError> {
        if band >= raster.bands.len() {
            return Err(ZonalError::Input(format!("the raster has no band {}", band + 1)));
        }
        let grid = Grid {
            width: raster.width,
            height: raster.height,
            transform: raster.transform,
            crs: raster.crs,
            nodata: raster.nodata,
            scale: 1.0,
            offset: 0.0,
        };
        Ok(MemoryBand { raster, band, grid })
    }

    /// The band's values are raw × `scale` + `offset`.
    pub fn with_scale_offset(mut self, scale: f64, offset: f64) -> MemoryBand<'a> {
        self.grid.scale = scale;
        self.grid.offset = offset;
        self
    }
}

impl RasterSource for MemoryBand<'_> {
    fn grid(&self) -> &Grid {
        &self.grid
    }

    fn read_window(&mut self, col0: u32, row0: u32, cols: u32, rows: u32, out: &mut [f32]) -> Result<(), ZonalError> {
        let data = &self.raster.bands.get(self.band).ok_or_else(|| ZonalError::Read("missing band".into()))?.data;
        let w = self.grid.width as usize;
        let (c0, r0, nc, nr) = (col0 as usize, row0 as usize, cols as usize, rows as usize);
        if c0 + nc > w || r0 + nr > self.grid.height as usize || out.len() < nc * nr {
            return Err(ZonalError::Read("window outside the raster".into()));
        }
        for r in 0..nr {
            let src = data.get((r0 + r) * w + c0..(r0 + r) * w + c0 + nc).ok_or_else(|| ZonalError::Read("short band".into()))?;
            out[r * nc..(r + 1) * nc].copy_from_slice(src);
        }
        Ok(())
    }
}

/// What a band's values mean, which decides the totals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueRole {
    /// A measured quantity such as temperature: mean, minimum, maximum; no total.
    Measured,
    /// A confirmed count per cell (people per cell): also `N = Σ n_i f_i`.
    Count,
    /// A confirmed density per `unit_area_m2` (10⁶ for per km²): also `N = Σ d_i a_i / unit`.
    Density { unit_area_m2: f64 },
    /// Class codes: shares of the covered area per class; no mean, minimum or maximum.
    Category,
}

/// Which formula gave a total (the result card states it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalFormula {
    /// `N = Σ n_i f_i`: counts split across a boundary assuming an even spread in each cell.
    CountTimesFraction,
    /// `N = Σ d_i a_i`: density times covered ellipsoidal area.
    DensityTimesArea,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Total {
    pub value: f64,
    pub formula: TotalFormula,
}

/// One class's share of an area's valid covered area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CategoryShare {
    /// The class value (scale and offset applied).
    pub value: f64,
    pub share: f64,
    pub area_m2: f64,
}

/// The summary of one area.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZonalSummary {
    /// Area-weighted mean; `None` with no valid covered cell, or for a category raster.
    pub mean: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Share of the area with valid data, 0–1 (see the module notes).
    pub coverage: f64,
    /// Valid cells with a fraction inside.
    pub valid_cells: u64,
    /// `Σ f_i` over valid cells (exactextract's `count`).
    pub valid_fraction_sum: f64,
    /// NoData cells with a fraction inside.
    pub nodata_cells: u64,
    /// Share of the covered area inside the raster that is NoData, 0–1.
    pub nodata_fraction: f64,
    /// `Σ a_i` over valid cells (m²).
    pub covered_valid_area_m2: f64,
    /// `Σ a_i` over all cells inside the raster, NoData included (m²).
    pub covered_area_m2: f64,
    /// `A_P`, the polygon's ellipsoidal area (m²).
    pub polygon_area_m2: f64,
    pub total: Option<Total>,
    /// Area-weighted median, standard deviation, and `Σ x_i f_i` (see the module notes); `None`
    /// with no valid covered cell, or for a category raster.
    pub median: Option<f64>,
    pub std_dev: Option<f64>,
    pub sum: Option<f64>,
    /// For a category raster: classes present, by value.
    pub categories: Vec<CategoryShare>,
}

/// Summarizes a raster band in each area (polygons in WGS 84 longitude/latitude). Areas that
/// aren't polygons get an empty summary. Fails only on cancel, read errors or an unusable grid.
pub fn summarize(src: &mut dyn RasterSource, areas: &[Geometry], role: ValueRole, ctl: &Control) -> Result<Vec<ZonalSummary>, ZonalError> {
    let grid = src.grid().clone();
    grid.validate()?;
    if let ValueRole::Density { unit_area_m2 } = role
        && !(unit_area_m2.is_finite() && unit_area_m2 > 0.0)
    {
        return Err(ZonalError::Input("the density's area unit must be positive".into()));
    }
    let ctx = Context::new(&grid)?;
    let mut scratch = Scratch::default();
    let total = areas.len() as u64;
    let mut out = Vec::with_capacity(areas.len());
    ctl.report(PHASE_SUMMARY, 0, Some(total));
    for (k, g) in areas.iter().enumerate() {
        ctl.check()?;
        let s = match g {
            Geometry::Polygon(parts) => summarize_polygon(src, &ctx, parts, role, ctl, &mut scratch)?,
            _ => ZonalSummary::default(),
        };
        out.push(s);
        ctl.report(PHASE_SUMMARY, k as u64 + 1, Some(total));
    }
    Ok(out)
}

/// The value of the cell containing each point (WGS 84 longitude/latitude), scale and offset
/// applied; `None` for NoData and for points off the raster.
pub fn sample_points(src: &mut dyn RasterSource, points: &[Coord], ctl: &Control) -> Result<Vec<Option<f64>>, ZonalError> {
    let grid = src.grid().clone();
    grid.validate()?;
    let ctx = Context::new(&grid)?;
    let total = points.len() as u64;
    let mut out = Vec::with_capacity(points.len());
    let mut buf = [0f32; 1];
    ctl.report(PHASE_SAMPLE, 0, Some(total));
    for (k, p) in points.iter().enumerate() {
        if k % 1024 == 0 {
            ctl.check()?;
            if k > 0 {
                ctl.report(PHASE_SAMPLE, k as u64, Some(total));
            }
        }
        let Some((c, r)) = ctx.to_pixel(*p).map(|(c, r)| (c.floor(), r.floor())) else {
            out.push(None);
            continue;
        };
        if !(c >= 0.0 && r >= 0.0 && c < f64::from(grid.width) && r < f64::from(grid.height)) {
            out.push(None);
            continue;
        }
        src.read_window(c as u32, r as u32, 1, 1, &mut buf)?;
        out.push(grid.is_valid(buf[0]).then(|| grid.value(buf[0])));
    }
    ctl.report(PHASE_SAMPLE, total, Some(total));
    Ok(out)
}

// ---- Ellipsoid ------------------------------------------------------------------------------

/// The authalic function and constants of WGS 84.
#[derive(Clone, Copy, Debug)]
struct Authalic {
    e: f64,
    e2: f64,
    half_b2: f64,
}

impl Authalic {
    fn wgs84() -> Authalic {
        let e2 = WGS84_F * (2.0 - WGS84_F);
        let b = WGS84_A * (1.0 - WGS84_F);
        Authalic { e: e2.sqrt(), e2, half_b2: 0.5 * b * b }
    }

    /// `q(φ)` for latitude in degrees (clamped to ±90°).
    fn q(&self, lat_deg: f64) -> f64 {
        let s = lat_deg.clamp(-90.0, 90.0).to_radians().sin();
        s / (1.0 - self.e2 * s * s) + (self.e * s).atanh() / self.e
    }
}

/// Ellipsoidal area (m²) of the band between two parallels, `dlon_deg` wide (closed form).
pub fn lat_band_area_m2(lat1_deg: f64, lat2_deg: f64, dlon_deg: f64) -> f64 {
    let au = Authalic::wgs84();
    au.half_b2 * dlon_deg.abs().to_radians() * (au.q(lat1_deg) - au.q(lat2_deg)).abs()
}

// ---- Context: placing polygons on the grid --------------------------------------------------

struct Context {
    grid: Grid,
    au: Authalic,
    geographic: bool,
    /// None for WGS 84 longitude/latitude rasters (no conversion).
    transformer: Option<Transformer>,
}

impl Context {
    fn new(grid: &Grid) -> Result<Context, ZonalError> {
        let transformer = match grid.crs {
            Crs::LonLat => None,
            c => Some(c.transformer().map_err(ZonalError::Input)?),
        };
        Ok(Context { grid: grid.clone(), au: Authalic::wgs84(), geographic: grid.crs.is_geographic(), transformer })
    }

    /// Longitude/latitude to the raster's coordinates.
    fn to_native(&self, c: Coord) -> Option<Coord> {
        if !c.is_finite() {
            return None;
        }
        match &self.transformer {
            None => Some(c),
            Some(t) => t.from_lonlat(c),
        }
    }

    fn to_lonlat(&self, c: Coord) -> Option<Coord> {
        match &self.transformer {
            None => Some(c),
            Some(t) => t.to_lonlat(c),
        }
    }

    fn to_pixel(&self, lonlat: Coord) -> Option<(f64, f64)> {
        self.to_native(lonlat).map(|n| self.grid.transform.to_pixel(n))
    }

    /// `(λ radians, q)` of a point in the raster's coordinates.
    fn lambda_q(&self, native: Coord) -> Option<(f64, f64)> {
        let ll = if self.geographic { native } else { self.to_lonlat(native)? };
        Some((ll.x.to_radians(), self.au.q(ll.y)))
    }

    /// Unsigned ellipsoidal area of a ring (raster coordinates), edges straight in them.
    fn ring_area(&self, ring: &[Coord]) -> Result<f64, ZonalError> {
        let n = ring.len();
        let mut sum = 0.0;
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            sum += if self.geographic { self.edge_integral_geographic(a, b) } else { self.edge_integral_projected(a, b)? };
        }
        Ok(self.au.half_b2 * sum.abs())
    }

    /// `∫ q dλ` along a lon/lat-straight edge (degrees), by Gauss–Legendre.
    fn edge_integral_geographic(&self, a: Coord, b: Coord) -> f64 {
        const X: [f64; 5] = [0.0, -0.538_469_310_105_683_1, 0.538_469_310_105_683_1, -0.906_179_845_938_664, 0.906_179_845_938_664];
        const W: [f64; 5] =
            [0.568_888_888_888_888_9, 0.478_628_670_499_366_5, 0.478_628_670_499_366_5, 0.236_926_885_056_189_1, 0.236_926_885_056_189_1];
        let dlam = (b.x - a.x).to_radians();
        if dlam == 0.0 {
            return 0.0;
        }
        let dlat = b.y - a.y;
        let pieces = (dlat.abs() / 0.25).ceil().clamp(1.0, 100_000.0) as usize;
        let mut mean_q = 0.0;
        for p in 0..pieces {
            let (t0, t1) = (p as f64 / pieces as f64, (p + 1) as f64 / pieces as f64);
            let mut s = 0.0;
            for (x, w) in X.iter().zip(W.iter()) {
                let t = t0 + (t1 - t0) * 0.5 * (1.0 + x);
                s += w * self.au.q(a.y + t * dlat);
            }
            mean_q += 0.5 * s * (t1 - t0);
        }
        mean_q * dlam
    }

    /// `∫ q dλ` along an edge straight in projected coordinates, densified to about one cell.
    fn edge_integral_projected(&self, a: Coord, b: Coord) -> Result<f64, ZonalError> {
        let t = &self.grid.transform;
        let cells = ((b.x - a.x) / t.pixel_width).hypot((b.y - a.y) / t.pixel_height);
        let pieces = cells.ceil().clamp(1.0, 4096.0) as usize;
        let place = |s: f64| {
            self.lambda_q(Coord::new(a.x + s * (b.x - a.x), a.y + s * (b.y - a.y)))
                .ok_or_else(|| ZonalError::Input("an area reaches beyond where the raster's projection is defined".into()))
        };
        let mut prev = place(0.0)?;
        let mut sum = 0.0;
        for p in 1..=pieces {
            let cur = place(p as f64 / pieces as f64)?;
            sum += (cur.0 - prev.0) * 0.5 * (cur.1 + prev.1);
            prev = cur;
        }
        Ok(sum)
    }
}

// ---- Coverage -------------------------------------------------------------------------------

/// An edge in window coordinates: x local to the window (clamped to [0, w]), y in global rows,
/// `y0 < y1`, and `m` the signed weight (direction × ring orientation).
#[derive(Clone, Copy, Debug)]
struct Seg {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    m: f64,
}

/// Adds one segment's exact trapezoid areas to the strip accumulator (rows `row0..row0+nrows`,
/// `stride = w + 2` slots per row).
fn accumulate(acc: &mut [f64], stride: usize, w: f64, row0: i64, nrows: i64, s: &Seg) {
    let dxdy = (s.x1 - s.x0) / (s.y1 - s.y0);
    let ystart = (s.y0.floor() as i64).max(row0);
    let yend = (s.y1.ceil() as i64).min(row0 + nrows);
    for y in ystart..yend {
        let ya = (y as f64).max(s.y0);
        let yb = ((y + 1) as f64).min(s.y1);
        let dy = yb - ya;
        if dy <= 0.0 {
            continue;
        }
        let xa = (s.x0 + (ya - s.y0) * dxdy).clamp(0.0, w);
        let xb = (s.x0 + (yb - s.y0) * dxdy).clamp(0.0, w);
        let d = dy * s.m;
        let line = &mut acc[(y - row0) as usize * stride..(y - row0 + 1) as usize * stride];
        let (x0, x1) = if xa < xb { (xa, xb) } else { (xb, xa) };
        let x0f = x0.floor();
        let x0i = x0f as usize;
        let x1c = x1.ceil();
        let x1i = x1c as usize;
        if x1i <= x0i + 1 {
            let xmf = 0.5 * (xa + xb) - x0f;
            line[x0i] += d - d * xmf;
            line[x0i + 1] += d * xmf;
        } else {
            let sinv = 1.0 / (x1 - x0);
            let x0r = x0 - x0f;
            let a0 = 0.5 * sinv * (1.0 - x0r) * (1.0 - x0r);
            let x1r = x1 - x1c + 1.0;
            let am = 0.5 * sinv * x1r * x1r;
            line[x0i] += d * a0;
            if x1i == x0i + 2 {
                line[x0i + 1] += d * (1.0 - a0 - am);
            } else {
                let a1 = sinv * (1.5 - x0r);
                line[x0i + 1] += d * (a1 - a0);
                for v in &mut line[x0i + 2..x1i - 1] {
                    *v += d * sinv;
                }
                let a2 = a1 + (x1i - x0i - 3) as f64 * sinv;
                line[x1i - 1] += d * (1.0 - a2 - am);
            }
            line[x1i] += d * am;
        }
    }
}

/// Splits an edge (window-local x) where it crosses x = 0 and x = w, clamps the pieces to
/// [0, w] (projection onto the side keeps the winding to its right) and keeps the non-horizontal
/// ones.
fn push_clipped(segs: &mut Vec<Seg>, a: (f64, f64), b: (f64, f64), w: f64, ring_m: f64) {
    let mut ts = [0.0, 1.0, 1.0, 1.0];
    let mut n = 1;
    let dx = b.0 - a.0;
    if dx != 0.0 {
        for xc in [0.0, w] {
            let t = (xc - a.0) / dx;
            if t > 0.0 && t < 1.0 {
                ts[n] = t;
                n += 1;
            }
        }
    }
    ts[n] = 1.0;
    let cuts = &mut ts[..=n];
    cuts.sort_by(f64::total_cmp);
    let at = |t: f64| {
        if t <= 0.0 {
            a
        } else if t >= 1.0 {
            b
        } else {
            (a.0 + t * dx, a.1 + t * (b.1 - a.1))
        }
    };
    for k in 0..n {
        let (p, q) = (at(cuts[k]), at(cuts[k + 1]));
        if p.1 == q.1 {
            continue;
        }
        let (p, q, dir) = if p.1 < q.1 { (p, q, 1.0) } else { (q, p, -1.0) };
        segs.push(Seg { x0: p.0.clamp(0.0, w), y0: p.1, x1: q.0.clamp(0.0, w), y1: q.1, m: dir * ring_m });
    }
}

#[derive(Default)]
struct Scratch {
    acc: Vec<f64>,
    data: Vec<f32>,
    areas: Vec<f64>,
    corners: Vec<(f64, f64)>,
}

#[derive(Default)]
struct Sums {
    wx: f64,
    w_valid: f64,
    w_all: f64,
    f_valid: f64,
    n_valid: u64,
    n_nodata: u64,
    min: f64,
    max: f64,
    count_total: f64,
    density_total: f64,
    cats: BTreeMap<u32, (f64, f64)>,
    /// Σ a·x² and Σ x·f.
    wx2: f64,
    xf: f64,
    /// Each valid cell's value and weight, for the median.
    values: Vec<(f64, f64)>,
}

fn summarize_polygon(
    src: &mut dyn RasterSource,
    ctx: &Context,
    parts: &[Vec<Vec<Coord>>],
    role: ValueRole,
    ctl: &Control,
    scratch: &mut Scratch,
) -> Result<ZonalSummary, ZonalError> {
    let grid = &ctx.grid;
    // Rings in raster coordinates (for the area) and pixel coordinates (for the coverage).
    let mut polygon_area = 0.0;
    let mut px_rings: Vec<(Vec<(f64, f64)>, f64)> = Vec::new();
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (pi, rings) in parts.iter().enumerate() {
        if pi % 64 == 0 {
            ctl.check()?;
        }
        let mut part_area = 0.0;
        for (k, ring) in rings.iter().enumerate() {
            let mut native = Vec::with_capacity(ring.len());
            for c in ring.iter().filter(|c| c.is_finite()) {
                let n = ctx
                    .to_native(*c)
                    .ok_or_else(|| ZonalError::Input("an area can't be placed on the raster's coordinate system".into()))?;
                native.push(n);
            }
            if native.len() > 1 && native.first() == native.last() {
                native.pop();
            }
            if native.len() < 3 {
                continue;
            }
            let a = ctx.ring_area(&native)?;
            part_area += if k == 0 { a } else { -a };
            let px: Vec<(f64, f64)> = native
                .iter()
                .map(|n| {
                    let (x, y) = grid.transform.to_pixel(*n);
                    (x, y)
                })
                .collect();
            let mut s2 = 0.0;
            for i in 0..px.len() {
                let (a, b) = (px[i], px[(i + 1) % px.len()]);
                s2 += a.0 * b.1 - b.0 * a.1;
            }
            if s2 == 0.0 || !s2.is_finite() {
                continue;
            }
            // In this accumulation a ring with positive shoelace area has winding −1 inside.
            let m = if k == 0 { -s2.signum() } else { s2.signum() };
            for &(x, y) in &px {
                minx = minx.min(x);
                maxx = maxx.max(x);
                miny = miny.min(y);
                maxy = maxy.max(y);
            }
            px_rings.push((px, m));
        }
        polygon_area += part_area.max(0.0);
    }
    let mut out = ZonalSummary { polygon_area_m2: polygon_area, ..ZonalSummary::default() };
    if px_rings.is_empty() {
        return Ok(out);
    }
    // The window: the bounding box clipped to the raster.
    let c0 = minx.floor().max(0.0);
    let c1 = maxx.ceil().min(f64::from(grid.width));
    let r0 = miny.floor().max(0.0);
    let r1 = maxy.ceil().min(f64::from(grid.height));
    if c0 >= c1 || r0 >= r1 {
        finish(&mut out, Sums::default(), role);
        return Ok(out);
    }
    let (c0, c1, r0, r1) = (c0 as u32, c1 as u32, r0 as i64, r1 as i64);
    let w = (c1 - c0) as usize;
    let wf = w as f64;
    let mut segs = Vec::new();
    for (k, (px, m)) in px_rings.iter().enumerate() {
        if k % 64 == 0 {
            ctl.check()?;
        }
        for i in 0..px.len() {
            let (a, b) = (px[i], px[(i + 1) % px.len()]);
            push_clipped(&mut segs, (a.0 - f64::from(c0), a.1), (b.0 - f64::from(c0), b.1), wf, *m);
        }
    }
    // Strips ordered by their first row so each strip scans only the edges that reach it.
    segs.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    let stride = w + 2;
    let strip_rows = (STRIP_CELLS / stride).clamp(1, (r1 - r0) as usize) as i64;
    let mut sums = Sums { min: f64::INFINITY, max: f64::NEG_INFINITY, ..Sums::default() };
    let mut sr = r0;
    while sr < r1 {
        let nrows = strip_rows.min(r1 - sr);
        let nr = nrows as usize;
        ctl.check()?;
        scratch.acc.clear();
        scratch.acc.resize(nr * stride, 0.0);
        let strip_end = (sr + nrows) as f64;
        for (k, s) in segs.iter().enumerate() {
            if k % 4096 == 0 {
                ctl.check()?;
            }
            if s.y0 >= strip_end {
                break;
            }
            if s.y1 <= sr as f64 {
                continue;
            }
            accumulate(&mut scratch.acc, stride, wf, sr, nrows, s);
        }
        scratch.data.clear();
        scratch.data.resize(nr * w, 0.0);
        src.read_window(c0, sr as u32, w as u32, nrows as u32, &mut scratch.data)?;
        ctl.check()?;
        cell_areas(ctx, c0, sr, w, nr, scratch);
        for r in 0..nr {
            let acc = &scratch.acc[r * stride..r * stride + w];
            let data = &scratch.data[r * w..(r + 1) * w];
            let areas = if ctx.geographic { &scratch.areas[r..r + 1] } else { &scratch.areas[r * w..(r + 1) * w] };
            add_row(&mut sums, grid, role, acc, data, areas);
        }
        sr += nrows;
    }
    finish(&mut out, sums, role);
    Ok(out)
}

/// Cell areas of a strip: one per row for a geographic raster, one per cell otherwise.
fn cell_areas(ctx: &Context, c0: u32, r0: i64, w: usize, nr: usize, scratch: &mut Scratch) {
    let t = &ctx.grid.transform;
    scratch.areas.clear();
    if ctx.geographic {
        let dlam = t.pixel_width.abs().to_radians();
        let mut q_prev = ctx.au.q(t.origin_y + r0 as f64 * t.pixel_height);
        for r in 0..nr {
            let q = ctx.au.q(t.origin_y + (r0 + r as i64 + 1) as f64 * t.pixel_height);
            scratch.areas.push(ctx.au.half_b2 * dlam * (q - q_prev).abs());
            q_prev = q;
        }
        return;
    }
    scratch.corners.clear();
    for r in 0..=nr {
        for c in 0..=w {
            let n = t.to_world(f64::from(c0) + c as f64, (r0 + r as i64) as f64);
            scratch.corners.push(ctx.lambda_q(n).unwrap_or((f64::NAN, f64::NAN)));
        }
    }
    let cw = w + 1;
    for r in 0..nr {
        for c in 0..w {
            let p = [
                scratch.corners[r * cw + c],
                scratch.corners[r * cw + c + 1],
                scratch.corners[(r + 1) * cw + c + 1],
                scratch.corners[(r + 1) * cw + c],
            ];
            let mut s2 = 0.0;
            for i in 0..4 {
                let (a, b) = (p[i], p[(i + 1) % 4]);
                s2 += a.0 * b.1 - b.0 * a.1;
            }
            let area = ctx.au.half_b2 * 0.5 * s2.abs();
            scratch.areas.push(if area.is_finite() { area } else { 0.0 });
        }
    }
}

/// Adds one row of cells: `acc` holds the accumulated coverage deltas, `areas` one area for the
/// whole row or one per cell.
fn add_row(sums: &mut Sums, grid: &Grid, role: ValueRole, acc: &[f64], data: &[f32], areas: &[f64]) {
    let row_area = if areas.len() == 1 { areas.first().copied() } else { None };
    let mut cov = 0.0;
    // Row partial sums keep long sums accurate.
    let (mut wx, mut w_valid, mut w_all, mut f_valid, mut count_total, mut density_total) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut wx2, mut xf) = (0.0, 0.0);
    for (j, (&d, &raw)) in acc.iter().zip(data.iter()).enumerate() {
        cov += d;
        if cov <= MIN_FRACTION {
            continue;
        }
        let f = cov.min(1.0);
        let area = row_area.unwrap_or_else(|| areas.get(j).copied().unwrap_or(0.0));
        let a = f * area;
        w_all += a;
        if !grid.is_valid(raw) {
            sums.n_nodata += 1;
            continue;
        }
        let x = grid.value(raw);
        sums.n_valid += 1;
        f_valid += f;
        w_valid += a;
        wx += x * a;
        wx2 += x * x * a;
        xf += x * f;
        if role != ValueRole::Category {
            sums.values.push((x, a));
        }
        sums.min = sums.min.min(x);
        sums.max = sums.max.max(x);
        match role {
            ValueRole::Count => count_total += x * f,
            ValueRole::Density { .. } => density_total += x * a,
            ValueRole::Category => {
                let e = sums.cats.entry(raw.to_bits()).or_insert((x, 0.0));
                e.1 += a;
            }
            ValueRole::Measured => {}
        }
    }
    sums.wx += wx;
    sums.w_valid += w_valid;
    sums.w_all += w_all;
    sums.f_valid += f_valid;
    sums.count_total += count_total;
    sums.density_total += density_total;
    sums.wx2 += wx2;
    sums.xf += xf;
}

/// The smallest value whose cumulative weight reaches half the total.
fn weighted_median(values: &mut [(f64, f64)]) -> Option<f64> {
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = values.iter().map(|v| v.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut run = 0.0;
    for &(x, w) in values.iter() {
        run += w;
        if run >= total / 2.0 {
            return Some(x);
        }
    }
    values.last().map(|v| v.0)
}

fn finish(out: &mut ZonalSummary, mut s: Sums, role: ValueRole) {
    out.valid_cells = s.n_valid;
    out.nodata_cells = s.n_nodata;
    out.valid_fraction_sum = s.f_valid;
    out.covered_valid_area_m2 = s.w_valid;
    out.covered_area_m2 = s.w_all;
    out.nodata_fraction = if s.w_all > 0.0 { (s.w_all - s.w_valid) / s.w_all } else { 0.0 };
    let denom = out.polygon_area_m2.max(s.w_all);
    out.coverage = if denom > 0.0 { (s.w_valid / denom).min(1.0) } else { 0.0 };
    let any = s.n_valid > 0 && s.w_valid > 0.0;
    if role != ValueRole::Category && any {
        let mean = s.wx / s.w_valid;
        out.mean = Some(mean);
        out.min = Some(s.min);
        out.max = Some(s.max);
        out.std_dev = Some((s.wx2 / s.w_valid - mean * mean).max(0.0).sqrt());
        out.sum = Some(s.xf);
        out.median = weighted_median(&mut s.values);
    }
    out.total = match role {
        ValueRole::Count => Some(Total { value: s.count_total, formula: TotalFormula::CountTimesFraction }),
        ValueRole::Density { unit_area_m2 } => {
            Some(Total { value: s.density_total / unit_area_m2, formula: TotalFormula::DensityTimesArea })
        }
        _ => None,
    };
    if role == ValueRole::Category && s.w_valid > 0.0 {
        let mut cats: Vec<CategoryShare> =
            s.cats.values().map(|&(value, area)| CategoryShare { value, share: area / s.w_valid, area_m2: area }).collect();
        cats.sort_by(|a, b| a.value.total_cmp(&b.value));
        out.categories = cats;
    }
}

#[cfg(test)]
mod tests;
