"""Harness v3 runner: every case goes through the app's own Ask box, headless
(``crates/lm-ui/examples/ask_eval.rs``): the app's prompt, tools, context, checks, repairs, plan
executor and preview. Only the answers to the model requests come from here.

    python -I run_v3.py --mode mock                      # gold plans, no network (dry run)
    python -I run_v3.py --mode replay --from results/haiku-1/anthropic_claude-haiku-5.5.jsonl --tag prefix
    python -I run_v3.py --mode paid --models anthropic/claude-haiku-5.5,z-ai/glm-5.3-flash
    python -I run_v3.py --mode paid --models anthropic/claude-haiku-5.5 --tag repeat2
    python -I run_v3.py --estimate --models anthropic/claude-sonnet-5.5    # cost estimate only

Paid runs share one ledger (``results/spend.json``, cap US$10.00): each request reserves its
worst-case cost first and is not sent if that would pass the cap; the run then stops cleanly.
The OpenRouter key is read from the keychain inside this process and never printed or stored.
Raw results: ``$LM_EVAL_RESULTS/<run>/<model>.jsonl`` (default ``$LM_EVAL_DATA/results-v3``,
outside the repo), one line per case (no headers, no key); ``report_v3.py`` writes the scored
summaries into ``results/``.
"""

from __future__ import annotations

import argparse
import os
import json
import queue
import statistics
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import clients  # noqa: E402
import data_v3  # noqa: E402
import driver  # noqa: E402

CASES = HERE / "cases_v3.jsonl"
RESULTS = HERE / "results"  # the ledger and the summaries (in the repo)
# Raw results (every request's exchange, outcomes, preview keys: ~200 MB) stay outside the repo.
RAW = Path(os.environ.get("LM_EVAL_RESULTS", str(data_v3.DATA / "results-v3")))
CATALOG = HERE.parent / "openrouter_models_2026-10-17.json"
DEFAULT_MODELS = ["anthropic/claude-haiku-5.5", "z-ai/glm-5.3-flash"]


def load_cases(path: Path = CASES) -> list[dict]:
    return [json.loads(x) for x in path.read_text(encoding="utf-8").splitlines() if x.strip()]


def load_catalog() -> dict:
    d = json.loads(CATALOG.read_text())
    ms = d if isinstance(d, list) else d.get("data", d)
    return {m["id"]: m for m in ms}


def slug(model: str) -> str:
    return model.replace("/", "_").replace(":", "_")


def job_for(case: dict, model: str | None) -> dict:
    j = {"id": case["id"], "files": data_v3.files(case["map"]), "text": case["text"], "keys": data_v3.KEYS}
    if model:
        j["model"] = model
    if case.get("selected"):
        j["selected"] = case["selected"]
    return j


def strip(result: dict) -> dict:
    """What is kept of a result: everything the app returned (it never holds headers or keys)."""
    r = dict(result)
    r.pop("op", None)
    return r


def estimate(cases: list[dict], model: str, catalog: dict, mock_dir: Path | None) -> float:
    """Expected cost of one run: the prompt size per case from the mock run's request bodies (or
    12k tokens), 1.6 requests per case, 500 output tokens per request (a high guess)."""
    pin, pout = clients.prices(catalog, model)
    sizes = {}
    if mock_dir and mock_dir.exists():
        for f in (x for x in mock_dir.glob("*.jsonl") if not x.name.startswith("._")):
            for line in f.read_text(encoding="utf-8").splitlines():
                r = json.loads(line)
                if r.get("calls"):
                    sizes[r["id"]] = r["calls"][0].get("body_tokens_est", 12000)
    per = [sizes.get(c["id"], 12000) * 1.6 * pin + 1.6 * 500 * pout for c in cases]
    return sum(per)


def run(cases, mode, model, out_path: Path, workers: int, ledger, catalog, plans, binary, run_name, log_dir: Path, transform=None) -> dict:
    done = set()
    if out_path.exists():
        done = {json.loads(x)["id"] for x in out_path.read_text(encoding="utf-8").splitlines() if x.strip()}
    todo = [c for c in cases if c["id"] not in done]
    todo.sort(key=lambda c: (c["map"], c["id"]))
    q: queue.Queue = queue.Queue()
    # Contiguous chunks by map, so each worker opens few maps.
    chunk = max(1, (len(todo) + workers - 1) // workers)
    for i in range(0, len(todo), chunk):
        q.put(todo[i : i + chunk])
    lock = threading.Lock()
    stats = {"done": 0, "crashed": 0, "budget": False}
    key = clients.read_key() if mode == "paid" else None
    if mode == "paid" and not key:
        raise SystemExit("No OpenRouter key in the keychain (service openrouter, account layeredmap).")

    def worker(n: int):
        app = driver.AskEval(binary, log=log_dir / f"{run_name}-{slug(model or mode)}-{n}.log")
        client = clients.OpenRouterClient(key, ledger, catalog, run_name, transform=transform) if mode == "paid" else clients.PlanClient(plans)
        try:
            while not stats["budget"]:
                try:
                    batch = q.get_nowait()
                except queue.Empty:
                    return
                for c in batch:
                    if stats["budget"]:
                        return
                    try:
                        res = app.run(job_for(c, model), client)
                    except clients.BudgetExceeded:
                        stats["budget"] = True
                        app.close()
                        return
                    except driver.AppCrashed as e:
                        res = {"id": c["id"], "answer": {"kind": "crash", "text": str(e)}}
                        stats["crashed"] += 1
                    res["run"] = run_name
                    res["mode"] = mode
                    with lock:
                        with out_path.open("a", encoding="utf-8") as f:
                            f.write(json.dumps(strip(res), ensure_ascii=False) + "\n")
                        stats["done"] += 1
                        if stats["done"] % 20 == 0:
                            print(f"  {run_name} {model or mode}: {stats['done']}/{len(todo)} done; spent ${ledger.spent:.4f}" if ledger else f"  {stats['done']}/{len(todo)}", flush=True)
        finally:
            app.close()

    threads = [threading.Thread(target=worker, args=(i,)) for i in range(workers)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    return stats


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--mode", choices=["mock", "replay", "paid"], default="mock")
    ap.add_argument("--models", default=",".join(DEFAULT_MODELS))
    ap.add_argument("--from", dest="replay_from", default="", help="replay: a results .jsonl whose final plans are replayed")
    ap.add_argument("--tag", default="")
    ap.add_argument("--only", default="")
    ap.add_argument("--cases", default=str(CASES))
    ap.add_argument("--bin", default="")
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--cap", type=float, default=clients.CAP_USD)
    ap.add_argument("--estimate", action="store_true")
    ap.add_argument("--drop-rule19", action="store_true", help="ablation: send the app's request without system rule 19")
    ap.add_argument("--tool-choice-auto", action="store_true", help="send tool_choice auto (models that refuse a forced tool call)")
    a = ap.parse_args(argv)

    cases = load_cases(Path(a.cases))
    if a.only:
        keep = set(a.only.split(","))
        cases = [c for c in cases if c["id"] in keep or c["qtype"] in keep]
    catalog = load_catalog()
    RESULTS.mkdir(exist_ok=True)
    RAW.mkdir(parents=True, exist_ok=True)
    log_dir = RAW / "logs"
    log_dir.mkdir(exist_ok=True)
    binary = a.bin or None

    if a.mode in ("mock", "replay") and not a.estimate:
        if a.mode == "mock":
            plans = {c["id"]: c.get("gold") for c in cases}
            run_name = "mock" + (f"-{a.tag}" if a.tag else "")
        else:
            src = Path(a.replay_from)
            plans = {}
            for line in src.read_text(encoding="utf-8").splitlines():
                r = json.loads(line)
                plans[r["id"]] = final_submitted(r)
            run_name = f"replay-{src.parent.name}" + (f"-{a.tag}" if a.tag else "")
        out_dir = RAW / run_name
        out_dir.mkdir(exist_ok=True)
        out = out_dir / (f"{a.mode}.jsonl" if a.mode == "mock" else src.name)
        t0 = time.monotonic()
        st = run(cases, a.mode, None, out, a.workers, None, catalog, plans, binary, run_name, log_dir)
        print(f"{run_name}: {st['done']} cases in {time.monotonic() - t0:.0f} s, {st['crashed']} crashed -> {out}")
        return 0

    models = [m.strip() for m in a.models.split(",") if m.strip()]
    missing = [m for m in models if m not in catalog]
    if missing:
        print("not in the catalogue:", ", ".join(missing), file=sys.stderr)
        return 2
    ledger = clients.Ledger(RESULTS / "spend.json", a.cap)
    mock_dir = RAW / "mock"
    for m in models:
        est = estimate(cases, m, catalog, mock_dir)
        print(f"estimate {m}: ${est:.3f} for {len(cases)} cases; spent so far ${ledger.spent:.4f} of ${ledger.cap:.2f}")
        if a.estimate:
            continue
        if est > ledger.remaining():
            print(f"  skipped: the estimate is more than the ${ledger.remaining():.3f} left under the cap")
            continue
        run_name = "paid" + (f"-{a.tag}" if a.tag else "")
        out_dir = RAW / run_name
        out_dir.mkdir(exist_ok=True)
        t0 = time.monotonic()
        st = run(cases, "paid", m, out_dir / f"{slug(m)}.jsonl", a.workers, ledger, catalog, {}, binary, run_name, log_dir,
                 transform=transform_for(a))
        print(f"{run_name} {m}: {st['done']} cases in {time.monotonic() - t0:.0f} s; spent ${ledger.spent:.4f} of ${ledger.cap:.2f}")
        if st["budget"]:
            print("Budget cap reached: stopped cleanly; the remaining cases were not sent.")
            break
    return 0


def transform_for(a):
    fs = ([clients.drop_rule19] if a.drop_rule19 else []) + ([clients.tool_choice_auto] if a.tool_choice_auto else [])
    if not fs:
        return None

    def t(body):
        for f in fs:
            body = f(body)
        return body

    return t


def final_submitted(r: dict) -> list | None:
    """The last plan a model submitted in a recorded exchange (its steps), or its ask/refuse as a
    one-step plan; None when it never submitted."""
    last = None
    for m in r.get("messages") or []:
        for c in m.get("tool_calls") or []:
            if c.get("function", {}).get("name") == "submit_plan":
                try:
                    last = json.loads(c["function"].get("arguments") or "{}").get("steps")
                except (ValueError, TypeError):
                    pass
    return last


def summary_latency(rows: list[dict]) -> dict:
    s = [r.get("wall_seconds", 0) for r in rows]
    return {"median_s": statistics.median(s) if s else 0}


if __name__ == "__main__":
    raise SystemExit(main())
