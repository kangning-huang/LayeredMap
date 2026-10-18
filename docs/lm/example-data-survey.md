# Example data survey: what to put in "Try an example"

2026-10-18. The owner's goal is for users to think "I can't do that on Google Maps or Apple Maps". The Pro tier already carries Natural Earth (countries, provinces, cities, rivers, lakes, coastline, land, islands, regions, seas) and WorldClim elevation as hidden reference layers. So a good example is *the user's kind of data* that turns interesting once it meets those layers. Downloads, licences and trial trims are logged in `/Volumes/LaCie/data/layeredmap-example-survey/README.md` (outside the repo). The sample answers below come from real joins against Natural Earth 1:10m, run with geopandas.

## Shortlist

| # | Dataset | Licence | Size (trimmed) | Coverage | Wow angle | Best Ask request |
|---|---|---|---|---|---|---|
| 1 | [USGS earthquakes M6+, 1900–2026](https://earthquake.usgs.gov/fdsnws/event/1/) | US public domain (credit USGS) | 14,537 points, 1.0 MB CSV | Global; 322 on land in China | Taipei has had 197 M6+ quakes within 200 km | "Which capital has had the most big earthquakes within 200 km?" |
| 2 | [OurAirports](https://ourairports.com/data/) (large + medium) | Public domain | 5,280 points, 0.45 MB CSV | Global; 334 in China | 18 of the world's 29 big airports above 3,000 m are in China | "Which airports are higher than 4,000 m?" |
| 3 | [NOAA NCEI volcano locations](https://www.ncei.noaa.gov/products/natural-hazards/tsunamis-earthquakes-volcanoes/volcanoes) + 903 significant eruptions | NOAA, no restriction stated; cite doi:10.7289/V5JW8BSH | 1,609 points, 0.1 MB CSV | Global | 77 volcanoes stand above 5,000 m; 849 lie within 50 km of the sea | "Which country has the most volcanoes?" |
| 4 | [GHS-POP R2023A population grid](https://human-settlement.emergency.copernicus.eu/) (JRC) | CC BY 4.0 | 461 MB zip at 30″; about 5–10 MB at 0.1° (estimate) | Global | Lets any layer ask "how many people…" | "How many people live within 100 km of a volcano?" |
| 5 | [IBTrACS West Pacific typhoons since 1980](https://www.ncei.noaa.gov/products/international-best-track-archive) | NOAA/WMO "full and open"; commercial use under WMO Res. 40 | 1,537 tracks, 3.2 MB GeoJSON after dissolving | West Pacific, China coast | 181 typhoons have crossed Guangdong since 1980 | "Which typhoon came closest to Shanghai?" |
| 6 | [Solar farms in China, 2020](https://zenodo.org/records/6849477) (Zenodo; 3,566 views) | CC BY 4.0 | 2,539 polygons, 4.9 MB GeoJSON | China | Jiangsu has the most farms, but Qinghai has the most land under panels | "Which province has the most solar farms?" |
| 7 | [NCEI Significant Earthquakes](https://www.ncei.noaa.gov/products/natural-hazards/tsunamis-earthquakes-volcanoes/earthquakes) | NOAA; cite doi:10.7289/V5TD9V7K | 6,697 points (2,144 with deaths), ~1 MB | Global, 2150 BC to now; 626 in China | Deadliest ever: Shaanxi 1556, about 830,000 dead | "Which earthquakes in China killed more than 10,000 people?" |
| 8 | [OSM railway stations in China](https://www.openstreetmap.org/copyright) (Overpass) | ODbL 1.0 (© OpenStreetMap contributors; share-alike) | 10,350 points, ~1 MB | China; Chinese names | Tanggula, at 5,068 m, is the world's highest station | "Which railway stations are above 4,000 m?" |
| 9 | [WRI Global Power Plant Database v1.3](https://datasets.wri.org/datasets/global-power-plant-database) | CC BY 4.0 | 34,936 points, 2.2 MB CSV | Global; 4,235 in China | 82 of 195 nuclear plants sit within 10 km of the sea | "Which country has the most nuclear power plants?" |
| 10 | [Glacial Lake Outburst Flood Database v4.2](https://zenodo.org/records/23370149) (Zenodo; 4,145 views) | CC BY 4.0 | 1.5 MB .ods (19 regional sheets) | Global; strong in High Mountain Asia | Mapped history of glacier floods | "Which country has had the most glacier floods?" |

**What the current actions can answer.** Requests 1, 2, 3, 6, 7, 8, 9 and 10 use count-in-areas, buffer, filter, rank, nearest distance and join-by-location, which all exist. Request 4 is a buffer followed by a raster sum in areas, which also exists, but only once the raster is prepared. Request 5 needs nearest distance to a city point, which exists. The headline typhoon question ("which province do typhoons cross most?") needs counting the lines that *cross* an area, which is not there yet (see Gaps).

**Data prep.**
- Earthquakes: keep 6 columns.
- Airports: keep large and medium airports, convert feet to metres, keep 8 columns.
- Volcanoes: JSON from the API, merged to CSV.
- GHS-POP: sum-aggregate to 0.1° and save as a compressed GeoTIFF.
- IBTrACS: dissolve the 3-hourly segments into one line per storm (`SID`), keep name, season and peak wind, simplify by 0.05°, round coordinates to 3 decimals.
- Solar farms: simplify by 0.0002° and round to 5 decimals.
- OSM stations: convert to CSV with `name` and `name:en`.
- GLOF database: merge its sheets into one CSV with lon/lat.

## Demo scripts for the top 3

**1. Earthquakes: "the shaking map"** (open `earthquakes_m6.csv`)
1. "Which country has had the most magnitude 7+ earthquakes?" → countries shade, and Chile (51), Mexico (43) and China (42) top the ranking.
2. "Which capital has had the most magnitude 6+ quakes within 200 km?" → 200 km rings around capitals; Taipei (197) wins, then Port Vila, Nuku'alofa and Tokyo.
3. "Show the big ones within 100 km of the coast." → the Ring of Fire lights up as a coastline.

*Mei:* "Taipei beats Tokyo? I'm sending this to my cousin there." *Skeptic:* Google and Apple Maps show no earthquake history at all, and the USGS map can't count by country or draw rings around capitals. The data is public domain, kept current by USGS, and 1 MB. One caveat: 76% of M6+ quakes are offshore, so the per-country counts cover land only. The app should say so, or offer "nearest country" (see Gaps).

**2. Airports: "the roof of the world"** (open `airports.csv`)
1. "Which airports are higher than 4,000 m?" → seven dots: six in Tibet and western Sichuan (Daocheng Yading at 4,411 m, Qamdo Bangda, Shigatse Tingri, Kangding, Ngari Gunsa) plus El Alto in Bolivia.
2. "Which province are they in?" → join-by-location adds `name_zh` (四川省, 西藏自治区), so the answers come with Chinese names.
3. "Which big airport is farthest from the sea?" → nearest distance to the coastline, ranked; the top five are all in Gansu and Xinjiang, about 1,800–1,900 km from the sea. The very top is the Dingxin air base, a military field; filtering on `iata_code` keeps it to civil airports.
4. "Which province has the most airports?"

*Mei:* "Six of the seven highest are in China. That's a 小红书 post." *Skeptic:* consumer map apps can find an airport but can't filter by elevation or measure distance to the sea. The data is public domain, updated nightly and 0.45 MB. OurAirports elevations are crowd-sourced but match published figures for the top cases.

**3. Volcanoes: "who lives next to a volcano"** (open `volcanoes.csv`, then the population grid)
1. "Which country has the most volcanoes?" → Russia (156), the USA (147), Indonesia (129).
2. "Which volcanoes are higher than 5,000 m?" → 77 dots along the Andes, Kilimanjaro, Elbrus and Damavand.
3. "Which capital is closest to a volcano?" → nearest distance, ranked.
4. With the population grid: "How many people live within 100 km of a volcano?" → buffers, then a raster sum, gives one big number and a ranking by country.

*Mei:* "Step 4 is the one I'd screenshot." *Skeptic:* no consumer map can sum people inside a shape. NCEI's list derives from the Smithsonian's but is US-government work. Step 4 depends on preparing GHS-POP; without it, stop at step 3.

## Recommendation for "Try an example"

Replace cities, countries and elevation (they are now hidden reference layers) with:

1. **`earthquakes_m6.csv`** (USGS, 1.0 MB). It works for every audience, and every request uses built-in layers.
2. **`airports.csv`** (OurAirports, 0.45 MB). It has the strongest China reveal and is public domain.
3. **`volcanoes.csv`** (NCEI, 0.1 MB).
4. Optional, for the China focus: **`typhoons_wp_1980.geojson`** (IBTrACS, 3.2 MB), once line-crossing counts exist. Until then, its best questions are "Which typhoons had winds over 150 knots?" and "Which typhoon came closest to Shanghai?".

Total about 5 MB. Ship GHS-POP at 0.1° as a separate Pro download (it unlocks the "how many people" requests for every layer). Credit lines: "U.S. Geological Survey"; "OurAirports (public domain)"; "NOAA NCEI, doi:10.7289/V5JW8BSH"; for IBTrACS, the Knapp et al. 2010 and Gahtan et al. 2024 citations.

## Tempting but rejected

- **Smithsonian GVP Volcanoes of the World**: the Smithsonian Terms of Use invite non-commercial use, and redistribution in a paid app is unclear. Use NCEI instead, or ask GVP for permission.
- **UNESCO World Heritage list** (official XLS/KML): republishing needs UNESCO's prior written permission. The Wikidata copy is CC0, but its 3,378 items include about 1,468 Brazilian rows that look mis-tagged, so it would need heavy cleaning.
- **NASA Meteorite Landings**: data.nasa.gov says "License not specified", and the source is the Meteoritical Society.
- **TeleGeography submarine cables**: the map is CC BY-SA, but the underlying data needs a paid licence, and the old GitHub copy was deleted in 2022.
- **CHGIS (Chinese historical GIS)**: "no commercial use, resale, or redistribution".
- **Global Ski Resort Rankings** (Zenodo, CC BY as stated): it has no China rows and appears to be compiled from third-party sites. OpenSkiMap (ODbL) is a cleaner alternative, but large.
- **GBIF giant pandas**: only 61 records have coordinates (sensitive species), and 34 of those are CC BY-NC.
- **Global Dam Tracker** (CC BY): only 1,168 dams in China, heights stored as text, and a 361 MB .dbf.
- **Solar Asset Mapper** (CC BY-NC); **Kruitwagen global solar inventory** (CC BY, but 274 MB of predictions).
- **VIIRS night lights (EOG)**: needs registration, and the licence varies by product. NASA Black Marble is open but needs an Earthdata login and heavy aggregation. It is a good later candidate for "Which province is brightest at night?".
- **Global Energy Monitor trackers**: CC BY 4.0 by their pages and current (Sept 2026), but downloads go through a form, with a "commercial exemption" process. Confirm before bundling.
- **WRI power plants** made the list but is frozen at 2021. GEM would be the fresher replacement if its terms check out.

## Gaps: requests that would wow but need a missing action

1. **Count the lines that cross each area** (distinct count): "Which province do typhoons cross most often?" → Guangdong 181, Guangxi 110, Fujian 95. Today lines are counted by one point inside them.
2. **Sum a field within areas**: "Which province has the most land under solar panels?" → Qinghai 294 km², although Jiangsu has the most farms. The same action covers "Which country has the most nuclear megawatts?".
3. **Line length within areas**: "Which province has the most kilometres of high-speed rail?" OSM has 90,537 HSR ways in China, which would need prep.
4. **Nearest area for points off the coast**: "Which country's coast had the most quakes?" Three quarters of earthquakes fall outside every country polygon.
5. **Time**: "How many big quakes per decade?" or a time slider. No group-by-year action exists.
6. **Raster along a line**: "What's the highest point the Qinghai–Tibet railway crosses?"
7. **Live feeds**: USGS past-week quakes or NASA FIRMS fires, re-fetched on open, would beat any static example for "wow", but they need network fetching and a cache.
