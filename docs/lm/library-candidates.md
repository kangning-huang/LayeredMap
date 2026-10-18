# Data library candidates (WP5.4)

Status: draft for the owner to choose from, dated 2026-10-18. All licence pages listed below were opened on 2026-10-18. File sizes come from HTTP `Content-Length` (HEAD requests) or the provider's download page on the same date, unless stated otherwise. SHA-256 values are "to compute when hosted" for all entries.

Shanghai bounding box used below: roughly lon 120.85–122.20, lat 30.67–31.88.

## Summary

| id | Title | Kind | Size (bytes, as shipped by provider) | Licence | China note |
|---|---|---|---|---|---|
| `ne-countries-chn-pov` | Natural Earth countries, China point of view | areas | 4,904,124 (zip) | Public domain | Re-host. Still not a reviewed map (no 审图号). |
| `ne-provinces` | Natural Earth admin-1 provinces | areas | 14,909,524 (zip, global) | Public domain | Re-host. Clip to China. Same boundary caveat. |
| `ne-populated-places` | Natural Earth populated places (simple) | points | 652,145 (zip) | Public domain | Re-host. Small. |
| `ne-rivers` | Natural Earth rivers and lake centrelines | lines | 2,079,507 (zip) | Public domain | Re-host. Small. |
| `worldpop-chn-2020-1km` | WorldPop China population 2020, 1 km | raster | 49,445,858 (GeoTIFF) | CC-BY-4.0 | Re-host. Clip to Shanghai for class. |
| `ghs-pop-2020-shanghai` | GHS-POP 2020, 100 m tile covering Shanghai | raster | 31,528,808 (zip, one tile) | CC-BY-4.0 | Re-host. Mollweide; reproject. |
| `etopo-2022-east-china` | ETOPO 2022 elevation, 15″ tile | raster | 20,965,352 (GeoTIFF) | CC0-1.0 | Re-host. Clip. |
| `cop-dem-glo90-shanghai` | Copernicus DEM GLO-90, Shanghai tiles | raster | 17,656,385 (4 COGs) | LicenseRef-Copernicus-DEM | Re-host allowed with notice (check). |
| `worldcover-2021-n30e120` | ESA WorldCover 2021, tile N30E120 | raster | 93,443,932 (COG) | CC-BY-4.0 | AWS eu-central-1. Re-host. Clip. |
| `osm-shanghai` | OpenStreetMap Shanghai (Geofabrik) | points/lines/areas | 48,887,578 (gpkg zip) | ODbL-1.0 | Share-alike. Re-host the extract. |
| `chelsa-tas-july` | CHELSA mean July air temperature 1981–2010 | raster | 164,914,648 (GeoTIFF, global) | CC0-1.0 | Re-host. Clip to China/Shanghai. |
| `modis-lst-summer-shanghai` | MODIS land surface temperature, 8-day, summer | raster | ~3.6 MB per granule (catalogue average) | CC0-1.0 (NASA policy) | Needs Earthdata login. Re-host a composite. |

## Datasets

### 1. `ne-countries-chn-pov`

- English title: Natural Earth Admin 0 Countries (China point of view)
- Chinese title: Natural Earth 国家边界（中国视角版）
- Description (EN): World country polygons drawn from China's point of view on disputed borders.
- Description (中文): 按中国立场绘制争议边界的世界国家面数据。
- Kind: areas
- Format: Shapefile (zip)
- File size: 4,904,124 bytes (`Content-Length`, checked 2026-10-18). Standard (de facto) version: 4,930,492 bytes.
- Extent: global (lon -180 to 180, lat -90 to 84)
- Licence: public domain (no SPDX id; closest is `LicenseRef-PublicDomain`)
- Licence page opened: https://www.naturalearthdata.com/about/terms-of-use/
- Quote: "All versions of Natural Earth raster + vector map data found on this website are in the public domain." Also: "Crediting the authors is unnecessary."
- POV page opened: https://www.naturalearthdata.com/downloads/10m-cultural-vectors/ — "These optional point-of-view (POV) variants show the worldview for several dozen countries". China is listed (`ne_10m_admin_0_countries_chn.zip`).
- Official source: https://www.naturalearthdata.com/downloads/10m-cultural-vectors/
- Direct download: https://naciscdn.org/naturalearth/10m/cultural/ne_10m_admin_0_countries_chn.zip
- Recommended citation (verbatim, optional): "Made with Natural Earth. Free vector and raster map data @ naturalearthdata.com."
- Version: 5.1.1 (file last-modified 2022-05-13)
- Attribution to show: not required. Suggest "Made with Natural Earth."
- China reachability: naciscdn.org unknown, test from Shanghai. Plan to re-host.
- Preprocessing: convert to GeoJSON or GeoPackage. Optionally keep only Asia for a lighter file.
- Caution: the China POV variant follows China's claims, but it is not a reviewed map. Maps shown to the public in China that include national boundaries normally need map review (审图). See owner questions.

### 2. `ne-provinces`

- English title: Natural Earth Admin 1 States and Provinces
- Chinese title: Natural Earth 一级行政区（省级）
- Description (EN): First-level administrative units (provinces, states) for the world.
- Description (中文): 全球一级行政区划面（省、州）。
- Kind: areas
- Format: Shapefile (zip)
- File size: 14,909,524 bytes (`Content-Length`, 2026-10-18). Page lists 14.22 MB.
- Extent: global; we would subset to China (about lon 73–135, lat 18–54).
- Licence: public domain (same terms page and quote as item 1)
- Licence page opened: https://www.naturalearthdata.com/about/terms-of-use/
- Quote: "No permission is needed to use Natural Earth."
- Official source: https://www.naturalearthdata.com/downloads/10m-cultural-vectors/
- Direct download: https://naciscdn.org/naturalearth/10m/cultural/ne_10m_admin_1_states_provinces.zip
- Recommended citation: "Made with Natural Earth. Free vector and raster map data @ naturalearthdata.com."
- Version: 5.1.1 (last-modified 2022-05-13)
- Attribution to show: not required; suggest "Made with Natural Earth."
- China reachability: unknown, test from Shanghai. Re-host.
- Preprocessing: filter to China (`adm0_a3 = CHN`), drop unused columns, export GeoJSON (should end up well under 5 MB). Check how Taiwan and disputed areas are coded; admin-1 has no China POV variant.

### 3. `ne-populated-places`

- English title: Natural Earth Populated Places (simple)
- Chinese title: Natural Earth 居民点（简化版）
- Description (EN): Cities and towns as points with names and population estimates.
- Description (中文): 带名称和人口估计的城市与城镇点数据。
- Kind: points
- Format: Shapefile (zip)
- File size: 652,145 bytes (`Content-Length`, 2026-10-18). Full-attribute version: 2,811,199 bytes.
- Extent: global
- Licence: public domain
- Licence page opened: https://www.naturalearthdata.com/about/terms-of-use/
- Quote: "All versions of Natural Earth raster + vector map data found on this website are in the public domain."
- Official source: https://www.naturalearthdata.com/downloads/10m-cultural-vectors/
- Direct download: https://naciscdn.org/naturalearth/10m/cultural/ne_10m_populated_places_simple.zip
- Recommended citation: "Made with Natural Earth. Free vector and raster map data @ naturalearthdata.com."
- Version: 5.1.1 (last-modified 2022-05-13)
- Attribution to show: not required.
- China reachability: unknown, test from Shanghai. Re-host.
- Preprocessing: convert to GeoJSON or CSV (lon/lat columns). Good first "points" layer.

### 4. `ne-rivers`

- English title: Natural Earth Rivers and Lake Centerlines
- Chinese title: Natural Earth 河流与湖泊中心线
- Description (EN): Major rivers and lake centrelines as lines.
- Description (中文): 主要河流及湖泊中心线（线数据）。
- Kind: lines
- Format: Shapefile (zip)
- File size: 2,079,507 bytes (`Content-Length`, 2026-10-18)
- Extent: global
- Licence: public domain
- Licence page opened: https://www.naturalearthdata.com/about/terms-of-use/
- Quote: "Crediting the authors is unnecessary."
- Official source: https://www.naturalearthdata.com/downloads/10m-physical-vectors/
- Direct download: https://naciscdn.org/naturalearth/10m/physical/ne_10m_rivers_lake_centerlines.zip
- Recommended citation: "Made with Natural Earth. Free vector and raster map data @ naturalearthdata.com."
- Version: 5.x (last-modified 2021-12-08)
- Attribution to show: not required.
- China reachability: unknown, test from Shanghai. Re-host.
- Preprocessing: convert to GeoJSON. Too coarse for Shanghai streets; use OSM for local waterways. (Natural Earth roads also exist: 9,075,919 bytes.)

### 5. `worldpop-chn-2020-1km`

- English title: WorldPop China population counts 2020, 1 km (UN-adjusted)
- Chinese title: WorldPop 中国 2020 年人口分布（1 公里，联合国校正）
- Description (EN): Estimated number of people per 1 km grid cell in China, 2020.
- Description (中文): 2020 年中国每 1 公里格网的估计人口数。
- Kind: raster
- Format: GeoTIFF (WGS84, 30 arc-seconds)
- File size: 49,445,858 bytes (`Content-Length`, 2026-10-18; page says 47.16 MB). Higher-resolution option: 100 m constrained `chn_ppp_2020_constrained.tif`, 688,786,378 bytes (page says 656.88 MB) — too big to ship whole.
- Extent: China (national grid)
- Licence: CC-BY-4.0
- Licence pages opened: https://hub.worldpop.org/geodata/summary?id=34879 and https://hub.worldpop.org/data/licence.txt
- Quote: "WorldPop datasets are available under the Creative Commons Attribution 4.0 International License."
- Official source: https://hub.worldpop.org/geodata/summary?id=34879
- Direct download: https://data.worldpop.org/GIS/Population/Global_2000_2020_1km_UNadj/2020/CHN/chn_ppp_2020_1km_Aggregated_UNadj.tif
- Recommended citation (verbatim from the page): "WorldPop (www.worldpop.org - School of Geography and Environmental Science, University of Southampton; Department of Geography and Geosciences, University of Louisville; Departement de Geographie, Universite de Namur) and Center for International Earth Science Information Network (CIESIN), Columbia University (2018). Global High Resolution Population Denominators Project - Funded by The Bill and Melinda Gates Foundation (OPP1134076). https://dx.doi.org/10.5258/SOTON/WP00671"
- For the 100 m constrained file (summary id 49730) the page asks for: "Bondarenko M., Kerr D., Sorichetta A., and Tatem, A.J. 2020. Census/projection-disaggregated gridded population datasets for 189 countries in 2020 using Built-Settlement Growth Model (BSGM) outputs. WorldPop, University of Southampton, UK. doi:10.5258/SOTON/WP00684"
- Version: 2020 estimate (file dated 2020-06-22)
- Attribution to show: "Population: WorldPop (CC BY 4.0)"
- China reachability: worldpop.org unknown, test from Shanghai. Re-host.
- Preprocessing: keep the China file as is (~47 MB) or clip to Shanghai (tiny). Consider a Shanghai clip of the 100 m constrained file for a city-scale lesson. Note changes made (CC BY requires it).

### 6. `ghs-pop-2020-shanghai`

- English title: GHS-POP R2023A population grid, 2020, 100 m (tile R6_C30)
- Chinese title: 全球人类住区人口格网 GHS-POP 2020（100 米，上海所在图块）
- Description (EN): Residential population per 100 m cell from the EU JRC, for the tile covering Shanghai.
- Description (中文): 欧盟联合研究中心发布的 100 米格网居住人口（上海所在图块）。
- Kind: raster
- Format: GeoTIFF in zip, Mollweide (ESRI:54009)
- File size: 31,528,808 bytes for tile R6_C30 (`Content-Length`, 2026-10-18). Neighbour R6_C29 is listed as 87M. Global 30″ WGS84 file: 482,351,880 bytes.
- Tile choice: R6_C30 is my calculation from the 1000 km Mollweide grid (Shanghai ≈ x 11.04e6 m, y 3.78e6 m). Confirm with the tiling shapefile https://ghsl.jrc.ec.europa.eu/download/GHSL_data_54009_shapefile.zip before hosting.
- Extent: one 1000 × 1000 km Mollweide tile (east China coast incl. Shanghai)
- Licence: CC-BY-4.0
- Licence pages opened: https://human-settlement.emergency.copernicus.eu/GHSLhowToCite.php and the `copyright.txt` next to the data, https://jeodpp.jrc.ec.europa.eu/ftp/jrc-opendata/GHSL/GHS_POP_GLOBE_R2023A/GHS_POP_E2020_GLOBE_R2023A_54009_100/V1-0/copyright.txt
- Quote: "The GHSL data are provided under the Creative Commons Attribution 4.0 International (CC BY 4.0) license." and "Reuse is allowed provided appropriate credit is given and any changes are indicated."
- Official source: https://human-settlement.emergency.copernicus.eu/ghs_pop2023.php
- Direct download: https://jeodpp.jrc.ec.europa.eu/ftp/jrc-opendata/GHSL/GHS_POP_GLOBE_R2023A/GHS_POP_E2020_GLOBE_R2023A_54009_100/V1-0/tiles/GHS_POP_E2020_GLOBE_R2023A_54009_100_V1_0_R6_C30.zip
- Recommended citation: the how-to-cite page asks for the latest GHSL reference publication: "Pesaresi, M., Schiavina, M., Politis, P., Freire, S., Krasnodębska, K., Uhl, J. H., … Kemper, T. (2024). Advances on the Global Human Settlement Layer by joint assessment of Earth Observation and population survey data. International Journal of Digital Earth, 17(1)." The product-specific dataset citation is on the download page; I could not read it (data.jrc.ec.europa.eu rejected the request). To fill in when hosting.
- Version: R2023A, epoch 2020, V1-0 (files dated 2025-01-27)
- Attribution to show: "Population: GHS-POP R2023A, © European Union (CC BY 4.0)"
- China reachability: jeodpp.jrc.ec.europa.eu unknown, test from Shanghai. Re-host.
- Preprocessing: unzip, reproject to WGS84 (or keep Mollweide if the app handles it), clip to Shanghai. Overlaps with WorldPop; pick one or use both for a "two models disagree" lesson.

### 7. `etopo-2022-east-china`

- English title: ETOPO 2022 Global Relief Model, 15 arc-second surface elevation (tile N45E120)
- Chinese title: ETOPO 2022 全球地形起伏模型（15 角秒地表高程，N45E120 图块）
- Description (EN): Land elevation and sea-floor depth for a 15° tile that includes Shanghai.
- Description (中文): 含上海在内的 15° 图块陆地高程与海底深度。
- Kind: raster
- Format: GeoTIFF
- File size: 20,965,352 bytes (`Content-Length`, 2026-10-18). Global 60″ file: 465,969,062 bytes; global 30″: 1,585,813,987 bytes.
- Extent: lon 120–135, lat 30–45 (tile name gives the upper-left corner). Shanghai is near the south-west corner. For inland China add neighbouring tiles (e.g. N45E105, 35,044,259 bytes).
- Licence: CC0-1.0
- Licence page opened: https://www.ncei.noaa.gov/access/metadata/landing-page/bin/iso?id=gov.noaa.ngdc.mgg.dem:etopo_2022
- Quote: "NOAA waives any potential copyright and related rights in these data worldwide through the Creative Commons Zero 1.0 Universal Public Domain Dedication (CC0-1.0)."
- Official source: https://www.ncei.noaa.gov/products/etopo-global-relief-model
- Direct download: https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/15s/15s_surface_elev_gtif/ETOPO_2022_v1_15s_N45E120_surface.tif
- Recommended citation (verbatim): "NOAA National Centers for Environmental Information. 2022: ETOPO 2022 15 Arc-Second Global Relief Model. NOAA National Centers for Environmental Information. https://doi.org/10.25921/fd45-gt74 . Accessed [date]."
- Version: ETOPO 2022 v1 (files dated 2022-10-04)
- Attribution to show: not required (CC0). Suggest "Elevation: NOAA ETOPO 2022".
- China reachability: ngdc.noaa.gov unknown, test from Shanghai. Re-host.
- Preprocessing: clip to China or Shanghai. Shanghai is almost flat; this layer works better at China scale. Good for showing that rasters have values (metres).

### 8. `cop-dem-glo90-shanghai`

- English title: Copernicus DEM GLO-90, Shanghai tiles
- Chinese title: 哥白尼数字高程模型 GLO-90（上海图块）
- Description (EN): 90 m surface elevation for the four 1° tiles covering Shanghai.
- Description (中文): 覆盖上海的四个 1° 图块的 90 米地表高程。
- Kind: raster
- Format: Cloud-Optimised GeoTIFF
- File size (`Content-Length`, 2026-10-18): N31E121 4,634,711; N30E121 2,749,978; N31E120 4,727,454; N30E120 5,544,242. Total 17,656,385 bytes. GLO-30 alternative: N31E121 alone is 40,911,271 bytes.
- Extent: lon 120–122, lat 30–32
- Licence: ESA/Copernicus DEM licence (no SPDX id; use `LicenseRef-Copernicus-DEM`)
- Licence page opened: https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM
- Quote: "The GLO-30 and GLO-90 datasets are available worldwide with a free license."
- Official source: same page; AWS mirror https://registry.opendata.aws/copernicus-dem/
- Direct download (pattern): https://copernicus-dem-90m.s3.amazonaws.com/Copernicus_DSM_COG_30_N31_00_E121_00_DEM/Copernicus_DSM_COG_30_N31_00_E121_00_DEM.tif (swap N30/N31, E120/E121)
- Recommended citation: the page asks research users "to use the following DOI when citing the data source": https://doi.org/10.5270/ESA-c5d3d65
- Required notice if modified (verbatim): "produced using Copernicus WorldDEM-90 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved"
- Version: as on AWS bucket (GLO-90 DGED; check release year when hosting)
- China reachability: AWS S3 (us/eu) unknown, test from Shanghai. Re-host.
- Preprocessing: mosaic 4 tiles, clip to Shanghai. Keep the notice in the layer metadata and on screen.
- Note: "all rights reserved" plus a separate licence means this is not CC. I only read the summary page, not the full ESA licence PDF. Confirm redistribution terms before hosting. If in doubt, use ETOPO (CC0) instead.

### 9. `worldcover-2021-n30e120`

- English title: ESA WorldCover 10 m 2021 v200, tile N30E120
- Chinese title: 欧空局 WorldCover 2021 年 10 米土地覆盖（N30E120 图块）
- Description (EN): Land cover in 11 classes at 10 m for a 3° tile that contains Shanghai.
- Description (中文): 上海所在 3° 图块的 10 米分辨率 11 类土地覆盖。
- Kind: raster
- Format: Cloud-Optimised GeoTIFF (EPSG:4326)
- File size: 93,443,932 bytes (`Content-Length`, 2026-10-18). 2020 v100 version of the same tile: 93,686,631 bytes.
- Extent: lon 120–123, lat 30–33 (tile name is the lower-left corner; covers all of Shanghai)
- Licence: CC-BY-4.0
- Licence page opened: https://esa-worldcover.org/en/data-access
- Quote: "The ESA WorldCover product is provided free of charge, without restriction of use."
- Official source: https://esa-worldcover.org/en/data-access
- Direct download: https://esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/map/ESA_WorldCover_10m_2021_v200_N30E120_Map.tif
- Recommended citation (from the page): Zanaga, D. et al. (2022). ESA WorldCover 10 m 2021 v200. https://doi.org/10.5281/zenodo.7254221 (the page lists the full author list; copy it in full when hosting).
- Version: v200, 2021 (file dated 2022-10-26)
- Attribution to show (verbatim template, year 2021): "© ESA WorldCover project 2021 / Contains modified Copernicus Sentinel data (2021) processed by ESA WorldCover consortium"
- China reachability: AWS S3 eu-central-1, unknown, test from Shanghai. Re-host.
- Preprocessing: clip to Shanghai (should drop to a few MB). Keep the class palette. Good "what covers the land" layer.

### 10. `osm-shanghai`

- English title: OpenStreetMap data for Shanghai (Geofabrik extract)
- Chinese title: 上海 OpenStreetMap 数据（Geofabrik 提取）
- Description (EN): Roads, waterways, buildings and points of interest (parks, schools, hospitals) in Shanghai.
- Description (中文): 上海的道路、水系、建筑及兴趣点（公园、学校、医院等）。
- Kind: points, lines, areas (several layers)
- Format: GeoPackage in zip (`-free.gpkg.zip`); raw `.osm.pbf` also available
- File size (`Content-Length`, 2026-10-18): `shanghai-latest-free.gpkg.zip` 48,887,578 bytes; `shanghai-latest.osm.pbf` 26,004,927 bytes. Whole China pbf: 1,611,462,839 bytes (no China-wide free shapefile).
- Extent: Shanghai municipality (Geofabrik polygon)
- Licence: ODbL-1.0
- Licence pages opened: https://www.openstreetmap.org/copyright and https://osmfoundation.org/wiki/Licence/Attribution_Guidelines (Geofabrik page footer also says "License: ODbL 1.0": https://download.geofabrik.de/asia/china.html)
- Quote: "OpenStreetMap is open data, licensed under the Open Data Commons Open Database License (ODbL)" and "If you alter or build upon our data, you may distribute the result only under the same license."
- Official source: https://download.geofabrik.de/asia/china.html
- Direct download: https://download.geofabrik.de/asia/china/shanghai-latest-free.gpkg.zip
- Recommended citation / attribution (the guidelines accept): "© OpenStreetMap contributors"
- Version: snapshot of 2026-10-17/18 (Geofabrik updates daily; pin a dated copy)
- Attribution to show: "© OpenStreetMap contributors", visible on or next to the map.
- China reachability: download.geofabrik.de unknown, test from Shanghai. Re-host a dated copy.
- Preprocessing: split into small teaching layers (parks, schools, hospitals as points; main roads; waterways) as GeoJSON or GeoPackage. Keep the full layer names and the snapshot date. Coordinates are WGS84; OSM coverage in China is uneven, so check quality before class.

### 11. `chelsa-tas-july`

- English title: CHELSA V2.1 mean daily air temperature, July, 1981–2010
- Chinese title: CHELSA V2.1 七月平均气温（1981–2010）
- Description (EN): Average July near-surface air temperature at about 1 km, global.
- Description (中文): 全球约 1 公里分辨率的七月近地面平均气温。
- Kind: raster
- Format: GeoTIFF (30 arc-seconds)
- File size: 164,914,648 bytes (S3 listing of the official bucket, 2026-10-18). Other months are 148–165 MB each.
- Extent: global land; clip to China or Shanghai.
- Licence: CC0-1.0
- Licence record opened: EnviDat (WSL, the publisher) API for the dataset, https://www.envidat.ch/api/action/package_show?id=chelsa-climatologies (the human page https://www.envidat.ch/#/metadata/chelsa-climatologies is JavaScript-only and did not render)
- Quote (licence fields): "CC0-1.0", "Creative Commons Zero - No Rights Reserved (CC0 1.0)"
- Official source: https://www.envidat.ch/#/metadata/chelsa-climatologies (DOI 10.16904/envidat.228)
- Direct download: https://os.unil.cloud.switch.ch/chelsa02/chelsa/global/climatologies/tas/1981-2010/CHELSA_tas_07_1981-2010_V.2.1.tif
- Recommended citation: built from the EnviDat record (not a verbatim provider string): Karger, D.N., Conrad, O., Böhner, J., Kawohl, T., Kreft, H., Soria-Auza, R.W., Zimmermann, N.E., Linder, H.P., Kessler, M. (2021). Climatologies at high resolution for the earth's land surface areas. Version 2.1. EnviDat. https://doi.org/10.16904/envidat.228
- Version: V2.1
- Attribution to show: not required (CC0). Suggest "Climate: CHELSA V2.1".
- China reachability: os.unil.cloud.switch.ch unknown, test from Shanghai. Re-host.
- Preprocessing: clip to China (a few MB) or Shanghai. Values are stored scaled (check the CHELSA file specification for scale/offset; likely K/10) and should be converted to °C for students.
- Why not WorldClim: see "Also verified, not recommended".

### 12. `modis-lst-summer-shanghai`

- English title: MODIS/Terra Land Surface Temperature 8-Day 1 km (MOD11A2 v061), summer composite for Shanghai
- Chinese title: MODIS 地表温度 8 天合成 1 公里（MOD11A2 v061），上海夏季合成
- Description (EN): Daytime land surface temperature from satellite, averaged over summer, to show urban heat.
- Description (中文): 卫星观测的白天地表温度夏季平均，用于展示城市热岛。
- Kind: raster
- Format: provider ships HDF-EOS2 (sinusoidal grid); we would ship a GeoTIFF composite
- File size: catalogue lists "Dataset Size 3.6 MB" (per granule, average). Shanghai is in tile h28v05 (my calculation). A summer (Jun–Aug) set is about 12 granules. A clipped GeoTIFF composite would be well under 1 MB.
- Extent: tile h28v05 (1200 × 1200 km); clip to Shanghai.
- Licence: CC0-1.0 (NASA default for NASA-led missions)
- Licence pages opened: https://www.earthdata.nasa.gov/engage/open-data-services-software-policies/data-use-guidance (read with curl; WebFetch was blocked) and https://www.earthdata.nasa.gov/data/catalog/lpcloud-mod11a2-061
- Quote: "Unless the content is marked with a use restriction or license, data provided from a NASA-led mission are licensed as Creative Commons Zero (CC0)." Catalogue page: "This dataset is openly shared, without restriction, in accordance with the EOSDIS Data Use and Citation Guidance".
- Official source: https://www.earthdata.nasa.gov/data/catalog/lpcloud-mod11a2-061
- Direct download: via NASA Earthdata Search / LP DAAC (free Earthdata login required); DOI https://doi.org/10.5067/MODIS/MOD11A2.061
- Recommended citation (verbatim APA from the catalogue): "Wan, Z., Hook, S., & Hulley, G. (2021). MODIS/Terra Land Surface Temperature/Emissivity 8-Day L3 Global 1km SIN Grid V061 [Dataset]. NASA Land Processes Distributed Active Archive Center. https://doi.org/10.5067/MODIS/MOD11A2.061 Date Accessed: [date]"
- Version: v061; choose a year (e.g. summer 2024)
- Attribution to show: "LST: NASA MODIS MOD11A2 v061"
- China reachability: Earthdata login and LP DAAC unknown, test from Shanghai. Must re-host (students should not need a NASA login).
- Preprocessing: download granules, take `LST_Day_1km`, apply scale 0.02 and convert K to °C, mask bad QC, average Jun–Aug, reproject to WGS84, clip to Shanghai, save GeoTIFF. This is an open stand-in for the owner's own summer composite.

### Also verified, not recommended

- `worldclim-2.1-10m-tavg` (WorldClim 2.1). Licence page opened: https://www.worldclim.org/about.html. Quote: "Redistribution or commercial use is not allowed without prior permission." Also: "The data are freely available for academic use and other non-commercial use." We cannot re-host it in the app without permission. Size checked: `wc2.1_10m_tavg.zip` 37,364,656 bytes; `wc2.1_10m_tmax.zip` 37,446,442 bytes. Citation asked: "Fick, S.E. and R.J. Hijmans, 2017. WorldClim 2: new 1km spatial resolution climate surfaces for global land areas." (International Journal of Climatology). Use CHELSA instead, or ask WorldClim for permission.
- `geoboundaries-chn-adm1` (geoBoundaries China ADM1). Record opened: https://www.geoboundaries.org/api/current/gbOpen/CHN/ADM1/ and https://www.geoboundaries.org/countryDownloads.html. The China record says `boundaryLicense`: "Public Domain", source "geoBoundaries, Wikimedia Commons", year 2019, 34 units. Simplified GeoJSON is 263,343 bytes (https://github.com/wmgeolab/geoBoundaries/raw/9469f09/releaseData/gbOpen/CHN/ADM1/geoBoundaries-CHN-ADM1_simplified.geojson). The general site licence is "CC-BY 4.0" with "Attribution is required for all uses of this dataset." Not recommended: source is a Wikimedia map of unclear origin, the per-country and site-wide licences differ, and it carries the same map-review risk. Natural Earth covers the same need. GitHub raw is often slow or blocked in mainland China.

## Share-alike and attribution notes

- ODbL (OpenStreetMap). We must show "© OpenStreetMap contributors" wherever OSM layers are displayed. If we publish a changed or combined database built from OSM (for example a cleaned Shanghai parks layer), that database must be offered under ODbL too. A map image ("produced work") can be under any licence, but must still credit OSM. Merging OSM features with non-ODbL data into one shared file makes that file ODbL. Keep OSM layers as separate files to avoid this.
- CC BY 4.0 (WorldPop, GHS-POP, ESA WorldCover). We may copy, clip, convert and re-host. We must give credit, link the licence, and say what we changed (e.g. "clipped to Shanghai, converted to GeoTIFF"). No share-alike: our derived files can stay CC BY.
- CC0 / public domain (Natural Earth, ETOPO 2022, CHELSA, NASA MODIS). No legal duty to credit. We should still cite them in the library metadata.
- Copernicus DEM. Not a Creative Commons licence. The notice text is mandatory when distributing or showing the data, and a modified form when changed. I did not read the full ESA licence and annex; confirm redistribution by a third-party host before shipping.
- App suggestion: each library item carries an `attribution` string shown in the map corner while the layer is visible, plus a `licence` and `citation` in an info panel.

## Not verified (excluded)

- Shanghai Public Data Open Platform (上海市公共数据开放平台, https://data.sh.gov.cn/): WebFetch returned HTTP 412. I could not read any terms of use. Many datasets may also need login. Excluded until someone in Shanghai checks the terms.
- GHS-POP product-specific dataset citation: https://data.jrc.ec.europa.eu/dataset/2ff68a52-5b5b-4a22-8f40-c41da8332cfe returned "Request Rejected". The licence itself is verified (item 6); only the exact dataset citation string is missing.
- TerraClimate (https://www.climatologylab.org/terraclimate.html): page opened but it states no licence. Excluded.
- SRTM: not checked this session (ETOPO and Copernicus DEM cover elevation).
- Natural Earth 1:50m China POV (`ne_50m_admin_0_countries_chn.zip` on naciscdn): HTTP 403 on HEAD. The 1:10m version works. The 50m file may not exist under that name.

## Questions for the owner

1. The summer surface-temperature composite in the spec is your own data. What is its licence, and do we have permission to ship and re-host it to students? What citation and attribution should the app show? If unclear, item 12 (MODIS) is an open stand-in.
2. Hosting: which China-reachable host do we use (e.g. Alibaba Cloud OSS, Tencent COS, a university server)? None of the source hosts above were tested from Shanghai.
3. Clip scope: ship Shanghai-only clips (small, fast, focused), China-wide files, or both? Rasters like WorldCover and GHS-POP only make sense clipped.
4. National boundaries: maps shown to the public in mainland China that include national borders normally need map review (审图号). Natural Earth's China POV variant is closer to China's position but is not a reviewed map. Options: (a) only use Shanghai-scale data with no national borders; (b) ship admin-0/admin-1 only as data for classroom use with a disclaimer; (c) use an officially reviewed standard map instead. Please decide, ideally with NYU Shanghai's guidance.
5. Copernicus DEM: keep it (needs the full licence read and a mandatory notice) or drop it and use ETOPO only?
6. OSM: are you fine with ODbL share-alike for any cleaned OSM layers we publish?
7. Population: WorldPop, GHS-POP, or both (both allows a "models disagree" exercise)?
8. Climate: CHELSA (CC0, ~165 MB per month globally) or ask WorldClim for redistribution permission?
