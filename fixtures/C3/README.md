# C3 — Baidu POIs in BD-09

`baidu_pois.csv`: 30 synthetic POIs (seed 103), WGS 84 → GCJ-02 → **BD-09**
with prcoords 1.0.2 (`wgs_gcj`, then `gcj_bd`). `expected_wgs84.csv` has the
true WGS 84 coordinates.

Tests: second offset system (BD-09), suggested from the `baidu` name hint.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
