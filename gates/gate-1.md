# Gate 1 report — Map feel and the empty shell

Build: 0.2.0 · branch `phase-1` · 2026-10-15 (draft; not frozen)
Reference machine: Mac mini (Mac16,10), Apple M4, 16 GB, macOS 26.6.2, 5K display at 2×

**State:** the work packages are built; the gate needs your feel test (below) and the stress tests
marked "you". Install: `~/Applications/LayeredMap.app` (built on this Mac, so it opens without a
warning), or the `LayeredMap-macos-arm64` artifact from the CI run on `phase-1` (unsigned: right-
click › Open once).

## 1. Automated checks

| Check | Result |
|---|---|
| Lint and unit tests of the Phase 1 crates (`lm-core`, `lm-mapview`, `lm-ui`, `xtask`) | pass locally |
| Contrast of every colour token (body/secondary ≥ 4.5:1, icons/focus/status ≥ 3:1), light and dark | pass — `theme::tests::every_token_passes_its_contrast_check` |
| Screenshot regression: first launch light, dark, basemap chooser, search results | pass — `crates/lm-ui/tests/snapshots/` (macOS reference images) |
| Feel harness, `cargo xtask feel --runs 3` (F1 over the online basemap, vsync off) | **pass** — worst p95: pan 5.2 ms, zoom in 4.5, zoom out 4.5, pinch 4.6; 0 frames over 16.7 ms out of 20,223; 60 fps goal on **4 of 4** scripts. `docs/lm/feel/` |
| Cold launch to the first map frame, `cargo xtask launch` (10 launches) | **pass** — p50 149 ms, p95 198 ms (limit 2 s) |
| Place search: "Shanghai", "成都", "Lisbon" find their cities first | pass — `search::tests::gate_1_searches_find_their_cities`; ~0.7 ms per query over 34,163 cities |
| CI on GitHub (macOS arm64: fmt, clippy, all workspace tests incl. the screenshot tests; wasm32 build; app bundle) | **pass** — run 38720728077 on a67b96e |
| The feel harness in CI with deterministic counters against the base commit | **not built** — the harness records renders and frames per script; the CI comparison is open |

## 2. Debug pass

| Item | Result |
|---|---|
| Profile the worst camera script with puffin; fix the top 3 hotspots or explain | **not done** — every script is far inside the 60 fps goal; the visible cost is off the UI thread (the worker's CPU render). Profile if your feel test finds a problem. |
| Independent review of the changes since `gate-0-pass` | not yet run |

## 3. Your validation — the feel test (60 minutes)

1. Open `LayeredMap.app` three times with Wi-Fi on; time how long the map takes to appear.
2. Search "Shanghai", "成都" and "Lisbon" (⌘F, type, Return or click). Then zoom to your campus by hand.
3. Explore for 10 minutes with the trackpad, then with a mouse; then the same in Apple Maps. Rate
   1–5: pan, pinch, wheel zoom, double-click, tiles appearing, labels, overall calm.
4. Switch basemaps (the map button, bottom right): Auto (the default: light or dark with the
   appearance), Light, Dark, Offline; try "Background strength". Change System Settings to dark
   mode while the app is open: with Auto, the map turns dark too.
5. Turn Wi-Fi off and keep exploring: the notice "Detailed background unavailable offline…"
   appears and the map never goes blank. Turn Wi-Fi back on: detail returns without restarting
   (failed tiles are retried after up to 60 s).
6. Resize the window to its minimum, then full screen. Drag it between your display and an external one.

Other things to try: ⌥⌘F shows the frame rate; View › Metric/Imperial; View › Use System
Appearance / Light / Dark; File › Load fixture… (folder `fixtures/generated/F1`) puts 110,000
features on the map.

| Dimension | Rating (1–5) |
|---|---|
| Pan | |
| Pinch | |
| Wheel zoom | |
| Double-click | |
| Tiles appearing | |
| Labels | |
| Overall calm | |

## 4. Stress tests

| Test | Result | Threshold |
|---|---|---|
| Zoom spam: 50 wheel events/s for 10 s | you | lag after input stops < 200 ms |
| Resize storm: 30 s | you | no crash, no blank |
| Overnight idle: 8 h | you | memory grows ≤ 10% |
| Bad networks: throttled 3G; cold start with no network | offline simulated with `LM_BASEMAP_URL` (an unreachable host): notice shown, outline drawn, no blank. Real 3G/no-network: you | never blank |
| Large display: F1 via Load fixture… on a 4K display at 2× | you | Map feel targets |

## 5. Exit criteria

| Criterion | Met? | Evidence |
|---|---|---|
| Every hard limit in Map feel; 60 fps goal on ≥ 3 of 4 scripts | frame times: yes (4 of 4) | § 1 |
| Your ratings ≥ 4 on every dimension | pending | § 3 |
| Cold launch ≤ 2 s (p95); first online tiles within 1.5 s on campus | launch: yes (198 ms); tiles on campus: pending | § 1 |
| The map is never blank, in any network state | simulated offline: yes; real networks: pending | § 4 |
| Every colour token passes its contrast check | yes | § 1 |

## Questions for you (found while building)

1. **Basemap in dark mode.** ~~Should the default follow the appearance?~~ **Answered 2026-10-16:
   yes.** The new default "Auto" follows it; Light and Dark remain as fixed choices.
2. **Label density.** At zoom 11–13 over Shanghai the basemap names many villages (村/镇). Fewer,
   larger district and town names would be calmer. Tuning the built-in style's label filters is
   about a day.
3. **Label language.** ~~Local, English-first, or both?~~ **Answered 2026-10-16: English-first.**
   Places with no English name in OpenStreetMap (mostly villages) keep their local name.
4. **Soft data during long gestures.** With F1 loaded, a 4 s pinch re-renders the map 4–5 times:
   frames stay smooth (p95 4.6 ms), but the map is a scaled image for up to ~1 s at a time. If you
   notice it, strategy C (data on the GPU, ADR-001) is the fix.

## 6. Decision

Pending.
