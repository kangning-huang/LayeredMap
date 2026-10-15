"""Reference: for each point, list every area containing it.

Definition (LayeredMap certified action "Which area?"):
  * GeoPandas sjoin predicate="intersects": a point on a shared boundary is in
    BOTH (all) touching areas; a point in an overlap is in both areas;
  * points in no area are kept with an empty list.
Output columns: point_id, area_ids (sorted, ';'-separated), n_areas.
Points are listed in input order.

Usage:
  python -I reference/which_area.py                    # N1
  python -I reference/which_area.py AREAS POINTS AREA_ID POINT_ID OUT
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402


def which_area(areas_path: Path, points_path: Path, area_id: str, point_id: str, out: Path):
    areas = gpd.read_file(areas_path, engine="pyogrio")[[area_id, "geometry"]]
    pts = gpd.read_file(points_path, engine="pyogrio")[[point_id, "geometry"]]
    j = gpd.sjoin(pts, areas, predicate="intersects", how="left")
    rows = []
    for pid in pts[point_id]:
        hits = j.loc[j[point_id] == pid, area_id].dropna().astype(str)
        ids = sorted(set(hits))
        rows.append([str(pid), ";".join(ids), len(ids)])
    write_csv(out, ["point_id", "area_ids", "n_areas"], rows)
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("args", nargs="*")
    a = ap.parse_args(argv)
    if a.args:
        ar, pt, aid, pid, out = a.args
        which_area(Path(ar), Path(pt), aid, pid, Path(out))
        return
    rows = which_area(FIX / "N1" / "areas.geojson", FIX / "N1" / "points.geojson", "area_id", "point_id",
                      FIX / "N1" / "expected_which_area.csv")
    print(f"N1/expected_which_area.csv: {len(rows)} points, {sum(r[2] > 1 for r in rows)} in >1 area")


if __name__ == "__main__":
    main()
