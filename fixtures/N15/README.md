# N15 — Keep data inside an area, against GeoPandas `clip`

Inputs for `lm_analysis::clip::keep_inside` (WP6.4, docs/lm/actions-p2.md › Keep data inside
an area). Coordinates are 116° E, 39.75° N plus multiples of 1/64° (so every input vertex and
most crossings are exact binary fractions); Z2's slanted edges make crossings that are not.

- `zone.geojson` (`zone_id`): Z1 the square [0, 8]² with the hole [3, 5]²; Z2 a triangle
  (6, 2), (12, 2), (9, 7) overlapping Z1's east side; Z3 the square [14, 18] × [0, 4] on its own.
- `areas.geojson` (`id`): across a corner, sharing an edge only, touching a corner only,
  covering the whole mask, two parts with one inside, a holed area across Z2 and Z3, inside the
  mask's hole, far outside, and no geometry.
- `lines.geojson` (`id`): across Z1 and Z2 with a gap, along Z1's south edge, ending on a
  corner, through the hole, two parts with one inside, outside, and out and back in.
- `points.geojson` (`id`): inside, on an edge, on a corner, in the hole, on the hole's edge,
  outside, a two-point multipoint, on Z2's slanted edge, on Z3's corner.

Expected (from `tools/lm/reference_tests/clip_gpd.py`): `expected_clip.csv`, one row per
input feature: `kept`, `parts`, the geodesic area (pyproj Geod, WGS 84) or length, and for
lines and points the coordinates left (lines merged with `shapely.line_merge`, each part from
its smaller end, parts sorted, `|` between parts). Reference: GeoPandas `clip(gdf, zone,
keep_geom_type=True)`.

Tolerances: the same features kept and the same number of parts, exactly; areas and lengths
within 1e-6 relative; coordinates within 1e-9° (input vertices come back exactly).

Made by `tools/lm/reference_tests/make_p2_fixtures.py` (synthetic, CC0-1.0).
