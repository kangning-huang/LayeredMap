"""Build design/phase-0-mock/index.html from src/template.html and the V1/T1 fixtures.

Usage (from anywhere):  python3 -I design/phase-0-mock/src/build.py

What it inlines:
  * fixtures/V1/districts.geojson, plus ONE SYNTHETIC FIELD `july_temp_c`
    ("July mean surface temperature, °C") added to each district (see july_temp below);
  * fixtures/T1/schools.csv, verbatim, as text (the page parses it);
  * fixtures/V1/expected_insight.csv and fixtures/T1/expected_count_in_areas.csv,
    used only by the page's self-check (never displayed).
Nothing else is hard-coded: every number on screen is computed by the page's JS.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
MOCK = HERE.parent
ROOT = MOCK.parents[1]
FIX = ROOT / "fixtures"

# Synthetic heat-island model: hotter near the city centre, cooler outward, plus a
# small deterministic per-district offset so neighbours differ. Range ~28.9-35.8 °C.
CENTRE = (121.44, 31.26)  # lon, lat (near the V1 Huangpu cell)


def centroid(geom):
    polys = geom["coordinates"] if geom["type"] == "MultiPolygon" else [geom["coordinates"]]
    a_sum = x_sum = y_sum = 0.0
    for poly in polys:
        for ring in poly:  # holes are wound opposite, so they subtract
            a = x = y = 0.0
            for (x0, y0), (x1, y1) in zip(ring, ring[1:]):
                c = x0 * y1 - x1 * y0
                a += c
                x += (x0 + x1) * c
                y += (y0 + y1) * c
            a_sum += a / 2
            x_sum += x / 6
            y_sum += y / 6
    return x_sum / a_sum, y_sum / a_sum


def july_temp(feature) -> float:
    n = int(feature["properties"]["district_id"][1:])
    x, y = centroid(feature["geometry"])
    dist_km = math.hypot((x - CENTRE[0]) * 111.32 * math.cos(math.radians(31.2)),
                         (y - CENTRE[1]) * 110.57)
    offset = ((n * 5 + 3) % 13) / 12 - 0.5
    return round(35.8 - 0.165 * dist_km + offset, 1)


def main():
    gj = json.loads((FIX / "V1" / "districts.geojson").read_text(encoding="utf-8"))
    for f in gj["features"]:
        f["properties"]["july_temp_c"] = july_temp(f)
    districts_text = json.dumps(gj, ensure_ascii=False)
    schools_text = (FIX / "T1" / "schools.csv").read_text(encoding="utf-8")
    data = {
        "districts": {
            "file": "districts.geojson",
            "bytes": len(districts_text.encode("utf-8")),
            "geojson": gj,
        },
        "schools": {
            "file": "schools.csv",
            "bytes": len(schools_text.encode("utf-8")),
            "csv": schools_text,
        },
        "expected": {
            "insight_population_csv": (FIX / "V1" / "expected_insight.csv").read_text(encoding="utf-8"),
            "count_in_areas_csv": (FIX / "T1" / "expected_count_in_areas.csv").read_text(encoding="utf-8"),
        },
    }
    blob = json.dumps(data, ensure_ascii=False).replace("</", "<\\/")
    template = (HERE / "template.html").read_text(encoding="utf-8")
    assert template.count("__LM_DATA__") == 1
    out = template.replace("__LM_DATA__", blob)
    (MOCK / "index.html").write_text(out, encoding="utf-8")
    temps = sorted(((f["properties"]["july_temp_c"], f["properties"]["name_en"]) for f in gj["features"]),
                   reverse=True)
    print(f"wrote {MOCK / 'index.html'} ({len(out.encode('utf-8')):,} bytes)")
    print("july_temp_c:", ", ".join(f"{n} {t}" for t, n in temps))


if __name__ == "__main__":
    main()
