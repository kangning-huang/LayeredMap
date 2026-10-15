"""Reference: GCJ-02 / BD-09 test vectors (C2, library half).

WGS84 -> GCJ-02 and GCJ-02 -> BD-09 computed by two independent open-source
implementations, plus a third (eviltransform) as a cross-check:
  A = prcoords 1.0.2       (PyPI, CC0-1.0 / GPL-3.0-or-later dual)
  B = coord-convert 0.2.1  (PyPI, MIT)
  C = eviltransform 0.1.1  (PyPI, MIT; cross-check only)
BD-09 for both implementations is computed from implementation A's GCJ-02, so
the BD-09 columns compare the GCJ->BD step alone.
Writes fixtures/C2/vectors.csv, fixtures/C2/README.md, fixtures/C2/landmarks_TODO.csv.

Usage:  python -I reference/coord_vectors.py
"""

from __future__ import annotations

import os
import sys
import warnings
from importlib.metadata import version

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv, write_text  # noqa: E402

import numpy as np  # noqa: E402
import prcoords  # noqa: E402
import eviltransform  # noqa: E402
from coord_convert import transform as cc  # noqa: E402

CHINA = [
    ("Beijing Tiananmen", 116.397477, 39.908692), ("Shanghai People's Square", 121.473701, 31.230416),
    ("Guangzhou", 113.264385, 23.129112), ("Shenzhen", 114.057868, 22.543099),
    ("Chengdu", 104.066541, 30.572269), ("Chongqing", 106.551557, 29.563009),
    ("Wuhan", 114.305393, 30.593099), ("Xi'an", 108.940175, 34.341568),
    ("Hangzhou", 120.155070, 30.274085), ("Nanjing", 118.796877, 32.060255),
    ("Tianjin", 117.200983, 39.084158), ("Shenyang", 123.431475, 41.805698),
    ("Harbin", 126.534967, 45.803775), ("Changchun", 125.323544, 43.817071),
    ("Kunming", 102.832891, 24.880095), ("Lhasa", 91.140856, 29.645554),
    ("Urumqi", 87.616848, 43.825592), ("Kashgar", 75.989755, 39.470400),
    ("Hohhot", 111.749180, 40.842585), ("Lanzhou", 103.834303, 36.061089),
    ("Xining", 101.778228, 36.617144), ("Yinchuan", 106.230909, 38.487193),
    ("Nanning", 108.366543, 22.817002), ("Haikou", 110.198293, 20.044001),
    ("Sanya", 109.511909, 18.252847), ("Fuzhou", 119.296494, 26.074508),
    ("Xiamen", 118.089425, 24.479834), ("Jinan", 117.120098, 36.651200),
    ("Qingdao", 120.382640, 36.067082), ("Zhengzhou", 113.625368, 34.746600),
    ("Changsha", 112.938814, 28.228209), ("Nanchang", 115.858198, 28.682892),
    ("Hefei", 117.227239, 31.820587), ("Taiyuan", 112.548879, 37.870590),
    ("Shijiazhuang", 114.514859, 38.042306), ("Guiyang", 106.630153, 26.647661),
    ("Mohe", 122.538800, 52.972100), ("Zengmu Ansha area", 112.283333, 3.966667),
]
OUTSIDE = [("Tokyo", 139.767125, 35.681236), ("London", -0.127758, 51.507351),
           ("New York", -74.005941, 40.712784), ("Sydney", 151.209296, -33.868820),
           ("Moscow", 37.617300, 55.755826), ("Cape Town", 18.424055, -33.924869),
           ("Reykjavik", -21.817439, 64.126521), ("Honolulu", -157.858333, 21.306944)]


def main():
    rng = np.random.default_rng(202)
    pts = [(n, lo, la, "china") for n, lo, la in CHINA]
    for i in range(50 - len(CHINA)):
        pts.append((f"Grid sample {i + 1:02d}", round(float(rng.uniform(98.0, 122.0)), 6),
                    round(float(rng.uniform(22.0, 42.0)), 6), "china"))
    pts += [(n, lo, la, "outside") for n, lo, la in OUTSIDE]

    rows, max_gcj, max_bd, max_c = [], 0.0, 0.0, 0.0
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for i, (name, lon, lat, region) in enumerate(pts, 1):
            a = prcoords.wgs_gcj((lat, lon))
            b_lon, b_lat = cc.wgs2gcj(lon, lat)
            c_lat, c_lon = eviltransform.wgs2gcj(lat, lon)
            abd = prcoords.gcj_bd(a)
            bbd_lon, bbd_lat = cc.gcj2bd(a.lon, a.lat)
            max_gcj = max(max_gcj, abs(a.lon - b_lon), abs(a.lat - b_lat))
            max_c = max(max_c, abs(a.lon - c_lon), abs(a.lat - c_lat))
            max_bd = max(max_bd, abs(abd.lon - bbd_lon), abs(abd.lat - bbd_lat))
            if region == "outside":
                assert (a.lon, a.lat) == (lon, lat) == (b_lon, b_lat), name
            rows.append([f"{i:02d}", name, region, f"{lon:.6f}", f"{lat:.6f}",
                         f"{a.lon:.9f}", f"{a.lat:.9f}", f"{b_lon:.9f}", f"{b_lat:.9f}",
                         f"{abd.lon:.9f}", f"{abd.lat:.9f}", f"{bbd_lon:.9f}", f"{bbd_lat:.9f}"])
    assert max_gcj < 1e-8 and max_bd < 1e-8 and max_c < 1e-6, (max_gcj, max_bd, max_c)
    d = FIX / "C2"
    write_csv(d / "vectors.csv",
              ["id", "name", "region", "wgs84_lon", "wgs84_lat",
               "gcj02_lon_prcoords", "gcj02_lat_prcoords", "gcj02_lon_coordconvert", "gcj02_lat_coordconvert",
               "bd09_lon_prcoords", "bd09_lat_prcoords", "bd09_lon_coordconvert", "bd09_lat_coordconvert"], rows)
    write_csv(d / "landmarks_TODO.csv", ["name", "osm_node_id", "wgs84_lon", "wgs84_lat", "amap_lon", "amap_lat"], [])
    vers = {p: version(p) for p in ("prcoords", "coord-convert", "eviltransform")}
    write_text(d / "README.md", f"""# C2 — GCJ-02 / BD-09 test vectors

## `vectors.csv` (library half — done)

{len(pts)} points: {len(pts) - len(OUTSIDE)} in China ({len(CHINA)} named cities/places incl. far
north-east (Mohe), far west (Kashgar), far south (Sanya, Zengmu Ansha area);
{50 - len(CHINA)} random points in 98–122°E, 22–42°N, seed 202) and {len(OUTSIDE)} outside
China, where GCJ-02 must be the identity (asserted).

Columns: WGS 84 input; GCJ-02 from implementation A and B; BD-09 from A and B
(both computed from A's GCJ-02, so they test the GCJ-02 → BD-09 step alone).
Values printed with 9 decimals (~0.1 mm). For the outside-China points the BD-09
columns are just the raw GCJ→BD formula applied to the unshifted input (neither
library has an out-of-China rule for that step); they are not meaningful positions.

| impl | package | version | license |
|---|---|---|---|
| A | `prcoords` (github.com/Artoria2e5/PRCoords) | {vers['prcoords']} | CC0-1.0 / GPL-3.0-or-later (dual) |
| B | `coord-convert` (github.com/sshuair/coord-convert) | {vers['coord-convert']} | MIT |
| C | `eviltransform` (github.com/googollee/eviltransform), cross-check only | {vers['eviltransform']} | MIT |

Agreement: max |A − B| GCJ-02 = {max_gcj:.1e}°, max |A − B| BD-09 = {max_bd:.1e}°,
max |A − C| GCJ-02 = {max_c:.1e}° (A/B: floating-point level; C differs by ~1 cm,
from slightly different constants/arithmetic — well below any map use). All three
derive from the same widely circulated reverse-engineered algorithm, so they
are independent *implementations*, not independent *ground truth* — the
hand-read Amap landmarks below supply that.

Caveat: all three libraries use the same crude rectangle as "China"
(72.004–137.8347°E, 0.8293–55.8271°N). Points such as Seoul, Hanoi or Taipei
fall inside that rectangle and **are** shifted by these libraries; none are
included here. LayeredMap's own in/out-of-China policy should be decided
separately.

## `landmarks_TODO.csv` (owner's half — TODO)

Headers only. The owner should hand-read ~10 Amap landmark positions and fill
`name, osm_node_id, wgs84_lon, wgs84_lat` (from the OSM node) and
`amap_lon, amap_lat` (read from Amap). These are the independent ground truth
for the GCJ-02 conversion.

Made by `reference/coord_vectors.py`.
""")
    print(f"C2/vectors.csv: {len(rows)} points; max diff gcj={max_gcj:.1e} bd={max_bd:.1e} evil={max_c:.1e}")


if __name__ == "__main__":
    main()
