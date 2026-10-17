"""GeoQuery-inspired cases: original US-geography questions of the kinds in GeoQuery
(Zelle & Mooney 1996; 880 questions over a Prolog "Geobase" of states, cities, rivers, mountains and
lakes), mapped to LayeredMap over the `geoquery_usa` context.

GeoQuery is distributed by the UT Austin ML group "under GPL 2.0". Copying its questions into this
proprietary product's repository is not done; every question here is written from scratch to cover
the same question types (population, area, capitals, largest/smallest, borders, rivers through
states, elevation, density, lakes, mountains) plus a few that need our extra layers. Records are
one per original question (source_id "orig-NN"), not one per GeoQuery item.

    python -I tools/lm/benchmarks/convert/geoquery.py   → sources/geoquery.jsonl
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))  # python -I drops the script's folder

from f4_common import ASK, call, case, cond, dist, obj, rec, refuse, sel, write  # noqa: E402,F401

SRC = "GeoQuery (inspired-by)"
LIC = "GeoQuery data: GPL-2.0 (UT Austin ML group page); cases here are original, inspired-by"
C = "geoquery_usa"
N = [0]


def st(name, abbrev):
    return obj("States", ("name", name), ("abbrev", abbrev))


def Q(summary, category, coverage, en=None, plan=None, zh=None, **kw):
    """One original question. en/zh are request texts; plan is the expected plan (now) or None."""
    N[0] += 1
    n = f"{N[0]:02d}"
    cases = []
    alts = kw.pop("alts", ())
    lenient = kw.pop("lenient_alts", ())
    cnotes = kw.pop("case_notes", "")
    if coverage == "now":
        cases.append(case(f"gq-{n}", en, plan, context=C, alts=alts, lenient_alts=lenient, notes=cnotes))
        if zh:
            cases.append(case(f"gq-{n}-zh", zh, plan, context=C, lang="zh", alts=alts, lenient_alts=lenient, notes=cnotes))
    elif coverage == "out_of_scope":
        cases.append(refuse(f"gq-{n}", en, C))
        if zh:
            cases.append(refuse(f"gq-{n}-zh", zh, C, lang="zh"))
    return rec(SRC, LIC, f"orig-{n}", summary, category, coverage, text_policy="own_words", cases=cases, **kw)


LOOKUP = "no attribute-lookup action: zoom_to selects the object and its card shows its fields; filtering to it is an accepted alternative"
TOUCH = "whats_inside returns everything that touches or lies inside the area, so neighbouring states (which share a boundary) are listed along with the state itself; depends on clean shared boundaries"

records = [
    Q("population of a named state", "attribute_query", "now", "What is the population of Texas?",
      [call("zoom_to", target=st("Texas", "TX"))], "得克萨斯州有多少人口？", confidence="medium", notes=LOOKUP,
      alts=[[call("filter", layer="States", conditions=[cond("name", "is", "Texas")])]], lenient_alts=[[call("view_data", layer="States")]]),
    Q("most populous state", "statistics", "now", "Which state has the most people?", [call("rank_by_value", layer="States", field="population")],
      "哪个州人口最多？"),
    Q("smallest state by area", "statistics", "now", "Which is the smallest state by area?", [call("rank_by_value", layer="States", field="area_km2")]),
    Q("capital of a named state", "attribute_query", "now", "What is the capital of California?",
      [call("filter", layer="Cities", conditions=[cond("state", "is", "California"), cond("capital", "is", "true")])], "加利福尼亚州的首府是哪里？"),
    Q("largest city of a state", "statistics", "now", "What is the biggest city in California?",
      [call("filter", layer="Cities", conditions=[cond("state", "is", "California")]), call("rank_by_value", layer="Cities", field="population")],
      "加州最大的城市是哪个？"),
    Q("number of large cities in a state", "attribute_query", "now", "How many cities in Ohio have more than 100,000 people?",
      [call("filter", layer="Cities", conditions=[cond("state", "is", "Ohio"), cond("population", "above", 100000)])], confidence="medium",
      notes="the filter's result reports how many objects are shown"),
    Q("states bordering a state", "spatial_join", "now", "Which states border Texas?", [call("whats_inside", area=st("Texas", "TX"), layer="States")],
      "哪些州与得克萨斯州接壤？", confidence="medium", notes=TOUCH),
    Q("number of bordering states", "spatial_join", "now", "How many states border Kentucky?", [call("whats_inside", area=st("Kentucky", "KY"), layer="States")],
      confidence="medium", notes=TOUCH + "; the count includes Kentucky itself"),
    Q("rivers through a state", "spatial_join", "now", "Which rivers run through Colorado?", [call("whats_inside", area=st("Colorado", "CO"), layer="Rivers")],
      "哪些河流流经科罗拉多州？"),
    Q("length of a named river", "measurement", "now", "How long is the Mississippi River?",
      [call("measure_length", line=obj("Rivers", ("name", "Mississippi"), lenient=["Mississippi River", "密西西比河"]))], confidence="medium",
      notes="assumes the river is one feature; a river split into many segments would need a merge first"),
    Q("states a named river flows through", "spatial_join", "upstream", missing=["select_by_location"],
      notes="the river is a line; whats_inside needs an area. A thin buffer then whats_inside approximates it"),
    Q("highest mountain of a state", "statistics", "now", "What is the highest mountain in Alaska?",
      [call("filter", layer="Mountains", conditions=[cond("state", "is", "Alaska")]), call("rank_by_value", layer="Mountains", field="elevation_m")],
      "阿拉斯加州最高的山是哪座？"),
    Q("highest mountain in the country", "statistics", "now", "What is the highest mountain in the United States?",
      [call("rank_by_value", layer="Mountains", field="elevation_m")]),
    Q("average elevation of a state", "zonal_stats", "now", "What is the average elevation of Colorado?",
      [call("summarize_here", raster="Elevation", area=st("Colorado", "CO"))], "科罗拉多州的平均海拔是多少？"),
    Q("population density of each state", "field_calc", "upstream", missing=["calculate_field"]),
    Q("most densely populated state", "field_calc", "upstream", missing=["calculate_field"]),
    Q("area of a named state", "measurement", "now", "How large is Alaska in square kilometres?",
      [call("measure_area", area=st("Alaska", "AK"))], lenient_alts=[[call("zoom_to", target=st("Alaska", "AK"))]],
      case_notes="area_km2 holds it too; measuring is the certified way"),
    Q("states above a population threshold", "attribute_query", "now", "Which states have more than 10 million people?",
      [call("filter", layer="States", conditions=[cond("population", "above", 10000000)])], "哪些州人口超过一千万？"),
    Q("total population of a census region", "statistics", "upstream", missing=["summary_statistics"]),
    Q("total population of a state's cities", "spatial_join", "now", "What is the combined population of the cities in each state?",
      [call("count_in", points="Cities", areas="States", stat="sum", field="population")], confidence="medium",
      notes="cities are points in the layer; the sum covers only the listed cities"),
    Q("largest lake", "statistics", "now", "Which lake is the largest?", [call("rank_by_value", layer="Lakes", field="area_km2")], "哪个湖面积最大？"),
    Q("state containing a named lake", "spatial_join", "upstream", missing=["select_by_location"],
      notes="the lake is an area, so which_container (points) does not apply; whats_inside the lake would list the states touching it"),
    Q("state of a named city", "spatial_join", "now", "Which state is Denver in?",
      [call("which_container", point=obj("Cities", ("name", "Denver"), lenient=["丹佛"]), areas="States")], "丹佛位于哪个州？",
      lenient_alts=[[call("zoom_to", target=obj("Cities", ("name", "Denver")))]]),
    Q("cities near a named river", "buffer", "now", "Which cities are within 5 km of the Ohio River?",
      [call("buffer", layer="Rivers", object=obj("Rivers", ("name", "Ohio"), lenient=["Ohio River", "俄亥俄河"]), distance=dist(5000)),
       call("whats_inside", area=sel(["#1"]), layer="Cities")], "俄亥俄河5公里范围内有哪些城市？"),
    Q("hospitals near an interstate", "buffer", "now", "How many hospitals are within 10 km of Interstate 80?",
      [call("buffer", layer="Interstates", object=obj("Interstates", ("route", "I-80"), lenient=["I 80", "80", "Interstate 80"]), distance=dist(10000)),
       call("count_here", points="Hospitals", area=sel(["#1"]))], confidence="medium", notes="route values assumed to be written 'I-80'"),
    Q("states with no Amtrak station", "spatial_join", "now", "Which states have no Amtrak station?",
      [call("count_in", points="Amtrak stations", areas="States"),
       call("filter", layer=["#1", "Amtrak stations in each state"], conditions=[cond("amtrak stations", "below", 1)])],
      alts=[[call("count_in", points="Amtrak stations", areas="States"),
             call("filter", layer=["#1", "Amtrak stations in each state"], conditions=[cond("amtrak stations", "is", 0)])]]),
    Q("state with the most Amtrak stations", "spatial_join", "now", "Which state has the most Amtrak stations?",
      [call("count_in", points="Amtrak stations", areas="States"), call("rank_by_value", layer=["#1", "Amtrak stations in each state"], field="amtrak stations")],
      "哪个州的美铁车站最多？"),
    Q("largest state capital", "statistics", "now", "Which state capital has the largest population?",
      [call("filter", layer="Cities", conditions=[cond("capital", "is", "true")]), call("rank_by_value", layer="Cities", field="population")]),
    Q("lowest point of a state", "zonal_stats", "now", "How low does the land go in California?",
      [call("summarize_here", raster="Elevation", area=st("California", "CA"), stat="min")], confidence="medium",
      notes="gives the lowest value, not where it is"),
    Q("height of a named mountain", "attribute_query", "now", "How tall is Mount Whitney?",
      [call("zoom_to", target=obj("Mountains", ("name", "Mount Whitney"), lenient=["Whitney", "Mt. Whitney", "Mt Whitney", "惠特尼峰"]))],
      "惠特尼峰有多高？", confidence="medium", notes=LOOKUP,
      alts=[[call("filter", layer="Mountains", conditions=[cond("name", "is", "Mount Whitney")])]]),
    Q("mountains above a height", "attribute_query", "now", "Which mountains are higher than 4,000 m?",
      [call("filter", layer="Mountains", conditions=[cond("elevation_m", "above", 4000)])]),
    Q("states that have a mountain above a height", "spatial_join", "now", "Which states have a mountain higher than 4,000 m?",
      [call("filter", layer="Mountains", conditions=[cond("elevation_m", "above", 4000)]), call("count_in", points="Mountains", areas="States"),
       call("filter", layer=["#2", "Mountains in each state"], conditions=[cond("mountains", "above", 0)])], confidence="medium"),
    Q("number of counties in a state", "attribute_query", "now", "How many counties does Texas have?",
      [call("filter", layer="Counties", conditions=[cond("state", "is", "Texas")])], "得克萨斯州有多少个县？", confidence="medium",
      notes="whats_inside would also list counties of neighbouring states that touch Texas"),
    Q("most populous county of a state", "statistics", "now", "Which county in California has the most people?",
      [call("filter", layer="Counties", conditions=[cond("state", "is", "California")]), call("rank_by_value", layer="Counties", field="population")]),
    Q("states of a census region", "attribute_query", "now", "List the states in the South.",
      [call("filter", layer="States", conditions=[cond("region", "is", "South")])], "列出南部地区的州"),
    Q("longest river", "measurement", "upstream", missing=["calculate_geometry"], notes="the river layer has no length field; ranking needs one"),
    Q("rivers longer than a length", "measurement", "upstream", missing=["calculate_geometry"]),
    Q("distance between two cities", "measurement", "upstream", missing=["distance_between"]),
    Q("city closest to a named mountain", "proximity", "upstream", missing=["nearest"]),
    Q("states bordering both of two states", "spatial_join", "upstream", missing=["select_by_location"], confidence="medium",
      notes="two whats_inside lists would have to be intersected"),
    Q("number of states a river crosses", "spatial_join", "upstream", missing=["select_by_location"]),
    Q("big cities of a state", "attribute_query", "now", "Which cities in Texas have more than a million people?",
      [call("filter", layer="Cities", conditions=[cond("state", "is", "Texas"), cond("population", "above", 1000000)])], "得克萨斯州有哪些人口超过一百万的城市？"),
    Q("states richer than a named state", "statistics", "now", "Which states have a higher median income than California?",
      [call("rank_by_value", layer="States", field="median_income")], confidence="medium",
      notes="the threshold is California's own value, which the assistant must not guess; the ranking shows which states are above it"),
    Q("share of national population in one state", "statistics", "upstream", missing=["summary_statistics", "calculate_field"]),
    Q("lakes in a state", "spatial_join", "now", "How many lakes are in Minnesota?", [call("whats_inside", area=st("Minnesota", "MN"), layer="Lakes")],
      confidence="medium", notes="lakes crossing the border are listed too"),
    Q("state with the most lakes", "spatial_join", "upstream", missing=["centroid"], confidence="medium",
      notes="count_in counts points; lakes are areas, so they would need a point each first"),
    Q("highest point of each state", "zonal_stats", "now", "What is the highest elevation in each state?",
      [call("summarize_each", raster="Elevation", areas="States", stat="max")], "每个州的最高海拔是多少？"),
    Q("states ranked by mean elevation", "zonal_stats", "now", "Rank the states by their average elevation.",
      [call("summarize_each", raster="Elevation", areas="States"), call("rank_by_value", layer=["#1", "Elevation in each state"], field="Elevation mean")]),
    Q("state with the highest mean elevation", "zonal_stats", "now", "Which state is highest on average?",
      [call("summarize_each", raster="Elevation", areas="States"), call("rank_by_value", layer=["#1", "Elevation in each state"], field="Elevation mean")],
      "平均海拔最高的是哪个州？"),
    Q("rivers draining to a named sea", "knowledge", "out_of_scope", "Which rivers flow into the Gulf of Mexico?", oos="missing_data", confidence="medium",
      notes="needs river-network connectivity, which the river layer does not carry"),
    Q("states an interstate passes through", "spatial_join", "upstream", missing=["select_by_location"]),
    Q("statehood date", "knowledge", "out_of_scope", "When did Texas become a state?", zh="得克萨斯州是哪一年成为美国的州的？", oos="knowledge"),
    Q("state nickname", "knowledge", "out_of_scope", "What is Florida's nickname?", oos="knowledge"),
    Q("highest major city", "raster_sampling", "now", "Which large city sits at the highest elevation?",
      [call("add_raster_values", raster="Elevation", points="Cities"), call("rank_by_value", layer="Cities", field="Elevation")], "哪个大城市海拔最高？",
      confidence="medium", notes="'large' is not given a threshold; ranking every city is accepted"),
    Q("map states by population", "map_making", "now", "Colour the states by population.", [call("color_by", layer="States", field="population")]),
    Q("states with high mean elevation", "zonal_stats", "now", "Which states average more than 1,500 m above sea level?",
      [call("summarize_each", raster="Elevation", areas="States"),
       call("filter", layer=["#1", "Elevation in each state"], conditions=[cond("Elevation mean", "above", 1500)])]),
    Q("largest county of a state", "statistics", "now", "What is the largest county in Nevada by area?",
      [call("filter", layer="Counties", conditions=[cond("state", "is", "Nevada")]), call("rank_by_value", layer="Counties", field="area_km2")]),
    Q("state with the longest boundary", "measurement", "upstream", missing=["calculate_geometry"], confidence="low",
      notes="a perimeter field would answer it; 'coastline' readings would need coast data"),
    Q("mean temperature of weather stations per state", "spatial_join", "now", "What is the average temperature recorded by the weather stations in each state?",
      [call("count_in", points="Weather stations", areas="States", stat="mean", field="temperature_f")], "各州气象站记录的平均气温是多少？"),
    Q("large wildfires in a state", "spatial_join", "now", "Which wildfires larger than 100,000 acres burned in California?",
      [call("filter", layer="Wildfires", conditions=[cond("acres", "above", 100000)]), call("whats_inside", area=st("California", "CA"), layer="Wildfires")],
      confidence="medium", notes="fires crossing the state line are included"),
    Q("population near a city", "zonal_stats", "now", "What is the average population density within 50 km of Atlanta?",
      [call("buffer", layer="Cities", object=obj("Cities", ("name", "Atlanta"), lenient=["亚特兰大"]), distance=dist(50000)),
       call("summarize_here", raster="Population density", area=sel(["#1"]))], "亚特兰大50公里范围内的平均人口密度是多少？"),
]

if __name__ == "__main__":
    write(records, "geoquery.jsonl")
