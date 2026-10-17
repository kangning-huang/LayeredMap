"""Summarize results: CSV, markdown tables, failure analysis and the Pareto chart.

    python -I report.py [--date 2026-10-17] [--chart ../../../docs/lm/img/llm-eval-pareto.png]

Reads ``results/<date>/<run>/*.jsonl`` and writes ``results/<date>/summary.csv`` and
``results/<date>/summary.md``.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import run_eval  # noqa: E402
import scoring  # noqa: E402

CASES = {c["id"]: c for c in run_eval.load_cases(HERE / "cases.jsonl")}
GROUPS = ["action", "filter", "style", "display", "multi", "zh", "vague", "impossible", "trap", "units"]


def load(date_dir: Path) -> dict[tuple[str, str], list[dict]]:
    out: dict[tuple[str, str], list[dict]] = {}
    for run in sorted(p for p in date_dir.iterdir() if p.is_dir() and not p.name.startswith("dryrun")):
        for f in sorted(p for p in run.glob("*.jsonl") if not p.name.startswith("._")):  # exFAT AppleDouble files
            rows = [json.loads(line) for line in f.read_text(encoding="utf-8").splitlines() if line.strip()]
            for r in rows:
                # Rescore from the stored calls, so every run is scored by the current rules.
                if "error" not in r and "calls" in r:
                    r.update(scoring.score(CASES[r["case"]], r["calls"], r.get("text", "")))
                    if r.get("malformed") and not r["strict"]:
                        r["reason"] = "malformed_call"
            if rows:
                out[(run.name, rows[0]["model"])] = rows
    return out


def pct(x: float) -> str:
    return f"{100 * x:.0f}%"


def summarize(rows: list[dict]) -> dict:
    scored = [r for r in rows if "strict" in r]
    n_cases = len(CASES)
    lat = [r["latency_s"] for r in scored if r.get("latency_s") is not None]
    cost = [r.get("cost_usd") or 0.0 for r in rows if "status" in r]

    def mean(key):
        v = [r.get(key) or 0 for r in scored]
        return statistics.mean(v) if v else 0.0

    by = {}
    for g in GROUPS:
        rs = [r for r in scored if g in CASES[r["case"]]["categories"]]
        by[g] = (sum(r["strict"] for r in rs) / len(rs)) if rs else None
    return {
        "n": len(scored),
        "errors": n_cases - len(scored),
        # Accuracy over all cases: a case that errored counts as wrong.
        "strict": sum(r["strict"] for r in scored) / n_cases,
        "lenient": sum(r["lenient"] for r in scored) / n_cases,
        "cost_per_1k": 1000 * statistics.mean(cost) if cost else 0.0,
        "cost_total": sum(cost),
        "lat_median": statistics.median(lat) if lat else None,
        "lat_p90": statistics.quantiles(lat, n=10)[-1] if len(lat) >= 10 else None,
        "prompt_tokens": mean("prompt_tokens"),
        "completion_tokens": mean("completion_tokens"),
        "reasoning_tokens": mean("reasoning_tokens"),
        "by": by,
        "reasons": Counter(r.get("reason") for r in scored if not r["strict"]),
    }


def pareto(points: list[tuple[str, float, float]]) -> set[str]:
    """Names not dominated (lower cost and higher accuracy)."""
    keep = set()
    for name, c, a in points:
        if not any((c2 <= c and a2 >= a) and (c2 < c or a2 > a) for _, c2, a2 in points):
            keep.add(name)
    return keep


def chart(summ: dict, path: Path, run: str) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import matplotlib.ticker  # noqa: F401

    items = [(m, s) for (r, m), s in summ.items() if r == run and s["n"] > 0]
    if not items:
        return
    pts = [(m, s["cost_per_1k"], s["strict"]) for m, s in items]
    front = pareto(pts)
    ink, muted, grid, blue, surface = "#0b0b0b", "#8a8984", "#e4e3df", "#2a78d6", "#fcfcfb"
    fig, axes = plt.subplots(1, 2, figsize=(12, 5.2), dpi=150, facecolor=surface)
    for ax, xkey, xlabel, logx in [
        (axes[0], "cost_per_1k", "Cost per 1,000 requests (US$, log scale)", True),
        (axes[1], "lat_median", "Median latency (s)", False),
    ]:
        ax.set_facecolor(surface)
        for side in ("top", "right"):
            ax.spines[side].set_visible(False)
        for side in ("left", "bottom"):
            ax.spines[side].set_color(muted)
        ax.grid(True, color=grid, linewidth=0.8)
        ax.set_axisbelow(True)
        for m, s in items:
            x = s[xkey]
            if x is None:
                continue
            on = m in front
            ax.scatter([x], [100 * s["strict"]], s=60 if on else 40, color=blue if on else muted, edgecolor=surface, linewidth=1.5, zorder=3)
            # Nudge labels that would sit on a neighbour's (same accuracy, close x).
            crowded = any(m2 < m and abs(100 * (s2["strict"] - s["strict"])) < 2 and s2[xkey] is not None
                          and abs((s2[xkey] - x) / (x if logx else 1)) < (0.6 if logx else 0.8) for m2, s2 in items)
            ax.annotate(m.split("/")[-1], (x, 100 * s["strict"]), xytext=(5, -10 if crowded else 4), textcoords="offset points", fontsize=7, color=ink if on else "#52514e")
        if xkey == "cost_per_1k":
            fr = sorted((s["cost_per_1k"], 100 * s["strict"]) for m, s in items if m in front)
            ax.step([c for c, _ in fr], [a for _, a in fr], where="post", color=blue, linewidth=2, alpha=0.6, zorder=2)
        if logx:
            ax.set_xscale("log")
            ax.xaxis.set_major_formatter(matplotlib.ticker.FuncFormatter(lambda v, _: f"${v:g}"))
        ax.set_xlabel(xlabel, color=ink)
        ax.set_ylabel("Strict accuracy (%)", color=ink)
        ax.tick_params(colors="#52514e", labelsize=8)
    axes[0].scatter([], [], color=blue, label="Pareto-optimal (cost vs accuracy)")
    axes[0].scatter([], [], color=muted, label="Dominated")
    axes[0].legend(frameon=False, fontsize=8, loc="lower right")
    fig.suptitle(f"LayeredMap assistant: accuracy vs cost and latency ({len(CASES)} cases, {run})", color=ink, fontsize=11)
    fig.tight_layout()
    path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(path, facecolor=surface)
    plt.close(fig)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--date", default=dt.date.today().isoformat())
    ap.add_argument("--out", default=str(HERE / "results"))
    ap.add_argument("--chart", default=str(HERE.parents[2] / "docs/lm/img/llm-eval-pareto.png"))
    ap.add_argument("--chart-run", default="per_action")
    a = ap.parse_args(argv)
    date_dir = Path(a.out) / a.date
    data = load(date_dir)
    summ = {k: summarize(v) for k, v in data.items()}

    with open(date_dir / "summary.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["run", "model", "scored", "errors", "strict", "lenient", "cost_per_1k_usd", "cost_total_usd", "latency_median_s", "latency_p90_s",
                    "prompt_tokens", "completion_tokens", "reasoning_tokens"] + [f"strict_{g}" for g in GROUPS])
        for (run, m), s in sorted(summ.items(), key=lambda kv: (kv[0][0], -kv[1]["strict"], kv[1]["cost_per_1k"])):
            w.writerow([run, m, s["n"], s["errors"], f"{s['strict']:.4f}", f"{s['lenient']:.4f}", f"{s['cost_per_1k']:.4f}", f"{s['cost_total']:.5f}",
                        f"{s['lat_median']:.2f}" if s["lat_median"] is not None else "", f"{s['lat_p90']:.2f}" if s["lat_p90"] is not None else "",
                        f"{s['prompt_tokens']:.0f}", f"{s['completion_tokens']:.0f}", f"{s['reasoning_tokens']:.0f}"]
                       + [f"{s['by'][g]:.3f}" if s["by"][g] is not None else "" for g in GROUPS])

    lines = []
    for run in sorted({r for r, _ in summ}):
        rows = sorted(((m, s) for (r, m), s in summ.items() if r == run), key=lambda kv: (-kv[1]["strict"], kv[1]["cost_per_1k"]))
        front = pareto([(m, s["cost_per_1k"], s["strict"]) for m, s in rows])
        lines += [f"## {run}", "",
                  "| Model | Strict | Lenient | $ / 1,000 req | Median s | p90 s | Prompt tok | Completion tok | Reasoning tok | Errors |",
                  "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
        for m, s in rows:
            star = " *" if m in front else ""
            lines.append(f"| {m}{star} | {pct(s['strict'])} | {pct(s['lenient'])} | {s['cost_per_1k']:.3f} | "
                         f"{s['lat_median']:.2f} | {s['lat_p90']:.2f} | {s['prompt_tokens']:.0f} | {s['completion_tokens']:.0f} | {s['reasoning_tokens']:.0f} | {s['errors']} |"
                         if s["lat_median"] is not None and s["lat_p90"] is not None else f"| {m} | (no results) |")
        lines += ["", "\\* Pareto-optimal on cost vs strict accuracy.", "", "Strict accuracy by category:", "",
                  "| Model | " + " | ".join(GROUPS) + " |", "|---|" + "---:|" * len(GROUPS)]
        for m, s in rows:
            lines.append(f"| {m} | " + " | ".join(pct(s["by"][g]) if s["by"][g] is not None else "" for g in GROUPS) + " |")
        reasons: Counter = Counter()
        for _, s in rows:
            reasons.update(s["reasons"])
        lines += ["", "Strict failures by kind (all models):", "", "| Kind | Count |", "|---|---:|"]
        for k, v in reasons.most_common():
            lines.append(f"| {k or 'error'} | {v} |")
        # Hardest cases
        per_case = defaultdict(int)
        for (r, m), rs in data.items():
            if r == run:
                for x in rs:
                    if not x.get("strict"):
                        per_case[x["case"]] += 1
        lines += ["", "Cases most often failed (strict):", "", "| Case | Request | Models failing |", "|---|---|---:|"]
        for cid, n in sorted(per_case.items(), key=lambda kv: -kv[1])[:15]:
            lines.append(f"| {cid} | {CASES[cid]['request']} | {n} |")
        lines.append("")
    spend = date_dir / "spend.json"
    if spend.exists():
        lines.append(f"Total spent on {a.date}: US${json.loads(spend.read_text())['spent_usd']:.4f}")
    (date_dir / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    chart(summ, Path(a.chart), a.chart_run)
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
