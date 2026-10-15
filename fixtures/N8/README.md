# N8 — Land-cover category shares per district

Inputs (not duplicated): `../R3/landcover.tif` (synthetic WorldCover-style
categories) × `../V1/districts.geojson` (17 districts, id `district_id`).

Expected: `expected_category_shares.csv` — one row per (district, category
present): `share` of the district's valid covered area, and `area_km2`.
Produced by `reference/category_shares.py`: exactextract `weighted_frac` with a
geodesic cell-area weight raster (pyproj.Geod), i.e. fractional pixel coverage
× cell area. Shares sum to 1 per district. D04 (MultiPolygon) and D06 (polygon
with a hole) exercise multipart / hole handling.

Tests: category shares, not means.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
