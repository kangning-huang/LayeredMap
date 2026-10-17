"""GeoAnalystBench (Zhang et al. 2025) → sources/geoanalystbench.jsonl.

Source: https://github.com/GeoDS/GeoAnalystBench, commit b5d8c40a8d23639ec77e9acb11f79fd033c07338
(2026-02-11), dataset/GeoAnalystBench.csv, 50 tasks. Licence: Apache-2.0.

The tasks are Python code-generation workflows. Each record summarises the task in our words,
classifies it by the workflow's core operations, and for the tasks today's actions can carry,
gives a plan-level request rewritten for our map contexts (text_policy "adapted"). The task
text itself is not copied. The mapping is written by hand below; the raw CSV is only read to
check that every id is covered (when it is present on this machine).

    python -I tools/lm/benchmarks/convert/geoanalystbench.py
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, call, case, cond, sel  # noqa: E402

SOURCE = "GeoAnalystBench"
LICENCE = "Apache-2.0"
RAW = Path("/Volumes/LaCie/data/layeredmap-benchmarks/geoanalystbench/src/dataset/GeoAnalystBench.csv")
OUT = HERE.parent / "sources/geoanalystbench.jsonl"

GAP_FOCAL = "Focal (neighbourhood) statistics are new."
GAP_RASTERIZE = "Polygon-to-raster conversion (rasterize) is new."
GAP_TESS = "Hexagon/quadtree tessellation is new."
GAP_NETCDF = "Reading multidimensional NetCDF variables is new."


def rec(sid, summary, category, coverage, missing=(), cases=(), confidence="high", notes="", oos=None):
    r = {
        "source": SOURCE,
        "source_id": str(sid),
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


def refuse(sid, request, context="city"):
    return [case(f"gab-{int(sid):02d}", request, CANNOT, context=context)]


R = [
    rec(1, "Urban heat islands: krige point temperatures into a surface, average it per census block and highlight blocks with many people over 65.",
        "interpolation", "new", ["kriging"], confidence="high",
        notes="With a temperature raster already present, summarize_each would do the averaging; the kriging step is the core and is new."),
    rec(2, "Transit access: overlay bus-stop service areas with three census-block factors (poverty, density, car access) as transparent layers.",
        "map_making", "new", ["graduated_symbols"], confidence="low",
        notes="A buffer of stops plus one color_by is possible now; layering three demographic factors with transparency is not."),
    rec(3, "Burn scars: difference the Normalized Burn Ratio between two years of local imagery and turn the damaged area into polygons.",
        "raster_algebra", "new", ["band_math", "raster_to_vector"], confidence="medium",
        notes="Band math on supplied imagery, not classification, so not out of scope; vectorising is new."),
    rec(4, "Groundwater vulnerability: project soils, rasterise drainage and water depth, reclassify and combine them into vulnerable areas.",
        "raster_algebra", "new", ["rasterize", "reproject", "reclassify", "weighted_overlay"], confidence="low", notes=GAP_RASTERIZE),
    rec(5, "Privacy-preserving map of child blood-lead cases: hot-spot analysis and counts in a hexagon tessellation.",
        "density", "new", ["hot_spots", "tessellation"], confidence="medium", notes=GAP_TESS),
    rec(6, "Elk GPS tracks: home range by convex hull, kernel density and DBSCAN clusters.",
        "clustering", "new", ["convex_hull", "kernel_density", "point_clustering"]),
    rec(7, "Land subsidence and flooding: raster calculation of future flood areas, intersect with buildings, estimate damage.",
        "raster_algebra", "new", ["band_math", "raster_to_vector", "calculate_field"], confidence="medium"),
    rec(8, "Fire station coverage gaps: buffer the stations and find the part of the district with no coverage.",
        "buffer", "upstream", ["erase"], confidence="high",
        notes="The buffer itself is the certified buffer action; the gap (district minus buffers) needs erase."),
    rec(9, "Deforestation rate: share of deforested area inside a 5.5 km road buffer in Rondônia.",
        "overlay", "upstream", ["clip"], confidence="medium",
        notes="Deforestation is a polygon layer; clipping it to the merged buffer and measuring is needed. If it were a raster, summarize_here would answer it now."),
    rec(10, "Proposed roads: 5.5 km buffer around planned roads overlaid on deforestation, protected forest and the state boundary.",
        "overlay", "upstream", ["clip"], confidence="medium",
        notes="Buffer and display are possible now; the impact (buffer outside existing road zones / clipped to forest) needs an overlay."),
    rec(11, "Coral and sponge sites: slope and aspect from bathymetry, sampled at each site, mean per species.",
        "terrain", "upstream", ["slope", "aspect", "summary_statistics"]),
    rec(12, "Terrain ruggedness from elevation for mountain-lion habitat.",
        "terrain", "new", ["focal_statistics"], confidence="low", notes=GAP_FOCAL),
    rec(13, "South Atlantic: vertical temperature–salinity profile from NetCDF within a lat/lon/depth window.",
        "format_io", "new", ["netcdf_multidimensional"], confidence="low", notes=GAP_NETCDF),
    rec(14, "240 years of North American temperature: count five-year spells above 280 K.",
        "temporal", "new", ["temporal_change"]),
    rec(15, "Ionosphere Total Electron Content from NetCDF as a filled contour map.",
        "map_making", "new", ["contours", "netcdf_multidimensional"], confidence="medium", notes=GAP_NETCDF),
    rec(16, "Quadratic trend fit of North American temperature time series.",
        "temporal", "new", ["regression", "temporal_change"]),
    rec(17, "Map the fatal car crashes of 2016 over the city's districts.",
        "attribute_query", "now", confidence="medium",
        notes="The original also reprojects for plotting; LayeredMap displays in its own projection.",
        cases=[
            case("gab-17", "Show only the 2016 car crashes in which someone was killed.",
                 [call("filter", layer="Car crashes", conditions=[cond("year", "is", 2016), cond("killed", "above", 0)])],
                 context="city_open_data",
                 lenient_alts=[[call("filter", layer="Car crashes", conditions=[cond("year", "is", 2016), cond("killed", "above", 0)]),
                                call("count_in", points="Car crashes", areas="Neighbourhoods")]]),
            case("gab-17-zh", "只显示2016年有人死亡的车祸",
                 [call("filter", layer="Car crashes", conditions=[cond("year", "is", 2016), cond("killed", "above", 0)])],
                 context="city_open_data", lang="zh"),
        ]),
    rec(18, "Street trees: share of trees with no species recorded in each region, shown as a quadtree map.",
        "field_calc", "new", ["calculate_field", "tessellation"], confidence="medium",
        notes="Filtering trees with empty species and counting them per neighbourhood is possible now; the percentage needs a field calculation and the quadtree cells are new. " + GAP_TESS),
    rec(19, "Water quality: interpolate point samples into a surface.",
        "interpolation", "upstream", ["idw"], confidence="medium",
        notes="The task leaves the method open; IDW (rasterops::idw) satisfies it, kriging would be new."),
    rec(20, "Random-forest prospectivity map for tin-tungsten deposits in Tasmania.",
        "modelling", "out_of_scope", oos="prediction", confidence="medium",
        cases=refuse(20, "Train a random forest on the mineral occurrences and evidence rasters and map the probability of new deposits.", "usa")),
    rec(21, "Reclassify land cover and protected status to a common suitability scale.",
        "raster_algebra", "upstream", ["reclassify"]),
    rec(22, "Weighted cost surface from ruggedness, road distance, land cover and protected status.",
        "raster_algebra", "upstream", ["weighted_overlay"]),
    rec(23, "Open space for flood insurance: predominant land cover and open-space area of each parcel.",
        "zonal_stats", "now", confidence="medium",
        notes="summarize_each gives each parcel's land-cover shares (and the largest class); converting a share to m² is a field calculation the card does not do.",
        cases=[
            case("gab-23", "What share of each parcel is covered by each land cover class?",
                 [call("summarize_each", raster="Land cover", areas="Parcels")], context="city_open_data"),
        ]),
    rec(24, "De-identify child records by rounding coordinates to two decimals and rebuilding the points.",
        "data_cleaning", "upstream", ["calculate_field", "xy_to_points"], confidence="medium"),
    rec(25, "COVID-19 risk profiles in Hong Kong: similarity search per risk, joins, multivariate clustering.",
        "clustering", "new", ["similar_features", "table_join", "point_clustering"], confidence="medium",
        notes="Multivariate clustering here is attribute clustering of constituencies; point_clustering is the nearest op."),
    rec(26, "Groundwater vulnerability from rasterised drainage and water depth.",
        "raster_algebra", "new", ["rasterize", "reproject", "reclassify", "weighted_overlay"], confidence="low", notes=GAP_RASTERIZE),
    rec(27, "High-risk undeveloped zones from vulnerability and land-cover rasters.",
        "raster_algebra", "upstream", ["reclassify", "band_math"]),
    rec(28, "Random-forest model of origin–destination flows from socio-economic attributes.",
        "modelling", "out_of_scope", oos="prediction",
        cases=refuse(28, "Predict the flows between neighbourhoods with a random forest trained on their socio-economic data.")),
    rec(29, "Tsunami travel time over bathymetry from an origin point.",
        "terrain", "new", ["band_math", "distance_raster"]),
    rec(30, "Commuter bike routes: demand hot spots, weighted suitability of streets, network routing.",
        "network", "new", ["hot_spots", "weighted_overlay", "shortest_route", "spatial_join"]),
    rec(31, "Aggregation scales of geographical flows with the L-function.",
        "density", "new", ["hot_spots"], confidence="medium", notes="Point-pattern statistics; hot_spots is the nearest op."),
    rec(32, "Optimal corridors between mountain-lion habitats over a weighted cost surface.",
        "terrain", "new", ["focal_statistics", "weighted_overlay", "reclassify"], confidence="medium",
        notes="Least-cost corridors need a cost-distance surface (new). " + GAP_FOCAL),
    rec(33, "Flood damage to buildings in 2050: flood depth from elevation and sea level, depth per building, damage.",
        "raster_algebra", "upstream", ["band_math", "raster_threshold_area", "calculate_field"], confidence="medium",
        notes="Once a depth raster exists, summarize_each gives depth per building now."),
    rec(34, "Rural access to all-season roads in Japan: share of rural population within 2 km of those roads.",
        "overlay", "upstream", ["clip", "calculate_field"], confidence="medium",
        notes="Buffer of roads and a population sum in the buffer are possible now; restricting to rural areas and the percentage are not."),
    rec(35, "Landslide potential after a wildfire: burn severity (NBR change), slope, land cover remapped and combined.",
        "raster_algebra", "upstream", ["band_math", "slope", "reclassify", "weighted_overlay"]),
    rec(36, "Hail damage to cornfields: SAVI change between two images, mean loss per field.",
        "raster_algebra", "upstream", ["band_math"], confidence="medium",
        notes="Once the loss raster exists, summarize_each gives the mean per field now."),
    rec(37, "Heat sentiment from tweets: score weather tweets with a vocabulary, assign them to counties, mean per county.",
        "knowledge", "out_of_scope", oos="code", confidence="low",
        notes="Scoring tweet text is text processing, not a map action; if the tweets carried a score field, count_in with stat mean would do the spatial part.",
        cases=refuse(37, "Score these tweets for heat sentiment using my word list and average the scores in each county.", "usa")),
    rec(38, "Campus travel times from one building to all others over an OpenStreetMap street graph.",
        "network", "out_of_scope", oos="internet_data", confidence="medium",
        notes="The task downloads the OSM graph; with a local street layer this would be a one-to-many network cost (od_matrix, new).",
        cases=refuse(38, "Download the campus street network from OpenStreetMap and compute driving times from the geography building to every other building.")),
    rec(39, "Geographically weighted regression of bachelor's degree rate in Georgia counties.",
        "modelling", "new", ["regression"]),
    rec(40, "Change in malaria prevalence 2000–2015: raster difference with a diverging classification.",
        "raster_algebra", "upstream", ["band_math", "reclassify"]),
    rec(41, "Campsite data quality: joins, relates and relationship classes between campsites and campgrounds.",
        "data_join", "new", ["table_join"], confidence="medium"),
    rec(42, "Airbnb prices in Berlin: median price per neighbourhood, then local Moran's I.",
        "density", "new", ["hot_spots", "xy_to_points"], confidence="medium",
        notes="The median price per neighbourhood is count_in with stat median today; the autocorrelation is the core and is new."),
    rec(43, "Elk home ranges: convex hull, kernel density and DBSCAN outputs saved to a geodatabase.",
        "clustering", "new", ["convex_hull", "kernel_density", "point_clustering"]),
    rec(44, "Fire station coverage: buffers, the most compromised areas and the no-service area saved.",
        "buffer", "upstream", ["erase"]),
    rec(45, "Terrain ruggedness by 3×3 focal statistics, rescaled into a cost layer.",
        "terrain", "new", ["focal_statistics", "reclassify"], confidence="low", notes=GAP_FOCAL),
    rec(46, "Peak-time crash hot spots on roads: select, snap to roads, spatial join, crash rate, Getis-Ord Gi*.",
        "density", "new", ["hot_spots", "spatial_join", "calculate_field"]),
    rec(47, "Impervious surface area of each land parcel (tabulate area and join).",
        "zonal_stats", "now", confidence="medium",
        notes="summarize_each on a categorical raster gives each parcel's impervious share; area in m² needs a field calculation.",
        cases=[
            case("gab-47", "How much of each parcel is impervious surface?",
                 [call("summarize_each", raster="Impervious surface", areas="Parcels")], context="city_open_data"),
            case("gab-47-zh", "每个地块的不透水面占多少？",
                 [call("summarize_each", raster="Impervious surface", areas="Parcels")], context="city_open_data", lang="zh"),
        ]),
    rec(48, "Loan interest rates: tracts with ≥ 30 loans, hot spots, geographically weighted regression.",
        "modelling", "new", ["hot_spots", "regression"]),
    rec(49, "Homelessness in North Dakota: join a table to US states by abbreviation and map it.",
        "data_join", "new", ["table_join", "calculate_field"]),
    rec(50, "Seagrass habitat prediction: dissolve, random points, kriging of ocean variables, MaxEnt presence-only model.",
        "modelling", "out_of_scope", oos="prediction", confidence="medium",
        notes="Uses dissolve, random points and kriging on the way, but the deliverable is a predictive model.",
        cases=refuse(50, "Predict where seagrass could grow along the US coast with a presence-only model.", "usa")),
]


def main() -> None:
    ids = [r["source_id"] for r in R]
    assert len(ids) == len(set(ids)) == 50, len(ids)
    if RAW.exists():
        csv.field_size_limit(10**9)
        raw = {row["id"] for row in csv.DictReader(RAW.open(encoding="utf-8"))}
        assert raw == set(ids), raw ^ set(ids)
    OUT.parent.mkdir(exist_ok=True)
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in R), encoding="utf-8")
    print(f"{OUT.name}: {len(R)} records")


if __name__ == "__main__":
    main()
