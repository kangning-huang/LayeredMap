"""Summarize the v2 runs next to the v1 baseline.

    python -I report_v2.py

Reads ``v2/results/<run>/*.jsonl`` (skipping dry runs, smoke tests and the development runs
``*-dev*``) and the v1 runs in ``../results/2026-10-17/``; rescores every row from its stored plan
with the current rules; writes ``v2/results/summary.md`` and ``summary.csv``.

Metrics per run and model:

* v1 mode: strict / lenient over the original 78 cases with v1's cases and rules;
* v2 mode: strict / lenient over the 97 cases of ``cases_v2.jsonl``;
* Gate-7-shaped: answerable strict (v2 set) and honest refusal (lenient; cases that should be
  refused or where refusing is right);
* US$ per 1,000 requests (all rounds, as billed), the same without caching (cached tokens repriced
  at the input price, cache-write premium removed), median and p90 latency (all rounds), mean
  model calls, mean repair rounds, resolve calls, plans read from text, prose replies read as
  ask/refuse.
"""

from __future__ import annotations

import csv
import json
import statistics
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

import run_eval  # noqa: E402
import scoring  # noqa: E402
import scoring_v2  # noqa: E402

V1_DIR = HERE.parent / "results" / "2026-10-17"
GROUPS = ["action", "filter", "multi", "zh", "vague", "impossible", "trap", "units", "stat", "resolve"]
CATALOG = run_eval.load_catalog()


def rows_of(path: Path) -> list[dict]:
    return [json.loads(x) for x in path.read_text(encoding="utf-8").splitlines() if x.strip()]


def rescore_v2(r: dict) -> dict:
    if "error" in r or "plan" not in r:
        return r
    calls = [scoring.canonicalize(c["name"], c.get("args") or {}) for c in r["plan"]]
    sc = scoring_v2.score_both(r["case"], calls, r.get("text", ""))
    r["score_v1"], r["score_v2"] = sc["v1"], sc["v2"]
    return r


def rescore_v1(r: dict) -> dict:
    """A v1-harness row: v1 scoring, and v2 scoring on the 78 (no new cases were run)."""
    if "error" in r or "calls" not in r:
        return r
    r["score_v1"] = scoring.score(scoring_v2.CASES_V1[r["case"]], r["calls"], r.get("text", ""))
    r["score_v2"] = scoring_v2.score_v2(scoring_v2.CASES_V2[r["case"]], r["calls"], r.get("text", ""))
    return r


def load_runs() -> dict[tuple[str, str], list[dict]]:
    out = {}
    for run in sorted(p for p in (HERE / "results").iterdir() if p.is_dir()):
        if run.name.startswith("dryrun") or "-smoke" in run.name or "-dev" in run.name:
            continue
        for f in sorted(run.glob("*.jsonl")):
            if f.name.startswith("._"):
                continue
            rows = [rescore_v2(r) for r in rows_of(f)]
            if rows:
                out[(run.name, rows[0]["model"])] = rows
    return out


def load_v1() -> dict[tuple[str, str], list[dict]]:
    out = {}
    for run in ("per_action", "per_action-repeat2"):
        for f in sorted((V1_DIR / run).glob("*.jsonl")):
            if f.name.startswith("._"):
                continue
            rows = [rescore_v1(r) for r in rows_of(f)]
            if rows and rows[0]["model"] in run_eval_models():
                out[("v1-" + run, rows[0]["model"])] = rows
    return out


def run_eval_models():
    return {"anthropic/claude-haiku-5.5", "z-ai/glm-5.3-flash", "openai/gpt-oss-120b", "google/gemini-3.5-flash"}


def nocache_cost(r: dict, model: str) -> float:
    p = CATALOG[model]["pricing"]
    pin = float(p.get("prompt") or 0)
    read = float(p.get("input_cache_read") or pin)
    write = float(p.get("input_cache_write") or pin)
    return (r.get("cost_usd") or 0.0) + (r.get("cached_tokens") or 0) * (pin - read) - (r.get("cache_write_tokens") or 0) * (write - pin)


def summarize(rows: list[dict], model: str, v1_harness: bool = False) -> dict:
    scored = [r for r in rows if "error" not in r and r.get("score_v2")]
    in_v1 = [r for r in scored if r["case"] in scoring_v2.CASES_V1]
    n1, n2 = len(scoring_v2.CASES_V1), (len(scoring_v2.CASES_V1) if v1_harness else len(scoring_v2.CASES_V2))
    ans = [c for c in scoring_v2.CASES_V2.values() if scoring_v2.is_answerable(c) and (not v1_harness or c["id"] in scoring_v2.CASES_V1)]
    una = [c for c in scoring_v2.CASES_V2.values() if scoring_v2.is_unanswerable(c) and (not v1_harness or c["id"] in scoring_v2.CASES_V1)]
    by = {r["case"]: r for r in scored}
    lat = [r.get("latency_s") for r in scored if r.get("latency_s") is not None]
    cost = [r.get("cost_usd") or 0.0 for r in rows if "status" in r or "cost_usd" in r]
    out = {
        "n": len(scored),
        "v1_strict": sum(r["score_v1"]["strict"] for r in in_v1) / n1,
        "v1_lenient": sum(r["score_v1"]["lenient"] for r in in_v1) / n1,
        "v2_strict": sum(r["score_v2"]["strict"] for r in scored) / n2,
        "v2_lenient": sum(r["score_v2"]["lenient"] for r in scored) / n2,
        "answerable_strict": sum(by[c["id"]]["score_v2"]["strict"] for c in ans if c["id"] in by) / len(ans),
        "n_answerable": len(ans),
        "refusal_lenient": sum(by[c["id"]]["score_v2"]["lenient"] for c in una if c["id"] in by) / len(una),
        "n_unanswerable": len(una),
        "cost_per_1k": 1000 * statistics.mean(cost) if cost else 0.0,
        "cost_per_1k_nocache": 1000 * statistics.mean(nocache_cost(r, model) for r in rows) if rows else 0.0,
        "cached_share": (sum(r.get("cached_tokens") or 0 for r in rows) / max(1, sum(r.get("prompt_tokens") or 0 for r in rows))),
        "lat_median": statistics.median(lat) if lat else None,
        "lat_p90": statistics.quantiles(lat, n=10)[-1] if len(lat) >= 10 else None,
        "turns": statistics.mean(r.get("turns", 0) for r in scored) if scored else 0,
        "repairs": statistics.mean(r.get("repair_rounds", 0) for r in scored) if scored else 0,
        "repaired_cases": sum(1 for r in scored if r.get("repair_rounds")),
        "resolves": statistics.mean(r.get("resolve_calls", 0) for r in scored) if scored else 0,
        "from_text": sum(1 for r in scored if r.get("from_text")),
        "text_as_meta": sum(1 for r in scored if r.get("text_as_meta")),
        "prompt_tokens": statistics.mean(r.get("prompt_tokens", 0) for r in scored) if scored else 0,
        "completion_tokens": statistics.mean(r.get("completion_tokens", 0) for r in scored) if scored else 0,
        "by": {},
        "reasons": Counter(r["score_v2"]["reason"] for r in scored if not r["score_v2"]["strict"]),
        "fails": sorted(r["case"] for r in scored if not r["score_v2"]["strict"]),
    }
    for g in GROUPS:
        rs = [r for r in scored if g in scoring_v2.CASES_V2[r["case"]]["categories"]]
        out["by"][g] = (sum(r["score_v2"]["strict"] for r in rs) / len(rs), len(rs)) if rs else None
    return out


def pct(x) -> str:
    return "–" if x is None else f"{100 * x:.0f}%"


def main() -> None:
    runs = load_runs()
    v1 = load_v1()
    lines = ["# Harness v2 results", "", "Rescored from the stored plans by `report_v2.py`. v1 rows: the 2026-10-17 v1 runs "
             "(v2 mode for them is the 78 corrected cases only; the v1 tools had no `stat`).", ""]
    hdr = ("| Run | Model | v1 mode strict | v1 mode lenient | v2 strict | v2 lenient | Answerable strict | Honest refusal | "
           "US$/1k | US$/1k no cache | Cached | Median s | p90 s | Model calls | Repair rounds | Resolve calls |")
    lines += [hdr, "|" + "---|" * 16]
    table = []
    for (run, model), rows in sorted(v1.items()) + sorted(runs.items()):
        s = summarize(rows, model, v1_harness=run.startswith("v1-"))
        table.append((run, model, s))
        lines.append(f"| {run} | {model} | {pct(s['v1_strict'])} | {pct(s['v1_lenient'])} | {pct(s['v2_strict'])} | {pct(s['v2_lenient'])} | "
                     f"{pct(s['answerable_strict'])} ({s['n_answerable']}) | {pct(s['refusal_lenient'])} ({s['n_unanswerable']}) | "
                     f"{s['cost_per_1k']:.3f} | {s['cost_per_1k_nocache']:.3f} | {pct(s['cached_share'])} | "
                     f"{(s['lat_median'] or 0):.2f} | {(s['lat_p90'] or 0):.2f} | {s['turns']:.2f} | {s['repairs']:.2f} | {s['resolves']:.2f} |")
    lines += ["", "## By category (v2 strict)", "", "| Run | Model | " + " | ".join(GROUPS) + " |", "|" + "---|" * (len(GROUPS) + 2)]
    for run, model, s in table:
        cells = [("–" if s["by"][g] is None else f"{pct(s['by'][g][0])}") for g in GROUPS]
        lines.append(f"| {run} | {model} | " + " | ".join(cells) + " |")
    lines += ["", "## Failures (v2 strict)", ""]
    for run, model, s in table:
        if run.startswith("v1-"):
            continue
        reasons = ", ".join(f"{k} {v}" for k, v in s["reasons"].most_common())
        lines.append(f"- **{run} / {model}**: {len(s['fails'])} — {', '.join(s['fails'])} ({reasons}); "
                     f"repaired cases {s['repaired_cases']}, plans read from text {s['from_text']}, prose read as ask/refuse {s['text_as_meta']}")
    (HERE / "results" / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    with open(HERE / "results" / "summary.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        keys = ["v1_strict", "v1_lenient", "v2_strict", "v2_lenient", "answerable_strict", "refusal_lenient", "cost_per_1k", "cost_per_1k_nocache",
                "cached_share", "lat_median", "lat_p90", "turns", "repairs", "repaired_cases", "resolves", "from_text", "text_as_meta", "prompt_tokens", "completion_tokens", "n"]
        w.writerow(["run", "model"] + keys)
        for run, model, s in table:
            w.writerow([run, model] + [round(s[k], 4) if isinstance(s[k], float) else s[k] for k in keys])
    print("\n".join(lines))


if __name__ == "__main__":
    main()
