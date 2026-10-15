"""Reference: geodesic lengths, areas and buffer areas (N5).

Definitions (WGS 84 ellipsoid, pyproj.Geod):
  * length of a line = sum of geodesic distances between consecutive vertices
    (Multi*: sum over parts);
  * area / perimeter of a polygon = pyproj.Geod geometry_area_perimeter, edges
    are geodesics between the given vertices (Multi*: sum over parts). The
    perimeter of a polygon split at the antimeridian (RFC 7946 encoding) would
    include the artificial 180-degree seam, so it is left blank for such
    features;
  * buffer of a POINT = geodesic circle: 360 vertices from Geod.fwd at
    azimuths 0..359 deg, area via Geod.polygon_area_perimeter;
  * buffer of a LINE / AREA = densify along geodesics (<=100 m), project to a
    local azimuthal equidistant projection centred on the feature, shapely
    buffer (quad_segs=90, i.e. 360 vertices per full turn), densify the result
    (<=100 m), project back and measure with Geod.
Outputs (in the fixture folder):
  expected_measure.csv : feature_id, geom_type, length_m, area_m2, perimeter_m
  expected_buffer.csv  : feature_id, distance_m, buffer_area_m2

Usage:  python -I reference/geodesic.py [CASES_GEOJSON OUT_DIR]
"""

from __future__ import annotations

import json
import math
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, geod, write_csv  # noqa: E402

import numpy as np  # noqa: E402
import pyproj  # noqa: E402
import shapely  # noqa: E402
from shapely.geometry import LineString, Polygon, shape  # noqa: E402

G = geod()
DISTANCES = [1000.0, 50000.0]
DENSE_M = 100.0


def parts(geom):
    return list(geom.geoms) if hasattr(geom, "geoms") else [geom]


def crosses_antimeridian(geom) -> bool:
    xs = [x for p in parts(geom) for x in np.asarray(shapely.get_coordinates(p))[:, 0]]
    return max(xs) - min(xs) > 180 or (180.0 in xs and -180.0 in xs)


def densify_geodesic(coords):
    out = [tuple(coords[0])]
    for (x1, y1), (x2, y2) in zip(coords[:-1], coords[1:]):
        _, _, dist = G.inv(x1, y1, x2, y2)
        n = int(math.ceil(dist / DENSE_M)) - 1
        if n > 0:
            out.extend(G.npts(x1, y1, x2, y2, n))
        out.append((x2, y2))
    return out


def measure(fid, geom):
    gt = geom.geom_type
    length = area = perim = None
    if gt in ("LineString", "MultiLineString"):
        length = sum(G.line_length(*zip(*p.coords)) for p in parts(geom))
    elif gt in ("Polygon", "MultiPolygon"):
        a, p = G.geometry_area_perimeter(geom)
        area = abs(a)
        perim = None if (gt == "MultiPolygon" and crosses_antimeridian(geom)) else p
    return [fid, gt, length, area, perim]


def ring_area(lons, lats):
    a, _ = G.polygon_area_perimeter(lons, lats)
    return abs(a)


def buffer_area(geom, d):
    if geom.geom_type == "Point":
        lon0, lat0 = geom.x, geom.y
        az = np.arange(360, dtype=float)
        lons, lats, _ = G.fwd(np.full(360, lon0), np.full(360, lat0), az, np.full(360, d))
        return ring_area(lons, lats)
    # local AEQD centred on the feature
    pts = shapely.get_coordinates(geom)
    lat0 = float(np.mean(pts[:, 1]))
    lon0 = 180.0 if crosses_antimeridian(geom) else float(np.mean(pts[:, 0]))
    aeqd = pyproj.CRS.from_proj4(f"+proj=aeqd +lat_0={lat0} +lon_0={lon0} +ellps=WGS84 +units=m +no_defs")
    fwd = pyproj.Transformer.from_crs(4326, aeqd, always_xy=True)
    inv = pyproj.Transformer.from_crs(aeqd, 4326, always_xy=True)
    projected = []
    for p in parts(geom):
        if p.geom_type == "LineString":
            projected.append(LineString([fwd.transform(x, y) for x, y in densify_geodesic(list(p.coords))]))
        else:
            ext = densify_geodesic(list(p.exterior.coords))
            holes = [densify_geodesic(list(h.coords)) for h in p.interiors]
            projected.append(Polygon([fwd.transform(x, y) for x, y in ext],
                                     [[fwd.transform(x, y) for x, y in h] for h in holes]))
    buf = shapely.union_all([q.buffer(d, quad_segs=90) for q in projected])
    buf = shapely.segmentize(buf, DENSE_M)
    total = 0.0
    for poly in parts(buf):
        ll = shapely.transform(poly, lambda x, y: inv.transform(x, y), interleaved=False)
        lons, lats = zip(*ll.exterior.coords)
        total += ring_area(lons, lats)
        for h in ll.interiors:
            lons, lats = zip(*h.coords)
            total -= ring_area(lons, lats)
    return total


def run(cases: Path, out_dir: Path):
    fc = json.loads(cases.read_text("utf-8"))
    mrows, brows = [], []
    for f in fc["features"]:
        fid = f["properties"]["feature_id"]
        g = shape(f["geometry"])
        mrows.append(measure(fid, g))
        for d in DISTANCES:
            brows.append([fid, d, buffer_area(g, d)])
    write_csv(out_dir / "expected_measure.csv", ["feature_id", "geom_type", "length_m", "area_m2", "perimeter_m"],
              mrows)
    write_csv(out_dir / "expected_buffer.csv", ["feature_id", "distance_m", "buffer_area_m2"], brows)
    return mrows, brows


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv:
        run(Path(argv[0]), Path(argv[1]))
        return
    m, b = run(FIX / "N5" / "cases.geojson", FIX / "N5")
    print(f"N5/expected_measure.csv: {len(m)} rows; N5/expected_buffer.csv: {len(b)} rows")


if __name__ == "__main__":
    main()
