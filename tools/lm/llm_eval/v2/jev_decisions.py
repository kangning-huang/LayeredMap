"""Jev 1.13 (TypeSafe, OpenRouter's Decisions API) on the decisions it can make.

Jev returns typed choices only (choice / yes-no / score / box): it can't write a plan's
values (distances, thresholds, field names typed by the user). So it is scored on the two
decisions a plan starts with, against the same decisions taken from the full-plan runs:

* mode: act, ask the user, or say it can't be done;
* first action: which action the plan starts with (for cases that should act).

The request goes to POST https://openrouter.ai/api/alpha/decisions with `state` = the same map
description and request the plan models see, and two `choice` questions whose options are the
actions (described as in the tools). The key is read from the keychain as in v1; never printed.

Usage:  python -I tools/lm/llm_eval/v2/jev_decisions.py [--model typesafe/jev-1.13] [--budget 0.10]
Writes results/jev-decisions/<model>.jsonl and prints a comparison with the plan models' runs.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import prompt_v2  # noqa: E402
import run_eval  # noqa: E402
import tools_v2  # noqa: E402

URL = "https://openrouter.ai/api/alpha/decisions"
DISPLAY = {
    "filter": "Show only the objects of a layer that meet conditions (a display filter).",
    "color_by": "Colour a layer by one of its fields.",
    "zoom_to": "Zoom the map to a layer or an object.",
    "view_data": "Open a layer's table.",
}
MODES = {
    "act": "The request can be answered with the listed map actions on the open layers.",
    "ask_user": "The request is too vague or ambiguous to act on; one question to the user is needed first.",
    "cannot_do": "The request needs something these actions or layers can't do (data not on the map, routing, forecasting, code).",
}


def cases() -> list[dict]:
    return [json.loads(l) for l in (HERE / "cases_v2.jsonl").read_text(encoding="utf-8").splitlines() if l.strip()]


def expected_mode(c: dict) -> str:
    first = c["expected"][0]["name"] if c["expected"] else "cannot_do"
    return first if first in ("ask_user", "cannot_do") else "act"


def expected_firsts(c: dict) -> set[str]:
    out = set()
    for plan in [c["expected"]] + [a for a in c.get("alts", []) if isinstance(a, list)]:
        if plan and plan[0]["name"] not in ("ask_user", "cannot_do", "resolve_feature"):
            out.add(plan[0]["name"])
    return out


def action_criteria() -> dict[str, str]:
    crit = {a["id"]: tools_v2.DESCRIPTIONS.get(a["id"], a["id"]) for a in tools_v2.toolmod.action_table()}
    crit.update(DISPLAY)
    return crit


def ask(key: str, model: str, c: dict, crit: dict) -> tuple[dict, float]:
    state = prompt_v2.map_context(c["context"], c.get("selection")) + "\n\nThe user's request: " + c["request"]
    body = {
        "model": model,
        "state": state,
        "questions": {
            "mode": {"type": "choice", "instructions": "What should the map assistant do with this request?", "criteria": MODES},
            "first_action": {
                "type": "choice",
                "instructions": "If the request can be answered, which map action does the plan start with? (Filtering a layer first counts as the filter.)",
                "criteria": crit,
            },
        },
    }
    req = urllib.request.Request(URL, data=json.dumps(body).encode(), headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"})
    t = time.time()
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.load(r), time.time() - t
        except urllib.error.HTTPError as e:
            if e.code in (429, 500, 502, 503, 524, 529) and attempt < 3:
                time.sleep(2 ** attempt)
                continue
            return {"error": f"HTTP {e.code}: {e.read().decode()[:200]}"}, time.time() - t
    return {"error": "retries exhausted"}, time.time() - t


def plan_decisions(path: Path) -> dict[str, tuple[str, str | None]]:
    """(mode, first action) from a full-plan run's records."""
    out = {}
    for l in path.read_text(encoding="utf-8").splitlines():
        r = json.loads(l)
        plan = r.get("plan") or []
        names = [p["name"] for p in plan]
        if not names:
            mode = "cannot_do" if "can't" in (r.get("text") or "").lower() or "cannot" in (r.get("text") or "").lower() else "none"
            out[r["case"]] = (mode, None)
        elif names[0] in ("ask_user", "cannot_do"):
            out[r["case"]] = (names[0], None)
        else:
            out[r["case"]] = ("act", names[0])
    return out


def score(decided: dict[str, tuple[str, str | None]], cs: list[dict]) -> dict:
    n_mode = ok_mode = n_first = ok_first = 0
    for c in cs:
        d = decided.get(c["id"])
        if not d:
            continue
        n_mode += 1
        ok_mode += d[0] == expected_mode(c)
        firsts = expected_firsts(c)
        if expected_mode(c) == "act" and firsts:
            n_first += 1
            ok_first += d[1] in firsts
    return {"mode": (ok_mode, n_mode), "first_action": (ok_first, n_first)}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="typesafe/jev-1.13")
    ap.add_argument("--budget", type=float, default=0.10)
    a = ap.parse_args(argv)
    cs = cases()
    crit = action_criteria()
    out_dir = HERE / "results" / "jev-decisions"
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / (a.model.replace("/", "__") + ".jsonl")
    key = run_eval.read_key()
    if not key:
        print("No OpenRouter key in the keychain.", file=sys.stderr)
        return 1
    spent = 0.0
    recs = []
    for c in cs:
        if spent >= a.budget:
            print("budget reached")
            break
        d, dt = ask(key, a.model, c, crit)
        if "error" in d:
            recs.append({"case": c["id"], "error": d["error"], "latency_s": round(dt, 3)})
            continue
        ans = d.get("answers", {})
        cost = float((d.get("usage") or {}).get("cost") or 0.0)
        spent += cost
        recs.append({
            "case": c["id"],
            "mode": ans.get("mode", {}).get("choice"),
            "mode_confidence": ans.get("mode", {}).get("confidence"),
            "first_action": ans.get("first_action", {}).get("choice"),
            "first_confidence": ans.get("first_action", {}).get("confidence"),
            "input_tokens": (d.get("usage") or {}).get("input_tokens"),
            "cost_usd": cost,
            "latency_s": round(dt, 3),
        })
    out.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in recs), encoding="utf-8")
    jev = {r["case"]: (r["mode"], r["first_action"] if r["mode"] == "act" else None) for r in recs if "error" not in r}
    errors = sum("error" in r for r in recs)
    lat = sorted(r["latency_s"] for r in recs if "error" not in r)
    rows = [(a.model + " (decisions only)", score(jev, cs), spent / max(1, len(jev)) * 1000, lat[len(lat) // 2] if lat else 0)]
    for label, f in [
        ("claude-haiku-5.5 (full plan, submit_plan)", "v2-submit_plan/anthropic__claude-haiku-5.5.jsonl"),
        ("glm-5.3-flash (full plan, submit_plan)", "v2-submit_plan/z-ai__glm-5.3-flash.jsonl"),
        ("jev-router (full plan, submit_plan)", "v2-submit_plan-jev/typesafe__jev-router.jsonl"),
        ("jev-router (full plan, per_action)", "v2-per_action-jev/typesafe__jev-router.jsonl"),
    ]:
        p = HERE / "results" / f
        if p.exists():
            recs2 = [json.loads(l) for l in p.read_text(encoding="utf-8").splitlines()]
            cost = sum(r.get("cost_usd") or 0 for r in recs2) / max(1, len(recs2)) * 1000
            lat2 = sorted(r.get("latency_s") or 0 for r in recs2)
            rows.append((label, score(plan_decisions(p), cs), cost, lat2[len(lat2) // 2]))
    print(f"{'model':48s} {'mode':>10s} {'1st action':>12s} {'$/1k':>7s} {'median s':>9s}")
    for label, s, cost, med in rows:
        m, f = s["mode"], s["first_action"]
        print(f"{label:48s} {m[0]:>4d}/{m[1]:<4d}{100*m[0]/max(1,m[1]):4.0f}% {f[0]:>4d}/{f[1]:<4d}{100*f[0]/max(1,f[1]):4.0f}% {cost:7.3f} {med:9.2f}")
    print(f"errors: {errors}; spent on Jev: ${spent:.4f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
