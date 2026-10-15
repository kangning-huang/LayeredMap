# Bundled city index (place search)

`cities.tsv` is the offline index behind toolbar place search (WP1.5).

**Data from GeoNames (geonames.org), CC BY 4.0.** <https://www.geonames.org/>,
licence <https://creativecommons.org/licenses/by/4.0/>. Derived from `cities15000.zip`,
`admin1CodesASCII.txt` and `countryInfo.txt` at <https://download.geonames.org/export/dump/>. Changes: columns selected, region and
country codes replaced by names, one Simplified-Chinese name chosen from the alternate names
(heuristic documented in `tools/lm/build_city_index.py`), coordinates rounded to 5 decimals,
rows sorted by population.

| | |
|---|---|
| Source file date | 2026-10-15 |
| Built | 2026-10-15 |
| Rows | 34163 |
| Rows with a Chinese name | 12137 |
| Size | 2913185 bytes |
| SHA-256 of cities.tsv | `65b81ae07249918caa5e628cce20c56d41f7023f3fd6f700d968b03752e14590` |

Rebuild:

    python3 tools/lm/build_city_index.py --scratch /path/to/scratch

Columns (tab-separated, UTF-8, no header): geonameid, name, asciiname, zh, admin1 (region),
country, country code, lat, lon, population, feature code.
