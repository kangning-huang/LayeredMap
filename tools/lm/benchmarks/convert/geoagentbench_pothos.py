"""GeoAgentBench (pothos-dev, Daniel Allmer 2026) → sources/geoagentbench_pothos.jsonl.

Source: https://github.com/pothos-dev/geoagentbench, commit 1cd6f83b59c99048b3094f454de4a40b5ab4f749
(2026-06-18), tasks/<id>/task.json, 36 tasks. Licence: Apache-2.0 for code and task definitions
(LICENSE + NOTICE: "GeoAgentBench, Copyright 2026 Daniel Allmer"); the bundled OSM/Overture data
under tasks/*/inputs and reference is ODbL / CDLA-Permissive-2.0 and is not used here.

Not to be confused with GeoAgentBench / GABench by Yu et al. (arXiv 2604.13888); see
geoagentbench_paper.py.

Each record summarises the task in our words and classifies it; tasks today's actions carry get a
request adapted to our contexts (text_policy "adapted"). Most tasks end in a file deliverable in a
given format; we classify by the analysis and note when the export is the only part missing.

    python -I tools/lm/benchmarks/convert/geoagentbench_pothos.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, call, case, cond, dist, sel  # noqa: E402

SOURCE = "GeoAgentBench (pothos-dev)"
LICENCE = "Apache-2.0"
RAW = Path("/Volumes/LaCie/data/layeredmap-benchmarks/geoagentbench-pothos/src/tasks")
OUT = HERE.parent / "sources/geoagentbench_pothos.jsonl"


def short(tid: str) -> str:
    return "gap-" + tid


def rec(tid, summary, category, coverage, missing=(), cases=(), confidence="high", notes="", oos=None):
    r = {
        "source": SOURCE,
        "source_id": tid,
        "licence": LICENCE,
        "text_policy": "adapted",
        "summary": summary,
        "category": category,
        "coverage": coverage,
        "missing": list(missing),
        "confidence": confidence,
        "notes": notes,
        "cases": list(cases),
    }
    if oos:
        r["oos_reason"] = oos
    return r


def refuse(tid, request, context="city"):
    return [case(short(tid), request, CANNOT, context=context)]


R = [
    rec("crs-l1-london-laea-areas", "Area in km² of every administrative unit of London, as a table.",
        "measurement", "upstream", ["calculate_geometry"],
        notes="measure_area measures one feature; an area for every feature is calculate_geometry."),
    rec("crs-l1-nyc-webmercator-cycleways", "Convert Web-Mercator cycleways to lon/lat and write GeoParquet, keeping single LineStrings.",
        "projection", "upstream", ["reproject", "export_format"]),
    rec("crs-l1-paris-lambert93", "Convert Paris buildings to the official metric projection (Lambert-93) and write a GeoPackage.",
        "projection", "upstream", ["reproject", "export_format"]),
    rec("crs-l2-fiji-antimeridian", "Fix antimeridian-crossing transects, convert to Fiji's grid and add lengths in metres.",
        "projection", "new", ["fix_geometry", "reproject", "calculate_geometry"]),
    rec("crs-l2-svalbard-polar-areas", "Rank Svalbard glaciers by true area (top 20) with bounding boxes in a polar projection.",
        "measurement", "upstream", ["calculate_geometry", "reproject"], confidence="medium",
        notes="LayeredMap measures on the ellipsoid, so the honest area is easy; ranking needs an area field per glacier, and the polar bboxes need a reprojection."),
    rec("crs-l3-tokyo-jgd-crossings", "Fetch Tokyo wards and roads from Overpass, find road–boundary crossings, 50 m buffers, in the regional grid.",
        "data_acquisition", "out_of_scope", oos="internet_data",
        cases=refuse("crs-l3-tokyo-jgd-crossings", "Pull Tokyo's ward boundaries and drivable roads from OpenStreetMap via Overpass and find every place a road crosses a ward boundary.")),
    rec("dc-l1-bangkok-attribute-coercion", "Fix numeric columns stored as text in an air-quality station file so averages work.",
        "data_cleaning", "upstream", ["edit_attributes"]),
    rec("dc-l1-capetown-waterway-nulls", "Drop waterway features with no geometry or no type and report how many were dropped.",
        "data_cleaning", "new", ["fix_geometry", "export_format"], confidence="medium",
        notes="Dropping rows with an empty type is a filter; there is no 'is not empty' operator and no geometry-validity filter."),
    rec("dc-l1-tokyo-ring-orientation", "Repair building polygons with wrong ring orientation so viewers render holes correctly.",
        "data_cleaning", "new", ["fix_geometry"]),
    rec("dc-l2-cairo-invalid-dedup", "One canonical record per parcel: remove duplicates, slivers and invalid geometries.",
        "data_cleaning", "new", ["deduplicate", "fix_geometry"]),
    rec("dc-l2-lagos-snap-normalize", "Normalise zoning class names, fix vertex precision and dissolve one geometry per class with its area.",
        "data_cleaning", "new", ["edit_attributes", "fix_geometry", "dissolve", "calculate_geometry"]),
    rec("dc-l3-vienna-overpass-historical", "Fetch current and 2014 Vienna districts from Overpass and classify boundary changes.",
        "data_acquisition", "out_of_scope", oos="internet_data",
        cases=refuse("dc-l3-vienna-overpass-historical", "Download Vienna's district boundaries as they are today and as they were in 2014 from OpenStreetMap and show what changed.")),
    rec("dd-l1-capetown-clinics-bbox", "Clinic inventory from a CSV: total count, bounding box and count per health subdistrict.",
        "statistics", "upstream", ["xy_to_points", "frequency"], confidence="medium",
        notes="Counting per subdistrict is by an attribute, not by polygons, so count_in does not apply."),
    rec("dd-l1-london-parks-count", "How many inner-London parks are at least one hectare, their combined area and bounding box.",
        "measurement", "upstream", ["calculate_geometry", "summary_statistics"],
        notes="The parks have no area field, so filtering by size needs areas per feature first."),
    rec("dd-l1-vienna-gpkg-manifest", "Catalogue a GeoPackage: one record per layer with CRS, geometry type, count and bbox, as JSON.",
        "format_io", "out_of_scope", oos="code", confidence="low",
        notes="LayeredMap's import report shows this information, but writing a manifest file is not an action; refusing via the assistant is the honest answer.",
        cases=refuse("dd-l1-vienna-gpkg-manifest", "Write a JSON manifest of every layer in this GeoPackage with its CRS, geometry type, feature count and bounding box.")),
    rec("dd-l2-bangkok-multicrs-audit", "Audit a contractor GeoPackage: per layer declared CRS, geometry type, count, a sample coordinate and text encoding.",
        "format_io", "out_of_scope", oos="code", confidence="low",
        cases=refuse("dd-l2-bangkok-multicrs-audit", "Audit every layer of the contractor's GeoPackage: declared CRS, geometry type, feature count, a raw sample coordinate and whether its text is mis-encoded, as a CSV.")),
    rec("dd-l2-tokyo-overture-schools", "Schools for ages 8–14 from a places layer that fall inside a study-area rectangle, exported as GeoJSON.",
        "spatial_join", "now", confidence="medium",
        notes="The selection is filter + whats_inside; writing the GeoJSON with fixed property names is not covered (export_format).",
        cases=[
            case(short("dd-l2-tokyo-overture-schools"),
                 "From Places, show the schools for children aged 8 to 14 that lie inside the study area.",
                 [call("filter", layer="Places", conditions=[cond("category", "is", "school")]),
                  call("whats_inside", area=sel("Study area"), layer="Places")],
                 context="city_open_data", selection={"layer": "Study area", "count": 1}),
        ]),
    rec("dd-l3-lagos-overture-buildings", "Download Lagos building footprints over 1000 m² from Overture and summarise them per LGA.",
        "data_acquisition", "out_of_scope", oos="internet_data",
        cases=refuse("dd-l3-lagos-overture-buildings", "Get every building larger than 1000 m² in Lagos State from Overture Maps and summarise them per local government area.", "world_plus")),
    rec("fio-l1-nyc-csvwkt-addresses", "Convert a CSV with WKT points to typed GeoParquet.",
        "format_io", "upstream", ["export_format"]),
    rec("fio-l1-paris-kml-pois", "Convert KML placemarks to GeoJSON with category and a date parsed from the description.",
        "format_io", "upstream", ["export_format", "calculate_field"]),
    rec("fio-l1-vienna-shapefile-recovery", "Recover full column names and encoding of an old shapefile and write GeoJSON.",
        "format_io", "upstream", ["export_format", "edit_attributes"]),
    rec("fio-l2-cairo-mixedgeom-split", "Split mixed-geometry heritage features into single-part point, line and polygon layers in Egypt's grid.",
        "format_io", "upstream", ["explode", "reproject", "export_format"]),
    rec("fio-l2-capetown-landuse-dissolve", "One row per land-use class: dissolved geometry, total area and parcel count.",
        "generalization", "upstream", ["dissolve", "calculate_geometry", "export_format"]),
    rec("fio-l3-vienna-geofabrik-highways", "Download the Austria PBF and extract highways and transit routes near Vienna's Gürtel.",
        "data_acquisition", "out_of_scope", oos="internet_data",
        cases=refuse("fio-l3-vienna-geofabrik-highways", "Download the Geofabrik Austria extract and give me every road and public-transport route within 500 m of Vienna's Gürtel.")),
    rec("geo-l1-cairo-multipoint-hull", "Convex hull of each metro station's entrances, one polygon per station.",
        "generalization", "upstream", ["convex_hull"], notes="Per-station grouping needs a group field; tools::convex_hull offers all-or-each."),
    rec("geo-l1-capetown-building-centroids", "A centroid point for every building footprint, keeping building ids.",
        "generalization", "upstream", ["centroid"]),
    rec("geo-l1-tokyo-busstop-buffer", "A 400 m catchment polygon around every bus stop, one per stop, keeping its id.",
        "buffer", "now", confidence="medium",
        notes="The buffer is now; writing GeoParquet is not covered (export_format).",
        cases=[
            case(short("geo-l1-tokyo-busstop-buffer"), "Draw a 400 m catchment around every bus stop, one separate area per stop.",
                 [call("buffer", layer="Bus stops", distance=dist(400), merge=False)], context="city"),
            case(short("geo-l1-tokyo-busstop-buffer") + "-zh", "给每个公交站画400米的服务范围，每个站单独一个区域",
                 [call("buffer", layer="Bus stops", distance=dist(400), merge=False)], context="city", lang="zh"),
        ]),
    rec("geo-l2-bangkok-landuse-intersect", "Land cover inside the study boundary, simplified, with area per feature.",
        "overlay", "upstream", ["clip", "simplify", "calculate_geometry"]),
    rec("geo-l2-nyc-park-symdiff", "Patches where official and OSM park layers disagree, merged into groups with label points.",
        "overlay", "new", ["symmetric_difference", "dissolve", "centroid"]),
    rec("geo-l3-antarctica-stations-geodesic", "Fetch Antarctic stations from Overture, 200 km zones clipped to land and grouped where they overlap.",
        "data_acquisition", "out_of_scope", oos="internet_data",
        cases=refuse("geo-l3-antarctica-stations-geodesic", "Fetch all Antarctic research stations from Overture Maps and draw their 200 km operating zones over land.", "world_plus")),
    rec("spa-l1-capetown-hospital-nn", "Nearest hospital and its straight-line distance for every address.",
        "proximity", "upstream", ["nearest"]),
    rec("spa-l1-paris-amenity-within", "Tag every amenity with the arrondissement it falls in, as a table.",
        "spatial_join", "new", ["spatial_join"],
        notes="which_container answers it for one selected point, not for every amenity."),
    rec("spa-l1-vienna-pip-count", "Number of air-quality monitoring stations in every district, districts with none included.",
        "spatial_join", "now",
        notes="count_in keeps areas with no points (count 0). The original asks for a CSV sorted by district code.",
        cases=[
            case(short("spa-l1-vienna-pip-count"), "How many air quality stations are in each neighbourhood? Include neighbourhoods with none.",
                 [call("count_in", points="Air quality stations", areas="Neighbourhoods")], context="city"),
            case(short("spa-l1-vienna-pip-count") + "-zh", "每个街区有几个空气质量监测站？没有监测站的街区也要列出来",
                 [call("count_in", points="Air quality stations", areas="Neighbourhoods")], context="city", lang="zh"),
        ]),
    rec("spa-l2-cairo-shop-knn", "Five nearest shops to each retail anchor with distances, and a shop-to-anchor distance matrix.",
        "proximity", "new", ["nearest", "distance_matrix", "edit_attributes"]),
    rec("spa-l2-lagos-hotspot-overlaps", "Top 10 % of hexagons by area-weighted population density of overlapping land-use polygons.",
        "spatial_join", "new", ["polygon_in_polygon_summary"]),
    rec("spa-l3-paris-emergency-routing", "Fetch Paris roads and hospitals from Overpass; routes to the nearest hospital, distance matrix and 15-minute isochrones.",
        "network", "out_of_scope", oos="internet_data",
        notes="With local roads it would need shortest_route (upstream), od_matrix and service_area (new).",
        cases=refuse("spa-l3-paris-emergency-routing", "Pull the driveable roads and hospitals of central Paris from OpenStreetMap and draw a 15-minute drive-time area around each hospital.")),
]


def main() -> None:
    ids = [r["source_id"] for r in R]
    assert len(ids) == len(set(ids)) == 36, len(ids)
    if RAW.exists():
        raw = {p.name for p in RAW.iterdir() if (p / "task.json").exists()}
        assert raw == set(ids), raw ^ set(ids)
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in R), encoding="utf-8")
    print(f"{OUT.name}: {len(R)} records")


if __name__ == "__main__":
    main()
