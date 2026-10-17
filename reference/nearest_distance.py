"""Reference: distance to the nearest target feature (A3, "nearest_distance").

Definition (LayeredMap certified action "Add the distance to the nearest [other]"):
  * 0 when the feature intersects a target (shapely `intersects` in lon/lat: the app's planar
    convention for touching, containment and crossing);
  * otherwise the feature and every target are densified along geodesics (pyproj Geod, WGS 84)
    to steps of at most 1 km, projected to an azimuthal equidistant projection centred on the
    feature (circular mean of its vertices), and the distance is shapely `distance` between the
    projected feature and the projected targets' boundaries (rings as lines);
  * same layer: the feature's own row (all its parts) is left out;
  * the nearest target is the one with the smallest distance, ties to the lower row.
Output: CSV with columns feature_id, nearest_id, distance_m (in feature order).

Usage:
  python -I reference/nearest_distance.py                       # all fixture jobs
  python -I reference/nearest_distance.py FEATURES ID TARGETS TID OUT [--same]
"""

from __future__ import annotations

import argparse
import math
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import pyproj  # noqa: E402
import shapely  # noqa: E402
from shapely.geometry import LineString, Point  # noqa: E402

GEOD = pyproj.Geod(ellps="WGS84")
STEP = 1000.0


def densify_path(coords, close=False):
    pts = [tuple(c[:2]) for c in coords]
    if close and len(pts) > 2 and pts[0] != pts[-1]:
        pts.append(pts[0])
    out = [pts[0]] if pts else []
    for (x0, y0), (x1, y1) in zip(pts[:-1], pts[1:]):
        az, _, s = GEOD.inv(x0, y0, x1, y1)
        n = max(1, math.ceil(s / STEP))
        for k in range(1, n):
            x, y, _ = GEOD.fwd(x0, y0, az, s * k / n)
            out.append((x, y))
        out.append((x1, y1))
    return out


def paths(g):
    """The geometry as a list of (coords, kind): kind 'point', 'line' or 'ring'."""
    if g.geom_type == "Point":
        return [([g.coords[0]], "point")]
    if g.geom_type == "MultiPoint":
        return [([p.coords[0]], "point") for p in g.geoms]
    if g.geom_type == "LineString":
        return [(list(g.coords), "line")]
    if g.geom_type == "MultiLineString":
        return [(list(p.coords), "line") for p in g.geoms]
    polys = [g] if g.geom_type == "Polygon" else list(g.geoms)
    out = []
    for p in polys:
        out.append((list(p.exterior.coords), "ring"))
        out.extend((list(r.coords), "ring") for r in p.interiors)
    return out


def center(g):
    xs, ys, lats = [], [], []
    for coords, _ in paths(g):
        for x, y in coords:
            xs.append(math.cos(math.radians(x)))
            ys.append(math.sin(math.radians(x)))
            lats.append(y)
    return math.degrees(math.atan2(sum(ys), sum(xs))), sum(lats) / len(lats)


def projected(g, tr):
    """Densified, projected boundary geometry (points stay points, rings become lines)."""
    parts = []
    for coords, kind in paths(g):
        if kind == "point" or len(coords) == 1:
            x, y = tr.transform(*coords[0][:2])
            parts.append(Point(x, y))
            continue
        d = densify_path(coords, close=(kind == "ring"))
        xs, ys = tr.transform([c[0] for c in d], [c[1] for c in d])
        parts.append(LineString(list(zip(xs, ys))))
    return shapely.union_all(parts) if len(parts) > 1 else parts[0]


def nearest(features, fid, targets, tid, same):
    rows = []
    tg = list(targets.geometry)
    for i, g in enumerate(features.geometry):
        if g is None or g.is_empty:
            rows.append((str(features[fid].iloc[i]), "", None))
            continue
        lon0, lat0 = center(g)
        tr = pyproj.Transformer.from_crs(
            "EPSG:4326", f"+proj=aeqd +lat_0={lat0!r} +lon_0={lon0!r} +ellps=WGS84 +units=m +no_defs", always_xy=True)
        best = (math.inf, None)
        fp = None
        for j, t in enumerate(tg):
            if (same and j == i) or t is None or t.is_empty:
                continue
            if g.intersects(t):
                d = 0.0
            else:
                fp = fp if fp is not None else projected(g, tr)
                d = fp.distance(projected(t, tr))
            if d < best[0]:
                best = (d, j)
        if best[1] is None:
            rows.append((str(features[fid].iloc[i]), "", None))
        else:
            rows.append((str(features[fid].iloc[i]), str(targets[tid].iloc[best[1]]), best[0]))
    return rows


def run(features_path, fid, targets_path, tid, out, same=False):
    f = gpd.read_file(features_path, engine="pyogrio")
    t = f if same else gpd.read_file(targets_path, engine="pyogrio")
    rows = nearest(f, fid, t, tid, same)
    write_csv(out, ["feature_id", "nearest_id", "distance_m"], rows)
    return rows


N9 = FIX / "N9"
JOBS = [
    (N9 / "islands.geojson", "island_id", N9 / "islands.geojson", "island_id", N9 / "expected_nearest_islands_same.csv", True),
    (N9 / "points.geojson", "point_id", N9 / "points.geojson", "point_id", N9 / "expected_nearest_points_same.csv", True),
    (N9 / "points.geojson", "point_id", N9 / "islands.geojson", "island_id", N9 / "expected_nearest_points_to_islands.csv", False),
    (N9 / "points.geojson", "point_id", N9 / "lines.geojson", "line_id", N9 / "expected_nearest_points_to_lines.csv", False),
    (N9 / "lines.geojson", "line_id", N9 / "islands.geojson", "island_id", N9 / "expected_nearest_lines_to_islands.csv", False),
    (N9 / "islands.geojson", "island_id", N9 / "points.geojson", "point_id", N9 / "expected_nearest_islands_to_points.csv", False),
]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("features", nargs="?")
    ap.add_argument("fid", nargs="?")
    ap.add_argument("targets", nargs="?")
    ap.add_argument("tid", nargs="?")
    ap.add_argument("out", nargs="?")
    ap.add_argument("--same", action="store_true")
    a = ap.parse_args(argv)
    if a.features:
        run(Path(a.features), a.fid, Path(a.targets), a.tid, Path(a.out), a.same)
        return
    for job in JOBS:
        rows = run(*job)
        zeros = sum(1 for r in rows if r[2] == 0.0)
        print(f"{job[4].relative_to(FIX)}: {len(rows)} features, {zeros} touching")


if __name__ == "__main__":
    main()
