# P2 actions: definitions and reference tests (WP6.4)

**As of 2026-10-18.** Three actions the plan lists as P2 for Phase 6: match a table to a layer,
nearest feature, and keep data inside an area. Each has a definition here, a reference written
in Python with the library the plan names, and Rust tests that compare the two with stated
tolerances. Most of the machinery came earlier: the join is WP3.11's (`lm_jobs::join`), and
nearest and clip are Phase 4's actions A3 and A2 (`lm_analysis::nearest`, `lm_analysis::clip`,
run by `lm_jobs::actions` as `nearest_distance`, `nearest_other`, `keep_inside` and
`remove_inside`). WP6.4 added the join's cardinality check and expected row count, and the
references against pandas `merge`, GeoPandas `sjoin_nearest` and GeoPandas `clip`.

| Action | Code | Reference | Fixture | Rust test |
|---|---|---|---|---|
| Match a table to a layer | `lm_jobs::join::{expect, apply}` | pandas `merge(validate=…)` (`tools/lm/reference_tests/join_validate.py`) | `fixtures/J6/` | `join::tests::j6_matches_pandas_merge_validate` |
| Nearest feature | `lm_analysis::nearest::nearest_distance` | GeoPandas `sjoin_nearest` in UTM 51N, pyproj `Geod` (`nearest_sjoin.py`) | `fixtures/N14/` | `nearest::tests::n14_matches_sjoin_nearest_and_pyproj` |
| Keep data inside an area | `lm_analysis::clip::keep_inside` | GeoPandas `clip(keep_geom_type=True)` (`clip_gpd.py`) | `fixtures/N15/` | `clip::tests::n15_matches_geopandas_clip` |

The older references stay: N9 (nearest, against a per-feature azimuthal equidistant projection,
including the antimeridian and high latitudes) and N12 (keep and remove, against GeoPandas
`overlay`). To regenerate everything: `mamba run -n layeredmap-ref python -I reference/run_all.py --check`
(it runs the WP6.4 scripts too).

## Match a table to a layer

**Inputs.** A table (no geometry) and a map layer, a key field on each, and how loosely keys
are compared (WP3.11's ladder: exact, folded, stripped, codes, crosswalks). Normally the join
finds these itself (WP3.11); the definition below is about what happens once they are chosen.

**Output.** The layer with the table's fields added after its own. The layer keeps exactly one
row per record, whatever the table looks like: a record matching several table rows takes the
latest by a year or date column, or else counts summed and other numbers averaged (WP3.11's
rule, with its chip). Records with no match get blanks ("No data" on the map).

**Matching.** A table row matches every layer record whose key is the same at the chosen level.
A blank key (empty, or only spaces) never matches anything. A record whose key repeats in the
layer (one place stored as two records, such as a mainland and an island part) gets the row
too: every such record, not just one. Before WP6.4, records whose key repeated were left out of
the matching, so their rows were listed as unmatched; see *Decisions* below.

**Cardinality.** With the layer on the left, as in `layer.merge(table, how="left")`:

| | no key repeats in the table | a key repeats in the table |
|---|---|---|
| **no key repeats in the layer** | one-to-one | one-to-many |
| **a key repeats in the layer** | many-to-one | many-to-many |

"Repeats" counts every row with a key, matched or not, as pandas' `validate` does; blank keys
are left out. So a table that repeats a key nothing matches is still one-to-many: the check is
about the keys, and the card says how many rows share one.

**Expected row count.** Worked out before anything is joined (`join::expect`):

- *pairs*: matching (record, table row) pairs, the rows an inner merge gives;
- *plain merge rows*: each record once per table row it matches, and at least once: the rows a
  left merge gives. This is the number that surprises people in other tools when a key repeats;
- *rows kept*: the layer's own row count, always.

**What the user sees.** The result card has one line under "how it matched", for example:

- "One-to-one: no key repeats. The layer keeps its 17 rows."
- "One-to-many: a key repeats in the table (up to 5 rows). A plain merge would give 85 rows; the
  layer keeps its 17, one per area." (followed by the chip: "Showing 2023")
- "Many-to-one: a key repeats in the layer (up to 2 areas), and each gets the row. The layer
  keeps its 14 rows."

When the join has to ask (no clear pairing), each choice shows the same line under it, so the
count is seen before anything is joined. A clear join asks nothing (WP3.11); its line is on the
card right after, beside Undo.

**Unmatched rows.** Listed for review from the card: "Show the 3 not matched" lists the
unmatched keys as written, in table order (up to 20, then "and N more"). When a key repeats or
is blank, the link counts rows too ("Show the 2 not matched (5 rows)"), and the list ends with
"1 row with no ward_code". `Joined::unmatched_rows` keeps every unmatched table row (blank keys
included), for the table drawer or an export.

**Reference.** pandas `merge` with `validate`: the cardinality is the tightest of `one_to_one`,
`one_to_many`, `many_to_one`, `many_to_many` that pandas accepts; plain merge rows are
`len(merge(how="left"))`, pairs `len(merge(how="inner"))`; each record's value is the merge's
(with one row per record) or the latest year's. Deliberate differences: rows with a blank key
are dropped before merging (pandas matches a blank with a blank, and counts two blanks as a
repeated key); keys are compared exactly in the reference, while LayeredMap compares them at the
join's level (the ladder is tested separately, J1–J5).

**Tolerance.** Everything exact: the cardinality, all five counts, the unmatched keys and rows
(in order), the blank keys, and the value of every record (52 values over four pairings).

## Nearest feature

**Inputs.** A layer to annotate (points, lines or areas) and a target layer (points, lines or
areas, or the same layer for "nearest other").

**Output.** Two new fields on each feature: the distance in km to the nearest target, and the
target's name. A feature without a location gets blanks and a notice.

**Definition** (A3, unchanged): the shortest geodesic distance on the WGS 84 ellipsoid from any
point of the feature to any point of any target; 0 when they touch, cross or one lies inside the
other (decided exactly, with edges straight in longitude and latitude, as the rest of the app).
Edges are followed at least every 1 km, and each vertex-to-edge distance is measured on the
ellipsoid (GeographicLib); the search is exact, not sampled. Ties go to the lower target row.
On the same layer a feature never matches itself or its own other parts.

**Geodesic method, and why the reference is projected.** The plan names GeoPandas
`sjoin_nearest` in a local projection, checked against pyproj. `sjoin_nearest` measures in the
projection, where UTM's scale error in N14's region is about 1e-4 (0.1 m per km), so the
reference uses it to *find* the nearest target and the two nearest points, takes those points
back to longitude and latitude, and measures between them with pyproj `Geod.inv`. The script
asserts that the two distances agree within 0.1%, and N14's sources are kept only when the
nearest target is at least 2% closer than the next, so the projection can't change the answer.
`Geod.inv` is exact between two points; to a line or an area the reference's nearest point is
on a straight edge in UTM rather than on the geodesic edge LayeredMap follows, which over these
1–10 km edges is a few centimetres apart.

**Tolerance.** The nearest target exactly; touching features exactly 0; point to point within
1 mm; to lines and areas within 10 cm. That is tighter than the plan's 0.1% or 1 m, and leaves
room only for the reference's own straight-edge approximation. The test prints the largest error
of each pairing (see *Results*).

## Keep data inside an area

**Inputs.** An area, line or point layer, and an area layer (the mask: all its features, or
those its filter shows).

**Output.** A new layer with what is left of each feature, attributes copied; features left
with nothing are dropped and counted on the card.

**Definition** (A2's keep, now checked against GeoPandas `clip`): the mask is the union of the
mask layer's areas, edges straight in longitude and latitude. Each feature becomes its
intersection with the mask, and keeps only its own kind of geometry:

- **Points on the boundary** (an edge, a corner, a hole's edge) are kept; points in a hole are
  not. A multipoint keeps the points left and stays one feature.
- **Lines** are cut where they cross the boundary; a stretch along the boundary is kept. A line
  that leaves and comes back becomes one feature with several parts. A line that only touches
  the mask at a point is left with nothing and dropped (GeoPandas would make a point and, with
  `keep_geom_type`, drop it too).
- **Areas** are cut to the mask, holes included. An area that only shares an edge or a corner
  with the mask is dropped (the intersection is a line or a point, not an area). An area that
  covers the whole mask becomes the mask, in as many parts as the mask has.
- **Multipart features** keep the parts that are left, as one feature.
- **Empty results**: features with nothing left, or no geometry to begin with, are dropped and
  counted ("3 lines had nothing left and were dropped"). A mask layer with no areas keeps nothing.

**Precision.** Vertices that come from the inputs are exact; new crossing points carry the
overlay's integer-grid rounding (about span × 2⁻²⁹, under 1e-9° for N15), as recorded for A2
(decisions.md, 2026-10-17).

**Reference.** GeoPandas `clip(gdf, mask, keep_geom_type=True)`, areas and lengths by pyproj
Geod. GEOS splits a line where it meets the boundary, so the reference merges line parts with
`shapely.line_merge` before counting them.

**Tolerance.** The same features kept and the same number of parts, exactly; areas and lengths
within 1e-6 relative; coordinates within 1e-9° (input vertices exactly).

## Results

Run on 2026-10-18 (macOS, `cargo test`):

| Test | Compared | Largest difference | Tolerance |
|---|---|---|---|
| J6, join | 4 pairings: cardinality, 5 counts, unmatched keys and rows, blank keys; 52 record values | none | exact |
| N14, sources → facilities | 16 nearest targets and distances | 0.0000 m | 1 mm |
| N14, sources → roads | 16 (1 touching) | 0.029 m | 10 cm |
| N14, sources → parks | 16 (1 inside, 1 in a hole) | 0.004 m | 10 cm |
| N14, parks → facilities | 4 | 0.005 m | 10 cm |
| N15, clip | 25 features: 16 kept, 9 dropped, all as GeoPandas | areas and lengths 9.5e-10 relative; coordinates 1.4e-10° | 1e-6; 1e-9° |

One real discrepancy was found and fixed: a layer whose key repeats (J6's `wards_split`) got no
data at all, or lost the repeated records, because the join matched only keys that are unique
in the layer. Nearest and clip matched their references without changes; the edge cases GeoPandas
`clip` drops (an area sharing only an edge or a corner, a line touching at one point) are
dropped by LayeredMap too.

## Decisions

Recorded in `decisions.md` (2026-10-18): records whose key repeats in the layer now all take the
row; the cardinality follows pandas (all keys, blank keys left out); a clear join still asks
nothing, so its expected row count is on the card right after, and on the question card before
when the join asks.
