"""The files and maps of harness v3, the app's layer names for them, and how objects are keyed.

The data lives outside the repo, under ``$LM_EVAL_DATA`` (default ``/Volumes/LaCie/lm-eval-data``;
its README lists sources, licences and sizes). Built-in layers are the app's own bundled copies
(``$LM_REFERENCE_DIR``, default ``/Volumes/LaCie/data/layeredmap-reference``).
"""

from __future__ import annotations

from gt import DATA, REF

FILES = {
    "storms": DATA / "launch/IBTrACS_storm_tracks_1980-2026.shp",
    "quakes": DATA / "launch/usgs_earthquakes_M6plus_1900-2026.gpkg",
    "volcanoes": DATA / "launch/NCEI_volcano_locations.csv",
    "etopo": DATA / "launch/ETOPO_2022_relief_30min.tif",
    "heat22": DATA / "launch/GDO_heatwave_intensity_JJA_2022.nc",
    "heat23": DATA / "launch/GDO_heatwave_intensity_JJA_2023.nc",
    "usgs": DATA / "datasets/usgs-quakes-month/usgs_all_month_2026-10-20.csv",
    "tornadoes": DATA / "datasets/tornadoes/1950-2025-torn-aspath/1950-2025-torn-aspath.shp",
    "airports": DATA / "datasets/airports/airports.csv",
    "nuclear": DATA / "datasets/nuclear/nuclear_power_plants.csv",
}

# The name each file gets on the map (the app's import names them; checked by the mock run).
LAYER = {
    "storms": "IBTrACS_storm_tracks_1980-2026",
    "quakes": "earthquakes",
    "volcanoes": "NCEI_volcano_locations",
    "etopo": "ETOPO_2022_relief_30min",
    "heat22": "GDO_heatwave_intensity_JJA_2022",
    "heat23": "GDO_heatwave_intensity_JJA_2023",
    "usgs": "usgs_all_month_2026-10-20",
    "tornadoes": "1950-2025-torn-aspath",
    "airports": "airports",
    "nuclear": "nuclear_power_plants",
}

# Fields the app's steps make, named from the layer names (lm-jobs actions: a layer name that
# reads as a noun of at most three words names the field, otherwise the kind's noun does). The
# tool descriptions promise "<points layer, lowercase>" ('Schools' -> 'schools'), which these file
# names don't follow: the model has to guess.
COUNT_FIELD = {
    "storms": "lines",
    "quakes": "earthquakes",
    "volcanoes": "ncei volcano locations",
    "usgs": "points",
    "tornadoes": "lines",
    "airports": "airports",
    "nuclear": "nuclear power plants",
}
NEAREST_FIELD = {
    "cities": "km to nearest world city",
    "volcanoes": "km to nearest ncei volcano location",
    "nuclear": "km to nearest nuclear power plant",
}

BUILTIN = {
    "cities": "World cities",
    "countries": "World countries",
    "states_provinces": "World states and provinces",
    "rivers": "World rivers",
    "lakes": "World lakes",
    "land": "World land",
    "regions": "World regions",
    "seas": "World seas",
    "coastline": "World coastline",
    "elevation": "World elevation",
}

MAPS = {
    "storms": ["storms"],
    "quakes": ["quakes"],
    "volcanoes": ["volcanoes"],
    "volc_etopo": ["volcanoes", "etopo"],
    "heat22": ["heat22"],
    "heat23": ["heat23"],
    "launch": ["storms", "quakes", "heat23", "etopo", "volcanoes"],
    "usgs": ["usgs"],
    "tornadoes": ["tornadoes"],
    "airports": ["airports"],
    "nuclear": ["nuclear"],
    "air_nuke": ["airports", "nuclear"],
}

# Field lists that name one object; a layer is keyed by the first list whose fields it has (the
# Rust side does the same), else by its row number. Order matters: most specific first.
KEYS = [
    ["storm_id"],
    ["ident"],
    ["id", "place"],
    ["Id", "Name"],
    ["om", "date"],
    ["place", "magnitude", "depth_km"],
    ["name", "country", "latitude"],
    ["name", "country"],
    ["name", "iso_a3"],
    ["name"],
]


def files(map_name: str) -> list[str]:
    return [str(FILES[k]) for k in MAPS[map_name]]


def fmt_value(v) -> str:
    """A value as the app's Value::to_string writes it (keys must match across both sides)."""
    import math

    if v is None or (isinstance(v, float) and math.isnan(v)):
        return "<Null>"
    if isinstance(v, bool):
        return "True" if v else "False"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        if v == int(v) and abs(v) < 1e15:
            return str(int(v))
        r = round(v * 1e6) / 1e6
        return repr(r)
    return str(v)


def key_of(gdf, row: int) -> str:
    cols = {str(c).lower(): c for c in gdf.columns if c != "geometry"}
    for spec in KEYS:
        if all(f.lower() in cols for f in spec):
            return "|".join(fmt_value(_py(gdf[cols[f.lower()]].iloc[row])) for f in spec)
    return f"row:{row}"


def keys_of(gdf, rows) -> list[str]:
    return [key_of(gdf, int(r)) for r in rows]


def _py(v):
    import numpy as np

    if isinstance(v, np.generic):
        return v.item()
    return v
