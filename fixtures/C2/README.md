# C2 — GCJ-02 / BD-09 test vectors

## `vectors.csv` (library half — done)

58 points: 50 in China (38 named cities/places incl. far
north-east (Mohe), far west (Kashgar), far south (Sanya, Zengmu Ansha area);
12 random points in 98–122°E, 22–42°N, seed 202) and 8 outside
China, where GCJ-02 must be the identity (asserted).

Columns: WGS 84 input; GCJ-02 from implementation A and B; BD-09 from A and B
(both computed from A's GCJ-02, so they test the GCJ-02 → BD-09 step alone).
Values printed with 9 decimals (~0.1 mm). For the outside-China points the BD-09
columns are just the raw GCJ→BD formula applied to the unshifted input (neither
library has an out-of-China rule for that step); they are not meaningful positions.

| impl | package | version | license |
|---|---|---|---|
| A | `prcoords` (github.com/Artoria2e5/PRCoords) | 1.0.2 | CC0-1.0 / GPL-3.0-or-later (dual) |
| B | `coord-convert` (github.com/sshuair/coord-convert) | 0.2.1 | MIT |
| C | `eviltransform` (github.com/googollee/eviltransform), cross-check only | 0.1.1 | MIT |

Agreement: max |A − B| GCJ-02 = 0.0e+00°, max |A − B| BD-09 = 0.0e+00°,
max |A − C| GCJ-02 = 1.2e-07° (A/B: floating-point level; C differs by ~1 cm,
from slightly different constants/arithmetic — well below any map use). All three
derive from the same widely circulated reverse-engineered algorithm, so they
are independent *implementations*, not independent *ground truth* — the
hand-read Amap landmarks below supply that.

Caveat: all three libraries use the same crude rectangle as "China"
(72.004–137.8347°E, 0.8293–55.8271°N). Points such as Seoul, Hanoi or Taipei
fall inside that rectangle and **are** shifted by these libraries; none are
included here. LayeredMap's own in/out-of-China policy should be decided
separately.

## `landmarks_TODO.csv` (owner's half — TODO)

Headers only. The owner should hand-read ~10 Amap landmark positions and fill
`name, osm_node_id, wgs84_lon, wgs84_lat` (from the OSM node) and
`amap_lon, amap_lat` (read from Amap). These are the independent ground truth
for the GCJ-02 conversion.

Made by `reference/coord_vectors.py`.
