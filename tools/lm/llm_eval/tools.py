"""Tool definitions for the LayeredMap Pro assistant benchmark.

The analysis tools are generated from the app's own sources so they stay in sync:

* which actions are connected: the ``ACTIONS`` list in ``crates/lm-jobs/src/actions.rs``;
* each action's inputs, in the runner's order: the module-doc table in the same file;
* labels, methods and decisions: ``crates/lm-actions/rules.toml``.

Two designs are built:

* ``per_action`` (design A): one function tool per connected action;
* ``single`` (design B): one ``run_action`` tool whose ``action`` is an enum.

Both designs share the display tools (filter, color_by, view_data, zoom_to) and the two
meta tools (ask_user, cannot_do).

``python -I tools.py`` writes ``tools_per_action.json`` and ``tools_single.json`` next to this
file and prints their sizes.
"""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RULES = ROOT / "crates/lm-actions/rules.toml"
RUNNER = ROOT / "crates/lm-jobs/src/actions.rs"

# ---------------------------------------------------------------------------------------------
# Reading the sources


def connected_actions(src: str | None = None) -> list[str]:
    """The ids in `pub const ACTIONS: &[&str] = &[ ... ];`."""
    src = src if src is not None else RUNNER.read_text(encoding="utf-8")
    m = re.search(r"pub const ACTIONS: &\[&str\] = &\[(.*?)\];", src, re.S)
    if not m:
        raise ValueError("ACTIONS not found in actions.rs")
    return re.findall(r'"([a-z_]+)"', m.group(1))


def runner_inputs(src: str | None = None) -> dict[str, list[str]]:
    """The module-doc table `//! | `count_in` | points, areas |` as {id: [phrase, ...]}."""
    src = src if src is not None else RUNNER.read_text(encoding="utf-8")
    out: dict[str, list[str]] = {}
    for line in src.splitlines():
        m = re.match(r"//! \| (`[a-z_]+`(?:, `[a-z_]+`)*) \| (.+?) \|\s*$", line)
        if not m:
            continue
        ids = re.findall(r"`([a-z_]+)`", m.group(1))
        phrases = [p.strip() for p in re.split(r",(?![^(]*\))", m.group(2))]
        for i in ids:
            out[i] = phrases
    return out


def rules() -> dict[str, dict]:
    data = tomllib.loads(RULES.read_text(encoding="utf-8"))
    return {a["id"]: a for a in data["action"]}


# ---------------------------------------------------------------------------------------------
# Roles: each runner input phrase becomes a named argument.
#
# kind "layer": a layer's display name; kind "object": {layer, field, value} (one object).
# A phrase we don't recognize raises, so a change to the runner's table is noticed.

ROLE_OF_PHRASE = {
    # (action-independent phrases)
    "points": ("points", "layer"),
    "areas": ("areas", "layer"),
    "raster": ("raster", "layer"),
    "the area (one row)": ("area", "object"),
    "points (the selected rows)": ("point", "object"),
    "the layer to look in": ("layer", "layer"),
    "the layer (or its selected rows)": ("layer", "layer+object"),
    "the layer; parameter `field`": ("layer", "layer"),
    "the layer": ("layer", "layer"),
    "the layer (parameters `a`, `op`, `b`, `name`)": ("layer", "layer"),
    "the rasters (parameter `conditions`)": ("raster", "layer"),
    "the areas": ("areas", "layer"),
    "the other layer": ("other", "layer"),
}


def roles_for(action: str, phrases: list[str]) -> list[tuple[str, str]]:
    roles = []
    for p in phrases:
        if action == "summarize_drawn" and p == "the area (one row)":
            # The user draws it after the plan is accepted, unless one is drawn already.
            roles.append(("area", "object?"))
        elif p == "the layer (one row)":
            roles.append(("line" if action == "measure_length" else "area", "object"))
        elif p in ROLE_OF_PHRASE:
            name, kind = ROLE_OF_PHRASE[p]
            if kind == "layer+object":
                roles.append((name, "layer"))
                roles.append(("object", "object?"))
            else:
                roles.append((name, kind))
        else:
            raise ValueError(f"unknown runner input phrase for {action}: {p!r}")
    return roles


# Parameters from the runner's module docs: `distance`, `merge`, `band`, `field`.
PARAMS_OF_ACTION = {
    "buffer": ["distance", "merge"],
    "summarize_each": ["band"],
    "summarize_here": ["band"],
    "summarize_drawn": ["band"],
    "add_raster_values": ["band"],
    "rank_by_value": ["field"],
    "calculate_field": ["a", "op", "b", "name"],
    "find_where": ["conditions"],
}

# What each action makes, so a later call can use it. Not in rules.toml (the runner builds
# these names), so written here from actions.rs; tests check the runner still uses them.
OUTPUT_NOTE = {
    "count_in": "New layer: the areas with a count field named after the points layer in lowercase (Schools: 'schools').",
    "buffer": "New area layer.",
    "summarize_each": "New layer: the areas with fields '<raster name> mean', '… min', '… max' (Heat: 'Heat mean').",
    "add_raster_values": "Adds a field named after the raster to the points (Heat: 'Heat').",
    "whats_inside": "Shows only the objects inside.",
    "summarize_drawn": "Omit area to let the user draw one.",
}

TOKEN_WORDS = {
    "[Schools]": "points",
    "[School]": "point",
    "[Districts]": "areas",
    "[District]": "area",
    "[Lines]": "lines",
    "[Line]": "line",
    "[Heat]": "raster",
    "[layer]": "a layer",
    "[Type]": "a value",
    "[field]": "a field",
}


def clean_label(label: str) -> str:
    for k, v in TOKEN_WORDS.items():
        label = label.replace(k, v)
    return label.rstrip("?…")


def describe(action: str, spec: dict) -> str:
    parts = [clean_label(spec["label"]) + "."]
    if spec.get("method"):
        # The first clause only: the rest is recipe detail the model doesn't need.
        m = spec["method"].split(";")[0].strip()
        parts.append(m[0].upper() + m[1:] + ".")
    if action in OUTPUT_NOTE:
        parts.append(OUTPUT_NOTE[action])
    return " ".join(parts)


# ---------------------------------------------------------------------------------------------
# Schemas (kept small: every token is paid on every request)

S = {"type": "string"}
OBJECT = {
    "type": "object",
    "properties": {"layer": S, "field": S, "value": S},
    "required": ["layer"],
}
PARAM_SCHEMA = {
    "distance": {"type": "string", "description": "e.g. '500 m', '2 km', '3 mi'"},
    "merge": {"type": "boolean", "description": "merge overlaps (default true)"},
    "band": {"type": "integer", "description": "1-based, default 1"},
    "field": S,
    "a": {"type": "string", "description": "a number field"},
    "op": {"type": "string", "enum": ["add", "subtract", "multiply", "divide"]},
    "b": {"type": "string", "description": "a number field, or a number"},
    "name": {"type": "string", "description": "the new field's name"},
    "conditions": {"type": "string", "description": "conditions joined by ';', each 'raster name|at_least|value', 'raster name|at_most|value' or 'raster name|between|low|high', e.g. 'Elevation|at_least|3000;Rainfall|between|500|1000'"},
}


def fn(name: str, description: str, props: dict, required: list[str]) -> dict:
    params: dict = {"type": "object", "properties": props}
    if required:
        params["required"] = required
    return {"type": "function", "function": {"name": name, "description": description, "parameters": params}}


def display_tools() -> list[dict]:
    cond = {
        "type": "object",
        "properties": {
            "field": S,
            "op": {
                "type": "string",
                "enum": ["is", "is_not", "contains", "is_empty", "above", "below", "between", "before", "after", "any_of"],
            },
            "value": {"type": "string", "description": "number or text"},
            "value2": {"type": "string", "description": "upper end for between"},
            "values": {"type": "array", "items": S, "description": "for any_of"},
        },
        "required": ["field", "op"],
    }
    return [
        fn(
            "filter",
            "Show only a layer's objects meeting all conditions (replaces its filter; [] clears). Later actions use only shown objects.",
            {"layer": S, "conditions": {"type": "array", "items": cond}},
            ["layer", "conditions"],
        ),
        fn("color_by", "Color a layer by a field's values.", {"layer": S, "field": S}, ["layer", "field"]),
        fn("view_data", "Open a layer's table.", {"layer": S}, ["layer"]),
        fn("zoom_to", "Zoom to and select a layer, or one object.", {"layer": S, "field": S, "value": S}, ["layer"]),
    ]


def meta_tools() -> list[dict]:
    return [
        fn(
            "ask_user",
            "Ask one question when the request is ambiguous.",
            {"question": S, "choices": {"type": "array", "items": S}},
            ["question"],
        ),
        fn("cannot_do", "The tools can't answer this; say why.", {"reason": S}, ["reason"]),
    ]


def action_table() -> list[dict]:
    """[{id, roles: [(name, kind)], params: [...], description}] for the connected actions."""
    spec = rules()
    phrases = runner_inputs()
    out = []
    for a in connected_actions():
        if a not in spec:
            raise ValueError(f"{a} is connected but not in rules.toml")
        if a not in phrases:
            raise ValueError(f"{a} has no row in the runner's input table")
        out.append(
            {
                "id": a,
                "roles": roles_for(a, phrases[a]),
                "params": PARAMS_OF_ACTION.get(a, []),
                "description": describe(a, spec[a]),
            }
        )
    return out


def per_action_tools() -> list[dict]:
    tools = []
    for a in action_table():
        props: dict = {}
        required = []
        for name, kind in a["roles"]:
            if kind == "object?":
                props[name] = OBJECT if a["id"] == "summarize_drawn" else dict(OBJECT, description="one object only (optional)")
            else:
                props[name] = OBJECT if kind == "object" else S
            if not kind.endswith("?"):
                required.append(name)
        for p in a["params"]:
            props[p] = PARAM_SCHEMA[p]
        if a["id"] == "rank_by_value":
            required.append("field")
        if a["id"] == "buffer":
            required.append("distance")
        tools.append(fn(a["id"], a["description"], props, required))
    return tools + display_tools() + meta_tools()


def single_tools() -> list[dict]:
    table = action_table()
    lines = []
    for a in table:
        layers = [n for n, k in a["roles"] if k == "layer"]
        objs = [n for n, k in a["roles"] if k.startswith("object")]
        sig = f"layers=[{', '.join(layers)}]" if layers else ""
        if objs:
            opt = "?" if a["roles"][[n for n, _ in a["roles"]].index(objs[0])][1].endswith("?") else ""
            sig += (", " if sig else "") + f"object{opt}={objs[0]}"
        if a["params"]:
            sig += (", " if sig else "") + ", ".join(a["params"])
        lines.append(f"{a['id']}({sig}): {a['description']}")
    desc = "Run one analysis action.\n" + "\n".join(lines)
    props = {
        "action": {"type": "string", "enum": [a["id"] for a in table]},
        "layers": {"type": "array", "items": S, "description": "layer names, in the action's order"},
        "object": OBJECT,
    }
    for p in ["distance", "merge", "band", "field"]:
        props[p] = PARAM_SCHEMA[p]
    return [fn("run_action", desc, props, ["action"])] + display_tools() + meta_tools()


DESIGNS = {"per_action": per_action_tools, "single": single_tools}


def tools_for(design: str) -> list[dict]:
    return DESIGNS[design]()


# ---------------------------------------------------------------------------------------------
# Turning either design's calls into canonical calls: {"name": action id, "args": {role: …}}

DISPLAY_AND_META = {"filter", "color_by", "view_data", "zoom_to", "ask_user", "cannot_do"}


def canonical_call(name: str, args: dict) -> dict:
    """Map one tool call from either design onto the canonical per-action form."""
    if not isinstance(args, dict):
        args = {}
    if name != "run_action":
        return {"name": name, "args": args}
    action = args.get("action")
    table = {a["id"]: a for a in action_table()}
    if action not in table:
        return {"name": f"run_action:{action}", "args": args}
    a = table[action]
    layers = args.get("layers") or []
    if isinstance(layers, str):
        layers = [layers]
    obj = args.get("object")
    out: dict = {}
    li = 0
    for role, kind in a["roles"]:
        if kind == "layer":
            if li < len(layers):
                out[role] = layers[li]
            li += 1
        elif kind.startswith("object"):
            if isinstance(obj, dict) and obj:
                out[role] = obj
            elif kind == "object" and li < len(layers):
                # The object given as a layer name (a drawn area, or a one-object layer).
                out[role] = {"layer": layers[li]}
                li += 1
    for p in a["params"]:
        if p in args:
            out[p] = args[p]
    return {"name": action, "args": out}


def output_note(call: dict) -> str:
    """What a planned step will make, as the app can say before running it (tool results)."""
    a = call.get("args") or {}
    name = call.get("name")

    def nm(x):
        return x if isinstance(x, str) else "?"

    if name == "count_in":
        return f"Its new layer has field '{nm(a.get('points')).lower()}'."
    if name == "summarize_each":
        r = nm(a.get("raster"))
        return f"Its new layer has fields '{r} mean', '{r} min', '{r} max'."
    if name == "add_raster_values":
        return f"It adds field '{nm(a.get('raster'))}' to {nm(a.get('points'))}."
    return ""


def main() -> None:
    import sys

    sys.path.insert(0, str(HERE))  # python -I drops the script's folder
    from tokens import count_tokens

    for design, make in DESIGNS.items():
        tools = make()
        path = HERE / f"tools_{design}.json"
        text = json.dumps(tools, ensure_ascii=False, indent=1)
        path.write_text(text + "\n", encoding="utf-8")
        compact = json.dumps(tools, ensure_ascii=False, separators=(",", ":"))
        print(f"{design}: {len(tools)} tools, {len(compact)} chars, ~{count_tokens(compact)} tokens (o200k)")


if __name__ == "__main__":
    main()
