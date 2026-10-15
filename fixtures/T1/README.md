# T1 — Schools CSV with lon/lat (synthetic)

`schools.csv`: 248 synthetic schools over V1's extent (lon 121.2–121.8,
lat 30.9–31.5), columns `name, type (Primary/Middle/High), students, lon, lat`.
Six rows have blank coordinates (data rows 13, 58, 102, 151, 200, 241;
two of them have only one of lon/lat blank). Random draws use seed 20260101;
no point lies exactly on a V1 district boundary.

Tests: visible WGS 84 assumption; import report must say **242 located,
6 excluded**.

Expected: `expected.json`; `expected_count_in_areas.csv` = schools counted in each
V1 district (`reference/count_in_areas.py`, boundary rule: intersects). Points in
Chongming's channel count in no district.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
