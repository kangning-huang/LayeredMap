"""Regenerate every hand-made fixture and every expected output.

Runs make_handmade.py and make_summary_fixtures.py first, then each reference script
(make_vector_ops.py builds N9, N10 and N13 just before their references), each in a fresh
`python -I` subprocess of the current interpreter. Deterministic: rerunning
gives the same bytes. With --check, runs everything twice and verifies that
the SHA-256 of every file under fixtures/ (excluding fixtures/generated/) is
unchanged between the two runs.

Usage:
  python -I reference/run_all.py           # regenerate
  python -I reference/run_all.py --check   # regenerate twice and compare
"""

from __future__ import annotations

import hashlib
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIX = HERE.parent / "fixtures"
# zonal_r7.py and n7_sample.py need generated fixtures (cargo xtask fixtures --only R7, and
# --only R4,N7) and say "skipped" without them.
SCRIPTS = ["make_handmade.py", "make_summary_fixtures.py", "coord_vectors.py", "count_in_areas.py", "which_area.py",
           "zonal_mean.py", "category_shares.py", "zonal_totals.py", "zonal_r7.py", "n7_sample.py", "geodesic.py",
           "raster_sample.py", "rank_check.py", "make_where_clip_fixtures.py", "find_where.py", "clip_erase.py",
           "make_vector_ops.py", "nearest_distance.py", "join_by_location.py", "measure_features.py",
           "make_kml_gpx_fixtures.py", "make_netcdf_fixtures.py"]


def run_once():
    for s in SCRIPTS:
        r = subprocess.run([sys.executable, "-I", str(HERE / s)], capture_output=True, text=True)
        out = (r.stdout + r.stderr).strip()
        print(f"[{s}] {out}" if out else f"[{s}] ok")
        if r.returncode != 0:
            raise SystemExit(f"{s} failed with exit code {r.returncode}")


def snapshot() -> dict[str, str]:
    out = {}
    for p in sorted(FIX.rglob("*")):
        rel = p.relative_to(FIX)
        if rel.parts[0] == "generated" or not p.is_file() or p.name.startswith("._") or p.name == ".DS_Store":
            continue
        out[str(rel)] = hashlib.sha256(p.read_bytes()).hexdigest()
    return out


def main():
    check = "--check" in sys.argv[1:]
    run_once()
    if not check:
        return
    first = snapshot()
    print("--- second run ---")
    run_once()
    second = snapshot()
    diff = sorted(k for k in set(first) | set(second) if first.get(k) != second.get(k))
    if diff:
        raise SystemExit("NOT deterministic:\n  " + "\n  ".join(diff))
    print(f"deterministic: {len(first)} files identical across two runs")


if __name__ == "__main__":
    main()
