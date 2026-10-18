# N12 — Keep or remove the part inside an area layer

Inputs for `lm_analysis::clip::keep_inside` and `remove_inside` (puzzles › mechanics › A2).
Coordinates are offsets from 120° E, 30° N; every vertex is a multiple of 1/16°
except A6 (a slanted triangle) and A7 (a sliver 1e-8° wide).

- `mask.geojson` (`mask_id`): M1 the square [0, 4]² with the hole [1.5, 2.5]²; M2 [3, 6] × [1, 3],
  overlapping M1 and written clockwise; M3 a multipolygon of [8, 9] × [0, 1] and [8, 9] × [2, 3].
- `areas.geojson` (`id`): A1 straddles M1's corner; A2 a holed square over the mask's hole;
  A3 a two-part multipolygon across M3; A4 outside; A5 inside; A6 a slanted triangle across
  M1, M2 and beyond; A7 a sliver over M2's east edge; A8 no geometry; A9 a clockwise square
  inside the mask's hole.
- `lines.geojson` (`id`): L1 across M1 and M3; L2 through the hole; L3 a diagonal crossing
  M1's edge and the hole; L4 outside; L5 a two-part line; L6 a bent line in and out. No line
  runs along a mask edge (that case is tested in Rust).
- `points.geojson` (`id`): inside, in the hole, on the hole's edge, on the outer edge, in the
  overlap, outside, a multipoint with one point in and one out, on M2's east edge, and on
  M1's corner.

Expected (from `reference/clip_erase.py`): `expected_clip.csv`, one row per input feature and
operation (`keep`, `remove`): `kept` (whether a feature is left), `parts`, the geodesic
area (pyproj Geod, WGS 84, polygons oriented counter-clockwise) or length, and for points and
lines the exact coordinates left (`x y` pairs; lines as parts separated by `|`, each part
starting at its smaller end, parts sorted). Reference: GeoPandas `overlay` with
`how="intersection"` or `"difference"` against the mask dissolved into one feature.

Tolerances (mechanics › A2): same features kept; areas within 1e-6 relative (1 m² absolute for
slivers); lines and points exact.

Made by `reference/make_where_clip_fixtures.py` (synthetic, CC0-1.0).
