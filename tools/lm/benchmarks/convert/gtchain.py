"""GTChain-Eval → sources/gtchain.jsonl.

Source: AGI-GIS/GTChain, ``EvalData.json`` (500 tasks), commit
c77d8e00d53e40754b7f45417257f64cb18110c6 (2024-12-11). The paper's benchmark is called
GTChain-Eval; the follow-up GeoTP paper (maxapress, doi 10.48130/ker-0026-0013) trains on a
GeoITC instruction set that is not public, so only this evaluation set is mapped.

Licence: unclear. The LICENSE file is MIT, but the README says the data is "intended for research
preview and non-commercial use only, subject to the model License of LLaMA2". So no task text is
copied: each record's summary is generated from the reference tool chain (tool names and
distances), and every request below is written by us. The raw data stays outside git:

    python -I tools/lm/benchmarks/convert/gtchain.py [/path/to/GTChain]

Classification is mechanical: each GTChain tool maps to a coverage class (TOOLS below); a
chain takes the worst class of its tools (out_of_scope > new > upstream > now). Buffer is "now"
for point and line inputs and "upstream" (buffer_polygons) for area inputs, judged from the input
file's name.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, call, case, cond, dist, sel  # noqa: E402,F401

RAW = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/Volumes/LaCie/data/layeredmap-benchmarks/gtchain/src")
OUT = HERE.parent / "sources/gtchain.jsonl"
SOURCE = "GTChain-Eval"
LICENCE = "unclear: MIT LICENSE file, README restricts to non-commercial research use"

# tool: (coverage, op or oos reason, category)
TOOLS = {
    "Buffer": ("now", None, "buffer"),
    "Select": ("now", None, "attribute_query"),
    "Extract Values to Points": ("now", None, "raster_sampling"),
    "Visualization": ("upstream", "map_export", "map_making"),
    "Summary Statistics": ("upstream", "summary_statistics", "statistics"),
    "Frequency": ("upstream", "frequency", "statistics"),
    "Clip": ("upstream", "clip", "overlay"),
    "Erase": ("upstream", "erase", "overlay"),
    "Intersect": ("upstream", "intersect", "overlay"),
    "Union": ("new", "union_overlay", "overlay"),
    "Near": ("upstream", "nearest", "proximity"),
    "Point Distance": ("new", "distance_matrix", "proximity"),
    "Slope": ("upstream", "slope", "terrain"),
    "Aspect": ("upstream", "aspect", "terrain"),
    "Resample": ("upstream", "resample", "raster_algebra"),
    "NDVI": ("upstream", "band_math", "raster_algebra"),
    "Clip Raster": ("new", "clip_raster", "raster_algebra"),
    "Mosaic": ("new", "mosaic", "raster_algebra"),
    "raster2vector": ("new", "raster_to_vector", "raster_algebra"),
    "Kriging": ("new", "kriging", "interpolation"),
    "Create Thiessen polygons": ("new", "thiessen", "interpolation"),
    "Kernel Density": ("new", "kernel_density", "density"),
    "Point Clustering": ("new", "point_clustering", "clustering"),
    "Get POI By Rectangle": ("out_of_scope", "internet_data", "data_acquisition"),
    "Get POI By Keywords": ("out_of_scope", "internet_data", "data_acquisition"),
    "Obtain one POI": ("out_of_scope", "internet_data", "data_acquisition"),
    "Get Road Network By Rectangle": ("out_of_scope", "internet_data", "data_acquisition"),
    "Get Remote Sensing Image": ("out_of_scope", "internet_data", "data_acquisition"),
    "Land Use Classification Based on Remote Sensing Image": ("out_of_scope", "imagery", "imagery"),
    "Water Extraction From Remote Sensing Image": ("out_of_scope", "imagery", "imagery"),
    "Building Extraction From Remote Sensing Image": ("out_of_scope", "imagery", "imagery"),
    "Road Network Extraction From Remote Sensing Image": ("out_of_scope", "imagery", "imagery"),
}
RANK = {"now": 0, "upstream": 1, "new": 2, "out_of_scope": 3}
AREA_WORDS = re.compile(r"park|lake|zone|area|waterbod|intersect|conservation|residential|industrial|forest|district|boundary|region", re.I)

# ---------------------------------------------------------------------------------------------
# Our own requests for the "now" chains (source index → cases). Same operations, our data.


def two(a, b):
    return [[b, a]]


NOW_CASES: dict[int, list] = {}


def now(i, *cases):
    NOW_CASES[i] = list(cases)


B_RIV_W = call("buffer", layer="Rivers", distance=dist(1000))
B_PP_W = call("buffer", layer="Populated places", distance=dist(500))
B_RIV_U = call("buffer", layer="Rivers", distance=dist(1000))
B_CITY_U = call("buffer", layer="Cities", distance=dist(500))
now(8, case("gtc-0008", "Show 1 km around every river and 500 m around every populated place.", [B_RIV_W, B_PP_W], context="world_plus", alts=two(B_RIV_W, B_PP_W)))
now(
    9,
    case("gtc-0009", "Draw a 1 km zone along each river and a 500 m zone around each city.", [B_RIV_U, B_CITY_U], context="usa", alts=two(B_RIV_U, B_CITY_U)),
    case("gtc-0009-zh", "给每条河流画1公里的缓冲区，再给每个城市画500米的缓冲区", [B_RIV_U, B_CITY_U], context="usa", lang="zh", alts=two(B_RIV_U, B_CITY_U)),
)
now(10, case("gtc-0010", "I need two buffers: 500 metres around the cities and one kilometre along the rivers.", [B_CITY_U, B_RIV_U], context="usa", alts=two(B_CITY_U, B_RIV_U)))
B_HOSP = call("buffer", layer="Hospitals", distance=dist(2000))
now(
    12,
    case("gtc-0012", "Show the area within 2 km of every hospital.", [B_HOSP], context="city"),
    case("gtc-0012-zh", "显示每家医院周边2公里的范围", [B_HOSP], context="city", lang="zh"),
)
now(13, case("gtc-0013", "Show the area within 1 km of each Amtrak station.", [call("buffer", layer="Amtrak stations", distance=dist(1000))], context="usa"))
now(15, case("gtc-0015", "Mark a 200 m radius around each bus stop.", [call("buffer", layer="Bus stops", distance=dist(200))], context="city"))

A_ST = call("add_raster_values", raster="Surface temperature", points="Schools")
A_EL = call("add_raster_values", raster="Elevation", points="Schools")
now(
    56,
    case("gtc-0056", "Add the surface temperature and the elevation to each school.", [A_ST, A_EL], context="city", alts=two(A_ST, A_EL)),
    case("gtc-0056-zh", "给每所学校加上地表温度和海拔两个值", [A_ST, A_EL], context="city", lang="zh", alts=two(A_ST, A_EL)),
)
A_WE = call("add_raster_values", raster="Elevation", points="Weather stations")
A_WL = call("add_raster_values", raster="Land cover", points="Weather stations")
now(57, case("gtc-0057", "Give every weather station its elevation and its land cover class.", [A_WE, A_WL], context="usa", alts=two(A_WE, A_WL)))
A_TC = call("add_raster_values", raster="Tree canopy", points="Trees")
A_BS = call("add_raster_values", raster="Surface temperature", points="Bus stops")
now(58, case("gtc-0058", "Add tree canopy cover to each tree, and surface temperature to each bus stop.", [A_TC, A_BS], context="city", alts=two(A_TC, A_BS)))
A_AIR = call("add_raster_values", raster="Elevation", points="Airports")
now(
    59,
    case("gtc-0059", "Add the elevation to each airport.", [A_AIR], context="world_plus"),
    case("gtc-0059-zh", "给每个机场加上海拔", [A_AIR], context="world_plus", lang="zh"),
)
now(60, case("gtc-0060", "Add annual precipitation to each populated place.", [call("add_raster_values", raster="Precipitation", points="Populated places")], context="world_plus"))
now(61, case("gtc-0061", "What is the ground elevation at each hospital? Put it in the table.", [call("add_raster_values", raster="Elevation", points="Hospitals")], context="usa"))
now(62, case("gtc-0062", "Add the surface temperature to every school.", [call("add_raster_values", raster="Surface temperature", points="Schools")], context="shanghai"))
now(63, case("gtc-0063", "Sample the elevation at every seaport.", [call("add_raster_values", raster="Elevation", points="Seaports")], context="world_plus"))

F_BLD = call("filter", layer="Buildings", conditions=[cond("use", "is", "residential"), cond("height_m", "above", 30)])
now(
    176,
    case("gtc-0176", "Show residential buildings taller than 30 m.", [F_BLD], context="city"),
    case("gtc-0176-zh", "只显示高度超过30米的住宅楼", [F_BLD], context="city", lang="zh"),
)
now(177, case("gtc-0177", "Show counties in Texas with more than 1 million people.", [call("filter", layer="Counties", conditions=[cond("state", "is", "Texas"), cond("population", "above", 1000000)])], context="usa"))
now(178, case("gtc-0178", "Show coal power plants with a capacity over 1,000 MW.", [call("filter", layer="Power plants", conditions=[cond("fuel", "is", "Coal"), cond("capacity_mw", "above", 1000)])], context="world_plus"))
F_LE = call("filter", layer="Countries", conditions=[cond("life_expectancy", "above", 80)])
now(
    179,
    case("gtc-0179", "Show only countries where life expectancy is above 80 years.", [F_LE], context="world_plus"),
    case("gtc-0179-zh", "只显示人均预期寿命超过80岁的国家", [F_LE], context="world_plus", lang="zh"),
)
now(180, case("gtc-0180", "Show only neighbourhoods larger than 10 km².", [call("filter", layer="Neighbourhoods", conditions=[cond("area_km2", "above", 10)])], context="city"))
F_INC = call("filter", layer="States", conditions=[cond("median_income", "above", 70000)])
now(
    181,
    case("gtc-0181", "Show only the states with a median income above $70,000.", [F_INC], context="usa"),
    case("gtc-0181-zh", "只显示家庭收入中位数超过7万美元的州", [F_INC], context="usa", lang="zh"),
)
now(182, case("gtc-0182", "Show earthquakes deeper than 300 km.", [call("filter", layer="Earthquakes", conditions=[cond("depth_km", "above", 300)])], context="world_plus"))
F_CA = call("filter", layer="Hospitals", conditions=[cond("state", "is", "California"), cond("beds", "above", 500)])
now(
    183,
    case("gtc-0183", "Show hospitals in California with more than 500 beds.", [F_CA], context="usa"),
    case("gtc-0183-zh", "显示加州床位超过500张的医院", [F_CA], context="usa", lang="zh"),
)

# ---------------------------------------------------------------------------------------------
# Our own refusal requests for the out_of_scope chains: a template for the deciding tool, a
# place, and a clause for one other tool in the chain.

PLACES = ["Wuhan", "Hangzhou", "Chengdu", "Nanjing", "Xi'an", "Shenzhen", "Lyon", "Porto", "Denver", "Nairobi", "Lima", "Hanoi", "Kraków", "Osaka", "Adelaide", "Bogotá", "Accra", "Tbilisi", "Quito", "Leeds"]
PLACES_ZH = ["武汉", "杭州", "成都", "南京", "西安", "深圳", "苏州", "长沙", "青岛", "昆明"]

OOS_TEMPLATES = {
    "Get POI By Rectangle": ("Fetch every point of interest inside the box around {p} from an online map", "从网上地图获取{p}范围内的所有兴趣点"),
    "Get POI By Keywords": ("Search online for all the coffee shops in {p} and load them", "在网上搜索{p}所有的咖啡店并加载到地图上"),
    "Obtain one POI": ("Look up the location of {p}'s main railway station online", "在网上查一下{p}火车站的位置"),
    "Get Road Network By Rectangle": ("Download the road network of {p} from OpenStreetMap", "从OpenStreetMap下载{p}的道路网"),
    "Get Remote Sensing Image": ("Download a recent satellite image of {p}", "下载一张{p}最新的卫星影像"),
    "Land Use Classification Based on Remote Sensing Image": ("Classify the land use of {p} from a satellite image", "根据卫星影像对{p}进行土地利用分类"),
    "Water Extraction From Remote Sensing Image": ("Extract the water bodies around {p} from satellite imagery", "从卫星影像中提取{p}周边的水体"),
    "Building Extraction From Remote Sensing Image": ("Detect the building footprints of {p} in an aerial photo", "从航拍影像中识别{p}的建筑轮廓"),
    "Road Network Extraction From Remote Sensing Image": ("Trace the roads of {p} from a satellite image", "从卫星影像中提取{p}的道路"),
}
CLAUSE = {
    "Buffer": ("and show 500 m around them", "并画出周边500米的范围"),
    "Kernel Density": ("and make a density map of them", "并做一张密度图"),
    "Point Clustering": ("and group them into clusters", "并对它们做聚类"),
    "Near": ("and find the nearest one to each school", "并找出离每所学校最近的一个"),
    "Point Distance": ("and measure the distances between them", "并计算它们之间的距离"),
    "Intersect": ("and overlay the result with the districts", "再与行政区叠加"),
    "Clip": ("and clip the result to the city limits", "再裁剪到市区范围"),
    "Erase": ("and remove the parts inside parks", "再去掉公园内的部分"),
    "Union": ("and merge it with the districts layer", "再与行政区图层合并"),
    "Frequency": ("and count them by type", "并按类型统计数量"),
    "Visualization": ("and export a map image", "并导出一张地图图片"),
    "Kriging": ("and interpolate a surface from them", "并插值出一个表面"),
    "Create Thiessen polygons": ("and draw Thiessen polygons around them", "并生成泰森多边形"),
    "Slope": ("and compute the slope", "并计算坡度"),
    "Aspect": ("and compute the aspect", "并计算坡向"),
    "Clip Raster": ("and clip it to the district", "并裁剪到该区"),
    "Extract Values to Points": ("and add the values to the sample points", "并把数值提取到采样点上"),
    "Select": ("and keep only the large ones", "并只保留大的"),
    "Summary Statistics": ("and give me summary statistics", "并给出统计汇总"),
    "Resample": ("and resample it to 30 m", "并重采样到30米"),
    "NDVI": ("and compute NDVI", "并计算NDVI"),
    "Mosaic": ("and mosaic the tiles", "并把影像拼接起来"),
    "raster2vector": ("and turn the result into polygons", "并把结果转成多边形"),
}


def oos_request(i: int, tools: list[str], zh: bool) -> str:
    decider = next(t for t in tools if TOOLS[t][0] == "out_of_scope")
    en, zh_t = OOS_TEMPLATES[decider]
    other = next((t for t in tools if t in CLAUSE), None)
    if zh:
        s = zh_t.format(p=PLACES_ZH[i % len(PLACES_ZH)])
        return s + (("，" + CLAUSE[other][1]) if other else "")
    s = en.format(p=PLACES[i % len(PLACES)])
    return s + ((" " + CLAUSE[other][0]) if other else "") + "."


# ---------------------------------------------------------------------------------------------


def chain_of(output: str) -> list[tuple[str, str]]:
    return re.findall(r"\[([A-Za-z0-9 ]+?),\s*\((.*?)\)", output)


def classify(steps: list[tuple[str, str]]):
    tools = [t for t, _ in steps]
    classes = []
    missing: list[str] = []
    for t, args in steps:
        cov, op, _cat = TOOLS[t]
        if t == "Buffer" and AREA_WORDS.search(args.split(",")[0]):
            cov, op = "upstream", "buffer_polygons"
        classes.append(cov)
        if cov in ("upstream", "new") and op not in missing:
            missing.append(op)
    worst = max(classes, key=RANK.get)
    return tools, worst, missing


def summary_of(steps, worst) -> str:
    parts = []
    for t, args in steps:
        dist_m = re.search(r",\s*(\d+(?:\.\d+)?\s*(?:km|m)?)\s*$", args)
        if t == "Buffer" and dist_m:
            d = dist_m.group(1).strip()
            kind = "areas" if AREA_WORDS.search(args.split(",")[0]) else "points/lines"
            parts.append(f"Buffer ({kind}, {d if not d.isdigit() else d + ' m'})")
        else:
            parts.append(t)
    n = len(steps)
    return f"{n}-step tool chain: " + " → ".join(parts)


def main() -> int:
    data = json.loads((RAW / "EvalData.json").read_text(encoding="utf-8"))
    from vocab import OPS

    records = []
    oos_n = 0
    for i, x in enumerate(data):
        steps = chain_of(x["output"])
        tools, worst, missing = classify(steps)
        rec = {
            "source": SOURCE,
            "source_id": f"{i:04d}",
            "licence": LICENCE,
            "text_policy": "own_words",
            "summary": summary_of(steps, worst),
            "coverage": worst,
            "confidence": "high",
            "notes": "",
            "cases": [],
        }
        if worst == "now":
            rec["category"] = TOOLS[tools[0]][2]
            if i not in NOW_CASES:
                raise SystemExit(f"task {i} is 'now' but has no case")
            rec["cases"] = NOW_CASES[i]
        elif worst == "out_of_scope":
            decider = next(t for t in tools if TOOLS[t][0] == "out_of_scope")
            rec["category"] = TOOLS[decider][2]
            rec["oos_reason"] = "imagery" if any(TOOLS[t][1] == "imagery" for t in tools) else "internet_data"
            if rec["oos_reason"] == "imagery":
                rec["category"] = "imagery"
            zh = oos_n % 5 == 4
            rec["cases"] = [case(f"gtc-{i:04d}" + ("-zh" if zh else ""), oos_request(i, tools, zh), CANNOT, context="city", lang="zh" if zh else "en")]
            oos_n += 1
        else:
            news = [m for m in missing if OPS[m][1] == "new"]
            decider = news[0] if news else missing[0]
            rec["category"] = OPS[decider][0]
            rec["missing"] = missing
            if "buffer_polygons" in missing:
                rec["confidence"] = "medium"
                rec["notes"] = "Buffer input judged an area layer from its file name."
            if "map_export" in missing:
                rec["notes"] = (rec["notes"] + " GTChain's Visualization renders a layer to an image file: map export.").strip()
        if i in NOW_CASES and worst != "now":
            raise SystemExit(f"task {i} has cases but is {worst}")
        records.append(rec)
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in records), encoding="utf-8")
    from collections import Counter

    print(OUT.name, len(records), dict(Counter(r["coverage"] for r in records)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
