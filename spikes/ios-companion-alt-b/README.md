# iPhone companion spike — Alternative B (persistent sheet)

Throwaway spike for **Alternative B**: map-first home with one **persistent bottom sheet** (not floating Open/Layers/Ask pills). Design: `docs/lm/ios-redesign.md` (on `phase-4` after #54).

**Not product code.** Lives under `spikes/` and will be thrown away or rewritten when the real app lands.

## Alt A vs Alt B (try both)

| | **Alt A** | **Alt B** (this spike) |
| --- | --- | --- |
| Path | `spikes/ios-companion/` | `spikes/ios-companion-alt-b/` |
| Xcode project | `…/ios/NimbleMapCompanion.xcodeproj` | `…/ios/NimbleMapAltB.xcodeproj` |
| Bundle id | `com.nimblemap.companion.spike` | `com.nimblemap.companion.altb` |
| Home screen name | NimbleMap | **NimbleMap B** |
| Chrome | Floating Open / Layers / Ask pills; Layers modal → Style | Full map + persistent sheet (small/medium/large); never dismisses |
| Row tap | Symbology form | **Data card** with insight stubs |
| Map tap | (none in A chrome) | **Insight card** stub |
| First run | Empty / open URL | Example data on map + hint “Tap any area to learn” |
| Design doc | `docs/lm/ios-layers-symbology.md` | `docs/lm/ios-redesign.md` |

Install both on one phone/simulator — different bundle ids. Compare by trying; no pick until after that.

## Locked choices (shared stack)

| Piece | Choice |
| --- | --- |
| Shell | SwiftUI (iOS 17+) |
| Map | MapLibre Native iOS (SPM), Metal |
| Decode | `rustgis-data` behind `lm_decode_*` C ABI (`lm-ios-alt-b-decode-ffi`) |
| Sheet | `.presentationDetents([.height(136), .medium, .large])`, `.presentationBackgroundInteraction(.enabled(upThrough: .medium))`, `.interactiveDismissDisabled()` |
| Layers habits | Live legend swatches; swipe Remove; Edit/`onMove` reorder; basemap never a data row |
| Out of this PR | Share-a-page, free catalog search, paid search, full Change look, Ask Pro, MapLibre paint from `LayerStore` |

## Layout

```
spikes/ios-companion-alt-b/
  Cargo.toml                 # lm-ios-alt-b-decode-ffi
  include/lm_decode.h
  src/{lib,ffi}.rs
  scripts/{check-rust-host,build-rust-ios}.sh
  ios/NimbleMapAltB.xcodeproj
  ios/NimbleMapAltB/*.swift
```

## Rust (works without Xcode)

From the repo root:

```bash
cargo test -p lm-ios-alt-b-decode-ffi --locked
# or
./spikes/ios-companion-alt-b/scripts/check-rust-host.sh
```

### iOS Rust targets

```bash
./spikes/ios-companion-alt-b/scripts/build-rust-ios.sh aarch64-apple-ios-sim
./spikes/ios-companion-alt-b/scripts/build-rust-ios.sh aarch64-apple-ios
```

Needs **full Xcode** (iPhoneSimulator / iPhoneOS SDKs).

## Xcode / simulator

1. Install Xcode; run once; accept the license.
2. `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`
3. Open **`spikes/ios-companion-alt-b/ios/NimbleMapAltB.xcodeproj`**
4. Set your **Team** for device; simulator can use automatic signing.
5. Wait for SPM to resolve **MapLibre**.
6. Pick an iPhone simulator (or device), Build & Run.
7. Home screen label: **NimbleMap B**.

### Try Alt A beside it

Open `spikes/ios-companion/ios/NimbleMapCompanion.xcodeproj` (Alt A branch/PR) and run that too. Bundle id differs so both stay installed.

## What this spike exercises

- Persistent sheet at three heights; map stays interactive through medium
- Medium list: swatch, name, eye; swipe Remove; Edit reorder
- Row tap → lean data card (insight stubs; Change look disabled stub)
- Map tap → feature insight card stub
- First run: Cities/Roads/Parks demo rows + world-cities GeoJSON overlay + hint
- Top-trailing map style / my location stubs (no floating Open/Layers/Ask)
- “+” → Add data sheet; Paste link still opens HTTPS GeoJSON via Rust decode

## CI

Desktop CI compiles this crate on host targets via `spikes/*`. It does **not** build the Xcode app.

## Compare build (2026-11-01)

Alt A and Alt B now share one core, so a hands-on compare is about the chrome, not about which
spike got more plumbing. These files are **identical** in both apps (edit one, copy to the other):
`GeoData.swift`, `Insights.swift`, `Styling.swift`, `MapDocument.swift`, `Importer.swift`,
`Library.swift`, `DecodeBridge.swift`, `MapScreen.swift`.

- **Data:** the same four bundled examples (`assets/examples/`: Countries, Cities, Earthquakes,
  Volcanoes; referenced from the repo, not copied). Files from Files / Open In / links decode
  through `lm_open_path` → `rustgis-data::io::read_path` (GeoJSON, CSV with coordinates, KML,
  KMZ, GPX, zipped shapefile, GeoPackage), copied into the app first.
- **Map:** OpenFreeMap basemap (light), data from MapLibre file sources (parsed off the main
  thread), "Show by" a field (viridis / Heat / Blues; Okabe–Ito categories), fly to new data,
  logo and attribution moved to the top so bottom chrome never covers them.
- **Project files:** after adding or removing a Swift file run
  `python3 scripts/sync-xcodeproj.py ios/<App>.xcodeproj`. The Rust build phase now always runs
  cargo (incremental) so a changed C ABI is never linked from a stale library.

Headless helpers (spike-only), as environment variables at launch:

| Variable | Effect |
| --- | --- |
| `LM_SCENE=<name>` | Opens a state for screenshots (see `runSceneIfAsked` in `ContentView.swift`) |
| `LM_BENCH=1` | 4 s after the style loads, a scripted 15 s camera tour; prints one `LM_BENCH …` line with map frame times |
| `LM_FPS=1` | Shows the CADisplayLink FPS badge |

```bash
xcrun simctl launch --console-pty booted <bundle id>   # with SIMCTL_CHILD_LM_BENCH=1 in the env
```

### On your iPhone

1. `open ios/NimbleMapAltB.xcodeproj` (full Xcode, license accepted; `rustup` installed — the build
   phase adds the `aarch64-apple-ios` target and builds the Rust library itself).
2. Target NimbleMapAltB › Signing & Capabilities: pick your Team (the project ships with none). If the
   bundle id is taken, change its suffix.
3. Plug in the iPhone, turn on Settings › Privacy & Security › Developer Mode, pick it as the run
   destination, Run. The first build fetches MapLibre (SPM) and compiles the Rust crate.
4. It installs as "NimbleMap B" next to the other alternative, so both can be on the phone at once.

Status at merge: the Swift was type-checked against MapLibre 6.31 headers (Catalyst SDK, Command
Line Tools) and the Rust tests pass, but no Xcode build or simulator run was possible on the
build machine (Xcode license not yet accepted), so expect small first-build fixes.

## Test files on the iPhone (2026-11-03)

Documents is shown in Files as On My iPhone › NimbleMap B (`UIFileSharingEnabled`, `LSSupportsOpeningDocumentsInPlace`); the app's own copies of opened files moved from Documents/Data to Application Support/Data. The persona test files (from `LM-tmp/personas/data/maya` and `walt`, unchanged) were copied into Documents/Test files with `xcrun devicectl device copy to`.
