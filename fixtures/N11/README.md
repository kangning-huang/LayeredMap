# N11 — Measurements and label points (A4)

`areas.geojson` (10 areas), `lines.geojson` (5 lines) and `points.geojson` (3 points and a
multipoint), keyed by `feature_id`.

What it exercises: a C shape and a comb (whose centroids lie outside them), a clockwise exterior
with a hole, a thin frame that is almost all hole, a multipolygon whose largest part is not the
first, a polygon split at the antimeridian, one near the pole, a 30° square, a 110 m sliver; lines
at the equator, at 80°N, split at 180°, in two parts, and 90° of longitude long; points including
one beside 180° and a multipoint.

Expected (`reference/measure_features.py`): `expected_measure.csv`, columns `feature_id`,
`area_km2`, `perimeter_km` (areas), `length_km` (lines), `lon`, `lat` (points: the input
coordinates; a multipoint its first point). Areas and perimeters come from pyproj
`Geod.geometry_area_perimeter` after orienting every polygon counter-clockwise (holes clockwise),
lengths from `Geod.geometry_length`. The perimeter counts holes' rings too (pyproj's own
perimeter leaves them out, so the reference adds each hole's ring perimeter). M_AM's perimeter is left blank: the RFC 7946 split adds a
seam along 180° that LayeredMap leaves out (see `crate::geodesic`). Tolerance (mechanics.md ›
A4): 0.01% for areas and lengths, exact for point coordinates; label points of lines and areas
are not in the CSV, the Rust tests assert that each lies on its line or strictly inside its area.

Made by `reference/make_vector_ops.py` (synthetic, CC0-1.0).
