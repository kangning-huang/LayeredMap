# V12 — GPX

- `hike.gpx`: GPX 1.1 in the Bernese Oberland. 3 waypoints (elevation; 2 with a time), 1 route
  of 4 points, and 2 tracks: `Ascent` (2 segments, 5 + 4 points, with elevations and times) and
  `Descent (no clock)` (1 segment of 3 points, elevations, no times).
- `old_garmin.gpx`: GPX 1.0 in ISO-8859-1 (2 waypoints named "Café de la Tour" and "Musée",
  é as the single byte 0xE9): the declared encoding is used.
- `malformed/`: `truncated.gpx` (cut mid-file) and `bad_points.gpx` (a waypoint without
  latitude and one with words are excluded; the only route has 1 point, so there is no routes
  layer and a notice says so; a track with one point
  at latitude 999 keeps its other 2 points, with a notice).

`expected.json` holds GDAL's reading (GPX driver, through pyogrio) of `hike.gpx` and
`old_garmin.gpx`: features with a geometry, vertices and bounds of the `waypoints`, `routes`
and `tracks` layers, plus the fields LayeredMap should add. The malformed files' outcomes are
LayeredMap's own.

Made by `reference/make_kml_gpx_fixtures.py` (synthetic, CC0-1.0).
