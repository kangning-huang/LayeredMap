# Gate 3 report — Inspect, select, and insight cards

Build: 0.3.0-dev · branch `phase-3` · draft (not frozen)
Reference machine: Mac mini (Mac16,10), Apple M4, 16 GB, macOS 26.6.2, 5K display at 2×

**State:** the P0 and P1 work packages are built; the automated checks and stress numbers below
are in; your validation is next.

## What Phase 3 adds

- **Click anything on the map** (WP3.1): the object under the pointer is selected and outlined
  (3 px accent outline, 2 px halo; points grow 1.4×). Several objects within 4 px open the overlap
  chooser ("2 objects here", topmost first; hovering a row highlights it). ⇧-click adds or removes
  objects ("3 selected · Clear"). A click on empty map clears the objects and keeps the layer; Esc
  steps back one level at a time. A raster cell can be clicked too. Rows a filter hides are never
  hit. ⌘-click on the shelf adds a layer as a companion.
- **Object card** (WP3.2): the object's name (the Name field, else the most unique short text,
  else "Districts 7"), its kind and geodesic size ("districts · area · 237 km²"), up to three
  insight rows, up to six facts ("No value" for blanks, never confused with zero), Show all fields,
  suggested actions, View in table · Zoom to.
- **Insight strip** (WP3.4): "5th highest of 16 with values", "+693,386 above the median", a
  120 px dot strip; percentiles from 1,000 objects; categories as "41% of districts are
  Residential"; raster cells as "higher than 82% of this layer's cells (estimated from a sample)".
  Ranks come from indexes built in the background, so a rank is a binary search.
- **Field roles** (WP3.3): Details lists each field with a role chip (Value, Count, Category, ID,
  Name, Text) you can change, and a star to show a field in insight. Changes are undoable and
  update insight, presets and actions at once.
- **Style tab** (WP3.5): presets drawn from the layer's own fields (single colour, by category, by
  value, size by value, Heat for temperatures, labels on); Smooth or Groups (5 equal-count
  classes, or equal steps, 2–7 behind Classes); Viridis, Cividis, Heat, Blues; Fit range to view;
  opacity (one undo step per drag), outline, labels; the count guard ("Larger areas tend to have
  higher counts. Color by count per km² instead?") with a geodesic density; rasters: band,
  2–98% stretch, range sliders, palette, Show only values above…, Show where values are like this.
- **Legends generated from the style** with units, "No data" (hatched), and the range indicator.
- **Table drawer** (WP3.6, ⌘T or View data): virtualized rows, IDs pinned left, numbers
  right-aligned, "—" for blanks, header click sorts the view, a search box; selection shared with
  the map both ways (a map selection scrolls its row into view).
- **Filters** (WP3.7): "Show districts where population above 500,000" with a live count, chips
  with ×, a funnel on the shelf row; insight counts only filtered records ("of 12 filtered").
- **Undo** (WP3.8): 100 steps; a slider drag is one step; selection and camera are never steps;
  layer data is kept only for revisions the document or undo can still reach.
- **Search your data** (WP3.9): the toolbar search lists matching layers and values (first five
  text fields) above places; choosing a value selects its object and opens its card.
- **Suggested actions**: from the action registry (`lm-actions/rules.toml`, 18 rows, merge and
  companion rules). Per Product tiers the free app shows display actions only; analysis actions
  are never buttons.

## 1. Automated checks

| Check | Result |
|---|---|
| `cargo xtask ci` (fmt, clippy, all tests, layering, wasm) | see § 6 at freeze |
| Property tests for ranks (ties, NaN, ±∞, nulls, −0) against a naive sort | pass (`lm-analysis` proptests) |
| Ranks against fixture V1's reference (`expected_insight.csv`) | pass: every rank, tie, median for 17 districts (`v1_population_ranks_match_the_reference`) |
| `check_insight.py` and the Rust phrases agree | pass: 52 shared golden cases, plus 7 CSV layers row by row |
| Legend–style consistency | pass at data level: every row's colour is a legend swatch, blanks are No data (`groups_match_the_legend`); rendered-pixel sampling still to add |
| Selection in sync between table and map | pass (`selection_is_shared_between_table_and_map`) |
| Undo round trip for style, filter, role and star operations | pass (`filters_roles_and_stars_round_trip`, `a_slider_drag_is_one_step`) |
| Screenshot regression: object card, no value, overlap chooser, filtered, Style tab | pass (`phase3_*`) |
| Action table: a snapshot per row and per merged pair, determinism ×1,000 | pass (`lm-actions` tests) |
| Geodesic length and area against N5 | pass, within 1e-9 relative |
| Fuzzing (Gate 2 carry-over, 1 hour per adapter) | geojson 2,983,162 runs and dbf 1,995,516 runs: 0 findings. csv, shapefile, geotiff_header: one finding each (a slow CSV profile, a 7.8 GB Shapefile allocation, a GeoTIFF header stall) — being fixed, see Gate 2 |

## 2. Debug pass

| Item | Result |
|---|---|
| Independent review of every number a card can show | to do before freeze |

## 3. Your validation — "Explain this map" (75 minutes)

Teaching set K1 is yours to supply (district areas with a July mean temperature field, the
surface-temperature raster with its unit set, school points). Until then V1 + T1 + the examples
exercise the same paths.

1. Open K1.
2. Click five districts and write down their insight rows. Run
   `python validation/gate-3/check_insight.py districts.geojson july_temp --id <district> --id-field <id field> --unit "°C" --noun districts`
   for each and compare every rank and median difference.
3. Details → mark population as ID; confirm it leaves the insight strip; Undo.
4. Style districts with presets; colour by a count field and accept "per km²".
5. ⌘T, + Filter → population above 500,000; select rows and watch the map.
6. Undo ten times and redo ten times; the map must end exactly as it started.
7. Hallway test with two students: "Which district is hottest, and how do you know?"

## 4. Stress tests

Release build, reference Mac (`cargo test --release -p lm-jobs gate3_stress -- --ignored --nocapture`).

| Test | Measured | Threshold | Result |
|---|---|---|---|
| Card on F1 (100,000 points): insight rows + facts after a click | 23 µs + 5 µs (index built in the background in 14 ms) | ≤ 100 ms | pass |
| Card on V7 (100,000 polygons) | 51 µs + 0.2 ms (index 13 ms) | ≤ 200 ms | pass |
| V7: sort the table of 100,000 polygons | 20 ms, off the interface thread | scrolls smoothly | pass (scroll at 60 fps: you) |
| F2: 1,000,000 points styled by value | 42 ms to work out colours (background); index 155 ms; table sort 355 ms (background) | interactive | pass |
| V8: a layer with 200 fields | fixture not generated yet | layout holds | — |
| 100 rapid clicks on overlapping objects | you | no crash, chooser each time | — |
| Opacity slider dragged for 30 s → exactly one undo step | 1,800 changes fold into one op (`a_slider_drag_is_one_step`) | one step | pass |
| 1,000 undo steps, memory measured | stack holds 100; older steps dropped (`depth_is_bounded`); data revisions pruned | bounded | pass (memory figure at freeze) |

## 5. Exit criteria

| Criterion | Met? | Evidence |
|---|---|---|
| Every number from your five districts matches the check script exactly | pending | § 3 |
| A card within 100 ms of a click on F1, 200 ms on V7 | yes | § 4 |
| Both students find the hottest district and explain why, without help | pending | § 3 |
| Undo and redo round trips are exact | yes so far | § 1 |

## Known limits (honest list)

- Keep as new layer (Filter builder) is not built yet; filters are display only.
- The legend–style check samples data colours, not rendered pixels.
- Right-click on a map object (menu of actions) is not built; the card has the same actions.
- Shelf swatches stay the layer's base colour when coloured by value.
- V8 (200 fields) needs `cargo xtask fixtures` to generate it.

## 6. Decision

Pending.
