"""Scoring of harness v3: what the app answered and showed, against the ground truth.

Per case (``score``):
- **honest**: a plan when one is possible; ``ask_user`` / ``cannot_do`` when the case calls for it
  (each case lists the alternatives it accepts);
- **plan valid**: the app's checks accepted a plan (``repairs`` counts the rounds it took; a plan
  that still carried check notes after the last round is not valid);
- **executes**: the plan ran without an error;
- **answer correct**: the answer the user reads (the last step's card, filter, new layer or new
  fields) matches the spec: a count range, a set of objects (every strict one, nothing beyond the
  edge ones), a value within tolerance, the top-k of a ranking, or a name;
- **preview correct**: the objects the map highlights (on a layer that is on the map) plus the
  objects of the layer drawn over it are exactly the expected ones (edge objects optional); for
  per-area answers, the drawn layer is the counted/summarised one. Reported apart: whether a
  "within N km" area is drawn (``area_shown``) and whether the view is asked to frame the result
  (``frames``); ``preview_strict`` needs all three.

Each failure gets one class, from the first layer that failed (``classify``): model plan,
checker, executor, preview-UI, prompt-tool gap, ground-truth ambiguity. The gold plan's own run
(mock mode) separates the app's faults from the model's: when the gold plan fails the same way,
the app is at fault.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent

CLASSES = ["model plan", "checker", "executor", "preview-UI", "prompt-tool gap", "ground-truth ambiguity"]

# Cases whose ground truth rests on a rule the app may reasonably apply differently (documented in
# the case notes); a wrong answer there that the gold plan shares is "ground-truth ambiguity".
GT_AMBIGUOUS_TAGS = {"gt_rule"}


def load_cases(path: Path | None = None) -> dict[str, dict]:
    p = path or HERE / "cases_v3.jsonl"
    return {c["id"]: c for c in (json.loads(x) for x in p.read_text(encoding="utf-8").splitlines() if x.strip())}


# ---------------------------------------------------------------------------------------------
# What the user reads


def _final(r: dict) -> dict | None:
    outs = [o for o in (r.get("outcomes") or []) if o.get("kind") != "display"]
    return outs[-1] if outs else None


def _card_answer(card: dict) -> tuple[str, dict]:
    a = (card or {}).get("answer")
    if isinstance(a, dict) and a:
        k = next(iter(a))
        return k, a[k]
    return "None", {}


def view(r: dict) -> dict:
    """The answer as the user reads it: ``headline`` (the one number), ``numbers`` (every number
    shown for a single object), ``keys`` (the objects listed or kept), ``ranking`` ([(key, value)]),
    ``texts`` (names shown)."""
    v = {"headline": None, "numbers": [], "keys": None, "ranking": None, "texts": [], "final": None}
    f = _final(r)
    if f is None:
        return v
    v["final"] = f["kind"]
    if f["kind"] == "card":
        kind, a = _card_answer(f.get("card"))
        h = f.get("highlight") or {}
        if kind == "Number":
            v["headline"] = a.get("value")
            v["numbers"].append(a.get("value"))
        if kind == "List":
            v["texts"] += [str(x) for x in a.get("items", [])]
        if kind == "Ranking":
            rows = a.get("rows", [])
            ks = f["card"].get("row_keys") or [None] * len(rows)
            v["ranking"] = [(k, row.get("value"), row.get("label")) for k, row in zip(ks, rows)]
            v["texts"] += [str(row.get("label")) for row in rows[:3]]
        if "keys" in h:
            v["keys"] = h["keys"]
            if v["headline"] is None:
                v["headline"] = len(h.get("rows", []))
    elif f["kind"] == "filtered":
        v["headline"] = f.get("shown")
        v["keys"] = f.get("final_keys")
        # A filter that ends the plan on one or a few objects shows their made fields (the value
        # an earlier step added: "add the elevation, then show Popocatepetl").
        if (f.get("shown") or 0) <= 3:
            name = str(f.get("layer", "")).lower()
            for o in reversed([o for o in r.get("outcomes") or [] if o.get("kind") in ("new_layer", "new_fields")]):
                lr = o.get("layer_rows") or {}
                if name not in (str(lr.get("name", "")).lower(), str(lr.get("alias") or "").lower()):
                    continue
                fields = [o["color_by"]] if o["kind"] == "new_layer" and o.get("color_by") else o.get("fields", [])
                made = []
                for row in lr.get("values") or []:
                    for fl in fields:
                        x = row.get(fl)
                        if isinstance(x, (int, float)) and not isinstance(x, bool):
                            v["numbers"].append(x)
                            made.append(x)
                        elif isinstance(x, str):
                            v["texts"].append(x)
                if made and f.get("shown") == 1:
                    v["headline"] = made[0]
                break
    elif f["kind"] in ("new_layer", "new_fields"):
        lr = f.get("layer_rows") or {}
        v["keys"] = lr.get("keys")
        m = f.get("matched") or {}
        if m.get("keys") is not None:
            # After askfix: the features the step found (count_in or join over a few areas).
            v["keys"] = m["keys"]
        vals = lr.get("values") or []
        fields = [f["color_by"]] if f["kind"] == "new_layer" and f.get("color_by") else f.get("fields", [])
        if lr.get("shown") == 1 and vals:
            row = vals[0]
            for fl in fields:
                x = row.get(fl)
                if isinstance(x, (int, float)) and not isinstance(x, bool):
                    v["numbers"].append(x)
                    if v["headline"] is None:
                        v["headline"] = x
                elif isinstance(x, str):
                    v["texts"].append(x)
        if v["headline"] is None:
            v["headline"] = lr.get("shown")
    return v


def _parts(keys) -> int:
    return min((str(k).count("|") + 1 for k in keys), default=0)


def _cut(keys, n: int) -> set:
    """Keys shortened to their first n parts: a layer made from another keeps only some fields
    (a buffer of World cities has its "name" but not its "country")."""
    return {"|".join(str(k).split("|")[:n]) for k in keys}


def _set_ok(got, spec) -> bool:
    if got is None:
        return False
    s = set(got)
    strict, edge = set(spec["strict"]), set(spec.get("edge", []))
    n = _parts(s)
    if s and n < _parts(strict | edge):
        strict, edge = _cut(strict, n), _cut(edge, n)
    return strict <= s <= strict | edge


def check_answer(spec: dict | None, v: dict) -> bool:
    return which_answer(spec, v) is not None


def which_answer(spec: dict | None, v: dict) -> int | None:
    """0 when the main spec holds, k for the k-th accepted alternative, None when none does."""
    if spec is None:
        return 0
    for i, s in enumerate([spec] + list(spec.get("alt", []))):
        if _check_one(s, v):
            return i
    return None


def _check_one(spec: dict, v: dict) -> bool:
    t = spec["type"]
    if t == "count":
        h = v["headline"]
        return isinstance(h, (int, float)) and spec["lo"] - 1e-9 <= h <= spec["hi"] + 1e-9
    if t == "set":
        return _set_ok(v["keys"], spec)
    if t == "value":
        if spec["value"] is None:
            return False
        return any(isinstance(n, (int, float)) and abs(n - spec["value"]) <= spec["tol"] for n in v["numbers"] + ([v["headline"]] if v["headline"] is not None else []))
    if t == "top":
        k, want = spec["k"], set(spec["keys"])
        if v["ranking"]:
            got = [x[0] for x in v["ranking"] if x[1] is not None][:k]
            n = _parts(got)
            if got and n < _parts(want):
                want = _cut(want, n)
            if not got or not set(got) <= want:
                return False
            return len(want) > k or set(got) == want
        if v["keys"] is not None and 0 < len(v["keys"]) <= k:
            n = _parts(v["keys"])
            if n < _parts(want):
                want = _cut(want, n)
            return set(v["keys"]) <= want and (len(want) > k or set(v["keys"]) == want)
        return False
    if t == "name":
        acc = [a.lower() for a in spec["accept"]]
        texts = [t.lower() for t in v["texts"]] + [str(k).split("|")[0].lower() for k in (v["keys"] or [])[:3]]
        return any(a == t or a in t for a in acc for t in texts)
    raise ValueError(t)


# ---------------------------------------------------------------------------------------------
# What the map shows


def visible(r: dict, universe: set[str]) -> tuple[set[str], dict]:
    """The objects the preview shows: highlighted rows of a layer on the map, plus the drawn
    layer's objects when they are objects of the expected kind (they share keys with it)."""
    p = r.get("preview") or {}
    out: set[str] = set()
    info = {"highlight_on_map": None, "highlight_n": None, "drawn": None, "drawn_n": None}
    h = p.get("highlight")
    if h:
        info["highlight_on_map"] = h.get("on_map")
        info["highlight_n"] = h.get("count")
        if h.get("on_map") and h.get("keys"):
            out |= set(h["keys"])
    d = p.get("layer")
    if d:
        info["drawn"] = d.get("name")
        info["drawn_n"] = d.get("count")
        # A drawn layer with selected rows (after askfix: the answer in the new layer) shows
        # those as the answer; otherwise its objects are the answer when they are of the kind.
        ks = [k for k in (d.get("selected") or d.get("keys") or []) if not str(k).startswith("row:")]
        if ks and _parts(ks) < _parts(universe):
            universe = _cut(universe, _parts(ks))
        if ks and set(ks) & universe:
            out |= set(ks)
    return out, info


def check_preview(case: dict, r: dict) -> dict:
    spec = case["expect"].get("preview")
    p = r.get("preview") or {}
    res = {"preview_ok": None, "area_shown": None, "frames": bool(p.get("fit"))}
    if case["expect"].get("area"):
        d = p.get("layer") or {}
        res["area_shown"] = bool(d) and str(d.get("name", "")).startswith("Within ")
    if spec is None:
        return res
    if spec["type"] == "set":
        universe = set(spec["strict"]) | set(spec.get("edge", []))
        vis, info = visible(r, universe)
        res.update(info)
        res["visible_n"] = len(vis)
        strict = set(spec["strict"])
        if vis and _parts(vis) < _parts(universe):
            strict, universe = _cut(strict, _parts(vis)), _cut(universe, _parts(vis))
        res["preview_ok"] = strict <= vis <= universe
    elif spec["type"] == "layer":
        d = p.get("layer") or {}
        made = [o for o in r.get("outcomes") or [] if o.get("kind") == "new_layer" and o.get("color_by")]
        res["preview_ok"] = bool(d) and bool(made) and d.get("name") == made[-1].get("name")
        res["drawn"] = d.get("name")
    res["preview_strict"] = bool(res["preview_ok"]) and res["frames"] and (res["area_shown"] is not False)
    return res


# ---------------------------------------------------------------------------------------------
# One case


def repairs(r: dict) -> list[str]:
    """The app's repair messages (each a round in which the plan went back to the model)."""
    out = []
    for m in r.get("messages") or []:
        if m.get("role") == "tool" and str(m.get("content", "")).startswith("Plan not accepted"):
            out.append(m["content"])
    return out


def score(case: dict, r: dict) -> dict:
    exp = case["expect"]
    kind = (r.get("answer") or {}).get("kind")
    s = {"case": case["id"], "answer_kind": kind, "repairs": len(repairs(r))}
    v = view(r) if kind == "plan" else {"headline": None, "numbers": [], "keys": None, "ranking": None, "texts": [], "final": None}
    s["headline"] = v["headline"]
    accept = set(exp.get("accept") or [])
    if exp["kind"] == "plan":
        s["honest"] = kind in ("plan", "error", "crash") or kind in accept
        s["accepted_alt"] = kind in accept
    else:
        ok = kind == exp["kind"] or kind in accept
        if kind == "plan" and "zero" in accept:
            ok = v["headline"] == 0
        if kind == "plan" and "plan" in accept:
            ok = True
        s["honest"] = ok
        s["accepted_alt"] = ok and kind != exp["kind"]
    if exp["kind"] != "plan" or s["accepted_alt"]:
        s.update(plan_valid=None, executes=None, answer_ok=None, preview_ok=None, e2e=s["honest"])
        return s
    s["plan_valid"] = kind == "plan" and not (r.get("notes") or [])
    s["first_try"] = s["plan_valid"] and s["repairs"] == 0
    s["executes"] = kind == "plan"
    s["error"] = (r.get("answer") or {}).get("text") if kind in ("error", "crash") else None
    which = which_answer(exp.get("answer"), v) if s["executes"] else None
    s["answer_ok"] = which is not None
    pv = check_preview(case, r) if kind == "plan" else {"preview_ok": None}
    if which:  # right by an accepted alternative reading: the preview spec is the main reading's
        pv["preview_ok"] = None
    s.update(pv)
    pv_ok = pv.get("preview_ok")
    s["e2e"] = bool(s["honest"] and s["answer_ok"] and (pv_ok is None or pv_ok))
    return s


DERIVED_WORDS = re.compile(r"km to nearest|nearest |per (person|million|capita)|\bmean\b|minus|difference|ibtracs|earthquakes\b|_locations|grid|#\d")


def classify(case: dict, s: dict, gold: dict | None) -> str | None:
    """The layer a case failed at, as one class (None when it passed end to end)."""
    if s.get("e2e"):
        return None
    tags = set(case.get("tags") or [])
    if not s["honest"]:
        return "prompt-tool gap" if "tool_gap" in tags else "model plan"
    if case["expect"]["kind"] != "plan":
        return None
    gold_failed = gold is not None and not gold.get("executes")
    if not s["executes"]:
        if s["answer_kind"] == "crash":
            return "executor"
        msg = str(s.get("error") or "")
        if gold_failed:
            return "executor"
        if "no field" in msg and DERIVED_WORDS.search(msg.lower()):
            return "prompt-tool gap"
        return "model plan"
    if not s["plan_valid"]:
        return "model plan"
    if not s["answer_ok"]:
        if gold is not None and gold.get("executes") and not gold.get("answer_ok"):
            return "ground-truth ambiguity" if tags & GT_AMBIGUOUS_TAGS else "executor"
        return "model plan"
    if s.get("preview_ok") is False:
        return "preview-UI"
    return None
