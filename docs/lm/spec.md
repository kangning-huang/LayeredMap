<!--
  docs/lm/spec.md — the LayeredMap build plan, exported as Markdown for agents to read.
  Source: the Claude Docs document "LayeredMap Build Plan"
  (https://claude.ai/artifact/VRu8z2LFG6YcXodK1Z48Yg), revision 128, exported 2026-10-15.
  Refreshed at every phase start (build plan › Working with coding agents). Do not edit here:
  change the plan, then re-export. Diagrams are not exported; their captions mark where they sit.
-->


# LayeredMap Build Plan
2026-10-15 · Kangning Huang

## Summary
LayeredMap is built in seven phases. Phases 0–5 (about 22 effort-weeks) end in a classroom pilot. Phase 6 (about 6 more) ends in a public beta.
Every phase ends at a gate. A gate has four parts: automated checks, a debug pass, a hands-on validation session with you, and stress tests. A gate passes only when all four pass. Otherwise the product rolls back to the last passing tag, and work resumes from there.
1. **Phase 0 — Foundation and risk spikes.** Decide whether RustGIS's rendering and UI stack can feel like a consumer map. Settle this before writing product code. Also build the test fixtures and set up signed builds.
2. **Phase 1 — Map feel.** An app with no data yet that pans, zooms and searches places like Apple Maps, online and offline.
3. **Phase 2 — One-click open.** Common files open with no wizard. Uncertain interpretations show as visible, changeable assumptions instead of questions.
4. **Phase 3 — Inspect and insight.** Click anything and see where it stands ("3rd hottest of 17") before choosing any tool.
5. **Phase 4 — Ask.** Up to three contextual actions, a sentence-style composer, previews, and result cards with a readable recipe.
6. **Phase 5 — Keep and share.** Save, recovery, export, a curated data library and a signed macOS build. Ends with the classroom pilot.
7. **Phase 6 — Harden.** Pilot fixes, Windows, full accessibility, more actions. Ends with the public beta.

This plan keeps the original specification's core loop (open → see → select → ask → preview → keep → share) and its correctness rules. It adds the five review fixes:
- Visible assumptions replace most questions, with a measured question budget.
- Insight appears on selection, before any action is chosen.
- Selecting one object is enough; a second layer is picked automatically.
- Map feel is specified in numbers and is the first gate.
- Gates are tiered (internal → pilot → beta), so a pilot can ship before full accessibility and cross-platform work.

**How to read a phase section:** Goal → Scope → Work packages (tick boxes) → Gate (automated, debug, your validation, stress) → Exit criteria → Rollback. Priorities are defined in *Priority model*. Fixture IDs such as **V3** or **R2** refer to *Test fixtures and stress tests*. Effort-weeks are planning estimates: one effort-week is about 20 focused hours with a coding agent.

## Planning assumptions
The plan rests on eight assumptions you have not yet confirmed. Each row says what changes if it is wrong. Revisit them at Gate 0, before any product code is written.

| Assumption | What the plan does | If it is wrong |
|---|---|---|
| **Builder:** you, directing an AI coding agent, about 20 focused hours a week; you review behavior, not Rust code | Phases are sized in effort-weeks. Every work package ends in something you can see or run. | Hired developer: phases compress about 40%. Under 8 hours a week: stop at Phase 5 and skip Phase 6 until there is help. |
| **First users:** your undergraduates at NYU Shanghai, in one course pilot | China-specific work is in the pilot: coordinate offsets at P0, the reachable basemap and Chinese interface at P1. | Users outside China: those items drop to P2, saving about 1.5 effort-weeks. |
| **First platform:** macOS on Apple silicon | Windows waits until Phase 6. Linux is best effort. | Many students on Windows: move Windows packaging into Phase 5 (+2 effort-weeks). |
| **Stack:** RustGIS engine crates plus a new egui front end | Phase 0 tests this before any commitment. | The rendering spike fails: switch to a GPU map layer inside egui, or to a Tauri + MapLibre shell (the GeoLibre approach). The choice is made at Gate 0. |
| **Upstream:** a GitHub fork; product code lives only in new crates | Upstream merges happen only at phase starts. Changes to upstream crates are small, labelled, and offered back as pull requests. | Maintainer welcomes a second front end upstream: develop there instead and drop the fork. |
| **Distribution:** signed and notarized macOS app for the pilot | Enroll in the Apple Developer Program (USD 99 a year) in Phase 0, so every CI build is signed and notarized. | Enrollment blocked: postpone the pilot. macOS blocks unsigned apps, and RustGIS's web build is RustGIS's interface, not LayeredMap's. |
| **AI:** none in the core through Phase 5 | Suggestions are deterministic rules. | You want natural-language questions earlier: add it as a Phase 6 experiment over the same action contract. |
| **Name:** LayeredMap is provisional | The code uses `layeredmap` and `lm-` prefixes. These are internal names that are cheap to change. | Choose the final name at Gate 5, before installers, domains and file associations exist. |

Inputs needed from you, and when:

| Input | Needed by | Used for |
|---|---|---|
| Apple Developer Program enrollment | Week 1 of Phase 0 | Signed, notarized builds at every gate |
| Your Mac's model, chip, memory and macOS version | Phase 0 | The reference machine in every gate report |
| The pilot course and term | End of Phase 0 | Calendar and scope |
| 15–20 real files your students would open (CSVs, Earth Engine exports, Shapefiles, Excel) | End of Phase 0 | Fixtures B1–B20, Gate 2 |
| Three questions a student should answer in the pilot, such as "Which district is hottest in July, and by how much?" | End of Phase 0 | Phase 4 action list, Gate 4 |
| IRB approval or exemption, if any test data may be published | Before the first hallway test | All student testing |
| Three students for hallway tests, recruited once for the term | Gate 0 | Gates 0–4 |
| The summer surface-temperature export (R1), plus a per-district July mean temperature field for teaching set K1 | Start of Phase 2 | Fixtures R1 and K1 |
| Ten Shanghai landmarks with their Amap coordinates | Start of Phase 2 | Fixture C2 |
| A Chinese-speaking student reviewer | Phase 5 | WP5.5 |
| Eight novice students outside the pilot course | Gate 5 | Formative usability round |

## Product principles
Twelve rules decide every design dispute in this plan. When two rules conflict, the one higher in the list wins. Rules marked *new* come from the review. The rest carry over from the original specification.
1. **A basic feature that sometimes lies is a release blocker.** Wrong numbers, silent record loss, silent unit or scope changes, and modified source files block every gate, pilot included.
2. **Sources are never modified.** Imported files are read-only. Removing a layer never deletes a file. Results are new layers.
3. **The map is the home screen.** The first window is a usable map. There is no wizard, account, project picker or empty toolbox.
4. **Feel is a feature** *(new)*. Pan, zoom, tile loading and label behavior meet the numeric targets in *Interaction specification → Map feel*. If the map stutters, the product fails, however correct it is.
5. **Make visible assumptions; ask only when it matters** *(new)*. The likely interpretation is applied, stated in one line, and can be changed in one click. A question appears only when confidence is low and a wrong guess would change numbers or locations. See *Assumptions and questions*.
6. **Insight comes before action** *(new)*. Selecting something first shows where it stands (rank, share, comparison to the median). Only then does it show what you can do with it.
7. **One selection is enough** *(new)*. Selecting a single layer or object can offer actions that involve another open layer, which is chosen automatically and can be changed. Multi-select is an expert path, never a requirement.
8. **Every action shows its scope.** "All 248 schools", "12 selected schools" or "Inside the drawn area" is always visible before and after running.
9. **Every result explains itself.** A result card states what was computed, from which inputs, over what scope, and what was excluded. The full recipe is one click away and can be copied as text.
10. **Show at most three suggested actions, in a stable order.** The same selection and data always produce the same suggestions in the same order. "More actions…" is always in the same place.
11. **Technical words are findable, never required.** Searching "buffer" finds "Show an area within a distance." Primary labels use everyday words.
12. **Deterministic before AI.** Suggestions and calculations work with no model, account or network.

**Question budget** *(new)*. In novice sessions (one sitting with one task script, from launch to quit), the median number of question cards is at most 1 per session and the 90th percentile at most 3. The app keeps a local count of question cards. That count is a pass/fail number at Gates 2, 4 and 5.

## Priority model
Every function carries one of four priorities. Priority decides what gets cut, never what gets tested less. P0 items pass the same correctness checks in every phase.
- **P0 — Core.** Without it the product is not LayeredMap. It must be in the pilot and must pass every correctness test.
- **P1 — Pilot.** It must be in the pilot, but may ship reduced: fewer formats, fewer presets, partial translation.
- **P2 — Beta.** Required for the public beta, not for the pilot.
- **P3 — Later.** Built only when pilot or beta users show that the value is worth the added complexity.

**Cutting rule.** When a phase runs late, cut or reduce its P1 items first. P0 scope is never cut to make a date. If a P0 item cannot pass its gate, the gate fails and the rollback rules apply.

| Area | Function or module | Priority | Phase |
|---|---|---|---|
| Map | Map canvas: inertial pan, smooth zoom, tile placeholders, label fade | P0 | 1 |
| Map | Bundled offline world basemap (Natural Earth; China point-of-view boundaries in builds distributed in China) | P0 | 1 |
| Map | Online detailed basemap, Light and Dark, reachable from mainland China | P0 | 0 (host trial), 1 (client), 5 (production) |
| Map | Offline place search (bundled city index) | P1 | 1 |
| Map | Top bar, zoom controls, scale bar, attribution, basemap chooser | P0 | 1 |
| Map | Light and dark themes | P0 | 1 |
| Platform | Test fixture factory and reference scripts | P0 | 0 |
| Platform | Signed and notarized macOS builds from CI | P1 | 0 |
| Platform | Phase feature flags (`min_phase`, `LM_PHASE`) | P0 | 1 |
| Open | Import coordinator: Open button, drag and drop, Open With | P0 | 2 |
| Open | GeoJSON, Shapefile (also zipped), CSV/TSV with coordinates, GeoTIFF/COG | P0 | 2 |
| Open | GeoPackage and Excel | P1 | 2 |
| Open | Layer profiles and visible-assumption badges | P0 | 2 |
| Open | "Where should this data be?" coordinate-system resolver | P1 | 2 |
| Open | GCJ-02 / BD-09 offset notice and conversion (P2 if users are outside China) | P0 | 2 |
| Open | Import report with an inspectable list of excluded records | P0 | 2 |
| Open | Layer shelf: visibility, reorder, rename, remove, status badges | P0 | 2 |
| Open | Minimal layer overview: header, assumption lines, Details tab | P0 | 2 |
| Open | Undo core over document transactions | P0 | 2 |
| Open | Document v0: save and load for tests (developer menu) | P0 | 2 |
| Inspect | Hit testing, selection scopes, overlap chooser | P0 | 3 |
| Inspect | Object card and full layer overview | P0 | 3 |
| Inspect | Insight strip: rank, share, comparison to the median | P0 | 3 |
| Inspect | Field roles (identifier, category, measure) with one-click correction | P0 | 3 |
| Inspect | Geodesic length and area | P0 | 3 |
| Inspect | Style panel, presets, automatic legend | P0 | 3 |
| Inspect | Action registry skeleton for display-only actions | P0 | 3 |
| Inspect | Search your data: layer names and text fields | P1 | 3 |
| Inspect | Data table drawer with linked selection | P1 | 3 |
| Inspect | Filter chips and sentence-style filter builder | P1 | 3 |
| Inspect | Undo for style gestures (one step per gesture), 100 steps deep | P0 | 3 |
| Ask | Full action registry and deterministic relevance engine | P0 | 4 |
| Ask | Sentence composer, preview, keep | P0 | 4 |
| Ask | Job system: background work, cancel, stale-result guard, atomic commit | P0 | 4 |
| Ask | Result cards and copyable recipes | P0 | 4 |
| Ask | Area drawing: polygon and rectangle | P0 | 4 |
| Ask | Actions: count in areas, which area contains this, what is inside, area within a distance, measure, rank by a value, add raster values to points | P0 | 4 |
| Ask | Action: summarize a raster in areas (fractional pixel coverage, area-weighted) | P0 | 4 |
| Ask | Action search (⌘K) with expert aliases | P1 | 4 |
| Keep | Save and autosave (full format) | P0 | 5 |
| Keep | Crash recovery and relinking missing files | P1 | 5 |
| Keep | PNG export with legend, scale bar, attribution, caption | P0 | 5 |
| Keep | Data export (GeoJSON, CSV, GeoTIFF) with field and privacy review | P1 | 5 |
| Keep | Curated data library for the course | P1 | 5 |
| Platform | English and Simplified Chinese interface | P1 | 5 |
| Platform | Full keyboard operation of core tasks | P1 | 5 |
| Ask | Actions: match a table to a layer, nearest feature, keep data inside an area | P2 | 6 |
| Open | KML/KMZ and GPX | P2 | 6 |
| Keep | PDF export and portable map package | P2 | 6 |
| Platform | Screen-reader support (VoiceOver, then Narrator) | P2 | 6 |
| Platform | Windows build | P2 | 6 |
| Platform | Strict offline mode that blocks all network requests | P2 | 6 |
| Ask | Raster comparison: side by side, swipe, change | P3 | After 6 |
| Ask | Natural-language "Ask about this layer" | P3 | After 6 |
| Platform | Plugins, 3D, routing, live collaboration | P3 | After 6 |

## Architecture
LayeredMap adds eight new crates and one app on top of seven RustGIS engine crates. The new interface reaches the engine only through the action and job layers, so the UI never does spatial math. RustGIS's ribbon front end is not used.

> *Diagram: module map · 3 groups, 12 modules — see the original document for the drawing.*

The only direct line from the interface to RustGIS is the map view's drawing path. Everything that computes goes through product logic.

| Module | Job | Depends on |
|---|---|---|
| `apps/layeredmap` | App entry, window, native menu bar (via the `muda` crate), file associations, packaging | `lm-ui` |
| `lm-ui` | The map-first shell: top bar, layer shelf, inspector, composer, result cards, table drawer, theme tokens | `lm-core`, `lm-actions`, `lm-jobs`, `lm-doc`, `lm-mapview` |
| `lm-mapview` | Map widget: camera, gestures, inertia, tile placeholders, hit testing, selection highlight | `rustgis-map`, `rustgis-tiles`, `rustgis-render` |
| `lm-core` | State model and data contracts (below). No UI, no file or network I/O | — |
| `lm-profile` | Import-time profiling: field roles, coordinate-system confidence, NoData, units, China offset heuristics, assumption generation | `lm-core`, `rustgis-data`, `rustgis-geom` |
| `lm-actions` | Action registry, eligibility checks, relevance ranking, per-phase feature flags | `lm-core`, `lm-profile` |
| `lm-jobs` | Plans, previews, background execution, cancellation, stale-result guard, atomic commit, compound undo | `lm-core`, `lm-analysis`, `rustgis-engine`, `rustgis-analysis` |
| `lm-analysis` | Certified algorithms where upstream lacks the needed meaning: fractional-coverage zonal summary, boundary-aware point counts, geodesic buffers, GCJ-02/BD-09 conversion | `rustgis-geom`, `rustgis-data` |
| `lm-doc` | Document format, autosave, recovery journal, source fingerprints, relinking | `lm-core`, rustgis-data (to write FlatGeobuf and GeoTIFF) |

`lm-mapview` starts as an extraction of the map view inside RustGIS's `ui-egui` crate. It is the first candidate to offer upstream as a pull request, because any second front end needs it.
**Rules enforced in CI.** Extend RustGIS's `cargo xtask layers` check to cover the new crates.
- Below `lm-ui`, only `lm-mapview` may depend on egui, and nothing depends on eframe or rfd.
- `lm-ui` never calls `rustgis-data` or `rustgis-analysis` directly. All work goes through `lm-jobs`.
- Numbers never come from the renderer. Counts and means are computed from data, never from drawn pixels or visible symbols.
- Every `lm-` crate except `lm-ui`, `lm-mapview` and the app builds for `wasm32-unknown-unknown`, matching upstream's rule. This keeps a future web version possible; it is not a tested fallback.
- Upstream rules carry over: no `unwrap()`, `expect()`, `panic!` or `unsafe` outside tests, and every interface string is wrapped for translation. One exception: unsafe code is allowed in the app's macOS module for native macOS hooks (Open With, Dock drops, the Edited state), with a SAFETY comment on each block and a second review.

**State model.** There are three stores with one update loop.
- **Document state** (saved): layers with source references, profiles and styles; results and recipes; notes; the saved view.
- **Interaction state** (not saved): the selection context, camera, open panel, composer draft.
- **Execution state** (memory only): running jobs, progress, temporary previews.

The UI emits an *intent*. A reducer either updates interaction state or starts a job. A finished job returns an event, and a single transaction commits its result into document state. The undo stack records transactions, so one user action is one undo step.
**Data contracts.** These are sketches for the agent to refine, not final signatures.

```rust
// Every inferred property carries how sure we are and why.
pub struct Known<T> { pub value: T, pub confidence: Confidence, pub evidence: Evidence }
pub enum Confidence { Certain, Likely, Possible, Unknown }
pub enum Consequence { DisplayOnly, ChangesNumbers, ChangesLocations }

pub struct LayerProfile {
    pub layer_id: LayerId,
    pub source: SourceRef,                 // path + fingerprint (size, mtime, optional hash)
    pub kind: LayerKind,                   // Points | Lines | Areas | Raster | Table
    pub count: u64,                        // features or cells
    pub extent_wgs84: Option<Extent>,
    pub crs: Known<Crs>,
    pub fields: Vec<FieldProfile>,
    pub bands: Vec<BandProfile>,
    pub assumptions: Vec<Assumption>,
}
pub struct FieldProfile {
    pub name: String, pub dtype: DataType,
    pub role: Known<FieldRole>, pub unit: Known<Option<Unit>>,
    pub stats: Option<FieldStats>,         // min, max, median, distinct count, null count
    pub rank_index: Option<RankIndex>,     // sorted values, for O(log n) ranks in insight cards
}
pub enum FieldRole { Identifier, Category, Measure(MeasureKind), Text, Date, Unknown }
pub enum MeasureKind { Intensive /* e.g. temperature */, Count, Density, Unknown }

pub struct Assumption {
    pub id: AssumptionId, pub subject: Subject, pub chosen: Choice,
    pub alternatives: Vec<Choice>, pub confidence: Confidence,
    pub consequence: Consequence, pub confirmed_by_user: bool,
}
pub struct SelectionContext {
    pub active_layer: Option<LayerId>, pub features: Vec<FeatureRef>,
    pub layers: Vec<LayerId>, pub filter: Option<FilterId>, pub area: Option<AreaId>,
}
pub struct ActionSpec {
    pub id: &'static str, pub label_key: &'static str, pub aliases: &'static [&'static str],
    pub inputs: Vec<InputRole>, pub requires: Vec<Requirement>, pub decisions: Vec<Decision>,
    pub preview: PreviewPolicy, pub output: OutputContract, pub min_phase: u8,
}
pub struct WorkflowPlan {
    pub action: ActionId, pub inputs: Vec<BoundInput>, pub scope: Scope,
    pub decisions: Vec<(DecisionId, Value)>, pub assumptions: Vec<AssumptionId>,
    pub generation: u64,                   // a newer plan makes older job results stale
}
pub struct ResultArtifact {
    pub id: ResultId, pub plan: WorkflowPlan, pub layer: Option<LayerId>,
    pub summary: ResultSummary, pub recipe: Recipe,
    pub warnings: Vec<Warning>, pub source_versions: Vec<Fingerprint>,
}
```

## Visual design system
The interface is quiet, neutral and solid. Floating panels sit over a full-bleed map, with one accent color and system fonts. Data uses a short curated palette set. Every value below is a starting token: Phase 1 implements it and Gate 1 checks it.

### Layout
- **Window:** default 1280 × 800 logical px; minimum 1024 × 640. Below the minimum, the window does not shrink further.
- **Map:** full-bleed under all chrome. Panels never push the map, so its center stays put when panels open or close.
- **Toolbar:** 52 px, full width, solid surface. The search field (280–480 px wide) sits left of center, with Open, Layers, Undo and Share at the right.
- **Layer shelf:** a floating panel, 280 px wide, 12 px from the window edges, with 12 px corner radius. Below 1180 px window width it collapses to a Layers button that opens it as a drawer.
- **Inspector:** a floating panel, 360 px wide, 12 px from the right edge. Its height fits its content, up to the window height minus 76 px, and it scrolls internally.
- **Map controls:** bottom right, stacked: zoom in, zoom out, basemap. Each has a 36 × 36 px hit area. When the inspector is open, the controls move left of it.
- **Scale bar** sits bottom left; **attribution** bottom right at 12 px text, never hidden.
- **Table drawer:** docked at the bottom, between the shelf and the inspector. It opens at 35% of window height (minimum 160 px) and is resizable.
- **Preview banner:** a pill centered 12 px below the toolbar, shown only while a preview exists.

### Color tokens
UI color and data color are separate systems. The accent never appears in a data palette.

| Token | Light | Dark | Used for |
|---|---|---|---|
| `surface` | #FFFFFF | #1E1F22 | Panels, toolbar |
| `surface-raised` | #F6F7F9 | #2A2B2F | Cards inside panels, table header |
| `border` | #E3E5E8 | #3A3C41 | Panel and card outlines, dividers |
| `text-primary` | #1B1D21 | #F1F2F4 | Body text, values |
| `text-secondary` | #5F6670 | #A9AEB6 | Captions, units, scope lines |
| `accent` | #2563EB | #5B8DEF | Primary buttons, selection outline, focus ring. Text on accent: #FFFFFF in light, #0B1220 in dark. |
| `accent-weak` | #DCE7FD | #24324F | Selected rows, active tokens in the composer |
| `assumption` | #6D4FD1 | #A78BFA | The dot that marks a visible assumption |
| `warning` | #B45309 | #F59E0B | Needs a decision; partial coverage |
| `critical` | #B91C1C | #F87171 | Failed import or job |
| `success` | #15803D | #4ADE80 | Import complete; job finished |

Gate 1 verifies contrast: body and secondary text at least 4.5:1 against their surfaces, icons and focus rings at least 3:1.

### Type, spacing, shape
- **Fonts:** bundled on every platform: Inter for Latin text and Noto Sans SC for Chinese, as static Regular and SemiBold files. egui does not apply OpenType features and cannot reliably pick weights from variable system fonts, so Inter is rebuilt once with tabular figures as its default (a feature-freezing tool such as pyftfeatfreeze does this). System fonts serve only as a fallback for rare characters.
- **Sizes** (size/line height, px): caption 12/16, body 14/20, label 14/20 semibold, headline 16/22 semibold, title 20/26 semibold, big number 28/32 semibold. All numbers use tabular figures so columns align.
- **Spacing scale:** 4, 8, 12, 16, 24, 32. Panel padding is 16; gaps between cards are 12.
- **Radius:** 6 for controls, 8 for cards, 12 for panels.
- **Hit targets:** at least 36 × 36 for icon buttons and 32 px tall for text buttons and list rows.
- **Icons:** the Phosphor set (MIT license) through an egui icon-font crate. Regular weight, 18 px in the toolbar, 16 px in panels.
- **Elevation:** panels get one soft shadow (0 2 8, 12% black in light; none in dark, which uses the border instead). There is no translucency or blur over the map.

### Data palettes
- **Categorical:** Okabe–Ito, 8 colors, colorblind-safe. More than 8 categories shows the 7 largest plus "Other".
- **Sequential (default for measures):** viridis. Alternatives: cividis; "Heat" (ColorBrewer YlOrRd), offered first when the unit is a temperature; "Blues" for counts.
- **Diverging:** ColorBrewer RdBu, offered only when a center with a stated meaning is set (zero, the median, or a value you type). The legend labels the center.
- **Forbidden defaults:** rainbow and jet ramps, 3D bars, more than 7 classes for a classed map.
- **Missing data:** light gray #D0D3D8 with a diagonal hatch, always labelled "No data" in the legend.

### Default symbols
- **Points:** 7 px circles with a 1.5 px white edge. Above 10,000 points, 4 px with no edge. Above 200,000, the display thins points and shows a note; analysis always uses every record.
- **Lines:** 2 px.
- **Areas:** fill at 35% opacity with a 1 px outline in a darker shade of the fill.
- **Selected:** 3 px accent outline plus a 2 px halo (white in light, black in dark). Selected points grow 1.4×.
- **Hover:** a 1 px brighter outline and a pointer cursor. Hover never opens a card.
- **Preview:** a 45° stripe pattern in the accent at 35% opacity over the preview geometry, plus the preview banner.

### Motion

| Element | Duration | Easing |
|---|---|---|
| Panel opens / closes | 180 ms / 140 ms | ease-out |
| Card content changes | 120 ms | crossfade |
| Zoom step (button, double-click, keyboard) | 250 ms | ease-out cubic |
| Fly to a search result | 600–900 ms, scaled by distance | ease-in-out |
| Tile and label appearance | 150 ms | fade |

When the operating system's Reduce Motion setting is on, flights become instant jumps with a 100 ms fade, and panels appear without sliding. Any camera animation stops at the first new input.

### Microcopy rules
- Use sentence case everywhere. Buttons start with a verb: "Open file", "Keep result".
- Numbers carry units and thousands separators: "1,248 schools", "31.2 °C", "4.3 km²".
- No exclamation marks, no ALL CAPS and no blame ("This file does not say…", never "You forgot…").
- Technical terms appear only in Details, Help and search aliases.

## Interaction specification
Every surface below defines its trigger, its contents in order, and how it closes. Phase sections build these surfaces and cite the subsection names. A behavior not written here is not built until it is added here first.

> *Diagram: workspace mock-up · one district selected — see the original document for the drawing.*

The insight rows answer "where does this stand?" before any tool is offered. The three actions come from row 8 of the suggested-actions table, because a raster is open.

### Screen states

| State | Trigger | What the user sees | Exit |
|---|---|---|---|
| First launch | App opens with no saved view | World map on the offline basemap. A welcome card at bottom center: "Open a file to see your data on the map." Buttons: **Open file…**, **Try an example**, and from Phase 5 **Browse data library**. | First successful import, or the card's close button |
| Importing | File chosen or dropped | A shelf row with a spinner and progress text ("Reading 1.2 of 3.4 MB"). The map stays interactive. | Complete, needs a decision, or failed |
| Loaded | Any layer present, nothing selected | Map, shelf; the inspector is closed | — |
| Object selected | Click an object | Selection highlight plus the object card | Click empty map, or Esc |
| Layer selected | Click a shelf row | Layer overview in the inspector | Esc, or select something else |
| Composer | Click a suggested action | The composer replaces the lower inspector; the header stays | Cancel, Esc, or Keep result |
| Preview | Click Preview | Striped result on the map, the preview banner, a summary in the composer | Keep result, Change, or Discard |
| Result kept | Keep result | A new layer nested under its input; the result card in the inspector | — |
| Table open | View data, or ⌘T | The table drawer | Close button, or ⌘T |
| Export | Share → Map image… | A full-window sheet showing the exact output | Save, or Cancel |
| Offline | Network lost | One notice: "Detailed background unavailable offline. Your data and tools still work." The basemap falls back. | Network returns |

### Map gestures and keys

| Input | Result |
|---|---|
| Drag on the map (mouse, or trackpad click-drag) | Pan; inertia on release |
| Two-finger trackpad scroll | Pan, as in Apple Maps |
| Pinch | Continuous zoom around the pinch center |
| Mouse wheel | Zoom around the pointer, 0.5 zoom levels per notch, animated |
| Double-click / ⌥ double-click | Zoom in / out one level, centered on the click |
| Click an object / click empty map | Select it / clear the object selection (the active layer stays) |
| ⇧-click | Add or remove an object from the selection |
| Right-click an object | Menu: the same suggested actions, View in table, Copy coordinates |
| Arrow keys / + and − (map focused) | Pan a quarter of the view / zoom one level |
| ⌘0 | Zoom to all data |
| Rotate or tilt gestures | Ignored. The map stays north-up in version 1. |

### Map feel
These targets are measured on the reference Mac at 2× display scale, with the online basemap, 100,000 points and 10,000 polygons loaded (fixture **F1**). They are the Gate 1 exit criteria.

| Behavior | Target | How it is checked |
|---|---|---|
| Frame time while panning and zooming | 95th percentile ≤ 16.7 ms (60 fps) as the goal; ≤ 33 ms as the hard limit | Scripted camera paths in the performance harness |
| Input to first visible motion | Within one frame | Harness timestamps |
| Inertia | Release speed matches drag speed; motion stops within 300–600 ms | Harness plus your feel test |
| Wheel zoom | Animated over 250 ms; the point under the pointer drifts by at most 1 px | Harness |
| Pinch zoom | Continuous, with no snapping to whole zoom levels | Your feel test |
| Missing tiles | Never blank. Show the nearest loaded parent tile (up to 4 levels up) or the offline basemap; new tiles fade in over 150 ms. | Screen recording on a throttled network |
| Labels | No flicker while panning. With vector tiles, placement is recomputed at most every 150 ms while zooming, with 150 ms fades. With pre-rendered raster tiles, labels are baked in and tiles crossfade. | Screen recording |
| Data layers | Never disappear mid-gesture. They are transformed during the gesture and redrawn sharply after it settles. | Screen recording |
| UI thread | No file or network I/O on it; any stall over 50 ms is logged | Instrumented build |

### Toolbar
- **Search** (⌘F). Placeholder: "Search places or your data". Results fall in two groups:
    - **Your data** matches layer names and values in each layer's first five text fields. Choosing a result selects the object and opens its card.
    - **Places** come from the bundled city index (an online provider is optional later). Choosing a place flies the map there.
    - The last row, "Find an action…", switches to action search (⌘K).

- **Open** (⌘O). A file dialog with multi-select.
- **Layers.** Shows or hides the shelf.
- **Undo.** Its tooltip names the step ("Undo Count schools in each district"), and a caret lists the last 10 steps. Redo is ⇧⌘Z and in the Edit menu.
- **Share.** A menu: Map image…, Data for another app…; from Phase 6 also PDF… and Portable map…

### Layer shelf
- **Row anatomy** (32 px tall): visibility eye, style swatch, display name (truncated in the middle), status badge.
- **Badges:**
    - purple dot: a visible assumption (opens Details → Assumptions)
    - amber triangle: needs a decision
    - spinner: working
    - red circle: failed
    - broken link: source file missing

- **Click** the row to select the layer. **Click the eye** to toggle visibility only. **Double-click** the name to rename its display alias. **Drag** to reorder, with an insertion line.
- **Right-click:** Zoom to layer, Rename, Style…, View data, Remove from map.
- **Nesting:** results nest under their main input, indented 16 px, with a "Result" tag. A file with several layers (a GeoPackage) becomes one collapsible group.
- **Dropping files:** a "+ Open file" row closes the list. Dragging files over the window shows a full-window overlay: "Drop to add to the map".

### Inspector: object card
1. **Header:** the object's name (the first text field whose role is Name; otherwise layer name plus number, "District 7"). Subtitle: layer, kind and size ("Districts · area · 23.4 km²"), measured geodesically.
2. **Insight strip:** up to three rows (see *Insight cards and suggested actions*).
3. **Facts:** up to six fields in source order, skipping identifiers and long text, then "Show all 24 fields".
4. **Suggested actions:** up to three full-width buttons, then "More actions…", always last.
5. **Footer:** View in table · Zoom to.

**Overlap chooser.** When a click hits more than one object within 4 px, a small popover appears at the click: "3 objects here". It lists each object with its layer swatch, topmost first. Hovering a row highlights that object; clicking selects it.

### Inspector: layer overview
Tabs across the top: **Overview · Style · Details**. The Overview tab shows, in order:
1. **Header:** the display name; subtitle "248 points · schools.csv".
2. **Assumptions:** up to two lines, each with a purple dot and a Change link, such as "Located using lon and lat as GPS coordinates (WGS 84). Change". More appear as "+1 more".
3. **Legend** for the current style, plus a Style button.
4. **Field summary:** up to five measure fields, each with its range and a 60 px histogram (20 bins).
5. **Suggested actions** (three), then More actions…

Details holds the technical record: source path, format, coordinate system, extent, every field with its role and evidence, and all assumptions.

### Composer
The composer replaces the lower inspector; the card header stays visible.
- **Sentence:** for example, "Count **Schools** in each **District**". Bold parts are tokens with an `accent-weak` background. Clicking a token opens its picker: a layer, a distance with units, a field, or a summary measure.
- **Scope line:** "All 248 schools · all 17 districts", with Change: all, selected, inside a drawn area, or filtered.
- **Notices** appear inline. Amber means a decision is needed, for example: "12 schools sit on a shared boundary. Count them in: [both districts] [one district, flagged for review]". Purple means an assumption is in effect.
- **Buttons:** **Preview** (primary, Return) and **Cancel** (Esc). "More details" discloses method parameters and their technical names.
- **After a preview:** a summary appears above the buttons ("17 districts · 0–34 schools · 2 with none"). The buttons become **Keep result**, **Change** and **Discard**.
- **Editing a token:** the preview refreshes automatically when its estimated cost is under one second. Otherwise the banner shows "Preview out of date — Preview again".

### Preview
- The banner reads "Preview — not saved" and has Keep and Discard buttons.
- Preview geometry is striped, and its legend says "Preview".
- If the preview was reduced in any way, the banner says so: "computed for the current view only".
- Previews never appear in saved documents or exports.

### Result card
1. **Title:** "Schools in each district".
2. **Answer.** A ranking shows the top 5 as horizontal bars, then "Show all 17". A single-area summary shows a big number with its unit and coverage: "31.2 °C average, over 83% of this district; the rest has no data."
3. **Context lines:** inputs ("Schools, 248; Districts, 17"), scope, and exclusions ("6 schools have no location — Review").
4. **Buttons:** Show on map · View table · Change question · Export.
5. **"How this was calculated"** discloses the recipe as plain text, with a Copy button.

### Table drawer
- **Header:** layer name, "Showing 12 of 248", filter chips, + Filter, a search box, Close.
- **Columns:** source order, with identifier columns pinned left. Numbers are right-aligned in tabular figures. Blank cells show "—" in secondary color, so blank and zero stay distinct.
- **Selection** is shared both ways: selecting rows selects objects, and selecting objects highlights rows. Clicking a header sorts the view without changing the data.
- **Scale:** rows are virtualized; 1,000,000 rows scroll at 60 fps.

### Filter builder
- **Sentence:** "Show [schools] where [Type] [is] [Primary]".
- **Conditions by type:**
    - text: is, is not, contains, is empty
    - number: above, below, between, is empty
    - date: before, after, between
    - category: is any of

- **Live count:** "42 match", updated as you edit.
- **Applying** turns the filter into a chip ("Type is Primary ×").
- **Keep as new layer** asks: Live view (follows the source) or Snapshot.

### Keyboard
- **Files and history:** ⌘O open, ⌘S save, ⌘Z undo, ⇧⌘Z redo, ⌘E export a map image.
- **Finding:** ⌘F search, ⌘K find an action, ⌘L layer shelf, ⌘T data table, ⌘0 zoom to all data.
- **Moving between regions:** Tab and ⇧Tab follow toolbar → shelf → map → inspector → drawer. F6 jumps between regions.
- **Return** triggers the panel's primary button.
- **Esc** works as a ladder, one step per press:
    1. Close an open popover or menu.
    2. Cancel a drawing in progress.
    3. Discard a preview (with a confirmation if it took more than 10 s to compute).
    4. Close the composer.
    5. Clear the object selection.
    6. Close the inspector.

    Esc never cancels a running job; that takes the job's own Cancel button.

### Error and status copy

| Situation | Message | Actions |
|---|---|---|
| No coordinate system | "This file doesn't say where on Earth its coordinates are. Your file has not been changed." | Find location… · Open as table · Details |
| Unsupported format | "LayeredMap can't open .dwg files yet." | Which files work · Details |
| Partial import | "248 records opened; 6 have no usable location." | Review 6 · Export problem list |
| Job failed | "The summary could not finish. Nothing was added to your map." | Try again · Details |
| Disk full | "There isn't enough free space to keep this result. Your files are unchanged." | Choose another location · Cancel |
| Missing source on reopen | "Can't find heat_july.tif. It was last seen in Downloads." | Locate… · Remove layer |

Errors never show stack traces. The Details pane shows the technical message, which can be copied.

## Assumptions and questions
Every uncertain interpretation gets one of four treatments. Two things decide which: how sure the app is, and what a wrong guess would change. Most cases end up as a visible assumption, not a question. That is how the app stays fast without becoming dishonest.

> *Diagram: assumption grid · 4 confidence levels × 3 consequences — see the original document for the drawing.*

Only the three amber cells cost a question, and only the red cell stops work. The catalog's Treatment column applies this grid; its Detection column is the evidence that sets the confidence.
**The four treatments**
1. **Silent.** Applied and recorded in Details. Used when the evidence is certain, or when a wrong guess changes only appearance.
2. **Visible assumption.** Applied immediately and stated in one line with a purple dot and a Change link, on the layer overview and in any recipe that used it. It never blocks and does not count against the question budget.
3. **Question card.** Asked once, with a recommended answer preselected and a **Not sure** option. It counts against the budget.
4. **Block.** The action is not suggested. In More actions it appears with the reason and what would unblock it.

**Ask at the moment of consequence, not at import.** Questions about *where* data is (coordinate system, coordinate columns) come at import, because nothing can be drawn without the answer. Questions about *what values mean* come only when an action first needs the answer. Opening a raster never asks whether its cells are totals or averages; the first "Summarize in areas" does.
**The Not sure path.** Choosing Not sure applies the safest interpretation and narrows what is offered. It never dead-ends.
- **Cell meaning:** summaries show "average cell value" only, so labelled, and totals are hidden. The layer shows: "Treated as measured values because the meaning is unconfirmed. Totals are hidden. Change".
- **Location:** the file opens as a table, and "Find location…" stays available in its overview.
- **China offset:** points stay where they are, and the offset notice stays on the layer until it is dismissed or converted.

### Assumption catalog
These are the cases Phase 2 and Phase 4 implement. A new case is added here before it is coded.

| Situation | Detection | Treatment | Line shown to the user |
|---|---|---|---|
| CSV with lon/lng/longitude/x/经度 and lat/latitude/y/纬度 columns | Column names match, values are in range, and at least 95% of rows parse | Visible | "Located using lon and lat as GPS coordinates (WGS 84). Change" |
| CSV with ambiguous coordinate columns | Two candidate pairs, or latitude values above 90 | Question at import | "Which columns locate these records?" Each candidate pair is shown as a small map preview. |
| No coordinate system; values within ±180 / ±90 | Value ranges | Visible | "Assumed GPS coordinates (WGS 84), because the file doesn't say. Change" |
| No coordinate system; projected-looking values (e.g. 350,000 / 3,450,000) | Value ranges | Question at import | "Where should this data be?" (see below) |
| Points inside mainland China, assumed WGS 84 | The extent falls inside China's boundary | Visible, with Compare | "If these came from a Chinese map app (Amap, Tencent or Baidu), they may sit a few hundred meters off. Compare" |
| As above, plus the name hints at the source | The file or column names contain gcj, amap, gaode, 高德, baidu, 百度 or bd09 | Question at import, conversion preselected | "These look like Amap (GCJ-02) coordinates. Convert to GPS coordinates?" |
| Raster without a NoData value | A spike of one extreme value (−9999, or the lowest 32-bit float value) concentrated at the edges | Visible | "Treated −9999 as No data. Change" |
| Raster with no unit | No unit in the metadata | Visible (in the legend) | "Unit unknown. Add unit" (in the legend) |
| Raster named lst, temp or ST_B10, with values between 250 and 340 | Name plus value range | Visible | "Treated as surface temperature in kelvin; shown in °C. Change" |
| Landsat Collection 2 ST_B10 stored as raw integers in the tens of thousands, with no scale metadata | Band name plus integer range | Visible | "Applied the Landsat Collection 2 surface-temperature scale and offset. Change" |
| Raster cell meaning | Decimals or negative values suggest a measured value. Integers plus a name containing pop or count suggest a total. | Visible if likely; otherwise a question at the first summary | "Treated as measured values (averaged, not summed). Change" |
| Field looks like an identifier | Unique per row, and integers, leading zeros, or a name ending in id/code/adcode/fips | Silent | In the field list: "Treated as an ID. Change". Excluded from insight and color suggestions. |
| Numeric field with 12 or fewer distinct values | Distinct-value count | Silent; noted in the Style panel | "Colored as categories. Change" |
| Shapefile text with no .cpg encoding file | UTF-8 decoding fails and GBK decoding yields valid Chinese | Silent; noted in Details | "Read text as Chinese (GBK). Change" |
| Excel workbook with several sheets | Only one sheet has coordinate columns | Visible | "Opened sheet 'Schools'. Change" |
| GeoJSON with mixed geometry types | Points, lines and areas in one file | Silent; the layer group shows the split | "Split into 3 layers: points, lines, areas" |

**"Where should this data be?"** This replaces asking a novice for an EPSG code.
1. The user types a place or clicks the map.
2. The app tries every coordinate system whose area of use covers that place. That includes UTM zones and China's Gauss–Krüger zones on CGCS2000, Xian 1980 and Beijing 1954.
3. It keeps the candidates that place the data's extent near the chosen point, and shows each as a thumbnail over the basemap.
4. The user picks the one that lands on the right streets.
5. The choice becomes a visible assumption that names the system, so an expert can check it. It also warns that the datum cannot be verified from the file, so positions may be off by up to about 100 m.

**Compare for China offsets.** Compare shows the original and converted positions together over a street-level basemap. Whichever set sits on the streets is right. One click keeps it.
**Question budget instrumentation.** The app writes a local log of question cards: timestamp, question type, answer chosen, seconds to answer. It never records data values or file names. The log can be exported for pilot analysis. Gates 2, 4 and 5 compare it with the budget: median at most 1 per session, 90th percentile at most 3.

## Insight cards and suggested actions
Selecting anything shows up to three insight rows immediately. Below them come up to three actions, chosen by the table in this section. Insight uses only attributes the layer already has, so it needs no question and no computation the user waits for.

### Insight rows
- **Which fields:** the field used for styling comes first, then fields the user starred, then other measure fields in source order. Three at most. Only fields whose role is Measure, at Certain or Likely confidence, qualify. Identifiers never do.
- **Measure row:** field name, the value with its unit, and a rank phrase. Beside them sits a 120 px dot strip: every value as a faint tick, this object as an accent dot, the median as a short line. A second line compares with the median: "+2.1 °C above the median".
- **Rank phrase:**
    - Upper half: "3rd highest of 17". Lower half: "4th lowest of 17". Ties: "tied 3rd highest".
    - Two or fewer objects: no rank.
    - 1,000 or more objects: a percentile instead, "higher than 82% of 1,248 schools".

- **Category row:** "Residential · 41% of districts are Residential".
- **Missing values:** shown as "No value". They are left out of the count, and the denominator says so: "of 16 with values".
- **Filtered layers:** ranks count only the filtered records: "of 12 filtered".
- **Raster pixel:** the decoded value with its unit, the band, and "higher than 82% of this layer's cells". Until full statistics finish in the background, the comparison carries "(estimated from a sample)".
- **Forbidden words in insight:** significant, cause, because, explains, better, worse.
- **Speed:** each measure field gets a sorted index in the background at import. A rank is a binary search, so the card appears instantly on click.

### Choosing the companion layer
When an action needs a second layer, the app picks one in this order:
1. A layer the user explicitly selected together with the first (the expert path).
2. Otherwise, the most recently added visible layer of a compatible kind.
3. Ties go to the higher layer in the shelf.

The companion appears as a token in the action's sentence, so it can be swapped. Hidden layers are never chosen.

### Relevance rules
1. Build the context: what is selected, the open layers and their kinds, field roles, confirmed units.
2. Find the matching row in the table below. Drop any action whose inputs, requirements or phase flag fail.
3. Show the first three remaining actions in the row's order. Everything else that is eligible goes under More actions, grouped as Understand, Compare, Measure, Change look, Keep.
4. Recompute only when the selection or the layer list changes. Never recompute on pan or hover, or when background profiling finishes while the inspector is open; queue that until the next selection.

The table is the specification. Phase 4 builds it as data (one file, `lm-actions/rules.toml`), so changing an order is a one-line edit with a snapshot test.

### Selection → suggested actions
Bracketed words are tokens. [Schools] stands for any point layer and [Districts] for any area layer; [Heat] stands for any raster.

| # | Selected | Other open layer | 1st | 2nd | 3rd |
|---|---|---|---|---|---|
| 1 | Nothing, no data | — | Open file | Try an example | Browse data library |
| 2 | One point | none | Show an area within a distance | Measure from here | Filter by [Type] |
| 3 | One point | area layer | Which [District] is it in? | Show an area within a distance | Measure from here |
| 4 | Point layer | none | Color or size by a value | Filter [Schools] | Show an area within a distance of each |
| 5 | Point layer | area layer | Count [Schools] in each [District] | Color or size by a value | Filter [Schools] |
| 6 | One area | none | Compare with other [Districts] | Measure its area | Filter by [Type] |
| 7 | One area | point layer | What's inside? | Compare with other [Districts] | Measure its area |
| 8 | One area | raster | Summarize [Heat] here | Compare with other [Districts] | Measure its area |
| 9 | Area layer | raster | Summarize [Heat] in each [District] | Color by a value | Rank [Districts] by a value |
| 10 | Area layer | point layer | Count [Schools] in each [District] | Color by a value | Rank [Districts] by a value |
| 11 | Area layer | none | Color by a value | Rank [Districts] by a value | Filter [Districts] |
| 12 | One line or line layer | — | Measure length | Color by a value | Show an area within a distance |
| 13 | One raster pixel | — | Show where values are like this | Change colors | Summarize in an area I draw |
| 14 | Raster layer | area layer | Summarize [Heat] in each [District] | Change colors and range | Show only values above… |
| 15 | Raster layer | none | Change colors and range | Show only values above… | Summarize in an area I draw |
| 16 | Table with no locations | layer with a shared code | Match to [Districts] by [code] *(Phase 6)* | Map using coordinate columns | View data |
| 17 | A drawn area | any | Summarize data here | Count [Schools] here | Measure its area |
| 18 | Point layer | raster | Add [Heat] values to each [School] | Color or size by a value | Filter [Schools] |

When two rows match, as when a point layer and a raster are both open, merge them. Take the 1st action of the row whose companion layer was added most recently, then the 1st action of the other row, then their first shared action. Drop duplicates. Each merged pair gets its own snapshot test.
**What the attribute-only actions do**
- **Compare with other [Districts]** expands the insight strip into a full ranking chart for one field.
- **Rank [Districts] by a value** creates a ranking result card.
- **Show where values are like this** highlights raster cells within a chosen band around the clicked value.
- **Show only values above…** applies a display threshold. None of these four creates new data unless the user keeps it. Likewise, Filter by [Type] opens the filter builder preset to that field and the selected object's value.

## Phase overview
The seven phases run in sequence, and each depends on the one before. A design track runs one phase ahead: you click through a mock of each phase's screens before it is coded. In total that is 28 effort-weeks, 22 of them before the pilot.

> *Diagram: roadmap · 7 phases, 7 gates, 28 effort-weeks — see the original document for the drawing.*

Each gate shows its most decisive criterion; the full list is in each phase section. The classroom pilot runs between Gate 5 and Phase 6.
**Calendar.** Effort includes gate work. Add 20% contingency when converting to dates. At 20 hours a week, the pilot build is about 26 calendar weeks after Phase 0 starts. Starting in November 2026, a Fall 2027 course pilot is realistic; a Spring 2027 pilot is not.
**Gate tiers**
- **Internal gates (0–4).** You validate in a 60–90 minute session. Three students join for 10-minute hallway tests; Gates 3 and 4 depend on them.
- **Pilot gate (5).** You, plus a formative usability round with eight novice students who will not be in the pilot course. Passing produces the pilot build.
- **Beta gate (6).** An acceptance round with 20 novice participants, on macOS and Windows.

**Design track.** Before a phase's code starts, the agent builds a clickable HTML mock of that phase's surfaces, taken directly from *Interaction specification*. You click through it. Two or three students do a 10-minute task each ("find the hottest district"). Anything that changes goes into the interaction specification first, then into code. This costs about two days per phase and is included in the effort numbers.
**Branches and tags.** Each phase works on a branch, `phase-N`. It merges into `main` only when its gate passes, and the merge is tagged `gate-N-pass`. That tag is the rollback point for the next phase. Upstream RustGIS is merged only at the start of a phase, never in the middle.

## Gate protocol
Every gate follows the same five steps and ends in one of three decisions: pass, fix forward within five working days, or roll back. A gate usually takes three to five working days. Each phase section lists only what is specific to its gate.

> *Diagram: gate flow · 5 steps, 1 decision, 3 outcomes — see the original document for the drawing.*

A fix forward reruns only the affected checks and your re-test. A rollback leaves `main` at the previous tag and keeps the failed branch for reference.

### The five steps
1. **Freeze.** Feature work on `phase-N` stops. The agent opens the gate report, `gates/gate-N.md`, from the template below.
2. **Automated checks.** Every one must be green. Unit tests, snapshots and screenshot checks run in CI on macOS. Hosted runners are small (limited disk and memory, a 6-hour job limit), so large fixtures and stress runs use cargo xtask stress on your Mac or a self-hosted runner, and fuzzing runs on Linux runners in 6-hour shards:
    - `cargo xtask ci`: formatting, clippy, tests, and the layering check
    - the phase's own test suites
    - the performance harness and screenshot regression
    - fuzzing, from Phase 2 on

3. **Debug pass.** The agent triages every failure, flaky test and open S1 or S2 issue. Then a fresh agent session, with no context from the build work, reviews all changes since the last gate. It looks for wrong numbers, data loss, and scope or unit labels that could mislead. Its findings go into the report.
4. **Your validation.** You run the phase's script on your own Mac. Use the build that CI produced, not one from the agent's machine.
    - Start from a fresh app state, with screen recording on.
    - Think aloud, and log every issue as it happens.
    - Rate each item in the script from 1 to 5.

5. **Stress tests.** Run the phase's stress list from *Test fixtures and stress tests*. Record each as pass or fail, with the measured number.

### Severity
- **S1 Blocker:** a wrong number; silent data loss; a source file modified; a hidden scope or unit change; a crash, or a freeze over 5 seconds, in a core task; a corrupted save.
- **S2 Major:** a core task cannot be finished without help; a map-feel target missed; the question budget exceeded.
- **S3 Minor:** a workaround exists; confusing wording; a visual glitch outside core states.
- **S4 Cosmetic.**

### Decision rules
- **Pass:** no S1 and no S2 issues, and every exit criterion met. Merge `phase-N` into `main`, tag `gate-N-pass`, and archive the signed build.
- **Fix forward:** every open S1 issue and at most three S2 issues have a clear fix that fits in five working days, and every numeric exit criterion is within 25% of its threshold. Yes/no criteria have no tolerance. Fix them on `phase-N` within five working days. Then rerun the affected checks, and re-test those items yourself.
- **Roll back** when any of these happens:
    - an S1 issue cannot be fixed within five days
    - there are more than three S2 issues
    - a numeric exit criterion is missed by more than 25%, or a yes/no criterion cannot be met within five days
    - an S2 issue returns after a fix forward

    `main` stays at `gate-(N-1)-pass`, and `phase-N` is kept read-only for reference. The agent writes a rollback note: root cause, what to change, a smaller scope. You approve the re-plan before Phase N restarts on a new branch, `phase-N-retry`.

### What makes rollback cheap
- **Code:** gate tags. A problem found after a merge is undone by reverting that phase's single merge commit.
- **Builds:** every passing gate's signed build and checksum go into a "Gate builds" folder, so you can always reopen the last good version.
- **Feature flags:** every surface and action declares `min_phase`. Launching with `LM_PHASE=3` hides everything from Phase 4 on. A broken action can be switched off without reverting code.
- **Documents:** the document format is versioned. Until Gate 5, a phase may only add fields. A document saved by Phase N must still open in Phase N−1, which ignores unknown fields. Fixture documents from every gate are re-opened in CI.
- **Your data:** the files and documents from every validation session are kept in `validation/gate-N/`, so the next gate can rerun the same session.

### Issue template

```markdown
**Gate / build:** gate-3 · 0.3.0-rc2 (CI run #)
**What I did:** clicked Districts, then the 2nd district from the top
**What I expected:** the insight row to rank Heat
**What happened:** it ranked district_code instead
**Severity (my guess):** S2
**Recording:** validation/gate-3/2026-12-03-session.mov at 12:40
```

### Gate report template

```markdown
# Gate N report — <phase name>
Build: <version> · commit <sha> · date

## 1. Automated checks      (all green? links to CI runs)
## 2. Debug pass            (failures triaged; independent review findings)
## 3. Owner validation      (script results, 1–5 ratings, issues by severity)
## 4. Stress tests          (fixture ID · measured value · threshold · pass/fail)
## 5. Exit criteria         (each criterion · met? · evidence)
## 6. Decision              Pass / Fix forward (issues + deadline) / Roll back (note link)
```

## Phase 0 — Foundation and risk spikes
**Goal:** before any product code, prove that the stack can feel like a consumer map, accept Chinese input and talk to a screen reader. Then set up the repository so every later phase can be gated and rolled back. **Effort:** 3 effort-weeks. **Scope:** no user-facing product. Spike code lives in `spikes/` and is thrown away.

### Work packages
**WP0.1 Upstream and repository** (P0)
- Open a discussion on opengeos/RustGIS describing the plan. Ask whether an extracted map-view crate, or a second front end, would be welcome upstream.
- Fork the repository and record the baseline in `UPSTREAM.md`: commit hash, date, workspace version.
- Add empty `lm-*` crates and `apps/layeredmap`. Extend `cargo xtask layers` with the rules in *Architecture*.
- Set up GitHub Actions on a macOS arm64 runner: `cargo xtask ci` on every push, and an `.app` bundle uploaded as an artifact. Enroll in the Apple Developer Program in week 1; CI then signs every build with a Developer ID, notarizes it with notarytool and staples the ticket, so every gate build opens without a security warning.
- Add the product rules to `AGENTS.md` (see *Working with coding agents*).

**WP0.2 Capability audit** (P0)
- Build and run RustGIS, and open every fixture in *Test fixtures* with it. Record each result in `docs/lm/capabilities.md`: opens, lands in the right place, fields correct, time to open.
- Inventory the analysis LayeredMap needs, recording for each whether it exists upstream, under which command ID, and how its results compare with a reference tool:
    - point-in-polygon counts (Summarize Within)
    - geodesic buffers and measurement
    - zonal summary, with and without fractional pixel coverage
    - coordinate transforms
    - GCJ-02 conversion (expected to be absent)

- Record whether rustgis-tiles and rustgis-render can draw the Protomaps style with Chinese labels, and how well. Find the map-view code inside `ui-egui` and estimate what extracting it into `lm-mapview` takes.

**WP0.3 Rendering spike — the decisive one** (P0)
- Build a minimal egui window in which the RustGIS renderer draws an online vector basemap (PMTiles) plus fixture **F1**.
- Add scripted camera paths with frame times logged to CSV: a 2,000 px pan, zoom in 6 levels, zoom out, and a continuous pinch.
- Measure three drawing strategies:
    - **(A)** Redraw everything every frame.
    - **(B)** Draw into cached textures, move and scale those textures during a gesture, and redraw sharply when it settles.
    - **(C)** Draw data layers, not the basemap, on the GPU (wgpu) through egui paint callbacks. Capped at 3 days.

- Write `docs/lm/adr-001-rendering.md` with the measured numbers, an estimate for each stack of the remaining effort to meet every Map feel target (labels, Pinyin input and VoiceOver included), and a recommendation.

**WP0.4 Fonts and Chinese input** (P0)
- Put an egui text field under macOS Pinyin input. Type 上海市徐汇区, mixed with English, and check the candidate window position, backspace and committed text.
- Bundle Inter and Noto Sans SC in Regular and SemiBold. Render mixed Chinese and English labels on the map and in panels, at 1× and 2×, and confirm that tabular figures line up.

- Spike Finder Open With and drops on the Dock icon through the app's macOS module.

**WP0.5 Accessibility probe** (P1)
- Enable AccessKit and turn on VoiceOver. Record whether it can reach and read five buttons, a text field and a list.

**WP0.6 Basemaps and reachability** (P1)
- Build a Shanghai-region extract of the Protomaps basemap (OpenStreetMap data, ODbL attribution) as a PMTiles file.
- Host it on two candidates: a global CDN, and a host with mainland-China edge nodes. Serving from mainland China under a custom domain requires an ICP filing (备案), which takes weeks. A Hong Kong region avoids the filing.
- From Shanghai, measure time to first tile and 95th-percentile tile latency on three connections: the campus network, home broadband and mobile data.
- Package Natural Earth 1:50m land, water, boundaries and populated places as the offline basemap, and record its size. Builds distributed in China use Natural Earth's China point-of-view boundary files.

**WP0.7 Design track** (P1)
- Build a clickable HTML mock of first launch, the loaded workspace, the object card with its insight strip, and the layer overview.
- Run three students through it, 10 minutes each, with one task each.

**WP0.8 Fixture factory** (P0, about 1 effort-week)
- Write `cargo xtask fixtures` to generate F1, F2, V6–V8, T3, R4, R5, R7 and X1–X5.
- Hand-make the small V, T, C and N fixtures, each with its expected output.
- Write the reference scripts in `reference/` (GeoPandas, exactextract, pyproj, rasterio) that produce the expected CSVs.
- Host the large fixtures in a pinned bucket with checksums, and record every license in `fixtures/LICENSES.md`.

### Gate 0
**Automated**
- CI is green on the empty crates.
- The layering check fails when a forbidden dependency is added on purpose. This tests the test.

**Debug**
- Every spike measurement repeats within 10% across five runs.

**Your validation (45 minutes)**
1. Install the spike build from CI, and use your Mac's trackpad.
2. Pan and zoom around Shanghai for 5 minutes, then do the same in Apple Maps. Rate 1–5 for each: pan, pinch zoom, wheel zoom, tiles appearing, labels.
3. Type 上海市徐汇区, then "Xuhui", in the test field.
4. Turn on VoiceOver (⌘F5) and move through the test buttons.
5. Read ADR-001 and approve a stack choice.

**Stress tests:** **F1** on a 4K external display at 2×; **F2** (1,000,000 points); **R4** (2 GB COG) preview time; tiles on a throttled 3G profile using Apple's Network Link Conditioner.
**Exit criteria**
- At least one drawing strategy keeps the 95th-percentile frame time at or below 33 ms on F1 at 2×, and shows no blank areas on the throttled network.
- Your ratings for pan and zoom are at least 4 of 5 with that strategy.
- Pinyin input works: the candidate window sits near the caret, and the committed text is correct.
- At least one basemap host reaches a 95th-percentile tile latency at or below 500 ms from campus.
- VoiceOver results are recorded. A failure here goes into the risk register with a plan; it does not fail Gate 0.

- ADR-001 estimates the remaining effort for each stack. The cheapest stack that meets the 33 ms limit wins.
- Signed, notarized CI builds open on a fresh Mac with no warning, and the fixture factory produces every fixture.

**Rollback**
There is nothing to roll back yet. If no strategy passes, change stack and repeat WP0.3 (+1 to 2 effort-weeks):
- **Option 1:** keep the egui shell and write a wgpu map renderer that reuses RustGIS data and analysis crates.
- **Option 2:** a Tauri + MapLibre GL shell, the approach of the GeoLibre project, calling RustGIS crates as Tauri commands.

## Phase 1 — Map feel and the empty shell
**Goal:** an app with no data yet that opens straight to a world map and pans, zooms and searches places like Apple Maps, online and offline. **Effort:** 3 effort-weeks. **Scope:** P0 map canvas, offline basemap and chrome; P1 online basemap client, place search and themes.

### Work packages
**WP1.1 App skeleton** (P0)
- Create a window that remembers its size and position.
- Add a native macOS menu bar via `muda`: LayeredMap, File, Edit, View, Window, Help. Items for features not built yet are hidden, not grayed out.
- Add phase feature flags: every surface and action declares min_phase, and launching with LM_PHASE=N hides everything from later phases. Then implement the color, type and spacing tokens from *Visual design system* as one `Theme` struct. It follows the system light or dark setting, with an override in the View menu.
- Load fonts as proven in WP0.4.

**WP1.2 Map view,`lm-mapview`** (P0)
- Extract or adapt RustGIS's map view. The camera supports fractional zoom, and vertical panning is clamped at the poles.
- Implement every row of *Map gestures and keys*: inertia, zoom around the pointer, double-click zoom, keyboard pan and zoom.
- Apply the rendering strategy chosen in ADR-001, with parent-tile placeholders and fades.
- Add a debug overlay (⌥⌘F) that shows live fps and 95th-percentile frame time.

**WP1.3 Basemaps** (P0 offline, P1 online)
- Ship the Natural Earth offline basemap as a "Quiet" style in light and dark. Beyond zoom 8 while offline, show "Detailed background unavailable offline".
- Define a provider record: `id, name, url_template, attribution, max_zoom, offline_allowed, export_allowed`. Providers can be swapped without changing the UI.
- Add two online styles from the Protomaps basemap on the host chosen in WP0.6: Light (default) and Dark. Draw them with RustGIS's vector-tile renderer if WP0.2 showed it handles the style and Chinese labels well. Otherwise serve them as pre-rendered @2x raster tiles with labels baked in, generated once. They are muted: gray roads, pale blue-gray water, labels at reduced contrast, so data stands out.
- Build the basemap chooser popover: thumbnails for Light, Dark and Offline, plus a "Background strength" slider (40–100%).
- Keep attribution always visible. Clicking it opens the full credits.

**WP1.4 Chrome** (P0)
- Build the toolbar. Search works. Open, Layers, Undo and Share are visible but disabled until the phase that gives them something to do.
- Add zoom buttons, the basemap button, a scale bar (metric; imperial in the View menu) and attribution.
- Build the welcome card from *Screen states* behind the Phase 2 flag. It first appears once Open file and Try an example work.

**WP1.5 Place search** (P1)
- Bundle a city index from GeoNames cities15000 (CC BY 4.0), with Chinese names taken from its alternate names. Expect about 3 MB.
- Search locally as you type, by prefix and with typo tolerance. Each result shows name, region and country. Choosing one flies there, at a zoom chosen from the place's population.

**WP1.6 Performance harness** (P0)
- Write `cargo xtask feel`. It runs the camera scripts and writes p50, p95, p99 and dropped frames to CSV.
- CI runners have virtual GPUs and noisy timing, so CI compares counters that do not vary between runs (draw calls, tiles decoded, frames over budget in a fixed replay) against the base commit in the same job. Absolute numbers come from your Mac at the gate.
- Add a developer-only menu item, "Load fixture…", so F1 can be tested before import exists.

**WP1.7 Screenshot regression** (P0)
- Follow upstream's `ui_shot` approach: render chrome states headlessly and compare them with approved images. States: first launch in light and dark, basemap chooser, search results.

### Gate 1
**Automated**
- CI is green, the feel harness shows no regression, and screenshot differences have been approved.
- A script measures cold launch time over 10 launches.

**Debug**
- Profile the worst camera script with `puffin`, the profiler egui supports. Fix the top three hotspots, or document why they stay.

**Your validation — the feel test (60 minutes)**
1. Install fresh from the CI artifact. Launch three times with Wi-Fi on and time how long the map takes to appear.
2. Search "Shanghai", "成都" and "Lisbon". Then zoom to your campus by hand.
3. Explore freely for 10 minutes with the trackpad, then with a mouse. Then repeat in Apple Maps. Rate 1–5 for each: pan, pinch, wheel zoom, double-click, tiles appearing, labels, overall calm.
4. Switch basemaps. Change System Settings to dark mode while the app is open.
5. Turn Wi-Fi off and keep exploring. Confirm the notice appears and the map never goes blank. Turn Wi-Fi back on, and confirm detail returns without restarting.
6. Resize the window to its minimum, then make it full screen. Drag it between your built-in display and an external monitor.

**Stress tests**
- **Zoom spam:** 50 wheel events per second for 10 seconds. Lag after the input stops is under 200 ms.
- **Resize storm:** resize the window continuously for 30 seconds.
- **Overnight idle:** leave the app open for 8 hours. Memory grows by at most 10%.
- **Bad networks:** browse on a throttled 3G profile, and cold-start with no network.
- **Large display:** F1 loaded through the fixture menu, on a 4K external display at 2×.

**Exit criteria**
- Every hard limit in *Map feel* is met. The 60 fps goal is met on at least 3 of the 4 camera scripts.
- Your ratings are at least 4 on every dimension.
- Cold launch to a usable map is at most 2 s (95th percentile). First online tiles appear within 1.5 s on campus.
- The map is never blank, in any network state.
- Every color token passes its contrast check.

**Rollback**
- If the feel failure is tuning, fix forward.
- If it is architectural, return to `gate-0-pass`, adopt ADR-001's alternative stack, and repeat WP0.3 before restarting Phase 1.

## Phase 2 — One-click open and visible assumptions
**Goal:** common files open with one action and land in the right place. Uncertainty shows as visible assumptions, and no record is lost without notice. **Effort:** 4 effort-weeks. **Scope:**
- **P0:** the import coordinator, core formats, profiles and badges, China offsets (under the China assumption), the import report, the layer shelf, a minimal layer overview, the undo core and document v0.
- **P1:** GeoPackage, Excel, the location resolver and example data.

### Work packages
**WP2.1 Import coordinator** (P0)
- Build one pipeline for every way in: the Open button, dropping on the window, dropping on the Dock icon, and Finder's Open With. Register document types for Open With, but never claim to be the default app.
- Run the import stages off the UI thread, each reporting progress to its shelf row: detect → read metadata → extent → profile fields or bands → first draw → ready. Rank indexes and full statistics continue in the background.
- Make a multi-file drop one batch. With no data yet on the map, fit to the first extent; otherwise show "Zoom to new data".
- Let each row be canceled (× on hover). A canceled import leaves nothing behind.
- Never load a whole file into memory just to learn its name or bounds. COGs use their overviews; GeoJSON and CSV are scanned as streams.

**WP2.2 Formats** (P0: GeoJSON, Shapefile, zipped Shapefile, CSV/TSV, GeoTIFF/COG · P1: GeoPackage, Excel)
- Wrap the upstream `rustgis-data` readers behind a `FormatAdapter` trait: `sniff`, `open_metadata`, `stream_features`, `read_window`.
- **Shapefile:** gather the .shp, .shx, .dbf, .prj and .cpg siblings, also from inside a ZIP.
    - A missing .dbf gives geometry only, with a notice.
    - A missing .shx is rebuilt.
    - Text encoding comes from the .cpg file, or is detected.

- **CSV:** sniff the delimiter, quoting and encoding (UTF-8, UTF-8 with BOM, GBK).
    - Keep raw strings: leading zeros and dates stay as written.
    - Store inferred types separately from the raw values.

- **GeoTIFF:** read scale and offset, NoData, band descriptions and overviews. A large TIFF without overviews shows "Preparing a faster preview…", builds overviews in the cache, and can be canceled.
- **GeoPackage:** its layers become one group. **Excel:** sheets are chosen as the assumption catalog describes.

**WP2.3 Profiles and assumptions** (P0)
- Implement every row of the assumption catalog in `lm-profile`, with a unit test for both a matching and a non-matching case.
- Show badges and assumption lines. Change opens a popover listing the alternatives plus Not sure.
- Changing an assumption reruns only the affected stages, keeps the style, and can be undone.
- Question cards attach to the layer's shelf row; they are never modal dialogs. The map stays usable, and the row shows "Needs a decision" until it is answered.
- Write the question log as specified in *Assumptions and questions*.

**WP2.4 "Where should this data be?"** (P1)
- Search a bundled subset of coordinate systems that have defined areas of use: every UTM zone on WGS 84; the 3° and 6° Gauss–Krüger zones on CGCS2000, Xian 1980 and Beijing 1954; and common national grids. Show up to six candidates as thumbnails.

**WP2.5 China offsets** (P0 under the China assumption)
- Implement GCJ-02 ↔ WGS 84 and BD-09 ↔ GCJ-02 in `lm-analysis`. The inverse is iterated until the residual is under 0.5 m.
- Build the notice and the Compare view. Converting creates a derived layer and leaves the source unchanged; its recipe says "Converted from GCJ-02 to WGS 84".
- Test against vectors from two independent open-source implementations, with residuals under 0.5 m. Then check 10 Shanghai landmarks read off Amap by hand against WGS 84 positions from OpenStreetMap nodes; each converted landmark must land within 10 m (fixture **C2**).

**WP2.6 Import report** (P0)
- Store each import's final status in the layer's Details: complete, complete with exclusions, needs a decision, canceled, or failed.
- "Review 6" opens a read-only list of the excluded records with a Reason column. The Phase 3 table drawer later replaces this list. "Export problem list" writes those rows to a CSV.

**WP2.7 Layer shelf and first styles** (P0)
- Build everything in *Layer shelf*, with the default symbols from *Visual design system*.
- Stack each new layer by kind: points and lines above areas, areas above rasters. Once the user reorders, never re-sort.

**WP2.8 Safety limits** (P0)
- For ZIP archives: at most 2 GB expanded, a 100:1 compression ratio and 10,000 entries. Reject absolute paths and `..`.
- Set caps with clear messages. A GeoJSON over 2 GB gets a suggestion to convert it. A raster over 8,192 px per side without overviews goes through the preparation step; one over 200,000 px per side is refused with an explanation.
- Never follow URLs found in files, and never execute anything from them.

**WP2.9 Example data** (P1)
- Wire up Try an example with three datasets:
    - Natural Earth populated places (points)
    - Natural Earth countries with population fields (areas)
    - ETOPO 2022 elevation downsampled to 30 arc-minutes (a public-domain numeric raster)

**WP2.10 Minimal overview, undo core, document v0** (P0, about 1 effort-week)
- Build the layer overview's header, assumption lines and Details tab: enough to show, change and compare assumptions. Phase 3 completes the overview.
- Build the transaction and undo core: every document change is one transaction, and undo reverses it. Phase 3 adds gesture merging and the depth limit.
- Build document v0 in `lm-doc`: save and load a `manifest.json` with source references, profiles, assumptions and styles, from a developer menu. Gate documents D1–D4 start here.

### Gate 2
**Automated**
- Every V, T, C and R fixture opens, and its profile matches the approved JSON snapshot: counts, field roles, coordinate system, assumptions. Every X fixture is refused with its expected message.
- `cargo fuzz` runs for one hour per adapter (CSV, GeoJSON, Shapefile, DBF, GeoTIFF header) with no crash or hang.
- No UI stall over 100 ms occurs during fixture imports; stalls over 50 ms are logged.
- Every source file is byte-identical before and after import (hash check).

**Debug**
- Explain every changed snapshot in the gate report.

**Your validation — bring your own data (90 minutes)**
1. Install fresh. Drag your folder of 15–20 real files onto the window, five at a time.
2. For each file, note:
    - Did it land in the right place?
    - Is its name readable?
    - Did a question appear, and could you answer it without help?
    - Is every assumption line true?

3. Open the three known-difficult files: a Shapefile without .prj, a GBK-encoded Shapefile, and an Amap-sourced CSV. Use Change and Compare on each.
4. Open the file with deliberately bad rows. Use Review, then Export problem list.
5. Export the question log and count the question cards. Save the session as document D1 from the developer menu.

**Stress tests**
- **X1:** a ZIP bomb is rejected in under 1 second.
- **V6:** a 500 MB GeoJSON opens, and the map stays interactive throughout.
- **T3:** a 1,000,000-row CSV opens.
- **R4:** the 2 GB COG previews within 3 seconds.
- **R5:** the preparation step for a 4 GB TIFF without overviews can be canceled.
- 50 files dropped at once.
- A file deleted while it is importing.
- A file in a read-only folder.

**Exit criteria**
- Every fixture matches its snapshot, and fuzzing finds no crashes.
- At least 90% of your real files open in the right place without help. Every other file fails with an honest message.
- Your session shows at most 1 question card per 5 files opened.
- No source file changes. Any change is an S1 issue.
- The map never stalls for more than 100 ms during an import.

**Rollback**
- Return to `gate-1-pass` if a P0 format or the coordinator fails.
- A failing P1 format (GeoPackage, Excel) is switched off by its flag and moved to Phase 6, under the cutting rule. It does not fail the gate.

## Phase 3 — Inspect, select, and insight cards
**Goal:** click anything and immediately understand three things: what it is, where it stands among its peers, and how to make the map show it clearly. **Effort:** 3 effort-weeks. **Scope:** P0 selection, cards, insight, field roles, style and undo; P1 table drawer and filters. Display-only actions ship here: Color by a value, Filter, Compare with other [Districts], Change colors and range, Show only values above…, Show where values are like this. Actions that compute new data wait for Phase 4.

### Work packages
**WP3.1 Selection** (P0)
- Implement `SelectionContext` as described in *Architecture*.
- Hit-test against an R-tree per layer, built at import. Tolerance is 4 px, or 6 px for points drawn under 5 px.
- Build the overlap chooser. Basemap labels are never selectable.
- Draw the selection highlight, and implement the blank-click and Esc ladder behaviors.
- Support ⇧-click multi-select, with a selection chip: "3 selected · Clear".

- Add geodesic length and area with GeographicLib's Rust port, tested against fixture N5's length and area cases.

**WP3.2 Inspector** (P0)
- Build the object card and layer overview exactly as specified, with Overview, Style and Details tabs.
- Detect the Name field: the first text field called name, 名称, title or NAME_EN; otherwise the most unique short text field.
- Apply the facts-list rules and add "Show all fields".
- Build a skeleton of the action registry and its rules file, then wire the display-only actions listed in Scope into their table rows. Phase 4 completes the registry.

**WP3.3 Field roles** (P0)
- Infer roles per the assumption catalog. Show role chips with Change in the Details field list.
- A role change updates insight, style suggestions and actions at once, and can be undone.
- Let users star a field ("Show in insight").

**WP3.4 Insight strip** (P0)
- Build each measure field's rank index in the background.
- Implement the rank phrases, the dot strip, the provisional label for sampled raster percentiles, and filtered denominators.
- Add property-based tests: ranks equal those from a naive sort on random data, including ties, NaN and nulls.

**WP3.5 Style panel** (P0)
- **Presets:** 4–6 thumbnails drawn from the layer's own data: single color, by category, by value, size by value (points), Heat (when the unit is a temperature), labels on.
- **Classes:** "Smooth" (continuous) or "Groups" (5 quantile classes). Class method and count sit behind a disclosure.
- **Size by value:** circle *area* is proportional to the value. The legend shows three reference sizes.
- **Other controls:** opacity, outline, labels (field and size).
- **Legend:**
    - Generated from the style, never typed.
    - Shows units and a "No data" entry.
    - Uses the full dataset's range, with a "Fit range to view" toggle that leaves a visible indicator while on.

- **Count guard:** when areas are colored by a Count field, show a purple line: "Larger areas tend to have higher counts. Color by count per km² instead?" One click adds a virtual density field, using geodesic area.
- **Raster:** band, stretch (2–98% by default), palette, opacity.

**WP3.6 Table drawer** (P1)
- Build the drawer per *Table drawer*: virtualized rows, shared selection, blank shown separately from zero.

**WP3.7 Filters** (P1)
- Build the sentence filter builder and chips. A filtered layer shows a funnel icon on its shelf row.

**WP3.8 Undo** (P0)
- Undo works over document transactions, 100 steps deep. A slider gesture is one step. Selection and camera moves are not steps.
- Keep undo memory bounded with structural sharing. Rasters are never duplicated.

**WP3.9 Search your data** (P1)
- Extend toolbar search to the Your data group: layer names, and values in each layer's first five text fields. Choosing a result selects the object and opens its card.

### Gate 3
**Automated**
- Property tests for ranks.
- A legend–style consistency test: colors sampled from the rendered map match the legend.
- Selection stays in sync between table and map.
- An undo round trip for every style operation: the state after undo equals the state before the operation.
- Screenshot regression for card states: object, layer, overlap chooser, no value, filtered.

**Debug**
- The independent review checks every number a card can show.

**Your validation — "Explain this map" (75 minutes)**
1. Open teaching set **K1**: district areas with a July mean temperature field, the surface-temperature raster with its unit set, and school points.
2. Click five districts and write down their insight rows. Run `validation/gate-3/check_insight.py`, a pandas script the agent provides, and compare every rank and median difference.
3. Mark a population field as an identifier. Confirm it leaves the insight strip, then undo.
4. Style districts with presets. Then color them by a count field, and accept the density suggestion.
5. Open the table and filter "population above 500,000". Select rows and watch the map.
6. Undo ten times and redo ten times. The map must end exactly as it started.
7. Hallway test with two students: "Which district is hottest, and how do you know?"

**Stress tests**
- **V7:** scroll the table of 100,000 polygons.
- **V8:** a layer with 200 fields.
- **F2:** 1,000,000 points styled by value.
- 100 rapid clicks on overlapping objects.
- An opacity slider dragged for 30 seconds, which must leave exactly one undo step.
- 1,000 undo steps, with memory measured.

**Exit criteria**
- Every number from your five districts matches the check script exactly.
- A card appears within 100 ms of a click on F1, and within 200 ms on V7.
- Both students find the hottest district and explain why, without help.
- Undo and redo round trips are exact.

**Rollback**
- Return to `gate-2-pass` if selection, cards or insight fail.
- The P1 table drawer and filters may be cut to Phase 6 under the cutting rule. The Phase 2 excluded-records list stays as the fallback.

## Phase 4 — Contextual actions, previews, result cards
**Goal:** from any selection, a novice can ask one of up to three relevant questions, preview the answer, and keep a result that explains itself. **Effort:** 5 effort-weeks, the largest phase. **Scope:** P0 registry, relevance, composer, jobs, area drawing, certified actions and result cards; P1 action search.

### Work packages
**WP4.1 Action registry and relevance** (P0)
- Write an `ActionSpec` for every action in *Selection → suggested actions*. The row orders live in `lm-actions/rules.toml`.
- Eligibility checks return a reason the user can act on: "Add an area layer to count schools by district."
- Add a snapshot test per table row: a fixture context in, the exact three actions and the More list out.
- Add a determinism test: the same context run 1,000 times gives the same output.
- Gate each action by its `min_phase` flag.

**WP4.2 Composer** (P0)
- Build everything in *Composer*: token pickers, the scope selector, inline notices, and automatic preview refresh when the estimated cost is under 1 second.

**WP4.3 Job system** (P0)
- Run jobs on a worker pool against immutable input snapshots (shared, reference-counted data handles).
- Progress events report the phase and the amount processed. There are no invented percentages.
- Cancellation is cooperative at chunk boundaries, with a 200 ms target. No work is uninterruptible: every lm-analysis loop checks a cancel token at least every 50 ms.
- A generation counter drops any result from an outdated plan, so a stale preview can never appear.
- Commit is atomic: layer, style, result card and undo entry go in one transaction.
- A cost estimator per action (features × vertices, raster cells) decides automatic preview, and warns when a job will exceed 30 seconds.

**WP4.4 Drawing an area** (P0)
- **Polygon mode:** click to add points, double-click or Return to finish, Esc to cancel. A banner reads "Drawing — Esc to cancel".
- **Rectangle mode:** drag.
- Drawn areas become layers in WGS 84, named "Area 1" and so on, and can be renamed.

**WP4.5 Certified actions** (P0)
Each action has a written definition, a reference implementation for tests, and a tolerance.

| Action | Definition | Reference for tests | Tolerance |
|---|---|---|---|
| Count [Schools] in each [District] | A point on a boundary counts in every area that touches it. Overlapping areas each count their points; the notice says how many points were counted more than once. Areas with zero points stay in the output. | GeoPandas `sjoin` with `intersects`, then a count | Exact |
| Which [District] is it in? | Lists every containing area. A point on a boundary lists both. | GeoPandas `sjoin` | Exact |
| What's inside? | Points and lines that intersect the area, listed in the card and highlighted. Keep makes a filtered view. | GeoPandas `sjoin` | Exact |
| Show an area within a distance | A geodesic buffer at the given meters or kilometers. Layer-wide buffers merge overlaps by default. Labelled "Straight-line distance". | Points: pyproj `Geod` circle points. Lines and areas: a densified buffer per feature in a local azimuthal equidistant projection | Points: area within 0.1%. Lines and areas: area within 0.5% |
| Measure distance or area | Geodesic, on the WGS 84 ellipsoid | pyproj `Geod` (GeographicLib) | 0.01% |
| Summarize [Heat] in each [District] | Fractional pixel coverage, each cell weighted by its area on the ellipsoid. Gives the area-weighted mean, minimum, maximum and coverage (formulas below). Totals appear only for confirmed count or density rasters. Category rasters give shares (fixture N8). | `exactextract` weighted mean with a cell-area weight raster; for R7, also a high-resolution geodesic integration | 1 part per million against exactextract; 1 part per thousand against the geodesic integration |
| Rank [Districts] by a value | Attribute sort, returned as a ranking card | pandas `sort_values` | Exact |
| Add [Heat] values to each [School] | The value of the raster cell containing each point, with scale and offset applied. No interpolation. NoData gives a blank, never zero. | rasterio sample | Exact |

For a polygon, the area-weighted mean of a measured value, such as temperature:

```latex
\bar{x}_P = \frac{\sum_{i} x_i \, a_i}{\sum_{i} a_i},
\qquad a_i = f_i \, A_i
```

```latex
f_i = \text{fraction of cell } i \text{ inside } P \text{ with valid data (as exactextract computes it)},
\qquad A_i = \text{area of cell } i \text{ on the WGS 84 ellipsoid}
```

The coverage reported on every summary card:

```latex
c_P = \frac{\sum_{i} a_i}{A_P},
\qquad A_P = \text{area of } P \text{ on the same ellipsoid}
```

Tests assert that coverage never exceeds 1 by more than one part per billion.
The estimated total from a raster of counts per cell, assuming people are spread evenly within each cell:

```latex
N_P = \sum_{i} n_i \, f_i,
\qquad f_i = \text{fraction of cell } i \text{ inside } P
```

The total from a density raster, such as people per km²:

```latex
N_P = \sum_{i} d_i \, a_i
```

The card always states which formula applied, and the uniform-spread assumption whenever counts are split across a boundary.
**WP4.6 Result cards and recipes** (P0)
- Build everything in *Result card*. Generate the recipe from the plan: inputs (name, file, fingerprint), scope, decisions, assumptions with confidence, method and version, coordinate handling, exclusions, software version.
- Copy produces plain text. A machine-readable JSON copy is stored in the document.
- A fingerprint mismatch marks the result "Source changed". "Update result" creates a new version, and the old one stays readable.
- Change question reopens the composer with the earlier inputs. The old result stays until it is replaced, or both are kept.

An example of the recipe text a user copies:

```markdown
Schools in each district
Computed 14 Dec 2026 with LayeredMap 0.4.0
Inputs: Schools (schools.csv, 248 points); Districts (districts.geojson, 17 areas)
Scope: all schools; all districts
Method: point-in-area count; a point on a boundary counts in every area it touches
Decision: overlapping areas each count their points (2 schools counted twice)
Assumption: schools located using lon/lat as WGS 84 (likely, from column names)
Excluded: 6 schools with no usable location
```

**WP4.7 Action search, ⌘K** (P1)
- Fuzzy-search action labels and expert aliases (buffer, zonal statistics, spatial join, clip). An unavailable action shows why, and what would make it available.

### Gate 4
**Automated**
- **Numeric suite:** fixtures N1–N6 against reference outputs. The references are Python scripts in `reference/` (GeoPandas, exactextract, pyproj) that write the expected CSVs. Tolerances are those in the table.
- **Relevance:** snapshot and determinism tests.
- **Race:** 1,000 random rapid parameter changes. The preview on screen always matches the final parameters.
- **Cancellation:** 100 cancels at random moments leave no partial layers and no temporary files.
- **Compound undo:** keep a result, undo, and the state equals the state before. Redo gives the identical result.

**Debug**
- The independent review checks each certified algorithm against its written definition.

**Your validation — answer three questions (90 minutes)**
1. Take the three questions you supplied in *Planning assumptions*, and answer them with K1 and your own data.
2. Use only the suggested actions, no ⌘K. Count clicks and question cards.
3. Compare each answer with your own Python or Earth Engine result, using a notebook the agent provides. Every difference must be explained by the recipe; cell-center versus fractional coverage is the usual one.
4. Change a question twice, from 500 m to 1 km. Keep both results and compare them.
5. Cancel a long summary midway. Confirm that nothing was added.
6. Read each recipe aloud. Would a student understand it?
7. Hallway test with three students:
    - "Which district has the most schools per km²?"
    - "What is the average surface temperature in your home district?" and "How many schools are within 1 km of your home?"

**Stress tests**
- **N7:** summarize R4 (2 GB) across 3,000 polygons in under 5 minutes, with progress shown and cancellation within 1 second.
- Count F2 (1,000,000 points) in V7 (100,000 polygons) in under 60 seconds.
- Buffer 10,000 points by 1 km, merged, in under 30 seconds.
- Keep 50 results in one document.
- Run 100 undo and redo cycles across actions.

**Exit criteria**
- Every numeric fixture is within tolerance. Any miss is an S1 issue.
- Each of your three questions is answered through suggestions in at most 6 clicks and at most 1 question card.
- Your answers match your own pipeline, or the recipe explains the difference.
- At least 2 of 3 students finish all three tasks without help and state the scope correctly ("all schools", "straight-line").
- Cancellation takes at most 1 second, and no stale preview is ever shown.

**Rollback**
- Return to `gate-3-pass` if the registry, composer or job system fails.
- An action that fails its numeric tests is switched off by its flag rather than shipped, and the gate can still pass. The exceptions are Count in areas and Summarize in areas: they are the pilot's core, so their failure fails the gate.

## Phase 5 — Keep, share, data library → classroom pilot
**Goal:** students can install a signed build, find course data inside the app, and save, reopen, recover, export and share maps. Then the pilot starts. **Effort:** 4 effort-weeks. **Scope:** P0 save, autosave and PNG export; P1 recovery, data export, data library, Chinese interface, keyboard, signed build, pilot kit.

### Work packages
**WP5.1 Document format** (P0)
- Define `.lmap`, a single-file ZIP container. It holds:
    - a versioned `manifest.json`
    - owned results (FlatGeobuf for vectors, GeoTIFF for rasters)
    - styles and recipes
    - source references (relative path, absolute path, fingerprint)

- Save atomically: write a temporary file in the same folder, then rename it over the old one. The previous version stays until the rename succeeds.
- Autosave every 30 seconds, and when the app loses focus, to `~/Library/Application Support/LayeredMap/Autosave/`. The user's own file changes only on ⌘S.
- The title bar shows the document name with the macOS "Edited" state.
- Write the format spec in `docs/lm/format-v1.md`. Migrate the fixture documents from Gates 2–4.

**WP5.2 Recovery and relinking** (P1)
- Keep a recovery journal. After a crash, the next launch asks: "LayeredMap quit unexpectedly. Restore 'Heat study' from 2 minutes ago?" with Restore and Discard.
- Show the missing-source card from *Error and status copy*.
- Relinking checks the fingerprint. On a mismatch, it asks: "This file looks different (its size changed). Use it anyway?" A yes marks dependent results "Source changed".

**WP5.3 Export** (P0 map image, P1 data)
- **Map image sheet:**
    - A preview at exactly the output size.
    - Presets: Slide 16:9 (1920 × 1080), Report A4 at 300 dpi, Square (1080 × 1080).
    - A title field.
    - A legend that places itself bottom left, keeps clear of the data, and can be dragged.
    - A scale bar and mandatory attribution.
    - An editable caption, generated automatically: "Data: Districts (…); Heat (…). Made with LayeredMap."

- **Export warnings:**
    - a color scale has no legend
    - previews are excluded
    - a filtered view is being exported ("12 of 248")
    - the basemap's terms forbid export, in which case the offline basemap is substituted and shown in the preview

- **Data export:**
    - Choose a layer, a scope (all, filtered or selected), and fields.
    - Fields that look sensitive (phone, email, ID numbers) start unchecked, with a note.
    - Formats: GeoJSON, CSV (lon/lat or WKT), or GeoTIFF for rasters, in WGS 84 by default.
    - The dialog states what is lost: styles and recipes do not travel in these formats.

**WP5.4 Data library** (P1)
- Host a catalog JSON next to the basemap. Each entry has: id, title (English and Chinese), description, kind, size, extent, license, source URL, citation, date.
- Add a library panel, opened from the welcome card and the File menu. Each card shows a thumbnail, size and license. "Add to map" downloads into the cache with progress, then imports through the normal pipeline, assumptions included.
- Pick 8–12 course datasets, each with a verified license and citation (you choose; see *Risks and open decisions*). Candidates:
    - district boundaries
    - your summer surface-temperature composite
    - a population grid
    - elevation
    - parks, schools and hospitals from OpenStreetMap

**WP5.5 Chinese interface** (P1)
- Complete English and Simplified Chinese strings through upstream's translation system. Numbers and dates follow the locale; dataset values are never changed. The language follows the system, with an override.
- A Chinese-speaking student assistant reviews every screen against `docs/lm/glossary-zh.md`, for example layer 图层 and basemap 底图.

**WP5.6 Keyboard and basic accessibility** (P1)
- Every core task can be done by keyboard alone: open, select (through search or the table), run an action, keep, export.
- Focus rings are visible, and every control has an accessible name through AccessKit.

**WP5.7 Packaging** (P1)
- Signing and notarization already run in CI from Phase 0. Add the hardened-runtime checks, and ship a stapled .dmg with drag-to-Applications from the China-reachable download host, not from GitHub releases.
- On launch, check a JSON feed and show "A new version is available" with a download link. It is skipped when there is no network, and blocked by the strict offline mode added in Phase 6.
- Crash logs stay local. "Export diagnostics" shows its contents for review before anything is sent.
- Build the network-blocking test harness that Gate 5 uses: core tasks must make zero network calls.

- Move basemap tiles, the data library and the update feed onto the production host chosen at Gate 0. Set a monthly budget alert, and keep the offline basemap as the fallback.

**WP5.8 Pilot kit** (P1)
- Write a one-page quickstart in English and Chinese.
- Write three assignments built on library data.
- Prepare a feedback form, and instructions for exporting the question log.
- If pilot logs or surveys may be used in a publication, get NYU Shanghai IRB approval or an exemption before the pilot. Use consent text in both languages.

### Gate 5 — the pilot gate
**Automated**
- Save and reopen every fixture document; the states must be equal.
- Kill the app with `kill -9` during a save, 200 times. A file is never corrupted: the old or the new version always opens.
- Autosave restore tests.
- Export pixel tests: exact dimensions, legend and attribution present, colors that match the legend.
- Format migration from the Gate 2–4 documents.
- The .dmg passes `xcrun stapler validate` and an `spctl` assessment.
- Core tasks with all network access blocked make zero network calls. Fixture X5: exported CSVs neutralize cells that begin with =, +, - or @.

**Debug**
- An independent review of save and export.
- A license audit of all code, data, fonts and icons with `cargo about` and `cargo deny`, producing the ATTRIBUTION file.

**Your validation — dress rehearsal (2 hours)**
1. On a fresh macOS user account, download the .dmg from the China-reachable download host as a student would, timing the download on the campus network. Install and open it; no security warning should appear.
2. Complete all three pilot assignments using only the quickstart.
3. Save, quit and reopen. Move a source file and relink it.
4. Force-quit mid-task (⌥⌘Esc), reopen, and restore.
5. Export one image for a slide and one for an A4 report. Check the legend, units, attribution and caption.
6. Switch macOS to Chinese and repeat assignment 1.
7. Turn off Wi-Fi before launching, and repeat assignment 1 with library data already downloaded.

**Formative usability round.** Eight novice students who are not in the pilot course do four tasks, without being told where anything is:
- "Open this file and tell me what it shows."
- "Find which district contains the most schools."
- "Explain this raster value."
- "Export a map another person can understand."

Record wrong interpretations as well as completions. Ask each student to explain the result in their own words.
**Stress tests**
- A 50-layer document, and 1 GB of owned results.
- Saving with under 200 MB of free disk.
- Saving to an iCloud Drive folder and to a USB drive.
- An 8-hour session with autosave running.
- 30 exports back to back, with memory measured.

**Exit criteria**
- Zero corrupted files across all kill tests.
- Usability targets:
    - at least 7 of 8 open a file and describe it correctly, without help
    - at least 6 of 8 complete one contextual analysis
    - at least 7 of 8 state the scope and any missing-data qualification correctly
    - the question budget is met: median at most 1 question card per session, and no session over 3

- Every exported image has its needed legend, units and attribution, and matches its preview.
- Installation shows no security warning.
- The license audit is clean.
- The final name is chosen after a trademark search, or LayeredMap is confirmed.

**Rollback and pilot fallback**
- If the code fails, return to `gate-4-pass`.
- If Gate 5 fails within three weeks of the course start, postpone the pilot by a term. Never pilot a build with an open S1 issue.

### Running the pilot
- Ship from a `pilot` branch. Only S1 and S2 fixes go in, at most one release a week, and each passes the Gate 5 automated suite.
- Each week, review the exported question logs and the feedback form. Tag issues for Phase 6.
- End with a short survey and two student interviews. These are the input to Phase 6 planning.

## Phase 6 — Hardening → public beta
**Goal:** fix what the pilot found, reach a second platform, complete accessibility and the P2 actions, then release a public beta. **Effort:** 6 effort-weeks. **Scope:** pilot fixes (P0), plus every P2 item in *Priority model*.

### Work packages
- **WP6.1 Pilot fixes** (P0). Rank pilot issues by severity, then by how many students hit them. Fix the top ten. Update the interaction specification before changing code.
- **WP6.2 Windows build** (P2).
    - Segoe UI and Microsoft YaHei fonts, file associations, and precision-touchpad gestures.
    - Correct scaling at 125% and 150%.
    - A signed installer. Budget for a code-signing certificate, and for the time it takes to build SmartScreen reputation.

- **WP6.3 Screen readers** (P2). A full VoiceOver pass, then Narrator.
    - Searchable records and selection summaries are the alternative to the map canvas.
    - Announcements read like: "District 7 selected. Heat 31.2 degrees, 3rd highest of 17."

- **WP6.4 P2 actions.** Each has a written definition and reference tests:
    - Match a table to a layer: an attribute join that checks one-to-one, one-to-many and many-to-many matches, shows the expected row count first, and lists unmatched rows for review. Reference: pandas `merge` with `validate`.
    - Nearest feature, measured geodesically. Reference: GeoPandas `sjoin_nearest` in a local projection, checked against pyproj.
    - Keep data inside an area (clip). Reference: GeoPandas `clip`.

- **WP6.5 PDF export and portable map package** (P2). The package dialog shows its size estimate, the sources it includes, each dataset's license, and a review of paths and private fields.
- **WP6.6 Strict offline mode** (P2). One switch blocks every network request: basemaps, search, library, update check. The Phase 5 network-blocking harness verifies that zero requests are made.
- **WP6.7 Deferred formats.** KML/KMZ (with styles) and GPX, plus any P1 format cut earlier.
- **WP6.8 Security hardening.** 24 hours of fuzzing per adapter, in 6-hour shards on Linux runners, `cargo audit` in CI, and a software bill of materials.
- **WP6.9 Upstream contributions.** Offer `lm-mapview`, the GCJ-02 conversion and the fractional-coverage zonal summary to RustGIS as pull requests.
- **WP6.10 Website and help.** A landing page, a searchable glossary, and three complete workflows.
- **WP6.11 Optional experiment** (P3, off by default). An "Ask about this layer" box that maps a typed question onto an existing action. It shows exactly what would be sent to the model, and still runs through the composer and preview. It is judged on 50 real student questions collected during the pilot.

### Gate 6 — the beta gate
**Automated**
- Every suite from Gates 1–5, on both macOS and Windows CI.
- 24 hours of fuzzing per adapter, a clean `cargo audit`, and the offline harness.

**Debug**
- An independent end-to-end review, plus a security review of every import path.

**Your validation (2 hours)**
1. Repeat the Gate 5 rehearsal on a Windows laptop.
2. Complete assignment 1 with VoiceOver and the screen curtain on, so the screen is dark.

**Acceptance round.** 20 novice participants across both platforms do the four Gate 5 tasks plus one P2 task.
**Stress tests**
- Every earlier stress test, rerun on a low-end Windows laptop (8 GB RAM, integrated graphics).
- A one-week soak with daily use.

**Exit criteria**
- The usability targets are met with 20 participants: at least 18 open and describe a file, at least 16 complete an analysis, and at least 18 state the scope correctly.
- Zero S1 issues, and at most five open S2 issues, each with a documented workaround.
- The release matrix is green on both platforms.
- A basemap hosting budget is set for public traffic: estimated monthly tile requests, a spending cap, and what happens when the cap is reached.

**Rollback**
- If Windows fails, release a macOS-only beta from `gate-5-pass` plus the pilot fixes. Windows follows when its gate passes.

### After the beta
P3 items are built only with evidence: at least three independent user requests, or a clear pilot signal. Each must also answer the original specification's six questions:
1. Who needs this?
2. What question does it answer?
3. When should it appear?
4. What does the user need to understand?
5. What can go wrong?
6. How is the result explained and undone?

Candidates, in likely order:
- raster comparison (side by side, swipe, change after alignment checks)
- a time slider for dated data
- natural-language questions, if WP6.11 succeeds
- travel time over a street network
- 3D
- plugins

## Test fixtures and stress tests
Every gate test names its fixtures by ID. Small fixtures live in the repository under `fixtures/`. Large ones are generated by `cargo xtask fixtures`, or downloaded from a pinned bucket with checksums. Each fixture's license is recorded in `fixtures/LICENSES.md`.

| ID | Contents | What it tests | Built from |
|---|---|---|---|
| F1 | Online basemap + 100,000 points + 10,000 polygons (at most 1,000,000 vertices) | Map feel | Generated |
| F2 | 1,000,000 points with 5 attribute fields | Point rendering, styling, counts | Generated |
| V1 | GeoJSON, 17 district areas with Chinese and English names, population and area fields; includes a hole and a multipolygon | Import, insight, counts | Hand-made |
| V2 | GeoJSON with points, lines and areas mixed | Mixed-geometry split | Hand-made |
| V3 | Shapefile without .prj, in degrees | Visible WGS 84 assumption | Hand-made |
| V4 | Shapefile without .prj, projected in a CGCS2000 Gauss–Krüger zone covering Shanghai | "Where should this data be?" | Hand-made |
| V5 | Shapefile with a GBK-encoded .dbf and no .cpg | Encoding detection | Hand-made |
| V6 | 500 MB GeoJSON of building footprints | Streaming import | OpenStreetMap extract |
| V7 | 100,000 polygons | Table, hit testing, counts | Generated |
| V8 | A layer with 200 fields | Inspector and table layout | Generated |
| V9 | A zipped Shapefile in a nested folder, plus a 3-layer GeoPackage | Archives and groups | Hand-made |
| V10 | Geometry edge cases: antimeridian crossing, polar polygon, zero-area polygon, self-intersection, empty geometry | Correctness and honest errors | Hand-made |
| T1 | CSV with lon/lat columns, 248 rows, 6 with blank coordinates | Visible assumption, import report | Hand-made |
| T2 | CSV with ambiguous columns (x, y, lat2, lng2) and some swapped rows | Question card | Hand-made |
| T3 | 1,000,000-row CSV | Import speed, table | Generated |
| T4 | GBK-encoded CSV with semicolons, decimal commas, leading-zero codes and dates | Raw-string preservation | Hand-made |
| T5 | Excel workbook with 3 sheets, one with coordinates | Sheet choice | Hand-made |
| C1 | 50 points of interest in GCJ-02, named `amap_pois.csv` | Offset question (name hint) | Hand-made |
| C2 | 10 Shanghai landmarks in WGS 84 and GCJ-02, plus test vectors from two open-source implementations | Conversion accuracy | You, by hand in Amap |
| C3 | Points in BD-09 (Baidu) | Second offset system | Hand-made |
| R1 | Summer land-surface temperature over Shanghai, with no unit metadata | Raster profile, insight, summaries | Your Earth Engine export |
| R2 | Integer population counts with NoData | Count totals, cell-meaning question | Open population grid (license to verify) |
| R3 | Land-cover categories as integer codes | Category shares, not means | ESA WorldCover (CC BY 4.0) |
| R4 | 2 GB COG with overviews | Preview speed, large summaries | Generated or public imagery |
| R5 | 4 GB striped TIFF without overviews | Preparation step, cancel | Generated |
| R6 | Raster with scale and offset, and −9999 only at the edges, with no NoData tag | NoData assumption, decoded values | Hand-made |
| R7 | A geographic-coordinate raster spanning a wide latitude range | Unequal cell areas in weighting | Generated |
| K1 | Teaching set: V1 districts plus a July mean temperature field you compute in Earth Engine, a copy of R1 with its unit set to °C, and school points (a variant of T1), with your expected answers | Phases 3–5 validation | You |
| N1 | 20 points and 3 areas, with boundary points, an overlap and a zero-count area | Count rules | Hand-checkable |
| N2 | Counts in polygons with holes and multipolygons | Count rules | Hand-checkable |
| N3 | A 4 × 4 raster with a polygon covering half cells and two NoData cells | Area-weighted mean and coverage | Hand-computed |
| N4 | Summary over R7 | Unequal cell areas | exactextract |
| N5 | Buffers and measurements at the equator, at 60° N and near the antimeridian | Geodesic accuracy | pyproj |
| N6 | Totals from R2 and from a density version of R2 | Count versus density totals | exactextract |
| N7 | R4 across 3,000 polygons | Performance, spot correctness | exactextract (sampled) |
| N8 | R3 land-cover categories across V1 districts | Category shares, not means | exactextract frac |
| X1 | Nested ZIP bomb | Expansion limits | Generated |
| X2 | ZIP with `../` path entries | Path traversal | Generated |
| X3 | GeoJSON with extreme nesting, huge arrays and NaN coordinates | Parser limits | Generated |
| X4 | GeoTIFF header claiming a billion by a billion pixels | Allocation limits | Generated |
| X5 | CSV cells beginning with =, +, - or @ | No spreadsheet-formula injection on export | Generated |
| D1–D4 | Documents saved at Gates 2–5 | Format migration, rollback compatibility | Gate archives |
| B1–B20 | Your bring-your-own-data folder | Real-world opening | You (kept local if private) |

**Regression ladder.** A stress test that passes at a gate reruns at every later gate. Phase sections list only the new stress tests. From Gate 2 on, every gate also reruns:
- the F1 feel script
- the fuzz corpus
- the source-unchanged hash check
- from Gate 5, the save kill test

**Reference machines.** Your Mac is the reference for feel and speed. Record its model, chip, memory and macOS version in every gate report. From Phase 6, a low-end Windows laptop with 8 GB RAM and integrated graphics is added.

## Working with coding agents
The agent writes the code, and you own the behavior. Each work package becomes a set of task cards. Each card gets one branch and one pull request, with a demo you can watch. You judge what the app does against this plan, not the Rust.

### Repository conventions
- **`AGENTS.md`** gains a LayeredMap section with:
    - the twelve product principles, in one line each
    - the layering rules
    - "the UI never computes numbers"
    - "every string is translatable"
    - "tests first in `lm-analysis`"
    - "every UI change includes a screenshot from `ui_shot`"

- **`docs/lm/spec.md`** is this document's specification sections exported as Markdown. It is refreshed at every phase start, so the agent always reads the version you approved.
- **Branches:** one per task, off the phase branch, such as `wp/2.3-csv-coordinates`.
- **Upstream edits:** any change inside a RustGIS crate is its own commit, prefixed `upstream:`, so it can be offered back.
- **Pull request template:** what changed in plain words, a screen recording or screenshots, tests added, fixtures touched, risks.
- **Decision log:** `docs/lm/decisions.md` records every choice made outside this plan, one short entry each.

### Task card template

```markdown
# WP2.3 · task 2 of 5 — CSV coordinate detection
Read first: docs/lm/spec.md → Assumption catalog, rows 1–2; crates/lm-profile/src/csv.rs
Do: detect lon/lat column pairs by name, range and parse rate; emit an Assumption
    with confidence and evidence; pick the visible line or the question card per the grid.
Acceptance: snapshots for T1 and T2 match; unit tests for 12 header variants,
    including 经度/纬度 and swapped columns; nothing in the source file changes.
Out of scope: Excel, GeoPackage, the Change popover UI.
Show me: a 30-second recording opening T1 and T2, plus the profile JSON.
Done when: CI is green and the owner approves the snapshots.
```

### Weekly rhythm
- **Monday (30 min):** the agent drafts the week's task cards from the work packages, and you approve or reorder them.
- **During the week:** the agent works task by task. Every pull request carries its demo.
- **Friday (45 min):** you watch the demos and try the CI build. Approved pull requests merge into the phase branch.

### Review rules for you
- Judge behavior against the plan. If the plan is silent, decide, then add the decision to the plan.
- Ask for a second, independent agent review on any pull request touching `lm-analysis`, `lm-jobs` or `lm-doc`. That code handles numbers, jobs and saving.
- Never merge a pull request that changes a test or an expected output without a written reason in its description.
- Ask "show me it failing first" for every bug fix: a test that fails before the fix and passes after.

### Agent guardrails
These go into `AGENTS.md`. The agent may not:
- change gate criteria, thresholds or this plan
- disable or skip tests
- edit a fixture's expected output without a note
- add network calls outside the basemap, search, library and update services
- add a dependency whose license is not on the allow list (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0, Unicode-3.0, BSL-1.0, CC0-1.0, OFL-1.1 for fonts, and the Ubuntu font licence used by egui's default fonts) without your approval, enforced by `cargo deny`

Upstream merges happen only at a phase start. Each one runs the full CI suite and the feel harness, and is recorded in `UPSTREAM.md`.

### Phase kickoff prompt

```markdown
We are starting Phase N of the LayeredMap build plan (docs/lm/spec.md).
The last gate tag is gate-(N-1)-pass. Read the Phase N section, the sections it cites,
and AGENTS.md. Then: (1) list every work package as task cards using the template,
ordered by dependency; (2) list anything in the plan that is ambiguous or contradicts
the code, as questions for me; (3) do not write code until I approve the cards.
```

## Risks and open decisions
The two biggest risks are builder time and map feel. Phase 0 settles the second; the cutting rule handles the first. The register is sorted by likelihood and impact. Update both columns at every gate.

| # | Risk | Likelihood | Impact | Early warning | Response |
|---|---|---|---|---|---|
| 1 | Less than 20 hours a week of build time | High | High | Two weeks in a row under 10 hours | Cut P1 items under the cutting rule, or move the pilot one term. Never skip a gate. |
| 2 | The CPU renderer cannot reach the feel targets | Medium | High | Gate 0 spike numbers | A GPU map layer in egui, or a Tauri + MapLibre shell. Decided at Gate 0. |
| 3 | Agent-written numerical code is subtly wrong | Medium | High | Reference tests disagree; independent review flags a definition mismatch | Reference implementations, hand-checkable fixtures, an S1 rule, second review on `lm-analysis` |
| 4 | Basemap host slow or unreachable in mainland China, or too costly | Medium | High | Gate 0 latency; monthly bill | Hong Kong region, the offline basemap fallback, local tile cache |
| 5 | The assumption engine asks too often | Medium | High | Gate 2 question count | Retune the catalog. Turn questions into visible assumptions where the consequence allows. |
| 6 | Scope creep inside a phase | High | Medium | Task cards for work not in the plan | New features enter only through an edit to this plan at a phase start |
| 7 | Fast-moving upstream breaks adapters or the map view | Medium | Medium | Merge conflicts at a phase start | Merge only at phase starts; thin adapters; offer shared pieces upstream |
| 8 | Map-publication rules in China | Medium | Medium | Students posting exported maps publicly | China point-of-view boundaries in builds distributed in China, a basemap variant without national boundaries for export, guidance in the quickstart, and NYU Shanghai's guidance sought before Gate 5. Maps published in China require official review. |
| 9 | egui falls short on accessibility or Chinese input | Medium | Medium | Gate 0 probes | Fix upstream in egui or AccessKit, or change stack at Gate 0 |
| 10 | Many students use Windows | Medium | Medium | Course roster survey | Move Windows packaging into Phase 5 |
| 11 | Library datasets have unclear licenses | Medium | Medium | Any entry without a written license | Open-licensed datasets only, with citations recorded |
| 12 | Pilot logs count as research data | Medium | Medium | Plans to publish on the pilot | Seek IRB approval or exemption before the first hallway test |
| 13 | Apple signing or notarization delays | Low | Medium | Enrollment not done in week 1 of Phase 0 | Internal gates use locally built, unsigned apps; the pilot waits for signing |
| 14 | Name conflict | Low | Low | Trademark search results | Choose the name at Gate 5, before installers and domains exist |

### Open decisions

| Decision | Options | Decide by | Needs |
|---|---|---|---|
| First course and term for the pilot | Which course; Fall 2027 or later | End of Phase 0 | You |
| Rendering stack | egui with cached textures · egui with a GPU map layer · Tauri + MapLibre | Gate 0 | Spike numbers (WP0.3) |
| Fork, or a front end upstream | GitHub fork · second front end inside RustGIS | Gate 0 | Maintainer's reply (WP0.1) |
| Basemap host | Global CDN · Hong Kong region · mainland China with ICP filing | Gate 0; contract by Gate 5 | Latency results (WP0.6) |
| Student download host | China-reachable host from WP0.6 · GitHub releases | Gate 0 | Download timing from campus |
| Data library contents | 8–12 datasets | Start of Phase 5 | You, with license checks |
| Final name | LayeredMap · another name | Gate 5 | Trademark search |
| Windows timing | Phase 5 · Phase 6 | Once the course roster is known | Student survey |
| Natural-language experiment | Run WP6.11 · skip | Gate 5 | Pilot question log |

