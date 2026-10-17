"""Reference for "Find areas where [rasters] meet conditions" (N11; puzzles › mechanics › A1).

Definition (the Rust implementation, lm_analysis::where_areas, must match it):

- every raster is read on the grid of the first one; another raster's value for a cell is its
  cell holding the first grid's cell centre, by floor of the pixel position (centres converted
  with pyproj when the coordinate systems differ); off the raster = NoData;
- values are raw × scale + offset (the file's scale/offset); NoData (or NaN) anywhere = out;
- a cell is in when every test holds (numpy comparisons; "between" is inclusive);
- areas: scipy.ndimage.label with a 3×3 structure (8-connected), numbered in raster order;
- area_m2: the sum of cell areas on the WGS 84 ellipsoid: latitude bands (closed authalic form)
  for a geographic grid; for a projected grid each cell's corners converted to lon/lat and the
  quadrilateral straight in (λ, q) measured by the shoelace formula;
- geod_area_m2: pyproj Geod area of the polygons rasterio.features.shapes traces
  (connectivity 8), densified to 1/64 cell in lon/lat (geographic) or to one cell in the
  raster's coordinates before conversion (projected); asserted within 1e-6 of area_m2;
- means: Σ x·A / Σ A over the area's cells, per raster.

Usage:  python -I reference/find_where.py
"""

from __future__ import annotations

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common  # noqa: E402,F401  (must precede pyproj/rasterio imports)
from _common import FIX, write_csv, write_text  # noqa: E402

import numpy as np  # noqa: E402
import rasterio  # noqa: E402
from pyproj import CRS, Geod, Transformer  # noqa: E402
from rasterio import features  # noqa: E402
from scipy import ndimage  # noqa: E402
from shapely import segmentize  # noqa: E402
from shapely.geometry import Polygon, shape  # noqa: E402
from shapely.geometry.polygon import orient  # noqa: E402
import shapely  # noqa: E402

WGS84_A = 6378137.0
WGS84_F = 1 / 298.257223563
E2 = WGS84_F * (2 - WGS84_F)
E = math.sqrt(E2)
HALF_B2 = 0.5 * (WGS84_A * (1 - WGS84_F)) ** 2

CASES = {
    "A": [("a_shapes.tif", ("between", 10.0, 20.0))],
    "B": [("b_first.tif", ("at_least", 15.0)), ("b_second.tif", ("any_of", [2.0, 4.0]))],
    "C": [("c_utm.tif", ("at_most", 0.3))],
}


def q(lat_deg):
    s = np.sin(np.radians(np.clip(lat_deg, -90, 90)))
    return s / (1 - E2 * s * s) + np.arctanh(E * s) / E


class Band:
    def __init__(self, path):
        with rasterio.open(path) as r:
            raw = r.read(1).astype(np.float64)
            self.nodata = r.nodata
            self.t = r.transform
            self.crs = r.crs
            self.shape = r.shape
            scale = r.scales[0] if r.scales else 1.0
            offset = r.offsets[0] if r.offsets else 0.0
            raw32 = r.read(1).astype(np.float32)
        valid = np.isfinite(raw32)
        if self.nodata is not None:
            valid &= raw32 != np.float32(self.nodata)
        self.valid = valid
        self.value = raw * scale + offset
        self.geographic = CRS.from_user_input(self.crs).is_geographic


def test(values, t):
    kind = t[0]
    if kind == "at_least":
        return values >= t[1]
    if kind == "at_most":
        return values <= t[1]
    if kind == "between":
        lo, hi = min(t[1], t[2]), max(t[1], t[2])
        return (values >= lo) & (values <= hi)
    return np.isin(values, t[1])


def sample(first: Band, other: Band):
    """Values and validity of `other` at the first grid's cell centres (floor indexing)."""
    h, w = first.shape
    rows, cols = np.indices((h, w))
    t = first.t
    x = t.c + (cols + 0.5) * t.a
    y = t.f + (rows + 0.5) * t.e
    if CRS.from_user_input(first.crs) != CRS.from_user_input(other.crs):
        tr = Transformer.from_crs(first.crs, other.crs, always_xy=True)
        x, y = tr.transform(x, y)
    o = other.t
    c = np.floor((x - o.c) / o.a)
    r = np.floor((y - o.f) / o.e)
    oh, ow = other.shape
    inside = (c >= 0) & (c < ow) & (r >= 0) & (r < oh)
    ci = np.where(inside, c, 0).astype(int)
    ri = np.where(inside, r, 0).astype(int)
    return other.value[ri, ci], inside & other.valid[ri, ci]


def cell_areas(first: Band):
    h, w = first.shape
    t = first.t
    if first.geographic:
        tops = t.f + np.arange(h) * t.e
        bottoms = t.f + (np.arange(h) + 1) * t.e
        row = HALF_B2 * math.radians(abs(t.a)) * np.abs(q(tops) - q(bottoms))
        return np.repeat(row[:, None], w, axis=1)
    tr = Transformer.from_crs(first.crs, "EPSG:4326", always_xy=True)
    jj, ii = np.indices((h + 1, w + 1))
    lon, lat = tr.transform(t.c + ii * t.a, t.f + jj * t.e)
    lam, qq = np.radians(lon), q(lat)
    p = [(lam[:-1, :-1], qq[:-1, :-1]), (lam[:-1, 1:], qq[:-1, 1:]), (lam[1:, 1:], qq[1:, 1:]), (lam[1:, :-1], qq[1:, :-1])]
    s2 = np.zeros((h, w))
    for k in range(4):
        (ax, ay), (bx, by) = p[k], p[(k + 1) % 4]
        s2 += ax * by - bx * ay
    return HALF_B2 * 0.5 * np.abs(s2)


def geod_areas(first: Band, lab: np.ndarray, n: int):
    geod = Geod(ellps="WGS84")
    t = first.t
    out = np.zeros(n + 1)
    tr = None if first.geographic else Transformer.from_crs(first.crs, "EPSG:4326", always_xy=True)
    for geom, val in features.shapes(lab.astype(np.int32), mask=lab > 0, connectivity=8, transform=t):
        g = shape(geom)
        if first.geographic:
            g = segmentize(g, abs(t.a) / 64)
        else:
            g = shapely.transform(segmentize(g, abs(t.a) * 1.0000001), lambda a: np.column_stack(tr.transform(a[:, 0], a[:, 1])))
        polys = [g] if isinstance(g, Polygon) else list(g.geoms)
        for p in polys:
            a, _ = geod.geometry_area_perimeter(orient(p, 1.0))
            out[int(val)] += a
    return out


def run_case(name, spec):
    bands = [Band(FIX / "N11" / f) for f, _ in spec]
    first = bands[0]
    vals = [first.value]
    ok = first.valid & test(first.value, spec[0][1])
    for b, (_, t) in zip(bands[1:], spec[1:]):
        v, valid = sample(first, b)
        vals.append(v)
        ok &= valid & test(np.where(valid, v, np.nan), t)
    lab, n = ndimage.label(ok, structure=np.ones((3, 3)))
    area = cell_areas(first)
    idx = np.arange(1, n + 1)
    a_sum = ndimage.sum(area, lab, idx)
    cells = ndimage.sum(np.ones_like(area), lab, idx)
    means = [ndimage.sum(np.where(ok, v, 0) * area, lab, idx) / a_sum for v in vals]
    geod = geod_areas(first, lab, n)
    rows = []
    for k in range(n):
        rel = abs(geod[k + 1] - a_sum[k]) / a_sum[k]
        assert rel < 1e-6, f"case {name} area {k + 1}: Geod {geod[k + 1]} vs cell sum {a_sum[k]} ({rel:.2e})"
        m = [means[j][k] for j in range(len(vals))] + [None] * (2 - len(vals))
        rows.append([name, k + 1, int(cells[k]), a_sum[k], geod[k + 1], *m])
    write_text(FIX / "N11" / f"expected_labels_{name}.csv", "\n".join(",".join(str(int(v)) for v in row) for row in lab) + "\n")
    return rows, n


def main():
    rows = []
    summary = []
    for name, spec in CASES.items():
        r, n = run_case(name, spec)
        rows += r
        summary.append(f"{name}: {n} areas")
    write_csv(FIX / "N11" / "expected_find_where.csv", ["case", "area", "cells", "area_m2", "geod_area_m2", "mean_1", "mean_2"], rows)
    print("N11 " + ", ".join(summary))


if __name__ == "__main__":
    main()
