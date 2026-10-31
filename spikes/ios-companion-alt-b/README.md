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
