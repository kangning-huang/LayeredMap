"""Harness v2 runner: per-action or submit_plan tools, resolve_feature, the plan checker with
repair rounds, static worked examples, prompt caching, tool_choice "required", a fallback parser
for plans written as text.

    python -I run_v2.py --dry-run                                   # mocked client
    python -I run_v2.py --models anthropic/claude-haiku-5.5         # per_action, full v2
    python -I run_v2.py --design submit_plan --models ...
    python -I run_v2.py --no-examples --models ... / --no-checker / --no-resolve
    python -I run_v2.py --tag repeat2 --models ...

The OpenRouter key is read at run time from the macOS keychain (as in v1) and never printed,
logged or stored. Results: ``v2/results/<run>/<model>.jsonl``. Spending over *all* v2 runs is
in ``v2/results/spend.json`` and capped by ``--budget`` (default US$3.00, the cap for this
harness, separate from v1's ledger).
"""

from __future__ import annotations

import argparse
import json
import random
import re
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

import checker as chk  # noqa: E402
import prompt_v2  # noqa: E402
import resolve  # noqa: E402
import run_eval  # noqa: E402  (v1: catalog, key, client, budget, retries)
import scoring  # noqa: E402
import scoring_v2  # noqa: E402
import tools_v2  # noqa: E402
from tokens import count_tokens  # noqa: E402

MAX_TURNS = 8
MAX_REPAIRS = 2
MODELS = ["anthropic/claude-haiku-5.5", "z-ai/glm-5.3-flash", "openai/gpt-oss-120b", "google/gemini-3.5-flash"]
CACHE_VENDORS = ("anthropic/", "google/")


# ---------------------------------------------------------------------------------------------
# Fallback: a plan written as text


def _json_spans(text: str) -> list:
    """Every top-level JSON object or array in the text that parses."""
    out, i, n = [], 0, len(text)
    while i < n:
        if text[i] in "{[":
            depth, j, in_str, esc = 0, i, False, False
            while j < n:
                ch = text[j]
                if in_str:
                    if esc:
                        esc = False
                    elif ch == "\\":
                        esc = True
                    elif ch == '"':
                        in_str = False
                elif ch == '"':
                    in_str = True
                elif ch in "{[":
                    depth += 1
                elif ch in "}]":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            try:
                out.append((i, json.loads(text[i : j + 1])))
                i = j + 1
                continue
            except ValueError:
                pass
        i += 1
    return out


def calls_from_text(text: str, known: set[str]) -> list[tuple[str, dict]]:
    """(name, args) pairs parsed from a reply that wrote its calls as text: JSON in any of the
    shapes models use ({"tool"|"name"|"function"|"action"|"call"|"type": n, "arguments"|"args"|
    "parameters": {...}}, {"buffer": {...}}, {"plan"|"steps": [...]}, a bare {"question": ...} or
    {"reason": ...}), "functions.name {json}", or the compact notation of the worked examples
    ('#1 filter(layer="X", conditions=[{field: "f", op: "is", value: "v"}])'). In the submit_plan
    design all steps found are merged into one submit_plan call."""
    if not text:
        return []
    found: list[tuple[str, dict]] = []
    plan_names = set(chk.ACTIONS) | {"filter", "color_by", "view_data", "zoom_to", "ask_user", "cannot_do"}
    name_keys = ("name", "tool", "tool_name", "function", "action", "call", "type")
    arg_keys = ("arguments", "args", "parameters", "input", "params")

    def add(name, args):
        name = name.split(".")[-1]
        if not isinstance(args, dict):
            return
        if name in known:
            found.append((name, args))
        elif "submit_plan" in known and name in plan_names:
            found.append(("submit_plan", {"steps": [{"action": name, **args}]}))

    def take(o, hint: str | None = None):
        if isinstance(o, list):
            for x in o:
                take(x)
            return
        if not isinstance(o, dict):
            return
        for k in ("steps", "plan", "calls", "tool_calls"):
            if isinstance(o.get(k), list):
                if k == "steps" and "submit_plan" in known:
                    found.append(("submit_plan", {"steps": o[k]}))
                else:
                    take(o[k])
                return
        if isinstance(o.get("function"), dict):
            o = o["function"]
        if len(o) == 1:
            (k, v), = o.items()
            if k.split(".")[-1] in known | plan_names and isinstance(v, dict):
                add(k, v)
                return
        name = next((o[k] for k in name_keys if isinstance(o.get(k), str) and o[k].split(".")[-1] in known | plan_names), None) or hint
        args = next((o[k] for k in arg_keys if k in o), None)
        if isinstance(args, str):
            try:
                args = json.loads(args)
            except ValueError:
                args = None
        if args is None:
            args = {k: v for k, v in o.items() if k not in name_keys}
        if name is None:
            if set(o) <= {"question", "choices"} and "question" in o:
                name = "ask_user"
            elif set(o) == {"reason"}:
                name = "cannot_do"
        if isinstance(name, str):
            add(name, args)

    for pos, o in _json_spans(text):
        before = text[max(0, pos - 60) : pos]
        m = re.search(r"(?:functions\.|to=functions\.|\b)([a-z_]+)\W*$", before)
        hint = m.group(1) if m and m.group(1) in known | plan_names else None
        take(o, hint)
    if not found:
        for name, args in _notation_calls(text, known | plan_names):
            add(name, args)
    if "submit_plan" in known and len([f for f in found if f[0] == "submit_plan"]) > 1:
        steps = [s for n, a in found if n == "submit_plan" for s in (a.get("steps") or [])]
        found = [f for f in found if f[0] != "submit_plan"] + [("submit_plan", {"steps": steps})]
    return found


def _notation_calls(text: str, names: set[str]) -> list[tuple[str, dict]]:
    """Calls in the compact notation: name(key="v", obj={k: "v"}, list=[...], flag=false)."""
    out = []
    for m in re.finditer(r"\b([a-z_]+)\(", text):
        name = m.group(1)
        if name not in names:
            continue
        i, depth, in_str, q = m.end(), 1, False, ""
        j = i
        while j < len(text) and depth:
            ch = text[j]
            if in_str:
                if ch == "\\":
                    j += 1
                elif ch == q:
                    in_str = False
            elif ch in "\"'" and not (ch == "'" and text[j - 1 : j].isalpha()):
                in_str, q = True, ch
            elif ch in "([{":
                depth += 1
            elif ch in ")]}":
                depth -= 1
            j += 1
        if depth:
            continue
        inner = text[i : j - 1]
        js = "{" + inner + "}"
        js = re.sub(r'([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*[=:]\s*', r'\1"\2": ', js)
        js = re.sub(r"\bTrue\b", "true", re.sub(r"\bFalse\b", "false", re.sub(r"\bNone\b", "null", js)))
        try:
            args = json.loads(js)
        except ValueError:
            continue
        if isinstance(args, dict):
            out.append((name, args))
    return out


# ---------------------------------------------------------------------------------------------
# Requests


def request_body(model: dict, messages: list, tools: list, tool_choice: str) -> dict:
    sp = set(model.get("supported_parameters") or [])
    body = {"model": model["id"], "messages": messages, "tools": tools, "usage": {"include": True}, "max_tokens": 4000,
            "provider": {"require_parameters": True}}
    if "temperature" in sp:
        body["temperature"] = 0
    if "tool_choice" in sp:
        body["tool_choice"] = tool_choice
    if "parallel_tool_calls" in sp:
        body["parallel_tool_calls"] = True
    rs = run_eval.reasoning_setting(model)
    if rs is not None:
        body["reasoning"] = rs
    if run_eval.PROVIDER_IGNORE:
        body["provider"]["ignore"] = list(run_eval.PROVIDER_IGNORE)
    return body


def usage_numbers(usage: dict) -> dict:
    ptd = usage.get("prompt_tokens_details") or {}
    ctd = usage.get("completion_tokens_details") or {}
    return {
        "prompt_tokens": usage.get("prompt_tokens") or 0,
        "completion_tokens": usage.get("completion_tokens") or 0,
        "reasoning_tokens": ctd.get("reasoning_tokens") or 0,
        "cached_tokens": ptd.get("cached_tokens") or 0,
        "cache_write_tokens": ptd.get("cache_write_tokens") or 0,
    }


class Settings:
    def __init__(self, design="per_action", examples=True, checker=True, resolve=True, required=True, cache=True):
        self.design, self.examples, self.checker, self.resolve, self.required, self.cache = design, examples, checker, resolve, required, cache

    def run_name(self, tag: str = "") -> str:
        n = "v2-" + self.design
        for flag, word in ((self.examples, "noexamples"), (self.checker, "nochecker"), (self.resolve, "noresolve"), (self.required, "auto"), (self.cache, "nocache")):
            if not flag:
                n += "-" + word
        return n + (f"-{tag}" if tag else "")

    def as_dict(self) -> dict:
        return dict(design=self.design, examples=self.examples, checker=self.checker, resolve=self.resolve, required=self.required, cache=self.cache)


# Learned during a run, per model: no endpoint takes tool_choice "required" (Anthropic on
# OpenRouter, 2026-10-17: 404 "No endpoints found that support the provided 'tool_choice'"), and
# providers that reject the request outright (e.g. "Upstream error from Inceptron: Invalid
# structural tag specification" for GLM): those are then ignored for that model.
NO_REQUIRED: set[str] = set()
BAD_PROVIDERS: dict[str, set[str]] = {}
_LEARN = threading.Lock()


def call_model(client, model, messages, tools, choice, budget, est_prompt, turn, rec, tot):
    """One completion (with retries, a tool_choice fallback, and skipping a provider that rejects
    the request as invalid). Returns (data, error)."""
    pin, pout = run_eval.price(model)
    if choice == "required" and model["id"] in NO_REQUIRED:
        choice = "auto"
        rec["tool_choice_fallback"] = True
    body = request_body(model, messages, tools, choice)
    bad = BAD_PROVIDERS.get(model["id"])
    if bad:
        body["provider"]["ignore"] = sorted(set(body["provider"].get("ignore", [])) | bad)
    reserve = (est_prompt + 400 * turn) * pin * 1.3 + 1500 * pout
    if not budget.reserve(reserve):
        return None, "budget"
    actual = 0.0
    lat = 0.0
    tries = 0
    try:
        for _attempt in range(3):
            status, data, lat1, tries1, err = run_eval._request(client, body, 4)
            lat += lat1
            tries += tries1
            if not err:
                break
            if body.get("tool_choice") == "required" and status in (400, 404, 422) and ("tool_choice" in str(err) or "No endpoints" in str(err)):
                if "tool_choice" in str(err):
                    with _LEARN:
                        NO_REQUIRED.add(model["id"])
                rec["tool_choice_fallback"] = True
                body["tool_choice"] = "auto"
                continue
            m = re.match(r"Upstream error from ([^:]+): ", str(err))
            if m and "invalid" in str(err).lower():
                with _LEARN:
                    BAD_PROVIDERS.setdefault(model["id"], set()).add(m.group(1).strip())
                rec.setdefault("providers_skipped", []).append(m.group(1).strip())
                body["provider"]["ignore"] = sorted(set(body["provider"].get("ignore", [])) | BAD_PROVIDERS[model["id"]])
                continue
            break
        usage = (data.get("usage") or {}) if isinstance(data, dict) else {}
        actual = float(usage.get("cost") or 0.0)
    finally:
        budget.settle(reserve, actual)
    tot["cost_usd"] += actual
    tot["latency_s"] += lat
    tot["tries"] += tries
    tot["model_calls"] += 1
    for k, v in usage_numbers(usage).items():
        tot[k] += v
    rec["provider"] = data.get("provider") if isinstance(data, dict) else None
    rec["status"] = status
    if err:
        return data, err
    return data, None


def parse_message(msg: dict, known: set[str]) -> tuple[list[dict], str, bool]:
    """[(id, name, args)], text, from_text."""
    text = msg.get("content") or ""
    if isinstance(text, list):
        text = " ".join(p.get("text", "") for p in text if isinstance(p, dict))
    out = []
    for i, tc in enumerate(msg.get("tool_calls") or []):
        f = tc.get("function") or {}
        name = (f.get("name") or "").split(".")[-1]  # Gemini's "default_api." prefix
        raw = f.get("arguments")
        if isinstance(raw, dict):
            args = raw
        else:
            try:
                args = json.loads(raw) if raw else {}
            except (TypeError, ValueError):
                args = {"_unparsed": raw}
        out.append({"id": tc.get("id") or f"call_{i}", "name": name, "args": args if isinstance(args, dict) else {}, "raw": tc})
    if out:
        return out, text, False
    parsed = calls_from_text(text, known)
    return [{"id": None, "name": n, "args": a, "raw": None} for n, a in parsed], text, bool(parsed)


def assistant_echo(msg: dict, calls: list[dict], turn: int) -> dict:
    amsg = {k: v for k, v in msg.items() if k in ("role", "content", "reasoning_details", "reasoning")}
    amsg["role"] = "assistant"
    fixed = []
    for i, c in enumerate(calls):
        tc = dict(c["raw"])
        tc["id"] = tc.get("id") or f"call_{turn}_{i}"
        c["id"] = tc["id"]
        tc.setdefault("type", "function")
        fixed.append(tc)
    amsg["tool_calls"] = fixed
    return amsg


def lint_and_reset(checker, plan, st, repairs, log, turn, messages, text) -> bool:
    """Plan-level checks once the model has finished (per-action design). On errors, with repair
    rounds left: say what's wrong, empty the plan, and let the model make the calls again."""
    if not (st.checker and st.design == "per_action" and plan and repairs < MAX_REPAIRS):
        return False
    if any(c["name"] in ("ask_user", "cannot_do") for c in plan):
        return False
    errs = checker.lint(plan)
    if not errs:
        return False
    log.append({"turn": turn + 1, "lint": errs})
    if text is not None:
        messages.append({"role": "assistant", "content": text or "Done."})
    messages.append({"role": "user", "content": "The plan was not accepted: " + " ".join(errs)
                     + " The plan is now empty: make all the calls again, corrected."})
    checker.reset()
    return True


def run_case(client, model: dict, case: dict, st: Settings, budget) -> dict:
    ctx, sel, request = case["context"], case.get("selection"), case["request"]
    tools = tools_v2.DESIGNS[st.design](ctx, with_resolve=st.resolve)
    known = {t["function"]["name"] for t in tools}
    cache = st.cache and model["id"].startswith(CACHE_VENDORS)
    system = prompt_v2.system_message(ctx, sel, st.design, st.examples, cache)
    messages: list = [system, {"role": "user", "content": request}]
    est_prompt = count_tokens(json.dumps(system, ensure_ascii=False)) + count_tokens(json.dumps(tools, ensure_ascii=False)) + 50
    checker = chk.Checker(ctx, sel, request, enabled=st.checker)
    rec: dict = {"case": case["id"], "model": model["id"], **st.as_dict()}
    tot = {"prompt_tokens": 0, "completion_tokens": 0, "reasoning_tokens": 0, "cached_tokens": 0, "cache_write_tokens": 0,
           "cost_usd": 0.0, "latency_s": 0.0, "tries": 0, "model_calls": 0}
    plan: list[dict] = []
    raw_calls: list[dict] = []
    log: list[dict] = []
    repairs = 0
    resolves = 0
    text = ""
    error = None
    done = False
    from_text_any = False
    for turn in range(MAX_TURNS):
        if st.design == "submit_plan":
            choice = "required" if st.required else "auto"
        else:
            # Forced until the plan has a step: a lookup alone doesn't end the turn in prose.
            choice = "required" if (st.required and not plan) else "auto"
        data, err = call_model(client, model, messages, tools, choice, budget, est_prompt, turn, rec, tot)
        if err:
            error = err
            break
        choice0 = data["choices"][0]
        rec["finish_reason"] = choice0.get("finish_reason")
        if choice0.get("native_finish_reason") == "MALFORMED_FUNCTION_CALL":
            rec["malformed"] = True
            break
        msg = choice0.get("message") or {}
        calls, t, from_text = parse_message(msg, known)
        text = t or text
        from_text_any |= from_text
        for c in calls:
            raw_calls.append({"turn": turn + 1, "name": c["name"], "args": c["args"], "from_text": from_text})
        if not calls:
            if lint_and_reset(checker, plan, st, repairs, log, turn, messages, t):
                repairs += 1
                plan = []
                continue
            break
        results: list[str] = []
        turn_errors = False
        stop = False
        for c in calls:
            name, args = c["name"], c["args"]
            if name == "resolve_feature":
                resolves += 1
                res = resolve.resolve_feature(str(args.get("layer", "")), args.get("text", ""))
                results.append(resolve.tool_result_text(res))
                log.append({"turn": turn + 1, "resolve": args, "candidates": res.get("candidates"), "error": res.get("error")})
                continue
            if name == "submit_plan":
                steps = tools_v2.steps_to_calls(args.get("steps"))
                fixed, errs, fixes = checker.check_plan(steps) if st.checker else (steps, [], [])
                log.append({"turn": turn + 1, "submit": len(steps), "errors": errs, "fixes": fixes})
                if errs and repairs < MAX_REPAIRS:
                    turn_errors = True
                    results.append("Plan not accepted. Fix these and call submit_plan again with the whole plan: " + " | ".join(errs))
                else:
                    plan = fixed
                    if errs:
                        rec["unrepaired"] = errs
                    done = stop = True
                    results.append("Plan sent to the user for approval." + (" Corrected: " + "; ".join(fixes) if fixes else ""))
                continue
            if name in ("ask_user", "cannot_do"):
                plan.append({"name": name, "args": args})
                results.append("Sent to the user.")
                stop = done = True
                continue
            if st.design == "submit_plan":
                # A per-action call in the submit_plan design: read it as a one-step plan.
                c2 = {"name": name, "args": args}
                plan.append(c2)
                results.append("Use submit_plan for plans.")
                continue
            call = {"name": name, "args": args}
            v = checker.check(call)
            if v.duplicate_of:
                results.append(f"Already in the plan as step #{v.duplicate_of}; not added again.")
                log.append({"turn": turn + 1, "duplicate": v.duplicate_of})
                continue
            if v.ok:
                k = checker.add(v.call)
                plan.append(v.call)
                results.append(checker.accepted_note(k, v))
                if v.fixes:
                    log.append({"turn": turn + 1, "step": k, "fixes": v.fixes})
            elif repairs < MAX_REPAIRS:
                turn_errors = True
                results.append("Not added. " + " ".join(v.errors))
                log.append({"turn": turn + 1, "rejected": call, "errors": v.errors})
            else:
                # Out of repair rounds: the step goes into the plan as given (the app would show the error).
                k = checker.add(call)
                plan.append(call)
                rec.setdefault("unrepaired", []).extend(v.errors)
                results.append(f"Step #{k} added with problems: " + " ".join(v.errors))
        if turn_errors:
            repairs += 1
        if stop and not turn_errors:
            break
        if from_text:
            messages.append({"role": "assistant", "content": text})
            note = "Your plan was read from your text. " + " ".join(results)
            if turn_errors:
                messages.append({"role": "user", "content": note + " Make the corrected calls with the tools."})
                continue
            if not done and lint_and_reset(checker, plan, st, repairs, log, turn, messages, None):
                repairs += 1
                plan = []
                continue
            break
        messages.append(assistant_echo(msg, calls, turn))
        for c, r in zip(calls, results):
            messages.append({"role": "tool", "tool_call_id": c["id"], "content": r})
        if done:
            break
    if not plan and not error and (text or "").strip() and not rec.get("malformed"):
        # A reply in prose with no plan: shown to the user as a question if it asks one, else as a
        # refusal (the Ask box has nothing to run). Recorded, and part of the v2 harness design.
        t = text.strip()
        plan = [{"name": "ask_user", "args": {"question": t[:300]}}] if ("?" in t or "？" in t) else [{"name": "cannot_do", "args": {"reason": t[:300]}}]
        rec["text_as_meta"] = plan[0]["name"]
    final, dropped = checker.finalize(plan)
    scored_calls = [scoring.canonicalize(c["name"], c.get("args") or {}) for c in final]
    rec.update({**tot, "turns": tot["model_calls"], "repair_rounds": repairs, "resolve_calls": resolves, "from_text": from_text_any,
                "raw_calls": raw_calls, "checker_log": log, "plan": final, "dropped": dropped, "text": (text or "")[:500]})
    rec["cost_usd"] = round(rec["cost_usd"], 8)
    rec["latency_s"] = round(rec["latency_s"], 3)
    if error:
        rec["error"] = error
        return rec
    sc = scoring_v2.score_both(case["id"], scored_calls, text)
    rec["score_v1"], rec["score_v2"] = sc["v1"], sc["v2"]
    return rec


# ---------------------------------------------------------------------------------------------
# Mock client (dry runs and tests)


class MockClient:
    """Answers with the case's expected plan, in the design's format."""

    def __init__(self, cases: dict[str, dict]):
        self.by_request = {c["request"]: c for c in cases.values()}

    def complete(self, body: dict):
        usage = {"prompt_tokens": 100, "completion_tokens": 10, "cost": 0.0}
        if body["messages"][-1]["role"] == "tool":
            last = body["messages"][-1]["content"]
            if last.startswith(("Plan sent", "Sent to")):
                return 200, {"choices": [{"message": {"role": "assistant", "content": "Done."}, "finish_reason": "stop"}], "usage": usage}
            return 200, {"choices": [{"message": {"role": "assistant", "content": "Done."}, "finish_reason": "stop"}], "usage": usage}
        case = self.by_request[body["messages"][1]["content"]]
        names = {t["function"]["name"] for t in body["tools"]}
        args = [(c["name"], run_eval.mock_args(c, False)) for c in case["expected"]]
        if "submit_plan" in names and args[0][0] not in ("ask_user", "cannot_do"):
            calls = [{"id": "c0", "type": "function", "function": {"name": "submit_plan", "arguments": json.dumps({"steps": [{"action": n, **a} for n, a in args]}, ensure_ascii=False)}}]
        elif "submit_plan" in names:
            calls = [{"id": "c0", "type": "function", "function": {"name": "submit_plan", "arguments": json.dumps({"steps": [{"action": args[0][0], "question": "?", "reason": "mock"}]})}}]
        else:
            calls = [{"id": f"c{i}", "type": "function", "function": {"name": n, "arguments": json.dumps(a or ({"question": "?"} if n == "ask_user" else {"reason": "mock"} if n == "cannot_do" else {}), ensure_ascii=False)}} for i, (n, a) in enumerate(args)]
        return 200, {"choices": [{"message": {"role": "assistant", "content": None, "tool_calls": calls}, "finish_reason": "tool_calls"}], "usage": usage}


# ---------------------------------------------------------------------------------------------


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--models", default=",".join(MODELS[:3]))
    ap.add_argument("--design", choices=list(tools_v2.DESIGNS), default="per_action")
    ap.add_argument("--no-examples", action="store_true")
    ap.add_argument("--no-checker", action="store_true")
    ap.add_argument("--no-resolve", action="store_true")
    ap.add_argument("--auto", action="store_true", help="tool_choice auto instead of required")
    ap.add_argument("--no-cache", action="store_true")
    ap.add_argument("--only", default="")
    ap.add_argument("--budget", type=float, default=3.0)
    ap.add_argument("--concurrency", type=int, default=8)
    ap.add_argument("--per-model", type=int, default=4)
    ap.add_argument("--tag", default="")
    ap.add_argument("--out", default=str(HERE / "results"))
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--rerun", action="store_true")
    ap.add_argument("--provider-ignore", default="")
    a = ap.parse_args(argv)

    run_eval.PROVIDER_IGNORE[:] = [p.strip() for p in a.provider_ignore.split(",") if p.strip()]
    st = Settings(a.design, not a.no_examples, not a.no_checker, not a.no_resolve, not a.auto, not a.no_cache)
    catalog = run_eval.load_catalog()
    models = [m.strip() for m in a.models.split(",") if m.strip()]
    missing = [m for m in models if m not in catalog]
    if missing:
        print("not in the catalog:", ", ".join(missing), file=sys.stderr)
        return 2
    cases = list(scoring_v2.CASES_V2.values())
    if a.only:
        keep = set(a.only.split(","))
        cases = [c for c in cases if c["id"] in keep]
    run_name = ("dryrun-" if a.dry_run else "") + st.run_name(a.tag)
    out_dir = Path(a.out) / run_name
    out_dir.mkdir(parents=True, exist_ok=True)
    budget = run_eval.Budget(Path(a.out) / ("spend-dryrun.json" if a.dry_run else "spend.json"), a.budget)
    if a.dry_run:
        client = MockClient({c["id"]: c for c in cases})
    else:
        key = run_eval.read_key()
        if not key:
            print("No OpenRouter key in the keychain (service openrouter, account layeredmap).", file=sys.stderr)
            return 3
        client = run_eval.OpenRouterClient(key)
        del key

    print(f"{run_name}: {len(models)} models x {len(cases)} cases; budget ${a.budget:.2f}, spent so far ${budget.spent:.4f}", flush=True)
    sems = {m: threading.Semaphore(a.per_model) for m in models}
    locks = {m: threading.Lock() for m in models}
    tasks = []
    for m in models:
        path = out_dir / f"{run_eval.slug(m)}.jsonl"
        done = set()
        if path.exists():
            rows = [json.loads(x) for x in path.read_text(encoding="utf-8").splitlines() if x.strip()]
            if a.rerun:
                ids = {c["id"] for c in cases}
                rows = [r for r in rows if r["case"] not in ids]
                path.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows), encoding="utf-8")
            done = {r["case"] for r in rows if "error" not in r}
        tasks += [(m, c, path) for c in cases if c["id"] not in done]

    def work(m, c, path):
        with sems[m]:
            rec = run_case(client, catalog[m], c, st, budget)
        if rec.get("error") == "budget":
            return rec
        with locks[m]:
            lines = [x for x in path.read_text(encoding="utf-8").splitlines() if x.strip() and json.loads(x)["case"] != c["id"]] if path.exists() else []
            lines.append(json.dumps(rec, ensure_ascii=False))
            path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return rec

    random.seed(0)
    random.shuffle(tasks)
    n_ok = n_err = n_budget = 0
    with ThreadPoolExecutor(max_workers=a.concurrency) as ex:
        futs = [ex.submit(work, *t) for t in tasks]
        for i, f in enumerate(as_completed(futs), 1):
            r = f.result()
            if r.get("error") == "budget":
                n_budget += 1
            elif "error" in r:
                n_err += 1
            else:
                n_ok += 1
            if i % 50 == 0 or i == len(futs):
                print(f"  {i}/{len(futs)}: {n_ok} scored, {n_err} failed, {n_budget} skipped (budget); spent ${budget.spent:.4f}", flush=True)
    if budget.stopped:
        print(f"Budget reached: stopped at ${budget.spent:.4f} of ${a.budget:.2f}.")
    print(f"Done: {n_ok} scored, {n_err} failed, {n_budget} skipped; total spent ${budget.spent:.4f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
