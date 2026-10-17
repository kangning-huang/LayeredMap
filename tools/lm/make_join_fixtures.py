"""Write the join fixtures J1-J5 (WP3.11) into fixtures/J1..J5. Deterministic; no downloads.

    python tools/lm/make_join_fixtures.py

J1  the bundled countries (assets/examples/countries.geojson) + a World Bank-style table
    keyed by country names with WB spellings, and one keyed by ISO3 codes
J2  six US counties as squares with 5-digit GEOID text + a table whose FIPS codes lost their
    leading zeros
J3  V1 districts + one row per district per year, 2019-2023
J4  ten places with a numeric id and a name + a table matching both partly and crosswise
J5  eight Chinese provinces (English names) + a table keyed by Chinese names
"""

import csv
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIX = ROOT / "fixtures"


def square(x, y, s=1.0):
    return {"type": "Polygon", "coordinates": [[[x, y], [x + s, y], [x + s, y + s], [x, y + s], [x, y]]]}


def write_geojson(path, rows):
    fc = {"type": "FeatureCollection", "features": [{"type": "Feature", "properties": p, "geometry": g} for p, g in rows]}
    path.write_text(json.dumps(fc, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


def write_csv(path, header, rows):
    with path.open("w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(header)
        w.writerows(rows)


def j1():
    d = FIX / "J1"
    d.mkdir(exist_ok=True)
    countries = json.loads((ROOT / "assets/examples/countries.geojson").read_text(encoding="utf-8"))
    iso = [f["properties"]["iso_a3"] for f in countries["features"] if f["properties"].get("iso_a3")]
    write_csv(d / "gdp_growth_iso3.csv", ["Country Code", "gdp_growth_2023"], [[c, round((i * 37 % 100) / 10 - 3, 1)] for i, c in enumerate(iso)])
    wb = [
        ("Korea, Rep.", 51.7), ("Egypt, Arab Rep.", 112.7), ("Russian Federation", 143.8), ("Viet Nam", 98.9),
        ("Cote d'Ivoire", 28.9), ("Iran, Islamic Rep.", 89.2), ("Turkiye", 85.3), ("Congo, Dem. Rep.", 102.3),
        ("Lao PDR", 7.6), ("Yemen, Rep.", 34.4), ("Venezuela, RB", 28.8), ("Syrian Arab Republic", 23.2),
        ("France", 68.2), ("Brazil", 216.4), ("Japan", 124.5), ("Germany", 84.5), ("China", 1410.7),
        ("United States", 334.9), ("India", 1428.6), ("Kenya", 55.1),
    ]
    write_csv(d / "population_wb_names.csv", ["Country Name", "population_millions"], wb)


def j2():
    d = FIX / "J2"
    d.mkdir(exist_ok=True)
    counties = [("06037", "Los Angeles"), ("06059", "Orange"), ("06073", "San Diego"), ("36061", "New York"), ("01001", "Autauga"), ("08031", "Denver")]
    write_geojson(d / "counties.geojson", [({"GEOID": g, "NAME": n}, square(i * 1.2, 0)) for i, (g, n) in enumerate(counties)])
    write_csv(d / "cases.csv", ["fips", "cases"], [[int(g), (i + 1) * 100] for i, (g, _) in enumerate(counties)])


def j3():
    d = FIX / "J3"
    d.mkdir(exist_ok=True)
    districts = json.loads((FIX / "V1/districts.geojson").read_text(encoding="utf-8"))
    rows = []
    for k, f in enumerate(districts["features"]):
        p = f["properties"]
        for y in range(2019, 2024):
            pop = p["population"]
            rows.append([p["name_en"], y, "" if pop is None else pop + (y - 2023) * (1000 + 37 * k)])
    write_csv(d / "population_by_year.csv", ["district", "year", "population"], rows)


def j4():
    d = FIX / "J4"
    d.mkdir(exist_ok=True)
    names = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel", "India", "Juliet"]
    write_geojson(d / "places.geojson", [({"id": i + 1, "name": n}, square(i * 1.2, 0)) for i, n in enumerate(names)])
    rows = []
    for i in range(10):
        code = i + 1 if i < 6 else 100 + i
        label = names[(13 - i) % 10] if i >= 4 else f"Zulu {i + 1}"
        rows.append([code, label, i * 10])
    write_csv(d / "mixed_keys.csv", ["code", "label", "value"], rows)


def j5():
    d = FIX / "J5"
    d.mkdir(exist_ok=True)
    prov = [("Beijing", "北京市"), ("Shanghai", "上海市"), ("Guangdong", "广东省"), ("Sichuan", "四川省"), ("Inner Mongolia", "内蒙古自治区"), ("Xinjiang", "新疆维吾尔自治区"), ("Zhejiang", "浙江省"), ("Hubei", "湖北省")]
    write_geojson(d / "provinces.geojson", [({"name": en}, square(i * 1.2, 30)) for i, (en, _) in enumerate(prov)])
    # Short Chinese names (the crosswalk matches them) and one misspelt English name.
    table = [("北京", 21.9), ("上海", 24.9), ("广东", 126.6), ("四川", 83.7), ("内蒙古", 24.0), ("新疆", 25.9), ("浙江", 65.8), ("Hubel", 58.4)]
    write_csv(d / "population_zh.csv", ["province", "population_millions"], table)


if __name__ == "__main__":
    for f in (j1, j2, j3, j4, j5):
        f()
    print("wrote fixtures/J1..J5")
