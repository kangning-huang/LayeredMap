# iPhone companion spike — Alternative D (C, with dataset discovery put first)

Throwaway spike for **Alternative D**, built from a fresh persona study of Alt C (2026-11-03). Alt D
is Alt C in every respect — the same map, cards, legend, rasters, saved map, Change look — except for
**how a person finds and opens data**.

**Not product code.** Lives under `spikes/` and will be thrown away or rewritten when the real app lands.

| | Alt A | Alt B | Alt C | **Alt D** (this spike) |
| --- | --- | --- | --- | --- |
| Path | `spikes/ios-companion/` | `spikes/ios-companion-alt-b/` | `spikes/ios-companion-alt-c/` | `spikes/ios-companion-alt-d/` |
| Bundle id | `…spike.alta` | `…companion.altb` | `…companion.altc` | `com.nimblemap.companion.altd` |
| Home screen name | NimbleMap A | NimbleMap B | NimbleMap C | **NimbleMap D** |

## The study behind D

A new persona, deliberately unlike the earlier rounds. Priya and Walt were novices at *both* halves
of the problem. **Marisol** is the opposite shape, and she is the person the owner keeps meeting:

> A hobby map collector with sixty bookmarks of data portals. She can name from memory which agency
> publishes what — data.gov, TIGER/Line, Geofabrik, USGS, her city's open-data portal. She has
> **never once managed to open any of it.** She knows exactly *where* data lives and nothing about
> *how* to get it onto a map. She has given up about a dozen times and is one bad experience away
> from never trying again.

Her verdict on C: the Data library is "the first time a map app has ever actually put something on my
screen" — and it is the **last section of a sheet that opens at half height**, behind sixty-five
words about `.shp`, `.dbf` and `.shx`. Everything above the fold is file management, which is the one
thing she cannot do. She failed the task "get my city's bike lanes on the map" outright.

## What D changes

The Open sheet is reordered by **what the person already has** — an appetite, a link, or a
download — instead of by the mechanism the app uses.

| | Alt C order | **Alt D order** |
| --- | --- | --- |
| 1 | Browse map files… | **Search** the catalogue |
| 2 | On this iPhone | **Browse data** — Add tiles + "Browse all data" |
| 3 | More ways to open (Shapefile folder, Any file, Paste) | **I have a link** — a real, visible field |
| 4 | Data library, ending in "More data on nimblemap.com" | **My downloads** (C's phone list) |
| 5 | — | Collapsed: "Downloaded a .zip, or several files?" |

Concretely:

- **The sheet is titled "Add data", not "Open"**, and opens at `.large`. All four sections are
  visible without scrolling. The map pill and the start-card button say "Add data" too — Marisol read
  C's "Open a map file" as "files only", concluded she had none, and stopped on the first screen.
- **Search works.** `Library.search` existed in C and **had no callers**; there was no `.searchable`
  anywhere in the app, so the four bundled sets were unfindable by name. A miss is not a dead end: it
  says where that kind of data is actually published and offers the link field and the catalogue.
- **"I have a link" is a visible field**, not a bare system `PasteButton` inside "More ways to open".
  `Importer.download` already fetched a URL and guessed the format from the MIME type when the URL
  lied — a working capability that nothing in the UI advertised. This is the single move that fits
  Marisol's actual skill ("hold the download button → Copy Link") and it works on every portal she has
  bookmarked. Its failure text now says what to do instead of scolding about `https://`.
- **"Browse all data" is a full-width row** directly under the Add tiles — not the last tile of a
  horizontal strip, and not the last line of the last section. It keeps C's in-app web view on
  `nimblemap.com/data.html` (9 themed sections, 67 links, 22 directly openable).
- **"On this iPhone" is "My downloads"**, and its empty state says what will fill it. "List map files
  from a folder…" is "Show my Downloads folder…", with one line explaining *why* iPhone needs this.
- **The jargon is collapsed** into one disclosure at the bottom, which leads with the most useful
  fact in the app and the one C never stated out loud: **a .zip opens as it is.**
- **An error that named a button which does not exist is fixed.** C's `lonePartsMessage` told people
  to tap "Open a folder…"; no spike has ever had a button with that name.

## NetCDF grids (climate and population)

A and B and C could not open NetCDF at all, so the two climate entries on the site's data page
(`.nc` files) could only ever end in "NimbleMap can't open .nc files yet." D opens them.

- **Why a new reader.** `rustgis-data` reads NetCDF only by shelling out to GDAL's
  `gdal_translate` (`crates/data/src/raster/gdal.rs`), and there is no GDAL on an iPhone. The
  desktop reader (`crates/lm-jobs/src/import/netcdf.rs`) is pure Rust but is wired into `lm-core`'s
  layer and grid profiles, the time slider and the variable dropdown. So `src/netcdf.rs` is a small
  reading of the same format with the same `netcdf-reader` crate, cut to what the spike shows:
  **one variable, its first time step and first level, as one grid of values.**
- **What it keeps**, because leaving these out gives wrong maps rather than missing ones:
  latitude/longitude found by CF `units` and `standard_name` (not just by name), `scale_factor` /
  `add_offset` unpacking and `_FillValue` / `missing_value` masking, longitudes of 0–360 wrapped to
  −180–180, and a grid stored south-to-north turned north up.
- **What it drops:** picking a variable, the time axis, levels, and the colour range sampled across
  time. Curvilinear and projected grids are refused with a plain sentence, not a crash.
- **The file names the quantity.** A GeoTIFF says nothing, so C guessed "Height"/"m" from the file
  name. A NetCDF variable carries `long_name` and `units`, so the card and legend use those;
  machine-written units ("degC") are shown the way people write them ("°C").
- Everything after the read is shared with GeoTIFF: the same Web Mercator resample, Viridis ramp,
  values grid for "tap to read the value", legend and See-through slider.

Verified against NOAA's NCEP monthly mean air temperature (26 MB, NetCDF-3, 0–360 longitudes):
`long_name` "Monthly Mean Air Temperature at sigma level 0.995", 144×73 cells, extent
−180…178.75 / −90…90, range −41.0…34.2 °C, drawn with the tropics yellow and the poles purple.

## Known gaps (not addressed in D)

Found in the study, deliberately out of scope for this spike — they are real and worth their own pass:

- **No `WKDownloadDelegate` anywhere.** Every "Open download page ↗" link on the data page that ends
  in a blob or POST download fails silently — no error, no toast.
- **The size cap is checked after the download finishes** (`Importer.swift`, `maxBytes` = 60 MB), and
  the data page links a 220 MB CSV, a 263 MB zip and a 161 MB NetCDF.
- **`com.esri.shapefile` is not in `LSItemContentTypes`** (only exported), so Share → NimbleMap never
  appears for a `.shp` — the format most portals hand you.
- **`web/lm/data.html` carries Mac-only instructions** (Finder, double-click, right-click, ⌘S) that
  are read inside an iPhone web view.
- **The catalogue has no per-portal recipes.** The study's strongest suggestion: three cards —
  Socrata, ArcGIS Hub, CKAN/data.gov — each giving the Export → Copy Link gesture for that portal
  family would unlock most of a collector's bookmarks at once.

## Build

Same as the other spikes. `scripts/build-rust-ios.sh` builds this spike's copy of the decode crate
(`lm-ios-alt-d-decode-ffi`) from Xcode's build phase. Regenerate the project's file list after adding
Swift files with `scripts/sync-xcodeproj.py ios/NimbleMapAltD.xcodeproj`.

```
xcodebuild -project ios/NimbleMapAltD.xcodeproj -scheme NimbleMapAltD -sdk iphonesimulator \
  ARCHS=arm64 ONLY_ACTIVE_ARCH=YES build
```

Spike-only environment: `LM_FRESH=1` forgets the saved map; `LM_SCENE=open|layers|look|feature`.

Two notes for anyone building these spikes on a fresh machine:

- `scripts/*.sh` must be executable. Alt C was committed with mode `100644` and could not build from
  a clean clone at all (`Permission denied` in the Build Rust FFI phase); A, B and D are `100755`.
- A conda base environment exports `LD`, `CC`, `AR`, `RANLIB`, `CFLAGS` and `LDFLAGS`, which hijack
  Xcode's clang and linker (`ld: unknown option: -Xlinker`). Unset them before building.
