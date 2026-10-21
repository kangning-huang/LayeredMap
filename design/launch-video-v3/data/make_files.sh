#!/bin/bash
# v3.1: write the app's example data out as the real files a person downloads, into a scratch
# folder OUTSIDE the repo (nothing here is committed):
#   data/make_files.sh "<folder>"
# Tool: GDAL 3.13 (ogr2ogr, gdal_translate) and netCDF (nccopy) from the mamba env layeredmap-ref.
set -euo pipefail
OUT="$1"
X="$(cd "$(dirname "$0")/../../.." && pwd)/assets/examples"
mkdir -p "$OUT"
cd "$OUT"
run() { env -u PROJ_LIB -u GDAL_DATA PROJ_DATA="$HOME/miniforge3/envs/layeredmap-ref/share/proj" \
          "$HOME/miniforge3/bin/mamba" run -n layeredmap-ref "$@"; }
# storms (IBTrACS, GeoPackage) -> ESRI Shapefile (.shp .shx .dbf .prj .cpg). DBF names are
# limited to 10 characters, so max_wind_kt becomes max_wind_k.
run ogr2ogr -f "ESRI Shapefile" -lco ENCODING=UTF-8 -nln IBTrACS_storm_tracks_1980-2026 \
  IBTrACS_storm_tracks_1980-2026.shp "$X/storm tracks.gpkg" storms
# earthquakes (USGS, CSV with latitude/longitude) -> GeoPackage points (EPSG:4326)
run ogr2ogr -f GPKG -nln earthquakes -a_srs EPSG:4326 -oo X_POSSIBLE_NAMES=longitude \
  -oo Y_POSSIBLE_NAMES=latitude -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES \
  usgs_earthquakes_M6plus_1900-2026.gpkg "$X/earthquakes.csv"
# heatwaves (Copernicus GDO, NetCDF-4) -> NetCDF-4, rewritten by nccopy under a download's name
run nccopy "$X/heatwaves summer 2023.nc" GDO_heatwave_intensity_JJA_2023.nc
# elevation (NOAA ETOPO 2022, GeoTIFF) -> GeoTIFF (Deflate, predictor 2)
run gdal_translate -q -co COMPRESS=DEFLATE -co PREDICTOR=2 "$X/elevation.tif" ETOPO_2022_relief_30min.tif
# volcanoes (NOAA NCEI) -> CSV with latitude/longitude, as it is
cp "$X/volcanoes.csv" NCEI_volcano_locations.csv
find . -name '._*' -delete
ls -la
