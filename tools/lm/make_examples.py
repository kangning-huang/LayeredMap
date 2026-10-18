"""Builds the "Try an example" data in assets/examples/ (WP2.9).

    python -I tools/lm/make_examples.py \
        --places   <dir>/ne_50m_populated_places.shp \
        --countries <dir>/ne_110m_admin_0_countries.shp \
        --etopo    <dir>/ETOPO_2022_v1_60s_N90W180_surface.tif \
        --out      assets/examples

Sources, versions and download commands: assets/examples/README.md. Run in the
`layeredmap-ref` mamba environment (geopandas, rasterio, numpy).

- cities.geojson: Natural Earth 1:50m populated places (cities and towns), five fields.
- countries.geojson: Natural Earth 1:110m admin-0 countries, eight fields, not simplified.
- elevation.tif: ETOPO 2022 60 arc-second surface elevation, averaged to 30 arc-minutes
  (720 x 360 cells, each the mean of the 30 x 30 source cells inside it, computed as a block
  mean in numpy), rounded to int16
  metres, Deflate-compressed, EPSG:4326, no NoData value.
"""

import argparse
import json
import math
from pathlib import Path

import geopandas as gpd
import numpy as np
import rasterio
from rasterio.transform import from_origin

PRECISION = 5  # decimal places in GeoJSON coordinates (about 1 m)


def text(v):
    """A text attribute, or None for Natural Earth's empty and -99 placeholders."""
    if v is None or (isinstance(v, float) and math.isnan(v)):
        return None
    s = str(v).strip()
    return None if s in ("", "-99") else s


def integer(v):
    if v is None or (isinstance(v, float) and math.isnan(v)):
        return None
    i = int(round(float(v)))
    return None if i == -99 else i


def rounded(coords):
    if isinstance(coords, (list, tuple)) and coords and isinstance(coords[0], (int, float)):
        return [round(float(c), PRECISION) for c in coords]
    return [rounded(c) for c in coords]


def write_geojson(path, gdf, props):
    features = []
    for geom, p in zip(gdf.geometry, props):
        g = geom.__geo_interface__
        features.append({
            "type": "Feature",
            "properties": p,
            "geometry": {"type": g["type"], "coordinates": rounded(g["coordinates"])},
        })
    fc = {"type": "FeatureCollection", "features": features}
    path.write_text(json.dumps(fc, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")


def places(src, out):
    g = gpd.read_file(src).to_crs(4326)
    g = g.sort_values(["POP_MAX", "NAME"], ascending=[False, True], kind="stable")
    props = [
        {
            "name": text(r.NAME),
            "name_zh": text(r.NAME_ZH),
            "country": text(r.ADM0NAME),
            "population": integer(r.POP_MAX),
            "capital": bool(integer(r.ADM0CAP) == 1),
        }
        for r in g.itertuples()
    ]
    write_geojson(out / "cities.geojson", g, props)
    return len(g)


def countries(src, out):
    g = gpd.read_file(src).to_crs(4326)
    g = g.sort_values("NAME", kind="stable")
    props = [
        {
            "name": text(r.NAME),
            "name_zh": text(r.NAME_ZH),
            # ISO_A3_EH fills the codes ISO_A3 leaves as -99 (France, Norway).
            "iso_a3": text(r.ISO_A3_EH),
            "continent": text(r.CONTINENT),
            "pop_est": integer(r.POP_EST),
            "pop_year": integer(r.POP_YEAR),
            "gdp_md": integer(r.GDP_MD),
            "gdp_year": integer(r.GDP_YEAR),
        }
        for r in g.itertuples()
    ]
    write_geojson(out / "countries.geojson", g, props)
    return len(g)


def elevation(src, out):
    width, height, step = 720, 360, 0.5
    dst_transform = from_origin(-180.0, 90.0, step, step)
    with rasterio.open(src) as s:
        # The source is EPSG:9518 (WGS 84 plus EGM2008 heights): the same horizontal grid as
        # EPSG:4326, 1/60° cells from (-180°, 90°), so each output cell is exactly 30 x 30 source
        # cells and a block mean needs no reprojection (and no vertical transformation).
        k = int(round(step / s.transform.a))
        if s.crs.to_epsg() not in (4326, 9518) or abs(s.transform.e + s.transform.a) > 1e-12 or \
                abs(s.transform.a * k - step) > 1e-9 or (s.transform.c, s.transform.f) != (-180.0, 90.0) or \
                (s.width, s.height) != (width * k, height * k):
            raise SystemExit(f"unexpected source grid: {s.crs} {s.transform} {s.width}x{s.height}")
        data = s.read(1, masked=True).astype(np.float32).filled(np.nan)
    dst = np.nanmean(data.reshape(height, k, width, k), axis=(1, 3), dtype=np.float64)
    if np.isnan(dst).any():
        raise SystemExit(f"{int(np.isnan(dst).sum())} output cells have no source data")
    cells = np.clip(np.rint(dst), -32768, 32767).astype(np.int16)
    profile = {
        "driver": "GTiff",
        "width": width,
        "height": height,
        "count": 1,
        "dtype": "int16",
        "crs": "EPSG:4326",
        "transform": dst_transform,
        "compress": "deflate",
        "predictor": 2,
        "zlevel": 9,
        "tiled": False,
        "blockysize": 8,
    }
    with rasterio.open(out / "elevation.tif", "w", **profile) as d:
        d.write(cells, 1)
        d.update_tags(
            TIFFTAG_IMAGEDESCRIPTION="ETOPO 2022 v1 60 arc-second surface elevation, averaged to 30 arc-minutes (metres)",
            source="ETOPO_2022_v1_60s_N90W180_surface.tif, NOAA NCEI, doi:10.25921/fd45-gt74",
        )
    return int(cells.min()), int(cells.max())


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--places", type=Path, required=True)
    ap.add_argument("--countries", type=Path, required=True)
    ap.add_argument("--etopo", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    print("populated places:", places(a.places, a.out))
    print("countries:", countries(a.countries, a.out))
    print("elevation min/max (m):", elevation(a.etopo, a.out))


if __name__ == "__main__":
    main()
