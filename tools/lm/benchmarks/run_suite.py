"""Run the benchmark suite with the eval's own runner (``tools/lm/llm_eval/run_eval.py``).

    python -I tools/lm/benchmarks/run_suite.py --dry-run --out /tmp/bench-results
    python -I tools/lm/benchmarks/run_suite.py --models anthropic/claude-haiku-5.5 --only gbx-009,ace-zh-003

Every argument is passed to run_eval.py, with ``--cases`` defaulting to ``suite.jsonl`` here.
The only addition is that run_eval can now build system messages for this folder's contexts
(``contexts/*.json``); it is done in memory, and nothing in ``llm_eval`` changes. Live runs call
OpenRouter and spend money under run_eval's budget rules; ``--dry-run`` uses its mocked client.
Pass ``--out`` to keep results outside ``llm_eval/results``.
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # python -I drops the script's folder

import benchlib  # noqa: E402

benchlib.install_contexts()

import run_eval  # noqa: E402  (from llm_eval, on sys.path via benchlib)


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if not any(a == "--cases" or a.startswith("--cases=") for a in argv):
        argv = ["--cases", str(HERE / "suite.jsonl")] + argv
    return run_eval.main(argv)


if __name__ == "__main__":
    raise SystemExit(main())
