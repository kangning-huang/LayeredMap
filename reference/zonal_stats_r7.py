"""Reference: median, standard deviation and sum over R7 for the N4 areas.

Definitions (crates/lm-analysis/src/zonal.rs module notes), with f_i the exact covered
fraction of cell i (shapely, `_crosscheck.planar_fractions`) and A_i its area on the WGS 84
ellipsoid (`_common.cell_area_rows`), a_i = f_i A_i:
  * median: the smallest value whose cumulative weight a_i (values ascending) reaches half
    the total;
  * standard deviation: sqrt(sum a_i (x_i - mean)^2 / sum a_i), mean = sum a_i x_i / sum a_i;
  * sum: sum x_i f_i (exactextract's `sum`). Also checked against exactextract's `sum`
    (column ee_sum; empty where exactextract 0.3.0 is defective, see zonal_r7.py).
Output: fixtures/N4/expected_zonal_stats.csv (area_id, median, std_dev, sum, ee_sum).

R7 is generated: `cargo xtask fixtures --only R7`.

Usage:
  python -I reference/zonal_stats_r7.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, cell_area_rows, geod, write_csv  # noqa: E402
from _crosscheck import planar_fractions  # noqa: E402
from zonal_r7 import r7_path  # noqa: E402

import geopandas as gpd  # noqa: E402
import numpy as np  # noqa: E402
import rasterio  # noqa: E402
from exactextract import exact_extract  # noqa: E402


def main():
    raster = r7_path()
    if not raster.is_file():
        print(f"{raster} is not generated; run `cargo xtask fixtures --only R7` first")
        return 1
    with rasterio.open(raster) as src:
        t = src.transform
        arr = src.read(1).astype(np.float64)
    h, w = arr.shape
    x0, y0, dx, dy = t.c, t.f, t.a, -t.e
    row_area = np.array(cell_area_rows(geod(), x0, x0 + dx, [y0 - r * dy for r in range(h + 1)]))
    areas = gpd.read_file(FIX / "N4" / "polygons.geojson", engine="pyogrio")
    ee = exact_extract(str(raster), areas, ["sum"], include_cols=["area_id"], output="pandas")
    ee_sum = {str(r["area_id"]): float(r["sum"]) for _, r in ee.iterrows()}
    rows = []
    for aid, geom in zip(areas["area_id"], areas.geometry):
        r, c, f = planar_fractions(x0, y0, dx, dy, h, w, geom)
        x = arr[r, c]
        a = f * row_area[r]
        mean = float((a * x).sum() / a.sum())
        std = float(np.sqrt((a * (x - mean) ** 2).sum() / a.sum()))
        order = np.argsort(x, kind="stable")
        cum = np.cumsum(a[order])
        median = float(x[order][np.searchsorted(cum, cum[-1] / 2.0)])
        s = float((x * f).sum())
        e = ee_sum.get(str(aid))
        ok = e is not None and abs(e - s) <= 1e-7 * max(abs(s), 1.0)
        rows.append([str(aid), median, std, s, e if ok else ""])
    rows.sort(key=lambda r: r[0])
    write_csv(FIX / "N4" / "expected_zonal_stats.csv", ["area_id", "median", "std_dev", "sum", "ee_sum"], rows)
    print(f"wrote {len(rows)} rows; exactextract sum agrees on {sum(1 for r in rows if r[4] != '')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
