/* Spike-only C ABI for NimbleMap iPhone companion (spikes/ios-companion-alt-d).
 * Not a stable API. Free every non-NULL string returned with lm_string_free.
 */
#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Error codes from lm_decode_geojson / lm_decode_geojson_path. */
enum LmDecodeStatus {
  LM_DECODE_OK = 0,
  LM_DECODE_EMPTY = 1,
  LM_DECODE_INVALID_UTF8 = 2,
  LM_DECODE_PARSE = 3,
  LM_DECODE_IO = 4,
  LM_DECODE_NULL_ARG = 5,
};

/**
 * Decode GeoJSON bytes with rustgis-data::io::read_geojson, then emit a
 * FeatureCollection JSON string (overlay-friendly).
 *
 * On LM_DECODE_OK, *out_json is a heap UTF-8 C string; caller must lm_string_free it.
 * On error, *out_json may be an error message string (still free) or NULL.
 */
int32_t lm_decode_geojson(
    const uint8_t *bytes,
    size_t len,
    char **out_json,
    size_t *out_feature_count);

/** Same as lm_decode_geojson, reading a UTF-8 filesystem path. */
int32_t lm_decode_geojson_path(
    const char *path,
    char **out_json,
    size_t *out_feature_count);

/**
 * Open any vector file rustgis-data reads by extension (GeoJSON, CSV with coordinates, KML, KMZ,
 * GPX, zipped shapefile, GeoPackage). On LM_DECODE_OK, *out_json is
 * {"name","geometry":"point|line|area|table","skipped","unreadable","messages":[...],"geojson":{...}}.
 */
int32_t lm_open_path(
    const char *path,
    char **out_json,
    size_t *out_feature_count);

/**
 * lm_open_path for data that may not be in longitude/latitude (metres in a national grid).
 * has_near != 0: guess the grid near (near_lon, near_lat). epsg != 0: a grid the person chose.
 * A ".prj" beside the file wins. The envelope gains "placement":
 * {"how": "prj"|"guess"|"chosen"|"unplaced", "epsg", "name", "km", "alternatives": [...]}.
 */
int32_t lm_open_path_at(
    const char *path,
    int32_t has_near,
    double near_lon,
    double near_lat,
    uint32_t epsg,
    char **out_json,
    size_t *out_feature_count);

/**
 * Open a raster (GeoTIFF, ASCII grid). Writes <out_stem>.png (and <out_stem>.values, little-endian
 * f32, for value rasters). On LM_DECODE_OK, *out_json is
 * {"name","look":"picture|values","bounds":[w,s,e,n],"png","values","grid":[w,h],"low","high",...}.
 */
int32_t lm_open_raster(
    const char *path,
    const char *out_stem,
    char **out_json);

/**
 * Re-read a NetCDF grid at another time step. For a file with a time axis the envelope also
 * carries {"times":["July 2023",...],"time":n}.
 *
 * Pass the low/high the first open returned so the colour ramp means the same at every step;
 * pass low >= high to let it work the range out itself.
 */
int32_t lm_open_raster_step(
    const char *path,
    const char *out_stem,
    int32_t time,
    double low,
    double high,
    const char *ramp,
    double west,
    double south,
    double east,
    double north,
    char **out_json);

/** Free a string returned by this library. No-op on NULL. */
void lm_string_free(char *s);

#ifdef __cplusplus
}
#endif
