# N2 — Count rules (holes and multipolygons)

`areas.geojson` (3 areas, property `area_id`) and `points.geojson`
(12 points, `point_id`). All area vertices and every boundary point
are exact binary fractions (multiples of 1/32°), so "on the boundary" is exact
in floating point.

H is a polygon with a square hole; I is an island inside the hole (not touching it); M is a MultiPolygon of two squares with a gap. Q03, Q12 are in the hole (count nowhere), Q04 in the island, Q05 on the hole's ring (counts in H), Q06 on I's edge, Q09 in M's gap (nowhere), Q10 on M's edge, Q11 on H's outer edge.

Rule (see `reference/count_in_areas.py`): GeoPandas `sjoin(predicate="intersects")`,
so boundary points count in every touching area.

Expected: `expected_count_in_areas.csv`.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
