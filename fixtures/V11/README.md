# V11 — KML and KMZ

- `places.kml`: a KML 2.2 document "Field trip" near Shanghai.
  - Outside any folder: `Visitor centre` (a point, no style).
  - `Schools`: 3 points and 1 place with no shape (`Planned school`), all styled through the
    StyleMap `schoolMap` (normal → `schoolIcon`, a red icon `ff0000ff`). Fields from `name`,
    `description` (CDATA HTML), `ExtendedData/Data` (`pupils`) and `SchemaData` (`district`).
  - `Rivers`: 2 lines, style `riverLine` (blue `ffff0000`, width 3); one has a `TimeSpan`.
  - `Parks`: 2 polygons (one with a hole) and a MultiGeometry of 2 polygons, style `parkPoly`
    (outline dark green `ff006400` width 2, fill green at alpha `80`).
  - `Survey`: a MultiGeometry mixing a point and a line (style `riverLine`), and a line with its
    own inline red style: a folder that splits into points and lines, and lines with two
    different styles (a tie, so the first seen, `riverLine`, colours the layer).
  - A `NetworkLink` and a `GroundOverlay`, which LayeredMap doesn't open (notice).
- `trail.kmz`: `doc.kml` (a `gx:Track` of 3 points styled orange `ff00a5ff` width 4, and 2
  points with a purple icon `ff800080`) plus `files/stop.png`, zipped with fixed timestamps.
- `malformed/`: `truncated.kml` (cut mid-file), `not_kml.kml` (an HTML page), `only_links.kml`
  (2 network links and a ground overlay, no places), `bad_coords.kml` (1 good point; words for
  coordinates, latitude 95, a 2-corner area, a 1-point line and a 3D model are excluded with
  reasons), `dtd.kml` (entity expansion, refused), `no_kml.kmz` (a zip with only a text file).

`expected.json` holds GDAL's reading (LIBKML driver, through pyogrio) of `places.kml` and
`trail.kmz`: features with a geometry, vertices and bounds per GDAL layer, and the styles and
layers LayeredMap should show. The malformed files' outcomes are LayeredMap's own (listed in
`expected.json` under `malformed`).

Made by `reference/make_kml_gpx_fixtures.py` (synthetic, CC0-1.0).
