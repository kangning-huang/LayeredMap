"""Reference solver for the LayeredMap Pro geographic puzzles (docs/lm/puzzles/).

    python -I tools/lm/puzzles/solve.py                       # all puzzles, counts and robustness
    python -I tools/lm/puzzles/solve.py --only P3 P7          # some puzzles
    python -I tools/lm/puzzles/solve.py --json results.json   # machine-readable results
    python -I tools/lm/puzzles/solve.py --update docs/lm/puzzles/puzzles.json \
        --answers docs/lm/puzzles/answers.geojson             # write verification into the docs

Data: the open datasets listed in /Volumes/LaCie/data/layeredmap-puzzles/README.md (WorldClim 2.1
10 arc-minute, Natural Earth 1:10m, GEM Global Active Faults). Pass --data to use another folder.
Derived tables are cached in <data>/cache/ (delete it to rebuild). Run in the `layeredmap-ref`
mamba environment (geopandas, rasterio, shapely, pyproj, scipy).

Each puzzle is a list of clues applied in order. A clue narrows a candidate table: cities (Natural
Earth populated places), islands (Natural Earth land and minor-island polygons) or regions
(8-connected groups of WorldClim cells where raster conditions hold). The solver prints how many
candidates remain after each clue, then re-runs the puzzle with every numeric threshold moved
by the stated step, one at a time, to show how far each threshold can move before the answer
changes.

Method notes (the tolerances a LayeredMap action must meet are in docs/lm/puzzles/mechanics.md):
- Raster values at a city are the value of the cell containing it (rasterio sample, no
  interpolation). At an island they are the mean of the valid cells the island touches, because
  most islands are smaller than one 10' cell (about 18 km).
- Distances are great-circle distances on a sphere of radius 6371.0088 km to the nearest vertex
  of the target lines or polygons, densified to at most 0.02 degrees between vertices, so a
  distance is at most about 1.2 km too long, plus the spherical error (under 0.5%). Every
  distance threshold used in a puzzle keeps a margin well above that.
- Areas of islands are geodesic (pyproj Geod, WGS 84). Areas of raster regions are the sum of
  the cells' areas on the sphere.
"""

import argparse
import json
import math
import os
import sys
from dataclasses import dataclass, field
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import rasterio
import rasterio.windows
from pyproj import Geod
from rasterio import features
from scipy import ndimage
from scipy.spatial import cKDTree
from shapely import get_coordinates, segmentize
from shapely.geometry import mapping, shape

DATA = Path(os.environ.get("LM_PUZZLE_DATA", "/Volumes/LaCie/data/layeredmap-puzzles"))
R_KM = 6371.0088
STEP_DEG = 0.02  # densification step for distance vertices
CACHE_VERSION = 1

RASTERS = {
    "bio_1": "wc2.1_10m_bio_1.tif",  # annual mean temperature, °C
    "bio_12": "wc2.1_10m_bio_12.tif",  # annual precipitation, mm
    "tavg_01": "wc2.1_10m_tavg_01.tif",  # January mean temperature, °C
    "tavg_07": "wc2.1_10m_tavg_07.tif",  # July mean temperature, °C
    "elev": "wc2.1_10m_elev.tif",  # elevation, m
}


# --------------------------------------------------------------------------------------------
# geometry helpers


def xyz(lon, lat):
    lon = np.radians(np.asarray(lon, dtype=float))
    lat = np.radians(np.asarray(lat, dtype=float))
    return np.c_[np.cos(lat) * np.cos(lon), np.cos(lat) * np.sin(lon), np.sin(lat)]


def chord_to_km(d):
    return 2 * R_KM * np.arcsin(np.clip(np.asarray(d) / 2, 0, 1))


def km_to_chord(km):
    return 2 * math.sin(km / (2 * R_KM))


def vertices(geoms, step=STEP_DEG):
    """All vertices of the geometries, densified, with the index of the geometry they came from."""
    coords, labels = [], []
    for i, g in enumerate(segmentize(np.asarray(geoms), step)):
        if g is None or g.is_empty:
            continue
        c = get_coordinates(g)
        coords.append(c)
        labels.append(np.full(len(c), i))
    return np.vstack(coords), np.concatenate(labels)


def nearest_km(target_geoms, lon, lat):
    c, _ = vertices(target_geoms)
    d, _ = cKDTree(xyz(c[:, 0], c[:, 1])).query(xyz(lon, lat))
    return chord_to_km(d)


# --------------------------------------------------------------------------------------------
# the world: datasets and derived tables


class World:
    def __init__(self, data: Path):
        self.data = data
        self.cache = data / "cache"
        self.cache.mkdir(exist_ok=True)
        self._countries = None
        self._rasters = None
        self._cities = None
        self._islands = None

    def path(self, *parts):
        p = self.data.joinpath(*parts)
        if not p.exists():
            sys.exit(f"missing {p}: see {self.data / 'README.md'}")
        return p

    def ne(self, name):
        return gpd.read_file(self.path("naturalearth", name, name + ".shp"))

    @property
    def countries(self):
        if self._countries is None:
            c = self.ne("ne_10m_admin_0_countries")
            self._countries = c[["ADMIN", "ADM0_A3", "SOV_A3", "CONTINENT", "POP_EST", "GDP_MD", "geometry"]]
        return self._countries

    @property
    def rasters(self):
        """Raster arrays (NaN for no data) and the cell grid, all on the same 10' global grid."""
        if self._rasters is None:
            out = {}
            for k, f in RASTERS.items():
                with rasterio.open(self.path("worldclim", f)) as r:
                    a = r.read(1).astype(float)
                    a[(a < -1e4) | (a == r.nodata)] = np.nan
                    out[k] = a
                    transform, shape_ = r.transform, r.shape
            rows, cols = np.indices(shape_)
            out["lon"] = transform.c + (cols + 0.5) * transform.a
            out["lat"] = transform.f + (rows + 0.5) * transform.e
            dlon, dlat = math.radians(transform.a), math.radians(-transform.e)
            top = np.radians(out["lat"] + (-transform.e) / 2)
            out["cell_km2"] = R_KM**2 * dlon * (np.sin(top) - np.sin(top - dlat))
            out["transform"] = transform
            self._rasters = out
        return self._rasters

    def sample(self, key, lon, lat):
        with rasterio.open(self.path("worldclim", RASTERS[key])) as r:
            v = np.array([x[0] for x in r.sample(zip(lon, lat))], dtype=float)
            v[(v < -1e4) | (v == r.nodata)] = np.nan
        return v

    def _cached(self, name, build):
        p = self.cache / f"{name}.v{CACHE_VERSION}.pkl"
        if p.exists():
            return pd.read_pickle(p)
        df = build()
        df.to_pickle(p)
        return df

    @property
    def cities(self):
        if self._cities is None:
            self._cities = self._cached("cities", self._build_cities)
        return self._cities

    def _build_cities(self):
        pp = self.ne("ne_10m_populated_places")
        lon, lat = pp.geometry.x.values, pp.geometry.y.values
        df = pd.DataFrame(
            {
                "name": pp.NAME,
                "name_zh": pp.NAME_ZH,
                "country": pp.ADM0NAME,
                "adm0_a3": pp.ADM0_A3,
                "pop": pp.POP_MAX.astype(float),
                "lon": lon,
                "lat": lat,
            }
        )
        # Country attributes through a point-in-polygon join (populated places' own country
        # codes do not always match the countries layer's).
        pts = gpd.GeoDataFrame(df[[]], geometry=gpd.points_from_xy(lon, lat), crs="EPSG:4326")
        j = gpd.sjoin(pts, self.countries, how="left", predicate="within")
        j = j[~j.index.duplicated()]
        miss = j.ADMIN.isna()
        if miss.any():  # coastal points just outside the 1:10m coastline: nearest country
            near = gpd.sjoin_nearest(pts[miss].to_crs("EPSG:3857"), self.countries.to_crs("EPSG:3857"), how="left")
            near = near[~near.index.duplicated()]
            for col in ["ADMIN", "ADM0_A3", "CONTINENT", "POP_EST", "GDP_MD"]:
                j.loc[miss, col] = near[col].values
        df["country_layer"] = j.ADMIN.values
        df["country_a3"] = j.ADM0_A3.values
        df["continent"] = j.CONTINENT.values
        df["country_pop"] = j.POP_EST.astype(float).values
        for k in RASTERS:
            df[k] = self.sample(k, lon, lat)
        faults = gpd.read_file(self.path("gem", "gem_active_faults_harmonized.geojson"))
        df["d_fault_km"] = nearest_km(faults.geometry.values, lon, lat)
        rivers = self.ne("ne_10m_rivers_lake_centerlines")
        df["d_major_river_km"] = nearest_km(rivers[rivers.scalerank <= 4].geometry.values, lon, lat)
        coast = self.ne("ne_10m_coastline")
        df["d_coast_km"] = nearest_km(coast.geometry.values, lon, lat)
        # Countries within 30 km: the country the city is in, plus every country whose
        # boundary has a vertex within 30 km.
        c, lab = vertices(self.countries.geometry.boundary.values)
        tree = cKDTree(xyz(c[:, 0], c[:, 1]))
        a3 = self.countries.ADM0_A3.values
        hits = tree.query_ball_point(xyz(lon, lat), km_to_chord(30.0))
        df["countries_30km"] = [
            len({a3[lab[h]] for h in hs} | ({df.country_a3[i]} if isinstance(df.country_a3[i], str) else set()))
            for i, hs in enumerate(hits)
        ]
        return df

    @property
    def islands(self):
        if self._islands is None:
            self._islands = self._cached("islands", self._build_islands)
        return self._islands

    def _build_islands(self):
        land = self.ne("ne_10m_land")[["geometry"]]
        minor = self.ne("ne_10m_minor_islands")[["geometry"]]
        g = pd.concat([land, minor]).explode(index_parts=False).reset_index(drop=True)
        g = gpd.GeoDataFrame(g, geometry="geometry", crs="EPSG:4326")
        geod = Geod(ellps="WGS84")
        g["area_km2"] = [abs(geod.geometry_area_perimeter(x)[0]) / 1e6 for x in g.geometry]
        rp = g.geometry.representative_point()
        g["lon"], g["lat"] = rp.x.values, rp.y.values
        # Drop slivers at 0°, 0° (an artefact of the minor-islands layer) and continents.
        g = g[~((g.lon.abs() < 0.01) & (g.lat.abs() < 0.01))]
        g = g.reset_index(drop=True)
        # Distance to the nearest other land: nearest vertex on any other polygon.
        c, lab = vertices(g.geometry.exterior.values)
        tree = cKDTree(xyz(c[:, 0], c[:, 1]))
        near = np.full(len(g), np.nan)
        for i in np.where(g.area_km2 < 2e6)[0]:
            ci = c[lab == i]
            if len(ci) > 400:
                ci = ci[:: len(ci) // 400]
            q = xyz(ci[:, 0], ci[:, 1])
            k = 64
            while True:
                d, j = tree.query(q, k=min(k, len(c)))
                d = np.where(lab[j] != i, d, np.inf)
                if np.isfinite(d).any(axis=1).all() or k >= len(c):
                    near[i] = chord_to_km(d.min())
                    break
                k *= 4
        g["nearest_land_km"] = near
        # Country: the country polygon the island touches, else the nearest within 50 km.
        j = gpd.sjoin(g[["geometry"]], self.countries, how="left", predicate="intersects")
        j = j[~j.index.duplicated()]
        g["country"] = j.ADMIN.values
        g["continent"] = j.CONTINENT.values
        miss = g.country.isna() & (g.area_km2 < 2e6)
        if miss.any():
            m = g[miss].to_crs("EPSG:3857")
            n = gpd.sjoin_nearest(m[["geometry"]], self.countries.to_crs("EPSG:3857"), how="left", max_distance=50_000)
            n = n[~n.index.duplicated()]
            g.loc[miss, "country"] = n.ADMIN.values
            g.loc[miss, "continent"] = n.CONTINENT.values
        # Raster values: mean of the valid cells each island touches.
        rs = self.rasters
        for k in RASTERS:
            vals = []
            for geom in g.geometry:
                if geom.area > 50:  # continents: not needed
                    vals.append(np.nan)
                    continue
                minx, miny, maxx, maxy = geom.bounds
                t = rs["transform"]
                c0 = max(int((minx - t.c) / t.a) - 1, 0)
                c1 = min(int((maxx - t.c) / t.a) + 2, rs[k].shape[1])
                r0 = max(int((maxy - t.f) / t.e) - 1, 0)
                r1 = min(int((miny - t.f) / t.e) + 2, rs[k].shape[0])
                win = rs[k][r0:r1, c0:c1]
                wt = rasterio.windows.transform(rasterio.windows.Window(c0, r0, c1 - c0, r1 - r0), t)
                m = features.geometry_mask([geom], win.shape, wt, all_touched=True, invert=True)
                v = win[m]
                v = v[np.isfinite(v)]
                vals.append(float(v.mean()) if len(v) else np.nan)
            g[k] = vals
        return pd.DataFrame(g.drop(columns="geometry"))

    def island_geometry(self, index):
        land = self.ne("ne_10m_land")[["geometry"]]
        minor = self.ne("ne_10m_minor_islands")[["geometry"]]
        g = pd.concat([land, minor]).explode(index_parts=False).reset_index(drop=True)
        rp = g.geometry.representative_point()
        keep = ~((rp.x.abs() < 0.01) & (rp.y.abs() < 0.01))
        return g[keep.values].reset_index(drop=True).geometry.iloc[index]

    # ---- raster regions

    def regions(self, mask):
        """8-connected regions of a boolean cell mask, as a table, plus the label grid."""
        rs = self.rasters
        lab, n = ndimage.label(mask, structure=np.ones((3, 3)))
        idx = np.arange(1, n + 1)
        if n == 0:
            return pd.DataFrame(columns=["region", "area_km2", "lon", "lat", "cells"]), lab
        area = ndimage.sum(rs["cell_km2"], lab, idx)
        w = rs["cell_km2"]
        lat = ndimage.sum(rs["lat"] * w, lab, idx) / area
        lon = ndimage.sum(rs["lon"] * w, lab, idx) / area
        cells = ndimage.sum(np.ones_like(w), lab, idx)
        df = pd.DataFrame({"region": idx, "area_km2": area, "lon": lon, "lat": lat, "cells": cells.astype(int)})
        return df, lab

    def country_mask(self, admin):
        rs = self.rasters
        geoms = self.countries[self.countries.ADMIN == admin].geometry.values
        return features.rasterize(
            [(g, 1) for g in geoms], out_shape=rs["elev"].shape, transform=rs["transform"], fill=0
        ).astype(bool)


# --------------------------------------------------------------------------------------------
# puzzles


@dataclass
class Param:
    value: float
    step: float  # one robustness step, in the parameter's units
    unit: str = ""


@dataclass
class Puzzle:
    id: str
    title: str
    kind: str  # city, island or region
    params: dict
    answer: str  # the expected answer's name, as the solver reports it
    run: object = field(repr=False)  # (world, params) -> list of (clue, candidates)
    allow: int = 1  # the puzzle text says the answer is unique (1) or one of at most two (2)


def label_cities(df):
    return [f"{r['name']} ({r.country})" for _, r in df.iterrows()]


def p1_kunming(w, p):
    c = w.cities
    out = []
    c = c[(c.country_layer == "China") & (c["pop"] > p["pop"])]
    out.append((f"in China, population over {p['pop']:,.0f}", c))
    c = c[c.elev >= p["elev"]]
    out.append((f"elevation at least {p['elev']:.0f} m", c))
    c = c[c.tavg_01 >= p["jan"]]
    out.append((f"January mean at least {p['jan']:.1f} °C", c))
    return out


def p2_winnipeg(w, p):
    c = w.cities
    out = []
    c = c[c.continent == "North America"]
    out.append(("in North America", c))
    c = c[c["pop"] >= p["pop"]]
    out.append((f"population at least {p['pop']:,.0f}", c))
    c = c[c.tavg_01 <= p["jan"]]
    out.append((f"January mean at most {p['jan']:.1f} °C", c))
    return out


def p3_ulleungdo(w, p):
    i = w.islands
    out = []
    i = i[i.country == "South Korea"]
    out.append(("an island of South Korea", i))
    i = i[(i.area_km2 >= p["area_min"]) & (i.area_km2 <= p["area_max"])]
    out.append((f"area {p['area_min']:.0f}–{p['area_max']:.0f} km²", i))
    i = i[i.nearest_land_km >= p["isolation"]]
    out.append((f"at least {p['isolation']:.0f} km from any other land", i))
    return out


def p4_tea(w, p):
    rs = w.rasters
    out = []
    m = (rs["elev"] >= p["elev_min"]) & (rs["elev"] <= p["elev_max"])
    out.append((f"elevation {p['elev_min']:.0f}–{p['elev_max']:.0f} m", w.regions(m)[0]))
    m &= rs["bio_1"] >= p["annual"]
    out.append((f"annual mean at least {p['annual']:.1f} °C", w.regions(m)[0]))
    m &= rs["bio_12"] >= p["rain"]
    out.append((f"annual rainfall at least {p['rain']:.0f} mm", w.regions(m)[0]))
    m &= rs["tavg_01"] >= p["jan"]
    out.append((f"January mean at least {p['jan']:.1f} °C", w.regions(m)[0]))
    m &= w.country_mask("China")
    df, lab = w.regions(m)
    out.append(("inside China", df))
    df = df[df.area_km2 >= p["min_area"]]
    out.append((f"one connected area of at least {p['min_area']:,.0f} km²", df))
    w._last_regions = (lab, df)
    return out


def p5_zaragoza(w, p):
    c = w.cities
    out = []
    c = c[c.continent == "Europe"]
    out.append(("in Europe", c))
    c = c[c["name"].str.upper().str.startswith("Z")]
    out.append(("name starts with Z", c))
    c = c[c["pop"] >= p["pop"]]
    out.append((f"population at least {p['pop']:,.0f}", c))
    c = c[c.bio_12 <= p["rain"]]
    out.append((f"annual rainfall at most {p['rain']:.0f} mm", c))
    return out


def p6_nairobi(w, p):
    c = w.cities
    out = []
    c = c[c.continent == "Africa"]
    out.append(("in Africa", c))
    c = c[c["pop"] >= p["pop"]]
    out.append((f"population at least {p['pop']:,.0f}", c))
    c = c[c.elev >= p["elev"]]
    out.append((f"elevation at least {p['elev']:.0f} m", c))
    c = c[(c.tavg_07 - c.tavg_01) <= -p["cooler"]]
    out.append((f"July at least {p['cooler']:.1f} °C cooler than January", c))
    c = c[c.d_coast_km >= p["coast"]]
    out.append((f"at least {p['coast']:.0f} km from the sea", c))
    c = c[c.d_fault_km <= p["fault"]]
    out.append((f"within {p['fault']:.0f} km of a mapped active fault", c))
    return out


def p7_iguacu(w, p):
    c = w.cities
    out = []
    c = c[c.continent == "South America"]
    out.append(("in South America", c))
    c = c[c.bio_12 >= p["rain"]]
    out.append((f"annual rainfall at least {p['rain']:.0f} mm", c))
    c = c[c.d_major_river_km <= p["river"]]
    out.append((f"within {p['river']:.0f} km of a major river", c))
    c = c[c.tavg_07 <= p["july"]]
    out.append((f"July mean at most {p['july']:.1f} °C", c))
    c = c[c.countries_30km >= 3]
    out.append(("three countries within 30 km", c))
    c = c[c.country_pop >= p["country_pop"]]
    out.append((f"its country has more than {p['country_pop'] / 1e6:.0f} million people", c))
    return out


def p8_christmas(w, p):
    i = w.islands
    out = []
    i = i[i.lat.abs() <= 23.44]
    out.append(("between the tropics", i))
    i = i[(i.area_km2 >= p["area_min"]) & (i.area_km2 <= p["area_max"])]
    out.append((f"area {p['area_min']:.0f}–{p['area_max']:.0f} km²", i))
    i = i[i.nearest_land_km >= p["isolation"]]
    out.append((f"at least {p['isolation']:.0f} km from any other land", i))
    i = i[i.bio_12 >= p["rain"]]
    out.append((f"annual rainfall at least {p['rain']:.0f} mm", i))
    ratio = i.lon.abs() / i.lat.abs()
    i = i[(ratio >= 10 - p["ratio_tol"]) & (ratio <= 10 + p["ratio_tol"])]
    out.append((f"longitude is ten times latitude (±{p['ratio_tol']:.1f}), ignoring signs", i))
    return out


PUZZLES = [
    Puzzle(
        "P1",
        "Grandma's Flower Postcard",
        "city",
        {"pop": Param(2_000_000, 100_000), "elev": Param(1500, 100, "m"), "jan": Param(5.0, 0.5, "°C")},
        "Kunming",
        p1_kunming,
    ),
    Puzzle(
        "P2",
        "Aurora Week",
        "city",
        {"pop": Param(500_000, 50_000), "jan": Param(-15.0, 0.5, "°C")},
        "Winnipeg",
        p2_winnipeg,
    ),
    Puzzle(
        "P3",
        "Grandpa's Squid Boat",
        "island",
        {
            "area_min": Param(50, 5, "km²"),
            "area_max": Param(100, 5, "km²"),
            "isolation": Param(75, 2, "km"),
        },
        "Ulleungdo",
        p3_ulleungdo,
    ),
    Puzzle(
        "P4",
        "The Tea Caravan Notebook",
        "region",
        {
            "elev_min": Param(1000, 50, "m"),
            "elev_max": Param(2000, 50, "m"),
            "annual": Param(17.0, 0.5, "°C"),
            "rain": Param(1300, 50, "mm"),
            "jan": Param(10.0, 0.5, "°C"),
            "min_area": Param(10000, 1000, "km²"),
        },
        "Simao",
        p4_tea,
    ),
    Puzzle(
        "P5",
        "The Windy Letter Z",
        "city",
        {"pop": Param(300_000, 25_000), "rain": Param(400, 20, "mm")},
        "Zaragoza",
        p5_zaragoza,
    ),
    Puzzle(
        "P6",
        "The Running Club's Secret Hill",
        "city",
        {
            "pop": Param(1_000_000, 250_000),
            "elev": Param(1500, 50, "m"),
            "cooler": Param(1.0, 0.5, "°C"),
            "fault": Param(50, 5, "km"),
            "coast": Param(300, 25, "km"),
        },
        "Nairobi",
        p6_nairobi,
    ),
    Puzzle(
        "P7",
        "Three Flags and a Thousand Waterfalls",
        "city",
        {
            "rain": Param(1500, 50, "mm"),
            "river": Param(10, 2, "km"),
            "july": Param(18.0, 0.5, "°C"),
            "country_pop": Param(100e6, 20e6),
        },
        "Foz do Iguaçu",
        p7_iguacu,
    ),
    Puzzle(
        "P8",
        "The Red Crab Crossing",
        "island",
        {
            "area_min": Param(50, 5, "km²"),
            "area_max": Param(150, 10, "km²"),
            "isolation": Param(300, 10, "km"),
            "rain": Param(1500, 100, "mm"),
            "ratio_tol": Param(0.5, 0.1),
        },
        "Christmas Island",
        p8_christmas,
    ),
]

ISLAND_NAMES = {  # representative point (rounded) → a name, for reporting only
    (130.86, 37.5): "Ulleungdo",
    (105.65, -10.5): "Christmas Island",
    (63.4, -19.72): "Rodrigues",
    (-5.71, -15.95): "Saint Helena",
    (-14.36, -7.93): "Ascension",
    (162.99, 5.33): "Kosrae",
}


def cities_in_region(w, region):
    lab, _ = w._last_regions
    t = w.rasters["transform"]
    c = w.cities
    col = ((c.lon - t.c) / t.a).astype(int).clip(0, lab.shape[1] - 1)
    rr = ((c.lat - t.f) / t.e).astype(int).clip(0, lab.shape[0] - 1)
    return c[lab[rr.values, col.values] == region].sort_values("pop", ascending=False)


def towns_in_region(w, region):
    return [f"{r['name']} ({int(r['pop']):,})" for _, r in cities_in_region(w, region).iterrows()]


def name_of(w, kind, row):
    if kind == "city":
        return row["name"]
    if kind == "island":
        return ISLAND_NAMES.get((round(row.lon, 2), round(row.lat, 2)), f"island at {row.lat:.2f}, {row.lon:.2f}")
    # region: the most populous city in its cells
    inside = cities_in_region(w, row.region)
    if len(inside) == 0:
        return f"region at {row.lat:.2f}, {row.lon:.2f}"
    return inside.sort_values("pop", ascending=False).iloc[0]["name"]


def describe(w, pz, row):
    d = {"name": name_of(w, pz.kind, row), "lat": round(float(row.lat), 3), "lon": round(float(row.lon), 3)}
    if pz.kind == "city":
        d.update(country=row.country, population=int(row["pop"]))
        for k in ["elev", "bio_1", "bio_12", "tavg_01", "tavg_07"]:
            d[k] = round(float(row[k]), 1)
        d["fault_km"] = round(float(row.d_fault_km), 1)
        d["major_river_km"] = round(float(row.d_major_river_km), 1)
        d["coast_km"] = round(float(row.d_coast_km), 1)
        d["countries_within_30km"] = int(row.countries_30km)
    elif pz.kind == "island":
        d.update(country=row.country, area_km2=round(float(row.area_km2), 1))
        d["nearest_land_km"] = round(float(row.nearest_land_km), 1)
        for k in ["bio_1", "bio_12", "tavg_01", "tavg_07"]:
            d[k] = round(float(row[k]), 1)
    else:
        d.update(area_km2=round(float(row.area_km2)), cells=int(row.cells))
        d["towns_inside"] = towns_in_region(w, row.region)[:6]
    return d


def solve(w, pz, values=None):
    p = {k: v.value for k, v in pz.params.items()}
    p.update(values or {})
    steps = pz.run(w, p)
    final = steps[-1][1]
    names = [name_of(w, pz.kind, r) for _, r in final.iterrows()]
    ok = 1 <= len(names) <= pz.allow and pz.answer in names
    return steps, names, ok


def robustness(w, pz, max_steps=8):
    """For each threshold, how many steps it can move down and up with the answer unchanged."""
    out = {}
    for k, prm in pz.params.items():
        rng = {}
        for sign, key in [(-1, "down"), (1, "up")]:
            n = 0
            for s in range(1, max_steps + 1):
                _, _, ok = solve(w, pz, {k: prm.value + sign * s * prm.step})
                if not ok:
                    break
                n = s
            rng[key] = n
        lo = prm.value - rng["down"] * prm.step
        hi = prm.value + rng["up"] * prm.step
        out[k] = {
            "value": prm.value,
            "step": prm.step,
            "unit": prm.unit,
            "steps_down_ok": rng["down"],
            "steps_up_ok": rng["up"],
            "holds_from": lo,
            "holds_to": hi,
            "capped": rng["down"] == max_steps or rng["up"] == max_steps,
        }
    return out


def region_geojson(w):
    lab, df = w._last_regions
    t = w.rasters["transform"]
    m = np.isin(lab, df.region.values).astype(np.uint8)
    polys = [shape(g) for g, v in features.shapes(m, mask=m.astype(bool), transform=t) if v == 1]
    from shapely.ops import unary_union

    return mapping(unary_union(polys))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--data", type=Path, default=DATA)
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--json", type=Path, help="write the results here")
    ap.add_argument("--update", type=Path, help="write each puzzle's verification into this puzzles.json")
    ap.add_argument("--answers", type=Path, help="write the answers as a GeoJSON file")
    ap.add_argument("--no-robustness", action="store_true")
    a = ap.parse_args()
    w = World(a.data)
    results = {}
    answers = []
    failed = False
    for pz in PUZZLES:
        if a.only and pz.id not in a.only:
            continue
        steps, names, ok = solve(w, pz)
        print(f"\n{pz.id} {pz.title}  ({pz.kind}; expected {pz.answer})")
        counts = []
        for i, (clue, cand) in enumerate(steps, 1):
            n = len(cand)
            sample = ""
            if n <= 6:
                sample = ": " + ", ".join(name_of(w, pz.kind, r) for _, r in cand.iterrows())
            print(f"  clue {i}: {clue:<55} {n:>6} left{sample}")
            counts.append({"clue": i, "test": clue, "candidates": n})
        final = steps[-1][1]
        found = [describe(w, pz, r) for _, r in final.iterrows()]
        status = "OK" if ok else "FAIL"
        failed |= not ok
        print(f"  answer: {', '.join(names) or 'none'}  [{status}]")
        for f in found:
            print("   ", json.dumps(f, ensure_ascii=False))
        rob = {} if a.no_robustness else robustness(w, pz)
        for k, r in rob.items():
            cap = " (or more)" if r["capped"] else ""
            print(
                f"  robust: {k:<12} {r['value']:>12,.1f} {r['unit']:<4} holds from {r['holds_from']:,.1f} to "
                f"{r['holds_to']:,.1f}{cap} (step {r['step']:,.1f})"
            )
        results[pz.id] = {
            "title": pz.title,
            "kind": pz.kind,
            "expected": pz.answer,
            "found": found,
            "unique": len(found) == 1,
            "verified": ok,
            "counts": counts,
            "robustness": rob,
        }
        for f in found:
            geom = {"type": "Point", "coordinates": [f["lon"], f["lat"]]}
            if pz.kind == "region":
                geom = region_geojson(w)
            elif pz.kind == "island":
                row = final[final.lat.round(3) == f["lat"]].index[0]
                geom = mapping(w.island_geometry(int(row)))
            answers.append({"type": "Feature", "properties": {"puzzle": pz.id, **f}, "geometry": geom})
    if a.json:
        a.json.write_text(json.dumps(results, ensure_ascii=False, indent=1) + "\n")
    if a.update:
        doc = json.loads(a.update.read_text())
        for p in doc["puzzles"]:
            r = results.get(p["id"])
            if r:
                p["verification"] = {
                    "solver": "tools/lm/puzzles/solve.py",
                    "verified": r["verified"],
                    "found": r["found"],
                    "candidates_after_each_clue": [c["candidates"] for c in r["counts"]],
                    "solver_tests": [c["test"] for c in r["counts"]],
                    "robustness": r["robustness"],
                }
        a.update.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n")
    if a.answers:
        def rnd(o):
            if isinstance(o, float):
                return round(o, 4)
            if isinstance(o, (list, tuple)):
                return [rnd(x) for x in o]
            if isinstance(o, dict):
                return {k: rnd(v) for k, v in o.items()}
            return o

        fc = {"type": "FeatureCollection", "features": rnd(answers)}
        a.answers.write_text(json.dumps(fc, ensure_ascii=False) + "\n")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
