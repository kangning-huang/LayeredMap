"""The benchmark cases, written by hand; ``python -I make_cases.py`` writes ``cases.jsonl``.

Each case: id, categories, language, map context (+ selection), the request, and the expected
plan. A plan is an ordered list of canonical calls ``{"name": action, "args": {...}}``; only the
arguments listed are checked. Notation in expected arguments:

* a layer or field: a string, or a list of acceptable strings (e.g. a result layer as "#1" or by
  the name the runner gives it);
* one object: ``obj(layer, (field, value), …)`` lists every (field, value) pair that names it in
  the data; ``sel(layer)`` is "the selected object" (field and value omitted);
* a distance: ``{"distance_m": metres}``;
* filter conditions: field, op, value / value2 / values (numbers as numbers).

``alts`` are other plans that are equally right (strict and lenient); ``lenient_alts`` are
acceptable but not ideal (lenient only). See README.md › Scoring.
"""

from __future__ import annotations

import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def call(name, **args):
    return {"name": name, "args": args}


def obj(layer, *pairs, lenient=()):
    return {"layer": layer, "any": [list(p) for p in pairs], "lenient": list(lenient)}


def sel(layer):
    return {"layer": layer, "selected": True}


def drawn(layer="Area 1"):
    return {"layer": layer, "selected": True}


def dist(m):
    return {"distance_m": m}


def cond(field, op, value=None, value2=None, values=None, tol=None):
    c = {"field": field, "op": op}
    if value is not None:
        c["value"] = value
    if value2 is not None:
        c["value2"] = value2
    if values is not None:
        c["values"] = values
    if tol is not None:
        c["tol"] = tol
    return c


def filt(layer, *conds):
    return call("filter", layer=layer, conditions=list(conds))


ASK = call("ask_user")
REFUSE = call("cannot_do")

FRANCE = obj("Countries", ("name", "France"), ("name_zh", "法国"), ("iso_a3", "FRA"))
CENTURY = obj("Roads", ("name", "Century Ave"), ("road_id", "2"), lenient=["Century Avenue", "世纪大道", "Century Ave."])
YANAN = obj("Roads", ("name", "Yan'an Rd"), ("road_id", "1"), lenient=["Yan'an Road", "Yanan Road", "Yanan Rd", "延安路", "Yan'an", "Yan’an Rd", "Yan’an Road"])
SCHOOL_COUNT_FIELD = ["schools"]

CASES = []


def case(id, cats, lang, ctx, request, expected, sel_=None, alts=(), lenient_alts=(), notes=""):
    CASES.append(
        {
            "id": id,
            "categories": cats,
            "lang": lang,
            "context": ctx,
            "selection": sel_,
            "request": request,
            "expected": expected,
            "alts": list(alts),
            "lenient_alts": list(lenient_alts),
            "notes": notes,
        }
    )


SEL_SCHOOL = {"layer": "Schools", "count": 1}
SEL_DISTRICT = {"layer": "Districts", "count": 1}
SEL_PLACE = {"layer": "Populated places", "count": 1}

# ---------------------------------------------------------------------------------------------
# Every connected action

case("a01", ["action", "count_in"], "en", "shanghai", "Count the schools in each district.",
     [call("count_in", points="Schools", areas="Districts")])
case("a02", ["action", "count_in", "zh"], "zh", "shanghai", "统计每个区有多少所学校",
     [call("count_in", points="Schools", areas="Districts")])
case("a03", ["action", "count_here"], "en", "shanghai_drawn", "How many schools are in the area I drew?",
     [call("count_here", points="Schools", area=drawn())])
case("a04", ["action", "which_container"], "en", "shanghai", "Which district is this school in?",
     [call("which_container", point=sel("Schools"), areas="Districts")], sel_=SEL_SCHOOL)
case("a05", ["action", "which_container"], "en", "world", "Which country is Timbuktu in?",
     [call("which_container", point=obj("Populated places", ("name", "Timbuktu"), ("name_zh", "通布图")), areas="Countries")])
case("a06", ["action", "whats_inside"], "en", "shanghai", "What schools are inside this district?",
     [call("whats_inside", area=sel("Districts"), layer="Schools")], sel_=SEL_DISTRICT)
case("a07", ["action", "whats_inside"], "en", "world", "Show me all the cities inside Brazil.",
     [call("whats_inside", area=obj("Countries", ("name", "Brazil"), ("name_zh", "巴西"), ("iso_a3", "BRA")), layer="Populated places")],
     lenient_alts=[[filt("Populated places", cond("country", "is", "Brazil"))]],
     notes="Filtering on country = Brazil gives the same places by attribute, not location.")
case("a08", ["action", "summarize_data_here"], "en", "shanghai_drawn", "Summarize the schools in the area I drew.",
     [call("summarize_data_here", area=drawn(), layer="Schools")])
case("a09", ["action", "buffer"], "en", "shanghai", "Show the area within 500 m of each school.",
     [call("buffer", layer="Schools", distance=dist(500))])
case("a10", ["action", "buffer"], "en", "shanghai", "Draw a 1 km buffer around each road, keeping each road's buffer separate.",
     [call("buffer", layer="Roads", distance=dist(1000), merge=False)])
case("a11", ["action", "buffer", "units"], "en", "world", "Show everything within 100 miles of Paris.",
     [call("buffer", layer="Populated places", object=obj("Populated places", ("name", "Paris"), ("name_zh", "巴黎")), distance=dist(160934.4))],
     notes="'Paris' exists only in Populated places (the country is France).")
case("a12", ["action", "buffer", "zh", "units"], "zh", "shanghai", "以每所学校为中心画半径500米的范围",
     [call("buffer", layer="Schools", distance=dist(500))])
case("a13", ["action", "measure_area"], "en", "shanghai", "How big is this district in square kilometres?",
     [call("measure_area", area=sel("Districts"))], sel_=SEL_DISTRICT,
     lenient_alts=[[call("view_data", layer="Districts")]],
     notes="area_km2 also holds the answer, but measuring is the certified way.")
case("a14", ["action", "measure_area"], "en", "world", "What is the area of France?",
     [call("measure_area", area=FRANCE)])
case("a15", ["action", "measure_length"], "en", "shanghai", "How long is Century Ave?",
     [call("measure_length", line=CENTURY)])
case("a16", ["action", "measure_length", "zh"], "zh", "shanghai", "延安路有多长？",
     [call("measure_length", line=YANAN)],
     notes="Value-free context: the model can't see that the road is written 'Yan'an Rd'; strict accepts only data values.")
case("a17", ["action", "summarize_each"], "en", "world", "What's the average elevation in each country?",
     [call("summarize_each", raster="Elevation", areas="Countries")])
case("a18", ["action", "summarize_each"], "en", "shanghai", "Show the land cover shares for each district.",
     [call("summarize_each", raster="Land cover", areas="Districts")])
case("a19", ["action", "summarize_here"], "en", "shanghai", "What's the average surface temperature in this district?",
     [call("summarize_here", raster="Surface temperature", area=sel("Districts"))], sel_=SEL_DISTRICT)
case("a20", ["action", "summarize_here"], "en", "world", "Mean elevation of Nepal?",
     [call("summarize_here", raster="Elevation", area=obj("Countries", ("name", "Nepal"), ("name_zh", "尼泊尔"), ("iso_a3", "NPL")))])
case("a21", ["action", "summarize_drawn"], "en", "world", "I want to draw an area and get the average elevation inside it.",
     [call("summarize_drawn", raster="Elevation")])
case("a22", ["action", "add_raster_values"], "en", "world", "Add the elevation to each city.",
     [call("add_raster_values", raster="Elevation", points="Populated places")])
case("a23", ["action", "rank_by_value"], "en", "shanghai", "Rank the districts by population.",
     [call("rank_by_value", layer="Districts", field="population")])
case("a24", ["action", "rank_by_value", "zh"], "zh", "world", "按GDP给国家排名",
     [call("rank_by_value", layer="Countries", field="gdp_md")])
case("a25", ["action", "summarize_each", "zh"], "zh", "shanghai", "各区的平均地表温度是多少？",
     [call("summarize_each", raster="Surface temperature", areas="Districts")])

# ---------------------------------------------------------------------------------------------
# Filters and styling

case("f01", ["filter"], "en", "world", "Show only countries in Africa.",
     [filt("Countries", cond("continent", "is", "Africa"))],
     alts=[[filt("Countries", cond("continent", "any_of", values=["Africa"]))]])
case("f02", ["filter"], "en", "world", "Only show cities with more than 5 million people.",
     [filt("Populated places", cond("population", "above", 5_000_000))])
case("f03", ["filter"], "en", "world", "Show capitals only.",
     [filt("Populated places", cond("capital", "is", "true"))])
case("f04", ["filter"], "en", "shanghai", "Show primary and middle schools.",
     [filt("Schools", cond("type", "any_of", values=["Primary", "Middle"]))])
case("f05", ["filter"], "en", "shanghai", "Show only schools with more than 500 students.",
     [filt("Schools", cond("students", "above", 500))])
case("f06", ["filter"], "en", "shanghai", "Districts with a population between 1 and 2 million.",
     [filt("Districts", cond("population", "between", 1_000_000, 2_000_000))])
case("f07", ["filter"], "en", "world", "Show cities whose name contains 'San'.",
     [filt("Populated places", cond("name", "contains", "San"))])
case("f08", ["style"], "en", "world", "Color the countries by GDP.",
     [call("color_by", layer="Countries", field="gdp_md")])
case("f09", ["style", "zh"], "zh", "shanghai", "按学生人数给学校上色",
     [call("color_by", layer="Schools", field="students")])
case("f10", ["display"], "en", "world", "Open the attribute table of the cities.",
     [call("view_data", layer="Populated places")])
case("f11", ["display"], "en", "world", "Zoom to Japan.",
     [call("zoom_to", target=obj("Countries", ("name", "Japan"), ("name_zh", "日本"), ("iso_a3", "JPN")))])
case("f12", ["filter", "zh"], "zh", "world", "只显示亚洲的国家",
     [filt("Countries", cond("continent", "is", "Asia"))],
     alts=[[filt("Countries", cond("continent", "any_of", values=["Asia"]))]],
     notes="Value-free: the model must guess that continent values are English.")
case("f13", ["filter"], "en", "world", "Clear the filter on countries.",
     [filt("Countries")])
case("f14", ["filter"], "en", "world", "Show the countries with no GDP figure.",
     [filt("Countries", cond("gdp_md", "is_empty"))])
case("f15", ["filter", "zh", "units"], "zh", "world", "显示人口超过一亿的国家",
     [filt("Countries", cond("pop_est", "above", 100_000_000))])
case("f16", ["filter", "units"], "en", "world", "Countries with a GDP over 1 trillion dollars.",
     [filt("Countries", cond("gdp_md", "above", 1_000_000))],
     notes="gdp_md is in millions of US$: 1 trillion = 1,000,000.")
case("f17", ["filter"], "en", "world", "Show countries whose population estimate is older than 2019.",
     [filt("Countries", cond("pop_year", "below", 2019))])
case("f18", ["filter", "units"], "en", "shanghai", "Show districts bigger than 20,000 hectares.",
     [filt("Districts", cond("area_km2", "above", 200))],
     notes="100 ha = 1 km².")

# ---------------------------------------------------------------------------------------------
# Multi-step plans

case("m01", ["multi"], "en", "world", "How many cities above 1,000 m are there in each country?",
     [call("add_raster_values", raster="Elevation", points="Populated places"),
      filt("Populated places", cond("Elevation", "above", 1000)),
      call("count_in", points="Populated places", areas="Countries")])
case("m02", ["multi"], "en", "shanghai", "Count the schools in each district, then rank the districts by that count.",
     [call("count_in", points="Schools", areas="Districts"),
      call("rank_by_value", layer=["#1", "Schools in each district"], field=SCHOOL_COUNT_FIELD)])
case("m03", ["multi"], "en", "shanghai", "How many primary schools are there in each district?",
     [filt("Schools", cond("type", "is", "Primary")),
      call("count_in", points="Schools", areas="Districts")],
     alts=[[filt("Schools", cond("type", "any_of", values=["Primary"])), call("count_in", points="Schools", areas="Districts")]])
case("m04", ["multi"], "en", "world", "Which capital city sits highest above sea level?",
     [call("add_raster_values", raster="Elevation", points="Populated places"),
      filt("Populated places", cond("capital", "is", "true")),
      call("rank_by_value", layer="Populated places", field="Elevation")],
     alts=[[filt("Populated places", cond("capital", "is", "true")),
            call("add_raster_values", raster="Elevation", points="Populated places"),
            call("rank_by_value", layer="Populated places", field="Elevation")]])
case("m05", ["multi", "zh"], "zh", "shanghai", "统计每个区的高中数量，然后按数量排名",
     [filt("Schools", cond("type", "is", "High")),
      call("count_in", points="Schools", areas="Districts"),
      call("rank_by_value", layer=["#2", "Schools in each district"], field=SCHOOL_COUNT_FIELD)],
     notes="Value-free: the type value 'High' must be guessed from 高中.")
case("m06", ["multi"], "en", "world", "Rank the African countries by their average elevation.",
     [filt("Countries", cond("continent", "is", "Africa")),
      call("summarize_each", raster="Elevation", areas="Countries"),
      call("rank_by_value", layer=["#2", "Elevation in each country"], field=["Elevation mean"])])
case("m07", ["multi"], "en", "shanghai", "Show 2 km around Century Ave and list the schools inside that zone.",
     [call("buffer", layer="Roads", object=CENTURY, distance=dist(2000)),
      call("whats_inside", area={"layer": ["#1"], "selected": True}, layer="Schools")])
case("m08", ["multi"], "en", "world", "Find the most populous capitals.",
     [filt("Populated places", cond("capital", "is", "true")),
      call("rank_by_value", layer="Populated places", field="population")])
case("m09", ["multi", "zh", "units"], "zh", "shanghai", "世纪大道周围1公里内有哪些学校？",
     [call("buffer", layer="Roads", object=CENTURY, distance=dist(1000)),
      call("whats_inside", area={"layer": ["#1"], "selected": True}, layer="Schools")],
     notes="Value-free: 世纪大道 is 'Century Ave' in the data.")
case("m10", ["multi"], "en", "world", "Color each country by how many cities it has.",
     [call("count_in", points="Populated places", areas="Countries"),
      call("color_by", layer=["#1", "Populated places in each country"], field=["populated places"])])

# ---------------------------------------------------------------------------------------------
# More Chinese

case("z01", ["action", "measure_area", "zh"], "zh", "world", "法国的面积是多少平方公里？",
     [call("measure_area", area=FRANCE)])
case("z02", ["action", "add_raster_values", "zh"], "zh", "world", "给每个城市加上海拔值",
     [call("add_raster_values", raster="Elevation", points="Populated places")])
case("z03", ["filter", "zh"], "zh", "world", "只显示首都",
     [filt("Populated places", cond("capital", "is", "true"))])

# ---------------------------------------------------------------------------------------------
# Vague: one question

case("v01", ["vague"], "en", "shanghai", "Analyze the schools.", [ASK])
case("v02", ["vague"], "en", "world", "Show me the big ones.", [ASK])
case("v03", ["vague"], "en", "shanghai", "Make a buffer around the schools.", [ASK],
     lenient_alts=[[call("buffer", layer="Schools")]],
     notes="No distance given; lenient accepts a buffer with any distance (the runner defaults to 1 km).")
case("v04", ["vague"], "en", "world", "Compare elevation.", [ASK])
case("v05", ["vague", "zh"], "zh", "shanghai", "帮我分析一下这些数据", [ASK])

# ---------------------------------------------------------------------------------------------
# Impossible: refuse

case("x01", ["impossible"], "en", "shanghai", "Find the fastest driving route from School 1 to Century Ave.", [REFUSE])
case("x02", ["impossible"], "en", "world", "Forecast each country's population for 2050.", [REFUSE])
case("x03", ["impossible", "zh"], "zh", "world", "预测2030年各国的人口", [REFUSE])
case("x04", ["impossible"], "en", "world", "Download the latest satellite images of Japan.", [REFUSE])
case("x05", ["impossible", "trap"], "en", "shanghai", "What is the total number of students in each district?", [REFUSE],
     lenient_alts=[[ASK]],
     notes="Summing a field per area is not an action (count_in only counts).")
case("x06", ["impossible"], "en", "shanghai", "Which district will get the most new schools next year?", [REFUSE])

# ---------------------------------------------------------------------------------------------
# Wrong-layer traps (the field named exists only in another layer)

case("t01", ["trap", "rank_by_value"], "en", "world", "Rank countries by population.",
     [call("rank_by_value", layer="Countries", field="pop_est")],
     notes="'population' is a Populated places field; Countries has pop_est.")
case("t02", ["trap"], "en", "world", "Color the cities by continent.", [ASK],
     alts=[[REFUSE]],
     lenient_alts=[[call("color_by", layer="Populated places", field="country")]],
     notes="continent is a Countries field; places only have country.")
case("t03", ["trap"], "en", "shanghai", "Show only the districts of type Primary.", [ASK],
     alts=[[REFUSE]],
     lenient_alts=[[filt("Schools", cond("type", "is", "Primary"))]],
     notes="type is a Schools field.")
case("t04", ["trap"], "en", "shanghai", "Rank the schools by area_km2.", [ASK], alts=[[REFUSE]],
     notes="area_km2 is a Districts field.")
case("t05", ["trap", "style"], "en", "shanghai", "Color the districts by population.",
     [call("color_by", layer="Districts", field="population")],
     notes="Control: here population is a Districts field.")
case("t06", ["trap", "filter", "zh"], "zh", "world", "只显示人口超过1000万的城市",
     [filt("Populated places", cond("population", "above", 10_000_000))],
     notes="城市 are Populated places (population), not Countries (pop_est).")

# ---------------------------------------------------------------------------------------------
# Units

case("u01", ["units", "buffer"], "en", "shanghai", "Show a quarter mile around each school.",
     [call("buffer", layer="Schools", distance=dist(402.336))])
case("u02", ["units", "buffer", "zh"], "zh", "shanghai", "以世纪大道为中心，缓冲半径 500 米",
     [call("buffer", layer="Roads", object=CENTURY, distance=dist(500))])
case("u03", ["units", "multi"], "en", "world", "Show the cities higher than 5,000 feet.",
     [call("add_raster_values", raster="Elevation", points="Populated places"),
      filt("Populated places", cond("Elevation", "above", 1524, tol=0.01))],
     notes="Elevation is in metres: 5,000 ft = 1,524 m.")
case("u04", ["units", "buffer"], "en", "world", "Draw 5 km around the selected city.",
     [call("buffer", layer="Populated places", object=sel("Populated places"), distance=dist(5000))], sel_=SEL_PLACE)
case("u05", ["units", "filter"], "en", "shanghai", "Show districts with more than 1.5 million people.",
     [filt("Districts", cond("population", "above", 1_500_000))])


def main() -> None:
    ids = [c["id"] for c in CASES]
    assert len(ids) == len(set(ids)), "duplicate case id"
    with open(HERE / "cases.jsonl", "w", encoding="utf-8") as f:
        for c in CASES:
            f.write(json.dumps(c, ensure_ascii=False) + "\n")
    zh = sum(c["lang"] == "zh" for c in CASES)
    cats: dict[str, int] = {}
    for c in CASES:
        for k in c["categories"]:
            cats[k] = cats.get(k, 0) + 1
    print(f"{len(CASES)} cases ({zh} Chinese)")
    print(", ".join(f"{k} {v}" for k, v in sorted(cats.items())))


if __name__ == "__main__":
    main()
