"""LayeredMap Pro end to end on the eight geographic puzzles (docs/lm/puzzles/).

    python -I tools/lm/puzzles/pro_e2e.py --gold                      # run the gold plans (no model)
    python -I tools/lm/puzzles/pro_e2e.py --show-context P6           # print what the model sees
    python -I tools/lm/puzzles/pro_e2e.py --dry-run                   # mocked model answers with the gold plans
    python -I tools/lm/puzzles/pro_e2e.py --models anthropic/claude-haiku-5.5,z-ai/glm-5.3-flash --runs 2
    python -I tools/lm/puzzles/pro_e2e.py --report                    # results/summary.md from the runs

For each puzzle and each of its requests (English, and Chinese where the puzzle has one), the
assistant gets:

* the v2 harness pieces in the ``submit_plan`` design (``tools/lm/llm_eval/v2``): the tool with
  layer enums, the v2 rules and worked examples, the plan checker with two repair rounds, and
  ``resolve_feature``, here answering from the puzzle layers' own attribute tables;
* a map context built from the puzzle's datasets: layer names, kinds, object counts, field names
  with types, units and meanings (from Natural Earth's, WorldClim's and GEM's documentation), and
  the rasters' units. No values, except what ``resolve_feature`` returns;
* a few extra rules and worked examples about the puzzle-era actions (find_where, keep_inside,
  nearest_distance, join_by_location, add_measurements, calculate_field, split_parts). They use
  the v2 harness's invented example map, not the puzzles, and ``--check-leaks`` checks that no
  example request is close to a puzzle request.

The returned plan runs with ``run_plan`` (``cargo build -p lm-jobs --release --example
run_plan``) on the files in the data folder, and the answer is checked against ``puzzles.json``:
a city by name or within 25 km of the answer, an island within 30 km of the answer point, the P4
area by a label point within 250 km of the answer's centroid and the town (Simao) listed in a
card. Results: ``results/<run>/<model>.jsonl`` (plan, checker log, tokens, cost, latency, run
output, outcome) and ``results/spend.json`` (all model spending of this harness, capped by
``--budget``, US$1.50 by default).

The OpenRouter key is read at run time from the macOS keychain (service ``openrouter``, account
``layeredmap``), as in the v1 and v2 harnesses, and never printed, logged or stored.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import struct
import subprocess
import sys
import threading
import time
import unicodedata
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
V2 = ROOT / "tools/lm/llm_eval/v2"
sys.path.insert(0, str(V2))
sys.path.insert(0, str(V2.parent))

import checker as chk  # noqa: E402
import context as ctxmod  # noqa: E402
import data as v2data  # noqa: E402
import prompt_v2  # noqa: E402
import resolve  # noqa: E402
import run_eval  # noqa: E402
import run_v2  # noqa: E402
import tools_v2  # noqa: E402
from tokens import count_tokens  # noqa: E402

DATA = Path(os.environ.get("LM_PUZZLE_DATA", "/Volumes/LaCie/data/layeredmap-puzzles"))
PUZZLES_JSON = ROOT / "docs/lm/puzzles/puzzles.json"
PLANS = HERE / "plans"
RESULTS = HERE / "results"
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", "/Volumes/LaCie/cargo-target/layeredmap"))
RUN_PLAN = TARGET / "release/examples/run_plan"
MODELS = ["anthropic/claude-haiku-5.5", "z-ai/glm-5.3-flash"]
DESIGN = "submit_plan"
MAX_TURNS = 8
MAX_REPAIRS = 2
# Each layer's filter when the request is made ({context: {layer: [fields]}}); stepwise.py sets it.
CURRENT_FILTERS: dict[str, dict[str, list[str]]] = {}
SINGLE_SHOWN: dict[str, set[str]] = {}

# ---------------------------------------------------------------------------------------------
# The puzzle layers

NE = DATA / "naturalearth"
WC = DATA / "worldclim"

# Natural Earth field meanings, from naturalearthdata.com (the layers' README and the attribute
# notes of the 1:10m cultural and physical themes). Fields not listed here are named in the
# context without a meaning ("other fields").
LAYERS: dict[str, dict] = {
    "Populated places": {
        "kind": "points", "path": NE / "ne_10m_populated_places/ne_10m_populated_places.shp",
        "about": "Natural Earth 1:10m populated places: cities and towns worldwide",
        "fields": {
            "NAME": "the place's name in English or local Latin script (e.g. 'São Paulo')",
            "NAMEASCII": "the name in plain ASCII letters",
            "NAME_EN": "the English name", "NAME_ZH": "the Chinese name (Simplified)",
            "ADM0NAME": "the country the place is in, short English name ('China', 'United States of America', 'South Korea')",
            "SOV0NAME": "the sovereign state", "ADM1NAME": "the first-level division (state, province)",
            "ADM0_A3": "the country's 3-letter code", "ISO_A2": "the country's 2-letter ISO code",
            "FEATURECLA": "category: kind of place ('Admin-0 capital', 'Admin-1 capital', 'Populated place', ...)",
            "ADM0CAP": "1 if the national capital, else 0",
            "POP_MAX": ("people", "population of the place: the larger of its city-proper and metropolitan figures"),
            "POP_MIN": ("people", "population of the city proper"),
            "LATITUDE": ("degrees", "latitude"), "LONGITUDE": ("degrees", "longitude"),
            "TIMEZONE": "time zone name",
        },
        "categories": {"FEATURECLA": None},
    },
    "Countries": {
        "kind": "areas", "path": NE / "ne_10m_admin_0_countries/ne_10m_admin_0_countries.shp",
        "about": "Natural Earth 1:10m admin-0 countries (one area per country or dependency)",
        "fields": {
            "ADMIN": "the country's name ('China', 'South Korea', 'United States of America')",
            "NAME": "short display name", "NAME_LONG": "long name", "NAME_ZH": "the Chinese name",
            "SOVEREIGNT": "the sovereign state", "ISO_A3": "3-letter ISO code", "ADM0_A3": "3-letter code",
            "CONTINENT": "category: the continent ('Africa', 'Asia', 'Europe', 'North America', 'South America', 'Oceania', 'Antarctica', 'Seven seas (open ocean)')",
            "REGION_UN": "category: UN region", "SUBREGION": "category: UN subregion",
            "POP_EST": ("people", "estimated population of the country"), "POP_YEAR": ("year", "year of POP_EST"),
            "GDP_MD": ("million US$", "gross domestic product"), "GDP_YEAR": ("year", "year of GDP_MD"),
            "ECONOMY": "category: economy group", "INCOME_GRP": "category: income group",
            "TYPE": "category: kind of unit ('Sovereign country', 'Country', 'Dependency', ...)",
        },
        "categories": {"CONTINENT": None, "REGION_UN": None, "SUBREGION": None, "ECONOMY": None, "INCOME_GRP": None, "TYPE": None},
    },
    "Boundary lines": {
        "kind": "lines", "path": NE / "ne_10m_admin_0_boundary_lines_land/ne_10m_admin_0_boundary_lines_land.shp",
        "about": "Natural Earth 1:10m land boundaries between countries",
        "fields": {
            "ADM0_LEFT": "the country on one side", "ADM0_RIGHT": "the country on the other side",
            "FEATURECLA": "category: kind of boundary", "TYPE": "category: boundary type",
        },
        "categories": {"FEATURECLA": None, "TYPE": None},
    },
    "Land": {
        "kind": "areas", "path": NE / "ne_10m_land/ne_10m_land.shp",
        "about": "Natural Earth 1:10m land: 11 multi-part areas holding the continents, every island larger than about 70 km² and about 6,000 smaller ones (6,837 parts in all)",
        "fields": {"featurecla": "category: 'Land'", "scalerank": ("rank", "importance rank (lower = more important)")},
        "categories": {"featurecla": None},
    },
    "Minor islands": {
        "kind": "areas", "path": NE / "ne_10m_minor_islands/ne_10m_minor_islands.shp",
        "about": "Natural Earth 1:10m minor islands: 2,795 further small islands (none larger than about 70 km²) that are not in Land",
        "fields": {"featurecla": "category: 'Minor island'", "scalerank": ("rank", "importance rank")},
        "categories": {"featurecla": None},
    },
    "Coastline": {
        "kind": "lines", "path": NE / "ne_10m_coastline/ne_10m_coastline.shp",
        "about": "Natural Earth 1:10m coastline (the sea's edge)",
        "fields": {"featurecla": "category: 'Coastline'", "scalerank": ("rank", "importance rank")},
        "categories": {"featurecla": None},
    },
    "Rivers": {
        "kind": "lines", "path": NE / "ne_10m_rivers_lake_centerlines/ne_10m_rivers_lake_centerlines.shp",
        "about": "Natural Earth 1:10m rivers and lake centerlines; 'major rivers' are those of scalerank 4 or less",
        "fields": {
            "name": "the river's name", "name_en": "English name", "name_zh": "Chinese name",
            "scalerank": ("rank", "Natural Earth importance rank, 0-12: lower = larger, more important river"),
            "featurecla": "category: 'River', 'Lake Centerline', ...",
        },
        "categories": {"featurecla": None},
    },
    "Active faults": {
        "kind": "lines", "path": DATA / "gem/gem_active_faults_harmonized.geojson",
        "about": "GEM Global Active Faults (harmonized): mapped active fault traces",
        "fields": {"name": "fault name", "slip_type": "category: kind of slip", "catalog_name": "source catalogue"},
        "categories": {"slip_type": None},
    },
    "Elevation": {"kind": "raster", "path": WC / "wc2.1_10m_elev.tif",
                  "desc": "1 band, 10 arc-minute cells (about 18 km), WorldClim 2.1 elevation, metres above sea level"},
    "Annual mean temperature": {"kind": "raster", "path": WC / "wc2.1_10m_bio_1.tif",
                                "desc": "1 band, 10 arc-minute cells, WorldClim 2.1 BIO1 annual mean air temperature, °C (1970-2000 average)"},
    "Annual precipitation": {"kind": "raster", "path": WC / "wc2.1_10m_bio_12.tif",
                             "desc": "1 band, 10 arc-minute cells, WorldClim 2.1 BIO12 annual precipitation (rain and snow), mm per year"},
    "January mean temperature": {"kind": "raster", "path": WC / "wc2.1_10m_tavg_01.tif",
                                 "desc": "1 band, 10 arc-minute cells, WorldClim 2.1 average air temperature of January, °C"},
    "July mean temperature": {"kind": "raster", "path": WC / "wc2.1_10m_tavg_07.tif",
                              "desc": "1 band, 10 arc-minute cells, WorldClim 2.1 average air temperature of July, °C"},
}

DATASET_LAYERS = {
    "ne_places": ["Populated places"], "ne_countries": ["Countries"], "ne_boundaries": ["Boundary lines"],
    "ne_land": ["Land", "Minor islands"], "ne_coastline": ["Coastline"], "ne_rivers": ["Rivers"],
    "wc_elev": ["Elevation"], "wc_bio1": ["Annual mean temperature"], "wc_bio12": ["Annual precipitation"],
    "wc_tavg01": ["January mean temperature"], "wc_tavg07": ["July mean temperature"], "gem_faults": ["Active faults"],
}

# Chinese names of the continents for resolve_feature on Countries.CONTINENT (the v2 table has
# them for the benchmark's lowercase `continent`).
CONTINENT_ZH = {k: v for k, v in resolve.ALIASES["Countries"]["continent"].items()}


def _strip_num(t: str, dec: int) -> str:
    try:
        f = float(t)
    except ValueError:
        return t
    if dec == 0 and f.is_integer():
        return str(int(f))
    return repr(f) if not f.is_integer() else str(int(f))


def read_dbf(path: Path) -> tuple[list[tuple[str, str, int, int]], list[dict]]:
    """A dBase III table (the .dbf of a shapefile): fields (name, type, length, decimals) and rows
    (values as text, None when blank). Encoding from the .cpg (UTF-8 by default)."""
    cpg = path.with_suffix(".cpg")
    enc = cpg.read_text().strip() if cpg.exists() else "utf-8"
    enc = {"UTF-8": "utf-8", "UTF8": "utf-8"}.get(enc.upper(), enc)
    with path.open("rb") as f:
        head = f.read(32)
        n = struct.unpack("<I", head[4:8])[0]
        hlen, rlen = struct.unpack("<HH", head[8:12])
        fields = []
        while True:
            d = f.read(32)
            if not d or d[0] == 0x0D:
                break
            fields.append((d[:11].split(b"\0")[0].decode("ascii", "replace"), chr(d[11]), d[16], d[17]))
        f.seek(hlen)
        rows = []
        for _ in range(n):
            rec = f.read(rlen)
            if len(rec) < rlen:
                break
            if rec[:1] == b"*":
                continue
            pos, row = 1, {}
            for name, t, ln, dec in fields:
                raw = rec[pos : pos + ln].decode(enc, "replace").strip().strip("\0")
                pos += ln
                if not raw:
                    row[name] = None
                elif t in "NF":
                    row[name] = _strip_num(raw, dec)
                else:
                    row[name] = raw
            rows.append(row)
    return fields, rows


def read_geojson_table(path: Path) -> tuple[list[tuple[str, str, int, int]], list[dict]]:
    d = json.loads(path.read_text(encoding="utf-8"))
    rows, types = [], {}
    for feat in d.get("features", []):
        props = feat.get("properties") or {}
        row = {}
        for k, v in props.items():
            if isinstance(v, bool):
                types.setdefault(k, "L")
            elif isinstance(v, int):
                types[k] = types.get(k) if types.get(k) in ("F", "C") else "N"
            elif isinstance(v, float):
                types[k] = "F" if types.get(k) != "C" else "C"
            elif v is not None:
                types[k] = "C"
            row[k] = None if v is None else (str(v).lower() if isinstance(v, bool) else str(v))
        rows.append(row)
    fields = [(k, t, 0, 0 if t == "N" else 6) for k, t in types.items()]
    return fields, rows


_TABLES: dict[str, tuple] = {}
_TLOCK = threading.Lock()


def table(layer: str) -> tuple[list, list]:
    with _TLOCK:
        if layer not in _TABLES:
            p = LAYERS[layer]["path"]
            _TABLES[layer] = read_geojson_table(p) if p.suffix == ".geojson" else read_dbf(p.with_suffix(".dbf"))
        return _TABLES[layer]


def field_type(t: str, dec: int) -> str:
    return {"C": "text", "D": "date", "L": "yes/no"}.get(t, "integer" if t == "N" and dec == 0 else "number")


def register() -> None:
    """Puts the puzzle layers into the v2 harness's catalogue (context, checker, resolve_feature)."""
    for name, L in LAYERS.items():
        if L["kind"] == "raster":
            ctxmod.LAYERS[name] = {"kind": "raster", "desc": L["desc"]}
            continue
        fields, rows = table(name)
        out = []
        for fname, t, _ln, dec in fields:
            ftype = field_type(t, dec)
            meaning = L["fields"].get(fname)
            unit = meaning[0] if isinstance(meaning, tuple) else None
            cats = None
            if fname in L.get("categories", {}):
                # The checker and resolve_feature know a category's values; the context doesn't list them.
                ftype = "category"
                cats = sorted({r[fname] for r in rows if r.get(fname)})
            elif ftype == "text" and meaning is None:
                # Not searched by resolve_feature (dozens of translations and class codes), but still a
                # text field for the checker.
                ftype = "other text"
            out.append((fname, ftype, unit, cats))
        ctxmod.LAYERS[name] = {"kind": L["kind"], "count": len(rows), "fields": out}
        v2data.REGISTERED[name] = (lambda n=name: table(n)[1])
    resolve.ALIASES["Countries"] = {"continent": CONTINENT_ZH, "CONTINENT": CONTINENT_ZH}


def puzzle_layers(p: dict) -> list[str]:
    return [n for d in p["datasets"] for n in DATASET_LAYERS[d]]


def context_name(p: dict) -> str:
    name = f"puzzle_{p['id']}"
    ctxmod.CONTEXTS[name] = puzzle_layers(p)
    return name


def layer_text(i: int, name: str) -> str:
    """One line of the map context: kind, count, the documented fields with meaning and unit,
    then the other fields by name only."""
    L, C = LAYERS[name], ctxmod.LAYERS[name]
    if L["kind"] == "raster":
        return f"{i}. {name}: raster, {L['desc']}"
    key, other = [], []
    for fname, ftype, unit, _ in C["fields"]:
        meaning = L["fields"].get(fname)
        if meaning is None:
            other.append(fname)
            continue
        m = meaning[1] if isinstance(meaning, tuple) else meaning
        m = m.removeprefix("category: ")
        t = "text" if ftype == "other text" else ftype
        key.append(f"{fname} ({t}{', ' + unit if unit else ''}: {m})")
    s = f"{i}. {name}: {L['kind']}, {C['count']:,} ({L['about']}). Fields: " + "; ".join(key)
    if other:
        s += ". Other fields (names only): " + ", ".join(other)
    return s


def map_context(ctx: str) -> str:
    lines = ["The user's map. Open layers:"]
    for i, name in enumerate(ctxmod.CONTEXTS[ctx], 1):
        lines.append(layer_text(i, name))
    lines.append("Selected: nothing")
    return "\n".join(lines)


# ---------------------------------------------------------------------------------------------
# Extra rules and worked examples for the puzzle-era actions (general; not the puzzles)

EXTRA_RULES = """11. Numbers in the request: "at least", "or more", "or warmer", "不低于", "至少" -> at_least; "at most", "or less", "or colder", "不超过", "不高于" -> at_most; "more than", "over", "超过" -> above; "less than", "below" -> below; "between a and b", "a to b" -> between. Write numbers in the field's or raster's unit, digits only ("two million" = 2000000, "一亿" = 100000000, "a million" = 1000000).
12. Which action gives which value: a raster's value at each point -> add_raster_values; details of the area each object lies in (its country, continent, country population) -> join_by_location; distance to the nearest object of another layer -> nearest_distance; distance to the nearest other object of the same layer -> nearest_other; a value computed from two values (a difference, a ratio) -> calculate_field; one object per island or part of a multi-part area -> split_parts; area, length and coordinates -> add_measurements; places where raster conditions hold -> find_where; the part of a layer inside some areas -> keep_inside; the objects of a layer inside one area -> whats_inside.
13. Steps that add values fill them only for the objects shown at that moment; hidden objects stay blank. To answer "which one", end the plan with the filter that leaves only the objects meeting every condition.
14. Well-known geographic terms have standard definitions: use them without asking. Tropical / in the tropics: latitude between -23.44 and 23.44; northern / southern hemisphere: latitude above / below 0; Arctic: latitude at least 66.56; Antarctic: latitude at most -66.56; a latitude needs the layer's latitude field (add_measurements adds one).
15. "Which <point> has the highest / lowest <raster value>" (the coldest city, the wettest hospital) asks about the points themselves: add_raster_values to the points, then rank_by_value on that field (order "lowest" for the lowest, coldest, driest, least). summarize_each is for areas (each country's average), never for "which city"."""

EXTRA_MAP = """More example layers on the same invented map: Temperature: raster, °C. Lakes: areas, 1 object with many parts; fields name (text)."""

EXTRA_EXAMPLES = [
    ("Which hospitals with at least 100 beds get more than 800 mm of rain and are no more than 2 km from a river?", [],
     ['filter(layer="Hospitals", conditions=[{field: "beds", op: "at_least", value: "100"}])',
      'add_raster_values(raster="Rainfall", points="Hospitals")',
      'nearest_distance(layer="Hospitals", other="Rivers")',
      'filter(layer="Hospitals", conditions=[{field: "beds", op: "at_least", value: "100"}, {field: "Rainfall", op: "above", value: "800"}, '
      '{field: "km to nearest river", op: "at_most", value: "2"}])']),
    ("给每家医院加上所在区的人口，再算每千人的床位数", [],
     ['join_by_location(layer="Hospitals", areas="Wards", fields="population")',
      'calculate_field(layer="Hospitals", first="beds", op="divide", second="population", name="beds per person")',
      'calculate_field(layer="Hospitals", first="beds per person", op="multiply", second="1000", name="beds per 1000 people")']),
    ("Show where it is at least 25 °C and gets 600 to 900 mm of rain, but only the part inside the Riverside ward.", [],
     ['find_where(raster="Temperature", conditions=[{field: "Temperature", op: "at_least", value: "25"}, '
      '{field: "Rainfall", op: "between", value: "600", value2: "900"}])',
      'filter(layer="Wards", conditions=[{field: "name", op: "is", value: "Riverside"}])',
      'keep_inside(layer="#1", areas="Wards")']),
    ("Which hospital gets the least rain?", [],
     ['add_raster_values(raster="Rainfall", points="Hospitals")',
      'rank_by_value(layer="Hospitals", field="Rainfall", order="lowest")']),
    ("Which hospitals are more than 3 km from any other hospital?", [],
     ['nearest_other(layer="Hospitals")',
      'filter(layer="Hospitals", conditions=[{field: "km to nearest other hospital", op: "above", value: "3"}])']),
    ("把湖泊拆成一个个单独的湖，并算出每个湖的面积", [],
     ['split_parts(layer="Lakes")', 'add_measurements(layer="#1")']),
]


def extra_examples_text() -> str:
    lines = [EXTRA_MAP, ""]
    for req, look, plan in EXTRA_EXAMPLES:
        lines.append(f"User: {req}")
        for x in look:
            lines.append(f"  Lookup: {x}")
        lines.append("  submit_plan(steps=[" + ", ".join(f"#{i} {p}" for i, p in enumerate(plan, 1)) + "])")
    return "\n".join(lines)


def static_prefix() -> str:
    return prompt_v2.static_prefix(DESIGN, True) + "\n\n" + EXTRA_RULES + "\n\n" + extra_examples_text()


def system_message(ctx: str, cache: bool) -> dict:
    static, dynamic = static_prefix(), map_context(ctx)
    if cache:
        return {"role": "system", "content": [
            {"type": "text", "text": static, "cache_control": {"type": "ephemeral"}},
            {"type": "text", "text": dynamic},
        ]}
    return {"role": "system", "content": static + "\n\n" + dynamic}


def _grams(s: str) -> set:
    s = re.sub(r"\s+", " ", s.casefold())
    return {s[i : i + 3] for i in range(len(s) - 2)}


def leak_report(puzzles: list[dict]) -> list[tuple[float, str, str]]:
    """Character-trigram Jaccard of every example request (v2 and extra) against every puzzle request."""
    reqs = [r for p in puzzles for r in p["request"].values()]
    ex = [e[0] for e in prompt_v2.EXAMPLES] + [e[0] for e in EXTRA_EXAMPLES]
    out = []
    for e in ex:
        for r in reqs:
            a, b = _grams(e), _grams(r)
            out.append((len(a & b) / max(1, len(a | b)), e, r))
    return sorted(out, reverse=True)


# ---------------------------------------------------------------------------------------------
# Running plans and checking answers


def run_plan(layers: list[str], steps: list[dict], limit: int = 50, timeout: int = 600) -> dict:
    spec = {"layers": [{"name": n, "path": str(LAYERS[n]["path"])} for n in layers], "steps": steps, "limit": limit}
    tmp = RESULTS / "tmp"
    tmp.mkdir(parents=True, exist_ok=True)
    path = tmp / f"plan-{os.getpid()}-{threading.get_ident()}.json"
    path.write_text(json.dumps(spec, ensure_ascii=False), encoding="utf-8")
    t0 = time.monotonic()
    try:
        r = subprocess.run([str(RUN_PLAN), str(path)], capture_output=True, text=True, timeout=timeout)
        out = json.loads(r.stdout) if r.stdout.strip() else {"ok": False, "error": {"step": 0, "message": r.stderr[-300:]}}
    except subprocess.TimeoutExpired:
        out = {"ok": False, "error": {"step": 0, "message": f"timed out after {timeout} s"}}
    except ValueError:
        out = {"ok": False, "error": {"step": 0, "message": "unreadable run_plan output"}}
    finally:
        path.unlink(missing_ok=True)
    out["wall_s"] = round(time.monotonic() - t0, 3)
    return out


def km(lon1, lat1, lon2, lat2) -> float:
    p1, p2 = math.radians(lat1), math.radians(lat2)
    a = math.sin((p2 - p1) / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(math.radians(lon2 - lon1) / 2) ** 2
    return 2 * 6371.0088 * math.asin(min(1.0, math.sqrt(a)))


def fold(s) -> str:
    s = unicodedata.normalize("NFKD", str(s or ""))
    return "".join(c for c in s if not unicodedata.combining(c)).casefold().strip()


TOL_KM = {"city": 25.0, "island": 30.0, "area": 250.0}


def candidates_per_step(out: dict, layers: list[str]) -> list:
    """Objects left in the layer each step works on: a filter's shown count, a new layer's object
    count, the layer's current count for steps that add fields, 'card' for card steps."""
    shown: dict[str, int] = {}
    res = []
    for s in out.get("steps", []):
        if "filtered" in s:
            shown[s["filtered"]] = s["shown"]
            res.append(s["shown"])
        elif "new_layer" in s:
            shown[s["new_layer"]] = s.get("rows")
            res.append(s.get("rows"))
        elif "new_fields" in s:
            res.append(shown.get(s["layer"], "all"))
        elif "card" in s:
            res.append("card")
        elif "asked" in s or "refused" in s:
            res.append("ask" if "asked" in s else "refuse")
        else:
            res.append("-")
    return res


def answer_layer(out: dict, helpers: frozenset = frozenset()) -> dict | None:
    """The layer the plan's answer is in: the last filtered layer, else the last new layer. When
    that layer ends up empty but another layer the plan filtered still shows objects (a plan that
    runs the same tests on two layers, e.g. Land and Minor islands), that one. Layers the plan
    filters only to use them as a mask or a target (``helpers``: Countries filtered to China, then
    used as keep_inside's areas) are not answers."""
    ls = out.get("layers", [])

    def state(target):
        if target[0] == "alias":
            hits = [l for l in ls if l.get("alias") == target[1]]
        else:
            hits = [l for l in ls if (l.get("name") or "").casefold() == target[1].casefold()]
        return hits[-1] if hits else None

    targets = []
    for s in out.get("steps", []):
        if "filtered" in s:
            targets.append(("name", s["filtered"]))
        elif "new_layer" in s:
            targets.append(("alias", s.get("alias")))
    filtered = [t for t in targets if t[0] == "name" and t[1] not in helpers] or [t for t in targets if t[0] == "name"]
    order = filtered or targets
    if not order:
        return None
    last = state(order[-1])
    if last is not None and last.get("shown", 0) == 0:
        for t in reversed(order[:-1]):
            other = state(t)
            if other is not None and other.get("shown", 0) > 0 and other is not last:
                return other
    return last


def helper_layers(plan: list[dict]) -> frozenset:
    """Layers a plan uses as areas (a mask, a join) or as the other layer of a distance."""
    return frozenset(s["args"][k] for s in plan or [] for k in ("areas", "other")
                     if isinstance(s.get("args"), dict) and isinstance(s["args"].get(k), str) and s["name"] not in ("count_in", "summarize_each"))


def check_answer(p: dict, out: dict, plan: list[dict] | None = None) -> dict:
    """{outcome, shown, names, detail}: solved (the answer alone), solved_with_cards (the
    answer alone once the cards are read), ambiguous (the answer among several), wrong, empty,
    error, no_layer."""
    if not out.get("ok"):
        return {"outcome": "exec_error", "detail": out.get("error")}
    ans, kind = p["answer"], p["answer_kind"]
    lyr = answer_layer(out, helper_layers(plan))
    if lyr is None:
        return {"outcome": "no_layer", "detail": "the plan filters nothing and makes no layer"}
    objs = lyr.get("objects", [])
    cards = [s for s in out.get("steps", []) if "card" in s]

    def is_answer(o) -> bool:
        if o.get("lon") is None:
            return False
        d = km(o["lon"], o["lat"], ans["lon"], ans["lat"])
        if kind == "city":
            names = {fold(o.get("title"))} | {fold(v) for k, v in (o.get("attrs") or {}).items() if k.upper() in ("NAME", "NAMEASCII", "NAME_EN")}
            return fold(ans["name"]) in names or d <= TOL_KM["city"]
        return d <= TOL_KM[kind]

    hit = [o for o in objs if is_answer(o)]
    names = [o.get("title") for o in objs[:10]]
    res = {"shown": lyr.get("shown"), "names": names, "layer": lyr.get("name")}
    if lyr.get("shown", 0) == 0:
        return {**res, "outcome": "empty"}
    if not hit:
        return {**res, "outcome": "wrong"}
    if kind == "area":
        town = fold(ans.get("town", "").split(" (")[0].split(",")[0]) if ans.get("town") else ""
        listed = any(town and town in fold(c.get("answer", "")) for c in cards)
        if lyr["shown"] == 1:
            return {**res, "outcome": "solved" if listed else "area_only", "detail": "town listed" if listed else "area right; town not listed"}
        return {**res, "outcome": "ambiguous"}
    if lyr["shown"] == 1:
        return {**res, "outcome": "solved"}
    # Several left: do the cards rule the others out? ("N countries inside <name>")
    counts = {}
    for c in cards:
        m = re.match(r"(\d+) countr(?:y|ies) inside (.+)$", c.get("summary", ""))
        if m:
            counts[fold(m.group(2))] = int(m.group(1))
    if counts and lyr["shown"] <= len(objs):
        need = 3
        left = [o for o in objs if counts.get(fold(o.get("title")), need) >= need]
        if len(left) == 1 and is_answer(left[0]):
            return {**res, "outcome": "solved_with_cards", "detail": counts}
    return {**res, "outcome": "ambiguous"}


# ---------------------------------------------------------------------------------------------
# Gold plans


def gold(puzzles: list[dict], only: set[str]) -> list[dict]:
    rows = []
    for p in puzzles:
        if only and p["id"] not in only:
            continue
        spec = json.loads((PLANS / f"{p['id']}.json").read_text(encoding="utf-8"))
        layers = [l["name"] for l in spec["layers"]]
        out = run_plan(layers, spec["steps"], spec.get("limit", 50))
        verdict = check_answer(p, out, spec["steps"])
        counts = None
        cpath = PLANS / f"{p['id']}_counts.json"
        if cpath.exists():
            cs = json.loads(cpath.read_text(encoding="utf-8"))
            cout = run_plan([l["name"] for l in cs["layers"]], cs["steps"], cs.get("limit", 50))
            counts = candidates_per_step(cout, [])
        row = {"puzzle": p["id"], "steps": len(spec["steps"]), "ok": out.get("ok"), "seconds": out.get("seconds"),
               "candidates": candidates_per_step(out, layers), "counts_plan": counts,
               "solver": p["verification"]["candidates_after_each_clue"], **verdict}
        rows.append(row)
        print(f"{p['id']}: {verdict['outcome']} ({out.get('seconds') or 0:.1f} s); per step {row['candidates']}; solver {row['solver']}"
              + (f"; counts plan {counts}" if counts else ""), flush=True)
    return rows


# ---------------------------------------------------------------------------------------------
# The assistant


def plan_request(client, model: dict, ctx: str, request: str, budget, cache: bool) -> dict:
    """One request in the submit_plan design (as ``run_v2.run_case``, without the benchmark
    scoring): lookups, the plan, up to two repair rounds."""
    tools = tools_v2.submit_plan_tools(ctx, with_resolve=True)
    known = {t["function"]["name"] for t in tools}
    system = system_message(ctx, cache)
    messages: list = [system, {"role": "user", "content": request}]
    est_prompt = count_tokens(json.dumps(system, ensure_ascii=False)) + count_tokens(json.dumps(tools, ensure_ascii=False)) + 50
    checker = chk.Checker(ctx, None, request, enabled=True, current_filters=CURRENT_FILTERS.get(ctx),
                          single_shown=SINGLE_SHOWN.get(ctx, frozenset()))
    rec: dict = {}
    tot = {"prompt_tokens": 0, "completion_tokens": 0, "reasoning_tokens": 0, "cached_tokens": 0, "cache_write_tokens": 0,
           "cost_usd": 0.0, "latency_s": 0.0, "tries": 0, "model_calls": 0}
    plan: list[dict] = []
    raw_calls, log = [], []
    repairs = resolves = 0
    text, error, done = "", None, False
    for turn in range(MAX_TURNS):
        data, err = run_v2.call_model(client, model, messages, tools, "required", budget, est_prompt, turn, rec, tot)
        if err:
            error = err
            break
        choice0 = data["choices"][0]
        rec["finish_reason"] = choice0.get("finish_reason")
        msg = choice0.get("message") or {}
        calls, t, from_text = run_v2.parse_message(msg, known)
        text = t or text
        for c in calls:
            raw_calls.append({"turn": turn + 1, "name": c["name"], "args": c["args"], "from_text": from_text})
        if not calls:
            break
        results, turn_errors, stop = [], False, False
        for c in calls:
            name, args = c["name"], c["args"]
            if name == "resolve_feature":
                resolves += 1
                res = resolve.resolve_feature(str(args.get("layer", "")), args.get("text", ""))
                results.append(resolve.tool_result_text(res))
                log.append({"turn": turn + 1, "resolve": args, "candidates": res.get("candidates"), "error": res.get("error")})
                continue
            if name == "submit_plan":
                steps = tools_v2.steps_to_calls(args.get("steps"))
                fixed, errs, fixes = checker.check_plan(steps) if steps else ([], ["submit_plan came with no steps (the call was cut off or unreadable)."], [])
                log.append({"turn": turn + 1, "submit": len(steps), "errors": errs, "fixes": fixes})
                if errs and repairs < MAX_REPAIRS:
                    turn_errors = True
                    results.append("Plan not accepted. Fix these and call submit_plan again with the whole plan: " + " | ".join(errs))
                else:
                    plan = fixed
                    if errs:
                        rec["unrepaired"] = errs
                    done = stop = True
                    results.append("Plan sent to the user for approval." + (" Corrected: " + "; ".join(fixes) if fixes else ""))
                continue
            if name in ("ask_user", "cannot_do"):
                plan = [{"name": name, "args": args}]
                results.append("Sent to the user.")
                stop = done = True
                continue
            plan.append({"name": name, "args": args})
            results.append("Use submit_plan for plans.")
        if turn_errors:
            repairs += 1
        if stop and not turn_errors:
            break
        if from_text:
            messages.append({"role": "assistant", "content": text})
            if turn_errors:
                messages.append({"role": "user", "content": "Your plan was read from your text. " + " ".join(results) + " Make the corrected calls with the tools."})
                continue
            break
        messages.append(run_v2.assistant_echo(msg, calls, turn))
        for c, r in zip(calls, results):
            messages.append({"role": "tool", "tool_call_id": c["id"], "content": r})
        if done:
            break
    if not plan and not error and (text or "").strip():
        tt = text.strip()
        plan = [{"name": "ask_user", "args": {"question": tt[:300]}}] if ("?" in tt or "？" in tt) else [{"name": "cannot_do", "args": {"reason": tt[:300]}}]
        rec["text_as_meta"] = plan[0]["name"]
    final, dropped = checker.finalize(plan)
    rec.update({**tot, "repair_rounds": repairs, "resolve_calls": resolves, "raw_calls": raw_calls, "checker_log": log,
                "plan": final, "dropped": dropped, "text": (text or "")[:500]})
    rec["cost_usd"] = round(rec["cost_usd"], 8)
    rec["latency_s"] = round(rec["latency_s"], 3)
    if error:
        rec["error"] = error
    return rec


def run_one(client, model: dict, p: dict, lang: str, run: int, budget, cache: bool) -> dict:
    ctx = context_name(p)
    request = p["request"][lang]
    t0 = time.monotonic()
    rec = plan_request(client, model, ctx, request, budget, cache)
    rec.update({"puzzle": p["id"], "lang": lang, "run": run, "model": model["id"], "request": request})
    plan = rec.get("plan") or []
    if rec.get("error"):
        rec["outcome"] = "budget" if rec["error"] == "budget" else "model_error"
    elif not plan:
        rec["outcome"] = "no_plan"
    elif plan[0]["name"] in ("ask_user", "cannot_do"):
        rec["outcome"] = "asked" if plan[0]["name"] == "ask_user" else "refused"
    else:
        out = run_plan(puzzle_layers(p), plan)
        verdict = check_answer(p, out, plan)
        rec.update({"exec": {k: out.get(k) for k in ("ok", "error", "steps", "seconds", "wall_s")},
                    "candidates": candidates_per_step(out, []), **verdict})
    rec["seconds_total"] = round(time.monotonic() - t0, 3)
    rec["steps"] = len(plan)
    return rec


class MockClient:
    """Answers each request with the puzzle's gold plan (dry runs and tests)."""

    def __init__(self, puzzles: list[dict]):
        self.by_request = {r: p for p in puzzles for r in p["request"].values()}

    def complete(self, body: dict):
        usage = {"prompt_tokens": 100, "completion_tokens": 10, "cost": 0.0}
        if body["messages"][-1]["role"] == "tool":
            return 200, {"choices": [{"message": {"role": "assistant", "content": "Done."}, "finish_reason": "stop"}], "usage": usage}
        p = self.by_request[body["messages"][1]["content"]]
        spec = json.loads((PLANS / f"{p['id']}.json").read_text(encoding="utf-8"))
        steps = [{"action": s["name"], **s["args"]} for s in spec["steps"]]
        call = {"id": "c0", "type": "function", "function": {"name": "submit_plan", "arguments": json.dumps({"steps": steps}, ensure_ascii=False)}}
        return 200, {"choices": [{"message": {"role": "assistant", "content": None, "tool_calls": [call]}, "finish_reason": "tool_calls"}], "usage": usage}


# ---------------------------------------------------------------------------------------------
# Report


SOLVED = ("solved", "solved_with_cards")


def summarize(run_dir: Path) -> str:
    rows = []
    for f in sorted(x for x in run_dir.glob("*.jsonl") if not x.name.startswith("._")):  # exFAT AppleDouble files
        rows += [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x.strip()]
    groups: dict = {}
    for r in rows:
        groups.setdefault((r["puzzle"], r["model"].split("/")[-1], r["lang"]), []).append(r)
    agg = ["| Puzzle | Model | Lang | Solved | Outcomes | Steps | Cost US$ (all runs) | Seconds (mean) |", "|---|---|---|---|---|---|---|---|"]
    for (pz, m, lang), rs in sorted(groups.items()):
        ok = sum(r.get("outcome") in SOLVED for r in rs)
        outs = ", ".join(str(r.get("outcome")) for r in sorted(rs, key=lambda r: r["run"]))
        steps = "/".join(str(r.get("steps", 0)) for r in sorted(rs, key=lambda r: r["run"]))
        agg.append(f"| {pz} | {m} | {lang} | {ok}/{len(rs)} | {outs} | {steps} | {sum(r.get('cost_usd', 0) for r in rs):.4f} | "
                   f"{sum(r.get('seconds_total', 0) for r in rs) / len(rs):.0f} |")
    tot = sum(r.get("cost_usd", 0) for r in rows)
    solved = sum(r.get("outcome") in SOLVED for r in rows)
    head = f"{len(rows)} requests, {solved} solved, US${tot:.4f} in all.\n\n"
    lines = ["| Puzzle | Model | Lang | Run | Outcome | Steps | Shown | Cost US$ | Model s | Total s |", "|---|---|---|---|---|---|---|---|---|---|"]
    for r in sorted(rows, key=lambda r: (r["puzzle"], r["model"], r["lang"], r["run"])):
        lines.append(f"| {r['puzzle']} | {r['model'].split('/')[-1]} | {r['lang']} | {r['run']} | {r.get('outcome')} | {r.get('steps')} | "
                     f"{r.get('shown', '')} | {r.get('cost_usd', 0):.4f} | {r.get('latency_s', 0):.1f} | {r.get('seconds_total', 0):.1f} |")
    return head + "\n".join(agg) + "\n\nEvery request:\n\n" + "\n".join(lines) + "\n"


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gold", action="store_true")
    ap.add_argument("--show-context", default="")
    ap.add_argument("--check-leaks", action="store_true")
    ap.add_argument("--models", default=",".join(MODELS))
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--only", default="", help="puzzle ids, comma-separated")
    ap.add_argument("--langs", default="en,zh")
    ap.add_argument("--budget", type=float, default=1.50)
    ap.add_argument("--tag", default="")
    ap.add_argument("--concurrency", type=int, default=4)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--no-cache", action="store_true")
    ap.add_argument("--report", default="", help="a run folder under results/ to summarize")
    a = ap.parse_args(argv)

    puzzles = json.loads(PUZZLES_JSON.read_text(encoding="utf-8"))["puzzles"]
    only = {x.strip() for x in a.only.split(",") if x.strip()}
    if a.report:
        print(summarize(RESULTS / a.report))
        return 0
    if a.check_leaks:
        for score, e, r in leak_report(puzzles)[:5]:
            print(f"{score:.3f}  {e[:60]!r}  ~  {r[:60]!r}")
        return 0
    if not RUN_PLAN.exists():
        print(f"run_plan not built: {RUN_PLAN} (cargo build -p lm-jobs --release --example run_plan)", file=sys.stderr)
        return 2
    RESULTS.mkdir(parents=True, exist_ok=True)
    if a.gold:
        rows = gold(puzzles, only)
        (RESULTS / "gold.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        return 0
    register()
    if a.show_context:
        p = next(p for p in puzzles if p["id"] == a.show_context)
        ctx = context_name(p)
        sm = system_message(ctx, False)["content"]
        tools = tools_v2.submit_plan_tools(ctx)
        print(sm)
        print(f"\n[{count_tokens(sm)} tokens of system message; {count_tokens(json.dumps(tools, ensure_ascii=False))} tokens of tools]")
        return 0

    catalog = run_eval.load_catalog()
    models = [m.strip() for m in a.models.split(",") if m.strip()]
    run_name = ("dryrun-" if a.dry_run else "") + "pro-e2e" + (f"-{a.tag}" if a.tag else "")
    out_dir = RESULTS / run_name
    out_dir.mkdir(parents=True, exist_ok=True)
    budget = run_eval.Budget(RESULTS / ("spend-dryrun.json" if a.dry_run else "spend.json"), a.budget)
    if a.dry_run:
        client = MockClient(puzzles)
    else:
        key = run_eval.read_key()
        if not key:
            print("No OpenRouter key in the keychain (service openrouter, account layeredmap).", file=sys.stderr)
            return 3
        client = run_eval.OpenRouterClient(key)
        del key
    langs = [x.strip() for x in a.langs.split(",") if x.strip()]
    tasks = []
    for m in models:
        path = out_dir / f"{run_eval.slug(m)}.jsonl"
        done = set()
        if path.exists():
            for x in path.read_text(encoding="utf-8").splitlines():
                if x.strip():
                    r = json.loads(x)
                    if r.get("outcome") not in ("budget", "model_error"):
                        done.add((r["puzzle"], r["lang"], r["run"]))
        for run in range(1, a.runs + 1):
            for p in puzzles:
                if only and p["id"] not in only:
                    continue
                for lang in langs:
                    if lang in p["request"] and (p["id"], lang, run) not in done:
                        tasks.append((m, p, lang, run, path))
    print(f"{run_name}: {len(tasks)} requests; budget ${a.budget:.2f}, spent so far ${budget.spent:.4f}", flush=True)
    locks = {m: threading.Lock() for m in models}

    def work(m, p, lang, run, path):
        model = catalog[m]
        cache = not a.no_cache and m.startswith(run_v2.CACHE_VENDORS)
        try:
            rec = run_one(client, model, p, lang, run, budget, cache)
        except Exception as e:  # noqa: BLE001  (a harness failure is recorded, not fatal)
            rec = {"puzzle": p["id"], "lang": lang, "run": run, "model": m, "outcome": "harness_error", "error": f"{type(e).__name__}: {e}"}
        with locks[m]:
            with path.open("a", encoding="utf-8") as fh:
                fh.write(json.dumps(rec, ensure_ascii=False) + "\n")
        return rec

    with ThreadPoolExecutor(max_workers=a.concurrency) as ex:
        futs = [ex.submit(work, *t) for t in tasks]
        for f in as_completed(futs):
            r = f.result()
            print(f"  {r['puzzle']} {r['lang']} run {r['run']} {r['model'].split('/')[-1]}: {r.get('outcome')} "
                  f"({r.get('steps', 0)} steps, ${r.get('cost_usd', 0):.4f}, {r.get('seconds_total', 0):.0f} s)", flush=True)
    print(f"Spent so far ${budget.spent:.4f} of ${a.budget:.2f}")
    (out_dir / "summary.md").write_text(summarize(out_dir), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
