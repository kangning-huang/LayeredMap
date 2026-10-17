"""Build the fixtures for "Find areas where" (N11) and "Keep / remove the part inside" (N12).

  * N11 — three small GeoTIFF cases for find_where: A (one geographic raster drawn by hand:
    holes, corner contacts, bays closed at a corner, an island touching a hole, NoData, values
    exactly on the thresholds, cells on the grid's edges); B (two geographic rasters on
    different grids: int16 with scale 0.1 and a uint8 category raster on a coarser, shifted
    grid); C (a projected raster, UTM zone 51N).
  * N12 — a mask layer (overlapping areas, a hole, a clockwise ring, a multipolygon) and
    area, line and point layers to keep or remove the part inside it.

Deterministic: fixed values, GDAL's GTiff writer with fixed options; rerunning gives identical
bytes. Expected numbers are written by `find_where.py` (N11) and `clip_erase.py` (N12).

Usage:  python -I reference/make_where_clip_fixtures.py
"""

from __future__ import annotations

import math
import os
import shutil
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common  # noqa: E402,F401  (must precede pyproj/rasterio imports)
from _common import FIX, write_geojson, write_json, write_text  # noqa: E402

import numpy as np  # noqa: E402
import rasterio  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402

MADE_BY = "Made by `reference/make_where_clip_fixtures.py` (synthetic, CC0-1.0)."

# ---- N11 ---------------------------------------------------------------------------------------

# Case A: '#' passes "between 10 and 20", '.' fails, 'x' is NoData; 'A' = 10.0 and 'B' = 20.0
# (pass, inclusive), 'C' = 9.999 and 'D' = 20.001 (fail).
A_PICTURE = [
    "........................",
    ".###..#....##..........#",
    ".#.#.#.#...#.#..#.......",
    ".###..#....###...#......",
    "..................#.....",
    "#######.................",
    "##....#......####..###..",
    "#.#...#......#.##..#x#..",
    "#.....#......##.#..###..",
    "#.....#......####..#.#..",
    "#.....#............###..",
    "#######..AB.............",
    "........CD.....#.#.#....",
    "#..............#.#.#....",
    "##.............#####....",
    "#.......#...............",
    "#......#.#..............",
    "#####...#..............#",
]
A_X0, A_Y1, A_RES = 100.0, 40.0, 0.5
ND_F32 = -9999.0

B_X0, B_Y1, B_RES, B_W, B_H = 110.0, 30.0, 0.25, 16, 12
B2_X0, B2_Y1, B2_RES, B2_W, B2_H = 109.83, 30.17, 0.6, 8, 6
B_ND_I16 = -32768
B_ND_U8 = 255

C_X0, C_Y1, C_RES, C_W, C_H = 350_000.0, 3_480_000.0, 2000.0, 12, 10


def fresh(fid: str) -> Path:
    """Empty fixtures/<fid>/ except the expected_*.csv the reference scripts own."""
    d = FIX / fid
    if d.exists():
        for p in d.iterdir():
            if p.name.startswith("expected_") and p.suffix == ".csv":
                continue
            if p.is_dir():
                shutil.rmtree(p, ignore_errors=True)
            else:
                p.unlink(missing_ok=True)
    d.mkdir(parents=True, exist_ok=True)
    return d


def a_values() -> np.ndarray:
    h, w = len(A_PICTURE), len(A_PICTURE[0])
    a = np.zeros((h, w), dtype=np.float32)
    for r, row in enumerate(A_PICTURE):
        assert len(row) == w, f"row {r} has {len(row)} cells"
        for c, ch in enumerate(row):
            if ch == "#":
                v = 10.0 + 10.0 * ((c * 7 + r * 3) % 11) / 10.0
            elif ch == ".":
                v = 25.0 if (c + r) % 2 else 5.0
            elif ch == "x":
                v = ND_F32
            else:
                v = {"A": 10.0, "B": 20.0, "C": 9.999, "D": 20.001}[ch]
            a[r, c] = v
    return a


def b_values():
    first = np.zeros((B_H, B_W), dtype=np.int16)
    for r in range(B_H):
        for c in range(B_W):
            t = 14.0 + 3.0 * math.sin(c / 2.3) * math.cos(r / 1.7) + 1.5 * math.sin((c + 2 * r) / 3.1)
            first[r, c] = int(round(t * 10))
    first[3, 4] = B_ND_I16
    first[8, 11] = B_ND_I16
    second = np.zeros((B2_H, B2_W), dtype=np.uint8)
    for r in range(B2_H):
        for c in range(B2_W):
            second[r, c] = 1 + (c * 3 + r * 5 + (c * r) % 3) % 4
    second[2, 3] = B_ND_U8
    return first, second


def c_values() -> np.ndarray:
    a = np.zeros((C_H, C_W), dtype=np.float32)
    for r in range(C_H):
        for c in range(C_W):
            a[r, c] = 0.5 * math.sin(c / 1.9) * math.cos(r / 1.4) + 0.3 * math.cos((c - r) / 2.2)
    a[4, 5] = ND_F32
    a[0, 11] = ND_F32
    return a


def write_tif(path: Path, data, transform, crs: str, nodata, scale=None):
    h, w = data.shape
    with rasterio.open(path, "w", driver="GTiff", width=w, height=h, count=1, dtype=str(data.dtype), crs=crs,
                       transform=transform, nodata=nodata) as dst:
        dst.write(data, 1)
        if scale is not None:
            dst.scales = (scale,)
            dst.offsets = (0.0,)


def make_n11():
    d = fresh("N11")
    a = a_values()
    write_tif(d / "a_shapes.tif", a, from_origin(A_X0, A_Y1, A_RES, A_RES), "EPSG:4326", ND_F32)
    first, second = b_values()
    write_tif(d / "b_first.tif", first, from_origin(B_X0, B_Y1, B_RES, B_RES), "EPSG:4326", B_ND_I16, scale=0.1)
    write_tif(d / "b_second.tif", second, from_origin(B2_X0, B2_Y1, B2_RES, B2_RES), "EPSG:4326", B_ND_U8)
    write_tif(d / "c_utm.tif", c_values(), from_origin(C_X0, C_Y1, C_RES, C_RES), "EPSG:32651", ND_F32)
    picture = "\n".join(A_PICTURE)
    write_json(d / "expected.json", {
        "fixture": "N11", "kind": "raster",
        "files": ["a_shapes.tif", "b_first.tif", "b_second.tif", "c_utm.tif"],
        "cases": {
            "A": {"rasters": ["a_shapes.tif"], "tests": ["between 10 and 20"]},
            "B": {"rasters": ["b_first.tif", "b_second.tif"], "tests": ["at least 15 (raw x 0.1)", "one of 2, 4"]},
            "C": {"rasters": ["c_utm.tif"], "tests": ["at most 0.3"]},
        },
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# N11 — Find areas where rasters meet conditions

Three cases for `lm_analysis::where_areas::find_where` (puzzles › mechanics › A1).

**A** `a_shapes.tif`: {len(A_PICTURE[0])} × {len(A_PICTURE)} cells of {A_RES}° from {A_X0}° E, {A_Y1}° N
(float32, NoData {ND_F32:g}). Condition: between 10 and 20 (inclusive). Drawn by hand
(`#` passes with values 10–20, `.` fails with 5 or 25, `x` NoData; `A` = 10.0 and `B` = 20.0
pass, `C` = 9.999 and `D` = 20.001 fail):

```
{picture}
```

It has a ring with a hole, four cells meeting only at corners around a gap (one area of
four parts; the gap is not a hole), a bay closed off at a corner (a hole touching the outline),
cells in a diagonal chain, a 7 × 7 ring whose inner corner cell touches an island at a corner
(the island is a part inside the hole), two holes meeting at a corner, a block with a NoData
hole and a value hole, a comb whose bays stay open, and cells on all four edges of the grid.

**B** `b_first.tif` ({B_W} × {B_H} cells of {B_RES}° from {B_X0}° E, {B_Y1}° N, int16, NoData
{B_ND_I16}, scale 0.1: tenths of a degree) with "at least 15", and `b_second.tif`
({B2_W} × {B2_H} cells of {B2_RES}° from {B2_X0}° E, {B2_Y1}° N, uint8 class codes 1–4, NoData
{B_ND_U8}) with "one of 2, 4". The second grid is coarser and shifted, so each first-grid cell
takes the class of the second-grid cell holding its centre (no centre lies on a cell edge).
Two first-grid cells and one second-grid cell are NoData. The scale is stored in the file's
metadata; LayeredMap's test gives it explicitly (`MemoryBand::with_scale_offset(0.1, 0.0)`).

**C** `c_utm.tif`: {C_W} × {C_H} cells of {C_RES:g} m in UTM zone 51N (EPSG:32651) from
x = {C_X0:g}, y = {C_Y1:g} (about 119.4° E, 31.4° N), float32 with two NoData cells.
Condition: at most 0.3.

Expected (from `reference/find_where.py`): `expected_find_where.csv` (one row per area:
`case`, `area` (1-based, raster order of the first cell), `cells`, `area_m2`, `geod_area_m2`,
`mean_1`, `mean_2`) and `expected_labels_<case>.csv` (the area number of every cell of the
first grid, 0 = out, one line per row). Definitions:

- cells in: numpy comparisons on values × scale + offset, NoData out; other rasters sampled at
  the first grid's cell centres by floor of the pixel position (through pyproj for a different
  coordinate system);
- areas: `scipy.ndimage.label` with a 3 × 3 structure;
- `area_m2`: the sum of cell areas on the WGS 84 ellipsoid: the closed latitude-band
  (authalic) formula per row for A and B; for C each cell's corners converted with pyproj and
  the quadrilateral straight in (λ, authalic q) measured by the shoelace formula;
- `geod_area_m2`: pyproj `Geod(ellps="WGS84").geometry_area_perimeter` of the polygons
  `rasterio.features.shapes` traces (connectivity 8), each oriented counter-clockwise, edges
  first densified to 1/64 cell in longitude/latitude (A, B: so the geodesic edges follow the
  parallels) or to one cell in UTM before conversion (C). The script asserts it is within
  1e-6 of `area_m2`;
- means: area-weighted, Σ x·A / Σ A over the area's cells, per raster (on the first grid).

Tolerances (mechanics › A1): identical cell sets and area counts; each area within 1e-9 of
`area_m2` and within 1e-6 of `geod_area_m2`.

{MADE_BY}
""")


# ---- N12 ---------------------------------------------------------------------------------------

X0, Y0 = 120.0, 30.0


def P(x, y):
    return [X0 + x, Y0 + y]


def rect(x0, y0, x1, y1, cw=False):
    r = [P(x0, y0), P(x1, y0), P(x1, y1), P(x0, y1), P(x0, y0)]
    return r[::-1] if cw else r


def feat(props, geom):
    return {"type": "Feature", "properties": props, "geometry": geom}


def poly(*rings):
    return {"type": "Polygon", "coordinates": list(rings)}


def mpoly(*polys):
    return {"type": "MultiPolygon", "coordinates": [list(p) for p in polys]}


def line(*pts):
    return {"type": "LineString", "coordinates": [P(*p) for p in pts]}


def mline(*parts):
    return {"type": "MultiLineString", "coordinates": [[P(*p) for p in part] for part in parts]}


def make_n12():
    d = fresh("N12")
    mask = [
        feat({"mask_id": "M1"}, poly(rect(0, 0, 4, 4), rect(1.5, 1.5, 2.5, 2.5, cw=True))),
        feat({"mask_id": "M2"}, poly(rect(3, 1, 6, 3, cw=True))),  # overlaps M1, written clockwise
        feat({"mask_id": "M3"}, mpoly([rect(8, 0, 9, 1)], [rect(8, 2, 9, 3)])),
    ]
    areas = [
        feat({"id": "A1", "name": "corner square"}, poly(rect(-1, -1, 1, 1))),
        feat({"id": "A2", "name": "holed square over the mask's hole"}, poly(rect(1, 1, 3, 3), rect(1.75, 1.75, 2.25, 2.25, cw=True))),
        feat({"id": "A3", "name": "two parts across M3"}, mpoly([rect(7.5, 0.5, 8.5, 2.5)], [rect(8.75, -0.5, 10, 0.5)])),
        feat({"id": "A4", "name": "outside"}, poly(rect(11, 5, 12, 6))),
        feat({"id": "A5", "name": "inside"}, poly(rect(0.25, 0.25, 1.25, 1.25))),
        feat({"id": "A6", "name": "slanted triangle"}, poly([P(2.3, 2.9), P(7.1, 1.7), P(5.3, 4.6), P(2.3, 2.9)])),
        feat({"id": "A7", "name": "sliver past M2's east edge"}, poly(rect(5.99999999, 1.5, 7, 2))),
        feat({"id": "A8", "name": "no geometry"}, None),
        feat({"id": "A9", "name": "clockwise, inside the mask's hole"}, poly(rect(1.6, 1.6, 2.4, 2.4, cw=True))),
    ]
    lines = [
        feat({"id": "L1", "name": "across M1 and M3"}, line((-1, 0.5), (10, 0.5))),
        feat({"id": "L2", "name": "through the hole"}, line((0.5, 2), (3.5, 2))),
        feat({"id": "L3", "name": "diagonal"}, line((-1, -0.5), (5, 5.5))),
        feat({"id": "L4", "name": "outside"}, line((11, 5), (12, 6))),
        feat({"id": "L5", "name": "two parts"}, mline([(8.5, 2.5), (10, 2.5)], [(12, 0), (13, 1)])),
        feat({"id": "L6", "name": "bent, in and out"}, line((5, 2), (7, 2), (7, 0.5), (8.5, 0.5))),
    ]
    points = [
        feat({"id": "P1", "name": "inside"}, {"type": "Point", "coordinates": P(0.5, 0.5)}),
        feat({"id": "P2", "name": "in the hole"}, {"type": "Point", "coordinates": P(2, 2)}),
        feat({"id": "P3", "name": "on the hole's edge"}, {"type": "Point", "coordinates": P(1.5, 2)}),
        feat({"id": "P4", "name": "on the outer edge"}, {"type": "Point", "coordinates": P(0, 2)}),
        feat({"id": "P5", "name": "in the overlap"}, {"type": "Point", "coordinates": P(3.5, 2)}),
        feat({"id": "P6", "name": "outside"}, {"type": "Point", "coordinates": P(7, 4)}),
        feat({"id": "P7", "name": "multipoint, one in one out"}, {"type": "MultiPoint", "coordinates": [P(8.5, 0.5), P(8.5, 1.5)]}),
        feat({"id": "P8", "name": "on M2's east edge"}, {"type": "Point", "coordinates": P(6, 2)}),
        feat({"id": "P9", "name": "on a corner"}, {"type": "Point", "coordinates": P(0, 0)}),
    ]
    write_geojson(d / "mask.geojson", mask, "mask")
    write_geojson(d / "areas.geojson", areas, "areas")
    write_geojson(d / "lines.geojson", lines, "lines")
    write_geojson(d / "points.geojson", points, "points")
    write_json(d / "expected.json", {
        "fixture": "N12", "kind": "vector",
        "files": ["mask.geojson", "areas.geojson", "lines.geojson", "points.geojson"],
        "layers": [
            {"file": "mask.geojson", "geometry": "area", "count": len(mask)},
            {"file": "areas.geojson", "geometry": "area", "count": len(areas)},
            {"file": "lines.geojson", "geometry": "line", "count": len(lines)},
            {"file": "points.geojson", "geometry": "point", "count": len(points)},
        ],
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# N12 — Keep or remove the part inside an area layer

Inputs for `lm_analysis::clip::keep_inside` and `remove_inside` (puzzles › mechanics › A2).
Coordinates are offsets from {X0:g}° E, {Y0:g}° N; every vertex is a multiple of 1/16°
except A6 (a slanted triangle) and A7 (a sliver 1e-8° wide).

- `mask.geojson` (`mask_id`): M1 the square [0, 4]² with the hole [1.5, 2.5]²; M2 [3, 6] × [1, 3],
  overlapping M1 and written clockwise; M3 a multipolygon of [8, 9] × [0, 1] and [8, 9] × [2, 3].
- `areas.geojson` (`id`): A1 straddles M1's corner; A2 a holed square over the mask's hole;
  A3 a two-part multipolygon across M3; A4 outside; A5 inside; A6 a slanted triangle across
  M1, M2 and beyond; A7 a sliver over M2's east edge; A8 no geometry; A9 a clockwise square
  inside the mask's hole.
- `lines.geojson` (`id`): L1 across M1 and M3; L2 through the hole; L3 a diagonal crossing
  M1's edge and the hole; L4 outside; L5 a two-part line; L6 a bent line in and out. No line
  runs along a mask edge (that case is tested in Rust).
- `points.geojson` (`id`): inside, in the hole, on the hole's edge, on the outer edge, in the
  overlap, outside, a multipoint with one point in and one out, on M2's east edge, and on
  M1's corner.

Expected (from `reference/clip_erase.py`): `expected_clip.csv`, one row per input feature and
operation (`keep`, `remove`): `kept` (whether a feature is left), `parts`, the geodesic
area (pyproj Geod, WGS 84, polygons oriented counter-clockwise) or length, and for points and
lines the exact coordinates left (`x y` pairs; lines as parts separated by `|`, each part
starting at its smaller end, parts sorted). Reference: GeoPandas `overlay` with
`how="intersection"` or `"difference"` against the mask dissolved into one feature.

Tolerances (mechanics › A2): same features kept; areas within 1e-6 relative (1 m² absolute for
slivers); lines and points exact.

{MADE_BY}
""")


def main():
    make_n11()
    make_n12()


if __name__ == "__main__":
    main()
