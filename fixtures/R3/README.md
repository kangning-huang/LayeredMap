# R3 — Land-cover categories (synthetic WorldCover stand-in)

`landcover.tif`: 200×200 uint8 GeoTIFF, EPSG:4326, bounds 121.2–121.8°E,
30.9–31.5°N (V1's extent), pixel 0.003°, NoData tag = 0 (no NoData pixels).
Values use the **ESA WorldCover class codes** (10 tree … 80 water … 100 moss)
but the map is **synthetic**: a nearest-seed (Voronoi) pattern from 60 seeds
(seed 33). It is NOT WorldCover data and nothing was downloaded.

Tests: the app must treat values as categories (shares by area), not average
them. Expected shares per V1 district are in `../N8/`.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
