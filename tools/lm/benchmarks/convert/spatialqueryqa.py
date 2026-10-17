"""SpatialQueryQA (Autonomous-GIS/Spatial-Text-to-SQL, paper "From Questions to Queries", IJDE 2026):
90 natural-language spatial questions over a US PostGIS database (30 per level), mapped to
LayeredMap over the `sqqa_usa` context (the same tables as layers).

The repository has no LICENSE file, so nothing is copied: each record summarises its question in
our own words and the requests are rewritten (identifiers such as county names and unit codes are
facts about the data, kept so the plans can name objects). Downloaded originals stay outside git.

    python -I tools/lm/benchmarks/convert/spatialqueryqa.py   → sources/spatialqueryqa.jsonl
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))  # python -I drops the script's folder

from f4_common import call, case, cond, dist, obj, rec, refuse, sel, write  # noqa: E402

SRC = "SpatialQueryQA"
LIC = "none (no LICENSE file in github.com/Autonomous-GIS/Spatial-Text-to-SQL)"
C = "sqqa_usa"
LOOKUP = "no attribute-lookup action: zoom_to selects the object and its card shows its fields"


def R(sid, summary, category, coverage, en=None, plan=None, zh=None, **kw):
    cases = []
    alts = kw.pop("alts", ())
    lenient = kw.pop("lenient_alts", ())
    n = sid.replace("L", "").replace("-", "")
    if coverage == "now":
        cases.append(case(f"sqqa-{n}", en, plan, context=C, alts=alts, lenient_alts=lenient))
        if zh:
            cases.append(case(f"sqqa-{n}-zh", zh, plan, context=C, lang="zh", alts=alts, lenient_alts=lenient))
    elif coverage == "out_of_scope":
        cases.append(refuse(f"sqqa-{n}", en, C))
    level = sid.split("-")[0]
    notes = kw.pop("notes", "")
    kw["notes"] = f"level {level[1]}" + (f". {notes}" if notes else "")
    return rec(SRC, LIC, sid, summary, category, coverage, cases=cases, **kw)


def zoom(layer, field, value, **lk):
    return [call("zoom_to", target=obj(layer, (field, value), **lk))]


def pa(code):
    return obj("Protected areas", ("unit_code", code))


records = [
    # ------------------------------------------------------------------ level 1
    R("L1-01", "points of interest of one class (benches)", "attribute_query", "now", "Show me all the benches.",
      [call("filter", layer="POIs", conditions=[cond("fclass", "is", "bench")])], "显示所有长椅"),
    R("L1-02", "area of a named county in km²", "measurement", "now", "How many square kilometres is Winn County?",
      [call("measure_area", area=obj("Counties", ("name", "Winn")))], "温县的面积是多少平方公里？"),
    R("L1-03", "roads flagged as bridges", "attribute_query", "now", "Which roads are bridges?",
      [call("filter", layer="Roads", conditions=[cond("bridge", "is", "true")])]),
    R("L1-04", "roads with a speed limit above 50 mph", "attribute_query", "now", "List the roads where the speed limit is over 50 mph.",
      [call("filter", layer="Roads", conditions=[cond("maxspeed", "above", 50)])], "列出限速超过50英里每小时的道路"),
    R("L1-05", "geometry of a named state", "attribute_query", "now", "Show me Nevada.", zoom("States", "name", "Nevada", lenient=["NV", "内华达"]),
      alts=[[call("zoom_to", target=obj("States", ("stusps", "NV")))]]),
    R("L1-06", "length in metres of one road segment by id", "measurement", "now", "How long, in metres, is road segment 360036?",
      [call("measure_length", line=obj("Roads", ("gid", "360036")))]),
    R("L1-07", "location of a named point of interest", "attribute_query", "now", "Where is the Staples store?", zoom("POIs", "name", "Staples"),
      confidence="medium", notes="several POIs may share the name"),
    R("L1-08", "area of a named protected area in km²", "measurement", "now", "What is the area of the Everglades in square kilometres?",
      [call("measure_area", area=obj("Protected areas", ("name", "Everglades"), ("unit_code", "EVER"), lenient=["Everglades National Park", "大沼泽地"]))],
      "大沼泽地国家公园的面积是多少平方公里？"),
    R("L1-09", "weather stations of one state", "attribute_query", "now", "Show the weather stations in Mississippi.",
      [call("filter", layer="Weather stations", conditions=[cond("state", "is", "MS")])], confidence="medium",
      notes="the state field holds postal codes"),
    R("L1-10", "coordinates of a weather station by id", "field_calc", "upstream", missing=["calculate_geometry"], confidence="medium",
      notes="zoom_to shows where it is, not the numbers"),
    R("L1-11", "roads flagged as tunnels", "attribute_query", "now", "Find the tunnels in the road layer.",
      [call("filter", layer="Roads", conditions=[cond("tunnel", "is", "true")])], "找出道路图层里的隧道"),
    R("L1-12", "area of a census tract in m²", "measurement", "now", "What is the area of tract 27027020700?",
      [call("measure_area", area=obj("Tracts", ("geoid", "27027020700")))]),
    R("L1-13", "protected areas of type National Park", "attribute_query", "now", "List the national parks.",
      [call("filter", layer="Protected areas", conditions=[cond("unit_type", "is", "National Park")])], "列出所有国家公园"),
    R("L1-14", "centroid of a state", "generalization", "upstream", missing=["centroid"]),
    R("L1-15", "geometry type of the road layer", "knowledge", "out_of_scope", "What geometry type are the features in the roads layer?", oos="knowledge",
      confidence="low", notes="the layer list already says 'lines'; a direct reply is ideal, which the plan protocol cannot express, so cannot_do is the closest expectation"),
    R("L1-16", "perimeter of a named county in km", "measurement", "now", "What is the perimeter of Glasscock County?",
      [call("measure_area", area=obj("Counties", ("name", "Glasscock")))], confidence="medium",
      notes="the measure gives area and perimeter (geodesic AreaPerimeter); whether the card shows the perimeter is unverified"),
    R("L1-17", "name of a county from its code", "attribute_query", "now", "Which county has the code 27117?", zoom("Counties", "geoid", "27117"),
      confidence="medium", notes=LOOKUP, alts=[[call("filter", layer="Counties", conditions=[cond("geoid", "is", "27117")])]]),
    R("L1-18", "longitude of a POI by id", "field_calc", "upstream", missing=["calculate_geometry"]),
    R("L1-19", "latitude of a POI by id", "field_calc", "upstream", missing=["calculate_geometry"]),
    R("L1-20", "geometry of a named road", "attribute_query", "now", "Show me Veronica Road.", zoom("Roads", "name", "Veronica Road")),
    R("L1-21", "number of convenience stores", "attribute_query", "now", "How many convenience stores are there?",
      [call("filter", layer="POIs", conditions=[cond("fclass", "is", "convenience")])], "一共有多少家便利店？", confidence="medium",
      notes="the filter's result reports how many are shown"),
    R("L1-22", "area of a time zone in km²", "measurement", "now", "How big is the UTC−3 time zone in square kilometres?",
      [call("measure_area", area=obj("Time zones", ("name", "-3")))], confidence="medium"),
    R("L1-23", "counties of a state given by FIPS code", "attribute_query", "now", "List the counties in state 51.",
      [call("filter", layer="Counties", conditions=[cond("state", "is", "51")])]),
    R("L1-24", "geometry of a protected area by unit code", "attribute_query", "now", "Zoom to the protected area EVER.", [call("zoom_to", target=pa("EVER"))]),
    R("L1-25", "geometry of a block group by code", "attribute_query", "now", "Show block group 060710017031.", zoom("Block groups", "geoid", "060710017031")),
    R("L1-26", "water area attribute of a named state", "attribute_query", "now", "What water area is recorded for Guam?", zoom("States", "name", "Guam"),
      confidence="medium", notes=LOOKUP),
    R("L1-27", "recorded elevation of a named weather station", "attribute_query", "now", "What elevation is recorded for the Howell Canyon station?",
      zoom("Weather stations", "name", "Howell Canyon"), confidence="medium", notes=LOOKUP),
    R("L1-28", "time zone whose places mention a country", "attribute_query", "now", "Which time zone covers New Zealand?",
      [call("filter", layer="Time zones", conditions=[cond("places", "is", "New Zealand")])], confidence="medium",
      alts=[[call("zoom_to", target=obj("Time zones", ("places", "New Zealand")))],
            [call("filter", layer="Time zones", conditions=[cond("places", "contains", "New Zealand")])]]),
    R("L1-29", "geometry type of the POI layer", "knowledge", "out_of_scope", "What geometry type does the POI layer have?", oos="knowledge", confidence="low",
      notes="as L1-15"),
    R("L1-30", "name of a state from its FIPS code", "attribute_query", "now", "Which state has FIPS code 41?", zoom("States", "statefp", "41"),
      confidence="medium", notes=LOOKUP, alts=[[call("filter", layer="States", conditions=[cond("statefp", "is", "41")])]]),
    # ------------------------------------------------------------------ level 2
    R("L2-01", "roads within a block group", "spatial_join", "now", "Which roads are inside block group 421010336004?",
      [call("whats_inside", area=obj("Block groups", ("geoid", "421010336004")), layer="Roads")], confidence="medium",
      notes="whats_inside also lists roads that only touch the block group"),
    R("L2-02", "five POIs nearest a protected area", "proximity", "upstream", missing=["nearest"]),
    R("L2-03", "five weather stations nearest a time zone", "proximity", "upstream", missing=["nearest"]),
    R("L2-04", "weather stations inside a named protected area", "spatial_join", "now", "Which weather stations are inside Sleeping Bear Dunes?",
      [call("whats_inside", area=obj("Protected areas", ("name", "Sleeping Bear Dunes NL"), lenient=["Sleeping Bear Dunes", "Sleeping Bear Dunes National Lakeshore"]),
            layer="Weather stations")], "睡熊沙丘国家湖岸内有哪些气象站？"),
    R("L2-05", "five roads nearest a weather station", "proximity", "upstream", missing=["nearest"]),
    R("L2-06", "five POIs nearest a weather station", "proximity", "upstream", missing=["nearest"]),
    R("L2-07", "POIs within 5 km of a named road", "buffer", "now", "Which points of interest are within 5 km of Veronica Road?",
      [call("buffer", layer="Roads", object=obj("Roads", ("name", "Veronica Road")), distance=dist(5000)),
       call("whats_inside", area=sel(["#1"]), layer="POIs")], "距离Veronica Road 5公里以内有哪些兴趣点？"),
    R("L2-08", "POIs nearest a county's centroid", "proximity", "upstream", missing=["centroid", "nearest"]),
    R("L2-09", "three states nearest a time zone", "proximity", "upstream", missing=["nearest"]),
    R("L2-10", "weather stations in a state given by FIPS code", "spatial_join", "now", "Which weather stations are in state 41?",
      [call("whats_inside", area=obj("States", ("statefp", "41")), layer="Weather stations")]),
    R("L2-11", "five tracts nearest a state's centroid", "proximity", "upstream", missing=["centroid", "nearest"]),
    R("L2-12", "tracts intersecting a protected area", "spatial_join", "now", "Which census tracts overlap the Mojave protected area (MOJA)?",
      [call("whats_inside", area=pa("MOJA"), layer="Tracts")], "哪些人口普查区与莫哈韦保护区（MOJA）重叠？"),
    R("L2-13", "three protected areas nearest a county", "proximity", "upstream", missing=["nearest"]),
    R("L2-14", "counties within 20 km of a named POI", "buffer", "now", "Which counties are within 20 km of a Staples store?",
      [call("buffer", layer="POIs", object=obj("POIs", ("name", "Staples")), distance=dist(20000)), call("whats_inside", area=sel(["#1"]), layer="Counties")],
      confidence="medium", notes="if several POIs are named Staples, buffering one object covers only one of them"),
    R("L2-15", "weather stations in a named state", "spatial_join", "now", "List the weather stations in South Dakota.",
      [call("whats_inside", area=obj("States", ("name", "South Dakota"), ("stusps", "SD")), layer="Weather stations")], "列出南达科他州的气象站",
      alts=[[call("filter", layer="Weather stations", conditions=[cond("state", "is", "SD")])]]),
    R("L2-16", "roads within 1 km of a block group", "buffer", "upstream", missing=["buffer_polygons"]),
    R("L2-17", "time zones intersecting a named state", "spatial_join", "now", "Which time zones does Nevada fall in?",
      [call("whats_inside", area=obj("States", ("name", "Nevada"), ("stusps", "NV")), layer="Time zones")]),
    R("L2-18", "POIs in a named county of a given state", "spatial_join", "now", "What points of interest are in Centre County, Pennsylvania?",
      [call("whats_inside", area=obj("Counties", ("name", "Centre")), layer="POIs")], confidence="medium",
      notes="an object is named by one field; the state qualifier cannot be added, which is safe only if the name is unique"),
    R("L2-19", "three protected areas nearest a named POI", "proximity", "upstream", missing=["nearest"]),
    R("L2-20", "five POIs nearest a station by id", "proximity", "upstream", missing=["nearest"]),
    R("L2-21", "weather stations within 15 km of the protected areas of one region", "buffer", "upstream", missing=["buffer_polygons"]),
    R("L2-22", "five roads nearest a named time zone", "proximity", "upstream", missing=["nearest"]),
    R("L2-23", "five block groups nearest a time zone's centroid", "proximity", "upstream", missing=["centroid", "nearest"]),
    R("L2-24", "tracts overlapping a named county", "spatial_join", "now", "Which tracts overlap Conecuh County?",
      [call("whats_inside", area=obj("Counties", ("name", "Conecuh")), layer="Tracts")]),
    R("L2-25", "ten roads nearest a tract", "proximity", "upstream", missing=["nearest"]),
    R("L2-26", "roads intersecting a tract by id", "spatial_join", "now", "Which roads cross tract 21463?",
      [call("whats_inside", area=obj("Tracts", ("gid", "21463")), layer="Roads")]),
    R("L2-27", "counties overlapping a named protected area", "spatial_join", "now", "Which counties does the Everglades overlap?",
      [call("whats_inside", area=obj("Protected areas", ("name", "Everglades"), ("unit_code", "EVER")), layer="Counties")], "大沼泽地跨越了哪些县？"),
    R("L2-28", "five POIs nearest a named station", "proximity", "upstream", missing=["nearest"]),
    R("L2-29", "POIs inside the block groups of one county code", "spatial_join", "upstream", missing=["select_by_location"],
      notes="the block groups of one county are many areas; whats_inside takes one"),
    R("L2-30", "tracts intersecting a county by code", "spatial_join", "now", "List the tracts that intersect county 27117.",
      [call("whats_inside", area=obj("Counties", ("geoid", "27117")), layer="Tracts")]),
    # ------------------------------------------------------------------ level 3
    R("L3-01", "POI count per state, sorted", "spatial_join", "now", "Count the points of interest in each state and sort the states by that count.",
      [call("count_in", points="POIs", areas="States"), call("rank_by_value", layer=["#1", "POIs in each state"], field="pois")],
      "统计每个州的兴趣点数量，并按数量排序"),
    R("L3-02", "road length inside protected areas, by park type", "spatial_join", "new", missing=["line_in_polygon_summary", "summary_statistics"]),
    R("L3-03", "average county area per census region (via states)", "spatial_join", "new", missing=["spatial_join", "calculate_geometry", "summary_statistics"]),
    R("L3-04", "total length of the 20 roads nearest a time zone", "proximity", "upstream", missing=["nearest", "calculate_geometry", "summary_statistics"]),
    R("L3-05", "weather-station count per time zone, keeping zones with more than 5", "spatial_join", "now",
      "Which time zones have more than 5 weather stations?",
      [call("count_in", points="Weather stations", areas="Time zones"),
       call("filter", layer=["#1", "Weather stations in each time zone"], conditions=[cond("weather stations", "above", 5)])]),
    R("L3-06", "highest station elevation per state", "spatial_join", "now", "What is the highest weather-station elevation in each state?",
      [call("count_in", points="Weather stations", areas="States", stat="max", field="elev")], "每个州气象站的最高海拔是多少？"),
    R("L3-07", "area of tracts intersecting block groups, per county", "spatial_join", "new", missing=["polygon_in_polygon_summary"]),
    R("L3-08", "bridges among the 20 roads nearest a protected area", "proximity", "upstream", missing=["nearest"]),
    R("L3-09", "number of roads crossing more than one county", "spatial_join", "new", missing=["spatial_join"]),
    R("L3-10", "block groups per state, keeping states whose mean block-group area exceeds 100 km²", "spatial_join", "new",
      missing=["polygon_in_polygon_summary", "calculate_geometry"]),
    R("L3-11", "total area of the 10 block groups nearest a tract", "proximity", "upstream", missing=["nearest", "calculate_geometry", "summary_statistics"]),
    R("L3-12", "total length of fast roads that intersect tracts", "measurement", "upstream", missing=["select_by_location", "calculate_geometry", "summary_statistics"]),
    R("L3-13", "POI counts by class within one state's counties", "statistics", "upstream", missing=["select_by_location", "frequency"]),
    R("L3-14", "weather stations in the largest state by land area", "spatial_join", "now", "How many weather stations are in the state with the most land area?",
      [call("count_in", points="Weather stations", areas="States"), call("rank_by_value", layer=["#1", "Weather stations in each state"], field="aland")],
      confidence="low", notes="the plan cannot name the largest state before ranking; counting every state and ranking by land area puts the answer in the top row"),
    R("L3-15", "total water area of states containing national parks", "statistics", "upstream", missing=["select_by_location", "summary_statistics"]),
    R("L3-16", "mean maximum temperature of stations per protected area", "spatial_join", "now",
      "What is the average maximum temperature recorded at the weather stations in each protected area?",
      [call("filter", layer="Weather stations", conditions=[cond("element", "is", "TMAX")]),
       call("count_in", points="Weather stations", areas="Protected areas", stat="mean", field="value")], "每个保护区内气象站的平均最高气温是多少？"),
    R("L3-17", "average county perimeter per state", "statistics", "upstream", missing=["calculate_geometry", "summary_statistics"]),
    R("L3-18", "ten POIs nearest the largest county, counted by class", "proximity", "upstream", missing=["calculate_geometry", "nearest", "frequency"]),
    R("L3-19", "weather stations per county, for states above a land-area threshold", "spatial_join", "new", missing=["spatial_join"],
      notes="the counties carry a state code, not the state's land area"),
    R("L3-20", "POIs within 10 km of protected areas", "buffer", "upstream", missing=["buffer_polygons"]),
    R("L3-21", "minimum and maximum latitude of stations per time zone", "field_calc", "upstream", missing=["calculate_geometry"],
      notes="with a latitude field, count_in with stat min and max would do the rest"),
    R("L3-22", "mean speed limit of the 10 roads nearest a protected area", "proximity", "upstream", missing=["nearest", "summary_statistics"]),
    R("L3-23", "tracts larger than the mean tract area, counted by code prefix", "statistics", "upstream",
      missing=["calculate_geometry", "summary_statistics", "calculate_field"]),
    R("L3-24", "road length by class for roads intersecting one region's states", "measurement", "upstream",
      missing=["select_by_location", "calculate_geometry", "summary_statistics"]),
    R("L3-25", "the state with most POIs, and POI counts of its counties", "spatial_join", "now",
      "Which state has the most points of interest, and how many does each county have?",
      [call("count_in", points="POIs", areas="States"), call("rank_by_value", layer=["#1", "POIs in each state"], field="pois"),
       call("count_in", points="POIs", areas="Counties")], confidence="low",
      notes="the second part should be limited to the winning state's counties, which a single plan cannot name"),
    R("L3-26", "mean snowfall reading per state for one year", "spatial_join", "now", "What was the average snowfall reading per state in 2024?",
      [call("filter", layer="Weather stations", conditions=[cond("element", "is", "SNOW"), cond("date", "after", "2023-12-31")]),
       call("count_in", points="Weather stations", areas="States", stat="mean", field="value")], confidence="medium",
      notes="assumes the layer holds no readings after 2024",
      alts=[[call("filter", layer="Weather stations", conditions=[cond("element", "is", "SNOW"), cond("date", "after", "2023-12-31"), cond("date", "before", "2025-01-01")]),
             call("count_in", points="Weather stations", areas="States", stat="mean", field="value")]]),
    R("L3-27", "roads in time zones that intersect protected areas", "spatial_join", "new", missing=["select_by_location", "line_in_polygon_summary"]),
    R("L3-28", "ten POIs nearest a protected area, counted by class", "proximity", "upstream", missing=["nearest", "frequency"]),
    R("L3-29", "variance of block-group areas per county", "statistics", "upstream", missing=["calculate_geometry", "summary_statistics"]),
    R("L3-30", "POIs within 10 km of hot stations, counted by class", "buffer", "upstream", missing=["frequency"],
      notes="filter, buffer and whats_inside are possible now; the count per class is not"),
]

if __name__ == "__main__":
    write(records, "spatialqueryqa.jsonl")
