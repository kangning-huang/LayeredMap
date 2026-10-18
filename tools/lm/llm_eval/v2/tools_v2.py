"""Harness v2 tool definitions: built per map context, with enums for layer names.

Generated from the same app sources as v1 (``tools.action_table()``: the connected actions and
their inputs in the runner's order), plus:

* the statistics menus: ``count_in`` takes ``stat`` (count|sum|mean|median|min|max) and ``field``
  (a numeric point field); ``summarize_each/here/drawn`` take ``stat``
  (mean|median|min|max|sum|std) -- ``POINT_STATS`` and ``RASTER_STATS`` in actions.rs;
* layer-name enums built from the open layers, per slot kind: a raster slot lists only rasters,
  a points slot only point layers, an areas slot area layers, drawn areas and step outputs
  ("#1".."#6"); ``additionalProperties: false`` everywhere;
* ``resolve_feature(layer, text)`` (owner decision 2026-10-17);
* rewritten descriptions (when to use, what the step makes, category rasters give shares).

Two designs:

* ``per_action``: one tool per action, display and meta tools (v1 design A, refined);
* ``submit_plan``: one ``submit_plan(steps[])`` tool, each step ``{action, ...named inputs}``,
  plus ``resolve_feature``.

``strict: true`` is not set on the functions: OpenAI-style strict mode requires every property
to be required (optional inputs would have to become nullable unions), which several providers
on OpenRouter reject. Enums and ``additionalProperties: false`` carry most of the benefit, and
the plan checker catches the rest.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import context as ctxmod  # noqa: E402
import tools as toolmod  # noqa: E402

REFS = [f"#{i}" for i in range(1, 7)]
POINT_STATS = ["count", "sum", "mean", "median", "min", "max"]
RASTER_STATS = ["mean", "median", "min", "max", "sum", "std"]
OPS = ["is", "is_not", "contains", "starts_with", "ends_with", "is_empty", "above", "below", "at_least", "at_most", "between", "before", "after", "any_of"]
DISPLAY = ["filter", "color_by", "view_data", "zoom_to"]
META = ["ask_user", "cannot_do"]
# Actions whose step makes a new layer ("#k"); the others change their layer in place or make a card.
NEW_LAYER_ACTIONS = {"count_in", "buffer", "summarize_each", "split_parts", "find_where", "keep_inside", "remove_inside"}

S = {"type": "string"}


def stat_actions() -> dict[str, list[str]]:
    """Which actions take ``stat``, read from the runner (fails loudly if the menus move)."""
    src = toolmod.RUNNER.read_text(encoding="utf-8")
    pts = re.findall(r'\("([a-z]+)", "[A-Za-z ]+"\)', src.split("pub const POINT_STATS")[1].split(";")[0])
    ras = re.findall(r'\("([a-z]+)", "[A-Za-z ]+"\)', src.split("pub const RASTER_STATS")[1].split(";")[0])
    assert pts == POINT_STATS and ras == RASTER_STATS, (pts, ras)
    return {"count_in": pts, "summarize_each": ras, "summarize_here": ras, "summarize_drawn": ras}


def layer_sets(context: str) -> dict[str, list[str]]:
    names = ctxmod.CONTEXTS[context]
    by = {n: ctxmod.LAYERS[n]["kind"] for n in names}
    return {
        "raster": [n for n, k in by.items() if k == "raster"],
        "points": [n for n, k in by.items() if k == "points"],
        "lines": [n for n, k in by.items() if k == "lines"],
        "areas": [n for n, k in by.items() if k in ("areas", "drawn area")] + REFS,
        "vector": [n for n, k in by.items() if k != "raster"] + REFS,
    }


DESCRIPTIONS = {
    "count_in": (
        "Count points in each area of an area layer, or take a statistic of a numeric point field per area "
        "(stat sum|mean|median|min|max with field, e.g. total students per district: stat 'sum', field 'students'). "
        "New layer: the areas with field '<points layer, lowercase>' (Schools: 'schools') and, with a statistic, "
        "'<stat> <field>' ('sum students', 'mean students', 'median students', 'minimum students', 'maximum students'). "
        "It counts any layer's objects: areas and lines (islands, lakes, roads) count by a point inside each. "
        "To count what lies near each point (islands within 100 km of each city), first buffer the points with merge false, "
        "then count_in(points=<the layer to count>, areas='#k')."
    ),
    "count_here": "Count points inside one area (a selected, named or drawn area).",
    "which_container": "Which area (of an area layer) one point is in.",
    "whats_inside": "Show the objects of a layer that lie inside one area (lists them). Not for counting per area.",
    "summarize_data_here": "Counts and value summaries of a layer's objects inside one area.",
    "buffer": (
        "The area within a distance of a whole layer, or of one object of it (give object: a named object, or {layer} "
        "alone for the selected object). New area layer. "
        "merge false keeps each object's buffer separate."
    ),
    "measure_area": "Measure one area object (geodesic).",
    "measure_length": "Measure one line object (geodesic).",
    "summarize_each": (
        "Summarize a raster in each area of an area layer. New layer: the areas with fields '<raster> mean', "
        "'<raster> median', '<raster> minimum', '<raster> maximum', '<raster> sum', '<raster> std dev' (Heat: 'Heat mean'); "
        "stat picks the headline (default mean). A category raster (e.g. land cover) gives each class's share "
        "of every area instead."
    ),
    "summarize_here": "Summarize a raster inside one area (stat default mean). A category raster gives class shares.",
    "summarize_drawn": "Summarize a raster in an area the user will draw (omit area), or in a drawn area.",
    "add_raster_values": "Add the raster's value at each (shown) point as a field named after the raster (Heat: 'Heat'); hidden points stay blank. Changes the points layer; no new layer.",
    "rank_by_value": "Rank a layer's objects by a numeric field: highest first, or with order 'lowest' lowest first (the coldest, smallest, fewest, lowest).",
    "calculate_field": (
        "Add a number field calculated from two values: a (a number field) op (add|subtract|multiply|divide) b "
        "(a number field or a number), e.g. density = population divide area_km2. Blank inputs or division by "
        "zero give blanks. Adds the field 'name' (default 'a op b') to the layer; no new layer."
    ),
    "split_parts": "Split a line or area layer's multi-part features into one feature per part (e.g. separate islands). New layer with a 'part' field.",
    "find_where": (
        "Find the areas where every raster condition holds (e.g. elevation at least 3000 and rainfall at least 500). "
        "raster: the first raster; conditions: [{field: <raster name>, op: at_least | at_most | between, value, value2}] in the raster's unit. "
        "New area layer, one area per connected group of cells, largest first, with fields name, area_km2, cells and '<raster> mean'."
    ),
    "keep_inside": "Keep only the part of a layer inside an area layer's (shown) areas (clip). New layer with the same fields (an area_km2 field is copied, not recomputed).",
    "remove_inside": "Remove the part of a layer inside an area layer's (shown) areas (erase; 'away from', 'outside'). New layer with the same fields.",
    "nearest_distance": (
        "Add to each object of a layer the geodesic distance in km to the nearest (shown) object of another layer (other), and that "
        "object's name. Adds fields 'km to nearest <other, singular>' and 'nearest <other, singular>' (other Rivers: 'km to nearest river'). "
        "Changes the layer; no new layer."
    ),
    "nearest_other": (
        "Add to each object the distance in km to the nearest other object of the same layer (isolation), and its name: fields "
        "'km to nearest other <layer, singular>' ('km to nearest other hospital'). Only the shown objects count as others, so run it "
        "before filtering that layer. Changes the layer; no new layer."
    ),
    "join_by_location": (
        "Add to each object of a layer the fields of the area it lies in (spatial join; lines and areas use a point on them): layer, "
        "areas, fields (comma list of the areas' fields; all by default). The new fields keep the areas' field names. Changes the layer; no new layer."
    ),
    "add_measurements": (
        "Add area_km2 and perimeter_km (areas), length_km (lines), and longitude and latitude (degrees, of a point on each object) as "
        "fields. A field name the layer already has gets ' 2' (area_km2 2). Changes the layer; no new layer."
    ),
}

# In the submit_plan step schema, calculate_field's `a` and `b` are `first` and `second`: one
# step object holds every action's inputs, and a property name that is the start of another
# (`a` of `area`/`areas`, `b` of `band`) was never produced by GLM 5.3 Flash through its provider's
# constrained decoding (it wrote `area: {...}` instead), in 61 of 61 calculate_field steps
# (tools/lm/puzzles results, 2026-10-17); with `first`/`second` it used them in 64 of 64.
SUBMIT_RENAMES = {"first": "a", "second": "b"}
SUBMIT_CALCULATE = (
    "Add a number field calculated from two values: first (a number field) op (add|subtract|multiply|divide) second "
    "(a number field or a number), e.g. density = population divide area_km2. Blank inputs or division by zero give "
    "blanks. Adds the field 'name' (default 'first op second') to the layer; no new layer."
)

OBJ_NOTE = "One object: {layer, field, value} with value as stored (resolve_feature finds it); omit field and value for the selected object, a drawn area, or the layer's only shown object."


def _enum(values: list[str]) -> dict:
    """A string enum; a plain string when the map has no layer of that kind (an empty enum is an
    invalid schema, and some providers reject the whole request for it)."""
    return {"type": "string", "enum": values} if values else dict(S)


def _object(layers: list[str], optional: bool = False) -> dict:
    o = {
        "type": "object",
        "properties": {"layer": _enum(layers), "field": S, "value": S},
        "required": ["layer"],
        "additionalProperties": False,
        "description": OBJ_NOTE + (" Optional." if optional else ""),
    }
    return o


def fn(name: str, description: str, props: dict, required: list[str]) -> dict:
    params: dict = {"type": "object", "properties": props, "additionalProperties": False}
    if required:
        params["required"] = required
    return {"type": "function", "function": {"name": name, "description": description, "parameters": params}}


def slot_schema(role: str, kind: str, action: str, L: dict) -> dict:
    """Schema of one runner input."""
    if kind == "layer":
        pool = {"points": L["points"], "areas": L["areas"], "raster": L["raster"]}.get(role, L["vector"])
        if (action, role) == ("count_in", "points"):
            pool = L["vector"]
        return _enum(pool)
    obj_pool = {"point": L["points"], "line": L["lines"], "area": L["areas"]}.get(role, L["vector"])
    return _object(obj_pool, optional=kind.endswith("?"))


def param_schema(p: str, action: str, stats: dict) -> dict:
    if p == "distance":
        return {"type": "string", "description": "number and unit: '500 m', '2 km', '3 mi', '100 ft'"}
    if p == "merge":
        return {"type": "boolean", "description": "merge overlapping buffers (default true)"}
    if p == "band":
        return {"type": "integer", "description": "1-based, default 1"}
    if p == "stat":
        return {"type": "string", "enum": stats[action]}
    if p == "field":
        if action == "count_in":
            return {"type": "string", "description": "numeric field of the points, for a stat other than count"}
        return S
    if p in toolmod.PARAM_SCHEMA:
        return toolmod.PARAM_SCHEMA[p]
    raise KeyError(p)


def action_params(action: str, stats: dict) -> list[str]:
    ps = list(toolmod.PARAMS_OF_ACTION.get(action, []))
    if action in stats:
        ps.append("stat")
    if action == "count_in":
        ps.append("field")
    return ps


def required_of(a: dict) -> list[str]:
    req = [n for n, k in a["roles"] if not k.endswith("?")]
    if a["id"] == "rank_by_value":
        req.append("field")
    if a["id"] == "buffer":
        req.append("distance")
    return req


def condition_schema() -> dict:
    return {
        "type": "object",
        "properties": {
            "field": S,
            "op": {"type": "string", "enum": OPS},
            "value": {"type": "string", "description": "number (in the field's unit) or a stored text/category value"},
            "value2": {"type": "string", "description": "upper end for between"},
            "values": {"type": "array", "items": S, "description": "for any_of"},
        },
        "required": ["field", "op"],
        "additionalProperties": False,
    }


def display_tools(L: dict) -> list[dict]:
    vec = {"type": "string", "enum": L["vector"]}
    return [
        fn("filter", "Show only a layer's objects meeting all conditions (replaces its filter; [] clears it). Later steps use only the shown objects. Changes the layer; no new layer.",
           {"layer": vec, "conditions": {"type": "array", "items": condition_schema()}}, ["layer", "conditions"]),
        fn("color_by", "Color a layer by a field. Only when the user asks for colors or a map styled by a value.", {"layer": vec, "field": S}, ["layer", "field"]),
        fn("view_data", "Open a layer's table. Only when the user asks for the table.", {"layer": vec}, ["layer"]),
        fn("zoom_to", "Zoom to a layer or one object. Only when the user asks to zoom or go to it.", {"layer": vec, "field": S, "value": S}, ["layer"]),
    ]


def meta_tools() -> list[dict]:
    return [
        fn("ask_user", "Ask the user one question, only when the request can't be planned without the answer (missing distance, unclear layer, several objects match).",
           {"question": S, "choices": {"type": "array", "items": S}}, ["question"]),
        fn("cannot_do", "Refuse: no combination of these tools answers the request (routing, forecasts, downloads, editing data...). Say why.", {"reason": S}, ["reason"]),
    ]


def resolve_tool(L: dict) -> dict:
    named = [n for n in L["vector"] if not n.startswith("#")]
    return fn(
        "resolve_feature",
        "Look up how a layer stores a name or category the user wrote (another language, abbreviation, nickname): returns up to 5 stored values with their field. A lookup, not a plan step; it doesn't take a step number.",
        {"layer": {"type": "string", "enum": named}, "text": {"type": "string", "description": "the user's words"}},
        ["layer", "text"],
    )


def per_action_tools(context: str, with_resolve: bool = True) -> list[dict]:
    L = layer_sets(context)
    stats = stat_actions()
    tools = []
    for a in toolmod.action_table():
        props = {}
        for role, kind in a["roles"]:
            props[role] = slot_schema(role, kind, a["id"], L)
        for p in action_params(a["id"], stats):
            props[p] = param_schema(p, a["id"], stats)
        tools.append(fn(a["id"], DESCRIPTIONS[a["id"]], props, required_of(a)))
    tools += display_tools(L) + meta_tools()
    if with_resolve:
        tools.append(resolve_tool(L))
    return tools


def submit_plan_tools(context: str, with_resolve: bool = True) -> list[dict]:
    """One ``submit_plan(steps[])``: each step is {action, ...its named inputs}."""
    L = layer_sets(context)
    stats = stat_actions()
    table = toolmod.action_table()
    lines = []
    for a in table:
        ins = [f"{r}{'?' if k.endswith('?') else ''}" for r, k in a["roles"]] + [f"{p}?" for p in action_params(a["id"], stats) if p not in required_of(a)]
        ins += [p for p in action_params(a["id"], stats) if p in required_of(a)]
        if a["id"] == "calculate_field":
            lines.append(f"- calculate_field(layer, first, op, second, name?): {SUBMIT_CALCULATE}")
            continue
        lines.append(f"- {a['id']}({', '.join(ins)}): {DESCRIPTIONS[a['id']]}")
    lines += [
        "- filter(layer, conditions): show only objects meeting all conditions (it replaces the layer's earlier filter, so repeat earlier conditions to keep them); changes the layer, no new layer. Ops: is, is_not, contains, starts_with, ends_with, is_empty, above (>), below (<), at_least (>=), at_most (<=), between (inclusive), before, after, any_of.",
        "- color_by(layer, field) / view_data(layer) / zoom_to(layer, field?, value?): only when asked for.",
        "- ask_user(question, choices?): one question, only when needed. Alone in the plan.",
        "- cannot_do(reason): the tools can't answer. Alone in the plan.",
    ]
    all_actions = [a["id"] for a in table] + DISPLAY + META
    step_props = {
        "action": {"type": "string", "enum": all_actions},
        "points": {**_enum(L["vector"]), "description": "count_in: the layer to count (points; areas and lines, e.g. islands, count by a point inside each); add_raster_values: a point layer"},
        "areas": {"type": "string", "enum": L["areas"]},
        "raster": _enum(L["raster"]),
        "layer": {"type": "string", "enum": L["vector"]},
        "point": _object(L["points"]),
        "line": _object(L["lines"]),
        "area": _object(L["areas"]),
        "object": _object(L["vector"], optional=True),
        "distance": param_schema("distance", "buffer", stats),
        "merge": param_schema("merge", "buffer", stats),
        "band": param_schema("band", "summarize_each", stats),
        "stat": {"type": "string", "enum": sorted(set(POINT_STATS) | set(RASTER_STATS)), "description": "count_in: count|sum|mean|median|min|max; summarize_*: mean|median|min|max|sum|std"},
        "field": S,
        "order": {"type": "string", "enum": ["highest", "lowest"], "description": "rank_by_value: highest first (default) or lowest first"},
        "other": {"type": "string", "enum": L["vector"], "description": "nearest_distance: the layer to measure to"},
        "fields": {"type": "string", "description": "join_by_location: comma list of the areas' fields to add (all by default)"},
        "first": {"type": "string", "description": "calculate_field: the first value, a number field of the layer (first op second)"},
        "op": {"type": "string", "enum": ["add", "subtract", "multiply", "divide"]},
        "second": {"type": "string", "description": "calculate_field: the second value, a number field or a number"},
        "name": {"type": "string", "description": "calculate_field: the new field's name"},
        "conditions": {"type": "array", "items": condition_schema()},
        "value": S,
        "question": S,
        "choices": {"type": "array", "items": S},
        "reason": S,
    }
    desc = ("Submit the whole plan at once: the steps in order. Nothing runs until the user approves it. "
            "A step that makes a new layer (" + ", ".join(sorted(NEW_LAYER_ACTIONS)) + ") is referred to by later steps as \"#k\" "
            "(k = its position, from 1). Steps:\n" + "\n".join(lines))
    tools = [fn("submit_plan", desc, {"steps": {"type": "array", "minItems": 1, "items": {"type": "object", "properties": step_props, "required": ["action"], "additionalProperties": False}}}, ["steps"])]
    if with_resolve:
        tools.append(resolve_tool(L))
    return tools


DESIGNS = {"per_action": per_action_tools, "submit_plan": submit_plan_tools}


def steps_to_calls(steps) -> list[dict]:
    """submit_plan steps -> canonical calls {name, args}. calculate_field's ``first`` and ``second``
    become the runner's ``a`` and ``b``."""
    out = []
    for s in steps if isinstance(steps, list) else []:
        if not isinstance(s, dict):
            continue
        s = dict(s)
        name = str(s.pop("action", "") or "")
        for new, old in SUBMIT_RENAMES.items():
            if new in s:
                v = s.pop(new)
                s.setdefault(old, v)
        out.append({"name": name, "args": s})
    return out


if __name__ == "__main__":
    import json

    sys.path.insert(0, str(HERE.parent))
    from tokens import count_tokens

    for d, make in DESIGNS.items():
        for c in ctxmod.CONTEXTS:
            t = make(c)
            s = json.dumps(t, ensure_ascii=False, separators=(",", ":"))
            print(f"{d:12s} {c:15s} {len(t):2d} tools ~{count_tokens(s)} tokens")
