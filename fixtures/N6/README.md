# N6 — Totals from a count raster and from a density raster

Inputs (not duplicated): `../R2/population_count.tif` (people per cell, uint16,
NoData 65535), `../R2/population_density.tif` (people per km², float32, NoData
-9999), `../V1/districts.geojson` (17 districts, id `district_id`; D04 is a
MultiPolygon, D06 has a hole).

`areas.geojson`: six more areas (id `area_id`) that cut cells, because V1's
districts mostly follow R2's cell edges: S1 a star across the town centre and the
river (NoData), S2 a triangle partly west of the raster, S3 a quadrilateral with a
star-shaped hole, S4 a two-part multipolygon, S5 a sliver into the NoData sea, S6 a
triangle inside one cell.

Expected: `expected_totals.csv` (V1's districts, then S1–S6) from
`reference/zonal_totals.py`:

- `count_total` = Σ n_i f_i over valid cells — exactextract `sum` (coverage
  fraction × value) on the count raster;
- `density_total` = Σ d_i f_i A_i / 10⁶ — exactextract `weighted_sum` of the density
  raster with a geodesic cell-area weight raster (pyproj.Geod), in people;
- `covered_valid_area_m2` = Σ f_i A_i over valid cells.

Every area is cross-checked with exact shapely fractions
(`reference/_crosscheck.py`); for S5 the checked values are used because
exactextract 0.3.0 gives a cell outside S5 (beyond the raster's eastern edge) a
coverage of 1, 0.57 % of its covered area. The `source` column says which.

Because the density was made as count ÷ cell area, the two totals agree to the
density's float32 rounding (about 1e-7), which the reference asserts: the two
formulas are different routes to the same people, and a card must use the one
that matches the raster's meaning. Tolerance 1 part per million.

Made by `reference/make_summary_fixtures.py` (synthetic, CC0-1.0).
