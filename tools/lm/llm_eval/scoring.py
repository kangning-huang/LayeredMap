"""Scoring a model's tool calls against a case's expected plan.

Two scores (README.md › Scoring):

* strict: what LayeredMap could run as is. Same calls in the same order, nothing extra; layer
  and field names exactly as listed; objects named by a (field, value) pair that exists in the
  data; distances that the runner's own parser reads (``parse_distance`` in
  ``crates/lm-jobs/src/actions.rs``) within 0.5 %; filter numbers that parse as numbers.
* lenient: what a forgiving plan checker could repair. Names compared case-insensitively;
  objects matched on the value alone (including a few spelled-out aliases); distances also in
  Chinese units, within 2 %; operator synonyms; "is X" = "any of [X]"; "above a" + "below b" =
  "between a and b"; consecutive filters on one layer merged; extra zoom/table/color calls
  ignored; JSON given as a string decoded; a plain-text question counts as ask_user and a plain-
  text refusal as cannot_do; and the case's ``lenient_alts`` plans accepted.
"""

from __future__ import annotations

import json
import math
import re

import tools as toolmod

LAYER_KEYS = {"points", "areas", "raster", "layer"}
OBJECT_KEYS = {"point", "area", "line", "object", "target"}
DISPLAY_EXTRAS = {"zoom_to", "view_data", "color_by"}

# ---------------------------------------------------------------------------------------------
# Parsing the model's reply


def parse_tool_calls(message: dict) -> tuple[list[dict], str]:
    """(canonical calls, text) from an OpenAI-style assistant message."""
    calls = []
    for tc in message.get("tool_calls") or []:
        f = tc.get("function") or {}
        name = f.get("name") or ""
        raw = f.get("arguments")
        if isinstance(raw, dict):
            args = raw
        else:
            try:
                args = json.loads(raw) if raw else {}
            except (TypeError, ValueError):
                args = {"_unparsed": raw}
        calls.append(canonicalize(name, args))
    text = message.get("content") or ""
    if isinstance(text, list):
        text = " ".join(p.get("text", "") for p in text if isinstance(p, dict))
    return calls, text


def canonicalize(name: str, args) -> dict:
    c = toolmod.canonical_call(name, args if isinstance(args, dict) else {})
    if c["name"] == "zoom_to":
        a = c["args"]
        target = {k: a[k] for k in ("layer", "field", "value") if k in a}
        c = {"name": "zoom_to", "args": {"layer": a.get("layer"), "target": target}}
    return c


# ---------------------------------------------------------------------------------------------
# Distances


def parse_distance_app(s) -> float | None:
    """A port of the runner's parse_distance: metres, or None if the app would reject it."""
    if isinstance(s, bool) or s is None:
        return None
    s = str(s).strip().lower().replace(",", "")
    split = len(s)
    for i, ch in enumerate(s):
        if not (ch.isascii() and (ch.isdigit() or ch == ".")):
            split = i
            break
    try:
        value = float(s[:split].strip())
    except ValueError:
        return None
    unit = s[split:].strip()
    factor = {
        "": 1.0, "m": 1.0, "meter": 1.0, "meters": 1.0, "metre": 1.0, "metres": 1.0,
        "km": 1000.0, "kilometer": 1000.0, "kilometers": 1000.0, "kilometre": 1000.0, "kilometres": 1000.0,
        "mi": 1609.344, "mile": 1609.344, "miles": 1609.344,
        "ft": 0.3048, "foot": 0.3048, "feet": 0.3048,
    }.get(unit)
    if factor is None:
        return None
    m = value * factor
    return m if math.isfinite(m) and m > 0 else None


LENIENT_UNITS = {
    "米": 1.0, "公尺": 1.0, "公里": 1000.0, "千米": 1000.0, "英里": 1609.344, "英尺": 0.3048,
    "yd": 0.9144, "yard": 0.9144, "yards": 0.9144, "nmi": 1852.0,
}


def parse_distance_lenient(s) -> float | None:
    if isinstance(s, (int, float)) and not isinstance(s, bool):
        return float(s) if s > 0 else None
    if isinstance(s, dict):  # {"value": 500, "unit": "m"}
        v = s.get("value", s.get("amount"))
        u = s.get("unit", s.get("units", "m"))
        return parse_distance_lenient(f"{v} {u}")
    m = parse_distance_app(s)
    if m is not None:
        return m
    t = str(s).strip().lower().replace(",", "").replace(" ", "")
    mm = re.match(r"^([0-9.]+)(.*)$", t)
    if not mm:
        return None
    try:
        v = float(mm.group(1))
    except ValueError:
        return None
    unit = mm.group(2)
    if unit in LENIENT_UNITS:
        return v * LENIENT_UNITS[unit]
    return parse_distance_app(f"{v} {unit}")


# ---------------------------------------------------------------------------------------------
# Argument matchers


def _norm(s) -> str:
    return re.sub(r"\s+", " ", str(s)).strip().casefold()


def _as_list(x) -> list:
    return x if isinstance(x, list) else [x]


def _maybe_json(x, lenient: bool):
    if lenient and isinstance(x, str) and x.strip()[:1] in "[{":
        try:
            return json.loads(x)
        except ValueError:
            return x
    return x


def match_name(exp, got, lenient: bool, allow_ref: bool = True) -> bool:
    if not isinstance(got, str):
        return False
    options = _as_list(exp)
    if got.strip() in options:
        return True
    if lenient:
        if _norm(got) in {_norm(o) for o in options}:
            return True
        if allow_ref and any(str(o).startswith("#") for o in options) and re.fullmatch(r"#\d+", got.strip()):
            return True
    return False


def match_object(exp: dict, got, lenient: bool) -> bool:
    got = _maybe_json(got, lenient)
    if lenient and isinstance(got, str):
        got = {"layer": got}
    if not isinstance(got, dict):
        return False
    if not match_name(exp["layer"], got.get("layer"), lenient):
        return False
    field, value = got.get("field"), got.get("value")
    if exp.get("selected"):
        return lenient or (field in (None, "") and value in (None, ""))
    if value in (None, ""):
        return False
    v = _norm(value)
    if lenient:
        values = {_norm(p[1]) for p in exp["any"]} | {_norm(x) for x in exp.get("lenient", [])}
        return v in values
    return any(field == f and v == _norm(val) for f, val in exp["any"])


def match_distance(exp: dict, got, lenient: bool) -> bool:
    m = parse_distance_lenient(got) if lenient else parse_distance_app(got)
    if m is None:
        return False
    tol = 0.02 if lenient else 0.005
    return abs(m - exp["distance_m"]) <= tol * exp["distance_m"]


def as_bool(x):
    if isinstance(x, bool):
        return x
    if isinstance(x, str) and x.strip().lower() in ("yes", "true"):
        return True
    if isinstance(x, str) and x.strip().lower() in ("no", "false"):
        return False
    return None


OP_SYNONYMS = {
    ">": "above", "gt": "above", "greater_than": "above", "greater": "above", "more_than": "above", ">=": "above",
    "<": "below", "lt": "below", "less_than": "below", "less": "below", "<=": "below",
    "=": "is", "==": "is", "eq": "is", "equals": "is", "equal": "is",
    "!=": "is_not", "not": "is_not", "ne": "is_not",
    "in": "any_of", "one_of": "any_of", "is_any_of": "any_of",
    "is_null": "is_empty", "empty": "is_empty", "missing": "is_empty",
    "like": "contains",
}
TRUE_WORDS = {"true", "yes"}


def _num(x, lenient: bool) -> float | None:
    if isinstance(x, bool):
        return None
    if isinstance(x, (int, float)):
        return float(x)
    if isinstance(x, str):
        t = x.strip()
        if lenient:
            t = t.replace(",", "").replace("_", "")
        try:
            return float(t)
        except ValueError:
            return None
    return None


def _value_eq(exp, got, lenient: bool, tol: float | None) -> bool:
    if isinstance(exp, (int, float)) and not isinstance(exp, bool):
        g = _num(got, lenient)
        if g is None:
            return False
        rel = tol if tol is not None else 1e-9
        return abs(g - exp) <= rel * max(1.0, abs(exp))
    if exp == "true":
        words = TRUE_WORDS | ({"1"} if lenient else set())
        return got is True or (isinstance(got, str) and got.strip().casefold() in words)
    return isinstance(got, (str, int, float)) and not isinstance(got, bool) and _norm(got) == _norm(exp)


def normalize_conditions(conds, lenient: bool) -> list[dict] | None:
    conds = _maybe_json(conds, lenient)
    if conds is None:
        return []
    if isinstance(conds, dict) and lenient:
        conds = [conds]
    if not isinstance(conds, list):
        return None
    out = []
    for c in conds:
        c = _maybe_json(c, lenient)
        if not isinstance(c, dict):
            return None
        c = dict(c)
        op = str(c.get("op", "")).strip().lower()
        if lenient:
            op = OP_SYNONYMS.get(op, op.replace(" ", "_"))
            # between given as values [a, b]
            if op == "between" and "value2" not in c and isinstance(c.get("values"), list) and len(c["values"]) == 2:
                c["value"], c["value2"] = c["values"]
            if op == "any_of" and isinstance(c.get("value"), list) and "values" not in c:
                c["values"] = c["value"]
            if op == "any_of" and isinstance(c.get("values"), list) and len(c["values"]) == 1:
                op, c["value"] = "is", c["values"][0]
        c["op"] = op
        out.append(c)
    if lenient:
        # "above a" and "below b" on one field = "between a and b"
        merged, used = [], set()
        for i, a in enumerate(out):
            if i in used:
                continue
            if a["op"] in ("above", "below"):
                for j in range(i + 1, len(out)):
                    b = out[j]
                    if j not in used and b.get("field") == a.get("field") and {a["op"], b["op"]} == {"above", "below"}:
                        lo, hi = (a, b) if a["op"] == "above" else (b, a)
                        merged.append({"field": a.get("field"), "op": "between", "value": lo.get("value"), "value2": hi.get("value")})
                        used.add(j)
                        break
                else:
                    merged.append(a)
            else:
                merged.append(a)
        out = merged
    return out


def match_condition(exp: dict, got: dict, lenient: bool) -> bool:
    eop = exp["op"]
    if lenient and eop == "any_of" and len(exp.get("values", [])) == 1:
        exp = {"field": exp["field"], "op": "is", "value": exp["values"][0], **({"tol": exp["tol"]} if "tol" in exp else {})}
        eop = "is"
    if got.get("op") != eop:
        return False
    if not match_name(exp["field"], got.get("field"), lenient, allow_ref=False):
        return False
    tol = exp.get("tol")
    if eop == "is_empty":
        return True
    if eop == "any_of":
        gv = got.get("values")
        if not isinstance(gv, list) or len(gv) != len(exp["values"]):
            return False
        rest = list(gv)
        for v in exp["values"]:
            hit = next((g for g in rest if _value_eq(v, g, lenient, tol)), None)
            if hit is None:
                return False
            rest.remove(hit)
        return True
    if eop == "between":
        lo, hi = sorted([exp["value"], exp["value2"]])
        g1, g2 = _num(got.get("value"), lenient), _num(got.get("value2"), lenient)
        if g1 is None or g2 is None:
            return False
        g1, g2 = sorted([g1, g2])
        return _value_eq(lo, g1, lenient, tol) and _value_eq(hi, g2, lenient, tol)
    return _value_eq(exp["value"], got.get("value"), lenient, tol)


def match_conditions(exp: list, got, lenient: bool) -> bool:
    g = normalize_conditions(got, lenient)
    if g is None or len(g) != len(exp):
        return False
    rest = list(g)
    for e in exp:
        hit = next((x for x in rest if match_condition(e, x, lenient)), None)
        if hit is None:
            return False
        rest.remove(hit)
    return True


def match_args(name: str, exp: dict, got: dict, lenient: bool) -> str | None:
    """None if the arguments match, else the kind of mismatch."""
    for key, e in exp.items():
        if key == "conditions":
            g = got.get("conditions")
            if not e and g in (None, [], "[]"):
                continue
            if not match_conditions(e, g, lenient):
                return "wrong_filter"
            continue
        if key not in got:
            return "wrong_object" if key in OBJECT_KEYS else ("wrong_layer" if key in LAYER_KEYS else f"missing_{key}")
        g = got[key]
        if key in LAYER_KEYS:
            if not match_name(e, g, lenient):
                return "wrong_layer"
        elif key in OBJECT_KEYS:
            if not match_object(e, g, lenient):
                return "wrong_object"
        elif key == "distance":
            if not match_distance(e, g, lenient):
                return "wrong_distance"
        elif key == "merge":
            if as_bool(g) != e:
                return "wrong_param"
        elif key == "band":
            if _num(g, lenient) != e:
                return "wrong_param"
        elif key == "field":
            if not match_name(e, g, lenient, allow_ref=False):
                return "wrong_field"
        else:
            if g != e:
                return "wrong_param"
    # Buffering one object when the request meant the whole layer (or the reverse) is wrong.
    if name == "buffer" and "object" not in exp:
        o = _maybe_json(got.get("object"), lenient)
        if isinstance(o, dict) and (o.get("value") not in (None, "") or o.get("field")):
            return "wrong_object"
    return None


# ---------------------------------------------------------------------------------------------
# Plans


def lenient_prepare(calls: list[dict], expected_names: set[str]) -> list[dict]:
    out: list[dict] = []
    for c in calls:
        if c["name"] in DISPLAY_EXTRAS and c["name"] not in expected_names:
            continue
        if c["name"] == "filter" and out and out[-1]["name"] == "filter" and _norm(out[-1]["args"].get("layer")) == _norm(c["args"].get("layer")):
            a = normalize_conditions(out[-1]["args"].get("conditions"), True) or []
            b = normalize_conditions(c["args"].get("conditions"), True) or []
            out[-1] = {"name": "filter", "args": {**out[-1]["args"], "conditions": a + b}}
            continue
        out.append(c)
    return out


def match_plan(exp: list[dict], got: list[dict], lenient: bool) -> str | None:
    if lenient:
        got = lenient_prepare(got, {c["name"] for c in exp})
    names_e = [c["name"] for c in exp]
    names_g = [c["name"] for c in got]
    if names_e != names_g:
        return diagnose_names(names_e, names_g)
    for e, g in zip(exp, got):
        r = match_args(e["name"], e["args"], g["args"], lenient)
        if r:
            return r
    return None


META = {"ask_user", "cannot_do"}


def diagnose_names(exp: list[str], got: list[str]) -> str:
    if not got:
        return "no_call"
    if exp == ["ask_user"]:
        return "refused_instead_of_asking" if got == ["cannot_do"] else "acted_instead_of_asking"
    if exp == ["cannot_do"]:
        return "asked_instead_of_refusing" if got == ["ask_user"] else "acted_instead_of_refusing"
    if "cannot_do" in got:
        return "refused_answerable"
    if "ask_user" in got:
        return "asked_unnecessarily"
    if set(exp) <= set(got) and len(got) > len(exp):
        return "extra_steps"
    if set(got) <= set(exp) and len(got) < len(exp):
        return "missing_steps"
    return "wrong_action"


def text_only_ok(exp: list[dict], text: str) -> bool:
    names = [c["name"] for c in exp]
    t = (text or "").strip()
    if not t:
        return False
    if names == ["ask_user"]:
        return "?" in t or "？" in t
    return names == ["cannot_do"]


def score(case: dict, calls: list[dict], text: str = "") -> dict:
    """{"strict": bool, "lenient": bool, "reason": first mismatch against the expected plan}."""
    plans_strict = [case["expected"]] + case.get("alts", [])
    plans_lenient = plans_strict + case.get("lenient_alts", [])
    strict = any(match_plan(p, calls, False) is None for p in plans_strict)
    lenient = strict or any(match_plan(p, calls, True) is None for p in plans_lenient)
    if not lenient and not calls and any(text_only_ok(p, text) for p in plans_strict):
        lenient = True
    reason = None
    if not strict:
        reasons = [match_plan(p, calls, False) for p in plans_strict]
        # Prefer a reason from a plan with the same calls (an argument error) over a call mismatch.
        arg = [r for r in reasons if r and r.startswith(("wrong_layer", "wrong_object", "wrong_distance", "wrong_filter", "wrong_field", "wrong_param", "missing_"))]
        reason = arg[0] if arg else reasons[0]
    return {"strict": strict, "lenient": lenient, "reason": reason}
