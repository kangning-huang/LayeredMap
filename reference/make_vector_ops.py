"""Build the vector-operation fixtures N9 (nearest distance, island isolation), N10 (join by
location) and N11 (per-feature measurements).

Every coordinate is written by hand (exact binary fractions wherever a point must sit exactly on
an edge), so the output is deterministic. The expected values are computed by
`nearest_distance.py`, `join_by_location.py` and `measure_features.py`.

Usage:  python -I reference/make_vector_ops.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_geojson, write_json, write_text  # noqa: E402

MADE_BY = "Made by `reference/make_vector_ops.py` (synthetic, CC0-1.0)."


def rect(x0, y0, x1, y1, cw=False):
    r = [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]
    return r[::-1] if cw else r


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


def manifest(fid, layers, expected, rule):
    return {
        "fixture": fid,
        "files": [f"{name}.geojson" for name, _, _ in layers],
        "kind": "vector",
        "layers": [{"name": name, "geometry": kind, "count": n} for name, kind, n in layers],
        "crs": {"epsg": 4326, "source": "declared (GeoJSON RFC 7946 default)"},
        "rule": rule,
        "expected_files": expected,
        "assumptions": [],
    }


# ---------------------------------------------------------------------------------------------
# N9: nearest distance and island isolation

def n9():
    d = FIX / "N9"
    islands = [
        # A multipart island whose own parts are 6.9 km apart; the nearest *other* island is ~34 km.
        feat("island_id", "I01", mpoly([rect(150.0, -10.0, 150.25, -9.75)], [rect(150.3125, -10.0, 150.4375, -9.875)]), name="Twin"),
        feat("island_id", "I02", poly(rect(150.75, -10.0, 150.875, -9.875)), name="West Sister"),
        # Shares I02's east edge: touching, distance 0.
        feat("island_id", "I03", poly(rect(150.875, -10.0, 151.0, -9.875)), name="East Sister"),
        # I04 and I05 overlap: 0.
        feat("island_id", "I04", poly(rect(151.5, -9.5, 151.75, -9.25)), name="Overlap A"),
        feat("island_id", "I05", poly(rect(151.625, -9.375, 151.875, -9.125)), name="Overlap B"),
        # I06 sits in I07's lagoon (a hole): its distance is to the hole's ring.
        feat("island_id", "I06", poly(rect(152.9375, -10.0625, 153.0625, -9.9375)), name="Lagoon Islet"),
        feat("island_id", "I07", poly(rect(152.5, -10.5, 153.5, -9.5), rect(152.75, -10.25, 153.25, -9.75, cw=True)), name="Atoll"),
        # Split at the antimeridian (RFC 7946): one island in two parts meeting at 180°.
        feat("island_id", "I08", mpoly([rect(179.75, -16.0, 180.0, -15.75)], [rect(-180.0, -16.0, -179.875, -15.75)]), name="Dateline"),
        feat("island_id", "I09", poly(rect(-179.5, -16.0, -179.375, -15.875)), name="East of the line"),
        feat("island_id", "I10", poly(rect(179.25, -15.875, 179.375, -15.75)), name="West of the line"),
        # High latitude (Svalbard-like); I13 is multipart with parts 300 km apart.
        feat("island_id", "I11", poly(rect(20.0, 78.0, 21.0, 78.25)), name="North A"),
        feat("island_id", "I12", poly(rect(22.0, 78.5, 22.5, 78.75)), name="North B"),
        feat("island_id", "I13", mpoly([rect(10.0, 79.5, 10.5, 79.625)], [rect(23.0, 78.25, 23.25, 78.375)]), name="North Split"),
        # Remote: thousands of km from everything else.
        feat("island_id", "I14", poly(rect(-140.0, -30.0, -139.875, -29.875)), name="Remote"),
    ]
    points = [
        feat("point_id", "S01", pt(150.125, -9.875), note="inside I01"),
        feat("point_id", "S02", pt(150.75, -9.9375), note="on I02's west edge"),
        feat("point_id", "S03", pt(152.8125, -10.125), note="in I07's lagoon, not on I06"),
        feat("point_id", "S04", pt(-179.75, -15.5), note="near the antimeridian"),
        feat("point_id", "S05", pt(179.9375, -16.25), note="south of I08's western part"),
        feat("point_id", "S06", pt(21.5, 78.375), note="high latitude"),
        feat("point_id", "S07", pt(0.0, 0.0), note="far from everything"),
        feat("point_id", "S08", pt(-139.9375, -29.9375), note="inside I14"),
        feat("point_id", "S09", pt(0.0, 89.5), note="near the pole"),
        feat("point_id", "S10", pt(150.5, -9.0), note="exactly on L01 (a lon/lat straight edge)"),
        feat("point_id", "S11", pt(150.5, -9.5), note="between I01 and L01"),
        feat("point_id", "S12", pt(-179.8125, -15.0625), note="just south of L02's eastern part"),
    ]
    lines = [
        feat("line_id", "L01", line((149.5, -9.0), (151.5, -9.0)), note="straight lon/lat edge, 2 degrees"),
        feat("line_id", "L02", mline([(179.5, -15.0), (180.0, -15.0)], [(-180.0, -15.0), (-179.5, -15.0)]), note="split at 180"),
        feat("line_id", "L03", line((15.0, 79.0), (25.0, 79.0), (30.0, 80.0)), note="high latitude"),
        feat("line_id", "L04", line((150.8125, -10.25), (150.8125, -9.5)), note="crosses I02"),
        feat("line_id", "L05", line((153.5, -9.5), (153.75, -9.25)), note="starts on I07's corner"),
        feat("line_id", "L06", line((-20.0, -40.0), (-10.0, -35.0)), note="South Atlantic"),
    ]
    write_geojson(d / "islands.geojson", islands, "islands")
    write_geojson(d / "points.geojson", points, "points")
    write_geojson(d / "lines.geojson", lines, "lines")
    expected = [
        "expected_nearest_islands_same.csv", "expected_nearest_points_same.csv",
        "expected_nearest_points_to_islands.csv", "expected_nearest_points_to_lines.csv",
        "expected_nearest_lines_to_islands.csv", "expected_nearest_islands_to_points.csv",
    ]
    write_json(d / "expected.json", manifest(
        "N9", [("islands", "area", len(islands)), ("points", "point", len(points)), ("lines", "line", len(lines))], expected,
        "shortest geodesic (WGS 84) distance to the nearest target, 0 when touching; same layer excludes the feature's own row (all its parts)"))
    write_text(d / "README.md", f"""# N9 — Nearest distance and island isolation (A3)

`islands.geojson` (14 areas, `island_id`), `points.geojson` (12 points, `point_id`) and
`lines.geojson` (6 lines, `line_id`). Every coordinate is a binary fraction of a degree.

What it exercises:

- **Same-layer exclusion of multipart features.** I01's two parts are 6.9 km apart; its nearest
  *other* island (I02) is about 34 km away. I13's parts are 300 km apart, at 78–80°N.
- **Touching and overlap give 0.** I02 and I03 share an edge; I04 and I05 overlap; S01 and S08
  lie inside islands, S02 on an edge; S10 lies exactly on L01's straight lon/lat edge; L04
  crosses I02; L05 starts on I07's corner.
- **Holes.** I06 lies in I07's lagoon (a hole), so their distance is to the hole's ring; S03 is
  in the lagoon too.
- **Antimeridian.** I08 is split at 180° (RFC 7946), with I09 and I10 on either side; L02 is a
  split line; S04, S05, S12 sit beside them.
- **High latitude and far away.** I11–I13, L03, S06 near 78–80°N; S09 at 89.5°N; S07 at 0°, 0°
  and I14 in the South Pacific are thousands of km from anything.
- **Kinds.** Points to points (same layer), points to lines, points to areas, lines to areas,
  areas to areas (same layer) and areas to points.

Expected (`reference/nearest_distance.py`): one CSV per pairing, columns `feature_id`,
`nearest_id`, `distance_m`. Definition: 0 when the feature intersects a target (shapely
`intersects` in lon/lat, the app's planar convention); otherwise targets and the feature are
densified along geodesics to steps of at most 1 km (pyproj `Geod`), projected to an azimuthal
equidistant projection centred on the feature (circular mean of its vertices) and measured with
shapely `distance` between their boundaries. Same layer: the feature's own row is left out.
Tolerance (mechanics.md › A3): 0.1% or 50 m, whichever is larger.

{MADE_BY}
""")


# ---------------------------------------------------------------------------------------------
# N10: join by location

def n10():
    d = FIX / "N10"
    areas = [
        feat("area_id", "A1", poly(rect(0.0, 0.0, 1.0, 1.0)), name="West"),
        feat("area_id", "A2", poly(rect(1.0, 0.0, 2.0, 1.0)), name="East"),
        feat("area_id", "A3", poly(rect(1.5, 0.5, 2.5, 1.5)), name="Overlapping East"),
        feat("area_id", "A4", poly(rect(3.0, 0.0, 5.0, 2.0), rect(3.5, 0.5, 4.5, 1.5, cw=True)), name="Ring"),
        feat("area_id", "A5", mpoly([rect(6.0, 0.0, 6.5, 0.5)], [rect(7.0, 0.0, 7.5, 0.5)]), name="Two parts"),
        feat("area_id", "A6", poly(rect(3.75, 0.75, 4.25, 1.25)), name="Island in the ring's hole"),
        feat("area_id", "A7", mpoly([rect(179.5, -1.0, 180.0, 1.0)], [rect(-180.0, -1.0, -179.5, 1.0)]), name="Dateline"),
        feat("area_id", "A8", poly(rect(-30.0, 80.0, 30.0, 85.0)), name="Arctic"),
        feat("area_id", "A9", poly(rect(0.25, 0.25, 0.75, 0.75)), name="Inside West, listed later"),
        feat("area_id", "A10", None, name="No geometry"),
    ]
    points = [
        feat("feature_id", "P01", pt(0.5, 0.5), note="in A1 and A9: A1 (first)"),
        feat("feature_id", "P02", pt(1.0, 0.5), note="on the A1/A2 edge: A1"),
        feat("feature_id", "P03", pt(1.75, 0.75), note="in A2 and A3: A2"),
        feat("feature_id", "P04", pt(2.25, 1.25), note="A3 only"),
        feat("feature_id", "P05", pt(4.0, 1.0), note="in A4's hole, inside A6"),
        feat("feature_id", "P06", pt(3.625, 0.625), note="in A4's hole: outside"),
        feat("feature_id", "P07", pt(3.5, 1.0), note="on A4's hole ring: A4"),
        feat("feature_id", "P08", pt(6.75, 0.25), note="between A5's parts: outside"),
        feat("feature_id", "P09", pt(7.25, 0.25), note="A5's second part"),
        feat("feature_id", "P10", pt(180.0, 0.0), note="on A7's seam"),
        feat("feature_id", "P11", pt(-179.75, 0.5), note="A7's eastern part"),
        feat("feature_id", "P12", pt(0.0, 82.0), note="Arctic"),
        feat("feature_id", "P13", pt(30.0, 82.0), note="on A8's east edge"),
        feat("feature_id", "P14", pt(10.0, 10.0), note="outside"),
        feat("feature_id", "P15", None, note="no geometry"),
        feat("feature_id", "P16", pt(1.5, 1.0), note="on A2's top edge and A3's west edge: A2"),
        feat("feature_id", "P17", pt(0.25, 0.25), note="inside A1, on A9's corner: A1, on a boundary"),
        feat("feature_id", "M01", mpt((10.0, 10.0), (1.75, 0.75)), note="multipoint: one point in A2 and A3"),
    ]
    lines = [
        feat("feature_id", "L01", line((0.125, 0.125), (0.5, 0.375), (0.875, 0.875)), note="wholly in A1"),
        feat("feature_id", "L02", line((3.125, 0.25), (4.875, 0.25), (4.875, 1.75)), note="wholly in A4's ring"),
        feat("feature_id", "L03", line((10.0, 10.0), (11.0, 11.0)), note="outside"),
        feat("feature_id", "L04", mline([(179.625, 0.0), (179.75, 0.25), (180.0, 0.25)], [(-180.0, 0.25), (-179.75, 0.5)]), note="split at 180, in A7"),
        feat("feature_id", "L05", line((-20.0, 81.0), (0.0, 83.0), (20.0, 81.0)), note="Arctic"),
    ]
    c_shape = [(3.125, 0.125), (4.875, 0.125), (4.875, 0.375), (3.375, 0.375), (3.375, 1.625), (4.875, 1.625),
               (4.875, 1.875), (3.125, 1.875), (3.125, 0.125)]
    polygons = [
        feat("feature_id", "G01", poly(c_shape), note="C shape around A4's hole, in A4's ring"),
        feat("feature_id", "G02", poly(rect(1.0625, 0.0625, 1.4375, 0.9375), rect(1.125, 0.125, 1.375, 0.875, cw=True)), note="thin frame with a hole, in A2 only"),
        feat("feature_id", "G03", mpoly([rect(7.0625, 0.0625, 7.4375, 0.4375)], [rect(8.0, 0.0, 8.0625, 0.0625)]), note="largest part in A5, small part outside"),
        feat("feature_id", "G05", mpoly([rect(179.75, -0.5, 180.0, 0.5)], [rect(-180.0, -0.5, -179.875, 0.5)]), note="split at 180, in A7"),
        feat("feature_id", "G06", poly(rect(-10.0, 81.0, 10.0, 84.0)), note="Arctic"),
        feat("feature_id", "G07", poly(rect(20.0, 20.0, 21.0, 21.0)), note="outside"),
    ]
    write_geojson(d / "areas.geojson", areas, "areas")
    write_geojson(d / "points.geojson", points, "points")
    write_geojson(d / "lines.geojson", lines, "lines")
    write_geojson(d / "polygons.geojson", polygons, "polygons")
    write_json(d / "expected.json", manifest(
        "N10", [("areas", "area", len(areas)), ("points", "point", len(points)), ("lines", "line", len(lines)),
                ("polygons", "area", len(polygons))], ["expected_join.csv"],
        "each feature takes the first area row (row order) its point intersects; lines and areas by their label point"))
    write_text(d / "README.md", f"""# N10 — Join by location (A6)

`areas.geojson` (10 rows, `area_id`; A10 has no geometry), joined to `points.geojson` (17 points
and a multipoint), `lines.geojson` (5 lines) and `polygons.geojson` (6 areas), all keyed by
`feature_id`. Every boundary coordinate is a binary fraction, so "on the boundary" is exact.

What it exercises:

- **First area in row order.** P01 is in A1 and A9 (A9 lies inside A1 but is listed later): A1.
  P03 is in A2 and A3: A2. P16 lies on A2's top edge and A3's west edge: A2.
- **Boundaries.** P02 on the A1/A2 edge (A1), P07 on A4's hole ring (A4), P10 on A7's 180° seam,
  P13 on A8's edge, P16; P17 is inside A1 and on A9's corner, so it is counted as on a boundary
  although it takes A1.
- **Holes and gaps.** P06 is in A4's hole (outside); P05 is in the hole but inside A6. P08 is
  between A5's parts (outside).
- **Antimeridian and high latitude.** A7 is split at 180° (P10, P11, L04, G05); A8 spans
  80–85°N (P12, P13, L05, G06).
- **Missing geometry.** A10 and P15 have none.
- **Lines and areas by their label point.** Each line and area lies wholly inside one area's
  part (or wholly outside), so any point of it gives the same answer: L01, L02, L04, L05 and
  G01 (a C shape wrapped around A4's hole and A6: a label point in the C's mouth would land in the
  hole or in A6), G02 (a frame with a hole), G03 (largest part in A5, small part outside), G05,
  G06; L03, G07 are outside.

Expected (`reference/join_by_location.py`): `expected_join.csv`, columns `layer`, `feature_id`,
`area_id` (blank when none) and `on_boundary` (points and multipoints only: whether a point lies
on the boundary of any area). Definition: GeoPandas `sjoin(predicate="intersects", how="left")`,
keeping the match with the lowest area row; lines and areas use shapely's `representative_point`.
Tolerance: exact.

{MADE_BY}
""")


# ---------------------------------------------------------------------------------------------
# N11: measurements

def n11():
    d = FIX / "N11"
    c_shape = [(0.0, 44.0), (3.0, 44.0), (3.0, 44.5), (0.5, 44.5), (0.5, 45.5), (3.0, 45.5), (3.0, 46.0), (0.0, 46.0), (0.0, 44.0)]
    comb = [(10.0, 0.0), (14.0, 0.0), (14.0, 3.0), (13.5, 3.0), (13.5, 0.5), (12.5, 0.5), (12.5, 3.0), (12.0, 3.0),
            (12.0, 0.5), (11.0, 0.5), (11.0, 3.0), (10.5, 3.0), (10.5, 0.5), (10.0, 0.5), (10.0, 0.0)]
    areas = [
        feat("feature_id", "M_SQ", poly(rect(0.0, 0.0, 1.0, 1.0)), note="1 degree at the equator"),
        feat("feature_id", "M_C", poly(c_shape), note="C shape at 45N"),
        feat("feature_id", "M_COMB", poly(comb), note="comb: four teeth"),
        feat("feature_id", "M_HOLE", poly(rect(10.0, 60.0, 12.0, 61.0, cw=True), rect(10.5, 60.25, 11.5, 60.75)), note="clockwise exterior with a hole, 60N"),
        feat("feature_id", "M_FRAME", poly(rect(20.0, 20.0, 21.0, 21.0), rect(20.0625, 20.0625, 20.9375, 20.9375, cw=True)), note="thin frame: almost all hole"),
        feat("feature_id", "M_MULTI", mpoly([rect(30.0, 0.0, 30.5, 0.5)], [rect(31.0, 0.0, 33.0, 2.0)], [rect(34.0, 0.0, 34.25, 0.25)]), note="three parts, the second largest"),
        feat("feature_id", "M_AM", mpoly([rect(179.5, -20.0, 180.0, -19.0)], [rect(-180.0, -20.0, -179.0, -19.0)]), note="split at 180 (perimeter left blank)"),
        feat("feature_id", "M_POLAR", poly(rect(-20.0, 85.0, 20.0, 89.0)), note="near the pole"),
        feat("feature_id", "M_BIG", poly(rect(-20.0, -40.0, 10.0, -10.0)), note="30 by 30 degrees"),
        feat("feature_id", "M_THIN", poly(rect(0.0, 10.0, 1.0, 10.001)), note="sliver 110 m wide"),
    ]
    lines = [
        feat("feature_id", "L_EQ", line((0.0, 0.0), (1.0, 0.0), (2.0, 0.0)), note="equator"),
        feat("feature_id", "L_HIGH", line((0.0, 80.0), (40.0, 80.0), (80.0, 82.0)), note="high latitude"),
        feat("feature_id", "L_AM", mline([(179.0, 10.0), (180.0, 10.0)], [(-180.0, 10.0), (-178.0, 11.0)]), note="split at 180"),
        feat("feature_id", "L_MULTI", mline([(5.0, 5.0), (6.0, 5.0)], [(7.0, 5.0), (7.0, 6.0), (8.0, 7.0)]), note="two parts"),
        feat("feature_id", "L_LONG", line((0.0, 30.0), (45.0, 30.0), (90.0, 30.0)), note="a quarter of the way round"),
    ]
    points = [
        feat("feature_id", "P_A", pt(121.4737, 31.2304), note="Shanghai"),
        feat("feature_id", "P_B", pt(-179.999, -16.5), note="antimeridian"),
        feat("feature_id", "P_C", pt(15.6, 78.2), note="Svalbard"),
        feat("feature_id", "P_M", mpt((1.25, 2.5), (3.0, 4.0)), note="multipoint: its first point"),
    ]
    write_geojson(d / "areas.geojson", areas, "areas")
    write_geojson(d / "lines.geojson", lines, "lines")
    write_geojson(d / "points.geojson", points, "points")
    write_json(d / "expected.json", manifest(
        "N11", [("areas", "area", len(areas)), ("lines", "line", len(lines)), ("points", "point", len(points))], ["expected_measure.csv"],
        "geodesic (WGS 84) area and perimeter of areas, length of lines; coordinates of points; label points inside their area"))
    write_text(d / "README.md", f"""# N11 — Measurements and label points (A4)

`areas.geojson` (10 areas), `lines.geojson` (5 lines) and `points.geojson` (3 points and a
multipoint), keyed by `feature_id`.

What it exercises: a C shape and a comb (whose centroids lie outside them), a clockwise exterior
with a hole, a thin frame that is almost all hole, a multipolygon whose largest part is not the
first, a polygon split at the antimeridian, one near the pole, a 30° square, a 110 m sliver; lines
at the equator, at 80°N, split at 180°, in two parts, and 90° of longitude long; points including
one beside 180° and a multipoint.

Expected (`reference/measure_features.py`): `expected_measure.csv`, columns `feature_id`,
`area_km2`, `perimeter_km` (areas), `length_km` (lines), `lon`, `lat` (points: the input
coordinates; a multipoint its first point). Areas and perimeters come from pyproj
`Geod.geometry_area_perimeter` after orienting every polygon counter-clockwise (holes clockwise),
lengths from `Geod.geometry_length`. The perimeter counts holes' rings too (pyproj's own
perimeter leaves them out, so the reference adds each hole's ring perimeter). M_AM's perimeter is left blank: the RFC 7946 split adds a
seam along 180° that LayeredMap leaves out (see `crate::geodesic`). Tolerance (mechanics.md ›
A4): 0.01% for areas and lengths, exact for point coordinates; label points of lines and areas
are not in the CSV, the Rust tests assert that each lies on its line or strictly inside its area.

{MADE_BY}
""")


def main():
    n9()
    n10()
    n11()
    print("N9, N10, N11 written")


if __name__ == "__main__":
    main()
