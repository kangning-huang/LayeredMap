# V4 — Shapefile without .prj, projected (CGCS2000 GK CM 120E)

`districts_gk.*`: V1's 17 districts reprojected to **EPSG:4549 — CGCS2000 / 3-degree Gauss-Kruger CM 120E**
(tmerc, lon_0 = 120, k = 1, x_0 = 500000, GRS80), coordinates rounded to 1 mm;
the `.prj` was deliberately deleted. Edges were densified to 0.01° before
reprojection.

Zone choice (verified with pyproj): the 3° GK zone whose area of use contains
lon 121.5 / central Shanghai is CM 120E (EPSG:4549, area of use 118.5–121.5°E);
the next zone CM 123E (EPSG:4550) starts at 121.5°E. Shanghai straddles the
seam; most of the city (incl. its centre, 121.47°E) is in CM 120E. Eastern
districts lie slightly outside the nominal zone, which is normal for GK data.

Projected bounds (m): [np.float64(614006.599), np.float64(3420504.505), np.float64(672100.28), np.float64(3487815.1)].

Tests: the "Where should this data be?" resolver — the expected answer is
Shanghai with EPSG:4549 (see `expected.json` for equivalent / non-candidates).

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
