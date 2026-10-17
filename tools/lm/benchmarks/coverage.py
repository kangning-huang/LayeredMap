"""Coverage tables for docs/lm/benchmarks/coverage.md, computed from sources/*.jsonl.

    python -I tools/lm/benchmarks/coverage.py           # rewrite the generated part of coverage.md
    python -I tools/lm/benchmarks/coverage.py --print   # print it instead

Only the text between the ``<!-- generated:start -->`` and ``<!-- generated:end -->`` markers
is rewritten; the analysis around it is written by hand.

Definitions:
* GIS tasks: every record except those in the categories ``irrelevant`` and ``knowledge``
  (BFCL's irrelevance items, chit-chat, general-knowledge questions).
* micro share: over all GIS tasks pooled; macro share: the mean of each benchmark's share, so
  a 500-task benchmark counts as much as a 30-task one.
* GIS-analysis benchmarks: ANALYSIS below (the refusal and live-service sets left out).
* A missing op's rank: the number of benchmarks needing it, then the number of tasks.
"""

from __future__ import annotations

import argparse
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # python -I drops the script's folder

import benchlib as b  # noqa: E402
from vocab import OPS  # noqa: E402

DOC = b.ROOT / "docs/lm/benchmarks/coverage.md"
START, END = "<!-- generated:start -->", "<!-- generated:end -->"
NON_GIS = {"irrelevant", "knowledge"}
# Benchmarks whose tasks are GIS analysis over the user's own data (not refusal or live-service
# sets: BFCL, ACEBench, T-Eval, MapEval-API, UnivEARTH, GeoLLM-Engine).
ANALYSIS = {
    "GeoBenchX", "GeoAnalystBench", "GeoAgentBench (pothos-dev)", "GeoAgentBench (Yu et al. 2026)", "GTChain-Eval",
    "GIS Copilot", "GeoJSON Agent", "ShapefileGPT", "GeoQuery (inspired-by)", "SpatialQueryQA",
}

# Which gate checks a task category feeds (spec › Gate 4, Gate 7).
GATES = {
    "spatial_join": ("Gate 4 numeric suite N1–N6 (counts, which area, what's inside) and hallway questions", "Gate 7 answerable"),
    "buffer": ("Gate 4 numeric suite (buffer areas), stress test 10,000-point buffer, '500 m → 1 km' change", "Gate 7 answerable"),
    "zonal_stats": ("Gate 4 numeric suite N4–N7 (zonal summaries, coverage), 'average surface temperature in my district'", "Gate 7 answerable"),
    "raster_sampling": ("Gate 4 numeric suite (add raster values)", "Gate 7 answerable"),
    "measurement": ("Gate 4 numeric suite (geodesic area/length)", "Gate 7 answerable"),
    "statistics": ("Gate 4 relevance and ranking snapshots ('most schools per km²')", "Gate 7 answerable"),
    "attribute_query": ("—", "Gate 7 answerable (filters)"),
    "map_making": ("—", "Gate 7 answerable (display calls)"),
    "vague": ("—", "Gate 7 question budget (ask_user)"),
}
REFUSAL_GATE = "Gate 7 honest refusal (≥95%)"


def records() -> list[dict]:
    return [r for _f, _n, r in b.load_sources()]


def pct(x: float) -> str:
    return f"{100 * x:.0f}%"


def table(header: list[str], rows: list[list]) -> str:
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" if i == 0 else "---:" for i in range(len(header))) + "|"]
    out += ["| " + " | ".join(str(x) for x in r) + " |" for r in rows]
    return "\n".join(out)


def generate() -> str:
    recs = records()
    by_src: dict[str, list[dict]] = defaultdict(list)
    for r in recs:
        by_src[r["source"]].append(r)
    parts = []

    # 1. Per benchmark
    rows = []
    for src in sorted(by_src, key=lambda s: -len(by_src[s])):
        rs = by_src[src]
        c = Counter(r["coverage"] for r in rs)
        cases = [x for r in rs for x in r["cases"]]
        zh = sum(x["lang"] == "zh" for x in cases)
        ref = sum(x["expected"] == b.CANNOT for x in cases)
        lic = sorted({r["licence"] for r in rs})
        pol = sorted({r["text_policy"] for r in rs})
        low = sum(r.get("confidence", "high") != "high" for r in rs)
        rows.append([src, "; ".join(lic), ", ".join(pol), len(rs), c["now"], c["upstream"], c["new"], c["out_of_scope"], low, f"{len(cases)} / {zh} / {ref}"])
    tot = Counter(r["coverage"] for r in recs)
    allc = [x for r in recs for x in r["cases"]]
    rows.append(["**All**", "", "", len(recs), tot["now"], tot["upstream"], tot["new"], tot["out_of_scope"], sum(r.get("confidence", "high") != "high" for r in recs),
                 f"{len(allc)} / {sum(x['lang'] == 'zh' for x in allc)} / {sum(x['expected'] == b.CANNOT for x in allc)}"])
    parts.append("### Tasks by benchmark and coverage class\n\n" + table(
        ["Benchmark", "Licence", "Text in git", "Tasks", "now", "upstream", "new", "out of scope", "medium/low confidence", "Cases (all / zh / refusals)"], rows))

    # 2. Overall shares
    gis = [r for r in recs if r["category"] not in NON_GIS]
    gsrc: dict[str, list[dict]] = defaultdict(list)
    for r in gis:
        gsrc[r["source"]].append(r)

    def shares(rs):
        n = len(rs) or 1
        c = Counter(r["coverage"] for r in rs)
        return {"now": c["now"] / n, "upstream": (c["now"] + c["upstream"]) / n, "new": c["new"] / n, "out_of_scope": c["out_of_scope"] / n}

    def view(rs):
        by: dict[str, list[dict]] = defaultdict(list)
        for r in rs:
            by[r["source"]].append(r)
        mi = shares(rs)
        ms = [shares(x) for x in by.values() if x]
        ma = {k: sum(m[k] for m in ms) / len(ms) for k in mi}
        return mi, ma, len(rs), len(by)

    ana = [r for r in gis if r["source"] in ANALYSIS]
    doable = [r for r in ana if r["coverage"] != "out_of_scope"]
    views = [("All GIS tasks", gis), ("GIS-analysis benchmarks only", ana), ("GIS-analysis tasks LayeredMap should do (out of scope removed)", doable)]
    rows = []
    for label, rs in views:
        mi, ma, n, nb = view(rs)
        rows.append([f"{label} ({n} tasks, {nb} benchmarks)", f"{pct(mi['now'])} / {pct(ma['now'])}", f"{pct(mi['upstream'])} / {pct(ma['upstream'])}",
                     f"{pct(mi['new'])} / {pct(ma['new'])}", f"{pct(mi['out_of_scope'])} / {pct(ma['out_of_scope'])}"])
    hi = [r for r in ana if r.get("confidence", "high") == "high" or r["coverage"] != "now"]
    strict_now = sum(r["coverage"] == "now" for r in hi) / (len(ana) or 1)
    parts.append(
        f"### Overall shares ({len(recs) - len(gis)} irrelevant or general-knowledge items excluded throughout)\n\n"
        "Each cell is micro (pooled) / macro (mean of benchmarks). *After wrapping* = now + upstream.\n\n"
        + table(["Pool", "Now", "After wrapping upstream", "Needs new algorithm", "Never (refuse)"], rows)
        + f"\n\nIn the GIS-analysis benchmarks, counting only high-confidence `now` calls, the micro share answerable now is {pct(strict_now)}."
    )

    # 3. Top missing ops
    need: dict[str, dict] = defaultdict(lambda: {"tasks": 0, "srcs": Counter()})
    asrc: dict[str, int] = Counter(r["source"] for r in gis if r["source"] in ANALYSIS)
    for r in gis:
        if r["coverage"] in ("upstream", "new"):
            for op in dict.fromkeys(r.get("missing") or []):
                need[op]["tasks"] += 1
                need[op]["srcs"][r["source"]] += 1
    ranked = []
    for op, d in need.items():
        macro_share = sum(d["srcs"][s] / asrc[s] for s in asrc) / len(asrc)
        ranked.append((len(d["srcs"]), d["tasks"], macro_share, op))
    ranked.sort(reverse=True)
    rows = []
    for i, (nsrc, ntask, ms, op) in enumerate(ranked, 1):
        cat, up, desc = OPS[op]
        rows.append([i, f"`{op}`", desc, cat, up if up == "new" else f"`{up}`", ntask, nsrc, pct(ms) if ms >= 0.005 else f"{100 * ms:.1f}%"])
    parts.append("### Missing operations, ranked\n\nRanked by the number of benchmarks with at least one task needing the op, then by tasks. "
                 "A task needing several ops counts for each. *Macro share*: the mean, over the 10 GIS-analysis benchmarks, of the share of each one's tasks needing the op.\n\n"
                 + table(["#", "Op", "What", "Family", "Wrap (RustGIS) or new", "Benchmarks", "Tasks", "Macro share"], [[r[0], r[1], r[2], r[3], r[4], r[6], r[5], r[7]] for r in rows]))

    # 4. Category x coverage
    cat = defaultdict(Counter)
    for r in recs:
        cat[r["category"]][r["coverage"]] += 1
    rows = [[k, sum(v.values()), v["now"], v["upstream"], v["new"], v["out_of_scope"]] for k, v in sorted(cat.items(), key=lambda kv: -sum(kv[1].values()))]
    parts.append("### Tasks by operation family\n\n" + table(["Family", "Tasks", "now", "upstream", "new", "out of scope"], rows))

    # 5. Out-of-scope reasons
    oos = Counter(r.get("oos_reason") for r in recs if r["coverage"] == "out_of_scope")
    parts.append("### Why tasks are out of scope\n\n" + table(["Reason", "Tasks"], [[k, v] for k, v in oos.most_common()]))

    # 6. Gates
    suite = b.HERE / "suite.jsonl"
    sc = Counter()
    if suite.exists():
        import json

        for line in suite.read_text(encoding="utf-8").splitlines():
            c = json.loads(line)
            key = "refusal" if c["expected"] == b.CANNOT else c["task_category"]
            sc[key] += 1
    rows = [[k, g4, g7, sc.get(k, 0)] for k, (g4, g7) in GATES.items()]
    rows.append(["any family, out of scope", "—", REFUSAL_GATE, sc.get("refusal", 0)])
    parts.append("### Benchmark families and gate checks\n\n" + table(["Family", "Gate 4", "Gate 7", "Suite cases"], rows))
    return "\n\n".join(parts) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--print", action="store_true")
    a = ap.parse_args(argv)
    text = generate()
    if a.print:
        print(text)
        return 0
    doc = DOC.read_text(encoding="utf-8") if DOC.exists() else f"# Coverage\n\n{START}\n{END}\n"
    if START not in doc or END not in doc:
        raise SystemExit(f"{DOC} lacks the generated-block markers")
    head, rest = doc.split(START, 1)
    _, tail = rest.split(END, 1)
    DOC.write_text(head + START + "\n\n" + text + "\n" + END + tail, encoding="utf-8")
    print(f"updated {DOC}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
