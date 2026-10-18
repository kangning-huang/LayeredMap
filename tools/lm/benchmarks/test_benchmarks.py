"""Tests for the benchmark suite: contexts, the plan validator, the suite build, and that the
eval's loader, scorer and runner accept the suite.

    ~/miniforge3/envs/lm-llm-eval/bin/python -I -m pytest tools/lm/benchmarks/test_benchmarks.py -q
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import benchlib as b  # noqa: E402
import build_suite  # noqa: E402
import validate  # noqa: E402

b.install_contexts()

import run_eval  # noqa: E402
import scoring  # noqa: E402

SUITE = HERE / "suite.jsonl"


def suite_cases() -> list[dict]:
    return run_eval.load_cases(SUITE)


# ---------------------------------------------------------------------------------------------
# Contexts


def test_system_message_matches_the_eval_for_its_contexts():
    for ctx in b.ctxmod.CONTEXTS:
        for selection in (None, {"layer": b.ctxmod.CONTEXTS[ctx][0], "count": 1}):
            for wv in (False, True):
                assert b.system_message(ctx, selection, wv) == b.ctxmod.system_message(ctx, selection, wv)


def test_our_contexts_are_value_free_and_well_formed():
    for p in sorted(b.CONTEXT_DIR.glob("*.json")):
        if p.name.startswith("._"):
            continue
        d = json.loads(p.read_text(encoding="utf-8"))
        for L in d["layers"]:
            assert L["kind"] in {"points", "lines", "areas", "raster", "drawn area"}, (p.name, L["name"])
            if L["kind"] == "raster":
                assert L.get("desc")
            else:
                assert isinstance(L["count"], int)
                for f in L["fields"]:
                    assert len(f) == 4 and isinstance(f[0], str) and isinstance(f[1], str)
        msg = b.system_message(d["name"])
        assert "Open layers:" in msg


def test_installed_contexts_reach_run_eval():
    msg = run_eval.ctxmod.system_message("usa")
    assert "Counties: areas" in msg and msg.startswith(b.ctxmod.INSTRUCTION)


# ---------------------------------------------------------------------------------------------
# The validator


def test_the_eval_cases_pass_the_validator():
    for line in (b.LLM_EVAL / "cases.jsonl").read_text(encoding="utf-8").splitlines():
        c = json.loads(line)
        assert b.validate_case({k: c[k] for k in b.CASE_KEYS}) == [], c["id"]


def _errors(plan, ctx="shanghai"):
    return b.validate_plan(plan, ctx)


def test_validator_catches_bad_plans():
    call, sel, dist, cond = b.call, b.sel, b.dist, b.cond
    assert _errors([call("count_in", points="Schools", areas="Districts")]) == []
    assert any("unknown action" in e for e in _errors([call("clip", layer="Schools")]))
    assert any("unknown argument" in e for e in _errors([call("count_in", points="Schools", areas="Districts", radius=3)]))
    assert any("no layer" in e for e in _errors([call("count_in", points="Hospitals", areas="Districts")]))
    assert any("needs points" in e for e in _errors([call("count_in", points="Districts", areas="Districts")]))
    assert any("missing required" in e for e in _errors([call("buffer", layer="Schools")]))
    assert any("needs lines or points" in e for e in _errors([call("buffer", layer="Districts", distance=dist(100))]))
    assert any("field" in e for e in _errors([call("rank_by_value", layer="Districts", field="students")]))
    assert any("earlier step" in e for e in _errors([call("whats_inside", area=sel("#2"), layer="Schools")]))
    assert any("makes no new layer" in e for e in _errors([call("filter", layer="Schools", conditions=[]), call("whats_inside", area=sel("#1"), layer="Schools")]))
    assert any("stat" in e for e in _errors([call("summarize_each", raster="Surface temperature", areas="Districts", stat="mode")]))
    assert any("needs a field" in e for e in _errors([call("count_in", points="Schools", areas="Districts", stat="sum")]))
    assert any("unknown op" in e for e in _errors([call("filter", layer="Schools", conditions=[cond("students", "greater", 5)])]))
    assert any("only call" in e for e in _errors(b.CANNOT + b.ASK))


def test_validator_follows_fields_made_by_earlier_steps():
    call, sel, cond = b.call, b.sel, b.cond
    plan = [
        call("count_in", points="Schools", areas="Districts", stat="sum", field="students"),
        call("rank_by_value", layer=["#1", "Sum students of schools in each district"], field="sum students"),
    ]
    assert _errors(plan) == []
    plan = [call("add_raster_values", raster="Surface temperature", points="Schools"), call("filter", layer="Schools", conditions=[cond("Surface temperature", "above", 30)])]
    assert _errors(plan) == []
    plan = [call("summarize_each", raster="Surface temperature", areas="Districts"), call("rank_by_value", layer="#1", field="Surface temperature mean")]
    assert _errors(plan) == []
    plan = [call("buffer", layer="Roads", distance=b.dist(500)), call("count_here", points="Schools", area=sel("#1"))]
    assert _errors(plan) == []


def test_record_rules():
    rec = {
        "source": "X", "source_id": "1", "licence": "MIT", "text_policy": "adapted", "summary": "clip roads to a park",
        "category": "overlay", "coverage": "upstream", "missing": ["clip"], "cases": [],
    }
    assert b.validate_record(rec) == []
    assert b.validate_record({**rec, "missing": ["kriging"]})  # kriging is new, not upstream
    assert b.validate_record({**rec, "missing": ["no_such_op"]})
    assert b.validate_record({**rec, "coverage": "out_of_scope", "missing": [], "cases": []})


# ---------------------------------------------------------------------------------------------
# Sources and the suite


def test_sources_validate():
    assert validate.validate_sources() == []


def test_suite_is_built_from_the_sources():
    cases, _, _ = build_suite.build(40)
    on_disk = suite_cases()
    assert [c["id"] for c in on_disk] == [c["id"] for c in cases], "run build_suite.py"
    assert on_disk == json.loads(json.dumps(cases, ensure_ascii=False)), "run build_suite.py"
    assert validate.validate_suite(SUITE) == []


def test_the_scorer_accepts_every_expected_plan():
    """A model that makes exactly the expected calls scores strict, through the eval's own parser."""
    for c in suite_cases():
        for plan in [c["expected"]] + c["alts"]:
            msg = {"tool_calls": [{"function": {"name": s["name"], "arguments": json.dumps(run_eval.mock_args(s, False), ensure_ascii=False)}} for s in plan]}
            calls, text = scoring.parse_tool_calls(msg)
            assert scoring.score(c, calls, text)["strict"], c["id"]


def test_dry_run_through_run_eval(tmp_path):
    import run_suite

    ids = []
    seen_requests = set()
    for c in suite_cases():
        if c["request"] not in seen_requests:  # the mock finds a case by its request text
            ids.append(c["id"])
            seen_requests.add(c["request"])
        if len(ids) == 25:
            break
    rc = run_suite.main(["--dry-run", "--models", "anthropic/claude-haiku-5.5", "--only", ",".join(ids), "--out", str(tmp_path)])
    assert rc == 0
    out = next(tmp_path.rglob("*.jsonl"))
    recs = [json.loads(line) for line in out.read_text(encoding="utf-8").splitlines()]
    assert len(recs) == len(ids)
    assert all(r["strict"] for r in recs), [r["case"] for r in recs if not r["strict"]]
