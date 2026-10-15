# Gate 0 — your validation script (45 minutes)

Use the spike build (CI artifact `lm-render-spike-macos-arm64`, or locally
`cargo run --release -p lm-render-spike`), your Mac's trackpad, and a mouse for the wheel test.
Start a screen recording first, think aloud, and note every issue with the issue template
(`.github/ISSUE_TEMPLATE/lm-gate-issue.md`). Rate each item 1–5.

The window opens on Shanghai at about zoom 11 with fixture F1 (100,000 blue points, 10,000 orange
polygons) over the online Protomaps basemap. The overlay (top left) shows the strategy, frame rate
and 95th-percentile frame time.

| Key | Does |
|---|---|
| 1 / 2 / 3 / 4 | Strategy A / B / B2 / C (C is the default) |
| H | Back to Shanghai |
| P | The probe panel (Pinyin field, VoiceOver buttons and list, font samples) |
| Arrows, + / − | Pan a quarter view, zoom one level |
| Double-click / ⌥ double-click | Zoom in / out one level |

## 1. Feel (25 minutes)

1. With strategy **C**, pan and zoom around Shanghai for 5 minutes: drag, two-finger scroll
   (pans), pinch (zooms), double-click. Then plug in a mouse and use the wheel.
2. Do the same in Apple Maps for 5 minutes.
3. Rate the spike 1–5 on: **pan**, **pinch zoom**, **wheel zoom**, **tiles appearing**, **labels**.
4. Switch to **B2** (key 3) and **B** (key 2) for a minute each. Note any difference you feel. B is
   RustGIS's current behaviour: watch for the pause when a gesture ends.

| Dimension | C | B2 | B | Apple Maps (reference) |
|---|---|---|---|---|
| Pan | | | | |
| Pinch zoom | | | | |
| Wheel zoom | | | | |
| Tiles appearing | | | | |
| Labels | | | | |

## 2. Chinese input (5 minutes)

Press P. In the field, with the macOS Pinyin input source:
1. Type 上海市徐汇区. Does the candidate window sit near the caret? Is the committed text exactly
   上海市徐汇区 (shown under the field)? Does backspace delete one character at a time?
2. Switch to English and type "Xuhui", mixed after the Chinese.

## 3. VoiceOver (5 minutes)

Turn VoiceOver on (⌘F5). With VO keys, move through the five buttons, the text field and the
district list in the probe panel. Record what VoiceOver reads for each, and whether it can reach
all of them. Turn VoiceOver off (⌘F5).

| Element | Reached? | What VoiceOver said |
|---|---|---|
| Open file | | |
| Layers | | |
| Undo | | |
| Share | | |
| Zoom to all data | | |
| Text field | | |
| District list (5 rows) | | |

## 4. Stress (10 minutes)

1. **F1 on a 4K external display at 2×.** Drag the window to the external display, maximise it,
   repeat a minute of panning and zooming, and note the overlay's p95.
2. **Throttled network.** In Apple's Network Link Conditioner (Xcode › Additional Tools), choose
   the 3G profile. Restart the spike, pan to an area you haven't visited (e.g. Hangzhou) and zoom.
   Is any part of the map ever blank (not even a coarser tile)?
3. F2 and R4: see `docs/lm/adr-001-rendering.md` › Stress results (measured by the agent).

## 5. Decide

Read `docs/lm/adr-001-rendering.md` and approve a stack choice (or ask for changes).
