# "Data to explore" page: research record

Pages: `web/lm/data.html` and its ten translations (`web/lm/<lang>/data.html`; styles in `web/lm/assets/data.css`).
Every link below was fetched on **2026-10-19**. Where I could, I also downloaded the file or
read its header to confirm it has coordinates or geometry and to count rows. The facts in each
one-liner come from the source page or from the data file itself; where they come from
somewhere else, the "Facts" line says so.

Flags used on the page: **account** (needs a free account), **network** (may need a network
outside mainland China), **large** (over about 75 MB, or several GB at most).

A note on coordinates: NimbleMap finds coordinate columns by name (`lat`/`latitude`,
`lon`/`lng`/`long`/`longitude`, also inside names like `location-lat`; see `crates/data/src/io.rs`
and `crates/lm-profile/src/csv.rs`). When a dataset uses names it won't guess (`reclat`, `xlong`,
`representative_latitude`), the page names the columns to choose.

## One-click downloads (checked 2026-10-20)

Each card's button downloads the file itself when the source has a stable direct URL; otherwise it
opens the source's download page and a short note says why and what to click. Every direct URL was
checked on **2026-10-20** with `curl -sIL` (status 200 after redirects, content type, length), with a
ranged GET where HEAD gave no length or was refused (Dataverse's signed S3 redirects and Zenodo
answer HEAD differently; Zenodo also returns 403 to some scripted user agents but serves browsers).
Copies were downloaded to `/Volumes/LaCie/lm-data-previews/` (README there), and the ones marked
"yes" were opened in the 0.7.0 release build with `--tour` to confirm they load. No source link was
dead; simplemaps, GBIF and UNESCO answer scripts with a Cloudflare 403 but load in a browser.

Sizes on the buttons are decimal MB, rounded. Every direct card also links its source page.

| # | Dataset | Button | Direct URL | Content-Type | Bytes | Opened in app |
|---|---|---|---|---|---|---|
| 1 | Earthquakes, live | CSV · 2 MB | https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_month.csv | text/csv | 2,005,531 (GET; HEAD gives no length) | yes |
| 2 | Fires (FIRMS) | CSV · 0.8 MB | https://firms.modaps.eosdis.nasa.gov/data/active_fire/modis-c6.1/csv/MODIS_C6_1_Global_24h.csv | application/octet-stream | 810,361 | yes |
| 3 | US tornadoes | Shapefile zip · 5 MB | https://www.spc.noaa.gov/gis/svrgis/zipped/1950-2025-torn-aspath.zip | application/zip | 4,968,330 | yes |
| 4 | GISTEMP | NetCDF .gz · 26 MB | https://data.giss.nasa.gov/pub/gistemp/gistemp1200_GHCNv4_ERSSTv5.nc.gz | application/x-gzip | 25,866,289 | yes |
| 5 | ERSST v5 | NetCDF · 161 MB | https://downloads.psl.noaa.gov/Datasets/noaa.ersst.v5/sst.mnmean.nc | application/x-netcdf | 160,754,608 | yes |
| 6 | GPCP | NetCDF · 20 MB | https://downloads.psl.noaa.gov/Datasets/gpcp/precip.mon.mean.nc | application/x-netcdf | 20,187,988 | yes |
| 7 | US wind turbines | GeoJSON zip · 1.4 MB | https://energy.usgs.gov/uswtdb/assets/data/uswtdbGeoJSON.zip | application/x-zip-compressed | 1,447,465 | yes |
| 8 | GeoNuclearData | CSV · 110 KB | https://raw.githubusercontent.com/cristianst85/GeoNuclearData/master/data/csv/denormalized/nuclear_power_plants.csv | text/plain | 108,458 | yes |
| 9 | Solar PV inventory | GeoJSON · 17 MB | https://zenodo.org/records/5005868/files/trn_polygons.geojson?download=1 | application/octet-stream | 17,227,983 | yes |
| 10 | Mining polygons | GeoPackage · 25 MB | https://download.pangaea.de/dataset/942325/files/global_mining_polygons_v2.gpkg | application/octet-stream | 24,657,920 | yes |
| 11 | simplemaps cities | page link | — (Cloudflare browser check; the zip's name carries the version number (no stable URL)) | | | |
| 12 | OWID life expectancy | CSV · 0.6 MB | https://ourworldindata.org/grapher/life-expectancy.csv | text/csv | 605,250 | not tried |
| 13 | WorldPop | GeoTIFF · 49 MB | https://data.worldpop.org/GIS/Population/Global_2000_2020_1km_UNadj/2020/CHN/chn_ppp_2020_1km_Aggregated_UNadj.tif | image/tiff | 49,445,858 | yes |
| 14 | OurAirports | CSV · 13 MB | https://davidmegginson.github.io/ourairports-data/airports.csv | text/csv | 12,748,669 | yes |
| 15 | World Port Index | CSV · 3.5 MB | https://msi.nga.mil/api/publications/download?key=16920959/SFH00000/UpdatedPub150.csv&type=download | application/octet-stream | 3,508,891 | yes |
| 16 | Japan railways N02 | zip · 13 MB | https://nlftp.mlit.go.jp/ksj/gml/data/N02/N02-24/N02-24_GML.zip | application/zip | 12,718,026 | yes |
| 17 | UNESCO World Heritage | page link | — (Cloudflare browser check (403 to scripts); © UNESCO terms) | | | |
| 18 | Pleiades | zip · 36 MB | https://atlantides.org/downloads/pleiades/gis/pleiades_gis_data.zip | application/zip | 35,671,726 | not tried |
| 19 | Itiner-e roads | GeoPackage · 34 MB | https://zenodo.org/records/17122148/files/itinere_roads.gpkg?download=1 | application/octet-stream | 33,619,968 | yes |
| 20 | CHGIS county seats | Shapefile zip · 545 KB | https://dataverse.harvard.edu/api/access/datafile/3048165 | application/zipped-shapefile | 544,703 (ranged GET) | yes |
| 21 | BGIS temples | Shapefile zip · 1.5 MB | https://dataverse.harvard.edu/api/access/datafile/3994640 | application/zipped-shapefile | 1,547,207 (ranged GET) | yes |
| 22 | Castles (Wikidata) | page link | — (Query builder: the file is made when you press Run) | | | |
| 23 | GBIF | page link | — (Free account needed (download API returns 403 without login)) | | | |
| 24 | White storks | CSV · 54 MB | https://datarepository.movebank.org/server/api/core/bitstreams/f0147626-8366-4b23-9a55-b2a76e75acbc/content | application/octet-stream | 53,576,363 (ranged GET) | yes |
| 25 | NYC street trees | CSV · 220 MB | https://data.cityofnewyork.us/api/views/uvpi-gqnh/rows.csv?accessType=DOWNLOAD | text/csv | 220,354,362 (full GET; streamed, no length) | yes |
| 26 | Hansen forest loss | GeoTIFF · 100 MB | https://storage.googleapis.com/earthenginepartners-hansen/GFC-2025-v1.13/Hansen_GFC-2025-v1.13_lossyear_30N_110E.tif | image/tiff | 100,173,392 | yes |
| 27 | Meteorites | CSV · 4 MB | https://data.nasa.gov/docs/legacy/meteorite_landings/Meteorite_Landings.csv | text/csv | 3,952,161 (ranged GET) | yes |
| 28 | 2024 eclipse | Shapefile zip · 79 MB | https://svs.gsfc.nasa.gov/vis/a000000/a005000/a005073/2024eclipse_shapefiles.zip | application/zip | 79,468,554 | yes |
| 29 | Black Marble | GeoTIFF · 64 MB | https://assets.science.nasa.gov/content/dam/science/esd/eo/images/imagerecords/144000/144898/BlackMarble_2016_3km_geo.tif | image/tiff | 64,383,740 | yes |
| 30 | All the Places | GeoJSON · 13 MB | https://data.alltheplaces.xyz/runs/latest/output/starbucks_us.geojson | application/geo+json | 13,124,933 | yes |
| 31 | overpass turbo | page link | — (Query builder) | | | |
| 32 | BBBike | page link | — (Made to order, link sent by email) | | | |
| 33 | Geofabrik | Shapefile zip · 263 MB | https://download.geofabrik.de/asia/taiwan-latest-free.shp.zip | application/zip | 262,956,628 | not tried |
| 34 | Singapore hawker centres | page link | — (data.gov.sg issues a signed S3 link that expires after 1 hour) | | | |

Findings from the check:
- **Solar PV (Kruitwagen):** `predicted_set.geojson` contains `NaN` values (`"ind_comm_10km": [ NaN ]`),
  which is not valid JSON, so NimbleMap refuses it ("isn't valid GeoJSON"). The button now gets
  `trn_polygons.geojson` (17 MB, 36,882 polygons, no NaN; opens), and the card says why. Accepting
  NaN in the GeoJSON reader would let the full file open.
- **White storks:** the CSV opens, but NimbleMap's column question recommends `argos:lon1/argos:lat1`;
  the card tells people to pick `location-long / location-lat` (the Movebank standard columns).
- **Meteorites:** opens as a table until `reclat`/`reclong` are chosen, as the card already says.
- Sizes that changed since 2026-10-19: GISTEMP .nc.gz 11 → 26 MB; 2024 eclipse shapefile zip
  38 → 79 MB; NYC tree CSV export 85 → 220 MB (same 683,788 rows; the export now carries more columns);
  storks CSV 51 → 54 MB (decimal); WorldPop: the UN-adjusted China 2020 file (the listing's own
  product) is 49 MB.
- GitHub raw serves `text/plain`, and some CSVs may show as text in a browser; the page's tip says
  to press ⌘S.

The card pictures are field-journal pages in the manner of John Muir Laws's nature journaling (pen
and watercolour, a main sketch with zoomed insets, leader lines, arrows and scale bars), not data
previews: `web/lm/img/data-laws/*.svg`, written by `tools/lm/site/data_art_laws.py` from the kit in
`tools/lm/site/dsart_laws/` (320×180, about 11–52 KB each, no words, letters or digits, since the page
is in 11 languages). Each is a light page; `data.css` dims it in dark mode. The earlier set of
journal-figure plates (cut-away block diagrams, sections, globes) is kept as an alternative:
`web/lm/img/data/*.svg`, from `tools/lm/site/data_art.py` and `tools/lm/site/dsart/`.

## Map data you already have (intro section, not counted)

| Source | URL checked | What was confirmed |
|---|---|---|
| Strava single-activity GPX | https://support.strava.com/hc/en-us/articles/216918437-Exporting-your-Data-and-Bulk-Export | "Export GPX" in the activity's ⋯ menu; bulk export by email |
| Garmin Connect | https://support.garmin.com/en-US/?faq=W1TvTPW8JZ6LfJSfK512Q8 | Web: Activities › activity › gear icon › Export to GPX (also TCX, FIT, CSV splits) |
| Apple Health | https://support.apple.com/guide/iphone/share-your-health-data-iph5ede58c3d/ios | Summary › picture › Export All Health Data (XML). The `workout-routes` GPX folder inside the zip comes from third-party write-ups (iGeeksBlog, rasterweb.net), not Apple |
| Google Takeout | https://takeout.google.com/ | Page loads. "Maps (your places)" › `Saved Places.json` as a GeoJSON FeatureCollection comes from community sources (json2kml, gists). Timeline is now stored on the phone and exports a plain JSON file (Android Authority, Google community threads). NimbleMap's GeoJSON reader accepts `.json` that has a `"type"` key (`crates/lm-jobs/src/import/geojson.rs`). Flagged as needing a non-mainland network in Chinese |

## Datasets (34, in 9 sections)

### Earth & hazards (3)

1. **Earthquakes, live**: https://earthquake.usgs.gov/earthquakes/feed/v1.0/csv.php
   - **Format and size:** CSV (GeoJSON feeds also exist). `all_month.csv` was 9,878 rows / 2.0 MB on the check date.
   - **Licence:** public domain (USGS).
   - **Coordinates:** `latitude`, `longitude`.
   - **Facts:** "Updated every minute" (past hour and past day feeds); feeds for past hour, day, week and 30 days.
2. **Fires burning right now (NASA FIRMS)**: https://firms.modaps.eosdis.nasa.gov/active_fire/
   - **Format and size:** CSV, shapefile or KML. The MODIS global 24 h CSV had 10,989 rows (0.85 MB) and downloads without login; only the archive needs an Earthdata login.
   - **Licence:** NASA open data.
   - **Coordinates:** `latitude`, `longitude`. Other fields include `frp`, `daynight`.
   - **Facts:** files for the last 24 h, 48 h and 7 days, as shapefile, KML or text.
3. **Every US tornado since 1950 (NOAA SPC SVRGIS)**: https://www.spc.noaa.gov/gis/svrgis/
   - **Format and size:** shapefile zip, `1950-2025-torn-aspath.zip`, 4.97 MB, 73,458 line features.
   - **Licence:** public domain (NOAA).
   - **Coordinates:** the paths are lines (fields `slat slon elat elon`).
   - **Facts:** fields `mag inj fat len wid`. Hail (1955–2025) and wind files are on the same page.

### Climate & oceans (3)

4. **Global temperature anomalies (GISTEMP v4)**: https://data.giss.nasa.gov/gistemp/
   - **Format and size:** `gistemp1200_GHCNv4_ERSSTv5.nc.gz`, 11.4 MB.
   - **Licence:** NASA. Credit "NASA GISS/GISTEMP" (page wording).
   - **Coordinates:** a 2°×2° grid of monthly anomalies.
   - **Facts:** "netCDF data files on a regular 2°×2° regular grid". The 1951–1980 base period is stated on the page for the LOTI tables and is the standard GISTEMP baseline; the page doesn't restate it for the grids. I didn't claim a start year, because the page doesn't give one.
5. **Sea surface temperature since 1854 (ERSST v5)**: https://psl.noaa.gov/data/gridded/data.noaa.ersst.v5.html
   - **Format and size:** NetCDF-4 `sst.mnmean.nc`, 160.8 MB (HEAD, last modified 2026-10-03). NimbleMap reads NetCDF-4 (`docs/lm/netcdf-spike.md`). Flagged large.
   - **Licence:** NOAA. PSL asks for acknowledgement.
   - **Facts:** from the NCEI ERSST page (https://www.ncei.noaa.gov/products/extended-reconstructed-sst): January 1854 to present, 2°×2°, ships and buoys (ICOADS) plus Argo floats. File title: "NOAA ERSSTv5 (in situ only)". "El Niño" is offered as a suggestion of what to look for, not as a sourced fact.
6. **Rain and snow since 1979 (GPCP v2.3)**: https://psl.noaa.gov/data/gridded/data.gpcp.html
   - **Format and size:** NetCDF-4 `precip.mon.mean.nc`, 20.2 MB.
   - **Unit:** mm/day (from the file header).
   - **Licence:** NOAA.
   - **Facts:** from the NCEI GPCP CDR page: January 1979 to present, 2.5° grid, satellites plus gauges. The monsoon remark is a suggestion.

### Energy & industry (4)

7. **US Wind Turbine Database v9.1**: https://energy.usgs.gov/uswtdb/data/
   - **Format and size:** shapefile (~7 MB), GeoJSON or CSV zip (~2 MB). The CSV has 77,839 rows; `p_year` runs 1982–2026.
   - **Licence:** USGS public domain. The page gives a citation only and no licence statement; USGS data releases are public domain.
   - **Coordinates:** `xlong`, `ylat`. The GeoJSON avoids picking columns.
   - **Facts:** version 9.1 dated 28 Sep 2026.
8. **GeoNuclearData**: https://github.com/cristianst85/GeoNuclearData
   - **Format and size:** CSV and JSON, 804 reactors (414 operating, 79 planned, 61 under construction, 209 shut down). The denormalized CSV has 805 lines.
   - **Licence:** ODbL 1.0, contents DbCL.
   - **Coordinates:** `Latitude`, `Longitude`.
   - **Facts:** last data update 2024-03-03 (v0.17.17).
   - **Flag:** network (GitHub raw files can be unreliable in mainland China).
9. **Global solar PV inventory (Kruitwagen et al. 2021)**: https://zenodo.org/records/5005868
   - **Format and size:** `predicted_set.geojson`, 274.5 MB (zip 82.5 MB). Flagged large.
   - **Licence:** CC BY 4.0.
   - **Facts:** 68,661 polygons, status at end of 2018.
10. **Global-scale mining polygons v2 (Maus et al. 2022)**: https://doi.org/10.1594/PANGAEA.942325
    - **Format and size:** `global_mining_polygons_v2.gpkg`, 23.5 MB. It downloads directly without an account (HEAD 200); only `allfiles.zip` needs a PANGAEA account.
    - **Licence:** CC BY-SA 4.0.
    - **Facts:** 44,929 features, 101,583 km², open cuts, tailing dams, waste rock dumps and more, traced by visual interpretation of satellite images. Field `AREA`.

### Cities & people (3)

11. **simplemaps World Cities Basic**: https://simplemaps.com/data/world-cities
    - **Format and size:** CSV and Excel, 50.2 thousand entries, last refreshed 30 Aug 2026.
    - **Licence:** CC BY 4.0, attribution required.
    - **Coordinates:** `lat`, `lng`.
    - **Facts:** fields include `population`, `capital`, `iso3`. The page is behind a Cloudflare bot check, so it was read in a real browser.
12. **Life expectancy (Our World in Data)**: https://ourworldindata.org/grapher/life-expectancy
    - **Format and size:** CSV `life-expectancy.csv`, 21,564 rows, columns `Entity, Code, Year, Life expectancy`.
    - **Coordinates:** none. Join on `Code` (ISO3).
    - **Licence:** CC BY (OWID FAQ).
    - **Facts:** timespan 1543–2023, and "In 1900, the global average life expectancy was 32 years. By 2023 … 73" (from the metadata JSON).
13. **WorldPop 1 km population**: https://hub.worldpop.org/geodata/listing?id=75
    - **Format and size:** GeoTIFF. China 2020 `chn_ppp_2020_1km_Aggregated.tif` is 52.1 MB.
    - **Licence:** CC BY 4.0.
    - **Facts:** "Unconstrained individual countries 2000-2020 UN adjusted (1km resolution)", people per pixel. The Hu Line remark is a suggestion.

### Transport (3)

14. **OurAirports**: https://github.com/davidmegginson/ourairports-data
    - ourairports.com/data returned HTTP 500 on the check date, so the page links the official daily GitHub mirror, which the site itself points to.
    - **Format and size:** `airports.csv`, 86,258 rows, 12.7 MB.
    - **Licence:** Unlicense (public domain).
    - **Coordinates:** `latitude_deg`, `longitude_deg`.
    - **Facts:** type counts small_airport 42,820, heliport 23,248, closed 13,574, seaplane_base 1,273, balloonport 63; "updated daily" (README).
    - **Flag:** network.
15. **World Port Index (NGA Pub 150)**: https://msi.nga.mil/Publications/WPI
    - **Format and size:** `UpdatedPub150.csv`, 3,807 ports, updated monthly.
    - **Licence:** public domain (US Government).
    - **Coordinates:** `Latitude`, `Longitude`.
    - **Facts:** "over 100 key characteristics" per port; columns include `Channel Depth (m)`, `Harbor Size`, `Maximum Vessel Length (m)`.
16. **Japan railways N02 (国土数値情報)**: https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N02-2024.html
    - **Format and size:** `N02-24_GML.zip`, 12.7 MB. It contains `UTF-8/N02-24_RailroadSection.geojson` (21,932 features) and `N02-24_Station.geojson` (10,235 features, about 8,500 distinct names), plus shapefiles.
    - **Licence:** CC BY 4.0 for data from 2020 onwards.
    - **Facts:** passenger rail and tram lines with operator (`N02_004`) and line name.

### History & culture (6)

17. **UNESCO World Heritage List**: https://whc.unesco.org/en/syndication
    - **Format and size:** XLSX (2.9 MB, fetched once in a browser) and XLS; the XML feed has 1,273 rows.
    - **Facts:** category counts Cultural 991, Natural 240, Mixed 42; fields `category`, `date_inscribed`, `danger`, and per-site `latitude`/`longitude` in the XML.
    - **Not confirmed:** the XLSX column headers. Cloudflare blocked a second binary fetch, so I couldn't open the file; it is UNESCO's spreadsheet of the same list.
    - **Licence:** © UNESCO. The terms allow personal, non-commercial use and forbid republication, so the page links only and says "personal use".
18. **Pleiades**: https://pleiades.stoa.org/downloads (GIS package https://atlantides.org/downloads/pleiades/gis/)
    - **Format and size:** `pleiades_gis_data.zip`, 35.7 MB. `places.csv` has 42,404 places, 34,823 with representative coordinates.
    - **Licence:** CC BY 3.0.
    - **Coordinates:** `representative_latitude`, `representative_longitude`.
    - **Facts:** "updated several times each week".
19. **Itiner-e Roman roads (2024 static version)**: https://zenodo.org/records/17122148
    - **Format and size:** `itinere_roads.gpkg` 33.6 MB, `.geojson` 78.1 MB, plus a shapefile.
    - **Licence:** CC BY 4.0.
    - **Facts:** ca. 300 BCE–300 CE; 299,171 km of roads, "almost double" comparable resources (Scientific Data 12, 1731, 2025, via the paper as quoted in search results).
20. **CHGIS V6 Time Series County Points**: https://doi.org/10.7910/DVN/Q9VOF5
    - **Format and size:** `v6_time_cnty_pts_utf_wgs84.zip`, 0.54 MB shapefile, 10,522 records.
    - **Fields:** `NAME_CH`, `NAME_PY`, `BEG_YR`, `END_YR` and more.
    - **Licence:** Dataverse lists CC0, but the README says "free for academic research, no commercial use, resale, or redistribution". The page states the stricter wording.
    - **Facts:** covers 221 BCE–1911 (Dataverse description, "mostly complete 1350–1911").
21. **BGIS Buddhist temples of China**: https://doi.org/10.7910/DVN/VAYEUZ
    - **Format and size:** shapefile zip 1.5 MB plus XLS/ODS, 18,938 features.
    - **Fields:** `NM_HZ`, `TYPE_ENG`, `PROV_HZ`, `LAT`, `LONG` and more.
    - **Licence:** CC0 (Dataverse).
    - **Facts:** source is the 2006 Zhongguo Fojiao siyuan minglu (README).
22. **Castles from Wikidata**: query link on the page
    - **Query:** `?castle wdt:P31 wd:Q23413; wdt:P625 ?geometry`, plus an optional English label.
    - **Result:** run against the SPARQL endpoint it returned 22,935 rows in 29 s, with a `geometry` column of `Point(lon lat)` WKT, which NimbleMap's WKT detection reads.
    - **Licence:** CC0.
    - **Flag:** network (Wikidata is not reachable from mainland China).

### Nature & wildlife (4)

23. **GBIF occurrences**: https://www.gbif.org/occurrence/search
    - **Count:** 3,982,839,502 records (API, check date).
    - **Account:** downloads need a free account; an unauthenticated download request returned 403.
    - **Format:** SIMPLE_CSV is tab-separated, with `decimalLatitude`/`decimalLongitude`.
    - **Licence:** per dataset (CC0, CC BY, CC BY-NC).
    - The site is behind Cloudflare, so it was checked through the API.
24. **MPIAB Argos white stork tracking (1991–2017)**: https://doi.org/10.5441/001/1.k29d81dh
    - **Format and size:** argos CSV, 51.09 MB.
    - **Licence:** CC0 1.0.
    - **Facts:** over 200 storks tagged in Belgium, Germany, Greece, Israel, Poland, Russia, South Africa, Spain and Switzerland; "Prinzesschen, tracked for over a decade".
    - **Not confirmed:** the column names `location-lat`, `location-long`, `individual-local-identifier` are the standard Movebank columns; I did not open this file.
25. **NYC 2015 Street Tree Census**: https://data.cityofnewyork.us/Environment/2015-Street-Tree-Census-Tree-Data/uvpi-gqnh
    - **Format and size:** 683,788 rows (Socrata count); the CSV export was 85 MB. Flagged large.
    - **Fields:** `latitude`, `longitude`, `spc_common`, `health`, `tree_dbh`.
    - **Licence:** NYC Open Data terms (free use).
26. **Hansen Global Forest Change v1.13**: https://storage.googleapis.com/earthenginepartners-hansen/GFC-2025-v1.13/download.html
    - **Format and size:** 10°×10° GeoTIFF tiles, about 100 MB each (the `lossyear_30N_110E` tile is 100.2 MB), 30 m pixels.
    - **Facts:** loss years 2001–2025.
    - **Licence:** CC BY 4.0. Credit "Hansen/UMD/Google/USGS/NASA".
    - **Flags:** large, network (Google Cloud Storage).

### Space & curiosities (3)

27. **Meteorite Landings**: https://data.nasa.gov/dataset/meteorite-landings
    - **File:** CSV https://data.nasa.gov/docs/legacy/meteorite_landings/Meteorite_Landings.csv, 45,716 rows, 3.95 MB.
    - **Fields:** `reclat`, `reclong`, `mass (g)`, `recclass`, `fall`, `year`.
    - **Licence:** the page says "License not specified"; it is NASA open data from the Meteoritical Society.
    - The page's own description still says 34,513; the one-liner uses the count from the file.
28. **2023 and 2024 solar eclipses**: https://svs.gsfc.nasa.gov/5073
    - **Format and size:** `2024eclipse_shapefiles.zip`, 38.4 MB, plus KML zips and the 2023 equivalents.
    - **Facts:** center line, duration contours every 30 s, penumbra (obscuration) contours every 5%.
    - **Licence:** credit NASA Goddard.
29. **Black Marble, Earth at night**: https://science.nasa.gov/earth/earth-observatory/earth-at-night/maps/
    - The old earthobservatory.nasa.gov URL now 301-redirects here.
    - **Format and size:** GeoTIFFs for 2012 and 2016 at 0.1°, 3 km and 500 m tiles. `BlackMarble_2016_3km_geo.tif` is 64.4 MB.
    - **Licence:** NASA imagery (credit).

### Everyday places (5)

30. **All the Places**: https://alltheplaces.xyz/spiders.html
    - **Format and size:** a GeoJSON FeatureCollection per spider, e.g. `.../runs/2026-10-10-13-32-41/output/starbucks_us.geojson` at 13.1 MB.
    - **Facts:** the 2026-10-10 run has 5,641 spiders and 41,066,413 features; weekly runs.
    - **Licence:** CC0.
31. **overpass turbo**: https://overpass-turbo.eu/
    - Page loads. Wizard, Run and Export (GeoJSON, KML) are its standard controls.
    - **Licence:** ODbL.
32. **BBBike extracts**: https://extract.bbbike.org/
    - **Formats:** OSM, PBF, Garmin, Shapefile, GeoJSON, SQLite and more.
    - **Email:** required; "You will be notified by email when your extract is ready".
    - **Licence:** ODbL.
33. **Geofabrik**: https://download.geofabrik.de/
    - **Format and size:** `*-latest-free.shp.zip`. Taiwan is 263 MB. China's free shapefile is 2.25 GB and is a 2024-01-01 snapshot. Flagged large.
    - **Facts:** the layer list (roads, railways, buildings, pofw, pois…) is Geofabrik's documented free-shapefile schema.
    - **Licence:** ODbL.
34. **Singapore hawker centres (NEA)**: https://data.gov.sg/datasets/d_4a086da0a5553be1d89383cd90d07ecd/view
    - **Format and size:** GeoJSON, 137 KB, 129 point features.
    - **Facts:** `STATUS`: Existing 103, Existing (new) 16, Under Construction 6, Existing (replacement) 3, Interim Centre 1; fields include address and `PHOTOURL`.
    - **Licence:** Singapore Open Data Licence.

## Rejected candidates

- **TeleGeography submarine cables:** the GitHub repo is gone (404). The public `cable-geo.json` API still works, but the underlying data is sold under licence (the CC BY-SA terms cover the maps only).
- **IM3 data center atlas (PNNL):** I found no download format or licence.
- **Other open data-center lists:** none found with a clear licence.
- **NOAA AWOIS wrecks:** retired, and redirected to ENC Direct with no simple download.
- **NASA Global Landslide Catalog:** download only through the Landslide Viewer after accepting terms; no stable direct link.
- **NOAA NCEI tsunami database (HazEL):** a JavaScript app with only a JSON API, nothing directly openable. Bundled earthquakes and volcanoes already cover hazards.
- **NUFORC UFO reports (planetsig/ufo-reports):** no licence.
- **BFRO Bigfoot reports:** no licence. Likewise Shadowlands "haunted places".
- **World Atlas of Artificial Night Sky Brightness (Falchi 2016):** CC BY-NC, but the full GeoTIFF is 2.9 GB behind a request form.
- **Atlas of Hillforts:** no visible download or licence on the landing page.
- **GHSL Urban Centre Database 2024:** the package is about 1.7 GB, in Mollweide; too heavy for a first exploration.
- **NOAA OISST v2 1° monthly:** the file stopped updating in Feb 2023; replaced by ERSST v5.
- **WDPA protected areas:** needs a terms form; non-commercial.
- **US NPS boundaries:** ArcGIS Hub only, and not distinctive enough.
- **GeoNames cities15000:** the TSV has no header row, so column detection would fail.
- **OpenFlights routes:** no coordinates, and `airports.dat` has no header.
- **Overture Places:** GeoParquet, which NimbleMap doesn't open.
- **JPL CNEOS fireballs:** latitude and longitude are unsigned, with separate N/S and E/W columns.
- **Native Land Digital:** the API now needs a key.
- **Boundary downloads (DataV GeoAtlas, geoBoundaries, Natural Earth admin-1):** NimbleMap already bundles boundaries for joins. DataV's licence is unclear, and disputed borders are drawn differently by each source, which is sensitive for the Chinese audience.
- **USGS live feeds as GeoJSON:** the CSV page was linked instead, because it is simpler.
- **WRI Global Power Plant Database:** already bundled.
