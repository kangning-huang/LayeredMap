# V10 — Geometry edge cases

`edge_cases.geojson`: one feature per case (property `case`):

| case | what |
|---|---|
| `control` | Ordinary valid 0.1° square near Shanghai |
| `antimeridian_polygon` | 2°×1° box crossing 180° encoded unsplit (violates RFC 7946 §3.1.9) |
| `antimeridian_line` | 1° line along the equator crossing 180°, unsplit |
| `polar_polygon` | Cap north of 85°N, encoded with explicit pole vertices |
| `zero_area_polygon` | Collinear ring along a meridian (a geodesic): zero area |
| `self_intersecting_polygon` | Bow-tie ring |
| `null_geometry` | geometry: null |
| `empty_polygon` | Polygon with no rings |
| `empty_collection` | Empty GeometryCollection |

Tests: correctness and **honest errors** — nothing may crash, invalid/empty
geometries must be reported (not silently dropped), antimeridian and polar
shapes must not be measured as if they wrapped the globe.

Expected per-case validity, reasons and geodesic measures are in
`expected.json` (pyproj.Geod on WGS 84).

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
