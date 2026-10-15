# Gate 0 report — Foundation and risk spikes

Build: 0.2.0 · branch `phase-0` · 2026-10-15 (draft; not frozen)
Reference machine: Mac mini (Mac16,10), Apple M4, 16 GB, macOS 26.6.2, 5K display at 2×

**State:** the agent's part is done except where marked. The gate cannot pass yet: it needs your
validation session, the Apple Developer enrollment (signed builds), and basemap latency from
campus.

## 1. Automated checks

| Check | Result |
|---|---|
| `cargo xtask ci` (fmt, clippy -D warnings, workspace tests, layering incl. `lm-*`) | **pass locally** on the reference Mac, 2026-10-15 (after the `docs/lm` test fix) |
| Layering check fails on a forbidden dependency (tests the test) | **pass** — `xtask/src/lm.rs` tests `forbidden_dependencies_fail`, `a_forbidden_dependency_in_a_real_manifest_fails` |
| Fixture factory produces every generated fixture | **pass** — F1, F2, V7, V8, T3, R4, R5, R7, X1–X5 in 52 s; deterministic (test `generators_are_deterministic`, GeoTIFFs read back by an independent decoder) |
| Hand-made fixtures and reference outputs | **pass** — 21 fixtures; `reference/run_all.py --check` regenerates 91 files byte-identically |
| CI on GitHub (macOS arm64) | **not run yet** — `.github/workflows/lm-ci.yml` runs on the first push |

## 2. Debug pass

| Item | Result |
|---|---|
| Spike measurements repeat within 10% across five runs | **fail as written** — see ADR-001 › Repeatability. Your decision. |
| Independent review of the changes since the baseline | not yet run |

## 3. Owner validation

Script: `gates/gate-0-validation.md` (45 minutes). In progress.

- 2026-10-15, first session: owner reported the spike "feels very smooth". The pan direction was
  reversed (drag, trackpad, inertia, arrow keys); fixed in `c84e239`. After the fix, the owner
  compared B2 and C and found them **comparable in performance**.
- Same day, the owner reported: "all the tests passed" (the validation script). The individual
  1–5 ratings and notes (Pinyin, VoiceOver, 4K display, throttled network) were not itemized; add
  them here for the record.

## 4. Stress tests

| Fixture | Measured | Threshold | Result |
|---|---|---|---|
| F1 at 2× (5K reference display) | p95 4–9 ms (B2, C) | ≤ 33 ms | pass |
| F1 on a 4K external display at 2× | — | ≤ 33 ms | your session |
| F2, 1,000,000 points | p95 ≤ 7.6 ms (B2), ≤ 18.7 ms (C) | ≤ 33 ms | pass |
| R4, 2 GB COG preview | ≈ 1.2 s | (≤ 3 s at Gate 2) | pass |
| Tiles on throttled 3G | — | no blank areas | your session |

## 5. Exit criteria

| Criterion | Met? | Evidence |
|---|---|---|
| A strategy keeps p95 ≤ 33 ms on F1 at 2×, with no blank areas on a throttled network | frame time: yes (B, B2, C); throttled: pending | ADR-001 |
| Your pan and zoom ratings ≥ 4 with that strategy | pending (B2 and C felt comparable; ratings not yet recorded) | your session |
| Pinyin input works (candidate window near the caret, committed text correct) | pending | probe panel (P) |
| A basemap host reaches p95 tile latency ≤ 500 ms from campus | pending | `tools/lm/tile_latency.py`, `docs/lm/basemap-hosting.md` |
| VoiceOver results recorded | pending | probe panel (P) |
| ADR-001 estimates the remaining effort per stack; cheapest meeting 33 ms wins | yes | ADR-001: egui + B2 |
| Signed, notarized CI builds open on a fresh Mac with no warning | pending | needs Apple Developer enrollment, `docs/lm/signing.md` |

## Not done in Phase 0 (carried as open items)

- WP0.1: the upstream discussion is drafted (`docs/lm/upstream-discussion-draft.md`), not posted —
  it is public and in your name.
- WP0.4: Finder "Open With" and Dock-icon drops are not spiked. winit owns the app delegate, so
  this needs the app's macOS module (the one `unsafe` exception). Moved to Phase 2 WP2.1, where
  the import coordinator needs it.
- WP0.6: the Shanghai extract and the two hosts (needs an account and you in Shanghai).
- WP0.8: hosting the large fixtures in a pinned bucket with checksums.

## 6. Decision

Pending.
