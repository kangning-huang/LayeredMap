# Puzzle mechanics and the action gaps they expose

This note looks at how clue-based "where is it?" geography puzzles work, so that LayeredMap Pro can
solve original puzzles of the same kind (`puzzles.md`) with certified actions. It reads ten Esri
"ArcGIS Puzzles" StoryMaps for their spatial reasoning only. Their stories, characters, wording and
answers are Esri's and are not reproduced here or anywhere in the repository; each is referred to
by title, and what follows is our own description of the operations a solver needs.

## 1. Reasoning chains in the ten reference puzzles

Every puzzle intersects several independent constraints until one or two places are left. The
constraints fall into five families: raster thresholds, proximity, vector attributes, coordinate
arithmetic and island geometry.

| Puzzle (title only) | Data layers | Reasoning chain (operations in order) |
|---|---|---|
| Returning Raster | Holocene volcanoes; elevation; monthly and annual temperature; annual precipitation; land cover | Keep a narrow band around the equator (latitude range) → keep cells above a high elevation threshold → keep cells whose annual mean is near a target and whose seasonal range is small (a range test on monthly means, or a seasonality index) → keep cells above a precipitation floor → keep cells near a volcano (point proximity) → keep cells at the border between two land-cover classes (class adjacency) → expect at most two places, read off as coordinates. |
| Pardoning Pickles | Continent outline; volcanoes or volcanic soils; elevation; annual temperature; precipitation; land cover | Restrict to one continent (vector mask) → keep cells near volcanic features (proximity) → elevation floor → annual mean near a target (two-sided range) → precipitation floor → forest land-cover class → snap the answer to the nearest whole-degree grid intersection. |
| Bigfoot's Brother | Home city; lakes with names; elevation; population density; faults with type attributes; country borders | Ring of distances from a home city (outer buffer minus inner buffer) → lakes whose name follows a word-order pattern (name ends with a word rather than starts with it) → low ground at the lake but high ground in its neighbourhood (local elevation versus neighbourhood maximum) → low population density (density raster threshold) → near faults of particular types (attribute filter, then proximity) → may involve another country (country overlay) → latitude plus longitude equals a given number (derived coordinate field). Open-ended answer. |
| Magic Dan's Extreme Sea & Ski Resort | Populated places with population and names; coastline; land cover; elevation; precipitation; January and July temperature; major rivers; faults | Cities whose name ends in given letters (suffix filter) → coastal (coast proximity) with a population near a target (range) → land-cover mix (class shares around the site) → elevation ceiling → precipitation band (two-sided) → January floor and July ceiling (two monthly rasters) → a major river within a box of a few degrees (coordinate-window proximity) → at least a minimum distance from any fault (inverse proximity: "away from"). |
| The Left Handed Chocolate Caper | Countries with population; terrain; faults with type; precipitation; January and July temperature; cities with population and names; large historical earthquakes; coastline | Country population above a threshold (country attribute) → steep relief (slope or relief) → at least two fault types nearby (distinct attribute values within a distance) → dry (precipitation ceiling) → January mild, July cold (implies the southern hemisphere) → a nearby city whose name has a given prefix and population below a threshold → that city near a large historical earthquake (point proximity) → coastal. |
| The Lost City of Franklin Shaw | Latitude lines; elevation or distance; cities with names; population density; land cover; earthquakes, volcanoes and faults; rivers; precipitation; January temperature | Partly erased clues are reconstructed by testing alternatives: a latitude band → a distance or elevation figure → a nearby city with a name prefix → very low population density → edge of a grassland class → far from quakes and volcanoes but surrounded by small faults (inverse proximity plus fault density) → a given distance from water → very low precipitation → a cold January → name the modern city nearby. |
| Wandering Juan's Treasure | Volcanoes with heights; coastline and bays; elevation profile; peaks; rivers; precipitation; land cover | A volcano above a height (point attribute) → close to a bay (coast proximity, a day's walk) → land rising by a set amount between coast and summit (elevation profile) → the region's highest peak lies due south (ranking plus bearing) → a large river a few days' sail further south (bearing and distance) → moderate rainfall → many vegetation types nearby (land-cover diversity in a neighbourhood). Open-ended answer. |
| Clark Louis' Bad Adventure | A coordinate box; rivers with flow direction; cities; faults; elevation; terrain roughness; protected forests | Start in a one-degree coordinate box → follow a river downstream for a distance (network tracing) → no large city passed on the way (proximity along a path) → the river crosses at least two faults (line intersection count) → high, rugged terrain nearby (elevation and slope) → next to a national forest (polygon proximity). Several answers accepted. |
| Finding the Island of Misfit Toys | Arctic Circle; land and ice sheets; islands; populated places; countries and languages | Inside the Arctic Circle (latitude floor) → land, not permanent ice (land-cover or ice mask) → a longitude sector suggested by a journey's direction → at least a minimum distance to any other land (island isolation) → uninhabited (no populated places, low human footprint) → at least a minimum area → in a country with given languages (country attribute). Many answers accepted. |
| The Case of the Missing Ship | Land; elevation; lakes; rivers and their downstream cities; annual temperature; land cover; faults; earthquakes and volcanoes; national borders; country GDP and population; prevailing winds | On land, low elevation nearby → far from large lakes (inverse proximity) → near a large river that flows to a city whose name starts with given letters (name prefix plus downstream relation) → hot annual mean (range) → tropical forest class → one isolated small fault nearby but no fault network (fault density) → far from quakes and volcanoes → on a national border between countries of very different GDP per person (border proximity plus a ratio of country attributes) → expect at most two sites. |

### What the chains have in common

1. **Raster conditions combine into areas.** Most puzzles apply two to five raster tests (elevation,
   annual or monthly temperature, precipitation, land cover) and want the cells where all hold, as
   an area the user can see and measure.
2. **Proximity, in both directions.** Near a volcano, fault, river, coast, border or earthquake; or
   at least some distance away. Sometimes a ring (between two distances).
3. **Attribute tests on vector features.** Population ranges, country attributes (population, GDP,
   continent, language), feature types, and name patterns (prefix, suffix, word order).
4. **Derived values.** Seasonal range (July minus January), latitude plus longitude, a GDP ratio:
   arithmetic on fields or rasters, then a test.
5. **Geometry of the features themselves.** Island area, distance to the nearest other land, and
   the size of a connected area.
6. **A final pick.** Rank and take the top one, or "the city inside the area", or coordinates
   snapped to a grid.

## 2. Operation → LayeredMap today → gap

"Today" means the Phase 4 certified actions (`crates/lm-actions/rules.toml`, `crates/lm-jobs/src/actions.rs`,
`docs/lm/spec.md` › Phase 4 › WP4.5) and the display filter (`crates/lm-core/src/filter.rs`). The
upstream RustGIS crates in the fork already implement much of the rest; the "Upstream to wrap"
column names the function (signatures checked in this tree) and what certifying it would take.
Upstream geometry tools work on longitude/latitude coordinates: `buffer` and
`selectByLocation` buffer in Web Mercator units scaled by 1/cos(latitude of the feature's label
point), `calculateGeometry` and `network::within` use a sphere (haversine), and the overlay tools
use the `geo` crate's planar boolean operations on longitude/latitude.

| # | Operation | LayeredMap today | Upstream to wrap (crates/analysis unless noted) | Gap and what certifying needs |
|---|---|---|---|---|
| O1 | Attribute test on a number or category (population over X, country is Y) | **Yes.** `filter` (display filter: Is, IsNot, Contains, Above, Below, Between, AnyOf, dates; all conditions must hold) | `selectByAttributes` → `rustgis_data::query::select` (SQL WHERE) | None. |
| O2 | Name starts with / ends with | **No.** `filter.rs` has `Contains` only | `rustgis_data::query::like(text, pat)`: SQL LIKE with `%` and `_`, case-insensitive | Add `StartsWith` and `EndsWith` tests to `filter::Test` (reuse `like` semantics with `pat%` / `%pat`). Certify: exact against Python `str.lower().startswith()` after trimming and NFC normalisation, including names with diacritics (Zürich, Ürümqi) and CJK names (`NAME_ZH`). |
| O3 | Raster value at each point | **Yes.** `add_raster_values` (`zonal::sample`, the cell containing the point, exact vs rasterio) | (`rasterops::elevation_at` is bilinear; not used) | None. |
| O4 | Raster summary for each area (rain on each island) | **Yes.** `summarize_each` (fractional coverage, ellipsoidal cell areas, exact vs exactextract) | `rasterops::zonal_means` (cell centres only: not certifiable for small islands) | None; keep LayeredMap's own. |
| O5 | One raster above or below a value, shown | **Display only.** `threshold` card ("Show only values above…"); no layer, no area | `rasterops::remap(r, b, ranges, keep)` | Covered by A1 below. |
| O6 | Several raster conditions → areas | **No** | `rasterops::resample_to(src, b, target)` (nearest neighbour onto one grid) → `remap` per raster to 1 or `NODATA` → `band_math(r, expr)` (`NODATA` in any band gives `NODATA`, so a product of the 1/NODATA masks is the AND). **Missing upstream:** a raster-to-polygon step and connected-region labelling (only in the optional `crates/geolibre` runner, Whitebox `RasterToVectorPolygons` and `Clump`, native-only and file-staged, so not usable as a certified built-in). `band_math` works on the bands of one raster, so the inputs must first be stacked on one grid, a step upstream lacks. | New action A1. Certify cell membership exactly (inclusive bounds, NoData is never inside), polygons tracing cell edges exactly, areas from `zonal::lat_band_area_m2`. |
| O7 | Derived raster or field (July minus January; latitude plus longitude; a ratio) | **No** | `band_math` (raster bands only, + − × ÷); no field calculator upstream | New action A5 for fields. For rasters, A1 accepts a difference of two rasters as a condition. |
| O8 | Area within a distance of features | **Yes.** `buffer` (geodesic; certified) | `buffer` (Web Mercator with per-feature latitude scale: approximate for features spanning latitude) | None; keep LayeredMap's. |
| O9 | Features near / away from other features, layer-wide (cities within 50 km of a fault; at least 300 km from the sea) | **Partly.** `buffer` (merged) then `whats_inside` gives "near", but buffering 13,696 fault traces geodesically is slow and "away from" needs an erase | `selectByLocation` (search distance, `invert` = away from): distances are Web Mercator buffers scaled at each selector's label-point latitude, so a long fault or coastline that spans latitudes gets a wrong radius away from its label point; `network::within(ds, rows, at, radius_m)`: per point, spherical with one scale factor | New action A3 (a distance field, then `filter`). Certify against geodesic reference distances (below); the upstream Web Mercator scaling cannot be certified as is. |
| O10 | Distance to the nearest other feature of the same layer (island isolation) | **No** | None (A3 with "other features of the same layer") | A3, same-layer variant; a multipart feature's own parts are not "other". |
| O11 | Area of every feature, coordinates of every feature | **Card only.** `measure_area` for one area; `map_coordinates` builds points from columns | `calculateGeometry`: area_km2 and perimeter (spherical, rounded to 0.01 km²), point_x/point_y for point layers only (label point) | New action A4. The spherical area differs from WGS 84 by up to about 0.5%, outside the 0.01% tolerance of `measure_area`; use `lm-analysis::geodesic::polygon_area_perimeter` instead. |
| O12 | Split a multipart layer into one feature per part (Natural Earth "land" has 11 features for all islands) | **No** | `topology::explode(g)` | New action A7, a thin wrapper. Certify: part count and coordinates exact against GeoPandas `explode`. |
| O13 | Keep the part of a layer inside another (areas inside China) | **No** | `clip` (`geo` intersection with the union of the clip layer) | New action A2. Certify: per-feature geodesic area within 1e-6 of GeoPandas `overlay(how="intersection")`; check rings crossing the antimeridian and holes. |
| O14 | Remove the part inside another ("away from X" as an area; distance rings) | **No** | `erase` (`geo` difference with the union of the erase layer) | Same action A2, "remove" variant; same certification with `how="difference"`. |
| O15 | Copy attributes of the containing area to points (continent, country population) | **Card only.** `which_container` for one point | None (`selectByLocation` selects, `countPoints` counts) | New action A6. Certify exactly against GeoPandas `sjoin(predicate="intersects")`. |
| O16 | Count distinct features within a distance (three countries within 30 km) | **Per point only.** `buffer` one point then `whats_inside` lists the boundary lines it touches | `selectByLocation` with distance (approximate, as above) | No new action proposed: a plan can use `buffer` + `whats_inside` on the few points left by earlier clues. Candidate for later: a "Count [Lines] near each [School]" variant of `count_in`. |
| O17 | Rank and pick the top | **Yes.** `rank_by_value` | `summaryStatistics` | None. |
| O18 | Size of a connected area | **No** | — | Part of A1's output (each region carries its area). |
| O19 | Relief, slope, neighbourhood maximum | **No** | `rasterops::slope`, `aspect`, `hillshade`; no focal maximum | Not needed by our eight puzzles; slope could be certified against `numpy.gradient` with Horn's method later. |
| O20 | River network tracing downstream | **No** | `network::shortest_route` is for routes, not flow | Out of scope. |
| O21 | Land-cover class shares, class edges | **Shares yes** (`summarize_each` on a category raster gives shares, fixture N8); edges no | `remap` | Edges out of scope. |

## 3. Proposed new certified actions

Seven small actions plus two filter tests cover every operation the eight puzzles in `puzzles.md`
need. Each wraps an upstream function where one exists. Each follows the WP4.5 pattern: a written
definition, a reference implementation for tests (Python, `reference/`), a tolerance, and a
plain-English label in the style of `rules.toml` (tokens such as `[layer]`, `[Heat]`, `[Schools]`,
`[District]`). All are `surface = "assistant"`, `min_phase = 4` (so Pro can call them) unless noted.
None is implemented here.

### A1 `find_where`: Find areas where [Heat] meets conditions

- **Label:** "Find areas where [Heat] meets conditions"; aliases: raster calculator, map algebra,
  suitability, reclassify, raster to polygon, multi-criteria.
- **Inputs:** one or more raster layers (`raster_layer`), each with a condition: at least a value,
  at most a value, between two values (inclusive), or one of several category codes. Optional:
  a condition on the difference of two rasters ("July at least 1 °C cooler than January").
- **Definition:** all rasters are read on the grid of the first one; another raster's value for a
  cell is the value of its cell containing the first grid's cell centre (nearest neighbour, the
  same rule as `add_raster_values`). A cell is in when every condition holds; NoData in any input
  means out. Cells that touch by an edge or a corner form one area (8-connected). Output: an area
  layer with one feature per connected area, its boundary following cell edges exactly, with
  fields `area_km2` (sum of cell areas on the WGS 84 ellipsoid, `zonal::lat_band_area_m2`),
  `cells`, and the mean of each input raster over the area. The card says how many areas and the
  total area.
- **Upstream to wrap:** `rasterops::resample_to` (grid alignment), `rasterops::remap` (conditions).
  Build in LayeredMap: the labelling and the cell-edge tracer (simple: union of cell squares per
  label), since upstream has no raster-to-polygon step.
- **Reference:** numpy comparisons on arrays read with rasterio; `scipy.ndimage.label` with a 3×3
  structure; `rasterio.features.shapes` for the polygons; areas by the latitude-band formula.
  (`tools/lm/puzzles/solve.py` does exactly this for puzzle P4.)
- **Tolerance:** identical cell set; identical polygon count; each area within 1e-9 relative of
  the latitude-band sum, and within 1e-6 of pyproj `Geod.geometry_area_perimeter` on the traced
  polygon.

### A2 `keep_inside` and `remove_inside`: Keep / remove the part of [layer] inside [other]

- **Labels:** "Keep the part of [layer] inside [other]" (aliases: clip, intersect) and "Remove the
  part of [layer] inside [other]" (aliases: erase, difference, away from).
- **Inputs:** an area, line or point layer; an area layer (all its features, or those the filter
  shows, merged).
- **Definition:** geometry intersection or difference with the union of the other layer, edges
  taken as straight lines in longitude/latitude (the convention the whole app uses for drawing),
  attributes copied. Features that end up empty are dropped and counted on the card.
- **Upstream to wrap:** `clip` and `erase` in `tools.rs` (`geo::BooleanOps`). They already orient
  rings before the union; certification adds the antimeridian and hole cases.
- **Reference:** GeoPandas `overlay(how="intersection" | "difference")`; areas by pyproj Geod.
- **Tolerance:** same feature count; each feature's geodesic area within 1e-6 relative (absolute
  1 m² for slivers); points and lines exact.

### A3 `nearest_distance`: Add the distance to the nearest [Line] to each [School]

- **Labels:** "Add the distance to the nearest [other] to each [School]"; variant on the same layer:
  "Add the distance to the nearest other [School]". Aliases: near, proximity, distance to nearest,
  isolation, how far.
- **Inputs:** the layer to annotate (points, lines or areas); the target layer (points, lines or
  areas, or the same layer).
- **Definition:** for each feature, the shortest geodesic distance on the WGS 84 ellipsoid from
  any point of it to any point of any target feature (0 when they touch or overlap), in km, plus
  the target's name or identifier. On the same layer, the feature itself and its other parts are
  excluded. Distances are computed by densifying target edges to at most 1 km and taking the
  minimum over vertex-to-geodesic-segment distances; the card states the densification.
- **Upstream to wrap:** none certifiable (`selectByLocation` and `network::within` use scaled
  Web Mercator distances). Reuse `lm-analysis::geodesic` and the R-tree already used by
  `overlay`.
- **Reference:** for each feature, an azimuthal equidistant projection centred on it (pyproj),
  targets densified to 1 km, shapely `distance`; for the same-layer variant, exclude the feature's
  own index. (`solve.py` uses a faster vertex-only sphere version, adequate for its margins.)
- **Tolerance:** within 0.1% or 50 m, whichever is larger.
- **Then:** `filter` with Above / Below / Between gives "near", "away from" and rings; `rank_by_value`
  gives "the most isolated".

### A4 `add_measurements`: Add area and coordinates to each [District]

- **Label:** "Add area and coordinates to each [District]" (points: "Add coordinates to each
  [School]"). Aliases: calculate geometry, area field, xy coordinates, centroid.
- **Definition:** fields `area_km2` and `perimeter_km` (geodesic, WGS 84) for areas,
  `length_km` for lines, and `longitude`, `latitude` in degrees for every feature: the point itself,
  or for lines and areas a label point that is guaranteed to lie on the feature (the same
  `label_point` the map uses for labels).
- **Upstream to wrap:** `calculateGeometry`'s field plumbing; replace its spherical
  `geodesic_ring_area_m2` and haversine lengths with `lm-analysis::geodesic` (Karney), and extend
  coordinates from point layers to all layers.
- **Reference:** pyproj `Geod.geometry_area_perimeter` and `Geod.geometry_length`; for coordinates of
  points, the input coordinates.
- **Tolerance:** areas and lengths 0.01% (as `measure_area`); point coordinates exact; label points
  must lie inside their polygon (asserted).

### A5 `calculate_field`: Add a field calculated from [layer]'s values

- **Label:** "Add a field calculated from two values"; aliases: field calculator, difference,
  ratio, compute column.
- **Definition:** a new number field from two number fields (or a field and a constant): a + b,
  a − b, a × b, a ÷ b, or |a| ÷ |b|, with an optional absolute value. A blank input or division by
  zero gives a blank, never 0; the card counts blanks.
- **Upstream to wrap:** none (`band_math`'s expression parser handles raster bands only; it could be
  reused for a later free-form version).
- **Reference:** pandas column arithmetic with `NaN` for blanks.
- **Tolerance:** exact (IEEE-754 double).

### A6 `join_by_location`: Add [District] details to each [School]

- **Label:** "Add [District] details to each [School]"; aliases: spatial join, attributes by
  location, which area.
- **Definition:** for each point (or a line's or area's label point), the fields chosen from the
  area that contains it. A point on a shared boundary takes the area listed first and the card
  counts such points; a point in no area gets blanks and is counted.
- **Upstream to wrap:** none; reuse `lm-analysis::overlay::locate_in_area`, the certified
  point-in-area test behind `which_container`.
- **Reference:** GeoPandas `sjoin(predicate="intersects")`, first match kept.
- **Tolerance:** exact.

### A7 `split_parts`: Split [layer] into separate parts

- **Label:** "Split [layer] into separate parts"; aliases: explode, multipart to singlepart.
- **Definition:** one feature per polygon part (holes stay with their part) or line part, attributes
  copied, plus a `part` index.
- **Upstream to wrap:** `topology::explode`.
- **Reference:** GeoPandas `explode(index_parts=True)`.
- **Tolerance:** exact coordinates and counts.

### Filter tests `starts with` and `ends with` (extension of `filter`, display)

- **Chip words:** "Name starts with Z", "Name ends with shima".
- **Definition:** after trimming, compared case-insensitively with Unicode lowercase on NFC text.
- **Upstream:** `rustgis_data::query::like` already implements case-insensitive LIKE; the filter
  can call it with `pattern%` and `%pattern` (escaping `%` and `_` in the user's text).
- **Reference and tolerance:** Python `str.lower()` comparison; exact.

## 4. Which puzzles need which actions

| Puzzle | Existing actions | New actions |
|---|---|---|
| P1 Kunming | filter, add_raster_values | none |
| P2 Winnipeg | filter, add_raster_values, rank_by_value | A6 (continent), or filter "country is any of" |
| P3 Ulleungdo | filter | A7, A6, A4, A3 |
| P4 Pu'er tea hills | filter, whats_inside | A1, A2, A4 |
| P5 Zaragoza | filter, add_raster_values | A6, starts-with test |
| P6 Nairobi | filter, add_raster_values | A6, A5, A3 |
| P7 Foz do Iguaçu | filter, add_raster_values, buffer, whats_inside | A6, A3 |
| P8 Christmas Island | filter, summarize_each | A7, A4, A3, A5 |

A3 (nearest distance) and A6 (join by location) unlock six of the eight puzzles; A1 is the one
that makes the most visual results (areas appearing as conditions are added), so it matters most
for the promotional videos.
