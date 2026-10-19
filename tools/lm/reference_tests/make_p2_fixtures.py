"""Build the WP6.4 fixtures for the P2 actions (docs/lm/actions-p2.md):

J6   match a table to a layer: wards with unique and with repeated codes, and tables with one
     row per ward and one row per ward per year (all four pandas `validate` cases)
N14  nearest feature: points, lines and areas in a 25 km city-sized region around Shanghai, for
     GeoPandas `sjoin_nearest` in a local projection, checked against pyproj Geod
N15  keep data inside an area: a mask with a hole, a slanted part and a separate part, and the
     edge cases of GeoPandas `clip` (points on the boundary, lines along an edge, areas that only
     touch, multipart features, empty results)

Coordinates are written by hand, except N14's sources, which come from a seeded generator and
are kept only when their nearest target is at least 2% closer than the next one in every
pairing (so the reference's answer can't flip between projections). Deterministic.
The expected values are computed by `join_validate.py`, `nearest_sjoin.py` and `clip_gpd.py`.

Usage:  python -I tools/lm/reference_tests/make_p2_fixtures.py
"""

from __future__ import annotations

import csv
import io
import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "reference"))
import _common  # noqa: E402,F401  (must precede pyproj imports)
from _common import FIX, write_geojson, write_json, write_text  # noqa: E402

MADE_BY = "Made by `tools/lm/reference_tests/make_p2_fixtures.py` (synthetic, CC0-1.0)."


def rect(x0, y0, x1, y1):
    return [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]


def poly(*rings):
    return {"type": "Polygon", "coordinates": [list(r) for r in rings]}


def mpoly(*polys):
    return {"type": "MultiPolygon", "coordinates": [list(p) for p in polys]}


def pt(x, y):
    return {"type": "Point", "coordinates": [x, y]}


def mpt(*xy):
    return {"type": "MultiPoint", "coordinates": [list(c) for c in xy]}


def line(*xy):
    return {"type": "LineString", "coordinates": [list(c) for c in xy]}


def mline(*parts):
    return {"type": "MultiLineString", "coordinates": [[list(c) for c in p] for p in parts]}


def feat(key, fid, g, **props):
    return {"type": "Feature", "properties": {key: fid, **props}, "geometry": g}


def manifest(fid, files, layers, expected, rule):
    return {
        "fixture": fid,
        "files": files,
        "kind": "vector",
        "layers": [{"name": name, "geometry": kind, "count": n} for name, kind, n in layers],
        "crs": {"epsg": 4326, "source": "declared (GeoJSON RFC 7946 default)"},
        "rule": rule,
        "expected_files": expected,
        "assumptions": [],
    }


def write_table(path, header, rows):
    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(header)
    w.writerows(rows)
    write_text(path, buf.getvalue())


# ---------------------------------------------------------------------------------------------
# J6: join cardinality

def j6():
    d = FIX / "J6"
    wards = []
    for i in range(12):
        x = 121.0 + i * 0.0625
        wards.append(feat("ward_code", f"W{i + 1:02d}", poly(rect(x, 31.0, x + 0.05, 31.05)), name=f"Ward {i + 1}"))
    write_geojson(d / "wards.geojson", wards, "wards")
    # Two wards stored as two records each (a mainland part and an island): codes repeat.
    split = list(wards) + [
        feat("ward_code", "W03", poly(rect(121.125, 30.9, 121.15, 30.925)), name="Ward 3 (island)"),
        feat("ward_code", "W07", poly(rect(121.375, 30.9, 121.4, 30.925)), name="Ward 7 (island)"),
    ]
    write_geojson(d / "wards_split.geojson", split, "wards_split")
    pop = [[f"W{i:02d}", 1000 * i + 37 * i * i] for i in range(1, 11)]
    pop.insert(4, ["W99", 555])  # no such ward
    pop.insert(8, ["", 777])  # a blank key
    pop.append(["W12", 12345])
    write_table(d / "ward_population.csv", ["ward_code", "population"], pop)
    years = []
    for i in range(1, 7):
        ys = [2021, 2022, 2023] if i != 2 else [2023, 2021, 2022]  # W02 out of order
        for y in ys:
            years.append([f"W{i:02d}", y, 1000 * i + (y - 2020) * 10 + i])
    years.insert(7, ["W98", 2022, 98])
    years.insert(12, ["W98", 2023, 99])
    years.append(["W12", 2023, 12023])
    write_table(d / "ward_population_by_year.csv", ["ward_code", "year", "population"], years)
    # Different column names: the key is `ward`, not `ward_code` (the action names both).
    people = [[f"W{i:02d}", 500 * i + 3] for i in range(12, 0, -1) if i != 5] + [["W77", 7]]
    write_table(d / "ward_people.csv", ["ward", "people"], people)
    write_json(d / "expected.json", {
        "fixture": "J6",
        "kind": "vector + table",
        "files": ["wards.geojson", "wards_split.geojson", "ward_population.csv", "ward_population_by_year.csv", "ward_people.csv"],
        "layers": [
            {"name": "wards", "geometry": "area", "count": len(wards)},
            {"name": "wards_split", "geometry": "area", "count": len(split)},
        ],
        "tables": [
            {"name": "ward_population", "rows": len(pop)},
            {"name": "ward_population_by_year", "rows": len(years)},
            {"name": "ward_people", "rows": len(people)},
        ],
        "rule": "pandas merge(how='left', validate=...) with the layer on the left, left_on/right_on the key columns; blank keys dropped",
        "expected_files": ["expected_join.csv", "expected_join_values.csv"],
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# J6 — Match a table to a layer: one-to-one, one-to-many, many-to-one, many-to-many

Inputs for `lm_jobs::join::expect` and `apply` (WP6.4, docs/lm/actions-p2.md › Match a table
to a layer). Twelve square wards near 121° E, 31° N.

- `wards.geojson` ({len(wards)} areas): `ward_code` W01–W12, each once.
- `wards_split.geojson` ({len(split)} areas): the same wards, plus a second record for W03 and for
  W07 (an island part stored as its own record), so two codes repeat.
- `ward_population.csv` ({len(pop)} rows): one row per ward for W01–W10 and W12 (W11 has none),
  plus W99 (no such ward) and a row with a blank key.
- `ward_population_by_year.csv` ({len(years)} rows): W01–W06 for 2021–2023 (W02's rows out of
  order), W98 twice (no such ward, repeated) and W12 for 2023 only.
- `ward_people.csv` ({len(people)} rows): the key column is `ward` (not `ward_code`) and the value
  `people`; every ward but W05, in reverse order, plus W77 (no such ward).

The four pairings cover pandas `validate`'s four cases: wards × one row each is one-to-one,
wards × by year one-to-many, split wards × one row each many-to-one, split wards × by year
many-to-many. Wards × people (keys `ward_code` ↔ `ward`) is the different-names case, matched
through the action `match_table` with both columns named.

Expected (from `tools/lm/reference_tests/join_validate.py`):

- `expected_join.csv`, one row per pairing: the cardinality (the tightest `validate` that
  pandas accepts), the rows of a plain left merge (`merge_rows`) and an inner merge (`pairs`),
  layer records with data, table rows with a place, the unmatched keys in table order and the
  unmatched rows (1-based data rows, `|` between them), and the blank keys.
- `expected_join_values.csv`: the value (population or people) each layer record gets (blank:
  none). With one row per ward it is the merge's value; with several, the row with the latest year.

Keys are compared exactly; blank keys never match (pandas would match a blank with a blank) and
are left out of the cardinality check. Tolerances: everything exact.

{MADE_BY}
""")


# ---------------------------------------------------------------------------------------------
# N14: nearest feature (sjoin_nearest in a local projection)

FACILITIES = [
    ("F01", 121.400, 31.180), ("F02", 121.475, 31.195), ("F03", 121.530, 31.240), ("F04", 121.430, 31.260),
    ("F05", 121.365, 31.235), ("F06", 121.500, 31.300), ("F07", 121.560, 31.170), ("F08", 121.445, 31.215),
]
ROADS = [
    ("R01", line((121.35, 31.15), (121.42, 31.20), (121.47, 31.21), (121.55, 31.27))),
    ("R02", line((121.38, 31.30), (121.46, 31.29), (121.52, 31.33))),
    ("R03", mline([(121.58, 31.20), (121.60, 31.26)], [(121.33, 31.27), (121.35, 31.32)])),
    ("R04", line((121.50, 31.14), (121.51, 31.19))),
]
PARKS = [
    ("P01", poly(rect(121.40, 31.225, 121.425, 31.245))),
    ("P02", poly(rect(121.48, 31.23, 121.52, 31.265), rect(121.49, 31.24, 121.505, 31.255)[::-1])),
    ("P03", mpoly([rect(121.36, 31.17, 121.375, 31.185)], [rect(121.54, 31.29, 121.555, 31.305)])),
    ("P04", poly([[121.44, 31.15], [121.47, 31.15], [121.455, 31.175], [121.44, 31.15]])),
]
# Hand-placed sources: inside a park, on a road's vertex, in P02's lagoon (a hole).
SPECIAL = [("S01", 121.41, 31.235), ("S02", 121.47, 31.21), ("S03", 121.496, 31.247)]


def margins_ok(p, groups):
    """At least 50 m from every target group, and its nearest target at least 2% closer than the
    next one."""
    for geoms in groups:
        ds = sorted(p.distance(g) for g in geoms)
        if ds[0] < 50.0 or ds[1] < ds[0] * 1.02:
            return False
    return True


def n14():
    import numpy as np
    import pyproj
    import shapely
    from shapely.geometry import Point, shape

    d = FIX / "N14"
    utm = pyproj.Transformer.from_crs("EPSG:4326", "EPSG:32651", always_xy=True)
    proj = lambda g: shapely.transform(g, lambda a: np.column_stack(utm.transform(a[:, 0], a[:, 1])))  # noqa: E731
    fac = [proj(Point(x, y)) for _, x, y in FACILITIES]
    roads = [proj(shape(g)) for _, g in ROADS]
    parks = [proj(shape(g)) for _, g in PARKS]
    rng = random.Random(14)
    sources = list(SPECIAL)
    while len(sources) < 16:
        x, y = round(rng.uniform(121.34, 121.58), 4), round(rng.uniform(31.14, 31.32), 4)
        if margins_ok(proj(Point(x, y)), [fac, roads, parks]):
            sources.append((f"S{len(sources) + 1:02d}", x, y))
    write_geojson(d / "sources.geojson", [feat("source_id", s, pt(x, y)) for s, x, y in sources], "sources")
    write_geojson(d / "facilities.geojson", [feat("facility_id", f, pt(x, y)) for f, x, y in FACILITIES], "facilities")
    write_geojson(d / "roads.geojson", [feat("road_id", r, g) for r, g in ROADS], "roads")
    write_geojson(d / "parks.geojson", [feat("park_id", p, g) for p, g in PARKS], "parks")
    write_json(d / "expected.json", manifest(
        "N14",
        ["sources.geojson", "facilities.geojson", "roads.geojson", "parks.geojson"],
        [("sources", "point", len(sources)), ("facilities", "point", len(FACILITIES)), ("roads", "line", len(ROADS)),
         ("parks", "area", len(PARKS))],
        ["expected_nearest_sources_facilities.csv", "expected_nearest_sources_roads.csv",
         "expected_nearest_sources_parks.csv", "expected_nearest_parks_facilities.csv"],
        "GeoPandas sjoin_nearest in UTM zone 51N (EPSG:32651); distances checked with pyproj Geod (WGS 84)",
    ))
    write_text(d / "README.md", f"""# N14 — Nearest feature, against GeoPandas `sjoin_nearest`

Inputs for `lm_analysis::nearest::nearest_distance` (WP6.4, docs/lm/actions-p2.md › Nearest
feature). A 25 km region around Shanghai (121.33–121.60° E, 31.14–31.33° N).

- `sources.geojson` ({len(sources)} points, `source_id`): S01 inside park P01, S02 exactly on a
  vertex of road R01, S03 in park P02's lagoon (a hole); the rest from a seeded generator, each
  kept only when its nearest target is at least 2% closer than the next one, in every pairing.
- `facilities.geojson` ({len(FACILITIES)} points, `facility_id`).
- `roads.geojson` ({len(ROADS)} lines, `road_id`): R03 is a two-part line.
- `parks.geojson` ({len(PARKS)} areas, `park_id`): P02 has a hole, P03 two parts, P04 is a triangle.

Expected (from `tools/lm/reference_tests/nearest_sjoin.py`), one CSV per pairing
(sources → facilities, roads, parks; parks → facilities), columns `feature_id`, `nearest_id`,
`distance_m` (geodesic), `projected_m` and `runner_up_ratio`. The nearest target is GeoPandas
`sjoin_nearest` in UTM zone 51N (EPSG:32651). The geodesic distance is pyproj `Geod.inv` (WGS 84)
between the two nearest points that shapely finds in that projection, taken back to lon/lat;
the script asserts it is within 0.1% of the projected distance.

Tolerances: the nearest target exactly; distances within 0.1% or 1 m of `distance_m`, whichever
is larger (docs/lm/actions-p2.md says why that is the right bound for a projected reference);
point to point within 1 mm (Geod.inv is exact there).

{MADE_BY}
""")
    return sources


# ---------------------------------------------------------------------------------------------
# N15: keep data inside an area (clip)

U = 1 / 64  # one grid unit, in degrees
X0, Y0 = 116.0, 39.75


def g(u, v):
    return [X0 + u * U, Y0 + v * U]


def grect(u0, v0, u1, v1):
    return [g(u0, v0), g(u1, v0), g(u1, v1), g(u0, v1), g(u0, v0)]


def n15():
    d = FIX / "N15"
    zone = [
        feat("zone_id", "Z1", poly(grect(0, 0, 8, 8), grect(3, 3, 5, 5)[::-1])),
        feat("zone_id", "Z2", poly([g(6, 2), g(12, 2), g(9, 7), g(6, 2)])),
        feat("zone_id", "Z3", poly(grect(14, 0, 18, 4))),
    ]
    areas = [
        feat("id", "A1", poly(grect(-2, -2, 2, 2)), note="across Z1's corner"),
        feat("id", "A2", poly(grect(-4, 2, 0, 6)), note="shares Z1's west edge only"),
        feat("id", "A3", poly(grect(-3, -3, 0, 0)), note="touches Z1's corner only"),
        feat("id", "A4", poly(grect(-1, -1, 19, 9)), note="covers the whole mask"),
        feat("id", "A5", mpoly([grect(1, 1, 2, 2)], [grect(20, 20, 22, 22)]), note="two parts, one inside"),
        feat("id", "A6", poly(grect(10, 1, 16, 3), grect(14.5, 1.5, 15.5, 2.5)[::-1]), note="holed, across Z2 and Z3"),
        feat("id", "A7", poly(grect(3.5, 3.5, 4.5, 4.5)), note="inside the mask's hole"),
        feat("id", "A8", poly(grect(30, 30, 31, 31)), note="far outside"),
        {"type": "Feature", "properties": {"id": "A9", "note": "no geometry"}, "geometry": None},
    ]
    lines = [
        feat("id", "L1", line(g(-2, 6), g(20, 6)), note="across Z1 and Z2, with a gap between them"),
        feat("id", "L2", line(g(1, 0), g(5, 0)), note="along Z1's south edge"),
        feat("id", "L3", line(g(-2, -2), g(0, 0)), note="ends on Z1's corner"),
        feat("id", "L4", line(g(4, -1), g(4, 9)), note="through the hole"),
        feat("id", "L5", mline([g(15, 1), g(17, 3)], [g(20, 1), g(22, 3)]), note="two parts, one inside Z3"),
        feat("id", "L6", line(g(-5, 10), g(-1, 12)), note="outside"),
        feat("id", "L7", line(g(2, 1), g(2, -2), g(6, -2), g(6, 1)), note="leaves Z1 and comes back"),
    ]
    points = [
        feat("id", "P1", pt(*g(2, 2)), note="inside"),
        feat("id", "P2", pt(*g(0, 4)), note="on Z1's west edge"),
        feat("id", "P3", pt(*g(8, 8)), note="on Z1's corner"),
        feat("id", "P4", pt(*g(4, 4)), note="in the hole"),
        feat("id", "P5", pt(*g(3, 4)), note="on the hole's edge"),
        feat("id", "P6", pt(*g(-1, -1)), note="outside"),
        feat("id", "P7", mpt(g(1, 1), g(30, 30)), note="two points, one inside"),
        feat("id", "P8", pt(*g(10.5, 4.5)), note="on Z2's slanted edge, outside Z1"),
        feat("id", "P9", pt(*g(18, 4)), note="on Z3's corner"),
    ]
    write_geojson(d / "zone.geojson", zone, "zone")
    write_geojson(d / "areas.geojson", areas, "areas")
    write_geojson(d / "lines.geojson", lines, "lines")
    write_geojson(d / "points.geojson", points, "points")
    write_json(d / "expected.json", manifest(
        "N15",
        ["zone.geojson", "areas.geojson", "lines.geojson", "points.geojson"],
        [("zone", "area", len(zone)), ("areas", "area", len(areas)), ("lines", "line", len(lines)), ("points", "point", len(points))],
        ["expected_clip.csv"],
        "GeoPandas clip(gdf, zone, keep_geom_type=True)",
    ))
    write_text(d / "README.md", f"""# N15 — Keep data inside an area, against GeoPandas `clip`

Inputs for `lm_analysis::clip::keep_inside` (WP6.4, docs/lm/actions-p2.md › Keep data inside
an area). Coordinates are 116° E, 39.75° N plus multiples of 1/64° (so every input vertex and
most crossings are exact binary fractions); Z2's slanted edges make crossings that are not.

- `zone.geojson` (`zone_id`): Z1 the square [0, 8]² with the hole [3, 5]²; Z2 a triangle
  (6, 2), (12, 2), (9, 7) overlapping Z1's east side; Z3 the square [14, 18] × [0, 4] on its own.
- `areas.geojson` (`id`): across a corner, sharing an edge only, touching a corner only,
  covering the whole mask, two parts with one inside, a holed area across Z2 and Z3, inside the
  mask's hole, far outside, and no geometry.
- `lines.geojson` (`id`): across Z1 and Z2 with a gap, along Z1's south edge, ending on a
  corner, through the hole, two parts with one inside, outside, and out and back in.
- `points.geojson` (`id`): inside, on an edge, on a corner, in the hole, on the hole's edge,
  outside, a two-point multipoint, on Z2's slanted edge, on Z3's corner.

Expected (from `tools/lm/reference_tests/clip_gpd.py`): `expected_clip.csv`, one row per
input feature: `kept`, `parts`, the geodesic area (pyproj Geod, WGS 84) or length, and for
lines and points the coordinates left (lines merged with `shapely.line_merge`, each part from
its smaller end, parts sorted, `|` between parts). Reference: GeoPandas `clip(gdf, zone,
keep_geom_type=True)`.

Tolerances: the same features kept and the same number of parts, exactly; areas and lengths
within 1e-6 relative; coordinates within 1e-9° (input vertices come back exactly).

{MADE_BY}
""")


def main():
    j6()
    s = n14()
    n15()
    print(f"J6, N14 ({len(s)} sources), N15 written")


if __name__ == "__main__":
    main()
