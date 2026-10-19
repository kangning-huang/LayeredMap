# LayeredMap test fixtures

Each fixture lives in `fixtures/<ID>/` and has its data files, a `README.md`
(what it is, what it tests, how it was made) and, where there is an expected
outcome, an `expected.json` (profile expectations) and/or `expected_<action>.csv`
(numbers from `reference/`). Licences are in [`LICENSES.md`](LICENSES.md).

To regenerate the hand-made fixtures and every expected file (the output is
deterministic):

```bash
mamba run -n layeredmap-ref python -I reference/run_all.py --check
```

The generated fixtures are rebuilt with `cargo xtask fixtures` and written to
`fixtures/generated/` (ignored by git; never commit it). `reference/zonal_r7.py`
(N4) needs `--only R7` and `reference/n7_sample.py` (N7) needs `--only R4,N7`;
without them they print "skipped".

## Index

| ID | path | what it tests | status |
|---|---|---|---|
| F1 | `generated/F1/` | — | generated (`cargo xtask fixtures`) |
| F2 | `generated/F2/` | — | generated (`cargo xtask fixtures`) |
| V1 | `V1/` | import, insight, counts (17 districts, hole, multipolygon) | present |
| V2 | `V2/` | mixed-geometry split into 3 layers | present |
| V3 | `V3/` | Shapefile with no .prj, degrees: visible WGS 84 assumption | present |
| V4 | `V4/` | Shapefile with no .prj, EPSG:4549: "Where should this data be?" | present |
| V5 | `V5/` | GBK .dbf with no .cpg: encoding detection | present |
| V6 | — | OSM extract | TBD (licence to verify, ODbL) |
| V7 | `generated/V7/` | — | generated (`cargo xtask fixtures`) |
| V8 | `generated/V8/` | — | generated (`cargo xtask fixtures`) |
| V9 | `V9/` | Shapefile in a nested zip folder, and a 3-layer GeoPackage | present |
| V10 | `V10/` | geometry edge cases and honest errors | present |
| V11 | `V11/` | KML with folders, styles and MultiGeometry; KMZ; malformed KML/KMZ (WP6.7), checked against GDAL | present |
| V12 | `V12/` | GPX waypoints, tracks and routes; GPX 1.0 in ISO-8859-1; malformed GPX (WP6.7), checked against GDAL | present |
| T1 | `T1/` | lon/lat CSV with 242 located and 6 excluded | present |
| T2 | `T2/` | ambiguous coordinate columns: question card | present |
| T3 | `generated/T3/` | — | generated (`cargo xtask fixtures`) |
| T4 | `T4/` | GBK, `;`, decimal commas, leading zeros: raw strings preserved | present |
| T5 | `T5/` | 3-sheet workbook: sheet choice | present |
| C1 | `C1/` | GCJ-02 points: offset question (name hint) | present |
| C2 | `C2/` | GCJ-02 / BD-09 test vectors | vectors present; landmarks owner-supplied (`landmarks_TODO.csv`) |
| C3 | `C3/` | BD-09 points | present |
| R1 | — | — | owner-supplied |
| R2 | `R2/` | integer population counts with NoData, and a density version (synthetic stand-in for the open population grid) | present |
| R3 | `R3/` | land-cover categories (synthetic WorldCover stand-in) | present |
| R4 | `generated/R4/` | 2.9 GB tiled BigTIFF COG with overviews (N7) | generated (`cargo xtask fixtures --only R4`) |
| R5 | `generated/R5/` | — | generated (`cargo xtask fixtures`) |
| R6 | `R6/` | scale/offset, -9999 edges, no NoData tag | present |
| R7 | `generated/R7/` | geographic raster 10°S–70°N, unequal cell areas (N4) | generated (`cargo xtask fixtures`) |
| R8 | `R8/` | NetCDF grids (WP6.12): NetCDF-3 (CDF-1, CDF-2) and NetCDF-4 with deflate and shuffle; scale/offset, fill values, levels; standard, noleap and 360_day calendars; 0–360 longitudes; a curvilinear refusal; checked against xarray | present |
| N1 | `N1/` | count rules: shared boundary, overlap, zero-count area | present |
| N2 | `N2/` | count rules: holes, multipolygons | present |
| N3 | `N3/` | 4×4 area-weighted mean and coverage (with hand calculation) | present |
| N4 | `N4/` (+ `generated/R7/`) | summary over R7: unequal cell areas, against exactextract and a geodesic integration | present |
| N5 | `N5/` | geodesic lengths, areas, buffers | present |
| N6 | `N6/` | count versus density totals (R2 × V1 and six cut-cell areas) | present |
| N7 | `N7/` (+ `generated/R4/`, `generated/N7/`) | R4 across 3,000 areas (stress, sampled exactextract check); `layouts/`: GeoTIFFs in every layout the window reader handles | present; polygons generated (`cargo xtask fixtures`) |
| N8 | `N8/` | category shares (R3 × V1) | present |
| N11 | `N11/` | find_where: holes, corner contacts, NoData, two rasters on different grids (scale/offset, categories), a projected raster | present |
| N12 | `N12/` | keep_inside / remove_inside: overlapping mask with a hole and parts; areas with holes and parts, a sliver; lines; points on edges | present |
| N9 | `N9/` | nearest distance and island isolation: same-layer multipart exclusion, touching, holes, antimeridian, high latitude | present |
| N10 | `N10/` | join by location: first area in row order, boundaries, holes, antimeridian, lines and areas by label point | present |
| J6 | `J6/` | match a table to a layer (WP6.4): one-to-one, one-to-many, many-to-one, many-to-many, key columns with different names; expected row counts; unmatched and blank keys; against pandas `merge(validate=…)` | present |
| N14 | `N14/` | nearest feature (WP6.4): points to points, lines and areas, areas to points; against GeoPandas `sjoin_nearest` in UTM 51N and pyproj Geod | present |
| N15 | `N15/` | keep data inside an area (WP6.4): points on edges and corners, lines along an edge, areas that only touch, multipart, empty results; against GeoPandas `clip` | present |
| N13 | `N13/` | per-feature measurements (area, perimeter with holes, length, coordinates) and label points | present |
| K1 | — | — | owner-supplied |
| D1–D3 | `D/D1`–`D/D3` | v0 document folders as Gates 2, 3 and 4 saved them (synthetic: layers with answers; then styles, filters, roles, a join; then a drawn area and a kept result); format migration and save/reopen equality (`lm-doc` `container::tests`) | present; rewrite with `LM_WRITE_DOC_FIXTURES=1 cargo test -p lm-doc --lib write_gate_fixture` |
| D4 | — | — | a Gate 5 `.lmap`, at Gate 5 |
| B1–B20 | — | — | owner-supplied |
| X1–X4 | `generated/X1/` … `generated/X4/` | — | generated (`cargo xtask fixtures`) |
| X5 | `X5/` (also `generated/X5/`) | CSV cells beginning with =, +, - or @: no formula injection on export | present |

## `expected.json` conventions

- `kind`: `vector`, `table`, `raster` or `raster+vector`.
- `layers[]`: `geometry` is `point`, `line` or `area` (Multi* types count as their single-part kind), plus `count`.
- `crs.source`: `declared` (in the file, or the GeoJSON RFC 7946 default),
  `assumed` (there must be a visible assumption) or `resolved` (the user confirms
  it through the "Where should this data be?" resolver).
- `fields`: the role of each column. Roles are `id`, `name`, `measure`,
  `category`, `code`, `date`, `longitude` and `latitude`. A trailing `?` means the
  role is ambiguous and the app should ask.
- `assumptions[]`: assumptions that should fire and be shown to the user. Each
  has an `id`, plus `visible: true`.
- Row numbers are 1-based data rows, not counting the header.
- Numeric expectations are in `expected_*.csv` and are written with up to 12
  significant digits. A suggested tolerance is 1e-9 relative for closed-form
  results and 1e-3 relative for buffers.
