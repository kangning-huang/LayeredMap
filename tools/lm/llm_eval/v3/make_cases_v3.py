"""Writes ``cases_v3.jsonl``: the end-to-end spatial benchmark of harness v3.

Every case is a request on a map of files a user would open (the launch-video files and four
datasets from the website's Data page), with NimbleMap Pro's built-in world layers available.
Each case carries:
- ``gold``: a plan in the app's ``submit_plan`` format that answers it (the mock run feeds it to
  the app, which checks that the app can answer the case at all and that its answer matches the
  ground truth);
- ``expect``: what a right answer is: the kind (plan / ask_user / cannot_do), the answer spec
  (count range, set of objects, value with tolerance, top-k, names) and the preview spec (the
  objects the map should highlight), all computed here with GeoPandas/Shapely (``gt.py``),
  independently of the app.

Run: ``python -I make_cases_v3.py`` (env lm-llm-eval with PROJ_DATA set; see README). Cases are
written in a fixed order; the ground truth is recomputed each time.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import numpy as np  # noqa: E402

import data_v3 as D  # noqa: E402
import gt  # noqa: E402

L = D.LAYER
B = D.BUILTIN
OUT = HERE / "cases_v3.jsonl"
CASES: list[dict] = []


# ---------------------------------------------------------------------------------------------
# Plan steps (submit_plan format)


def cond(field, op, value=None, value2=None, values=None):
    c = {"field": field, "op": op}
    if value is not None:
        c["value"] = str(value)
    if value2 is not None:
        c["value2"] = str(value2)
    if values is not None:
        c["values"] = list(values)
    return c


def filt(layer, *conds):
    return {"action": "filter", "layer": layer, "conditions": list(conds)}


def obj(layer, field=None, value=None):
    o = {"layer": layer}
    if field is not None:
        o["field"], o["value"] = field, str(value)
    return o


def step(action, **kw):
    return {"action": action, **kw}


def ask(q):
    return [step("ask_user", question=q)]


def refuse(r):
    return [step("cannot_do", reason=r)]


# ---------------------------------------------------------------------------------------------
# Specs


def keys(src, rows):
    return D.keys_of(gt.load(src), rows)


def count_spec(lo, hi=None):
    return {"type": "count", "lo": lo, "hi": lo if hi is None else hi}


def set_spec(src, strict, edge=()):
    return {"type": "set", "strict": keys(src, strict), "edge": keys(src, edge)}


def add(cid, map_name, text, lang, qtype, diff, gold, answer=None, preview=None, kind="plan", accept=(), notes="", tags=(), area=False):
    assert not any(c["id"] == cid for c in CASES), cid
    CASES.append({
        "id": cid,
        "map": map_name,
        "text": text,
        "lang": lang,
        "qtype": qtype,
        "difficulty": diff,
        "gold": gold,
        "expect": {"kind": kind, "accept": list(accept), "answer": answer, "preview": preview, "area": area},
        "notes": notes,
        "tags": list(tags),
    })


# Masks (ground truth) next to the filter conditions (gold plan) they stand for.
def m_strength(*names):
    return lambda g: g["strength"].isin(names).to_numpy()


def place(name, country=None):
    _, x, y = gt.city(name, country)
    return x, y


def city_plan(name, country=None):
    """Steps that make one World city the object to buffer: by name, or filtered first when the
    name repeats."""
    if country is None:
        return [], obj(B["cities"], "name", name)
    return [filt(B["cities"], cond("name", "is", name), cond("country", "is", country))], obj(B["cities"])


def within_case(cid, map_name, src, text, lang, diff, name, km, country=None, mask=None, conds=(), how="count", accept=(), notes="", tags=()):
    """"How many / which <src> within km of <city>": buffer the city, then count_here / whats_inside."""
    g = gt.load(src)
    x, y = place(name, country)
    w = gt.within(x, y, g, km, None if mask is None else mask(g))
    pre, o = city_plan(name, country)
    steps = list(pre)
    if conds:
        steps.append(filt(L[src], *conds))
    steps.append(step("buffer", layer=B["cities"], object=o, distance=f"{km} km"))
    k = len(steps)
    if how == "count":
        steps.append(step("count_here", points=L[src], area=obj(f"#{k}")))
        answer = count_spec(len(w["strict"]), len(w["strict"]) + len(w["edge"]))
    else:
        steps.append(step("whats_inside", area=obj(f"#{k}"), layer=L[src]))
        answer = set_spec(src, w["strict"], w["edge"])
    preview = set_spec(src, w["strict"], w["edge"])
    n = f"{len(w['strict'])}" + (f"–{len(w['strict']) + len(w['edge'])}" if w["edge"] else "")
    add(cid, map_name, text, lang, "within", diff, steps, answer, preview, accept=accept, area=True,
        notes=(notes + f" GT {n} within {km} km of {name}.").strip(), tags=tags)


def per_area_counts(src, areas, mask=None):
    g = gt.load(src)
    a = gt.load(areas)
    m = gt.in_areas(g, a, None if mask is None else mask(g))
    return {D.key_of(a, i): len(v) for i, v in m.items()}, m


def top_keys(counts: dict, k=1):
    vals = sorted(counts.items(), key=lambda kv: -kv[1])
    if not vals:
        return []
    cut = vals[min(k, len(vals)) - 1][1]
    return [kk for kk, v in vals if v >= cut]


# ---------------------------------------------------------------------------------------------
# A. Within N km of a named place


S5 = cond("strength", "is", "Category 5")
S45 = cond("strength", "any_of", values=["Category 4", "Category 5"])
S345 = cond("strength", "any_of", values=["Category 3", "Category 4", "Category 5"])

within_case("w01", "storms", "storms", "How many Category 5 storms have passed within 100 km of Tokyo?", "en", "medium", "Tokyo", 100, mask=m_strength("Category 5"), conds=[S5],
            notes="The owner's bug report.", tags=["owner"])
within_case("w02", "storms", "storms", "How many Category 5 storms have passed within 100 km of New York?", "en", "hard", "New York", 100, mask=m_strength("Category 5"), conds=[S5], tags=["zero"])
within_case("w03", "storms", "storms", "马尼拉100公里范围内经过了多少条5级台风？", "zh", "medium", "Manila", 100, mask=m_strength("Category 5"), conds=[S5])
within_case("w04", "storms", "storms", "列出距离香港50公里以内的4级和5级台风", "zh", "hard", "Hong Kong", 50, mask=m_strength("Category 4", "Category 5"), conds=[S45], how="set")
within_case("w05", "storms", "storms", "上海50公里内经过了多少条风暴路径？", "zh", "easy", "Shanghai", 50)
within_case("w06", "storms", "storms", "Which Category 5 hurricanes came within 200 km of Miami?", "en", "medium", "Miami", 200, mask=m_strength("Category 5"), conds=[S5], how="set")
within_case("w07", "storms", "storms", "How many storms of Category 3 or stronger passed within 150 km of Taipei?", "en", "hard", "Taipei", 150, mask=m_strength("Category 3", "Category 4", "Category 5"), conds=[S345])
within_case("w08", "launch", "storms", "How many Category 5 storms have passed within 100 km of Tokyo?", "en", "medium", "Tokyo", 100, mask=m_strength("Category 5"), conds=[S5],
            notes="w01 on the full launch map (five layers).", tags=["owner"])
within_case("w09", "storms", "storms", "2015年以来，有多少条风暴路径经过香港100公里范围内？", "zh", "hard", "Hong Kong", 100, mask=lambda g: (g["season"] >= 2015).to_numpy(),
            conds=[cond("season", "at_least", 2015)])
within_case("w10", "storms", "storms", "How many Category 5 typhoons in the 1990s passed within 300 km of Manila?", "en", "hard", "Manila", 300,
            mask=lambda g: ((g["strength"] == "Category 5") & g["season"].between(1990, 1999)).to_numpy(), conds=[S5, cond("season", "between", 1990, 1999)])
within_case("w11", "quakes", "quakes", "How many earthquakes of magnitude 7 or more happened within 200 km of Tokyo?", "en", "medium", "Tokyo", 200,
            mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)])
within_case("w12", "quakes", "quakes", "How many magnitude 8+ earthquakes struck within 100 km of Paris?", "en", "hard", "Paris", 100,
            mask=lambda g: (g["magnitude"] >= 8).to_numpy(), conds=[cond("magnitude", "at_least", 8)], tags=["zero"])
within_case("w13", "quakes", "quakes", "¿Cuántos terremotos de magnitud 7 o más hubo a menos de 300 km de Santiago de Chile?", "es", "hard", "Santiago", 300, country="Chile",
            mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)], accept=["ask_user"],
            notes="Four Santiagos in World cities; asking which is accepted (the request names Chile).")
within_case("w14", "quakes", "quakes", "¿Cuántos sismos de magnitud 6.5 o mayor ocurrieron a menos de 100 km de la Ciudad de México?", "es", "medium", "Mexico City", 100,
            mask=lambda g: (g["magnitude"] >= 6.5).to_numpy(), conds=[cond("magnitude", "at_least", 6.5)])
within_case("w15", "quakes", "quakes", "台北50公里内发生过哪些地震？", "zh", "medium", "Taipei", 50, how="set")
within_case("w16", "quakes", "quakes", "How many quakes of magnitude 7.5 or more occurred within 500 km of Jakarta?", "en", "medium", "Jakarta", 500,
            mask=lambda g: (g["magnitude"] >= 7.5).to_numpy(), conds=[cond("magnitude", "at_least", 7.5)])
within_case("w17", "volcanoes", "volcanoes", "Quali vulcani si trovano entro 100 km da Napoli?", "it", "hard", "Naples", 100, country="Italy", how="set",
            accept=["ask_user"], notes="Two Naples (Italy, USA); asking is accepted, the Italian one is the plain reading.")
within_case("w18", "volcanoes", "volcanoes", "Which volcanoes are within 200 km of Seattle?", "en", "easy", "Seattle", 200, how="set")
within_case("w19", "volcanoes", "volcanoes", "¿Cuántos volcanes hay a menos de 100 km de la Ciudad de México?", "es", "medium", "Mexico City", 100)
within_case("w20", "volcanoes", "volcanoes", "东京100公里范围内有几座火山？", "zh", "easy", "Tokyo", 100)
within_case("w21", "volcanoes", "volcanoes", "How many volcanoes lie within 100 km of Berlin?", "en", "hard", "Berlin", 100, tags=["zero"])
within_case("w22", "nuclear", "nuclear", "Combien de centrales nucléaires se trouvent à moins de 100 km de Paris ?", "fr", "hard", "Paris", 100, tags=["boundary"])
within_case("w23", "nuclear", "nuclear", "上海周围150公里内有哪些核电站？", "zh", "medium", "Shanghai", 150, how="set")
within_case("w24", "nuclear", "nuclear", "東京から200キロ以内に稼働中の原子力発電所はいくつありますか？", "ja", "hard", "Tokyo", 200,
            mask=lambda g: (g["Status"] == "Operational").to_numpy(), conds=[cond("Status", "is", "Operational")])
within_case("w25", "nuclear", "nuclear", "How many nuclear power plants are within 50 km of Jakarta?", "en", "medium", "Jakarta", 50, tags=["zero"])
within_case("w26", "airports", "airports", "How many large airports are within 100 km of New York?", "en", "medium", "New York", 100,
            mask=lambda g: (g["type"] == "large_airport").to_numpy(), conds=[cond("type", "is", "large_airport")])
within_case("w27", "airports", "airports", "Which large airports are within 60 km of London?", "en", "hard", "London", 60, country="United Kingdom", how="set",
            mask=lambda g: (g["type"] == "large_airport").to_numpy(), conds=[cond("type", "is", "large_airport")], accept=["ask_user"],
            notes="Three Londons; asking is accepted, the capital is the plain reading.")
within_case("w28", "airports", "airports", "Wie viele Flughäfen mit Linienverkehr liegen im Umkreis von 30 km um Berlin?", "de", "medium", "Berlin", 30,
            mask=lambda g: (g["scheduled_service"] == "yes").to_numpy(), conds=[cond("scheduled_service", "is", "yes")])
within_case("w29", "tornadoes", "tornadoes", "How many tornadoes rated 4 or higher touched down within 50 km of Oklahoma City?", "en", "medium", "Oklahoma City", 50,
            mask=lambda g: (g["mag"] >= 4).to_numpy(), conds=[cond("mag", "at_least", 4)])
within_case("w30", "tornadoes", "tornadoes", "How many F5/EF5 tornadoes were within 100 km of Seattle?", "en", "hard", "Seattle", 100,
            mask=lambda g: (g["mag"] == 5).to_numpy(), conds=[cond("mag", "is", 5)], tags=["zero"])
within_case("w31", "tornadoes", "tornadoes", "Which tornadoes since 2020 passed within 15 km of Oklahoma City?", "en", "medium", "Oklahoma City", 15,
            mask=lambda g: (g["yr"] >= 2020).to_numpy(), conds=[cond("yr", "at_least", 2020)], how="set")
within_case("w32", "usgs", "usgs", "How many earthquakes in this file were within 100 km of Los Angeles?", "en", "hard", "Los Angeles", 100, country="United States of America",
            accept=["ask_user"], notes="Two Los Angeles (USA, Chile); asking is accepted.")
within_case("w33", "usgs", "usgs", "東京から200km以内でマグニチュード4.5以上の地震はいくつありましたか？", "ja", "medium", "Tokyo", 200,
            mask=lambda g: (g["mag"] >= 4.5).to_numpy(), conds=[cond("mag", "at_least", 4.5)])
within_case("w34", "usgs", "usgs", "Show the earthquakes within 50 km of Anchorage.", "en", "easy", "Anchorage", 50, how="set")
within_case("w35", "launch", "volcanoes", "東京100公里以内有哪些火山？", "zh", "medium", "Tokyo", 100, how="set", notes="On the full launch map.")
within_case("w36", "storms", "storms", "서울 100km 이내를 지나간 태풍은 몇 개입니까?", "ko", "medium", "Seoul", 100)
within_case("w37", "nuclear", "nuclear", "Quantas centrais nucleares estão a menos de 300 km de Seul?", "pt", "medium", "Seoul", 300)
within_case("w38", "volcanoes", "volcanoes", "Hoeveel vulkanen liggen binnen 150 km van Quito?", "nl", "medium", "Quito", 150)
within_case("w39", "quakes", "quakes", "Сколько землетрясений магнитудой 7 и выше произошло в пределах 200 км от Сендая?", "ru", "medium", "Sendai", 200,
            mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)])


# ---------------------------------------------------------------------------------------------
# B. Counts per country or region


def per_country_top(cid, map_name, src, text, lang, diff, mask=None, conds=(), areas="countries", k=1, notes="", accept_alt=None):
    counts, _ = per_area_counts(src, areas, mask)
    top = top_keys(counts, k)
    steps = [filt(L[src], *conds)] if conds else []
    steps.append(step("count_in", points=L[src], areas=B[areas]))
    n = len(steps)
    steps.append(step("rank_by_value", layer=f"#{n}", field=D.COUNT_FIELD[src]))
    answer = {"type": "top", "k": k, "keys": top, "values": {kk: counts[kk] for kk in top}}
    if accept_alt:
        answer["alt"] = accept_alt
    add(cid, map_name, text, lang, "per_area", diff, steps, answer, {"type": "layer", "role": "count"},
        notes=(notes + f" GT top: {', '.join(f'{kk} ({counts[kk]})' for kk in top)}.").strip())


def in_one_area(cid, map_name, src, text, lang, diff, areas, field, value, mask=None, conds=(), notes="", accept_alt=None, how="count"):
    g = gt.load(src)
    a = gt.load(areas)
    rows_a = [i for i in range(len(a)) if a[field].iloc[i] == value]
    assert len(rows_a) == 1, (value, rows_a)
    m = gt.in_areas(g, a.iloc[rows_a], None if mask is None else mask(g))
    inside = sorted(set(sum(m.values(), [])))
    steps = [filt(L[src], *conds)] if conds else []
    if how == "count":
        steps.append(step("count_here", points=L[src], area=obj(B[areas], field, value)))
        answer = count_spec(len(inside))
    else:
        steps.append(step("whats_inside", area=obj(B[areas], field, value), layer=L[src]))
        answer = set_spec(src, inside)
    if accept_alt:
        answer["alt"] = accept_alt
    add(cid, map_name, text, lang, "inside", diff, steps, answer, set_spec(src, inside), notes=(notes + f" GT {len(inside)} in {value}.").strip())


per_country_top("c01", "quakes", "quakes", "Which country has had the most earthquakes of magnitude 7 or more?", "en", "medium",
                mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)])
per_country_top("c02", "quakes", "quakes", "哪个国家境内发生的7级以上地震最多？", "zh", "medium",
                mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "above", 7)], notes="'7级以上' read as above 7 or at least 7; GT uses at least.")
per_country_top("c03", "volcanoes", "volcanoes", "Which country has the most volcanoes inside its borders?", "en", "easy")
per_country_top("c04", "nuclear", "nuclear", "Which country has the most operational nuclear power plants?", "en", "medium",
                mask=lambda g: (g["Status"] == "Operational").to_numpy(), conds=[cond("Status", "is", "Operational")])
per_country_top("c05", "airports", "airports", "Which US state has the most large airports?", "en", "hard", areas="states_provinces",
                mask=lambda g: (g["type"] == "large_airport").to_numpy(), conds=[cond("type", "is", "large_airport")],
                notes="Every state/province in the world is counted; the top is the answer either way.")
per_country_top("c06", "storms", "storms", "Which country have the most Category 5 storm tracks crossed?", "en", "hard",
                mask=m_strength("Category 5"), conds=[S5])
per_country_top("c07", "storms", "storms", "哪个国家被5级台风路径穿过的次数最多？", "zh", "hard", mask=m_strength("Category 5"), conds=[S5])
per_country_top("c08", "usgs", "usgs", "Which country had the most earthquakes in this file?", "en", "easy")
per_country_top("c09", "tornadoes", "tornadoes", "Which state had the most tornadoes rated 4 or 5?", "en", "medium", areas="states_provinces",
                mask=lambda g: (g["mag"] >= 4).to_numpy(), conds=[cond("mag", "at_least", 4)], notes="The st field also answers it; any plan giving the top state is right.")
per_country_top("c10", "volcanoes", "volcanoes", "火山最多的国家是哪个？", "zh", "easy")
per_country_top("c11", "quakes", "quakes", "Top 3 countries by number of magnitude 8+ earthquakes?", "en", "hard", k=3,
                mask=lambda g: (g["magnitude"] >= 8).to_numpy(), conds=[cond("magnitude", "at_least", 8)])
per_country_top("c12", "nuclear", "nuclear", "どの国に原子力発電所が一番多いですか？", "ja", "easy")
in_one_area("i01", "quakes", "quakes", "How many earthquakes of magnitude 7 or more happened inside Japan?", "en", "medium", "countries", "name", "Japan",
            mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)], notes="Inside the land borders (most Japanese quakes are offshore).")
in_one_area("i02", "volcanoes", "volcanoes", "中国境内有多少座火山？", "zh", "easy", "countries", "name", "China")
in_one_area("i03", "nuclear", "nuclear", "Combien de centrales nucléaires y a-t-il en France ?", "fr", "easy", "countries", "name", "France",
            accept_alt=[count_spec(int((gt.load("nuclear")["Country"] == "France").sum()))], notes="The Country field also answers it (accepted).")
in_one_area("i04", "tornadoes", "tornadoes", "How many tornado tracks cross Kansas?", "en", "medium", "states_provinces", "name", "Kansas")
in_one_area("i05", "airports", "airports", "How many airports are there in Iceland?", "en", "easy", "countries", "name", "Iceland",
            accept_alt=[count_spec(int((gt.load("airports")["iso_country"] == "IS").sum()))], notes="iso_country IS also answers it (accepted).")
in_one_area("i06", "volcanoes", "volcanoes", "Show the volcanoes in Indonesia.", "en", "easy", "countries", "name", "Indonesia", how="set")
in_one_area("i07", "quakes", "quakes", "Muestra los terremotos de magnitud 8 o más dentro de Chile.", "es", "medium", "countries", "name", "Chile",
            mask=lambda g: (g["magnitude"] >= 8).to_numpy(), conds=[cond("magnitude", "at_least", 8)], how="set")
in_one_area("i08", "tornadoes", "tornadoes", "How many tornadoes rated 3 or higher crossed Texas?", "en", "medium", "states_provinces", "name", "Texas",
            mask=lambda g: (g["mag"] >= 3).to_numpy(), conds=[cond("mag", "at_least", 3)])
in_one_area("i09", "storms", "storms", "How many storm tracks crossed the Philippines?", "en", "medium", "countries", "name", "Philippines")
in_one_area("i10", "storms", "storms", "有多少条5级台风路径穿过日本？", "zh", "medium", "countries", "name", "Japan", mask=m_strength("Category 5"), conds=[S5])


# ---------------------------------------------------------------------------------------------
# C. Outside (at sea) and the part inside an area


def outside_land(cid, map_name, src, text, lang, diff, mask=None, conds=(), notes=""):
    g = gt.load(src)
    a = gt.load("countries")
    m = gt.in_areas(g, a, None if mask is None else mask(g))
    inside = set(sum(m.values(), []))
    pool = range(len(g)) if mask is None else np.nonzero(mask(g))[0]
    def located(i):
        p = g.geometry.iloc[i]
        return p is not None and not p.is_empty and (p.geom_type != "Point" or (np.isfinite(p.x) and np.isfinite(p.y)))

    out = [int(i) for i in pool if int(i) not in inside and located(int(i))]
    steps = [filt(L[src], *conds)] if conds else []
    steps.append(step("remove_inside", layer=L[src], areas=B["countries"]))
    add(cid, map_name, text, lang, "outside", diff, steps, count_spec(len(out)), set_spec(src, out),
        notes=(notes + f" GT {len(out)} outside every country (World countries; World land gives a close alternative).").strip())


outside_land("o01", "quakes", "quakes", "How many earthquakes of magnitude 8 or more happened at sea, outside any country?", "en", "medium",
             mask=lambda g: (g["magnitude"] >= 8).to_numpy(), conds=[cond("magnitude", "at_least", 8)])
outside_land("o02", "quakes", "quakes", "有多少次8级以上地震发生在海上（不在任何国家境内）？", "zh", "medium", mask=lambda g: (g["magnitude"] >= 8).to_numpy(), conds=[cond("magnitude", "at_least", 8)])
outside_land("o03", "volcanoes", "volcanoes", "Which volcanoes are not inside any country?", "en", "medium")
outside_land("o04", "nuclear", "nuclear", "Are there nuclear plants outside every country's borders? How many?", "en", "hard", notes="Coastal plants can fall just outside 1:10m coastlines.")


# ---------------------------------------------------------------------------------------------
# D. Nearest


def nearest_case(cid, map_name, src, text, lang, diff, name, country=None, mask=None, conds=(), want="name", notes=""):
    g = gt.load(src)
    x, y = place(name, country)
    row, km, km2 = gt.nearest_point(x, y, g, None if mask is None else mask(g))
    d_all = gt.geod_km(x, y, g.geometry.x.to_numpy(), g.geometry.y.to_numpy())
    if mask is not None:
        d_all = np.where(mask(g), d_all, np.inf)
    ties = [int(i) for i in np.nonzero(d_all <= km + gt.TOL_KM)[0]]
    pre, _ = city_plan(name, country)
    steps = list(pre) if pre else [filt(B["cities"], cond("name", "is", name))]
    if conds:
        steps.append(filt(L[src], *conds))
    steps.append(step("nearest_distance", layer=B["cities"], other=L[src]))
    if want == "name":
        col = "name" if "name" in g.columns else "Name"
        answer = {"type": "name", "accept": sorted({str(g[col].iloc[i]) for i in ties})}
    else:
        answer = {"type": "value", "value": round(km, 2), "tol": max(1.0, km * 0.01)}
    tie = " (near tie)" if km2 - km < 2 else ""
    add(cid, map_name, text, lang, "nearest", diff, steps, answer, set_spec(src, [row], [i for i in ties if i != row]),
        notes=(notes + f" GT nearest: {answer.get('accept', '')} row {row}, {km:.1f} km{tie}.").strip())


nearest_case("n01", "volcanoes", "volcanoes", "Which volcano is closest to Tokyo?", "en", "easy", "Tokyo")
nearest_case("n02", "nuclear", "nuclear", "离上海最近的核电站是哪一个？", "zh", "easy", "Shanghai")
nearest_case("n03", "nuclear", "nuclear", "Wie weit ist das nächste Kernkraftwerk von Berlin entfernt?", "de", "medium", "Berlin", want="km")
nearest_case("n04", "volcanoes", "volcanoes", "¿Cuál es el volcán más cercano a la Ciudad de México?", "es", "easy", "Mexico City")
nearest_case("n05", "volcanoes", "volcanoes", "How far is the nearest volcano from Seattle, in km?", "en", "medium", "Seattle", want="km")
nearest_case("n06", "nuclear", "nuclear", "What's the nearest operational nuclear plant to Tokyo?", "en", "medium", "Tokyo", mask=lambda g: (g["Status"] == "Operational").to_numpy(),
             conds=[cond("Status", "is", "Operational")])
nearest_case("n07", "volcanoes", "volcanoes", "离台北最近的火山有多远？", "zh", "medium", "Taipei", want="km")
nearest_case("n08", "nuclear", "nuclear", "Quelle est la centrale nucléaire la plus proche de Rome ?", "fr", "easy", "Rome")


def nearest_city_to(cid, map_name, src, text, lang, diff, field, value, notes=""):
    g = gt.load(src)
    rows = np.nonzero((g[field] == value).to_numpy())[0]
    assert len(rows) == 1, (value, rows)
    p = g.geometry.iloc[int(rows[0])]
    c = gt.load("cities")
    crow, km, km2 = gt.nearest_point(p.x, p.y, c)
    d_all = gt.geod_km(p.x, p.y, c.geometry.x.to_numpy(), c.geometry.y.to_numpy())
    accept = sorted({str(c["name"].iloc[int(i)]) for i in np.nonzero(d_all <= km + gt.TOL_KM)[0]})
    steps = [filt(L[src], cond(field, "is", value)), step("nearest_distance", layer=L[src], other=B["cities"])]
    tie = " (near tie)" if km2 - km < 2 else ""
    add(cid, map_name, text, lang, "nearest", diff, steps, {"type": "name", "accept": accept}, set_spec(src, [int(rows[0])]),
        notes=(notes + f" GT nearest city {c['name'].iloc[crow]} at {km:.1f} km{tie}.").strip())


nearest_city_to("n09", "volcanoes", "volcanoes", "Which city is nearest to Mount Etna?", "en", "easy", "name", "Etna")
nearest_city_to("n10", "volcanoes", "volcanoes", "富士山に一番近い都市はどこですか？", "ja", "medium", "name", "Fujisan", notes="Stored as 'Fujisan' (resolve_feature).")
nearest_city_to("n11", "volcanoes", "volcanoes", "Which city is closest to Pinatubo?", "en", "easy", "name", "Pinatubo")
nearest_city_to("n12", "volcanoes", "volcanoes", "离维苏威火山最近的城市是哪座？", "zh", "medium", "name", "Vesuvius")


# ---------------------------------------------------------------------------------------------
# E. Which cities are near something


def cities_near(cid, map_name, src, text, lang, diff, km, src_mask=None, src_conds=(), city_mask=None, city_conds=(), notes=""):
    """Cities (World cities) within km of any (filtered) feature of src: buffer src, keep_inside."""
    g = gt.load(src)
    c = gt.load("cities")
    pool = np.nonzero(src_mask(g))[0] if src_mask is not None else range(len(g))
    cm = np.ones(len(c), bool) if city_mask is None else city_mask(c)
    dist = np.full(len(c), np.inf)
    cx, cy = c.geometry.x.to_numpy(), c.geometry.y.to_numpy()
    for i in pool:
        p = g.geometry.iloc[int(i)]
        if p is None or p.is_empty or not (np.isfinite(p.x) and np.isfinite(p.y)):
            continue
        dist = np.minimum(dist, gt.geod_km(p.x, p.y, cx, cy))
    dist = np.where(cm, dist, np.inf)
    strict = np.nonzero(dist <= km - gt.TOL_KM)[0].tolist()
    edge = np.nonzero((dist > km - gt.TOL_KM) & (dist <= km + gt.TOL_KM))[0].tolist()
    steps = []
    if src_conds:
        steps.append(filt(L[src], *src_conds))
    if city_conds:
        steps.append(filt(B["cities"], *city_conds))
    steps.append(step("buffer", layer=L[src], distance=f"{km} km"))
    k = len(steps)
    steps.append(step("keep_inside", layer=B["cities"], areas=f"#{k}"))
    add(cid, map_name, text, lang, "cities_near", diff, steps, set_spec("cities", strict, edge), set_spec("cities", strict, edge), area=True,
        notes=(notes + f" GT {len(strict)}{'+' + str(len(edge)) + ' edge' if edge else ''} cities.").strip())


def cities_near_object(cid, map_name, src, text, lang, diff, field, value, km, city_mask=None, city_conds=(), notes=""):
    g = gt.load(src)
    rows = np.nonzero((g[field] == value).to_numpy())[0]
    assert len(rows) == 1, (value, rows)
    p = g.geometry.iloc[int(rows[0])]
    c = gt.load("cities")
    w = gt.within(p.x, p.y, c, km, None if city_mask is None else city_mask(c))
    steps = [filt(B["cities"], *city_conds)] if city_conds else []
    steps.append(step("buffer", layer=L[src], object=obj(L[src], field, value), distance=f"{km} km"))
    k = len(steps)
    steps.append(step("whats_inside", area=obj(f"#{k}"), layer=B["cities"]))
    add(cid, map_name, text, lang, "cities_near", diff, steps, set_spec("cities", w["strict"], w["edge"]), set_spec("cities", w["strict"], w["edge"]), area=True,
        notes=(notes + f" GT {len(w['strict'])}{'+' + str(len(w['edge'])) + ' edge' if w['edge'] else ''} cities.").strip())


cities_near_object("x01", "volcanoes", "volcanoes", "Which cities are within 50 km of Vesuvius?", "en", "easy", "name", "Vesuvius", 50)
cities_near_object("x02", "volcanoes", "volcanoes", "富士山50公里以内有哪些城市？", "zh", "medium", "name", "Fujisan", 50, notes="Stored as 'Fujisan'.")
cities_near_object("x03", "volcanoes", "volcanoes", "Which cities with more than 1 million people are within 100 km of Popocatepetl?", "en", "medium", "name", "Popocatepetl", 100,
                   city_mask=lambda c: (c["population"] > 1_000_000).to_numpy(), city_conds=[cond("population", "above", 1000000)])
cities_near_object("x04", "volcanoes", "volcanoes", "Which towns lie within 30 km of Mount Rainier?", "en", "easy", "name", "Rainier", 30)
cities_near("x05", "nuclear", "nuclear", "Which capital cities are within 100 km of an operational nuclear power plant?", "en", "hard", 100,
            src_mask=lambda g: (g["Status"] == "Operational").to_numpy(), src_conds=[cond("Status", "is", "Operational")],
            city_mask=lambda c: (c["capital"] == "yes").to_numpy(), city_conds=[cond("capital", "is", "yes")])
cities_near("x06", "volcanoes", "volcanoes", "哪些首都距离火山不到50公里？", "zh", "hard", 50, city_mask=lambda c: (c["capital"] == "yes").to_numpy(), city_conds=[cond("capital", "is", "yes")])
cities_near("x07", "quakes", "quakes", "Which capitals had a magnitude 8+ earthquake within 200 km?", "en", "hard", 200,
            src_mask=lambda g: (g["magnitude"] >= 8).to_numpy(), src_conds=[cond("magnitude", "at_least", 8)],
            city_mask=lambda c: (c["capital"] == "yes").to_numpy(), city_conds=[cond("capital", "is", "yes")])
cities_near("x08", "nuclear", "nuclear", "Which cities of over 5 million people are within 50 km of a nuclear plant?", "en", "medium", 50,
            city_mask=lambda c: (c["population"] > 5_000_000).to_numpy(), city_conds=[cond("population", "above", 5000000)])
cities_near("x09", "volcanoes", "volcanoes", "Quelles villes de plus d'un million d'habitants sont à moins de 30 km d'un volcan ?", "fr", "hard", 30,
            city_mask=lambda c: (c["population"] > 1_000_000).to_numpy(), city_conds=[cond("population", "above", 1000000)])


# ---------------------------------------------------------------------------------------------
# F. Joins by location


def join_filter(cid, map_name, src, text, lang, diff, area_field, value, areas="countries", how="count", mask=None, conds=(), notes=""):
    """join_by_location, then filter on the joined field. The app's join gives points just outside
    every area the nearest one within 25 km (owner's rule); the strictly-inside reading is
    accepted too."""
    g = gt.load(src)
    a = gt.load(areas)
    inside, near = gt.join_areas(g, a, None if mask is None else mask(g))
    strict = sorted(i for i, j in inside.items() if a[area_field].iloc[j] == value)
    extra = sorted(i for i, j in near.items() if a[area_field].iloc[j] == value)
    steps = [filt(L[src], *conds)] if conds else []
    steps.append(step("join_by_location", layer=L[src], areas=B[areas], fields=area_field))
    steps.append(filt(L[src], *conds, cond(area_field, "is", value)))
    if how == "count":
        answer = count_spec(len(strict) + len(extra))
        answer["alt"] = [count_spec(len(strict))]
    else:
        answer = set_spec(src, strict + extra)
        answer["alt"] = [set_spec(src, strict)]
    add(cid, map_name, text, lang, "join", diff, steps, answer, set_spec(src, strict, extra),
        notes=(notes + f" GT {len(strict) + len(extra)} with the app's 25 km rule ({len(strict)} strictly inside).").strip())


join_filter("j01", "volcanoes", "volcanoes", "Add the continent to each volcano, then tell me how many are in South America.", "en", "medium", "continent", "South America")
join_filter("j02", "quakes", "quakes", "Give each magnitude 8+ earthquake its continent and show the ones in Asia.", "en", "hard", "continent", "Asia", how="set",
            mask=lambda g: (g["magnitude"] >= 8).to_numpy(), conds=[cond("magnitude", "at_least", 8)])
join_filter("j03", "usgs", "usgs", "Add the state or province to each earthquake and show those in California.", "en", "medium", "name", "California", areas="states_provinces", how="set",
            notes="The place text also names California (not spatial).")
join_filter("j04", "volcanoes", "volcanoes", "给每座火山加上所在大洲，亚洲有多少座？", "zh", "medium", "continent", "Asia")
join_filter("j05", "nuclear", "nuclear", "Add each nuclear plant's continent and count the operational ones in Europe.", "en", "hard", "continent", "Europe",
            mask=lambda g: (g["Status"] == "Operational").to_numpy(), conds=[cond("Status", "is", "Operational")])
join_filter("j06", "airports", "airports", "Ajoute la région du monde (subregion) à chaque grand aéroport et montre ceux d'Afrique de l'Ouest.", "fr", "hard", "subregion", "Western Africa", how="set",
            mask=lambda g: (g["type"] == "large_airport").to_numpy(), conds=[cond("type", "is", "large_airport")])


# ---------------------------------------------------------------------------------------------
# G. Ranking after a spatial step


def busiest_city(cid, map_name, src, text, lang, diff, km, city_mask, city_conds, src_mask=None, src_conds=(), notes=""):
    g = gt.load(src)
    c = gt.load("cities")
    cm = city_mask(c)
    sm = np.ones(len(g), bool) if src_mask is None else src_mask(g)
    counts = {}
    for i in np.nonzero(cm)[0]:
        p = c.geometry.iloc[int(i)]
        w = gt.within(p.x, p.y, g, km, sm)
        counts[D.key_of(c, int(i))] = (len(w["strict"]), len(w["strict"]) + len(w["edge"]))
    best = max(v[0] for v in counts.values())
    top = [k for k, v in counts.items() if v[1] >= best]
    steps = []
    if src_conds:
        steps.append(filt(L[src], *src_conds))
    steps.append(filt(B["cities"], *city_conds))
    steps.append(step("buffer", layer=B["cities"], distance=f"{km} km", merge=False))
    k = len(steps)
    steps.append(step("count_in", points=L[src], areas=f"#{k}"))
    steps.append(step("rank_by_value", layer=f"#{k + 1}", field=D.COUNT_FIELD[src]))
    add(cid, map_name, text, lang, "rank", diff, steps, {"type": "top", "k": 1, "keys": top}, {"type": "layer", "role": "count"},
        notes=(notes + f" GT top {top} ({best}).").strip(), tags=["derived"])


busiest_city("r01", "storms", "storms", "Which capital city has had the most Category 4 or 5 storm tracks within 200 km?", "en", "hard", 200,
             lambda c: (c["capital"] == "yes").to_numpy(), [cond("capital", "is", "yes")], m_strength("Category 4", "Category 5"), [S45])
busiest_city("r02", "quakes", "quakes", "哪个首都300公里范围内发生的7级以上地震最多？", "zh", "hard", 300,
             lambda c: (c["capital"] == "yes").to_numpy(), [cond("capital", "is", "yes")], lambda g: (g["magnitude"] >= 7).to_numpy(), [cond("magnitude", "at_least", 7)])
busiest_city("r03", "volcanoes", "volcanoes", "Which city of more than 5 million people has the most volcanoes within 300 km?", "en", "medium", 300,
             lambda c: (c["population"] > 5_000_000).to_numpy(), [cond("population", "above", 5000000)])
busiest_city("r04", "nuclear", "nuclear", "Which city of over 10 million people has the most nuclear plants within 200 km?", "en", "medium", 200,
             lambda c: (c["population"] > 10_000_000).to_numpy(), [cond("population", "above", 10000000)])
busiest_city("r05", "tornadoes", "tornadoes", "Which US state capital had the most tornadoes rated 3+ within 50 km?", "en", "hard", 50,
             lambda c: ((c["country"] == "United States of America") & (c["kind"] == "Admin-1 capital")).to_numpy(),
             [cond("country", "is", "United States of America"), cond("kind", "is", "Admin-1 capital")], lambda g: (g["mag"] >= 3).to_numpy(), [cond("mag", "at_least", 3)])
per_country_top("r06", "nuclear", "nuclear", "Rank the countries by number of nuclear plants under construction; which is first?", "en", "medium",
                mask=lambda g: (g["Status"] == "Under Construction").to_numpy(), conds=[cond("Status", "is", "Under Construction")])
per_country_top("r07", "volcanoes", "volcanoes", "Rangschik de landen naar aantal stratovulkanen. Welk land staat bovenaan?", "nl", "medium",
                mask=lambda g: (g["morphology"] == "Stratovolcano").to_numpy(), conds=[cond("morphology", "is", "Stratovolcano")])


# ---------------------------------------------------------------------------------------------
# H. Raster values at points


ETOPO = D.FILES["etopo"]
REF_ELEV = gt.REF / "elevation.tif"


def raster_value(cid, map_name, src, text, lang, diff, raster_key, field, value, notes=""):
    g = gt.load(src)
    rows = np.nonzero((g[field] == value).to_numpy())[0]
    assert len(rows) == 1
    p = g.geometry.iloc[int(rows[0])]
    path = ETOPO if raster_key == "etopo" else REF_ELEV
    v = gt.raster_at(path, [p.x], [p.y])[0]
    rname = L["etopo"] if raster_key == "etopo" else B["elevation"]
    steps = [filt(L[src], cond(field, "is", value)), step("add_raster_values", raster=rname, points=L[src])]
    add(cid, map_name, text, lang, "raster", diff, steps, {"type": "value", "value": v, "tol": 1.0}, set_spec(src, [int(rows[0])]),
        notes=(notes + f" GT {v} m (cell containing the point).").strip())


def raster_top(cid, map_name, src, text, lang, diff, raster_key, lowest=False, mask=None, conds=(), notes=""):
    g = gt.load(src)
    path = ETOPO if raster_key == "etopo" else REF_ELEV
    vals = np.array([np.nan if x is None else x for x in gt.raster_at(path, g.geometry.x, g.geometry.y)], float)
    if mask is not None:
        vals = np.where(mask(g), vals, np.nan)
    best = np.nanmin(vals) if lowest else np.nanmax(vals)
    top = [D.key_of(g, int(i)) for i in np.nonzero(vals == best)[0]]
    rname = L["etopo"] if raster_key == "etopo" else B["elevation"]
    steps = [filt(L[src], *conds)] if conds else []
    steps += [step("add_raster_values", raster=rname, points=L[src]), step("rank_by_value", layer=L[src], field=rname, **({"order": "lowest"} if lowest else {}))]
    add(cid, map_name, text, lang, "raster", diff, steps, {"type": "top", "k": 1, "keys": top}, None, notes=(notes + f" GT {best} m: {top[:3]}.").strip(), tags=["derived"])


def raster_count(cid, map_name, src, text, lang, diff, raster_key, op, thr, notes=""):
    g = gt.load(src)
    path = ETOPO if raster_key == "etopo" else REF_ELEV
    vals = np.array([np.nan if x is None else x for x in gt.raster_at(path, g.geometry.x, g.geometry.y)], float)
    sel = np.nonzero(vals < thr if op == "below" else vals > thr)[0]
    rname = L["etopo"] if raster_key == "etopo" else B["elevation"]
    steps = [step("add_raster_values", raster=rname, points=L[src]), filt(L[src], cond(rname, op, thr))]
    add(cid, map_name, text, lang, "raster", diff, steps, count_spec(len(sel)), set_spec(src, sel), notes=(notes + f" GT {len(sel)}.").strip(), tags=["derived"])


raster_value("e01", "volc_etopo", "volcanoes", "What does the elevation grid say the height is at Mount Fuji?", "en", "medium", "etopo", "name", "Fujisan",
             notes="The 0.5-degree ETOPO cell is far lower than the summit.")
raster_value("e02", "volc_etopo", "volcanoes", "根据高程栅格，埃特纳火山处的海拔是多少？", "zh", "medium", "etopo", "name", "Etna")
raster_top("e03", "volc_etopo", "volcanoes", "According to the elevation grid, which volcano stands highest?", "en", "medium", "etopo")
raster_top("e04", "volc_etopo", "volcanoes", "根据高程数据，哪座火山所在位置最低？", "zh", "medium", "etopo", lowest=True)
raster_count("e05", "volc_etopo", "volcanoes", "How many volcanoes sit below sea level according to the elevation grid?", "en", "medium", "etopo", "below", 0)
raster_top("e06", "nuclear", "nuclear", "Which nuclear power plant sits at the highest elevation?", "en", "hard", "ref", notes="Needs the built-in World elevation.")
raster_value("e07", "volcanoes", "volcanoes", "What is the elevation at Popocatepetl in the built-in elevation data?", "en", "medium", "ref", "name", "Popocatepetl")
raster_count("e08", "quakes", "quakes", "How many earthquakes struck where the sea floor is deeper than 4000 m?", "en", "hard", "ref", "below", -4000, notes="World elevation below -4000 m.")
raster_top("e09", "launch", "volcanoes", "標高グリッドによると、最も高い場所にある火山はどれですか？", "ja", "medium", "etopo", notes="On the full launch map.")


# ---------------------------------------------------------------------------------------------
# I. Time slices (NetCDF heat-wave grids)


def heat_point(cid, map_name, text, lang, diff, city_name, date, nc_key, country=None, shown_date=None, notes=""):
    x, y = place(city_name, country)
    v = gt.netcdf_at(D.FILES[nc_key], "heatwave_intensity", date, [x], [y])[0]
    pre, o = city_plan(city_name, country)
    steps = list(pre) if pre else [filt(B["cities"], cond("name", "is", city_name))]
    steps.append(step("add_raster_values", raster=L[nc_key], points=B["cities"]))
    if shown_date and date != shown_date:
        add(cid, map_name, text, lang, "time", diff, refuse("The plan can't pick a date of the grid; the map shows another day."), None, None,
            kind="cannot_do", accept=["ask_user", "plan_on_date"], notes=(notes + f" GT {v} on {date}; the app shows {shown_date} and no tool picks a date.").strip(),
            tags=["tool_gap"])
        CASES[-1]["expect"]["date_plan"] = {"gold": steps, "answer": {"type": "value", "value": v, "tol": 0.05}}
    else:
        add(cid, map_name, text, lang, "time", diff, steps, {"type": "value", "value": v, "tol": 0.05}, None, notes=(notes + f" GT {v} on {date}.").strip())


SHOWN22 = "2022-06-01"
SHOWN23 = "2023-06-01"
heat_point("t01", "heat22", "How intense was the heat wave in Rome on 1 June 2022?", "en", "medium", "Rome", "2022-06-01", "heat22", shown_date=SHOWN22)
heat_point("t02", "heat22", "How intense was the heat wave in Barcelona on 16 August 2022?", "en", "hard", "Barcelona", "2022-08-16", "heat22", country="Spain", shown_date=SHOWN22,
           notes="The launch film's question.")
heat_point("t03", "heat23", "2023年7月18日罗马的热浪强度是多少？", "zh", "hard", "Rome", "2023-07-18", "heat23", shown_date=SHOWN23)
heat_point("t04", "heat23", "2023年6月1日昆明的热浪强度是多少？", "zh", "medium", "Kunming", "2023-06-01", "heat23", shown_date=SHOWN23)
heat_point("t05", "heat22", "2022年8月1日巴塞罗那的热浪有多强？", "zh", "hard", "Barcelona", "2022-08-01", "heat22", country="Spain", shown_date=SHOWN22)


def heat_country(cid, map_name, text, lang, diff, nc_key, date, notes=""):
    import xarray as xr

    ds = xr.open_dataset(D.FILES[nc_key])
    arr = ds["heatwave_intensity"].sel(time=np.datetime64(date))
    a = gt.load("countries")
    import shapely

    lon, lat = np.meshgrid(arr["lon"].values, arr["lat"].values)
    vals = arr.values
    ok = np.isfinite(vals)
    pts = shapely.points(lon[ok], lat[ok])
    v = vals[ok]
    import geopandas as gpd

    cells = gpd.GeoDataFrame({"v": v}, geometry=pts, crs=4326)
    j = gpd.sjoin(cells, a[["geometry"]], predicate="within")
    means = j.groupby("index_right")["v"].mean()
    best = means.idxmax()
    top = [D.key_of(a, int(best))]
    steps = [step("summarize_each", raster=L[nc_key], areas=B["countries"], stat="mean"), step("rank_by_value", layer="#1", field=f"{L[nc_key]} mean")]
    add(cid, map_name, text, lang, "time", diff, steps, {"type": "top", "k": 1, "keys": top}, {"type": "layer", "role": "count"},
        notes=(notes + f" GT (cell centres in each country, {date}): {top} {means.max():.2f}. Small countries are sensitive to the cell rule.").strip(), tags=["derived", "gt_rule"])


heat_country("t06", "heat22", "On the day the map shows, which country had the highest average heat-wave intensity?", "en", "hard", "heat22", SHOWN22)
heat_country("t07", "heat23", "地图显示的这一天，哪个国家的平均热浪强度最高？", "zh", "hard", "heat23", SHOWN23)
add("t08", "heat23", "Compare this summer's heat wave with the summer before.", "en", "honest", "medium", refuse("Only one summer is open."), None, None, kind="cannot_do",
    accept=["ask_user"], notes="Only 2023 is on the map.", tags=["unanswerable"])


# ---------------------------------------------------------------------------------------------
# J. Multi-step chains that reference derived fields


def far_from_cities(cid, map_name, src, text, lang, diff, km, mask=None, conds=(), how="count", notes=""):
    g = gt.load(src)
    c = gt.load("cities")
    cx, cy = c.geometry.x.to_numpy(), c.geometry.y.to_numpy()
    pool = np.nonzero(mask(g))[0] if mask is not None else np.arange(len(g))
    d = {}
    for i in pool:
        p = g.geometry.iloc[int(i)]
        if p is None or p.is_empty or not (np.isfinite(p.x) and np.isfinite(p.y)):
            continue
        d[int(i)] = float(gt.geod_km(p.x, p.y, cx, cy).min())
    strict = [i for i, v in d.items() if v > km + gt.TOL_KM]
    edge = [i for i, v in d.items() if km - gt.TOL_KM <= v <= km + gt.TOL_KM]
    steps = [filt(L[src], *conds)] if conds else []
    steps.append(step("nearest_distance", layer=L[src], other=B["cities"]))
    steps.append(filt(L[src], *conds, cond(D.NEAREST_FIELD["cities"], "above", km)))
    answer = count_spec(len(strict), len(strict) + len(edge)) if how == "count" else set_spec(src, strict, edge)
    add(cid, map_name, text, lang, "chain", diff, steps, answer, set_spec(src, strict, edge), notes=(notes + f" GT {len(strict)} (+{len(edge)} edge).").strip(), tags=["derived"])


far_from_cities("k01", "volcanoes", "volcanoes", "Work out each volcano's distance to the nearest city, then tell me how many are more than 300 km from any city.", "en", "medium", 300)
far_from_cities("k02", "volcanoes", "volcanoes", "给每座火山算出到最近城市的距离，然后只显示超过200公里的。", "zh", "medium", 200, how="set")
far_from_cities("k03", "nuclear", "nuclear", "How many nuclear plants are more than 100 km from the nearest city? First measure the distance, then filter.", "en", "medium", 100)
far_from_cities("k04", "volcanoes", "volcanoes", "火山から最寄りの都市までの距離を計算し、500km以上離れた火山の数を教えてください。", "ja", "medium", 500)
far_from_cities("k05", "quakes", "quakes", "For the magnitude 8+ earthquakes, add the distance to the nearest city and keep those more than 250 km away.", "en", "hard", 250, how="set",
                mask=lambda g: (g["magnitude"] >= 8).to_numpy(), conds=[cond("magnitude", "at_least", 8)])


def capitals_with_n(cid, map_name, src, text, lang, diff, km, nmin, src_mask, src_conds, notes=""):
    g = gt.load(src)
    c = gt.load("cities")
    rows = np.nonzero((c["capital"] == "yes").to_numpy())[0]
    strict, edge = [], []
    for i in rows:
        p = c.geometry.iloc[int(i)]
        w = gt.within(p.x, p.y, g, km, src_mask(g))
        lo, hi = len(w["strict"]), len(w["strict"]) + len(w["edge"])
        if lo >= nmin:
            strict.append(int(i))
        elif hi >= nmin:
            edge.append(int(i))
    field = D.COUNT_FIELD[src]
    steps = [filt(L[src], *src_conds), filt(B["cities"], cond("capital", "is", "yes")), step("buffer", layer=B["cities"], distance=f"{km} km", merge=False),
             step("count_in", points=L[src], areas="#3"), filt("#4", cond(field, "at_least", nmin))]
    add(cid, map_name, text, lang, "chain", diff, steps, set_spec("cities", strict, edge), {"type": "layer", "role": "count"},
        notes=(notes + f" GT {len(strict)} capitals (+{len(edge)} edge). The count field is '{field}' (the tool description says the layer name, lowercase).").strip(), tags=["derived"])


capitals_with_n("k06", "storms", "storms", "Count the Category 5 tracks within 200 km of each capital, then show the capitals with at least 3.", "en", "hard", 200, 3, m_strength("Category 5"), [S5])
capitals_with_n("k07", "quakes", "quakes", "统计每个首都200公里内的7级以上地震数量，然后只显示至少有2次的首都。", "zh", "hard", 200, 2,
                lambda g: (g["magnitude"] >= 7).to_numpy(), [cond("magnitude", "at_least", 7)])


def elev_difference(cid, map_name, text, lang, diff, notes=""):
    g = gt.load("volcanoes")
    vals = np.array([np.nan if x is None else x for x in gt.raster_at(ETOPO, g.geometry.x, g.geometry.y)], float)
    d = g["elevation_m"].to_numpy(float) - vals
    best = np.nanmax(d)
    top = [D.key_of(g, int(i)) for i in np.nonzero(d == best)[0]]
    r = L["etopo"]
    steps = [step("add_raster_values", raster=r, points=L["volcanoes"]),
             step("calculate_field", layer=L["volcanoes"], first="elevation_m", op="subtract", second=r, name="summit above grid"),
             step("rank_by_value", layer=L["volcanoes"], field="summit above grid")]
    add(cid, map_name, text, lang, "chain", diff, steps, {"type": "top", "k": 1, "keys": top}, None, notes=(notes + f" GT {top} ({best:.0f} m).").strip(), tags=["derived"])


elev_difference("k08", "volc_etopo", "Add the grid elevation to each volcano, compute elevation_m minus that grid value, and rank by the difference.", "en", "hard")
elev_difference("k09", "volc_etopo", "给火山加上栅格高程，算出 elevation_m 减去栅格值的差，并按差值排序。", "zh", "hard")


def per_capita(cid, map_name, src, text, lang, diff, mask=None, conds=(), min_pop=1_000_000, notes=""):
    counts, _ = per_area_counts(src, "countries", mask)
    a = gt.load("countries")
    rate = {}
    for i in range(len(a)):
        k = D.key_of(a, i)
        pop = a["population"].iloc[i]
        if k in counts and pop and pop > 0:
            rate[k] = counts[k] / pop * 1e6
    best = max(rate.values())
    top = [k for k, v in rate.items() if abs(v - best) < 1e-9]
    field = D.COUNT_FIELD[src]
    steps = [filt(L[src], *conds)] if conds else []
    n = len(steps)
    steps += [step("count_in", points=L[src], areas=B["countries"]),
              step("calculate_field", layer=f"#{n + 1}", first=field, op="divide", second="population", name=f"{field} per person"),
              step("calculate_field", layer=f"#{n + 1}", first=f"{field} per person", op="multiply", second="1000000", name=f"{field} per million people"),
              step("rank_by_value", layer=f"#{n + 1}", field=f"{field} per million people")]
    add(cid, map_name, text, lang, "chain", diff, steps, {"type": "top", "k": 1, "keys": top}, {"type": "layer", "role": "count"},
        notes=(notes + f" GT {top} ({best:.2f} per million).").strip(), tags=["derived"])


per_capita("k10", "quakes", "quakes", "Which country has the most magnitude 7+ earthquakes per million people?", "en", "hard",
           mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)])
per_capita("k11", "nuclear", "nuclear", "按人均（每百万人）核电站数量，哪个国家排第一？", "zh", "hard")
per_capita("k12", "volcanoes", "volcanoes", "Which country has the most volcanoes per million inhabitants?", "en", "hard")


def isolated_quakes(cid, map_name, text, lang, diff, mag, km, notes=""):
    g = gt.load("quakes")
    sel = np.nonzero((g["magnitude"] >= mag).to_numpy())[0]
    xs, ys = g.geometry.x.to_numpy(), g.geometry.y.to_numpy()
    strict, edge = [], []
    for i in sel:
        d = gt.geod_km(xs[i], ys[i], xs[sel], ys[sel])
        d = d[d > 0] if (d > 0).any() else d
        m = float(np.sort(gt.geod_km(xs[i], ys[i], xs[sel], ys[sel]))[1])
        if m > km + gt.TOL_KM:
            strict.append(int(i))
        elif m >= km - gt.TOL_KM:
            edge.append(int(i))
    q = L["quakes"]
    steps = [filt(q, cond("magnitude", "at_least", mag)), step("nearest_other", layer=q),
             filt(q, cond("magnitude", "at_least", mag), cond("km to nearest other earthquake", "above", km))]
    add(cid, map_name, text, lang, "chain", diff, refuse("nearest_other measures to every quake, also the hidden ones."), None, None, kind="cannot_do", accept=["ask_user"],
        notes=(notes + f" GT {len(strict)} (+{len(edge)}) among the M{mag}+ quakes. No tool answers it: nearest_other measures to every object of the layer, even hidden"
               " ones (owner's rule), though its tool description says only shown objects count. A plan that trusts the description answers 0.").strip(),
        tags=["derived", "tool_gap"])
    CASES[-1]["expect"]["trap_plan"] = steps


isolated_quakes("k13", "quakes", "Among the magnitude 8+ quakes, how many are more than 1000 km from any other magnitude 8+ quake?", "en", "hard", 8, 1000)


def joined_state_count(cid, map_name, text, lang, diff, state, mag, notes=""):
    """The join places a line by a point on it; which point is the app's choice, so the answer is
    accepted between the smallest and largest count of the readings (start, middle and end point,
    or any part touching the state), each with the 25 km rule."""
    import geopandas as gpd

    g = gt.load("tornadoes")
    a = gt.load("states_provinces")
    sel = [i for i in range(len(a)) if a["name"].iloc[i] == state and a["country"].iloc[i] == "United States of America"]
    assert len(sel) == 1
    mask = (g["mag"] >= mag).to_numpy()
    rows = np.nonzero(mask)[0]
    touching = sorted(set(sum(gt.in_areas(g, a.iloc[sel], mask).values(), [])))
    counts = [len(touching)]
    sets = {}
    for where in (0.0, 0.5, 1.0):
        pts = gpd.GeoDataFrame(geometry=[g.geometry.iloc[int(i)].interpolate(where, normalized=True) for i in rows], crs=4326)
        inside, near = gt.join_areas(pts, a)
        hit = sorted(int(rows[k]) for k, j in list(inside.items()) + list(near.items()) if j == sel[0])
        sets[where] = hit
        counts.append(len(hit))
    t = L["tornadoes"]
    steps = [filt(t, cond("mag", "at_least", mag)), step("join_by_location", layer=t, areas=B["states_provinces"], fields="name"),
             filt(t, cond("mag", "at_least", mag), cond("name", "is", state))]
    core = sorted(set(sets[0.0]) & set(sets[0.5]) & set(sets[1.0]))
    every = sorted(set(touching) | set(sets[0.0]) | set(sets[0.5]) | set(sets[1.0]))
    add(cid, map_name, text, lang, "chain", diff, steps, count_spec(min(counts), max(counts)), set_spec("tornadoes", core, sorted(set(every) - set(core))),
        notes=(notes + f" GT {min(counts)}–{max(counts)} (start/middle/end point or touching: {counts}).").strip(), tags=["derived", "gt_rule"])


joined_state_count("k14", "tornadoes", "Add the state to each tornado rated 4 or more, then count the ones in Oklahoma.", "en", "hard", "Oklahoma", 4)


def storms_far(cid, map_name, text, lang, diff, km, notes=""):
    g = gt.load("storms")
    c = gt.load("cities")
    sel = np.nonzero((g["strength"] == "Category 5").to_numpy())[0]
    strict, edge = [], []
    import shapely

    cxy = np.c_[c.geometry.x.to_numpy(), c.geometry.y.to_numpy()]
    for i in sel:
        geom = g.geometry.iloc[int(i)]
        xy = shapely.get_coordinates(shapely.segmentize(geom, gt.DENSIFY_DEG))
        # nearest city to any vertex (geodesic), cities prefiltered by a box
        b = geom.bounds
        box = (cxy[:, 0] > b[0] - 15) & (cxy[:, 0] < b[2] + 15) & (cxy[:, 1] > b[1] - 12) & (cxy[:, 1] < b[3] + 12)
        best = np.inf
        for cx, cy in cxy[box]:
            best = min(best, float(gt.geod_km(cx, cy, xy[:, 0], xy[:, 1]).min()))
        if best > km + gt.TOL_KM:
            strict.append(int(i))
        elif best >= km - gt.TOL_KM:
            edge.append(int(i))
    s = L["storms"]
    steps = [filt(s, S5), step("nearest_distance", layer=s, other=B["cities"]), filt(s, S5, cond(D.NEAREST_FIELD["cities"], "above", km))]
    add(cid, map_name, text, lang, "chain", diff, steps, count_spec(len(strict), len(strict) + len(edge)), set_spec("storms", strict, edge),
        notes=(notes + f" GT {len(strict)} (+{len(edge)}).").strip(), tags=["derived"])


storms_far("k15", "storms", "How many Category 5 tracks never came within 500 km of any city? Measure each track's distance to the nearest city first.", "en", "hard", 500)


def chain_tokyo_after(cid, map_name, text, lang, diff, year):
    g = gt.load("storms")
    x, y = place("Tokyo")
    mask = ((g["strength"] == "Category 5") & (g["season"] >= year)).to_numpy()
    w = gt.within(x, y, g, 100, mask)
    s = L["storms"]
    steps = [filt(s, S5, cond("season", "at_least", year)), step("buffer", layer=B["cities"], object=obj(B["cities"], "name", "Tokyo"), distance="100 km"),
             step("count_here", points=s, area=obj("#2"))]
    add(cid, map_name, text, lang, "chain", diff, steps, count_spec(len(w["strict"]), len(w["strict"]) + len(w["edge"])), set_spec("storms", w["strict"], w["edge"]),
        area=True, notes=f"GT {len(w['strict'])}.")


chain_tokyo_after("k16", "launch", "Of the Category 5 storms that passed within 100 km of Tokyo, how many were in 2000 or later?", "en", "hard", 2000)


# ---------------------------------------------------------------------------------------------
# K. Vague and unanswerable


def meta(cid, map_name, text, lang, kind, accept=(), notes="", diff="medium"):
    gold = ask("What would you like to know?") if kind == "ask_user" else refuse("No tool or layer answers this.")
    add(cid, map_name, text, lang, "honest", diff, gold, None, None, kind=kind, accept=accept, notes=notes, tags=["unanswerable" if kind == "cannot_do" else "vague"])


meta("u01", "storms", "Analyze the storms.", "en", "ask_user", accept=["cannot_do"])
meta("u02", "storms", "Which storm will hit Tokyo next year?", "en", "cannot_do", accept=["ask_user"], notes="A forecast.")
meta("u03", "volcanoes", "Draw the fastest driving route from Tokyo to the nearest volcano.", "en", "cannot_do", notes="Routing.")
meta("u04", "quakes", "How many people died in these earthquakes?", "en", "cannot_do", accept=["ask_user"], notes="No casualties field.")
meta("u05", "volcanoes", "Buffer the volcanoes.", "en", "ask_user", notes="No distance.")
meta("u06", "volcanoes", "Which volcanoes are near Portland?", "en", "ask_user", accept=["plan"], notes="Three Portlands; a plan for Portland, Oregon is also accepted.", diff="hard")
meta("u07", "storms", "台风给上海造成了多少经济损失？", "zh", "cannot_do", accept=["ask_user"], notes="No loss data.")
meta("u08", "quakes", "预测明年会在哪里发生大地震。", "zh", "cannot_do", notes="A forecast.")
meta("u09", "launch", "Show me something interesting.", "en", "ask_user", accept=["cannot_do"])
meta("u10", "airports", "Which airports will close because of volcanic ash this week?", "en", "cannot_do", accept=["ask_user"], notes="A forecast; no volcano layer either.")
meta("u11", "quakes", "Download the latest earthquakes from USGS and add them.", "en", "cannot_do")
meta("u12", "tornadoes", "Which was the deadliest tornado within 100 km of Springfield?", "en", "ask_user", notes="Five Springfields in World cities.", diff="hard")
meta("u13", "heat23", "Wie heiß ist es heute in Berlin?", "de", "cannot_do", accept=["ask_user"], notes="The grid is summer 2023, not today.")
meta("u14", "nuclear", "这些核电站的发电量加起来是多少千瓦时？", "zh", "cannot_do", accept=["ask_user"], notes="Capacity (MW) exists, generation (kWh) doesn't.")
meta("u15", "storms", "把这些弄一下。", "zh", "ask_user", accept=["cannot_do"])
meta("u16", "storms", "How many Category 6 storms passed within 100 km of Tokyo?", "en", "cannot_do", accept=["ask_user", "zero"],
     notes="There is no Category 6; answering 0 (an empty filter) is also accepted.", diff="hard")
meta("u17", "volcanoes", "火山の噴火で何人が避難しましたか？", "ja", "cannot_do", accept=["ask_user"], notes="No evacuation data.")
meta("u18", "airports", "¿Cuál es el vuelo más barato de Madrid a Tokio?", "es", "cannot_do", notes="Flights and prices.")


# ---------------------------------------------------------------------------------------------
# More Chinese (the app's second language), across the types above


within_case("w40", "storms", "storms", "纽约100公里范围内经过了多少条5级飓风？", "zh", "hard", "New York", 100, mask=m_strength("Category 5"), conds=[S5], tags=["zero"])
within_case("w41", "quakes", "quakes", "东京200公里以内发生过几次7级以上的地震？", "zh", "medium", "Tokyo", 200, mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)])
within_case("w42", "volcanoes", "volcanoes", "西雅图200公里内有哪些火山？", "zh", "easy", "Seattle", 200, how="set")
within_case("w43", "airports", "airports", "首尔100公里内有几个大型机场？", "zh", "medium", "Seoul", 100, mask=lambda g: (g["type"] == "large_airport").to_numpy(), conds=[cond("type", "is", "large_airport")])
within_case("w44", "tornadoes", "tornadoes", "俄克拉荷马城50公里内发生过多少次4级以上的龙卷风？", "zh", "medium", "Oklahoma City", 50, mask=lambda g: (g["mag"] >= 4).to_numpy(), conds=[cond("mag", "at_least", 4)])
within_case("w45", "usgs", "usgs", "这个文件里，洛杉矶100公里内有多少次地震？", "zh", "hard", "Los Angeles", 100, country="United States of America", accept=["ask_user"],
            notes="Two Los Angeles; asking is accepted.")
nearest_case("n13", "volcanoes", "volcanoes", "离墨西哥城最近的火山是哪座？", "zh", "easy", "Mexico City")
nearest_case("n14", "nuclear", "nuclear", "柏林离最近的核电站有多少公里？", "zh", "medium", "Berlin", want="km")
in_one_area("i11", "quakes", "quakes", "日本国境内发生过多少次7级以上地震？", "zh", "medium", "countries", "name", "Japan", mask=lambda g: (g["magnitude"] >= 7).to_numpy(), conds=[cond("magnitude", "at_least", 7)])
in_one_area("i12", "nuclear", "nuclear", "法国境内有多少座核电站？", "zh", "easy", "countries", "name", "France",
            accept_alt=[count_spec(int((gt.load("nuclear")["Country"] == "France").sum()))], notes="The Country field also answers it (accepted).")
per_country_top("c13", "airports", "airports", "哪个州或省的大型机场最多？", "zh", "hard", areas="states_provinces",
                mask=lambda g: (g["type"] == "large_airport").to_numpy(), conds=[cond("type", "is", "large_airport")])
capitals_with_n("k17", "storms", "storms", "统计每个首都200公里内的5级台风路径数，然后只显示至少有3条的首都。", "zh", "hard", 200, 3, m_strength("Category 5"), [S5])
raster_top("e10", "nuclear", "nuclear", "根据内置高程数据，哪座核电站海拔最高？", "zh", "hard", "ref")
cities_near_object("x10", "volcanoes", "volcanoes", "维苏威火山50公里以内有哪些城市？", "zh", "easy", "name", "Vesuvius", 50)
meta("u19", "volcanoes", "分析一下这些火山。", "zh", "ask_user", accept=["cannot_do"])
meta("u20", "airports", "帮我给机场做个缓冲区。", "zh", "ask_user", notes="No distance.")


def main() -> int:
    ids = [c["id"] for c in CASES]
    assert len(ids) == len(set(ids))
    OUT.write_text("".join(json.dumps(c, ensure_ascii=False) + "\n" for c in CASES), encoding="utf-8")
    from collections import Counter

    print(f"{len(CASES)} cases -> {OUT}")
    print("by type:", dict(Counter(c["qtype"] for c in CASES)))
    print("by language:", dict(Counter(c["lang"] for c in CASES)))
    print("by kind:", dict(Counter(c["expect"]["kind"] for c in CASES)))
    print("by map:", dict(Counter(c["map"] for c in CASES)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
