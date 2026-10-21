"""Talks to the headless Ask box (``crates/lm-ui/examples/ask_eval.rs``) over its stdin/stdout.

The Rust side builds every model request exactly as the app does and runs the plan through the
app's checker, executor and preview; this side only decides who answers each request.
"""

from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path

TARGET = Path(os.environ.get("CARGO_TARGET_DIR", "/Volumes/LaCie/cargo-target/layeredmap-harness"))
DEFAULT_BIN = TARGET / "release/examples/ask_eval"


class AppCrashed(Exception):
    pass


class AskEval:
    def __init__(self, binary: Path | str | None = None, log: Path | None = None, reference_dir: str | None = None):
        self.binary = str(binary or os.environ.get("LM_ASK_EVAL_BIN") or DEFAULT_BIN)
        self.log = log
        self.reference_dir = reference_dir
        self.p: subprocess.Popen | None = None

    def _start(self) -> None:
        env = dict(os.environ)
        for k in ("OPENROUTER_API_KEY",):  # the app side never needs a key
            env.pop(k, None)
        if self.reference_dir:
            env["LM_REFERENCE_DIR"] = self.reference_dir
        err = open(self.log, "a") if self.log else subprocess.DEVNULL  # noqa: SIM115
        self.p = subprocess.Popen([self.binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, text=True, bufsize=1, env=env)

    def close(self) -> None:
        if self.p and self.p.poll() is None:
            try:
                self.p.stdin.write(json.dumps({"op": "quit"}) + "\n")
                self.p.stdin.flush()
                self.p.wait(timeout=10)
            except (OSError, subprocess.TimeoutExpired):
                self.p.kill()
        self.p = None

    def _send(self, obj: dict) -> None:
        assert self.p and self.p.stdin
        self.p.stdin.write(json.dumps(obj, ensure_ascii=False) + "\n")
        self.p.stdin.flush()

    def _read(self) -> dict:
        assert self.p and self.p.stdout
        line = self.p.stdout.readline()
        if not line:
            code = self.p.poll()
            self.p = None
            raise AppCrashed(f"ask_eval exited ({code})")
        return json.loads(line)

    def run(self, job: dict, client) -> dict:
        """Runs one case. ``client.complete(body) -> (reply, error, info)`` answers each request.
        Returns the app's result plus ``calls`` (one entry per request: latency, tries, error)."""
        if self.p is None or self.p.poll() is not None:
            self._start()
        if hasattr(client, "start"):
            client.start(job["id"])
        calls = []
        t0 = time.monotonic()
        self._send({"op": "case", **job})
        while True:
            msg = self._read()
            if msg.get("op") == "request":
                reply, error, info = client.complete(msg["body"])  # may raise BudgetExceeded
                calls.append({"body_tokens_est": len(json.dumps(msg["body"], ensure_ascii=False)) // 3 + 50, "latency": round(info.get("latency", 0.0), 3), "tries": info.get("tries", 1), "error": error,
                              "usage": (reply or {}).get("usage"), "provider": (reply or {}).get("provider")})
                self._send({"op": "reply", "error": error} if error else {"op": "reply", "reply": reply})
            elif msg.get("op") == "result":
                msg["calls"] = calls
                msg["wall_seconds"] = round(time.monotonic() - t0, 3)
                return msg
            else:
                raise AppCrashed(f"unexpected message {str(msg)[:200]}")
