"""Reference: totals from a count raster and from a density raster (N6 = R2 x V1).

Definition (LayeredMap certified action "Summarize", totals for confirmed count or
density rasters):
  * count raster (people per cell): N = sum(n_i * f_i) over valid cells, f_i the
    exactextract coverage fraction (people assumed evenly spread within a cell)
    -> exactextract 'sum';
  * density raster (people per km2): N = sum(d_i * f_i * A_i) / 1e6 over valid cells,
    A_i the geodesic cell area on WGS 84 (pyproj.Geod, parallels densified)
    -> exactextract 'weighted_sum' with a cell-area weight raster;
  * NoData cells contribute nothing.
Rasters must be in EPSG:4326 on the same grid. R2's density is count / cell area,
so the two totals must agree to float32 rounding; this is asserted (1e-6).
Each area is also computed with exact shapely fractions (`_crosscheck.py`); where
exactextract 0.3.0 disagrees by more than 1e-7 the checked values are used and the
`source` column says so (S5: exactextract gives a cell outside the area, beyond
the raster's edge, a coverage of 1).
Output columns: area_id, count_total, density_total, covered_valid_area_m2, source.

Usage:
  python -I reference/zonal_totals.py                     # N6 (V1 districts, then N6/areas.geojson)
  python -I reference/zonal_totals.py COUNT DENSITY AREAS AREA_ID OUT
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv, zone_area_exact  # noqa: E402
from _crosscheck import choose, planar_fractions, summary  # noqa: E402
from zonal_mean import load_sources  # noqa: E402

import geopandas as gpd  # noqa: E402
import numpy as np  # noqa: E402
import rasterio  # noqa: E402
from exactextract import exact_extract  # noqa: E402


def zonal_totals(count: Path, density: Path, areas_path: Path, area_id: str, out: Path):
    cv, cones, cw, cbox = load_sources(count)
    dv, _, dw, dbox = load_sources(density)
    assert cbox == dbox, "count and density rasters must share a grid"
    areas = gpd.read_file(areas_path, engine="pyogrio")
    rc = exact_extract(cv, areas, ["sum"], include_cols=[area_id], output="pandas")
    rd = exact_extract(dv, areas, ["weighted_sum"], weights=dw, include_cols=[area_id], output="pandas")
    ra = exact_extract(cones, areas, ["weighted_sum"], weights=cw, include_cols=[area_id], output="pandas")
    with rasterio.open(count) as src:
        t, carr, cnod = src.transform, src.read(1).astype(np.float64), src.nodata
    with rasterio.open(density) as src:
        darr, dnod = src.read(1).astype(np.float64), src.nodata
    h, w = carr.shape
    x0, y0, dx, dy = t.c, t.f, t.a, -t.e
    # Closed-form cell areas for the check (load_sources cross-checks them against Geod to 1e-8).
    row_area = np.array([zone_area_exact(y0 - (r + 1) * dy, y0 - r * dy, dx) for r in range(h)])
    rows = []
    for (_, a), (_, b), (_, c), geom in zip(rc.iterrows(), rd.iterrows(), ra.iterrows(), areas.geometry):
        assert a[area_id] == b[area_id] == c[area_id]
        ee = dict(count=float(a["sum"]), density=float(b["weighted_sum"]) / 1e6, area=float(c["weighted_sum"]))
        fr = planar_fractions(x0, y0, dx, dy, h, w, geom)
        sc, sd = summary(carr, cnod, row_area, *fr), summary(darr, dnod, row_area, *fr)
        chk = dict(count=sc["sum_vf"], density=sd["sum_va"] / 1e6, area=sc["area"])
        use, source = choose(ee, chk, ["count", "density", "area"])
        assert abs(use["count"] - use["density"]) <= 1e-6 * max(abs(use["count"]), 1.0), (a[area_id], use)
        rows.append([str(a[area_id]), use["count"], use["density"], use["area"], source])
    rows.sort(key=lambda r: r[0])
    write_csv(out, ["area_id", "count_total", "density_total", "covered_valid_area_m2", "source"], rows)
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("args", nargs="*")
    a = ap.parse_args(argv)
    if a.args:
        c, d, ar, aid, out = a.args
        zonal_totals(Path(c), Path(d), Path(ar), aid, Path(out))
        return
    c, d, out = FIX / "R2/population_count.tif", FIX / "R2/population_density.tif", FIX / "N6/expected_totals.csv"
    rows = zonal_totals(c, d, FIX / "V1/districts.geojson", "district_id", out)
    rows += zonal_totals(c, d, FIX / "N6/areas.geojson", "area_id", out)
    write_csv(out, ["area_id", "count_total", "density_total", "covered_valid_area_m2", "source"], rows)
    fixed = [r[0] for r in rows if r[-1] != "exactextract"]
    worst = max(abs(r[1] - r[2]) / max(abs(r[1]), 1.0) for r in rows)
    print(f"N6/expected_totals.csv: {len(rows)} areas, total {sum(r[1] for r in rows):,.1f} people; "
          f"count vs density totals agree to {worst:.1e}; exactextract defects replaced by the check: {fixed or 'none'}")


if __name__ == "__main__":
    main()
