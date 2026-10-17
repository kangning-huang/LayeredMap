"""The deterministic plan checker (harness v2).

Each planned step is checked against the open layers, the steps accepted so far and the data,
before it is added:

* layers exist and fit the slot (a raster slot needs a raster; points, areas, lines; a step output
  "#k" exists, and makes a layer -- a "#k" naming a filter or add-values step is replaced by the
  layer that step changed);
* required inputs are present; fields exist on the layer *at that point of the plan* (fields
  added by earlier steps count: add_raster_values adds the raster's name, count_in adds
  '<points>' and '<stat> <field>', summarize_each adds '<raster> mean' ...), with the right type
  (numeric for rank_by_value and count_in statistics);
* units parse with the runner's parser (``scoring.parse_distance_app``); a distance in Chinese
  units ("500米") or as a bare number is rewritten;
* objects ``{layer, field, value}`` name exactly one stored object; filter values match the
  field's stored values or categories. Both go through ``resolve.py``: a unique, confident match
  (case, punctuation, alias table, base name, pinyin) is corrected in place; anything else is
  an error naming the candidates, and several objects with one name is an error that says to
  ask the user;
* statistics are on the action's menu, with a numeric ``field`` when count_in takes a statistic;
* a step identical to an accepted one is not added again (and is not an error).

Errors go back to the model as the tool result of the offending call ("Not added: ...") for up to
2 repair rounds (the runner counts them). Corrections are reported in the tool result too.

At the end, ``finalize`` drops trailing display-only steps (zoom_to, view_data, color_by) that the
request does not ask for. Heuristic: a step is kept if the request contains one of its words
(``DISPLAY_WORDS``, English and Chinese), or if dropping would leave the plan empty.
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

import context as ctxmod  # noqa: E402
import data  # noqa: E402
import resolve  # noqa: E402
import scoring  # noqa: E402
import tools as toolmod  # noqa: E402
import tools_v2  # noqa: E402

DISPLAY_ONLY = {"zoom_to", "view_data", "color_by"}
DISPLAY_WORDS = {
    "color_by": ["colo", "shade", "style", "choropleth", "上色", "颜色", "着色", "配色", "渲染"],
    "view_data": ["table", "attribute", "spreadsheet", "表格", "属性表", "数据表"],
    "zoom_to": ["zoom", "go to", "take me", "fly to", "center on", "centre on", "focus on", "缩放", "定位", "跳到", "放大到", "飞到"],
}
SELECTION_WORDS = r"\bselected\b|\bthis\b|\bthese\b|选中|选定|这个|这些|该"
# A filter with no conditions clears it: only when the request says so (heuristic).
CLEAR_WORDS = ["clear", "remove", "reset", "show all", "all the", "every", "unfilter", "清除", "取消", "全部", "所有", "移除", "重置", "去掉"]
OP_FIX = {
    ">": "above", ">=": "above", "gt": "above", "greater_than": "above", "<": "below", "<=": "below", "lt": "below",
    "less_than": "below", "=": "is", "==": "is", "eq": "is", "equals": "is", "!=": "is_not", "ne": "is_not",
    "in": "any_of", "one_of": "any_of", "is_null": "is_empty", "empty": "is_empty", "like": "contains",
}
STAT_FIX = {"average": "mean", "avg": "mean", "total": "sum", "minimum": "min", "maximum": "max",
            "stddev": "std", "std_dev": "std", "standard_deviation": "std", "standard deviation": "std", "number": "count"}
POINT_STAT_WORD = {"sum": "sum", "mean": "mean", "median": "median", "min": "minimum", "max": "maximum"}
RASTER_STAT_WORD = {"mean": "mean", "median": "median", "min": "minimum", "max": "maximum", "sum": "sum", "std": "std dev"}
RASTER_STAT_TITLE = {"median": "Median", "min": "Minimum", "max": "Maximum", "sum": "Sum", "std": "Standard deviation"}

ACTIONS = {a["id"]: a for a in toolmod.action_table()}
STATS = tools_v2.stat_actions()
DISPLAY_REQUIRED = {"filter": ["layer", "conditions"], "color_by": ["layer", "field"], "view_data": ["layer"], "zoom_to": ["layer"]}
SLOT_NEEDS = {"points": "points", "areas": "areas", "raster": "raster", "point": "points", "line": "lines", "area": "areas"}


# Steps that change their layer in place (no new layer): the argument naming that layer.
IN_PLACE = {"filter": "layer", "add_raster_values": "points", "join_by_location": "layer", "nearest_distance": "layer",
            "nearest_other": "layer", "add_measurements": "layer", "calculate_field": "layer"}
FIELD_OP_SIGN = {"subtract": "−", "add": "+", "multiply": "×", "divide": "÷"}
WHERE_OPS = {"at_least": "at_least", "above": "at_least", "at_most": "at_most", "below": "at_most", "between": "between"}
KIND_NOUNS = {"points": ("point", "points"), "lines": ("line", "lines"), "areas": ("area", "areas"), "drawn area": ("area", "areas")}


def rs_singular(name: str) -> str:
    """``actions::singular`` in the runner."""
    n = name.strip()
    if n.endswith("ies") and len(n) > 3:
        return n[:-3] + "y"
    for end in ("sses", "shes", "ches", "xes"):
        if n.endswith(end):
            return n[:-2]
    if n.endswith("s") and not n.endswith("ss") and len(n) > 1:
        return n[:-1]
    return n


def rs_plural(name: str) -> str:
    """``actions::plural`` in the runner."""
    n = name.strip()
    if rs_singular(n) != n:
        return n
    if not n or n[-1].isdigit() or not (n[-1].isascii() and n[-1].isalpha()):
        return n
    if n.endswith("y") and not n.endswith(("ay", "ey", "oy")):
        return n[:-1] + "ies"
    if n.endswith(("s", "x", "ch", "sh")):
        return n + "es"
    return n + "s"


def rs_word(name: str) -> str | None:
    """``PlanInput::word``: the layer name as a noun (at most 3 words, letters only), else None."""
    w = " ".join(name.replace("_", " ").replace("-", " ").lower().split())
    return w if w and len(w.split(" ")) <= 3 and all(c.isalpha() or c == " " for c in w) else None


def _word(name: str) -> str | None:
    w = re.sub(r"[_\-]", " ", name).lower().split()
    ok = w and len(w) <= 3 and all(x.isalpha() for x in w)
    return " ".join(w) if ok else None


def _singular(w: str) -> str:
    if w.endswith("ies"):
        return w[:-3] + "y"
    if w.endswith("s") and not w.endswith("ss"):
        return w[:-1]
    return w


def _each(name: str) -> str:
    w = _word(name)
    return f"each {_singular(w)}" if w else f"each area of {name}"


def _canon(call: dict) -> str:
    return json.dumps([call.get("name"), call.get("args")], sort_keys=True, ensure_ascii=False)


@dataclass
class Verdict:
    ok: bool
    call: dict
    errors: list[str] = field(default_factory=list)
    fixes: list[str] = field(default_factory=list)
    duplicate_of: int | None = None
    note: str = ""


class Checker:
    def __init__(self, context: str, selection: dict | None, request: str = "", enabled: bool = True):
        self.context = context
        self.selection = selection
        self.request = request or ""
        self.enabled = enabled
        self.open = list(ctxmod.CONTEXTS[context])
        self.steps: list[dict] = []
        self.outputs: dict[int, dict] = {}  # k -> {kind, fields, name}
        self.added: dict[str, dict[str, str]] = {}  # layer -> fields added by earlier steps
        self.origin: dict[tuple[str, str], tuple[str, str]] = {}  # (layer, joined field) -> (area layer, its field)

    # -----------------------------------------------------------------------------------------
    # Layers and fields

    def kind_of(self, key: str) -> str | None:
        if key.startswith("#"):
            o = self.outputs.get(int(key[1:]))
            return o["kind"] if o else None
        return data.kind(key)

    def fields_of(self, key: str) -> dict[str, str]:
        if key.startswith("#"):
            o = self.outputs.get(int(key[1:]))
            return dict(o["fields"]) if o else {}
        f = data.fields(key)
        f.update(self.added.get(key, {}))
        return f

    def layer(self, name, need: str | None, role: str, action: str, v: Verdict) -> str | None:
        """The layer key (a context layer name or "#k"), fixing case or a #k to a changed layer."""
        if not isinstance(name, str) or not name.strip():
            v.errors.append(f"{action}: {role} is missing; give a layer name.")
            return None
        s = name.strip()
        key = None
        m = re.fullmatch(r"#\s*(\d+)", s)
        if m:
            k = int(m.group(1))
            if not 1 <= k <= len(self.steps):
                v.errors.append(f"{action}: {role} {s!r} doesn't exist: the plan has {len(self.steps)} step(s) so far; \"#k\" names the new layer of step k.")
                return None
            prev = self.steps[k - 1]
            if prev["name"] in tools_v2.NEW_LAYER_ACTIONS:
                key = f"#{k}"
            elif prev["name"] in IN_PLACE and isinstance(prev["args"].get(IN_PLACE[prev["name"]]), str):
                target = prev["args"][IN_PLACE[prev["name"]]]
                v.fixes.append(f"{role} {s!r} -> {target!r} (step #{k} changes {target} and makes no new layer)")
                key = target
            else:
                v.errors.append(f"{action}: {role} {s!r}: step #{k} ({prev['name']}) makes no layer.")
                return None
        elif s in self.open:
            key = s
        else:
            same = [n for n in self.open if resolve.norm(n) == resolve.norm(s)]
            outs = [f"#{k}" for k, o in self.outputs.items() if resolve.norm(o.get("name", "")) == resolve.norm(s)]
            if len(same) == 1:
                key = same[0]
                v.fixes.append(f"{role} {s!r} -> {key!r}")
            elif len(outs) == 1:
                key = outs[0]
                v.fixes.append(f"{role} {s!r} -> {key!r} (the new layer of step {key})")
            else:
                v.errors.append(f"{action}: there is no layer {s!r}. Open layers: {', '.join(self.open)}; step outputs are \"#k\".")
                return None
        kind = self.kind_of(key)
        if need and not self._fits(kind, need):
            fit = [n for n in self.open if self._fits(data.kind(n), need)]
            hint = f" Layers of that kind: {', '.join(fit)}." if fit else " No open layer is of that kind: ask_user or cannot_do."
            made = [f"#{k} ({self.steps[k - 1]['name']}, {o['kind']})" for k, o in self.outputs.items() if self._fits(o["kind"], need)]
            if made:
                hint += " Step outputs of that kind: " + ", ".join(made) + "."
            if m and not made:
                hint += f" Step #{m.group(1)} is {self.steps[int(m.group(1)) - 1]['name']}; \"#k\" names the layer made by step k (count the steps from 1)."
            if action == "add_raster_values" and role == "points" and kind in ("areas", "drawn area"):
                hint = " For areas, summarize_each(raster, areas) gives each area the raster's statistics (new layer with '<raster> mean')."
            if action in ("whats_inside", "count_here", "summarize_here", "summarize_data_here") and need == "areas" and kind in ("points", "lines"):
                hint += " To look near points or lines, buffer them first (merge false keeps one area each) and use the buffer's layer."
            v.errors.append(f"{action}: {role} needs {need}, but {key} is {kind}.{hint}")
            return None
        return key

    @staticmethod
    def _fits(kind: str | None, need: str) -> bool:
        if need == "vector":
            return kind not in (None, "raster")
        if need == "areas":
            return kind in ("areas", "drawn area")
        return kind == need

    def field(self, key: str, fname, action: str, v: Verdict, numeric: bool = False, role: str = "field") -> str | None:
        fields = self.fields_of(key)
        if not isinstance(fname, str) or not fname.strip():
            v.errors.append(f"{action}: {role} is missing.")
            return None
        s = fname.strip()
        got = s if s in fields else None
        if got is None:
            same = [f for f in fields if resolve.norm(f) == resolve.norm(s)]
            if len(same) == 1:
                got = same[0]
                v.fixes.append(f"{role} {s!r} -> {got!r}")
        if got is None and key.startswith("#") and self._category_raster_output(key) and s.startswith(self._category_raster_output(key)):
            return s  # class-share fields of a category raster summary ('Land cover 3 share')
        if got is None:
            elsewhere = [n for n in self.open if not n.startswith("#") and s in self.fields_of(n)]
            listing = ", ".join(f"{f} ({t})" for f, t in fields.items())
            msg = f"{action}: {key} has no field {s!r}. Its fields: {listing}."
            if elsewhere:
                msg += f" {s!r} is a field of {', '.join(elsewhere)}: if the user's words don't fit {key}, ask_user."
            v.errors.append(msg)
            return None
        if numeric and not data.is_numeric(fields[got]):
            nums = [f for f, t in fields.items() if data.is_numeric(t)]
            v.errors.append(f"{action}: {role} {got!r} of {key} is {fields[got]}, not a number. Numeric fields: {', '.join(nums) or 'none'}.")
            return None
        return got

    def _category_raster_output(self, key: str) -> str | None:
        o = self.outputs.get(int(key[1:]))
        return o.get("class_prefix") if o else None

    # -----------------------------------------------------------------------------------------
    # Objects and values

    def object(self, obj, need: str, role: str, action: str, v: Verdict, optional: bool = False):
        if isinstance(obj, str):
            obj = {"layer": obj}
        if not isinstance(obj, dict):
            v.errors.append(f"{action}: {role} must be one object {{layer, field, value}}.")
            return None
        key = self.layer(obj.get("layer"), need, f"{role}.layer", action, v)
        if key is None:
            return None
        out = {"layer": key}
        fname, value = obj.get("field"), obj.get("value")
        if value in (None, "") and fname in (None, ""):
            sel = (self.selection or {}).get("layer")
            if key.startswith("#") or self.kind_of(key) == "drawn area" or sel == key or action == "summarize_drawn":
                return out
            v.errors.append(f"{action}: nothing in {key} is selected. Name the object as {{layer, field, value}} (resolve_feature finds the stored name), or ask_user which one.")
            return None
        if value in (None, ""):
            v.errors.append(f"{action}: {role} has a field but no value.")
            return None
        if key.startswith("#"):
            return {**out, "field": fname, "value": value}
        value = str(value)
        rows = data.rows(key)
        fields = self.fields_of(key)
        if isinstance(fname, str) and fname in fields:
            hits = [r[fname] for r in rows if r.get(fname) is not None and resolve.norm(r[fname]) == resolve.norm(value) and r[fname].casefold() == value.casefold()]
            if len(hits) == 1:
                if hits[0] != value:
                    v.fixes.append(f"{role} value {value!r} -> {hits[0]!r}")
                return {**out, "field": fname, "value": hits[0]}
            if len(hits) > 1:
                v.errors.append(f"{action}: {len(hits)} objects of {key} have {fname} {value!r}; ask_user which one is meant.")
                return None
        res = resolve.resolve_feature(key, value)
        cands = [c for c in res.get("candidates", []) if data.categories(key, c["field"]) is None]
        if cands:
            top = cands[0]
            rivals = [c for c in cands[1:] if c["score"] >= top["score"] - 0.05]
            if top["objects"] > 1:
                v.errors.append(f"{action}: {top['objects']} objects of {key} have {top['field']} {top['value']!r}; ask_user which one is meant.")
                return None
            if top["score"] >= 0.88 and not rivals:
                v.fixes.append(f"{role} {fname or '?'} {value!r} -> {top['field']} {top['value']!r}")
                return {**out, "field": top["field"], "value": top["value"]}
            names = "; ".join(f"{c['field']} {c['value']!r}" for c in cands)
            v.errors.append(f"{action}: {value!r} is not stored in {key}. Closest stored names: {names}. Use the one the user meant, or ask_user.")
            return None
        v.errors.append(f"{action}: no object of {key} is stored as {value!r} (nor under another spelling). If it isn't in this layer, ask_user or cannot_do.")
        return None

    def category_value(self, key: str, fname: str, value, action: str, v: Verdict):
        """A filter value on a text, category or yes/no field: the stored value, or None."""
        ftype = self.fields_of(key).get(fname, "")
        if isinstance(value, bool):
            value = "true" if value else "false"
        value = str(value).strip()
        base = key
        if fname in self.added.get(key, {}) or (key, fname) in self.origin:
            # A field an earlier step added: a joined field has its area layer's values; others aren't known yet.
            src = self.origin.get((key, fname))
            if not src:
                return value
            base, fname = src
        elif key.startswith("#"):
            base = (self.outputs.get(int(key[1:])) or {}).get("base") or ""
            if not base or base.startswith("#") or fname not in data.fields(base):
                return value  # a field the plan made: its values aren't known before running
        rows = data.rows(base)
        stored = {r.get(fname) for r in rows if r.get(fname) is not None} | set(data.categories(base, fname) or [])
        exact = [s for s in stored if s.casefold() == value.casefold()]
        if exact:
            if exact[0] != value:
                v.fixes.append(f"{fname} value {value!r} -> {exact[0]!r}")
            return exact[0]
        cands = resolve.match_field(base, fname, value)
        if cands and cands[0]["score"] >= 0.88 and not [c for c in cands[1:] if c["score"] >= cands[0]["score"] - 0.05]:
            v.fixes.append(f"{fname} value {value!r} -> {cands[0]['value']!r}")
            return cands[0]["value"]
        kind = "category" if ftype.startswith(("category", "yes/no")) else "stored value"
        if cands:
            names = ", ".join(repr(c["value"]) for c in cands[:5])
            v.errors.append(f"{action}: {value!r} is not a {kind} of {key}.{fname}. Matching {kind}s: {names}. Use the one(s) the user meant (any_of for several), or ask_user.")
        else:
            v.errors.append(f"{action}: {value!r} matches no {kind} of {key}.{fname}. Try resolve_feature with the user's words, or ask_user.")
        return None

    @staticmethod
    def number(x):
        if isinstance(x, bool):
            return None
        if isinstance(x, (int, float)):
            return x
        t = str(x).strip().replace(",", "").replace("_", "").replace(" ", "")
        try:
            f = float(t)
        except ValueError:
            return None
        return int(f) if f.is_integer() and "." not in t and "e" not in t.lower() else f

    def conditions(self, key: str, conds, action: str, v: Verdict):
        if isinstance(conds, str):
            try:
                conds = json.loads(conds)
            except ValueError:
                v.errors.append(f"{action}: conditions must be a list of {{field, op, value}}.")
                return None
        if isinstance(conds, dict):
            conds = [conds]
        if not isinstance(conds, list):
            v.errors.append(f"{action}: conditions must be a list ([] clears the filter).")
            return None
        out = []
        for c in conds:
            if not isinstance(c, dict):
                v.errors.append(f"{action}: each condition is {{field, op, value}}.")
                return None
            c = {k: x for k, x in c.items() if x not in (None, "") or k == "value"}
            f = self.field(key, c.get("field"), action, v, role="condition field")
            if f is None:
                return None
            c["field"] = f
            op = str(c.get("op", "")).strip().lower().replace(" ", "_")
            if op in OP_FIX:
                v.fixes.append(f"op {c.get('op')!r} -> {OP_FIX[op]!r}")
                op = OP_FIX[op]
            if op not in tools_v2.OPS:
                v.errors.append(f"{action}: unknown op {c.get('op')!r}; use one of {', '.join(tools_v2.OPS)}.")
                return None
            c["op"] = op
            ftype = self.fields_of(key)[f]
            if op == "is_empty":
                c.pop("value", None)
                out.append(c)
                continue
            if op == "any_of":
                vals = c.get("values")
                if vals in (None, []) and c.get("value") not in (None, ""):
                    vals = c["value"] if isinstance(c["value"], list) else [c["value"]]
                    c.pop("value", None)
                if not isinstance(vals, list) or not vals:
                    v.errors.append(f"{action}: any_of needs values [...].")
                    return None
                fixed = []
                for x in vals:
                    if data.is_numeric(ftype):
                        n = self.number(x)
                        if n is None:
                            v.errors.append(f"{action}: {f} is a number; {x!r} isn't one.")
                            return None
                        fixed.append(str(n))
                    else:
                        y = self.category_value(key, f, x, action, v)
                        if y is None:
                            return None
                        fixed.append(y)
                c["values"] = fixed
                out.append(c)
                continue
            if op in ("above", "below", "at_least", "at_most", "between"):
                if not data.is_numeric(ftype):
                    v.errors.append(f"{action}: {op} needs a numeric field; {key}.{f} is {ftype}.")
                    return None
                for k in ("value", "value2") if op == "between" else ("value",):
                    n = self.number(c.get(k))
                    if n is None:
                        v.errors.append(f"{action}: {k} of {f} {op} must be a number (in the field's unit); got {c.get(k)!r}.")
                        return None
                    if str(c.get(k)) != str(n):
                        v.fixes.append(f"{k} {c.get(k)!r} -> {n}")
                    c[k] = str(n)
                out.append(c)
                continue
            if op in ("before", "after"):
                if "date" not in ftype and "time" not in ftype:
                    alt = "above" if op == "after" else "below"
                    v.errors.append(f"{action}: {op} is for dates; {key}.{f} is {ftype}" + (f": use {alt}." if data.is_numeric(ftype) else "."))
                    return None
                out.append(c)
                continue
            if c.get("value") in (None, ""):
                v.errors.append(f"{action}: condition on {f} has no value.")
                return None
            if op in ("contains", "starts_with", "ends_with"):
                out.append(c)  # a part of a stored text: no stored value to match
                continue
            # is / is_not
            if data.is_numeric(ftype):
                n = self.number(c["value"])
                if n is None:
                    v.errors.append(f"{action}: {f} is a number; {c['value']!r} isn't one.")
                    return None
                c["value"] = str(n)
            else:
                y = self.category_value(key, f, c["value"], action, v)
                if y is None:
                    return None
                c["value"] = y
            out.append(c)
        return out

    # -----------------------------------------------------------------------------------------
    # One call

    def check(self, call: dict) -> Verdict:
        name = call.get("name") or ""
        args = dict(call.get("args") or {}) if isinstance(call.get("args"), dict) else {}
        v = Verdict(ok=True, call={"name": name, "args": args})
        if not self.enabled or name in ("ask_user", "cannot_do"):
            return v
        for k, step in enumerate(self.steps, 1):
            if _canon(step) == _canon(v.call):
                v.duplicate_of = k
                return v
        if name in ACTIONS:
            self._check_action(name, args, v)
        elif name in DISPLAY_REQUIRED:
            self._check_display(name, args, v)
        else:
            v.errors.append(f"There is no tool {name!r}.")
        v.ok = not v.errors
        if v.ok:
            for k, step in enumerate(self.steps, 1):
                if _canon(step) == _canon(v.call):
                    v.duplicate_of = k
        return v

    def _check_action(self, name: str, args: dict, v: Verdict) -> None:
        a = ACTIONS[name]
        out: dict = {}
        if name == "find_where" and args.get("raster") in (None, "") and isinstance(args.get("conditions"), list) and args["conditions"]:
            c0 = args["conditions"][0]
            if isinstance(c0, dict) and (c0.get("raster") or c0.get("field")):
                args = {**args, "raster": c0.get("raster") or c0.get("field")}
        for role, kind in a["roles"]:
            given = args.get(role)
            if kind == "layer":
                need = SLOT_NEEDS.get(role, "vector")
                if given in (None, ""):
                    v.errors.append(f"{name}: {role} is missing.")
                    continue
                key = self.layer(given, need, role, name, v)
                if key:
                    out[role] = key
            else:
                optional = kind.endswith("?")
                if given in (None, "", {}):
                    if not optional:
                        v.errors.append(f"{name}: {role} is missing (one object {{layer, field, value}}).")
                    continue
                need = SLOT_NEEDS.get(role, "vector")
                if name == "buffer" and isinstance(out.get("layer"), str):
                    need = "vector"
                o = self.object(given, need, role, name, v, optional)
                if o is not None:
                    if name == "buffer" and out.get("layer") and o["layer"] != out["layer"]:
                        v.errors.append(f"buffer: object must be one object of {out['layer']} (got {o['layer']}).")
                    out[role] = o
        # A buffer of a layer that has a selection, when the request points at it ("this", "selected"):
        # the runner buffers the selected rows, which the plan states as object {layer} (heuristic).
        if name == "buffer" and "object" not in out and out.get("layer") and (self.selection or {}).get("layer") == out["layer"] \
                and re.search(SELECTION_WORDS, self.request, re.I):
            out["object"] = {"layer": out["layer"]}
            v.fixes.append(f"object -> the selected object of {out['layer']} (the request refers to the selection)")
        # parameters
        if name == "buffer":
            d = args.get("distance")
            if d in (None, ""):
                v.errors.append("buffer: distance is missing. If the user gave none, ask_user for it.")
            else:
                m = scoring.parse_distance_app(d)
                if m is None:
                    m2 = scoring.parse_distance_lenient(d)
                    if m2 is None:
                        v.errors.append(f"buffer: distance {d!r} doesn't parse; write a number and unit: '500 m', '2 km', '3 mi', '100 ft'.")
                    else:
                        fixed = f"{m2:g} m"
                        v.fixes.append(f"distance {d!r} -> {fixed!r}")
                        out["distance"] = fixed
                else:
                    out["distance"] = d
            if "merge" in args:
                mg = scoring.as_bool(args["merge"])
                if mg is None:
                    v.errors.append("buffer: merge is true or false.")
                else:
                    out["merge"] = mg
        if "band" in args and args["band"] not in (None, ""):
            b = self.number(args["band"])
            if b != 1:
                v.errors.append(f"{name}: band {args['band']!r}: the raster has 1 band.")
            else:
                out["band"] = 1
        if name in STATS:
            st = args.get("stat")
            default = "count" if name == "count_in" else "mean"
            if st in (None, ""):
                st = default
            st = str(st).strip().lower()
            if st in STAT_FIX:
                v.fixes.append(f"stat {args.get('stat')!r} -> {STAT_FIX[st]!r}")
                st = STAT_FIX[st]
            if st not in STATS[name]:
                v.errors.append(f"{name}: stat {args.get('stat')!r} isn't offered; use one of {', '.join(STATS[name])}.")
            else:
                if st != default or "stat" in args:
                    out["stat"] = st
            if name == "count_in" and st not in ("count",) and st in STATS[name]:
                if out.get("points"):
                    f = self.field(out["points"], args.get("field"), name, v, numeric=True)
                    if f:
                        out["field"] = f
            elif name == "count_in" and args.get("field") not in (None, ""):
                v.fixes.append(f"field {args.get('field')!r} dropped (stat count counts points)")
        if name == "rank_by_value" and out.get("layer"):
            f = self.field(out["layer"], args.get("field"), name, v, numeric=True)
            if f:
                out["field"] = f
        if name == "calculate_field" and out.get("layer"):
            self._check_calculate(out, args, v)
        if name == "join_by_location" and out.get("areas") and args.get("fields") not in (None, ""):
            fs = args["fields"] if isinstance(args["fields"], list) else str(args["fields"]).split(",")
            got = [self.field(out["areas"], f, name, v, role="fields") for f in fs if str(f).strip()]
            if got and all(got):
                out["fields"] = ",".join(got)
        if name == "find_where":
            self._check_where(out, args, v)
        v.call = {"name": name, "args": out}

    def _check_calculate(self, out: dict, args: dict, v: Verdict) -> None:
        key = out["layer"]
        if args.get("a") in (None, ""):
            nums = [f for f, t in self.fields_of(key).items() if data.is_numeric(t)]
            v.errors.append(f"calculate_field: the first value (first; a in the per-step tools) is missing: a number field of {key} "
                            f"(the step is first op second, e.g. first: {nums[-1] if nums else 'population'!r}, op: 'divide', "
                            f"second: a field or a number, name: the new field).")
            a = None
        else:
            a = self.field(key, args.get("a"), "calculate_field", v, numeric=True, role="a")
        if a:
            out["a"] = a
        op = str(args.get("op") or "subtract").strip().lower()
        op = {"minus": "subtract", "plus": "add", "times": "multiply", "-": "subtract", "+": "add", "*": "multiply", "/": "divide",
              "sub": "subtract", "mul": "multiply", "div": "divide"}.get(op, op)
        if op not in FIELD_OP_SIGN:
            v.errors.append(f"calculate_field: op {args.get('op')!r} isn't offered; use add, subtract, multiply or divide.")
        else:
            out["op"] = op
        b = args.get("b")
        if b in (None, ""):
            v.errors.append("calculate_field: the second value (second; b in the per-step tools) is missing: a number field or a number.")
        elif self.number(b) is not None and str(b).strip() not in self.fields_of(key):
            out["b"] = str(self.number(b))
        else:
            fb = self.field(key, b, "calculate_field", v, numeric=True, role="b")
            if fb:
                out["b"] = fb
        if args.get("name") not in (None, ""):
            out["name"] = str(args["name"]).strip()

    def _check_where(self, out: dict, args: dict, v: Verdict) -> None:
        conds = args.get("conditions")
        if isinstance(conds, dict):
            conds = [conds]
        if not isinstance(conds, list) or not conds:
            v.errors.append("find_where: conditions are missing: [{field: <raster name>, op: at_least|at_most|between, value, value2}].")
            return
        fixed = []
        for c in conds:
            if not isinstance(c, dict):
                v.errors.append("find_where: each condition is {field: <raster name>, op, value, value2}.")
                return
            r = c.get("raster") or c.get("field") or out.get("raster")
            key = self.layer(r, "raster", "condition raster", "find_where", v)
            if key is None:
                return
            op = str(c.get("op") or "").strip().lower().replace(" ", "_")
            op = OP_FIX.get(op, op)
            if op not in WHERE_OPS:
                v.errors.append(f"find_where: op {c.get('op')!r} isn't offered; use at_least, at_most or between.")
                return
            item = {"field": key, "op": WHERE_OPS[op]}
            for k in ("value", "value2") if WHERE_OPS[op] == "between" else ("value",):
                n = self.number(c.get(k))
                if n is None:
                    v.errors.append(f"find_where: {k} of {key} {op} must be a number in the raster's unit; got {c.get(k)!r}.")
                    return
                item[k] = str(n)
            fixed.append(item)
        out["conditions"] = fixed
        if "raster" not in out and fixed:
            out["raster"] = fixed[0]["field"]

    def _check_display(self, name: str, args: dict, v: Verdict) -> None:
        out: dict = {}
        for r in DISPLAY_REQUIRED[name]:
            if r not in args or (args[r] in (None, "") and r != "conditions"):
                v.errors.append(f"{name}: {r} is missing.")
        key = self.layer(args.get("layer"), "vector", "layer", name, v) if args.get("layer") else None
        if key is None:
            v.call = {"name": name, "args": args}
            return
        out["layer"] = key
        if name == "filter":
            conds = self.conditions(key, args.get("conditions", []), name, v)
            if conds is not None:
                out["conditions"] = conds
                if not conds and not any(w in self.request.casefold() for w in CLEAR_WORDS):
                    v.errors.append("filter: conditions [] clears the filter, and the request doesn't ask to clear it. "
                                    "Add the condition (resolve_feature finds stored categories), or ask_user.")
        elif name == "color_by":
            f = self.field(key, args.get("field"), name, v)
            if f:
                out["field"] = f
        elif name == "zoom_to":
            if args.get("value") not in (None, ""):
                o = self.object({"layer": key, "field": args.get("field"), "value": args.get("value")}, "vector", "object", name, v)
                if o:
                    out.update({k: o[k] for k in ("field", "value") if k in o})
        v.call = {"name": name, "args": out}

    # -----------------------------------------------------------------------------------------
    # Accepting a step

    def add(self, call: dict) -> int:
        self.steps.append(call)
        k = len(self.steps)
        n, a = call["name"], call.get("args") or {}
        if n == "count_in":
            pts, areas = a.get("points"), a.get("areas")
            if isinstance(pts, str) and isinstance(areas, str):
                f = self.fields_of(areas)
                things = _word(pts) or "points"
                f[things] = "integer"
                st = a.get("stat", "count")
                name = f"{things[:1].upper()}{things[1:]} in {_each(self._name(areas))}"
                if st != "count" and a.get("field"):
                    sf = f"{POINT_STAT_WORD.get(st, st)} {a['field']}"
                    f[sf] = "number"
                    name = f"{POINT_STAT_WORD.get(st, st).capitalize()} {a['field']} of the {things} in {_each(self._name(areas))}"
                self.outputs[k] = {"kind": "areas", "fields": f, "name": name, "base": areas}
        elif n == "summarize_each":
            r, areas = a.get("raster"), a.get("areas")
            if isinstance(r, str) and isinstance(areas, str):
                f = self.fields_of(areas)
                st = a.get("stat", "mean")
                if "categories" in (ctxmod.LAYERS.get(r, {}).get("desc") or ""):
                    self.outputs[k] = {"kind": "areas", "fields": f, "name": f"{r} in {_each(self._name(areas))}", "class_prefix": r, "base": areas}
                else:
                    for w in RASTER_STAT_WORD.values():
                        f[f"{r} {w}"] = "number"
                    f[f"{r} coverage %"] = "number"
                    name = f"{r} in {_each(self._name(areas))}" if st == "mean" else f"{RASTER_STAT_TITLE.get(st, st)} {r} in {_each(self._name(areas))}"
                    self.outputs[k] = {"kind": "areas", "fields": f, "name": name, "base": areas}
        elif n == "buffer":
            lyr = a.get("layer")
            # The runner's buffer layer has one field, "name": the object's title (or the layer's, merged).
            self.outputs[k] = {"kind": "areas", "fields": {"name": "text"}, "name": "", "base": None}
        elif n == "add_raster_values":
            r, pts = a.get("raster"), a.get("points")
            if isinstance(r, str) and isinstance(pts, str):
                self._add_fields(pts, [(r, "number")])
        elif n == "split_parts" and isinstance(a.get("layer"), str):
            lyr = a["layer"]
            f = self.fields_of(lyr)
            f[self._free(f, "part")] = "integer"
            name = f"{self._plural_noun(lyr)[:1].upper()}{self._plural_noun(lyr)[1:]} in separate parts"
            self.outputs[k] = {"kind": self.kind_of(lyr) or "areas", "fields": f, "name": name, "base": lyr}
        elif n == "find_where":
            f = {"name": "text", "area_km2": "number", "cells": "integer"}
            rasters = []
            for c in a.get("conditions") or []:
                if isinstance(c, dict) and isinstance(c.get("field"), str) and c["field"] not in rasters:
                    rasters.append(c["field"])
            for r in rasters:
                f[f"{r} mean"] = "number"
            self.outputs[k] = {"kind": "areas", "fields": f, "name": "Where " + " and ".join(rasters), "base": None}
        elif n in ("keep_inside", "remove_inside") and isinstance(a.get("layer"), str) and isinstance(a.get("areas"), str):
            lyr = a["layer"]
            word = "inside" if n == "keep_inside" else "outside"
            pn = self._plural_noun(lyr)
            self.outputs[k] = {"kind": self.kind_of(lyr) or "areas", "fields": self.fields_of(lyr),
                               "name": f"{pn[:1].upper()}{pn[1:]} {word} {self._name(a['areas'])}", "base": lyr}
        elif n in ("nearest_distance", "nearest_other") and isinstance(a.get("layer"), str):
            lyr = a["layer"]
            if n == "nearest_other":
                words = f"nearest other {self._singular_noun(lyr)}"
            elif isinstance(a.get("other"), str):
                words = f"nearest {self._singular_noun(a['other'])}"
            else:
                return k
            self._add_fields(lyr, [(f"km to {words}", "number"), (words, "text")])
        elif n == "join_by_location" and isinstance(a.get("layer"), str) and isinstance(a.get("areas"), str):
            af = self.fields_of(a["areas"])
            wanted = [x.strip() for x in str(a.get("fields") or "").split(",") if x.strip()] or list(af)
            got = self._add_fields(a["layer"], [(w, af.get(w, "text")) for w in wanted if w in af])
            src = a["areas"]
            if src.startswith("#"):
                src = (self.outputs.get(int(src[1:])) or {}).get("base") or src
            for w, name in zip([w for w in wanted if w in af], got):
                if not src.startswith("#"):
                    self.origin[(a["layer"], name)] = (src, w)
        elif n == "add_measurements" and isinstance(a.get("layer"), str):
            kind = self.kind_of(a["layer"])
            new = [("area_km2", "number"), ("perimeter_km", "number")] if kind in ("areas", "drawn area") else [("length_km", "number")] if kind == "lines" else []
            self._add_fields(a["layer"], new + [("longitude", "number"), ("latitude", "number")])
        elif n == "calculate_field" and isinstance(a.get("layer"), str) and a.get("a"):
            name = a.get("name") or f"{a.get('a')} {FIELD_OP_SIGN.get(a.get('op') or 'subtract', '−')} {a.get('b')}"
            self._add_fields(a["layer"], [(name, "number")])
        return k

    @staticmethod
    def _free(fields: dict, base: str) -> str:
        """``free_field`` in the runner: the first name not taken (case-insensitive), 'x', 'x 2', ..."""
        taken = {f.casefold() for f in fields}
        if base.casefold() not in taken:
            return base
        i = 2
        while f"{base} {i}".casefold() in taken:
            i += 1
        return f"{base} {i}"

    def _add_fields(self, key: str, new: list[tuple[str, str]]) -> list[str]:
        """Fields a step adds to a layer, named as the runner names them; returns the names."""
        names = []
        for fname, ftype in new:
            have = self.fields_of(key)
            fname = self._free(have, fname)
            if key.startswith("#"):
                o = self.outputs.get(int(key[1:]))
                if o is not None:
                    o["fields"][fname] = ftype
            else:
                self.added.setdefault(key, {})[fname] = ftype
            names.append(fname)
        return names

    def _plural_noun(self, key: str) -> str:
        w = rs_word(self._name(key))
        return rs_plural(w) if w else KIND_NOUNS.get(self.kind_of(key) or "", ("object", "objects"))[1]

    def _singular_noun(self, key: str) -> str:
        w = rs_word(self._name(key))
        return rs_singular(w) if w else KIND_NOUNS.get(self.kind_of(key) or "", ("object", "objects"))[0]

    def _name(self, key: str) -> str:
        if key.startswith("#"):
            return (self.outputs.get(int(key[1:])) or {}).get("name") or key
        return key

    def accepted_note(self, k: int, v: Verdict) -> str:
        call = self.steps[k - 1]
        n, a = call["name"], call.get("args") or {}
        note = f"Step #{k} added to the plan (not run yet)."
        if n in tools_v2.NEW_LAYER_ACTIONS and k in self.outputs:
            o = self.outputs[k]
            new = [f for f in o["fields"] if f not in self.fields_of(a.get("areas") or a.get("layer") or "")]
            note += f' Later steps use its new layer as "#{k}".'
            if new:
                note += " New fields: " + ", ".join(repr(f) for f in new) + "."
            elif o.get("class_prefix"):
                note += f" New fields: one share field per class ('{o['class_prefix']} <class> share')."
        elif n == "add_raster_values":
            note += f" {a.get('points')} gets field {a.get('raster')!r}; keep using the name {a.get('points')!r} (no new layer)."
        elif n == "filter":
            note += f" {a.get('layer')} now shows only matching objects; keep using its name (a filter makes no new layer)."
        if v.fixes:
            note += " Corrected: " + "; ".join(v.fixes) + "."
        return note

    # -----------------------------------------------------------------------------------------
    # The whole plan

    def reset(self) -> None:
        self.steps, self.outputs, self.added, self.origin = [], {}, {}, {}

    @staticmethod
    def _uses(call: dict, key: str) -> bool:
        for v in (call.get("args") or {}).values():
            if v == key or (isinstance(v, dict) and v.get("layer") == key):
                return True
        return False

    def lint(self, plan: list[dict]) -> list[str]:
        """Data-flow checks over the finished plan: a filter or add-values step whose layer no later
        analysis step uses (e.g. filtering Countries after summarize_each already made "#1" from
        it), when later analysis steps exist."""
        if not self.enabled:
            return []
        errs = []
        for i, c in enumerate(plan):
            if c["name"] not in ("filter", "add_raster_values"):
                continue
            key = (c.get("args") or {}).get("layer" if c["name"] == "filter" else "points")
            later = [x for x in plan[i + 1 :] if x["name"] in ACTIONS]
            if not isinstance(key, str) or not later or any(self._uses(x, key) for x in plan[i + 1 :]):
                continue
            errs.append(f"Step #{i + 1} ({c['name']}) changes {key}, but no later step uses {key}, so it has no effect on the "
                        f"result. Put it before the step that should use it, or apply it to the layer the later steps use.")
        return errs

    def finalize(self, plan: list[dict]) -> tuple[list[dict], list[dict]]:
        """Drop trailing display-only steps the request doesn't ask for (see the module doc)."""
        if not self.enabled:
            return plan, []
        req = self.request.casefold()
        out = list(plan)
        dropped = []
        while out and out[-1]["name"] in DISPLAY_ONLY and len(out) > 1:
            words = DISPLAY_WORDS[out[-1]["name"]]
            if any(w in req for w in words):
                break
            dropped.insert(0, out.pop())
        return out, dropped

    def check_plan(self, calls: list[dict]) -> tuple[list[dict], list[str], list[str]]:
        """Check a whole plan at once (submit_plan): (fixed calls, errors, fixes). Resets state."""
        self.steps, self.outputs, self.added, self.origin = [], {}, {}, {}
        fixed, errors, fixes = [], [], []
        for i, c in enumerate(calls, 1):
            v = self.check(c)
            if v.duplicate_of:
                fixes.append(f"step {i} repeats step {v.duplicate_of}; dropped")
                continue
            if not v.ok:
                errors += [f"step {i}: {e}" for e in v.errors]
                # keep going with the call as given, so later steps' "#k" still line up
                self.add(c)
                fixed.append(c)
                continue
            fixes += [f"step {i}: {x}" for x in v.fixes]
            self.add(v.call)
            fixed.append(v.call)
        metas = [c for c in fixed if c["name"] in ("ask_user", "cannot_do")]
        if metas and len(fixed) > 1:
            errors.append("ask_user or cannot_do must be the only step: either plan the steps or ask/refuse.")
        if not errors and not metas:
            errors += self.lint(fixed)
        return fixed, errors, fixes
