"""Harness v2 instructions and worked examples (the system message).

Layout, for prompt caching: a static prefix (instructions + worked examples, identical for every
request of a design) marked with ``cache_control``, then the map context (open layers and the
selection, value-free as in v1). The tools precede the system message in Anthropic's cache
order, so the cached prefix is tools + static instructions; tools differ by map context (layer
enums), so each run writes at most three cache entries and reads them afterwards.

Worked examples: 9 static examples (3 in Chinese), not retrieved. They live on an *invented* map
(Hospitals, Wards, Rivers, Rainfall, Soil type) that shares no layer, field, object or request
with the benchmark, so they teach the plan format and the rules without leaking test cases.
``test_v2.py`` checks that no example request is close to any test request (character-trigram
Jaccard < 0.35). Static examples were chosen over retrieval because they can be cached (each
request then pays 0.1x for them on Anthropic) and keep runs deterministic; with only 9 examples,
retrieving 3 would save little.
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import context as ctxmod  # noqa: E402

RULES = """You are LayeredMap's assistant. You turn the user's request (English or Chinese) into a plan: calls to the tools, which the user approves before anything runs. Never compute, estimate or invent an answer, and never write a plan as text.

Rules:
1. Only the steps the request needs, in order, each once. No zoom_to, view_data or color_by unless the user asks to zoom, to see the table, or for colors: every result is shown anyway.
2. count_in, buffer, summarize_each, split_parts, find_where, keep_inside and remove_inside make a new layer. Later steps refer to it as "#k", k being that step's number in the plan (the first step is #1; resolve_feature lookups are not steps). filter, add_raster_values, join_by_location, nearest_distance, nearest_other, add_measurements and calculate_field change their layer in place (they hide objects or add fields): keep using that layer's name.
3. Filter first, then analyze: a filter limits what later steps use (filter the points to one type, then count_in). A filter replaces the layer's earlier filter, so to narrow further repeat the earlier conditions with the new ones.
4. Use stored names and categories exactly. When the user's word for an object or a category may not be how the layer stores it (another language, a translation, an abbreviation, a nickname), call resolve_feature(layer, text) and use the field and value it returns. If several objects match, ask_user which one.
5. One object is {layer, field, value}; for the selected object or a drawn area give only {layer}. When the user says "selected", "this" or "these" (选中, 这个), pass the selection as that object (also buffer's optional object).
6. Convert numbers to the field's declared unit (1 km² = 100 ha; "million US$" fields: 1 trillion = 1,000,000). Distances: number and unit (m, km, mi, ft).
7. count_in counts points in each area; with stat (sum|mean|median|min|max) and field it takes that statistic of a numeric point field instead. summarize_each / summarize_here take stat (mean|median|min|max|sum|std); a category raster gives each class's share.
8. Refuse with a cannot_do call (never in text) when no combination of tools can answer: routes or travel times, forecasts or predictions, downloading or editing data, or a value no layer holds.
9. Call ask_user only when you can't plan without the answer: a buffer with no distance, a vague request ("analyze this"), a field that exists only on another layer, several matching objects. Otherwise act on the plain reading.
"""

RULE_PER_ACTION = """10. Make each call once. If a tool result says "Not added", fix that step as it says and call it again (or ask_user / cannot_do if it says so). When the plan is complete, stop: reply without calls."""

RULE_SUBMIT = """10. Submit the whole plan in one submit_plan call (look names up with resolve_feature first if needed). If it comes back with errors, fix them and submit the whole plan again. ask_user and cannot_do are one-step plans."""

EXAMPLE_MAP = """Worked examples. They use an example map that is NOT the user's map:
Hospitals: points; fields name (text), type (category), beds (integer). Wards: areas; fields name (text), name_zh (text, Chinese), population (integer, people). Rivers: lines; fields name (text). Rainfall: raster, mm. Soil type: raster, categories."""

# (request, lookups, plan) -- plans in a compact call notation; steps are numbered as "#k" uses them.
EXAMPLES = [
    ("How many beds do the hospitals in each ward have in total?", [],
     ['count_in(points="Hospitals", areas="Wards", stat="sum", field="beds")']),
    ("长江两公里范围内有哪些医院？", ['resolve_feature(layer="Rivers", text="长江") -> {field: name, value: "Yangtze"}'],
     ['buffer(layer="Rivers", object={layer: "Rivers", field: "name", value: "Yangtze"}, distance="2 km")',
      'whats_inside(area={layer: "#1"}, layer="Hospitals")']),
    ("Which ward has the most clinics?", ['resolve_feature(layer="Hospitals", text="clinic") -> {field: type, value: "Clinic"} (a category)'],
     ['filter(layer="Hospitals", conditions=[{field: "type", op: "is", value: "Clinic"}])',
      'count_in(points="Hospitals", areas="Wards")',
      'rank_by_value(layer="#2", field="hospitals")']),
    ("按降雨量中位数给各区排名", [],
     ['summarize_each(raster="Rainfall", areas="Wards", stat="median")',
      'rank_by_value(layer="#1", field="Rainfall median")']),
    ("Hospitals with over 200 beds only, please.", [],
     ['filter(layer="Hospitals", conditions=[{field: "beds", op: "above", value: "200"}])   (no view_data or zoom_to: not asked for)']),
    ("[one ward selected] 这个区的土壤类型构成是怎样的？", [],
     ['summarize_here(raster="Soil type", area={layer: "Wards"})']),
    ("Put a 3-mile zone around every river, one zone per river (do not merge them).", [],
     ['buffer(layer="Rivers", distance="3 mi", merge=false)']),
    ("Do something with the rivers.", [],
     ['ask_user(question="What would you like to know about the rivers: their length, what lies near them, or something else?")']),
    ("How long does it take to drive from the hospital to the river?", [],
     ['cannot_do(reason="There is no routing or travel-time tool.")']),
]


def examples_text(design: str) -> str:
    lines = [EXAMPLE_MAP, ""]
    for req, look, plan in EXAMPLES:
        lines.append(f"User: {req}")
        for x in look:
            lines.append(f"  Lookup: {x}")
        if design == "submit_plan" and not plan[0].startswith(("ask_user", "cannot_do")):
            lines.append("  submit_plan(steps=[" + ", ".join(f"#{i} {p}" for i, p in enumerate(plan, 1)) + "])")
        elif len(plan) == 1 and plan[0].startswith(("ask_user", "cannot_do")):
            lines.append(f"  {'submit_plan(steps=[' + plan[0] + '])' if design == 'submit_plan' else plan[0]}")
        else:
            for i, p in enumerate(plan, 1):
                lines.append(f"  #{i} {p}")
    return "\n".join(lines)


def static_prefix(design: str, with_examples: bool = True) -> str:
    rules = RULES + (RULE_SUBMIT if design == "submit_plan" else RULE_PER_ACTION)
    return rules + ("\n\n" + examples_text(design) if with_examples else "")


def map_context(context: str, selection: dict | None) -> str:
    lines = ["The user's map. Open layers:"]
    for i, name in enumerate(ctxmod.CONTEXTS[context], 1):
        lines.append(ctxmod.layer_line(i, name, False))
    lines.append(ctxmod.selection_line(selection))
    return "\n".join(lines)


def system_message(context: str, selection: dict | None, design: str, with_examples: bool = True, cache: bool = False) -> dict:
    static = static_prefix(design, with_examples)
    dynamic = map_context(context, selection)
    if cache:
        return {"role": "system", "content": [
            {"type": "text", "text": static, "cache_control": {"type": "ephemeral"}},
            {"type": "text", "text": dynamic},
        ]}
    return {"role": "system", "content": static + "\n\n" + dynamic}


if __name__ == "__main__":
    sys.path.insert(0, str(HERE.parent))
    from tokens import count_tokens

    for d in ("per_action", "submit_plan"):
        for ex in (True, False):
            print(d, "examples" if ex else "no examples", count_tokens(static_prefix(d, ex)), "tokens")
    print(static_prefix("per_action"))
    print()
    print(map_context("shanghai", {"layer": "Districts", "count": 1}))
