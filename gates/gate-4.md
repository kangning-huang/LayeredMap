# Gate 4 report — Contextual actions, previews, result cards

Build: 0.4.0-dev · branch `phase-4` (from `phase-3`; Gate 3 still awaits your validation) · frozen 2026-10-19
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
| WP4.5 Certified actions | done in the engine: count, which area, what's inside, buffer, measure, summarize, sample (rank is Phase 3's insight ranking); summaries read GeoTIFFs window by window (`zonal::TiffSource`); wiring them to jobs and result cards is next | `lm-analysis::{overlay, buffer, zonal}` |
| WP4.6 Result cards and recipes | recipe model, text, fingerprints, versions done; the card UI to do | `lm-core::recipe`, `lm-jobs::fingerprint` |
| WP4.7 Action search | to do (P1) | |

## 1. Automated checks (so far)

| Check | Result |
|---|---|
| N1, N2 counts and which-area against GeoPandas | exact; plus T1 schools in V1 districts, all 17 rows |
| N3 summary against the hand calculation | mean 3.3e-12, coverage 2.4e-11 relative |
| N8 category shares | 3.7e-10 absolute |
| R3 × V1 weighted means against exactextract 0.3.0 | 2.3e-9 relative |
| N4: R7 (generated, 10°S–70°N) × 8 areas against exactextract 0.3.0 | worst 1.2e-7 relative (limit 1e-6) over mean, coverage, covered area, polygon area, min, max; for TRI and DIAG against the exact shapely check, because exactextract drops two fully covered cells of each (see Decisions) |
| N4 against the high-resolution geodesic integration | worst 3.0e-5 relative (limit 1e-3; the tiny SMALL triangle's covered area), means within 4.8e-8 |
| N6: totals from R2 counts (Σ n·f) and R2 density (Σ d·a), V1's 17 districts + 6 cut-cell areas, against exactextract | worst 2.0e-9 relative (limit 1e-6); S5 against the shapely check (exactextract adds a cell outside it); the two totals agree to 1.5e-8 |
| GeoTIFF window reader (`zonal::TiffSource`) on 10 GDAL-written layouts (`fixtures/N7/layouts/`: strips and tiles; chunky and planar bands; uint8, int16, uint16, float32, float64; none, LZW, Deflate, PackBits, Zstandard; BigTIFF; a COG with overviews; EPSG:4326 and EPSG:32651; NaN NoData) | every cell of 64 windows per file and band exact, NoData recognised; only the tiles a window touches are decoded; truncated or corrupted files give errors, never a panic; a summary from the file equals one from memory |
| R6 point sampling against rasterio | exact |
| N5 buffers against pyproj (18 cases: equator, 60° N, antimeridian; 1 and 50 km) | worst −6.8e-5 relative area (limits 1e-3 points, 5e-3 lines and areas); antimeridian cases split at ±180° |
| N5 measurements | within tolerance (wrappers over the Gate 3 geodesic code, 1e-9) |
| Cancellation | worst of 5: 7.3 ms (limit 200 ms); 100 random cancels leave no temporary files and deliver nothing |
| Race | 1,000 rapid submissions: every preview matched its own parameters; the last equals the final parameters |
| Compound undo (job commit) | one undo step; undo restores, redo gives the identical state |
| Recipe text | matches the plan's example exactly |

## 4. Stress tests (so far, release build)

N7 runs with `cargo xtask fixtures --only R4,N7` then
`cargo test --release -p lm-analysis zonal::tests::n7_r4_cog_stress -- --ignored --nocapture`
(also part of `cargo xtask stress`). Measured 17 October 2026 on the reference machine.


| Test | Measured | Threshold |
|---|---|---|
| Count 1,000,000 points in 100,000 polygons (synthetic) | 0.77 s | 60 s |
| **N7:** summarize R4 (the generated 2.9 GB tiled COG, 23,040² float32 cells in 512 px tiles, 6 overviews) across N7's 3,000 areas with `TiffSource` | **12.0 s** reading from the LaCie drive (2.4 s with the file already in the page cache); each of the 2,025 tiles decoded once; progress reported for every area | 5 min |
| N7: cancel during that summary (at 1,000 and at 2,000 areas) | **0.4 ms** worst of 2 | 1 s |
| N7: spot check, 31 sampled areas (holes, multipolygons, 11 crossing the edge) against exactextract | **2.3e-8** worst relative error (all 31 agree with the shapely check too) | 1e-6 |
| Summarize an R4-sized synthetic raster across 3,000 polygons (earlier check, kept) | 1.6 s in memory; 15.6 s from a 2.1 GB raw tiled file | 5 min |
| Cancel during that summary | 18 µs | 1 s |
| Buffer 10,000 clustered points by 1 km, merged | 1.26 s (one polygon, 12 holes, 1,294.5 km²) | 30 s |

## Decisions (owner, 2026-10-19)

- **Coverage formula:** Google Earth Engine's weighted-reducer rule,
  c = Σ_valid a / (Σ_all a + A_out), with the part of the area beyond the raster counting as no
  data (docs/lm/decisions.md, 2026-10-19; `coverage_follows_earth_engine_weighting`). Replaces the
  max(A_P, Σ a) cap.
- **exactextract defects:** signed off. The N4, N6 and N7 references use exact shapely fractions
  where exactextract 0.3.0 is off by more than 1e-7, naming the source (3 of 62 areas).

## Known limits

- The window reader reads the full-resolution image only (summaries never use overviews) and
  doesn't read GDAL's scale and offset metadata (the caller sets them, as for in-memory bands).
  It isn't wired to the app's open-file path yet.
- The app's whole-file GeoTIFF reader (`rustgis_data::raster::read_path`, on `tiff` 0.11.3)
  fails on a GDAL LZW tiled file and panics on a planar 3-band tiled file (both in
  `fixtures/N7/layouts/`); the window reader decodes those compressions itself. To fix or report
  upstream.
- N7's first-run time includes reading from the external drive; a truly cold cache wasn't
  forced (that needs `sudo purge`).
- R2 is a synthetic stand-in until the open population grid's licence is verified.
- Buffers of features thousands of km across lose accuracy (one projection per feature); a hole
  winding around a pole isn't handled.
- Overlay is planar in lon/lat like GeoPandas: polygons crossing the antimeridian aren't handled.
- The cost model's constants for features, indexing and union are estimates until measured on
  the real actions.

## Decision

**Passed** — 2026-10-19. The owner reviewed this report and closed the gate ("gate 4 looks good.
close it."), with two decisions: summary coverage follows Google Earth Engine's weighted-reducer
rule, and the exactextract cross-check is signed off (see Decisions above). The engine has shipped
in the Pro test builds since 0.4.0 (the composers, previews, result cards and recipes behind the
Ask box). The known limits above carry forward. Gate 3 is still open: Phase 4 was built on Phase
3, and Gate 3's own validation session is next.
