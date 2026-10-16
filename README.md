# LayeredMap

LayeredMap is a calm, map-first desktop app for seeing how datasets relate in space. You open a
few files (a CSV of schools, a Shapefile of districts, a surface-temperature raster), each
lands on one map as a layer, and you can see and ask how they relate. It is meant for students
and other newcomers to GIS. They should get honest answers without first learning coordinate
systems, toolboxes or file formats.

LayeredMap is a fork of [RustGIS](https://github.com/opengeos/RustGIS) and is built on its engine
crates, with a new interface on top. It is open source (Apache-2.0) and at an early stage:
two of its seven phases are done. See [Status](#status).

## Contents

- [Design philosophy](#design-philosophy)
- [Status](#status)
- [Getting started](#getting-started)
- [Architecture](#architecture)
- [Repository map](#repository-map)
- [Contributing and working with coding agents](#contributing-and-working-with-coding-agents)
- [The RustGIS engine](#the-rustgis-engine)
- [Credits and license](#credits-and-license)

## Design philosophy

Full GIS packages are powerful, but they ask a lot of a newcomer up front. They want a project,
a coordinate system, a tool and a set of parameters before they show anything. Most of them
also assume the user can tell when an answer is wrong. LayeredMap reverses this. The map comes
first, likely interpretations are applied and shown, insight comes before tools, and every
number can be traced back to its inputs.

The build plan ([docs/lm/spec.md](docs/lm/spec.md)) settles design disputes with twelve
product principles, ranked so that the higher one wins a conflict. The first is the strictest:
**a basic feature that sometimes lies is a release blocker.** Wrong numbers, records lost
without notice, unit or scope changes nobody was told about, and modified source files all
block every gate. The ideas below come from those principles.

### The map first, and calm

The map is the home screen. The first window is a usable world map, with no wizard, account,
project picker or empty toolbox. A welcome card says "Open a file to see your data on the map."

The interface is quiet, neutral and solid. Floating panels sit over a full-bleed map, with one
accent colour and a short, curated set of data palettes. Panels never push the map, so its
centre stays put when they open or close. Motion is short and purposeful, and it follows the
system's Reduce Motion setting. The wording follows fixed rules: sentence case, buttons that
start with a verb, numbers with units and thousands separators ("1,248 schools", "31.2 °C"),
no exclamation marks, and no blame ("This file does not say…", never "You forgot…").
Technical words are findable but never required. Searching "buffer" finds "Show an area within
a distance", and terms like EPSG appear only in Details, Help and search aliases.

### One action to open a file, and it lands in the right place

Common files (GeoJSON, Shapefile, zipped Shapefile, CSV/TSV with coordinates, GeoTIFF/COG)
should open with one action: the Open button, a drop on the window or the Dock icon, or
Finder's Open With. There is no import wizard. Each file goes through the same pipeline off the
UI thread, and the map stays usable while it loads. The file is detected, read, profiled and
drawn, and its row on the layer shelf shows progress. It also lands where it belongs: the app
works out the coordinates, the encoding and how the values should be styled.

If the file has no coordinate system and its numbers look projected, the app does not ask for an
EPSG code. It asks **"Where should this data be?"** The user types a place or clicks the map.
The app tries every coordinate system that covers that place and shows each candidate as a
thumbnail over the basemap, and the user picks the one that lands on the right streets.

### Uncertainty as visible assumptions

Files often leave things unsaid. Which columns are longitude and latitude? Is −9999 "no data"?
Is this raster in kelvin? Are its cells averages or totals? LayeredMap neither hides these
guesses nor asks about each one. Every uncertain interpretation gets one of four treatments,
chosen by how sure the app is and what a wrong guess would change:

1. **Silent.** Applied and recorded in Details. Used when the evidence is certain, or when a
   wrong guess changes only appearance.
2. **Visible assumption.** Applied at once and stated in one line with a Change link, for
   example "Located using lon and lat as GPS coordinates (WGS 84). Change". The line appears on
   the layer overview and in any recipe that used it. It never blocks.
3. **Question card.** Asked once, with a recommended answer preselected and a **Not sure**
   option. Used only when confidence is low and a wrong guess would change numbers or locations.
4. **Block.** The action is not suggested. In More actions it appears with the reason and what
   would unblock it.

Most cases end up as a visible assumption, not a question. The plan puts it this way: "That is
how the app stays fast without becoming dishonest."

**Ask at the moment of consequence, not at import.** Questions about *where* data is come at
import, because nothing can be drawn without the answer. Questions about *what values mean*
come only when an action first needs the answer. Opening a raster never asks whether its cells
are totals or averages; the first "Summarize in areas" does. Choosing **Not sure** applies the
safest interpretation and narrows what is offered, so it never leads to a dead end.

Questions are rationed. In a novice session the median number of question cards must be at most
1, and the 90th percentile at most 3. The app counts them locally, without data values or file
names, and the count is a pass/fail number at Gates 2, 4 and 5.

Some cases are specific to China, because the first users are students in Shanghai. Points
copied from Amap, Tencent or Baidu maps can sit a few hundred metres off. LayeredMap says so,
and its Compare view shows both positions over a street map, so you can keep the set that sits
on the streets.

### Numbers come from data, never from pixels

Counts, means and ranks are computed from the data itself, never from drawn pixels or visible
symbols. This is a layering rule: the interface cannot do spatial math. All work goes through
the job layer, and CI checks this.

Every action shows its scope, both before and after it runs: "All 248 schools", "12 selected
schools" or "Inside the drawn area". Numbers carry units. If a preview was computed only for
the current view, its banner says so. The algorithms behind the answers (fractional-coverage
zonal summaries, boundary-aware point counts, geodesic measurement, Chinese coordinate
conversion) are written tests first. Each has a written definition, an independent reference
implementation in [`reference/`](reference/) and a tolerance.

### No record lost without notice, and sources never modified

Imported files are read-only. Removing a layer never deletes a file, and every result is a new
layer. Converting coordinates, for example, creates a derived layer and leaves the source alone.

If some records cannot be used, such as rows with no usable location, the import says so.
Each import ends with a status: complete, complete with exclusions, needs a decision, canceled,
or failed. "Review 6" opens the excluded records with a reason for each, and they can be
exported as a CSV problem list. A canceled import leaves nothing behind.

### Every result explains itself

A result card states what was computed, from which inputs, over what scope, and what was
excluded. The full recipe is one click away and can be copied as plain text. Here is the plan's
example (the version number is illustrative):

```text
Schools in each district
Computed 14 Dec 2026 with LayeredMap 0.4.0
Inputs: Schools (schools.csv, 248 points); Districts (districts.geojson, 17 areas)
Scope: all schools; all districts
Method: point-in-area count; a point on a boundary counts in every area it touches
Decision: overlapping areas each count their points (2 schools counted twice)
Assumption: schools located using lon/lat as WGS 84 (likely, from column names)
Excluded: 6 schools with no usable location
```

If a source file changes later, the result is marked "Source changed" rather than silently
going stale.

### Insight before action

Selecting something first shows where it stands, before it offers tools: rank, share, and a
comparison with the median ("3rd hottest of 17"). Only then does the app suggest what you can do,
with at most three suggested actions in a stable order. One selection is enough. An action that
needs a second layer picks one automatically, and you can change it. Suggestions are
deterministic rules, so they work with no AI model, account or network.

### Undo for everything

Every document change is one transaction, and undo reverses it. One user action is one undo
step: dragging a slider for 30 seconds leaves exactly one step. Keeping a result and then undoing
returns the exact earlier state, and redo gives the identical result. Changing an assumption can
be undone too. The Undo button's tooltip names the step ("Undo Count schools in each district").

### Map feel is a requirement

"If the map stutters, the product fails, however correct it is." Pan, zoom, tile loading and
labels have numeric targets. They are measured on a reference Mac at 2× display scale with
100,000 points and 10,000 polygons loaded (fixture F1):

| Behaviour | Target |
|---|---|
| Frame time while panning and zooming | p95 ≤ 16.7 ms (60 fps) as the goal; ≤ 33 ms as the hard limit |
| Input to first visible motion | within one frame |
| Wheel zoom | animated over 250 ms; the point under the pointer drifts by at most 1 px |
| Missing tiles | never blank: the nearest loaded parent tile or the offline basemap, new tiles fading in |
| Labels | no flicker while panning |
| Data layers | never disappear mid-gesture |
| UI thread | no file or network I/O |

At Gate 1 the feel harness (`cargo xtask feel`) measured a worst p95 of 5.2 ms for panning and
4.5–4.6 ms for zooming and pinching. Not one of 20,223 frames went over 16.7 ms. A cold
launch took 198 ms to the first map frame (p95, against a 2 s limit). The results are in
[`docs/lm/feel/`](docs/lm/feel/).

### Gated phases, validated by the owner

LayeredMap is built in seven phases, and each ends at a gate. A gate has automated checks, a
debug pass (including a review by a fresh agent session with no context from the build), a
hands-on validation session with the owner, and stress tests. A gate passes only when all of
them pass. The two other outcomes are a fix forward within five working days or a rollback to
the last passing tag. Each phase works on a `phase-N` branch, which is merged into `main` and
tagged `gate-N-pass` only when its gate passes. Priority decides what gets cut when a phase runs
late, never what gets tested less.

## Status

**Phases 0 and 1 have passed their gates** (tags `gate-0-pass` and `gate-1-pass`, 2026-10-15
and 2026-10-16). Today the app opens straight to a world map that pans, zooms and searches
places online and offline, in light and dark. **Phase 2 is in progress:** one-click open and
visible assumptions.

| Phase | Goal | Status |
|---|---|---|
| 0. Foundation and risk spikes | Prove the stack can feel like a consumer map before any product code; build the test fixtures | Done (`gate-0-pass`) |
| 1. Map feel and the empty shell | An app with no data yet that pans, zooms and searches places like Apple Maps, online and offline | Done (`gate-1-pass`) |
| 2. One-click open and visible assumptions | Common files open with one action and land in the right place; uncertainty shows as visible assumptions | In progress |
| 3. Inspect, select, and insight cards | Click anything and see what it is, where it stands among its peers, and how to show it clearly | Planned |
| 4. Contextual actions, previews, result cards | Ask one of up to three relevant questions, preview the answer, keep a result that explains itself | Planned |
| 5. Keep, share, data library | Save, recovery, export, a curated data library and a signed macOS build; ends with a classroom pilot | Planned |
| 6. Hardening | Pilot fixes, Windows, full accessibility, more actions; ends with a public beta | Planned |

Gate reports, including items carried open, are in [`gates/`](gates/). Choices made outside the
plan are logged in [`docs/lm/decisions.md`](docs/lm/decisions.md). The name "LayeredMap" is
provisional until Gate 5.

## Getting started

There are no published releases yet. Build from source.

### Requirements

- A Rust toolchain from [rustup](https://rustup.rs), version 1.90 or newer (the workspace's
  `rust-version`).
- A C compiler for a few native dependencies (on macOS, the Xcode Command Line Tools).

### Build and run

```bash
git clone https://github.com/kangning-huang/LayeredMap.git
cd LayeredMap
cargo run -p layeredmap --release
```

The package is `layeredmap` (`apps/layeredmap`). The first build compiles the whole engine and
takes a while.

### A macOS app bundle

```bash
packaging/lm/bundle-macos.sh            # writes dist/lm/LayeredMap.app
```

Without signing credentials, the bundle is ad-hoc signed. It opens normally on the Mac that built
it. On another Mac, macOS blocks it the first time. Open it once through **System Settings ›
Privacy & Security › Open Anyway**. Developer ID signing and notarization are already wired
into the script and CI; whether to enroll is decided before the classroom pilot. See
[`docs/lm/signing.md`](docs/lm/signing.md).

### Platforms

macOS on Apple silicon comes first, and it is where every gate is validated. Windows packaging is
planned for Phase 6, and Linux is best effort. The code is cross-platform Rust, and the project
is open source.

### Checks

| Command | What it does |
|---|---|
| `cargo xtask ci` | Formatting, clippy with warnings as errors, workspace tests, and the layering checks; run before every commit |
| `cargo xtask layers` | The layering checks alone, for the RustGIS crates and the `lm-*` crates |
| `cargo xtask fixtures` | Generates the large test fixtures into `fixtures/generated` (or `$LM_FIXTURES_DIR`); options `--out DIR`, `--only IDS`, `--large` |
| `cargo xtask feel` | The map-feel harness: scripted camera paths over fixture F1, reporting frame times; `--runs N`, `--out DIR` |
| `cargo xtask launch` | Cold-launch time to the first map frame; `--n N` |

## Architecture

LayeredMap adds eight crates and one app to seven RustGIS engine crates. The new interface
reaches the engine only through the action and job layers, so the UI never does spatial math.
The only direct line from the interface to RustGIS is the map view's drawing path. RustGIS's
ribbon front end is not used.

| Module | Job | Depends on |
|---|---|---|
| `apps/layeredmap` | App entry, window, native menu bar, file associations, packaging | `lm-ui` |
| `lm-ui` | The map-first shell: top bar, layer shelf, inspector, composer, result cards, table drawer, theme | `lm-core`, `lm-actions`, `lm-jobs`, `lm-doc`, `lm-mapview` |
| `lm-mapview` | Map widget: camera, gestures, inertia, tile placeholders, hit testing, selection highlight | `rustgis-map`, `rustgis-tiles`, `rustgis-render`, `rustgis-geom` |
| `lm-core` | State model and data contracts; no UI, no file or network I/O | none |
| `lm-profile` | Import-time profiling: field roles, coordinate-system confidence, NoData, units, China offsets, assumptions | `lm-core`, `rustgis-data`, `rustgis-geom` |
| `lm-actions` | Action registry, eligibility, relevance ranking, per-phase feature flags | `lm-core`, `lm-profile` |
| `lm-jobs` | Plans, previews, background work, cancellation, stale-result guard, atomic commit, compound undo | `lm-core`, `lm-profile`, `lm-analysis`, `rustgis-engine`, `rustgis-analysis` |
| `lm-analysis` | Certified algorithms where upstream lacks the needed meaning | `rustgis-geom`, `rustgis-data` |
| `lm-doc` | Document format, autosave, recovery, source fingerprints, relinking | `lm-core`, `rustgis-data` |

Rules enforced by `cargo xtask layers` (and so by `cargo xtask ci`), with the allowed
dependencies in [`xtask/src/lm.rs`](xtask/src/lm.rs):

- Each `lm-*` crate uses only the internal crates its row allows.
- Below `lm-ui`, only `lm-mapview` may depend on egui; only the app uses eframe or rfd.
- `lm-ui` never calls `rustgis-data` or `rustgis-analysis`; all work goes through `lm-jobs`.

Other rules carried over from RustGIS: no `unwrap()`, `expect()`, `panic!` or `unsafe` outside
tests, with one reviewed exception for native hooks in the app's macOS module. Every interface
string is translatable. Every `lm-*` crate except `lm-ui`, `lm-mapview` and the app builds for
`wasm32-unknown-unknown`.

State lives in three stores with one update loop. Document state is saved, interaction state
is not, and execution state stays in memory. The UI emits an intent. A finished job commits
its result in a single transaction, and that transaction is what undo records.

## Repository map

| Path | What is there |
|---|---|
| [`docs/lm/spec.md`](docs/lm/spec.md) | The build plan: principles, interaction specification, phases and gates. Exported from the owner's design document at each phase start; change the plan, not this file |
| [`docs/lm/decisions.md`](docs/lm/decisions.md) | Every choice made outside the plan, one line each |
| [`docs/lm/capabilities.md`](docs/lm/capabilities.md) | Audit of what upstream RustGIS can and cannot do for LayeredMap |
| `docs/lm/` (other) | ADR-001 (rendering), basemap hosting, signing, the task-card template, feel results |
| [`gates/`](gates/) | Gate reports (`gate-N.md`), validation scripts, the report template |
| [`fixtures/`](fixtures/) | Small hand-made test fixtures with expected outputs; licences in `fixtures/LICENSES.md` |
| [`reference/`](reference/) | Independent Python reference scripts that produce the expected numbers |
| [`design/`](design/) | The clickable HTML design mock (one is built ahead of each phase) and app-icon concepts |
| [`spikes/`](spikes/) | Phase 0 throwaway spikes (rendering, capability audit) |
| [`packaging/lm/`](packaging/lm/) | `bundle-macos.sh`, which builds `LayeredMap.app` |
| [`tools/lm/`](tools/lm/) | Helper scripts: the place-search index builder and the tile-latency probe |
| `assets/` | Bundled fonts and the GeoNames city index, each with a README |
| `crates/lm-*`, `apps/layeredmap` | LayeredMap's code |
| `crates/*` (other), `apps/rustgis*` | The RustGIS engine and its own apps, kept close to upstream |

## Contributing and working with coding agents

LayeredMap is built mostly by AI coding agents, directed by the owner. **The agent writes the
code, and the owner owns the behaviour.** Behaviour is judged against the plan, not the Rust.
If the plan is silent, the decision is made and added to the plan first.

- Read [`AGENTS.md`](AGENTS.md) first. Its LayeredMap section lists the twelve principles, the
  layering rules and the guardrails. Agents may not change gate criteria or thresholds, skip
  tests, edit expected fixture outputs without a note, add network calls outside the basemap,
  search, library and update services, or add dependencies outside the licence allow list.
- Work is split into task cards ([template](docs/lm/task-card-template.md)), one branch per card
  off the phase branch (for example `wp/2.3-csv-coordinates`), and one pull request per card,
  each with a screenshot or recording.
- Product code goes only in `crates/lm-*` and `apps/layeredmap`. A change inside a RustGIS crate
  is its own commit, prefixed `upstream:`, so it can be offered back. Upstream is merged only at
  the start of a phase ([`UPSTREAM.md`](UPSTREAM.md)).
- Every bug fix shows a test failing first. Pull requests that touch `lm-analysis`, `lm-jobs` or
  `lm-doc`, the code that handles numbers, jobs and saving, get a second, independent review.
- Run `cargo xtask ci` before every commit.

Issues and suggestions from students, teachers and researchers are welcome, especially real files
that open badly and wording that confuses.

## The RustGIS engine

RustGIS is a fast, free and open-source GIS in pure Rust by the
[opengeos](https://github.com/opengeos) project. It reads many vector and raster formats, draws
online vector basemaps, runs geoprocessing tools, and ships its own ribbon desktop app, command
line, web app and Python package. LayeredMap uses its geometry, data, map, tiles, render,
analysis and engine crates. Those crates' own documentation (`docs/` outside `docs/lm`,
`zensical.toml`) is kept as it is upstream. For RustGIS itself, see
[github.com/opengeos/RustGIS](https://github.com/opengeos/RustGIS) and
[rustgis.app](https://rustgis.app).

## Credits and license

LayeredMap is a fork of [opengeos/RustGIS](https://github.com/opengeos/RustGIS). Its maps,
data reading, rendering and much of what makes the app fast come from the RustGIS engine and the
people who build it. The upstream baseline and every merge are recorded in
[`UPSTREAM.md`](UPSTREAM.md).

Licensed under the [Apache License 2.0](LICENSE), inherited from RustGIS.

The app's Credits panel lists:

| What | Credit |
|---|---|
| Map data | © OpenStreetMap contributors (ODbL), through the Protomaps basemap |
| Offline basemap | Natural Earth (public domain) |
| Place search | GeoNames ([geonames.org](https://www.geonames.org/)), CC BY 4.0 ([`assets/geonames/README.md`](assets/geonames/README.md)) |
| Fonts | Inter and Noto Sans SC, SIL Open Font License 1.1 ([`assets/fonts/README.md`](assets/fonts/README.md)) |
| Icons | Phosphor Icons (MIT) |
| Engine | RustGIS (Apache-2.0) |

RustGIS's own assets and their licences are listed in [`ATTRIBUTION.md`](ATTRIBUTION.md), and
the test fixtures' sources in [`fixtures/LICENSES.md`](fixtures/LICENSES.md).
