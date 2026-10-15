"""Reference: count points in each area.

Definition (LayeredMap certified action "Count in areas"):
  * a point on an area's boundary counts in EVERY area that touches it
    (GeoPandas sjoin, predicate="intersects");
  * overlapping areas each count their points;
  * areas with zero points stay in the output with count 0;
  * points with blank / unparseable coordinates are excluded.
Output: CSV with columns area_id, count (sorted by area_id).

Usage:
  python -I reference/count_in_areas.py                 # all fixture jobs
  python -I reference/count_in_areas.py AREAS POINTS AREA_ID OUT [--lon COL --lat COL]
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import pandas as pd  # noqa: E402


def read_points(path: Path, lon: str = "lon", lat: str = "lat") -> gpd.GeoDataFrame:
    if path.suffix.lower() == ".csv":
        df = pd.read_csv(path, dtype=str, keep_default_na=False)
        x = pd.to_numeric(df[lon].str.strip(), errors="coerce")
        y = pd.to_numeric(df[lat].str.strip(), errors="coerce")
        ok = x.notna() & y.notna()
        df = df[ok].copy()
        return gpd.GeoDataFrame(df, geometry=gpd.points_from_xy(x[ok], y[ok]), crs="EPSG:4326")
    return gpd.read_file(path, engine="pyogrio")


def count_in_areas(areas_path: Path, points_path: Path, area_id: str, out: Path,
                   lon: str = "lon", lat: str = "lat") -> pd.DataFrame:
    areas = gpd.read_file(areas_path, engine="pyogrio")[[area_id, "geometry"]]
    pts = read_points(points_path, lon, lat)
    pts = pts[~pts.geometry.is_empty & pts.geometry.notna()]
    j = gpd.sjoin(pts[["geometry"]], areas, predicate="intersects", how="inner")
    counts = j.groupby(area_id).size()
    ids = sorted(areas[area_id].astype(str))
    res = pd.DataFrame({"area_id": ids, "count": [int(counts.get(i, 0)) for i in ids]})
    write_csv(out, ["area_id", "count"], res.itertuples(index=False, name=None))
    return res


JOBS = [
    (FIX / "N1" / "areas.geojson", FIX / "N1" / "points.geojson", "area_id", FIX / "N1" / "expected_count_in_areas.csv"),
    (FIX / "N2" / "areas.geojson", FIX / "N2" / "points.geojson", "area_id", FIX / "N2" / "expected_count_in_areas.csv"),
    # V1 districts x T1 schools (K-style sanity); 6 blank-coordinate rows excluded
    (FIX / "V1" / "districts.geojson", FIX / "T1" / "schools.csv", "district_id", FIX / "T1" / "expected_count_in_areas.csv"),
]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("areas", nargs="?")
    ap.add_argument("points", nargs="?")
    ap.add_argument("area_id", nargs="?")
    ap.add_argument("out", nargs="?")
    ap.add_argument("--lon", default="lon")
    ap.add_argument("--lat", default="lat")
    a = ap.parse_args(argv)
    if a.areas:
        count_in_areas(Path(a.areas), Path(a.points), a.area_id, Path(a.out), a.lon, a.lat)
        return
    for job in JOBS:
        res = count_in_areas(*job)
        print(f"{job[3].relative_to(FIX)}: {len(res)} areas, total {int(res['count'].sum())}")


if __name__ == "__main__":
    main()
