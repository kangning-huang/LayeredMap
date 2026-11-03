"""Unit tests: scoring, tool generation, the runner with a mocked client.

    python -I -m pytest tools/lm/llm_eval/test_scoring.py -q
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import context as ctxmod  # noqa: E402
import run_eval  # noqa: E402
import scoring  # noqa: E402
import tools as toolmod  # noqa: E402

CASES = {c["id"]: c for c in run_eval.load_cases(HERE / "cases.jsonl")}


def tc(name, **args):
    return {"function": {"name": name, "arguments": json.dumps(args, ensure_ascii=False)}}


def score(case_id, *tool_calls, text=""):
    calls, _ = scoring.parse_tool_calls({"tool_calls": list(tool_calls)})
    return scoring.score(CASES[case_id], calls, text)


# ---------------------------------------------------------------------------------------------
# Tools stay in sync with the app


def test_connected_actions_come_from_the_runner():
    ids = toolmod.connected_actions()
    assert "count_in" in ids and "rank_by_value" in ids
    assert "measure_from_here" not in ids  # in rules.toml, not connected
    names = [t["function"]["name"] for t in toolmod.tools_for("per_action")]
    assert names[: len(ids)] == ids
    assert {"filter", "color_by", "view_data", "zoom_to", "ask_user", "cannot_do"} <= set(names)


def test_runner_inputs_table_parses_every_action():
    table = toolmod.action_table()
    roles = {a["id"]: a["roles"] for a in table}
    assert roles["count_in"] == [("points", "layer"), ("areas", "layer")]
    assert roles["which_container"] == [("point", "object"), ("areas", "layer")]
    assert roles["add_raster_values"] == [("raster", "layer"), ("points", "layer")]
    assert roles["buffer"] == [("layer", "layer"), ("object", "object?")]
    assert roles["measure_length"] == [("line", "object")]


def test_output_notes_match_the_runner():
    src = toolmod.RUNNER.read_text(encoding="utf-8")
    # Updated after the statistics menus (commit c56d0f5) changed these lines in the runner.
    assert 'format!("{} in {each}", capitalized(&things))' in src
    assert "let count_field = free_field(&out, &things);" in src
    assert 'free_field(&out, &format!("{rname} {words}"))' in src and '("mean", "Mean"), ("median", "Median")' in src
    assert "let field = free_field(&out, &rname);" in src


def test_single_design_maps_to_canonical_calls():
    c = toolmod.canonical_call("run_action", {"action": "count_in", "layers": ["Schools", "Districts"]})
    assert c == {"name": "count_in", "args": {"points": "Schools", "areas": "Districts"}}
    c = toolmod.canonical_call("run_action", {"action": "whats_inside", "layers": ["Schools"], "object": {"layer": "Districts"}})
    assert c["args"] == {"area": {"layer": "Districts"}, "layer": "Schools"}
    # A drawn area given as a layer name fills the object role.
    c = toolmod.canonical_call("run_action", {"action": "count_here", "layers": ["Schools", "Area 1"]})
    assert c["args"] == {"points": "Schools", "area": {"layer": "Area 1"}}
    c = toolmod.canonical_call("run_action", {"action": "buffer", "layers": ["Roads"], "distance": "1 km", "merge": False})
    assert c["args"] == {"layer": "Roads", "distance": "1 km", "merge": False}


def test_context_is_value_free_by_default():
    msg = ctxmod.system_message("world")
    assert "Africa" not in msg and "Primary" not in msg
    assert "gdp_md (integer, million US$)" in msg
    assert "Africa" in ctxmod.system_message("world", with_values=True)
    assert "Selected: 1 object of Schools" in ctxmod.system_message("shanghai", {"layer": "Schools", "count": 1})


# ---------------------------------------------------------------------------------------------
# Distances


def test_parse_distance_app_matches_the_runner():
    assert scoring.parse_distance_app("1 km") == 1000
    assert scoring.parse_distance_app("500") == 500
    assert abs(scoring.parse_distance_app("2 mi") - 3218.688) < 1e-9
    assert scoring.parse_distance_app("1,500 m") == 1500
    assert scoring.parse_distance_app("500米") is None
    assert scoring.parse_distance_app("0 m") is None
    assert scoring.parse_distance_app("five km") is None


def test_parse_distance_lenient_reads_chinese_units():
    assert scoring.parse_distance_lenient("500米") == 500
    assert scoring.parse_distance_lenient("1公里") == 1000
    assert scoring.parse_distance_lenient(500) == 500
    assert scoring.parse_distance_lenient({"value": 2, "unit": "km"}) == 2000


# ---------------------------------------------------------------------------------------------
# Strict and lenient


def test_exact_plan_is_strict():
    r = score("a01", tc("count_in", points="Schools", areas="Districts"))
    assert r == {"strict": True, "lenient": True, "reason": None}


def test_layer_case_only_lenient():
    r = score("a01", tc("count_in", points="schools", areas="districts"))
    assert not r["strict"] and r["lenient"] and r["reason"] == "wrong_layer"


def test_swapped_inputs_fail():
    r = score("a01", tc("count_in", points="Districts", areas="Schools"))
    assert not r["strict"] and not r["lenient"]


def test_unit_conversion_within_tolerance():
    assert score("a11", tc("buffer", layer="Populated places", object={"layer": "Populated places", "field": "name", "value": "Paris"}, distance="100 mi"))["strict"]
    assert score("a11", tc("buffer", layer="Populated places", object={"layer": "Populated places", "field": "name", "value": "Paris"}, distance="160.9 km"))["strict"]
    r = score("a11", tc("buffer", layer="Populated places", object={"layer": "Populated places", "field": "name", "value": "Paris"}, distance="100 km"))
    assert not r["lenient"] and r["reason"] == "wrong_distance"


def test_chinese_unit_is_lenient_only():
    r = score("a12", tc("buffer", layer="Schools", distance="500米"))
    assert not r["strict"] and r["lenient"]


def test_buffering_one_object_instead_of_the_layer_fails():
    r = score("a09", tc("buffer", layer="Schools", object={"layer": "Schools", "field": "name", "value": "X"}, distance="500 m"))
    assert not r["lenient"] and r["reason"] == "wrong_object"


def test_object_pairs():
    ok = tc("measure_area", area={"layer": "Countries", "field": "name_zh", "value": "法国"})
    assert score("z01", ok)["strict"]
    wrong_field = tc("measure_area", area={"layer": "Countries", "field": "name_zh", "value": "France"})
    r = score("z01", wrong_field)
    assert not r["strict"] and r["lenient"]
    alias = tc("measure_length", line={"layer": "Roads", "field": "name", "value": "延安路"})
    r = score("a16", alias)
    assert not r["strict"] and r["lenient"]


def test_selected_object():
    assert score("a13", tc("measure_area", area={"layer": "Districts"}))["strict"]
    r = score("a13", tc("measure_area", area={"layer": "Districts", "field": "name_en", "value": "Jing'an"}))
    assert not r["strict"] and r["lenient"]


def test_filter_equivalences():
    assert score("f01", tc("filter", layer="Countries", conditions=[{"field": "continent", "op": "is", "value": "Africa"}]))["strict"]
    assert score("f01", tc("filter", layer="Countries", conditions=[{"field": "continent", "op": "any_of", "values": ["africa"]}]))["strict"]
    r = score("f02", tc("filter", layer="Populated places", conditions=[{"field": "population", "op": ">", "value": "5,000,000"}]))
    assert not r["strict"] and r["lenient"]
    r = score("f06", tc("filter", layer="Districts", conditions=[{"field": "population", "op": "above", "value": "1000000"}, {"field": "population", "op": "below", "value": "2000000"}]))
    assert not r["strict"] and r["lenient"]
    assert score("f06", tc("filter", layer="Districts", conditions=[{"field": "population", "op": "between", "value": "2000000", "value2": "1000000"}]))["strict"]
    assert score("f13", tc("filter", layer="Countries", conditions=[]))["strict"]
    assert score("f03", tc("filter", layer="Populated places", conditions=[{"field": "capital", "op": "is", "value": "yes"}]))["strict"]
    r = score("f12", tc("filter", layer="Countries", conditions=[{"field": "continent", "op": "is", "value": "亚洲"}]))
    assert not r["lenient"] and r["reason"] == "wrong_filter"


def test_two_filters_merge_leniently():
    r = score(
        "f06",
        tc("filter", layer="Districts", conditions=[{"field": "population", "op": "above", "value": "1000000"}]),
        tc("filter", layer="Districts", conditions=[{"field": "population", "op": "below", "value": "2000000"}]),
    )
    assert not r["strict"] and r["lenient"]


def test_feet_tolerance():
    base = tc("add_raster_values", raster="Elevation", points="Populated places")
    assert score("u03", base, tc("filter", layer="Populated places", conditions=[{"field": "Elevation", "op": "above", "value": "1524"}]))["strict"]
    assert score("u03", base, tc("filter", layer="Populated places", conditions=[{"field": "Elevation", "op": "above", "value": "1520"}]))["strict"]
    assert not score("u03", base, tc("filter", layer="Populated places", conditions=[{"field": "Elevation", "op": "above", "value": "5000"}]))["lenient"]


def test_multi_step_and_result_refs():
    a = tc("count_in", points="Schools", areas="Districts")
    assert score("m02", a, tc("rank_by_value", layer="#1", field="schools"))["strict"]
    assert score("m02", a, tc("rank_by_value", layer="Schools in each district", field="schools"))["strict"]
    r = score("m02", a, tc("rank_by_value", layer="#2", field="Schools"))
    assert not r["strict"] and r["lenient"]
    r = score("m02", a)
    assert not r["lenient"] and r["reason"] == "missing_steps"


def test_order_alternatives():
    f = tc("filter", layer="Populated places", conditions=[{"field": "capital", "op": "is", "value": "true"}])
    add = tc("add_raster_values", raster="Elevation", points="Populated places")
    rank = tc("rank_by_value", layer="Populated places", field="Elevation")
    assert score("m04", add, f, rank)["strict"]
    assert score("m04", f, add, rank)["strict"]
    assert not score("m04", f, rank, add)["lenient"]


def test_ask_and_refuse():
    assert score("v01", tc("ask_user", question="What about them?", choices=["a", "b"]))["strict"]
    r = score("v01", tc("cannot_do", reason="no"))
    assert not r["lenient"] and r["reason"] == "refused_instead_of_asking"
    r = score("x02", tc("ask_user", question="?"))
    assert not r["lenient"] and r["reason"] == "asked_instead_of_refusing"
    r = score("x02", text="Sorry, NimbleMap can't forecast population.")
    assert not r["strict"] and r["lenient"] and r["reason"] == "no_call"
    r = score("v01", text="Which analysis would you like?")
    assert not r["strict"] and r["lenient"]
    r = score("a01", tc("cannot_do", reason="no"))
    assert r["reason"] == "refused_answerable"
    # Strict alternatives in trap cases: asking or refusing are both right.
    assert score("t02", tc("cannot_do", reason="continent is on Countries"))["strict"]


def test_extra_display_calls_are_lenient():
    r = score("a01", tc("count_in", points="Schools", areas="Districts"), tc("zoom_to", layer="Districts"))
    assert not r["strict"] and r["lenient"] and r["reason"] == "extra_steps"


def test_stringified_json_is_lenient():
    r = score("a13", tc("measure_area", area=json.dumps({"layer": "Districts"})))
    assert not r["strict"] and r["lenient"]


def test_lenient_alt_plan():
    r = score("v03", tc("buffer", layer="Schools", distance="1 km"))
    assert not r["strict"] and r["lenient"]


def test_single_design_end_to_end():
    r = score("a03", tc("run_action", action="count_here", layers=["Schools"], object={"layer": "Area 1"}))
    assert r["strict"]
    r = score("m07", tc("run_action", action="buffer", layers=["Roads"], object={"layer": "Roads", "field": "name", "value": "Century Ave"}, distance="2 km"),
              tc("run_action", action="whats_inside", layers=["Schools"], object={"layer": "#1"}))
    assert r["strict"]


def test_bad_json_arguments_dont_crash():
    calls, _ = scoring.parse_tool_calls({"tool_calls": [{"function": {"name": "count_in", "arguments": "{oops"}}]})
    assert calls[0]["args"] == {"_unparsed": "{oops"}
    assert not scoring.score(CASES["a01"], calls)["lenient"]


# ---------------------------------------------------------------------------------------------
# Runner pieces (no network)


def test_reasoning_settings_pick_the_lowest():
    cat = run_eval.load_catalog()
    assert run_eval.reasoning_setting(cat["openai/gpt-oss-20b"]) == {"effort": "low"}
    assert run_eval.reasoning_setting(cat["openai/gpt-5-nano"]) == {"effort": "minimal"}
    assert run_eval.reasoning_setting(cat["deepseek/deepseek-v4-flash"]) == {"enabled": False}
    assert run_eval.reasoning_setting(cat["qwen/qwen3.7-flash"]) == {"enabled": False}
    assert run_eval.reasoning_setting(cat["anthropic/claude-sonnet-5.5"]) == {"effort": "low"}
    assert run_eval.reasoning_setting(cat["meta-llama/llama-3.3-70b-instruct"]) is None
    body = run_eval.request_body(cat["openai/gpt-5-mini"], [], [])
    assert "temperature" not in body and body["usage"] == {"include": True}
    assert run_eval.request_body(cat["openai/gpt-oss-20b"], [], [])["temperature"] == 0


def test_every_shortlisted_model_is_in_the_catalog_with_tools():
    cat = run_eval.load_catalog()
    for m in run_eval.SHORTLIST:
        assert "tools" in cat[m]["supported_parameters"], m


def test_budget_stops(tmp_path):
    b = run_eval.Budget(tmp_path / "spend.json", 0.01)
    assert b.reserve(0.004)
    b.settle(0.004, 0.006)
    assert b.reserve(0.003)
    b.settle(0.003, 0.003)
    assert not b.reserve(0.002)  # 0.009 + 0.002 > 0.01
    assert not b.reserve(0.0)  # stopped for good
    assert json.loads((tmp_path / "spend.json").read_text())["spent_usd"] == 0.009


class FlakyClient:
    def __init__(self, inner, fails):
        self.inner, self.fails, self.calls = inner, fails, 0

    def complete(self, body):
        self.calls += 1
        if self.calls <= self.fails:
            return 429, {"error": {"code": 429, "message": "rate limited"}}
        return self.inner.complete(body)


def test_retry_then_score(tmp_path, monkeypatch):
    monkeypatch.setattr(run_eval.time, "sleep", lambda s: None)
    cat = run_eval.load_catalog()
    client = FlakyClient(run_eval.MockClient(CASES), fails=2)
    b = run_eval.Budget(tmp_path / "s.json", 1.0)
    rec = run_eval.run_case(client, cat["openai/gpt-oss-20b"], CASES["m02"], toolmod.tools_for("per_action"), False, b)
    assert rec["tries"] == 4 and rec["turns"] == 2 and rec["strict"] and rec["lenient"]
    rec = run_eval.run_case(run_eval.MockClient(CASES), cat["openai/gpt-oss-20b"], CASES["m07"], toolmod.tools_for("single"), False, b)
    assert rec["strict"], rec


def test_mock_perfect_client_scores_every_case(tmp_path, monkeypatch):
    cat = run_eval.load_catalog()
    b = run_eval.Budget(tmp_path / "s.json", 1.0)
    for design in toolmod.DESIGNS:
        tools = toolmod.tools_for(design)
        for c in CASES.values():
            rec = run_eval.run_case(run_eval.MockClient(CASES), cat["openai/gpt-oss-20b"], c, tools, False, b)
            assert rec["strict"], (design, c["id"], rec)


def test_cases_cover_the_brief():
    cs = list(CASES.values())
    assert len(cs) >= 50
    assert sum(c["lang"] == "zh" for c in cs) >= 10
    expected_names = {call["name"] for c in cs for call in c["expected"]}
    # The v1 benchmark is frozen; actions connected after it (2026-10-17; match_table 2026-10-18) are covered by v2 and
    # tools/lm/benchmarks instead (summarize_over_time 2026-10-28: the lm-ask live test and v2).
    added_later = {"calculate_field", "split_parts", "find_where", "keep_inside", "remove_inside", "nearest_distance", "nearest_other", "join_by_location", "add_measurements", "match_table", "summarize_over_time"}
    assert set(toolmod.connected_actions()) - added_later <= expected_names
    for cat in ["multi", "vague", "impossible", "trap", "units", "filter", "style"]:
        assert any(cat in c["categories"] for c in cs), cat


def test_gemini_namespace_is_lenient():
    r = score("a01", tc("default_api.count_in", points="Schools", areas="Districts"))
    assert not r["strict"] and r["lenient"]


class SequentialClient:
    """Answers one call per turn, as many models do, then a short text."""

    def __init__(self, plan):
        self.plan, self.bodies = plan, []

    def complete(self, body):
        self.bodies.append(body)
        k = sum(1 for m in body["messages"] if m["role"] == "tool")
        usage = {"prompt_tokens": 100, "completion_tokens": 10, "cost": 0.001}
        if k >= len(self.plan):
            return 200, {"choices": [{"message": {"role": "assistant", "content": "Done."}, "finish_reason": "stop"}], "usage": usage}
        name, args = self.plan[k]
        msg = {"role": "assistant", "content": None, "tool_calls": [{"id": f"x{k}", "type": "function", "function": {"name": name, "arguments": json.dumps(args)}}]}
        return 200, {"choices": [{"message": msg, "finish_reason": "tool_calls"}], "usage": usage}


def test_sequential_calls_are_collected_over_turns(tmp_path):
    cat = run_eval.load_catalog()
    client = SequentialClient([("count_in", {"points": "Schools", "areas": "Districts"}), ("rank_by_value", {"layer": "#1", "field": "schools"})])
    b = run_eval.Budget(tmp_path / "s.json", 1.0)
    rec = run_eval.run_case(client, cat["openai/gpt-oss-20b"], CASES["m02"], toolmod.tools_for("per_action"), False, b)
    assert rec["turns"] == 3 and rec["strict"], rec
    assert abs(rec["cost_usd"] - 0.003) < 1e-12 and rec["prompt_tokens"] == 300
    tool_msgs = [m for m in client.bodies[-1]["messages"] if m["role"] == "tool"]
    assert '"#1"' in tool_msgs[0]["content"] and "#2" in tool_msgs[1]["content"]


def test_ask_ends_the_turns(tmp_path):
    cat = run_eval.load_catalog()
    client = SequentialClient([("ask_user", {"question": "Which?"}), ("count_in", {})])
    rec = run_eval.run_case(client, cat["openai/gpt-oss-20b"], CASES["v01"], toolmod.tools_for("per_action"), False, run_eval.Budget(tmp_path / "s.json", 1.0))
    assert rec["turns"] == 1 and rec["strict"]


def test_repeated_call_is_lenient():
    a = tc("measure_length", line={"layer": "Roads", "field": "name", "value": "Century Ave"})
    r = score("a15", a, a)
    assert not r["strict"] and r["lenient"] and r["reason"] == "extra_steps"


class MalformedClient:
    def complete(self, body):
        return 200, {"choices": [{"message": {"role": "assistant", "content": None}, "finish_reason": "error", "native_finish_reason": "MALFORMED_FUNCTION_CALL"}], "usage": {}}


def test_malformed_call_is_scored_not_retried(tmp_path):
    cat = run_eval.load_catalog()
    rec = run_eval.run_case(MalformedClient(), cat["google/gemini-2.5-flash-lite"], CASES["a01"], toolmod.tools_for("per_action"), False, run_eval.Budget(tmp_path / "s.json", 1.0))
    assert rec["tries"] == 1 and rec["strict"] is False and rec["reason"] == "malformed_call" and "error" not in rec


def test_refs_to_changed_layers_are_lenient():
    add = tc("add_raster_values", raster="Elevation", points="Populated places")
    r = score("u03", add, tc("filter", layer="#1", conditions=[{"field": "Elevation", "op": "above", "value": "1524"}]))
    assert not r["strict"] and r["lenient"] and r["reason"] == "wrong_layer"
