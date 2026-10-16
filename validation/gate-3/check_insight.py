#!/usr/bin/env python3
"""Gate 3, step 2: check the app's insight rows with pandas.

Prints the value, rank phrase and median line for one object, computed with exactly the rules and
formatting of the app (crates/lm-analysis/src/insight.rs; the module notes there are the rule
book). Compare every line with the object card.

Usage
-----
    python check_insight.py PATH FIELD --row N            # N = 0-based record number
    python check_insight.py PATH FIELD --id VALUE [--id-field NAME]
    python check_insight.py PATH FIELD --all              # one tab-separated line per record

Options: --unit "°C" (the field's unit, as set in the app), --noun districts (the layer's plural
noun; used from 1,000 records on and in category rows), --query "population > 500000" (a pandas
query: ranks then count only the filtered records, like a filtered layer).

PATH may be a .csv (e.g. a Shapefile's attribute table exported to CSV), a .geojson/.json
FeatureCollection, or, when geopandas is installed, any file geopandas reads (.shp, .gpkg).

A field whose values are not numbers is treated as a category field ("Residential · 41% of
districts are Residential").

Dependencies: pandas (geopandas optional). `--selftest GOLDEN.json` runs the shared golden cases.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from decimal import Decimal

import pandas as pd

MINUS = "−"
NO_VALUE = "No value"
MIN_RANKED = 3
PERCENTILE_FROM = 1000
MAX_FIELD_DECIMALS = 6
EXACT_REL = 1e-9

# ----------------------------------------------------------------------------------------------
# Number formatting (insight.rs › format_number and friends)
# ----------------------------------------------------------------------------------------------


def sig3_decimals(a: float) -> int:
    if not math.isfinite(a) or a <= 0.0:
        return 0
    if a >= 100.0:
        return 0
    if a >= 10.0:
        return 1
    if a >= 1.0:
        return 2
    for k, p in enumerate([1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10]):
        if a >= p:
            return 3 + k
    return 13


def group_thousands(s: str) -> str:
    int_part, dot, frac = s.partition(".")
    out = []
    n = len(int_part)
    for i, ch in enumerate(int_part):
        if i > 0 and (n - i) % 3 == 0:
            out.append(",")
        out.append(ch)
    return "".join(out) + (dot + frac if dot else "")


def trim_zeros(s: str) -> str:
    return s.rstrip("0").rstrip(".") if "." in s else s


def has_nonzero_digit(s: str) -> bool:
    return any(c.isdigit() and c != "0" for c in s)


def format_number(v: float, field_decimals: int | None) -> str:
    if v is None or not math.isfinite(v):
        return NO_VALUE
    a = abs(v)
    sig = sig3_decimals(a)
    if field_decimals is not None:
        d, trim = min(field_decimals, max(sig, 2)), False
    else:
        d, trim = sig, True
    s = f"{a:.{d}f}"
    if trim:
        s = trim_zeros(s)
    body = group_thousands(s)
    return MINUS + body if v < 0.0 and has_nonzero_digit(s) else body


def format_count(n: int) -> str:
    return group_thousands(str(n))


def ordinal(n: int) -> str:
    if 11 <= n % 100 <= 13:
        suffix = "th"
    else:
        suffix = {1: "st", 2: "nd", 3: "rd"}.get(n % 10, "th")
    return format_count(n) + suffix


def with_unit(number: str, unit: str | None) -> str:
    u = (unit or "").strip()
    if not u:
        return number
    if u.startswith("%") or u.startswith("‰") or u in ("°", "′", "″"):
        return number + u
    return f"{number} {u}"


def format_value(v, fd, unit):
    if v is None or not math.isfinite(v):
        return NO_VALUE
    return with_unit(format_number(v, fd), unit)


def value_decimals(v: float) -> int:
    if not math.isfinite(v) or float(v).is_integer():
        return 0
    exp = Decimal(repr(abs(float(v)))).as_tuple().exponent
    return min(max(0, -exp), MAX_FIELD_DECIMALS)


def derived_decimals(x: float, fd: int) -> int:
    hi = max(fd, 2)
    for d in range(fd, hi + 1):
        r = float(f"{x:.{d}f}")
        if abs(r - x) <= EXACT_REL * max(abs(x), 1.0):
            return d
    return hi


def format_derived(x: float, fd: int) -> str:
    return format_number(x, derived_decimals(x, fd))


# ----------------------------------------------------------------------------------------------
# Rank index (insight.rs › RankIndex)
# ----------------------------------------------------------------------------------------------


class RankIndex:
    def __init__(self, values, filtered: bool = False):
        fin = []
        missing = 0
        decimals = 0
        for v in values:
            try:
                x = None if v is None else float(v)
            except (TypeError, ValueError):
                x = None
            if x is None or not math.isfinite(x):
                missing += 1
                continue
            if x == 0.0:
                x = 0.0
            if decimals < MAX_FIELD_DECIMALS:
                decimals = max(decimals, value_decimals(x))
            fin.append(x)
        fin.sort()
        self.sorted = fin
        self.missing = missing
        self.filtered = filtered
        self.decimals = decimals

    def rank(self, x):
        import bisect

        if x is None or not math.isfinite(x) or not self.sorted:
            return None
        n = len(self.sorted)
        lower = bisect.bisect_left(self.sorted, x)
        not_higher = bisect.bisect_right(self.sorted, x)
        return dict(higher=n - not_higher, lower=lower, equal=not_higher - lower, n=n, missing=self.missing, filtered=self.filtered)

    def median(self):
        n = len(self.sorted)
        if n == 0:
            return None
        if n % 2 == 1:
            return self.sorted[n // 2]
        return (self.sorted[n // 2 - 1] + self.sorted[n // 2]) / 2.0


def denominator(r) -> str:
    s = f"of {format_count(r['n'])}"
    if r["filtered"]:
        s += " filtered"
    if r["missing"] > 0:
        s += " with values"
    return s


def denominator_noun(r, noun: str) -> str:
    s = f"of {format_count(r['n'])}"
    if r["filtered"]:
        s += " filtered"
    noun = noun.strip()
    if noun:
        s += " " + noun
    if r["missing"] > 0:
        s += " with values"
    return s


def percent_phrase(r, of: str) -> str:
    n = max(r["n"], 1)
    if r["higher"] <= r["lower"]:
        word, pct = "higher than", 100 * r["lower"] // n
    else:
        word, pct = "lower than", 100 * r["higher"] // n
    if pct == 0:
        return f"tied with {100 * r['equal'] // n}% {of}"
    return f"{word} {pct}% {of}"


def rank_phrase(r, noun: str):
    if r["n"] < MIN_RANKED:
        return None
    tied = "tied " if r["equal"] >= 2 else ""
    if r["n"] >= PERCENTILE_FROM:
        of = denominator_noun(r, noun)
        if r["higher"] == 0:
            return f"{tied}highest {of}"
        if r["lower"] == 0:
            return f"{tied}lowest {of}"
        return percent_phrase(r, of)
    if r["higher"] <= r["lower"]:
        pos, word = r["higher"] + 1, "highest"
    else:
        pos, word = r["lower"] + 1, "lowest"
    ord_ = "" if pos == 1 else ordinal(pos) + " "
    return f"{tied}{ord_}{word} {denominator(r)}"


def median_line(value, median, fd, unit):
    if value is None or median is None or not math.isfinite(value) or not math.isfinite(median):
        return None
    diff = value - median
    if abs(diff) <= EXACT_REL * max(abs(median), 1.0):
        return "at the median"
    s = format_derived(abs(diff), fd)
    if not has_nonzero_digit(s):
        s = format_number(abs(diff), None)
    sign, word = ("+", "above") if diff > 0 else (MINUS, "below")
    return f"{sign}{with_unit(s, unit)} {word} the median"


def measure_row(index: RankIndex, value, unit, noun):
    fd = index.decimals
    v = value if value is not None and math.isfinite(value) else None
    r = index.rank(v) if v is not None else None
    ranked = r is not None and r["n"] >= MIN_RANKED
    return (
        format_value(v, fd, unit),
        rank_phrase(r, noun) if r is not None else None,
        median_line(v, index.median(), fd, unit) if ranked else None,
    )


def category_row(values, category, noun, filtered=False):
    counts = {}
    n = missing = 0
    for v in values:
        if v is None or (isinstance(v, float) and math.isnan(v)) or not str(v).strip():
            missing += 1
            continue
        counts[str(v)] = counts.get(str(v), 0) + 1
        n += 1
    if category is None or (isinstance(category, float) and math.isnan(category)) or not str(category).strip():
        return NO_VALUE
    cat = str(category)
    if n < MIN_RANKED:
        return cat
    count = counts.get(cat, 0)
    pct = 100 * count // n
    share = "fewer than 1%" if pct == 0 and count > 0 else f"{pct}%"
    who = ("filtered " if filtered else "") + (noun.strip() or "objects") + (" with values" if missing > 0 else "")
    return f"{cat} · {share} of {who} are {cat}"


# ----------------------------------------------------------------------------------------------
# Reading files
# ----------------------------------------------------------------------------------------------


def read_table(path: str) -> pd.DataFrame:
    low = path.lower()
    if low.endswith(".csv") or low.endswith(".txt"):
        # Read as text, then parse numbers with Python's float() (correctly rounded, like the app).
        return pd.read_csv(path, keep_default_na=False, na_values=[""], dtype=str).pipe(_numeric_where_possible)
    if low.endswith(".geojson") or low.endswith(".json"):
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
        feats = doc.get("features", [doc] if doc.get("type") == "Feature" else [])
        return pd.DataFrame([ft.get("properties") or {} for ft in feats])
    try:
        import geopandas as gpd  # optional
    except ImportError:
        sys.exit(f"{path}: install geopandas to read this format, or export the attribute table to CSV")
    return pd.DataFrame(gpd.read_file(path).drop(columns="geometry"))


def _numeric_where_possible(df: pd.DataFrame) -> pd.DataFrame:
    """Columns whose non-blank cells all parse as numbers become floats (parsed by Python's
    float(), which is correctly rounded); others stay text."""
    out = df.copy()
    for col in out.columns:
        cells = out[col]
        nonblank = cells.dropna()
        try:
            for c in nonblank:
                float(c)
        except ValueError:
            continue
        out[col] = [None if c is None or (isinstance(c, float) and math.isnan(c)) else float(c) for c in cells]
    return out


def to_number(v):
    if v is None:
        return None
    if isinstance(v, bool):
        return 1.0 if v else 0.0
    try:
        x = float(v)
    except (TypeError, ValueError):
        return None
    return x if math.isfinite(x) else None


def is_numeric_field(series: pd.Series) -> bool:
    vals = [v for v in series.tolist() if v is not None and not (isinstance(v, float) and math.isnan(v)) and str(v).strip() != ""]
    def parses(v):
        try:
            float(v)
            return True
        except (TypeError, ValueError):
            return False

    return bool(vals) and all(parses(v) for v in vals)


# ----------------------------------------------------------------------------------------------
# Self-test against the golden cases shared with the Rust tests
# ----------------------------------------------------------------------------------------------


def golden_value(v):
    if v == "NaN":
        return float("nan")
    if v == "Infinity":
        return float("inf")
    if v == "-Infinity":
        return float("-inf")
    return v if isinstance(v, (int, float)) and not isinstance(v, bool) else None


def selftest(path: str) -> int:
    with open(path, encoding="utf-8") as f:
        g = json.load(f)
    fails = 0
    total = 0
    for c in g["numbers"]:
        got = format_number(float(c["v"]), c.get("decimals"))
        total += 1
        if got != c["expect"]:
            fails += 1
            print(f"number {c}: got {got!r}")
    for c in g["rows"]:
        idx = RankIndex([golden_value(v) for v in c["values"]], filtered=c.get("filtered", False))
        x = golden_value(c.get("value"))
        x = float(x) if x is not None else None
        got = measure_row(idx, x, c.get("unit"), c.get("noun", "objects"))
        want = (c["expect_value"], c.get("expect_rank"), c.get("expect_median"))
        total += 1
        if got != want:
            fails += 1
            print(f"row {c['name']}: got {got!r}, want {want!r}")
    for c in g["categories"]:
        got = category_row(c["values"], c.get("value"), c.get("noun", ""), c.get("filtered", False))
        total += 1
        if got != c["expect"]:
            fails += 1
            print(f"category {c['name']}: got {got!r}, want {c['expect']!r}")
    print(f"{total - fails}/{total} golden cases pass")
    return 1 if fails else 0


# ----------------------------------------------------------------------------------------------


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("path", nargs="?")
    ap.add_argument("field", nargs="?")
    who = ap.add_mutually_exclusive_group()
    who.add_argument("--row", type=int, help="0-based record number")
    who.add_argument("--id", help="value of the identifier field")
    who.add_argument("--all", action="store_true", help="every record, tab-separated: value, rank, median line")
    ap.add_argument("--id-field", help="identifier field for --id (default: id, ID, fid, name, NAME, first column)")
    ap.add_argument("--unit", default=None)
    ap.add_argument("--noun", default="objects", help="plural noun for the layer's objects, e.g. districts")
    ap.add_argument("--query", default=None, help="pandas query; ranks count only matching records")
    ap.add_argument("--selftest", metavar="GOLDEN_JSON")
    a = ap.parse_args(argv)

    if a.selftest:
        return selftest(a.selftest)
    if not a.path or not a.field:
        ap.error("PATH and FIELD are required")

    df = read_table(a.path)
    if a.field not in df.columns:
        sys.exit(f"no field {a.field!r}; fields: {', '.join(map(str, df.columns))}")
    filtered = a.query is not None
    peers = df.query(a.query) if filtered else df

    if a.all:
        index = RankIndex([to_number(v) for v in peers[a.field].tolist()], filtered)
        for v in df[a.field].tolist():
            value, rank, med = measure_row(index, to_number(v), a.unit, a.noun)
            print(f"{value}\t{rank or ''}\t{med or ''}")
        return 0

    if a.row is not None:
        if not 0 <= a.row < len(df):
            sys.exit(f"--row must be 0..{len(df) - 1}")
        rec = df.iloc[a.row]
    elif a.id is not None:
        cands = [a.id_field] if a.id_field else [c for c in ("id", "ID", "fid", "FID", "name", "NAME") if c in df.columns] or [df.columns[0]]
        col = cands[0]
        hits = df[df[col].astype(str) == str(a.id)]
        if hits.empty:
            try:
                hits = df[pd.to_numeric(df[col], errors="coerce") == float(a.id)]
            except ValueError:
                pass
        if hits.empty:
            sys.exit(f"no record with {col} = {a.id!r}")
        rec = hits.iloc[0]
    else:
        ap.error("choose --row, --id or --all")

    raw = rec[a.field]
    print(f"Field: {a.field}")
    if is_numeric_field(peers[a.field]):
        index = RankIndex([to_number(v) for v in peers[a.field].tolist()], filtered)
        value, rank, med = measure_row(index, to_number(raw), a.unit, a.noun)
        print(f"Value: {value}")
        print(f"Rank: {rank or '(none: two or fewer values, or no value)'}")
        m = index.median()
        if m is not None:
            print(f"Median: {with_unit(format_derived(m, index.decimals), a.unit)}")
        print(f"Compared with the median: {med or '(none)'}")
        print(f"(n = {len(index.sorted)}, missing = {index.missing}, field decimals = {index.decimals}{', filtered' if filtered else ''})")
    else:
        print(category_row(peers[a.field].tolist(), raw, a.noun, filtered))
    return 0


if __name__ == "__main__":
    sys.exit(main())
