"""Reference: share of each land-cover category's area per area (N8 = R3 x V1).

Definition (LayeredMap certified action "Category shares in areas"):
  * category rasters are never averaged; for each area and category,
    share = sum(frac * cell_area over cells of that category)
            / sum(frac * cell_area over all valid cells),
    with frac = exactextract coverage fraction and cell_area = geodesic cell
    area on WGS 84 (pyproj.Geod) — i.e. exactextract 'weighted_frac' with a
    cell-area weight raster;
  * NoData cells are excluded (shares sum to 1 over the valid covered area);
  * only categories present in an area are listed.
Output columns: area_id, category, share, area_km2 (= share x covered valid area).
Sorted by area_id, then category.

Usage:
  python -I reference/category_shares.py                      # N8
  python -I reference/category_shares.py RASTER AREAS AREA_ID OUT
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402
from zonal_mean import load_sources  # noqa: E402

import geopandas as gpd  # noqa: E402
import numpy as np  # noqa: E402
from exactextract import exact_extract  # noqa: E402


def category_shares(raster: Path, areas_path: Path, area_id: str, out: Path):
    v, ones, wts, _ = load_sources(raster)
    areas = gpd.read_file(areas_path, engine="pyogrio")
    r = exact_extract(v, areas, ["unique", "weighted_frac"], weights=wts, include_cols=[area_id],
                      output="pandas")
    r2 = exact_extract(ones, areas, ["weighted_sum"], weights=wts, include_cols=[area_id], output="pandas")
    covered = dict(zip(r2[area_id].astype(str), r2["weighted_sum"].astype(float)))
    rows = []
    for _, row in r.iterrows():
        aid = str(row[area_id])
        cats = np.asarray(row["unique"], dtype=float)
        fr = np.asarray(row["weighted_frac"], dtype=float)
        assert abs(fr.sum() - 1) < 1e-9 or len(fr) == 0, (aid, fr.sum())
        for c, f in sorted(zip(cats, fr)):
            rows.append([aid, int(c), float(f), float(f) * covered[aid] / 1e6])
    rows.sort(key=lambda x: (x[0], x[1]))
    write_csv(out, ["area_id", "category", "share", "area_km2"], rows)
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("args", nargs="*")
    a = ap.parse_args(argv)
    if a.args:
        r, ar, aid, out = a.args
        category_shares(Path(r), Path(ar), aid, Path(out))
        return
    rows = category_shares(FIX / "R3" / "landcover.tif", FIX / "V1" / "districts.geojson", "district_id",
                           FIX / "N8" / "expected_category_shares.csv")
    print(f"N8/expected_category_shares.csv: {len(rows)} rows over {len({r[0] for r in rows})} districts")


if __name__ == "__main__":
    main()
