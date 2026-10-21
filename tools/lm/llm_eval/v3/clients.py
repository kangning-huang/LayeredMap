"""Who answers the app's model requests in harness v3, and the spending ledger.

- ``OpenRouterClient``: the real service. The key is read from the macOS keychain inside this
  process (``security find-generic-password -s openrouter -a layeredmap -w``), kept only in the
  client, and never printed, logged or written anywhere; results never contain headers.
- ``PlanClient``: no network. Answers with a fixed plan per case (the case's gold plan for the
  mock run, or a plan recorded in an earlier run for a replay), as a ``submit_plan`` call.
- ``Ledger``: every paid request first reserves its worst-case cost (prompt tokens estimated from
  the body, plus ``max_tokens`` of output, at the catalogue's prices); a request that would take
  the total over the cap is not sent. The actual cost (OpenRouter's ``usage.cost``) is settled
  afterwards and written to ``results/spend.json`` at once.
"""

from __future__ import annotations

import json
import random
import subprocess
import threading
import time
from pathlib import Path

API = "https://openrouter.ai/api/v1/chat/completions"
CAP_USD = 10.00
RETRYABLE = {408, 409, 425, 429, 500, 502, 503, 504, 520, 522, 524, 529}


def est_tokens(obj) -> int:
    """A deliberately high estimate of a body's prompt tokens (about 3 characters per token)."""
    return len(json.dumps(obj, ensure_ascii=False)) // 3 + 50


def prices(catalog: dict, model: str) -> tuple[float, float]:
    p = (catalog.get(model) or {}).get("pricing") or {}
    return float(p.get("prompt") or 0), float(p.get("completion") or 0)


class BudgetExceeded(Exception):
    pass


class Ledger:
    def __init__(self, path: Path, cap: float = CAP_USD):
        self.path = path
        self.cap = cap
        self.lock = threading.Lock()
        self.reserved = 0.0
        self.spent = 0.0
        self.runs: dict = {}
        if path.exists():
            d = json.loads(path.read_text())
            self.spent = float(d.get("spent_usd", 0.0))
            self.runs = d.get("runs", {})
        self.stopped = False

    def remaining(self) -> float:
        with self.lock:
            return self.cap - self.spent - self.reserved

    def reserve(self, amount: float) -> bool:
        with self.lock:
            if self.stopped or self.spent + self.reserved + amount > self.cap:
                self.stopped = True
                return False
            self.reserved += amount
            return True

    def settle(self, reserved: float, actual: float, run: str = "") -> None:
        with self.lock:
            self.reserved -= reserved
            self.spent += actual
            if run:
                self.runs[run] = round(self.runs.get(run, 0.0) + actual, 6)
            self.path.parent.mkdir(parents=True, exist_ok=True)
            self.path.write_text(json.dumps({"spent_usd": round(self.spent, 6), "cap_usd": self.cap, "runs": self.runs}, indent=1) + "\n")


def read_key() -> str | None:
    try:
        r = subprocess.run(["security", "find-generic-password", "-s", "openrouter", "-a", "layeredmap", "-w"], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    k = r.stdout.strip()
    return k if r.returncode == 0 and k else None


class OpenRouterClient:
    """Sends the app's request body unchanged (the app's own transport adds only headers)."""

    def __init__(self, key: str, ledger: Ledger, catalog: dict, run: str, timeout: float = 90.0, max_tries: int = 5, transform=None):
        import requests

        self.transform = transform
        self._s = requests.Session()
        self._key = key
        self.ledger = ledger
        self.catalog = catalog
        self.run = run
        self.timeout = timeout
        self.max_tries = max_tries

    def __repr__(self) -> str:  # never show the key
        return "OpenRouterClient()"

    def complete(self, body: dict) -> tuple[dict | None, str | None, dict]:
        """(reply, error, info). Raises BudgetExceeded before sending when the cap would be passed."""
        if self.transform:
            body = self.transform(body)
        pin, pout = prices(self.catalog, body.get("model", ""))
        reserve = est_tokens(body) * pin + int(body.get("max_tokens") or 2500) * pout
        if not self.ledger.reserve(reserve):
            raise BudgetExceeded()
        info = {"tries": 0, "latency": 0.0}
        actual = 0.0
        try:
            for tries in range(1, self.max_tries + 1):
                info["tries"] = tries
                t0 = time.monotonic()
                try:
                    r = self._s.post(
                        API,
                        json=body,
                        headers={"Authorization": "Bearer " + self._key, "Content-Type": "application/json", "X-Title": "NimbleMap llm-eval v3"},
                        timeout=self.timeout,
                    )
                    status = r.status_code
                    try:
                        data = r.json()
                    except ValueError:
                        data = {"error": {"message": r.text[:200]}}
                except Exception as e:  # noqa: BLE001  (network errors are retried)
                    status, data = 599, {"error": {"message": type(e).__name__}}
                info["latency"] += time.monotonic() - t0
                cost = float(((data or {}).get("usage") or {}).get("cost") or 0.0)
                actual += cost
                err = (data or {}).get("error")
                if status == 200 and not err and data.get("choices"):
                    return data, None, info
                code = status
                if status == 200 and err:
                    code = int(err.get("code")) if str(err.get("code", "")).isdigit() else 502
                if code in RETRYABLE and tries < self.max_tries:
                    time.sleep(min(30, 2**tries) * (0.5 + random.random()))
                    continue
                return None, f"HTTP {status}: {str((err or {}).get('message', 'no choices'))[:200]}", info
            return None, "no tries", info
        finally:
            self.ledger.settle(reserve, actual, self.run)


def submit_reply(steps: list, text: str | None = None, call_id: str = "plan1") -> dict:
    """A chat-completions reply carrying one submit_plan call (or plain text)."""
    usage = {"prompt_tokens": 0, "completion_tokens": 0, "cost": 0.0}
    if steps is None:
        return {"choices": [{"message": {"role": "assistant", "content": text or ""}, "finish_reason": "stop"}], "usage": usage}
    call = {"id": call_id, "type": "function", "function": {"name": "submit_plan", "arguments": json.dumps({"steps": steps}, ensure_ascii=False)}}
    return {"choices": [{"message": {"role": "assistant", "content": None, "tool_calls": [call]}, "finish_reason": "tool_calls"}], "usage": usage}


class PlanClient:
    """No network: answers every request of a case with that case's fixed plan. After a repair
    message the same plan is sent again (a fixed plan can't repair itself); the app gives up after
    its two repair rounds."""

    def __init__(self, plans: dict[str, list | None]):
        self.plans = plans
        self.case: str | None = None
        self.n = 0

    def start(self, case_id: str) -> None:
        self.case = case_id
        self.n = 0

    def complete(self, body: dict) -> tuple[dict | None, str | None, dict]:
        self.n += 1
        steps = self.plans.get(self.case or "")
        if steps is None:
            return submit_reply(None, "No plan recorded."), None, {"tries": 1, "latency": 0.0}
        return submit_reply(steps, call_id=f"plan{self.n}"), None, {"tries": 1, "latency": 0.0}


RULE19_START = "19. A field an earlier step made can be named by that step"


def drop_rule19(body: dict) -> dict:
    """Ablation: the app's request without system rule 19 (the "#k" field rule added by askfix).
    Everything else, including the executor's own name resolution, stays."""
    b = json.loads(json.dumps(body))
    for part in b["messages"][0].get("content") or []:
        t = part.get("text", "") if isinstance(part, dict) else ""
        i = t.find(RULE19_START)
        if i >= 0:
            j = t.find("\n\n", i)
            part["text"] = t[:i] + t[j + 2 :] if j >= 0 else t[:i]
    return b


def tool_choice_auto(body: dict) -> dict:
    """For models whose providers refuse a forced tool choice (Claude Sonnet 5.5 on OpenRouter:
    "tool_choice: type tool and any are not supported for this model"): the app's request with
    tool_choice "auto". Used only for the reference model; the app itself always forces it."""
    return {**body, "tool_choice": "auto"}
