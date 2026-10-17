//! "Find areas where [Heat] meets conditions" (puzzles › mechanics › A1 `find_where`).
//!
//! # Definition
//!
//! - **Grid.** Every raster is read on the grid of the first condition's raster. Another
//!   raster's value for a cell is the value of its cell containing the first grid's cell centre
//!   (nearest neighbour, `floor` of the pixel position, the rule of `zonal::sample_points` and of
//!   upstream `rasterops::resample_to`). Centres are converted through WGS 84 longitude/latitude
//!   when the two rasters' coordinate systems differ. A centre off the other raster is NoData.
//! - **Values** are raw × scale + offset ([`zonal::Grid::value`]). Tests: at least `t` (`v ≥ t`),
//!   at most `t` (`v ≤ t`), between `a` and `b` inclusive (either order), one of several codes
//!   (exact equality, for class codes).
//! - **In.** A cell is in when every condition holds; NoData (or NaN) in any input means out.
//! - **Areas.** In cells that touch by an edge or a corner form one area (8-connected, as
//!   `scipy.ndimage.label` with a 3×3 structure). Areas are numbered in raster order of their
//!   first cell (top row first, then left to right), as `ndimage.label` numbers them.
//! - **Cell area** on the WGS 84 ellipsoid, the zonal module's formulas: for a geographic grid
//!   one value per row, [`zonal::lat_band_area_m2`] (the closed authalic form, exact); for a
//!   projected grid each cell's four corners are converted to longitude/latitude and the cell is
//!   the quadrilateral straight in `(λ, q)` (`q` the authalic function), `(b²/2)·|shoelace|`.
//!   Cells whose corners can't be converted weigh nothing. An area's `area_m2` is the sum of its
//!   cells' areas.
//! - **Means.** Per condition raster, the area-weighted mean `Σ x_i A_i / Σ A_i` over the area's
//!   cells (every one of them is valid in every raster, or it would not be in).
//!
//! # Outlines
//!
//! An area's boundary follows cell edges exactly. Boundary edges are collected while the rows
//! stream past (with the area on their left), then linked into rings on the cell lattice in
//! exact integer arithmetic. Where two cells of an area touch only at a corner (a pinch vertex)
//! the ring turns so as to stay with the cell it came along, which splits the outline there:
//! the area becomes a multipolygon whose parts touch at that corner. That is the only valid
//! representation (OGC simple features): a polygon's interior must be connected, and two cells
//! meeting at a corner have no interior in common. A ring that comes back to one of its own
//! vertices (a bay closed off by a corner contact) is cut there into a simple ring and a hole
//! touching it at that point, again the valid form. So every output ring is simple, rings meet
//! only at isolated vertices, exteriors are counter-clockwise, holes clockwise, rings closed,
//! and each hole sits in the part whose cells border it. The cells covered are exactly the in
//! cells of the area, so the polygon's area (edges as the grid draws them) is `area_m2`.
//!
//! Output coordinates are WGS 84 longitude/latitude. For a geographic grid a ring has a vertex
//! only where it turns (edges straight in longitude/latitude, which is how the grid's edges
//! run). For a projected grid every cell corner along the outline is kept and converted, so no
//! edge is longer than one cell.
//!
//! Not handled: a global grid's left and right edges are not joined across the antimeridian
//! (an area touching both is two areas, as in `ndimage.label`), and longitudes are not wrapped
//! between rasters using different longitude ranges (−180…180 versus 0…360).
//!
//! # Streaming
//!
//! Upstream's `rasterops::resample_to` and `remap` (which define the same sampling rule and the
//! same kind of tests) work on whole in-memory rasters, and this crate may not depend on
//! `rustgis-analysis`, so the sampling and tests are done here, a window at a time.
//!
//! The first grid is processed in strips of at most [`STRIP_CELLS`] cells. Each strip reads its
//! window of the first raster, then, for its in cells only, the windows of the other rasters
//! that hold their centres (in blocks of at most [`WINDOW_CELLS`] cells). Labels are kept for
//! two rows only (union–find over provisional labels, as in two-pass labelling); per-area sums
//! and boundary edges are what remains, so memory is proportional to the output, not to the
//! raster. The control is checked after every strip read and every 65,536 edges traced;
//! progress reports rows done, then areas outlined.

use std::collections::HashMap;

use crate::control::Control;
use crate::zonal::{self, Grid, MemoryBand, RasterSource, ZonalError};
use rustgis_data::Geometry;
use rustgis_data::raster::{Crs, Transformer};
use rustgis_geom::Coord;

/// Most cells of the first grid processed at once (one strip).
pub const STRIP_CELLS: usize = 1 << 18;

/// Most cells of another raster read in one window.
pub const WINDOW_CELLS: usize = 1 << 20;

/// WGS 84 semi-major axis (m) and flattening (as in the zonal module).
const WGS84_A: f64 = 6_378_137.0;
const WGS84_F: f64 = 1.0 / 298.257_223_563;

const PHASE_CELLS: &str = "Finding the cells that meet the conditions";
const PHASE_TRACE: &str = "Drawing the outlines of the areas";

const NONE: u32 = u32::MAX;

/// A condition on one raster's values (after scale and offset).
#[derive(Clone, Debug, PartialEq)]
pub enum Test {
    /// `v ≥ t`.
    AtLeast(f64),
    /// `v ≤ t`.
    AtMost(f64),
    /// Between the two values, inclusive (in either order).
    Between(f64, f64),
    /// Equal to one of these codes.
    AnyOf(Vec<f64>),
}

impl Test {
    /// Whether a (valid) value passes.
    pub fn holds(&self, v: f64) -> bool {
        match self {
            Test::AtLeast(t) => v >= *t,
            Test::AtMost(t) => v <= *t,
            Test::Between(a, b) => v >= a.min(*b) && v <= a.max(*b),
            Test::AnyOf(codes) => codes.contains(&v),
        }
    }

    fn validate(&self) -> Result<(), ZonalError> {
        let ok = match self {
            Test::AtLeast(t) | Test::AtMost(t) => !t.is_nan(),
            Test::Between(a, b) => !a.is_nan() && !b.is_nan(),
            Test::AnyOf(codes) => !codes.is_empty() && codes.iter().all(|c| c.is_finite()),
        };
        if ok { Ok(()) } else { Err(ZonalError::Input("a condition's value isn't a number".into())) }
    }
}

/// One raster and the test its values must pass.
pub struct Condition<'a> {
    pub band: MemoryBand<'a>,
    pub test: Test,
}

/// One connected area of cells where every condition holds.
#[derive(Clone, Debug, PartialEq)]
pub struct WhereArea {
    /// The area's outline (WGS 84 longitude/latitude), along cell edges.
    pub geometry: Geometry,
    pub cells: u64,
    /// Sum of the cells' areas on the WGS 84 ellipsoid (m²).
    pub area_m2: f64,
    /// Area-weighted mean of each condition's raster over the area, in condition order.
    pub means: Vec<Option<f64>>,
}

/// The areas found, in raster order of their first cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WhereResult {
    pub areas: Vec<WhereArea>,
    /// Cells of the first grid where every condition holds.
    pub cells_in: u64,
    /// Cells of the first grid.
    pub grid_cells: u64,
}

/// Cells of the FIRST condition's grid where every condition holds (others sampled at the cell
/// centre, nearest neighbour; NoData anywhere = out), grouped 8-connected into areas whose
/// boundaries follow cell edges exactly; areas in WGS 84 lon/lat (geographic grids) or converted
/// from the raster CRS.
pub fn find_where(conditions: &mut [Condition<'_>], ctl: &Control) -> Result<WhereResult, ZonalError> {
    let tests: Vec<Test> = conditions.iter().map(|c| c.test.clone()).collect();
    let mut sources: Vec<&mut dyn RasterSource> = conditions.iter_mut().map(|c| &mut c.band as &mut dyn RasterSource).collect();
    find_where_sources(&mut sources, &tests, ctl)
}

/// [`find_where`] on any windowed raster sources (for example [`zonal::TiffSource`], which
/// reads a GeoTIFF from disk a window at a time). `tests[k]` applies to `sources[k]`.
pub fn find_where_sources(sources: &mut [&mut dyn RasterSource], tests: &[Test], ctl: &Control) -> Result<WhereResult, ZonalError> {
    if sources.is_empty() {
        return Err(ZonalError::Input("give at least one condition".into()));
    }
    if sources.len() != tests.len() {
        return Err(ZonalError::Input("each raster needs one condition".into()));
    }
    for t in tests {
        t.validate()?;
    }
    let grids: Vec<Grid> = sources.iter().map(|s| s.grid().clone()).collect();
    for g in &grids {
        validate_grid(g)?;
    }
    let Some(first) = grids.first().cloned() else { return Err(ZonalError::Input("give at least one condition".into())) };
    let first_tf = transformer(first.crs)?;
    let samplers: Vec<Sampler> = grids.iter().skip(1).map(|g| Sampler::new(&first, first_tf.as_ref(), g)).collect::<Result<_, _>>()?;
    let areas = CellAreas::new(&first, first_tf.as_ref());

    let w = first.width as usize;
    let h = first.height as usize;
    let k = sources.len();
    let strip_rows = (STRIP_CELLS / w).clamp(1, h);
    let mut raw = vec![0f32; strip_rows * w];
    let mut ok = vec![false; strip_rows * w];
    let mut vals = vec![0f64; strip_rows * w * k];
    let mut cell_area = Vec::new();
    let mut fetch = Fetch::default();
    let mut lab = Labeller::new(w, k);

    ctl.report(PHASE_CELLS, 0, Some(h as u64));
    let mut r0 = 0usize;
    while r0 < h {
        ctl.check()?;
        let nr = strip_rows.min(h - r0);
        let n = nr * w;
        // The first raster decides which cells are candidates.
        let src0 = sources.first_mut().ok_or_else(|| ZonalError::Input("give at least one condition".into()))?;
        src0.read_window(0, r0 as u32, w as u32, nr as u32, &mut raw[..n])?;
        let t0 = &tests[0];
        for i in 0..n {
            let v = raw[i];
            let pass = first.is_valid(v) && {
                let x = first.value(v);
                vals[i * k] = x;
                t0.holds(x)
            };
            ok[i] = pass;
        }
        ctl.check()?;
        // Each other raster, read only where the cells are still in.
        for (j, sampler) in samplers.iter().enumerate() {
            let src = sources.get_mut(j + 1).ok_or_else(|| ZonalError::Input("missing raster".into()))?;
            fetch.run(&mut **src, sampler, r0, nr, w, &mut ok[..n], &mut vals, k, j + 1, &tests[j + 1], ctl)?;
        }
        // Cell areas, only when some cell of the strip is in.
        let any = ok[..n].iter().any(|&b| b);
        if any {
            areas.strip(r0, nr, &mut cell_area, ctl)?;
        }
        for r in 0..nr {
            let row_ok = &ok[r * w..(r + 1) * w];
            let row_area: &[f64] = if !any {
                &[]
            } else if areas.geographic {
                &cell_area[r..r + 1]
            } else {
                &cell_area[r * w..(r + 1) * w]
            };
            lab.push_row((r0 + r) as u32, row_ok, &vals[r * w * k..(r + 1) * w * k], row_area);
        }
        r0 += nr;
        ctl.report(PHASE_CELLS, r0 as u64, Some(h as u64));
    }
    lab.finish(h as u32);
    let out = Outliner { first: &first, tf: first_tf.as_ref(), geographic: areas.geographic };
    let (areas, cells_in) = out.build(lab, ctl)?;
    Ok(WhereResult { areas, cells_in, grid_cells: w as u64 * h as u64 })
}

fn validate_grid(g: &Grid) -> Result<(), ZonalError> {
    if g.width == 0 || g.height == 0 {
        return Err(ZonalError::Input("the raster has no cells".into()));
    }
    if !g.transform.is_valid() {
        return Err(ZonalError::Input("the raster has no usable georeferencing".into()));
    }
    if !g.scale.is_finite() || !g.offset.is_finite() {
        return Err(ZonalError::Input("the raster's scale or offset isn't a number".into()));
    }
    Ok(())
}

/// None for WGS 84 longitude/latitude (no conversion).
fn transformer(crs: Crs) -> Result<Option<Transformer>, ZonalError> {
    match crs {
        Crs::LonLat => Ok(None),
        c => c.transformer().map(Some).map_err(ZonalError::Input),
    }
}

// ---- Sampling another raster at the first grid's cell centres --------------------------------

/// Where the first grid's cell centres fall on another raster.
struct Sampler {
    grid: Grid,
    /// Same coordinate system: the column depends only on the column, the row only on the row.
    cols: Option<Vec<u32>>,
    rows: Option<Vec<u32>>,
    first: Grid,
    first_tf: Option<Transformer>,
    other_tf: Option<Transformer>,
}

impl Sampler {
    fn new(first: &Grid, first_tf: Option<&Transformer>, grid: &Grid) -> Result<Sampler, ZonalError> {
        let mut s =
            Sampler { grid: grid.clone(), cols: None, rows: None, first: first.clone(), first_tf: first_tf.cloned(), other_tf: None };
        if first.crs == grid.crs {
            let ft = &first.transform;
            let gt = &grid.transform;
            let cols = (0..first.width)
                .map(|c| index((ft.origin_x + (f64::from(c) + 0.5) * ft.pixel_width - gt.origin_x) / gt.pixel_width, grid.width))
                .collect();
            let rows = (0..first.height)
                .map(|r| index((ft.origin_y + (f64::from(r) + 0.5) * ft.pixel_height - gt.origin_y) / gt.pixel_height, grid.height))
                .collect();
            s.cols = Some(cols);
            s.rows = Some(rows);
        } else {
            s.other_tf = transformer(grid.crs)?;
        }
        Ok(s)
    }

    /// The other raster's (column, row) holding the centre of the first grid's cell.
    fn cell(&self, c: usize, r: usize) -> Option<(u32, u32)> {
        if let (Some(cols), Some(rows)) = (&self.cols, &self.rows) {
            let (x, y) = (*cols.get(c)?, *rows.get(r)?);
            return (x != NONE && y != NONE).then_some((x, y));
        }
        let n = self.first.transform.to_world(c as f64 + 0.5, r as f64 + 0.5);
        let ll = match &self.first_tf {
            None => n,
            Some(t) => t.to_lonlat(n)?,
        };
        let o = match &self.other_tf {
            None => ll,
            Some(t) => t.from_lonlat(ll)?,
        };
        let (x, y) = self.grid.transform.to_pixel(o);
        let (x, y) = (index(x, self.grid.width), index(y, self.grid.height));
        (x != NONE && y != NONE).then_some((x, y))
    }
}

/// `floor(p)` when it is a cell index below `n`, else [`NONE`].
fn index(p: f64, n: u32) -> u32 {
    let f = p.floor();
    if f >= 0.0 && f < f64::from(n) { f as u32 } else { NONE }
}

/// Reads another raster's values for a strip's in cells and applies its test.
#[derive(Default)]
struct Fetch {
    /// (cell index in the strip, source column, source row) for each in cell.
    want: Vec<(u32, u32, u32)>,
    buf: Vec<f32>,
}

impl Fetch {
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        src: &mut dyn RasterSource,
        s: &Sampler,
        r0: usize,
        nr: usize,
        w: usize,
        ok: &mut [bool],
        vals: &mut [f64],
        k: usize,
        j: usize,
        test: &Test,
        ctl: &Control,
    ) -> Result<(), ZonalError> {
        self.want.clear();
        for r in 0..nr {
            // A conversion between coordinate systems per cell can be slow: check every row.
            ctl.check()?;
            for c in 0..w {
                let i = r * w + c;
                if !ok[i] {
                    continue;
                }
                match s.cell(c, r0 + r) {
                    Some((x, y)) => self.want.push((i as u32, x, y)),
                    None => ok[i] = false,
                }
            }
        }
        if self.want.is_empty() {
            return Ok(());
        }
        // Rows ascending, so each block of rows is read once.
        self.want.sort_unstable_by_key(|&(i, x, y)| (y, x, i));
        let (mut cmin, mut cmax) = (u32::MAX, 0u32);
        for &(_, x, _) in &self.want {
            cmin = cmin.min(x);
            cmax = cmax.max(x);
        }
        let span = (cmax - cmin + 1) as usize;
        let block_rows = (WINDOW_CELLS / span).max(1) as u32;
        let g = &s.grid;
        let mut a = 0usize;
        while a < self.want.len() {
            ctl.check()?;
            let y0 = self.want[a].2;
            let ylimit = y0.saturating_add(block_rows);
            let mut b = a;
            while b < self.want.len() && self.want[b].2 < ylimit {
                b += 1;
            }
            let y1 = self.want[b - 1].2;
            let rows = (y1 - y0 + 1) as usize;
            self.buf.clear();
            self.buf.resize(rows * span, 0.0);
            src.read_window(cmin, y0, span as u32, rows as u32, &mut self.buf)?;
            for &(i, x, y) in &self.want[a..b] {
                let v = self.buf[(y - y0) as usize * span + (x - cmin) as usize];
                let i = i as usize;
                if g.is_valid(v) {
                    let val = g.value(v);
                    vals[i * k + j] = val;
                    ok[i] = test.holds(val);
                } else {
                    ok[i] = false;
                }
            }
            a = b;
        }
        Ok(())
    }
}

// ---- Cell areas -------------------------------------------------------------------------------

/// The authalic function of WGS 84 (the zonal module's, which keeps it private).
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

    fn q(&self, lat_deg: f64) -> f64 {
        let s = lat_deg.clamp(-90.0, 90.0).to_radians().sin();
        s / (1.0 - self.e2 * s * s) + (self.e * s).atanh() / self.e
    }
}

struct CellAreas {
    grid: Grid,
    geographic: bool,
    tf: Option<Transformer>,
    au: Authalic,
}

impl CellAreas {
    fn new(grid: &Grid, tf: Option<&Transformer>) -> CellAreas {
        CellAreas { grid: grid.clone(), geographic: grid.crs.is_geographic(), tf: tf.cloned(), au: Authalic::wgs84() }
    }

    /// One area per row (geographic) or per cell (projected) for rows `r0..r0+nr`.
    fn strip(&self, r0: usize, nr: usize, out: &mut Vec<f64>, ctl: &Control) -> Result<(), ZonalError> {
        let t = &self.grid.transform;
        out.clear();
        if self.geographic {
            let dlon = t.pixel_width.abs();
            for r in r0..r0 + nr {
                let top = t.origin_y + r as f64 * t.pixel_height;
                let bottom = t.origin_y + (r + 1) as f64 * t.pixel_height;
                out.push(zonal::lat_band_area_m2(top, bottom, dlon));
            }
            return Ok(());
        }
        let w = self.grid.width as usize;
        let cw = w + 1;
        let mut corners = Vec::with_capacity((nr + 1) * cw);
        for r in 0..=nr {
            ctl.check()?;
            for c in 0..=w {
                let n = t.to_world(c as f64, (r0 + r) as f64);
                let ll = match &self.tf {
                    None => Some(n),
                    Some(tf) => tf.to_lonlat(n),
                };
                corners.push(ll.map(|ll| (ll.x.to_radians(), self.au.q(ll.y))).unwrap_or((f64::NAN, f64::NAN)));
            }
        }
        for r in 0..nr {
            for c in 0..w {
                let p = [corners[r * cw + c], corners[r * cw + c + 1], corners[(r + 1) * cw + c + 1], corners[(r + 1) * cw + c]];
                let mut s2 = 0.0;
                for i in 0..4 {
                    let (a, b) = (p[i], p[(i + 1) % 4]);
                    s2 += a.0 * b.1 - b.0 * a.1;
                }
                let area = self.au.half_b2 * 0.5 * s2.abs();
                out.push(if area.is_finite() { area } else { 0.0 });
            }
        }
        Ok(())
    }
}

// ---- Labelling and boundary edges ---------------------------------------------------------------

/// A boundary edge on the cell lattice (x = column line, y = row line), with its area on the left
/// (in the frame where rows go up, so exteriors run counter-clockwise).
#[derive(Clone, Copy, Debug)]
struct Edge {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    label: u32,
}

/// Streaming 8-connected labelling with union–find over provisional labels.
struct Labeller {
    w: usize,
    k: usize,
    prev_ok: Vec<bool>,
    prev: Vec<u32>,
    cur: Vec<u32>,
    parent: Vec<u32>,
    cells: Vec<u64>,
    area: Vec<f64>,
    /// `Σ x·A` per label and condition (`label * k + j`).
    sums: Vec<f64>,
    edges: Vec<Edge>,
}

impl Labeller {
    fn new(w: usize, k: usize) -> Labeller {
        Labeller {
            w,
            k,
            prev_ok: vec![false; w],
            prev: vec![NONE; w],
            cur: vec![NONE; w],
            parent: Vec::new(),
            cells: Vec::new(),
            area: Vec::new(),
            sums: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn find(&mut self, mut a: u32) -> u32 {
        let mut root = a;
        while self.parent[root as usize] != root {
            root = self.parent[root as usize];
        }
        while self.parent[a as usize] != root {
            let next = self.parent[a as usize];
            self.parent[a as usize] = root;
            a = next;
        }
        root
    }

    /// Joins two sets; the smaller label (the earlier first cell) stays the root.
    fn union(&mut self, a: u32, b: u32) -> u32 {
        let (ra, rb) = (self.find(a), self.find(b));
        let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
        self.parent[hi as usize] = lo;
        lo
    }

    fn push_row(&mut self, y: u32, ok: &[bool], vals: &[f64], areas: &[f64]) {
        let w = self.w;
        let k = self.k;
        for c in 0..w {
            if !ok[c] {
                self.cur[c] = NONE;
                continue;
            }
            let mut l = NONE;
            let neighbours = [
                if c > 0 { self.cur[c - 1] } else { NONE },
                if c > 0 { self.prev[c - 1] } else { NONE },
                self.prev[c],
                if c + 1 < w { self.prev[c + 1] } else { NONE },
            ];
            for n in neighbours {
                if n == NONE {
                    continue;
                }
                l = if l == NONE { self.find(n) } else { self.union(l, n) };
            }
            if l == NONE {
                l = self.parent.len() as u32;
                self.parent.push(l);
                self.cells.push(0);
                self.area.push(0.0);
                self.sums.extend(std::iter::repeat_n(0.0, k));
            }
            self.cur[c] = l;
            let a = if areas.len() == 1 { areas[0] } else { areas.get(c).copied().unwrap_or(0.0) };
            let li = l as usize;
            self.cells[li] += 1;
            self.area[li] += a;
            for j in 0..k {
                self.sums[li * k + j] += vals[c * k + j] * a;
            }
        }
        self.horizontal_edges(y, Some(ok));
        // Vertical edges between columns c−1 and c of this row.
        for c in 0..=w {
            let left = c > 0 && ok[c - 1];
            let right = c < w && ok[c];
            if left == right {
                continue;
            }
            let x = c as u32;
            if right {
                let label = self.cur[c];
                self.edges.push(Edge { x0: x, y0: y, x1: x, y1: y + 1, label });
            } else {
                let label = self.cur[c - 1];
                self.edges.push(Edge { x0: x, y0: y + 1, x1: x, y1: y, label });
            }
        }
        std::mem::swap(&mut self.prev, &mut self.cur);
        self.prev_ok.copy_from_slice(ok);
    }

    /// Edges along row line `y`, between the previous row and `ok` (None below the last row),
    /// merged into runs.
    fn horizontal_edges(&mut self, y: u32, ok: Option<&[bool]>) {
        // A run in progress: (start x, end x, label root, eastward).
        let mut run: Option<(u32, u32, u32, bool)> = None;
        for c in 0..self.w {
            let above = self.prev_ok[c];
            let below = ok.is_some_and(|o| o[c]);
            let x = c as u32;
            let piece = if above == below {
                None
            } else if above {
                // Area above: run east along its bottom.
                let l = self.prev[c];
                Some((x, x + 1, self.find(l), true))
            } else {
                let l = self.cur[c];
                Some((x + 1, x, self.find(l), false))
            };
            run = match (run, piece) {
                (Some(r), Some(p)) if r.3 == p.3 && r.2 == p.2 => Some(if p.3 { (r.0, p.1, r.2, true) } else { (p.0, r.1, r.2, false) }),
                (r, p) => {
                    if let Some((x0, x1, label, _)) = r {
                        self.edges.push(Edge { x0, y0: y, x1, y1: y, label });
                    }
                    p
                }
            };
        }
        if let Some((x0, x1, label, _)) = run {
            self.edges.push(Edge { x0, y0: y, x1, y1: y, label });
        }
    }

    /// Closes the bottom of the last row.
    fn finish(&mut self, h: u32) {
        self.horizontal_edges(h, None);
    }
}

// ---- Outlines -----------------------------------------------------------------------------------

struct Outliner<'g> {
    first: &'g Grid,
    tf: Option<&'g Transformer>,
    geographic: bool,
}

/// A closed ring of lattice vertices (no repeated last vertex).
type Ring = Vec<(i64, i64)>;

impl Outliner<'_> {
    fn build(&self, mut lab: Labeller, ctl: &Control) -> Result<(Vec<WhereArea>, u64), ZonalError> {
        let n = lab.parent.len();
        let k = lab.k;
        // Roots in label order are the areas in raster order of their first cell.
        let mut region = vec![NONE; n];
        let mut roots = Vec::new();
        for (l, slot) in region.iter_mut().enumerate() {
            if lab.find(l as u32) as usize == l {
                *slot = roots.len() as u32;
                roots.push(l);
            }
        }
        let mut cells = vec![0u64; roots.len()];
        let mut area = vec![0f64; roots.len()];
        let mut sums = vec![0f64; roots.len() * k];
        for l in 0..n {
            let g = region[lab.find(l as u32) as usize] as usize;
            cells[g] += lab.cells[l];
            area[g] += lab.area[l];
            for j in 0..k {
                sums[g * k + j] += lab.sums[l * k + j];
            }
        }
        let cells_in = cells.iter().sum();
        let mut edges = std::mem::take(&mut lab.edges);
        for e in &mut edges {
            e.label = region[lab.find(e.label) as usize];
        }
        edges.sort_unstable_by_key(|e| (e.label, e.y0, e.x0, e.y1, e.x1));
        let total = roots.len() as u64;
        ctl.report(PHASE_TRACE, 0, Some(total));
        let mut out = Vec::with_capacity(roots.len());
        let mut used = vec![false; edges.len()];
        let mut a = 0usize;
        let mut traced = 0usize;
        for g in 0..roots.len() {
            let mut b = a;
            while b < edges.len() && edges[b].label == g as u32 {
                b += 1;
            }
            let rings = trace(&edges[a..b], &mut used[a..b], ctl, &mut traced)?;
            let geometry = self.polygons(rings, ctl)?;
            let means = (0..k).map(|j| (area[g] > 0.0).then(|| sums[g * k + j] / area[g])).collect();
            out.push(WhereArea { geometry, cells: cells[g], area_m2: area[g], means });
            a = b;
            if g % 64 == 63 {
                ctl.check()?;
                ctl.report(PHASE_TRACE, g as u64 + 1, Some(total));
            }
        }
        ctl.report(PHASE_TRACE, total, Some(total));
        Ok((out, cells_in))
    }

    /// Groups an area's simple rings into polygons (each hole in the part whose cells border it)
    /// and converts them to longitude/latitude.
    fn polygons(&self, rings: Vec<Ring>, ctl: &Control) -> Result<Geometry, ZonalError> {
        // In lattice coordinates (rows down) an exterior has a negative shoelace sum.
        let outer: Vec<bool> = rings.iter().map(|r| shoelace2(r) < 0).collect();
        let n_outer = outer.iter().filter(|&&o| o).count();
        let mut part_of = vec![usize::MAX; rings.len()];
        let mut parts: Vec<Vec<usize>> = Vec::with_capacity(n_outer);
        for (i, &o) in outer.iter().enumerate() {
            if o {
                part_of[i] = parts.len();
                parts.push(vec![i]);
            }
        }
        if n_outer == 1 {
            for (i, &o) in outer.iter().enumerate() {
                if !o {
                    parts[0].push(i);
                }
            }
        } else if n_outer < rings.len() {
            // A hole belongs to the part whose cells border it. From the cell just left of the
            // hole's leftmost edge, go left along the row to the first boundary edge of the area:
            // the cells passed are one 4-connected part, so that edge's ring (its exterior, or
            // another of its holes, further left) belongs to the same part.
            let mut verticals: Vec<(i64, i64, usize)> = Vec::new();
            for (i, r) in rings.iter().enumerate() {
                for k in 0..r.len() {
                    let (a, b) = (r[k], r[(k + 1) % r.len()]);
                    if a.0 == b.0 {
                        verticals.push((a.1.min(b.1), a.0, i));
                    }
                }
            }
            verticals.sort_unstable();
            let broken = || ZonalError::Input("an area's hole lies outside it (internal error)".into());
            let mut link = vec![usize::MAX; rings.len()];
            for (i, r) in rings.iter().enumerate() {
                if outer[i] {
                    continue;
                }
                if i % 1024 == 0 {
                    ctl.check()?;
                }
                let mut left = (i64::MAX, i64::MAX);
                for k in 0..r.len() {
                    let (a, b) = (r[k], r[(k + 1) % r.len()]);
                    if a.0 == b.0 && (a.0, a.1.min(b.1)) < left {
                        left = (a.0, a.1.min(b.1));
                    }
                }
                let (x, y) = left;
                let at = verticals.partition_point(|&(vy, vx, _)| (vy, vx) < (y, x));
                let prev = at.checked_sub(1).and_then(|p| verticals.get(p)).filter(|v| v.0 == y).ok_or_else(broken)?;
                link[i] = prev.2;
            }
            // Follow the links to an exterior (each step goes strictly further left).
            for i in 0..rings.len() {
                if outer[i] {
                    continue;
                }
                let mut j = i;
                let mut steps = 0;
                while !outer[j] {
                    j = link[j];
                    steps += 1;
                    if j >= rings.len() || steps > rings.len() {
                        return Err(broken());
                    }
                }
                parts[part_of[j]].push(i);
            }
        }
        let parts: Vec<Vec<&Ring>> = parts.iter().map(|p| p.iter().filter_map(|&i| rings.get(i)).collect()).collect();
        let mut polys = Vec::with_capacity(parts.len());
        for part in parts {
            let mut rings = Vec::with_capacity(part.len());
            for (i, r) in part.iter().enumerate() {
                let mut ring = self.to_lonlat(r, ctl)?;
                // Exterior counter-clockwise, holes clockwise, in longitude/latitude.
                let ccw = signed_area(&ring) > 0.0;
                if ccw != (i == 0) {
                    ring.reverse();
                }
                if let Some(&f) = ring.first() {
                    ring.push(f);
                }
                rings.push(ring);
            }
            polys.push(rings);
        }
        Ok(Geometry::Polygon(polys))
    }

    /// A lattice ring in longitude/latitude: corners only on a geographic grid, every cell
    /// corner on a projected one.
    fn to_lonlat(&self, r: &Ring, ctl: &Control) -> Result<Vec<Coord>, ZonalError> {
        let t = &self.first.transform;
        let place = |x: i64, y: i64| -> Result<Coord, ZonalError> {
            let n = t.to_world(x as f64, y as f64);
            match self.tf {
                None => Ok(n),
                Some(tf) => {
                    tf.to_lonlat(n).ok_or_else(|| ZonalError::Input("an area's outline can't be converted to longitude/latitude".into()))
                }
            }
        };
        let corners = corners_only(r);
        let mut out = Vec::with_capacity(corners.len());
        for i in 0..corners.len() {
            let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
            if self.geographic {
                out.push(place(a.0, a.1)?);
                continue;
            }
            let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
            let (dx, dy) = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
            for s in 0..steps {
                out.push(place(a.0 + s * dx, a.1 + s * dy)?);
            }
            if i % 4096 == 4095 {
                ctl.check()?;
            }
        }
        Ok(out)
    }
}

/// Links an area's edges into simple rings: at a vertex with two ways on, turn left (stay with
/// the cell the ring came along), then cut rings at any vertex they visit twice.
fn trace(edges: &[Edge], used: &mut [bool], ctl: &Control, traced: &mut usize) -> Result<Vec<Ring>, ZonalError> {
    let broken = || ZonalError::Input("an area's outline doesn't close (internal error)".into());
    let mut rings = Vec::new();
    for e0 in 0..edges.len() {
        if used[e0] {
            continue;
        }
        let mut verts: Ring = Vec::new();
        let mut e = e0;
        loop {
            *traced += 1;
            if (*traced).is_multiple_of(65_536) {
                ctl.check()?;
            }
            used[e] = true;
            let cur = edges[e];
            verts.push((i64::from(cur.x0), i64::from(cur.y0)));
            let dir = (sign(cur.x1, cur.x0), sign(cur.y1, cur.y0));
            let lo = edges.partition_point(|f| (f.y0, f.x0) < (cur.y1, cur.x1));
            let mut hi = lo;
            while hi < edges.len() && (edges[hi].y0, edges[hi].x0) == (cur.y1, cur.x1) {
                hi += 1;
            }
            let next = match hi - lo {
                1 => lo,
                2 => {
                    // Left of (dx, dy) with rows down is (dy, −dx).
                    let left = (dir.1, -dir.0);
                    (lo..hi).find(|&f| (sign(edges[f].x1, edges[f].x0), sign(edges[f].y1, edges[f].y0)) == left).ok_or_else(broken)?
                }
                _ => return Err(broken()),
            };
            if next == e0 {
                break;
            }
            if used[next] {
                return Err(broken());
            }
            e = next;
        }
        split_simple(verts, &mut rings);
    }
    Ok(rings)
}

fn sign(b: u32, a: u32) -> i64 {
    (i64::from(b) - i64::from(a)).signum()
}

/// Cuts a closed vertex sequence into simple loops at every repeated vertex.
fn split_simple(verts: Ring, out: &mut Vec<Ring>) {
    let mut stack: Ring = Vec::with_capacity(verts.len());
    let mut at: HashMap<(i64, i64), usize> = HashMap::new();
    for v in verts {
        if let Some(&p) = at.get(&v) {
            let lp: Ring = stack.drain(p..).collect();
            for u in lp.iter().skip(1) {
                at.remove(u);
            }
            out.push(lp);
        }
        at.insert(v, stack.len());
        stack.push(v);
    }
    if stack.len() >= 3 {
        out.push(stack);
    }
}

/// Drops vertices where the ring goes straight on.
fn corners_only(r: &Ring) -> Ring {
    let n = r.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (p, v, q) = (r[(i + n - 1) % n], r[i], r[(i + 1) % n]);
        let d1 = ((v.0 - p.0).signum(), (v.1 - p.1).signum());
        let d2 = ((q.0 - v.0).signum(), (q.1 - v.1).signum());
        if d1 != d2 {
            out.push(v);
        }
    }
    out
}

/// Twice the signed area (shoelace) of a lattice ring.
fn shoelace2(r: &Ring) -> i64 {
    let n = r.len();
    let mut s = 0i64;
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        s += a.0 * b.1 - b.0 * a.1;
    }
    s
}

fn signed_area(r: &[Coord]) -> f64 {
    let n = r.len();
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        s += a.x * b.y - b.x * a.y;
    }
    s
}

#[cfg(test)]
mod tests;
