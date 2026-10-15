# C1 — Amap POIs in GCJ-02

`amap_pois.csv`: 50 synthetic POIs over central Shanghai; true WGS 84 positions
(seed 101) were converted to **GCJ-02** with `prcoords.wgs_gcj` (prcoords 1.0.2,
CC0/GPL-3.0+). The file name `amap_…` is the only hint.

`expected_wgs84.csv`: the true WGS 84 coordinates and the GCJ-02 values written.

Tests: the offset question (name hint → suggest GCJ-02).

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
