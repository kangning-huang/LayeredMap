"""GeoJSON Agents (Luo et al., arXiv 2509.08863): the 70 test cases of its Appendices 3–5 (40 basic,
20 intermediate, 10 advanced), summarised in our own words and mapped to LayeredMap.

No code or data release was found and the paper states no data licence, so nothing is copied:
summaries, requests and plans are ours. The PDF's two-column appendix tables extract poorly; a few
task boundaries are uncertain (noted per record, confidence lowered).

    python -I tools/lm/benchmarks/convert/geojson_agent.py   → sources/geojson_agent.jsonl
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))  # python -I drops the script's folder

from f4_common import call, case, cond, dist, rec, refuse, write  # noqa: E402

SRC = "GeoJSON Agent"
LIC = "arXiv paper (2509.08863) appendix tables; no code or data released; no data licence stated"
C = "copilot_us"
PA = cond("state", "is", "Pennsylvania")
CONV = "VOCAB_GAP: geometry_conversion (tagged polygon_to_line)"


def R(sid, summary, category, coverage, **kw):
    tier = sid.split("-")[0]
    notes = kw.pop("notes", "")
    kw["notes"] = f"tier: {tier}" + (f". {notes}" if notes else "")
    return rec(SRC, LIC, sid, summary, category, coverage, **kw)


records = [
    # ------------------------------------------------------------------ basic (Appendix 3)
    R("basic-1", "download OSM buildings for a bounding box and save as GeoJSON", "data_acquisition", "out_of_scope", oos="internet_data",
      cases=[refuse("gja-01", "Download the OpenStreetMap buildings of Pittsburgh and save them as GeoJSON.", C)]),
    R("basic-2", "convert a GeoJSON file to a shapefile", "format_io", "upstream", missing=["export_format"]),
    R("basic-3", "reproject a file from EPSG:4326 to EPSG:3857", "projection", "upstream", missing=["reproject"]),
    R("basic-4", "remove a file's coordinate system", "projection", "upstream", missing=["reproject"], confidence="low",
      notes="clearing a CRS is an odd operation for LayeredMap, which always needs a known location"),
    R("basic-5", "visualise a file and save the image", "map_making", "upstream", missing=["map_export"]),
    R("basic-6", "add a direction (orientation) field to rectangles", "field_calc", "upstream", missing=["calculate_geometry"], confidence="low",
      notes="VOCAB_GAP: polygon_orientation (tagged calculate_geometry)"),
    R("basic-7", "rename fields to Chinese names", "data_cleaning", "upstream", missing=["edit_attributes"], confidence="low",
      notes="renaming fields; LayeredMap never modifies sources, so this would be a display alias"),
    R("basic-8", "an interactive HTML query page for a layer", "map_making", "out_of_scope", oos="code", confidence="medium",
      notes="asks for a generated web page; the app itself is the interactive map",
      cases=[refuse("gja-08", "Build me an HTML page where people can click the schools, search them and switch layers.", C)]),
    R("basic-9", "an interactive HTML editing page that saves attribute changes", "data_cleaning", "out_of_scope", oos="code", confidence="medium",
      cases=[refuse("gja-09", "Make a web page where I can click a building and edit its height and use, saving the changes.", C)]),
    R("basic-10", "length of every feature", "field_calc", "upstream", missing=["calculate_geometry"]),
    R("basic-11", "clip roads to an area layer keeping attributes", "overlay", "upstream", missing=["clip"]),
    R("basic-12", "convert features to lines", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("basic-13", "convert line or polygon vertices to points", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("basic-14", "convert lines to polygons", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("basic-15", "intersection of two layers", "overlay", "upstream", missing=["intersect"]),
    R("basic-16", "buffer each feature by a given distance", "buffer", "now", confidence="medium",
      notes="the input layer is not legible in the extracted table; mapped to a line layer",
      cases=[case("gja-16", "Make a 50 m buffer around each street.", [call("buffer", layer="Streets", distance=dist(50), merge=False)], context=C,
                  alts=[[call("buffer", layer="Streets", distance=dist(50))]]),
             case("gja-16-zh", "给每条街道做50米缓冲区", [call("buffer", layer="Streets", distance=dist(50), merge=False)], context=C, lang="zh",
                  alts=[[call("buffer", layer="Streets", distance=dist(50))]])]),
    R("basic-17", "Thiessen polygons for points", "interpolation", "new", missing=["thiessen"]),
    R("basic-18", "minimum bounding rectangles for point groups", "generalization", "upstream", missing=["convex_hull"], confidence="medium"),
    R("basic-19", "convert road vertices to points", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("basic-20", "describe the spatial pattern of clustered points", "clustering", "new", missing=["point_clustering"], confidence="medium"),
    R("basic-21", "add X and Y coordinate fields to points", "field_calc", "upstream", missing=["calculate_geometry"]),
    R("basic-22", "lines from start and end coordinate fields", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("basic-23", "nearest perpendicular lines between two line features (multi-step: vertices, near, lines)", "proximity", "new",
      missing=["polygon_to_line", "nearest"], confidence="low", notes=CONV),
    R("basic-24", "a second multi-step nearest-line construction task (text partly illegible in the extracted PDF)", "proximity", "new",
      missing=["polygon_to_line", "nearest"], confidence="low", notes=CONV),
    R("basic-25", "split polygons with lines", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("basic-26", "distance between every pair of points to CSV", "proximity", "new", missing=["distance_matrix"]),
    R("basic-27", "join the two closest points with a line", "proximity", "new", missing=["distance_matrix"], confidence="medium"),
    R("basic-28", "summarise stored vertex-to-line distances per polygon", "statistics", "upstream", missing=["summary_statistics"], confidence="medium"),
    R("basic-29", "nearest point on a line for each point, with distance and coordinates", "proximity", "upstream", missing=["nearest"], confidence="low"),
    R("basic-30", "perpendicular distance from points to a line", "proximity", "upstream", missing=["nearest"], confidence="medium"),
    R("basic-31", "nearest location from each polygon to a line (vertices, near, summarise, join)", "proximity", "new", missing=["polygon_to_line", "nearest"],
      confidence="medium", notes=CONV),
    R("basic-32", "principal direction of rectangular polygons", "field_calc", "upstream", missing=["calculate_geometry"], confidence="low",
      notes="VOCAB_GAP: polygon_orientation (tagged calculate_geometry)"),
    R("basic-33", "total precipitation per region identifier", "statistics", "upstream", missing=["summary_statistics"]),
    R("basic-34", "representative point or centroid of features", "generalization", "upstream", missing=["centroid"]),
    R("basic-35", "rainfall statistics of gauges per region", "spatial_join", "now", confidence="medium",
      cases=[case("gja-35", "What is the average yearly precipitation of the weather stations in each county?",
                  [call("count_in", points="Weather stations", areas="Counties", stat="mean", field="precip_mm")], context=C),
             case("gja-35-zh", "每个县内气象站的年平均降水量是多少？",
                  [call("count_in", points="Weather stations", areas="Counties", stat="mean", field="precip_mm")], context=C, lang="zh")]),
    R("basic-36", "sort points by a distance field", "statistics", "now",
      cases=[case("gja-36", "Sort the flood events by their distance to the river.", [call("rank_by_value", layer="Flood events", field="near_dist")], context=C)]),
    R("basic-37", "spatial join of two layers", "spatial_join", "new", missing=["spatial_join"]),
    R("basic-38", "filter points with precipitation above 1000", "attribute_query", "now",
      cases=[case("gja-38", "Show only the weather stations with more than 1,000 mm of rain a year.",
                  [call("filter", layer="Weather stations", conditions=[cond("precip_mm", "above", 1000)])], context=C),
             case("gja-38-zh", "只显示年降水量超过1000毫米的气象站",
                  [call("filter", layer="Weather stations", conditions=[cond("precip_mm", "above", 1000)])], context=C, lang="zh")]),
    R("basic-39", "export geometry coordinates to CSV", "format_io", "upstream", missing=["calculate_geometry", "export_format"]),
    R("basic-40", "bar chart of precipitation per city", "map_making", "out_of_scope", oos="code", confidence="medium", notes="VOCAB_GAP: chart",
      cases=[refuse("gja-40", "Draw a bar chart of the precipitation at each weather station.", C)]),
    # ------------------------------------------------------------------ intermediate (Appendix 4)
    R("intermediate-1", "fast-food count per county saved to CSV and GeoJSON", "spatial_join", "now", confidence="medium",
      notes="saving the files is not offered; the counts are",
      cases=[case("gja-i01", "Count the fast-food restaurants in each county.", [call("count_in", points="Fast food restaurants", areas="Counties")], context=C),
             case("gja-i01-zh", "统计每个县的快餐店数量", [call("count_in", points="Fast food restaurants", areas="Counties")], context=C, lang="zh")]),
    R("intermediate-2", "reproject roads to UTM and add a length field", "field_calc", "upstream", missing=["reproject", "calculate_geometry"]),
    R("intermediate-3", "reproject hospitals and buffer each by 1000 m", "buffer", "now", confidence="medium",
      notes="LayeredMap's geodesic buffer needs no reprojection",
      cases=[case("gja-i03", "Show 1 km around every hospital, keeping each hospital's area separate.",
                  [call("buffer", layer="Hospitals", distance=dist(1000), merge=False)], context=C)]),
    R("intermediate-4", "reproject railways and compute their length", "measurement", "upstream", missing=["calculate_geometry"]),
    R("intermediate-5", "centroids then an HTML map with pop-ups", "generalization", "upstream", missing=["centroid", "map_export"]),
    R("intermediate-6", "boundary length of each county as a field", "field_calc", "upstream", missing=["calculate_geometry"]),
    R("intermediate-7", "Thiessen polygons plotted to PNG", "interpolation", "new", missing=["thiessen", "map_export"]),
    R("intermediate-8", "minimum-area rectangle and convex hull of points, visualised", "generalization", "upstream", missing=["convex_hull"]),
    R("intermediate-9", "scatter plot of obesity against COVID-19 death rates", "map_making", "out_of_scope", oos="code", confidence="medium", notes="VOCAB_GAP: chart",
      cases=[refuse("gja-i09", "Plot obesity rate against COVID-19 death rate for every county as a scatter chart.", C)]),
    R("intermediate-10", "counties with household income above $100,000, then a bar chart", "attribute_query", "now", confidence="medium",
      notes="the bar chart (VOCAB_GAP: chart) is dropped; the selection is the analytical part",
      cases=[case("gja-i10", "Which Pennsylvania counties have a median household income above $100,000?",
                  [call("filter", layer="Counties", conditions=[PA, cond("median_income", "above", 100000)])], context=C)]),
    R("intermediate-11", "download OSM buildings for a bounding box", "data_acquisition", "out_of_scope", oos="internet_data",
      cases=[refuse("gja-i11", "Fetch the OSM building footprints for this part of Pennsylvania.", C)]),
    R("intermediate-12", "count damaged houses inside special study zones and save them", "spatial_join", "now", confidence="medium",
      cases=[case("gja-i12", "How many damaged houses fall inside each fault zone?", [call("count_in", points="Damaged houses", areas="Fault zones")], context=C)]),
    R("intermediate-13", "line from each coffee shop to its nearest coffee shop", "proximity", "upstream", missing=["nearest"], confidence="low"),
    R("intermediate-14", "features to polygons, then a main-direction field", "field_calc", "new", missing=["polygon_to_line", "calculate_geometry"], confidence="low",
      notes=CONV + "; VOCAB_GAP: polygon_orientation"),
    R("intermediate-15", "tree totals per species and a bar chart of common species", "statistics", "upstream", missing=["frequency"],
      notes="the chart part is VOCAB_GAP: chart"),
    R("intermediate-16", "500 m buffers around cafés and bus stops; which cafés' buffers overlap", "overlay", "upstream", missing=["intersect"], confidence="medium",
      notes="equivalent to cafés within 1 km of a stop (select_by_location)"),
    R("intermediate-17", "sort restaurants by their id", "statistics", "now",
      cases=[case("gja-i17", "Sort the fast-food restaurants by their id.", [call("rank_by_value", layer="Fast food restaurants", field="id")], context=C)]),
    R("intermediate-18", "geometric centre of points and the point closest to it", "proximity", "upstream", missing=["centroid", "nearest"], confidence="low",
      notes="VOCAB_GAP: mean_center (tagged centroid)"),
    R("intermediate-19", "population density from population and land area, to CSV", "field_calc", "upstream", missing=["calculate_field"]),
    R("intermediate-20", "areas with a poverty ratio below 0.05", "attribute_query", "now",
      cases=[case("gja-i20", "Select the census tracts with a poverty ratio below 0.05.",
                  [call("filter", layer="Census tracts", conditions=[cond("poverty_ratio", "below", 0.05)])], context=C),
             case("gja-i20-zh", "选出贫困率低于0.05的人口普查区",
                  [call("filter", layer="Census tracts", conditions=[cond("poverty_ratio", "below", 0.05)])], context=C, lang="zh")]),
    # ------------------------------------------------------------------ advanced (Appendix 5)
    R("advanced-1", "street-tree count within 20 m of each street", "spatial_join", "now",
      cases=[case("gja-a01", "Count the street trees within 20 metres of every street.",
                  [call("buffer", layer="Streets", distance=dist(20), merge=False),
                   call("count_in", points="Street trees", areas=["#1", "Within 20 m of streets"])], context=C)]),
    R("advanced-2", "list the weather stations in each county", "spatial_join", "new", missing=["spatial_join"], confidence="medium"),
    R("advanced-3", "counties of a state whose rainfall exceeds a threshold, and how many", "attribute_query", "now",
      cases=[case("gja-a03", "How many Pennsylvania counties get more than 2.5 inches of rain a year?",
                  [call("filter", layer="Counties", conditions=[PA, cond("annual_rain_in", "above", 2.5)])], context=C),
             case("gja-a03-zh", "宾夕法尼亚州年降雨量超过2.5英寸的县有多少个？",
                  [call("filter", layer="Counties", conditions=[PA, cond("annual_rain_in", "above", 2.5)])], context=C, lang="zh")]),
    R("advanced-4", "sidewalk length within 500 m of each school", "spatial_join", "new", missing=["line_in_polygon_summary"]),
    R("advanced-5", "Thiessen service areas for bus stops on a web page", "interpolation", "new", missing=["thiessen"]),
    R("advanced-6", "tracts with poverty above 0.5 shown on an interactive map", "attribute_query", "now", confidence="medium",
      notes="the web-page output is read as showing them on the map",
      cases=[case("gja-a06", "Show the census tracts where the poverty ratio is above 0.5.",
                  [call("filter", layer="Census tracts", conditions=[cond("poverty_ratio", "above", 0.5)])], context=C)]),
    R("advanced-7", "population density map of a state's counties", "field_calc", "upstream", missing=["calculate_field"],
      notes="needs density = population / area as a field before colouring"),
    R("advanced-8", "fast-food accessibility score per county, mapped", "field_calc", "upstream", missing=["calculate_field"]),
    R("advanced-9", "building footprint share of each county's land, mapped", "spatial_join", "new", missing=["polygon_in_polygon_summary"]),
    R("advanced-10", "choose a site for a bus depot from the bus-stop pattern", "modelling", "new", missing=["optimisation"]),
]

if __name__ == "__main__":
    write(records, "geojson_agent.jsonl")
