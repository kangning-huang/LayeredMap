# V1 — Shanghai districts (synthetic)

`districts.geojson`: 17 district areas laid out on a 4×4 grid of 0.15° cells over
lon 121.2–121.8, lat 30.9–31.5, plus one district filling a hole.

- Names (`name_zh`, `name_en`) are real Shanghai district names (including the
  former Zhabei 闸北区), but **geometry and population are synthetic**.
- `D06` Jing'an is a polygon **with a hole**; the hole is exactly `D17` Huangpu.
- `D04` Chongming is a **MultiPolygon**: two parts separated by a 0.01° channel
  (lon 121.72–121.73) that belongs to no district.
- `D08` Zhabei has `population: null` (tests missing-value handling); `D15` and
  `D16` have the same population (tests ties).
- `area_km2` is the geodesic area on WGS 84 (pyproj.Geod), rounded to 0.01 km².

Tests: import, profile/insight (counts, ranks), counts in areas (with T1).

Expected: `expected.json` (profile), `expected_insight.csv` (rank phrases for
`population`, from `reference/rank_check.py`). The V1 × T1 point counts are in
`../T1/expected_count_in_areas.csv`; V1 × R3 category shares in `../N8/`.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
