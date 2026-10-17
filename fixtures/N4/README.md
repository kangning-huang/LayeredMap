# N4 — Summary over R7 (unequal cell areas)

`polygons.geojson`: eight areas (id `area_id`) over R7, the generated
geographic raster (`cargo xtask fixtures --only R7` →
`fixtures/generated/R7/r7_wide_latitude.tif`: 1,600 × 1,600 float32, 0.05° cells,
100–180°E, 10°S–70°N, so a cell at 70°N has a third of the area of one at the
equator).

| id | shape |
|---|---|
| TALL | a sloped quadrilateral from 5°S to 66°N |
| TRI | a large triangle, 0–60°N |
| HOLE | a quadrilateral 20–58°N with a star-shaped hole |
| MULTI | two triangles, one at the equator and one at 62–69.5°N |
| EDGE | a quadrilateral crossing R7's northern edge (70°N), partly outside |
| STAR | a 9-pointed star at 45°N |
| DIAG | a thin sliver from 8°S, 101°E to 68°N, 178°E (long sloped edges) |
| SMALL | a triangle smaller than a cell at 60°N |

Expected:

- `expected_zonal_mean.csv` — `reference/zonal_r7.py` with `reference/zonal_mean.py`'s
  definition: exactextract coverage fractions × geodesic cell areas (pyproj.Geod).
  Tolerance 1 part per million. Every area is cross-checked with exact shapely
  fractions (`reference/_crosscheck.py`); for TRI and DIAG the checked values are used
  because exactextract 0.3.0 leaves out two fully covered interior cells of each
  (6.4e-6 and 3.9e-5 of the covered area). The `source` column says which.
- `expected_geodesic_integration.csv` — `reference/zonal_r7.py`: the value integrated
  over each area on the ellipsoid, with each covered piece of a cell (the exact
  intersection of the cell and the area, edges straight in longitude/latitude)
  measured by pyproj.Geod after densifying its edges to 0.001°. Tolerance 1 part per
  thousand (the plan's tolerance for the geodesic integration: a flat coverage
  fraction weighted by the whole cell's area differs from the covered piece's own
  ellipsoidal area along the boundary).

Made by `reference/make_summary_fixtures.py` (synthetic, CC0-1.0).
