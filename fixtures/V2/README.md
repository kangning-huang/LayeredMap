# V2 — Mixed geometry GeoJSON

`mixed.geojson`: 10 features in one FeatureCollection, interleaved: 4 points,
3 lines (2 LineString + 1 MultiLineString), 3 areas (2 Polygon + 1 MultiPolygon).
The `kind` property states the intended layer.

Tests: mixed-geometry split — expect **3 layers** (points 4, lines 3, areas 3);
Multi* types go with their single-part kind.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
