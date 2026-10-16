# Example data (Try an example)

The three files LayeredMap opens from the welcome card's **Try an example** button and
**File › Open example data** (build plan, WP2.9). Built by `tools/lm/make_examples.py` from the
sources below; nothing else was changed. The macOS bundle copies this folder to
`LayeredMap.app/Contents/Resources/examples/`.

| File | What | Size |
|---|---|---|
| `populated_places.geojson` | 1,251 populated places (points) | 239 KB |
| `countries.geojson` | 177 countries (areas) | 250 KB |
| `elevation.tif` | Land and sea-floor elevation, 720 × 360 cells of 0.5° (raster) | 354 KB |

## Sources and licences

All sources were downloaded on 2026-10-16.

| File | Source | Version | Licence |
|---|---|---|---|
| `populated_places.geojson` | Natural Earth 1:50m Cultural Vectors, *Populated Places*: <https://naciscdn.org/naturalearth/50m/cultural/ne_50m_populated_places.zip> (SHA-256 `9022c3be867be79c44bd6fc6ff4dcc210e85ffee13b033bf4d1d87d83c9ee6b4`) | 5.1.2 | Public domain ([Natural Earth terms of use](https://www.naturalearthdata.com/about/terms-of-use/)) |
| `countries.geojson` | Natural Earth 1:110m Cultural Vectors, *Admin 0 – Countries*: <https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip> (SHA-256 `0f243aeac8ac6cf26f0417285b0bd33ac47f1b5bdb719fd3e0df37d03ea37110`) | 5.1.1 | Public domain |
| `elevation.tif` | ETOPO 2022 Global Relief Model, 60 arc-second ice-surface elevation GeoTIFF `ETOPO_2022_v1_60s_N90W180_surface.tif`: <https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif> (465,969,062 bytes, last modified 2022-10-04, SHA-256 `9d27d4b8ea8e76977e2988bca667d7c8fa68b927355feffcddd6b4875a7fd08e`); product page <https://www.ncei.noaa.gov/products/etopo-global-relief-model> | ETOPO 2022 v1 | Public domain: "These data were produced by NOAA and are not subject to copyright protection in the United States. NOAA waives any potential copyright and related rights in these data worldwide through the Creative Commons Zero 1.0 Universal Public Domain Dedication (CC0-1.0)." (ETOPO 2022 ISO metadata) |

ETOPO 2022 citation, as NOAA asks:

> NOAA National Centers for Environmental Information. 2022: ETOPO 2022 15 Arc-Second Global
> Relief Model. NOAA National Centers for Environmental Information.
> https://doi.org/10.25921/fd45-gt74. Accessed 2026-10-16.

## Fields

`populated_places.geojson` (sorted by population, largest first):

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
