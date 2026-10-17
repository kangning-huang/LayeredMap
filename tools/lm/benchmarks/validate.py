"""Validate the benchmark mappings and the merged suite.

    python -I tools/lm/benchmarks/validate.py              # sources/*.jsonl and suite.jsonl
    python -I tools/lm/benchmarks/validate.py sources/geobenchx.jsonl

Checks, for every record: the fields and their allowed values (coverage class, text policy,
missing operations named with an upstream RustGIS function or "new"); and for every case: the
eval's case keys, Chinese text in zh cases, and that each expected plan (``expected``, ``alts``,
``lenient_alts``) uses connected action names, argument names those actions take, layers that
exist in the case's context (or a "#k" naming an earlier step that makes a layer), layers of
the right kind for each role, fields those layers have (including fields made by earlier steps),
valid filter operators, and ``stat`` values the runner accepts. Case ids must be unique across
all sources. Exit status 1 on any error.
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # python -I drops the script's folder

import benchlib as b  # noqa: E402


def validate_sources(files: list[Path] | None = None) -> list[str]:
    errors: list[str] = []
    ids: Counter = Counter()
    keys: Counter = Counter()
    for name, n, r in b.load_sources():
        if files and not any(f.name == name for f in files):
            continue
        for e in b.validate_record(r):
            errors.append(f"{name}:{n} [{r.get('source_id')}] {e}")
        keys[(r.get("source"), r.get("source_id"))] += 1
        for c in r.get("cases", []):
            ids[c.get("id")] += 1
    for k, v in keys.items():
        if v > 1:
            errors.append(f"duplicate task {k} ({v} records)")
    for k, v in ids.items():
        if v > 1:
            errors.append(f"duplicate case id {k!r} ({v} times)")
    return errors


def validate_suite(path: Path) -> list[str]:
    errors = []
    seen = set()
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        c = json.loads(line)
        if c["id"] in seen:
            errors.append(f"suite:{n} duplicate id {c['id']}")
        seen.add(c["id"])
        if not isinstance(c.get("categories"), list):
            errors.append(f"suite:{n} categories must be a list")
        core = {k: c[k] for k in b.CASE_KEYS if k in c}
        for e in b.validate_case(core):
            errors.append(f"suite:{n} [{c['id']}] {e}")
    return errors


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    files = [Path(a) for a in argv if a.endswith(".jsonl") and "suite" not in Path(a).name]
    errors = validate_sources(files or None)
    suite = HERE / "suite.jsonl"
    if not files and suite.exists():
        errors += validate_suite(suite)
    for e in errors:
        print(e)
    n = len(b.load_sources())
    print(f"{n} records checked; {len(errors)} errors")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
