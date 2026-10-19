#!/usr/bin/env python3
"""Build the hidden built-in reference data for NimbleMap Pro.

Downloads Natural Earth 1:10m layers (v5.1.x) plus NOAA ETOPO 2022 elevation (10') and writes
trimmed, renamed ESRI shapefiles, point-of-view (POV) country diffs, manifest.json and README.md.

    ~/miniforge3/bin/mamba run -n layeredmap-ref python -I \
        tools/lm/reference/build_reference.py [--out DIR] [--offline]

--out      output folder (default /Volumes/LaCie/data/layeredmap-reference); raw downloads go
           to <out>/raw/ and are reused when present.
--offline  never touch the network; use only what is already in raw/.

Requires geopandas, shapely >= 2, pyogrio, numpy, rasterio.

POV diff method (details in the generated README): each POV countries layer is trimmed exactly
like the default layer and diffed against it by match key (iso_a3, or iso_a3|name when iso_a3 is
empty or duplicated in the layer). Every distinct replacement/added shape across all POVs is
stored once in pov/shapes.geojson (with shape_id); manifest.json pov.variants.<code> =
{"replace": {key: shape_id}, "add": [shape_id], "remove": [key]}. The build verifies that
applying each entry to countries.shp reproduces the original variant exactly.
"""
import argparse, datetime, hashlib, json, os, pathlib, shutil, sys, urllib.request, zipfile

NE_BASE = "https://naciscdn.org/naturalearth/10m/{theme}/{name}.zip"
# Elevation: NOAA ETOPO 2022 (public domain), averaged to 10 arc-minutes. WorldClim was used
# first but doesn't allow redistribution without permission (decisions.md, 2026-10-18).
ETOPO_URL = ("https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/"
             "ETOPO_2022_v1_60s_N90W180_surface.tif")
POV_CODES = ("arg bdg bra chn deu egy esp fra gbr grc idn ind iso isr ita jpn kor mar nep nld "
             "pak pol prt pse rus sau swe tur twn ukr usa vnm").split()
CULTURAL = ["ne_10m_admin_0_countries", "ne_10m_admin_1_states_provinces",
            "ne_10m_populated_places"]
PHYSICAL = ["ne_10m_rivers_lake_centerlines", "ne_10m_lakes", "ne_10m_coastline", "ne_10m_land",
            "ne_10m_minor_islands", "ne_10m_geography_regions_polys",
            "ne_10m_geography_marine_polys"]
UA = {"User-Agent": "NimbleMap-reference-builder/1.0"}


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def head_ok(url):
    req = urllib.request.Request(url, method="HEAD", headers=UA)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status == 200
    except Exception:
        return False


def fetch(url, dest, offline):
    dest = pathlib.Path(dest)
    if not dest.exists():
        if offline:
            raise SystemExit(f"--offline but {dest} is missing")
        print(f"  downloading {url}", flush=True)
        tmp = dest.with_suffix(dest.suffix + ".part")
        with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=300) as r, \
                open(tmp, "wb") as f:
            shutil.copyfileobj(r, f)
        tmp.rename(dest)
    return {"url": url, "file": f"raw/{dest.name}", "bytes": dest.stat().st_size,
            "sha256": sha256(dest)}


def download_all(raw, offline):
    """Return (sources dict name -> record, list of existing POV codes)."""
    raw.mkdir(parents=True, exist_ok=True)
    sources = {}
    for theme, names in (("cultural", CULTURAL), ("physical", PHYSICAL)):
        for n in names:
            sources[n] = fetch(NE_BASE.format(theme=theme, name=n), raw / f"{n}.zip", offline)
    povs = []
    for c in POV_CODES:
        n = f"ne_10m_admin_0_countries_{c}"
        url = NE_BASE.format(theme="cultural", name=n)
        if (raw / f"{n}.zip").exists() or (not offline and head_ok(url)):
            sources[n] = fetch(url, raw / f"{n}.zip", offline)
            povs.append(c)
        else:
            print(f"  POV {c}: not available")
    (raw / "etopo").mkdir(exist_ok=True)
    sources["etopo_2022_60s"] = fetch(ETOPO_URL, raw / "etopo" / "ETOPO_2022_v1_60s_N90W180_surface.tif", offline)
    return sources, povs


def read_zip(raw, name):
    import pyogrio
    z = raw / f"{name}.zip"
    with zipfile.ZipFile(z) as zf:
        shp = [m for m in zf.namelist() if m.lower().endswith(".shp")]
    return pyogrio.read_dataframe(f"/vsizip/{z}/{shp[0]}", encoding="UTF-8")


def ne_version(raw, name):
    z = raw / f"{name}.zip"
    with zipfile.ZipFile(z) as zf:
        v = [m for m in zf.namelist() if m.endswith(".VERSION.txt")]
        return zf.read(v[0]).decode().strip() if v else None


# ---------------------------------------------------------------- field trimming

def col(df, *names):
    """Return the first column matching any of names (case-insensitive), or None."""
    low = {c.lower(): c for c in df.columns}
    for n in names:
        if n.lower() in low:
            return df[low[n.lower()]]
    return None


def text(df, *names):
    s = col(df, *names)
    if s is None:
        print(f"    note: no column {names}; field left empty")
        return [""] * len(df)
    return [("" if v is None or (isinstance(v, float) and v != v) else str(v).strip()) for v in s]


def integer(df, *names):
    import pandas as pd
    s = col(df, *names)
    if s is None:
        print(f"    note: no column {names}; field set to 0")
        return [0] * len(df)
    return pd.to_numeric(s, errors="coerce").fillna(0).round().astype("int64").tolist()


def country_codes(df):
    iso = text(df, "ISO_A3")
    adm = text(df, "ADM0_A3")
    return [a if (i in ("", "-99") and a) else (i if i not in ("", "-99") else "") for i, a in
            zip(iso, adm)]


def trim(layer, df):
    """Map a raw Natural Earth frame to the trimmed frame for one output layer."""
    import geopandas as gpd
    f = {}
    if layer == "countries":
        f["name"] = text(df, "ADMIN")
        f["name_zh"] = text(df, "NAME_ZH")
        f["iso_a3"] = country_codes(df)
        f["continent"] = text(df, "CONTINENT")
        f["region"] = text(df, "REGION_UN")
        f["subregion"] = text(df, "SUBREGION")
        f["population"] = integer(df, "POP_EST")
        f["gdp_musd"] = integer(df, "GDP_MD")
        f["pop_year"] = integer(df, "POP_YEAR")
        f["gdp_year"] = integer(df, "GDP_YEAR")
        f["type"] = text(df, "TYPE")
        f["sovereign"] = text(df, "SOVEREIGNT")
    elif layer == "states_provinces":
        f["name"] = text(df, "name")
        f["name_zh"] = text(df, "name_zh")
        f["country"] = text(df, "admin")
        f["iso_a3"] = text(df, "adm0_a3")
        f["type"] = text(df, "type_en")
    elif layer == "cities":
        f["name"] = text(df, "NAME")
        f["name_zh"] = text(df, "NAME_ZH")
        f["country"] = text(df, "ADM0NAME")
        f["iso_a3"] = text(df, "ADM0_A3")
        f["population"] = integer(df, "POP_MAX")
        f["capital"] = ["yes" if v == 1 else "no" for v in integer(df, "ADM0CAP")]
        f["kind"] = text(df, "FEATURECLA")
    elif layer == "rivers":
        f["name"] = text(df, "name")
        f["name_zh"] = text(df, "name_zh")
        f["kind"] = text(df, "featurecla")
        f["importance"] = integer(df, "scalerank")
    elif layer == "lakes":
        f["name"] = text(df, "name")
        f["name_zh"] = text(df, "name_zh")
        f["importance"] = integer(df, "scalerank")
    elif layer == "regions":
        f["name"] = text(df, "NAME")
        f["name_zh"] = text(df, "NAME_ZH")
        f["kind"] = text(df, "FEATURECLA")
        f["region"] = text(df, "REGION")
    elif layer == "seas":
        f["name"] = text(df, "name")
        f["name_zh"] = text(df, "name_zh")
        f["kind"] = text(df, "featurecla")
    else:  # land, minor_islands, coastline
        f["kind"] = text(df, "featurecla")
    out = gpd.GeoDataFrame(f, geometry=df.geometry.values, crs="EPSG:4326")
    if layer == "countries":
        # Antarctica is no country: no state owns it and nobody lives there for good (owner's
        # decision, 2026-10-18). It stays in `land`.
        out = out[out["name"] != "Antarctica"].reset_index(drop=True)
    return out


# ---------------------------------------------------------------- layer catalogue

LAYERS = [  # (output name, source, kind, description)
    ("countries", "ne_10m_admin_0_countries", "areas",
     "Natural Earth 1:10m countries (admin-0, default/de facto view), {ver} boundaries"),
    ("states_provinces", "ne_10m_admin_1_states_provinces", "areas",
     "Natural Earth 1:10m first-level subdivisions (states, provinces, regions), {ver}"),
    ("cities", "ne_10m_populated_places", "points",
     "Natural Earth 1:10m populated places (cities and towns, capitals), {ver}"),
    ("rivers", "ne_10m_rivers_lake_centerlines", "lines",
     "Natural Earth 1:10m rivers and lake centrelines, {ver}"),
    ("lakes", "ne_10m_lakes", "areas", "Natural Earth 1:10m lakes and reservoirs, {ver}"),
    ("regions", "ne_10m_geography_regions_polys", "areas",
     "Natural Earth 1:10m physical regions (mountain ranges, deserts, plateaus, islands...), {ver}"),
    ("seas", "ne_10m_geography_marine_polys", "areas",
     "Natural Earth 1:10m named marine areas (oceans, seas, gulfs, bays, straits), {ver}"),
    ("land", "ne_10m_land", "areas", "Natural Earth 1:10m land polygons, {ver}"),
    ("minor_islands", "ne_10m_minor_islands", "areas",
     "Natural Earth 1:10m minor islands (small islands not in the land layer), {ver}"),
    ("coastline", "ne_10m_coastline", "lines", "Natural Earth 1:10m coastline, {ver}"),
]

FIELD_DOC = {
    "sovereign": ("the sovereign state the area belongs to", None),
    "name": ("English name", None),
    "name_zh": ("Chinese (Simplified) name; empty when Natural Earth has none", None),
    "iso_a3": ("ISO 3166-1 alpha-3 country code (Natural Earth ADM0_A3 where ISO has none, e.g. "
               "FRA, NOR, KOS)", None),
    "continent": ("continent", None),
    "region": ("UN region (countries) / broad world region (physical regions)", None),
    "subregion": ("UN subregion", None),
    "population": ("population estimate", "people"),
    "gdp_musd": ("gross domestic product", "million US$ (current)"),
    "pop_year": ("year of the population estimate", "year"),
    "gdp_year": ("year of the GDP estimate", "year"),
    "country": ("English name of the country it belongs to", None),
    "type": ("kind of subdivision (Province, State, Region, District...)", None),
    "capital": ("\"yes\" if national capital (ADM0CAP = 1), else \"no\"", None),
    "kind": ("Natural Earth feature class", None),
    "importance": ("Natural Earth scalerank: lower = more important/bigger (0 = largest)",
                   "rank"),
}
FIELD_OVERRIDE = {
    ("cities", "population"): ("maximum population estimate of the city/agglomeration (POP_MAX)",
                               "people"),
    ("cities", "iso_a3"): ("Natural Earth ADM0_A3 code of the country (mostly = ISO alpha-3)",
                           None),
    ("states_provinces", "iso_a3"): ("Natural Earth adm0_a3 code of the country "
                                     "(mostly = ISO alpha-3)", None),
    ("cities", "kind"): ("place class: Populated place, Admin-0 capital, Admin-1 capital, ...",
                         None),
    ("rivers", "kind"): ("River, Lake Centerline, ...", None),
    ("regions", "kind"): ("Range/mtn, Desert, Plateau, Island, Plain, Basin, ...", None),
    ("seas", "kind"): ("ocean, sea, gulf, bay, strait, channel, sound, ...", None),
    ("cities", "country"): ("English name of the country (ADM0NAME)", None),
    ("countries", "region"): ("UN region (Africa, Americas, Asia, Europe, Oceania)", None),
    ("countries", "type"): ("Natural Earth TYPE: Sovereign country, Country, Dependency, Disputed, "
                            "Indeterminate or Lease", None),
    ("countries", "sovereign"): ("the sovereign state the area belongs to (SOVEREIGNT), e.g. "
                                 "Denmark for Greenland, United Kingdom for Dhekelia", None),
    ("regions", "region"): ("broad world region (Asia, Africa, Europe, North America...)", None),
}

# Suggested system-region (ISO 3166-1 alpha-2) -> POV code. Everything else -> default.
REGION_TO_POV = {
    "arg": ["AR"], "bdg": ["BD"], "bra": ["BR"], "chn": ["CN", "HK", "MO"], "deu": ["DE"],
    "egy": ["EG"], "esp": ["ES"],
    "fra": ["FR", "GF", "GP", "MQ", "RE", "YT", "PF", "NC", "PM", "WF", "BL", "MF"],
    "gbr": ["GB"], "grc": ["GR"], "idn": ["ID"], "ind": ["IN"], "isr": ["IL"], "ita": ["IT"],
    "jpn": ["JP"], "kor": ["KR"], "mar": ["MA"], "nep": ["NP"], "nld": ["NL"], "pak": ["PK"],
    "pol": ["PL"], "prt": ["PT"], "pse": ["PS"], "rus": ["RU"], "sau": ["SA"], "swe": ["SE"],
    "tur": ["TR"], "twn": ["TW"], "ukr": ["UA"],
    "usa": ["US", "PR", "GU", "VI", "AS", "MP", "UM"], "vnm": ["VN"],
    # "iso" (the ISO 3166 view) is not tied to a region; available for explicit selection.
}


def write_shp(gdf, out, name):
    import pyogrio
    for ext in (".shp", ".shx", ".dbf", ".prj", ".cpg"):
        (out / f"{name}{ext}").unlink(missing_ok=True)
    pyogrio.write_dataframe(gdf, out / f"{name}.shp", driver="ESRI Shapefile", encoding="UTF-8")
    if not (out / f"{name}.cpg").exists():
        (out / f"{name}.cpg").write_text("UTF-8")
    return sum((out / f"{name}{e}").stat().st_size for e in (".shp", ".shx", ".dbf", ".prj",
                                                            ".cpg"))


# ---------------------------------------------------------------- POV diff

def keyed(gdf):
    """Return {key: row index} for a trimmed countries frame (see module docstring)."""
    from collections import Counter
    codes = list(gdf["iso_a3"])
    cnt = Counter(codes)
    keys = [c if (c and cnt[c] == 1) else f"{c}|{n}" for c, n in zip(codes, gdf["name"])]
    # Some variants (e.g. pse) contain unattributed slivers (no code, no name): key them by
    # position. They never match the default layer, so they come out as "added".
    keys = [k if k != "|" else f"|#{i}" for i, k in enumerate(keys)]
    if len(set(keys)) != len(keys):
        raise SystemExit("duplicate country keys even after adding name")
    return keys


def geom_hash(geoms):
    import shapely
    g = shapely.normalize(geoms)  # no set_precision: it can drop tiny parts and merge shapes
    return [hashlib.sha1(shapely.to_wkb(x, output_dimension=2)).hexdigest() for x in g]


def shape_id(ghash, values):
    h = hashlib.sha1((ghash + json.dumps(norm_row(values), ensure_ascii=False))
                     .encode()).hexdigest()
    return h[:12]


def norm_row(row):
    """Canonical attribute tuple: integers (Python or numpy) -> int, missing -> "", else str."""
    import numbers
    return tuple("" if v is None or (isinstance(v, float) and v != v) else
                 (int(v) if isinstance(v, numbers.Integral) else str(v)) for v in row)


def exact_hash(geoms):
    import shapely
    return [hashlib.sha1(shapely.to_wkb(g, output_dimension=2)).hexdigest()
            for g in geoms]


def write_geojson(gdf, path):
    """GeoJSON with Python's shortest round-trip float repr, so every coordinate parses back to
    the identical double (GDAL's fixed-decimal writer loses bits near 0)."""
    from shapely.geometry import mapping
    cols = [c for c in gdf.columns if c != "geometry"]
    feats = []
    for row in gdf.itertuples(index=False):
        props = {}
        for c in cols:
            v = getattr(row, c)
            props[c] = int(v) if hasattr(v, "item") and not isinstance(v, str) else v
        feats.append({"type": "Feature", "properties": props,
                      "geometry": mapping(row.geometry)})
    fc = {"type": "FeatureCollection", "name": pathlib.Path(path).stem,
          "crs": {"type": "name", "properties": {"name": "urn:ogc:def:crs:OGC:1.3:CRS84"}},
          "features": feats}
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("{\"type\":\"FeatureCollection\",\"name\":%s,\"crs\":%s,\"features\":[\n"
                 % (json.dumps(fc["name"]), json.dumps(fc["crs"], separators=(",", ":"))))
        fh.write(",\n".join(json.dumps(x, ensure_ascii=False, separators=(",", ":"))
                            for x in feats))
        fh.write("\n]}\n")


def gpd_frame(items, attrs):
    import geopandas as gpd
    rows = {a: [r[a] for _, r in items] for a in attrs}
    rows["shape_id"] = [sid for sid, _ in items]
    return gpd.GeoDataFrame(rows, geometry=[r.geometry for _, r in items], crs="EPSG:4326")


def pov_diff(base, pov):
    attrs = [c for c in base.columns if c != "geometry"]
    bk, pk = keyed(base), keyed(pov)
    bh, ph = geom_hash(base.geometry.values), geom_hash(pov.geometry.values)
    bi = {k: i for i, k in enumerate(bk)}
    pi = {k: i for i, k in enumerate(pk)}
    changed_geom, changed_attr, added = [], [], []
    for k, j in pi.items():
        if k not in bi:
            added.append(k)
            continue
        i = bi[k]
        if bh[i] != ph[j]:
            changed_geom.append(k)
        elif [base.iloc[i][a] for a in attrs] != [pov.iloc[j][a] for a in attrs]:
            changed_attr.append(k)
    removed = [k for k in bk if k not in pi]
    return pk, changed_geom, changed_attr, added, removed


# ---------------------------------------------------------------- main

def fsize(p):
    return pathlib.Path(p).stat().st_size


def mb(n):
    return f"{n / 1e6:.1f} MB"


def raster_info(path):
    try:
        import rasterio
        with rasterio.open(path) as r:
            band = r.read(1, masked=True)
            return {"width": r.width, "height": r.height, "crs": str(r.crs),
                    "nodata": r.nodata, "dtype": r.dtypes[0],
                    "min": float(band.min()), "max": float(band.max())}
    except ImportError:
        from osgeo import gdal
        ds = gdal.Open(str(path))
        b = ds.GetRasterBand(1)
        mn, mx = b.ComputeRasterMinMax(False)
        return {"width": ds.RasterXSize, "height": ds.RasterYSize, "crs": "EPSG:4326",
                "nodata": b.GetNoDataValue(), "dtype": gdal.GetDataTypeName(b.DataType),
                "min": mn, "max": mx}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", default="/Volumes/LaCie/data/layeredmap-reference")
    ap.add_argument("--offline", action="store_true")
    a = ap.parse_args()
    out = pathlib.Path(a.out)
    raw = out / "raw"
    print("== download")
    sources, povs = download_all(raw, a.offline)
    print(f"  {len(povs)} POV variants exist: {' '.join(povs)}")
    missing_povs = [c for c in POV_CODES if c not in povs]

    manifest = {"name": "NimbleMap Pro built-in reference data", "crs": "EPSG:4326",
                "built": datetime.date.today().isoformat(), "layers": {}, "pov": {}}
    sizes = {}
    print("== layers")
    base = None
    for name, src, kind, desc in LAYERS:
        raw_df = read_zip(raw, src)
        if raw_df.crs is not None and raw_df.crs.to_epsg() != 4326:
            raise SystemExit(f"{src}: unexpected CRS {raw_df.crs}")
        gdf = trim(name, raw_df)
        if name == "countries":
            base = gdf
        ver = ne_version(raw, src)
        sizes[name] = write_shp(gdf, out, name)
        fields = {}
        for c in gdf.columns:
            if c == "geometry":
                continue
            meaning, unit = FIELD_OVERRIDE.get((name, c), FIELD_DOC[c])
            fields[c] = {"meaning": meaning, "unit": unit,
                         "type": "integer" if str(gdf[c].dtype).startswith("int") else "text"}
        manifest["layers"][name] = {
            "file": f"{name}.shp", "kind": kind,
            "description": desc.format(ver=f"Natural Earth v{ver}"),
            "source": src, "source_version": ver, "features": len(gdf), "fields": fields,
            "bytes": sizes[name]}
        print(f"  {name:17s} {len(gdf):6d} features  {mb(sizes[name])}")

    import numpy as np
    import rasterio
    from rasterio.enums import Resampling
    with rasterio.open(raw / "etopo" / "ETOPO_2022_v1_60s_N90W180_surface.tif") as src:
        w, h = 2160, 1080  # 10 arc-minutes, the cell mean of the 1-minute grid
        data = src.read(1, out_shape=(h, w), resampling=Resampling.average)
        t = src.transform * src.transform.scale(src.width / w, src.height / h)
    prof = dict(driver="GTiff", width=w, height=h, count=1, dtype="int16", crs="EPSG:4326", transform=t,
                nodata=-32768, compress="deflate", predictor=2)
    with rasterio.open(out / "elevation.tif", "w", **prof) as d:
        d.write(np.round(data).astype("int16"), 1)
    sizes["elevation"] = fsize(out / "elevation.tif")
    sources["etopo_2022_60s"]["tif_sha256"] = sha256(out / "elevation.tif")
    ri = raster_info(out / "elevation.tif")
    manifest["layers"]["elevation"] = {
        "file": "elevation.tif", "kind": "raster",
        "description": "NOAA ETOPO 2022 global relief, averaged to a 10 arc-minute grid (~18.5 km); land and sea floor",
        "source": "etopo_2022_60s", "source_version": "ETOPO 2022 v1", "features": None,
        "raster": ri, "fields": {"band 1": {"meaning": "elevation above sea level, negative on the sea floor "
                                            "(cell mean of the 1-minute grid)", "unit": "m"}},
        "bytes": sizes["elevation"]}
    print(f"  elevation         {ri['width']}x{ri['height']} raster  {mb(sizes['elevation'])}")

    print("== points of view")
    pov_dir = out / "pov"
    pov_dir.mkdir(exist_ok=True)
    for old in sorted(pov_dir.glob("*"), key=lambda q: q.name.startswith("._")):
        old.unlink(missing_ok=True)  # exFAT: AppleDouble ._ files vanish with their parents
    import pyogrio
    import pandas as pd
    attrs = [c for c in base.columns if c != "geometry"]
    base_keys = keyed(base)
    names = dict(zip(base_keys, base["name"]))
    shapes = {}        # shape_id -> row (trimmed fields + geometry)
    variants, stats, originals = {}, {}, {}
    for c in povs:
        p = trim("countries", read_zip(raw, f"ne_10m_admin_0_countries_{c}"))
        originals[c] = p
        pk, cg, ca, ad, rm = pov_diff(base, p)
        ph = geom_hash(p.geometry.values)
        ids = {}
        for k in cg + ca + ad:
            i = pk.index(k)
            row = p.iloc[i]
            sid = shape_id(ph[i], [row[a] for a in attrs])
            shapes.setdefault(sid, row)
            ids[k] = sid
        variants[c] = {"replace": {k: ids[k] for k in cg + ca}, "add": [ids[k] for k in ad],
                       "remove": rm}
        stats[c] = (len(cg), len(ca), len(ad), len(rm))
        for k in cg + ca + rm:
            names.setdefault(k, None)
    shp_gdf = gpd_frame(list(shapes.items()), attrs)
    f = pov_dir / "shapes.geojson"
    write_geojson(shp_gdf, f)
    sizes["pov/shapes"] = fsize(f)
    by_geom = pd.Series(geom_hash(shp_gdf.geometry.values)).value_counts()
    print(f"  pov/shapes.geojson: {len(shp_gdf)} distinct shapes "
          f"({int((by_geom > 1).sum())} geometries shared by >1 attribute set), "
          f"{mb(sizes['pov/shapes'])}")
    # verify: applying the manifest (reading shapes back from disk) reproduces every view
    back = pyogrio.read_dataframe(f)
    back_by_id = {r.shape_id: r for r in back.itertuples(index=False)}
    for c in povs:
        v = variants[c]
        drop = set(v["remove"]) | set(v["replace"])
        keep = [i for i, k in enumerate(base_keys) if k not in drop]
        geoms = list(base.geometry.values[keep])
        rows = [tuple(base.iloc[i][a] for a in attrs) for i in keep]
        for sid in list(v["replace"].values()) + v["add"]:
            r = back_by_id[sid]
            geoms.append(r.geometry)
            rows.append(tuple(getattr(r, a) for a in attrs))
        o = originals[c]
        got = sorted(zip(exact_hash(geoms), map(norm_row, rows)))
        want = sorted(zip(exact_hash(o.geometry.values),
                          (norm_row(tuple(o.iloc[i][a] for a in attrs)) for i in range(len(o)))))
        ok = got == want
        n_cg, n_ca, n_ad, n_rm = stats[c]
        print(f"  {c}: replace {n_cg + n_ca} ({n_ca} attribute-only), add {n_ad}, "
              f"remove {n_rm}; rebuilt {len(got)}/{len(o)} features, exact match: {ok}")
        if not ok:
            gs, ws = set(got), set(want)
            print("    only in rebuilt:", sorted(gs - ws)[:5])
            print("    only in original:", sorted(ws - gs)[:5])
            raise SystemExit(f"POV {c}: applying the manifest does not reproduce the view")
    region_map = {r: c for c, rs in REGION_TO_POV.items() if c in povs for r in rs}
    manifest["pov"] = {
        "default": "countries.shp (Natural Earth default / de facto view)",
        "shapes": "pov/shapes.geojson",
        "codes": povs, "missing_codes": missing_povs,
        "match_key": "For each feature of countries.shp: let c = its iso_a3 field. If c is "
                     "non-empty and occurs exactly once among all features of countries.shp, "
                     "key = c; otherwise key = c + '|' + name (name = the name field).",
        "how_to_apply": "start from countries.shp; drop every feature whose match key is in "
                        "remove or is a key of replace; append the shapes (from "
                        "pov/shapes.geojson, matched by shape_id) named by the values of "
                        "replace and by add.",
        "key_names": {k: names[k] for k in sorted(k for k in names if k in
                      {x for v in variants.values() for x in list(v["replace"]) + v["remove"]})},
        "region_to_pov": dict(sorted(region_map.items())), "region_default": "default",
        "shape_ids": sorted(shapes), "variants": variants}
    manifest["pov_stats"] = {c: dict(zip(("replaced_geometry", "replaced_attributes_only",
                                          "added", "removed"), st)) for c, st in stats.items()}
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1))
    write_readme(out, sources, manifest, sizes, povs, a)
    for junk in out.rglob("._*"):
        junk.unlink()
    total = sum(fsize(p) for p in out.rglob("*") if p.is_file() and "raw" not in p.relative_to(
        out).parts)
    print(f"== total (excluding raw/): {mb(total)}  (pov/shapes.geojson {mb(sizes['pov/shapes'])})")


def write_readme(out, sources, manifest, sizes, povs, args):
    L = manifest["layers"]
    rows = "\n".join(
        f"| `{v['file']}` | {v['kind']} | {v['features'] if v['features'] is not None else '-'} | "
        f"{mb(v['bytes'])} | {v['source']} (v{v['source_version']}) |" for v in L.values())
    fields = []
    for n, v in L.items():
        if n == "elevation":
            continue
        src_map = FIELD_SOURCES.get(n, {})
        fs = ", ".join(f"`{f}` <- {src_map.get(f, '?')}" for f in v["fields"])
        fields.append(f"- **{n}**: {fs}")
    srows = "\n".join(
        f"| {k} | {s['url']} | {s['bytes']:,} | `{s['sha256']}` |" for k, s in sources.items())
    pv = manifest["pov_stats"]
    shapes_mb = mb(sizes["pov/shapes"])
    example_code = min(pv, key=lambda c: len(json.dumps(manifest["pov"]["variants"][c])))
    example = json.dumps({example_code: manifest["pov"]["variants"][example_code]},
                         ensure_ascii=False, indent=1)
    prows = "\n".join(
        f"| {c} | {v['replaced_geometry']} | {v['replaced_attributes_only']} | {v['added']} | "
        f"{v['removed']} |" for c, v in pv.items())
    total = sum(fsize(p) for p in out.rglob("*") if p.is_file() and "raw" not in
                p.relative_to(out).parts)
    elev = sources["etopo_2022_60s"]
    text = f"""# NimbleMap Pro built-in reference data

Hidden reference layers bundled with NimbleMap Pro, so the assistant can answer questions such as
"which country has the most power plants" with only the user's own layer open. Built
{manifest['built']} by `tools/lm/reference/build_reference.py` in the NimbleMap repo.

Location: `{out}` (raw downloads in `raw/`, not part of the bundle).
Total size excluding `raw/`: **{mb(total)}**. All vector layers are ESRI shapefiles
(.shp .shx .dbf .prj .cpg, UTF-8), WGS 84 (EPSG:4326), geometry exactly as published by Natural
Earth (no simplification, no reprojection). `manifest.json` is the machine-readable catalogue the
app reads (layers, kinds, descriptions, counts, field meanings and units, POV codes and the
region -> POV mapping).

## Contents

| file | kind | features | size | source |
|---|---|---|---|---|
{rows}
| `pov/shapes.geojson` | areas | {len(manifest['pov']['shape_ids'])} | {shapes_mb} | ne_10m_admin_0_countries_<pov> (32 variants) |
| `manifest.json` | catalogue | | | |

## Sources

Natural Earth 1:10m, version 5.1.x (per-layer VERSION file: see table above; some physical
layers were last revised in 4.1/5.0 and are distributed unchanged in the 5.1 release), downloaded
{manifest['built']} from `https://naciscdn.org/naturalearth/10m/...`.
NOAA ETOPO 2022 global relief (60 arc-second surface elevation), averaged to 10 arc-minutes.

| dataset | URL | bytes | SHA-256 |
|---|---|---|---|
{srows}

Elevation: `raw/etopo/ETOPO_2022_v1_60s_N90W180_surface.tif`, averaged (cell mean) to a 2160 × 1080
grid, rounded to whole metres (int16), written as WGS 84. SHA-256 of `elevation.tif`:
`{elev['tif_sha256']}`. WorldClim's elevation was used in the first build (2026-10-18) and
replaced the same day: WorldClim doesn't allow redistribution without permission.

## Licences

- Natural Earth: public domain (https://www.naturalearthdata.com/about/terms-of-use/). No
  attribution required; "Made with Natural Earth" is appreciated.
- NOAA ETOPO 2022: public domain ("not subject to copyright protection in the United States";
  CC0 1.0 worldwide). Cite: NOAA National Centers for Environmental Information. 2022: ETOPO 2022
  15 Arc-Second Global Relief Model. https://doi.org/10.25921/fd45-gt74

## Transformations

Only attribute columns were dropped/renamed; text values are whitespace-trimmed, numbers are
cast to integers (missing -> 0), geometry is copied unchanged.

{chr(10).join(fields)}
- **elevation.tif**: ETOPO 2022 averaged to 10 arc-minutes (metres; land and sea floor, negative
  below sea level; no-data {manifest['layers']['elevation']['raster']['nodata']}).

`iso_a3` in countries: ISO_A3, or ADM0_A3 when ISO_A3 is -99 (France FRA, Norway NOR, Kosovo KOS,
Somaliland SOL, and small disputed/special areas). `capital` in cities is "yes" when ADM0CAP = 1.

## Points of view (country borders by system region)

Natural Earth publishes the admin-0 countries layer in {len(povs)} "point of view" variants
(all codes requested existed: {' '.join(povs)}). The default `countries` layer is stored once;
each variant is stored as a diff against it:

1. The variant is trimmed with exactly the same field mapping as `countries`.
2. **Match key.** For each feature of a layer, let `c` = its `iso_a3` field (ISO_A3, or ADM0_A3
   when ISO_A3 is -99). If `c` is non-empty and occurs exactly once among all features of that
   layer, key = `c`; otherwise key = `c + "|" + name` (exact strings, no case folding). The app
   only ever computes keys on `countries.shp`; in the current default layer every `iso_a3` is
   non-empty and unique (258 features), so every key is just the 3-letter code; implement the
   `|name` branch anyway for future data. (Variants contain a few
   features with neither code nor name, e.g. 9 unattributed disputed slivers in `pse`; during the
   diff they get the internal key `|#<row>` and are always "added", so such keys never appear in
   the manifest.)
3. Geometries are compared by SHA-1 of the WKB of `shapely.normalize(g)` (exact coordinates; only ring
   start/orientation and part order are normalized).
   Same key + different geometry (or different trimmed attributes) = **replace**; key only in
   the variant = **add**; key only in the default = **remove**.
4. **Shared shapes.** Every distinct replacement/added feature across all variants is stored once
   in `pov/shapes.geojson` (uncompressed GeoJSON, one feature per line, coordinates written in
   the shortest decimal form that parses back to the identical IEEE double; a correctly
   rounding parser, e.g. serde_json with the `float_roundtrip` feature, gets bit-exact
   geometry), with the trimmed country fields plus `shape_id`. One
   feature per distinct (geometry, attributes) pair; `shape_id` = first 12 hex digits of
   SHA-1(geometry hash + JSON of the trimmed attribute values), so it is stable across rebuilds
   as long as the source data is unchanged. Deduplicating by geometry alone was not worth it:
   in this release only one geometry is used with two attribute sets (the chn "China" shape and
   the twn "Taiwan" shape are the identical polygon), costing ~1 MB.
5. `manifest.json` -> `pov.variants.<code>` =
   `{{"replace": {{"<match key>": "<shape_id>", ...}}, "add": ["<shape_id>", ...], "remove": ["<match key>", ...]}}`.
   `pov.key_names` maps every key used there to the country name; `pov_stats` has counts.
6. **Applying a view:** start from `countries.shp`; drop every feature whose match key is in
   `remove` or is a key of `replace`; append the shapes named by the values of `replace` and by
   `add`. The build verifies, for every variant, that this reproduces the original Natural
   Earth variant exactly (same feature count, same trimmed attributes, byte-identical WKB of
   every geometry after reading `shapes.geojson` back from disk).

Smallest variant entry, as an example:

```json
{example}
```

Note: attributes (population, GDP) come from the variant as published; Natural Earth does not
recompute them, e.g. in the chn view China's polygon includes Taiwan but its POP_EST does not.

| pov | replaced (geometry) | replaced (attributes only) | added | removed |
|---|---|---|---|---|
{prows}

Known quirks of the source data (not introduced by this build):
- In every variant the Southern Patagonian Ice Field (SPI) is removed and its area is not given to
  Argentina or Chile, so it is a hole in all POVs.
- In `twn` the Taiwan feature (key TWN) covers Taiwan plus all of mainland China (China removed).
- Some variants (pse, rus) contain unattributed slivers (disputed areas with no name or code);
  they are kept as "added" features with empty attributes.
- Many changes appear in nearly every variant because the default layer is the de facto view
  (e.g. Crimea in Russia, Hans Island split, Brazilian Island), so Russia, Greenland, China,
  Brazil and India are replaced in most variants (stored once in shapes.geojson).

## Size

`pov/shapes.geojson` is {shapes_mb} of the {mb(total)} total. Storing a separate GeoJSON per
variant would have cost ~150 MB, because the default layer is the de facto view and Russia,
Greenland, China, Brazil and India are replaced in almost every variant; sharing shapes removes
that duplication without touching any geometry. The largest other items are states_provinces
(~23 MB), countries (~9 MB), land and coastline (~7 MB each).

Suggested system-region mapping (ISO 3166-1 alpha-2 -> POV, everything else -> default) is in
`manifest.json` `pov.region_to_pov`. `iso` (the ISO 3166 view) has no region and is available for
explicit selection only.

## Rebuild

```
~/miniforge3/bin/mamba run -n layeredmap-ref python -I \\
    /Volumes/LaCie/repos/LayeredMap/tools/lm/reference/build_reference.py
```

Downloads go to `raw/` and are reused if present (`--offline` forbids network access). ETOPO (466 MB) is downloaded
once into `raw/etopo/`. The script rewrites all
outputs, `manifest.json` and this README, and deletes macOS `._*` files.
"""
    (out / "README.md").write_text(text)


FIELD_SOURCES = {
    "countries": {"name": "ADMIN", "name_zh": "NAME_ZH", "iso_a3": "ISO_A3 / ADM0_A3",
                  "continent": "CONTINENT", "region": "REGION_UN", "subregion": "SUBREGION",
                  "population": "POP_EST", "gdp_musd": "GDP_MD", "pop_year": "POP_YEAR",
                  "gdp_year": "GDP_YEAR", "type": "TYPE", "sovereign": "SOVEREIGNT"},
    "states_provinces": {"name": "name", "name_zh": "name_zh", "country": "admin",
                         "iso_a3": "adm0_a3", "type": "type_en"},
    "cities": {"name": "NAME", "name_zh": "NAME_ZH", "country": "ADM0NAME", "iso_a3": "ADM0_A3",
               "population": "POP_MAX", "capital": "ADM0CAP", "kind": "FEATURECLA"},
    "rivers": {"name": "name", "name_zh": "name_zh", "kind": "featurecla",
               "importance": "scalerank"},
    "lakes": {"name": "name", "name_zh": "name_zh", "importance": "scalerank"},
    "regions": {"name": "NAME", "name_zh": "NAME_ZH", "kind": "FEATURECLA", "region": "REGION"},
    "seas": {"name": "name", "name_zh": "name_zh", "kind": "featurecla"},
    "land": {"kind": "featurecla"}, "minor_islands": {"kind": "featurecla"},
    "coastline": {"kind": "featurecla"},
}


if __name__ == "__main__":
    main()
