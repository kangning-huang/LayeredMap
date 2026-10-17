"""Reference: area-weighted summaries over R7, the wide-latitude raster (N4).

Two references for the same eight areas (fixtures/N4/polygons.geojson):

1. exactextract, as `zonal_mean.py` defines it (coverage fraction x geodesic cell
   area) -> fixtures/N4/expected_zonal_mean.csv. Tolerance 1e-6. Each area is
   also computed with exact shapely fractions (`_crosscheck.py`); where
   exactextract 0.3.0 disagrees by more than 1e-7 the checked values are used and
   the `source` column says so (TRI and DIAG: exactextract leaves out two fully
   covered cells each).

2. A geodesic integration that never uses a flat coverage fraction ->
   fixtures/N4/expected_geodesic_integration.csv. Tolerance 1e-3.
   For every cell of the area's bounding box:
     * a cell entirely inside the area contributes its whole geodesic area
       (pyproj.Geod on the cell with its parallels densified, as `_common.py`);
     * a cell crossed by the area's boundary contributes the geodesic area of the
       exact piece cell ∩ area (shapely, in lon/lat), with the piece's edges
       densified to 0.001° before pyproj.Geod measures it, so each edge is
       measured as the line straight in longitude/latitude that the grid sees;
   then mean = Σ x_i G_i / Σ G_i, covered_area = Σ G_i and
   coverage = covered_area / polygon_area (polygon area by pyproj.Geod on the
   area densified to 0.001°). R7 has no NoData.

R7 is generated, not committed: `cargo xtask fixtures --only R7` writes
fixtures/generated/R7/r7_wide_latitude.tif ($LM_FIXTURES_DIR overrides
fixtures/generated). Without it this script says so and writes nothing.
Output columns: (1) area_id, mean, min, max, coverage, covered_valid_area_m2,
polygon_area_m2, source; (2) area_id, mean, covered_area_m2, polygon_area_m2,
coverage, cells, boundary_cells.

Usage:
  python -I reference/zonal_r7.py
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, cell_area_rows, geod, write_csv  # noqa: E402
from _crosscheck import choose, planar_fractions, summary  # noqa: E402
from zonal_mean import load_sources, polygon_area_lonlat_edges  # noqa: E402

import geopandas as gpd  # noqa: E402
import numpy as np  # noqa: E402
import rasterio  # noqa: E402
import shapely  # noqa: E402
from exactextract import exact_extract  # noqa: E402

DENSIFY_DEG = 0.001


def r7_path() -> Path:
    gen = Path(os.environ["LM_FIXTURES_DIR"]) if os.environ.get("LM_FIXTURES_DIR") else FIX / "generated"
    return gen / "R7" / "r7_wide_latitude.tif"


def geodesic_integration(raster: Path, areas_path: Path, area_id: str, out: Path):
    with rasterio.open(raster) as src:
        assert src.crs.to_epsg() == 4326
        t = src.transform
        arr = src.read(1).astype(np.float64)
        nod = src.nodata
    assert nod is None, "R7 has no NoData"
    h, w = arr.shape
    x0, y0, dx, dy = t.c, t.f, t.a, -t.e
    g = geod()
    lats = [y0 - r * dy for r in range(h + 1)]
    row_area = np.array(cell_area_rows(g, x0, x0 + dx, lats))
    areas = gpd.read_file(areas_path, engine="pyogrio")
    rows = []
    for aid, geom in zip(areas[area_id].astype(str), areas.geometry):
        minx, miny, maxx, maxy = geom.bounds
        c0, c1 = max(int(np.floor((minx - x0) / dx)), 0), min(int(np.ceil((maxx - x0) / dx)), w)
        r0, r1 = max(int(np.floor((y0 - maxy) / dy)), 0), min(int(np.ceil((y0 - miny) / dy)), h)
        cols, rws = np.meshgrid(np.arange(c0, c1), np.arange(r0, r1))
        cols, rws = cols.ravel(), rws.ravel()
        cells = shapely.box(x0 + cols * dx, y0 - (rws + 1) * dy, x0 + (cols + 1) * dx, y0 - rws * dy)
        shapely.prepare(geom)
        inside = shapely.contains_properly(geom, cells)
        touches = ~inside & shapely.intersects(geom, cells)
        wsum = float(row_area[rws[inside]].sum())
        xsum = float((arr[rws[inside], cols[inside]] * row_area[rws[inside]]).sum())
        pieces = shapely.intersection(cells[touches], geom)
        n_boundary = 0
        for piece, r, c in zip(pieces, rws[touches], cols[touches]):
            if piece.is_empty or piece.area == 0:
                continue
            a, _ = g.geometry_area_perimeter(shapely.segmentize(piece, DENSIFY_DEG))
            a = abs(a)
            n_boundary += 1
            wsum += a
            xsum += arr[r, c] * a
        parea = polygon_area_lonlat_edges(geom)
        rows.append([aid, xsum / wsum, wsum, parea, wsum / parea, int(inside.sum()) + n_boundary, n_boundary])
    rows.sort(key=lambda r: r[0])
    write_csv(out, ["area_id", "mean", "covered_area_m2", "polygon_area_m2", "coverage", "cells", "boundary_cells"], rows)
    return rows


def zonal_mean_checked(raster: Path, areas_path: Path, area_id: str, out: Path):
    """`zonal_mean.py`'s definition and columns, cross-checked (see the module notes)."""
    v, ones, wts, _ = load_sources(raster)
    with rasterio.open(raster) as src:
        t = src.transform
        arr = src.read(1).astype(np.float64)
        nod = src.nodata
    h, w = arr.shape
    x0, y0, dx, dy = t.c, t.f, t.a, -t.e
    row_area = np.array(cell_area_rows(geod(), x0, x0 + dx, [y0 - r * dy for r in range(h + 1)]))
    areas = gpd.read_file(areas_path, engine="pyogrio")
    r1 = exact_extract(v, areas, ["weighted_mean", "min", "max"], weights=wts, include_cols=[area_id], output="pandas")
    r2 = exact_extract(ones, areas, ["weighted_sum"], weights=wts, include_cols=[area_id], output="pandas")
    rows = []
    for (_, a), (_, b), geom in zip(r1.iterrows(), r2.iterrows(), areas.geometry):
        ee = dict(mean=float(a["weighted_mean"]), min=float(a["min"]), max=float(a["max"]),
                  area=float(b["weighted_sum"]) if np.isfinite(b["weighted_sum"]) else 0.0)
        chk = summary(arr, nod, row_area, *planar_fractions(x0, y0, dx, dy, h, w, geom))
        use, source = choose(ee, chk, ["mean", "min", "max", "area"])
        parea = polygon_area_lonlat_edges(geom)
        rows.append([str(a[area_id]), use["mean"], use["min"], use["max"], use["area"] / parea, use["area"], parea, source])
    rows.sort(key=lambda r: r[0])
    write_csv(out, ["area_id", "mean", "min", "max", "coverage", "covered_valid_area_m2", "polygon_area_m2", "source"], rows)
    return rows


def main():
    r7 = r7_path()
    if not r7.is_file():
        print(f"skipped: {r7} is missing (cargo xtask fixtures --only R7)")
        return
    d = FIX / "N4"
    ee = zonal_mean_checked(r7, d / "polygons.geojson", "area_id", d / "expected_zonal_mean.csv")
    gi = geodesic_integration(r7, d / "polygons.geojson", "area_id", d / "expected_geodesic_integration.csv")
    worst = max(abs(a[1] - b[1]) / abs(b[1]) for a, b in zip(ee, gi))
    fixed = [a[0] for a in ee if a[-1] != "exactextract"]
    print(f"N4: {len(gi)} areas; summary vs geodesic integration means differ by at most {worst:.2e} relative; "
          f"exactextract defects replaced by the check: {fixed or 'none'}")


if __name__ == "__main__":
    main()
