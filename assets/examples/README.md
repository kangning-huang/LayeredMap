# Example data (the Examples menu)

The three files LayeredMap opens from the welcome card's **Try an example** button and
**File › Open example data** (build plan, WP2.9). Built by `tools/lm/make_examples.py` from the
sources below; nothing else was changed. `cities.geojson` was called `populated_places.geojson` until 18 Oct 2026 (renamed, contents unchanged): "populated places" is Natural Earth's term, and the layer's name is what people see. The macOS bundle copies this folder to
`LayeredMap.app/Contents/Resources/examples/`.

| File | What | Size |
|---|---|---|
| `cities.geojson` | 1,251 cities and towns (points) | 239 KB |
| `countries.geojson` | 177 countries (areas) | 250 KB |
| `elevation.tif` | Land and sea-floor elevation, 720 × 360 cells of 0.5° (raster) | 354 KB |

## Sources and licences

All sources were downloaded on 2026-10-16.

| File | Source | Version | Licence |
|---|---|---|---|
| `cities.geojson` | Natural Earth 1:50m Cultural Vectors, *Populated Places*: <https://naciscdn.org/naturalearth/50m/cultural/ne_50m_populated_places.zip> (SHA-256 `9022c3be867be79c44bd6fc6ff4dcc210e85ffee13b033bf4d1d87d83c9ee6b4`) | 5.1.2 | Public domain ([Natural Earth terms of use](https://www.naturalearthdata.com/about/terms-of-use/)) |
| `countries.geojson` | Natural Earth 1:110m Cultural Vectors, *Admin 0 – Countries*: <https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip> (SHA-256 `0f243aeac8ac6cf26f0417285b0bd33ac47f1b5bdb719fd3e0df37d03ea37110`) | 5.1.1 | Public domain |
| `elevation.tif` | ETOPO 2022 Global Relief Model, 60 arc-second ice-surface elevation GeoTIFF `ETOPO_2022_v1_60s_N90W180_surface.tif`: <https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif> (465,969,062 bytes, last modified 2022-10-04, SHA-256 `9d27d4b8ea8e76977e2988bca667d7c8fa68b927355feffcddd6b4875a7fd08e`); product page <https://www.ncei.noaa.gov/products/etopo-global-relief-model> | ETOPO 2022 v1 | Public domain: "These data were produced by NOAA and are not subject to copyright protection in the United States. NOAA waives any potential copyright and related rights in these data worldwide through the Creative Commons Zero 1.0 Universal Public Domain Dedication (CC0-1.0)." (ETOPO 2022 ISO metadata) |

ETOPO 2022 citation, as NOAA asks:

> NOAA National Centers for Environmental Information. 2022: ETOPO 2022 15 Arc-Second Global
> Relief Model. NOAA National Centers for Environmental Information.
> https://doi.org/10.25921/fd45-gt74. Accessed 2026-10-16.

## Fields

`cities.geojson` (sorted by population, largest first):

| Field | From | Meaning |
|---|---|---|
| `name` | `NAME` | Place name |
| `name_zh` | `NAME_ZH` | Chinese name (missing for some places) |
| `country` | `ADM0NAME` | Country |
| `population` | `POP_MAX` | Population of the metropolitan area (Natural Earth's estimate) |
| `capital` | `ADM0CAP` | `true` for a national capital |

`countries.geojson` (sorted by name):

| Field | From | Meaning |
|---|---|---|
| `name` | `NAME` | Country name |
| `name_zh` | `NAME_ZH` | Chinese name |
| `iso_a3` | `ISO_A3_EH` | ISO 3166-1 alpha-3 code (missing where Natural Earth has none, e.g. Kosovo) |
| `continent` | `CONTINENT` | Continent |
| `pop_est` | `POP_EST` | Population estimate |
| `pop_year` | `POP_YEAR` | Year of the population estimate |
| `gdp_md` | `GDP_MD` | GDP, millions of US dollars |
| `gdp_year` | `GDP_YEAR` | Year of the GDP figure |

Natural Earth's `-99` and empty values are written as `null`. Coordinates are rounded to 5 decimal
places (about 1 m); the country outlines are not simplified.

`elevation.tif`: one band, int16 metres above mean sea level (negative below it: sea floor and
land below sea level), EPSG:4326, origin (−180°, 90°), 0.5° cells, Deflate compression with a
horizontal predictor, no NoData value (every cell has data). Each cell is the mean of the 30 × 30
source cells it covers (an exact block mean: the source grid is 1/60° cells from (−180°, 90°)),
rounded to the nearest metre. The source's coordinate system is EPSG:9518 (WGS 84 with EGM2008
heights); its horizontal part is EPSG:4326, so no reprojection was needed and heights stay
relative to the EGM2008 geoid. Values range
from −8,714 m to 5,724 m; averaging lowers peaks and raises trenches.

## Processing

In the `layeredmap-ref` mamba environment (Python with geopandas 1.2, rasterio 1.5 / GDAL 3.13,
numpy):

```sh
d=$(mktemp -d)
curl -sSL -o "$d/places.zip" https://naciscdn.org/naturalearth/50m/cultural/ne_50m_populated_places.zip
curl -sSL -o "$d/countries.zip" https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip
curl -sSL -o "$d/ETOPO_2022_v1_60s_N90W180_surface.tif" \
  https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif
(cd "$d" && unzip -q places.zip && unzip -q countries.zip)
mamba run -n layeredmap-ref python -I tools/lm/make_examples.py \
  --places "$d/ne_50m_populated_places.shp" \
  --countries "$d/ne_110m_admin_0_countries.shp" \
  --etopo "$d/ETOPO_2022_v1_60s_N90W180_surface.tif" \
  --out assets/examples
```


## The Examples menu (added 2026-10-18)

Besides the world basics above, the Examples menu offers datasets an everyday map app doesn't show
(owner's request; chosen from the survey in `docs/lm/example-data-survey.md`). Each was trimmed to a
few readable columns; no values were changed. Prepared from the copies documented in
`/Volumes/LaCie/data/layeredmap-example-survey/README.md` (download dates, URLs, SHA-256).

| File | What | Size | Source | Licence and credit |
|---|---|---|---|---|
| `heatwaves summer 2023.nc` | Daily heat-wave intensity, 1 June – 31 August 2023, worldwide on a 0.5° grid (92 days, NetCDF-4): variable `heatwave_intensity` (accumulated warm-spell intensity, °C); days and places without a warm spell are no data | 12.6 MB | Copernicus Global Drought Observatory (JRC), "GDO Warm Spell Intensity ERA5 and Destination Earth Extreme DT" v3.0.0, 2023 file `hwi2a_m_gdo_20230101_20231231_d.nc` (JRC dataset 8b7e4cf0-6884-4bb6-b689-471ba8b50a97) | CC BY 4.0, "(c) European Union, 1995-2026". Changed: June–August only, averaged from 0.25° to 0.5° (2×2 mean), zeros (no warm spell) set to no data, variable renamed with a long name and unit. Source copy documented in `/Volumes/LaCie/data/layeredmap-netcdf-test/README.md` |
| `storm tracks.gpkg` | 4,999 tropical cyclone tracks 1980–2026, all ocean basins, one line per storm: name, season, basin, max wind (kt), Saffir–Simpson strength, start and end time | 7.5 MB | NOAA NCEI IBTrACS v04r01 "since 1980" lines, segments merged per storm (SID) | NOAA/WMO "full and open access"; commercial use guided by WMO Resolution 40 (**owner to confirm before a paid release**). Cite Knapp et al. 2010, BAMS, doi:10.1175/2009BAMS2755.1, and Gahtan et al. 2024, doi:10.25921/82ty-9e16 |
| `walled cities 1866.geojson` | 1,789 city-wall areas of late imperial China as of 1866: name (pinyin), type (Capital, Provincial Capital, Fu, Zhou, County, Ting, Wei, Suo, Manchu garrison), walled (no = reliability D, no wall), reliability, area (km²) | 0.8 MB | Xue et al. 2021, CUED 1866 snapshot, figshare doi:10.6084/m9.figshare.14112968.v3 | CC BY 4.0. Credit: Xue, Q., Jin, X., Cheng, Y., Yang, X., Zhou, Y.: The dataset of walled cities and urban extent in late imperial China in the 15th–19th centuries, Earth Syst. Sci. Data 13, 5071–5085 (2021), doi:10.5194/essd-13-5071-2021. Changed: fields renamed, `walled` added, `REFERENCES` dropped, areas rounded to 0.001 km² |
| `earthquakes.csv` | 14,537 earthquakes of magnitude 6+, 1900 to 2026-10-18: date, latitude, longitude, depth (km), magnitude, place | 1.0 MB | USGS ComCat (FDSN event query, minmagnitude 6) | Public domain (US Government). Credit "U.S. Geological Survey" |
| `volcanoes.csv` | 1,609 volcano locations: name, country, latitude, longitude, elevation (m), morphology, status | 0.1 MB | NOAA NCEI Global Volcano Locations (hazard service API) | US Government work, no restriction stated. Cite NCEI/WDS Global Significant Volcanic Eruptions Database, doi:10.7289/V5JW8BSH |
| `airports.csv` | 5,280 large and medium airports: name, IATA code, type, latitude, longitude, elevation (m), country (ISO 2-letter), city | 0.4 MB | OurAirports | Public domain ("All data is released to the Public Domain") |
| `power plants.csv` | 34,936 power plants: name, country, primary fuel, capacity (MW), commissioning year, latitude, longitude | 2.2 MB | WRI Global Power Plant Database v1.3.0 (2021) | CC BY 4.0. Credit: Global Energy Observatory, Google, KTH Royal Institute of Technology, Enipedia, World Resources Institute (2019), Global Power Plant Database, Published on Resource Watch and Google Earth Engine |
