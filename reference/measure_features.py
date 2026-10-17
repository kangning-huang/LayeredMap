"""Reference: per-feature measurements (A4, "add_measurements").

Definition (LayeredMap certified action "Add area and coordinates to each [feature]"):
  * areas: geodesic area and perimeter on WGS 84, pyproj `Geod.geometry_area_perimeter` after
    orienting every polygon counter-clockwise with clockwise holes (so the area is positive and
    holes subtract). The perimeter includes every ring, holes too (as shapely's `length`, QGIS's
    $perimeter and `lm_analysis::geodesic` count it): pyproj's own perimeter leaves holes out, so
    each hole's ring perimeter (`Geod.polygon_area_perimeter`) is added. A polygon split at the
    antimeridian gets no perimeter (its 180° seam is not boundary; LayeredMap leaves it out);
  * lines: geodesic length, `Geod.geometry_length` (the parts add up);
  * points: the input coordinates (a multipoint: its first point).
Output: CSV with columns feature_id, area_km2, perimeter_km, length_km, lon, lat.

Usage:
  python -I reference/measure_features.py                 # the N11 job
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import pyproj  # noqa: E402
import shapely  # noqa: E402

GEOD = pyproj.Geod(ellps="WGS84")
N11 = FIX / "N11"


def touches_seam(g) -> bool:
    polys = [g] if g.geom_type == "Polygon" else list(g.geoms)
    for p in polys:
        c = list(p.exterior.coords)
        if any(abs(a[0]) == 180 and abs(b[0]) == 180 for a, b in zip(c[:-1], c[1:])):
            return True
    return False


def measure(g):
    if g.geom_type in ("Polygon", "MultiPolygon"):
        a, p = GEOD.geometry_area_perimeter(shapely.orient_polygons(g, exterior_cw=False))
        for part in ([g] if g.geom_type == "Polygon" else list(g.geoms)):
            for hole in part.interiors:
                x, y = hole.xy
                p += GEOD.polygon_area_perimeter(list(x), list(y))[1]
        return (abs(a) / 1e6, None if touches_seam(g) else p / 1000, None, None, None)
    if g.geom_type in ("LineString", "MultiLineString"):
        return (None, None, GEOD.geometry_length(g) / 1000, None, None)
    p = g if g.geom_type == "Point" else g.geoms[0]
    return (None, None, None, p.x, p.y)


def main():
    rows = []
    for layer in ["areas", "lines", "points"]:
        f = gpd.read_file(N11 / f"{layer}.geojson", engine="pyogrio")
        for fid, g in zip(f["feature_id"], f.geometry):
            rows.append((str(fid), *measure(g)))
    write_csv(N11 / "expected_measure.csv", ["feature_id", "area_km2", "perimeter_km", "length_km", "lon", "lat"], rows)
    print(f"N11/expected_measure.csv: {len(rows)} features")


if __name__ == "__main__":
    main()
