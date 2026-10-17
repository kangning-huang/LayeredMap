"""The v2 benchmark: the 78 v1 cases with two corrections, plus 19 new cases.

``python -I make_cases_v2.py`` writes ``cases_v2.jsonl`` next to this file. v1's
``cases.jsonl`` is left as it was, so v1 scoring stays reproducible.

Changes from v1 (README.md › Benchmark changes):

* **x05** "What is the total number of students in each district?" was ``cannot_do``; the runner now
  offers count_in with stat sum of a numeric point field, so the expected plan is
  ``count_in(Schools, Districts, stat=sum, field=students)``. Categories: impossible -> stat.
* **m06** "Rank the African countries by their average elevation": filtering the summary layer
  after summarize_each is as right as filtering Countries first; added as a strict alternative.
* Every expected count_in / summarize_* call now states its ``stat`` (``count`` or ``mean`` when
  the request implies the default), so a plan that asks for another statistic fails.
  ``scoring_v2`` fills the default into a model's call when it gives none.
* 19 new cases (9 in Chinese): 10 for the statistics menus (s01-s10) and 9 for name and
  category resolution (r01-r09), including one ambiguous name (Portland: two places) that
  should be asked about.

Expected names follow the runner (``crates/lm-jobs/src/actions.rs``): count_in with a statistic
adds '<stat word> <field>' ('mean students', 'minimum students'); summarize_each adds
'<raster> mean', '<raster> median', '<raster> minimum', '<raster> maximum', '<raster> sum',
'<raster> std dev'; its layer is '<Raster> in each <area>' for the mean, else
'<Stat> <raster> in each <area>'.
"""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import make_cases as v1  # noqa: E402  (importing builds v1.CASES)
from make_cases import ASK, call, cond, drawn, filt, obj, sel  # noqa: E402

STAT_DEFAULT = {"count_in": "count", "summarize_each": "mean", "summarize_here": "mean", "summarize_drawn": "mean"}

CASES: list[dict] = []


def case(id, cats, lang, ctx, request, expected, sel_=None, alts=(), lenient_alts=(), notes=""):
    CASES.append({"id": id, "categories": cats, "lang": lang, "context": ctx, "selection": sel_, "request": request,
                  "expected": expected, "alts": list(alts), "lenient_alts": list(lenient_alts), "notes": notes, "new_in": "v2"})


# ---------------------------------------------------------------------------------------------
# The v1 cases, corrected

for c in copy.deepcopy(v1.CASES):
    c["new_in"] = "v1"
    if c["id"] == "x05":
        c["categories"] = ["stat", "count_in"]
        c["expected"] = [call("count_in", points="Schools", areas="Districts", stat="sum", field="students")]
        c["lenient_alts"] = []
        c["notes"] = "v2: answerable since count_in takes stat sum and a numeric point field (was cannot_do in v1)."
        c["changed_in_v2"] = "expected cannot_do -> count_in stat=sum field=students"
    if c["id"] == "m06":
        c["alts"] = [[call("summarize_each", raster="Elevation", areas="Countries"),
                      filt(["#1", "Elevation in each country"], cond("continent", "is", "Africa")),
                      call("rank_by_value", layer=["#1", "Elevation in each country"], field=["Elevation mean"])]]
        c["changed_in_v2"] = "alternative added: filter the summary layer after summarize_each"
    CASES.append(c)

SEL_DISTRICT = {"layer": "Districts", "count": 1}
INNER_RING = obj("Roads", ("name", "Inner Ring"), ("road_id", "6"))
PUDONG = obj("Districts", ("name_zh", "浦东新区"), ("name_en", "Pudong New Area"), ("district_id", "D12"))

# ---------------------------------------------------------------------------------------------
# Statistics menus

case("s01", ["stat", "count_in"], "en", "shanghai", "What's the average number of students per school in each district?",
     [call("count_in", points="Schools", areas="Districts", stat="mean", field="students")])
case("s02", ["stat", "count_in", "zh"], "zh", "shanghai", "每个区学校学生人数的中位数是多少？",
     [call("count_in", points="Schools", areas="Districts", stat="median", field="students")])
case("s03", ["stat", "count_in"], "en", "shanghai", "Show the largest school enrolment in each district.",
     [call("count_in", points="Schools", areas="Districts", stat="max", field="students")],
     notes="enrolment = students; the biggest school per district is the maximum.")
case("s04", ["stat", "summarize_each"], "en", "world", "What's the median elevation of each country?",
     [call("summarize_each", raster="Elevation", areas="Countries", stat="median")])
case("s05", ["stat", "summarize_each", "zh"], "zh", "world", "每个国家的最高海拔是多少？",
     [call("summarize_each", raster="Elevation", areas="Countries", stat="max")])
case("s06", ["stat", "summarize_each"], "en", "shanghai", "How much does surface temperature vary within each district? Use the standard deviation.",
     [call("summarize_each", raster="Surface temperature", areas="Districts", stat="std")])
case("s07", ["stat", "summarize_each", "multi"], "en", "world", "Rank the countries by their lowest elevation.",
     [call("summarize_each", raster="Elevation", areas="Countries", stat="min"),
      call("rank_by_value", layer=["#1", "Minimum Elevation in each country"], field=["Elevation minimum"])])
case("s08", ["stat", "summarize_here"], "en", "shanghai", "What's the hottest surface temperature in this district?",
     [call("summarize_here", raster="Surface temperature", area=sel("Districts"), stat="max")], sel_=SEL_DISTRICT)
case("s09", ["stat", "summarize_here", "zh"], "zh", "shanghai_drawn", "我画的区域内地表温度的中位数是多少？",
     [call("summarize_here", raster="Surface temperature", area=drawn(), stat="median")],
     alts=[[call("summarize_drawn", raster="Surface temperature", stat="median")]],
     notes="An area is drawn already (Area 1): summarize_here on it, or summarize_drawn.")
case("s10", ["stat", "count_in"], "en", "world", "What is the combined population of the cities in each country?",
     [call("count_in", points="Populated places", areas="Countries", stat="sum", field="population")])

# ---------------------------------------------------------------------------------------------
# Names and categories (resolve_feature)

case("r01", ["resolve", "measure_length", "zh"], "zh", "shanghai", "南京路有多长？",
     [call("measure_length", line=obj("Roads", ("name", "Nanjing Rd"), ("road_id", "3")))])
case("r02", ["resolve", "filter", "zh"], "zh", "shanghai", "只显示初中",
     [filt("Schools", cond("type", "is", "Middle"))],
     alts=[[filt("Schools", cond("type", "any_of", values=["Middle"]))]],
     notes="初中 = junior secondary = Middle.")
case("r03", ["resolve", "vague", "which_container"], "en", "world", "Which country is Portland in?", [ASK],
     notes="Two places are called Portland (USA and Australia): ask which.")
case("r04", ["resolve", "which_container", "zh"], "zh", "world", "东京在哪个国家？",
     [call("which_container", point=obj("Populated places", ("name", "Tokyo"), ("name_zh", "东京")), areas="Countries")])
case("r05", ["resolve", "summarize_here", "zh"], "zh", "shanghai", "浦东新区的平均地表温度是多少？",
     [call("summarize_here", raster="Surface temperature", area=PUDONG, stat="mean")])
case("r06", ["resolve", "measure_length"], "en", "shanghai", "How long is Huaihai Road?",
     [call("measure_length", line=obj("Roads", ("name", "Huaihai Rd"), ("road_id", "4")))])
case("r07", ["resolve", "multi", "zh", "units"], "zh", "shanghai", "内环周围500米内有哪些学校？",
     [call("buffer", layer="Roads", object=INNER_RING, distance={"distance_m": 500}),
      call("whats_inside", area={"layer": ["#1"], "selected": True}, layer="Schools")])
case("r08", ["resolve", "filter", "zh"], "zh", "shanghai", "只显示城市公园",
     [filt("Parks", cond("kind", "is", "city"))],
     alts=[[filt("Parks", cond("kind", "any_of", values=["city"]))]])
case("r09", ["resolve", "multi"], "en", "shanghai", "How many high schools are there in Pudong?",
     [filt("Schools", cond("type", "is", "High")),
      call("count_here", points="Schools", area=PUDONG)],
     alts=[[filt("Schools", cond("type", "any_of", values=["High"])), call("count_here", points="Schools", area=PUDONG)]])


def with_default_stats(plan: list[dict]) -> list[dict]:
    out = []
    for c in plan:
        if c["name"] in STAT_DEFAULT and "stat" not in c["args"]:
            c = {**c, "args": {**c["args"], "stat": STAT_DEFAULT[c["name"]]}}
        out.append(c)
    return out


def finish(c: dict) -> dict:
    c["expected"] = with_default_stats(c["expected"])
    c["alts"] = [with_default_stats(p) for p in c["alts"]]
    c["lenient_alts"] = [with_default_stats(p) for p in c["lenient_alts"]]
    return c


def main() -> None:
    cases = [finish(c) for c in CASES]
    ids = [c["id"] for c in cases]
    assert len(ids) == len(set(ids)), "duplicate case id"
    with open(HERE / "cases_v2.jsonl", "w", encoding="utf-8") as f:
        for c in cases:
            f.write(json.dumps(c, ensure_ascii=False) + "\n")
    new = [c for c in cases if c["new_in"] == "v2"]
    print(f"{len(cases)} cases: {len(cases) - len(new)} from v1 (2 changed), {len(new)} new "
          f"({sum(c['lang'] == 'zh' for c in new)} Chinese); {sum(c['lang'] == 'zh' for c in cases)} Chinese in all")


if __name__ == "__main__":
    main()
