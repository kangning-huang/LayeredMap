"""The eight puzzles as a user would ask them: one request at a time (``steps.json``).

    python -I tools/lm/puzzles/stepwise.py --gold                    # the gold steps, request by request
    python -I tools/lm/puzzles/stepwise.py --show-context P3 4       # what the model sees for request 4 of P3
    python -I tools/lm/puzzles/stepwise.py --models anthropic/claude-haiku-5.5 --budget 0.5
    python -I tools/lm/puzzles/stepwise.py --report stepwise-<tag>

The owner's direction (18 Oct 2026): the assistant is judged on single requests a non-expert
would make; the user breaks a puzzle into steps. Every request is asked twice:

* **guided**: the map is in the state the gold steps of the earlier requests leave it in. The
  request is right when its plan leaves the map as the request's gold steps do: the same objects
  shown in every layer (count and the first objects), the same new layers (by size), every gold
  value summary among the plan's (new fields, as range and blanks) and every gold card answer
  among its cards. Extra harmless steps are allowed.
* **on its own**: the map is in the state the model's own earlier plans left it in, as in the
  app. At the end the puzzle's answer is checked as in ``pro_e2e.check_answer``.

Each request sees the map as it is then: the open layers and the layers made earlier (by name),
each layer's fields (added ones included), the filter on each layer with its count, and the
earlier requests. The harness is ``pro_e2e``'s (the v2 ``submit_plan`` design with checker,
repair and ``resolve_feature``). Results: ``results/stepwise-<tag>/<model>.jsonl``; spending is
added to ``results/spend.json``.
"""

from __future__ import annotations

import argparse
import copy
import json
import re
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import pro_e2e as pe  # noqa: E402

STEPS = HERE / "steps.json"
RESULTS = pe.RESULTS
SAME_KIND = ("split_parts", "keep_inside", "remove_inside")
AREA_OUT = ("buffer", "find_where", "summarize_each", "count_in")


# ---------------------------------------------------------------------------------------------
# Plans across requests


def renumber(x, offset: int):
    """A plan's own "#k" -> "#(k + offset)" in the puzzle's running plan."""
    if isinstance(x, str):
        m = re.fullmatch(r"\s*#\s*(\d+)\s*", x)
        return f"#{int(m.group(1)) + offset}" if m else x
    if isinstance(x, list):
        return [renumber(v, offset) for v in x]
    if isinstance(x, dict):
        return {k: renumber(v, offset) for k, v in x.items()}
    return x


def run(layers: list[str], steps: list[dict], limit: int = 3) -> dict:
    return pe.run_plan(layers, steps, limit) if steps else {"ok": True, "steps": [], "layers": []}


# ---------------------------------------------------------------------------------------------
# The map after some steps, as the model sees it


class MapState:
    """Layers (open and made), their fields and filters after a running plan."""

    def __init__(self, base: list[str]):
        restore()
        self.base = list(base)
        self.made: dict[str, dict] = {}  # name -> {kind, count, ref, by}
        self.fields: dict[str, dict[str, tuple]] = {}  # layer -> field -> (name, type, unit, cats)
        self.filters: dict[str, tuple[list, int, int]] = {}  # layer -> (conditions, shown, of)
        self.alias: dict[str, str] = {}  # "#k" -> name
        for n in base:
            L = pe.ctxmod.LAYERS[n]
            if L["kind"] != "raster":
                self.fields[n] = {f[0]: tuple(f) for f in L["fields"]}

    def name_of(self, ref: str) -> str:
        return self.alias.get(ref.strip(), ref) if isinstance(ref, str) else ref

    def _field_def(self, fname: str, value) -> tuple:
        for n in self.base:
            for f in pe.ctxmod.LAYERS[n].get("fields") or []:
                if f[0] == fname:
                    return tuple(f)
        try:
            float(value)
            return (fname, "number", None, None)
        except (TypeError, ValueError):
            return (fname, "text", None, None)

    def update(self, steps: list[dict], out: dict) -> None:
        if not out.get("ok"):
            return
        for k, (s, o) in enumerate(zip(steps, out.get("steps", [])), 1):
            if "new_layer" in o:
                lyr = next((l for l in out.get("layers", []) if l.get("alias") == f"#{k}"), None)
                name = (lyr or {}).get("name") or o["new_layer"]
                self.alias[f"#{k}"] = name
                src = self.name_of((s.get("args") or {}).get("layer", ""))
                kind = "areas" if s["name"] in AREA_OUT else (self.kind(src) or "areas")
                self.made[name] = {"kind": kind, "count": o.get("rows"), "ref": f"#{k}", "by": s["name"]}
                self.fields.setdefault(name, {})
            if "filtered" in o:
                lname = self.name_of(o["filtered"])
                self.filters[lname] = ((s.get("args") or {}).get("conditions") or [], o.get("shown"), o.get("of"))
            if "new_fields" in o:
                lname = self.name_of(o["layer"])
                for f in o["new_fields"]:
                    # A joined field keeps its type in the area layer (CONTINENT stays a category).
                    self.fields.setdefault(lname, {}).setdefault(f, self._field_def(f, 0))
        for lyr in out.get("layers", []):
            if lyr.get("raster"):
                continue
            lname = self.name_of(lyr.get("alias") or lyr.get("name"))
            if lname in self.made:
                self.made[lname]["shown"] = lyr.get("shown")
            for obj in lyr.get("objects", [])[:1]:
                fs = self.fields.setdefault(lname, {})
                for fname, v in (obj.get("attrs") or {}).items():
                    if fname not in fs:
                        fs[fname] = self._field_def(fname, v)

    def kind(self, name: str) -> str | None:
        if name in self.made:
            return self.made[name]["kind"]
        L = pe.ctxmod.LAYERS.get(name)
        return L["kind"] if L else None

    def names(self) -> list[str]:
        return self.base + list(self.made)

    def register(self, ctx: str) -> None:
        """This state as a v2 context: the checker, the tool's layer enums and resolve_feature."""
        pe.ctxmod.CONTEXTS[ctx] = self.names()
        for n in self.names():
            if self.kind(n) == "raster":
                continue
            entry = copy.deepcopy(pe.ctxmod.LAYERS.get(n) or {"kind": self.kind(n)})
            entry["kind"] = self.kind(n)
            entry["fields"] = list(self.fields.get(n, {}).values())
            if n in self.made:
                entry["count"] = self.made[n].get("count") or 0
            pe.ctxmod.LAYERS[n] = entry
            # resolve_feature finds the values of joined categories (South Korea in ADMIN on the
            # islands) as it finds the layer's own; it reads rows, so give each value a row.
            own = {f[0] for f in (BASE.get(n) or {}).get("fields") or []}
            extra = [{f[0]: v} for f in entry["fields"] if f[0] not in own for v in joined_values(f[0])]
            rows = (lambda n=n: pe.table(n)[1]) if n in BASE and n in pe.LAYERS else (lambda: [])
            pe.v2data.REGISTERED[n] = (lambda rows=rows, extra=extra: list(rows()) + extra)
        pe.SINGLE_SHOWN[ctx] = {n for n, (_c, shown, _of) in self.filters.items() if shown == 1}
        pe.CURRENT_FILTERS[ctx] = {n: [str(c.get("field")) for c in conds if isinstance(c, dict)]
                                   for n, (conds, _shown, _of) in self.filters.items() if conds}

    def text(self, history: list[str]) -> str:
        lines = ["The user's map. Open layers:"]
        for i, n in enumerate(self.names(), 1):
            if n in self.made:
                m = self.made[n]
                fs = [f"{f[0]} ({'text' if f[1] in ('other text', 'category') else f[1]})" for f in self.fields.get(n, {}).values()]
                lines.append(f"{i}. {n}: {m['kind']}, {m.get('count') or 0:,} (made by {m['by']} in an earlier request). Fields: " + "; ".join(fs))
            else:
                lines.append(pe.layer_text(i, n))
                base = {f[0] for f in pe.ctxmod.LAYERS[n].get("fields") or []} if self.kind(n) != "raster" else set()
                added = [f for f in self.fields.get(n, {}) if f not in base]
                if added:
                    lines[-1] += ". Added in earlier requests: " + "; ".join(f"{f} ({self.fields[n][f][1]})" for f in added)
            if n in self.filters:
                conds, shown, of = self.filters[n]
                lines[-1] += f". Filter now: {cond_text(conds)} ({shown:,} of {of:,} shown)"
        lines.append("Selected: nothing")
        if history:
            lines.append("Earlier requests in this conversation (done):")
            lines += [f"- {h}" for h in history]
        return "\n".join(lines)


BASE: dict[str, dict] = {}


def restore() -> None:
    """The catalogue as ``pro_e2e.register`` made it (``MapState.register`` changes it per request)."""
    if not BASE:
        BASE.update(copy.deepcopy(pe.ctxmod.LAYERS))
    for n, entry in BASE.items():
        pe.ctxmod.LAYERS[n] = copy.deepcopy(entry)


def joined_values(fname: str) -> list[str]:
    """The values a joined field can hold: those of the open area layer it comes from (ADMIN from
    Countries). Numbers aren't looked up."""
    for n, entry in BASE.items():
        f = next((f for f in entry.get("fields") or [] if f[0] == fname), None)
        if f and f[1] in ("text", "category") and n in pe.LAYERS:
            return sorted({r[fname] for r in pe.table(n)[1] if r.get(fname)})
    return []


def cond_text(conds) -> str:
    out = []
    for c in conds if isinstance(conds, list) else []:
        if not isinstance(c, dict):
            continue
        v = c.get("value")
        if c.get("op") == "between":
            v = f"{v} and {c.get('value2')}"
        elif c.get("values"):
            v = ", ".join(map(str, c["values"]))
        out.append(f"{c.get('field') or c.get('raster')} {str(c.get('op', '')).replace('_', ' ')} {v}")
    return "; ".join(out)


def build_state(base: list[str], steps: list[dict]) -> tuple[MapState, dict]:
    """The map after ``steps``, re-run step by step's worth of output in one run."""
    st = MapState(base)
    out = run(base, steps)
    st.update(steps, out)
    return st, out


# ---------------------------------------------------------------------------------------------
# Comparing a request's effect with its gold steps


def signature(out: dict, start: int) -> dict:
    """What the steps after ``start`` leave: shown counts and first objects of every layer, new
    layers by size, value summaries and card answers."""
    sig = {"layers": {}, "new": [], "values": [], "cards": []}
    for lyr in out.get("layers", []):
        if lyr.get("raster"):
            continue
        key = lyr.get("alias") or lyr.get("name")
        sig["layers"][key] = (lyr.get("shown"), tuple(o.get("title") for o in lyr.get("objects", [])))
    for o in out.get("steps", [])[start:]:
        if "new_layer" in o:
            sig["new"].append(o.get("rows"))
        if "new_fields" in o:
            # A join that brings more fields than asked still matches the same objects.
            sig["values"].append(re.sub(r" · \d+ fields? added$", "", re.sub(r"^\S+ ", "", o.get("summary", ""))))
        if "card" in o:
            sig["cards"].append(o.get("answer") or o.get("summary"))
    return sig


def layer_answers(card: str, model: dict) -> bool:
    """A list card ("3 countries inside": Argentina, Brazil, Paraguay) answered by a new layer of
    those objects instead (keep_inside the circle): as many shown, the first ones among the items."""
    m = re.search(r'heading: "(\d+) ', card or "")
    if not m:
        return False
    items = set(re.findall(r'"([^"]+)"', card.split("items:", 1)[-1]))
    for k, (shown, titles) in model["layers"].items():
        if k.startswith("#") and shown == int(m.group(1)) and all(t in items for t in titles):
            return True
    return False


def same_effect(model: dict, gold: dict) -> tuple[bool, str]:
    gl = {k: v for k, v in gold["layers"].items() if not k.startswith("#")}
    ml = {k: v for k, v in model["layers"].items() if not k.startswith("#")}
    for k, v in gl.items():
        if ml.get(k) != v:
            return False, f"{k}: {ml.get(k, ('missing',))[0]} shown, gold {v[0]}"
    # New layers by size: their objects' titles follow the fields a plan joins.
    gnew, mnew = [v[0] for k, v in gold["layers"].items() if k.startswith("#")], [v[0] for k, v in model["layers"].items() if k.startswith("#")]
    # Every gold new layer, by size; an extra one (a layer of the answer) is allowed.
    left = list(mnew)
    for g in gnew:
        if g not in left:
            return False, f"new layers {mnew}, gold {gnew}"
        left.remove(g)
    for v in gold["values"]:
        if v not in model["values"]:
            return False, f"values {model['values']}, gold {v}"
    for c in gold["cards"]:
        if c not in model["cards"] and not layer_answers(c, model):
            return False, f"cards {model['cards']}, gold {c}"
    return True, ""


# ---------------------------------------------------------------------------------------------
# One puzzle, request by request


def ask(client, model, budget, cache, ctx: str, request: str) -> dict:
    rec = pe.plan_request(client, model, ctx, request, budget, cache)
    plan = rec.get("plan") or []
    if rec.get("error"):
        rec["kind"] = "budget" if rec["error"] == "budget" else "model_error"
    elif not plan:
        rec["kind"] = "no_plan"
    elif plan[0]["name"] in ("ask_user", "cannot_do"):
        rec["kind"] = "asked" if plan[0]["name"] == "ask_user" else "refused"
    else:
        rec["kind"] = "plan"
    return rec


def run_puzzle(client, model, budget, cache, p: dict, reqs: list[dict], log) -> dict:
    base = pe.puzzle_layers(p)
    gold_all: list[dict] = []
    own: list[dict] = []
    history: list[str] = []
    rows = []
    for i, r in enumerate(reqs, 1):
        # Guided: the gold steps so far.
        g_state, _ = build_state(base, gold_all)
        ctx = f"step_{p['id']}_{i}_g"
        pe.CTX_TEXT[ctx] = g_state.text(history)
        g_state.register(ctx)
        rec = ask(client, model, budget, cache, ctx, r["request"])
        row = {"puzzle": p["id"], "request_no": i, "request": r["request"], "model": model["id"], "gold": r["gold"]}
        gold_next = gold_all + r["gold"]
        gout = run(base, gold_next)
        row["guided"] = {k: rec.get(k) for k in ("kind", "plan", "checker_log", "cost_usd", "latency_s", "prompt_tokens", "completion_tokens", "error")}
        if rec["kind"] == "plan":
            mplan = gold_all + renumber(rec["plan"], len(gold_all))
            mout = run(base, mplan)
            if not mout.get("ok"):
                row["guided"]["right"], row["guided"]["why"] = False, f"run error: {mout.get('error')}"
            else:
                ok, why = same_effect(signature(mout, len(gold_all)), signature(gout, len(gold_all)))
                row["guided"]["right"], row["guided"]["why"] = ok, why
        else:
            row["guided"]["right"], row["guided"]["why"] = False, rec["kind"]
        # On its own: the model's own steps so far (the first request is the same as guided).
        if i == 1:
            orec = rec
        else:
            o_state, _ = build_state(base, own)
            octx = f"step_{p['id']}_{i}_o"
            pe.CTX_TEXT[octx] = o_state.text(history)
            o_state.register(octx)
            orec = ask(client, model, budget, cache, octx, r["request"])
        row["own"] = {k: orec.get(k) for k in ("kind", "plan", "cost_usd", "latency_s", "error")}
        if orec["kind"] == "plan":
            trial = own + renumber(orec["plan"], len(own))
            if run(base, trial).get("ok"):
                own = trial
            else:
                row["own"]["run_error"] = True
        gold_all = gold_next
        history.append(r["request"])
        rows.append(row)
        log(row)
    fout = run(base, own, 50)
    verdict = final_verdict(p, fout, own)
    return {"puzzle": p["id"], "model": model["id"], "final": verdict, "own_plan": own, "requests": rows}


# ---------------------------------------------------------------------------------------------


def final_verdict(p: dict, out: dict, plan: list[dict]) -> dict:
    """``pro_e2e.check_answer``, plus P7's last requests: the user asks which countries lie within
    30 km of each city left, so a card "3 countries inside Within 30 km of <city>" for the answer
    city, and fewer for the others, settles it."""
    if not plan:
        return {"outcome": "no_plan"}
    v = pe.check_answer(p, out, plan)
    if v["outcome"] != "ambiguous" or p["answer_kind"] != "city":
        return v
    counts = {}
    # keep_inside(Countries, the circle around a city) answers it too: count the countries kept.
    for i, (s, o) in enumerate(zip(plan, out.get("steps", []))):
        a = s.get("args") or {}
        m = re.fullmatch(r"#(\d+)", str(a.get("areas", "")))
        if s["name"] == "keep_inside" and m and "new_layer" in o:
            b = plan[int(m.group(1)) - 1]
            obj = (b.get("args") or {}).get("object") or {}
            if b["name"] == "buffer" and obj.get("value"):
                counts[pe.fold(obj["value"])] = o.get("rows") or 0
    for s in out.get("steps", []):
        m = re.match(r"(\d+) countr(?:y|ies) inside Within .+? of (.+)$", s.get("summary", "") if "card" in s else "")
        if m:
            counts[pe.fold(m.group(2))] = int(m.group(1))
    left = [n for n in v.get("names") or [] if counts.get(pe.fold(n), 0) >= 3]
    if len(left) == 1 and pe.fold(left[0]) == pe.fold(p["answer"]["name"]) and len(counts) >= len(v.get("names") or []):
        return {**v, "outcome": "solved_with_cards", "detail": counts}
    return v


def patch_context() -> None:
    """pro_e2e builds the map context from its catalogue; here each request's context is the
    state text made for it."""
    pe.CTX_TEXT = {}
    orig = pe.map_context
    pe.map_context = lambda ctx: pe.CTX_TEXT.get(ctx) or orig(ctx)


def gold_report(puzzles: list[dict], steps: dict) -> int:
    bad = 0
    for p in puzzles:
        reqs = steps[p["id"]]
        plan = [s for r in reqs for s in r["gold"]]
        base = pe.puzzle_layers(p)
        out = run(base, plan, 50)
        verdict = final_verdict(p, out, plan)
        counts = pe.candidates_per_step(out, [])
        print(f"{p['id']}: {len(reqs)} requests, {len(plan)} steps: {verdict['outcome']}; per step {counts}"
              + (f"; error {out.get('error')}" if not out.get("ok") else ""))
        bad += verdict["outcome"] not in pe.SOLVED
    return bad


def summarize(path: Path) -> str:
    lines = []
    for f in sorted(x for x in path.glob("*.jsonl") if not x.name.startswith("._")):
        recs = [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x.strip()]
        reqs = [r for p in recs for r in p["requests"]]
        g = sum(r["guided"].get("right") is True for r in reqs)
        cost = sum((r["guided"].get("cost_usd") or 0) + ((r["own"].get("cost_usd") or 0) if r["request_no"] > 1 else 0) for r in reqs)
        solved = [p["puzzle"] for p in recs if p["final"]["outcome"] in pe.SOLVED]
        lines.append(f"## {recs[0]['model'] if recs else f.stem}\n")
        lines.append(f"- Single requests right (guided): **{g} of {len(reqs)}** ({100 * g / max(1, len(reqs)):.0f}%)")
        lines.append(f"- Puzzles solved by following the requests on its own: **{len(solved)} of {len(recs)}** ({', '.join(solved) or 'none'})")
        lines.append(f"- Cost: US${cost:.4f} for {len(reqs)} requests asked twice (first request once)\n")
        lines.append("| Puzzle | Requests right | Final answer (on its own) |\n| --- | --- | --- |")
        for p in recs:
            ok = sum(r["guided"].get("right") is True for r in p["requests"])
            lines.append(f"| {p['puzzle']} | {ok} of {len(p['requests'])} | {p['final']['outcome']} |")
        miss = [r for r in reqs if r["guided"].get("right") is not True]
        if miss:
            lines.append("\nMissed requests (guided):\n")
            for r in miss:
                plan = "; ".join(f"{s['name']}({json.dumps(s['args'], ensure_ascii=False)[:140]})" for s in r["guided"].get("plan") or [])
                lines.append(f"- {r['puzzle']}.{r['request_no']} \"{r['request']}\": {r['guided'].get('why')}. Plan: {plan or r['guided'].get('kind')}")
        lines.append("")
    return "\n".join(lines)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gold", action="store_true")
    ap.add_argument("--show-context", nargs=2, metavar=("PUZZLE", "REQUEST"))
    ap.add_argument("--models", default="anthropic/claude-haiku-5.5")
    ap.add_argument("--only", default="")
    ap.add_argument("--budget", type=float, default=0.50, help="this run's cap in US$ (added to results/spend.json)")
    ap.add_argument("--tag", default="")
    ap.add_argument("--report", default="")
    a = ap.parse_args(argv)

    puzzles = json.loads(pe.PUZZLES_JSON.read_text(encoding="utf-8"))["puzzles"]
    steps = json.loads(STEPS.read_text(encoding="utf-8"))["puzzles"]
    only = {x.strip() for x in a.only.split(",") if x.strip()}
    puzzles = [p for p in puzzles if not only or p["id"] in only]
    if a.report:
        print(summarize(RESULTS / a.report))
        return 0
    if not pe.RUN_PLAN.exists():
        print(f"run_plan not built: {pe.RUN_PLAN}", file=sys.stderr)
        return 2
    if a.gold:
        return 1 if gold_report(puzzles, steps) else 0
    pe.register()
    restore()
    patch_context()
    if a.show_context:
        pid, k = a.show_context[0], int(a.show_context[1])
        p = next(p for p in puzzles if p["id"] == pid)
        reqs = steps[pid]
        st, _ = build_state(pe.puzzle_layers(p), [s for r in reqs[: k - 1] for s in r["gold"]])
        print(st.text([r["request"] for r in reqs[: k - 1]]))
        print(f"\nUser: {reqs[k - 1]['request']}")
        return 0

    catalog = pe.run_eval.load_catalog()
    key = pe.run_eval.read_key()
    if not key:
        print("No OpenRouter key in the keychain (service openrouter, account layeredmap).", file=sys.stderr)
        return 3
    client = pe.run_eval.OpenRouterClient(key)
    del key
    budget = pe.run_eval.Budget(RESULTS / "spend.json", 0)
    budget.cap = budget.spent + a.budget
    out_dir = RESULTS / ("stepwise" + (f"-{a.tag}" if a.tag else ""))
    out_dir.mkdir(parents=True, exist_ok=True)
    for m in [x.strip() for x in a.models.split(",") if x.strip()]:
        model = catalog[m]
        cache = m.startswith(pe.run_v2.CACHE_VENDORS)
        path = out_dir / f"{pe.run_eval.slug(m)}.jsonl"
        path.unlink(missing_ok=True)
        for p in puzzles:
            t0 = time.monotonic()

            def log(row):
                g = row["guided"]
                print(f"  {row['puzzle']}.{row['request_no']} {'right' if g.get('right') else 'MISS '} "
                      f"own:{row['own'].get('kind')} ${(g.get('cost_usd') or 0):.4f}  {row['request'][:60]}"
                      + ("" if g.get("right") else f"  [{g.get('why')}]"), flush=True)

            res = run_puzzle(client, model, budget, cache, p, steps[p["id"]], log)
            print(f"{p['id']} {m.split('/')[-1]}: final {res['final']['outcome']} ({time.monotonic() - t0:.0f} s; spent ${budget.spent:.4f})", flush=True)
            with path.open("a", encoding="utf-8") as fh:
                fh.write(json.dumps(res, ensure_ascii=False) + "\n")
    (out_dir / "summary.md").write_text(summarize(out_dir), encoding="utf-8")
    print(summarize(out_dir))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
