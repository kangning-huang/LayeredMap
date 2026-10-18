"""Reference for "Keep / remove the part of [layer] inside [other]" (N12; mechanics › A2).

Definition (the Rust implementation, lm_analysis::clip, must match it): the mask is the union
of the mask layer's polygons (here: GeoPandas `dissolve`), edges straight in lon/lat (planar,
as GEOS works); keep = GeoPandas `overlay(how="intersection")`, remove =
`overlay(how="difference")`, keep_geom_type=True, attributes copied; features left empty are
dropped. Points on the mask's boundary intersect it (kept by keep, removed by remove).

Written per input feature and operation: whether it is left, its parts, its geodesic area
(pyproj Geod, WGS 84, polygons oriented counter-clockwise) or length, and for lines and
points the exact coordinates left (lines: each part starting at its smaller end, parts sorted,
`|` between parts).

Usage:  python -I reference/clip_erase.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common  # noqa: E402,F401  (must precede pyproj/rasterio imports)
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
from pyproj import Geod  # noqa: E402
from shapely.geometry import LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon  # noqa: E402
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
    d = FIX / "N12"
    mask = gpd.read_file(d / "mask.geojson")
    union = mask[["geometry"]].dissolve()
    rows = []
    summary = []
    for layer in ["areas", "lines", "points"]:
        gdf = gpd.read_file(d / f"{layer}.geojson")
        for op, how in [("keep", "intersection"), ("remove", "difference")]:
            with_geom = gdf[gdf.geometry.notna() & ~gdf.geometry.is_empty]
            out = gpd.overlay(with_geom, union, how=how, keep_geom_type=True)
            by_id = {r["id"]: r.geometry for _, r in out.iterrows() if r.geometry is not None and not r.geometry.is_empty}
            kept = 0
            for _, f in gdf.iterrows():
                g = by_id.get(f["id"])
                parts, area, length, coords = describe(g)
                kept += parts > 0
                rows.append([op, layer, f["id"], "true" if parts > 0 else "false", parts, area, length, coords])
            summary.append(f"{op} {layer}: {kept} of {len(gdf)}")
    write_csv(d / "expected_clip.csv", ["op", "layer", "id", "kept", "parts", "geod_area_m2", "length_m", "coords"], rows)
    print("N12 " + ", ".join(summary))


if __name__ == "__main__":
    main()
