"""Build the hand-made LayeredMap fixtures (WP0.8).

Deterministic: every random draw uses a fixed seed, every container format
(zip, xlsx, shapefile .dbf header, GeoPackage) is written with fixed
timestamps. Rerunning produces identical bytes.

Usage:  python -I reference/make_handmade.py
"""

from __future__ import annotations

import os
import re
import shutil
import sys
import zipfile
from datetime import datetime
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common  # noqa: E402  (must precede pyproj/rasterio imports)
from _common import (FIX, deterministic_zip, fmt, patch_dbf, rezip_fixed,  # noqa: E402
                     write_csv, write_geojson, write_json, write_text, zone_area_exact)

import geopandas as gpd  # noqa: E402
import numpy as np  # noqa: E402
import prcoords  # noqa: E402
import pyogrio  # noqa: E402
import pyproj  # noqa: E402
import rasterio  # noqa: E402
import shapely  # noqa: E402
import shapely.validation  # noqa: E402
from rasterio.transform import from_bounds, from_origin  # noqa: E402
from shapely.geometry import (LineString, MultiLineString, MultiPolygon, Point,  # noqa: E402
                              Polygon, box, mapping)

GEOD = pyproj.Geod(ellps="WGS84")
MADE_BY = "Made by `reference/make_handmade.py` (synthetic, CC0-1.0)."


def geom(g):
    """shapely -> GeoJSON geometry dict with lists, not tuples."""
    def conv(o):
        if isinstance(o, (list, tuple)):
            return [conv(x) for x in o]
        if isinstance(o, dict):
            return {k: conv(v) for k, v in o.items()}
        if isinstance(o, float):
            return round(o, 9)
        return o
    return conv(mapping(g))


def feature(props, g):
    return {"type": "Feature", "properties": props,
            "geometry": g if (g is None or isinstance(g, dict)) else geom(g)}


def fresh(fid: str) -> Path:
    d = FIX / fid
    if d.exists():
        for p in d.iterdir():
            if p.name.startswith("expected_") and p.suffix == ".csv":
                continue  # owned by the reference scripts
            if p.is_dir():
                shutil.rmtree(p, ignore_errors=True)
            else:
                p.unlink(missing_ok=True)  # an exFAT drive's ._ files vanish with their partner
    d.mkdir(parents=True, exist_ok=True)
    return d


def geodesic_area(g) -> float:
    a, _ = GEOD.geometry_area_perimeter(g)
    return abs(a)


def write_shp(gdf: gpd.GeoDataFrame, path: Path, encoding: str | None = None,
              keep_prj: bool = True, keep_cpg: bool = True, ldid: int | None = None):
    kw = {"encoding": encoding} if encoding else {}
    pyogrio.write_dataframe(gdf, path, driver="ESRI Shapefile", **kw)
    stem = path.with_suffix("")
    if not keep_prj:
        stem.with_suffix(".prj").unlink(missing_ok=True)
    if not keep_cpg:
        stem.with_suffix(".cpg").unlink(missing_ok=True)
    patch_dbf(stem.with_suffix(".dbf"), ldid=ldid)


def shp_parts(path: Path):
    stem = path.with_suffix("")
    return [stem.with_suffix(e) for e in (".shp", ".shx", ".dbf", ".prj", ".cpg")
            if stem.with_suffix(e).exists()]


# --------------------------------------------------------------------------
# V1 — 17 synthetic districts over Shanghai
# --------------------------------------------------------------------------
LON_EDGES = [121.2, 121.35, 121.5, 121.65, 121.8]
LAT_EDGES = [30.9, 31.05, 31.2, 31.35, 31.5]
# (col, row) -> (id, name_en, name_zh, population); row 0 is the southern row.
V1_CELLS = {
    (0, 3): ("D01", "Jiading", "嘉定区", 1834258),
    (1, 3): ("D02", "Baoshan", "宝山区", 2235218),
    (2, 3): ("D03", "Yangpu", "杨浦区", 1242548),
    (3, 3): ("D04", "Chongming", "崇明区", 637921),
    (0, 2): ("D05", "Qingpu", "青浦区", 1271424),
    (1, 2): ("D06", "Jing'an", "静安区", 975707),
    (2, 2): ("D07", "Hongkou", "虹口区", 757498),
    (3, 2): ("D08", "Zhabei", "闸北区", None),          # missing value (merged 2015)
    (0, 1): ("D09", "Songjiang", "松江区", 1909852),
    (1, 1): ("D10", "Changning", "长宁区", 693051),
    (2, 1): ("D11", "Xuhui", "徐汇区", 1113078),
    (3, 1): ("D12", "Pudong New Area", "浦东新区", 5681512),
    (0, 0): ("D13", "Jinshan", "金山区", 822776),
    (1, 0): ("D14", "Minhang", "闵行区", 2653489),
    (2, 0): ("D15", "Fengxian", "奉贤区", 1140872),
    (3, 0): ("D16", "Putuo", "普陀区", 1140872),        # tie with Fengxian (deliberate)
}
V1_HOLE = box(121.40, 31.25, 121.45, 31.30)  # inside Jing'an's cell; = Huangpu
V1_HOLE_DISTRICT = ("D17", "Huangpu", "黄浦区", 662030)


def v1_geometries():
    """Ordered list of (id, name_en, name_zh, population, shapely geom)."""
    out = []
    for (c, r), (did, en, zh, pop) in V1_CELLS.items():
        cell = box(LON_EDGES[c], LAT_EDGES[r], LON_EDGES[c + 1], LAT_EDGES[r + 1])
        if did == "D06":  # polygon with a hole
            g = Polygon(cell.exterior.coords, [V1_HOLE.exterior.coords[::-1]])
        elif did == "D04":  # multipolygon: two islands separated by a 0.01° channel
            g = MultiPolygon([box(121.65, 31.35, 121.72, 31.5), box(121.73, 31.35, 121.8, 31.5)])
        else:
            g = cell
        out.append((did, en, zh, pop, shapely.normalize(g) if did not in ("D06",) else g))
    did, en, zh, pop = V1_HOLE_DISTRICT
    out.append((did, en, zh, pop, V1_HOLE))
    out.sort(key=lambda t: t[0])
    return out


def make_v1():
    d = fresh("V1")
    feats = []
    for did, en, zh, pop, g in v1_geometries():
        feats.append(feature({"district_id": did, "name_zh": zh, "name_en": en,
                              "population": pop,
                              "area_km2": round(geodesic_area(g) / 1e6, 2)}, g))
    write_geojson(d / "districts.geojson", feats, name="districts")
    pops = [f["properties"]["population"] for f in feats]
    write_json(d / "expected.json", {
        "fixture": "V1",
        "files": ["districts.geojson"],
        "kind": "vector",
        "layers": [{"name": "districts", "geometry": "area", "count": 17,
                    "geometry_types": {"Polygon": 16, "MultiPolygon": 1},
                    "polygons_with_holes": ["D06"], "multipolygons": ["D04"]}],
        "crs": {"epsg": 4326, "source": "declared (GeoJSON RFC 7946 default)"},
        "extent_lonlat": [121.2, 30.9, 121.8, 31.5],
        "fields": {"district_id": "id", "name_zh": "name", "name_en": "name",
                   "population": "measure", "area_km2": "measure"},
        "field_types": {"district_id": "text", "name_zh": "text", "name_en": "text",
                        "population": "integer", "area_km2": "real"},
        "missing_values": {"population": 1},
        "population": {"n_with_values": sum(p is not None for p in pops),
                       "sum": sum(p for p in pops if p is not None),
                       "ties": [["D15", "D16"]], "missing": ["D08"]},
        "assumptions": [],
        "encoding": "UTF-8",
        "notes": "Insight expectations are in expected_insight.csv (rank_check.py).",
    })
    write_text(d / "README.md", f"""# V1 — Shanghai districts (synthetic)

`districts.geojson`: 17 district areas laid out on a 4×4 grid of 0.15° cells over
lon 121.2–121.8, lat 30.9–31.5, plus one district filling a hole.

- Names (`name_zh`, `name_en`) are real Shanghai district names (including the
  former Zhabei 闸北区), but **geometry and population are synthetic**.
- `D06` Jing'an is a polygon **with a hole**; the hole is exactly `D17` Huangpu.
- `D04` Chongming is a **MultiPolygon**: two parts separated by a 0.01° channel
  (lon 121.72–121.73) that belongs to no district.
- `D08` Zhabei has `population: null` (tests missing-value handling); `D15` and
  `D16` have the same population (tests ties).
- `area_km2` is the geodesic area on WGS 84 (pyproj.Geod), rounded to 0.01 km².

Tests: import, profile/insight (counts, ranks), counts in areas (with T1).

Expected: `expected.json` (profile), `expected_insight.csv` (rank phrases for
`population`, from `reference/rank_check.py`). The V1 × T1 point counts are in
`../T1/expected_count_in_areas.csv`; V1 × R3 category shares in `../N8/`.

{MADE_BY}
""")
    return feats


# --------------------------------------------------------------------------
# T1 — 248 schools CSV, 6 rows with blank coordinates
# --------------------------------------------------------------------------
T1_BLANK = {12: "both", 57: "both", 101: "both", 150: "both", 199: "lon", 240: "lat"}


def _avoid_edges(v, edges):
    while any(abs(v - e) < 5e-7 for e in edges):
        v = round(v + 1e-6, 6)
    return v


def make_t1():
    d = fresh("T1")
    rng = np.random.default_rng(20260101)
    types = ["Primary", "Middle", "High"]
    rows = []
    lon_edges = LON_EDGES + [121.72, 121.73, 121.40, 121.45]
    lat_edges = LAT_EDGES + [31.25, 31.30]
    for i in range(248):
        t = types[int(rng.integers(0, 3))]
        lon = _avoid_edges(round(float(rng.uniform(121.205, 121.795)), 6), lon_edges)
        lat = _avoid_edges(round(float(rng.uniform(30.905, 31.495)), 6), lat_edges)
        students = int(rng.integers(180, 2600))
        name = f"Shanghai No. {i + 1} {t} School"
        slon, slat = f"{lon:.6f}", f"{lat:.6f}"
        b = T1_BLANK.get(i)
        if b in ("both", "lon"):
            slon = ""
        if b in ("both", "lat"):
            slat = ""
        rows.append([name, t, str(students), slon, slat])
    write_csv(d / "schools.csv", ["name", "type", "students", "lon", "lat"], rows)
    write_json(d / "expected.json", {
        "fixture": "T1",
        "files": ["schools.csv"],
        "kind": "table",
        "rows": 248,
        "delimiter": ",", "encoding": "UTF-8", "header": True,
        "coordinates": {"x": "lon", "y": "lat", "crs_epsg": 4326},
        "fields": {"name": "name", "type": "category", "students": "measure",
                   "lon": "longitude", "lat": "latitude"},
        "categories": {"type": ["High", "Middle", "Primary"]},
        "import_report": {"located": 242, "excluded": 6,
                          "excluded_rows_1based_data": sorted(i + 1 for i in T1_BLANK),
                          "excluded_reason": "blank coordinate"},
        "assumptions": [{"id": "crs_wgs84_assumed",
                         "text": "lon/lat columns in degrees assumed to be WGS 84",
                         "visible": True}],
    })
    write_text(d / "README.md", f"""# T1 — Schools CSV with lon/lat (synthetic)

`schools.csv`: 248 synthetic schools over V1's extent (lon 121.2–121.8,
lat 30.9–31.5), columns `name, type (Primary/Middle/High), students, lon, lat`.
Six rows have blank coordinates (data rows {', '.join(str(i + 1) for i in sorted(T1_BLANK))};
two of them have only one of lon/lat blank). Random draws use seed 20260101;
no point lies exactly on a V1 district boundary.

Tests: visible WGS 84 assumption; import report must say **242 located,
6 excluded**.

Expected: `expected.json`; `expected_count_in_areas.csv` = schools counted in each
V1 district (`reference/count_in_areas.py`, boundary rule: intersects). Points in
Chongming's channel count in no district.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# V2 — mixed geometry GeoJSON
# --------------------------------------------------------------------------
def make_v2():
    d = fresh("V2")
    feats = [
        feature({"id": 1, "name": "Ferry pier", "kind": "point"}, Point(121.49, 31.24)),
        feature({"id": 2, "name": "Riverside walk", "kind": "line"},
                LineString([(121.48, 31.22), (121.49, 31.235), (121.50, 31.25)])),
        feature({"id": 3, "name": "Park", "kind": "area"},
                box(121.45, 31.20, 121.47, 31.215)),
        feature({"id": 4, "name": "Bus stop", "kind": "point"}, Point(121.46, 31.23)),
        feature({"id": 5, "name": "Tram route", "kind": "line"},
                MultiLineString([[(121.40, 31.20), (121.42, 31.21)],
                                 [(121.43, 31.21), (121.45, 31.22)]])),
        feature({"id": 6, "name": "Campus", "kind": "area"},
                MultiPolygon([box(121.50, 31.28, 121.51, 31.29),
                              box(121.52, 31.28, 121.53, 31.29)])),
        feature({"id": 7, "name": "Library", "kind": "point"}, Point(121.475, 31.225)),
        feature({"id": 8, "name": "Canal", "kind": "line"},
                LineString([(121.41, 31.26), (121.44, 31.27)])),
        feature({"id": 9, "name": "Lake", "kind": "area"},
                Polygon([(121.42, 31.30), (121.44, 31.30), (121.43, 31.32), (121.42, 31.30)])),
        feature({"id": 10, "name": "Clinic", "kind": "point"}, Point(121.51, 31.21)),
    ]
    write_geojson(d / "mixed.geojson", feats, name="mixed")
    write_json(d / "expected.json", {
        "fixture": "V2", "files": ["mixed.geojson"], "kind": "vector",
        "split_into_layers": 3,
        "layers": [
            {"geometry": "point", "count": 4, "ids": [1, 4, 7, 10]},
            {"geometry": "line", "count": 3, "ids": [2, 5, 8], "geometry_types": {"LineString": 2, "MultiLineString": 1}},
            {"geometry": "area", "count": 3, "ids": [3, 6, 9], "geometry_types": {"Polygon": 2, "MultiPolygon": 1}},
        ],
        "crs": {"epsg": 4326, "source": "declared (GeoJSON RFC 7946 default)"},
        "fields": {"id": "id", "name": "name", "kind": "category"},
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# V2 — Mixed geometry GeoJSON

`mixed.geojson`: 10 features in one FeatureCollection, interleaved: 4 points,
3 lines (2 LineString + 1 MultiLineString), 3 areas (2 Polygon + 1 MultiPolygon).
The `kind` property states the intended layer.

Tests: mixed-geometry split — expect **3 layers** (points 4, lines 3, areas 3);
Multi* types go with their single-part kind.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# V3 — shapefile, no .prj, degrees
# --------------------------------------------------------------------------
def make_v3():
    d = fresh("V3")
    rng = np.random.default_rng(3)
    pts = [Point(round(float(rng.uniform(121.25, 121.75)), 6), round(float(rng.uniform(30.95, 31.45)), 6))
           for _ in range(25)]
    gdf = gpd.GeoDataFrame({"id": list(range(1, 26)),
                            "name": [f"Site {i}" for i in range(1, 26)],
                            "value": [round(float(v), 2) for v in rng.uniform(0, 100, 25)]},
                           geometry=pts, crs="EPSG:4326")
    write_shp(gdf, d / "sites.shp", keep_prj=False)
    write_json(d / "expected.json", {
        "fixture": "V3", "files": ["sites.shp", "sites.shx", "sites.dbf", "sites.cpg"],
        "kind": "vector",
        "layers": [{"name": "sites", "geometry": "point", "count": 25}],
        "crs": {"epsg": 4326, "source": "assumed"},
        "extent_lonlat_within": [121.25, 30.95, 121.75, 31.45],
        "fields": {"id": "id", "name": "name", "value": "measure"},
        "assumptions": [{"id": "crs_wgs84_assumed",
                         "text": "No .prj; coordinates fall in degree ranges, so WGS 84 is assumed",
                         "visible": True}],
    })
    write_text(d / "README.md", f"""# V3 — Shapefile without .prj, degree coordinates

`sites.shp/.shx/.dbf/.cpg`: 25 points over Shanghai in lon/lat degrees; the
`.prj` file was deliberately deleted (seed 3).

Tests: the app must apply a **visible WGS 84 assumption** (not silently).

{MADE_BY}
""")


# --------------------------------------------------------------------------
# V4 — shapefile, no .prj, CGCS2000 3-degree GK CM 120E (EPSG:4549)
# --------------------------------------------------------------------------
def make_v4():
    d = fresh("V4")
    rows = v1_geometries()
    gdf = gpd.GeoDataFrame({"dist_id": [r[0] for r in rows], "name_en": [r[1] for r in rows]},
                           geometry=[shapely.segmentize(r[4], 0.01) for r in rows], crs="EPSG:4326")
    g2 = gdf.to_crs(4549)
    g2["geometry"] = shapely.set_precision(g2.geometry.values, 0.001)
    write_shp(g2, d / "districts_gk.shp", keep_prj=False)
    b = [round(v, 3) for v in g2.total_bounds]
    crs = pyproj.CRS.from_epsg(4549)
    write_json(d / "expected.json", {
        "fixture": "V4",
        "files": ["districts_gk.shp", "districts_gk.shx", "districts_gk.dbf", "districts_gk.cpg"],
        "kind": "vector",
        "layers": [{"name": "districts_gk", "geometry": "area", "count": 17}],
        "crs": {"epsg": 4549, "name": crs.name, "source": "resolved (no .prj)"},
        "projected_bounds": b,
        "true_extent_lonlat": [121.2, 30.9, 121.8, 31.5],
        "resolver": {
            "question": "Where should this data be?",
            "expected_answer": "Shanghai, China",
            "expected_top_candidate_epsg": 4549,
            "equivalent_candidates_epsg": [2385, 2436],
            "equivalent_note": "Xian 1980 (2385) and Beijing 1954 (2436) 3-degree GK CM 120E use the same "
                               "false easting; they place the data within ~100 m of EPSG:4549.",
            "distinguishable_non_candidates": {
                "4528": "zone-prefixed (false easting 40500000) — x would be ~40.6 million",
                "4550": "CM 123E — would place the data ~3° east, in the East China Sea"},
        },
        "fields": {"dist_id": "id", "name_en": "name"},
        "assumptions": [{"id": "crs_unknown_projected",
                         "text": "No .prj and coordinates are not degrees; user must confirm the place",
                         "visible": True}],
    })
    write_text(d / "README.md", f"""# V4 — Shapefile without .prj, projected (CGCS2000 GK CM 120E)

`districts_gk.*`: V1's 17 districts reprojected to **EPSG:4549 — {crs.name}**
(tmerc, lon_0 = 120, k = 1, x_0 = 500000, GRS80), coordinates rounded to 1 mm;
the `.prj` was deliberately deleted. Edges were densified to 0.01° before
reprojection.

Zone choice (verified with pyproj): the 3° GK zone whose area of use contains
lon 121.5 / central Shanghai is CM 120E (EPSG:4549, area of use 118.5–121.5°E);
the next zone CM 123E (EPSG:4550) starts at 121.5°E. Shanghai straddles the
seam; most of the city (incl. its centre, 121.47°E) is in CM 120E. Eastern
districts lie slightly outside the nominal zone, which is normal for GK data.

Projected bounds (m): {b}.

Tests: the "Where should this data be?" resolver — the expected answer is
Shanghai with EPSG:4549 (see `expected.json` for equivalent / non-candidates).

{MADE_BY}
""")


# --------------------------------------------------------------------------
# V5 — GBK .dbf, no .cpg
# --------------------------------------------------------------------------
V5_ROWS = [
    ("人民广场", "黄浦区", "广场", 121.4737, 31.2304),
    ("外滩", "黄浦区", "景点", 121.4906, 31.2402),
    ("静安寺", "静安区", "寺庙", 121.4453, 31.2235),
    ("徐家汇", "徐汇区", "商圈", 121.4365, 31.1955),
    ("陆家嘴", "浦东新区", "商圈", 121.5055, 31.2397),
    ("虹桥火车站", "闵行区", "交通", 121.3203, 31.1940),
    ("复旦大学", "杨浦区", "学校", 121.5035, 31.2988),
    ("鲁迅公园", "虹口区", "公园", 121.4846, 31.2756),
    ("中山公园", "长宁区", "公园", 121.4178, 31.2198),
    ("七宝古镇", "闵行区", "景点", 121.3500, 31.1560),
    ("朱家角", "青浦区", "古镇", 121.0560, 31.1080),
    ("佘山", "松江区", "景点", 121.1900, 31.0970),
]


def make_v5():
    d = fresh("V5")
    gdf = gpd.GeoDataFrame({"id": list(range(1, len(V5_ROWS) + 1)),
                            "name": [r[0] for r in V5_ROWS],
                            "district": [r[1] for r in V5_ROWS],
                            "category": [r[2] for r in V5_ROWS]},
                           geometry=[Point(r[3], r[4]) for r in V5_ROWS], crs="EPSG:4326")
    write_shp(gdf, d / "places_gbk.shp", encoding="GBK", keep_cpg=False, ldid=0)
    raw = (d / "places_gbk.dbf").read_bytes()
    assert "人民广场".encode("gbk") in raw, "dbf is not GBK-encoded"
    assert "人民广场".encode("utf-8") not in raw
    write_json(d / "expected.json", {
        "fixture": "V5",
        "files": ["places_gbk.shp", "places_gbk.shx", "places_gbk.dbf", "places_gbk.prj"],
        "kind": "vector",
        "layers": [{"name": "places_gbk", "geometry": "point", "count": len(V5_ROWS)}],
        "crs": {"epsg": 4326, "source": "declared (.prj)"},
        "encoding": {"expected": "GBK", "accept": ["GBK", "GB18030", "CP936"],
                     "cpg_present": False, "dbf_language_driver_id": 0,
                     "source": "detected"},
        "fields": {"id": "id", "name": "name", "district": "category", "category": "category"},
        "first_row": {"id": 1, "name": V5_ROWS[0][0], "district": V5_ROWS[0][1], "category": V5_ROWS[0][2]},
        "assumptions": [{"id": "encoding_detected", "text": "Text encoding detected as GBK (no .cpg)",
                         "visible": True}],
    })
    write_text(d / "README.md", f"""# V5 — Shapefile with GBK-encoded .dbf and no .cpg

`places_gbk.*`: {len(V5_ROWS)} Shanghai landmarks (approximate WGS 84 positions)
with Chinese `name`, `district`, `category` attributes. Written by GDAL with
`ENCODING=GBK`, then the `.cpg` was deleted **and the dBASE language-driver byte
(offset 29) was set to 0**, so nothing in the files declares the encoding. The
`.prj` (WGS 84) is kept so only the encoding is under test.

Tests: encoding detection. Expected: GBK (GB18030/CP936 acceptable), first row
name `人民广场`. Decoding as UTF-8 or Latin-1 must not be presented silently.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# V9 — zipped shapefile in nested folder + 3-layer GeoPackage
# --------------------------------------------------------------------------
def make_v9(tmp: Path):
    d = fresh("V9")
    roads = gpd.GeoDataFrame(
        {"road_id": [1, 2, 3, 4, 5, 6],
         "name": ["Yan'an Rd", "Century Ave", "Nanjing Rd", "Huaihai Rd", "Zhongshan Rd", "Inner Ring"],
         "lanes": [8, 8, 4, 4, 6, 6]},
        geometry=[LineString([(121.40, 31.22), (121.50, 31.23)]),
                  LineString([(121.51, 31.24), (121.56, 31.21)]),
                  LineString([(121.44, 31.235), (121.49, 31.24)]),
                  LineString([(121.42, 31.21), (121.48, 31.22)]),
                  LineString([(121.49, 31.20), (121.49, 31.26)]),
                  LineString([(121.42, 31.18), (121.53, 31.18), (121.53, 31.28), (121.42, 31.28), (121.42, 31.18)])],
        crs="EPSG:4326")
    sd = tmp / "v9shp"
    sd.mkdir(parents=True, exist_ok=True)
    write_shp(roads, sd / "roads.shp")
    inner = "export/2024/shanghai/transport/"
    members = [("export/", b""), ("export/2024/", b""), ("export/2024/shanghai/", b""),
               (inner, b"")]
    members += [(inner + p.name, p.read_bytes()) for p in shp_parts(sd / "roads.shp")]
    members.append(("export/2024/readme.txt", b"Synthetic roads, CC0-1.0. Shapefile is in shanghai/transport/.\n"))
    deterministic_zip(d / "roads_bundle.zip", members)

    gp = d / "city_layers.gpkg"
    rng = np.random.default_rng(9)
    schools = gpd.GeoDataFrame({"name": [f"School {i}" for i in range(1, 9)],
                                "students": [int(v) for v in rng.integers(200, 2000, 8)]},
                               geometry=[Point(round(float(x), 6), round(float(y), 6)) for x, y in
                                         zip(rng.uniform(121.40, 121.55, 8), rng.uniform(31.18, 31.28, 8))],
                               crs="EPSG:4326")
    parks = gpd.GeoDataFrame({"name": ["Park A", "Park B", "Park C"], "kind": ["city", "district", "pocket"]},
                             geometry=[box(121.45, 31.20, 121.47, 31.215), box(121.50, 31.25, 121.52, 31.26),
                                       box(121.43, 31.24, 121.435, 31.245)], crs="EPSG:4326")
    pyogrio.write_dataframe(schools, gp, layer="schools", driver="GPKG")
    pyogrio.write_dataframe(roads, gp, layer="roads", driver="GPKG", append=False)
    pyogrio.write_dataframe(parks, gp, layer="parks", driver="GPKG", append=False)
    layers = [str(n) for n, _ in pyogrio.list_layers(gp)]
    assert sorted(layers) == ["parks", "roads", "schools"], layers
    write_json(d / "expected.json", {
        "fixture": "V9", "files": ["roads_bundle.zip", "city_layers.gpkg"], "kind": "vector",
        "archives": {"roads_bundle.zip": {
            "shapefile_path": inner + "roads.shp",
            "layers": [{"name": "roads", "geometry": "line", "count": 6}],
            "crs": {"epsg": 4326, "source": "declared (.prj)"},
            "other_members": ["export/2024/readme.txt"]}},
        "groups": {"city_layers.gpkg": {"layer_count": 3, "layers": [
            {"name": "schools", "geometry": "point", "count": 8},
            {"name": "roads", "geometry": "line", "count": 6},
            {"name": "parks", "geometry": "area", "count": 3}],
            "crs": {"epsg": 4326, "source": "declared"}}},
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# V9 — Archives and groups

- `roads_bundle.zip`: a Shapefile (6 roads, WGS 84 with .prj/.cpg) stored at
  `{inner}roads.shp` — four folders deep — plus a stray `readme.txt`.
  Zip members have fixed timestamps.
- `city_layers.gpkg`: one GeoPackage with **3 layers** — `schools` (8 points),
  `roads` (6 lines, same as the zip), `parks` (3 areas). GPKG timestamps are fixed
  via `OGR_CURRENT_DATE`.

Tests: finding a Shapefile inside a nested zip folder; presenting a multi-layer
file as a group of 3 layers.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# V10 — geometry edge cases
# --------------------------------------------------------------------------
def make_v10():
    d = fresh("V10")
    am_poly = {"type": "Polygon", "coordinates": [[[179.0, -17.0], [-179.0, -17.0], [-179.0, -16.0],
                                                   [179.0, -16.0], [179.0, -17.0]]]}
    am_line = {"type": "LineString", "coordinates": [[179.5, 0.0], [-179.5, 0.0]]}
    ring = [[float(lon), 85.0] for lon in range(-180, 181, 10)] + [[180.0, 90.0], [-180.0, 90.0], [-180.0, 85.0]]
    polar = {"type": "Polygon", "coordinates": [ring]}
    zero = {"type": "Polygon", "coordinates": [[[121.0, 31.0], [121.0, 31.1], [121.0, 31.2], [121.0, 31.0]]]}
    bowtie = {"type": "Polygon", "coordinates": [[[121.0, 31.0], [121.1, 31.1], [121.1, 31.0],
                                                  [121.0, 31.1], [121.0, 31.0]]]}
    control = {"type": "Polygon", "coordinates": [[[121.0, 31.0], [121.1, 31.0], [121.1, 31.1],
                                                   [121.0, 31.1], [121.0, 31.0]]]}
    cases = [
        ("control", "Ordinary valid 0.1° square near Shanghai", control),
        ("antimeridian_polygon", "2°×1° box crossing 180° encoded unsplit (violates RFC 7946 §3.1.9)", am_poly),
        ("antimeridian_line", "1° line along the equator crossing 180°, unsplit", am_line),
        ("polar_polygon", "Cap north of 85°N, encoded with explicit pole vertices", polar),
        ("zero_area_polygon", "Collinear ring along a meridian (a geodesic): zero area", zero),
        ("self_intersecting_polygon", "Bow-tie ring", bowtie),
        ("null_geometry", "geometry: null", None),
        ("empty_polygon", "Polygon with no rings", {"type": "Polygon", "coordinates": []}),
        ("empty_collection", "Empty GeometryCollection", {"type": "GeometryCollection", "geometries": []}),
    ]
    feats = [feature({"case": c, "description": desc}, g) for c, desc, g in cases]
    write_geojson(d / "edge_cases.geojson", feats, name="edge_cases")

    exp = []
    for c, desc, g in cases:
        sg = shapely.geometry.shape(g) if g is not None else None
        e = {"case": c}
        if sg is None or sg.is_empty:
            e.update({"empty": True, "valid": None, "expected": "report as empty / no geometry; do not crash; "
                                                               "keep attributes"})
        else:
            e["valid"] = bool(sg.is_valid)
            if not sg.is_valid:
                e["invalid_reason"] = shapely.validation.explain_validity(sg)
            if sg.geom_type == "Polygon":
                lons, lats = zip(*sg.exterior.coords)
                area, per = GEOD.polygon_area_perimeter(lons, lats)
                e["geodesic_area_m2_geod_edges"] = float(fmt(abs(area)))
            if sg.geom_type == "LineString":
                e["geodesic_length_m"] = float(fmt(GEOD.line_length(*zip(*sg.coords))))
            exp.append(e)
            continue
        exp.append(e)
    by = {e["case"]: e for e in exp}
    by["antimeridian_polygon"]["expected"] = ("treat as crossing 180° (small box ≈ "
                                             f"{by['antimeridian_polygon']['geodesic_area_m2_geod_edges']:.4g} m²), "
                                             "or warn honestly; must NOT draw/measure a 358°-wide polygon")
    by["antimeridian_polygon"]["naive_planar_wrong_width_deg"] = 358
    by["antimeridian_line"]["expected"] = "≈111 km geodesic length; must not report ≈39,900 km"
    cap = zone_area_exact(85.0, 90.0, 360.0)
    by["polar_polygon"]["cap_area_north_of_85N_m2_exact"] = float(fmt(cap))
    by["polar_polygon"]["expected"] = ("area of the polar cap (edges between 10°-spaced vertices are "
                                       "geodesics, so slightly less than the exact parallel-bounded cap); "
                                       "must contain the pole; must not be reported as ~0 or as the "
                                       "rest of the Earth")
    by["zero_area_polygon"]["expected"] = "honest error/warning: zero-area (degenerate) polygon"
    by["self_intersecting_polygon"]["expected"] = ("honest error/warning: self-intersection; if repaired "
                                                   "(make_valid) area = two triangles")
    by["self_intersecting_polygon"]["make_valid_geodesic_area_m2"] = float(fmt(geodesic_area(
        shapely.make_valid(shapely.geometry.shape(bowtie)))))
    by["control"]["expected"] = "imports and measures normally"
    write_json(d / "expected.json", {
        "fixture": "V10", "files": ["edge_cases.geojson"], "kind": "vector",
        "feature_count": len(cases),
        "crs": {"epsg": 4326, "source": "declared (GeoJSON RFC 7946 default)"},
        "cases": exp,
        "notes": "Areas computed with pyproj.Geod (WGS84), polygon edges = geodesics between the given "
                 "vertices. Validity from GEOS (shapely 2).",
    })
    write_text(d / "README.md", f"""# V10 — Geometry edge cases

`edge_cases.geojson`: one feature per case (property `case`):

| case | what |
|---|---|
""" + "".join(f"| `{c}` | {desc} |\n" for c, desc, _ in cases) + f"""
Tests: correctness and **honest errors** — nothing may crash, invalid/empty
geometries must be reported (not silently dropped), antimeridian and polar
shapes must not be measured as if they wrapped the globe.

Expected per-case validity, reasons and geodesic measures are in
`expected.json` (pyproj.Geod on WGS 84).

{MADE_BY}
""")


# --------------------------------------------------------------------------
# T2 — ambiguous coordinate columns with swapped rows
# --------------------------------------------------------------------------
T2_SWAP_XY = [5, 13, 21]      # 1-based data rows where x/y hold lat/lon
T2_SWAP_2 = [9, 26]           # rows where lat2/lng2 are swapped


def make_t2():
    d = fresh("T2")
    rng = np.random.default_rng(22)
    rows = []
    for i in range(1, 31):
        lon, lat = round(float(rng.uniform(121.3, 121.7)), 6), round(float(rng.uniform(31.0, 31.4)), 6)
        lon2, lat2 = round(float(rng.uniform(121.3, 121.7)), 6), round(float(rng.uniform(31.0, 31.4)), 6)
        x, y = (lat, lon) if i in T2_SWAP_XY else (lon, lat)
        la2, ln2 = (lon2, lat2) if i in T2_SWAP_2 else (lat2, lon2)
        rows.append([str(i), f"Delivery {i:02d}", f"{x:.6f}", f"{y:.6f}", f"{la2:.6f}", f"{ln2:.6f}"])
    write_csv(d / "deliveries.csv", ["id", "name", "x", "y", "lat2", "lng2"], rows)
    write_json(d / "expected.json", {
        "fixture": "T2", "files": ["deliveries.csv"], "kind": "table", "rows": 30,
        "question_card": {
            "asked": True,
            "question": "Which columns hold the location?",
            "candidate_pairs": [{"x": "x", "y": "y", "label": "x / y"},
                                {"x": "lng2", "y": "lat2", "label": "lng2 / lat2"}],
            "no_silent_default": True},
        "swapped_rows": {"x/y": T2_SWAP_XY, "lng2/lat2": T2_SWAP_2,
                         "note": "1-based data rows whose values are lat-in-x / lon-in-lat2 etc.; "
                                 "app should flag them (values ~31 in the lon column, ~121 in the lat column)"},
        "fields": {"id": "id", "name": "name", "x": "longitude?", "y": "latitude?",
                   "lat2": "latitude?", "lng2": "longitude?"},
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# T2 — Ambiguous coordinate columns

`deliveries.csv`: 30 rows, columns `id, name, x, y, lat2, lng2`. Both `x/y` and
`lng2/lat2` hold plausible Shanghai lon/lat pairs (pickup / drop-off).
Swapped rows: in data rows {T2_SWAP_XY} `x` holds the latitude and `y` the
longitude; in rows {T2_SWAP_2} `lat2` holds the longitude and `lng2` the latitude.

Tests: the **question card** — the app must ask which pair is the location
rather than silently choosing, and should flag the swapped rows (seed 22).

{MADE_BY}
""")


# --------------------------------------------------------------------------
# T4 — GBK CSV, semicolons, decimal commas, leading zeros, dates
# --------------------------------------------------------------------------
def make_t4():
    d = fresh("T4")
    header = ["编码", "名称", "经度", "纬度", "金额", "日期"]
    base = [("001234", "人民广场站", 121.473701, 31.230416, 1234.5, "2024-03-05"),
            ("000056", "南京东路站", 121.484390, 31.238046, 89.0, "2024-03-06"),
            ("120001", "陆家嘴站", 121.502471, 31.237924, 100200.75, "2024-11-30"),
            ("007700", "徐家汇站", 121.436553, 31.195468, 0.5, "2025-01-01"),
            ("000001", "虹桥火车站", 121.320250, 31.194065, 5000.0, "2025-02-28"),
            ("030303", "静安寺站", 121.445302, 31.223401, 12.34, "2024-07-14"),
            ("100000", "中山公园站", 121.417818, 31.219823, 760.0, "2024-12-31"),
            ("000999", "世纪大道站", 121.527040, 31.229051, 42.0, "2025-06-01"),
            ("045600", "龙阳路站", 121.557379, 31.203520, 3.14, "2024-02-29"),
            ("009000", "上海火车站", 121.455708, 31.249478, 999999.99, "2025-03-15")]

    def dc(v, nd):
        return f"{v:.{nd}f}".replace(".", ",")
    rows = [[c, n, dc(lo, 6), dc(la, 6), dc(m, 2), dt] for c, n, lo, la, m, dt in base]
    write_csv(d / "stations_gbk.csv", header, rows, encoding="gbk", delimiter=";")
    write_csv(d / "expected_raw_utf8.csv", header, rows, encoding="utf-8", delimiter=";")
    assert (d / "stations_gbk.csv").read_bytes().decode("gbk") == (d / "expected_raw_utf8.csv").read_text("utf-8")
    write_json(d / "expected.json", {
        "fixture": "T4", "files": ["stations_gbk.csv"], "kind": "table", "rows": len(rows),
        "encoding": {"expected": "GBK", "accept": ["GBK", "GB18030", "CP936"]},
        "delimiter": ";", "decimal_separator": ",",
        "coordinates": {"x": "经度", "y": "纬度", "crs_epsg": 4326},
        "fields": {"编码": "code", "名称": "name", "经度": "longitude", "纬度": "latitude",
                   "金额": "measure", "日期": "date"},
        "raw_string_preservation": {
            "编码": [r[0] for r in rows],
            "日期": [r[5] for r in rows],
            "rule": "codes keep leading zeros (text, never 1234); raw cell strings are kept verbatim "
                    "alongside any parsed value; decimal-comma numbers parse as 1234,50 -> 1234.5"},
        "parsed_examples": {"row1": {"经度": 121.473701, "纬度": 31.230416, "金额": 1234.5,
                                     "日期": "2024-03-05"}},
        "assumptions": [{"id": "encoding_detected", "visible": True},
                        {"id": "decimal_comma", "visible": True},
                        {"id": "crs_wgs84_assumed", "visible": True}],
    })
    write_text(d / "README.md", f"""# T4 — GBK CSV, semicolons, decimal commas, codes, dates

`stations_gbk.csv`: {len(rows)} rows, **GBK**-encoded, `;`-delimited, decimal
**commas** (`121,473701`, `1234,50`), a code column `编码` with leading zeros
(`001234`, `000056`, …) and ISO dates `日期` (`2024-02-29` is a leap day).
Coordinates are approximate WGS 84 metro station positions.

`expected_raw_utf8.csv` is byte-for-byte the same text re-encoded as UTF-8 —
the exact raw strings the app must preserve.

Tests: raw-string preservation (codes stay text with zeros; raw cell text is
kept), encoding + delimiter + decimal-comma detection.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# T5 — Excel workbook with 3 sheets
# --------------------------------------------------------------------------
def make_t5():
    import openpyxl
    d = fresh("T5")
    wb = openpyxl.Workbook()
    s1 = wb.active
    s1.title = "Summary"
    for r in [["Report", "School survey 2025"], ["Prepared by", "Education bureau (synthetic)"],
              ["Rows", 20], ["Notes", "See sheet Schools"]]:
        s1.append(r)
    s2 = wb.create_sheet("Schools")
    s2.append(["name", "type", "students", "longitude", "latitude"])
    rng = np.random.default_rng(5)
    for i in range(1, 21):
        s2.append([f"School {i:02d}", ["Primary", "Middle", "High"][i % 3], int(rng.integers(200, 2000)),
                   round(float(rng.uniform(121.3, 121.7)), 6), round(float(rng.uniform(31.0, 31.4)), 6)])
    s3 = wb.create_sheet("Budget")
    s3.append(["year", "item", "amount_cny"])
    for y, item, amt in [(2023, "Staff", 1200000), (2023, "Buildings", 450000), (2024, "Staff", 1250000),
                         (2024, "Buildings", 300000), (2025, "Staff", 1310000)]:
        s3.append([y, item, amt])
    fixed = datetime(2026, 1, 1)
    wb.properties.creator = "LayeredMap fixtures"
    wb.properties.created = fixed
    wb.properties.modified = fixed
    wb.properties.lastModifiedBy = "LayeredMap fixtures"
    out = d / "survey.xlsx"
    wb.save(out)
    # openpyxl stamps 'modified' with now(): normalise core.xml, fix zip times
    with zipfile.ZipFile(out) as z:
        members = [(i.filename, z.read(i.filename)) for i in z.infolist()]
    fixed_members = []
    for name, data in members:
        if name == "docProps/core.xml":
            txt = data.decode("utf-8")
            txt = re.sub(r"(<dcterms:(created|modified)[^>]*>)[^<]*(</dcterms:\2>)",
                         r"\g<1>2026-01-01T00:00:00Z\3", txt)
            data = txt.encode("utf-8")
        fixed_members.append((name, data))
    deterministic_zip(out, fixed_members)
    write_json(d / "expected.json", {
        "fixture": "T5", "files": ["survey.xlsx"], "kind": "table",
        "sheets": [{"name": "Summary", "has_coordinates": False},
                   {"name": "Schools", "has_coordinates": True, "rows": 20,
                    "coordinates": {"x": "longitude", "y": "latitude", "crs_epsg": 4326}},
                   {"name": "Budget", "has_coordinates": False}],
        "chosen_sheet": "Schools",
        "sheet_choice": "Schools is the only sheet with coordinates; it is NOT the first sheet — "
                        "the app should pick it (or ask), and say so",
        "fields": {"name": "name", "type": "category", "students": "measure",
                   "longitude": "longitude", "latitude": "latitude"},
        "assumptions": [{"id": "sheet_chosen", "visible": True}, {"id": "crs_wgs84_assumed", "visible": True}],
    })
    write_text(d / "README.md", f"""# T5 — Excel workbook, 3 sheets

`survey.xlsx`: sheets `Summary` (text), `Schools` (20 rows with
`longitude`/`latitude`), `Budget` (no coordinates). Only `Schools` has
coordinates and it is the **second** sheet. Workbook timestamps and zip member
times are fixed for reproducibility (seed 5).

Tests: sheet choice — expect `Schools`.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# C1 / C3 — GCJ-02 and BD-09 points
# --------------------------------------------------------------------------
def _pois(seed, n):
    rng = np.random.default_rng(seed)
    cats = ["restaurant", "cafe", "bank", "hotel", "pharmacy", "school", "park"]
    out = []
    for i in range(1, n + 1):
        lon, lat = round(float(rng.uniform(121.35, 121.65)), 6), round(float(rng.uniform(31.10, 31.35)), 6)
        out.append((f"POI {i:02d}", cats[int(rng.integers(0, len(cats)))], lon, lat))
    return out


def make_c1_c3():
    d = fresh("C1")
    rows, truth = [], []
    for name, cat, lon, lat in _pois(101, 50):
        g = prcoords.wgs_gcj((lat, lon))
        rows.append([name, cat, f"{g.lon:.6f}", f"{g.lat:.6f}"])
        truth.append([name, f"{lon:.6f}", f"{lat:.6f}", f"{g.lon:.6f}", f"{g.lat:.6f}"])
    write_csv(d / "amap_pois.csv", ["name", "category", "lng", "lat"], rows)
    write_csv(d / "expected_wgs84.csv", ["name", "wgs84_lon", "wgs84_lat", "gcj02_lon", "gcj02_lat"], truth)
    write_json(d / "expected.json", {
        "fixture": "C1", "files": ["amap_pois.csv"], "kind": "table", "rows": 50,
        "coordinates": {"x": "lng", "y": "lat"},
        "offset_question": {"asked": True, "suggested": "GCJ-02", "hint": "file name contains 'amap'",
                            "alternatives": ["WGS 84", "BD-09"]},
        "after_conversion": "matches expected_wgs84.csv wgs84_lon/lat within ~1e-5° (rough inverse) "
                            "or ~1e-7° (iterative inverse)",
        "fields": {"name": "name", "category": "category", "lng": "longitude", "lat": "latitude"},
        "assumptions": [{"id": "offset_system_gcj02", "visible": True}],
    })
    write_text(d / "README.md", f"""# C1 — Amap POIs in GCJ-02

`amap_pois.csv`: 50 synthetic POIs over central Shanghai; true WGS 84 positions
(seed 101) were converted to **GCJ-02** with `prcoords.wgs_gcj` (prcoords 1.0.2,
CC0/GPL-3.0+). The file name `amap_…` is the only hint.

`expected_wgs84.csv`: the true WGS 84 coordinates and the GCJ-02 values written.

Tests: the offset question (name hint → suggest GCJ-02).

{MADE_BY}
""")

    d = fresh("C3")
    rows, truth = [], []
    for name, cat, lon, lat in _pois(103, 30):
        g = prcoords.wgs_gcj((lat, lon))
        b = prcoords.gcj_bd(g)
        rows.append([name, cat, f"{b.lon:.6f}", f"{b.lat:.6f}"])
        truth.append([name, f"{lon:.6f}", f"{lat:.6f}", f"{b.lon:.6f}", f"{b.lat:.6f}"])
    write_csv(d / "baidu_pois.csv", ["name", "category", "lng", "lat"], rows)
    write_csv(d / "expected_wgs84.csv", ["name", "wgs84_lon", "wgs84_lat", "bd09_lon", "bd09_lat"], truth)
    write_json(d / "expected.json", {
        "fixture": "C3", "files": ["baidu_pois.csv"], "kind": "table", "rows": 30,
        "coordinates": {"x": "lng", "y": "lat"},
        "offset_question": {"asked": True, "suggested": "BD-09", "hint": "file name contains 'baidu'",
                            "alternatives": ["WGS 84", "GCJ-02"]},
        "fields": {"name": "name", "category": "category", "lng": "longitude", "lat": "latitude"},
        "assumptions": [{"id": "offset_system_bd09", "visible": True}],
    })
    write_text(d / "README.md", f"""# C3 — Baidu POIs in BD-09

`baidu_pois.csv`: 30 synthetic POIs (seed 103), WGS 84 → GCJ-02 → **BD-09**
with prcoords 1.0.2 (`wgs_gcj`, then `gcj_bd`). `expected_wgs84.csv` has the
true WGS 84 coordinates.

Tests: second offset system (BD-09), suggested from the `baidu` name hint.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# R3 — synthetic WorldCover-style land cover
# --------------------------------------------------------------------------
WC_CODES = [10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 100]
WC_NAMES = {10: "Tree cover", 20: "Shrubland", 30: "Grassland", 40: "Cropland", 50: "Built-up",
            60: "Bare / sparse vegetation", 70: "Snow and ice", 80: "Permanent water bodies",
            90: "Herbaceous wetland", 95: "Mangroves", 100: "Moss and lichen"}


def make_r3():
    d = fresh("R3")
    n = 200
    rng = np.random.default_rng(33)
    k = 60
    sx = rng.uniform(121.2, 121.8, k)
    sy = rng.uniform(30.9, 31.5, k)
    weights = np.array([10, 4, 8, 18, 35, 3, 1, 12, 6, 2, 1], float)
    cls = rng.choice(WC_CODES, size=k, p=weights / weights.sum())
    res = 0.6 / n
    xs = 121.2 + (np.arange(n) + 0.5) * res
    ys = 31.5 - (np.arange(n) + 0.5) * res
    X, Y = np.meshgrid(xs, ys)
    dist = (X[..., None] - sx) ** 2 + (Y[..., None] - sy) ** 2
    arr = cls[np.argmin(dist, axis=-1)].astype(np.uint8)
    prof = dict(driver="GTiff", width=n, height=n, count=1, dtype="uint8", crs="EPSG:4326",
                transform=from_bounds(121.2, 30.9, 121.8, 31.5, n, n), nodata=0, compress="deflate")
    with rasterio.open(d / "landcover.tif", "w", **prof) as dst:
        dst.write(arr, 1)
    vals, cnts = np.unique(arr, return_counts=True)
    write_json(d / "expected.json", {
        "fixture": "R3", "files": ["landcover.tif"], "kind": "raster",
        "size": [n, n], "bands": 1, "dtype": "uint8", "nodata": 0,
        "crs": {"epsg": 4326, "source": "declared"},
        "bounds": [121.2, 30.9, 121.8, 31.5], "pixel_size_deg": res,
        "value_role": "category",
        "categories_present": [int(v) for v in vals],
        "pixel_counts": {str(int(v)): int(c) for v, c in zip(vals, cnts)},
        "category_labels": {str(c): WC_NAMES[c] for c in WC_CODES},
        "analysis_rule": "category shares, never means (see N8)",
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# R3 — Land-cover categories (synthetic WorldCover stand-in)

`landcover.tif`: {n}×{n} uint8 GeoTIFF, EPSG:4326, bounds 121.2–121.8°E,
30.9–31.5°N (V1's extent), pixel 0.003°, NoData tag = 0 (no NoData pixels).
Values use the **ESA WorldCover class codes** (10 tree … 80 water … 100 moss)
but the map is **synthetic**: a nearest-seed (Voronoi) pattern from 60 seeds
(seed 33). It is NOT WorldCover data and nothing was downloaded.

Tests: the app must treat values as categories (shares by area), not average
them. Expected shares per V1 district are in `../N8/`.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# R6 — scale/offset, -9999 edges, no NoData tag
# --------------------------------------------------------------------------
R6_X0, R6_Y0, R6_RES, R6_N = 121.375, 31.3125, 1 / 256, 24  # all exact in binary
R6_SCALE, R6_OFFSET = 0.1, -100.0


def make_r6():
    d = fresh("R6")
    n = R6_N
    ii, jj = np.meshgrid(np.arange(n), np.arange(n), indexing="ij")
    raw = (1000 + 40 * ii + 25 * jj + (ii * jj) % 7).astype(np.int16)  # decoded 0 .. ~160
    edge = (ii < 2) | (jj < 2) | (ii >= n - 2) | (jj >= n - 2)
    raw[edge] = -9999
    prof = dict(driver="GTiff", width=n, height=n, count=1, dtype="int16", crs="EPSG:4326",
                transform=from_origin(R6_X0, R6_Y0, R6_RES, R6_RES))
    with rasterio.open(d / "scaled.tif", "w", **prof) as dst:
        dst.write(raw, 1)
        dst.scales = (R6_SCALE,)
        dst.offsets = (R6_OFFSET,)
    with rasterio.open(d / "scaled.tif") as src:
        assert src.nodata is None and src.scales == (R6_SCALE,) and src.offsets == (R6_OFFSET,)
    # sample points (row/col noted for humans; the reference script recomputes)
    c = lambda col: R6_X0 + (col + 0.5) * R6_RES  # noqa: E731
    r = lambda row: R6_Y0 - (row + 0.5) * R6_RES  # noqa: E731
    pts = [("p1", c(5), r(7), "interior cell centre (row 7, col 5)"),
           ("p2", c(12), r(12), "interior cell centre (row 12, col 12)"),
           ("p3", c(0), r(10), "edge cell (col 0) -> raw -9999 -> NoData -> blank"),
           ("p4", c(10), r(23), "edge cell (row 23) -> blank"),
           ("p5", R6_X0 + 8 * R6_RES, R6_Y0 - 9 * R6_RES,
            "exactly on the corner shared by rows 8-9, cols 7-8 -> rule picks row 9, col 8"),
           ("p6", R6_X0 + 6 * R6_RES, r(15), "exactly on the edge between cols 5 and 6 -> col 6"),
           ("p7", 121.30, 31.25, "outside the raster -> blank"),
           ("p8", c(20), r(3), "interior cell centre (row 3, col 20)")]
    write_csv(d / "points.csv", ["point_id", "lon", "lat", "note"],
              [[p, repr(x), repr(y), note] for p, x, y, note in pts])
    write_json(d / "expected.json", {
        "fixture": "R6", "files": ["scaled.tif"], "kind": "raster",
        "size": [n, n], "dtype": "int16", "crs": {"epsg": 4326, "source": "declared"},
        "scale": R6_SCALE, "offset": R6_OFFSET, "nodata_tag": None,
        "decoded_value_rule": "value = raw * scale + offset",
        "suspected_nodata_raw": -9999,
        "suspected_nodata_where": "outer 2-pixel border only",
        "valid_decoded_range": [float(raw[~edge].min()) * R6_SCALE + R6_OFFSET,
                                float(raw[~edge].max()) * R6_SCALE + R6_OFFSET],
        "assumptions": [{"id": "nodata_assumed", "raw_value": -9999, "visible": True,
                         "text": "No NoData tag; -9999 appears only along the edges, so it is treated as NoData"}],
        "notes": "Point sampling expectations in expected_raster_sample.csv (raster_sample.py).",
    })
    write_text(d / "README.md", f"""# R6 — Scale/offset, -9999 edges, no NoData tag

`scaled.tif`: {n}×{n} int16 GeoTIFF, EPSG:4326, origin ({R6_X0}, {R6_Y0}),
pixel 1/256° (all georeferencing values are exact binary fractions so the
boundary sample points are exact). Band has **scale {R6_SCALE} and offset
{R6_OFFSET}** (decoded = raw × scale + offset ≈ 0–160) and raw **-9999 in the outer
2-pixel border only**, with **no NoData tag**.

`points.csv`: 8 sample points (cell centres, edge cells, two exact cell
boundaries, one point outside).

Tests: the NoData assumption (-9999 should be offered as NoData, visibly) and
decoded values. Expected sampled values: `expected_raster_sample.csv`
(`reference/raster_sample.py`; cell-boundary rule documented there).

{MADE_BY}
""")


# --------------------------------------------------------------------------
# N1 / N2 — count rules
# --------------------------------------------------------------------------
N1_AREAS = [("A", box(121.0, 31.0, 121.25, 31.25)),
            ("B", box(121.25, 31.0, 121.5, 31.25)),
            ("C", box(121.375, 31.125, 121.625, 31.375)),
            ("D", box(121.75, 31.0, 122.0, 31.25))]
N1_POINTS = [("P01", 121.125, 31.125), ("P02", 121.0625, 31.0625), ("P03", 121.1875, 31.1875),
             ("P04", 121.25, 31.125), ("P05", 121.25, 31.0625), ("P06", 121.25, 31.25),
             ("P07", 121.3125, 31.0625), ("P08", 121.3125, 31.1875), ("P09", 121.4375, 31.1875),
             ("P10", 121.46875, 31.15625), ("P11", 121.4375, 31.0625), ("P12", 121.5, 31.1875),
             ("P13", 121.5625, 31.3125), ("P14", 121.4375, 31.3125), ("P15", 121.375, 31.25),
             ("P16", 121.0, 31.125), ("P17", 121.6875, 31.125), ("P18", 121.125, 31.4375),
             ("P19", 121.625, 31.375), ("P20", 121.1, 31.2)]

N2_AREAS = [("H", Polygon(box(121.0, 31.0, 121.5, 31.5).exterior.coords,
                          [box(121.125, 31.125, 121.375, 31.375).exterior.coords[::-1]])),
            ("I", box(121.1875, 31.1875, 121.3125, 31.3125)),
            ("M", MultiPolygon([box(121.625, 31.0, 121.75, 31.125), box(121.875, 31.0, 122.0, 31.125)]))]
N2_POINTS = [("Q01", 121.0625, 31.0625), ("Q02", 121.4375, 31.4375), ("Q03", 121.25, 31.15625),
             ("Q04", 121.25, 31.25), ("Q05", 121.125, 31.25), ("Q06", 121.1875, 31.25),
             ("Q07", 121.6875, 31.0625), ("Q08", 121.9375, 31.0625), ("Q09", 121.8125, 31.0625),
             ("Q10", 121.75, 31.0625), ("Q11", 121.5, 31.25), ("Q12", 121.34375, 31.34375)]


def make_n1_n2():
    for fid, areas, points, desc in [
        ("N1", N1_AREAS, N1_POINTS,
         "A and B share the edge lon 121.25 (points P04, P05 on it, P06 on the shared top corner); "
         "C overlaps B in [121.375, 121.5]×[31.125, 31.25] (P09, P10 inside the overlap; P12 on B's east "
         "edge inside C; P15 on B's top edge and C's west edge); D has no points (zero count). P16 lies on "
         "A's outer edge, P19 on C's corner; P17, P18 are outside every area."),
        ("N2", N2_AREAS, N2_POINTS,
         "H is a polygon with a square hole; I is an island inside the hole (not touching it); M is a "
         "MultiPolygon of two squares with a gap. Q03, Q12 are in the hole (count nowhere), Q04 in the "
         "island, Q05 on the hole's ring (counts in H), Q06 on I's edge, Q09 in M's gap (nowhere), Q10 on "
         "M's edge, Q11 on H's outer edge."),
    ]:
        d = fresh(fid)
        write_geojson(d / "areas.geojson", [feature({"area_id": a}, g) for a, g in areas], name="areas")
        write_geojson(d / "points.geojson", [feature({"point_id": p}, Point(x, y)) for p, x, y in points],
                      name="points")
        write_json(d / "expected.json", {
            "fixture": fid, "files": ["areas.geojson", "points.geojson"], "kind": "vector",
            "layers": [{"name": "areas", "geometry": "area", "count": len(areas)},
                       {"name": "points", "geometry": "point", "count": len(points)}],
            "crs": {"epsg": 4326, "source": "declared (GeoJSON RFC 7946 default)"},
            "count_rule": "a point on a boundary counts in every area it touches (intersects); "
                          "overlapping areas each count their points; zero-count areas are listed with 0",
            "expected_files": ["expected_count_in_areas.csv"] + (["expected_which_area.csv"] if fid == "N1" else []),
            "assumptions": [],
        })
        extra = ("\nNote: the plan says \"20 points and 3 areas\"; three areas cannot simultaneously show a "
                 "shared boundary with points on it, an overlap with points in it, and a zero-count area, so a "
                 "fourth area **D** (zero count) was added.\n" if fid == "N1" else "")
        write_text(d / "README.md", f"""# {fid} — Count rules ({'shared boundary, overlap, empty area' if fid == 'N1' else 'holes and multipolygons'})

`areas.geojson` ({len(areas)} areas, property `area_id`) and `points.geojson`
({len(points)} points, `point_id`). All area vertices and every boundary point
are exact binary fractions (multiples of 1/32°), so "on the boundary" is exact
in floating point.

{desc}
{extra}
Rule (see `reference/count_in_areas.py`): GeoPandas `sjoin(predicate="intersects")`,
so boundary points count in every touching area.

Expected: `expected_count_in_areas.csv`{' and `expected_which_area.csv`' if fid == 'N1' else ''}.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# N3 — 4x4 raster zonal mean
# --------------------------------------------------------------------------
N3_NODATA = -9999.0


def make_n3():
    d = fresh("N3")
    vals = np.arange(1, 17, dtype=np.float32).reshape(4, 4)
    vals[1, 1] = N3_NODATA  # value 6 -> NoData, fully inside the polygon
    vals[2, 0] = N3_NODATA  # value 9 -> NoData, half inside the polygon
    prof = dict(driver="GTiff", width=4, height=4, count=1, dtype="float32", crs="EPSG:4326",
                transform=from_origin(10.0, 62.0, 1.0, 1.0), nodata=N3_NODATA)
    with rasterio.open(d / "grid4x4.tif", "w", **prof) as dst:
        dst.write(vals, 1)
    poly = box(10.5, 58.0, 13.5, 61.0)
    write_geojson(d / "polygon.geojson", [feature({"area_id": "Z1"}, poly)], name="polygon")

    # hand calculation with the closed-form (authalic) zone area
    A = {r: zone_area_exact(62 - r - 1, 62 - r, 1.0) for r in range(4)}  # per 1° cell, row r
    cover = {1: {0: .5, 2: 1, 3: .5}, 2: {1: 1, 2: 1, 3: .5}, 3: {0: .5, 1: 1, 2: 1, 3: .5}}
    v64 = vals.astype(np.float64)
    num = sum(v64[r, c] * f * A[r] for r, cs in cover.items() for c, f in cs.items())
    den = sum(f * A[r] for r, cs in cover.items() for c, f in cs.items())
    poly_area = 3 * (A[1] + A[2] + A[3])
    mean, cov = num / den, den / poly_area
    unweighted = sum(float(v64[r, c]) * f for r, cs in cover.items() for c, f in cs.items()) / \
        sum(f for cs in cover.values() for f in cs.values())
    write_json(d / "expected.json", {
        "fixture": "N3", "files": ["grid4x4.tif", "polygon.geojson"], "kind": "raster+vector",
        "raster": {"size": [4, 4], "dtype": "float32", "nodata": N3_NODATA, "bounds": [10, 58, 14, 62],
                   "crs": {"epsg": 4326, "source": "declared"}, "nodata_cells_row_col": [[1, 1], [2, 0]]},
        "hand_calculation": {"mean": float(fmt(mean)), "min": 5.0, "max": 16.0,
                             "coverage": float(fmt(cov)), "polygon_area_m2": float(fmt(poly_area)),
                             "unweighted_mean_for_contrast": unweighted},
        "assumptions": [],
    })
    a1, a2, a3 = (f"{A[r] / 1e6:,.4f}" for r in (1, 2, 3))
    write_text(d / "README.md", f"""# N3 — 4×4 raster, area-weighted mean and coverage

`grid4x4.tif`: 4×4 float32, EPSG:4326, 1° cells, lon 10–14°E, lat 58–62°N
(high latitude so cell areas differ by row). NoData = -9999. Values (row 0 = north):

```
row 0 (61–62°N):   1    2    3    4
row 1 (60–61°N):   5  ND    7    8      (ND was 6)
row 2 (59–60°N):  ND   10   11   12     (ND was 9)
row 3 (58–59°N):  13   14   15   16
```

`polygon.geojson`: one rectangle `Z1` = lon 10.5–13.5, lat 58–61. It covers
rows 1–3; in each row it covers **half** of the first and last columns and all
of the middle two. Its east/west edges are meridians, which split a cell's
area exactly in half, and its north/south edges lie on cell boundaries, so the
exact geodesic answer equals the fractional-cell answer.

## Hand calculation

Area of a 1°×1° cell between latitudes φ1, φ2 on WGS 84 (authalic formula,
`A = b²/2 · Δλ · [q(φ2) − q(φ1)]`, q(φ) = sinφ/(1−e²sin²φ) + 1/(2e)·ln((1+e sinφ)/(1−e sinφ))):

| row | lat | A (km²) |
|---|---|---|
| 1 | 60–61 | {a1} |
| 2 | 59–60 | {a2} |
| 3 | 58–59 | {a3} |

Covered valid cells (value × coverage fraction):

- row 1: 5×0.5, 7×1, 8×0.5 → Σf = 2.0, Σvf = 13.5 (cell 6 is NoData, fraction 1 lost)
- row 2: 10×1, 11×1, 12×0.5 → Σf = 2.5, Σvf = 27.0 (cell 9 is NoData, fraction 0.5 lost)
- row 3: 13×0.5, 14×1, 15×1, 16×0.5 → Σf = 3.0, Σvf = 43.5

mean = (13.5·A1 + 27·A2 + 43.5·A3) / (2·A1 + 2.5·A2 + 3·A3) = **{mean:.10f}**

coverage = (2·A1 + 2.5·A2 + 3·A3) / (3·(A1 + A2 + A3)) = **{cov:.10f}**

min = **5**, max = **16**, polygon area = 3·(A1+A2+A3) = {poly_area / 1e6:,.4f} km².

(For contrast, the unweighted cell-count mean would be 84/7.5 = {unweighted:.4f}; the
southern rows have more area, so the weighted mean is slightly higher.)

`reference/zonal_mean.py` reproduces these with exactextract + a Geod cell-area
weight raster and asserts agreement with this hand calculation to 1e-9.
Expected: `expected_zonal_mean.csv`.

{MADE_BY}
""")


# --------------------------------------------------------------------------
# N5 — geodesic measurement / buffer cases
# --------------------------------------------------------------------------
def make_n5():
    d = fresh("N5")
    feats = [
        feature({"feature_id": "pt_eq", "location": "equator"}, Point(0.0, 0.0)),
        feature({"feature_id": "pt_60n", "location": "60N"}, Point(10.0, 60.0)),
        feature({"feature_id": "pt_am", "location": "antimeridian"}, Point(179.999, 10.0)),
        feature({"feature_id": "line_eq", "location": "equator"}, LineString([(0.0, 0.0), (1.0, 0.0)])),
        feature({"feature_id": "line_60n", "location": "60N"}, LineString([(10.0, 60.0), (11.0, 60.0)])),
        feature({"feature_id": "line_am", "location": "antimeridian"},
                MultiLineString([[(179.5, 10.0), (180.0, 10.0)], [(-180.0, 10.0), (-179.5, 10.0)]])),
        feature({"feature_id": "poly_eq", "location": "equator"}, box(0.0, 0.0, 1.0, 1.0)),
        feature({"feature_id": "poly_60n", "location": "60N"}, box(10.0, 60.0, 11.0, 61.0)),
        feature({"feature_id": "poly_am", "location": "antimeridian"},
                MultiPolygon([box(179.5, 10.0, 180.0, 11.0), box(-180.0, 10.0, -179.5, 11.0)])),
    ]
    write_geojson(d / "cases.geojson", [f for f in feats], name="cases")
    write_json(d / "expected.json", {
        "fixture": "N5", "files": ["cases.geojson"], "kind": "vector", "feature_count": len(feats),
        "crs": {"epsg": 4326, "source": "declared (GeoJSON RFC 7946 default)"},
        "expected_files": ["expected_measure.csv", "expected_buffer.csv"],
        "tolerance": {"length_rel": 1e-9, "area_rel": 1e-9, "buffer_area_rel": 1e-3},
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# N5 — Geodesic accuracy (equator, 60°N, antimeridian)

`cases.geojson`: points, lines and areas at the equator, at 60°N and at the
antimeridian. Antimeridian features are encoded per RFC 7946 (split at 180° into
Multi* parts).

Expected (from `reference/geodesic.py`, WGS 84 ellipsoid):

- `expected_measure.csv`: geodesic length (lines) and area/perimeter (areas);
  edges between vertices are geodesics (pyproj.Geod). For `poly_am` the
  perimeter is left blank because the split encoding adds an artificial 180°
  seam; the area is the sum of both parts.
- `expected_buffer.csv`: buffer areas for 1 km and 50 km. Points: a 360-vertex
  geodesic circle (Geod.fwd). Lines/areas: densified buffer in a local azimuthal
  equidistant projection centred on the feature, projected back and measured
  with Geod. Suggested tolerance 1e-3 relative (the 360-gon under-estimates a
  true circle by ~5e-5).

{MADE_BY}
""")


def make_n8():
    d = fresh("N8")
    write_json(d / "expected.json", {
        "fixture": "N8", "files": ["../R3/landcover.tif", "../V1/districts.geojson"], "kind": "raster+vector",
        "action": "category shares per area",
        "areas": {"file": "../V1/districts.geojson", "id": "district_id", "count": 17},
        "raster": {"file": "../R3/landcover.tif", "value_role": "category"},
        "rule": "share = sum(frac * cell_area) per category / sum(frac * cell_area) over valid cells; "
                "never a mean of category codes",
        "expected_files": ["expected_category_shares.csv"],
        "tolerance": {"share_abs": 1e-6},
        "assumptions": [],
    })
    write_text(d / "README.md", f"""# N8 — Land-cover category shares per district

Inputs (not duplicated): `../R3/landcover.tif` (synthetic WorldCover-style
categories) × `../V1/districts.geojson` (17 districts, id `district_id`).

Expected: `expected_category_shares.csv` — one row per (district, category
present): `share` of the district's valid covered area, and `area_km2`.
Produced by `reference/category_shares.py`: exactextract `weighted_frac` with a
geodesic cell-area weight raster (pyproj.Geod), i.e. fractional pixel coverage
× cell area. Shares sum to 1 per district. D04 (MultiPolygon) and D06 (polygon
with a hole) exercise multipart / hole handling.

Tests: category shares, not means.

{MADE_BY}
""")


def main():
    tmp = FIX / ".tmp_make_handmade"
    if tmp.exists():
        shutil.rmtree(tmp)
    tmp.mkdir(parents=True)
    try:
        make_v1()
        make_t1()
        make_v2()
        make_v3()
        make_v4()
        make_v5()
        make_v9(tmp)
        make_v10()
        make_t2()
        make_t4()
        make_t5()
        make_c1_c3()
        make_r3()
        make_r6()
        make_n1_n2()
        make_n3()
        make_n5()
        make_n8()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print("hand-made fixtures written to", FIX)


if __name__ == "__main__":
    main()
