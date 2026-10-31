/* Spike-only C ABI for NimbleMap iPhone companion (spikes/ios-companion-alt-b).
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

/** Free a string returned by this library. No-op on NULL. */
void lm_string_free(char *s);

#ifdef __cplusplus
}
#endif
