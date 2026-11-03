# Spike: Shapefile siblings under the App Sandbox

*Started 2026-10-30. Path rules and import preference land in-tree; Info.plist related-item
types already landed with MAS packaging on phase-4. A sandboxed MAS build on a Mac is still
needed to confirm file coordination.*

## Problem

App Review 2.4.5 requires the App Sandbox. Opening a `.shp` (or a GeoTIFF / CSV) grants access
only to that one file. A Shapefile needs its `.dbf`, `.shx`, `.prj` and `.cpg` beside it; a
GeoTIFF often needs `.tfw` / `.aux.xml`; a CSV may have a `.prj`. Without related items, import
fails with "No .dbf file" or wrong coordinates even when the companions are on disk.

## Approach

1. **Declare related types** in Info.plist (`NSIsRelatedItemType` on companion document types).
   Done in `packaging/lm/bundle-macos.sh` (MAS packaging on phase-4). This spike PR does not
   edit that script.
2. **Derive companion paths without listing the folder** — `lm_core::related::declared_siblings`.
   Under the sandbox, `read_dir` on the parent may be refused; requesting
   `districts.shp` → `districts.dbf` by path is what related items allow.
3. **Prefer the declared path when reading** — `lm_jobs` Shapefile import tries
   `path.with_extension(ext)` before a case-insensitive directory scan.
4. **File coordination (Mac verification)** — given a security-scoped URL for the primary file,
   present related items through `NSFilePresenter` / `NSFileCoordinator` so AppKit extends the
   scope to each declared sibling. The coordinator call belongs in the app's macOS module
   (the one place `unsafe` is allowed) once a sandboxed TestFlight or local MAS build exists.

## What this PR delivers

| Piece | Status |
|---|---|
| `lm_core::related` path rules + unit tests | Done |
| Shapefile import prefers declared companion paths | Done |
| Info.plist related-item document types in `bundle-macos.sh` | Already on phase-4 (packaging) |
| NSFilePresenter / coordinator wiring in `apps/layeredmap/src/macos.rs` | Not yet — needs a sandboxed run on a Mac |
| End-to-end open of a multi-file Shapefile in a MAS build | Ken verifies on Mac |

## Mac verification (Ken)

1. Build with `LM_CHANNEL=mas` (`packaging/lm/bundle-macos.sh`), install the `.pkg` (or run the
   sandboxed `.app` from the product archive).
2. Open a folder that holds a complete Shapefile (`*.shp` + `.dbf` + `.shx` + `.prj` + `.cpg`).
3. File › Open the `.shp` only (not the folder, not a ZIP). The layer should load with attributes
   and the right CRS. If attributes are missing, related-item coordination is not extending the
   scope — wire `NSFileCoordinator` next (see order of work in `docs/lm/app-store.md`).
4. Repeat with a GeoTIFF that has a `.tfw` and with a CSV that has a `.prj`.
5. Confirm Direct (Developer ID) builds are unchanged: companions still resolve when casing
   differs (the directory-scan fallback).

## Out of scope here

- Security-scoped bookmarks for reopening maps (app-store.md §5).
- StoreKit / update-check compile-out (already planned separately).
- Edits to `packaging/lm/*` (owned by the packaging track).
