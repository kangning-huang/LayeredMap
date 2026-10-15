# V3 — Shapefile without .prj, degree coordinates

`sites.shp/.shx/.dbf/.cpg`: 25 points over Shanghai in lon/lat degrees; the
`.prj` file was deliberately deleted (seed 3).

Tests: the app must apply a **visible WGS 84 assumption** (not silently).

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
