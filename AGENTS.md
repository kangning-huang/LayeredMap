# RustGIS — instructions for agents

RustGIS is a fast, free and open-source GIS in pure Rust for mapping, analyzing and sharing
geospatial data, on the desktop and in the browser. Its architecture: layered crates, everything
is a command, a thin egui front end, headless screenshots for UI work.

## Never crash
- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` in non-test code; no `unsafe`
  (`unsafe_code = "forbid"`). Crate roots carry the `#![deny(clippy::unwrap_used, …)]` block.
- Input from files and command parameters is hostile: `get()` over indexing, clamp sizes,
  bound recursion. `Session::run` and `analysis::run` catch escaped panics as a last resort.
- `crates/engine/src/tests.rs::hostile_params_never_panic` and the analysis equivalent run every
  command/tool with junk parameters. Keep them green; every crash fix gets a regression test.

## Architecture
| Layer | Crate | Job |
|---|---|---|
| L0 | `geom` | coordinates, extents, Web Mercator, geodesic measurement, scale |
| L1 | `data` | features, typed attributes, GeoJSON/CSV/Shapefile/KML/GPX I/O, GeoPackage/GeoParquet/FlatGeobuf/LiDAR (LAS/LAZ) providers (`source`; GeoPackages in the browser through the pure-Rust `sqlite` reader; GDAL formats and DuckDB queries through their programs), SQL where clauses, rasters (GeoTIFF/ASCII grid, Zarr stores, EPSG reprojection) |
| L2 | `map`, `analysis` | layers, symbology, labels, camera, basemaps, web tile layers; geoprocessing tools |
| L2 | `tiles` | web map tiles: XYZ/TMS/WMS rasters, Mapbox Vector Tiles, PMTiles and MBTiles archives, a MapLibre style subset; the tile cache and background fetching (see `docs/user-guide/maps.md` › Basemaps) |
| L3 | `render` | tiny-skia map renderer, label placement, PNG export, font discovery |
| L3 | `plugins` | plugins: sandboxed WebAssembly modules (`wasmi`) adding commands and tools; `.rgplugin` packages, the plugin registry (see `docs/plugins.md`) |
| L3 | `geolibre` | the GeoLibre/Whitebox tool suite as geoprocessing tools (engine feature `geolibre`): run by the optional `geolibre` program natively, by geolibre-rust's WASI runner in the browser |
| L4 | `engine` | `Session`, undo, command registry, sample project |
| L5 | `assistant` | the AI Assistant: a chat agent (sans I/O: `provider` builds and reads OpenAI/Anthropic/Gemini requests, `agent` loops, `tools` run commands; file, network and plugin commands wait for the user's approval) |
| L6 | `ui-egui` | ribbon, panes, map view, attribute table, dialogs (swappable) |
| apps | `rustgis`, `rustgis-cli`, `rustgis-web`, `rustgis-python` | desktop app, command line, browser (WebAssembly), the `rustgis` Python package (PyO3, built with maturin) |

- **Everything is a command**: a `CommandSpec` in `crates/engine/src/cmd/*.rs`. UI-only
  commands (`ui.*`) live in `crates/ui-egui/src/lib.rs`. The ribbon, shortcuts, command search
  and CLI dispatch by id. Commands never open dialogs.
- **Never break wasm**: everything below `apps/` must build for `wasm32-unknown-unknown`
  (CI runs `cargo build --target wasm32-unknown-unknown -p rustgis-web`; the Zstandard decoder in
  `rustgis-data` is C, so this needs clang). Use `web_time`, not
  `std::time`; file access goes through `Services` on the web. `rustgis-geolibre` builds for
  both: natively it runs the `geolibre` program; in the browser the web app runs geolibre-rust's
  WASI runner asynchronously (`external_start` → `Services::geolibre_run` → `gp.finish`).
- **Interface text is translatable**: wrap user-visible literals in `tl!("…")`, use
  `i18n::fmt("… {name} …", &[("name", …)])` instead of `format!` for messages, and pass labels to
  the ribbon widgets (they translate). Then add the new strings to every catalog in
  `crates/ui-egui/src/i18n/*.tsv` (format in `docs/i18n.md`). Command ids, data and the CLI are
  never translated.
- **Layering** is enforced by `cargo xtask layers`: nothing below L6 depends on egui/eframe/rfd.

## Assets
- No third-party vendor icons, images, styles, fonts, help text or code. Icons are drawn in
  code (`icons.rs`). Every asset file has a row in `ATTRIBUTION.md`.
- Screenshots and other images are not committed here: they live in
  [opengeos/rustgis-assets](https://github.com/opengeos/rustgis-assets) (WebP, served at
  `https://assets.rustgis.app/images/…`) and docs link to them by URL.

## Quality gates
`cargo xtask ci` (fmt, clippy -D warnings, tests, layers) before every commit.

## Looking at the app
- `cargo run --release -p rustgis` (opens the World Atlas sample).
- Headless screenshots: `cargo run --release -p rustgis-ui-egui --example ui_shot -- script.jsonl`
  (see the example header). For UI work, look at the PNG.
- CLI: `rustgis-cli run --sample --cmd 'gp.run={"tool":"buffer","params":{"input":"Cities","distance":100}}' --cmd 'share.exportMap={"path":"out.png"}'`.

---

# LayeredMap — instructions for agents

This repository is **LayeredMap**, a map-first GIS for novices built on RustGIS's engine crates.
The build plan is `docs/lm/spec.md` (exported from the owner's design document; re-exported at
every phase start). The owner judges behavior against that plan, not the Rust. Everything above
this section still applies to the RustGIS crates.

## Where things live
- Product code lives **only** in `crates/lm-*` and `apps/layeredmap`. RustGIS's ribbon front end
  (`rustgis-ui-egui`, `apps/rustgis`) is not used by LayeredMap.
- Any change inside a RustGIS crate is its own commit, prefixed `upstream:`, so it can be offered
  back. Upstream is merged only at a phase start (recorded in `UPSTREAM.md`).
- Phase 0 spikes live in `spikes/` and are thrown away.
- Small fixtures: `fixtures/<ID>/`; generated ones: `cargo xtask fixtures` (into
  `fixtures/generated`, or `$LM_FIXTURES_DIR`); reference scripts: `reference/`.
- Every choice made outside the plan: one short entry in `docs/lm/decisions.md`.

## The twelve product principles (higher wins)
1. A basic feature that sometimes lies is a release blocker.
2. Sources are never modified.
3. The map is the home screen.
4. Feel is a feature (Map feel targets are exit criteria).
5. Make visible assumptions; ask only when it matters (question budget: median ≤ 1, p90 ≤ 3).
6. Insight comes before action.
7. One selection is enough.
8. Every action shows its scope.
9. Every result explains itself.
10. Show at most three suggested actions, in a stable order.
11. Technical words are findable, never required.
12. Deterministic before AI.

## Rules
- **Layering** (`cargo xtask layers`, also checked for `lm-*`): each `lm-*` crate may use only the
  internal crates its row of the module table allows (`xtask/src/lm.rs`). Below `lm-ui` only
  `lm-mapview` may use egui; only the app uses eframe or rfd; `lm-ui` never calls `rustgis-data`
  or `rustgis-analysis` — all work goes through `lm-jobs`.
- **The UI never computes numbers.** Counts and means come from data, never from drawn pixels.
- **Every string is translatable.**
- **Tests first in `lm-analysis`**: a written definition, a reference implementation in
  `reference/`, a tolerance.
- **Every UI change includes a screenshot** from the headless `ui_shot` approach.
- Every `lm-*` crate except `lm-ui`, `lm-mapview` and the app builds for `wasm32-unknown-unknown`.
- No `unwrap()`, `expect()`, `panic!` or `unsafe` outside tests. The one exception: the app's macOS
  module may use `unsafe` (Open With, Dock drops, the Edited state), with a `SAFETY` comment on
  each block and a second review.

## Guardrails — the agent may not
- change gate criteria, thresholds or the plan;
- disable or skip tests;
- edit a fixture's expected output without a note;
- add network calls outside the basemap, search, library and update services;
- add a dependency whose license is not on the allow list (MIT, Apache-2.0, BSD, ISC, Zlib,
  MPL-2.0, Unicode-3.0, BSL-1.0, CC0-1.0, OFL-1.1 for fonts, and the Ubuntu font licence used by
  egui's default fonts) without the owner's approval.

## Working rhythm
- One branch per task off the phase branch (`wp/2.3-csv-coordinates`); one PR per task card
  (`docs/lm/task-card-template.md`), with a demo (screenshot or recording).
- A phase branch `phase-N` merges into `main` only when its gate passes; the merge is tagged
  `gate-N-pass`. Gate reports: `gates/gate-N.md` from `gates/TEMPLATE.md`.
- PRs touching `lm-analysis`, `lm-jobs` or `lm-doc` get a second, independent agent review.
- Every bug fix shows a test failing first.

## Building on this machine
- `cargo xtask ci` before every commit. `cargo run --release -p layeredmap` for the app.
- The rendering spike: `cargo run --release -p lm-render-spike` (interactive) or `-- --bench`.
