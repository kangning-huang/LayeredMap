# LayeredMap Pro and public GIS benchmarks: coverage

*2026-10-17. The tasks of 16 public GIS and tool-use benchmarks, mapped one by one onto LayeredMap's
assistant actions. The data is in `tools/lm/benchmarks/` (`sources/*.jsonl`, one record per task,
and `suite.jsonl`, the cases the eval can run). The tables between the markers come from
`coverage.py`; the text around them is written by hand.*

## Summary

- **Most public GIS tasks are not answerable today.** Among the 10 GIS-analysis benchmarks,
  14% of tasks are answerable now (micro; 19% macro). Counting only calls I'm confident in, it's
  8%. Wrapping existing RustGIS functions would raise that to 40% (52% macro). Another 31% need
  new algorithms, and 29% should always be refused: downloads, imagery, code, live services.
- **Of the tasks LayeredMap should do,** 19% work now and 56% would work after wrapping
  existing functions.
- **The gap is basic tabular GIS before it is advanced analysis.** The most widespread missing
  operations are a per-feature area or length field, a field calculator (density, per capita),
  summary statistics, the nearest feature and reprojection. Each appears in 7–9 of the 10
  analysis benchmarks, and each, except the nearest search, wraps code that already exists.
- **The Gate 4 hallway question "Which district has the most schools per km²?" can't be
  answered today.** It needs `calculate_field` (or `calculate_geometry` plus a ratio) after
  `count_in`. This is the strongest argument for adding a field calculator first.
- **Refusals are where the volume is.** 1,412 of 2,275 tasks should be refused. BFCL, ACEBench,
  T-Eval, MapEval-API and UnivEARTH supply a large bilingual refusal set for Gate 7's 95%
  threshold.
- **The suite has 582 cases:** 249 answerable, 2 ask_user, 331 refusals, 176 in Chinese.
  `--full` writes all 1,454 cases.

## Method

**Records.** Each benchmark task is one record. A record gives the source, task id, licence,
how much of the original text is in git (`text_policy`), a summary of the task, and the
operation family (`category`, from `vocab.CATEGORIES`). It also gives a coverage class:

| Class | Meaning |
|---|---|
| `now` | Today's 13 connected actions and the display tools answer it in one of our map contexts |
| `upstream` | Every missing operation wraps an existing function in this repo (`crates/analysis`, `crates/lm-analysis`, an engine command), named in `vocab.OPS` |
| `new` | At least one missing operation needs a new algorithm |
| `out_of_scope` | LayeredMap should refuse it (`oos_reason`: internet data, imagery, code, live routing, geocoding service, prediction, knowledge, missing data, irrelevant) |

Other fields:
- **missing:** the missing operations, from `vocab.OPS`. `validate.py` checks that the class
  agrees with them.
- **confidence:** high, medium or low.
- **cases:** for `now`, vague and `out_of_scope` tasks, requests in the eval's case schema
  (`expected`, `alts`, `lenient_alts`), with Chinese variants.

`upstream` and `new` tasks carry no cases, because their expected plan changes once the action
exists. Today they should be refused, so they are a ready source of future refusal and answerable
cases.

**Contexts.** Cases use the eval's own contexts (`world`, `shanghai`, `shanghai_drawn`) and eight
new value-free contexts in `tools/lm/benchmarks/contexts/`:
- `world_plus`: countries with indicators, places, ports, power plants, rivers, railways and
  global rasters;
- `usa`, `geoquery_usa`, `sqqa_usa` and `copilot_us`: US states, counties and cities, US POIs;
- `city`, `city_drawn` and `city_open_data`: a city's neighbourhoods, POIs, streets, buildings
  and rasters.

`run_suite.py` lets `run_eval.py` build system messages for them in memory. A test checks that
the system messages come out byte-identical to the eval's for its own contexts.

**Code-generation benchmarks.** GeoAnalystBench, GeoAgentBench, GTChain, GIS Copilot, GeoJSON
Agent and ShapefileGPT are workflows over their own data. Each task was reduced to the
plan-level request a LayeredMap user would make, then classified by the operations that carry
its analysis. A task whose only answerable part is a side step is not `now`.

**Checks.**
- `validate.py` checks every expected plan:
  - action and argument names against the runner;
  - parameters the runner reads, including `stat` and `field`;
  - layers that exist in the case's context, of the right kind for each role;
  - fields those layers have, or that earlier steps create;
  - filter operators, and `#k` references.
- `test_benchmarks.py` covers the following:
  - each expected plan, sent through the eval's own parser and scorer, scores strict;
  - a mocked dry run through `run_eval.py` scores every sampled case;
  - `suite.jsonl` matches a fresh build.

## How sure is this

- **249 of 2,275 records are medium or low confidence.** These are mostly `now` calls that
  assume a field exists in our context, or that take an approximate plan as the answer.
  - Examples of fields assumed: `net_migration` on Countries, so GeoBenchX's table join isn't
    tested; a `category` field on Places.
  - Examples of approximate plans: `whats_inside` for "states bordering Texas" also lists
    Texas; a filter for "how many …" leaves the count to the shown total.
  - Counting only high-confidence `now`, the analysis-benchmark share answerable now drops from
    14% to 8%.
- **Classification is by reading, not by running.** The models' real accuracy on these cases is
  unknown until the suite is run, and the current baseline is 88% strict on our own 78 cases.
  Treat "answerable now" as an upper bound on what a perfect planner could do with today's
  actions.
- **The pools are skewed.**
  - GTChain (500 tasks) and MapEval-API (300) dominate the pooled numbers, hence the macro
    columns.
  - GeoAgentBench/GABench (Yu et al.) was built from GeoAnalystBench. Its paper says it
    screened GeoAnalystBench's 50 cases, kept 40 and re-engineered them into tool flows. Counting them as two benchmarks inflates the benchmark count of
    their raster and hydrology ops by one.
  - GTChain's tool chains are machine-generated: its "now" tasks are single Buffer, Select or
    Extract Values steps.
- **The split between `upstream` and `new` depends on how much of an operation the existing
  function covers:**
  - `nearest` counts as upstream on `network::within`, but needs a nearest search on top;
  - `intersect` counts as upstream on `tools::clip`, but merging attributes is new;
  - `reproject` counts as upstream on `rustgis_data::raster::Crs`, whose vector side is
    unverified.

  All of these upstream functions still need certification (a written definition, a reference
  implementation, tolerances) before they can be LayeredMap actions. That work is not counted
  here.
- **For paper-only benchmarks, the record counts are the tasks I could identify:**
  - ShapefileGPT: 25 of 42;
  - GeoLLM-Engine: 11 templates, of 250 seed queries and 100k generated ones;
  - GIS Copilot: 110 tasks from its appendix tables;
  - GeoJSON Agent: all 70 tasks from its appendices.

  Their requests are our own wording, so they test the same operations, not the same sentences.
- **The eval's tools don't yet expose the runner's new parameters.** `tools/lm/llm_eval/tools.py`
  (`PARAMS_OF_ACTION`) has no `stat` on `count_in`/`summarize_*` and no `field` on `count_in`.
  19 suite cases expect them, and a model can't produce them until `tools.py` adds them.
  `validate.py` reads the parameters from `actions.rs`, so these plans count as valid.

## Recommended order for adding actions

The order weighs reach (benchmarks and tasks needing the op) against cost: wrapping certified or
existing code comes before new algorithms, and gate questions come first.

1. **`calculate_field` (ratio/density/per capita), with `calculate_geometry`**
   (`engine cmd edit.calculateField`, `analysis::tools::calculate_geometry`). Needed in 9
   benchmarks, about 90 tasks, and the Gate 4 hallway question. Area and length reuse the
   certified geodesic measures. Restrict the calculator to a few certified forms (a ÷ b, a ÷
   area, unit conversion) rather than free expressions.
2. **`summary_statistics` and `frequency`** (`analysis::tools::summary_statistics`). Needed in 9
   benchmarks: "average income of all counties", "how many plants of each fuel type". Cheap,
   and it closes the "how many …" gap that today's plans answer with a filter's shown count.
3. **`distance_between`** (rules.toml `measure_from_here`, `lm_analysis::geodesic`, already
   certified). Few tasks, but nearly free, and it turns several BFCL, T-Eval and ACEBench
   refusals into answers.
4. **`nearest`** (on `analysis::network::within`, adding a nearest search). Needed in 7
   benchmarks, 60 tasks: nearest hospital, distance to nearest station.
5. **`select_by_location` across a whole layer, plus `clip`, `erase` and `intersect`**
   (`analysis::tools`). These cover the overlay family (51 upstream tasks) and multi-feature
   "inside any of" questions that `whats_inside` can't chain.
6. **`table_join`** (new, but simple; `match_by_code` is already in rules.toml for Phase 6).
   GeoBenchX alone has 61 tasks that are "join an indicator table, then map it".
7. **`centroid`, `convex_hull`, `dissolve`, `buffer_polygons`** (`analysis::tools`). Cheap
   generalization wraps.
8. **Raster algebra: `reclassify`, `band_math`, `slope`/`aspect`, `weighted_overlay`**
   (`analysis::rasterops`). Suitability and terrain workflows (GeoAnalystBench, GABench, GIS
   Copilot).
9. **`reproject` and `export_format`.** Mostly data-management benchmarks (pothos GeoAgentBench).
   This is a product decision: LayeredMap never modifies sources, so this would be export
   only.
10. **New algorithms, by reach:** `point_clustering`, `kernel_density`/`hot_spots`,
    `raster_to_vector`, `thiessen`, `distance_matrix`, `contours`, `kriging`.

## Gate 4 and Gate 7

- **Gate 4** is about numeric correctness of the certified actions, and the owner's three
  questions. The benchmark families that exercise the same actions are spatial_join (counts,
  which area, what's inside), buffer, zonal_stats, raster_sampling, measurement and statistics
  (ranking). Their `now` cases are plan-level checks only. Gate 4's numeric fixtures (N1–N7)
  stay the numeric authority, and these cases add no numeric references.
- **Gate 7** asks for at least 200 real questions, at least 90% right on answerable ones and at
  least 95% honest refusals.
  - All 249 answerable suite cases feed the first threshold, all 331 refusals the second.
  - The 2 vague cases feed the question budget.
  - The suite meets the 200-question size on its own. It is not "real questions over the K1
    teaching set", so it complements WP7.5's evaluation set rather than replacing it.
  - Score the families separately: refusal-only sources (BFCL, MapEval-API, UnivEARTH) are
    easier than the near-miss refusals from GeoBenchX's unsolvable tasks.

The generated table at the end of the next section maps each family to its gate check.

## Tables

<!-- generated:start -->

### Tasks by benchmark and coverage class

| Benchmark | Licence | Text in git | Tasks | now | upstream | new | out of scope | medium/low confidence | Cases (all / zh / refusals) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| GTChain-Eval | unclear: MIT LICENSE file, README restricts to non-commercial research use | own_words | 500 | 22 | 111 | 131 | 236 | 10 | 266 / 55 / 236 |
| BFCL v4 irrelevance | Apache-2.0 | verbatim | 312 | 2 | 3 | 0 | 307 | 3 | 310 / 5 / 307 |
| MapEval-API | none (no LICENSE file) | own_words | 300 | 0 | 0 | 0 | 300 | 0 | 300 / 62 / 300 |
| ACEBench | MIT (LICENSE.md: Copyright (c) Microsoft Corporation) | adapted, verbatim | 253 | 5 | 5 | 2 | 241 | 12 | 248 / 124 / 241 |
| GeoBenchX | MIT | adapted, verbatim | 202 | 16 | 13 | 89 | 84 | 52 | 109 / 9 / 84 |
| UnivEARTH | MIT | verbatim | 140 | 0 | 0 | 0 | 140 | 0 | 168 / 28 / 168 |
| GIS Copilot | arXiv paper (2411.03205) appendix tables; plugin code GPL-3.0; no separate test-data licence | own_words | 110 | 17 | 48 | 35 | 10 | 34 | 35 / 8 / 10 |
| SpatialQueryQA | none (no LICENSE file in github.com/Autonomous-GIS/Spatial-Text-to-SQL) | own_words | 90 | 44 | 37 | 7 | 2 | 19 | 61 / 15 / 2 |
| GeoJSON Agent | arXiv paper (2509.08863) appendix tables; no code or data released; no data licence stated | own_words | 70 | 13 | 30 | 21 | 6 | 29 | 25 / 6 / 6 |
| GeoQuery (inspired-by) | GeoQuery data: GPL-2.0 (UT Austin ML group page); cases here are original, inspired-by | own_words | 61 | 43 | 15 | 0 | 3 | 19 | 69 / 23 / 4 |
| T-Eval | Apache-2.0 | verbatim | 58 | 0 | 2 | 0 | 56 | 2 | 56 / 28 / 56 |
| GeoAgentBench (Yu et al. 2026) | Apache-2.0 | adapted | 57 | 2 | 16 | 35 | 4 | 22 | 8 / 2 / 4 |
| GeoAnalystBench | Apache-2.0 | adapted | 50 | 3 | 15 | 27 | 5 | 30 | 10 / 2 / 5 |
| GeoAgentBench (pothos-dev) | Apache-2.0 | adapted | 36 | 3 | 16 | 9 | 8 | 7 | 13 / 2 / 8 |
| ShapefileGPT | arXiv paper (2410.12376); no code or data released; no data licence stated | own_words | 25 | 1 | 11 | 13 | 0 | 9 | 2 / 1 / 0 |
| GeoLLM-Engine | arXiv paper; no dataset released | own_words | 11 | 1 | 0 | 0 | 10 | 1 | 14 / 3 / 12 |
| **All** |  |  | 2275 | 172 | 322 | 369 | 1412 | 249 | 1694 / 373 / 1443 |

### Overall shares (419 irrelevant or general-knowledge items excluded throughout)

Each cell is micro (pooled) / macro (mean of benchmarks). *After wrapping* = now + upstream.

| Pool | Now | After wrapping upstream | Needs new algorithm | Never (refuse) |
|---|---:|---:|---:|---:|
| All GIS tasks (1856 tasks, 16 benchmarks) | 9% / 13% | 27% / 35% | 20% / 21% | 54% / 44% |
| GIS-analysis benchmarks only (1190 tasks, 10 benchmarks) | 14% / 19% | 40% / 52% | 31% / 33% | 29% / 14% |
| GIS-analysis tasks LayeredMap should do (out of scope removed) (843 tasks, 10 benchmarks) | 19% / 21% | 56% / 59% | 44% / 41% | 0% / 0% |

In the GIS-analysis benchmarks, counting only high-confidence `now` calls, the micro share answerable now is 8%.

### Missing operations, ranked

Ranked by the number of benchmarks with at least one task needing the op, then by tasks. A task needing several ops counts for each. *Macro share*: the mean, over the 10 GIS-analysis benchmarks, of the share of each one's tasks needing the op.

| # | Op | What | Family | Wrap (RustGIS) or new | Benchmarks | Tasks | Macro share |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | `calculate_geometry` | area or length of every feature as a field | field_calc | `analysis::tools::calculate_geometry` | 9 | 46 | 7% |
| 2 | `calculate_field` | new field from an expression (density, per capita, ratios) | field_calc | `engine cmd edit.calculateField` | 9 | 45 | 6% |
| 3 | `summary_statistics` | mean, sum, min, max of a field over a layer, optionally per category | statistics | `analysis::tools::summary_statistics` | 9 | 36 | 3% |
| 4 | `nearest` | nearest feature and its distance (within() finds features within a radius of one point; a nearest search is new) | proximity | `analysis::network::within` | 7 | 60 | 5% |
| 5 | `reproject` | transform to another coordinate system | projection | `rustgis_data::raster::Crs` | 7 | 22 | 4% |
| 6 | `clip` | keep the parts of features inside polygons | overlay | `analysis::tools::clip` | 6 | 25 | 2% |
| 7 | `centroid` | a point inside each feature | generalization | `analysis::tools::feature_to_point` | 6 | 18 | 2% |
| 8 | `convex_hull` | convex hull / minimum bounding geometry | generalization | `analysis::tools::convex_hull` | 6 | 8 | 2% |
| 9 | `point_clustering` | DBSCAN / k-means of point locations | clustering | new | 5 | 30 | 2% |
| 10 | `band_math` | raster calculator, NDVI and other indices | raster_algebra | `analysis::rasterops::band_math` | 5 | 26 | 3% |
| 11 | `raster_to_vector` | polygons from raster cells | raster_algebra | new | 5 | 17 | 2% |
| 12 | `spatial_join` | copy attributes of the containing/nearest feature onto every feature | spatial_join | new | 5 | 9 | 1% |
| 13 | `table_join` | join a table to features by a shared code (rules.toml match_by_code has no algorithm yet) | data_join | new | 4 | 74 | 4% |
| 14 | `kernel_density` | kernel density surface / heat map | density | new | 4 | 43 | 2% |
| 15 | `select_by_location` | select every feature intersecting or near another layer, then keep working with that selection | spatial_join | `analysis::tools::select_by_location` | 4 | 39 | 3% |
| 16 | `frequency` | count of features per category value | statistics | `analysis::tools::summary_statistics` | 4 | 30 | 1% |
| 17 | `slope` | slope from a DEM | terrain | `analysis::rasterops::slope` | 4 | 28 | 1% |
| 18 | `thiessen` | Thiessen / Voronoi polygons | interpolation | new | 4 | 27 | 3% |
| 19 | `erase` | remove the parts of polygons inside other polygons | overlay | `analysis::tools::erase` | 4 | 27 | 1% |
| 20 | `distance_matrix` | distances between every pair | proximity | new | 4 | 24 | 1% |
| 21 | `aspect` | aspect from a DEM | terrain | `analysis::rasterops::aspect` | 4 | 24 | 1% |
| 22 | `intersect` | intersection keeping both layers' attributes (clip gives geometry only; attribute merge is new) | overlay | `analysis::tools::clip` | 4 | 22 | 2% |
| 23 | `buffer_polygons` | buffer around area features (the certified buffer takes points and lines) | buffer | `analysis::tools::buffer` | 4 | 15 | 1% |
| 24 | `edit_attributes` | change or coerce attribute values | data_cleaning | `engine cmd edit.setValue` | 4 | 7 | 2% |
| 25 | `xy_to_points` | table with coordinates to points (rules.toml map_coordinates) | data_join | `analysis::tools::xy_table_to_point` | 4 | 7 | 1% |
| 26 | `regression` | regression / GWR between fields | modelling | new | 4 | 7 | 1% |
| 27 | `distance_between` | geodesic distance between two features | measurement | `lm_analysis::geodesic (rules.toml measure_from_here, not connected)` | 4 | 7 | 0.2% |
| 28 | `polygon_in_polygon_summary` | area-weighted summary of one polygon layer in another | spatial_join | new | 4 | 5 | 1% |
| 29 | `kriging` | kriging from points | interpolation | new | 3 | 23 | 1% |
| 30 | `export_format` | write another file format | format_io | `engine cmd data.convert` | 3 | 18 | 3% |
| 31 | `reclassify` | map value ranges to classes | raster_algebra | `analysis::rasterops::remap` | 3 | 16 | 3% |
| 32 | `resample` | resample or align rasters | raster_algebra | `analysis::rasterops::resample_to` | 3 | 14 | 1% |
| 33 | `polygon_to_line` | area boundaries as lines (e.g. 'near the border') | generalization | new | 3 | 12 | 2% |
| 34 | `contours` | contour lines from a raster | terrain | new | 3 | 11 | 1% |
| 35 | `focal_statistics` | moving-window (neighbourhood) statistics of a raster | raster_algebra | new | 3 | 9 | 1% |
| 36 | `graduated_symbols` | size points by a value, bivariate or multi-layer styled maps beyond color_by | map_making | new | 3 | 8 | 1% |
| 37 | `tessellation` | hexagon, square or quadtree grids over an area | generalization | new | 3 | 6 | 1% |
| 38 | `temporal_change` | compare two dates or a time series | temporal | new | 3 | 6 | 1% |
| 39 | `distance_raster` | Euclidean distance surface | raster_algebra | new | 3 | 6 | 1% |
| 40 | `dissolve` | merge polygons sharing a value | generalization | `analysis::tools::dissolve` | 3 | 5 | 1% |
| 41 | `rasterize` | polygons or points to a raster | raster_algebra | new | 3 | 5 | 1% |
| 42 | `raster_threshold_area` | area or polygons where a raster meets a condition (remap then vectorise; vectorising is new) | raster_algebra | `analysis::rasterops::remap` | 3 | 5 | 1% |
| 43 | `line_in_polygon_summary` | length of lines in each polygon | spatial_join | new | 3 | 5 | 1% |
| 44 | `shortest_route` | shortest or cheapest route over a line layer | network | `analysis::network::shortest_route` | 3 | 4 | 0.4% |
| 45 | `optimisation` | location-allocation, site selection optimisation | modelling | new | 3 | 3 | 0.2% |
| 46 | `map_export` | export or lay out a map image with legend | map_making | `rustgis_render (PNG export)` | 2 | 26 | 1% |
| 47 | `clip_raster` | clip a raster to polygons | raster_algebra | new | 2 | 26 | 1% |
| 48 | `union_overlay` | polygon union overlay of two layers | overlay | new | 2 | 17 | 0.4% |
| 49 | `weighted_overlay` | suitability from weighted criteria | raster_algebra | `analysis::rasterops::weighted_overlay` | 2 | 10 | 2% |
| 50 | `hydrology` | flow direction, accumulation, watersheds (the optional GeoLibre/Whitebox suite has these, not RustGIS) | terrain | new | 2 | 10 | 2% |
| 51 | `hot_spots` | Getis-Ord Gi*, Moran's I, local clusters | density | new | 2 | 9 | 2% |
| 52 | `mosaic` | merge rasters into one | raster_algebra | new | 2 | 9 | 0.3% |
| 53 | `fix_geometry` | repair invalid geometries | data_cleaning | new | 2 | 6 | 1% |
| 54 | `netcdf_multidimensional` | multidimensional (time/depth) rasters such as NetCDF cubes | temporal | new | 2 | 4 | 1% |
| 55 | `symmetric_difference` | symmetric difference of two layers | overlay | new | 2 | 3 | 0.5% |
| 56 | `similar_features` | features with similar attributes | clustering | `analysis::network::similar` | 2 | 2 | 0.4% |
| 57 | `idw` | inverse distance weighting from points | interpolation | `analysis::rasterops::idw` | 2 | 2 | 0.3% |
| 58 | `geometry_conversion` | vertices to points, lines to polygons, points to lines, split polygons by lines | generalization | new | 1 | 6 | 2% |
| 59 | `od_matrix` | origin–destination travel costs | network | new | 1 | 3 | 1% |
| 60 | `simulation` | simulation or forecasting | modelling | new | 1 | 2 | 0.4% |
| 61 | `viewshed` | visible area from an observer | terrain | `analysis::rasterops::viewshed` | 1 | 2 | 0.2% |
| 62 | `hillshade` | hillshade from a DEM | terrain | `analysis::rasterops::hillshade` | 1 | 2 | 0.2% |
| 63 | `topology_check` | overlaps, gaps, self-intersections, dangles (topology.rs) | data_cleaning | `analysis::topology::overlaps` | 1 | 1 | 0.4% |
| 64 | `multi_ring_buffer` | several distances at once | buffer | new | 1 | 1 | 0.4% |
| 65 | `simplify` | Douglas–Peucker simplification | generalization | `analysis::tools::simplify` | 1 | 1 | 0.3% |
| 66 | `explode` | multipart to single part | generalization | `analysis::topology::explode` | 1 | 1 | 0.3% |
| 67 | `deduplicate` | remove duplicate features | data_cleaning | new | 1 | 1 | 0.3% |
| 68 | `service_area` | area reachable within a travel time or network distance | network | new | 1 | 1 | 0.2% |
| 69 | `line_of_sight` | is B visible from A | terrain | `analysis::rasterops::line_of_sight` | 1 | 1 | 0.2% |
| 70 | `random_points` | random points inside polygons | clustering | `analysis::network::random_points` | 1 | 1 | 0.1% |

### Tasks by operation family

| Family | Tasks | now | upstream | new | out of scope |
|---|---:|---:|---:|---:|---:|
| irrelevant | 384 | 0 | 0 | 0 | 384 |
| data_acquisition | 268 | 0 | 0 | 0 | 268 |
| network | 197 | 0 | 3 | 5 | 189 |
| imagery | 152 | 0 | 0 | 0 | 152 |
| proximity | 147 | 0 | 41 | 23 | 83 |
| temporal | 119 | 0 | 0 | 6 | 113 |
| map_making | 118 | 10 | 13 | 56 | 39 |
| spatial_join | 102 | 44 | 15 | 21 | 22 |
| geocoding | 83 | 0 | 0 | 0 | 83 |
| raster_algebra | 80 | 0 | 30 | 50 | 0 |
| overlay | 71 | 0 | 51 | 20 | 0 |
| density | 68 | 0 | 0 | 41 | 27 |
| terrain | 58 | 0 | 26 | 32 | 0 |
| attribute_query | 50 | 48 | 2 | 0 | 0 |
| interpolation | 50 | 0 | 2 | 48 | 0 |
| statistics | 44 | 14 | 29 | 1 | 0 |
| knowledge | 35 | 0 | 0 | 0 | 35 |
| buffer | 33 | 19 | 12 | 2 | 0 |
| field_calc | 32 | 0 | 29 | 3 | 0 |
| generalization | 29 | 0 | 15 | 14 | 0 |
| measurement | 26 | 9 | 17 | 0 | 0 |
| clustering | 26 | 0 | 1 | 25 | 0 |
| modelling | 20 | 0 | 0 | 8 | 12 |
| zonal_stats | 19 | 14 | 4 | 1 | 0 |
| raster_sampling | 16 | 12 | 4 | 0 | 0 |
| format_io | 15 | 0 | 9 | 3 | 3 |
| data_cleaning | 14 | 0 | 7 | 5 | 2 |
| projection | 11 | 0 | 10 | 1 | 0 |
| data_join | 6 | 0 | 2 | 4 | 0 |
| vague | 2 | 2 | 0 | 0 | 0 |

### Why tasks are out of scope

| Reason | Tasks |
|---|---:|
| irrelevant | 383 |
| internet_data | 377 |
| live_routing | 186 |
| geocoding_service | 166 |
| imagery | 152 |
| missing_data | 93 |
| knowledge | 29 |
| code | 15 |
| prediction | 11 |

### Benchmark families and gate checks

| Family | Gate 4 | Gate 7 | Suite cases |
|---|---:|---:|---:|
| spatial_join | Gate 4 numeric suite N1–N6 (counts, which area, what's inside) and hallway questions | Gate 7 answerable | 65 |
| buffer | Gate 4 numeric suite (buffer areas), stress test 10,000-point buffer, '500 m → 1 km' change | Gate 7 answerable | 28 |
| zonal_stats | Gate 4 numeric suite N4–N7 (zonal summaries, coverage), 'average surface temperature in my district' | Gate 7 answerable | 22 |
| raster_sampling | Gate 4 numeric suite (add raster values) | Gate 7 answerable | 17 |
| measurement | Gate 4 numeric suite (geodesic area/length) | Gate 7 answerable | 11 |
| statistics | Gate 4 relevance and ranking snapshots ('most schools per km²') | Gate 7 answerable | 19 |
| attribute_query | — | Gate 7 answerable (filters) | 73 |
| map_making | — | Gate 7 answerable (display calls) | 14 |
| vague | — | Gate 7 question budget (ask_user) | 2 |
| any family, out of scope | — | Gate 7 honest refusal (≥95%) | 331 |

<!-- generated:end -->

## Benchmarks included, and what was left out

| Benchmark | Version used | What is in git |
|---|---|---|
| GeoBenchX | Solirinai/GeoBenchX @ 0edf610 (2026-08-29), MIT | Task text verbatim or adapted to our contexts |
| GeoAnalystBench | GeoDS/GeoAnalystBench @ b5d8c40 (2026-02-11), Apache-2.0 | Adapted requests |
| GeoAgentBench (pothos-dev) | pothos-dev/geoagentbench @ 1cd6f83 (2026-06-18), Apache-2.0 with NOTICE | Adapted requests |
| GeoAgentBench / GABench (Yu et al., arXiv 2604.13888) | geox-lab/GABench @ e8c64e8 (2026-09-03), Apache-2.0 | Adapted requests; benchmark.csv has 57 rows (paper: 53) |
| GTChain-Eval | AGI-GIS/GTChain @ c77d8e0 (2024-12-11) | Licence unclear (MIT LICENSE file vs README "non-commercial use only"): our own summaries and requests only |
| UnivEARTH | HF AaronKao/UnivEARTH @ 1bbeab5 (2025-04-23), MIT | Questions verbatim (140); the 408-question ACL-2026 version is noted, not mapped |
| GeoLLM-Engine | arXiv 2404.15500 (no data released) | Our own words, 11 templates |
| ShapefileGPT | arXiv 2410.12376v2 (no data released) | Our own words, 25 of 42 tasks |
| GeoJSON Agent | arXiv 2509.08863v2 (no data released) | Our own words, 70 tasks |
| GIS Copilot | arXiv 2411.03205 appendix (plugin GPL-3.0; test-case repo empty) | Our own words, 110 tasks |
| GeoQuery | UT Austin, GPL-2.0 | 61 original inspired-by questions; nothing copied |
| SpatialQueryQA | Autonomous-GIS/Spatial-Text-to-SQL @ 171dbce (2026-08-01), no licence | Our own words, 90 questions |
| BFCL v4 irrelevance | gorilla @ 6ea5797 (2026-03-23), Apache-2.0 | 240 irrelevance and 72 geographic live-irrelevance questions, verbatim; defects noted |
| ACEBench | ACEBench/ACEBench @ 56dd66c (2025-10-29), MIT | Geographic items adapted to our contexts; refusal items verbatim |
| T-Eval | open-compass/T-Eval @ 58f2240, HF @ af355ab, Apache-2.0 | 29 + 29 map-chain plans, verbatim |
| MapEval-API | MapEval/MapEval-API @ 35d481a (2024-11-26), no licence | Our own summaries and 73 original requests |

Not included:
- **GeoITC** (GeoTP's training set): not public.
- **The extended UnivEARTH set:** same kind of question as the 140 mapped.
- **MapEval-Textual** (HF, Apache-2.0): the same 300 questions as MapEval-API. We could use its
  questions verbatim; for now MapEval stays own-words.
- **AutoGEEval:** Earth Engine code, no licence.
- **StepGame and SpartQA:** text-only spatial puzzles, no map data.
- **GeoSQL-Eval:** no data.
- **GeoCode-Bench:** a computer-vision coding benchmark, not GIS.
- **GeoQA:** geometry maths, not GIS.

Licences and attribution: `tools/lm/benchmarks/LICENSES.md`. Raw downloads with provenance
READMEs: `/Volumes/LaCie/data/layeredmap-benchmarks/` (outside git).

## Reproducing

```sh
PY=~/miniforge3/envs/lm-llm-eval/bin/python
$PY -I tools/lm/benchmarks/convert/geobenchx.py        # regenerate one source (needs the raw data for permissive sets)
$PY -I tools/lm/benchmarks/validate.py                 # every source and the suite
$PY -I tools/lm/benchmarks/build_suite.py [--full]     # suite.jsonl
$PY -I tools/lm/benchmarks/coverage.py                 # the tables above
$PY -I -m pytest tools/lm/benchmarks/test_benchmarks.py -q
$PY -I tools/lm/benchmarks/run_suite.py --dry-run --out /tmp/bench   # mocked run through run_eval.py
```

Converters for unlicensed or paper-only sources hold their tables in the script and need no
download. The converters for GeoBenchX, GeoAnalystBench, pothos GeoAgentBench, GABench,
UnivEARTH, BFCL, ACEBench, T-Eval, GTChain and MapEval-API read the raw data from the folder
above.
