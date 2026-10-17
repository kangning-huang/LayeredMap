"""Shared helpers for the converters of paper-only and unlicensed benchmarks (ShapefileGPT,
GeoJSON Agent, GIS Copilot, GeoQuery-inspired, SpatialQueryQA).

Each converter holds a hand-written table, one row per task, with the task summarised in our own
words, its classification and (for `now` and `out_of_scope` tasks) our own request text and plan.
Nothing is read from the downloaded originals at build time, so the converters run without them.

A few operations are missing from vocab.OPS; they are tagged with the nearest op and the record's
notes start with "VOCAB_GAP: <op>" so they can be re-tagged when the vocabulary grows:

    line_intersections → intersect          affine_transform → edit_attributes
    mean_center        → centroid           layer_extent     → convex_hull
    geometry_conversion (vertices to points, lines to polygons, points to lines, split
                         polygons by lines) → polygon_to_line
    polygon_orientation → calculate_geometry  eliminate_slivers → fix_geometry
    chart (bar, scatter, histogram output)   → out_of_scope "code" when the chart is the task
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
BENCH = HERE.parent
sys.path.insert(0, str(BENCH))

from benchlib import ASK, CANNOT, call, case, cond, dist, obj, sel, validate_record  # noqa: E402,F401


def rec(source, licence, sid, summary, category, coverage, *, missing=(), oos=None, confidence="high", notes="", cases=(), text_policy="own_words"):
    r = {
        "source": source,
        "source_id": str(sid),
        "licence": licence,
        "text_policy": text_policy,
        "summary": summary,
        "category": category,
        "coverage": coverage,
        "missing": list(missing),
        "confidence": confidence,
        "notes": notes,
        "cases": list(cases),
    }
    if oos:
        r["oos_reason"] = oos
    return retag(r)


STAND_IN = {
    "line_intersections": "intersect", "affine_transform": "edit_attributes", "mean_center": "centroid",
    "layer_extent": "convex_hull", "geometry_conversion": "polygon_to_line",
    "polygon_orientation": "calculate_geometry", "eliminate_slivers": "fix_geometry",
}


def retag(r: dict) -> dict:
    """Swap the stand-in op for the op vocab.OPS now has, and let the class follow the ops."""
    from vocab import OPS

    m = re.match(r"VOCAB_GAP: ([a-z_]+)", r["notes"])
    if not m or m.group(1) not in STAND_IN or m.group(1) not in OPS:
        return r
    op, old = m.group(1), STAND_IN[m.group(1)]
    if old in r["missing"]:
        r["missing"] = [op if x == old else x for x in r["missing"]]
    elif r["coverage"] in ("upstream", "new"):
        r["missing"].append(op)
    if r["coverage"] in ("upstream", "new"):
        r["coverage"] = "new" if any(OPS[x][1] == "new" for x in r["missing"]) else "upstream"
    return r


def refuse(cid, request, context, lang="en", notes=""):
    return case(cid, request, CANNOT, context=context, lang=lang, notes=notes)


def write(records: list[dict], name: str) -> None:
    errors = []
    for r in records:
        for e in validate_record(r):
            errors.append(f"[{r['source_id']}] {e}")
    if errors:
        print("\n".join(errors))
        raise SystemExit(f"{name}: {len(errors)} errors")
    out = BENCH / "sources" / name
    out.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in records), encoding="utf-8")
    by = {}
    for r in records:
        by[r["coverage"]] = by.get(r["coverage"], 0) + 1
    ncases = sum(len(r["cases"]) for r in records)
    nzh = sum(1 for r in records for c in r["cases"] if c["lang"] == "zh")
    print(f"{name}: {len(records)} records {by}; {ncases} cases ({nzh} zh)")
