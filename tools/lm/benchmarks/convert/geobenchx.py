"""GeoBenchX (MIT) → sources/geobenchx.jsonl: one record per task, 202 tasks.

    python -I tools/lm/benchmarks/convert/geobenchx.py [--data DIR]

Reads ``benchmark_set/tasks_and_reference_solutions.json`` from the GeoBenchX checkout
(default ``/Volumes/LaCie/data/layeredmap-benchmarks/geobenchx/src``, commit
0edf6109ea5dca3e70fdfe081d39c9ccd0f832fb, 2026-08-29) and the hand-written mapping below.

Mapping rules (docs/lm/benchmarks/coverage.md › Method):

* Tasks whose reference solutions are all ``reject_task`` (or empty) are unsolvable in GeoBenchX
  because the data is missing: out_of_scope (missing_data), the original text verbatim, in a
  context that lacks that data too (world_plus unless noted).
* Control questions: out_of_scope (knowledge).
* Choropleths of World Bank indicators need GeoBenchX's statistics table joined to the country
  polygons. That join is ``table_join`` (no algorithm yet) unless our world_plus Countries layer
  already carries the indicator (gdp_per_capita, net_migration; pop_est in ``world``), in which
  case the task is ``now`` with medium confidence: the join is assumed done.
* Heat maps need ``kernel_density`` and contour lines ``contours`` (both new).
* "How many X in a set of areas" over several areas at once needs ``select_by_location``
  (count_in gives per-area counts only).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
BENCH = HERE.parent
sys.path.insert(0, str(BENCH))

from benchlib import ASK, CANNOT, call, case, cond, dist, sel  # noqa: E402,F401

DATA = Path("/Volumes/LaCie/data/layeredmap-benchmarks/geobenchx/src")
OUT = BENCH / "sources/geobenchx.jsonl"
SOURCE = "GeoBenchX"
LICENCE = "MIT"

# ---------------------------------------------------------------------------------------------
# Answerable today. index: (category, confidence, notes, [cases...]); case texts are verbatim
# unless the record says adapted.

C = "Countries"


def _now() -> dict[int, dict]:
    t = {}
    t[4] = dict(
        category="statistics", confidence="medium", policy="adapted",
        notes="Adapted: our net_migration field is a count of people for one year, not a 2019 rate; world_plus Countries is assumed to carry the World Bank indicator (GeoBenchX joins it from a table). Eastern Africa is a value of region.",
        cases=[
            ("en", "world_plus", "Which Eastern African countries have the highest net migration?",
             [call("filter", layer=C, conditions=[cond("region", "is", "Eastern Africa")]), call("rank_by_value", layer=C, field="net_migration")],
             [], [[call("filter", layer=C, conditions=[cond("region", "is", "Eastern Africa")]), call("color_by", layer=C, field="net_migration")]]),
            ("zh", "world_plus", "东非哪些国家的净移民人数最多？",
             [call("filter", layer=C, conditions=[cond("region", "is", "Eastern Africa")]), call("rank_by_value", layer=C, field="net_migration")],
             [], [[call("filter", layer=C, conditions=[cond("region", "is", "Eastern Africa")]), call("color_by", layer=C, field="net_migration")]]),
        ])
    snow_plan = [
        call("add_raster_values", raster="Snow depth", points="Amtrak stations"),
        call("filter", layer="Amtrak stations", conditions=[cond("Snow depth", "above", 12)]),
        call("count_in", points="Amtrak stations", areas="States"),
        call("rank_by_value", layer=["#3", "Amtrak stations in each state"], field="amtrak stations"),
    ]
    t[8] = dict(
        category="raster_sampling", confidence="high", policy="verbatim",
        notes="Snow depth is in inches, so 1 foot = 12. GeoBenchX lists the states of the stations; counting per state and ranking answers 'highest number'.",
        cases=[
            ("en", "usa", None, snow_plan, [], []),
            ("zh", "usa", "积雪超过1英尺的地区里，哪些州的Amtrak车站最多？", snow_plan, [], []),
        ])
    bgd = [
        call("filter", layer="Railways", conditions=[cond("country", "is", "Bangladesh")]),
        call("buffer", layer="Railways", distance=dist(1000)),
        call("summarize_here", raster="Population", area=sel("#2"), stat="sum"),
    ]
    t[9] = dict(
        category="zonal_stats", confidence="medium", policy="adapted",
        notes="Our Population raster is global, so the 1 km zone also sums people across the border where a line runs near it (GeoBenchX used a Bangladesh-only raster). Sum over a people-per-cell raster.",
        cases=[
            ("en", "world_plus", "How many people live within 1 km of a railway in Bangladesh?", bgd, [], []),
            ("zh", "world_plus", "孟加拉国铁路1公里范围内住着多少人？", bgd, [], []),
        ])
    towns = [
        call("add_raster_values", raster="Snow depth", points="Cities"),
        call("filter", layer="Cities", conditions=[cond("population", "above", 5000), cond("Snow depth", "above", 36)]),
    ]
    towns_alt = [
        call("filter", layer="Cities", conditions=[cond("population", "above", 5000)]),
        call("add_raster_values", raster="Snow depth", points="Cities"),
        call("filter", layer="Cities", conditions=[cond("population", "above", 5000), cond("Snow depth", "above", 36)]),
    ]
    t[13] = dict(
        category="raster_sampling", confidence="medium", policy="verbatim",
        notes="3 feet = 36 inches. The count is the number of cities the filter shows (no separate count action for one layer).",
        cases=[
            ("en", "usa", None, towns, [towns_alt], []),
            ("zh", "usa", "人口超过5000、积雪超过3英尺的美国城镇有多少个？", towns, [towns_alt], []),
        ])
    t[15] = dict(
        category="raster_sampling", confidence="medium", policy="adapted",
        notes="Adapted to our usa context: hospitals stand in for Algeria's mineral extraction facilities (not in any of our contexts); same operation (points kept by a raster value).",
        cases=[
            ("en", "usa", "How many hospitals are in places with a population density over 1,000 people per km²?",
             [call("add_raster_values", raster="Population density", points="Hospitals"), call("filter", layer="Hospitals", conditions=[cond("Population density", "above", 1000)])], [], []),
        ])
    ports = [call("buffer", layer="Rivers", distance=dist(5000)), call("count_here", points="Seaports", area=sel("#1"))]
    t[28] = dict(
        category="buffer", confidence="medium", policy="adapted",
        notes="Adapted: world_plus Seaports has no region field, so 'Latin American' is dropped; 'major' rivers are all rivers in the layer. Merged buffer gives one area for count_here.",
        cases=[
            ("en", "world_plus", "How many seaports are within 5 km of a river?", ports, [], []),
            ("zh", "world_plus", "距离河流5公里以内的港口有多少个？", ports, [], []),
        ])
    t[31] = dict(
        category="buffer", confidence="medium", policy="adapted",
        notes="Adapted: 'in Africa' dropped (Power plants and Railways carry a country, not a continent); all railways stand for 'major railways'.",
        cases=[
            ("en", "world_plus", "How many power plants are within 10 km of a railway?",
             [call("buffer", layer="Railways", distance=dist(10000)), call("count_here", points="Power plants", area=sel("#1"))], [], []),
        ])
    for i, field, zh in [
        (84, "net_migration", "在世界地图上显示各国的净移民情况"),
        (90, "gdp_per_capita", "绘制全球人均GDP分布图"),
        (96, "net_migration", None),
        (100, "gdp_per_capita", None),
    ]:
        cs = [("en", "world_plus", None, [call("color_by", layer=C, field=field)], [], [])]
        if zh:
            cs.append(("zh", "world_plus", zh, [call("color_by", layer=C, field=field)], [], []))
        t[i] = dict(
            category="map_making", confidence="medium", policy="verbatim",
            notes=f"GeoBenchX joins a World Bank table first; world_plus Countries is assumed to carry {field} already (the join is not tested here).",
            cases=cs)
    for i, zh in [(86, None), (97, "绘制全球各国人口分布图")]:
        cs = [("en", "world", None, [call("color_by", layer=C, field="pop_est")], [], [])]
        if zh:
            cs.append(("zh", "world", zh, [call("color_by", layer=C, field="pop_est")], [], []))
        t[i] = dict(category="map_making", confidence="high", policy="verbatim",
                    notes="The eval's world Countries layer has pop_est (Natural Earth); no join needed.", cases=cs)
    cauc = [call("filter", layer=C, conditions=[cond("name", "any_of", values=["Armenia", "Azerbaijan", "Georgia"])]), call("color_by", layer=C, field="pop_est")]
    t[149] = dict(
        category="map_making", confidence="low", policy="verbatim",
        notes="GeoBenchX accepts either reject_task or a population choropleth of the Caucasus countries; our world Countries has pop_est, so a filter to the three South Caucasus states and a choropleth answers it. Whether Russia's North Caucasus belongs is a judgement call; colouring everything is accepted leniently.",
        cases=[("en", "world", None, cauc, [], [[call("color_by", layer=C, field="pop_est")]])])
    t[161] = dict(
        category="map_making", confidence="low", policy="verbatim",
        notes="Natural Earth's 177 countries include only the Bahamas, Jamaica and Trinidad and Tobago among English-speaking Caribbean states; a model can't know that without values, so colouring all countries is the expected plan and the filtered version is an alternative.",
        cases=[("en", "world", None, [call("color_by", layer=C, field="pop_est")],
                [[call("filter", layer=C, conditions=[cond("name", "any_of", values=["Bahamas", "Jamaica", "Trinidad and Tobago"])]), call("color_by", layer=C, field="pop_est")]], [])])
    fires = [call("buffer", layer="Amtrak stations", distance=dist(80467.2)), call("whats_inside", area=sel("#1"), layer="Wildfires")]
    t[174] = dict(
        category="buffer", confidence="medium", policy="verbatim",
        notes="50 miles = 80,467 m. Our Wildfires are perimeters (areas) rather than incident points, so the count is the list whats_inside returns (fires touching the zone); 'active' is taken as every fire in the layer.",
        cases=[
            ("en", "usa", None, fires, [], []),
            ("zh", "usa", "Amtrak车站50英里范围内有多少处野火？", fires, [], []),
        ])
    return t


# ---------------------------------------------------------------------------------------------
# Needs new actions. index: (coverage, category, missing ops, confidence, notes)

TJ = "table_join"
UP = {
    6: ("spatial_join", ["select_by_location"], "medium", "world_plus has pop_growth, so no join is needed; 'high' needs a threshold, and the total over every selected country needs select_by_location (count_in gives per-country counts)."),
    12: ("zonal_stats", ["band_math"], "medium", "Mask the population raster by the flood extent (band math) and sum it; no flood raster in our contexts."),
    19: ("spatial_join", ["select_by_location"], "medium", "net_migration exists in world_plus; filter countries below 0, then the facilities inside any of them (mineral facilities are not in our contexts; count_in only gives per-country counts)."),
    21: ("spatial_join", ["select_by_location"], "medium", "Counties that intersect a river line; buffer + whats_inside would approximate with an arbitrary distance."),
    27: ("statistics", ["summary_statistics"], "medium", "count_in(Earthquakes, Countries) and a filter find the countries; the average of gdp_per_capita over them needs summary_statistics."),
    36: ("field_calc", ["calculate_field"], "low", "Two sums (people in the railway zone, people in the country) and two areas are possible today; the densities and their comparison need a calculation the assistant must not do itself."),
    169: ("spatial_join", ["select_by_location"], "medium", "Towns within 5 miles of a lake AND of a river: two zones intersected, or two location selections; no lakes layer in our contexts."),
    170: ("zonal_stats", ["erase", "calculate_field"], "medium", "People farther than 100 km from seaports = Peru minus the zone (erase), then a share (calculation)."),
    171: ("zonal_stats", ["calculate_field"], "medium", "Both sums are possible today (zone and country); the percentage needs a calculation. Our global raster would also count people across the border."),
    172: ("spatial_join", ["select_by_location"], "medium", "Counties with earthquakes, then stations inside any of them; no earthquakes layer in usa."),
    173: ("spatial_join", ["select_by_location"], "medium", "Counties intersecting both earthquakes and wildfires."),
    176: ("buffer", ["buffer_polygons"], "medium", "Our Wildfires are areas; the certified buffer takes points and lines only. With incident points (GeoBenchX) it would be filter + buffer + summarize_here(sum)."),
    177: ("zonal_stats", ["calculate_field"], "medium", "Zone sum and country sum are possible; the percentage needs a calculation. Mineral facilities are not in our contexts."),
    179: ("buffer", ["polygon_to_line", "select_by_location"], "low", "'Near the border' needs the boundary as a line (polygon_to_line, new); no lakes layer."),
}
NEW = {
    10: ("raster_algebra", ["raster_to_vector", "select_by_location", "calculate_geometry"], "medium", "Areas with > 36 in of snow as polygons, the railways in them, and their total length."),
    14: ("spatial_join", [TJ, "select_by_location"], "medium", "Forest loss comes from a World Bank table."),
    17: ("statistics", [TJ, "select_by_location", "summary_statistics"], "medium", "Electricity consumption comes from a table; average over the selected countries."),
    24: ("raster_algebra", ["raster_to_vector", "select_by_location"], "medium", "Snow > 2 ft as polygons, then lakes within 20 km."),
    29: ("spatial_join", [TJ, "calculate_field", "select_by_location"], "medium", "Urbanisation rate computed from two tables."),
    39: ("spatial_join", [TJ, "select_by_location"], "medium", "Forest share from a table; rivers crossing those countries."),
    41: ("terrain", ["contours"], "high", "Contours of a population raster."),
    42: ("terrain", ["contours", "select_by_location"], "medium", "GeoBenchX accepts either a rejection or contours of Brazil's population raster near selected rivers; either way contours are needed."),
    44: ("density", ["kernel_density", "select_by_location"], "high", "Heat map of earthquakes near Canadian railways."),
    45: ("density", [TJ, "kernel_density", "select_by_location"], "high", "Labour force from a table, then a heat map."),
    47: ("terrain", ["contours"], "high", "Contours of snow accumulation."),
    49: ("density", ["kernel_density", "select_by_location"], "high", "Magnitude-weighted heat map in three countries."),
    51: ("terrain", ["contours"], "high", "Contours of Peru's population raster with provinces and rivers."),
    53: ("terrain", ["contours"], "high", "Contours of Chile's population raster."),
    55: ("density", ["kernel_density"], "high", "Heat map of seaports (Pacific coast selected by country)."),
    56: ("terrain", ["contours"], "high", "Snow contours with stations of northern states."),
    57: ("terrain", ["contours"], "high", "Snow contours near major lakes."),
    59: ("density", [TJ, "centroid", "kernel_density"], "high", "TB cases from a county table, county centroids, heat map."),
    61: ("terrain", ["contours"], "medium", "GeoBenchX labels this Vague, but even after a clarifying question it needs contours."),
    64: ("density", ["kernel_density", "select_by_location"], "high", "Heat map of earthquakes in Pacific states."),
    65: ("density", ["kernel_density"], "medium", "GeoBenchX labels this Vague; any reading needs a heat map."),
    67: ("density", ["kernel_density"], "high", "Heat map weighted by fire size."),
    68: ("density", [TJ, "calculate_field", "kernel_density", "select_by_location"], "high", "Rural share from two tables, then a heat map."),
    71: ("density", [TJ, "kernel_density", "select_by_location"], "medium", "Vague in GeoBenchX; forest depletion from a table, heat map."),
    72: ("density", [TJ, "kernel_density", "select_by_location"], "medium", "Vague in GeoBenchX; water withdrawals from a table, heat map."),
    109: ("temporal", [TJ, "temporal_change", "calculate_field"], "medium", "Forest area change over ten years from a table."),
    110: ("map_making", [TJ, "graduated_symbols"], "medium", "Bivariate map of two joined indicators (graduated_symbols covers styled maps beyond color_by)."),
    111: ("map_making", [TJ, "graduated_symbols"], "medium", "Bivariate map of two joined indicators."),
    112: ("map_making", [TJ, "calculate_field", "graduated_symbols"], "medium", "Rural share computed, then bivariate."),
    113: ("map_making", [TJ, "graduated_symbols"], "medium", "Bivariate CO2 / GHG for two years (world_plus has co2_per_capita but not GHG)."),
    114: ("map_making", [TJ, "calculate_field", "graduated_symbols"], "medium", "Net migration per population, bivariate with fertility."),
    119: ("map_making", [TJ, "graduated_symbols"], "medium", "Fertility by income group: needs fertility and income-group tables."),
    135: ("temporal", [TJ, "temporal_change"], "medium", "A series of maps for three years from a table."),
    141: ("map_making", [TJ], "low", "GDP per capita exists in world_plus, but OECD / high-income membership is a column GeoBenchX's countries have and ours don't."),
    187: ("density", [TJ, "kernel_density", "select_by_location"], "high", "Water withdrawals from a table, heat map of seaports."),
    188: ("density", [TJ, "kernel_density", "select_by_location"], "high", "Water withdrawals from a table, heat map of facilities."),
    192: ("terrain", ["contours"], "high", "Contours of a snow raster."),
    193: ("density", ["kernel_density", "select_by_location"], "medium", "world_plus has pop_growth, so only the heat map and location selection are missing."),
    194: ("density", ["kernel_density", "select_by_location"], "high", "Heat map of earthquakes in South Asia."),
    196: ("density", [TJ, "centroid", "kernel_density"], "high", "TB tables per state, centroids, heat map."),
    198: ("density", ["kernel_density"], "high", "add_raster_values + filter work today; the heat map is missing."),
    201: ("density", [TJ, "calculate_field", "kernel_density", "select_by_location"], "high", "GDP growth from a table, heat map of seaports."),
}
# Choropleths of an indicator that only a joined table provides.
JOIN_MAPS = {
    16: [], 32: [], 34: [], 77: [], 78: [], 79: [], 81: [], 82: [], 83: [], 87: ["calculate_field"], 89: [], 91: [], 92: [],
    93: [], 94: [], 95: [], 98: [], 99: [], 101: [], 102: [], 103: [], 104: [], 106: ["calculate_field"], 120: [], 122: [],
    123: [], 124: [], 130: [], 131: [], 132: [], 133: [], 134: [], 137: [], 138: [], 139: ["calculate_field"], 140: [],
    144: [], 145: [], 147: [], 148: [], 150: [], 153: [], 155: [], 157: [], 162: [], 166: [],
}
for _i, _extra in JOIN_MAPS.items():
    NEW[_i] = ("map_making", [TJ] + _extra, "high",
               "The indicator (World Bank table) is not a field of our Countries layer; joining it needs table_join. After a join, a filter to the named countries and color_by would answer it.")

KNOWLEDGE = {0: "knowledge", 1: "knowledge", 2: "knowledge"}
# Unsolvable tasks placed in another context because world_plus would make them answerable.
REJECT_CONTEXT = {80: "world", 107: "world", 108: "world"}
REJECT_NOTES = {
    80: "Placed in the eval's world context (no gdp_per_capita, no region); in world_plus a choropleth would answer it.",
    107: "Placed in world (no power plants); world_plus has plant capacity but not production.",
    105: "Density would need a calculated field; GeoBenchX expects a rejection.",
    116: "Density would need a calculated field; GeoBenchX expects a rejection.",
}


def summarize(t: dict) -> str:
    steps = []
    for sol in t["reference_solutions"]:
        names = [s["function_name"] for s in sol["steps"]]
        if names:
            steps = names
            break
    text = " ".join(t["task_text"].split())
    return f"{text} [GeoBenchX reference: {' → '.join(steps) if steps else 'no solution / reject'}]"


def is_reject(t: dict) -> bool:
    return all(all(s["function_name"] == "reject_task" for s in sol["steps"]) for sol in t["reference_solutions"])


def build(data: Path) -> list[dict]:
    tasks = json.loads((data / "benchmark_set/tasks_and_reference_solutions.json").read_text(encoding="utf-8"))["tasks"]
    if len(tasks) != 202:
        raise SystemExit(f"expected 202 tasks, found {len(tasks)}")
    now = _now()
    out = []
    for i, t in enumerate(tasks):
        text = " ".join(t["task_text"].split())
        rec = {
            "source": SOURCE, "source_id": t["task_ID"], "licence": LICENCE, "text_policy": "verbatim",
            "summary": summarize(t), "category": "", "coverage": "", "missing": [], "confidence": "high",
            "notes": "", "cases": [],
        }
        labels = t.get("task_labels") or []
        if i in now:
            m = now[i]
            rec.update(category=m["category"], coverage="now", confidence=m["confidence"], notes=m["notes"], text_policy=m["policy"])
            for lang, ctx, req, plan, alts, lalts in m["cases"]:
                cid = f"gbx-{i:03d}" + ("-zh" if lang == "zh" else "")
                rec["cases"].append(case(cid, req or text, plan, context=ctx, lang=lang, alts=alts, lenient_alts=lalts, notes=m["notes"]))
        elif i in KNOWLEDGE:
            rec.update(category="knowledge", coverage="out_of_scope", oos_reason="knowledge",
                       notes="GeoBenchX control question: no map analysis (Blefuscu is fictional).")
            rec["cases"].append(case(f"gbx-{i:03d}", text, CANNOT, context="world_plus"))
        elif i in UP or i in NEW:
            cat, ops, conf, note = (UP if i in UP else NEW)[i]
            from vocab import OPS  # the class follows the ops: new if any needs a new algorithm

            cov = "new" if any(OPS[o][1] == "new" for o in ops) else "upstream"
            rec.update(category=cat, coverage=cov, missing=ops, confidence=conf, notes=note)
        elif is_reject(t):
            ctx = REJECT_CONTEXT.get(i, "world_plus")
            rec.update(category="map_making" if "Merge" in " ".join(labels) else ("density" if "Heatmaps" in " ".join(labels) else "spatial_join"),
                       coverage="out_of_scope", oos_reason="missing_data",
                       confidence="medium" if i in REJECT_NOTES else "high",
                       notes=REJECT_NOTES.get(i, "GeoBenchX expects reject_task: the data the request needs is not available."))
            rec["cases"].append(case(f"gbx-{i:03d}", text, CANNOT, context=ctx, notes=rec["notes"]))
        else:
            raise SystemExit(f"task {i} ({t['task_ID']}) is not mapped: {text}")
        if not rec["missing"]:
            del rec["missing"]
        out.append(rec)
    return out


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--data", default=str(DATA))
    a = ap.parse_args(argv)
    recs = build(Path(a.data))
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in recs), encoding="utf-8")
    from collections import Counter
    print(f"{OUT.name}: {len(recs)} records", dict(Counter(r["coverage"] for r in recs)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
