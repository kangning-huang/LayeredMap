# R6 — Scale/offset, -9999 edges, no NoData tag

`scaled.tif`: 24×24 int16 GeoTIFF, EPSG:4326, origin (121.375, 31.3125),
pixel 1/256° (all georeferencing values are exact binary fractions so the
boundary sample points are exact). Band has **scale 0.1 and offset
-100.0** (decoded = raw × scale + offset ≈ 0–160) and raw **-9999 in the outer
2-pixel border only**, with **no NoData tag**.

`points.csv`: 8 sample points (cell centres, edge cells, two exact cell
boundaries, one point outside).

Tests: the NoData assumption (-9999 should be offered as NoData, visibly) and
decoded values. Expected sampled values: `expected_raster_sample.csv`
(`reference/raster_sample.py`; cell-boundary rule documented there).

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
