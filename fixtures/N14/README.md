# N14 — Nearest feature, against GeoPandas `sjoin_nearest`

Inputs for `lm_analysis::nearest::nearest_distance` (WP6.4, docs/lm/actions-p2.md › Nearest
feature). A 25 km region around Shanghai (121.33–121.60° E, 31.14–31.33° N).

- `sources.geojson` (16 points, `source_id`): S01 inside park P01, S02 exactly on a
  vertex of road R01, S03 in park P02's lagoon (a hole); the rest from a seeded generator, each
  kept only when its nearest target is at least 2% closer than the next one, in every pairing.
- `facilities.geojson` (8 points, `facility_id`).
- `roads.geojson` (4 lines, `road_id`): R03 is a two-part line.
- `parks.geojson` (4 areas, `park_id`): P02 has a hole, P03 two parts, P04 is a triangle.

Expected (from `tools/lm/reference_tests/nearest_sjoin.py`), one CSV per pairing
(sources → facilities, roads, parks; parks → facilities), columns `feature_id`, `nearest_id`,
`distance_m` (geodesic), `projected_m` and `runner_up_ratio`. The nearest target is GeoPandas
`sjoin_nearest` in UTM zone 51N (EPSG:32651). The geodesic distance is pyproj `Geod.inv` (WGS 84)
between the two nearest points that shapely finds in that projection, taken back to lon/lat;
the script asserts it is within 0.1% of the projected distance.

Tolerances: the nearest target exactly; distances within 0.1% or 1 m of `distance_m`, whichever
is larger (docs/lm/actions-p2.md says why that is the right bound for a projected reference);
point to point within 1 mm (Geod.inv is exact there).

Made by `tools/lm/reference_tests/make_p2_fixtures.py` (synthetic, CC0-1.0).
