"""Build the Gate 4 raster-summary fixtures that live in the repository.

  * R2 — a synthetic population grid (integer counts with NoData) and its density
    version, over V1's extent (stand-in for an open population grid);
  * N4 — eight polygons over R7 (the generated wide-latitude raster);
  * N6 — the count / density totals fixture (R2 x V1), README and expected.json;
  * N7 — small GeoTIFFs in every layout the window reader must handle (written by
    GDAL, so the reader is checked against an independent writer), plus README.

Deterministic: fixed seeds, GDAL's own writers with fixed options; rerunning
gives identical bytes. Expected numbers are written by `zonal_totals.py` (N6),
`zonal_r7.py` (N4) and `n7_sample.py` (N7).

Usage:  python -I reference/make_summary_fixtures.py
"""

from __future__ import annotations

import math
import os
import shutil
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common  # noqa: E402,F401  (must precede pyproj/rasterio imports)
from _common import FIX, write_geojson, write_json, write_text, zone_area_exact  # noqa: E402

import numpy as np  # noqa: E402
import rasterio  # noqa: E402
import rasterio.shutil  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402

MADE_BY = "Made by `reference/make_summary_fixtures.py` (synthetic, CC0-1.0)."


def fresh(fid: str) -> Path:
    """Empty fixtures/<fid>/ except the expected_*.csv the reference scripts own."""
    d = FIX / fid
    if d.exists():
        for p in d.iterdir():
            if p.name.startswith("expected_") and p.suffix == ".csv":
                continue
            if p.is_dir():
                shutil.rmtree(p, ignore_errors=True)
            else:
                p.unlink(missing_ok=True)  # an exFAT drive's ._ files vanish with their partner
    d.mkdir(parents=True, exist_ok=True)
    return d


def ring(points):
    pts = [[round(x, 9), round(y, 9)] for x, y in points]
    if pts[0] != pts[-1]:
        pts.append(pts[0])
    return pts


def star(cx, cy, r_out, r_in, n, rot=0.0):
    return [(cx + (r_out if k % 2 == 0 else r_in) * math.cos(rot + math.pi * k / n),
             cy + (r_out if k % 2 == 0 else r_in) * math.sin(rot + math.pi * k / n)) for k in range(2 * n)]


def polygon(*rings_):
    return {"type": "Polygon", "coordinates": [ring(r) for r in rings_]}


def multipolygon(*polys):
    return {"type": "MultiPolygon", "coordinates": [[ring(r) for r in p] for p in polys]}


# --------------------------------------------------------------------------
# R2 — synthetic population counts with NoData, and the density version
# --------------------------------------------------------------------------
R2_X0, R2_Y1, R2_RES, R2_N = 121.2, 31.5, 1 / 120, 72  # 30 arc-seconds, V1's extent
R2_COUNT_NODATA = 65535
R2_DENSITY_NODATA = -9999.0


def r2_arrays():
    rng = np.random.default_rng(2)
    n = R2_N
    xs = R2_X0 + (np.arange(n) + 0.5) * R2_RES
    ys = R2_Y1 - (np.arange(n) + 0.5) * R2_RES
    X, Y = np.meshgrid(xs, ys)
    centres = [(121.47, 31.23, 0.06, 28000), (121.55, 31.33, 0.04, 14000), (121.33, 31.10, 0.05, 9000),
               (121.68, 31.02, 0.03, 6000)]
    lam = np.full((n, n), 120.0)
    for cx, cy, s, peak in centres:
        lam += peak * np.exp(-((X - cx) ** 2 + (Y - cy) ** 2) / (2 * s * s))
    counts = rng.poisson(lam).clip(0, 60000).astype(np.uint16)
    # NoData: a river crossing the grid, and the sea in the north-east corner.
    river = np.abs((Y - 31.20) - 0.35 * (X - 121.50)) < 0.006
    sea = (X - 121.80) + (Y - 31.50) > -0.12
    nodata = river | sea
    counts[nodata] = R2_COUNT_NODATA
    # Density in people per km², from each cell's ellipsoidal area (closed form).
    lat_edges = R2_Y1 - np.arange(n + 1) * R2_RES
    area_km2 = np.array([zone_area_exact(lat_edges[r + 1], lat_edges[r], R2_RES) for r in range(n)]) / 1e6
    density = (counts.astype(np.float64) / area_km2[:, None]).astype(np.float32)
    density[nodata] = R2_DENSITY_NODATA
    return counts, density, int(nodata.sum())


def make_r2():
    d = fresh("R2")
    counts, density, n_nodata = r2_arrays()
    t = from_origin(R2_X0, R2_Y1, R2_RES, R2_RES)
    common = dict(driver="GTiff", width=R2_N, height=R2_N, count=1, crs="EPSG:4326", transform=t)
    with rasterio.open(d / "population_count.tif", "w", dtype="uint16", nodata=R2_COUNT_NODATA,
                       compress="lzw", predictor=2, blockysize=8, **common) as dst:
        dst.write(counts, 1)
    with rasterio.open(d / "population_density.tif", "w", dtype="float32", nodata=R2_DENSITY_NODATA,
                       compress="deflate", predictor=3, tiled=True, blockxsize=32, blockysize=32, **common) as dst:
        dst.write(density, 1)
    valid = counts[counts != R2_COUNT_NODATA].astype(np.int64)
    write_json(d / "expected.json", {
        "fixture": "R2", "files": ["population_count.tif", "population_density.tif"], "kind": "raster",
        "size": [R2_N, R2_N], "bands": 1, "crs": {"epsg": 4326, "source": "declared"},
        "bounds": [R2_X0, R2_Y1 - R2_N * R2_RES, R2_X0 + R2_N * R2_RES, R2_Y1], "pixel_size_deg": R2_RES,
        "count": {"file": "population_count.tif", "dtype": "uint16", "nodata": R2_COUNT_NODATA,
                  "layout": "strips of 8 rows, LZW, horizontal predictor", "value_role": "count",
                  "total_valid": int(valid.sum())},
        "density": {"file": "population_density.tif", "dtype": "float32", "nodata": R2_DENSITY_NODATA,
                    "layout": "32 x 32 tiles, Deflate, floating-point predictor", "value_role": "density per km2"},
        "nodata_cells": n_nodata,
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# R2 — Population counts with NoData (synthetic stand-in)

The plan's R2 is an open population grid whose licence is still to be verified,
so this is a **synthetic stand-in** with the same shape of data. Nothing was
downloaded.

- `population_count.tif`: {R2_N}×{R2_N} **uint16** people per cell, EPSG:4326, 30 arc-second
  cells (1/120°) over V1's extent (121.2–121.8°E, 30.9–31.5°N). NoData = {R2_COUNT_NODATA}
  ({n_nodata} cells: a river and the sea in the north-east corner). Strips of 8 rows,
  LZW with the horizontal predictor. Values: Poisson draws (seed 2) around four
  Gaussian "town" centres over a rural floor of 120 people per cell; {int(valid.sum()):,}
  people in the valid cells.
- `population_density.tif`: the same grid as **float32** people per km²: each count
  divided by its cell's area on the WGS 84 ellipsoid (closed-form authalic formula).
  NoData = {R2_DENSITY_NODATA:g} on the same cells. 32 × 32 tiles, Deflate with the
  floating-point predictor.

Tests: count totals `N = Σ n_i f_i` versus density totals `N = Σ d_i a_i` (N6),
and the window reader on striped LZW uint16 and tiled Deflate float32 files.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# N4 — polygons over R7 (unequal cell areas)
# --------------------------------------------------------------------------
def n4_features():
    hole = star(155.0, 38.0, 7.0, 3.0, 6, 0.2)[::-1]
    return [
        ("TALL", polygon([(110.0, -5.0), (114.0, -4.0), (116.0, 66.0), (111.0, 64.0)])),
        ("TRI", polygon([(120.0, 0.0), (150.0, 10.0), (130.0, 60.0)])),
        ("HOLE", polygon([(140.0, 20.0), (170.0, 20.0), (172.0, 55.0), (138.0, 58.0)], hole)),
        ("MULTI", multipolygon([[(101.0, -9.5), (106.0, -9.5), (104.0, -2.0)]],
                               [[(160.0, 62.0), (175.0, 64.0), (168.0, 69.5)]])),
        ("EDGE", polygon([(150.0, 62.0), (165.0, 61.0), (162.0, 78.0), (148.0, 76.0)])),
        ("STAR", polygon(star(125.0, 45.0, 6.0, 2.5, 9, 0.1))),
        ("DIAG", polygon([(101.0, -8.0), (178.0, 68.0), (178.4, 67.6), (101.4, -8.4)])),
        ("SMALL", polygon([(130.012, 60.013), (130.071, 60.019), (130.03, 60.046)])),
    ]


def make_n4():
    d = fresh("N4")
    feats = [{"type": "Feature", "properties": {"area_id": k}, "geometry": g} for k, g in n4_features()]
    write_geojson(d / "polygons.geojson", feats, name="n4_polygons")
    write_json(d / "expected.json", {
        "fixture": "N4", "files": ["polygons.geojson", "../generated/R7/r7_wide_latitude.tif"],
        "kind": "raster+vector", "action": "summarize (area-weighted mean, coverage)",
        "raster": {"file": "generated/R7/r7_wide_latitude.tif", "generated_by": "cargo xtask fixtures --only R7",
                   "size": [1600, 1600], "bounds": [100, -10, 180, 70], "pixel_size_deg": 0.05,
                   "value_role": "measured"},
        "areas": {"file": "polygons.geojson", "id": "area_id", "count": len(feats)},
        "expected_files": ["expected_zonal_mean.csv", "expected_geodesic_integration.csv"],
        "tolerance": {"vs_exactextract_rel": 1e-6, "vs_geodesic_integration_rel": 1e-3},
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# N4 — Summary over R7 (unequal cell areas)

`polygons.geojson`: eight areas (id `area_id`) over R7, the generated
geographic raster (`cargo xtask fixtures --only R7` →
`fixtures/generated/R7/r7_wide_latitude.tif`: 1,600 × 1,600 float32, 0.05° cells,
100–180°E, 10°S–70°N, so a cell at 70°N has a third of the area of one at the
equator).

| id | shape |
|---|---|
| TALL | a sloped quadrilateral from 5°S to 66°N |
| TRI | a large triangle, 0–60°N |
| HOLE | a quadrilateral 20–58°N with a star-shaped hole |
| MULTI | two triangles, one at the equator and one at 62–69.5°N |
| EDGE | a quadrilateral crossing R7's northern edge (70°N), partly outside |
| STAR | a 9-pointed star at 45°N |
| DIAG | a thin sliver from 8°S, 101°E to 68°N, 178°E (long sloped edges) |
| SMALL | a triangle smaller than a cell at 60°N |

Expected:

- `expected_zonal_mean.csv` — `reference/zonal_r7.py` with `reference/zonal_mean.py`'s
  definition: exactextract coverage fractions × geodesic cell areas (pyproj.Geod).
  Tolerance 1 part per million. Every area is cross-checked with exact shapely
  fractions (`reference/_crosscheck.py`); for TRI and DIAG the checked values are used
  because exactextract 0.3.0 leaves out two fully covered interior cells of each
  (6.4e-6 and 3.9e-5 of the covered area). The `source` column says which.
- `expected_geodesic_integration.csv` — `reference/zonal_r7.py`: the value integrated
  over each area on the ellipsoid, with each covered piece of a cell (the exact
  intersection of the cell and the area, edges straight in longitude/latitude)
  measured by pyproj.Geod after densifying its edges to 0.001°. Tolerance 1 part per
  thousand (the plan's tolerance for the geodesic integration: a flat coverage
  fraction weighted by the whole cell's area differs from the covered piece's own
  ellipsoidal area along the boundary).

{MADE_BY}
""")


# --------------------------------------------------------------------------
# N6 — count versus density totals (R2 x V1)
# --------------------------------------------------------------------------
def n6_features():
    hole = star(121.52, 31.22, 0.05, 0.02, 5, 0.4)[::-1]
    return [
        ("S1", polygon(star(121.47, 31.23, 0.13, 0.05, 7, 0.1))),
        ("S2", polygon([(121.10, 30.95), (121.37, 31.02), (121.21, 31.31)])),
        ("S3", polygon([(121.40, 31.08), (121.66, 31.11), (121.63, 31.36), (121.42, 31.33)], hole)),
        ("S4", multipolygon([[(121.251, 31.402), (121.333, 31.411), (121.297, 31.473)]],
                            [[(121.601, 30.931), (121.689, 30.944), (121.648, 31.012)]])),
        ("S5", polygon([(121.55, 31.45), (121.85, 31.29), (121.87, 31.33), (121.57, 31.49)])),
        ("S6", polygon([(121.4731, 31.2302), (121.4769, 31.2311), (121.4744, 31.2349)])),
    ]


def make_n6():
    d = fresh("N6")
    feats = [{"type": "Feature", "properties": {"area_id": k}, "geometry": g} for k, g in n6_features()]
    write_geojson(d / "areas.geojson", feats, name="n6_areas")
    write_json(d / "expected.json", {
        "fixture": "N6", "files": ["areas.geojson", "../R2/population_count.tif", "../R2/population_density.tif",
                                   "../V1/districts.geojson"],
        "kind": "raster+vector", "action": "summarize with totals",
        "areas": [{"file": "../V1/districts.geojson", "id": "district_id", "count": 17},
                  {"file": "areas.geojson", "id": "area_id", "count": len(feats)}],
        "count": {"file": "../R2/population_count.tif", "value_role": "count", "formula": "N = sum(n_i f_i)"},
        "density": {"file": "../R2/population_density.tif", "value_role": "density per km2",
                    "formula": "N = sum(d_i a_i) / 1e6, a_i = f_i A_i"},
        "expected_files": ["expected_totals.csv"],
        "tolerance": {"vs_exactextract_rel": 1e-6},
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# N6 — Totals from a count raster and from a density raster

Inputs (not duplicated): `../R2/population_count.tif` (people per cell, uint16,
NoData 65535), `../R2/population_density.tif` (people per km², float32, NoData
-9999), `../V1/districts.geojson` (17 districts, id `district_id`; D04 is a
MultiPolygon, D06 has a hole).

`areas.geojson`: six more areas (id `area_id`) that cut cells, because V1's
districts mostly follow R2's cell edges: S1 a star across the town centre and the
river (NoData), S2 a triangle partly west of the raster, S3 a quadrilateral with a
star-shaped hole, S4 a two-part multipolygon, S5 a sliver into the NoData sea, S6 a
triangle inside one cell.

Expected: `expected_totals.csv` (V1's districts, then S1–S6) from
`reference/zonal_totals.py`:

- `count_total` = Σ n_i f_i over valid cells — exactextract `sum` (coverage
  fraction × value) on the count raster;
- `density_total` = Σ d_i f_i A_i / 10⁶ — exactextract `weighted_sum` of the density
  raster with a geodesic cell-area weight raster (pyproj.Geod), in people;
- `covered_valid_area_m2` = Σ f_i A_i over valid cells.

Every area is cross-checked with exact shapely fractions
(`reference/_crosscheck.py`); for S5 the checked values are used because
exactextract 0.3.0 gives a cell outside S5 (beyond the raster's eastern edge) a
coverage of 1, 0.57 % of its covered area. The `source` column says which.

Because the density was made as count ÷ cell area, the two totals agree to the
density's float32 rounding (about 1e-7), which the reference asserts: the two
formulas are different routes to the same people, and a card must use the one
that matches the raster's meaning. Tolerance 1 part per million.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# N7 — small GeoTIFFs in each layout the window reader handles
# --------------------------------------------------------------------------
LAYOUT_W, LAYOUT_H = 150, 110


def layout_values():
    c = np.arange(LAYOUT_W)[None, :]
    r = np.arange(LAYOUT_H)[:, None]
    v = (c * 7 + r * 13) % 251
    nd = (c * 3 + r * 5) % 97 == 0
    return v.astype(np.int64), nd


# name, dtype, value(v, band) as array, nodata, creation options, crs, transform
def layout_specs():
    geo = from_origin(120.5, 31.9, 0.002, 0.0025)
    utm = from_origin(350000.0, 3460000.0, 30.0, 30.0)
    return [
        ("cog_deflate_f32.tif", "float32", 1, lambda v, b: v + 0.25, -9999.0, "COG",
         dict(blocksize=64, compress="DEFLATE", predictor="YES", overviews="AUTO", resampling="NEAREST"), "EPSG:4326", geo),
        ("tiled_lzw_i16.tif", "int16", 1, lambda v, b: v - 100, -32768, "GTiff",
         dict(tiled=True, blockxsize=32, blockysize=32, compress="lzw", predictor=2), "EPSG:4326", geo),
        ("striped_none_u8.tif", "uint8", 1, lambda v, b: v, 255, "GTiff",
         dict(blockysize=7), "EPSG:4326", geo),
        ("striped_deflate_u16.tif", "uint16", 1, lambda v, b: v * 200, 65535, "GTiff",
         dict(blockysize=16, compress="deflate"), "EPSG:4326", geo),
        ("tiled_none_f32_utm.tif", "float32", 1, lambda v, b: v + 0.5, float("nan"), "GTiff",
         dict(tiled=True, blockxsize=48, blockysize=48), "EPSG:32651", utm),
        ("planar_tiled_u8_3band.tif", "uint8", 3, lambda v, b: (v + 3 * b) % 251, 255, "GTiff",
         dict(tiled=True, blockxsize=32, blockysize=32, compress="deflate", interleave="band"), "EPSG:4326", geo),
        ("chunky_striped_u16_3band.tif", "uint16", 3, lambda v, b: v + 1000 * b, 65535, "GTiff",
         dict(blockysize=5, interleave="pixel", compress="lzw"), "EPSG:4326", geo),
        ("striped_packbits_i16.tif", "int16", 1, lambda v, b: v - 100, -32768, "GTiff",
         dict(blockysize=9, compress="packbits"), "EPSG:4326", geo),
        ("tiled_zstd_f32.tif", "float32", 1, lambda v, b: v + 0.25, -9999.0, "GTiff",
         dict(tiled=True, blockxsize=64, blockysize=32, compress="zstd", predictor=3), "EPSG:4326", geo),
        ("bigtiff_tiled_f64.tif", "float64", 1, lambda v, b: v / 8.0 - 3.0, -1e30, "GTiff",
         dict(tiled=True, blockxsize=16, blockysize=16, compress="deflate", predictor=3, bigtiff="YES"), "EPSG:4326", geo),
    ]


def make_n7():
    d = fresh("N7")
    out = d / "layouts"
    out.mkdir()
    v, nd = layout_values()
    rows = []
    for name, dtype, bands, f, nodata, driver, opts, crs, t in layout_specs():
        arr = np.stack([np.asarray(f(v, b)) for b in range(bands)]).astype(dtype)
        for b in range(bands):
            arr[b][nd] = nodata
        prof = dict(driver="GTiff", width=LAYOUT_W, height=LAYOUT_H, count=bands, dtype=dtype, crs=crs,
                    transform=t, nodata=nodata)
        if driver == "COG":
            tmp = out / ("_tmp_" + name)
            with rasterio.open(tmp, "w", **prof) as dst:
                dst.write(arr)
            rasterio.shutil.copy(tmp, out / name, driver="COG", **opts)
            tmp.unlink()
        else:
            with rasterio.open(out / name, "w", **prof, **opts) as dst:
                dst.write(arr)
        with rasterio.open(out / name) as src:
            assert src.count == bands and src.dtypes[0] == dtype
            back = src.read()
            same = (back == arr) | (np.isnan(back) & np.isnan(arr)) if dtype.startswith("float") else back == arr
            assert same.all(), name
            ovr = len(src.overviews(1))
            rows.append((name, dtype, bands, src.block_shapes[0], src.compression.value if src.compression else "none",
                         src.interleaving.value if src.interleaving else "-", crs, nodata, ovr))
    lines = "\n".join(f"| `{n}` | {dt} | {b} | {bs[1]} × {bs[0]} | {c} | {il if b > 1 else '-'} | {crs} | {nod:g} | {ovr} |"
                      for n, dt, b, bs, c, il, crs, nod, ovr in rows)
    write_text(d / "README.md", f"""# N7 — R4 across 3,000 polygons (performance, spot correctness)

## The stress run

R4 (`cargo xtask fixtures --only R4`, a 2.9 GB tiled BigTIFF COG: 23,040² float32
cells of 5.2e-5° over Shanghai in 512 px tiles, with 6 overviews) and N7's 3,000
polygons (`cargo xtask fixtures --only N7` → `fixtures/generated/N7/n7_polygons.geojson`,
id `area_id`; every 7th has a hole, every 11th is a multipolygon; the western column
and northern row, and some eastern and southern areas, cross the raster's edge) are
generated, never committed (`fixtures/generated/` is ignored by git).
`lm-analysis` test `zonal::tests::n7_r4_cog_stress` (ignored; run in release)
summarizes R4 across the 3,000 polygons with the window reader
(`lm_analysis::zonal::TiffSource`), times it and a cancel, and spot-checks a sample
against `expected_zonal_mean_sample.csv`:

```bash
cargo xtask fixtures --only R4,N7
cargo test --release -p lm-analysis zonal::tests::n7_r4_cog_stress -- --ignored --nocapture
# or: cargo xtask stress (all the stress tests)
```

`expected_zonal_mean_sample.csv` — `reference/n7_sample.py`: the 31 polygons with
index ≡ 0 (mod 100) and the last one, by exactextract on each polygon's window of
R4 with a geodesic cell-area weight raster (same definition as
`reference/zonal_mean.py`), cross-checked with exact shapely fractions (all 31
agree; `source` column). Tolerance 1 part per million.

## Reader layouts

`layouts/`: {LAYOUT_W} × {LAYOUT_H} GeoTIFFs written by GDAL (rasterio
{rasterio.__version__}, GDAL {rasterio.__gdal_version__}), one per layout the window
reader must decode. Every band holds a formula of the column `c` and row `r` with
`v = (7c + 13r) mod 251`, and NoData wherever `(3c + 5r) mod 97 = 0`:

| file | type | bands | block | compression | interleave | CRS | NoData | overviews |
|---|---|---|---|---|---|---|---|---|
{lines}

Values: `cog_deflate_f32` v + 0.25; `tiled_lzw_i16` v − 100; `striped_none_u8` v;
`striped_deflate_u16` 200·v; `tiled_none_f32_utm` v + 0.5; `striped_packbits_i16` v − 100;
`tiled_zstd_f32` v + 0.25; `planar_tiled_u8_3band`
band b (0-based) (v + 3b) mod 251; `chunky_striped_u16_3band` v + 1000·b;
`bigtiff_tiled_f64` v/8 − 3. Geographic files: origin 120.5°E, 31.9°N, cells
0.002° × 0.0025°; the UTM file (EPSG:32651): origin 350,000 E, 3,460,000 N, 30 m.

{MADE_BY}
""")


def main():
    make_r2()
    make_n4()
    make_n6()
    make_n7()
    print("summary fixtures written: R2, N4, N6, N7")


if __name__ == "__main__":
    main()
