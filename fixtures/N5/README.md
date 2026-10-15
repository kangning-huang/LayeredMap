# N5 — Geodesic accuracy (equator, 60°N, antimeridian)

`cases.geojson`: points, lines and areas at the equator, at 60°N and at the
antimeridian. Antimeridian features are encoded per RFC 7946 (split at 180° into
Multi* parts).

Expected (from `reference/geodesic.py`, WGS 84 ellipsoid):

- `expected_measure.csv`: geodesic length (lines) and area/perimeter (areas);
  edges between vertices are geodesics (pyproj.Geod). For `poly_am` the
  perimeter is left blank because the split encoding adds an artificial 180°
  seam; the area is the sum of both parts.
- `expected_buffer.csv`: buffer areas for 1 km and 50 km. Points: a 360-vertex
  geodesic circle (Geod.fwd). Lines/areas: densified buffer in a local azimuthal
  equidistant projection centred on the feature, projected back and measured
  with Geod. Suggested tolerance 1e-3 relative (the 360-gon under-estimates a
  true circle by ~5e-5).

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
