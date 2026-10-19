"""Run the benchmark against models on OpenRouter.

    python -I run_eval.py --design per_action                 # the shortlist, value-free context
    python -I run_eval.py --design single --models openai/gpt-oss-120b
    python -I run_eval.py --variant with_values --models a,b,c
    python -I run_eval.py --dry-run                           # a mocked client, no network

The OpenRouter key is read at run time from the macOS keychain
(``security find-generic-password -s openrouter -a layeredmap -w``); it is never printed,
logged or written. Only the synthetic benchmark text (system message, tools, request) is sent.

Results: ``results/<date>/<design>[-<variant>]/<model>.jsonl`` (one line per case: calls, scores,
tokens, cost, latency; no headers). Spending is recorded in ``results/<date>/spend.json`` and the
budget (default US$3) caps the total over every run that day; the run stops when it is reached.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import random
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # python -I drops the script's folder

import context as ctxmod  # noqa: E402
import scoring  # noqa: E402
import tools as toolmod  # noqa: E402
from tokens import count_tokens  # noqa: E402

API = "https://openrouter.ai/api/v1/chat/completions"
MODELS_JSON = HERE / "openrouter_models_2026-10-17.json"

# The shortlist: about 12 cheap models and 3 stronger references (README.md › Shortlist).
SHORTLIST = [
    "openai/gpt-oss-20b",
    "openai/gpt-oss-120b",
    "deepseek/deepseek-v4-flash",
    "qwen/qwen3.7-flash",
    "openai/gpt-5-nano",
    "google/gemini-2.5-flash-lite",
    "google/gemini-3.1-flash-lite",
    "z-ai/glm-5.3-flash",
    "mistralai/mistral-small-3.2-24b-instruct",
    "meta-llama/llama-3.3-70b-instruct",
    "qwen/qwen3-235b-a22b-2507",
    "anthropic/claude-haiku-5.5",
    "openai/gpt-5-mini",
    "anthropic/claude-sonnet-5.5",
    "google/gemini-3.5-flash",
]

EFFORT_ORDER = ["none", "minimal", "low", "medium", "high", "xhigh", "max"]


def load_catalog() -> dict[str, dict]:
    return {m["id"]: m for m in json.loads(MODELS_JSON.read_text(encoding="utf-8"))["data"]}


def reasoning_setting(m: dict) -> dict | None:
    """The lowest reasoning OpenRouter allows for this model, from the catalog's `reasoning`."""
    r = m.get("reasoning")
    if not r:
        return None
    efforts = r.get("supported_efforts") or []
    if not r.get("mandatory"):
        if not efforts or "none" in efforts:
            return {"enabled": False}
    if efforts:
        lowest = min(efforts, key=lambda e: EFFORT_ORDER.index(e) if e in EFFORT_ORDER else 99)
        return {"effort": lowest}
    return {"enabled": False} if not r.get("mandatory") else None


PROVIDER_IGNORE: list[str] = []


def request_body(model: dict, messages: list, tools: list) -> dict:
    sp = set(model.get("supported_parameters") or [])
    body = {
        "model": model["id"],
        "messages": messages,
        "tools": tools,
        "usage": {"include": True},
        "max_tokens": 4000,
    }
    if "temperature" in sp:
        body["temperature"] = 0
    if "tool_choice" in sp:
        body["tool_choice"] = "auto"
    if "parallel_tool_calls" in sp:
        body["parallel_tool_calls"] = True
    rs = reasoning_setting(model)
    if rs is not None:
        body["reasoning"] = rs
    if PROVIDER_IGNORE:
        body["provider"] = {"ignore": list(PROVIDER_IGNORE)}
    return body


def price(model: dict) -> tuple[float, float]:
    p = model.get("pricing") or {}
    return float(p.get("prompt") or 0), float(p.get("completion") or 0)


# ---------------------------------------------------------------------------------------------
# Key and HTTP


def read_key() -> str | None:
    try:
        r = subprocess.run(
            ["security", "find-generic-password", "-s", "openrouter", "-a", "layeredmap", "-w"],
            capture_output=True, text=True, timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    k = r.stdout.strip()
    return k if r.returncode == 0 and k else None


class OpenRouterClient:
    def __init__(self, key: str, timeout: float = 90.0):
        import requests

        self._s = requests.Session()
        self._key = key
        self.timeout = timeout

    def complete(self, body: dict) -> tuple[int, dict]:
        headers = {
            "Authorization": "Bearer " + self._key,
            "Content-Type": "application/json",
            "X-Title": "NimbleMap llm-eval",
        }
        r = self._s.post(API, json=body, headers=headers, timeout=self.timeout)
        try:
            data = r.json()
        except ValueError:
            data = {"error": {"message": r.text[:300]}}
        return r.status_code, data


class MockClient:
    """No network. Answers with the case's expected plan (``perfect``) or a fixed wrong one."""

    def __init__(self, cases: dict[str, dict], mode: str = "perfect"):
        self.cases = cases
        self.mode = mode

    def complete(self, body: dict) -> tuple[int, dict]:
        n = count_tokens(json.dumps(body, ensure_ascii=False))
        usage = {"prompt_tokens": n, "completion_tokens": 30, "cost": 0.0, "completion_tokens_details": {"reasoning_tokens": 0}}
        if body["messages"][-1]["role"] == "tool":
            return 200, {"choices": [{"message": {"role": "assistant", "content": "Done."}, "finish_reason": "stop"}], "usage": usage, "provider": "mock"}
        request = body["messages"][1]["content"]
        case = next(c for c in self.cases.values() if c["request"] == request)
        tool_names = {t["function"]["name"] for t in body["tools"]}
        calls = []
        if self.mode == "perfect":
            for i, c in enumerate(case["expected"]):
                calls.append({"id": f"c{i}", "type": "function", "function": {"name": c["name"], "arguments": json.dumps(mock_args(c, "run_action" in tool_names), ensure_ascii=False)}})
        else:
            calls.append({"id": "c0", "type": "function", "function": {"name": "cannot_do", "arguments": "{\"reason\": \"mock\"}"}})
        if "run_action" in tool_names:
            calls = [mock_single(c) for c in calls]
        return 200, {
            "choices": [{"message": {"role": "assistant", "content": None, "tool_calls": calls}, "finish_reason": "tool_calls"}],
            "usage": usage,
            "provider": "mock",
        }


def _first(x):
    return x[0] if isinstance(x, list) else x


def mock_args(c: dict, _single: bool) -> dict:
    """Expected canonical arguments as a model would write them."""
    out = {}
    for k, v in c["args"].items():
        if isinstance(v, dict) and "distance_m" in v:
            out[k] = f"{v['distance_m']} m"
        elif isinstance(v, dict) and "layer" in v:
            o = {"layer": _first(v["layer"])}
            if not v.get("selected"):
                o["field"], o["value"] = v["any"][0]
            if k == "target":
                out.update(o)
                continue
            out[k] = o
        elif k == "conditions":
            out[k] = [{kk: (_first(vv) if kk == "field" else (str(vv) if kk in ("value", "value2") else vv)) for kk, vv in cc.items() if kk != "tol"} for cc in v]
        else:
            out[k] = _first(v)
    return out


def mock_single(tc: dict) -> dict:
    name = tc["function"]["name"]
    if name in toolmod.DISPLAY_AND_META:
        return tc
    args = json.loads(tc["function"]["arguments"])
    table = {a["id"]: a for a in toolmod.action_table()}
    a = table[name]
    out = {"action": name, "layers": []}
    for role, kind in a["roles"]:
        if kind == "layer" and role in args:
            out["layers"].append(args[role])
        elif kind.startswith("object") and role in args:
            out["object"] = args[role]
    for p in a["params"]:
        if p in args:
            out[p] = args[p]
    return {**tc, "function": {"name": "run_action", "arguments": json.dumps(out, ensure_ascii=False)}}


# ---------------------------------------------------------------------------------------------
# Budget


class Budget:
    def __init__(self, path: Path, cap: float):
        self.path = path
        self.cap = cap
        self.lock = threading.Lock()
        self.spent = 0.0
        self.reserved = 0.0
        if path.exists():
            self.spent = float(json.loads(path.read_text()).get("spent_usd", 0.0))
        self.stopped = False

    def reserve(self, amount: float) -> bool:
        with self.lock:
            if self.stopped or self.spent + self.reserved + amount > self.cap:
                self.stopped = True
                return False
            self.reserved += amount
            return True

    def settle(self, reserved: float, actual: float) -> None:
        with self.lock:
            self.reserved -= reserved
            self.spent += actual
            self.path.write_text(json.dumps({"spent_usd": round(self.spent, 6), "cap_usd": self.cap}) + "\n")


# ---------------------------------------------------------------------------------------------
# One case


RETRYABLE = {408, 409, 425, 429, 500, 502, 503, 504, 520, 522, 524, 529}


NEW_LAYER_ACTIONS = {"count_in", "buffer", "summarize_each"}
MAX_TURNS = 5


def _request(client, body: dict, max_tries: int) -> tuple[int, dict, float, int, str | None]:
    """One completion with retries. (status, data, latency of the last try, tries, error)."""
    status, data, latency, tries = 0, {}, 0.0, 0
    for tries in range(1, max_tries + 1):
        t0 = time.monotonic()
        try:
            status, data = client.complete(body)
        except Exception as e:  # noqa: BLE001  (network errors are retried)
            status, data = 599, {"error": {"message": type(e).__name__}}
        latency = time.monotonic() - t0
        err = data.get("error") if isinstance(data, dict) else None
        choice = ((data.get("choices") or [{}])[0]) if isinstance(data, dict) else {}
        if status == 200 and not err and choice.get("finish_reason") == "error":
            if choice.get("native_finish_reason") == "MALFORMED_FUNCTION_CALL":
                # The model's own failure (Gemini), not a transient one: scored, not retried.
                return status, data, latency, tries, None
            err = choice.get("error") or {"message": "finish_reason error", "code": 502}
        code = status
        if status == 200 and err:
            code = int(err.get("code")) if str(err.get("code", "")).isdigit() else 502
        if status == 200 and not err and data.get("choices"):
            return status, data, latency, tries, None
        if code in RETRYABLE and tries < max_tries:
            time.sleep(min(60, 2 ** tries) * (0.5 + random.random()))
            continue
        return status, data, latency, tries, str((err or {}).get("message", "no choices"))[:300]
    return status, data, latency, tries, "no tries"


def run_case(client, model: dict, case: dict, tools: list, with_values: bool, budget: Budget, max_tries: int = 6, prompt: str = ctxmod.DEFAULT_PROMPT) -> dict:
    """Plan one request. Each tool call gets a stub result ("step #k added") and the model may
    continue, up to MAX_TURNS turns, until it replies without calls or asks or refuses."""
    system = ctxmod.system_message(case["context"], case.get("selection"), with_values, prompt)
    messages: list = [{"role": "system", "content": system}, {"role": "user", "content": case["request"]}]
    pin, pout = price(model)
    est_prompt = count_tokens(system) + count_tokens(json.dumps(tools, ensure_ascii=False)) + 50
    rec: dict = {"case": case["id"], "model": model["id"]}
    tot = {"prompt_tokens": 0, "completion_tokens": 0, "reasoning_tokens": 0, "cached_tokens": 0}
    cost = latency = 0.0
    tries_all = 0
    calls: list[dict] = []
    raw_calls: list[dict] = []
    text = ""
    turns = 0
    finish = None
    error = None
    for turn in range(MAX_TURNS):
        body = request_body(model, messages, tools)
        reserve = (est_prompt + 300 * turn) * pin * 1.3 + 1500 * pout
        if not budget.reserve(reserve):
            error = "budget"
            break
        actual = 0.0
        try:
            status, data, lat, tries, err = _request(client, body, max_tries)
            usage = data.get("usage") or {} if isinstance(data, dict) else {}
            actual = float(usage.get("cost") or 0.0)
        finally:
            budget.settle(reserve, actual)
        turns += 1
        tries_all += tries
        cost += actual
        latency += lat
        for k, path in [("prompt_tokens", ()), ("completion_tokens", ()), ("reasoning_tokens", ("completion_tokens_details",)), ("cached_tokens", ("prompt_tokens_details",))]:
            src = usage
            for p in path:
                src = src.get(p) or {}
            tot[k] += src.get(k) or 0
        rec["provider"] = data.get("provider") if isinstance(data, dict) else None
        rec["status"] = status
        if err:
            error = err
            break
        choice = data["choices"][0]
        finish = choice.get("finish_reason")
        if choice.get("native_finish_reason") == "MALFORMED_FUNCTION_CALL":
            rec["malformed"] = True
            break
        msg = choice.get("message") or {}
        new_calls, t = scoring.parse_tool_calls(msg)
        text = t or text
        tcs = msg.get("tool_calls") or []
        for tc in tcs:
            f = tc.get("function") or {}
            raw_calls.append({"turn": turn + 1, "name": f.get("name"), "arguments": f.get("arguments")})
        if not new_calls:
            break
        calls.extend(new_calls)
        if any(c["name"] in ("ask_user", "cannot_do") for c in new_calls):
            break
        # Send back the assistant turn as given (keeping any reasoning details) and stub results.
        amsg = {k: v for k, v in msg.items() if k in ("role", "content", "tool_calls", "reasoning_details", "reasoning")}
        amsg["role"] = "assistant"
        fixed = []
        for i, tc in enumerate(tcs):
            tc = dict(tc)
            tc.setdefault("id", f"call_{turn}_{i}")
            if not tc.get("id"):
                tc["id"] = f"call_{turn}_{i}"
            tc.setdefault("type", "function")
            fixed.append(tc)
        amsg["tool_calls"] = fixed
        messages.append(amsg)
        first = len(calls) - len(new_calls)
        for i, (tc, c) in enumerate(zip(fixed, new_calls)):
            k = first + i + 1
            note = f"Step #{k} added to the plan (not run yet)."
            if c["name"] in NEW_LAYER_ACTIONS:
                note += f' Later steps can use its new layer as "#{k}".'
            extra = toolmod.output_note(c)
            if extra:
                note += " " + extra
            messages.append({"role": "tool", "tool_call_id": tc["id"], "content": note})
    rec.update(
        {
            "turns": turns,
            "tries": tries_all,
            "latency_s": round(latency, 3),
            **tot,
            "cost_usd": cost,
            "finish_reason": finish,
            "raw_calls": raw_calls,
            "calls": calls,
            "text": (text or "")[:500],
        }
    )
    if error:
        rec["error"] = error
        return rec
    rec.update(scoring.score(case, calls, text))
    if rec.get("malformed") and not rec["strict"]:
        rec["reason"] = "malformed_call"
    return rec


# ---------------------------------------------------------------------------------------------


def load_cases(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def slug(model_id: str) -> str:
    return model_id.replace("/", "__").replace(":", "_")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--models", default=",".join(SHORTLIST), help="comma-separated OpenRouter ids")
    ap.add_argument("--design", choices=list(toolmod.DESIGNS), default="per_action")
    ap.add_argument("--variant", choices=["value_free", "with_values"], default="value_free")
    ap.add_argument("--prompt", choices=list(ctxmod.INSTRUCTIONS), default=ctxmod.DEFAULT_PROMPT)
    ap.add_argument("--cases", default=str(HERE / "cases.jsonl"))
    ap.add_argument("--only", default="", help="comma-separated case ids")
    ap.add_argument("--budget", type=float, default=3.0, help="US$ cap for all runs on this date")
    ap.add_argument("--concurrency", type=int, default=8)
    ap.add_argument("--per-model", type=int, default=4, help="concurrent requests per model")
    ap.add_argument("--date", default=dt.date.today().isoformat())
    ap.add_argument("--out", default=str(HERE / "results"))
    ap.add_argument("--dry-run", action="store_true", help="mocked client, no network, no key")
    ap.add_argument("--mock-mode", default="perfect", choices=["perfect", "refuse"])
    ap.add_argument("--provider-ignore", default="", help="comma-separated OpenRouter provider names to avoid")
    ap.add_argument("--tag", default="", help="suffix for the run folder, e.g. repeat2")
    ap.add_argument("--rerun", action="store_true", help="ignore earlier results for these cases")
    a = ap.parse_args(argv)

    PROVIDER_IGNORE[:] = [p.strip() for p in a.provider_ignore.split(",") if p.strip()]
    catalog = load_catalog()
    models = [m.strip() for m in a.models.split(",") if m.strip()]
    missing = [m for m in models if m not in catalog]
    if missing:
        print("not in the catalog:", ", ".join(missing), file=sys.stderr)
        return 2
    cases = load_cases(Path(a.cases))
    if a.only:
        keep = set(a.only.split(","))
        cases = [c for c in cases if c["id"] in keep]
    tools = toolmod.tools_for(a.design)
    with_values = a.variant == "with_values"

    run_name = a.design + ("" if a.variant == "value_free" else f"-{a.variant}") + ("" if a.prompt == ctxmod.DEFAULT_PROMPT else f"-{a.prompt}")
    if a.tag:
        run_name += f"-{a.tag}"
    if a.dry_run:
        run_name = "dryrun-" + run_name
    out_dir = Path(a.out) / a.date / run_name
    out_dir.mkdir(parents=True, exist_ok=True)
    budget = Budget(Path(a.out) / a.date / ("spend-dryrun.json" if a.dry_run else "spend.json"), a.budget)

    if a.dry_run:
        client = MockClient({c["id"]: c for c in cases}, a.mock_mode)
    else:
        key = read_key()
        if not key:
            print("No OpenRouter key in the keychain (service openrouter, account layeredmap).", file=sys.stderr)
            return 3
        client = OpenRouterClient(key)
        del key

    print(f"{run_name}: {len(models)} models × {len(cases)} cases; budget ${a.budget:.2f}, spent so far ${budget.spent:.4f}")
    sems = {m: threading.Semaphore(a.per_model) for m in models}
    locks = {m: threading.Lock() for m in models}
    tasks = []
    for m in models:
        path = out_dir / f"{slug(m)}.jsonl"
        done = set()
        if path.exists() and not a.rerun:
            for line in path.read_text(encoding="utf-8").splitlines():
                r = json.loads(line)
                if "error" not in r:
                    done.add(r["case"])
        elif a.rerun and path.exists():
            kept = [line for line in path.read_text(encoding="utf-8").splitlines() if json.loads(line)["case"] not in {c["id"] for c in cases}]
            path.write_text("".join(k + "\n" for k in kept), encoding="utf-8")
        for c in cases:
            if c["id"] not in done:
                tasks.append((m, c, path))

    def work(m, c, path):
        with sems[m]:
            rec = run_case(client, catalog[m], c, tools, with_values, budget, prompt=a.prompt)
        rec["design"], rec["variant"], rec["prompt"] = a.design, a.variant, a.prompt
        if PROVIDER_IGNORE:
            rec["provider_ignore"] = list(PROVIDER_IGNORE)
        if rec.get("error") == "budget":
            return rec
        with locks[m]:
            # Drop an earlier failed attempt of this case, then append.
            if path.exists():
                lines = [line for line in path.read_text(encoding="utf-8").splitlines() if json.loads(line)["case"] != c["id"]]
            else:
                lines = []
            lines.append(json.dumps(rec, ensure_ascii=False))
            path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return rec

    random.seed(0)
    random.shuffle(tasks)  # spread each model's requests over the run
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
