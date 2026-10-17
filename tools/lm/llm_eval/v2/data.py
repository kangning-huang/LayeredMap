"""The benchmark layers' attribute tables, read from the real fixture files.

Used only locally: by ``resolve.py`` (the simulated ``resolve_feature`` lookup) and by the plan
checker (does an object or a category value exist?). Nothing here is sent to a model except what
``resolve_feature`` returns (up to 5 matching stored names or category labels, as the owner
allowed on 2026-10-17).
"""

from __future__ import annotations

import csv
import functools
import json
import sqlite3
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
EVAL = HERE.parent
ROOT = EVAL.parents[2]
sys.path.insert(0, str(EVAL))

import context as ctxmod  # noqa: E402

SOURCES = {
    "Countries": ("geojson", ROOT / "assets/examples/countries.geojson"),
    "Populated places": ("geojson", ROOT / "assets/examples/populated_places.geojson"),
    "Districts": ("geojson", ROOT / "fixtures/V1/districts.geojson"),
    "Schools": ("csv", ROOT / "fixtures/T1/schools.csv"),
    "Roads": ("gpkg", ROOT / "fixtures/V9/city_layers.gpkg", "roads"),
    "Parks": ("gpkg", ROOT / "fixtures/V9/city_layers.gpkg", "parks"),
}


# Layers registered at run time by another harness (the puzzles' ``pro_e2e.py``): name -> a
# function returning the attribute rows (values as text). Consulted before ``SOURCES``.
REGISTERED: dict = {}


def _text(v) -> str | None:
    if v is None:
        return None
    if isinstance(v, bool):
        return "true" if v else "false"
    return str(v)


@functools.cache
def rows(layer: str) -> tuple[dict, ...]:
    """The layer's attribute rows (values as text; ``()`` for rasters and drawn areas)."""
    if layer in REGISTERED:
        return tuple(REGISTERED[layer]())
    src = SOURCES.get(layer)
    if src is None:
        return ()
    kind, path = src[0], src[1]
    out: list[dict] = []
    if kind == "geojson":
        for f in json.loads(path.read_text(encoding="utf-8"))["features"]:
            out.append({k: _text(v) for k, v in (f.get("properties") or {}).items()})
    elif kind == "csv":
        with path.open(encoding="utf-8", newline="") as fh:
            for r in csv.DictReader(fh):
                if not (r.get("lon") or "").strip() or not (r.get("lat") or "").strip():
                    continue  # the app imports 242 located schools; 6 rows have no location
                out.append({k: v for k, v in r.items() if k not in ("lon", "lat")})
    elif kind == "gpkg":
        con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        try:
            cur = con.execute(f'select * from "{src[2]}"')
            names = [d[0] for d in cur.description]
            for r in cur.fetchall():
                out.append({n: _text(v) for n, v in zip(names, r) if n not in ("fid", "geom")})
        finally:
            con.close()
    return tuple(out)


def fields(layer: str) -> dict[str, str]:
    """{field: type} as the context describes it ("text", "category", "integer", ...)."""
    L = ctxmod.LAYERS.get(layer) or {}
    return {f[0]: f[1] for f in L.get("fields", [])}


def kind(layer: str) -> str | None:
    L = ctxmod.LAYERS.get(layer)
    return L["kind"] if L else None


def categories(layer: str, field: str) -> list[str] | None:
    for f in (ctxmod.LAYERS.get(layer) or {}).get("fields", []):
        if f[0] == field:
            return list(f[3]) if f[3] else None
    return None


NUMERIC = {"integer", "number"}


def is_numeric(ftype: str | None) -> bool:
    return bool(ftype) and ftype.split(",")[0].strip() in NUMERIC


def is_text_like(ftype: str | None) -> bool:
    return bool(ftype) and not is_numeric(ftype)
