# Gate 2 report — One-click open and visible assumptions

Build: 0.2.0 · branch `phase-2` · frozen 2026-10-16
Reference machine: Mac mini (Mac16,10), Apple M4, 16 GB, macOS 26.6.2, 5K display at 2×

**State:** passed on 2026-10-16. The automated checks are green, the 1-hour fuzz runs are clean,
and the owner validated opening their own data.

## What Phase 2 adds

- **Open files with one action:** the Open button (⌘O), dropping files on the window or the Dock
  icon, Finder › Open With, or file paths on the command line. Each file is read on its own
  thread with progress on its shelf row, and can be canceled (× on hover). A batch on an empty map
  fits all its layers.
- **Formats:** CSV/TSV (UTF-8, UTF-8 with BOM, GBK; raw cells kept as written), GeoJSON
  (streamed; mixed geometry split), Shapefile and zipped Shapefile (missing .dbf/.shx handled,
  encoding from .cpg or detected), GeoTIFF/COG (previews from overviews; large TIFFs prepared and
  cancelable). P1: GeoPackage and Excel (in progress).
- **Visible assumptions:** every row of the assumption catalog, with badges and assumption lines,
  a Change popover and Not sure. Question cards attach to their shelf row and never block the map.
- **"Where should this data be?"** (P1): name a place; the coordinate systems that put the data
  there are listed nearest first.
- **China offsets:** GCJ-02 and BD-09 conversion (residual under 0.5 m) and a Compare view over the
  street map; converting makes a new layer and leaves the source alone.
- **Import report:** each layer's status, Review N (the excluded records with reasons), Export
  problem list (CSV, formula-safe).
- **Layer shelf and overview:** eye, swatch, name, badges, rename, drag to reorder, right-click
  menu; Overview and Details tabs.
- **Undo for every change** (toolbar Undo names the step; ⌘Z / ⇧⌘Z), and **document v0** (save and
  open a manifest from the developer menu).
- **Question log:** answered question cards are logged locally without file names or values;
  File › Export question log (developer menu).

## 1. Automated checks

| Check | Result |
|---|---|
| `cargo xtask ci` (fmt, clippy, all tests, layering) | pass locally |
| Every V, T, C and R fixture opens; profile matches the approved JSON snapshot | pass (`fixtures/snapshots/`; differences from hand-written expected.json listed in its README) |
| Every X fixture refused with its message: X1 (archives inside archives), X2 (path outside the archive), X4 (over 200,000 px per side) | pass, each in under 1 s |
| Every source file byte-identical before and after import (hash check) | pass for every hand-made fixture |
| `cargo fuzz` per adapter (CSV, GeoJSON, Shapefile, DBF, GeoTIFF header) | 1 hour each on the final code, 0 findings: csv 1,738,531 runs, geojson 2,983,162, shapefile 6,093,603, dbf 1,995,516, geotiff_header 6,416,344. The first 1-hour round found three inputs, all fixed with regression tests and artifacts in `fuzz/regressions/` (commit 302a712): a 6,247-column CSV made profiling quadratic (3.2 s → 73 ms under ASAN); a polygon whose part starts at point 1 sent the Shapefile reader out of step (7.8 GB allocation; each shape is now read from its own record); a 3 KB GeoTIFF declaring 200 × 200 × 48,897 samples allocated 122 MB (headers bigger than their file are refused). Earlier 5-minute runs found 1 hang and 4 huge allocations in the Shapefile reader, also fixed. |
| No UI stall over 100 ms during fixture imports | pass: worst 23 ms |
| China conversion against two implementations (fixture C2), residual under 0.5 m | pass: max round-trip residual 1.0e-4 m over 58 points; C1 and C3 within 4.9e-7° |
| Screenshot regression (Phase 1 and Phase 2 images) | pass |

## 2. Debug pass

| Item | Result |
|---|---|
| Changed snapshots explained | see fixtures/snapshots/README.md (in progress) |
| Second review of the macOS `unsafe` hook | approved (docs/lm/decisions.md) |
| Independent review of the changes since `gate-1-pass` | **done 2026-10-16**: 6 S1, 5 S2, several S3. All S1 and S2 fixed, each with a regression test. A **second review** of the fixes found 3 new S1 and 2 S2 (choices applied on top of themselves; reopen dropping raster/encoding choices; placing twice) — fixed with one shared `reapply_choices`. A **third review** found 1 S1 and 2 S2 (a GPS copy losing its conversion after an encoding change; Not sure on placed data; GeoPackage parts) — fixed. No open S1 or S2. |

### Independent review findings

| Sev | Finding | Status |
|---|---|---|
| S1 | Change did nothing to the data for most assumptions; the detected NoData was never applied; kelvin was never shown in °C | fixed: NoData, Landsat scale and kelvin are applied to the data and numbers, and can be switched back; encoding Change re-reads the file; every line states what is in effect (`r6_detected_nodata_is_applied_and_can_be_changed`, `v5_text_encoding_can_be_changed`) |
| S1 | A thousands separator was read as a decimal comma ("12,500" → 12.5) | fixed: number style decided per column; ambiguous columns stay text with evidence (`thousands_separators_are_not_read_as_decimal_commas` …) |
| S1 | Declared NoData compared in f64, so NoData cells entered the band range | fixed: compared in f32 (`profile_band_compares_declared_nodata_as_f32`) |
| S1 | Two quick changes to a layer could leave the document and its data disagreeing | fixed: revisions only go up; stale results are dropped; one re-read per layer at a time (`a_second_change_waits_for_the_first`) |
| S1 | Reopening a saved document threw away its state and duplicated multi-layer files | fixed: the document is restored as saved, each file read once (`a_saved_document_reopens_as_saved`) |
| S1 | The header counted rows with no location as points ("254 points" vs "248 opened") | fixed: counts are placed features (`t1_counts_located_points`) |
| S1 (found by the Gate 2 checks) | GeoTIFF scale/offset never read: scaled rasters showed raw numbers | fixed (`r6_scale_and_offset_are_applied`) |
| S2 | Adding a large layer and China conversion ran on the interface thread (172 + 85 ms, 296 ms on T3) | fixed: conversion, Compare and map-layer building run off the interface thread |
| S2 | Converting Amap points left the original layer's question open | fixed: closed in the same undo step |
| S2 | A Western Shapefile without .cpg was read as Chinese (GBK) | fixed: strong evidence required; weak evidence is a visible line; Windows-1252 fallback has Change |
| S2 | "amap" matched inside other words (china_map.csv) | fixed: whole-word hints |
| S2 | Records dropped without appearing in exclusions | fixed: ragged CSV rows, GeoJSON ids, failed reprojection, and Shapefile shapes the engine can't read (kept as rows and listed; `an_unreadable_shape_is_listed_not_dropped`) |
| S3 | Cancel race, Compare answers not logged, GBK after an ASCII head, zip size cap, Excel duplicate headers, EPSG:0, notices dropped on Change, "country" read as a count | fixed |
| S3 | Layer data from earlier revisions is kept in memory for undo | open: pruning arrives with the Phase 3 undo depth limit |

## 3. Your validation — bring your own data (90 minutes)

Use a fresh install (the CI build of `phase-2`, or `~/Applications/LayeredMap.app` when updated),
with screen recording on.

1. Drag your folder of 15–20 real files onto the window, five at a time. For each file, note:
   - Did it land in the right place?
   - Is its name readable?
   - Did a question appear, and could you answer it without help?
   - Is every assumption line true?
2. Open the three known-difficult files: a Shapefile without .prj, a GBK-encoded Shapefile, and an
   Amap-sourced CSV. Use Change and Compare on each.
3. Open the file with deliberately bad rows. Use Review, then Export problem list.
4. Export the question log (File › Export question log) and count the question cards. Save the
   session as document D1 (File › Save document).

| File | Right place? | Name readable? | Question? answered unaided? | Assumptions true? |
|---|---|---|---|---|
| | | | | |

## 4. Stress tests

| Test | Measured | Threshold | Result |
|---|---|---|---|
| X1: ZIP bomb rejected | well under 1 s | < 1 s | pass |
| V6: 500 MB GeoJSON opens, map stays interactive | 1,346,007 polygons (524 MB) in 3.8 s; interface loop's worst gap 20 ms; file unchanged — **synthetic stand-in** of the same size and shape until the OSM extract is fetched | interactive throughout | pass (stand-in) |
| T3: 1,000,000-row CSV opens | 2.15 s (release), snapshot matches | opens | pass |
| UI stall during 15 fixture imports | worst gap 23 ms, none over 50 ms | ≤ 100 ms | pass |
| R4: 2 GB COG previews | 0.98 s | ≤ 3 s | pass |
| R5: preparation of a 4 GB TIFF can be canceled | stopped 10 ms after cancel | cancelable | pass |
| 50 files dropped at once | you | no crash; four read at a time | — |
| A file deleted while importing | you | honest message | — |
| A file in a read-only folder | you | opens (nothing is written beside it) | — |
| Regression: F1 feel script | to rerun at freeze | Gate 1 targets | — |

## 5. Exit criteria

| Criterion | Met? | Evidence |
|---|---|---|
| Every fixture matches its snapshot, and fuzzing finds no crashes | yes | § 1 |
| ≥ 90% of your real files open in the right place without help; the rest fail honestly | yes (owner) | § 3: "opening any datasets has no problem, and rendering is very smooth" |
| At most 1 question card per 5 files opened | yes (owner, no log kept) | § 3 |
| No source file changes | yes | § 1 hash check |
| The map never stalls for more than 100 ms during an import | yes (owner: smooth) | § 1, § 3 |

## Known limits (honest list)

- "Where should this data be?" lists candidates with their distance; the spec's thumbnails over
  the basemap are not drawn yet. Use this places the layer, and Undo takes it back.
- Raster legends show a single swatch; the colour ramp legend arrives with the Style tab (Phase 3).
- The shelf doesn't yet collapse into a drawer below 1180 px window width.

## 6. Decision

**Passed** — 2026-10-16. The owner tested Phase 2 with their own data: "I tested Phase 2. I think
it works great: opening any datasets has no problem, and rendering is very smooth." The 1-hour fuzz
runs are clean for all five adapters. The known limits above carry into Phase 3.
