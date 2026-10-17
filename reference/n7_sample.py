"""Reference: spot-check sample for N7 (R4 across 3,000 polygons).

Same definition as `zonal_mean.py` (exactextract coverage fraction x the cell's
area on the WGS 84 ellipsoid; NoData excluded; coverage = covered valid area /
polygon area with edges straight in lon/lat), computed for a sample of N7's
polygons: index ≡ 0 (mod 100), plus the last one — 31 areas, including holes,
multipolygons and areas crossing the raster's edge.

R4 is 23,040² float32 cells (2.1 GB at full resolution), so each polygon is
computed on its own window of R4 (rasterio windowed read, one cell of margin)
rather than on the whole raster. Cell areas use the closed-form authalic zone area
(`_common.zone_area_exact`; cross-checked against pyproj.Geod on a few rows):
R4's 5.2e-5° cells are far smaller than the cells the Geod row areas were made
for.

Each area is also computed with exact shapely fractions (`_crosscheck.py`);
where exactextract 0.3.0 disagrees by more than 1e-7 the checked values are used
and the `source` column says so.

Inputs are generated, never committed (`$LM_FIXTURES_DIR` overrides
fixtures/generated):
    cargo xtask fixtures --only R4,N7
Output: fixtures/N7/expected_zonal_mean_sample.csv with the columns of
`zonal_mean.py` plus `index` and `source`.

Usage:
  python -I reference/n7_sample.py
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, cell_area_rows, geod, write_csv, zone_area_exact  # noqa: E402
from _crosscheck import choose, planar_fractions, summary  # noqa: E402
from zonal_mean import polygon_area_lonlat_edges  # noqa: E402

import geopandas as gpd  # noqa: E402
import numpy as np  # noqa: E402
import rasterio  # noqa: E402
from exactextract import exact_extract  # noqa: E402
from exactextract.raster import NumPyRasterSource  # noqa: E402
from rasterio.windows import Window  # noqa: E402


def generated() -> Path:
    return Path(os.environ["LM_FIXTURES_DIR"]) if os.environ.get("LM_FIXTURES_DIR") else FIX / "generated"


def sample_indices(n: int) -> list[int]:
    return sorted(set(range(0, n, 100)) | {n - 1})


def main():
    r4 = generated() / "R4" / "r4_cog.tif"
    polys = generated() / "N7" / "n7_polygons.geojson"
    if not (r4.is_file() and polys.is_file()):
        print(f"skipped: {r4} or {polys} is missing (cargo xtask fixtures --only R4,N7)")
        return
    areas = gpd.read_file(polys, engine="pyogrio")
    assert len(areas) == 3000
    rows = []
    g = geod()
    with rasterio.open(r4) as src:
        assert src.crs.to_epsg() == 4326
        t = src.transform
        assert t.b == 0 and t.d == 0
        x0, y0, dx, dy = t.c, t.f, t.a, -t.e
        W, H = src.width, src.height
        nod = src.nodata
        wkt = src.crs.to_wkt()
        checked_geod = False
        for k in sample_indices(len(areas)):
            aid, geom = str(areas["area_id"][k]), areas.geometry[k]
            minx, miny, maxx, maxy = geom.bounds
            c0 = max(int(np.floor((minx - x0) / dx)) - 1, 0)
            c1 = min(int(np.ceil((maxx - x0) / dx)) + 1, W)
            r0 = max(int(np.floor((y0 - maxy) / dy)) - 1, 0)
            r1 = min(int(np.ceil((y0 - miny) / dy)) + 1, H)
            arr = src.read(1, window=Window(c0, r0, c1 - c0, r1 - r0)).astype(np.float64)
            lats = [y0 - r * dy for r in range(r0, r1 + 1)]
            row_area = np.array([zone_area_exact(lats[i + 1], lats[i], dx) for i in range(r1 - r0)])
            if not checked_geod:
                ga = cell_area_rows(g, x0, x0 + dx, lats[:4], n_dense=4)
                assert np.allclose(ga, row_area[:3], rtol=1e-6), (ga, row_area[:3])
                checked_geod = True
            wts = np.repeat(row_area[:, None], c1 - c0, axis=1)
            valid = np.ones_like(arr) if nod is None else np.where(arr == nod, np.nan, 1.0)
            vals = arr if nod is None else np.where(arr == nod, np.nan, arr)
            bx = (x0 + c0 * dx, y0 - r1 * dy, x0 + c1 * dx, y0 - r0 * dy)
            mk = lambda m, name: NumPyRasterSource(m, *bx, nodata=np.nan, name=name, srs_wkt=wkt)  # noqa: E731
            one = areas.iloc[[k]]
            a = exact_extract(mk(vals, "v"), one, ["weighted_mean", "min", "max"], weights=mk(wts, "w"),
                              output="pandas").iloc[0]
            b = exact_extract(mk(valid, "ones"), one, ["weighted_sum"], weights=mk(wts, "w"), output="pandas").iloc[0]
            parea = polygon_area_lonlat_edges(geom)
            ee = dict(mean=float(a["weighted_mean"]), min=float(a["min"]), max=float(a["max"]),
                      area=float(b["weighted_sum"]))
            chk = summary(arr, nod, row_area, *planar_fractions(bx[0], bx[3], dx, dy, r1 - r0, c1 - c0, geom))
            use, source = choose(ee, chk, ["mean", "min", "max", "area"])
            rows.append([aid, k, use["mean"], use["min"], use["max"], use["area"] / parea, use["area"], parea, source])
    write_csv(FIX / "N7" / "expected_zonal_mean_sample.csv",
              ["area_id", "index", "mean", "min", "max", "coverage", "covered_valid_area_m2", "polygon_area_m2", "source"],
              rows)
    edge = sum(1 for r in rows if r[5] < 0.99)
    fixed = [r[0] for r in rows if r[-1] != "exactextract"]
    print(f"N7/expected_zonal_mean_sample.csv: {len(rows)} areas ({edge} crossing R4's edge); "
          f"exactextract defects replaced by the check: {fixed or 'none'}")


if __name__ == "__main__":
    main()
