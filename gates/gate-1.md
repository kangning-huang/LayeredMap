# Gate 1 report — Map feel and the empty shell

Build: 0.2.0 · branch `phase-1` · frozen 2026-10-16
Reference machine: Mac mini (Mac16,10), Apple M4, 16 GB, macOS 26.6.2, 5K display at 2×

**State:** passed 2026-10-16 (owner-approved; see § 6). Install: `~/Applications/LayeredMap.app` (built on this Mac, so it opens without a
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
| Independent review of the changes since `gate-0-pass` | **done 2026-10-16** (fresh agent, after the pass): no S1; one S2, several S3/S4. See below. |

### Independent review findings (fixed on `phase-2` unless marked open)

| Sev | Finding | Status |
|---|---|---|
| S2 | Zooming out at the world view slid the map up to 270° sideways (the zoom anchor moved while the resolution was clamped), leaving most of the window plain ground colour | fixed: zoom limited to the real minimum (`camera::min_zoom`); test `zooming_out_at_the_world_view_does_not_drift` |
| S3 | A purely sideways wheel notch (tilt wheel, ⇧-scroll) zoomed in (`0.0.signum()` is 1) | fixed |
| S3 | Zoom targets built up past the limits, so reverse notches were ignored | fixed: targets clamped |
| S3 | Every map edit copied the whole map (all features) on the UI thread for the render thread | fixed: the same edit is applied to the render thread's copy |
| S3 | A dead render thread, or a render returning nothing, left the map waiting forever | fixed: "nothing" replies; a stopped thread is restarted (up to 3 times) |
| S3 | Any tile error shows the offline notice; the basemap URL is a dated download build | open: needs the WP0.6 host |
| S3 | lm-ui reaches engine types through lm-mapview/lm-jobs re-exports, which `xtask layers` can't see; `layeredmap → lm-core` added without a log entry | open: Phase 2 import moves layer work into lm-jobs; log entry added |
| S3 | "Cold launch" runs back to back (warm) and stops at the first frame | open: relabel or wait for tiles |
| S4 | No thousands separators in the scale bar and the fixture notice | fixed |
| S4 | Reduce Motion fade resets to 150 ms before the jump's frame arrives; Esc clears the search text anywhere; the spare-texture comment is wrong; Load fixture can't pick a single file | open (cosmetic); the dialog title now says folder |

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
| Pan | 5 |
| Pinch | 5 |
| Wheel zoom | 5 |
| Double-click | 5 |
| Tiles appearing | 5 |
| Labels | 5 |
| Overall calm | 5 |

## 4. Stress tests

| Test | Result | Threshold |
|---|---|---|
| Zoom spam: 50 wheel events/s for 10 s | owner: done, no problems reported | lag after input stops < 200 ms |
| Resize storm: 30 s | owner: done, no problems reported | no crash, no blank |
| Overnight idle: 8 h | owner: done, no problems reported | memory grows ≤ 10% |
| Bad networks: throttled 3G; cold start with no network | offline simulated with `LM_BASEMAP_URL` (an unreachable host): notice shown, outline drawn, no blank. Real networks: owner tested, no problems reported | never blank |
| Large display: F1 via Load fixture… on a 4K display at 2× | owner: done, no problems reported | Map feel targets |

## 5. Exit criteria

| Criterion | Met? | Evidence |
|---|---|---|
| Every hard limit in Map feel; 60 fps goal on ≥ 3 of 4 scripts | frame times: yes (4 of 4) | § 1 |
| Your ratings ≥ 4 on every dimension | yes (5 on every dimension) | § 3 |
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

**Pass — owner-approved, 2026-10-16.** The owner: "I have done enough stress tests on the app, and I
think it's incredible, very smooth and very responsive … we can move on to the next phase."
Merged to `main`, tagged `gate-1-pass`.

Found and fixed during validation: label flashing after a search jump (labels now fade one by
one); menu choices waited for a mouse move. Owner answers: the default basemap follows the
appearance ("Auto"); labels are English-first.

Carried open into Phase 2 (none blocks the pass by the owner's decision):
- the independent review of the changes since `gate-0-pass` (started at the pass, findings fixed
  on `phase-2`);
- the CI comparison of the feel harness's deterministic counters against the base commit;
- puffin profiling of the worst camera script (every script is far inside budget);
- first online tiles within 1.5 s on campus (WP0.6 host not chosen yet);
- open Gate 1 questions 2 (label density) and 4 (soft data during long pinches).
