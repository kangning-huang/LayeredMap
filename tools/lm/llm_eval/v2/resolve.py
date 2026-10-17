"""``resolve_feature(layer, text)``, simulated locally from the fixture data.

Owner decision (2026-10-17): the model may call ``resolve_feature``; it returns up to ~5 stored
names or category labels matching the user's words. No other feature values are sent.

Matching, best first (score in brackets):

1. exact stored value (1.0); the same ignoring case, spacing and punctuation (0.97);
2. the alias table below, standing in for a Wikidata/GeoNames zh/en label lookup (0.95);
3. the same base name once generic words are dropped and abbreviations expanded on both sides
   ("Huaihai Road" = "Huaihai Rd", "Pudong" = "Pudong New Area", "浦东" = "浦东新区") (0.9);
4. pinyin of a Chinese query against an English stored name (pypinyin, without tones):
   "南京路" -> "nanjing" = "Nanjing Rd" (0.88);
5. an inflected form, the stored value plus up to 3 letters ("African" -> "Africa") (0.86);
6. prefix / substring (0.75), then a fuzzy ratio (difflib, >= 0.8, scaled to <= 0.8).

``name_zh`` fields in the data are searched like any other text field, so 东京 finds Tokyo
through its stored ``name_zh``. Each candidate says which field holds the value and how many
objects carry it (2 for "Portland"), so ambiguity is visible and should lead to ``ask_user``.
"""

from __future__ import annotations

import difflib
import re
import sys
import unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import data  # noqa: E402

try:
    from pypinyin import lazy_pinyin
except ImportError:  # optional: pinyin matching is skipped without it
    lazy_pinyin = None

# ---------------------------------------------------------------------------------------------
# Alias table (hand-made for the benchmark; in the product this comes from Wikidata zh/en labels,
# GeoNames alternate names and a per-field category glossary). layer -> field -> alias -> stored.

ALIASES: dict[str, dict[str, dict[str, list[str]]]] = {
    "Roads": {
        "name": {
            "延安路": ["Yan'an Rd"], "延安高架": ["Yan'an Rd"], "延安高架路": ["Yan'an Rd"],
            "世纪大道": ["Century Ave"],
            "南京路": ["Nanjing Rd"],
            "淮海路": ["Huaihai Rd"],
            "中山路": ["Zhongshan Rd"],
            "内环": ["Inner Ring"], "内环路": ["Inner Ring"], "内环高架": ["Inner Ring"], "内环高架路": ["Inner Ring"],
        }
    },
    "Schools": {
        "type": {
            "高中": ["High"], "高级中学": ["High"], "high school": ["High"], "senior high": ["High"],
            "初中": ["Middle"], "初级中学": ["Middle"], "middle school": ["Middle"], "junior high": ["Middle"],
            "中学": ["Middle", "High"], "secondary": ["Middle", "High"], "secondary school": ["Middle", "High"],
            "小学": ["Primary"], "primary school": ["Primary"], "elementary": ["Primary"], "elementary school": ["Primary"],
        }
    },
    "Parks": {
        "kind": {
            "城市公园": ["city"], "市级公园": ["city"], "city park": ["city"],
            "区级公园": ["district"], "district park": ["district"],
            "口袋公园": ["pocket"], "袖珍公园": ["pocket"], "pocket park": ["pocket"],
        }
    },
    "Countries": {
        "continent": {
            "亚洲": ["Asia"], "非洲": ["Africa"], "欧洲": ["Europe"], "北美洲": ["North America"], "北美": ["North America"],
            "南美洲": ["South America"], "南美": ["South America"], "大洋洲": ["Oceania"], "南极洲": ["Antarctica"],
        }
    },
    "Populated places": {
        "capital": {"首都": ["true"], "是": ["true"], "否": ["false"], "yes": ["true"], "no": ["false"], "capital": ["true"]},
    },
}

# Generic words dropped for the base-name key, and abbreviations expanded first.
ABBREV = {"rd": "road", "ave": "avenue", "av": "avenue", "st": "street", "blvd": "boulevard", "hwy": "highway"}
GENERIC_EN = {"road", "avenue", "street", "boulevard", "highway", "district", "new", "area", "the", "of", "park", "school"}
GENERIC_ZH = ["新区", "大道", "高架路", "高架", "路", "区", "街", "市", "公园"]

SEARCH_TYPES = ("text", "category", "yes/no")


def _nfkc(s: str) -> str:
    return unicodedata.normalize("NFKC", str(s))


def norm(s) -> str:
    """Case, spacing and punctuation folded: "Yan’an Rd." -> "yanan rd"."""
    s = _nfkc(s).casefold()
    s = re.sub(r"[\'’‘`´\.\-_,·]", "", s)
    return re.sub(r"\s+", " ", s).strip()


def has_cjk(s: str) -> bool:
    return any("一" <= ch <= "鿿" for ch in str(s))


def base_key(s) -> str:
    """The name without generic words, abbreviations expanded, spaces removed."""
    t = norm(s)
    if has_cjk(t):
        for g in GENERIC_ZH:
            if t.endswith(g) and len(t) > len(g):
                t = t[: -len(g)]
                break
        return t.replace(" ", "")
    words = [ABBREV.get(w, w) for w in t.split()]
    kept = [w for w in words if w not in GENERIC_EN] or words
    return "".join(kept)


def pinyin_key(s) -> str | None:
    if lazy_pinyin is None or not has_cjk(s):
        return None
    b = base_key(s)
    return "".join(lazy_pinyin(b)).casefold() if b else None


def searchable_fields(layer: str) -> list[str]:
    return [f for f, t in data.fields(layer).items() if t.split(",")[0].strip() in SEARCH_TYPES]


def _score(query: str, stored: str, aliases: dict[str, list[str]]) -> float:
    if query == stored:
        return 1.0
    nq, ns = norm(query), norm(stored)
    if not nq or not ns:
        return 0.0
    if nq == ns:
        return 0.97
    if stored in aliases.get(nq, []) or stored in aliases.get(_nfkc(query).strip(), []):
        return 0.95
    bq, bs = base_key(query), base_key(stored)
    if bq and bq == bs:
        return 0.9
    pq = pinyin_key(query)
    if pq and not has_cjk(stored) and pq == bs:
        return 0.88
    if len(ns) >= 4 and nq.startswith(ns) and len(nq) - len(ns) <= 3 and " " not in nq[len(ns):]:
        return 0.86  # an inflected form: "African" -> "Africa", "Asian" -> "Asia"
    if len(nq) >= 2 and (ns.startswith(nq) or (len(nq) >= 3 and nq in ns)):
        return 0.75
    if has_cjk(nq) != has_cjk(ns):
        return 0.0
    r = difflib.SequenceMatcher(None, nq, ns).ratio()
    return 0.8 * r if r >= 0.8 else 0.0


def match_field(layer: str, field: str, text) -> list[dict]:
    """Stored values of one field matching ``text``: [{value, score, objects}], best first."""
    aliases = {norm(k): v for k, v in ALIASES.get(layer, {}).get(field, {}).items()}
    counts: dict[str, int] = {}
    for r in data.rows(layer):
        v = r.get(field)
        if v not in (None, ""):
            counts[v] = counts.get(v, 0) + 1
    for c in data.categories(layer, field) or []:
        counts.setdefault(c, 0)
    out = []
    for v, n in counts.items():
        s = _score(str(text), v, aliases)
        if s > 0:
            out.append({"value": v, "score": round(s, 3), "objects": n})
    out.sort(key=lambda c: (-c["score"], c["value"]))
    return out


def resolve_feature(layer: str, text, field: str | None = None, limit: int = 5) -> dict:
    """The tool: {layer, text, candidates: [{field, value, objects}]} or {error}."""
    if data.kind(layer) is None:
        return {"error": f"There is no layer named {layer!r}."}
    if data.kind(layer) == "raster":
        return {"error": f"{layer} is a raster; it has no names or categories to look up."}
    fields = [field] if field and field in data.fields(layer) else searchable_fields(layer)
    cands = []
    for f in fields:
        for c in match_field(layer, f, text):
            cands.append({"field": f, **c})
    if not cands:
        return {"layer": layer, "text": str(text), "candidates": []}
    top = max(c["score"] for c in cands)
    keep = [c for c in cands if c["score"] >= max(0.6, top - 0.15)]
    keep.sort(key=lambda c: (-c["score"], c["field"], c["value"]))
    # The same object named by several fields (Tokyo / 东京): keep the first field that names it.
    seen_rows: set[int] = set()
    out = []
    rows = data.rows(layer)
    for c in keep:
        hit = {i for i, r in enumerate(rows) if r.get(c["field"]) == c["value"]}
        if hit and hit <= seen_rows and data.categories(layer, c["field"]) is None:
            continue
        seen_rows |= hit
        out.append({"field": c["field"], "value": c["value"], "objects": c["objects"], "score": c["score"]})
        if len(out) >= limit:
            break
    return {"layer": layer, "text": str(text), "candidates": out}


def tool_result_text(res: dict) -> str:
    """What the model sees: candidates only (no scores, no other values)."""
    if "error" in res:
        return res["error"]
    c = res["candidates"]
    if not c:
        return (f"No stored name or category in {res['layer']} matches {res['text']!r}. If the user named "
                "something that isn't in this layer, ask_user or cannot_do.")
    parts = []
    is_cat = [data.categories(res["layer"], x["field"]) is not None for x in c]
    for x, cat in zip(c, is_cat):
        n = x["objects"]
        many = " (a category)" if cat else ("" if n <= 1 else f" ({n} objects share this name)")
        parts.append(f"{{field: {x['field']}, value: {x['value']!r}}}{many}")
    lead = "Match" if len(c) == 1 else f"{len(c)} candidates"
    tail = ""
    if any(x["objects"] > 1 and not cat for x, cat in zip(c, is_cat)) or (len(c) > 1 and not all(is_cat)):
        tail = " More than one object could be meant: ask_user which one, unless the request makes it clear."
    elif len(c) > 1:
        tail = " Use the value(s) the user meant (any_of for several); if unclear, ask_user."
    return f"{lead} in {res['layer']} for {res['text']!r}: " + "; ".join(parts) + "." + tail

if __name__ == "__main__":
    for layer, q in [("Roads", "世纪大道"), ("Roads", "南京路"), ("Roads", "Huaihai Road"), ("Roads", "内环"),
                     ("Schools", "高中"), ("Schools", "中学"), ("Populated places", "Portland"), ("Populated places", "东京"),
                     ("Districts", "浦东"), ("Districts", "Pudong"), ("Countries", "亚洲"), ("Parks", "城市公园"),
                     ("Populated places", "Timbuctu"), ("Countries", "france")]:
        print(layer, q, "->", tool_result_text(resolve_feature(layer, q)))
