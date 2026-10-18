"""ShapefileGPT (Lin et al., arXiv 2410.12376): its 42-task Shapefile benchmark, mapped as far as the
paper identifies the tasks.

No code or data release was found. The paper names the 22 geometric-operation tasks (its Table 2)
and gives only counts for the rest (7 query and computation, 7 distance and direction, 6 other);
of the "other" tasks it names three kinds (coordinate transformation, adding fields, renaming
fields). So 25 of the 42 tasks are recorded here; the 17 unnamed ones are not guessed. Summaries
and requests are ours.

    python -I tools/lm/benchmarks/convert/shapefilegpt.py   → sources/shapefilegpt.jsonl
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))  # python -I drops the script's folder

from f4_common import call, case, dist, rec, write  # noqa: E402

SRC = "ShapefileGPT"
LIC = "arXiv paper (2410.12376); no code or data released; no data licence stated"
CONV = "VOCAB_GAP: geometry_conversion (tagged polygon_to_line)"


def R(sid, summary, category, coverage, **kw):
    return rec(SRC, LIC, sid, summary, category, coverage, **kw)


records = [
    R("geometric-1", "calculate the length of line features", "field_calc", "upstream", missing=["calculate_geometry"]),
    R("geometric-2", "clip a layer with another", "overlay", "upstream", missing=["clip"]),
    R("geometric-3", "convert geometries to line features", "generalization", "new", missing=["polygon_to_line"]),
    R("geometric-4", "convert line or polygon vertices to points", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("geometric-5", "convert line features to polygons", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("geometric-6", "overlay analysis of two polygon layers", "overlay", "upstream", missing=["intersect"], confidence="medium",
      notes="the overlay type is not stated; intersect assumed"),
    R("geometric-7", "buffer zones of a set distance around each feature", "buffer", "now", confidence="medium",
      notes="the input geometry is not stated; mapped to points",
      cases=[case("sgpt-07", "Create a 300 m zone around each bus stop.", [call("buffer", layer="Bus stops", distance=dist(300), merge=False)], context="city",
                  alts=[[call("buffer", layer="Bus stops", distance=dist(300))]]),
             case("sgpt-07-zh", "在每个公交站周围生成300米的缓冲区", [call("buffer", layer="Bus stops", distance=dist(300), merge=False)], context="city", lang="zh",
                  alts=[[call("buffer", layer="Bus stops", distance=dist(300))]])]),
    R("geometric-8", "Thiessen (Voronoi) polygons for points", "interpolation", "new", missing=["thiessen"]),
    R("geometric-9", "minimum bounding rectangle for point groups", "generalization", "upstream", missing=["convex_hull"], confidence="medium"),
    R("geometric-10", "inner buffer of closed lines", "buffer", "upstream", missing=["buffer_polygons"], confidence="low",
      notes="a negative buffer; whether the upstream buffer takes negative distances is unverified"),
    R("geometric-11", "disaster impact zones at several distances", "buffer", "new", missing=["multi_ring_buffer"]),
    R("geometric-12", "vertices of lines or polygons to points (second variant)", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("geometric-13", "spatial characteristics of clustered points (centre, direction, spread)", "clustering", "new", missing=["point_clustering"], confidence="medium",
      notes="standard deviational ellipse-style statistics"),
    R("geometric-14", "add coordinate fields to points", "field_calc", "upstream", missing=["calculate_geometry"]),
    R("geometric-15", "lines from start and end coordinate fields", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("geometric-16", "nearest perpendicular lines between line features", "proximity", "new", missing=["polygon_to_line", "nearest"], confidence="low", notes=CONV),
    R("geometric-17", "overlapping areas between two polygon layers", "overlay", "upstream", missing=["intersect"]),
    R("geometric-18", "overlaps among polygons of one layer", "data_cleaning", "upstream", missing=["topology_check"]),
    R("geometric-19", "allocate space to points by distance (Thiessen polygons and buffers)", "interpolation", "new", missing=["thiessen"]),
    R("geometric-20", "Thiessen polygons for polygon features", "interpolation", "new", missing=["thiessen"]),
    R("geometric-21", "split polygons by lines", "generalization", "new", missing=["polygon_to_line"], notes=CONV),
    R("geometric-22", "mixed Thiessen polygons for points, lines and areas", "interpolation", "new", missing=["thiessen"]),
    R("other-crs", "coordinate system transformation (named as an 'other' task kind)", "projection", "upstream", missing=["reproject"], confidence="medium",
      notes="the paper names the kind, not the task"),
    R("other-add-field", "add a field (named as an 'other' task kind)", "field_calc", "upstream", missing=["calculate_field"], confidence="medium",
      notes="the paper names the kind, not the task"),
    R("other-rename-field", "rename a field (named as an 'other' task kind)", "data_cleaning", "upstream", missing=["edit_attributes"], confidence="low",
      notes="the paper names the kind, not the task"),
]

if __name__ == "__main__":
    write(records, "shapefilegpt.jsonl")
