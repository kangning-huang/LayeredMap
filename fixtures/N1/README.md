# N1 — Count rules (shared boundary, overlap, empty area)

`areas.geojson` (4 areas, property `area_id`) and `points.geojson`
(20 points, `point_id`). All area vertices and every boundary point
are exact binary fractions (multiples of 1/32°), so "on the boundary" is exact
in floating point.

A and B share the edge lon 121.25 (points P04, P05 on it, P06 on the shared top corner); C overlaps B in [121.375, 121.5]×[31.125, 31.25] (P09, P10 inside the overlap; P12 on B's east edge inside C; P15 on B's top edge and C's west edge); D has no points (zero count). P16 lies on A's outer edge, P19 on C's corner; P17, P18 are outside every area.

Note: the plan says "20 points and 3 areas"; three areas cannot simultaneously show a shared boundary with points on it, an overlap with points in it, and a zero-count area, so a fourth area **D** (zero count) was added.

Rule (see `reference/count_in_areas.py`): GeoPandas `sjoin(predicate="intersects")`,
so boundary points count in every touching area.

Expected: `expected_count_in_areas.csv` and `expected_which_area.csv`.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
