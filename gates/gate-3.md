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
- **Find in a layer** (WP3.10, added 2026-10-17): a search field at the top of the layer
  inspector, styled like the map's search box. Typing matches every field of every record (case
  and accents ignored; numbers with or without separators), selects the matches on the map, shows
  "47 found" and lists the first eight with the matched field in bold. Return or Zoom to all frames
  them; a row opens the record's card with "‹ All 47 found" to go back; Esc, × or a map click
  clears. ⌘F focuses it when a layer is selected. The table drawer's search matches the same way.
- **Joining a table to places** (WP3.11, added 2026-10-17): open a table without coordinates
  beside map layers and LayeredMap finds the shared column on its own. Keys are compared exactly,
  then ignoring case and accents, then punctuation, then as codes without leading zeros, then
  through bundled crosswalks (252 countries with ISO and World Bank names, 52 US states, 34
  Chinese provinces). A clear answer (80% of keys, 20 points ahead of any pairing that places
  rows differently) joins with no question: the layer gains the table's fields, is coloured by
  the first joined measure, and the table leaves the shelf, all one undo step. Otherwise one
  question card offers the top pairings. Several rows per place take the latest year ("Showing
  2023"), or are combined (counts summed). Typos are never matched on their own; the card offers
  them ("1 name looks like a typo: Hubel → Hubei · Use this match"). Details groups the joined
  fields under "From population.csv".
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
| Find in a layer: every field, accents, numbers, ranking, filter respected | pass (`lm-jobs::find` tests; `phase3_find` snapshot: "pu" selects 5 districts) |
| Join fixtures J1–J5 decide as specified (J1–J3 and J5 join with no question, J4 asks once) | pass (`gate3_join_fixtures`) |
| Join match counts equal `pandas.merge` on the same normalized keys | pass: J1 174 and 20, J2 6, J3 17, J5 7 (`validation/gate-3/check_join.py`) |
| A join is one undo step, and Undo doesn't redo it | pass (`phase3_join_card`) |
| Fuzzing (Gate 2 carry-over, 1 hour per adapter) | pass: 0 findings on the fixed code — csv 1,738,531 runs, geojson 2,983,162, shapefile 6,093,603, dbf 1,995,516, geotiff_header 6,416,344 (Gate 2) |

## 2. Debug pass

| Item | Result |
|---|---|
| Independent review of every number a card can show | to do before freeze |

## 3. Your validation — "Explain this map" (75 minutes)

Uses the bundled World basics example (177 countries with population and GDP), in any 0.6.x
build. (Teaching set K1 was dropped from this gate, owner 2026-10-19.)

1. Choose Examples › **World basics**.
2. Click five countries and write down their insight rows. For each, run
   `python validation/gate-3/check_insight.py assets/examples/countries.geojson pop_est --id <country> --id-field name --noun countries`
   and compare every rank and median difference.
3. Details → mark `pop_est` as an identifier; confirm it leaves the insight strip; Undo.
4. Style the countries with presets; colour them by `pop_est` and accept "per km²".
5. ⌘T, + Filter → `pop_est` above 50,000,000; select rows and watch the map.
6. Undo ten times and redo ten times; the map must end exactly as it started.
7. Hallway test with two students: "Which country is the most crowded, and how do you know?"

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
| Every number from your five countries matches the check script exactly | pending | § 3 |
| A card within 100 ms of a click on F1, 200 ms on V7 | yes | § 4 |
| Both students find the most crowded country and explain why, without help | pending | § 3 |
| Undo and redo round trips are exact | yes so far | § 1 |

## Known limits (honest list)

- Joining: dragging a table onto a layer row is not built (a table joins when opened beside a
  matching layer, or through its question card). Separate (undoing a join long after) is not
  built; Undo covers it until then. A join is re-applied on open only once documents are saved
  (Phase 5). The join itself runs on the interface thread after the background match (fine to
  about 100,000 records; to move to a job before the 1,000,000-row stress test).

- Keep as new layer (Filter builder) is not built yet; filters are display only.
- The legend–style check samples data colours, not rendered pixels.
- Right-click on a map object (menu of actions) is not built; the card has the same actions.
- Shelf swatches stay the layer's base colour when coloured by value.
- V8 (200 fields) needs `cargo xtask fixtures` to generate it.

## 6. Decision

Pending.
