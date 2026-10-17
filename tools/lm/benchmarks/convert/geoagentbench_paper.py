"""GeoAgentBench / GABench (Yu et al. 2026, arXiv 2604.13888) → sources/geoagentbench_paper.jsonl.

Source: https://github.com/geox-lab/GABench (named in the paper), commit
e8c64e883bbe45e2b94515c73941e7a0837320ae (2026-09-03); benchmark/benchmark.csv (Git LFS) with
ID, Domain, Task Description, Toolchain JSON. Licence: Apache-2.0 (LICENSE in the repository).

The paper describes 53 tasks (40 refactored from GeoAnalystBench plus 13 textbook tasks, mostly
hydrology); the released CSV has 57 rows, IDs 1–57 (two OD-flow tasks and two Chinese network
tasks beyond the paper's count). We map all 57. Many overlap GeoAnalystBench tasks with
re-numbered IDs; they are kept as separate records because this benchmark's tool chains are its
own. Rows 56 and 57 have their tool chains swapped relative to their descriptions (56 describes
a shortest path, 57 a service area); we classify by the description.

Summaries are in our words; requests for tasks today's actions carry are adapted to our
contexts (text_policy "adapted").

    python -I tools/lm/benchmarks/convert/geoagentbench_paper.py
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, call, case, cond  # noqa: E402

SOURCE = "GeoAgentBench (Yu et al. 2026)"
LICENCE = "Apache-2.0"
RAW = Path("/Volumes/LaCie/data/layeredmap-benchmarks/geoagentbench-paper/benchmark.csv")
OUT = HERE.parent / "sources/geoagentbench_paper.jsonl"

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
    return [case(f"gag-{int(sid):02d}", request, CANNOT, context=context)]


def hydro(sid, summary, extra=()):
    return rec(sid, summary, "terrain", "new", ["hydrology", *extra],
               notes="Textbook hydrology task. The optional GeoLibre/Whitebox suite can do this outside the certified actions.")


R = [
    rec(1, "Urban heat islands: ordinary kriging of point temperatures, zonal mean per census block, highlight blocks with many elderly.",
        "interpolation", "new", ["kriging"]),
    rec(2, "Transit access: multilayer map of the bus service area over poverty, density and car-access census layers.",
        "map_making", "new", ["graduated_symbols"], confidence="low",
        notes="The tool chain is only a multilayer map; color_by handles one layer and one field."),
    rec(3, "Burn scars: NBR for two years of imagery, difference, threshold and vectorise the damaged area.",
        "raster_algebra", "new", ["band_math", "raster_threshold_area", "raster_to_vector"], confidence="medium"),
    rec(4, "Risk zones: reproject soils, boundary and land cover, rasterise drainage and water depth, reclassify and weight them.",
        "raster_algebra", "new", ["rasterize", "reproject", "reclassify", "weighted_overlay"], confidence="low", notes=GAP_RASTERIZE),
    rec(5, "Blood-lead cases: hexagon grid and count of cases per hexagon for a privacy-preserving map.",
        "spatial_join", "new", ["tessellation"], confidence="medium",
        notes="Once a hexagon layer exists, count_in does the counting; building the grid is new. " + GAP_TESS),
    rec(6, "Elk tracks: bandwidth, convex hull, kernel density and density clusters.",
        "clustering", "new", ["convex_hull", "kernel_density", "point_clustering"]),
    rec(7, "Subsidence flooding: flood depth from future elevation and a design water level, damage summarised per building.",
        "raster_algebra", "upstream", ["band_math", "calculate_field"], confidence="medium",
        notes="Once a depth raster exists, summarize_each gives depth per building now."),
    rec(8, "Fire station coverage: buffer stations and subtract from the district to show the gaps.",
        "buffer", "upstream", ["erase"]),
    rec(9, "Deforestation rate inside a 5.5 km road buffer in Rondônia (buffer, dissolve, clip, area ratio).",
        "overlay", "upstream", ["clip", "calculate_field"], confidence="medium",
        notes="The merged buffer and its area are now; clipping the deforestation polygons and the ratio are not."),
    rec(10, "Planned roads: 5.5 km buffer, overlay difference with existing road zones, map with forests and boundary.",
        "overlay", "upstream", ["erase"]),
    rec(11, "Corals and sponges: slope and aspect, sampled at sites, grouped statistics per species.",
        "terrain", "upstream", ["slope", "aspect", "summary_statistics"]),
    rec(12, "Terrain ruggedness for mountain-lion habitat.",
        "terrain", "new", ["focal_statistics"], confidence="low", notes=GAP_FOCAL),
    rec(13, "Vertical temperature–salinity profile from a NetCDF file in a lat/lon/depth window.",
        "format_io", "new", ["netcdf_multidimensional"], confidence="low", notes=GAP_NETCDF),
    rec(14, "Count five-year spells above 280 K in 240 years of NetCDF temperatures.",
        "temporal", "new", ["temporal_change"]),
    rec(15, "Show the Total Electron Content variable of a space-weather NetCDF file as a map.",
        "format_io", "new", ["netcdf_multidimensional"], confidence="low",
        notes="Displaying a 2-D grid is trivial once read; LayeredMap does not read NetCDF variables. " + GAP_NETCDF),
    rec(16, "Quadratic polynomial fit of a NetCDF temperature time series, plotted.",
        "temporal", "new", ["regression", "temporal_change"]),
    rec(17, "Map the fatal car crashes of 2016 over New York's boroughs.",
        "attribute_query", "now", confidence="medium",
        cases=[
            case("gag-17", "Show the fatal car crashes from 2016.",
                 [call("filter", layer="Car crashes", conditions=[cond("year", "is", 2016), cond("killed", "above", 0)])],
                 context="city_open_data"),
            case("gag-17-zh", "显示2016年的致命车祸",
                 [call("filter", layer="Car crashes", conditions=[cond("year", "is", 2016), cond("killed", "above", 0)])],
                 context="city_open_data", lang="zh"),
        ]),
    rec(18, "Street trees: quadtree cells with the share of trees missing a species.",
        "field_calc", "new", ["tessellation", "calculate_field"], confidence="medium", notes=GAP_TESS),
    rec(19, "Water quality surface from samples by kernel density estimation.",
        "interpolation", "new", ["kernel_density"]),
    rec(20, "Random-forest prospectivity raster for tin-tungsten deposits in Tasmania.",
        "modelling", "out_of_scope", oos="prediction",
        cases=refuse(20, "Build a random-forest model from the deposit points and the evidence rasters and map where new deposits are likely.", "usa")),
    rec(21, "Reclassify land-cover codes to a common habitat scale.",
        "raster_algebra", "upstream", ["reclassify"]),
    rec(22, "Weighted cost surface from road distance, ruggedness, protected status and land cover with given weights.",
        "raster_algebra", "upstream", ["weighted_overlay"]),
    rec(23, "De-identified child points: reproject, round coordinates to two decimals, plot.",
        "data_cleaning", "upstream", ["reproject", "calculate_field", "xy_to_points"], confidence="medium"),
    rec(24, "COVID-19 risk in Hong Kong: four similarity searches joined to constituencies, then multivariate clusters.",
        "clustering", "new", ["similar_features", "table_join", "point_clustering"], confidence="medium",
        notes="Multivariate clustering here is attribute clustering; point_clustering is the nearest op."),
    rec(25, "Groundwater-vulnerable areas from rasterised soil drainage and water depth, reclassified and weighted.",
        "raster_algebra", "new", ["rasterize", "reproject", "reclassify", "weighted_overlay"], confidence="low", notes=GAP_RASTERIZE),
    rec(26, "Undeveloped high-risk zones: align, normalise, reclassify and weight rasters, threshold and mask by land cover.",
        "raster_algebra", "upstream", ["reproject", "resample", "reclassify", "weighted_overlay", "raster_threshold_area"]),
    rec(27, "Random-forest regression of origin–destination flows from socio-economic data.",
        "modelling", "out_of_scope", oos="prediction",
        cases=refuse(27, "Train a model that predicts the flow between every pair of neighbourhoods from their census data.")),
    rec(28, "Tsunami travel time: invert bathymetry, wave speed, travel time along paths from the origin.",
        "terrain", "new", ["band_math", "distance_raster"]),
    rec(29, "Local K and global L function of geographical flows.",
        "density", "new", ["hot_spots"], confidence="medium", notes="Point-pattern statistics; hot_spots is the nearest op."),
    rec(30, "Flood damage: depth raster, flooded polygons, depth per building, damage costs.",
        "raster_algebra", "new", ["band_math", "raster_to_vector", "calculate_field"]),
    rec(31, "Share of rural population within 2 km of all-season roads in Shikoku.",
        "overlay", "upstream", ["clip", "calculate_field"], confidence="medium",
        notes="Population is a polygon layer here, so clipping is needed; a population raster would make the sum a summarize_here."),
    rec(32, "Hail damage: SAVI before and after, aligned and differenced.",
        "raster_algebra", "upstream", ["band_math", "resample"]),
    rec(33, "Campus travel times from one building to all nodes over a supplied street graph.",
        "network", "new", ["od_matrix"], confidence="medium",
        notes="Unlike the GeoAnalystBench version, the graph is supplied locally (GraphML)."),
    rec(34, "Malaria prevalence change 2000–2015 as a raster difference with a diverging map.",
        "raster_algebra", "upstream", ["band_math"]),
    rec(35, "Airbnb prices: CSV to points, spatial join to neighbourhoods, local Moran's I.",
        "density", "new", ["xy_to_points", "hot_spots"], confidence="medium",
        notes="The median per neighbourhood would be count_in with stat median; Moran's I is new."),
    rec(36, "Terrain ruggedness by focal statistics, normalised.",
        "terrain", "new", ["focal_statistics", "reclassify"], confidence="low", notes=GAP_FOCAL),
    rec(37, "Impervious surface area in each parcel (area above threshold, joined to parcels).",
        "zonal_stats", "now", confidence="medium",
        notes="summarize_each on the categorical raster gives the impervious share per parcel; m² needs a field calculation.",
        cases=[
            case("gag-37", "Summarize the impervious surface in each parcel.",
                 [call("summarize_each", raster="Impervious surface", areas="Parcels")], context="city_open_data"),
            case("gag-37-zh", "统计每个地块里的不透水面",
                 [call("summarize_each", raster="Impervious surface", areas="Parcels")], context="city_open_data", lang="zh"),
        ]),
    rec(38, "Loan tracts with ≥ 30 loans: interest-rate hot spots and a regression.",
        "modelling", "new", ["hot_spots", "regression"],
        notes="The first step (tracts with at least 30 loans) is a filter today."),
    rec(39, "Join homelessness figures to US states by abbreviation, compute a field and map it.",
        "data_join", "new", ["table_join", "calculate_field"]),
    rec(40, "Seagrass habitat: dissolve, random points, interpolate ocean variables, predict suitability.",
        "modelling", "out_of_scope", oos="prediction", confidence="medium",
        cases=refuse(40, "Predict suitable seagrass habitat along the US coast from ocean temperature and salinity.", "usa")),
    rec(41, "Reclassify protected-status codes to a common habitat scale.",
        "raster_algebra", "upstream", ["reclassify"]),
    rec(42, "3-D line of sight between two points with buildings as obstacles.",
        "terrain", "upstream", ["line_of_sight"], confidence="low",
        notes="rasterops::line_of_sight works over a raster surface; 3-D building obstacles would need a surface built from them first."),
    rec(43, "Flatten 3-D building polygons to footprints and dissolve them into one coverage.",
        "generalization", "upstream", ["dissolve"]),
    hydro(44, "Fill depressions in a DEM."),
    hydro(45, "D8 flow direction and flow accumulation."),
    hydro(46, "Depth of sinks: filled DEM minus original."),
    hydro(47, "Downstream flow length from a D8 direction raster."),
    hydro(48, "Upstream flow length from a D8 direction raster."),
    hydro(49, "Extract a stream network from flow accumulation and vectorise it.", ["raster_to_vector"]),
    hydro(50, "Strahler stream order."),
    hydro(51, "Shreve stream magnitude."),
    hydro(52, "Watershed basins from a D8 direction raster, mapped with rivers."),
    rec(53, "Random-forest classifier for tin-tungsten deposits, reporting performance metrics.",
        "modelling", "out_of_scope", oos="prediction",
        cases=refuse(53, "Train a random-forest classifier on the deposit data and report its AUC.", "usa")),
    rec(54, "OD mobility between intersections and supermarkets: network cost matrix, gravity model, desire lines.",
        "network", "new", ["od_matrix", "simulation"]),
    rec(55, "OD flows assigned to roads with a gravity model, and network betweenness.",
        "network", "new", ["od_matrix", "simulation"]),
    rec(56, "(Chinese) Shortest path by travel minutes from a named school to a named market over a road network.",
        "network", "upstream", ["shortest_route"],
        notes="The description asks for a shortest path; the row's tool chain is a service-area analysis (swapped with row 57)."),
    rec(57, "(Chinese) Service area of a named market over the road network, threshold by its type, and which schools it covers.",
        "network", "new", ["service_area"],
        notes="The description asks for a service area; the row's tool chain is a shortest path (swapped with row 56)."),
]


def main() -> None:
    ids = [r["source_id"] for r in R]
    assert len(ids) == len(set(ids)) == 57, len(ids)
    if RAW.exists():
        raw = {row["ID"].strip() for row in csv.DictReader(RAW.open(encoding="utf-8-sig")) if row["ID"].strip()}
        assert raw == set(ids), raw ^ set(ids)
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in R), encoding="utf-8")
    print(f"{OUT.name}: {len(R)} records")


if __name__ == "__main__":
    main()
