# R2 — Population counts with NoData (synthetic stand-in)

The plan's R2 is an open population grid whose licence is still to be verified,
so this is a **synthetic stand-in** with the same shape of data. Nothing was
downloaded.

- `population_count.tif`: 72×72 **uint16** people per cell, EPSG:4326, 30 arc-second
  cells (1/120°) over V1's extent (121.2–121.8°E, 30.9–31.5°N). NoData = 65535
  (205 cells: a river and the sea in the north-east corner). Strips of 8 rows,
  LZW with the horizontal predictor. Values: Poisson draws (seed 2) around four
  Gaussian "town" centres over a rural floor of 120 people per cell; 13,568,180
  people in the valid cells.
- `population_density.tif`: the same grid as **float32** people per km²: each count
  divided by its cell's area on the WGS 84 ellipsoid (closed-form authalic formula).
  NoData = -9999 on the same cells. 32 × 32 tiles, Deflate with the
  floating-point predictor.

Tests: count totals `N = Σ n_i f_i` versus density totals `N = Σ d_i a_i` (N6),
and the window reader on striped LZW uint16 and tiled Deflate float32 files.

Made by `reference/make_summary_fixtures.py` (synthetic, CC0-1.0).
