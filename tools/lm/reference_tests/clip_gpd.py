"""Reference for "Keep data inside an area" (N15; docs/lm/actions-p2.md › Keep data inside an area).

Definition (lm_analysis::clip::keep_inside must match it): GeoPandas `clip(gdf, zone,
keep_geom_type=True)`. The mask is the union of the zone layer's polygons, edges straight in
lon/lat; each feature becomes its intersection with the mask, keeping only its own kind of
geometry, so an area that only shares an edge or a corner with the mask, or a line that only
touches it, is left with nothing and dropped. Points on the boundary are kept. Features with
no geometry are dropped.

Written per input feature: whether it is kept, its parts, its geodesic area (pyproj Geod, WGS 84,
polygons oriented counter-clockwise) or length, and for lines and points the coordinates left.
Lines are merged with `shapely.line_merge` first (GEOS splits a line where it meets the mask's
boundary), each part written from its smaller end, parts sorted, `|` between parts.

Usage:  python -I tools/lm/reference_tests/clip_gpd.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "reference"))
import _common  # noqa: E402,F401  (must precede pyproj imports)
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import shapely  # noqa: E402
from pyproj import Geod  # noqa: E402
from shapely.geometry import LineString, MultiLineString, MultiPolygon, Point, Polygon  # noqa: E402
from shapely.geometry.polygon import orient  # noqa: E402

GEOD = Geod(ellps="WGS84")


def xy(c):
    return f"{c[0]!r} {c[1]!r}"


def describe(g):
    """(parts, area_m2, length_m, coords) of a result geometry."""
    if g is None or g.is_empty:
        return 0, None, None, ""
    if isinstance(g, (Polygon, MultiPolygon)):
        polys = [g] if isinstance(g, Polygon) else list(g.geoms)
        area = sum(GEOD.geometry_area_perimeter(orient(p, 1.0))[0] for p in polys)
        return len(polys), area, None, ""
    if isinstance(g, (LineString, MultiLineString)):
        g = shapely.line_merge(g) if isinstance(g, MultiLineString) else g
        lines = [g] if isinstance(g, LineString) else list(g.geoms)
        parts = []
        for ln in lines:
            cs = list(ln.coords)
            if tuple(cs[-1]) < tuple(cs[0]):
                cs = cs[::-1]
            parts.append(cs)
        parts.sort()
        length = sum(GEOD.geometry_length(ln) for ln in lines)
        return len(parts), None, length, "|".join(";".join(xy(c) for c in p) for p in parts)
    pts = [g] if isinstance(g, Point) else list(g.geoms)
    cs = sorted((p.x, p.y) for p in pts)
    return len(cs), None, None, ";".join(xy(c) for c in cs)


def main():
    d = FIX / "N15"
    zone = gpd.read_file(d / "zone.geojson", engine="pyogrio")
    rows, summary = [], []
    for layer in ["areas", "lines", "points"]:
        gdf = gpd.read_file(d / f"{layer}.geojson", engine="pyogrio")
        out = gpd.clip(gdf, zone, keep_geom_type=True)
        by_id = {r["id"]: r.geometry for _, r in out.iterrows() if r.geometry is not None and not r.geometry.is_empty}
        kept = 0
        for _, f in gdf.iterrows():
            parts, area, length, coords = describe(by_id.get(f["id"]))
            kept += parts > 0
            rows.append([layer, f["id"], "true" if parts > 0 else "false", parts, area, length, coords])
        summary.append(f"{layer}: {kept} of {len(gdf)} kept")
    write_csv(d / "expected_clip.csv", ["layer", "id", "kept", "parts", "geod_area_m2", "length_m", "coords"], rows)
    print("N15 " + ", ".join(summary))


if __name__ == "__main__":
    main()
