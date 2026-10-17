"""Scoring for harness v2: two modes on the same final plan.

* ``v1``: the original 78 cases (``../cases.jsonl``) and v1's ``scoring.score`` unchanged, so v2
  numbers compare with the 2026-10-17 v1 runs. x05 still expects cannot_do and m06 has no
  alternative; ``stat`` is not checked (v1 cases don't list it).
* ``v2``: the 97 cases of ``cases_v2.jsonl`` (corrections + new cases) with v1's rules plus the
  statistic: a model's count_in / summarize_* call without ``stat`` is read as the runner's
  default (count / mean), synonyms are folded (average -> mean, ...), and the statistic must equal
  the expected one. ``field`` on count_in with stat count is ignored, as the runner ignores it.

``resolve_feature`` calls are lookups, not plan steps; the runner never puts them in the plan.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import scoring  # noqa: E402

STAT_DEFAULT = {"count_in": "count", "summarize_each": "mean", "summarize_here": "mean", "summarize_drawn": "mean"}
STAT_SYN = {"average": "mean", "avg": "mean", "total": "sum", "minimum": "min", "maximum": "max", "stddev": "std",
            "std_dev": "std", "standard_deviation": "std", "standard deviation": "std"}


def load(path: Path) -> dict[str, dict]:
    return {c["id"]: c for c in (json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip())}


CASES_V1 = load(HERE.parent / "cases.jsonl")
CASES_V2 = load(HERE / "cases_v2.jsonl")


def normalize_stats(calls: list[dict], lenient: bool = False) -> list[dict]:
    out = []
    for c in calls:
        n = c.get("name")
        if n in STAT_DEFAULT and isinstance(c.get("args"), dict):
            a = dict(c["args"])
            st = a.get("stat")
            st = STAT_DEFAULT[n] if st in (None, "") else str(st).strip()
            if lenient:
                st = STAT_SYN.get(st.lower(), st.lower())
            a["stat"] = st
            if n == "count_in" and st == "count":
                a.pop("field", None)
            c = {**c, "args": a}
        out.append(c)
    return out


def score_v2(case: dict, calls: list[dict], text: str = "") -> dict:
    strict = scoring.score(case, normalize_stats(calls), text)
    if strict["strict"]:
        return strict
    lenient = scoring.score(case, normalize_stats(calls, lenient=True), text)
    return {"strict": False, "lenient": lenient["lenient"], "reason": strict["reason"]}


def score_both(case_id: str, calls: list[dict], text: str = "") -> dict:
    """{"v1": {...} or None, "v2": {...}} for a final plan."""
    out = {"v1": None, "v2": None}
    if case_id in CASES_V1:
        out["v1"] = scoring.score(CASES_V1[case_id], calls, text)
    if case_id in CASES_V2:
        out["v2"] = score_v2(CASES_V2[case_id], calls, text)
    return out


# Gate-7-shaped sets (as in the v1 report): unanswerable = should be refused, or refusing is right.
def is_unanswerable(case: dict) -> bool:
    plans = [case["expected"]] + case.get("alts", [])
    return any(p and p[0]["name"] == "cannot_do" for p in plans)


def is_answerable(case: dict) -> bool:
    plans = [case["expected"]] + case.get("alts", [])
    return all(p and p[0]["name"] not in ("cannot_do", "ask_user") for p in plans)
