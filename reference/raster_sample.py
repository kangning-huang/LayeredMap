"""Reference: add raster value to points.

Definition (LayeredMap certified action "Add raster value to points"):
  * value of the cell containing the point, no interpolation;
  * cell index = (floor((x - x0) / dx), floor((y0 - y) / |dy|)) — a point
    exactly on a cell edge takes the cell to its right / below (the cell whose
    west/north edge it lies on);
  * scale/offset applied: value = raw * scale + offset;
  * NoData -> blank. NoData is the band's NoData tag, or, if --assume-nodata
    is given (R6 has no tag; -9999 occurs only on the edges), that raw value;
  * points outside the raster -> blank (row/col blank too).
Output columns: point_id, row, col, raw, value.

Usage:
  python -I reference/raster_sample.py                        # R6
  python -I reference/raster_sample.py RASTER POINTS_CSV OUT [--assume-nodata V]
"""

from __future__ import annotations

import argparse
import math
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import pandas as pd  # noqa: E402
import rasterio  # noqa: E402


def raster_sample(raster: Path, points_csv: Path, out: Path, assume_nodata: float | None = None):
    pts = pd.read_csv(points_csv, dtype=str, keep_default_na=False)
    with rasterio.open(raster) as src:
        arr = src.read(1)
        t = src.transform
        scale = src.scales[0] if src.scales[0] is not None else 1.0
        offset = src.offsets[0] if src.offsets[0] is not None else 0.0
        nod = src.nodata if src.nodata is not None else assume_nodata
        h, w = arr.shape
    rows = []
    for _, p in pts.iterrows():
        x, y = float(p["lon"]), float(p["lat"])
        col = math.floor((x - t.c) / t.a)
        row = math.floor((y - t.f) / t.e)  # t.e < 0
        if not (0 <= row < h and 0 <= col < w):
            rows.append([p["point_id"], "", "", "", ""])
            continue
        raw = arr[row, col].item()
        val = "" if (nod is not None and raw == nod) else raw * scale + offset
        rows.append([p["point_id"], row, col, raw, val])
    write_csv(out, ["point_id", "row", "col", "raw", "value"], rows)
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("args", nargs="*")
    ap.add_argument("--assume-nodata", type=float, default=None)
    a = ap.parse_args(argv)
    if a.args:
        r, p, out = a.args
        raster_sample(Path(r), Path(p), Path(out), a.assume_nodata)
        return
    rows = raster_sample(FIX / "R6" / "scaled.tif", FIX / "R6" / "points.csv",
                         FIX / "R6" / "expected_raster_sample.csv", assume_nodata=-9999)
    print(f"R6/expected_raster_sample.csv: {len(rows)} points, {sum(r[4] == '' for r in rows)} blank")


if __name__ == "__main__":
    main()
