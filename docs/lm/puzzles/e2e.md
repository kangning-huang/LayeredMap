# The eight puzzles end to end through the Pro assistant

> **Owner's direction (2026-10-17):** Pro's assistant does not need to solve a puzzle from one request. Its job is to carry out each step a user asks for, so the measure that matters is single-step requests, plus the gold plans below, which show that the actions themselves solve every puzzle. The "still missing" items for solving P3, P7 and P8 in one request are recorded for reference, not planned.

Branch `puzzle-e2e`, 2026-10-17. What was run: each puzzle as a hand-written gold plan through
LayeredMap's plan executor (`run_plan`), then each puzzle's request (English, and Chinese where the
puzzle has one) through the Pro assistant pipeline: the v2 harness in the `submit_plan` design,
the plan checker, `resolve_feature`, then `run_plan` on the real files, then an answer check
against `puzzles.json`. Models: Claude Haiku 5.5 and GLM 5.3 Flash, two runs each.

**In short.** LayeredMap's certified actions solve all eight puzzles: every gold plan reaches the
answer, and its candidate counts match the reference solver's wherever both test the same thing.
From the request alone, the assistant solved 24 of 52 requests in the reported run. P1, P2 and P5
were solved every time (16 of 16), P6 3 of 4 times (after one harness fix). P4 3 of 8, P8 2 of 8,
P3 and P7 never. P7 can't be solved from a plan made before anything runs: no action gives a
per-city "number of countries within 30 km". P3 and P8 fail mostly because the islands are split
across two layers and the models search the wrong one, and because `nearest_other` only measures to
the objects still shown. All model spending: US$0.34 of the US$1.50 cap.

## 1. Gold plans

`tools/lm/puzzles/plans/P1.json` to `P8.json` are in the executor's format: layers with paths
into `/Volumes/LaCie/data/layeredmap-puzzles`, then steps `{name, args}`. `P4_counts.json` repeats
P4's find_where with one condition at a time, to count the areas per clue. To run them all and
check the answers:

```sh
cargo build -p lm-jobs --release --example run_plan
~/miniforge3/bin/mamba run -n lm-llm-eval python -I tools/lm/puzzles/pro_e2e.py --gold   # -> results/gold.json
```

Answer check (used for the gold plans and the assistant alike). A city must match by name or lie
within 25 km of the answer. An island's label point must lie within 30 km of the answer point. The
P4 area's label point must lie within 250 km of the answer's centroid, and a card must list Simao.
The check uses the last layer the plan filters, skipping layers that are only used as a mask or as a
distance target. "Solved" means exactly one object is left and it is the answer. "Solved with cards"
means several are left, but the plan's "N countries inside <city>" cards leave only the answer.

| Puzzle | Answer | Gold steps | Solves? | LayeredMap candidates per clue | `solve.py` per clue | Run time |
|---|---|---|---|---|---|---|
| P1 | Kunming | 5 | yes | 45 → 2 → 1 | 45 → 2 → 1 | 0.04 s |
| P2 | Winnipeg | 5 | yes | 1,438 → 146 → 1 | 1,448 → 146 → 1 | 0.15 s |
| P3 | Ulleungdo | 7 | yes | 52 → 6 → 1 | 231 → 6 → 1 | 5.6 s |
| P4 | Pu'er tea hills, Simao | 6 | yes (town from the card) | 1,075 → 573 → 355 → 356 → 12 → 1 | 1,075 → 573 → 355 → 356 → 15 → 1 | 0.2 s |
| P5 | Zaragoza | 6 | yes | 1,397 → 29 → 4 → 1 | 1,398 → 29 → 4 → 1 | 0.15 s |
| P6 | Nairobi | 13 | yes | 1,290 → 53 → 7 → 6 → 5 → 1 | 1,290 → 53 → 7 → 6 → 5 → 1 | 1.0 s |
| P7 | Foz do Iguaçu | 18 | yes, with cards | 1,037 → 342 → 48 → 7 → 2 (cards) → 1 | 1,042 → 343 → 48 → 7 → 2 → 1 | 0.3 s |
| P8 | Christmas Island | 12 | yes | 2,754 → 344 → 5 → 2 → 1 | 3,876 → 346 → 5 → 2 → 1 | 5.2 s |

The plans, briefly:

- **P1** filter (ADM0NAME is China, POP_MAX above 2,000,000), then add_raster_values (Elevation),
  filter, then add_raster_values (January), filter.
- **P2, P5** join_by_location (CONTINENT from Countries), then filter step by step (P5:
  `NAME starts_with Z`), then add_raster_values and a final filter.
- **P3** split_parts (Land), add_measurements, nearest_other, join_by_location (ADMIN), then filter
  step by step.
- **P4** find_where (four conditions), filter Countries to China, keep_inside, add_measurements,
  filter (area ≥ 10,000 km²), whats_inside (Populated places).
- **P6** join, filter, then add_raster_values ×3, calculate_field (July − January), and
  nearest_distance to Coastline and to Active faults, with a filter after each clue.
- **P7** join (CONTINENT, POP_EST), add_raster_values, filter Rivers (scalerank ≤ 4),
  nearest_distance, add July, buffer 30 km (one area per city), 7 × whats_inside (Countries),
  filter POP_EST.
- **P8** split_parts (Land), add_measurements, nearest_other, filter (tropics, area),
  nearest_distance (Minor islands), filter, summarize_each (rain), then calculate_field twice
  (ratio, then ratio²) and a final filter.

### Where LayeredMap's counts differ from `solve.py`, and why each answer stays unique

- **P2, P5, P7, first clue: 10, 1 and 5 fewer places.** `join_by_location` matches a point only to
  a country polygon it lies in or on. 18 of 7,342 places are coastal points just outside the 1:10m
  coastline and get no CONTINENT. The solver gives these the nearest country instead. From the
  second clue on the counts are equal (P7: 342 against 343), so no candidate that matters is
  affected.
- **P3, first clue: 52 against 231.** There are two causes. (a) The gold plan uses Land only (6,837
  parts). The solver also counts Natural Earth's minor-islands layer, and no action merges two layers
  (section 5). (b) `join_by_location` places an island by its label point, so islets whose point
  falls outside the 1:10m South Korea polygon get no ADMIN: 4,064 of 6,837 parts are matched. The
  solver uses "intersects, else the nearest country within 50 km". Both give the same 6 islands of
  50–100 km². Ulleungdo measures 73.09 km² (solver 73.1) and lies 88.4 km from the nearest land
  (solver 88.2). The certified distance is geodesic on WGS 84, edge to edge, densified to 1 km; the
  solver uses a sphere and vertices every 0.02°. The 13 km margin above the 75 km threshold is far
  larger than the 0.2 km difference, and the other five islands are within 9 km of land.
- **P4, inside China: 12 against 15.** `keep_inside` keeps each found area as one feature, cut by the
  1:10m border: an area split by the border becomes one multi-part feature. The solver re-labels the
  connected cells that fall inside a cell-centre China mask, so the same cells can make more
  regions. The tea area is 58,473 km² clipped geodesically (solver 57,516 km² from whole cells). The
  next largest is 3,333 km² (solver 1,576), so it is unique by a factor of 17. The four find_where
  counts are identical (1,075, 573, 355, 356) because find_where's cells and 8-connectivity are the
  solver's definition. Note a trap: find_where's `area_km2` field is copied, not recomputed, by
  keep_inside, and add_measurements then writes `area_km2 2` (section 4).
- **P6.** The counts are identical. Nairobi's fault distance is 22.17 km (solver 22.2) and its
  coast distance 408.1 km (408.1).
- **P7, three countries within 30 km.** A 30 km buffer per city, then whats_inside(buffer, Countries)
  for each of the 7 candidates. Foz do Iguaçu and Ciudad del Este get 3 countries, the other 5 get 2:
  the solver's 2. The gold plan can name the 7 buffers only because its author saw the 7 candidates;
  no plan written before running can do that (section 5).
- **P8, tropics and area: 2,754 and 344 against 3,876 and 346.** The plan uses Land parts as
  candidates; minor islands count only as neighbours, through nearest_distance to Minor islands, so
  "nearest other land" is the smaller of the two distances. The solver uses both layers as
  candidates. From the isolation clue on the counts are equal (5, 2, 1). Christmas Island is 345.9 km
  from Java (solver 347.6). Its rain is 1,978.6 mm, an area-weighted fractional mean (solver
  1,970.8 mm, from all touched cells), well above 1,500. Its ratio is −10.07 at the label point
  (solver 10.06 at the representative point).
- **P8, "ignoring signs".** calculate_field has no absolute value. The gold plan computes
  ratio = longitude ÷ latitude, then ratio × ratio, and keeps 90.25–110.25. That is exactly
  |ratio| between 9.5 and 10.5.

## 2. Fixes made

**Executor (`crates/lm-jobs`)**

| Fix | Why | Test |
|---|---|---|
| Filter ops `at_least` and `at_most` (inclusive) in `plan_exec`, mapped to the filter's inclusive `between` with an open end | `above`/`below` are strict, so "at least 1,500 m" had no exact translation (`between X and ∞` was the only route) | `plan_exec::tests::at_least_and_at_most_include_the_bound` |
| `run_plan` example: a new layer's outcome now gives its `alias` (`#k`) and object count | The per-step candidate counts in this report | (example only) |

No certified algorithm was changed, and no definition. GEM faults (GeoJSON lines, 13,696 traces) and
the Natural Earth shapefiles all import and run as they are, so input formats are not a gap.

**Assistant harness (`tools/lm/llm_eval/v2`, shared with the v2 benchmark)**

The checker and tool descriptions were written before the puzzle actions existed, and several
things broke on the first plans:

- **Parameters dropped.** The checker silently dropped calculate_field's `a`, `op`, `b` and `name`,
  find_where's `conditions` and join_by_location's `fields`. It now checks and keeps them (a and b
  numeric; find_where conditions name rasters; join fields exist on the areas).
- **New layers and fields.** The checker now knows which steps make a layer (split_parts,
  find_where, keep_inside, remove_inside) and which change one in place (join, nearest_*,
  add_measurements, calculate_field). It names the fields each step adds as the runner does: ports of
  `singular`, `plural`, `word` and `free_field`, so it knows `km to nearest active fault`,
  `km to nearest other area`, `area_km2 2` and joined fields. `#k` pointing at an in-place step is
  corrected to that step's layer. A buffer's layer has the one field `name`, not the source fields.
- **Filter values.** A filter on a joined field is checked against the area layer's values
  (CONTINENT is Africa), not the points'. `starts_with` and `ends_with` are no longer checked as
  whole stored values (P5's "Z" was rejected).
- **Ops.** `at_least` and `at_most` are in the tools and the checker.
- **Repair hints.** add_raster_values on areas now points to summarize_each; before, the hint said
  "No open layer is of that kind: ask_user or cannot_do", and Haiku refused P8 because of it.
  whats_inside on points says "buffer them first". A wrong `#k` lists the step outputs of the right
  kind.
- **calculate_field in the submit_plan schema.** `a` and `b` are now called `first` and `second`.
  GLM never produced `a` in 61 of 61 calculate_field steps in the first two runs. It wrote `area: {...}` instead: one step
  object holds every action's inputs, and `a` is the start of `area`/`areas` (`b` of `band`). After
  the rename it used `first` in 64 of 64 steps and solved P6 4 of 4 times.
- **Prompt and descriptions.** Rule 2 lists all layer-making and in-place actions. Rule 3 says a
  filter replaces the layer's earlier filter. The tool descriptions give the field names the new
  actions write, say that nearest_other counts only shown objects, and say keep_inside copies
  area_km2.
- **Registration hook.** `data.REGISTERED` lets another harness register attribute tables
  (resolve_feature and the checker read the puzzle layers through it).

New tests: `test_v2.py::test_checker_knows_the_puzzle_actions_fields_and_layers`,
`test_runner_word_ports`, and `test_puzzles_e2e.py`, which covers the leak check, the gold plans'
layers, the answer check and the DBF reader. All 62 Python tests pass.

**The e2e harness, `tools/lm/puzzles/pro_e2e.py`**

The model sees the v2 rules and nine worked examples. It also sees three extra rules (translating
"at least / or more / 不低于" etc.; which action gives which value; added values fill only shown
objects) and five extra worked examples on the v2 invented map (Hospitals, Wards, Rivers, Rainfall,
plus Temperature and Lakes). `--check-leaks` shows the closest pair of example and puzzle requests
at character-trigram Jaccard 0.25, under the v2 limit of 0.35.

The map context comes from each puzzle's own files. Each layer gets its kind and object count. Each
documented field gets its type, unit and meaning, from Natural Earth's attribute notes (POP_MAX:
"population of the place: the larger of its city-proper and metropolitan figures"; ADM0NAME;
CONTINENT; POP_EST, people; scalerank). The other fields are listed by name only. Each raster gets
its unit. Category values are not listed.

resolve_feature answers from the shapefiles' own DBF tables, read with a small built-in dBase reader
(the lm-llm-eval environment has no GDAL), and the GeoJSON's properties. Countries.CONTINENT takes
the v2 Chinese continent aliases.

The Rivers layer's description says "'major rivers' are those of scalerank 4 or less". That phrase
is in the dataset's own description in `puzzles.json`, not in the gold plan, but it does hand the
models P7's threshold.

The first `submit_plan` call is forced. The checker allows two repair rounds. Spending goes to the
ledger `results/spend.json`, capped at US$1.50, and the key is read from the keychain at run time.

## 3. Pro end to end: results

Reported run: `tools/lm/puzzles/results/pro-e2e-final/`, two runs × 13 requests × 2 models, with the
harness frozen as committed. Each cell gives: solved runs / runs · steps in each run · cost of both
runs · mean seconds per request (model time plus `run_plan`).

| Puzzle | Haiku 5.5, English | Haiku 5.5, Chinese | GLM 5.3 Flash, English | GLM 5.3 Flash, Chinese |
|---|---|---|---|---|
| P1 | 2/2 · 4/4 · $0.0016 · 5 s | 2/2 · 4/4 · $0.0026 · 7 s | 2/2 · 4/4 · $0.0014 · 8 s | 2/2 · 4/4 · $0.0014 · 8 s |
| P2 | 2/2 · 4/4 · $0.0013 · 3 s | (no Chinese request) | 2/2 · 5/4 · $0.0017 · 5 s | (no Chinese request) |
| P3 | 0/2 · 5/4 · $0.0024 · 27 s | 0/2 · 5/1 · $0.0031 · 8 s | 0/2 · 9/9 · $0.0036 · 9 s | 0/2 · 14/10 · $0.0051 · 23 s |
| P4 | 1/2 · 6/6 · $0.0026 · 7 s | 1/2 · 4/6 · $0.0025 · 5 s | 1/2 · 5/4 · $0.0017 · 6 s | 0/2 · 3/3 · $0.0015 · 4 s |
| P5 | 2/2 · 4/3 · $0.0019 · 5 s | (no Chinese request) | 2/2 · 5/4 · $0.0016 · 3 s | (no Chinese request) |
| P6 | 1/2 · 10/9 · $0.0035 · 9 s | (no Chinese request) | 2/2 · 10/9 · $0.0021 · 6 s | (no Chinese request) |
| P7 | 0/2 · 1/1 · $0.0022 · 4 s | 0/2 · 1/1 · $0.0024 · 3 s | 0/2 · 0/0 · $0.0080 · 37 s | 0/2 · 9/10 · $0.0068 · 26 s |
| P8 | 0/2 · 5/1 · $0.0012 · 5 s | 0/2 · 9/8 · $0.0035 · 12 s | 1/2 · 17/16 · $0.0125 · 76 s | 1/2 · 11/8 · $0.0082 · 47 s |

Totals for the reported run: 24 of 52 solved, 11 of 26 for Haiku and 13 of 26 for GLM. The
outcomes: 24 solved, 9 empty (the final filter left nothing), 7 execution errors, 4 asked, 2
refused, 2 wrong, 2 ambiguous, 2 with no plan. The run cost US$0.086, with 11 s of model time per
request on average.

Development runs, kept for the record: `pro-e2e-dev1`, `-dev2` and `-dev3`. Between them the
harness changed: hints, the answer check, the `first`/`second` rename, and the islands'
descriptions. They solved 21, 22 and 25 of 52. The spread from run to run is about as large as any
effect of those changes, so per-puzzle numbers from two runs are indicative only, except for the
GLM calculate_field fix (P6: 0 of 4 before, 4 of 4 after).

### Failures and their causes

| Puzzle | What went wrong (final run) | Cause |
|---|---|---|
| P3 (0/8) | Haiku searched Minor islands only (Ulleungdo is in Land) twice, searched Land without the country condition once, and once asked whether to drop that condition. GLM clipped Land and Minor islands to South Korea with keep_inside in all 4 runs, then measured the clipped land without splitting it into islands, measured isolation only among the clipped parts, or measured a layer against itself with nearest_distance (always 0). One GLM plan was down to a single island at step 6 (its area and isolation tests on South Korea's land parts) and lost it by step 10. In development runs, nearest_other after a South Korea filter measured only to the filtered islands | **Data**: islands split across two layers. **Action definition**: nearest_other's targets are the shown rows (section 4). **Model** |
| P4 (3/8) | Forgot "in China" and called whats_inside on #1 with 38 areas left ("which object of #1?") in 4 of 8. Used keep_inside (in development runs also join_by_location) for "which town is in it" instead of whats_inside | **Model**. whats_inside needs exactly one shown area, and the plan is made before anything runs |
| P6 (3/4) | Haiku wrote "July minus January at least 1" for "July at least 1 °C cooler" | **Model** (sign) |
| P7 (0/8) | Haiku refused or asked in all 4 runs: "no tool counts the countries within 30 km of each city". GLM buffered the cities, then used keep_inside or whats_inside on all the buffers, or ran out of its 4,000 output tokens (2, no plan) | **Action gap**: no per-feature count of nearby areas or lines; the gold plan needs the candidates' names |
| P8 (2/8) | Signed ratio between 9 and 11 (Christmas Island's is −10), dropped the ratio, searched Minor islands, wrong `#k` after summarize_each. GLM solved it twice: it squared the ratio by itself (lon²/lat² between 81 and 121, a wider band that still leaves one island) | **Action gap**: no absolute value in calculate_field. **Data**: two island layers. **Model** |

## 4. Questions before changing a definition

I left these as they are, as the brief asks. Each one cost the assistant solves.

1. **`nearest_other` targets.** Today the targets are the rows in scope, so after a filter, an
   island's "nearest other land" is the nearest *shown* island. A model that filters to South Korea
   first, which is the natural order and rule 3's "filter first", gets wrong isolation distances.
   Should the targets be every object of the layer, with only the shown rows getting values? (The
   current behaviour follows the A3 definition: "the targets are the features themselves".)
2. **`keep_inside` / `remove_inside` and measurement fields.** An `area_km2` field from find_where
   is copied unchanged into the clipped areas, and `add_measurements` then writes `area_km2 2`.
   Should clipping drop or recompute `area_km2` / `length_km`, or should `add_measurements`
   overwrite its own fields? (P4's answer happens to survive the stale field.)
3. **`join_by_location` for points just outside every area** (18 coastal cities): add the solver's
   "nearest area within N km" fallback, or keep the exact "in or on" rule?
4. **`filter` replaces the earlier filter.** The prompt now says so, and models mostly restate the
   earlier conditions. A narrowing filter (AND with the current one) would be simpler for plans.

## 5. Gaps no action fills (documented, not worked around)

- **Count nearby areas or lines per feature (P7).** count_in counts points in areas. Nothing gives
  each city "the number of countries within 30 km" (or "boundary lines within 30 km") as a field
  that a filter can test. With it, P7 is: buffer the cities, one area each, then
  count_in(Countries, buffers) generalized to areas and lines, then filter ≥ 3. Without it, only a
  plan written after seeing the candidates works.
- **Merge two layers (P3, P8).** Natural Earth keeps islands in Land and Minor islands. An
  "append layers" action, or one islands dataset, would remove the most common P3/P8 failure.
- **Absolute value in calculate_field (P8).** For example an `abs_divide` op, or a unary `abs`.
  The squared-ratio route works but only GLM found it.
- **One plan, no look at results.** whats_inside, measure_* and summarize_here act on one object.
  A plan made before running can use them only when earlier filters are sure to leave exactly one
  object (P4), so these puzzles favour multi-turn planning.

## 6. What each puzzle still needs to be solved by Pro from the request alone

| Puzzle | Status | Needed |
|---|---|---|
| P1, P2, P5 | Solved in 16 of 16 runs | Nothing |
| P6 | 3 of 4 | Nothing structural; the July − January sign is the model's to get right (a worked example of "X cooler than Y" would help) |
| P4 | 3 of 8 | A checker warning when an object step names `{layer: "#k"}` with no filter that guarantees one object, or whats_inside "for each area"; models also need to keep "in China" |
| P3 | 0 of 8 | nearest_other measuring to all objects (question 1), one islands layer or an append action, and a checker lint for nearest_distance from a layer to itself |
| P8 | 2 of 8 | The same two as P3, plus an absolute value in calculate_field |
| P7 | 0 of 8 | A per-feature count of areas or lines within a distance (or count_in for areas and lines), or multi-turn planning |

## Reproducing

```sh
export CARGO_TARGET_DIR=/Volumes/LaCie/cargo-target/layeredmap-wt-e2e
cargo build -p lm-jobs --release --example run_plan
PY="$HOME/miniforge3/bin/mamba run -n lm-llm-eval python -I"
$PY tools/lm/puzzles/pro_e2e.py --gold                       # gold plans -> results/gold.json
$PY tools/lm/puzzles/pro_e2e.py --show-context P6            # what the model sees
$PY tools/lm/puzzles/pro_e2e.py --dry-run                    # mock model answering with the gold plans
$PY tools/lm/puzzles/pro_e2e.py --runs 2 --tag final         # both models, ledger results/spend.json
$PY tools/lm/puzzles/pro_e2e.py --report pro-e2e-final       # summary table
```

The reported run's per-request records are in `results/pro-e2e-final/*.jsonl`. Each record holds
the plan, the checker log, tokens, cost, latency, run_plan's step outcomes, the candidates per step
and the outcome. The development runs' summaries are in `results/pro-e2e-dev*/summary.md`; their
JSONL files stay on the LaCie drive, not in git.
