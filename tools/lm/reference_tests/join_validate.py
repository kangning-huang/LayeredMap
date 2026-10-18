"""Reference for "Match a table to a layer" (J6; docs/lm/actions-p2.md › Match a table to a layer).

Definition (lm_jobs::join::expect and apply must match it), with the layer on the left of a
pandas merge on the key columns, keys compared exactly:

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

KEY = "ward_code"
CASES = ["one_to_one", "one_to_many", "many_to_one", "many_to_many"]


def cardinality(layer, table):
    for v in CASES:
        try:
            layer.merge(table, on=KEY, how="left", validate=v)
            return v
        except MergeError:
            continue
    raise AssertionError("many_to_many always validates")


def run(layer_file, table_file):
    d = FIX / "J6"
    layer = pd.DataFrame(gpd.read_file(d / layer_file, engine="pyogrio").drop(columns="geometry"))
    table_all = pd.read_csv(d / table_file, dtype={KEY: str}, keep_default_na=False)
    table_all["_row"] = range(1, len(table_all) + 1)
    blank = table_all[KEY].str.strip() == ""
    table = table_all[~blank]
    card = cardinality(layer, table)
    left = layer.merge(table, on=KEY, how="left", indicator=True)
    inner = layer.merge(table, on=KEY, how="inner")
    layer_keys = set(layer[KEY])
    hit = table[KEY].isin(layer_keys)
    unmatched_keys = list(dict.fromkeys(table.loc[~hit, KEY]))
    unmatched_rows = sorted(list(table.loc[~hit, "_row"]) + list(table_all.loc[blank, "_row"]))
    layer_matched = int(layer[KEY].isin(set(table[KEY])).sum())
    summary = [layer_file.removesuffix(".geojson"), table_file.removesuffix(".csv"), card, len(layer), len(table_all), len(inner),
               len(left), layer_matched, int(hit.sum()), "|".join(unmatched_keys), "|".join(str(r) for r in unmatched_rows),
               int(blank.sum())]
    # Values: the merge's value, or the latest year's when a record has several rows.
    if "year" in table.columns:
        latest = table.sort_values(["year", "_row"]).groupby(KEY, sort=False).tail(1)
        one = layer.merge(latest, on=KEY, how="left", validate="many_to_one")
    else:
        one = layer.merge(table, on=KEY, how="left")
    assert len(one) == len(layer)
    values = [[summary[0], summary[1], i + 1, r[KEY], None if pd.isna(r["population"]) else int(r["population"])]
              for i, (_, r) in enumerate(one.iterrows())]
    return summary, values


JOBS = [
    ("wards.geojson", "ward_population.csv"),
    ("wards.geojson", "ward_population_by_year.csv"),
    ("wards_split.geojson", "ward_population.csv"),
    ("wards_split.geojson", "ward_population_by_year.csv"),
]


def main():
    rows, values = [], []
    for job in JOBS:
        s, v = run(*job)
        rows.append(s)
        values.extend(v)
    d = FIX / "J6"
    write_csv(d / "expected_join.csv", ["layer", "table", "cardinality", "layer_rows", "table_rows", "pairs", "merge_rows",
                                        "layer_matched", "table_matched", "unmatched_keys", "unmatched_rows", "blank_keys"], rows)
    write_csv(d / "expected_join_values.csv", ["layer", "table", "row", KEY, "population"], values)
    print("J6 " + ", ".join(f"{r[0]}×{r[1]}: {r[2]}, {r[6]} merge rows" for r in rows))


if __name__ == "__main__":
    main()
