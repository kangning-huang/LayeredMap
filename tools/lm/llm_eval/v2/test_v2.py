"""Unit tests for harness v2: resolver, checker, scoring modes, text fallback, leakage, dry run.

    python -I -m pytest tools/lm/llm_eval/v2/test_v2.py -q -p no:cacheprovider
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

import checker as chk  # noqa: E402
import prompt_v2  # noqa: E402
import resolve  # noqa: E402
import run_v2  # noqa: E402
import scoring_v2  # noqa: E402
import tools_v2  # noqa: E402


def C(name, **args):
    return {"name": name, "args": args}


# ---------------------------------------------------------------------------------------------
# resolve_feature


def top(layer, text):
    c = resolve.resolve_feature(layer, text)["candidates"]
    return (c[0]["field"], c[0]["value"]) if c else None


def test_resolve_aliases_pinyin_and_name_zh():
    assert top("Roads", "世纪大道") == ("name", "Century Ave")  # alias table
    assert top("Roads", "南京路") == ("name", "Nanjing Rd")  # pinyin
    assert top("Roads", "Huaihai Road") == ("name", "Huaihai Rd")  # abbreviation
    assert top("Populated places", "东京") == ("name_zh", "东京")  # stored name_zh
    assert top("Districts", "Pudong") == ("name_en", "Pudong New Area")  # base name
    assert top("Schools", "高中") == ("type", "High")
    assert top("Populated places", "Timbuctu") == ("name", "Timbuktu")  # fuzzy


def test_resolve_reports_ambiguity_and_returns_at_most_five():
    r = resolve.resolve_feature("Populated places", "Portland")
    assert r["candidates"][0]["objects"] == 2
    assert "ask_user" in resolve.tool_result_text(r)
    r = resolve.resolve_feature("Schools", "中学")
    assert {c["value"] for c in r["candidates"]} == {"Middle", "High"}
    r = resolve.resolve_feature("Populated places", "San")
    assert len(r["candidates"]) <= 5
    assert "error" in resolve.resolve_feature("Elevation", "x")


# ---------------------------------------------------------------------------------------------
# The checker


def check(ctx, call, sel=None, steps=(), request=""):
    k = chk.Checker(ctx, sel, request)
    for s in steps:
        v = k.check(s)
        assert v.ok, v.errors
        k.add(v.call)
    return k.check(call)


def test_checker_corrects_object_names_and_categories():
    v = check("shanghai", C("measure_length", line={"layer": "Roads", "field": "name", "value": "延安路"}))
    assert v.ok and v.call["args"]["line"] == {"layer": "Roads", "field": "name", "value": "Yan'an Rd"}
    v = check("shanghai", C("filter", layer="Schools", conditions=[{"field": "type", "op": "is", "value": "高中"}]))
    assert v.ok and v.call["args"]["conditions"][0]["value"] == "High"
    v = check("world", C("filter", layer="Countries", conditions=[{"field": "continent", "op": "is", "value": "africa"}]))
    assert v.ok and v.call["args"]["conditions"][0]["value"] == "Africa"
    v = check("world", C("filter", layer="Populated places", conditions=[{"field": "capital", "op": "is", "value": "yes"}]))
    assert v.ok and v.call["args"]["conditions"][0]["value"] == "true"


def test_checker_errors_are_specific():
    v = check("world", C("color_by", layer="Populated places", field="continent"))
    assert not v.ok and "Countries" in v.errors[0] and "ask_user" in v.errors[0]
    v = check("world", C("which_container", point={"layer": "Populated places", "field": "name", "value": "Portland"}, areas="Countries"))
    assert not v.ok and "ask_user" in v.errors[0]
    v = check("shanghai", C("buffer", layer="Schools"))
    assert not v.ok and "distance" in v.errors[0]
    v = check("world", C("summarize_each", raster="Countries", areas="Countries"))
    assert not v.ok and "raster" in v.errors[0]
    v = check("shanghai", C("count_in", points="Schools", areas="Districts", stat="sum"))
    assert not v.ok and "field" in v.errors[0]
    v = check("shanghai", C("measure_area", area={"layer": "Districts"}))
    assert not v.ok and "selected" in v.errors[0]
    v = check("shanghai", C("rank_by_value", layer="#1", field="schools"))
    assert not v.ok and "#1" in v.errors[0]


def test_checker_units_refs_and_fields_made_by_steps():
    v = check("shanghai", C("buffer", layer="Schools", distance="500米"))
    assert v.ok and v.call["args"]["distance"] == "500 m"
    steps = [C("add_raster_values", raster="Elevation", points="Populated places"),
             C("filter", layer="Populated places", conditions=[{"field": "capital", "op": "is", "value": "true"}])]
    v = check("world", C("rank_by_value", layer="#2", field="Elevation"), steps=steps)
    assert v.ok and v.call["args"]["layer"] == "Populated places"  # "#2" is a filter: its layer
    v = check("world", C("filter", layer="Populated places", conditions=[{"field": "Elevation", "op": "above", "value": "1,000"}]))
    assert not v.ok  # no Elevation field before add_raster_values
    v = check("shanghai", C("rank_by_value", layer="#1", field="sum students"), steps=[C("count_in", points="Schools", areas="Districts", stat="sum", field="students")])
    assert v.ok
    v = check("world", C("rank_by_value", layer="#1", field="Elevation minimum"), steps=[C("summarize_each", raster="Elevation", areas="Countries", stat="min")])
    assert v.ok


def test_checker_duplicates_and_trailing_display_steps():
    k = chk.Checker("world", None, "Show only countries in Africa.")
    c = C("filter", layer="Countries", conditions=[{"field": "continent", "op": "is", "value": "Africa"}])
    v = k.check(c)
    k.add(v.call)
    assert k.check(c).duplicate_of == 1
    plan = [v.call, C("view_data", layer="Countries"), C("zoom_to", layer="Countries")]
    final, dropped = k.finalize(plan)
    assert final == [v.call] and len(dropped) == 2
    k2 = chk.Checker("world", None, "Show African countries and open their table.")
    final, dropped = k2.finalize(plan)
    assert [c["name"] for c in final] == ["filter", "view_data"]
    k3 = chk.Checker("world", None, "Zoom to Japan.")
    assert k3.finalize([C("zoom_to", layer="Countries")])[0]  # never empties a plan


def test_every_expected_plan_passes_the_checker():
    for case in scoring_v2.CASES_V2.values():
        for plan in [case["expected"]] + case["alts"]:
            if plan[0]["name"] in ("ask_user", "cannot_do"):
                continue
            calls = [{"name": c["name"], "args": run_v2.run_eval.mock_args(c, False)} for c in plan]
            fixed, errors, _ = chk.Checker(case["context"], case.get("selection"), case["request"]).check_plan(calls)
            assert not errors, (case["id"], errors)


# ---------------------------------------------------------------------------------------------
# Scoring modes


def test_v2_scoring_checks_the_statistic_and_v1_mode_does_not():
    good = [C("count_in", points="Schools", areas="Districts", stat="sum", field="students")]
    assert scoring_v2.score_both("x05", good)["v2"]["strict"]
    assert not scoring_v2.score_both("x05", good)["v1"]["strict"]  # v1 expects cannot_do
    plain = [C("count_in", points="Schools", areas="Districts")]
    assert scoring_v2.score_both("a01", plain)["v2"]["strict"]  # no stat = count
    wrong = [C("count_in", points="Schools", areas="Districts", stat="mean", field="students")]
    assert not scoring_v2.score_both("a01", wrong)["v2"]["strict"]
    assert scoring_v2.score_both("a01", wrong)["v1"]["strict"]
    avg = [C("count_in", points="Schools", areas="Districts", stat="average", field="students")]
    assert scoring_v2.score_both("s01", avg)["v2"]["lenient"] and not scoring_v2.score_both("s01", avg)["v2"]["strict"]


def test_m06_accepts_filter_after_summary():
    plan = [C("summarize_each", raster="Elevation", areas="Countries"),
            C("filter", layer="#1", conditions=[{"field": "continent", "op": "is", "value": "Africa"}]),
            C("rank_by_value", layer="#1", field="Elevation mean")]
    assert scoring_v2.score_both("m06", plan)["v2"]["strict"]


def test_case_set():
    v2 = scoring_v2.CASES_V2
    new = [c for c in v2.values() if c["new_in"] == "v2"]
    assert len(v2) == 97 and len(new) >= 10 and sum(c["lang"] == "zh" for c in new) >= 4
    assert set(scoring_v2.CASES_V1) <= set(v2)


# ---------------------------------------------------------------------------------------------
# Text fallback, tools, examples


def test_text_fallback_parses_json_plans():
    known = {t["function"]["name"] for t in tools_v2.per_action_tools("shanghai")}
    t = 'I will call {"tool": "buffer", "arguments": {"layer": "Schools", "distance": "500 m"}}'
    assert run_v2.calls_from_text(t, known) == [("buffer", {"layer": "Schools", "distance": "500 m"})]
    t = '```json\n[{"name": "count_in", "parameters": {"points": "Schools", "areas": "Districts"}}]\n```'
    assert run_v2.calls_from_text(t, known)[0][0] == "count_in"
    t = 'to=functions.cannot_do {"reason": "no routing"}'
    assert run_v2.calls_from_text(t, known) == [("cannot_do", {"reason": "no routing"})]
    known2 = {t["function"]["name"] for t in tools_v2.submit_plan_tools("shanghai")}
    t = '{"steps": [{"action": "count_in", "points": "Schools", "areas": "Districts"}]}'
    assert run_v2.calls_from_text(t, known2)[0][0] == "submit_plan"
    assert run_v2.calls_from_text("Which buffer distance do you want?", known) == []


def test_tools_have_layer_enums_by_slot():
    tools = {t["function"]["name"]: t["function"] for t in tools_v2.per_action_tools("world")}
    p = tools["summarize_each"]["parameters"]["properties"]
    assert p["raster"]["enum"] == ["Elevation"]
    assert "Countries" in p["areas"]["enum"] and "#1" in p["areas"]["enum"] and "Elevation" not in p["areas"]["enum"]
    assert tools["count_in"]["parameters"]["properties"]["stat"]["enum"] == tools_v2.POINT_STATS
    assert "resolve_feature" in tools


def _grams(s):
    s = re.sub(r"\s+", " ", s.casefold())
    return {s[i : i + 3] for i in range(len(s) - 2)}


def test_examples_do_not_leak_test_cases():
    worst = 0.0
    for req, _, _ in prompt_v2.EXAMPLES:
        for c in scoring_v2.CASES_V2.values():
            a, b = _grams(req), _grams(c["request"])
            worst = max(worst, len(a & b) / len(a | b))
    assert worst < 0.35, worst
    text = prompt_v2.static_prefix("per_action")
    for layer in ["Schools", "Districts", "Countries", "Roads", "Elevation", "Century", "Populated"]:
        assert layer not in text


def test_dry_run_is_perfect(tmp_path):
    for design in ("per_action", "submit_plan"):
        assert run_v2.main(["--dry-run", "--design", design, "--models", "openai/gpt-oss-120b", "--out", str(tmp_path)]) == 0
        rows = [json.loads(x) for x in (tmp_path / f"dryrun-v2-{design}" / "openai__gpt-oss-120b.jsonl").read_text().splitlines()]
        assert len(rows) == 97 and all(r["score_v2"]["strict"] for r in rows)
        assert sum(r["score_v1"]["strict"] for r in rows if r["score_v1"]) == 77  # x05 changed


def test_lint_catches_a_filter_nothing_uses():
    k = chk.Checker("world", None, "Rank the African countries by their average elevation.")
    plan = [C("summarize_each", raster="Elevation", areas="Countries", stat="mean"),
            C("filter", layer="Countries", conditions=[{"field": "continent", "op": "is", "value": "Africa"}]),
            C("rank_by_value", layer="#1", field="Elevation mean")]
    assert k.lint(plan) and "Step #2" in k.lint(plan)[0]
    assert not k.lint([plan[1], plan[0], C("rank_by_value", layer="#2", field="Elevation mean")])
    two_filters = [plan[1], C("filter", layer="Populated places", conditions=[{"field": "capital", "op": "is", "value": "true"}])]
    assert not k.lint(two_filters)


def test_empty_filter_only_when_asked_to_clear():
    assert not check("shanghai", C("filter", layer="Parks", conditions=[]), request="只显示城市公园").ok
    assert check("world", C("filter", layer="Countries", conditions=[]), request="Clear the filter on countries.").ok


def test_text_fallback_reads_the_example_notation_and_other_shapes():
    known = {t["function"]["name"] for t in tools_v2.per_action_tools("world")}
    t = '#2 filter(layer="Populated places", conditions=[{field: "Elevation", op: "above", value: "1524"}])'
    assert run_v2.calls_from_text(t, known)[0] == ("filter", {"layer": "Populated places", "conditions": [{"field": "Elevation", "op": "above", "value": "1524"}]})
    assert run_v2.calls_from_text('{"type": "measure_area", "area": {"layer": "Countries"}}', known)[0][0] == "measure_area"
    assert run_v2.calls_from_text('{"measure_area": {"area": {"layer": "Countries"}}}', known)[0][0] == "measure_area"
    assert run_v2.calls_from_text('{"question": "Which one?"}', known)[0][0] == "ask_user"
    assert run_v2.calls_from_text('{"plan": [{"call": "view_data", "args": {"layer": "Countries"}}]}', known)[0][0] == "view_data"


def test_buffer_of_a_selected_object_and_date_ops():
    sel = {"layer": "Populated places", "count": 1}
    v = check("world", C("buffer", layer="Populated places", distance="5 km"), sel=sel, request="Draw 5 km around the selected city.")
    assert v.ok and v.call["args"]["object"] == {"layer": "Populated places"}
    v = check("world", C("buffer", layer="Populated places", distance="5 km"), sel=sel, request="Draw 5 km around every city.")
    assert v.ok and "object" not in v.call["args"]
    v = check("world", C("filter", layer="Countries", conditions=[{"field": "pop_year", "op": "before", "value": "2019"}]))
    assert not v.ok and "below" in v.errors[0]
