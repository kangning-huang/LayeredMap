# N3 — 4×4 raster, area-weighted mean and coverage

`grid4x4.tif`: 4×4 float32, EPSG:4326, 1° cells, lon 10–14°E, lat 58–62°N
(high latitude so cell areas differ by row). NoData = -9999. Values (row 0 = north):

```
row 0 (61–62°N):   1    2    3    4
row 1 (60–61°N):   5  ND    7    8      (ND was 6)
row 2 (59–60°N):  ND   10   11   12     (ND was 9)
row 3 (58–59°N):  13   14   15   16
```

`polygon.geojson`: one rectangle `Z1` = lon 10.5–13.5, lat 58–61. It covers
rows 1–3; in each row it covers **half** of the first and last columns and all
of the middle two. Its east/west edges are meridians, which split a cell's
area exactly in half, and its north/south edges lie on cell boundaries, so the
exact geodesic answer equals the fractional-cell answer.

## Hand calculation

Area of a 1°×1° cell between latitudes φ1, φ2 on WGS 84 (authalic formula,
`A = b²/2 · Δλ · [q(φ2) − q(φ1)]`, q(φ) = sinφ/(1−e²sin²φ) + 1/(2e)·ln((1+e sinφ)/(1−e sinφ))):

| row | lat | A (km²) |
|---|---|---|
| 1 | 60–61 | 6,123.1409 |
| 2 | 59–60 | 6,309.8057 |
| 3 | 58–59 | 6,494.4470 |

Covered valid cells (value × coverage fraction):

- row 1: 5×0.5, 7×1, 8×0.5 → Σf = 2.0, Σvf = 13.5 (cell 6 is NoData, fraction 1 lost)
- row 2: 10×1, 11×1, 12×0.5 → Σf = 2.5, Σvf = 27.0 (cell 9 is NoData, fraction 0.5 lost)
- row 3: 13×0.5, 14×1, 15×1, 16×0.5 → Σf = 3.0, Σvf = 43.5

mean = (13.5·A1 + 27·A2 + 43.5·A3) / (2·A1 + 2.5·A2 + 3·A3) = **11.2734518278**

coverage = (2·A1 + 2.5·A2 + 3·A3) / (3·(A1 + A2 + A3)) = **0.8366028988**

min = **5**, max = **16**, polygon area = 3·(A1+A2+A3) = 56,782.1806 km².

(For contrast, the unweighted cell-count mean would be 84/7.5 = 11.2000; the
southern rows have more area, so the weighted mean is slightly higher.)

`reference/zonal_mean.py` reproduces these with exactextract + a Geod cell-area
weight raster and asserts agreement with this hand calculation to 1e-9.
Expected: `expected_zonal_mean.csv`.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
