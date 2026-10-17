# Gate 4 report — Contextual actions, previews, result cards

Build: 0.4.0-dev · branch `phase-4` (from `phase-3`; Gate 3 still awaits your validation) · draft
Reference machine: Mac mini (Mac16,10), Apple M4, 16 GB, macOS 26.6.2

**Scope after the tier decision (2026-10-16):** Phase 4 builds the engine behind Pro's assistant
— the job system, certified actions, previews, result cards and recipes, and drawing an area.
None of it has a button in the free app; it is exercised through a developer surface and tests.

## Work packages

| WP | State | Where |
|---|---|---|
| WP4.1 Action registry | done: every analysis action states its output, certified algorithm and recipe method; previews by cost; `describe()` for the assistant | `lm-actions` |
| WP4.2 Composer | developer surface only (per tiers) — to do | |
| WP4.3 Job system | done: worker pool, cancel tokens, progress, generation slots, atomic commit, cost estimator | `lm-jobs::jobs`, `lm-analysis::control` |
| WP4.4 Drawing an area | done: polygon and rectangle, banner, Esc, "Area N" in WGS 84 | `lm-ui::draw` |
| WP4.5 Certified actions | done in the engine: count, which area, what's inside, buffer, measure, summarize, sample (rank is Phase 3's insight ranking); wiring them to jobs and result cards is next | `lm-analysis::{overlay, buffer, zonal}` |
| WP4.6 Result cards and recipes | recipe model, text, fingerprints, versions done; the card UI to do | `lm-core::recipe`, `lm-jobs::fingerprint` |
| WP4.7 Action search | to do (P1) | |

## 1. Automated checks (so far)

| Check | Result |
|---|---|
| N1, N2 counts and which-area against GeoPandas | exact; plus T1 schools in V1 districts, all 17 rows |
| N3 summary against the hand calculation | mean 3.3e-12, coverage 2.4e-11 relative |
| N8 category shares | 3.7e-10 absolute |
| R3 × V1 weighted means against exactextract 0.3.0 | 2.3e-9 relative |
| R6 point sampling against rasterio | exact |
| N5 buffers against pyproj (18 cases: equator, 60° N, antimeridian; 1 and 50 km) | worst −6.8e-5 relative area (limits 1e-3 points, 5e-3 lines and areas); antimeridian cases split at ±180° |
| N5 measurements | within tolerance (wrappers over the Gate 3 geodesic code, 1e-9) |
| Cancellation | worst of 5: 7.3 ms (limit 200 ms); 100 random cancels leave no temporary files and deliver nothing |
| Race | 1,000 rapid submissions: every preview matched its own parameters; the last equals the final parameters |
| Compound undo (job commit) | one undo step; undo restores, redo gives the identical state |
| Recipe text | matches the plan's example exactly |

## 4. Stress tests (so far, release build)

| Test | Measured | Threshold |
|---|---|---|
| Count 1,000,000 points in 100,000 polygons (synthetic) | 0.77 s | 60 s |
| Summarize an R4-sized raster (23,040² cells) across 3,000 polygons | 1.6 s in memory; 15.6 s from a 2.1 GB file in 512 px tiles | 5 min |
| Cancel during that summary | 18 µs | 1 s |
| Buffer 10,000 clustered points by 1 km, merged | 1.26 s (one polygon, 12 holes, 1,294.5 km²) | 30 s |

## Open questions for you

- **Coverage formula** (docs/lm/decisions.md, 2026-10-17): coverage is capped at 1 because the
  plan's formula and its own rule can't both hold. Sign off or choose otherwise.

## Known limits

- No tiled reader for the real R4 COG yet (the summary streams from any tile source; the app's
  raster type is in memory). N4, N6 and N7 fixtures don't exist yet.
- Buffers of features thousands of km across lose accuracy (one projection per feature); a hole
  winding around a pole isn't handled.
- Overlay is planar in lon/lat like GeoPandas: polygons crossing the antimeridian aren't handled.
- The cost model's constants for features, indexing and union are estimates until measured on
  the real actions.
