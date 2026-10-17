# N9 — Nearest distance and island isolation (A3)

`islands.geojson` (14 areas, `island_id`), `points.geojson` (12 points, `point_id`) and
`lines.geojson` (6 lines, `line_id`). Every coordinate is a binary fraction of a degree.

What it exercises:

- **Same-layer exclusion of multipart features.** I01's two parts are 6.9 km apart; its nearest
  *other* island (I02) is about 34 km away. I13's parts are 300 km apart, at 78–80°N.
- **Touching and overlap give 0.** I02 and I03 share an edge; I04 and I05 overlap; S01 and S08
  lie inside islands, S02 on an edge; S10 lies exactly on L01's straight lon/lat edge; L04
  crosses I02; L05 starts on I07's corner.
- **Holes.** I06 lies in I07's lagoon (a hole), so their distance is to the hole's ring; S03 is
  in the lagoon too.
- **Antimeridian.** I08 is split at 180° (RFC 7946), with I09 and I10 on either side; L02 is a
  split line; S04, S05, S12 sit beside them.
- **High latitude and far away.** I11–I13, L03, S06 near 78–80°N; S09 at 89.5°N; S07 at 0°, 0°
  and I14 in the South Pacific are thousands of km from anything.
- **Kinds.** Points to points (same layer), points to lines, points to areas, lines to areas,
  areas to areas (same layer) and areas to points.

Expected (`reference/nearest_distance.py`): one CSV per pairing, columns `feature_id`,
`nearest_id`, `distance_m`. Definition: 0 when the feature intersects a target (shapely
`intersects` in lon/lat, the app's planar convention); otherwise targets and the feature are
densified along geodesics to steps of at most 1 km (pyproj `Geod`), projected to an azimuthal
equidistant projection centred on the feature (circular mean of its vertices) and measured with
shapely `distance` between their boundaries. Same layer: the feature's own row is left out.
Tolerance (mechanics.md › A3): 0.1% or 50 m, whichever is larger.

Made by `reference/make_vector_ops.py` (synthetic, CC0-1.0).
