"""Check LayeredMap's joins (WP3.11) against pandas.merge on the same normalized keys.

    python validation/gate-3/check_join.py

For each fixture J1-J5 it normalizes both key columns as the join's level does and counts how
many layer records get data with pandas. The Rust test `gate3_join_fixtures` asserts the same
numbers from LayeredMap's own join; the two must agree.
"""

import csv
import json
import re
import unicodedata
from pathlib import Path

import pandas as pd

ROOT = Path(__file__).resolve().parents[2]
CROSS = ROOT / "crates/lm-jobs/src/join"


def fold(s):
    s = unicodedata.normalize("NFKD", str(s).lower())
    s = "".join(c for c in s if not unicodedata.combining(c))
    return s.replace("ß", "ss").replace("æ", "ae").replace("œ", "oe").replace("’", "'")


def stripped(s):
    return "".join(c for c in fold(s) if c.isalnum())


def code(s):
    t = str(s).strip()
    t = t[:-2] if t.endswith(".0") else t
    return (t.lstrip("0") or "0") if t.isdigit() else None


def crosswalk(name):
    index, clash = {}, set()
    with open(CROSS / name, encoding="utf-8") as f:
        next(f)
        for line in f:
            key, _, names = line.rstrip("\n").partition(",")
            for n in [key] + names.split("|"):
                for k in filter(None, [stripped(n), code(n)]):
                    if index.get(k, key) != key:
                        clash.add(k)
                    index.setdefault(k, key)
    for k in clash:
        index.pop(k, None)
    return lambda s: index.get(stripped(s)) or index.get(code(s) or "")


LEVELS = {
    "exact": lambda s: str(s).strip(),
    "code": code,
    "countries": crosswalk("countries.csv"),
    "cn": crosswalk("cn_provinces.csv"),
}


def layer(path):
    fc = json.loads(Path(path).read_text(encoding="utf-8"))
    return pd.DataFrame([f["properties"] for f in fc["features"]])


def matched(table, tkey, lay, lkey, level):
    norm = LEVELS[level]
    t = pd.DataFrame({"k": table[tkey].map(lambda v: norm(v) if pd.notna(v) else None)}).dropna().drop_duplicates()
    l = pd.DataFrame({"k": lay[lkey].map(lambda v: norm(v) if pd.notna(v) else None)})
    return int(l.merge(t, on="k", how="inner")["k"].notna().sum())


CASES = [
    ("J1 ISO3", "fixtures/J1/gdp_growth_iso3.csv", "Country Code", "assets/examples/countries.geojson", "iso_a3", "exact", None),
    ("J1 World Bank names", "fixtures/J1/population_wb_names.csv", "Country Name", "assets/examples/countries.geojson", "iso_a3", "countries", 20),
    ("J2 FIPS without zeros", "fixtures/J2/cases.csv", "fips", "fixtures/J2/counties.geojson", "GEOID", "code", 6),
    ("J3 districts by year", "fixtures/J3/population_by_year.csv", "district", "fixtures/V1/districts.geojson", "name_en", "exact", 17),
    ("J5 Chinese names", "fixtures/J5/population_zh.csv", "province", "fixtures/J5/provinces.geojson", "name", "cn", 7),
]


def main():
    ok = True
    for label, tpath, tkey, lpath, lkey, level, expected in CASES:
        table = pd.read_csv(ROOT / tpath, dtype=str, keep_default_na=False).replace("", pd.NA)
        lay = layer(ROOT / lpath)
        n = matched(table, tkey, lay, lkey, level)
        exp = len(lay[lay[lkey].notna()]) if expected is None else expected
        flag = "ok" if n == exp else "MISMATCH"
        ok &= n == exp
        print(f"{label:28} {level:9} pandas: {n:4}  LayeredMap: {exp:4}  {flag}")
    print("all agree" if ok else "DISAGREEMENT")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
