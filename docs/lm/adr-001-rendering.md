# ADR-001: Rendering stack

**Status:** proposed, 2026-10-15. Awaiting your feel test and approval at Gate 0.

**Recommendation:** keep egui and RustGIS's CPU renderer, render off the UI thread into a cached
texture that moves and scales during gestures (strategy **B2**). It is the cheapest stack that
meets the 33 ms hard limit and every other measured Map feel target. Keep strategy **C** (data
layers on the GPU) as a measured upgrade: switch to it if your feel test rates B2's data
sharpness during gestures below 4.

## Measured result

On F1 at 2×, B, B2 and C all keep the 95th-percentile frame time between 4 and 9 ms, well under
both the 16.7 ms goal and the 33 ms limit. A (redraw every frame) fails by up to 20×. B — RustGIS's
own map view — freezes the UI thread for up to 790 ms when a gesture ends; B2 and C never block it
for more than 4.2 ms.

| Strategy | Script | p95 frame, mean of 5 runs (range), ms | Worst p99, ms | Frames > 33 ms (of all) | Longest UI-thread block, ms |
|---|---|---|---|---|---|
| A · redraw every frame | pan 2,000 px | 9.7 (8.3–13.0) | 171 | 25 / 1,192 | 361 |
| | zoom in 6 levels | 130 (98–150) | 300 | 90 / 873 | 291 |
| | zoom out 6 levels | 535 (475–678) | 751 | 101 / 101 | 749 |
| | pinch | 454 (363–749) | 1,223 | 40 / 386 | 1,221 |
| B · RustGIS today | pan 2,000 px | 5.4 (4.3–6.8) | 8.5 | 0 / 4,158 | 16 |
| | zoom in 6 levels | 5.2 (4.9–5.9) | 7.9 | 5 / 5,381 | 104 |
| | zoom out 6 levels | 5.6 (4.0–7.0) | 8.7 | 5 / 5,189 | **792** |
| | pinch | 6.5 (5.6–7.4) | 9.1 | 5 / 5,806 | **783** |
| **B2 · B, rendered on a worker** | pan 2,000 px | 5.5 (3.9–7.5) | 9.3 | 0 / 4,838 | 3.0 |
| | zoom in 6 levels | 5.2 (3.6–8.1) | 10.2 | 0 / 7,375 | 2.5 |
| | zoom out 6 levels | 5.3 (3.8–8.7) | 10.2 | 0 / 6,740 | 4.2 |
| | pinch | 5.1 (3.7–8.7) | 10.0 | 1 / 9,046 | 2.8 |
| C · B2 basemap + GPU data | pan 2,000 px | 5.7 (4.6–7.4) | 8.5 | 0 / 4,058 | 3.0 |
| | zoom in 6 levels | 5.9 (4.7–7.3) | 8.8 | 1 / 5,462 | 3.9 |
| | zoom out 6 levels | 5.3 (4.4–7.2) | 9.1 | 0 / 6,233 | 3.1 |
| | pinch | 6.6 (5.5–7.6) | 9.8 | 2 / 6,534 | 3.0 |

Frame time is the wall-clock interval between frames with vsync off, so it is the real cost, not
the display's cadence. Each run includes 0.6 s after the gesture ends, which is where B's redraw
lands. Raw data: `docs/lm/spike/summary.csv`, `docs/lm/spike/frames.csv.gz`.

**How long the map stays soft.** During a gesture B, B2 and C show a scaled copy of the last
render. How soon it sharpens is the render time:

| Strategy | Render time p95 (max), ms | What is soft during a zoom |
|---|---|---|
| B | 781 (785) | basemap and data, until the gesture ends |
| B2 | 386 (1,351) | basemap and data, for up to ~0.4 s |
| C | 172 (327) | basemap only; data is redrawn exactly every frame |

## Stress results

| Test | Result | Threshold |
|---|---|---|
| F2: 1,000,000 points, B2 | p95 4.1–7.6 ms, nothing over 33 ms | 33 ms |
| F2: 1,000,000 points, C | p95 11.1–18.7 ms; 56 frames over 16.7 ms, none over 33 ms | 33 ms |
| R4: 2 GB COG preview | read 0.79 s (from a 5,760² overview) + first render 391 ms ≈ 1.2 s | ≤ 3 s at Gate 2 |
| R5: 4 GB striped TIFF, no overviews | read 20.1 s, blocking | the cancellable preparation step in Phase 2 (WP2.2) |
| F1 on a 4K external display at 2× | the reference display is a 5K panel at 2×, a larger load than 4K; your test on an external 4K is still listed | 33 ms |
| Throttled 3G | **not measured** — your test with the Network Link Conditioner | no blank areas |

## Gate 0 debug criterion: repeatability — **fails as written**

"Every spike measurement repeats within 10% across five runs" is not met. The five p95 values per
row vary by 20–99% of their mean (e.g. B2 pinch 3.7–8.7 ms). The absolute spread is 1–5 ms on
values far below the limit: at 4–9 ms frame times, macOS scheduling and GPU clocking dominate.
A's rows vary 38–85%, at hundreds of milliseconds. The pass/fail outcome against 16.7 ms and 33 ms
is the same in every run. Background agents were also working on the machine during the runs (reading and writing files,
possibly a headless browser), which adds noise.

This is your call; the plan doesn't let me change the criterion. Options: (a) rerun on an idle
machine and see whether it tightens; (b) judge repeatability on the outcome (every run on the same
side of each threshold); (c) judge it on deterministic counters (draw calls, renders per script),
which is what WP1.6 does for CI anyway.

## Effort to meet every Map feel target, per stack

Estimates are in effort-weeks (about 20 focused hours each), for Phase 1's map work only.

| Stack | Measured | Still to build | Estimate |
|---|---|---|---|
| **egui + B2** (recommended) | All frame and stall targets met | Extract `lm-mapview` (~450 lines of core, 3–4 days); Chinese labels in the renderer (font + label expression, 2–3 days, an `upstream:` change); label fades and the 150 ms placement throttle (2 days); parent-tile placeholders with fades (2 days); gestures from the spike, tuned (1 day) | ~2.5 |
| egui + C | All met; data always sharp | All of B2, plus the GPU data path in production: styles by value and category, selection highlight, rasters stay CPU (~1 week) | ~3.5 |
| Tauri + MapLibre (GeoLibre approach) | Not measured | A web UI for every panel; data bridged to MapLibre (1,000,000 points needs vector tiling); RustGIS's renderer unused; IME, labels and VoiceOver come mostly from the browser | ~5, plus the shell work every later phase redoes in HTML |

Not settled by the spike, and not stack-specific: Pinyin input and VoiceOver (your probes, `gates/gate-0-validation.md`),
and basemap latency from Shanghai (WP0.6).

## Found on the way

- **Chinese basemap labels are dropped.** RustGIS's built-in Protomaps style prefers `name:en`,
  and its renderer keeps only label text its Latin fonts cover, so Chinese-only names vanish
  (`docs/lm/capabilities.md`, gap 1). In the spike you will see English labels or none in
  Shanghai. Fixing it is in the B2 estimate above.
- The spike's online basemap is a pinned Protomaps daily planet build, not the WP0.6 host.

## How the spike works

`spikes/render-spike` (throwaway). One window, 1,280 × 800 points (2,560 × 1,600 px at 2×), F1
over the Protomaps basemap, camera at Shanghai about zoom 11. Four scripted camera paths, each
time-based so a slow strategy cannot stretch the path: a 2,000 px pan over 2 s; zoom in 6 levels
over 3 s; zoom out 6 levels over 3 s; a 4 s pinch oscillating ±1.5 levels while drifting. A warm-up
pass fetches the tiles first. `cargo run --release -p lm-render-spike -- --bench` reproduces the
table; keys 1–4 switch strategies interactively.

**Reference machine:** Mac mini (Mac16,10), Apple M4, 16 GB, macOS 26.6.2, a 5,120 × 2,880 display
at 2×.
