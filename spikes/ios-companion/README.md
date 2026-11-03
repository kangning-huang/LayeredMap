# iPhone companion spike — **Alternative A** (throwaway)

Phase 0 spike for a **native iPhone companion**: SwiftUI shell, MapLibre Metal map, URL open, Rust GeoJSON decode via a tiny C ABI. Ask is a non-functional stub.

**This checkout / PR is Alternative A** — floating A→B→C layers/symbology chrome
(`docs/lm/ios-layers-symbology.md`). Home-screen label: **NimbleMap A**
(`com.nimblemap.companion.spike.alta`). Alternative B (persistent sheet) is a separate
branch/spike — do not merge B into A. Compare by trying both.

**Not product code.** Lives under `spikes/` and will be thrown away or rewritten when the real app lands in `apps/` / `crates/lm-*`.

## Locked choices

| Piece | Choice |
| --- | --- |
| Shell | SwiftUI (iOS 17+) |
| Map | MapLibre Native iOS (SPM: `maplibre-gl-native-distribution`), Metal |
| Decode | `rustgis-data::io::read_geojson` behind `lm_decode_*` C ABI |
| Open | Paste HTTPS URL → download off main thread → sandbox file → Rust decode → MapLibre shape source |
| Ask | UI stub only (edge composer; does not own the layers sheet) |
| Layers chrome | **Alt A:** A idle pills → B bottom sheet (data layers only) → C lean symbology; live legend swatches; swipe Remove; Edit/`onMove` reorder. Design: `docs/lm/ios-layers-symbology.md`. |
| Map paint | Demo Cities / Roads / Parks GeoJSON; MapLibre circle/line/fill paint follows `LayerStore` (color, size/width, opacity, visibility). |
| Out | Cloud pickers, iPad layout, egui, Mac B2 renderer, App Store, Alt B persistent-sheet chrome |

Decision log: `docs/lm/decisions.md` (iPhone companion spike stack).

## Layout

```
spikes/ios-companion/
  Cargo.toml                 # lm-ios-decode-ffi (staticlib + cdylib + rlib)
  include/lm_decode.h
  src/{lib,ffi}.rs
  scripts/check-rust-host.sh
  scripts/build-rust-ios.sh
  ios/NimbleMapCompanion.xcodeproj
  ios/NimbleMapCompanion/*.swift   # + LayerModels, LegendSwatch, LayersSheet, SymbologySheet
  ios/NimbleMapCompanion.xcodeproj/xcshareddata/xcschemes/NimbleMapCompanion.xcscheme
  screenshots/
```

## Rust (works without Xcode)

From the repo root:

```bash
cargo test -p lm-ios-decode-ffi --locked
# or
./spikes/ios-companion/scripts/check-rust-host.sh
```

Host (`aarch64-apple-darwin`) is enough to verify decode + C ABI. The spike crate **allows `unsafe` only for the C ABI** (`src/ffi.rs`); product crates keep `unsafe_code = "forbid"`.

### iOS Rust targets

```bash
./spikes/ios-companion/scripts/build-rust-ios.sh aarch64-apple-ios-sim
./spikes/ios-companion/scripts/build-rust-ios.sh aarch64-apple-ios
```

Needs **full Xcode** (iPhoneSimulator / iPhoneOS SDKs). Command Line Tools alone are not enough (`xcode-select` must point at `Xcode.app`).

## How to try Alternative A

1. Open `spikes/ios-companion/ios/NimbleMapCompanion.xcodeproj` in full Xcode.
2. Run on an iPhone simulator (or device). Home screen name: **NimbleMap A**.
3. You should see an **Alt A** chip top-left, Open / Layers / Ask pills, and NYC demo Cities / Roads / Parks on the map.
4. Tap **Layers** → reorder (Edit), eye toggle, swipe-left **Remove**, tap a row for **Style**.
5. Change color / size / opacity — legend swatch **and** map paint update. Done / dismiss → back to map-first A.

Parallel Alternative B is a different branch; install both to compare (different bundle ids).

## Xcode / simulator

1. Install Xcode from the App Store; run it once; accept the license.
2. `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`
3. Open `spikes/ios-companion/ios/NimbleMapCompanion.xcodeproj`
4. Set your **Team** on the NimbleMapCompanion target (Signing & Capabilities) for device; simulator can use automatic signing with an empty team in many setups.
5. Wait for SPM to resolve **MapLibre** (`https://github.com/maplibre/maplibre-gl-native-distribution`, product `MapLibre`).
6. Pick an iPhone 16 / 17 simulator (or a device), Build & Run.
7. The “Build Rust FFI” run script builds `liblm_ios_decode_ffi.a` for sim or device and copies it into `BUILT_PRODUCTS_DIR`.

### Device notes

- First device run needs a development team and a plugged-in trusted iPhone.
- Network: Open URL uses `URLSession` over HTTPS only.
- No App Store Connect / MAS review credentials in this spike.

## What works vs stubbed

| Works (verified on host Rust) | Stubbed / needs Xcode |
| --- | --- |
| GeoJSON decode via `rustgis-data` | Full app Run on simulator/device |
| C ABI (`lm_decode_geojson`, `_path`, `lm_string_free`) | MapLibre pan/pinch (code present; needs SPM + SDK) |
| Unit tests for decode + FFI | Ask assistant calls |
| A→B→C layers/symbology chrome (demo layers + live swatches) | Real dataset → layer list import |
| MapLibre paint from `LayerStore` on demo GeoJSON | |
| | Drive / Box / Dropbox pickers |
| | iPad denser chrome |

## CI

Desktop CI (`cargo test --workspace`, `cargo xtask ci`) will compile this crate on host targets. It does **not** build the Xcode app or link MapLibre. No App Store steps.

## Map library note

Preferred: **MapLibre Native** via SPM. MapKit was not used. If MapLibre SPM fails to resolve in Xcode, fix the package reference before falling back; do not silently ship MapKit without a decisions.md update.

## Simulator smoke (2026-10-30, Asia/Shanghai)

Built and run on office Mac (`NYUSH-C02GX119Q6X3`, Xcode 16.4, iPhone 16 Pro Simulator 18.5/18.6 UDID `2549FF6E-6F15-43FF-8B93-B1AAA0F31866`).

- Bundle id: `com.nimblemap.companion.spike.alta` (home screen: **NimbleMap A**)
- Screenshot: `screenshots/sim-iphone16pro-20261030-195256.png` (map + Open URL + Ask stub)
- Build notes: need Rust **1.99+** (if-let guards in `rustgis-data`); clear conda `CC`/`LD`/`SDKROOT` before `xcodebuild` or the Apple linker is shadowed; Xcode “Build Rust FFI” script uses `$(SRCROOT)/../scripts/…` and `$(SRCROOT)/../include`.

## Frame-rate architecture comparison (max FPS / butter-smooth)

Goal: **highest sustained gesture frame rate** on phone, with URL decode kept off the UI thread. Deciding metric: display-refresh delivery during pan/pinch (ProMotion up to 120 Hz on device; Simulator is typically 60 Hz).

| Route | Gesture path | Expected / measured FPS feel | Fits companion + URL decode? | Verdict for phone |
| --- | --- | --- | --- | --- |
| **1. MapLibre Native (Metal) + SwiftUI + Rust FFI** (this spike) | MapLibre owns pan/pinch on Metal; SwiftUI sheets never block the map | **Sim measured (2026-10-30):** CADisplayLink badge showed **60 FPS** idle with MapLibre demotiles visible on iPhone 16 Pro Simulator (display-limited; Simulator is not ProMotion). Screenshot `screenshots/sim-iphone16pro-fps-20261030-195417.png`. MapLibre draws basemap + overlays on GPU; gestures do not wait on Rust. Device ProMotion should allow up to 120 Hz when the GPU keeps up. | Yes — decode via `lm_decode_*`, overlay as `MLNShapeSource` | **Keep.** Best FPS ceiling among realistic options; matches locked butter-smooth bar. |
| **2. MapKit + SwiftUI + Rust decode** | MKMapView Metal gestures | Also ProMotion-capable; Apple-tuned. Overlay styling and GeoJSON parity are weaker than MapLibre for GIS overlays. | Yes for open/decode; weaker for custom vector styling | Viable fallback if MapLibre SPM breaks; not higher FPS than MapLibre in practice. |
| **3. Shared Mac B2 CPU renderer → MTKView** | Worker renders to texture; gesture moves/scales cached texture (ADR-001 B2) | **ADR-001 (Mac M4, vsync off):** B2 p95 frame ~5 ms (~200 Hz headroom) but **content soft** during zoom until worker catches up (p95 render ~386 ms). On phone, CPU soft-zoom + battery cost; UI thread stays free (&lt;4 ms block on Mac). | Possible, but duplicates MapLibre’s job and soft-zoom fights “butter sharp” on phone | Lower *perceived* sharpness during gestures than GPU map engines; keep as Mac strategy, not phone map. |
| **4. egui/eframe shrink on iOS** | egui paints whole UI | ADR-001 strategy A (redraw every frame) blows past 33 ms on zoom/pinch; B blocks UI up to ~790 ms on gesture end. Phone would feel like a web/desktop port, not native chrome. | Technically possible; fails butter-smooth UX goal | **Lower bar — reject** for companion shell. |

**Sources:** this Simulator run; `docs/lm/adr-001-rendering.md` (B2/C measurements); `docs/lm/decisions.md` (2026-10-30 iPhone companion stack).

**Conclusion:** keep **MapLibre Native + SwiftUI + Rust decode**. It wins on max frame-rate *feel* for phone (GPU map + native gestures + ProMotion headroom) without giving up URL/Rust decode. MapKit is the only close FPS peer; B2/MTKView is for Mac parity, not phone; egui-on-iOS is explicitly the lower bar. No stack switch unless device Instruments later shows MapLibre dropping below ~60 FPS under real overlays while MapKit holds 120.


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

1. `open ios/NimbleMapCompanion.xcodeproj` (full Xcode, license accepted; `rustup` installed — the build
   phase adds the `aarch64-apple-ios` target and builds the Rust library itself).
2. Target NimbleMapCompanion › Signing & Capabilities: pick your Team (the project ships with none). If the
   bundle id is taken, change its suffix.
3. Plug in the iPhone, turn on Settings › Privacy & Security › Developer Mode, pick it as the run
   destination, Run. The first build fetches MapLibre (SPM) and compiles the Rust crate.
4. It installs as "NimbleMap A" next to the other alternative, so both can be on the phone at once.

Status at merge: the Swift was type-checked against MapLibre 6.31 headers (Catalyst SDK, Command
Line Tools) and the Rust tests pass, but no Xcode build or simulator run was possible on the
build machine (Xcode license not yet accepted), so expect small first-build fixes.

## Test files on the iPhone (2026-11-03)

Documents is shown in Files as On My iPhone › NimbleMap A (`UIFileSharingEnabled`, `LSSupportsOpeningDocumentsInPlace`); the app's own copies of opened files moved from Documents/Data to Application Support/Data. The persona test files (from `LM-tmp/personas/data/maya` and `walt`, unchanged) were copied into Documents/Test files with `xcrun devicectl device copy to`.
