"""T-Eval (map items) → sources/teval.jsonl.

Source: open-compass/T-Eval, data from the Hugging Face dataset ``lovesnowbest/T-Eval`` at
revision af355ab2b62cdbae4262ac41c7529ffeae395012 (2024-02-18); code repository commit
58f22406404d7e2a4f36856a19c7f4dc28a0a5f0. Licence: Apache-2.0 (repository LICENSE and dataset
card), so the questions are used verbatim with attribution (LICENSES.md).

Only the planning items (``plan_json_v2.json`` and ``plan_json_v2_zh.json``) whose reference
tool chain uses BINGMap or GaodeMap, alone or with Weather and TimeTool, are taken: 29 English
and 29 Chinese, the same questions in both languages. Each chain calls live web services
(coordinates of addresses, nearby search, routes, weather), so LayeredMap should refuse; the one
exception is a question answerable from map layers (two cities' positions and distance), which
needs a distance action that wraps existing code.

    python -I tools/lm/benchmarks/convert/teval.py [/path/to/T-Eval/hf/data]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, case  # noqa: E402

RAW = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/Volumes/LaCie/data/layeredmap-benchmarks/t-eval/hf/data")
OUT = HERE.parent / "sources/teval.jsonl"
SOURCE = "T-Eval"
LICENCE = "Apache-2.0"
MAPS = {"BINGMap", "GaodeMap"}
ALLOWED = MAPS | {"Weather", "TimeTool"}

# Query key → context (where the places are).
CONTEXT = {
    "query_237_1": "shanghai",
    "query_464_1": "world",
    "query_281_2": "world",
    "query_35_2": "world",
}
UPSTREAM = {
    "query_312_2": {
        "missing": ["distance_between", "calculate_geometry"],
        "category": "measurement",
        "notes": "Both cities are in a Cities/Populated places layer: their positions (calculate_geometry) and the straight-line distance (measure_from_here, not connected) wrap existing code. 'How much ground' may mean road distance, and the weekday part is not a map question, so medium confidence.",
    },
}


def classify(names: list[str]) -> tuple[str, str]:
    tools = {n.split(".")[1] for n in names}
    if tools & {"get_route"}:
        return "network", "live_routing"
    if tools & {"search_nearby", "get_coordinates", "get_coordinate"}:
        return "geocoding", "geocoding_service"
    if any(n.startswith("Weather") for n in names):
        return "data_acquisition", "internet_data"
    if tools & {"get_distance"}:
        return "network", "live_routing"
    return "data_acquisition", "internet_data"


def describe(names: list[str]) -> str:
    words = {
        "get_route": "a route",
        "get_distance": "a travel distance",
        "search_nearby": "places nearby",
        "get_coordinates": "coordinates of named places",
        "get_coordinate": "coordinates of named places",
        "get_weather_today": "today's weather",
        "forecast_weather": "a weather forecast",
        "get_curr_time": "the current time",
        "get_weekday": "the weekday",
    }
    seen = []
    for n in names:
        w = words.get(n.split(".")[1], n.split(".")[1])
        if w not in seen:
            seen.append(w)
    return ", ".join(seen)


def main() -> None:
    out = []
    for fname, lang in (("plan_json_v2.json", "en"), ("plan_json_v2_zh.json", "zh")):
        d = json.loads((RAW / fname).read_text(encoding="utf-8"))
        k = 0
        for key in sorted(d, key=lambda s: [int(p) if p.isdigit() else p for p in s.split("_")]):
            v = d[key]
            names = [g["name"] for g in v["ground_truth"] if g["name"] != "FinishAction"]
            fams = {n.split(".")[0] for n in names}
            if not names or not fams <= ALLOWED or not fams & MAPS:
                continue
            k += 1
            q = next(m["content"] for m in v["origin_prompt"] if m["role"] == "user").strip()
            cid = f"tev-{lang}-{k:03d}"
            what = describe(names)
            summary = f"Map-service question ({lang}): asks for {what}."
            r = {
                "source": SOURCE,
                "source_id": f"{fname}:{key}",
                "licence": LICENCE,
                "text_policy": "verbatim",
                "summary": summary,
                "confidence": "high",
                "notes": "",
                "cases": [],
            }
            if key in UPSTREAM:
                u = UPSTREAM[key]
                r.update(category=u["category"], coverage="upstream", missing=u["missing"], confidence="medium", notes=u["notes"])
            else:
                cat, reason = classify(names)
                r.update(category=cat, coverage="out_of_scope", oos_reason=reason)
                r["notes"] = "The reference chain calls live map services (" + ", ".join(sorted({n.split('.')[1] for n in names})) + ")."
                r["cases"] = [case(cid, q, CANNOT, context=CONTEXT.get(key, "usa"), lang=lang)]
            out.append({k2: r[k2] for k2 in ("source", "source_id", "licence", "text_policy", "summary", "category", "coverage", "missing", "oos_reason", "confidence", "notes", "cases") if k2 in r})
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in out), encoding="utf-8")
    print(f"{OUT.name}: {len(out)} records")


if __name__ == "__main__":
    main()
