# iPhone companion spike — Alternative C (A's file flow + B's first run)

Throwaway spike for **Alternative C**, built from the A vs B novice persona study (2026-11-02; two
personas, seven tasks each, iPhone 18 Pro simulator). The study's recommendation: A's file flow and
clean map, plus B's first run and insight layer, and fix B's stuck "Change look".

**Not product code.** Lives under `spikes/` and will be thrown away or rewritten when the real app lands.

| | Alt A | Alt B | **Alt C** (this spike) |
| --- | --- | --- | --- |
| Path | `spikes/ios-companion/` | `spikes/ios-companion-alt-b/` | `spikes/ios-companion-alt-c/` |
| Bundle id | `com.nimblemap.companion.spike.alta` | `com.nimblemap.companion.altb` | `com.nimblemap.companion.altc` |
| Home screen name | NimbleMap A | NimbleMap B | **NimbleMap C** |

## What C takes from each

- **From A:** floating Open and Layers pills over a clean map; recents on the start card; the ranked
  "On this iPhone" list (a Shapefile's parts shown as one entry); the Layers list with eye, Edit to
  reorder, swipe to remove; A's full style page (now under Change look › More options).
- **From B:**
  - The sample map and one hint on first run.
  - Insight cards ("22nd highest of 177") and data cards ("Most common category: Food (8)").
  - The in-app data library with Add buttons, in place of A's web view.
  - "Change look" with picture previews, the Undo toast, and the map-style and show-all buttons.
- **Dropped:** A's Ask pill; B's persistent sheet and search. (A's web view on the site's data page was dropped too, then brought back: see the owner's notes below.)

## Study fixes (in C's copies of the shared files)

| Finding | Fix |
| --- | --- |
| B: Change look can't be closed | Cancel and Done sit inside the sheet; swipe down keeps the look; the map behind stays live |
| Both: new areas cover the user's pins | A new layer goes on top of its own kind (places > lines > areas); the list still shows the draw order (`MapDocument.add`) |
| Both: map not restored on relaunch | Layers, order, visibility and look are saved (`SavedMap.swift`) and reopened; the camera fits them |
| B: the demo layer stays under the user's data | The sample is put away when the user's own file arrives, with Undo, and is never saved |
| Both: notes hidden in More details | The notes (`description`, `notes`…) show first on the card; style ids and a folder that repeats a column are hidden |
| Walt: "Tell me who, not just where" | First name + surname columns make the title, and the subtitle reads "Born · 1812 · Haworth" |
| Both: no Year in Color by | Year columns can be coloured (a ramp labelled "1812", not "1,812") but never become the insight number |
| Both: no legend on the map | A small legend on the map (round two: every visible layer; tap → Layers) |
| A: legend shows "Other" with 3 categories | "Other" only when some values didn't get a colour |
| KML colours ignored | `FileColors.swift` reads `<Style>` colours and ties them to the category column that matches; the layer opens in its My Maps colours |
| "22 lisbon trip" | "22 places" / "12 areas" unless the name is a plain plural ("177 countries") |
| KML labelled "Google Earth" | "My Maps / Google Earth" |
| Walt: the Open sheet buried the phone list | The phone list moved up, with "choose the folder; the files inside look greyed out" (round two: Browse map files, On this iPhone, More ways to open, Data library) |

## Second round: fixes from the fresh-persona run of Alt C

| Finding | Fix |
| --- | --- |
| A card stated two areas ("42.1 ha" and "AREA HA 354.8") | When the file has its own area (or length) column, the card shows only that, under More details; the headline prefers a non-size number (population) |
| The ">" in Layers hid the layer | The eye sits at the row's leading edge; the rest of the row, ">" included, opens the layer card |
| A new layer blended in and wasn't announced | A new layer gets the one-color choice farthest from colors on the map, an "Added Family places · 20 places" toast with Undo, and a row in the on-map legend, which now lists every visible layer |
| Raw names and repeats ("POP1851", "Keighley · Keighley · Parishes 1851 · Parishes 1851", five identical earthquake rows) | Column names are made readable ("Population 1851", "Area (ha)", "Type"); the subtitle never repeats the title or layer name; "N things here" rows add up to two distinguishing facts (event and year, magnitude and date) |
| "Zoom to" hid the place behind the card; relaunch showed everything | A point is centered in the space above the card; the view is saved when the camera settles and restored exactly |
| Open sheet: a file listed twice, Browse pushed out of view, picker starting at On My iPhone | Browse map files comes first; the "Opened before" list is gone (On this iPhone already lists them); the picker opens in the last folder used |

## Rasters (map images and grids of values)

A and B could not open rasters (the importer listed vector formats only). C opens GeoTIFF (`.tif`,
`.tiff`) and ASCII grids (`.asc`):

- **Rust** (`src/raster.rs`, `lm_open_raster`): reads the raster with `rustgis-data` (any EPSG code
  it knows, e.g. British National Grid), resamples it onto a Web Mercator grid (≤ 1024 px a side),
  and writes a PNG. RGB, palette and 8-bit grey rasters are pictures, shown as they are. Other
  single-band rasters are values, colored on the Standard ramp between their 2nd and 98th
  percentiles, with no-data left transparent. Value rasters also get a little-endian f32 grid for taps.
- **Map:** an `MLNImageSource` pinned at the corners plus an `MLNRasterStyleLayer`. Rasters draw
  below areas, lines and places.
- **Tap:** "Height here: 259 m · One of the highest spots on this map". The quantity and unit are
  guessed from the name (elevation → Height, m). A picture's card explains See-through.
- **Change look:** a See-through slider (0–100 %); no "Show by" or "More options".
- **Legend, data card, Files:** the legend shows the ramp's ends, or a picture icon. The layer card
  gives the range and cell size. The Files picker and "On this iPhone" list GeoTIFFs (a plain photo
  .tif without georeferencing tags is left out).

## Third round: fixes from the fresh-persona raster run

| Finding | Fix |
| --- | --- |
| "2 things here" grouped Haworth and Keighley and left out the town | A tap keeps the nearest place plus only those drawn within 8 pt of it; people's rows keep the town ("Mary Ackroyd · Born · 1815 · Keighley") |
| The key read "11.5 ━ 226.5" | "Low 11.5 m … High 227 m", and the legend title names the quantity ("height (m)"); a value beyond the key says so on its card |
| Fading a map image was three screens deep | A See-through slider with a percent sits on the map image's row in Layers; the hint says "see the streets through it" for value grids |
| No ground height for a pin | A pin's card shows "Height here: 59.6 m · Lisbon elevation" for each grid of values under it; "N things here" lists the grid too |
| Adding data zoomed out to the whole Atlantic | Added on top of a map in view, the camera stays and the toast says "13 of 20 in view" with Show all and Undo (it still flies when none of it is in view, or when it's the first data) |

## The owner's notes on the iPhone (2026-11-03)

| Note | Change |
| --- | --- |
| The Layers pill repeats the legend next to it | The pill is gone; the legend is the Layers button, with a "3 layers ›" header. Hidden layers stay in it, dimmed, so it never disappears |
| The site's Data tab is a good way to browse datasets | Open › Data library › "More data on nimblemap.com" shows `nimblemap.com/data.html` in the app (A's web view); a link to a map file opens it on the map |
| Test files on the phone | Documents is shown in Files as On My iPhone › NimbleMap C (`UIFileSharingEnabled`, `LSSupportsOpeningDocumentsInPlace`); the app's own copies moved to Application Support/Data |

## Build

Same as Alt B: `scripts/build-rust-ios.sh` builds this spike's copy of the decode crate
(`lm-ios-alt-c-decode-ffi`) from Xcode's build phase. Regenerate the project's file list after adding
Swift files with `scripts/sync-xcodeproj.py ios/NimbleMapAltC.xcodeproj`.

```
xcodebuild -project ios/NimbleMapAltC.xcodeproj -scheme NimbleMapAltC -sdk iphonesimulator \
  ARCHS=arm64 ONLY_ACTIVE_ARCH=YES build
```

Spike-only environment: `LM_FRESH=1` forgets the saved map; `LM_SCENE=open|layers|look|feature`.
