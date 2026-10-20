"""Summary of the v3 runs: ``results/summary.md``, ``results/summary.csv`` and
``results/failures.csv`` (one row per failing case and run, with its class).

    python -I report_v3.py                     # every run under results/
    python -I report_v3.py --gold results/mock/mock.jsonl

The gold run (the mock run of the gold plans) is the baseline that tells the app's faults from
the model's (``scoring_v3.classify``).
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import run_v3  # noqa: E402
import scoring_v3 as S  # noqa: E402

RESULTS = HERE / "results"
RAW = run_v3.RAW


def read(path: Path) -> list[dict]:
    return [json.loads(x) for x in path.read_text(encoding="utf-8").splitlines() if x.strip()]


def pct(n, d):
    return f"{100 * n / d:.0f}%" if d else "–"


def refreshed(f: Path, rows: list[dict]) -> list[dict]:
    """A paid run's rows with outcomes and preview from its replay through the current
    ask_eval (``results/replay-<run>-post/``), when that replay ran the same steps: the replay
    adds what the recorded run's binary didn't write (the matched features), nothing else
    changes. Usage, latency and the exchange stay the paid run's."""
    rp = RAW / f"replay-{f.parent.name}-post" / f.name
    if not rp.exists():
        return rows
    rep = {r["id"]: r for r in read(rp)}
    out = []
    for r in rows:
        x = rep.get(r["id"])
        if x and r.get("answer", {}).get("kind") == "plan" and x.get("answer", {}).get("kind") == "plan" and x.get("steps") == r.get("steps"):
            r = dict(r, outcomes=x.get("outcomes"), preview=x.get("preview"))
        out.append(r)
    return out


def score_run(cases: dict, rows: list[dict], gold: dict[str, dict]) -> list[dict]:
    out = []
    for r in rows:
        c = cases.get(r["id"])
        if c is None:
            continue
        s = S.score(c, r)
        s["class"] = S.classify(c, s, gold.get(r["id"]))
        s["qtype"] = c["qtype"]
        s["lang"] = c["lang"]
        s["difficulty"] = c["difficulty"]
        s["kind"] = c["expect"]["kind"]
        u = r.get("usage") or {}
        s["prompt_tokens"] = u.get("prompt_tokens", 0)
        s["completion_tokens"] = u.get("completion_tokens", 0)
        s["cost"] = u.get("cost_usd", 0.0)
        s["calls"] = u.get("calls", 0)
        s["seconds"] = r.get("wall_seconds") or r.get("seconds") or 0.0
        s["model_seconds"] = sum(x.get("latency", 0) for x in r.get("calls") or [])
        out.append(s)
    return out


def table_row(name: str, sc: list[dict]) -> dict:
    n = len(sc)
    plan_cases = [s for s in sc if s["kind"] == "plan" and not s.get("accepted_alt")]
    np_ = len(plan_cases)
    cls = Counter(s["class"] for s in sc if s["class"])
    secs = [s["seconds"] for s in sc]
    return {
        "run": name,
        "cases": n,
        "e2e": sum(1 for s in sc if s["e2e"]),
        "honest": sum(1 for s in sc if s["honest"]),
        "plan_cases": np_,
        "plan_valid": sum(1 for s in plan_cases if s.get("plan_valid")),
        "first_try": sum(1 for s in plan_cases if s.get("first_try")),
        "executes": sum(1 for s in plan_cases if s.get("executes")),
        "answer_ok": sum(1 for s in plan_cases if s.get("answer_ok")),
        "preview_ok": sum(1 for s in plan_cases if s.get("preview_ok")),
        "preview_n": sum(1 for s in plan_cases if s.get("preview_ok") is not None),
        "area_shown": sum(1 for s in plan_cases if s.get("area_shown")),
        "area_n": sum(1 for s in plan_cases if s.get("area_shown") is not None),
        "frames": sum(1 for s in plan_cases if s.get("frames")),
        **{f"fail_{k}": cls.get(k, 0) for k in S.CLASSES},
        "median_s": round(statistics.median(secs), 2) if secs else 0,
        "p90_s": round(sorted(secs)[int(0.9 * (len(secs) - 1))], 2) if secs else 0,
        "prompt_tokens": sum(s["prompt_tokens"] for s in sc),
        "completion_tokens": sum(s["completion_tokens"] for s in sc),
        "cost": round(sum(s["cost"] for s in sc), 4),
        "cost_per_case": round(sum(s["cost"] for s in sc) / n, 5) if n else 0,
    }


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--gold", default=str(RAW / "mock" / "mock.jsonl"))
    ap.add_argument("--runs", default="", help="comma list of result files (default: all)")
    a = ap.parse_args(argv)
    cases = S.load_cases()
    gold_rows = read(Path(a.gold)) if Path(a.gold).exists() else []
    gold = {s["case"]: s for s in score_run(cases, gold_rows, {})}
    files = [Path(x) for x in a.runs.split(",") if x] or sorted(p for p in RAW.glob("*/*.jsonl") if not p.name.startswith("._") and not p.parent.name.endswith("-post") and "smoke" not in p.parent.name)
    rows, per_case, fails = [], {}, []
    for f in files:
        name = f"{f.parent.name}/{f.stem}"
        sc = score_run(cases, refreshed(f, read(f)) if f.parent.name.startswith("paid") else read(f), gold)
        per_case[name] = sc
        rows.append(table_row(name, sc))
        for s in sc:
            if s["class"]:
                fails.append({"run": name, "case": s["case"], "class": s["class"], "qtype": s["qtype"], "lang": s["lang"],
                              "answer_kind": s["answer_kind"], "headline": s.get("headline"), "error": (s.get("error") or "")[:160],
                              "preview_ok": s.get("preview_ok"), "highlight_n": s.get("highlight_n"), "drawn": s.get("drawn")})
    scored = RESULTS / "scored"
    scored.mkdir(parents=True, exist_ok=True)
    keep = ("case", "class", "qtype", "lang", "kind", "answer_kind", "honest", "accepted_alt", "plan_valid", "first_try", "repairs", "executes",
            "answer_ok", "preview_ok", "area_shown", "frames", "e2e", "headline", "error", "highlight_n", "highlight_on_map", "drawn", "visible_n",
            "prompt_tokens", "completion_tokens", "cost", "calls", "seconds")
    for name, sc in per_case.items():
        with (scored / (name.replace("/", "__") + ".jsonl")).open("w", encoding="utf-8") as fh:
            for s_ in sc:
                fh.write(json.dumps({k: s_.get(k) for k in keep}, ensure_ascii=False) + "\n")
    with (RESULTS / "summary.csv").open("w", newline="", encoding="utf-8") as fh:
        if rows:
            w = csv.DictWriter(fh, fieldnames=list(rows[0]))
            w.writeheader()
            w.writerows(rows)
    with (RESULTS / "failures.csv").open("w", newline="", encoding="utf-8") as fh:
        if fails:
            w = csv.DictWriter(fh, fieldnames=list(fails[0]))
            w.writeheader()
            w.writerows(fails)
    md = ["# Harness v3 results", "", f"{len(cases)} cases. Gold baseline: `{Path(a.gold).name if Path(a.gold).exists() else 'none'}`.", ""]
    md.append("| Run | e2e | honest | plan valid (1st try) | executes | answer | preview | area drawn | frames | median s | tokens in/out | cost |")
    md.append("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for r in rows:
        md.append(
            f"| {r['run']} | {r['e2e']}/{r['cases']} ({pct(r['e2e'], r['cases'])}) | {pct(r['honest'], r['cases'])} | "
            f"{pct(r['plan_valid'], r['plan_cases'])} ({pct(r['first_try'], r['plan_cases'])}) | {pct(r['executes'], r['plan_cases'])} | "
            f"{pct(r['answer_ok'], r['plan_cases'])} | {pct(r['preview_ok'], r['preview_n'])} | {pct(r['area_shown'], r['area_n'])} | {pct(r['frames'], r['plan_cases'])} | "
            f"{r['median_s']} | {r['prompt_tokens']:,}/{r['completion_tokens']:,} | ${r['cost']:.3f} |"
        )
    md += ["", "Failures by layer (first layer that failed):", "", "| Run | " + " | ".join(S.CLASSES) + " |", "|---|" + "---|" * len(S.CLASSES)]
    for r in rows:
        md.append(f"| {r['run']} | " + " | ".join(str(r[f"fail_{k}"]) for k in S.CLASSES) + " |")
    md += ["", "End-to-end by question type:", ""]
    qtypes = sorted({s["qtype"] for sc in per_case.values() for s in sc})
    md.append("| Run | " + " | ".join(qtypes) + " |")
    md.append("|---|" + "---|" * len(qtypes))
    for name, sc in per_case.items():
        by = defaultdict(list)
        for s in sc:
            by[s["qtype"]].append(s["e2e"])
        md.append(f"| {name} | " + " | ".join(f"{sum(by[q])}/{len(by[q])}" if by[q] else "–" for q in qtypes) + " |")
    md += ["", "End-to-end by language:", ""]
    langs = sorted({s["lang"] for sc in per_case.values() for s in sc})
    md.append("| Run | " + " | ".join(langs) + " |")
    md.append("|---|" + "---|" * len(langs))
    for name, sc in per_case.items():
        by = defaultdict(list)
        for s in sc:
            by[s["lang"]].append(s["e2e"])
        md.append(f"| {name} | " + " | ".join(f"{sum(by[q])}/{len(by[q])}" if by[q] else "–" for q in langs) + " |")
    (RESULTS / "summary.md").write_text("\n".join(md) + "\n", encoding="utf-8")
    print("\n".join(md))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
