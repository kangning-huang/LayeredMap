# N10 — Join by location (A6)

`areas.geojson` (10 rows, `area_id`; A10 has no geometry), joined to `points.geojson` (17 points
and a multipoint), `lines.geojson` (5 lines) and `polygons.geojson` (6 areas), all keyed by
`feature_id`. Every boundary coordinate is a binary fraction, so "on the boundary" is exact.

What it exercises:

- **First area in row order.** P01 is in A1 and A9 (A9 lies inside A1 but is listed later): A1.
  P03 is in A2 and A3: A2. P16 lies on A2's top edge and A3's west edge: A2.
- **Boundaries.** P02 on the A1/A2 edge (A1), P07 on A4's hole ring (A4), P10 on A7's 180° seam,
  P13 on A8's edge, P16; P17 is inside A1 and on A9's corner, so it is counted as on a boundary
  although it takes A1.
- **Holes and gaps.** P06 is in A4's hole (outside); P05 is in the hole but inside A6. P08 is
  between A5's parts (outside).
- **Antimeridian and high latitude.** A7 is split at 180° (P10, P11, L04, G05); A8 spans
  80–85°N (P12, P13, L05, G06).
- **Missing geometry.** A10 and P15 have none.
- **Lines and areas by their label point.** Each line and area lies wholly inside one area's
  part (or wholly outside), so any point of it gives the same answer: L01, L02, L04, L05 and
  G01 (a C shape wrapped around A4's hole and A6: a label point in the C's mouth would land in the
  hole or in A6), G02 (a frame with a hole), G03 (largest part in A5, small part outside), G05,
  G06; L03, G07 are outside.

Expected (`reference/join_by_location.py`): `expected_join.csv`, columns `layer`, `feature_id`,
`area_id` (blank when none) and `on_boundary` (points and multipoints only: whether a point lies
on the boundary of any area). Definition: GeoPandas `sjoin(predicate="intersects", how="left")`,
keeping the match with the lowest area row; lines and areas use shapely's `representative_point`.
Tolerance: exact.

Made by `reference/make_vector_ops.py` (synthetic, CC0-1.0).
