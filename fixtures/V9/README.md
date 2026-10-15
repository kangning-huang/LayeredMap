# V9 — Archives and groups

- `roads_bundle.zip`: a Shapefile (6 roads, WGS 84 with .prj/.cpg) stored at
  `export/2024/shanghai/transport/roads.shp` — four folders deep — plus a stray `readme.txt`.
  Zip members have fixed timestamps.
- `city_layers.gpkg`: one GeoPackage with **3 layers** — `schools` (8 points),
  `roads` (6 lines, same as the zip), `parks` (3 areas). GPKG timestamps are fixed
  via `OGR_CURRENT_DATE`.

Tests: finding a Shapefile inside a nested zip folder; presenting a multi-layer
file as a group of 3 layers.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
