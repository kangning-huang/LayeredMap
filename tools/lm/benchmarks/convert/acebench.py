"""ACEBench (geo and refusal-pattern items) → sources/acebench.jsonl.

Source: ACEBench/ACEBench, ``data_all/data_{zh,en}``, commit
56dd66cf6439b0d9655ee1b353e4cd745c6f664e (2025-10-29). Licence: MIT (LICENSE.md, which reads
"MIT License — Copyright (c) Microsoft Corporation"), so questions may be used verbatim or adapted
with attribution (LICENSES.md).

Taken:

* every ``special_irrelevant`` item (50 zh, 50 en): the tools offered can't do the request. Asked
  of LayeredMap they are not map tasks, so the answer is cannot_do (verbatim question);
* every ``special_incomplete`` item (50 zh, 50 en): a required parameter is missing. For
  LayeredMap almost none are map tasks, so they are refusals too; the few with a geographic
  intent are adapted to our map contexts where a clarifying question is the right answer
  (ask_user), as ADAPTED below;
* the GIS/geography items from the normal and error-parameter categories (hand-picked in GEO;
  a keyword search over-matches, e.g. "地点" in event planning). Each is classified against
  LayeredMap: most need live place search, routing or weather (out_of_scope), some wrap existing
  RustGIS functions (upstream), and a few are adapted to a map context where today's actions
  answer them (now). The request of an out_of_scope case is the item's user text (verbatim).

    python -I tools/lm/benchmarks/convert/acebench.py [/path/to/ACEBench]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import ASK, CANNOT, call, case, cond, dist, sel  # noqa: E402

RAW = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/Volumes/LaCie/data/layeredmap-benchmarks/acebench/src")
OUT = HERE.parent / "sources/acebench.jsonl"
SOURCE = "ACEBench"
LICENCE = "MIT (LICENSE.md: Copyright (c) Microsoft Corporation)"

TOKYO = {"layer": "Populated places", "any": [["name", "Tokyo"], ["name_zh", "东京"]], "lenient": ["東京"]}
QUAKE_PLANS = [
    call("filter", layer="Earthquakes", conditions=[cond("magnitude", "between", 5.0, 7.0)]),
    call("buffer", layer="Populated places", object=TOKYO, distance=dist(300000)),
    call("whats_inside", area=sel("#2"), layer="Earthquakes"),
]
QUAKE_ALT = [
    call("buffer", layer="Populated places", object=TOKYO, distance=dist(300000)),
    call("filter", layer="Earthquakes", conditions=[cond("magnitude", "between", 5.0, 7.0)]),
    call("whats_inside", area=sel("#1"), layer="Earthquakes"),
]

# (lang, item id) → spec. Out-of-scope specs: (category, oos_reason, context, note).
GEO_OOS = {
    ("zh", "normal_atom_bool_44"): ("network", "live_routing", "shanghai", "A bus route with live traffic."),
    ("zh", "normal_atom_number_5"): ("geocoding", "geocoding_service", "usa", "Restaurant search around coordinates."),
    ("zh", "normal_atom_number_9"): ("geocoding", "geocoding_service", "usa", "Place search around the user."),
    ("zh", "normal_atom_number_23"): ("geocoding", "geocoding_service", "shanghai", "Shopping centres within 5 km: no such layer, needs place search."),
    ("zh", "normal_atom_number_35"): ("geocoding", "geocoding_service", "world", "Place search around coordinates."),
    ("zh", "normal_atom_number_41"): ("geocoding", "geocoding_service", "usa", "Restaurant search within 3 km of coordinates."),
    ("zh", "normal_atom_number_42"): ("geocoding", "geocoding_service", "shanghai", "Restaurant search with amenities."),
    ("zh", "normal_atom_object_deep_40"): ("knowledge", "missing_data", "world", "Asks for an analysis of a mountain area from a coordinate and two figures; no layer holds the place."),
    ("zh", "normal_atom_object_short_31"): ("knowledge", "knowledge", "world", "Time zone of a coordinate."),
    ("zh", "normal_multi_turn_user_adjust_7_0"): ("data_acquisition", "internet_data", "usa", "Live incident details at a coordinate."),
    ("zh", "normal_multi_turn_user_adjust_32_0"): ("geocoding", "geocoding_service", "world", "Postal code of a coordinate (reverse geocoding)."),
    ("zh", "normal_multi_turn_user_adjust_44_0"): ("data_acquisition", "internet_data", "world", "Live parking availability."),
    ("zh", "normal_multi_turn_user_switch_8_1"): ("knowledge", "knowledge", "world", "A descriptive analysis of Guangzhou's landscape."),
    ("zh", "normal_preference_42"): ("geocoding", "geocoding_service", "city", "Restaurants near the user, open at lunch."),
    ("zh", "normal_single_turn_parallel_function_11"): ("network", "live_routing", "usa", "Walking, driving and cycling times."),
    ("zh", "normal_single_turn_parallel_function_68"): ("irrelevant", "irrelevant", "world", "A seismic signal-blockage tool, not map analysis."),
    ("zh", "normal_single_turn_single_function_12"): ("data_acquisition", "missing_data", "usa", "Mineral resources at a coordinate: no such data."),
    ("zh", "normal_single_turn_single_function_50"): ("data_acquisition", "internet_data", "usa", "Live vehicle tracking."),
    ("zh", "normal_atom_object_deep_17"): ("modelling", "prediction", "world", "Logistics cost prediction from given distances."),
    ("zh", "special_error_param_0"): ("data_acquisition", "internet_data", "world", "Weather forecast at a coordinate."),
    ("zh", "special_error_param_27"): ("data_acquisition", "internet_data", "usa", "Weekend weather forecast."),
    ("en", "normal_atom_bool_21"): ("geocoding", "geocoding_service", "city", "Restaurant search nearby."),
    ("en", "normal_atom_enum_21"): ("imagery", "imagery", "world", "A high-resolution vegetation map of the Congo forest."),
    ("en", "normal_atom_enum_39"): ("network", "live_routing", "usa", "Scenic driving routes."),
    ("en", "normal_atom_list_24"): ("data_acquisition", "missing_data", "world", "Species diversity map: no species data."),
    ("en", "normal_atom_list_26"): ("data_acquisition", "internet_data", "usa", "Current air quality of several cities."),
    ("en", "normal_atom_number_0"): ("geocoding", "geocoding_service", "world", "Historical sites near the user in Beijing."),
    ("en", "normal_atom_number_2"): ("network", "live_routing", "world", "Quickest bus route in heavy traffic."),
    ("en", "normal_atom_number_16"): ("geocoding", "geocoding_service", "city", "Nearest medication disposal point."),
    ("en", "normal_atom_object_deep_45"): ("data_acquisition", "internet_data", "usa", "Real-time traffic updates."),
    ("en", "normal_atom_object_short_10"): ("data_acquisition", "internet_data", "world", "Historical satellite imagery for an area."),
    ("en", "normal_atom_object_short_20"): ("imagery", "imagery", "world", "Land-use change and deforestation from imagery."),
    ("en", "normal_atom_object_short_38"): ("geocoding", "geocoding_service", "usa", "Address of a coordinate in 1950 and today."),
    ("en", "normal_multi_turn_user_adjust_8_0"): ("data_acquisition", "internet_data", "usa", "Hiking routes in the Smoky Mountains."),
    ("en", "normal_multi_turn_user_switch_20_1"): ("geocoding", "geocoding_service", "world", "Historical sites to visit in Rome."),
    ("en", "normal_multi_turn_user_switch_49_0"): ("data_acquisition", "internet_data", "world", "Hiking routes in the Lake District."),
    ("en", "normal_single_turn_single_function_20"): ("data_acquisition", "missing_data", "usa", "Deer migration tracking: no tracking data."),
    ("en", "normal_single_turn_single_function_39"): ("modelling", "prediction", "usa", "Predict tomorrow's traffic with a neural network."),
    ("en", "normal_single_turn_single_function_51"): ("modelling", "prediction", "usa", "Hurricane landfall probability next week."),
    ("en", "normal_single_turn_single_function_76"): ("geocoding", "geocoding_service", "usa", "Reverse geocoding a coordinate."),
    ("en", "normal_single_turn_parallel_function_79"): ("data_acquisition", "missing_data", "usa", "Environmental impact on heritage sites, 2000–2020: no such data."),
    ("en", "special_error_param_18"): ("network", "live_routing", "world", "Bus route; the place name is malformed in the source."),
    ("en", "special_error_param_28"): ("data_acquisition", "internet_data", "usa", "Road-trip highlights; the detour parameter is malformed in the source."),
}

GEO_UPSTREAM = {
    ("zh", "normal_atom_object_deep_27"): {
        "category": "measurement",
        "missing": ["distance_between"],
        "summary": "Is a device at a given coordinate inside a 1,000 m geofence around another coordinate.",
        "notes": "A point-to-point distance check; with both points on the map it is measure_from_here (not connected). Typed coordinates would also need xy_to_points.",
        "confidence": "medium",
    },
    ("zh", "normal_single_turn_single_function_21"): {
        "category": "projection",
        "missing": ["reproject"],
        "summary": "Convert coordinates from WGS84 to UTM with given shifts and rotations.",
        "notes": "A seven-parameter style transform; rustgis_data's Crs handles EPSG transforms, custom Helmert parameters may not be covered.",
        "confidence": "low",
    },
    ("zh", "normal_single_turn_parallel_function_0"): {
        "category": "network",
        "missing": ["shortest_route"],
        "summary": "Best driving routes between home, office and gym given by coordinates, with signal waits.",
        "notes": "Shortest route over a street layer wraps network::shortest_route; signal waiting times would need a cost field, and live traffic is out of scope.",
        "confidence": "low",
    },
    ("zh", "normal_multi_turn_user_switch_2_0"): {
        "category": "network",
        "missing": ["shortest_route"],
        "summary": "Analyse and optimise two colleagues' commutes between given coordinates.",
        "notes": "Routes over a street layer (network::shortest_route); 'optimise' is vague.",
        "confidence": "low",
    },
    ("zh", "normal_single_turn_parallel_function_22"): {
        "category": "measurement",
        "missing": ["distance_between"],
        "summary": "Unit conversion plus a drone flight path between two coordinates.",
        "notes": "The straight path's length is a geodesic distance; unit conversion is not a map task. Low confidence.",
        "confidence": "low",
    },
}

GEO_NEW = {
    ("zh", "normal_atom_enum_0"): {
        "category": "modelling",
        "missing": ["optimisation"],
        "summary": "Assess whether a downtown location suits a coffee shop with a given budget.",
        "notes": "Site suitability; with criteria layers weighted_overlay could help, but footfall and rent data are missing. Low confidence.",
        "confidence": "low",
    },
    ("zh", "normal_atom_list_41"): {
        "category": "modelling",
        "missing": ["regression"],
        "summary": "How land area affected GDP and unemployment in the US and Canada over ten years.",
        "notes": "A regression over time series; no time series in the project either.",
        "confidence": "medium",
    },
}

# Adapted to a map context where today's actions (or a question back) answer the intent.
ADAPTED = {
    ("zh", "normal_multi_turn_user_adjust_11_0"): {
        "category": "measurement",
        "summary": "Area (and symmetry) of a quadrilateral the user drew.",
        "notes": "Adapted: the source gives planar vertex coordinates; in LayeredMap the user draws the area and measure_area answers the area part (symmetry is not a map question).",
        "confidence": "medium",
        "context": "city_drawn",
        "cases": [("zh", "我画了一个四边形区域，帮我算一下它的面积。", [call("measure_area", area=sel("Area 1"))], [], [])],
    },
    ("zh", "normal_atom_list_34"): {
        "category": "attribute_query",
        "summary": "A daily water-quality report for one monitoring station, pH and chlorine only.",
        "notes": "Adapted: the city context has air-quality stations rather than water stations; the intent (monitoring stations above a threshold) maps to a filter. Medium confidence.",
        "confidence": "medium",
        "context": "city",
        "cases": [
            ("zh", "显示PM2.5超过35微克每立方米的空气质量监测站。", [call("filter", layer="Air quality stations", conditions=[cond("pm25", "above", 35)])], [], []),
            ("en", "Show the air-quality stations with PM2.5 above 35 µg/m³.", [call("filter", layer="Air quality stations", conditions=[cond("pm25", "above", 35)])], [], []),
        ],
    },
    ("en", "normal_single_turn_single_function_14"): {
        "category": "spatial_join",
        "summary": "Alert about magnitude 5.0–7.0 earthquakes around Tokyo in the past week.",
        "notes": "Adapted: world_plus has an Earthquakes layer (magnitude, year) but no live feed, so 'past week' is dropped and 'around' becomes 300 km. Filter, buffer Tokyo, then list the earthquakes inside.",
        "confidence": "medium",
        "context": "world_plus",
        "cases": [
            ("en", "Show the earthquakes of magnitude 5.0 to 7.0 within 300 km of Tokyo.", QUAKE_PLANS, [QUAKE_ALT], []),
            ("zh", "显示东京300公里范围内5到7级的地震。", QUAKE_PLANS, [QUAKE_ALT], []),
        ],
    },
    ("zh", "special_incomplete_28"): {
        "category": "vague",
        "summary": "Evaluate the habitat quality of 'our region' (no region or criteria given).",
        "notes": "Adapted to the city context: which area and which measure are both missing, so ask.",
        "confidence": "medium",
        "context": "city",
        "cases": [("zh", "帮我评估一下我们这个地区的环境质量。", ASK, [], [])],
    },
    ("zh", "special_incomplete_37"): {
        "category": "vague",
        "summary": "Effect of a region's environment on residents' health, given only an AQI of 75.",
        "notes": "Adapted to the city context: the neighbourhood is not named, so ask which one (a health-effect model itself is out of scope).",
        "confidence": "low",
        "context": "city",
        "cases": [("zh", "我想看看某个街区的空气质量对居民的影响。", ASK, [], [CANNOT])],
    },
}

INCOMPLETE_GEO_OOS = {
    ("zh", "special_incomplete_43"): ("data_acquisition", "missing_data", "world"),
    ("zh", "special_incomplete_45"): ("network", "missing_data", "city"),
    ("zh", "special_incomplete_47"): ("data_acquisition", "missing_data", "city"),
    ("zh", "special_incomplete_32"): ("data_acquisition", "internet_data", "city"),
    ("zh", "special_incomplete_11"): ("knowledge", "knowledge", "world"),
    ("zh", "special_incomplete_20"): ("knowledge", "knowledge", "world"),
    ("en", "special_incomplete_3"): ("modelling", "prediction", "usa"),
    ("en", "special_incomplete_4"): ("data_acquisition", "internet_data", "city"),
    ("en", "special_incomplete_17"): ("modelling", "missing_data", "city"),
    ("en", "special_incomplete_19"): ("irrelevant", "code", "city"),
    ("en", "special_incomplete_28"): ("data_acquisition", "internet_data", "usa"),
    ("en", "special_incomplete_35"): ("geocoding", "geocoding_service", "usa"),
    ("en", "special_incomplete_42"): ("irrelevant", "irrelevant", "city"),
    ("en", "special_incomplete_43"): ("knowledge", "knowledge", "world"),
}


def load(p: Path) -> list[dict]:
    t = p.read_text(encoding="utf-8").strip()
    try:
        return json.loads(t)
    except ValueError:
        return [json.loads(line) for line in t.splitlines() if line.strip()]


def user_text(q: str) -> str:
    """The user's turns of an ACEBench dialogue ("user: … system: … user: …"), joined."""
    parts = re.split(r"(?:^|\s)(user|system):\s*", q.strip())
    turns = [parts[i + 1].strip() for i in range(1, len(parts) - 1, 2) if parts[i] == "user"]
    return " ".join(t for t in turns if t) or q.strip()


def short(s: str, n: int = 150) -> str:
    s = " ".join(s.split())
    return s if len(s) <= n else s[: n - 1] + "…"


def base(lang: str, item: dict, **kw) -> dict:
    r = {
        "source": SOURCE,
        "source_id": f"{lang}/{item['id']}",
        "licence": LICENCE,
        "text_policy": "verbatim",
        "summary": short(user_text(item["question"])),
        "category": "irrelevant",
        "coverage": "out_of_scope",
        "confidence": "high",
        "notes": "",
        "cases": [],
    }
    r.update(kw)
    return r


def main() -> None:
    out = []
    n = {"zh": 0, "en": 0}

    def cid(lang: str) -> str:
        n[lang] += 1
        return f"ace-{lang}-{n[lang]:03d}"

    for lang in ("zh", "en"):
        items = {}
        for p in sorted((RAW / f"data_all/data_{lang}").glob("data_*.json")):
            for x in load(p):
                items[x["id"]] = x
        # Hand-picked geographic items first, then the refusal-pattern categories.
        for (lg, iid), spec in GEO_OOS.items():
            if lg != lang:
                continue
            x = items[iid]
            cat, reason, ctx, note = spec
            q = user_text(x["question"])
            out.append(base(lang, x, category=cat, oos_reason=reason, notes=note, cases=[case(cid(lang), q, CANNOT, context=ctx, lang=lang)]))
        for table, cov in ((GEO_UPSTREAM, "upstream"), (GEO_NEW, "new")):
            for (lg, iid), s in table.items():
                if lg != lang:
                    continue
                x = items[iid]
                out.append(base(lang, x, text_policy="adapted", summary=s["summary"], category=s["category"], coverage=cov, missing=s["missing"], confidence=s["confidence"], notes=s["notes"]))
        for (lg, iid), s in ADAPTED.items():
            if lg != lang:
                continue
            x = items[iid]
            cases = []
            for clang, text, plan, alts, lalts in s["cases"]:
                cases.append(case(cid(clang), text, plan, context=s["context"], lang=clang, alts=alts, lenient_alts=lalts))
            out.append(base(lang, x, text_policy="adapted", summary=s["summary"], category=s["category"], coverage="now", confidence=s["confidence"], notes=s["notes"], cases=cases))
        for kind in ("special_irrelevant", "special_incomplete"):
            for iid in sorted((i for i in items if i.startswith(kind + "_")), key=lambda s: int(s.rsplit("_", 1)[1])):
                if (lang, iid) in ADAPTED:
                    continue
                x = items[iid]
                q = user_text(x["question"])
                cat, reason, ctx = INCOMPLETE_GEO_OOS.get((lang, iid), ("irrelevant", "irrelevant", ["world", "shanghai", "city", "usa"][int(iid.rsplit("_", 1)[1]) % 4]))
                note = "Not a map task." if kind == "special_irrelevant" else "Incomplete in the source (a required parameter is missing); for LayeredMap it is not a map task, so refusing beats asking."
                out.append(base(lang, x, category=cat, oos_reason=reason, notes=note, cases=[case(cid(lang), q, CANNOT, context=ctx, lang=lang)]))
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in out), encoding="utf-8")
    print(f"{OUT.name}: {len(out)} records")


if __name__ == "__main__":
    main()
