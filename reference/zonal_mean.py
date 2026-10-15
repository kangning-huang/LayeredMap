"""Reference: area-weighted zonal mean with fractional pixel coverage.

Definition (LayeredMap certified action "Average value in areas"):
  * each cell's weight = (fraction of the cell covered by the polygon, from
    exactextract) x (the cell's geodesic area on the WGS 84 ellipsoid, from
    pyproj.Geod with the parallel edges densified);
  * mean = sum(v * frac * area) / sum(frac * area) over VALID cells;
  * min / max over valid cells with frac > 0;
  * coverage = covered valid area / polygon area, where polygon area is the
    geodesic area of the polygon with its edges taken as straight lines in
    lon/lat (densified to 0.001 deg before pyproj.Geod) — the same
    interpretation the raster grid uses;
  * NoData cells are excluded from mean/min/max and reduce coverage.
Raster must be in EPSG:4326 (lon/lat).
Output columns: area_id, mean, min, max, coverage, covered_valid_area_m2, polygon_area_m2.

Usage:
  python -I reference/zonal_mean.py                     # N3 (+ hand-calc check)
  python -I reference/zonal_mean.py RASTER AREAS AREA_ID OUT
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, cell_area_rows, geod, write_csv, zone_area_exact  # noqa: E402

import geopandas as gpd  # noqa: E402
import numpy as np  # noqa: E402
import rasterio  # noqa: E402
import shapely  # noqa: E402
from exactextract import exact_extract  # noqa: E402
from exactextract.raster import NumPyRasterSource  # noqa: E402


def load_sources(raster: Path):
    """Return (value source, valid-ones source, cell-area weight source, meta)."""
    with rasterio.open(raster) as src:
        assert src.crs.to_epsg() == 4326, "reference zonal stats expect EPSG:4326"
        t = src.transform
        assert t.b == 0 and t.d == 0, "rotated rasters not supported"
        arr = src.read(1).astype(np.float64)
        nod = src.nodata
        h, w = arr.shape
        xmin, ymax = t.c, t.f
        xmax, ymin = xmin + w * t.a, ymax + h * t.e
        wkt = src.crs.to_wkt()
    g = geod()
    lats = [ymax + r * t.e for r in range(h + 1)]
    row_area = cell_area_rows(g, xmin, xmin + t.a, lats)
    # cross-check the Geod cell areas against the closed-form zone area
    for r in range(h):
        exact = zone_area_exact(lats[r + 1], lats[r], t.a)
        assert abs(row_area[r] - exact) / exact < 1e-8, (r, row_area[r], exact)
    weights = np.repeat(np.array(row_area)[:, None], w, axis=1)
    valid = np.ones_like(arr) if nod is None else np.where(arr == nod, np.nan, 1.0)
    vals = arr if nod is None else np.where(arr == nod, np.nan, arr)
    mk = lambda m, name: NumPyRasterSource(m, xmin, ymin, xmax, ymax, nodata=np.nan, name=name, srs_wkt=wkt)  # noqa: E731
    return mk(vals, "v"), mk(valid, "ones"), mk(weights, "cellarea"), (xmin, ymin, xmax, ymax)


def polygon_area_lonlat_edges(g) -> float:
    dense = shapely.segmentize(g, 0.001)
    a, _ = geod().geometry_area_perimeter(dense)
    return abs(a)


def zonal_mean(raster: Path, areas_path: Path, area_id: str, out: Path):
    v, ones, wts, _ = load_sources(raster)
    areas = gpd.read_file(areas_path, engine="pyogrio")
    r1 = exact_extract(v, areas, ["weighted_mean", "min", "max"], weights=wts,
                       include_cols=[area_id], output="pandas")
    r2 = exact_extract(ones, areas, ["weighted_sum"], weights=wts, include_cols=[area_id], output="pandas")
    rows = []
    for (_, a), (_, b), geom in zip(r1.iterrows(), r2.iterrows(), areas.geometry):
        assert a[area_id] == b[area_id]
        parea = polygon_area_lonlat_edges(geom)
        cov_area = float(b["weighted_sum"]) if np.isfinite(b["weighted_sum"]) else 0.0
        rows.append({"area_id": str(a[area_id]), "mean": float(a["weighted_mean"]), "min": float(a["min"]),
                     "max": float(a["max"]), "coverage": cov_area / parea,
                     "covered_valid_area_m2": cov_area, "polygon_area_m2": parea})
    rows.sort(key=lambda r: r["area_id"])
    cols = ["area_id", "mean", "min", "max", "coverage", "covered_valid_area_m2", "polygon_area_m2"]
    write_csv(out, cols, [[r[c] for c in cols] for r in rows])
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("args", nargs="*")
    a = ap.parse_args(argv)
    if a.args:
        r, ar, aid, out = a.args
        zonal_mean(Path(r), Path(ar), aid, Path(out))
        return
    d = FIX / "N3"
    rows = zonal_mean(d / "grid4x4.tif", d / "polygon.geojson", "area_id", d / "expected_zonal_mean.csv")
    hand = json.loads((d / "expected.json").read_text("utf-8"))["hand_calculation"]
    r = rows[0]
    for k in ("mean", "coverage", "polygon_area_m2"):
        assert abs(r[k] - hand[k]) / abs(hand[k]) < 1e-9, (k, r[k], hand[k])
    assert r["min"] == hand["min"] and r["max"] == hand["max"]
    print(f"N3/expected_zonal_mean.csv: mean={r['mean']:.10f} coverage={r['coverage']:.10f} "
          f"(matches hand calculation)")


if __name__ == "__main__":
    main()
