"""BFCL v4 irrelevance → sources/bfcl_irrelevance.jsonl.

Source: ShishirPatil/gorilla, ``berkeley-function-call-leaderboard/bfcl_eval/data``, commit
6ea57973c7a6097fd7c5915698c54c17c5b1b6c8 (2026-03-23). Licence: Apache-2.0 (repository LICENSE),
so the questions are used verbatim with attribution (LICENSES.md).

* ``BFCL_v4_irrelevance.json``: all 240 items. In BFCL each question comes with one unrelated
  function; here the question is asked of LayeredMap, whose tools are map actions. Almost all
  are refusals, but each item was audited against our contexts: a few become geographic
  questions LayeredMap could partly answer (distance between two cities, US airports), and are
  classified as such rather than as refusals.
* ``BFCL_v4_live_irrelevance.json`` (884): only the geographic items (weather, geocoding,
  directions, places, distances, travel bookings); hand-picked in LIVE below.

Defects noted in ``notes``: exact duplicates, typos, and statements with no request.

    python -I tools/lm/benchmarks/convert/bfcl.py [/path/to/gorilla]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, call, case, cond  # noqa: E402

RAW = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/Volumes/LaCie/data/layeredmap-benchmarks/bfcl/src")
DATA = RAW / "berkeley-function-call-leaderboard/bfcl_eval/data"
OUT = HERE.parent / "sources/bfcl_irrelevance.jsonl"
SOURCE = "BFCL v4 irrelevance"
LICENCE = "Apache-2.0"
CONTEXTS = ["world", "shanghai", "usa", "city"]

# Irrelevance items that aren't plain "irrelevant": index → (category, oos_reason | coverage spec, note)
# Coverage spec for non-refusals: ("upstream", [ops]) or ("now", plan builder).
GEO = {
    12: ("network", "live_routing", "usa"),
    20: ("network", "live_routing", "usa"),
    31: ("geocoding", "geocoding_service", "world"),
    37: ("geocoding", "geocoding_service", "usa"),
    40: ("data_acquisition", "internet_data", "usa"),
    48: ("knowledge", "knowledge", "usa"),
    49: ("knowledge", "knowledge", "world"),
    69: ("data_acquisition", "missing_data", "usa"),
    91: ("knowledge", "knowledge", "usa"),
    92: ("geocoding", "geocoding_service", "usa"),
    97: ("geocoding", "geocoding_service", "usa"),
    101: ("knowledge", "knowledge", "usa"),
    102: ("knowledge", "knowledge", "usa"),
    103: ("knowledge", "knowledge", "usa"),
    105: ("knowledge", "knowledge", "world"),
    107: ("data_acquisition", "missing_data", "usa"),
    110: ("data_acquisition", "internet_data", "usa"),
    112: ("data_acquisition", "internet_data", "usa"),
    113: ("geocoding", "geocoding_service", "world"),
    114: ("network", "live_routing", "usa"),
    115: ("network", "live_routing", "usa"),
    154: ("knowledge", "knowledge", "world"),
    156: ("knowledge", "knowledge", "world"),
    158: ("data_acquisition", "internet_data", "usa"),
    161: ("knowledge", "knowledge", "usa"),
    168: ("knowledge", "knowledge", "world"),
    201: ("geocoding", "geocoding_service", "city"),
    208: ("irrelevant", "irrelevant", "city"),
    210: ("knowledge", "knowledge", "world"),
    213: ("geocoding", "geocoding_service", "usa"),
    214: ("geocoding", "geocoding_service", "usa"),
    215: ("network", "live_routing", "usa"),
    216: ("geocoding", "geocoding_service", "city"),
    223: ("geocoding", "geocoding_service", "usa"),
    224: ("geocoding", "geocoding_service", "usa"),
    225: ("data_acquisition", "internet_data", "usa"),
    226: ("knowledge", "knowledge", "world"),
    227: ("knowledge", "knowledge", "world"),
    228: ("knowledge", "knowledge", "world"),
    229: ("geocoding", "geocoding_service", "world"),
    230: ("geocoding", "geocoding_service", "usa"),
    231: ("geocoding", "geocoding_service", "usa"),
    232: ("geocoding", "geocoding_service", "usa"),
    236: ("network", "live_routing", "world"),
    237: ("geocoding", "geocoding_service", "city"),
}
NOTES = {
    0: "Plane geometry, not a map question.",
    5: "Plane geometry, not a map question.",
    6: "Plane geometry, not a map question.",
    24: "Exact duplicate of irrelevance_23.",
    81: "Exact duplicate of irrelevance_77.",
    98: "Duplicate of irrelevance_96 (case differs).",
    103: "Near-duplicate of irrelevance_102.",
    129: "Typo in the source ('MTBI' for MBTI).",
    138: "Duplicate of irrelevance_135 (case differs).",
    160: "Garbled question in the source.",
    219: "Near-duplicate of irrelevance_47.",
    225: "'Right now' asks for a live figure; the Cities layer's population field is a census value, so refusing (or saying so) is right.",
    208: "Shortest path in a video game, not on a map.",
}
# Items that aren't refusals in LayeredMap's world.
SPECIAL = {
    117: {
        "coverage": "now",
        "category": "attribute_query",
        "confidence": "medium",
        "summary": "List the major airports in the United States.",
        "notes": "Audit: irrelevant to BFCL's function, but world_plus has an Airports layer with country and type, so a filter answers it.",
        "plan": [call("filter", layer="Airports", conditions=[cond("country", "is", "United States"), cond("type", "is", "major")])],
        "alts": [
            [call("filter", layer="Airports", conditions=[cond("country", "is", "United States of America"), cond("type", "is", "major")])],
        ],
        "context": "world_plus",
        "zh": "列出美国的主要机场。",
    },
    239: {
        "coverage": "upstream",
        "category": "measurement",
        "confidence": "high",
        "missing": ["distance_between"],
        "summary": "Straight-line distance in km between San Francisco and Los Angeles.",
        "notes": "Audit: a map question. With both cities in the Cities layer, the geodesic distance is measure_from_here (rules.toml), not connected yet.",
    },
}

LIVE = {
    # id suffix → (category, oos_reason | spec, context, note)
    "0-0-0": ("geocoding", "geocoding_service", "usa"),
    "5-0-5": ("geocoding", "geocoding_service", "usa"),
    "9-0-9": ("data_acquisition", "internet_data", "world"),
    "35-2-23": ("data_acquisition", "internet_data", "usa"),
    "87-2-75": ("irrelevant", "irrelevant", "usa"),
    "114-7-4": ("data_acquisition", "internet_data", "usa"),
    "120-9-0": ("data_acquisition", "internet_data", "usa"),
    "121-9-1": ("data_acquisition", "internet_data", "usa"),
    "127-11-2": ("data_acquisition", "internet_data", "shanghai"),
    "138-13-4": ("data_acquisition", "internet_data", "usa"),
    "160-19-2": ("geocoding", "geocoding_service", "usa"),
    "169-23-1": ("data_acquisition", "internet_data", "usa"),
    "267-57-2": ("data_acquisition", "internet_data", "usa"),
    "304-76-2": ("geocoding", "geocoding_service", "usa"),
    "310-76-8": ("data_acquisition", "internet_data", "usa"),
    "311-76-9": ("data_acquisition", "internet_data", "usa"),
    "317-76-15": ("data_acquisition", "internet_data", "world"),
    "318-76-16": ("data_acquisition", "internet_data", "world"),
    "326-76-24": ("data_acquisition", "internet_data", "usa"),
    "329-76-27": ("data_acquisition", "internet_data", "usa"),
    "331-76-29": ("geocoding", "geocoding_service", "usa"),
    "339-81-0": ("data_acquisition", "internet_data", "usa"),
    "343-81-4": ("data_acquisition", "internet_data", "usa"),
    "345-81-6": ("data_acquisition", "internet_data", "usa"),
    "346-81-7": ("data_acquisition", "internet_data", "usa"),
    "352-81-13": ("data_acquisition", "internet_data", "usa"),
    "353-81-14": ("data_acquisition", "internet_data", "usa"),
    "354-81-15": ("knowledge", "knowledge", "world"),
    "359-81-20": ("data_acquisition", "internet_data", "world"),
    "373-81-34": ("data_acquisition", "internet_data", "world"),
    "374-81-35": ("geocoding", "geocoding_service", "usa"),
    "383-81-44": ("data_acquisition", "internet_data", "usa"),
    "384-81-45": ("data_acquisition", "internet_data", "usa"),
    "385-81-46": ("data_acquisition", "internet_data", "usa"),
    "387-81-48": ("data_acquisition", "internet_data", "world"),
    "390-81-51": ("data_acquisition", "internet_data", "world"),
    "392-81-53": ("data_acquisition", "internet_data", "world"),
    "403-85-0": ("data_acquisition", "internet_data", "usa"),
    "411-92-0": ("data_acquisition", "internet_data", "world"),
    "413-93-0": ("geocoding", "geocoding_service", "usa"),
    "442-113-0": ("data_acquisition", "internet_data", "world"),
    "454-121-1": ("network", "live_routing", "usa"),
    "457-124-0": ("data_acquisition", "internet_data", "usa"),
    "458-124-1": ("data_acquisition", "internet_data", "usa"),
    "459-125-0": ("network", "live_routing", "usa"),
    "462-127-0": ("data_acquisition", "internet_data", "world"),
    "478-135-0": ("data_acquisition", "internet_data", "world"),
    "482-137-0": ("data_acquisition", "internet_data", "usa"),
    "534-166-0": ("geocoding", "geocoding_service", "city"),
    "549-169-2": ("data_acquisition", "internet_data", "world"),
    "587-183-0": ("data_acquisition", "internet_data", "world"),
    "617-195-5": ("network", "live_routing", "usa"),
    "618-195-6": ("network", "live_routing", "usa"),
    "666-212-0": ("data_acquisition", "internet_data", "city"),
    "688-223-3": ("network", "live_routing", "city"),
    "706-231-1": ("data_acquisition", "internet_data", "world"),
    "732-247-0": ("network", "live_routing", "usa"),
    "748-259-0": ("network", "live_routing", "usa"),
    "763-273-0": ("data_acquisition", "internet_data", "usa"),
    "775-284-0": ("data_acquisition", "internet_data", "usa"),
    "784-293-0": ("geocoding", "geocoding_service", "world"),
    "825-318-0": ("data_acquisition", "internet_data", "world"),
    "830-323-0": ("data_acquisition", "internet_data", "shanghai"),
    "866-352-0": ("geocoding", "geocoding_service", "usa"),
    "873-358-0": ("irrelevant", "irrelevant", "world"),
    "874-359-0": ("data_acquisition", "internet_data", "shanghai"),
    "876-361-0": ("network", "live_routing", "world"),
    "877-362-0": ("network", "live_routing", "world"),
    "882-882-0": ("network", "live_routing", "usa"),
}
LIVE_SPECIAL = {
    "313-76-11": {
        "coverage": "now",
        "category": "attribute_query",
        "confidence": "low",
        "summary": "Which city the airport with code BLR belongs to.",
        "notes": "Audit: world_plus has Airports with an iata field; zooming to BLR shows where it is. The layer has no city field, so the answer is read off the map (low confidence).",
        "plan": [call("zoom_to", target={"layer": "Airports", "any": [["iata", "BLR"]], "lenient": []})],
        "alts": [[call("filter", layer="Airports", conditions=[cond("iata", "is", "BLR")])]],
        "context": "world_plus",
    },
    "328-76-26": {
        "coverage": "upstream",
        "category": "field_calc",
        "confidence": "medium",
        "missing": ["calculate_geometry"],
        "summary": "Latitude of the city of Rome.",
        "notes": "Audit: Rome is in Populated places; no action reports coordinates. calculateGeometry adds centroid coordinates as fields.",
    },
    "360-81-21": {
        "coverage": "upstream",
        "category": "measurement",
        "confidence": "high",
        "missing": ["distance_between"],
        "summary": "Great-circle (air) distance from Oslo to Tokyo.",
        "notes": "Audit: both cities are in Populated places; a geodesic distance action (measure_from_here) is not connected.",
    },
    "448-117-0": None,  # encoded map tiles for construction planning: not a GIS request we can map; skipped
}
LIVE_NOTES = {
    "87-2-75": "A false statement (54 °C in Boston), no request: defective.",
    "121-9-1": "Prompt text leaks the function-calling instruction into the user turn.",
    "331-76-29": "No address or place given: underspecified in the source.",
    "534-166-0": "No place given: underspecified in the source.",
    "160-19-2": "Asks for coordinates while giving coordinates: defective wording.",
    "873-358-0": "A game map, not geographic data.",
}


def user_text(x: dict) -> str:
    turns = x["question"]
    return " ".join(m["content"].strip() for t in turns for m in t if m["role"] == "user")


SUMMARY_GLOSS = {"获取武汉的天气": "Weather in Wuhan (获取武汉的天气).", "天气如何？": "How is the weather? (天气如何？), no place given."}


def summary_of(q: str) -> str:
    q = " ".join(q.split())
    if q in SUMMARY_GLOSS:
        return SUMMARY_GLOSS[q]
    return q if len(q) <= 160 else q[:157] + "…"


def record(sid, q, cid, cat, reason, ctx, note, policy="verbatim") -> dict:
    return {
        "source": SOURCE,
        "source_id": sid,
        "licence": LICENCE,
        "text_policy": policy,
        "summary": summary_of(q),
        "category": cat,
        "coverage": "out_of_scope",
        "oos_reason": reason,
        "confidence": "high",
        "notes": note,
        "cases": [case(cid, q, CANNOT, context=ctx, lang="zh" if any("一" <= ch <= "鿿" for ch in q) else "en")],
    }


def special(sid, q, cid, s) -> dict:
    r = {
        "source": SOURCE,
        "source_id": sid,
        "licence": LICENCE,
        "text_policy": "verbatim",
        "summary": s["summary"],
        "category": s["category"],
        "coverage": s["coverage"],
        "confidence": s["confidence"],
        "notes": s["notes"],
        "cases": [],
    }
    if s["coverage"] in ("upstream", "new"):
        r["missing"] = s["missing"]
    else:
        r["cases"].append(case(cid, q, s["plan"], context=s["context"], alts=s.get("alts", ())))
        if s.get("zh"):
            r["cases"].append(case(cid + "-zh", s["zh"], s["plan"], context=s["context"], lang="zh", alts=s.get("alts", ())))
    return r


def main() -> None:
    out = []
    items = [json.loads(line) for line in (DATA / "BFCL_v4_irrelevance.json").read_text(encoding="utf-8").splitlines() if line.strip()]
    for x in items:
        n = int(x["id"].split("_")[1])
        q = user_text(x)
        cid = f"bfcl-irr-{n:03d}"
        if n in SPECIAL:
            out.append(special(x["id"], q, cid, SPECIAL[n]))
            continue
        cat, reason, ctx = GEO.get(n, ("irrelevant", "irrelevant", CONTEXTS[n % len(CONTEXTS)]))
        out.append(record(x["id"], q, cid, cat, reason, ctx, NOTES.get(n, "")))
    live = {}
    for line in (DATA / "BFCL_v4_live_irrelevance.json").read_text(encoding="utf-8").splitlines():
        if line.strip():
            x = json.loads(line)
            live[x["id"].removeprefix("live_irrelevance_")] = x
    for suffix in list(LIVE) + [k for k in LIVE_SPECIAL if k not in LIVE]:
        x = live[suffix]
        q = user_text(x)
        n = int(suffix.split("-")[0])
        cid = f"bfcl-live-{n:03d}"
        if suffix in LIVE_SPECIAL:
            s = LIVE_SPECIAL[suffix]
            if s is not None:
                out.append(special(x["id"], q, cid, s))
            continue
        cat, reason, ctx = LIVE[suffix]
        out.append(record(x["id"], q, cid, cat, reason, ctx, LIVE_NOTES.get(suffix, "")))
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in out), encoding="utf-8")
    print(f"{OUT.name}: {len(out)} records")


if __name__ == "__main__":
    main()
