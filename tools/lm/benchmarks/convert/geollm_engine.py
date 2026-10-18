"""GeoLLM-Engine → sources/geollm_engine.jsonl (inspired-by; no data released).

Source: Singh, Fore and Stamoulis, "GeoLLM-Engine: A Realistic Environment for Building
Geospatial Copilots", arXiv 2404.15500 (v1, 2024). The authors report a 250-query seed set and
generated sets of 10k and 100k queries (about 5.2 tasks each) over ~1.1M remote-sensing images,
but released neither the queries nor the engine (github.com/dstamoulis/geollm-engine holds only a
project page, checked 2026-10-17). So this file has one record per task template or example the
paper describes (Table 2 and the text), summarised in our own words, with our own requests.
That is 11 templates, not the paper's half a million generated tasks.

    python -I tools/lm/benchmarks/convert/geollm_engine.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, call, case, obj  # noqa: E402

OUT = HERE.parent / "sources/geollm_engine.jsonl"
LIC = "arXiv paper; no dataset released"

ZOOM = [call("zoom_to", target=obj("Cities", ("name", "Seattle")))]

# (id, summary, category, coverage, oos_reason, cases)
TEMPLATES = [
    ("T01", "Load remote-sensing detection datasets, filter the images by place, date and object class, and plot them on the map", "imagery", "out_of_scope", "imagery",
     [case("gle-01", "Plot the aerial images with cars and trucks detected around Denver from last spring.", CANNOT, context="usa")]),
    ("T02", "Retrieve land-cover-classified image chips of one class within a date range and plot them", "imagery", "out_of_scope", "imagery",
     [case("gle-02", "Show me every image chip classified as a stadium between 2016 and 2018.", CANNOT, context="world_plus"),
      case("gle-02-zh", "把2016到2018年间所有被分类为体育场的影像切片显示出来", CANNOT, context="world_plus", lang="zh")]),
    ("T03", "Fetch ship detections from SAR imagery near a named coastal place", "imagery", "out_of_scope", "imagery",
     [case("gle-03", "Find all the fishing boats detected in radar images off the coast of Brittany.", CANNOT, context="world_plus")]),
    ("T04", "Make a change-detection heat map from image pairs (e.g. building damage)", "imagery", "out_of_scope", "imagery",
     [case("gle-04", "Make a heat map of buildings damaged by the hurricane by comparing before and after images.", CANNOT, context="usa")]),
    ("T05", "Run an object detector on the images of an area and count the objects", "imagery", "out_of_scope", "imagery",
     [case("gle-05", "Count the airplanes parked at each airport using the satellite photos.", CANNOT, context="world_plus")]),
    ("T06", "Answer a visual question about a remote-sensing image (VQA)", "imagery", "out_of_scope", "imagery",
     [case("gle-06", "Is there a bridge visible in this satellite picture of the river?", CANNOT, context="world_plus"),
      case("gle-06-zh", "这张卫星图里能看到桥吗？", CANNOT, context="world_plus", lang="zh")]),
    ("T07", "Show the image behind a detection when hovering over it on the map (UI)", "imagery", "out_of_scope", "imagery",
     [case("gle-07", "When I hover over a detected ship, show me its image chip.", CANNOT, context="world_plus")]),
    ("T08", "Search the web for a phrase and open a result", "knowledge", "out_of_scope", "internet_data",
     [case("gle-08", "Search the web for 'foundation models for Earth observation' and open the first result.", CANNOT, context="world_plus")]),
    ("T09", "Zoom the map to a named city", "map_making", "now", None,
     [case("gle-09", "Zoom the map to Seattle.", ZOOM, context="usa"),
      case("gle-09-zh", "把地图缩放到西雅图", ZOOM, context="usa", lang="zh")]),
    ("T10", "Answer a question from a research paper found on arXiv (document retrieval)", "knowledge", "out_of_scope", "internet_data",
     [case("gle-10", "How many semantic tags does the SkyScript paper on arXiv cover?", CANNOT, context="world_plus")]),
    ("T11", "Answer from an internal model wiki which detector suits a sensor (knowledge lookup)", "knowledge", "out_of_scope", "knowledge",
     [case("gle-11", "Which object detector works best for ships in radar images?", CANNOT, context="world_plus")]),
]


def main() -> int:
    records = []
    for sid, summary, cat, cov, reason, cases in TEMPLATES:
        r = {
            "source": "GeoLLM-Engine",
            "source_id": sid,
            "licence": LIC,
            "text_policy": "own_words",
            "summary": summary,
            "category": cat,
            "coverage": cov,
            "confidence": "high" if cov == "out_of_scope" else "medium",
            "notes": "Template from the paper's Table 2 / text; requests are ours.",
            "cases": cases,
        }
        if reason:
            r["oos_reason"] = reason
        if cov == "now":
            r["notes"] += " Seattle must exist in the city layer; GeoLLM-Engine geocodes the name instead."
        records.append(r)
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in records), encoding="utf-8")
    print(OUT.name, len(records))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
