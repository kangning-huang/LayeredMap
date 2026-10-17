"""Shared code for the public-benchmark suite: map contexts, the action catalogue, helpers to
write expected plans, and the plan validator.

It reads ``tools/lm/llm_eval`` (context, tools, scoring) without changing it. The action
catalogue comes from the same sources as the eval's tools:

* connected actions, their inputs (roles) and the eval's parameters: ``llm_eval/tools.py``;
* every parameter the runner reads (``plan.param("…")``), per action, and the allowed ``stat``
  values (``POINT_STATS``, ``RASTER_STATS``): ``crates/lm-jobs/src/actions.rs``.

Map contexts are the eval's own (``world``, ``shanghai``, ``shanghai_drawn``) plus the JSON files
in ``contexts/`` (layer names, kinds, counts and field types, never values).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
LLM_EVAL = ROOT / "tools/lm/llm_eval"
RUNNER = ROOT / "crates/lm-jobs/src/actions.rs"
CONTEXT_DIR = HERE / "contexts"
SOURCE_DIR = HERE / "sources"

for _p in (LLM_EVAL, HERE):
    if str(_p) not in sys.path:
        sys.path.insert(0, str(_p))

import context as ctxmod  # noqa: E402
import tools as toolmod  # noqa: E402

COVERAGE = ("now", "upstream", "new", "out_of_scope")
CONFIDENCE = ("high", "medium", "low")
TEXT_POLICY = ("verbatim", "adapted", "own_words")

# ---------------------------------------------------------------------------------------------
# Contexts


def _eval_layer(name: str) -> dict:
    L = ctxmod.LAYERS[name]
    out = {"name": name, "kind": L["kind"]}
    if L["kind"] == "raster":
        out["desc"] = L["desc"]
    else:
        out["count"] = L["count"]
        out["fields"] = [list(f) for f in L["fields"]]
    return out


def load_contexts() -> dict[str, list[dict]]:
    """{context name: [layer, ...]}; a layer is {name, kind, count, fields} or {name, kind, desc}."""
    out = {c: [_eval_layer(n) for n in names] for c, names in ctxmod.CONTEXTS.items()}
    raw = {}
    for p in sorted(CONTEXT_DIR.glob("*.json")):
        if p.name.startswith("._"):
            continue
        d = json.loads(p.read_text(encoding="utf-8"))
        if d["name"] != p.stem:
            raise ValueError(f"{p.name}: name {d['name']!r} differs from the file name")
        if d["name"] in out:
            raise ValueError(f"{p.name}: context {d['name']!r} already exists in llm_eval")
        raw[d["name"]] = d

    def resolve(name: str, seen: tuple = ()) -> list[dict]:
        if name in out:
            return out[name]
        if name in seen:
            raise ValueError(f"context base cycle: {seen}")
        d = raw[name]
        layers = (resolve(d["base"], seen + (name,)) if d.get("base") else []) + d["layers"]
        names = [x["name"] for x in layers]
        if len(set(names)) != len(names):
            raise ValueError(f"context {name}: duplicate layer names")
        out[name] = layers
        return layers

    for n in raw:
        resolve(n)
    return out


CONTEXTS = load_contexts()


def layer_line(i: int, L: dict, with_values: bool) -> str:
    """The eval's layer line (context.layer_line), for a layer given as a dict."""
    name = L["name"]
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


def system_message(context: str, selection: dict | None = None, with_values: bool = False, prompt: str = ctxmod.DEFAULT_PROMPT) -> str:
    """The eval's system message (context.system_message) for any context, ours included."""
    lines = [ctxmod.INSTRUCTIONS[prompt], "", "Open layers:"]
    for i, L in enumerate(CONTEXTS[context], 1):
        lines.append(layer_line(i, L, with_values))
    lines.append(ctxmod.selection_line(selection))
    return "\n".join(lines)


def install_contexts() -> None:
    """Let ``llm_eval/run_eval.py`` build system messages for our contexts too (in memory only)."""
    if getattr(ctxmod, "_bench_installed", False):
        return
    original = ctxmod.system_message

    def patched(context, selection=None, with_values=False, prompt=ctxmod.DEFAULT_PROMPT):
        if context in ctxmod.CONTEXTS:
            return original(context, selection, with_values, prompt)
        return system_message(context, selection, with_values, prompt)

    ctxmod.system_message = patched
    ctxmod._bench_installed = True


# ---------------------------------------------------------------------------------------------
# The action catalogue


def _runner_params(src: str) -> dict[str, set[str]]:
    """{action id: parameters its runner function reads}, from `run`'s match arms."""
    body = re.search(r"pub fn run\(plan: &Plan.*?\n\}\n", src, re.S)
    if not body:
        raise ValueError("run() not found in actions.rs")
    arms = re.findall(r'((?:"[a-z_]+"\s*\|?\s*)+)=>\s*([a-z_]+)\(plan', body.group(0))
    fns: dict[str, str] = {}
    for m in re.finditer(r"\nfn ([a-z_]+)\(plan: &Plan[^\n]*\n(.*?)\n\}\n", src, re.S):
        fns[m.group(1)] = m.group(2)
    out: dict[str, set[str]] = {}
    for ids, fn in arms:
        params = set(re.findall(r'plan\.param\("([a-z_]+)"\)', fns.get(fn, "")))
        if "band(plan" in fns.get(fn, ""):
            params.add("band")
        for a in re.findall(r'"([a-z_]+)"', ids):
            out[a] = params
    return out


def _stats(src: str, const: str) -> list[str]:
    m = re.search(rf"pub const {const}: &\[\(&str, &str\)\] =\s*&\[(.*?)\];", src, re.S)
    if not m:
        raise ValueError(f"{const} not found in actions.rs")
    return re.findall(r'\("([a-z_]+)", "', m.group(1))


def _stat_labels(src: str, const: str) -> dict[str, str]:
    m = re.search(rf"pub const {const}: &\[\(&str, &str\)\] =\s*&\[(.*?)\];", src, re.S)
    return dict(re.findall(r'\("([a-z_]+)", "([^"]+)"\)', m.group(1))) if m else {}


SRC = RUNNER.read_text(encoding="utf-8")
POINT_STATS = _stats(SRC, "POINT_STATS")
RASTER_STATS = _stats(SRC, "RASTER_STATS")
POINT_STAT_LABELS = _stat_labels(SRC, "POINT_STATS")

PARAM_TYPES = {"distance": "distance", "merge": "bool", "band": "int", "field": "field", "stat": "stat"}


def action_specs() -> dict[str, dict]:
    """{name: {"roles": {arg: kind}, "params": [..], "required": {..}}} for every tool a plan may use.

    Role kinds: "layer" (a layer name), "object" (one object), "object?" (optional object).
    """
    runner = _runner_params(SRC)
    specs: dict[str, dict] = {}
    for a in toolmod.action_table():
        roles = dict(a["roles"])
        params = list(dict.fromkeys(list(a["params"]) + sorted(runner.get(a["id"], set()))))
        required = {n for n, k in a["roles"] if not k.endswith("?")}
        if a["id"] == "rank_by_value":
            required.add("field")
        if a["id"] == "buffer":
            required.add("distance")
        specs[a["id"]] = {"roles": roles, "params": params, "required": required}
    specs["filter"] = {"roles": {"layer": "layer"}, "params": ["conditions"], "required": {"layer", "conditions"}}
    specs["color_by"] = {"roles": {"layer": "layer"}, "params": ["field"], "required": {"layer", "field"}}
    specs["view_data"] = {"roles": {"layer": "layer"}, "params": [], "required": {"layer"}}
    specs["zoom_to"] = {"roles": {"target": "object"}, "params": [], "required": {"target"}}
    specs["ask_user"] = {"roles": {}, "params": [], "required": set()}
    specs["cannot_do"] = {"roles": {}, "params": [], "required": set()}
    return specs


SPECS = action_specs()
NEW_LAYER = {"count_in": "areas", "buffer": "areas", "summarize_each": "areas"}
FILTER_OPS = {"is", "is_not", "contains", "is_empty", "above", "below", "between", "before", "after", "any_of"}

# Which layer kinds each role accepts (rules.toml `inputs` and the runner's table).
ROLE_KINDS = {
    "points": {"points"},
    "areas": {"areas"},
    "raster": {"raster"},
    "point": {"points"},
    "area": {"areas", "drawn area"},
    "line": {"lines"},
}
# Per action, for the generic role names.
ACTION_ROLE_KINDS = {
    ("buffer", "layer"): {"points", "lines"},
    ("buffer", "object"): {"points", "lines"},
    ("whats_inside", "layer"): {"points", "lines", "areas"},
    ("summarize_data_here", "layer"): {"points", "lines", "areas"},
    ("rank_by_value", "layer"): {"points", "lines", "areas"},
    ("filter", "layer"): {"points", "lines", "areas"},
    ("color_by", "layer"): {"points", "lines", "areas"},
    ("view_data", "layer"): {"points", "lines", "areas", "drawn area"},
    ("zoom_to", "target"): {"points", "lines", "areas", "drawn area"},
}


# ---------------------------------------------------------------------------------------------
# Writing expected plans (the eval's case schema; see llm_eval/make_cases.py)


def obj(layer, *pairs, lenient=()):
    """One object named by (field, value) pairs: obj("Countries", ("name", "Brazil"), ("iso_a3", "BRA"))."""
    return {"layer": layer, "any": [list(p) for p in pairs], "lenient": list(lenient)}


def sel(layer):
    """The selected object (or a drawn area) of a layer; layer may be "#k" or a list of names."""
    return {"layer": layer, "selected": True}


def dist(metres: float) -> dict:
    return {"distance_m": metres}


def cond(field, op, value=None, value2=None, values=None, tol=None) -> dict:
    c = {"field": field, "op": op}
    if value is not None:
        c["value"] = value
    if value2 is not None:
        c["value2"] = value2
    if values is not None:
        c["values"] = list(values)
    if tol is not None:
        c["tol"] = tol
    return c


def call(name: str, **args) -> dict:
    return {"name": name, "args": args}


CANNOT = [{"name": "cannot_do", "args": {}}]
ASK = [{"name": "ask_user", "args": {}}]


def case(cid, request, expected, *, context, lang="en", selection=None, alts=(), lenient_alts=(), notes="") -> dict:
    return {
        "id": cid,
        "lang": lang,
        "context": context,
        "selection": selection,
        "request": request,
        "expected": expected,
        "alts": [list(p) for p in alts],
        "lenient_alts": [list(p) for p in lenient_alts],
        "notes": notes,
    }


# ---------------------------------------------------------------------------------------------
# Validating a plan against its context


def _options(x) -> list:
    return x if isinstance(x, list) else [x]


REF = re.compile(r"#(\d+)$")


class PlanState:
    """Layers visible to step k: the context's, plus new layers and fields made by earlier steps."""

    def __init__(self, context: str):
        self.layers = {L["name"]: {"kind": L["kind"], "fields": {f[0] for f in L.get("fields", [])}, "desc": L.get("desc", "")} for L in CONTEXTS[context]}
        self.refs: dict[int, dict] = {}

    def resolve(self, name: str, step: int, errors: list, where: str) -> dict | None:
        m = REF.match(name) if isinstance(name, str) else None
        if m:
            k = int(m.group(1))
            if not 1 <= k < step:
                errors.append(f"{where}: {name} does not name an earlier step")
                return None
            if k not in self.refs:
                errors.append(f"{where}: step #{k} makes no new layer")
                return None
            return self.refs[k]
        if name not in self.layers:
            errors.append(f"{where}: no layer {name!r} in the context")
            return None
        return self.layers[name]


def _check_layer_arg(st: PlanState, value, step: int, kinds: set[str] | None, errors: list, where: str) -> dict | None:
    """A layer argument: a name, "#k", or a list of alternatives where at least one resolves and
    names that don't resolve are allowed only beside a "#k" (a new layer's display name)."""
    opts = _options(value)
    if not opts or not all(isinstance(o, str) for o in opts):
        errors.append(f"{where}: expected a layer name, got {value!r}")
        return None
    has_ref = any(REF.match(o) for o in opts)
    found = None
    for o in opts:
        sub: list = []
        L = st.resolve(o, step, sub, where)
        if L is None:
            if REF.match(o) or not has_ref:
                errors.extend(sub)
            continue
        if kinds and L["kind"] not in kinds:
            errors.append(f"{where}: {o!r} is {L['kind']}, needs {' or '.join(sorted(kinds))}")
            continue
        found = found or L
    if found is None and not errors:
        errors.append(f"{where}: no usable layer in {opts!r}")
    return found


def _check_field(L: dict | None, field, errors: list, where: str, numeric_ok=True) -> None:
    if L is None:
        return
    if L.get("any_field"):
        return
    opts = _options(field)
    if not any(isinstance(f, str) and f in L["fields"] for f in opts):
        errors.append(f"{where}: field {field!r} is not in the layer (has {sorted(L['fields'])})")


def _check_object(st: PlanState, action: str, role: str, value, step: int, errors: list, where: str) -> dict | None:
    if not isinstance(value, dict) or "layer" not in value:
        errors.append(f"{where}: expected an object {{layer, any|selected}}, got {value!r}")
        return None
    kinds = ACTION_ROLE_KINDS.get((action, role)) or ROLE_KINDS.get(role)
    L = _check_layer_arg(st, value["layer"], step, kinds, errors, where)
    if value.get("selected"):
        if set(value) - {"layer", "selected"}:
            errors.append(f"{where}: a selected object takes only layer and selected")
        return L
    pairs = value.get("any")
    if not isinstance(pairs, list) or not pairs or not all(isinstance(p, list) and len(p) == 2 for p in pairs):
        errors.append(f"{where}: an object needs any=[[field, value], ...] or selected=true")
        return L
    if set(value) - {"layer", "any", "lenient"}:
        errors.append(f"{where}: unknown keys {sorted(set(value) - {'layer', 'any', 'lenient'})}")
    for f, v in pairs:
        _check_field(L, f, errors, f"{where}.any")
        if not isinstance(v, str) or not v:
            errors.append(f"{where}: object value must be non-empty text, got {v!r}")
    return L


def _check_condition(L: dict | None, c, errors: list, where: str) -> None:
    if not isinstance(c, dict):
        errors.append(f"{where}: condition must be an object")
        return
    extra = set(c) - {"field", "op", "value", "value2", "values", "tol"}
    if extra:
        errors.append(f"{where}: unknown condition keys {sorted(extra)}")
    op = c.get("op")
    if op not in FILTER_OPS:
        errors.append(f"{where}: unknown op {op!r}")
    _check_field(L, c.get("field"), errors, where)
    if op == "between" and ("value" not in c or "value2" not in c):
        errors.append(f"{where}: between needs value and value2")
    elif op == "any_of" and not isinstance(c.get("values"), list):
        errors.append(f"{where}: any_of needs values")
    elif op in {"is", "is_not", "contains", "above", "below", "before", "after"} and "value" not in c:
        errors.append(f"{where}: {op} needs a value")
    if op in {"above", "below", "between"}:
        for k in ("value", "value2"):
            if k in c and not (isinstance(c[k], (int, float)) and not isinstance(c[k], bool)):
                errors.append(f"{where}: {op} needs numbers, got {c[k]!r}")


def validate_plan(plan: list, context: str, lenient: bool = False) -> list[str]:
    """Errors in one expected plan (empty when valid). A lenient alternative may leave out
    required arguments (the scorer checks only the arguments a plan gives)."""
    errors: list[str] = []
    if context not in CONTEXTS:
        return [f"unknown context {context!r}"]
    if not isinstance(plan, list) or not plan:
        return ["a plan is a non-empty list of calls"]
    names = [c.get("name") for c in plan if isinstance(c, dict)]
    if any(n in ("ask_user", "cannot_do") for n in names) and len(plan) > 1:
        errors.append("ask_user / cannot_do must be the only call")
    st = PlanState(context)
    for i, c in enumerate(plan, 1):
        where = f"step {i}"
        if not isinstance(c, dict) or set(c) != {"name", "args"} or not isinstance(c["args"], dict):
            errors.append(f"{where}: a call is {{name, args}}")
            continue
        name, args = c["name"], c["args"]
        spec = SPECS.get(name)
        if spec is None:
            errors.append(f"{where}: unknown action {name!r}")
            continue
        where = f"step {i} ({name})"
        allowed = set(spec["roles"]) | set(spec["params"])
        for k in args:
            if k not in allowed:
                errors.append(f"{where}: unknown argument {k!r} (allowed: {sorted(allowed)})")
        for k in spec["required"]:
            if lenient:
                break
            if k not in args and not (name == "zoom_to" and "target" in args):
                errors.append(f"{where}: missing required argument {k!r}")
        resolved: dict[str, dict | None] = {}
        for role, kind in spec["roles"].items():
            if role not in args:
                continue
            if kind == "layer":
                kinds = ACTION_ROLE_KINDS.get((name, role)) or ROLE_KINDS.get(role)
                resolved[role] = _check_layer_arg(st, args[role], i, kinds, errors, f"{where}.{role}")
            else:
                resolved[role] = _check_object(st, name, role, args[role], i, errors, f"{where}.{role}")
        # Parameters
        if "distance" in args:
            d = args["distance"]
            if not (isinstance(d, dict) and set(d) == {"distance_m"} and isinstance(d["distance_m"], (int, float)) and d["distance_m"] > 0):
                errors.append(f"{where}: distance must be {{distance_m: number > 0}}")
        if "merge" in args and not isinstance(args["merge"], bool):
            errors.append(f"{where}: merge must be true or false")
        if "band" in args and not (isinstance(args["band"], int) and args["band"] >= 1):
            errors.append(f"{where}: band must be an integer ≥ 1")
        if "stat" in args:
            allowed_stats = POINT_STATS if name == "count_in" else RASTER_STATS
            if args["stat"] not in allowed_stats:
                errors.append(f"{where}: stat {args['stat']!r} not in {allowed_stats}")
        if name == "count_in":
            if args.get("stat", "count") != "count" and "field" not in args:
                errors.append(f"{where}: stat {args['stat']!r} needs a field of the points")
            if "field" in args:
                _check_field(resolved.get("points"), args["field"], errors, f"{where}.field")
        if name in ("rank_by_value", "color_by") and "field" in args:
            _check_field(resolved.get("layer"), args["field"], errors, f"{where}.field")
        if name == "filter":
            conds = args.get("conditions")
            if not isinstance(conds, list):
                errors.append(f"{where}: conditions must be a list")
            else:
                for j, cc in enumerate(conds):
                    _check_condition(resolved.get("layer"), cc, errors, f"{where}.conditions[{j}]")
        if name == "buffer" and "object" in args and "layer" in args:
            lo = args["object"].get("layer") if isinstance(args["object"], dict) else None
            if lo is not None and _options(lo)[0] not in _options(args["layer"]):
                errors.append(f"{where}: the object must belong to the buffered layer")
        # Effects on later steps
        if name == "add_raster_values" and isinstance(args.get("raster"), str) and isinstance(resolved.get("points"), dict):
            resolved["points"]["fields"].add(args["raster"])
        if name in NEW_LAYER:
            out = {"kind": NEW_LAYER[name], "fields": set(), "desc": ""}
            if name == "count_in":
                base = resolved.get("areas") or {}
                out["fields"] = set(base.get("fields", set()))
                pts = _options(args.get("points"))[0]
                if isinstance(pts, str):
                    out["fields"].add(pts.lower())
                stat = args.get("stat", "count")
                if stat != "count" and "field" in args:
                    out["fields"].add(f"{POINT_STAT_LABELS.get(stat, stat).lower()} {_options(args['field'])[0]}")
            elif name == "summarize_each":
                base = resolved.get("areas") or {}
                out["fields"] = set(base.get("fields", set()))
                r = _options(args.get("raster"))[0]
                rl = resolved.get("raster") or {}
                if "categor" in rl.get("desc", ""):
                    out["any_field"] = True
                for w in ("mean", "median", "minimum", "maximum", "sum", "std dev", "coverage %", "total"):
                    out["fields"].add(f"{r} {w}")
            elif name == "buffer":
                out["fields"] = {"name"}
            st.refs[i] = out
    return errors


CASE_KEYS = {"id", "lang", "context", "selection", "request", "expected", "alts", "lenient_alts", "notes"}


def validate_case(c: dict) -> list[str]:
    errors = []
    missing = CASE_KEYS - set(c)
    if missing:
        errors.append(f"missing keys {sorted(missing)}")
        return errors
    if c["lang"] not in ("en", "zh"):
        errors.append(f"lang {c['lang']!r}")
    if not isinstance(c["request"], str) or not c["request"].strip():
        errors.append("empty request")
    if c["lang"] == "zh" and not re.search(r"[一-鿿]", c["request"]):
        errors.append("a zh case needs Chinese text")
    sel_ = c["selection"]
    if sel_ is not None:
        names = {L["name"] for L in CONTEXTS.get(c["context"], [])}
        if not isinstance(sel_, dict) or sel_.get("layer") not in names:
            errors.append(f"selection {sel_!r} names no layer of the context")
    for label, plans in (("expected", [c["expected"]]), ("alts", c["alts"]), ("lenient_alts", c["lenient_alts"])):
        for j, p in enumerate(plans):
            if label == "lenient_alts" and p in (ASK, CANNOT):
                continue
            for e in validate_plan(p, c["context"], lenient=label == "lenient_alts"):
                errors.append(f"{label}[{j}] {e}" if label != "expected" else f"expected {e}")
    return errors


RECORD_KEYS = {"source", "source_id", "licence", "text_policy", "summary", "category", "coverage", "missing", "oos_reason", "confidence", "notes", "cases"}


def validate_record(r: dict) -> list[str]:
    """Errors in one task record. ``missing`` lists op names from vocab.OPS; the coverage class
    must agree with them: upstream when every missing op has an upstream function, new when at
    least one is "new"."""
    from vocab import CATEGORIES, OPS, OUT_OF_SCOPE_REASONS

    errors = []
    missing = {"source", "source_id", "licence", "text_policy", "summary", "category", "coverage", "cases"} - set(r)
    if missing:
        return [f"missing keys {sorted(missing)}"]
    extra = set(r) - RECORD_KEYS
    if extra:
        errors.append(f"unknown keys {sorted(extra)}")
    if r["coverage"] not in COVERAGE:
        errors.append(f"coverage {r['coverage']!r}")
    if r["text_policy"] not in TEXT_POLICY:
        errors.append(f"text_policy {r['text_policy']!r}")
    if r.get("confidence", "high") not in CONFIDENCE:
        errors.append(f"confidence {r.get('confidence')!r}")
    if r["category"] not in CATEGORIES:
        errors.append(f"category {r['category']!r} is not in vocab.CATEGORIES")
    if not isinstance(r["summary"], str) or len(r["summary"]) < 8:
        errors.append("summary too short")
    ms = r.get("missing") or []
    unknown = [m for m in ms if m not in OPS]
    if unknown:
        errors.append(f"missing ops {unknown} are not in vocab.OPS")
    cases = r["cases"]
    if r["coverage"] in ("upstream", "new"):
        if not ms:
            errors.append(f"{r['coverage']} needs missing=[op, ...]")
        elif not unknown:
            any_new = any(OPS[m][1] == "new" for m in ms)
            if r["coverage"] == "upstream" and any_new:
                errors.append(f"coverage upstream but {[m for m in ms if OPS[m][1] == 'new']} need new algorithms")
            if r["coverage"] == "new" and not any_new:
                errors.append("coverage new but every missing op has an upstream function")
        if cases:
            errors.append("upstream/new tasks carry no cases (their expectation changes when the action lands)")
    elif ms and r["coverage"] == "now":
        errors.append("a 'now' task has no missing ops")
    if r["coverage"] == "out_of_scope":
        if r.get("oos_reason") not in OUT_OF_SCOPE_REASONS:
            errors.append(f"out_of_scope needs oos_reason in {sorted(OUT_OF_SCOPE_REASONS)}")
        if not cases:
            errors.append("an out_of_scope task needs its cannot_do case")
        for c in cases:
            if c.get("expected") != CANNOT:
                errors.append(f"case {c.get('id')}: out_of_scope expects cannot_do")
    elif "oos_reason" in r:
        errors.append("oos_reason is only for out_of_scope")
    if r["coverage"] == "now":
        if not cases:
            errors.append("a 'now' task needs at least one case")
        for c in cases:
            if c.get("expected") == CANNOT or (c.get("expected") == ASK and r["category"] != "vague"):
                errors.append(f"case {c.get('id')}: a 'now' task expects a plan, not cannot_do/ask_user")
    for c in cases:
        for e in validate_case(c):
            errors.append(f"case {c.get('id')}: {e}")
    return errors


def load_sources() -> list[tuple[str, int, dict]]:
    """[(file name, line number, record)] for every sources/*.jsonl."""
    out = []
    for p in sorted(SOURCE_DIR.glob("*.jsonl")):
        if p.name.startswith("._"):
            continue
        for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
            if line.strip():
                out.append((p.name, n, json.loads(line)))
    return out
