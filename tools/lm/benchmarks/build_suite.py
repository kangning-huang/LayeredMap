"""Build the merged benchmark suite from the per-benchmark mappings.

    python -I tools/lm/benchmarks/build_suite.py            # suite.jsonl (balanced, committed)
    python -I tools/lm/benchmarks/build_suite.py --full     # also suite_full.jsonl (every case)

Each line of ``sources/*.jsonl`` is one benchmark task; its ``cases`` (for tasks LayeredMap can
answer now, vague tasks, and tasks it should refuse) become suite cases in the eval's format
(``tools/lm/llm_eval/cases.jsonl``: id, categories, lang, context, selection, request, expected,
alts, lenient_alts, notes) plus provenance fields the eval ignores (source, source_id, licence,
coverage, task_category). Run them with ``run_suite.py``.

Balancing: refusal-only sources are large (BFCL irrelevance, MapEval-API, UnivEARTH), so
``suite.jsonl`` keeps every answerable and every ask_user case, and at most ``--refusal-cap``
refusal cases per source (default 40), picked deterministically and spread over each source's
refusal reasons and languages. ``--full`` writes every case to ``suite_full.jsonl`` as well.

Cases with the same context, selection and request (an original request reused for several
tasks of one kind) are merged into one, keeping the first and listing every task's id.

Share-alike sources (licence containing "BY-SA") go to ``suite_sharealike.jsonl`` instead, so
the main suite carries no share-alike obligations (LICENSES.md).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # python -I drops the script's folder

import benchlib as b  # noqa: E402
from validate import validate_sources  # noqa: E402


def slug(source: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in source.lower()).strip("_").replace("__", "_")


def suite_case(r: dict, c: dict) -> dict:
    cats = ["bench", slug(r["source"]), r["category"], r["coverage"]]
    if c["lang"] == "zh":
        cats.append("zh")
    if c["expected"] == b.CANNOT:
        cats.append("refusal")
    elif c["expected"] == b.ASK:
        cats.append("vague")
    return {
        "id": c["id"],
        "categories": cats,
        "lang": c["lang"],
        "context": c["context"],
        "selection": c["selection"],
        "request": c["request"],
        "expected": c["expected"],
        "alts": c["alts"],
        "lenient_alts": c["lenient_alts"],
        "notes": c["notes"],
        "source": r["source"],
        "source_id": r["source_id"],
        "licence": r["licence"],
        "coverage": r["coverage"],
        "task_category": r["category"],
    }


def _key(c: dict) -> str:
    return json.dumps([c["context"], c["selection"], c["request"].strip()], ensure_ascii=False, sort_keys=True)


def _rank(cid: str) -> str:
    return hashlib.sha256(cid.encode("utf-8")).hexdigest()


def pick_refusals(cases: list[dict], cap: int | None) -> list[dict]:
    """Up to `cap` refusals, round-robin over (oos reason, language) groups in hash order."""
    if cap is None or len(cases) <= cap:
        return cases
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for c in sorted(cases, key=lambda c: _rank(c["id"])):
        groups[(c.get("_reason"), c["lang"])].append(c)
    keys = sorted(groups, key=lambda k: (str(k[0]), k[1]))
    out: list[dict] = []
    while len(out) < cap and any(groups.values()):
        for k in keys:
            if groups[k] and len(out) < cap:
                out.append(groups[k].pop(0))
    keep = {c["id"] for c in out}
    return [c for c in cases if c["id"] in keep]


def build(refusal_cap: int | None) -> tuple[list[dict], list[dict], dict]:
    main_cases: list[dict] = []
    sa_cases: list[dict] = []
    seen: dict[str, dict] = {}
    refusals: dict[str, list[dict]] = defaultdict(list)
    stats = {"tasks": 0, "merged": 0}
    for _name, _n, r in b.load_sources():
        stats["tasks"] += 1
        for c in r["cases"]:
            sc = suite_case(r, c)
            k = _key(sc)
            if k in seen:
                prev = seen[k]
                if prev["expected"] != sc["expected"]:
                    raise SystemExit(f"{sc['id']} repeats the request of {prev['id']} with a different plan")
                prev.setdefault("also", []).append(f"{r['source']}:{r['source_id']}")
                stats["merged"] += 1
                continue
            seen[k] = sc
            target = sa_cases if "BY-SA" in r["licence"].upper() else main_cases
            if sc["expected"] == b.CANNOT and target is main_cases:
                sc["_reason"] = r.get("oos_reason")
                refusals[r["source"]].append(sc)
            target.append(sc)
    kept = set()
    for _src, cs in refusals.items():
        kept |= {c["id"] for c in pick_refusals(cs, refusal_cap)}
    out = [c for c in main_cases if c["expected"] != b.CANNOT or c["id"] in kept]
    for c in out + sa_cases:
        c.pop("_reason", None)
    return out, sa_cases, stats


def write(path: Path, cases: list[dict]) -> None:
    path.write_text("".join(json.dumps(c, ensure_ascii=False) + "\n" for c in cases), encoding="utf-8")


def summary(cases: list[dict]) -> str:
    n = len(cases)
    zh = sum(c["lang"] == "zh" for c in cases)
    ref = sum(c["expected"] == b.CANNOT for c in cases)
    ask = sum(c["expected"] == b.ASK for c in cases)
    return f"{n} cases: {n - ref - ask} answerable, {ask} ask_user, {ref} refusals; {zh} Chinese"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--refusal-cap", type=int, default=40, help="refusal cases kept per source in suite.jsonl (0: no cap)")
    ap.add_argument("--full", action="store_true", help="also write suite_full.jsonl with every case")
    a = ap.parse_args(argv)
    errors = validate_sources()
    if errors:
        for e in errors[:50]:
            print(e, file=sys.stderr)
        print(f"{len(errors)} validation errors; fix them first (validate.py)", file=sys.stderr)
        return 1
    cases, sa, stats = build(a.refusal_cap or None)
    write(HERE / "suite.jsonl", cases)
    print(f"suite.jsonl: {summary(cases)} (from {stats['tasks']} tasks; {stats['merged']} repeated requests merged)")
    if sa:
        write(HERE / "suite_sharealike.jsonl", sa)
        print(f"suite_sharealike.jsonl: {summary(sa)}")
    if a.full:
        full, _, _ = build(None)
        write(HERE / "suite_full.jsonl", full)
        print(f"suite_full.jsonl: {summary(full)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
