"""Reference for "Nearest feature" (N14; docs/lm/actions-p2.md › Nearest feature).

Definition (lm_analysis::nearest::nearest_distance must match it):

  * the nearest target is GeoPandas `sjoin_nearest(how="left")` with both layers projected to a
    local projection, UTM zone 51N (EPSG:32651); 0 when a feature touches or lies inside a target;
  * the distance is geodesic: shapely `nearest_points` of the feature and that target in the
    projection, taken back to lon/lat, then pyproj `Geod.inv` (WGS 84) between them. The script
    asserts it is within 0.1% (or 1 m) of the projected distance, and that the next target is at
    least 1% farther (so the answer is the same on the ellipsoid as in the projection).

Output: one CSV per pairing, columns feature_id, nearest_id, distance_m (geodesic), projected_m,
runner_up_ratio (the next target's projected distance over the nearest's; blank when 0).

Usage:  python -I tools/lm/reference_tests/nearest_sjoin.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "reference"))
import _common  # noqa: E402,F401  (must precede pyproj imports)
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import pyproj  # noqa: E402
from shapely.ops import nearest_points  # noqa: E402

GEOD = pyproj.Geod(ellps="WGS84")
LOCAL = "EPSG:32651"
BACK = pyproj.Transformer.from_crs(LOCAL, "EPSG:4326", always_xy=True)


def run(features, fid, targets, tid, out):
    d = FIX / "N14"
    f = gpd.read_file(d / features, engine="pyogrio").to_crs(LOCAL)
    t = gpd.read_file(d / targets, engine="pyogrio").to_crs(LOCAL)
    j = gpd.sjoin_nearest(f, t[[tid, "geometry"]], how="left", distance_col="projected_m")
    assert not j.index.duplicated().any(), f"{out}: a tie in sjoin_nearest"
    rows = []
    for i, r in j.iterrows():
        g = f.geometry.loc[i]
        target = t.loc[t[tid] == r[tid]].geometry.iloc[0]
        others = sorted(g.distance(x) for x in t.geometry if x is not target)
        a, b = nearest_points(g, target)
        lon1, lat1 = BACK.transform(a.x, a.y)
        lon2, lat2 = BACK.transform(b.x, b.y)
        geod = 0.0 if r["projected_m"] == 0.0 else GEOD.inv(lon1, lat1, lon2, lat2)[2]
        proj = float(r["projected_m"])
        assert abs(geod - proj) <= max(1.0, 1e-3 * geod), f"{r[fid]}: projected {proj} vs geodesic {geod}"
        ratio = None if proj == 0.0 else others[0] / proj
        assert ratio is None or ratio >= 1.01, f"{r[fid]}: runner-up only {ratio}"
        rows.append([r[fid], r[tid], geod, proj, ratio])
    write_csv(d / out, ["feature_id", "nearest_id", "distance_m", "projected_m", "runner_up_ratio"], rows)
    return rows


JOBS = [
    ("sources.geojson", "source_id", "facilities.geojson", "facility_id", "expected_nearest_sources_facilities.csv"),
    ("sources.geojson", "source_id", "roads.geojson", "road_id", "expected_nearest_sources_roads.csv"),
    ("sources.geojson", "source_id", "parks.geojson", "park_id", "expected_nearest_sources_parks.csv"),
    ("parks.geojson", "park_id", "facilities.geojson", "facility_id", "expected_nearest_parks_facilities.csv"),
]


def main():
    for job in JOBS:
        rows = run(*job)
        zeros = sum(1 for r in rows if r[2] == 0.0)
        print(f"N14/{job[4]}: {len(rows)} features, {zeros} touching")


if __name__ == "__main__":
    main()
