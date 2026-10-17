"""Build the country crosswalk for joining tables (WP3.11).

Reads Natural Earth 1:10m admin-0 countries (public domain, v5.1.x) and writes
crates/lm-jobs/src/join/countries.csv: one row per country, keyed by its ISO 3166-1
alpha-3 code, with every name and code a table might use for it.

    python tools/lm/make_crosswalk.py path/to/ne_10m_admin_0_countries.shp

Download: https://naciscdn.org/naturalearth/10m/cultural/ne_10m_admin_0_countries.zip
"""

import sys
from pathlib import Path

import geopandas as gpd

# Natural Earth columns that hold a name or a code for the country (not SOVEREIGNT: it names
# the parent state on every territory, so "France" would stand for Réunion too).
NAME_COLUMNS = [
    "NAME", "NAME_LONG", "FORMAL_EN", "NAME_SORT", "NAME_ALT", "BRK_NAME", "GEOUNIT", "ADMIN",
    "NAME_EN", "NAME_ZH", "NAME_ZHT", "NAME_CIAWF", "ABBREV",
]
CODE_COLUMNS = ["ISO_A2_EH", "ISO_A3_EH", "ISO_N3_EH", "ADM0_A3", "WB_A2", "WB_A3", "GU_A3", "SU_A3"]

# Names in common tables that Natural Earth doesn't carry (World Bank and UN spellings).
EXTRA = {
    "TUR": ["Turkiye", "Türkiye"],
    "VNM": ["Viet Nam"],
    "KOR": ["Korea, Rep.", "Republic of Korea", "South Korea"],
    "PRK": ["Korea, Dem. People's Rep.", "Democratic People's Republic of Korea", "North Korea"],
    "RUS": ["Russian Federation"],
    "IRN": ["Iran, Islamic Rep.", "Iran (Islamic Republic of)"],
    "EGY": ["Egypt, Arab Rep."],
    "VEN": ["Venezuela, RB", "Venezuela (Bolivarian Republic of)"],
    "YEM": ["Yemen, Rep."],
    "COD": ["Congo, Dem. Rep.", "Democratic Republic of the Congo", "DR Congo", "DRC"],
    "COG": ["Congo, Rep.", "Republic of the Congo"],
    "GMB": ["Gambia, The"],
    "BHS": ["Bahamas, The"],
    "LAO": ["Lao PDR", "Lao People's Democratic Republic"],
    "KGZ": ["Kyrgyz Republic"],
    "SVK": ["Slovak Republic"],
    "SYR": ["Syrian Arab Republic"],
    "BRN": ["Brunei Darussalam"],
    "CPV": ["Cabo Verde", "Cape Verde"],
    "SWZ": ["Eswatini", "Swaziland"],
    "MKD": ["North Macedonia", "Macedonia"],
    "CZE": ["Czechia", "Czech Republic"],
    "CIV": ["Cote d'Ivoire", "Ivory Coast"],
    "TLS": ["Timor-Leste", "East Timor"],
    "FSM": ["Micronesia, Fed. Sts."],
    "HKG": ["Hong Kong SAR, China", "Hong Kong, China"],
    "MAC": ["Macao SAR, China", "Macau"],
    "PSE": ["West Bank and Gaza", "State of Palestine", "Palestine"],
    "USA": ["United States of America", "United States", "USA", "U.S.", "US"],
    "GBR": ["United Kingdom", "UK", "Great Britain"],
    "TZA": ["Tanzania, United Republic of", "United Republic of Tanzania"],
    "BOL": ["Bolivia (Plurinational State of)"],
    "MDA": ["Moldova, Republic of", "Republic of Moldova"],
    "LCA": ["St. Lucia", "Saint Lucia"],
    "VCT": ["St. Vincent and the Grenadines"],
    "KNA": ["St. Kitts and Nevis"],
    "MMR": ["Myanmar", "Burma"],
    "TWN": ["Taiwan, China", "Taiwan Province of China"],
    "CHN": ["中国", "China, People's Republic of"],
}


def clean(v):
    if v is None:
        return ""
    s = str(v).strip()
    return "" if s in ("-99", "-099", "nan", "None") else s


def main(shp):
    gdf = gpd.read_file(shp, ignore_geometry=True)
    rows = {}
    for _, r in gdf.iterrows():
        key = clean(r.get("ISO_A3_EH")) or clean(r.get("ADM0_A3"))
        if not key:
            continue
        names = rows.setdefault(key, [])
        for c in CODE_COLUMNS + NAME_COLUMNS:
            v = clean(r.get(c))
            if v and v != key and v not in names and "|" not in v and "," != v:
                names.append(v)
    for key, extra in EXTRA.items():
        names = rows.setdefault(key, [])
        names.extend(x for x in extra if x not in names)
    out = Path(__file__).resolve().parents[2] / "crates/lm-jobs/src/join/countries.csv"
    with out.open("w", encoding="utf-8") as f:
        f.write("key,names\n")
        for key in sorted(rows):
            f.write(f"{key},{'|'.join(n.replace(chr(10), ' ') for n in rows[key])}\n")
    print(f"{out}: {len(rows)} countries")


if __name__ == "__main__":
    main(sys.argv[1])
