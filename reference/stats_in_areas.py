"""Reference: a statistic of a point field over the points in each area.

Definition (LayeredMap "Count [points] in each [area]" with a statistic): GeoPandas
sjoin(predicate="intersects"), so a point on a boundary is in every area it touches; per
area, over the points' non-blank values: sum (0 with none), mean, median (the mean of the two
middle values for an even count), min, max (blank with none).
Output: fixtures/T1/expected_stats_in_areas.csv (district_id, count, sum, mean, median, min,
max) for T1 schools' `students` in V1 districts.

Usage:
  python -I reference/stats_in_areas.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import pandas as pd  # noqa: E402


def main():
    areas = gpd.read_file(FIX / "V1" / "districts.geojson", engine="pyogrio")
    df = pd.read_csv(FIX / "T1" / "schools.csv")
    df = df[pd.to_numeric(df["lon"], errors="coerce").notna() & pd.to_numeric(df["lat"], errors="coerce").notna()]
    pts = gpd.GeoDataFrame(df, geometry=gpd.points_from_xy(df["lon"].astype(float), df["lat"].astype(float)), crs="EPSG:4326")
    j = gpd.sjoin(pts, areas[["district_id", "geometry"]], predicate="intersects")
    rows = []
    for did in sorted(areas["district_id"]):
        v = pd.to_numeric(j.loc[j["district_id"] == did, "students"], errors="coerce").dropna()
        n = int((j["district_id"] == did).sum())
        if len(v):
            rows.append([did, n, float(v.sum()), float(v.mean()), float(v.median()), float(v.min()), float(v.max())])
        else:
            rows.append([did, n, 0.0, "", "", "", ""])
    write_csv(FIX / "T1" / "expected_stats_in_areas.csv", ["district_id", "count", "sum", "mean", "median", "min", "max"], rows)
    print(f"wrote {len(rows)} rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
