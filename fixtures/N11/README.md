# N11 — Find areas where rasters meet conditions

Three cases for `lm_analysis::where_areas::find_where` (puzzles › mechanics › A1).

**A** `a_shapes.tif`: 24 × 18 cells of 0.5° from 100.0° E, 40.0° N
(float32, NoData -9999). Condition: between 10 and 20 (inclusive). Drawn by hand
(`#` passes with values 10–20, `.` fails with 5 or 25, `x` NoData; `A` = 10.0 and `B` = 20.0
pass, `C` = 9.999 and `D` = 20.001 fail):

```
........................
.###..#....##..........#
.#.#.#.#...#.#..#.......
.###..#....###...#......
..................#.....
#######.................
##....#......####..###..
#.#...#......#.##..#x#..
#.....#......##.#..###..
#.....#......####..#.#..
#.....#............###..
#######..AB.............
........CD.....#.#.#....
#..............#.#.#....
##.............#####....
#.......#...............
#......#.#..............
#####...#..............#
```

It has a ring with a hole, four cells meeting only at corners around a gap (one area of
four parts; the gap is not a hole), a bay closed off at a corner (a hole touching the outline),
cells in a diagonal chain, a 7 × 7 ring whose inner corner cell touches an island at a corner
(the island is a part inside the hole), two holes meeting at a corner, a block with a NoData
hole and a value hole, a comb whose bays stay open, and cells on all four edges of the grid.

**B** `b_first.tif` (16 × 12 cells of 0.25° from 110.0° E, 30.0° N, int16, NoData
-32768, scale 0.1: tenths of a degree) with "at least 15", and `b_second.tif`
(8 × 6 cells of 0.6° from 109.83° E, 30.17° N, uint8 class codes 1–4, NoData
255) with "one of 2, 4". The second grid is coarser and shifted, so each first-grid cell
takes the class of the second-grid cell holding its centre (no centre lies on a cell edge).
Two first-grid cells and one second-grid cell are NoData. The scale is stored in the file's
metadata; LayeredMap's test gives it explicitly (`MemoryBand::with_scale_offset(0.1, 0.0)`).

**C** `c_utm.tif`: 12 × 10 cells of 2000 m in UTM zone 51N (EPSG:32651) from
x = 350000, y = 3.48e+06 (about 119.4° E, 31.4° N), float32 with two NoData cells.
Condition: at most 0.3.

Expected (from `reference/find_where.py`): `expected_find_where.csv` (one row per area:
`case`, `area` (1-based, raster order of the first cell), `cells`, `area_m2`, `geod_area_m2`,
`mean_1`, `mean_2`) and `expected_labels_<case>.csv` (the area number of every cell of the
first grid, 0 = out, one line per row). Definitions:

- cells in: numpy comparisons on values × scale + offset, NoData out; other rasters sampled at
  the first grid's cell centres by floor of the pixel position (through pyproj for a different
  coordinate system);
- areas: `scipy.ndimage.label` with a 3 × 3 structure;
- `area_m2`: the sum of cell areas on the WGS 84 ellipsoid: the closed latitude-band
  (authalic) formula per row for A and B; for C each cell's corners converted with pyproj and
  the quadrilateral straight in (λ, authalic q) measured by the shoelace formula;
- `geod_area_m2`: pyproj `Geod(ellps="WGS84").geometry_area_perimeter` of the polygons
  `rasterio.features.shapes` traces (connectivity 8), each oriented counter-clockwise, edges
  first densified to 1/64 cell in longitude/latitude (A, B: so the geodesic edges follow the
  parallels) or to one cell in UTM before conversion (C). The script asserts it is within
  1e-6 of `area_m2`;
- means: area-weighted, Σ x·A / Σ A over the area's cells, per raster (on the first grid).

Tolerances (mechanics › A1): identical cell sets and area counts; each area within 1e-9 of
`area_m2` and within 1e-6 of `geod_area_m2`.

Made by `reference/make_where_clip_fixtures.py` (synthetic, CC0-1.0).
