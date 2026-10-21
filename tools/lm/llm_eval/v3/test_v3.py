"""Unit tests of harness v3 (no network). Tests that need the data on the LaCie drive or the built
``ask_eval`` example skip when those are missing."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import clients  # noqa: E402
import data_v3  # noqa: E402
import scoring_v3 as S  # noqa: E402

HAVE_DATA = (data_v3.FILES["storms"]).exists() and (data_v3.REF / "cities.shp").exists()
HAVE_BIN = Path(__import__("driver").DEFAULT_BIN).exists()


# ---------------------------------------------------------------------------------------------
# The benchmark file


def cases():
    return [json.loads(x) for x in (HERE / "cases_v3.jsonl").read_text(encoding="utf-8").splitlines() if x.strip()]


def test_cases_shape():
    cs = cases()
    assert 120 <= len(cs) <= 180
    ids = [c["id"] for c in cs]
    assert len(ids) == len(set(ids))
    zh = sum(1 for c in cs if c["lang"] == "zh") / len(cs)
    assert 0.2 <= zh <= 0.4
    assert len({c["lang"] for c in cs}) >= 5
    for c in cs:
        assert c["map"] in data_v3.MAPS
        assert c["gold"], c["id"]
        e = c["expect"]
        assert e["kind"] in ("plan", "ask_user", "cannot_do")
        if e["kind"] == "plan":
            assert e["answer"] is not None, c["id"]
    qtypes = {c["qtype"] for c in cs}
    for q in ("within", "per_area", "nearest", "cities_near", "inside", "join", "rank", "raster", "time", "chain", "honest"):
        assert q in qtypes
    assert sum(1 for c in cs if "zero" in c["tags"]) >= 4
    assert sum(1 for c in cs if "derived" in c["tags"]) >= 10


def test_gold_steps_use_app_layer_names():
    names = set(data_v3.LAYER.values()) | set(data_v3.BUILTIN.values())
    for c in cases():
        for st in c["gold"]:
            for k in ("layer", "points", "areas", "raster", "other"):
                v = st.get(k)
                if isinstance(v, str) and not v.startswith("#"):
                    assert v in names, (c["id"], k, v)


# ---------------------------------------------------------------------------------------------
# Keys


def test_fmt_value_matches_the_apps_display():
    f = data_v3.fmt_value
    assert f(135.0) == "135"
    assert f(20.842) == "20.842"
    assert f(1 / 3) == "0.333333"
    assert f(None) == "<Null>"
    assert f(True) == "True"
    assert f(7) == "7"
    assert f("Tokyo") == "Tokyo"


def test_key_spec_order():
    import pandas as pd

    df = pd.DataFrame({"name": ["A"], "country": ["X"], "latitude": [1.5], "geometry": [None]})
    assert data_v3.key_of(df, 0) == "A|X|1.5"
    df2 = pd.DataFrame({"storm_id": ["S1"], "name": ["N"]})
    assert data_v3.key_of(df2, 0) == "S1"
    df3 = pd.DataFrame({"foo": [1]})
    assert data_v3.key_of(df3, 0) == "row:0"


# ---------------------------------------------------------------------------------------------
# Scoring


def card(answer, highlight=None, row_keys=None):
    c = {"title": "t", "summary": "", "answer": answer, "context": [], "notices": []}
    if row_keys is not None:
        c["row_keys"] = row_keys
    return {"kind": "card", "card": c, "highlight": highlight}


def result(outcomes, preview=None, kind="plan", messages=()):
    return {"answer": {"kind": kind}, "outcomes": outcomes, "preview": preview or {}, "notes": [], "messages": list(messages)}


CASE = {
    "id": "w01", "qtype": "within", "lang": "en", "difficulty": "medium", "tags": [],
    "expect": {"kind": "plan", "accept": [], "area": True,
               "answer": {"type": "count", "lo": 7, "hi": 7},
               "preview": {"type": "set", "strict": list("abcdefg"), "edge": []}},
}


def test_count_and_preview_the_owner_bug():
    hl = {"layer": "storms", "rows": list(range(7)), "keys": list("abcdefg")}
    outs = [{"kind": "filtered", "shown": 228, "of": 4999, "final_keys": [str(i) for i in range(228)]},
            {"kind": "new_layer", "name": "Within 100 km of Tokyo", "color_by": None, "layer_rows": {"keys": ["row:0"], "shown": 1}},
            card({"Number": {"value": 7.0, "unit": "lines", "detail": ""}}, hl)]
    # Before the fix: the map highlights the filter's 228 tracks.
    before = {"layer": {"name": "Within 100 km of Tokyo", "keys": ["row:0"], "count": 1},
              "highlight": {"on_map": True, "count": 228, "keys": [str(i) for i in range(228)]}, "fit": None}
    s = S.score(CASE, result(outs, before))
    assert s["answer_ok"] and s["preview_ok"] is False and s["area_shown"] is True
    assert S.classify(CASE, s, None) == "preview-UI"
    after = dict(before, highlight={"on_map": True, "count": 7, "keys": list("abcdefg")})
    s = S.score(CASE, result(outs, after))
    assert s["answer_ok"] and s["preview_ok"] and s["e2e"]
    assert s["preview_strict"] is False  # nothing frames the view


def test_set_tolerance_and_builtin_highlight_is_not_visible():
    case = dict(CASE, expect=dict(CASE["expect"], answer={"type": "set", "strict": ["a"], "edge": ["b"]}, preview={"type": "set", "strict": ["a"], "edge": ["b"]}))
    outs = [card({"List": {"heading": "", "items": ["A"]}}, {"layer": "World cities", "rows": [0], "keys": ["a"]})]
    s = S.score(case, result(outs, {"highlight": {"on_map": False, "count": 1, "keys": ["a"]}}))
    assert s["answer_ok"] and s["preview_ok"] is False
    outs2 = [card({"List": {"heading": "", "items": ["A", "B"]}}, {"layer": "World cities", "rows": [0, 1], "keys": ["a", "b"]})]
    assert S.score(case, result(outs2, {"highlight": {"on_map": True, "count": 2, "keys": ["a", "b"]}}))["e2e"]
    outs3 = [card({"List": {"heading": "", "items": []}}, {"layer": "World cities", "rows": [], "keys": []})]
    assert not S.score(case, result(outs3))["answer_ok"]


def test_overlays_and_every_highlight_are_visible_context_is_not():
    # 2026-10-21: built-in answers are drawn as overlays (x01); an answer may light several layers;
    # the object measured from (nearest) is context and doesn't count as the answer.
    case = dict(CASE, expect=dict(CASE["expect"], answer={"type": "set", "strict": ["a"], "edge": []}, preview={"type": "set", "strict": ["a"], "edge": []}))
    outs = [card({"List": {"heading": "", "items": ["A"]}}, {"layer": "World cities", "rows": [0], "keys": ["a"]})]
    drawn = {"highlight": {"on_map": False, "count": 1, "keys": ["a"]}, "highlights": [{"on_map": False, "count": 1, "keys": ["a"]}],
             "overlays": [{"name": "World cities", "keys": ["a"], "count": 1}], "context": [{"on_map": True, "keys": ["z"]}]}
    assert S.score(case, result(outs, drawn))["preview_ok"]
    two = {"highlights": [{"on_map": True, "keys": ["a"]}, {"on_map": True, "keys": ["b"]}]}
    assert S.score(case, result(outs, two))["preview_ok"] is False


def test_nearest_preview_takes_any_near_tie():
    case = dict(CASE, qtype="nearest", expect=dict(CASE["expect"], area=False, answer={"type": "name", "accept": ["A", "B"]},
                                                    preview={"type": "set", "strict": ["a"], "edge": ["b"], "any": True}))
    nf = {"kind": "new_fields", "fields": ["nearest x"], "layer_rows": {"keys": ["src"], "shown": 1, "values": [{"nearest x": "B"}]}}
    lit = lambda keys: {"highlights": [{"on_map": True, "keys": keys}]}  # noqa: E731
    assert S.score(case, result([nf], lit(["b"])))["preview_ok"]
    assert S.score(case, result([nf], lit(["b", "c"])))["preview_ok"] is False
    assert S.score(case, result([nf], lit([])))["preview_ok"] is False


def test_top_name_value():
    rank = card({"Ranking": {"rows": [{"label": "Chile", "value": 51, "row": 2}, {"label": "Japan", "value": 40, "row": 9}], "unit": "n"}}, row_keys=["Chile|CHL", "Japan|JPN"])
    v = S.view(result([rank]))
    assert S.check_answer({"type": "top", "k": 1, "keys": ["Chile|CHL"]}, v)
    assert not S.check_answer({"type": "top", "k": 1, "keys": ["Japan|JPN"]}, v)
    assert S.check_answer({"type": "top", "k": 1, "keys": ["Chile|CHL", "Peru|PER"]}, v)  # a tie
    nf = {"kind": "new_fields", "fields": ["km to nearest volcano", "nearest volcano"],
          "layer_rows": {"keys": ["Tokyo|Japan"], "shown": 1, "values": [{"km to nearest volcano": 83.2, "nearest volcano": "Fujisan"}]}}
    v = S.view(result([nf]))
    assert S.check_answer({"type": "name", "accept": ["Fujisan"]}, v)
    assert S.check_answer({"type": "value", "value": 83.1, "tol": 1.0}, v)
    assert not S.check_answer({"type": "value", "value": 90, "tol": 1.0}, v)


def test_honesty():
    ask = {"id": "u05", "qtype": "honest", "lang": "en", "difficulty": "", "tags": ["vague"], "expect": {"kind": "ask_user", "accept": [], "answer": None, "preview": None}}
    assert S.score(ask, result([], kind="ask_user"))["e2e"]
    s = S.score(ask, result([{"kind": "filtered", "shown": 3}]))
    assert not s["honest"] and S.classify(ask, s, None) == "model plan"
    zero = dict(ask, expect={"kind": "cannot_do", "accept": ["zero"], "answer": None, "preview": None})
    assert S.score(zero, result([{"kind": "filtered", "shown": 0, "final_keys": []}]))["honest"]
    gap = dict(ask, tags=["tool_gap"], expect={"kind": "cannot_do", "accept": ["ask_user"], "answer": None, "preview": None})
    s = S.score(gap, result([{"kind": "filtered", "shown": 1}]))
    assert S.classify(gap, s, None) == "prompt-tool gap"


def test_classify_errors():
    s = S.score(CASE, {"answer": {"kind": "error", "text": "The plan didn't run: step 4: IBTrACS has no field “km to nearest buffer”"}, "messages": []})
    assert S.classify(CASE, s, {"executes": True}) == "prompt-tool gap"
    s = S.score(CASE, {"answer": {"kind": "error", "text": "The plan didn't run: step 2: no layer “Cities”"}, "messages": []})
    assert S.classify(CASE, s, {"executes": True}) == "model plan"
    assert S.classify(CASE, s, {"executes": False}) == "executor"
    s = S.score(CASE, {"answer": {"kind": "crash", "text": "exited"}})
    assert S.classify(CASE, s, None) == "executor"


def test_repairs_counted():
    r = result([card({"Number": {"value": 7.0}})], messages=[{"role": "tool", "content": "Plan not accepted. Fix these"}, {"role": "tool", "content": "Plan sent to the user for approval."}])
    s = S.score(CASE, r)
    assert s["repairs"] == 1 and s["plan_valid"] and not s["first_try"]


# ---------------------------------------------------------------------------------------------
# Ledger and clients


def test_ledger_cap(tmp_path):
    led = clients.Ledger(tmp_path / "spend.json", cap=0.10)
    assert led.reserve(0.06)
    assert not led.reserve(0.06)  # would pass the cap
    led.settle(0.06, 0.05, "run1")
    d = json.loads((tmp_path / "spend.json").read_text())
    assert d["spent_usd"] == 0.05 and d["runs"]["run1"] == 0.05 and d["cap_usd"] == 0.10
    led2 = clients.Ledger(tmp_path / "spend.json", cap=0.10)
    assert led2.spent == 0.05
    assert not led2.reserve(0.051)


def test_openrouter_client_refuses_before_sending(tmp_path):
    led = clients.Ledger(tmp_path / "spend.json", cap=0.0001)
    cat = {"m": {"pricing": {"prompt": "0.000001", "completion": "0.000005"}}}
    c = clients.OpenRouterClient("sk-test-not-a-key", led, cat, "t")
    assert "sk-" not in repr(c)
    with pytest.raises(clients.BudgetExceeded):
        c.complete({"model": "m", "messages": [{"role": "user", "content": "x" * 3000}], "max_tokens": 2500})
    assert not (tmp_path / "spend.json").exists() or json.loads((tmp_path / "spend.json").read_text())["spent_usd"] == 0


def test_plan_client_reply():
    pc = clients.PlanClient({"a": [{"action": "cannot_do", "reason": "x"}], "b": None})
    pc.start("a")
    reply, err, _ = pc.complete({})
    call = reply["choices"][0]["message"]["tool_calls"][0]
    assert err is None and call["function"]["name"] == "submit_plan"
    assert json.loads(call["function"]["arguments"])["steps"][0]["action"] == "cannot_do"
    pc.start("b")
    reply, _, _ = pc.complete({})
    assert reply["choices"][0]["message"]["content"]


def test_results_hold_no_key_or_headers():
    import run_v3

    for f in [*(HERE / "results").glob("**/*.jsonl"), *run_v3.RAW.glob("**/*.jsonl")]:
        if f.name.startswith("._"):  # exFAT AppleDouble files
            continue
        t = f.read_text(encoding="utf-8")
        assert "Authorization" not in t and "Bearer " not in t and "sk-or-" not in t, f


# ---------------------------------------------------------------------------------------------
# Ground truth helpers (need the env's geospatial stack)


def test_geodesic_helpers():
    gt = pytest.importorskip("gt")
    pytest.importorskip("pyproj")
    import geopandas as gpd
    import numpy as np
    from shapely.geometry import LineString, Point

    d = gt.geod_km(139.69, 35.69, np.array([139.64]), np.array([35.44]))[0]
    assert 27 < d < 30
    # A meridian segment 1 degree east of the place: about 111 * cos(lat) km at the closest point.
    g = gpd.GeoDataFrame(geometry=[LineString([(1, -5), (1, 5)]), Point(0, 3)], crs=4326)
    dist = gt.distances_km(0, 0, g)
    assert abs(dist[0] - 111.32) < 0.5 and abs(dist[1] - 331.7) < 1.0
    w = gt.within(0, 0, g, 112)
    assert w["edge"] == [0] and w["strict"] == []


@pytest.mark.skipif(not HAVE_DATA, reason="eval data not on this machine")
def test_owner_case_ground_truth():
    gt = pytest.importorskip("gt")
    s = gt.load("storms")
    _, x, y = gt.city("Tokyo")
    w = gt.within(x, y, s, 100, (s["strength"] == "Category 5").to_numpy())
    assert len(w["strict"]) == 7 and not w["edge"]


@pytest.mark.skipif(not (HAVE_DATA and HAVE_BIN), reason="needs the eval data and the built ask_eval example")
def test_dry_run_one_case_through_the_app():
    import driver

    c = next(x for x in cases() if x["id"] == "w01")
    app = driver.AskEval()
    try:
        r = app.run({"id": "w01", "files": data_v3.files("storms"), "text": c["text"], "keys": data_v3.KEYS}, clients.PlanClient({"w01": c["gold"]}))
    finally:
        app.close()
    s = S.score(c, r)
    assert s["executes"] and s["answer_ok"], s


def test_drop_rule19_removes_only_that_rule():
    text = "18. Per capita.\n19. A field an earlier step made can be named by that step: \"#k\".\n\nMore example layers"
    body = {"messages": [{"role": "system", "content": [{"type": "text", "text": text}, {"type": "text", "text": "map"}]}]}
    out = clients.drop_rule19(body)
    t = out["messages"][0]["content"][0]["text"]
    assert "19." not in t and "18. Per capita." in t and "More example layers" in t
    assert body["messages"][0]["content"][0]["text"] == text
    app = (HERE.parents[3] / "crates/lm-ask/assets/system.txt")
    if app.exists():
        assert clients.RULE19_START in app.read_text(encoding="utf-8")
