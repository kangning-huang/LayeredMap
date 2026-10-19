"""Map contexts: the system message the assistant sees (Phase 7, WP7.2).

The default context is value-free: layer names, kinds, counts, fields with types and declared
units, and the current selection, never feature values (spec › Product tiers: "Only
descriptions leave the Mac"). The variant ``with_values`` adds the values of category fields
(at most 8 per field), to measure whether they help; it is not the product default.

Layers describe the example data (``assets/examples``) and the Shanghai fixtures
(``fixtures/V1``, ``fixtures/T1``, ``fixtures/V9``). Two rasters in the Shanghai context
(Surface temperature, Land cover) are described as the app would describe such files; they
stand in for the R-series rasters and only their descriptions are used.
"""

from __future__ import annotations

_COMMON = (
    "Refer to layers by name and to one object as {layer, field, value}; omit field and value for "
    "the selected object or a drawn area. Use field names exactly as listed, and units as the field "
    "declares. A new layer made by your n-th call can be used by later calls as \"#n\". "
    "Distances: number and unit (m, km, mi, ft). If the request is ambiguous, call ask_user once; "
    "if the tools can't do it, call cannot_do."
)

# v1 is the benchmark prompt. v2 (the calls are the plan; never a text plan) was tried on 12 cases
# × 4 models after the full run because GPT-5 models often wrote the plan as text; it made no
# material difference (README.md › Calibration), so v1 stays.
INSTRUCTIONS = {
    "v1": (
        "You are NimbleMap's assistant. Turn the user's request into calls to the tools; never "
        "compute or guess answers yourself. Nothing runs until the user approves your whole plan; a tool "
        "result only confirms that the step was added. Add every step the request needs, in order (in one "
        "reply when you can), then stop. " + _COMMON
    ),
    "v2": (
        "You are NimbleMap's assistant. Answer only by calling tools: your calls are the plan, which "
        "the user approves before anything runs. Never write a plan as text instead, and never compute or "
        "guess answers. A tool result only confirms that the step was added. Make each call the request "
        "needs once, in order (in one reply when you can), then stop. " + _COMMON
    ),
}
DEFAULT_PROMPT = "v1"
INSTRUCTION = INSTRUCTIONS[DEFAULT_PROMPT]

# Each field: (name, type, unit or None, category values or None)
LAYERS = {
    "Countries": {
        "kind": "areas",
        "count": 177,
        "fields": [
            ("name", "text", None, None),
            ("name_zh", "text, Chinese", None, None),
            ("iso_a3", "text", None, None),
            (
                "continent",
                "category",
                None,
                ["Africa", "Antarctica", "Asia", "Europe", "North America", "Oceania", "Seven seas (open ocean)", "South America"],
            ),
            ("pop_est", "integer", "people", None),
            ("pop_year", "integer", "year", None),
            ("gdp_md", "integer", "million US$", None),
            ("gdp_year", "integer", "year", None),
        ],
    },
    "Populated places": {
        "kind": "points",
        "count": 1251,
        "fields": [
            ("name", "text", None, None),
            ("name_zh", "text, Chinese", None, None),
            ("country", "text", None, None),
            ("population", "integer", "people", None),
            ("capital", "yes/no", None, ["true", "false"]),
        ],
    },
    "Elevation": {"kind": "raster", "desc": "1 band, 0.5° cells, metres above sea level"},
    "Districts": {
        "kind": "areas",
        "count": 17,
        "fields": [
            ("district_id", "text", None, None),
            ("name_zh", "text, Chinese", None, None),
            ("name_en", "text", None, None),
            ("population", "integer", "people", None),
            ("area_km2", "number", "km²", None),
        ],
    },
    "Schools": {
        "kind": "points",
        "count": 242,
        "fields": [
            ("name", "text", None, None),
            ("type", "category", None, ["Primary", "Middle", "High"]),
            ("students", "integer", "students", None),
        ],
    },
    "Roads": {
        "kind": "lines",
        "count": 6,
        "fields": [("road_id", "integer", None, None), ("name", "text", None, None), ("lanes", "integer", None, None)],
    },
    "Parks": {
        "kind": "areas",
        "count": 3,
        "fields": [("name", "text", None, None), ("kind", "category", None, ["city", "district", "pocket"])],
    },
    "Surface temperature": {"kind": "raster", "desc": "1 band, 30 m cells, °C"},
    "Land cover": {"kind": "raster", "desc": "1 band, 30 m cells, categories"},
    "Area 1": {"kind": "drawn area", "count": 1, "fields": []},
}

CONTEXTS = {
    "world": ["Countries", "Populated places", "Elevation"],
    "shanghai": ["Districts", "Schools", "Roads", "Parks", "Surface temperature", "Land cover"],
    "shanghai_drawn": ["Districts", "Schools", "Roads", "Parks", "Surface temperature", "Land cover", "Area 1"],
}


def layer_line(i: int, name: str, with_values: bool) -> str:
    L = LAYERS[name]
    if L["kind"] == "raster":
        return f"{i}. {name}: raster, {L['desc']}"
    head = f"{i}. {name}: {L['kind']}, {L['count']:,}"
    if not L["fields"]:
        return head
    fs = []
    for fname, ftype, unit, values in L["fields"]:
        s = f"{fname} ({ftype}"
        if unit:
            s += f", {unit}"
        s += ")"
        if with_values and values:
            s += " [" + "|".join(values[:8]) + "]"
        fs.append(s)
    return head + "; fields: " + ", ".join(fs)


def selection_line(selection: dict | None) -> str:
    if not selection:
        return "Selected: nothing"
    n = selection.get("count", 1)
    what = "1 object" if n == 1 else f"{n} objects"
    return f"Selected: {what} of {selection['layer']}"


def system_message(context: str, selection: dict | None = None, with_values: bool = False, prompt: str = DEFAULT_PROMPT) -> str:
    lines = [INSTRUCTIONS[prompt], "", "Open layers:"]
    for i, name in enumerate(CONTEXTS[context], 1):
        lines.append(layer_line(i, name, with_values))
    lines.append(selection_line(selection))
    return "\n".join(lines)


def main() -> None:
    import sys
    from pathlib import Path

    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from tokens import count_tokens, method

    print(f"Token counts ({method()}):")
    print(f"  instruction alone: {count_tokens(INSTRUCTION)}")
    for c in CONTEXTS:
        for wv in (False, True):
            msg = system_message(c, None, wv)
            print(f"  {c:15s} {'with values' if wv else 'value-free ':12s} {count_tokens(msg):4d} tokens, {len(msg)} chars")
    print()
    print(system_message("world"))


if __name__ == "__main__":
    main()
