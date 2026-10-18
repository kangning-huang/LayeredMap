"""Exports the tested assistant harness for the app: the system prompt and the tools.

    python -I tools/lm/assistant/export_assets.py           # writes crates/lm-ask/assets/
    python -I tools/lm/assistant/export_assets.py --check   # fails if the files are out of date

The app's Ask box (Phase 7, early test build) uses the same prompt, rules, worked examples and
``submit_plan`` + ``resolve_feature`` tools that the v2 harness and the puzzle tests measured
(``tools/lm/puzzles/pro_e2e.py``: the v2 static prefix plus the puzzle-era rules and examples).

The tools depend on the map's layers (each layer argument is an enum of layer names). They are
exported for a template map whose layers are the placeholders ``__POINTS__``, ``__LINES__``,
``__AREAS__`` and ``__RASTER__``. The app replaces each placeholder in an enum with the names of
its layers of that kind (``lm_ask::tools``).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "tools/lm/puzzles"))

import pro_e2e as pe  # noqa: E402

OUT = ROOT / "crates/lm-ask/assets"
PLACEHOLDERS = {"__POINTS__": "points", "__LINES__": "lines", "__AREAS__": "areas", "__RASTER__": "raster"}


def build() -> dict[str, str]:
    for name, kind in PLACEHOLDERS.items():
        pe.ctxmod.LAYERS[name] = {"kind": kind, "desc": ""} if kind == "raster" else {"kind": kind, "count": 0, "fields": []}
    pe.ctxmod.CONTEXTS["__template__"] = list(PLACEHOLDERS)
    tools = pe.tools_v2.submit_plan_tools("__template__", with_resolve=True)
    return {
        "system.txt": pe.static_prefix() + "\n",
        "tools.json": json.dumps(tools, ensure_ascii=False, indent=1) + "\n",
    }


def main(argv: list[str]) -> int:
    files = build()
    if "--check" in argv:
        stale = [n for n, s in files.items() if not (OUT / n).exists() or (OUT / n).read_text(encoding="utf-8") != s]
        if stale:
            print(f"Out of date: {', '.join(stale)}. Run python -I tools/lm/assistant/export_assets.py", file=sys.stderr)
            return 1
        return 0
    OUT.mkdir(parents=True, exist_ok=True)
    for n, s in files.items():
        (OUT / n).write_text(s, encoding="utf-8")
        print(f"wrote {OUT / n} ({len(s):,} characters)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
