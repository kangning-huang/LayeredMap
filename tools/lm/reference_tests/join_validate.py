"""Reference for "Match a table to a layer" (J6; docs/lm/actions-p2.md › Match a table to a layer).

Definition (lm_jobs::join::expect and apply, and the action `match_table`, must match it), with
the layer on the left of a pandas merge on the key columns (`left_on`, `right_on`: they may have
different names), keys compared exactly:

  * blank keys never match and are left out of the cardinality check (pandas would match a blank
    with a blank, and count two blanks as a repeated key), so rows with a blank key are dropped
    before merging and reported as `blank_keys`;
  * cardinality: the tightest of one_to_one, one_to_many, many_to_one, many_to_many that
    `merge(validate=...)` accepts (left = layer: "many_to_one" means layer keys repeat);
  * merge_rows = len(merge(how="left")), what a plain merge gives; pairs = len(merge(how="inner"));
    layer_matched = layer records with at least one table row; table_matched = table rows with at
    least one layer record;
  * unmatched keys: table keys (as written) with no layer record, distinct, in table order;
    unmatched rows: every table row with no layer record, blank keys included (1-based);
  * values: each layer record's population. With one table row per record it is the merge's
    value; with several, the row with the latest year (LayeredMap keeps one row per record).

Usage:  python -I tools/lm/reference_tests/join_validate.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "reference"))
import _common  # noqa: E402,F401  (must precede pyproj imports)
from _common import FIX, write_csv  # noqa: E402

import geopandas as gpd  # noqa: E402
import pandas as pd  # noqa: E402
from pandas.errors import MergeError  # noqa: E402

CASES = ["one_to_one", "one_to_many", "many_to_one", "many_to_many"]


def cardinality(layer, table, lk, tk):
    for v in CASES:
        try:
            layer.merge(table, left_on=lk, right_on=tk, how="left", validate=v)
            return v
        except MergeError:
            continue
    raise AssertionError("many_to_many always validates")


def run(layer_file, table_file, lk, tk, value):
    d = FIX / "J6"
    layer = pd.DataFrame(gpd.read_file(d / layer_file, engine="pyogrio").drop(columns="geometry"))
    table_all = pd.read_csv(d / table_file, dtype={tk: str}, keep_default_na=False)
    table_all["_row"] = range(1, len(table_all) + 1)
    blank = table_all[tk].str.strip() == ""
    table = table_all[~blank]
    card = cardinality(layer, table, lk, tk)
    left = layer.merge(table, left_on=lk, right_on=tk, how="left", indicator=True)
    inner = layer.merge(table, left_on=lk, right_on=tk, how="inner")
    hit = table[tk].isin(set(layer[lk]))
    unmatched_keys = list(dict.fromkeys(table.loc[~hit, tk]))
    unmatched_rows = sorted(list(table.loc[~hit, "_row"]) + list(table_all.loc[blank, "_row"]))
    layer_matched = int(layer[lk].isin(set(table[tk])).sum())
    summary = [layer_file.removesuffix(".geojson"), table_file.removesuffix(".csv"), lk, tk, card, len(layer), len(table_all),
               len(inner), len(left), layer_matched, int(hit.sum()), "|".join(unmatched_keys),
               "|".join(str(r) for r in unmatched_rows), int(blank.sum())]
    # Values: the merge's value, or the latest year's when a record has several rows.
    if "year" in table.columns:
        latest = table.sort_values(["year", "_row"]).groupby(tk, sort=False).tail(1)
        one = layer.merge(latest, left_on=lk, right_on=tk, how="left", validate="many_to_one")
    else:
        one = layer.merge(table, left_on=lk, right_on=tk, how="left")
    assert len(one) == len(layer)
    values = [[summary[0], summary[1], i + 1, r[lk], value, None if pd.isna(r[value]) else int(r[value])]
              for i, (_, r) in enumerate(one.iterrows())]
    return summary, values


JOBS = [
    ("wards.geojson", "ward_population.csv", "ward_code", "ward_code", "population"),
    ("wards.geojson", "ward_population_by_year.csv", "ward_code", "ward_code", "population"),
    ("wards_split.geojson", "ward_population.csv", "ward_code", "ward_code", "population"),
    ("wards_split.geojson", "ward_population_by_year.csv", "ward_code", "ward_code", "population"),
    # Different names: the layer's ward_code against the table's ward.
    ("wards.geojson", "ward_people.csv", "ward_code", "ward", "people"),
]


def main():
    rows, values = [], []
    for job in JOBS:
        s, v = run(*job)
        rows.append(s)
        values.extend(v)
    d = FIX / "J6"
    write_csv(d / "expected_join.csv", ["layer", "table", "layer_key", "table_key", "cardinality", "layer_rows", "table_rows", "pairs", "merge_rows",
                                        "layer_matched", "table_matched", "unmatched_keys", "unmatched_rows", "blank_keys"], rows)
    write_csv(d / "expected_join_values.csv", ["layer", "table", "row", "key", "field", "value"], values)
    print("J6 " + ", ".join(f"{r[0]}×{r[1]}: {r[4]}, {r[8]} merge rows" for r in rows))


if __name__ == "__main__":
    main()
