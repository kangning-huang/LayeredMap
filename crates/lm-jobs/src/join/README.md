# Join crosswalks

Three tables of standard names and codes the join (WP3.11) uses as its loosest match level.
Each row is `key,names`: a canonical key, then every name or code a table might use for that
place, separated by `|`. Names that two places share in one table are ignored at load.

| File | Key | Names and codes | Source | Licence |
|---|---|---|---|---|
| `countries.csv` (252 rows) | ISO 3166-1 alpha-3 | ISO alpha-2 and numeric codes, World Bank codes, Natural Earth's names (short, long, formal, sort name, alternates, Chinese), plus World Bank and UN spellings listed in `tools/lm/make_crosswalk.py` | Natural Earth 1:10m admin-0 countries v5.1.x, `naciscdn.org/naturalearth/10m/cultural/ne_10m_admin_0_countries.zip` | Public domain |
| `us_states.csv` (52 rows) | 2-digit FIPS code | State name, USPS abbreviation (50 states, DC, Puerto Rico) | US Census Bureau FIPS state codes and USPS abbreviations, typed by hand | Public domain (facts) |
| `cn_provinces.csv` (34 rows) | 6-digit GB/T 2260 code | Chinese name with and without its suffix, English and pinyin names | GB/T 2260 province-level divisions, typed by hand | Facts |

To rebuild `countries.csv`, download the Natural Earth file above, unzip it, and run
`python tools/lm/make_crosswalk.py path/to/ne_10m_admin_0_countries.shp` (needs geopandas).
Generated 2026-10-17 from the download of that day; nothing else was transformed.
