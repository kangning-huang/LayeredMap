"""Reference: rank-phrase inputs for the insight panel.

Rule (LayeredMap insight "where does this object stand?"):
  * N = number of objects with a value in the field; missing values are
    excluded from the denominator and get no phrase;
  * rank_from_top    = 1 + number of values strictly greater (ties share a rank);
    rank_from_bottom = 1 + number of values strictly smaller;
    tie_count        = number of objects with exactly this value (1 = no tie);
  * N <= 2           -> no rank phrase;
  * N >= 1000        -> "higher than X% of N", X = floor(100 * strictly_smaller / N);
  * otherwise upper half (rank_from_top <= rank_from_bottom; the exact middle
    of an odd N counts as upper) -> "Nth highest of N", lower half -> "Nth
    lowest of N"; ties prefix "tied " (e.g. "tied 6th lowest of 16");
  * median = median of the N values (mean of the two middle values for even N);
    diff_from_median = value - median.
Output columns: feature_id, value, n_with_values, rank_from_top,
rank_from_bottom, tie_count, median, diff_from_median, phrase.

Usage:
  python -I reference/rank_check.py                           # V1 population
  python -I reference/rank_check.py INPUT FIELD ID_FIELD OUT  # .geojson/.gpkg/.shp/.csv
"""

from __future__ import annotations

import argparse
import math
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import FIX, write_csv  # noqa: E402

import pandas as pd  # noqa: E402


def ordinal(n: int) -> str:
    suf = "th" if 10 <= n % 100 <= 20 else {1: "st", 2: "nd", 3: "rd"}.get(n % 10, "th")
    return f"{n}{suf}"


def phrase(rt: int, rb: int, ties: int, n: int, smaller: int) -> str:
    if n <= 2:
        return ""
    if n >= 1000:
        return f"higher than {math.floor(100 * smaller / n)}% of {n}"
    tied = "tied " if ties > 1 else ""
    if rt <= rb:
        return f"{tied}{ordinal(rt)} highest of {n}"
    return f"{tied}{ordinal(rb)} lowest of {n}"


def read_table(path: Path) -> pd.DataFrame:
    if path.suffix.lower() == ".csv":
        return pd.read_csv(path, dtype=str, keep_default_na=False)
    import geopandas as gpd
    return pd.DataFrame(gpd.read_file(path, engine="pyogrio").drop(columns="geometry"))


def rank_check(path: Path, field: str, id_field: str, out: Path):
    df = read_table(path)
    vals = pd.to_numeric(df[field].replace("", None), errors="coerce")
    have = vals.dropna()
    n = len(have)
    med = float(have.median()) if n else None
    rows = []
    for fid, v in zip(df[id_field].astype(str), vals):
        if pd.isna(v):
            rows.append([fid, "", n, "", "", "", med, "", ""])
            continue
        greater = int((have > v).sum())
        smaller = int((have < v).sum())
        ties = int((have == v).sum())
        rt, rb = greater + 1, smaller + 1
        rows.append([fid, float(v), n, rt, rb, ties, med, float(v) - med, phrase(rt, rb, ties, n, smaller)])
    rows.sort(key=lambda r: r[0])
    write_csv(out, ["feature_id", "value", "n_with_values", "rank_from_top", "rank_from_bottom", "tie_count",
                    "median", "diff_from_median", "phrase"], rows)
    return rows


def _self_test():
    assert phrase(1, 1, 1, 1, 0) == ""
    assert phrase(1, 2, 1, 2, 1) == ""
    assert phrase(2, 2, 1, 3, 1) == "2nd highest of 3"
    assert phrase(3, 1, 1, 3, 0) == "1st lowest of 3"
    assert phrase(1, 1000, 1, 1000, 999) == "higher than 99% of 1000"
    assert ordinal(11) == "11th" and ordinal(22) == "22nd" and ordinal(13) == "13th"


def main(argv=None):
    _self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("args", nargs="*")
    a = ap.parse_args(argv)
    if a.args:
        p, f, i, o = a.args
        rank_check(Path(p), f, i, Path(o))
        return
    rows = rank_check(FIX / "V1" / "districts.geojson", "population", "district_id",
                      FIX / "V1" / "expected_insight.csv")
    print(f"V1/expected_insight.csv: {len(rows)} features, n_with_values={rows[0][2]}")


if __name__ == "__main__":
    main()
