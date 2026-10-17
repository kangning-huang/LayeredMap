"""An independent check of exactextract's coverage fractions.

`planar_fractions` computes, for every cell of a polygon's bounding box, the
fraction of the cell inside the polygon as the exact area of the shapely
intersection divided by the cell's area, both in the raster's own coordinates —
exactly the quantity exactextract's coverage fraction is defined to be. The
reference scripts compute each summary both ways and use exactextract's numbers
unless the two disagree by more than `DEFECT_REL` (exactextract keeps fractions
in float32, so honest agreement is about 1e-10 on an area's totals).

exactextract 0.3.0 has defects this catches, recorded in each CSV's `source`
column: whole interior cells left out near a polygon's vertex (N4 TRI and DIAG
lose two fully covered cells each), and a cell outside a polygon that crosses the
raster's edge given a coverage of 1 (N6 S5).
"""

from __future__ import annotations

import numpy as np
import shapely

DEFECT_REL = 1e-7


def planar_fractions(x0: float, y0: float, dx: float, dy: float, h: int, w: int, geom):
    """(rows, cols, f) of the cells of an h × w north-up grid (top-left corner x0, y0; cell
    dx × dy) that `geom` covers, with f the exact covered fraction (shapely)."""
    minx, miny, maxx, maxy = geom.bounds
    c0, c1 = max(int(np.floor((minx - x0) / dx)), 0), min(int(np.ceil((maxx - x0) / dx)), w)
    r0, r1 = max(int(np.floor((y0 - maxy) / dy)), 0), min(int(np.ceil((y0 - miny) / dy)), h)
    if c0 >= c1 or r0 >= r1:
        return np.zeros(0, int), np.zeros(0, int), np.zeros(0)
    cols, rows = np.meshgrid(np.arange(c0, c1), np.arange(r0, r1))
    cols, rows = cols.ravel(), rows.ravel()
    cells = shapely.box(x0 + cols * dx, y0 - (rows + 1) * dy, x0 + (cols + 1) * dx, y0 - rows * dy)
    shapely.prepare(geom)
    f = np.zeros(len(cells))
    inside = shapely.contains_properly(geom, cells)
    f[inside] = 1.0
    edge = ~inside & shapely.intersects(geom, cells)
    f[edge] = shapely.area(shapely.intersection(cells[edge], geom)) / (dx * dy)
    keep = f > 0
    return rows[keep], cols[keep], f[keep]


def summary(arr, nodata, row_area, rows, cols, f):
    """Weighted mean, min, max, covered valid area, Σ v·f and Σ v·f·A over valid cells (arr
    and row_area indexed by the same rows and columns as the fractions)."""
    v = arr[rows, cols]
    ok = np.isfinite(v) if nodata is None else (v != nodata) & np.isfinite(v)
    a = f[ok] * row_area[rows[ok]]
    vv = v[ok]
    if a.sum() == 0:
        return dict(mean=float("nan"), min=float("nan"), max=float("nan"), area=0.0, sum_vf=0.0, sum_va=0.0)
    return dict(mean=float((vv * a).sum() / a.sum()), min=float(vv.min()), max=float(vv.max()), area=float(a.sum()),
                sum_vf=float((vv * f[ok]).sum()), sum_va=float((vv * a).sum()))


def rel(a: float, b: float) -> float:
    return abs(a - b) / max(abs(b), 1e-300)


def choose(ee: dict, check: dict, keys) -> tuple[dict, str]:
    """exactextract's values unless they differ from the check by more than DEFECT_REL."""
    worst = max(rel(ee[k], check[k]) for k in keys)
    if worst <= DEFECT_REL:
        return ee, "exactextract"
    return check, f"shapely (exactextract 0.3.0 off by {worst:.1e})"
