"""Build the KML/KMZ (V11) and GPX (V12) fixtures and their GDAL reference expectations.

The fixture files are written from the literal texts below (synthetic, CC0-1.0), the KMZ with
fixed zip timestamps, so reruns give the same bytes. Each `expected.json` then records what
GDAL reads from them (pyogrio, drivers LIBKML for KML/KMZ and GPX): per GDAL layer the number
of features with a geometry, the number of vertices, the bounds and the sums of the vertices’
longitudes and latitudes (a check on every coordinate). LayeredMap's tests compare
their own layers against those numbers (`crates/lm-jobs/src/import/xml_tests.rs`).

Definitions where LayeredMap and GDAL lay a file out differently:
- KML: GDAL (LIBKML) makes one layer per folder plus one for places outside any folder;
  LayeredMap does the same, but splits a folder mixing points, lines and areas into one layer
  per kind, so for a split folder only vertices and bounds are compared, not feature counts.
- GPX: GDAL's `waypoints`, `tracks` and `routes` layers (its `track_points` and
  `route_points` layers have no LayeredMap counterpart). A GPX track is one multi-line in both.
- GDAL's LIBKML reads a GroundOverlay as a polygon; it is switched off here
  (`LIBKML_READ_GROUND_OVERLAY=NO`), as LayeredMap leaves overlays out with a notice.
- Places GDAL reads but LayeredMap excludes (out-of-range or broken coordinates) appear only in
  the malformed fixtures, whose expectations are LayeredMap's own (listed in each README).

Usage:
  python -I reference/make_kml_gpx_fixtures.py
"""

from __future__ import annotations

import json
import os
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common  # noqa: E402, F401  (pins GDAL/PROJ data first)

# GDAL's LIBKML reads a GroundOverlay as a polygon; LayeredMap leaves overlays out (notice).
os.environ["LIBKML_READ_GROUND_OVERLAY"] = "NO"
from _common import FIX, FIXED_ZIP_TIME  # noqa: E402

import pyogrio  # noqa: E402
import shapely  # noqa: E402

V11 = FIX / "V11"
V12 = FIX / "V12"

PLACES_KML = """<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2">
<Document>
  <name>Field trip</name>
  <Style id="schoolIcon"><IconStyle><color>ff0000ff</color><scale>1.1</scale></IconStyle></Style>
  <Style id="schoolIconHi"><IconStyle><color>ff00ffff</color><scale>1.4</scale></IconStyle></Style>
  <StyleMap id="schoolMap">
    <Pair><key>normal</key><styleUrl>#schoolIcon</styleUrl></Pair>
    <Pair><key>highlight</key><styleUrl>#schoolIconHi</styleUrl></Pair>
  </StyleMap>
  <Style id="riverLine"><LineStyle><color>ffff0000</color><width>3</width></LineStyle></Style>
  <Style id="parkPoly">
    <LineStyle><color>ff006400</color><width>2</width></LineStyle>
    <PolyStyle><color>8000ff00</color></PolyStyle>
  </Style>
  <Schema name="info" id="info"><SimpleField type="string" name="district"/></Schema>
  <Placemark>
    <name>Visitor centre</name>
    <Point><coordinates>121.4700,31.2300,0</coordinates></Point>
  </Placemark>
  <Folder>
    <name>Schools</name>
    <Placemark>
      <name>North school</name>
      <description><![CDATA[<b>Open</b> 8:00-16:00]]></description>
      <styleUrl>#schoolMap</styleUrl>
      <ExtendedData>
        <Data name="pupils"><value>420</value></Data>
        <SchemaData schemaUrl="#info"><SimpleData name="district">Jing'an</SimpleData></SchemaData>
      </ExtendedData>
      <Point><coordinates>121.4500,31.2500</coordinates></Point>
    </Placemark>
    <Placemark>
      <name>East school</name>
      <styleUrl>#schoolMap</styleUrl>
      <ExtendedData>
        <Data name="pupils"><value>385</value></Data>
        <SchemaData schemaUrl="#info"><SimpleData name="district">Hongkou</SimpleData></SchemaData>
      </ExtendedData>
      <Point><coordinates>121.5000,31.2600</coordinates></Point>
    </Placemark>
    <Placemark>
      <name>South school</name>
      <styleUrl>#schoolMap</styleUrl>
      <ExtendedData>
        <Data name="pupils"><value>512</value></Data>
        <SchemaData schemaUrl="#info"><SimpleData name="district">Xuhui</SimpleData></SchemaData>
      </ExtendedData>
      <Point><coordinates>121.4400,31.1900</coordinates></Point>
    </Placemark>
    <Placemark>
      <name>Planned school</name>
      <description>Site not chosen yet</description>
      <styleUrl>#schoolMap</styleUrl>
    </Placemark>
  </Folder>
  <Folder>
    <name>Rivers</name>
    <Placemark>
      <name>Suzhou Creek</name>
      <styleUrl>#riverLine</styleUrl>
      <TimeSpan><begin>2024-05-01</begin><end>2024-05-31</end></TimeSpan>
      <LineString><tessellate>1</tessellate><coordinates>
        121.3800,31.2400,0 121.4200,31.2450,0 121.4600,31.2420,0 121.4900,31.2440,0
      </coordinates></LineString>
    </Placemark>
    <Placemark>
      <name>Huangpu (part)</name>
      <styleUrl>#riverLine</styleUrl>
      <LineString><coordinates>121.4900,31.2440 121.5000,31.2300 121.4950,31.2100 121.4800,31.1900</coordinates></LineString>
    </Placemark>
  </Folder>
  <Folder>
    <name>Parks</name>
    <Placemark>
      <name>Square park</name>
      <styleUrl>#parkPoly</styleUrl>
      <Polygon><outerBoundaryIs><LinearRing><coordinates>
        121.4600,31.2250 121.4700,31.2250 121.4700,31.2350 121.4600,31.2350 121.4600,31.2250
      </coordinates></LinearRing></outerBoundaryIs></Polygon>
    </Placemark>
    <Placemark>
      <name>Lake park</name>
      <styleUrl>#parkPoly</styleUrl>
      <Polygon>
        <outerBoundaryIs><LinearRing><coordinates>
          121.4000,31.2000 121.4300,31.2000 121.4300,31.2200 121.4000,31.2200 121.4000,31.2000
        </coordinates></LinearRing></outerBoundaryIs>
        <innerBoundaryIs><LinearRing><coordinates>
          121.4100,31.2050 121.4200,31.2050 121.4200,31.2150 121.4100,31.2150 121.4100,31.2050
        </coordinates></LinearRing></innerBoundaryIs>
      </Polygon>
    </Placemark>
    <Placemark>
      <name>Twin gardens</name>
      <styleUrl>#parkPoly</styleUrl>
      <MultiGeometry>
        <Polygon><outerBoundaryIs><LinearRing><coordinates>
          121.5200,31.2000 121.5300,31.2000 121.5300,31.2100 121.5200,31.2000
        </coordinates></LinearRing></outerBoundaryIs></Polygon>
        <Polygon><outerBoundaryIs><LinearRing><coordinates>
          121.5400,31.2000 121.5500,31.2000 121.5500,31.2100 121.5400,31.2000
        </coordinates></LinearRing></outerBoundaryIs></Polygon>
      </MultiGeometry>
    </Placemark>
  </Folder>
  <Folder>
    <name>Survey</name>
    <Placemark>
      <name>Transect A</name>
      <styleUrl>#riverLine</styleUrl>
      <MultiGeometry>
        <Point><coordinates>121.4000,31.3000</coordinates></Point>
        <LineString><coordinates>121.4000,31.3000 121.4200,31.3100</coordinates></LineString>
      </MultiGeometry>
    </Placemark>
    <Placemark>
      <name>Transect B</name>
      <Style><LineStyle><color>ff0000ff</color><width>1.5</width></LineStyle></Style>
      <LineString><coordinates>121.4300,31.3000 121.4500,31.3100</coordinates></LineString>
    </Placemark>
  </Folder>
  <NetworkLink>
    <name>Live buses</name>
    <Link><href>https://example.com/buses.kml</href></Link>
  </NetworkLink>
  <GroundOverlay>
    <name>Old map</name>
    <Icon><href>https://example.com/old_map.jpg</href></Icon>
    <LatLonBox><north>31.30</north><south>31.15</south><east>121.55</east><west>121.38</west></LatLonBox>
  </GroundOverlay>
</Document>
</kml>
"""

TRAIL_DOC_KML = """<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2">
<Document>
  <name>Trail</name>
  <Style id="trail"><LineStyle><color>ff00a5ff</color><width>4</width></LineStyle></Style>
  <Style id="stop"><IconStyle><color>ff800080</color><Icon><href>files/stop.png</href></Icon></IconStyle></Style>
  <Placemark>
    <name>Morning walk</name>
    <styleUrl>#trail</styleUrl>
    <gx:Track>
      <when>2024-06-01T07:00:00Z</when><when>2024-06-01T07:10:00Z</when><when>2024-06-01T07:20:00Z</when>
      <gx:coord>116.3900 39.9000 50</gx:coord>
      <gx:coord>116.3950 39.9050 52</gx:coord>
      <gx:coord>116.4000 39.9080 55</gx:coord>
    </gx:Track>
  </Placemark>
  <Placemark>
    <name>Start</name>
    <styleUrl>#stop</styleUrl>
    <Point><coordinates>116.3900,39.9000,50</coordinates></Point>
  </Placemark>
  <Placemark>
    <name>Tea house</name>
    <styleUrl>#stop</styleUrl>
    <Point><coordinates>116.4000,39.9080,55</coordinates></Point>
  </Placemark>
</Document>
</kml>
"""

# A 1×1 transparent PNG: stands in for an icon packed in the KMZ.
PNG_1X1 = bytes.fromhex(
    "89504e470d0a1a0a0000000d4948445200000001000000010806000000"
    "1f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082")

HIKE_GPX = """<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="LayeredMap fixtures" xmlns="http://www.topografix.com/GPX/1/1">
  <metadata><name>Hike</name><time>2024-04-20T06:00:00Z</time></metadata>
  <wpt lat="46.5000" lon="7.9000"><ele>1034.5</ele><time>2024-04-20T06:05:00Z</time><name>Car park</name><sym>Parking Area</sym></wpt>
  <wpt lat="46.5200" lon="7.9300"><ele>1620</ele><time>2024-04-20T08:30:00Z</time><name>Hut</name><desc>Water and food</desc><sym>Lodge</sym></wpt>
  <wpt lat="46.5350" lon="7.9500"><ele>2105.2</ele><name>Summit</name><sym>Summit</sym></wpt>
  <rte>
    <name>Planned route</name>
    <rtept lat="46.5000" lon="7.9000"/>
    <rtept lat="46.5100" lon="7.9150"/>
    <rtept lat="46.5200" lon="7.9300"/>
    <rtept lat="46.5350" lon="7.9500"/>
  </rte>
  <trk>
    <name>Ascent</name>
    <type>hiking</type>
    <trkseg>
      <trkpt lat="46.5000" lon="7.9000"><ele>1034.5</ele><time>2024-04-20T06:10:00Z</time></trkpt>
      <trkpt lat="46.5040" lon="7.9060"><ele>1150.0</ele><time>2024-04-20T06:40:00Z</time></trkpt>
      <trkpt lat="46.5090" lon="7.9120"><ele>1290.2</ele><time>2024-04-20T07:10:00Z</time></trkpt>
      <trkpt lat="46.5140" lon="7.9190"><ele>1420.8</ele><time>2024-04-20T07:40:00Z</time></trkpt>
      <trkpt lat="46.5170" lon="7.9240"><ele>1510.0</ele><time>2024-04-20T08:00:00Z</time></trkpt>
    </trkseg>
    <trkseg>
      <trkpt lat="46.5180" lon="7.9260"><ele>1540.3</ele><time>2024-04-20T08:10:00Z</time></trkpt>
      <trkpt lat="46.5200" lon="7.9300"><ele>1620.0</ele><time>2024-04-20T08:30:00Z</time></trkpt>
      <trkpt lat="46.5280" lon="7.9400"><ele>1880.4</ele><time>2024-04-20T09:30:00Z</time></trkpt>
      <trkpt lat="46.5350" lon="7.9500"><ele>2105.2</ele><time>2024-04-20T10:40:00Z</time></trkpt>
    </trkseg>
  </trk>
  <trk>
    <name>Descent (no clock)</name>
    <trkseg>
      <trkpt lat="46.5350" lon="7.9500"><ele>2105.2</ele></trkpt>
      <trkpt lat="46.5250" lon="7.9450"><ele>1750.0</ele></trkpt>
      <trkpt lat="46.5000" lon="7.9000"><ele>1034.5</ele></trkpt>
    </trkseg>
  </trk>
</gpx>
"""

# GPX 1.0 in ISO-8859-1 (as older Garmin software writes it): "Café" is one byte é (0xE9).
OLD_GPX = ("""<?xml version="1.0" encoding="ISO-8859-1"?>
<gpx version="1.0" creator="old device" xmlns="http://www.topografix.com/GPX/1/0">
  <wpt lat="48.8584" lon="2.2945"><ele>35</ele><name>Café de la Tour</name></wpt>
  <wpt lat="48.8606" lon="2.3376"><ele>38</ele><name>Musée</name></wpt>
</gpx>
""").encode("iso-8859-1")

MALFORMED_KML = {
    "truncated.kml": PLACES_KML[: PLACES_KML.index("<name>Rivers</name>") + 10],
    "not_kml.kml": "<?xml version=\"1.0\"?>\n<html><body><p>Not a map.</p></body></html>\n",
    "only_links.kml": """<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <NetworkLink><name>Feed</name><Link><href>https://example.com/feed.kml</href></Link></NetworkLink>
  <NetworkLink><name>Feed 2</name><Link><href>https://example.com/feed2.kml</href></Link></NetworkLink>
  <GroundOverlay><name>Scan</name><Icon><href>scan.jpg</href></Icon>
    <LatLonBox><north>1</north><south>0</south><east>1</east><west>0</west></LatLonBox></GroundOverlay>
</Document></kml>
""",
    "bad_coords.kml": """<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <Placemark><name>good</name><Point><coordinates>10,20</coordinates></Point></Placemark>
  <Placemark><name>words</name><Point><coordinates>abc,def</coordinates></Point></Placemark>
  <Placemark><name>beyond the pole</name><Point><coordinates>10,95</coordinates></Point></Placemark>
  <Placemark><name>two corners</name><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 1,1</coordinates></LinearRing></outerBoundaryIs></Polygon></Placemark>
  <Placemark><name>one point line</name><LineString><coordinates>5,5</coordinates></LineString></Placemark>
  <Placemark><name>model</name><Model><Location><longitude>1</longitude><latitude>1</latitude></Location></Model></Placemark>
</Document></kml>
""",
    "dtd.kml": """<?xml version="1.0"?>
<!DOCTYPE kml [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;"><!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;">]>
<kml xmlns="http://www.opengis.net/kml/2.2"><Placemark><name>&c;</name><Point><coordinates>1,1</coordinates></Point></Placemark></kml>
""",
}

MALFORMED_GPX = {
    "truncated.gpx": HIKE_GPX[: HIKE_GPX.index("<trk>") + 30],
    "bad_points.gpx": """<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="fixtures" xmlns="http://www.topografix.com/GPX/1/1">
  <wpt lat="10.0" lon="20.0"><name>ok</name></wpt>
  <wpt lon="20.0"><name>no latitude</name></wpt>
  <wpt lat="north" lon="20.0"><name>words</name></wpt>
  <rte><name>one point</name><rtept lat="10.0" lon="20.0"/></rte>
  <trk><name>one bad point</name><trkseg>
    <trkpt lat="10.0" lon="20.0"/><trkpt lat="999" lon="20.1"/><trkpt lat="10.2" lon="20.2"/>
  </trkseg></trk>
</gpx>
""",
}

V11_README = """# V11 — KML and KMZ

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
"""

V12_README = """# V12 — GPX

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
"""


def write(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data.encode("utf-8") if isinstance(data, str) else data)


def write_zip(path: Path, members: list[tuple[str, bytes]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in members:
            info = zipfile.ZipInfo(name, date_time=FIXED_ZIP_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, data)


def r9(v: float) -> float:
    return round(float(v), 9)


def gdal_layers(path: Path, driver: str, only: list[str] | None = None) -> list[dict]:
    """Per GDAL layer: features with a geometry, vertex count, bounds and coordinate sums."""
    out = []
    for name, _ in pyogrio.list_layers(path):
        if only is not None and name not in only:
            continue
        kw = {"driver": driver} if driver != "GPX" else {}
        meta, _, geoms, _ = pyogrio.raw.read(path, layer=name, read_geometry=True, **kw)
        shapes = [shapely.from_wkb(g) for g in geoms if g is not None]
        shapes = [s for s in shapes if s is not None and not s.is_empty]
        coords = shapely.get_coordinates(shapes) if shapes else []
        if len(coords) == 0:
            bounds = None
        else:
            xs, ys = coords[:, 0], coords[:, 1]
            bounds = [r9(xs.min()), r9(ys.min()), r9(xs.max()), r9(ys.max())]
        sums = [round(float(coords[:, 0].sum()), 6), round(float(coords[:, 1].sum()), 6)] if len(coords) else None
        out.append({"layer": name, "count": len(shapes), "vertices": int(len(coords)), "bounds": bounds, "sum_xy": sums})
    return out


def main() -> None:
    write(V11 / "places.kml", PLACES_KML)
    write_zip(V11 / "trail.kmz", [("doc.kml", TRAIL_DOC_KML.encode()), ("files/stop.png", PNG_1X1)])
    for name, text in MALFORMED_KML.items():
        write(V11 / "malformed" / name, text)
    write_zip(V11 / "malformed" / "no_kml.kmz", [("readme.txt", b"No KML here.\n")])
    write(V11 / "README.md", V11_README)

    write(V12 / "hike.gpx", HIKE_GPX)
    write(V12 / "old_garmin.gpx", OLD_GPX)
    for name, text in MALFORMED_GPX.items():
        write(V12 / "malformed" / name, text)
    write(V12 / "README.md", V12_README)

    v11 = {
        "fixture": "V11",
        "files": ["places.kml", "trail.kmz"],
        "kind": "vector",
        "gdal": {
            "version": pyogrio.__gdal_version_string__,
            "places.kml": gdal_layers(V11 / "places.kml", "LIBKML"),
            "trail.kmz": gdal_layers(V11 / "trail.kmz", "LIBKML"),
        },
        "layeredmap": {
            "places.kml": {
                "crs": {"epsg": 4326, "source": "declared (KML is WGS 84 by definition)"},
                "notice": "network link",
                "layers": [
                    {"name": "places", "group": "Field trip", "geometry": "point", "count": 1, "style": None},
                    {"name": "Schools", "group": "Schools", "geometry": "point", "count": 3, "excluded": 1,
                     "style": {"fill": [255, 0, 0, 255]},
                     "fields": ["name", "description", "pupils", "district"]},
                    {"name": "Rivers", "group": "Rivers", "geometry": "line", "count": 2,
                     "style": {"stroke": [0, 0, 255, 255], "line_width": 3}},
                    {"name": "Parks", "group": "Parks", "geometry": "area", "count": 3,
                     "style": {"fill": [0, 255, 0, 255], "fill_opacity": 128 / 255, "stroke": [0, 100, 0, 255],
                               "stroke_width": 2}},
                    {"name": "Survey (points)", "group": "Survey", "geometry": "point", "count": 1, "style": None},
                    {"name": "Survey (lines)", "group": "Survey", "geometry": "line", "count": 2,
                     "style": {"stroke": [0, 0, 255, 255], "line_width": 3}, "mixed_styles": ["riverLine", "line #FF0000"]},
                ],
            },
            "trail.kmz": {
                "layers": [
                    {"name": "trail (points)", "group": "Trail", "geometry": "point", "count": 2,
                     "style": {"fill": [128, 0, 128, 255]}},
                    {"name": "trail (lines)", "group": "Trail", "geometry": "line", "count": 1,
                     "style": {"stroke": [255, 165, 0, 255], "line_width": 4}},
                ],
            },
        },
        "malformed": {
            "truncated.kml": "error: isn't valid KML",
            "not_kml.kml": "error: isn't a KML file",
            "only_links.kml": "unsupported: holds no places",
            "bad_coords.kml": {"count": 1, "excluded": 5},
            "dtd.kml": "error: DTD",
            "no_kml.kmz": "unsupported: holds no KML document",
        },
    }
    (V11 / "expected.json").write_text(json.dumps(v11, indent=2, ensure_ascii=False) + "\n")

    gpx_layers = ["waypoints", "routes", "tracks"]
    v12 = {
        "fixture": "V12",
        "files": ["hike.gpx", "old_garmin.gpx"],
        "kind": "vector",
        "gdal": {
            "version": pyogrio.__gdal_version_string__,
            "hike.gpx": gdal_layers(V12 / "hike.gpx", "GPX", gpx_layers),
            "old_garmin.gpx": gdal_layers(V12 / "old_garmin.gpx", "GPX", gpx_layers),
        },
        "layeredmap": {
            "hike.gpx": {
                "layers": [
                    {"name": "hike (waypoints)", "group": "waypoints", "geometry": "point", "count": 3,
                     "fields": ["name", "sym", "ele", "time", "desc"]},
                    {"name": "hike (tracks)", "group": "tracks", "geometry": "line", "count": 2,
                     "fields": ["name", "type", "start_time", "end_time", "min_ele", "max_ele"],
                     "first": {"start_time": "2024-04-20T06:10:00Z", "end_time": "2024-04-20T10:40:00Z",
                               "min_ele": 1034.5, "max_ele": 2105.2, "parts": 2}},
                    {"name": "hike (routes)", "group": "routes", "geometry": "line", "count": 1},
                ],
            },
            "old_garmin.gpx": {
                "layers": [{"name": "old_garmin", "group": "waypoints", "geometry": "point", "count": 2,
                            "names": ["Café de la Tour", "Musée"]}],
            },
        },
        "malformed": {
            "truncated.gpx": "error: isn't valid GPX",
            "bad_points.gpx": {"waypoints": {"count": 1, "excluded": 2}, "routes": "no layer; notice on the first layer",
                               "tracks": {"count": 1, "vertices": 2, "notice": "left out"}},
        },
    }
    (V12 / "expected.json").write_text(json.dumps(v12, indent=2, ensure_ascii=False) + "\n")
    print("V11, V12 written; GDAL", pyogrio.__gdal_version_string__)


if __name__ == "__main__":
    main()
