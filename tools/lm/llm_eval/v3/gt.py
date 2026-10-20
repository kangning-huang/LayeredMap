"""Ground truth for harness v3, computed independently of NimbleMap with GeoPandas, Shapely and
pyproj (mamba env ``lm-llm-eval``; run with ``-I`` and PROJ_DATA set, see README).

Semantics follow what the app promises, not how it computes it:
- distances are geodesic on WGS 84. The distance from a place to a line or area is measured from
  the place to the feature's edges taken as straight segments in longitude/latitude (how the files
  store them), densified to 0.02 degrees and measured in an azimuthal equidistant projection
  centred on the place, which gives exact geodesic distances from the centre;
- "within N km" means the distance is at most N km (a line counts if any part is that close);
- point-in-area and line-crosses-area tests are planar in longitude/latitude (a point on the
  boundary counts as inside);
- a raster's value at a point is the value of the cell containing it.

Boundary tolerance: features whose distance lies within ``TOL_KM`` of the limit are "edge"
features. They may be in or out; the answer is accepted anywhere between the strict count and the
loose count, and a highlighted set must contain every strict feature and nothing beyond the loose
set.
"""

from __future__ import annotations

import functools
import math
import os
import sys
from pathlib import Path

import numpy as np

# This env's own PROJ and GDAL data, whatever the shell exports (another install's PROJ_LIB
# breaks pyproj: "proj.db ... from another PROJ installation").
for _var, _sub in (("PROJ_DATA", "share/proj"), ("GDAL_DATA", "share/gdal")):
    _p = Path(sys.prefix) / _sub
    if _p.exists():
        os.environ[_var] = str(_p)
os.environ.pop("PROJ_LIB", None)

TOL_KM = 2.0
DENSIFY_DEG = 0.02

DATA = Path(os.environ.get("LM_EVAL_DATA", "/Volumes/LaCie/lm-eval-data"))
REF = Path(os.environ.get("LM_REFERENCE_DIR", "/Volumes/LaCie/data/layeredmap-reference"))


# ---------------------------------------------------------------------------------------------
# Loading


@functools.lru_cache(maxsize=None)
def load(name: str):
    """A layer as a GeoDataFrame in EPSG:4326, rows in file order (the app's row numbers)."""
    import geopandas as gpd
    import pandas as pd

    def pts(df, x, y):
        return gpd.GeoDataFrame(df, geometry=gpd.points_from_xy(pd.to_numeric(df[x], errors="coerce"), pd.to_numeric(df[y], errors="coerce")), crs=4326)

    files = {
        "storms": DATA / "launch/IBTrACS_storm_tracks_1980-2026.shp",
        "quakes": DATA / "launch/usgs_earthquakes_M6plus_1900-2026.gpkg",
        "tornadoes": DATA / "datasets/tornadoes/1950-2025-torn-aspath/1950-2025-torn-aspath.shp",
    }
    if name in files:
        return gpd.read_file(files[name])
    if name == "volcanoes":
        return pts(pd.read_csv(DATA / "launch/NCEI_volcano_locations.csv"), "longitude", "latitude")
    if name == "usgs":
        return pts(pd.read_csv(DATA / "datasets/usgs-quakes-month/usgs_all_month_2026-10-20.csv"), "longitude", "latitude")
    if name == "airports":
        return pts(pd.read_csv(DATA / "datasets/airports/airports.csv", low_memory=False, keep_default_na=False, na_values=[""]), "longitude_deg", "latitude_deg")
    if name == "nuclear":
        return pts(pd.read_csv(DATA / "datasets/nuclear/nuclear_power_plants.csv"), "Longitude", "Latitude")
    if name in ("cities", "countries", "states_provinces", "rivers", "lakes", "land", "regions", "seas", "coastline", "minor_islands"):
        return gpd.read_file(REF / f"{name}.shp")
    raise KeyError(name)


def city(name: str, country: str | None = None):
    """The World cities row (index, lon, lat) for a name (and country when the name repeats)."""
    c = load("cities")
    m = c[c["name"] == name]
    if country:
        m = m[m["country"] == country]
    if len(m) != 1:
        raise ValueError(f"{name!r} ({country}) matches {len(m)} cities")
    g = m.geometry.iloc[0]
    return int(m.index[0]), g.x, g.y


# ---------------------------------------------------------------------------------------------
# Geodesic distance from one place


def _aeqd(lon: float, lat: float):
    from pyproj import CRS, Transformer

    crs = CRS.from_proj4(f"+proj=aeqd +lat_0={lat} +lon_0={lon} +datum=WGS84 +units=m +no_defs")
    return Transformer.from_crs(4326, crs, always_xy=True)


def _coords(g) -> np.ndarray:
    import shapely

    if g is None or g.is_empty:
        return np.empty((0, 2))
    if g.geom_type in ("Point", "MultiPoint"):
        return shapely.get_coordinates(g)
    g = g.boundary if g.geom_type in ("Polygon", "MultiPolygon") else g
    try:
        return shapely.get_coordinates(shapely.segmentize(g, DENSIFY_DEG))
    except shapely.errors.GEOSException:  # a degenerate line (one point): its coordinates as they are
        return shapely.get_coordinates(g)


def distances_km(lon: float, lat: float, gdf, max_km: float | None = None) -> np.ndarray:
    """Geodesic distance (km) from (lon, lat) to every feature of ``gdf``; features clearly
    farther than ``max_km`` (by a generous box) get +inf without being measured. Areas that
    contain the place are 0."""
    t = _aeqd(lon, lat)
    out = np.full(len(gdf), np.inf)
    b = gdf.geometry.bounds.to_numpy()
    if max_km is not None:
        dlat = max_km / 100.0 + 0.5
        dlon = min(180.0, max_km / (100.0 * max(0.05, math.cos(math.radians(min(89.0, abs(lat) + dlat))))) + 0.5)
        lon2 = lon + 360 if lon < 0 else lon - 360
        near_lat = (b[:, 1] <= lat + dlat) & (b[:, 3] >= lat - dlat)
        near_lon = ((b[:, 0] <= lon + dlon) & (b[:, 2] >= lon - dlon)) | ((b[:, 0] <= lon2 + dlon) & (b[:, 2] >= lon2 - dlon))
        cand = np.nonzero(near_lat & near_lon)[0]
    else:
        cand = np.arange(len(gdf))
    import shapely

    here = shapely.Point(lon, lat)
    for i in cand:
        g = gdf.geometry.iloc[i]
        if g is None or g.is_empty:
            continue
        if g.geom_type in ("Polygon", "MultiPolygon") and g.covers(here):
            out[i] = 0.0
            continue
        if max_km is not None and g.geom_type not in ("Point", "MultiPoint"):
            # Only the part near the place can be the closest: clip the edges to the search box.
            edges = g.boundary if g.geom_type in ("Polygon", "MultiPolygon") else g
            near = edges.intersection(shapely.box(lon - dlon, lat - dlat, lon + dlon, lat + dlat))
            # A degenerate (zero-length) line clips to nothing: measure it whole.
            xy = _coords(g if near.is_empty else near)
        else:
            xy = _coords(g)
        if len(xy) == 0:
            continue
        x, y = t.transform(xy[:, 0], xy[:, 1])
        d = np.hypot(np.asarray(x), np.asarray(y))
        d = d[np.isfinite(d)]
        if len(d):
            out[i] = float(d.min()) / 1000.0
    return out


def within(lon: float, lat: float, gdf, km: float, mask=None) -> dict:
    """Rows of ``gdf`` (optionally only where ``mask``) within ``km`` of the place: strict rows
    (at most km - TOL), edge rows (within TOL of the limit), and each row's distance."""
    d = distances_km(lon, lat, gdf, km + 50)
    if mask is not None:
        d = np.where(np.asarray(mask), d, np.inf)
    strict = np.nonzero(d <= km - TOL_KM)[0].tolist()
    edge = np.nonzero((d > km - TOL_KM) & (d <= km + TOL_KM))[0].tolist()
    exact = np.nonzero(d <= km)[0].tolist()
    return {"strict": strict, "edge": edge, "exact": exact, "dist": {int(i): round(float(d[i]), 3) for i in strict + edge}}


def geod_km(lon1, lat1, lon2, lat2) -> np.ndarray:
    from pyproj import Geod

    _, _, m = Geod(ellps="WGS84").inv(np.broadcast_to(lon1, np.shape(lon2)), np.broadcast_to(lat1, np.shape(lat2)), lon2, lat2)
    return np.asarray(m) / 1000.0


def nearest_point(lon: float, lat: float, gdf, mask=None) -> tuple[int, float, float]:
    """(row, km, km to the second nearest) of the nearest point feature."""
    xs = gdf.geometry.x.to_numpy()
    ys = gdf.geometry.y.to_numpy()
    ok = np.isfinite(xs) & np.isfinite(ys)
    if mask is not None:
        ok &= np.asarray(mask)
    d = np.full(len(gdf), np.inf)
    d[ok] = geod_km(lon, lat, xs[ok], ys[ok])
    order = np.argsort(d)
    return int(order[0]), float(d[order[0]]), float(d[order[1]]) if len(order) > 1 else math.inf


# ---------------------------------------------------------------------------------------------
# Areas


def in_areas(points_gdf, areas_gdf, mask=None) -> dict[int, list[int]]:
    """For each area row, the point rows inside it (planar; boundary counts)."""
    import geopandas as gpd

    p = points_gdf if mask is None else points_gdf[np.asarray(mask)]
    p = p[p.geometry.notna() & ~p.geometry.is_empty]
    j = gpd.sjoin(p[["geometry"]], areas_gdf[["geometry"]], predicate="intersects", how="inner")
    out: dict[int, list[int]] = {}
    for pi, ai in zip(j.index, j["index_right"]):
        out.setdefault(int(ai), []).append(int(pi))
    return out


def lines_in_areas(lines_gdf, areas_gdf, mask=None) -> dict[int, list[int]]:
    return in_areas(lines_gdf, areas_gdf, mask)


JOIN_NEAR_KM = 25.0


def join_areas(points_gdf, areas_gdf, mask=None) -> tuple[dict[int, int], dict[int, int]]:
    """The area each point lies in (first one on a shared boundary), and separately the area
    within JOIN_NEAR_KM of the points in none: the app's join rule (owner's decision 2026-10-17,
    `lm-jobs` JOIN_NEAR_M): points just outside every area take the nearest one."""
    import shapely

    inside = {}
    m = in_areas(points_gdf, areas_gdf, mask)
    for a, pts in sorted(m.items()):
        for p in pts:
            inside.setdefault(p, a)
    pool = range(len(points_gdf)) if mask is None else np.nonzero(np.asarray(mask))[0]
    near = {}
    for i in pool:
        i = int(i)
        if i in inside:
            continue
        p = points_gdf.geometry.iloc[i]
        if p is None or p.is_empty or not isinstance(p, shapely.Point) or not (np.isfinite(p.x) and np.isfinite(p.y)):
            continue
        d = distances_km(p.x, p.y, areas_gdf, JOIN_NEAR_KM)
        j = int(np.argmin(d))
        if d[j] <= JOIN_NEAR_KM:
            near[i] = j
    return inside, near


def area_rows(name: str, field: str, value: str) -> list[int]:
    a = load(name)
    return [int(i) for i in a.index[a[field] == value]]


# ---------------------------------------------------------------------------------------------
# Rasters


def raster_at(path: Path, lons, lats) -> list[float | None]:
    import rasterio

    with rasterio.open(path) as r:
        arr = r.read(1)
        out = []
        for x, y in zip(lons, lats):
            if not (np.isfinite(x) and np.isfinite(y)):
                out.append(None)
                continue
            row, col = r.index(x, y)
            if 0 <= row < arr.shape[0] and 0 <= col < arr.shape[1]:
                v = float(arr[row, col])
                out.append(None if (r.nodata is not None and v == r.nodata) else v)
            else:
                out.append(None)
        return out


def netcdf_at(path: Path, var: str, date: str, lons, lats) -> list[float | None]:
    import xarray as xr

    ds = xr.open_dataset(path)
    a = ds[var].sel(time=np.datetime64(date))
    out = []
    for x, y in zip(lons, lats):
        v = float(a.sel(lat=y, lon=x, method="nearest").values)
        out.append(None if not np.isfinite(v) else v)
    return out
