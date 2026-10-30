# iPhone companion spike (throwaway)

Phase 0 spike for a **native iPhone companion**: SwiftUI shell, MapLibre Metal map, URL open, Rust GeoJSON decode via a tiny C ABI. Ask is a non-functional stub.

**Not product code.** Lives under `spikes/` and will be thrown away or rewritten when the real app lands in `apps/` / `crates/lm-*`.

## Locked choices

| Piece | Choice |
| --- | --- |
| Shell | SwiftUI (iOS 17+) |
| Map | MapLibre Native iOS (SPM: `maplibre-gl-native-distribution`), Metal |
| Decode | `rustgis-data::io::read_geojson` behind `lm_decode_*` C ABI |
| Open | Paste HTTPS URL → download off main thread → sandbox file → Rust decode → MapLibre shape source |
| Ask | UI stub only |
| Out | Cloud pickers, iPad layout, egui, Mac B2 renderer, App Store |

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
  ios/NimbleMapCompanion/NimbleMapCompanion/*.swift
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
| | Drive / Box / Dropbox pickers |
| | iPad denser chrome |

## CI

Desktop CI (`cargo test --workspace`, `cargo xtask ci`) will compile this crate on host targets. It does **not** build the Xcode app or link MapLibre. No App Store steps.

## Map library note

Preferred: **MapLibre Native** via SPM. MapKit was not used. If MapLibre SPM fails to resolve in Xcode, fix the package reference before falling back; do not silently ship MapKit without a decisions.md update.
