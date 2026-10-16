# RustGIS capability audit (WP0.2)

**As of 2026-10-15, upstream `75129448`.** Upstream already reads most of the formats and draws
online vector basemaps. Its analysis does not meet LayeredMap's definitions: every certified
action in Phase 4 needs a new implementation in `lm-analysis`. One finding matters for Gate 0:
the renderer drops Chinese basemap labels.

## Gaps that change the plan

| # | Finding | Consequence | Where it lands |
|---|---|---|---|
| 1 | Basemap labels: the built-in Protomaps style prefers `name:en`, and the renderer keeps only label text its Latin UI fonts can draw — **Chinese-only names are dropped** (`crates/render/src/tiles.rs:72`, `text::covers`). | Labels in Shanghai show English or nothing. | Phase 1 WP1.3: give the renderer the bundled Noto Sans SC and a Chinese-first label expression (`upstream:` change, offer back). |
| 2 | Geodesic length/area use a sphere (R = 6,371,008.8 m; Chamberlain–Duquette area). | Errors up to ~0.5%, vs the 0.01% target. | Phase 3 WP3.1 (GeographicLib port, as planned). |
| 3 | Buffers run in Web Mercator with one scale factor per feature; dissolve defaults to none. | Not geodesic. | Phase 4 `lm-analysis`. |
| 4 | Zonal summary: cell centres only, no area weighting, mean only, first polygon part only; large rasters silently read from an overview. | Fails the fractional-coverage definition and can summarise coarser pixels without saying so. | Phase 4 `lm-analysis`; the overview read must become a visible notice or be bypassed for analysis. |
| 5 | Summarize Within (`countPoints`) uses `intersects` (boundary counts in every area — matches) but runs in Web Mercator, so points on sloped edges can be missed; a MultiPoint counts once. | Close, not certified. | Phase 4. |
| 6 | No GCJ-02/BD-09 conversion. | As expected. | Phase 2 WP2.5. |
| 7 | GeoTIFF scale/offset ignored (only Zarr applies it). | R6 values decode wrong; Landsat ST_B10 assumption needs it. | Phase 2 WP2.2 (`upstream:` change). |
| 8 | CSV must be UTF-8 (T4, GBK, is refused); Chinese coordinate column names (经度/纬度) aren't recognised; Shapefile `.cpg` is never read (GBK .dbf with no code page decodes as lossy UTF-8, per code reading — not verified by printing). | Encoding detection is LayeredMap's to build. | Phase 2 WP2.2/WP2.3. |
| 9 | Mixed-geometry GeoJSON (V2) loads as one layer typed "Point" holding all 10 features. | LayeredMap must split it (assumption catalog row). | Phase 2. |
| 10 | No EPSG area of use in the bundled CRS definitions (CGCS2000 GK, Xian 1980, Beijing 1954 are present). | "Where should this data be?" needs its own bounds table. | Phase 2 WP2.4. |
| 11 | `read_path` refuses `.xlsx` (T5: "unsupported file type") although calamine is in the tree. | Excel needs its own path. | Phase 2 (P1). |
| 12 | Map view: `ui-egui/src/mapview.rs` (1,500 lines) is tied to `RustGisApp`, `Session` and 19 engine commands. A clean core of ~450 lines (texture cache, labels, camera, scale bar) extracts easily; ~300 lines of interaction go behind a host trait; ~750 lines are cut. | Extraction ≈ 3–4 days; the spike's ~900 lines already cover the core. | Phase 1 WP1.2. |

## Finder Open With and Dock-icon drops (spike carried from WP0.4, done in WP2.1)

**Works (2026-10-16).** winit 0.30.13 owns the application delegate and offers no opened-files
API. `apps/layeredmap/src/macos.rs` adds `application:openURLs:` to winit's delegate class when
`NSApplicationWillFinishLaunchingNotification` fires, queues the paths, and the app opens them
through `Command::Open` (the Open button's pipeline). `packaging/lm/bundle-macos.sh` registers
CSV/TSV, GeoJSON/JSON, Shapefile (.shp, .zip), GeoTIFF, GeoPackage and .xlsx as a Viewer with
`LSHandlerRank` Alternate (never the default app).

Verified on the bundled app with `open -a LayeredMap.app <files>` (the same Apple event as a Dock
drop or Open With), looking at window screenshots:
- not running: `T1/schools.csv` launches the app and "schools" (248 points) is in the Layers panel;
- running: `V1/districts.geojson` + `T5/survey.xlsx` in one event both arrive (districts draws;
  the workbook is misread as a zipped Shapefile by the importer, a separate bug), and a second
  `schools.csv` adds a second "schools" layer.

Not checked by hand: an actual mouse drop on the Dock icon or the Finder menu (not driven, to keep
input away from other apps).

## Analysis inventory

| Needed | Upstream | ID / function | Meets the definition? |
|---|---|---|---|
| Point-in-polygon counts | yes | tool `countPoints`, `analysis/src/tools.rs:614` | partly (see 5) |
| Which area contains a point | interactive only | `identify` → `hits_at`, `engine/src/cmd/select.rs:194` | no join tool |
| Geodesic buffer | no | tool `buffer`, `tools.rs:273` (Mercator) | no |
| Geodesic measurement | spherical | `geom/src/lib.rs:145,157` | no |
| Zonal summary (fractional coverage) | no | `rasterops::zonal_means`, `rasterops.rs:488` (centres) | no |
| Coordinate transforms | yes | `data/src/raster/crs.rs:119-198` (proj4rs, 6,184 EPSG codes) | mostly |
| GCJ-02 / BD-09 | no | — | — |
| Rank by a value | UI sort only | `ui-egui/src/table.rs:103` | trivial to build |
| Raster values at points | partly | `raster.identify` (nearest cell) | no scale/offset |

## Opening every fixture (RustGIS readers, `spikes/capability-audit`)

Run: `cargo run --release -p lm-capability-audit -- fixtures` (R5 and X1 run separately: R5 takes
20 s to read; X1 under a memory watchdog). "Right place" is judged against each `expected.json`.

| File | Opens | Layers · features · fields | Extent (lon/lat or native) | Time | Messages |
|---|---|---|---|---|---|
| C1/amap_pois.csv | yes | amap_pois Point · 50 · 4 | 121.364, 31.104, 121.652, 31.343 | 0.00 s (0 MB) |  |
| C2/vectors.csv | yes | vectors None · 58 · 13 | inf, inf, -inf, -inf | 0.00 s (0 MB) |  |
| C3/baidu_pois.csv | yes | baidu_pois Point · 30 · 4 | 121.367, 31.106, 121.657, 31.349 | 0.00 s (0 MB) |  |
| N1/areas.geojson | yes | areas Polygon · 4 · 1 | 121.000, 31.000, 122.000, 31.375 | 0.00 s (0 MB) |  |
| N1/points.geojson | yes | points Point · 20 · 1 | 121.000, 31.062, 121.688, 31.438 | 0.00 s (0 MB) |  |
| N2/areas.geojson | yes | areas Polygon · 3 · 1 | 121.000, 31.000, 122.000, 31.500 | 0.00 s (0 MB) |  |
| N2/points.geojson | yes | points Point · 12 · 1 | 121.062, 31.062, 121.938, 31.438 | 0.00 s (0 MB) |  |
| N3/grid4x4.tif | yes | raster 4×4 (1 bands) | 10.000, 58.000, 14.000, 62.000 | 0.00 s | crs LonLat |
| N3/polygon.geojson | yes | polygon Polygon · 1 · 1 | 10.500, 58.000, 13.500, 61.000 | 0.00 s (0 MB) |  |
| N5/cases.geojson | yes | cases Polygon · 9 · 2 | -180.000, 0.000, 180.000, 61.000 | 0.00 s (0 MB) |  |
| R3/landcover.tif | yes | raster 200×200 (1 bands) | 121.200, 30.900, 121.800, 31.500 | 0.00 s | crs LonLat |
| R6/points.csv | yes | points Point · 8 · 4 | 121.300, 31.221, 121.455, 31.299 | 0.00 s (0 MB) |  |
| R6/scaled.tif | yes | raster 24×24 (1 bands) | 121.375, 31.219, 121.469, 31.312 | 0.00 s | crs LonLat |
| T1/schools.csv | yes | schools Point · 248 · 5 | 121.214, 30.908, 121.793, 31.491 | 0.00 s (0 MB) | 6 skipped; 6 rows had no usable coordinates |
| T2/deliveries.csv | yes | deliveries Point · 30 · 6 | 31.041, 31.011, 121.674, 121.468 | 0.00 s (0 MB) | coordinates aren't longitude/latitude: give the file's coordinate system ("crs", such as E… |
| T4/stations_gbk.csv | **no** | — | — | 0.00 s | i/o: stream did not contain valid UTF-8 |
| T5/survey.xlsx | **no** | — | — | 0.00 s | unsupported file type: xlsx |
| V1/districts.geojson | yes | districts Polygon · 17 · 5 | 121.200, 30.900, 121.800, 31.500 | 0.00 s (0 MB) |  |
| V10/edge_cases.geojson | yes | edge_cases Polygon · 8 · 2 | -180.000, -17.000, 180.000, 90.000 | 0.00 s (0 MB) | 1 skipped; 1 features had unreadable geometry |
| V2/mixed.geojson | yes | mixed Point · 10 · 3 | 121.400, 31.200, 121.530, 31.320 | 0.00 s (0 MB) |  |
| V3/sites.shp | yes | sites Point · 25 · 3 | 121.251, 30.995, 121.696, 31.437 | 0.00 s (0 MB) |  |
| V4/districts_gk.shp | yes | districts_gk Polygon · 17 · 2 | 614006.599, 3420504.505, 672100.280, 3487815.100 | 0.00 s (0 MB) | coordinates are not longitude/latitude; RustGIS reads them as WGS 84 and they may draw out… |
| V5/places_gbk.shp | yes | places_gbk Point · 12 · 4 | 121.056, 31.097, 121.505, 31.299 | 0.00 s (0 MB) |  |
| V9/city_layers.gpkg | yes | parks Polygon · 3 · 2 | 121.430, 31.200, 121.520, 31.260 | 0.00 s (0 MB) | parks: the first of 3 feature tables (parks, roads, schools) |
| V9/roads_bundle.zip | yes | roads Polyline · 6 · 3 | 121.400, 31.180, 121.560, 31.280 | 0.00 s (0 MB) |  |
| generated/F1/f1_points.geojson | yes | f1_points Point · 100000 · 5 | 120.850, 30.650, 122.000, 31.850 | 0.23 s (18 MB) |  |
| generated/F1/f1_polygons.geojson | yes | f1_polygons Polygon · 10000 · 4 | 120.836, 30.641, 122.014, 31.862 | 0.22 s (22 MB) |  |
| generated/F2/f2_points.csv | yes | f2_points Point · 1000000 · 8 | 120.850, 30.650, 122.000, 31.850 | 0.92 s (67 MB) |  |
| generated/R4/r4_cog.tif | yes | raster 5760×5760 (1 bands) | 120.850, 30.650, 122.050, 31.850 | 0.78 s | crs LonLat |
| generated/R7/r7_wide_latitude.tif | yes | raster 1600×1600 (1 bands) | 100.000, -10.000, 180.000, 70.000 | 0.50 s | crs LonLat |
| generated/T3/t3_rows.csv | yes | t3_rows Point · 1000000 · 8 | 120.850, 30.650, 122.000, 31.850 | 0.86 s (67 MB) |  |
| generated/V7/v7_polygons.geojson | yes | v7_polygons Polygon · 100000 · 4 | 120.850, 30.650, 122.000, 31.851 | 0.31 s (28 MB) |  |
| generated/V8/v8_wide.geojson | yes | v8_wide Point · 1000 · 200 | 120.850, 30.650, 122.000, 31.850 | 0.06 s (4 MB) |  |
| generated/X2/x2_traversal.zip | **no** | — | — | 0.00 s | x2_traversal.zip: no Shapefile or other vector data in the archive could be read: ../../ev… |
| generated/X3/x3_huge_array.geojson | yes | x3_huge_array Polyline · 1 · 0 | 0.000, 0.000, 359.000, 89.000 | 3.73 s (172 MB) |  |
| generated/X3/x3_nan.geojson | **no** | — | — | 0.01 s | GeoJSON: expected value at line 1 column 77 |
| generated/X3/x3_nesting.geojson | **no** | — | — | 0.01 s | GeoJSON: recursion limit exceeded at line 1 column 203 |
| generated/X4/x4_billion.tif | **no** | — | — | 0.00 s | GeoTIFF: the file isn't georeferenced |
| generated/X5/x5_formulas.csv | yes | x5_formulas Point · 5 · 4 | 121.470, 31.230, 121.510, 31.270 | 0.00 s (0 MB) |  |
| generated/X1/x1_bomb.zip | **no** (refused) | — | — | < 0.01 s, peak RSS 6 MB | no vector data could be read |
| generated/R5/r5_striped.tif | yes | raster read through a decimated overview | — | 20.1 s, blocking | no overviews: read whole |

Notes on the table:
- **Right place:** every lon/lat fixture lands where its `expected.json` says. V4 (Gauss–Krüger, no
  .prj) is read as WGS 84 with a warning — it would draw off the map; T2's swapped columns are
  refused with a request for a coordinate system. Both are the cases the resolver and the question
  card exist for.
- **C2/vectors.csv** is a test-vector table (no geometry by design).
- **X3:** the huge-array file opens (longitudes up to 359° accepted silently); NaN and deep nesting
  are refused. **X4** is refused, but because it lacks georeferencing, not by a size limit; WP2.8
  adds explicit caps.
- Peak memory for the whole run: 3.3 GB (the 1,000,000-row CSVs and the 172 MB array).
