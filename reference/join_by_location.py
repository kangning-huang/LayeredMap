"""Reference: join by location (A6, "join_by_location").

Definition (LayeredMap certified action "Add [area] details to each [feature]"):
  * each point (a multipoint: any of its points) takes the area it intersects (inside or on the
    boundary: GeoPandas sjoin, predicate="intersects"); when several do, the one listed first
    (lowest area row) is kept;
  * lines and areas are placed by shapely's `representative_point()` (N10's lines and areas lie
    wholly inside one area part, or wholly outside, so any point of them gives the same answer);
  * features with no geometry, and points in no area, get a blank area;
  * on_boundary (points and multipoints only): a point lies on the boundary of any area.
Output: CSV with columns layer, feature_id, area_id, on_boundary.

Usage:
  python -I reference/join_by_location.py                 # the N10 job
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import pandas as pd  # noqa: E402

N10 = FIX / "N10"
LAYERS = ["points", "lines", "polygons"]


def join(features: gpd.GeoDataFrame, areas: gpd.GeoDataFrame):
    areas = areas.reset_index(drop=True)
    areas["area_row"] = range(len(areas))
    valid = areas[areas.geometry.notna() & ~areas.geometry.is_empty]
    f = features.reset_index(drop=True).copy()
    f["feature_row"] = range(len(f))
    has = f.geometry.notna() & ~f.geometry.is_empty
    probe = f[has].copy()
    is_point = probe.geom_type.isin(["Point", "MultiPoint"])
    probe.loc[~is_point, "geometry"] = probe.loc[~is_point].geometry.representative_point()
    j = gpd.sjoin(probe[["feature_row", "geometry"]], valid[["area_row", "geometry"]], predicate="intersects", how="left")
    first = j.groupby("feature_row")["area_row"].min()
    boundaries = valid.geometry.boundary
    rows = []
    for _, r in f.iterrows():
        k = r["feature_row"]
        a = first.get(k)
        area_id = "" if a is None or pd.isna(a) else str(areas["area_id"].iloc[int(a)])
        on_b = ""
        g = r.geometry
        if g is not None and not g.is_empty and g.geom_type in ("Point", "MultiPoint"):
            on_b = "true" if bool(boundaries.intersects(g).any()) else "false"
        rows.append((str(r["feature_id"]), area_id, on_b))
    return rows


def main():
    areas = gpd.read_file(N10 / "areas.geojson", engine="pyogrio")
    out = []
    for layer in LAYERS:
        f = gpd.read_file(N10 / f"{layer}.geojson", engine="pyogrio")
        out.extend((layer, *r) for r in join(f, areas))
    write_csv(N10 / "expected_join.csv", ["layer", "feature_id", "area_id", "on_boundary"], out)
    matched = sum(1 for r in out if r[2])
    print(f"N10/expected_join.csv: {len(out)} features, {matched} in an area")


if __name__ == "__main__":
    main()
