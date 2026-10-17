# Reference scripts (WP0.8 fixture factory)

Python scripts that (1) build the small hand-made fixtures in `fixtures/` and
(2) compute the expected outputs LayeredMap's certified actions are tested
against. They are the *reference*, not the product: each one states its
definition in its docstring, and the Rust implementation must match it.

## Environment

```bash
mamba env create -f reference/environment.yml
mamba run -n layeredmap-ref pip install --no-deps coord-convert==0.2.1
```

`_common.py` pins `PROJ_DATA` / `GDAL_DATA` to the interpreter's own env, so a
PROJ/GDAL from another install (for example an anaconda base env on PATH)
cannot leak in. It also switches PROJ networking off and fixes GPKG timestamps.

## Regenerate everything

```bash
mamba run -n layeredmap-ref python -I reference/run_all.py --check
```

`--check` runs the whole pipeline twice and fails unless every file under
`fixtures/` (except `fixtures/generated/`) has the same SHA-256 both times.
Leave out `--check` to run it once. You can also run any script on its own, with
no arguments for its fixture jobs or with explicit paths (see each `--help`).

| script | writes | definition (summary) |
|---|---|---|
| `make_handmade.py` | data + `README.md` + `expected.json` for V1–V5, V9, V10, T1, T2, T4, T5, C1, C3, R3, R6, N1–N3, N5, N8 | seeded and deterministic; zip/xlsx/dbf/gpkg timestamps fixed |
| `make_summary_fixtures.py` | R2 (synthetic population counts + density), N4 polygons, N6 areas, N7 `layouts/` GeoTIFFs (written by GDAL), with READMEs | seeded and deterministic |
| `coord_vectors.py` | `C2/vectors.csv`, `C2/README.md`, `C2/landmarks_TODO.csv` | WGS84→GCJ-02→BD-09 with prcoords and coord-convert, cross-checked against eviltransform |
| `count_in_areas.py` | `N1`, `N2`, `T1` (V1×T1) `expected_count_in_areas.csv` | sjoin `intersects`: a boundary point counts in every area it touches, and zero-count areas are kept |
| `which_area.py` | `N1/expected_which_area.csv` | every area containing each point (a boundary point gets both) |
| `zonal_mean.py` | `N3/expected_zonal_mean.csv` | exactextract fraction × geodesic cell area; NoData excluded; coverage = valid covered area / polygon area; checked against the hand calculation |
| `category_shares.py` | `N8/expected_category_shares.csv` | exactextract `weighted_frac` with a cell-area weight raster |
| `zonal_totals.py` | `N6/expected_totals.csv` | count total Σ n·f (exactextract `sum`); density total Σ d·f·A / 10⁶ (exactextract `weighted_sum` with a cell-area weight raster); cross-checked (below) |
| `zonal_r7.py` | `N4/expected_zonal_mean.csv`, `N4/expected_geodesic_integration.csv` | `zonal_mean.py`'s definition on R7, cross-checked; and a geodesic integration: each covered piece (exact cell ∩ area) measured by pyproj.Geod with edges densified to 0.001°. Needs `cargo xtask fixtures --only R7` |
| `n7_sample.py` | `N7/expected_zonal_mean_sample.csv` | `zonal_mean.py`'s definition on each of 31 sampled N7 areas' windows of R4, cross-checked. Needs `cargo xtask fixtures --only R4,N7` |
| `geodesic.py` | `N5/expected_measure.csv`, `N5/expected_buffer.csv` | pyproj.Geod lengths and areas; 360-vertex geodesic circles; local AEQD buffers |
| `raster_sample.py` | `R6/expected_raster_sample.csv` | containing cell with floor indexing (a point on an edge takes the cell to its right or below), scale/offset applied, NoData left blank |
| `rank_check.py` | `V1/expected_insight.csv` | rank phrases: upper or lower half, ties, ≤2 objects get no rank, ≥1000 objects get a percentile |
| `run_all.py` | — | runs all of the above in order |

Numbers are written with up to 12 significant digits.

## exactextract cross-check (`_crosscheck.py`)

exactextract 0.3.0 is wrong on some shapes, so the newer summary references
(`zonal_totals.py`, `zonal_r7.py`, `n7_sample.py`) compute every area twice: with
exactextract, and with each cell's exact covered fraction from a shapely
intersection in the raster's coordinates (the same quantity). The CSV keeps
exactextract's numbers unless the two differ by more than 1e-7 (exactextract
stores fractions in float32, so honest agreement is about 1e-10), and its `source`
column says which was used. Defects found:

- N4 TRI and DIAG: two fully covered interior cells each are missing from
  exactextract's output (6.4e-6 and 3.9e-5 of the covered area);
- N6 S5: a cell outside the area, past the raster's eastern edge, gets coverage 1
  (0.57 % of the covered area).

The geodesic integration (N4) and LayeredMap agree with the shapely check on all
of them. `zonal_mean.py` and `category_shares.py` (N3, R3 × V1, N8) are unchanged;
LayeredMap matches them to 1e-6 already, so no defect touches those shapes.
