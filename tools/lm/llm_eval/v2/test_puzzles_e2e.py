"""Tests of the puzzle harness (tools/lm/puzzles/pro_e2e.py) that need neither a model nor the
data folder (those parts skip when the data is missing). ``register()`` is never called here: it
replaces the world context's layers for the whole process."""

import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "tools/lm/puzzles"))

import pro_e2e  # noqa: E402

PUZZLES = __import__("json").loads(pro_e2e.PUZZLES_JSON.read_text(encoding="utf-8"))["puzzles"]


def test_examples_are_not_close_to_any_puzzle_request():
    worst = pro_e2e.leak_report(PUZZLES)[0]
    assert worst[0] < 0.35, worst


def test_every_puzzle_has_a_gold_plan_on_its_own_layers():
    for p in PUZZLES:
        spec = __import__("json").loads((pro_e2e.PLANS / f"{p['id']}.json").read_text(encoding="utf-8"))
        assert {l["name"] for l in spec["layers"]} <= set(pro_e2e.puzzle_layers(p)), p["id"]


def _out(objs, steps=None):
    return {"ok": True, "steps": steps or [{"filtered": "Populated places", "shown": len(objs), "of": 9}],
            "layers": [{"name": "Populated places", "shown": len(objs), "objects": objs}]}


def test_answers_are_checked_by_name_or_distance_and_cards():
    p1 = PUZZLES[0]
    kunming = {"title": "Kunming", "lon": 102.697, "lat": 25.043, "attrs": {"NAME": "Kunming"}}
    lanzhou = {"title": "Lanzhou", "lon": 103.8, "lat": 36.06, "attrs": {}}
    assert pro_e2e.check_answer(p1, _out([kunming]))["outcome"] == "solved"
    assert pro_e2e.check_answer(p1, _out([kunming, lanzhou]))["outcome"] == "ambiguous"
    assert pro_e2e.check_answer(p1, _out([lanzhou]))["outcome"] == "wrong"
    assert pro_e2e.check_answer(p1, _out([]))["outcome"] == "empty"
    assert pro_e2e.check_answer(p1, {"ok": False, "error": {"step": 2, "message": "x"}})["outcome"] == "exec_error"
    p7 = next(p for p in PUZZLES if p["id"] == "P7")
    foz = {"title": "Foz do Iguaçu", "lon": -54.53, "lat": -25.523, "attrs": {}}
    guaira = {"title": "Guaira", "lon": -54.27, "lat": -24.09, "attrs": {}}
    steps = [{"card": "c", "summary": "2 countries inside Guaira"}, {"card": "c", "summary": "3 countries inside Foz do Iguaçu"},
             {"filtered": "Populated places", "shown": 2, "of": 9}]
    assert pro_e2e.check_answer(p7, _out([guaira, foz], steps))["outcome"] == "solved_with_cards"


@pytest.mark.skipif(not pro_e2e.LAYERS["Populated places"]["path"].exists(), reason="puzzle data not mounted")
def test_dbf_reader_reads_natural_earth():
    fields, rows = pro_e2e.read_dbf(pro_e2e.LAYERS["Populated places"]["path"].with_suffix(".dbf"))
    assert len(rows) == 7342 and ("POP_MAX", "N", 12, 0) in fields
    k = next(r for r in rows if r["NAME"] == "Kunming")
    assert k["ADM0NAME"] == "China" and k["POP_MAX"] == "2931000"
