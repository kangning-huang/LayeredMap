# iOS redesign: map first, tap first, data without files

*Status: proposal, not decided. Dated 2026-10-31. Not an ADR yet.*
*Reviews: PR #50 (iPhone spike), PR #51 (`ios-open-paths.md`), PR #52 / #53 (`ios-layers-symbology.md` and its spike).*
*If accepted, this page replaces parts of both debate docs. See "Changes to existing decisions".*

> **Postpone every search option that costs money per query.**
> Version 1 searches only free sources: our own library, open-data portal APIs, and pages the
> person shares into the app. No paid web-search API (Brave, Bocha, Vertex AI Search) and no paid
> place-search provider ships in v1. We revisit this only after free search is live and the
> numbers show we need it. See [Postponed: paid search](#postponed-paid-search).

---

## Summary

1. **Keep the stack.** SwiftUI + MapLibre Native (Metal) + Rust decode is right. Gestures never
   wait on decode.
2. **Rebuild the interaction model.** The current design is a desktop GIS on a small screen. It
   answers "how do files get in" and "how do I manage layers". It does not yet answer "what does a
   person do with a map on an iPhone".
3. **Build on three habits every iPhone user already has:**
   - tap a thing to get a card (Apple Maps),
   - add with "+" (every app),
   - edit like Photos (preview, then Done).
4. **Port the desktop's best idea first:** tap → insight ("3rd hottest of 17") → three actions →
   preview → keep. Today this loop has no iPhone design at all.
5. **Data search is free in v1.** Catalogs first, shared pages second, paid web search postponed.

## Who the first iPhone user is

Owner (Ken, 2026-10-31): a person who **has never used GIS** and is **curious about data online
that Google Maps and Apple Maps cannot open**.

What this means for design:

- They do not know the words layer, shapefile, CRS, basemap or symbology. Principle 11 applies to
  every screen.
- They find data in Safari, in a news story, or in WeChat. NimbleMap must be the app that **opens**
  what they find, not only an app that **searches**.
- They have no files of their own on day one. The first minute must work with no file at all.
- They judge the app in the first 30 seconds. A wall of choices loses them.

## Keep (from PRs #50–#53)

| Item | Why keep |
|---|---|
| SwiftUI + MapLibre Native + Rust FFI decode | Butter-smooth gestures; decode never blocks the UI |
| Basemap is not a data row and cannot be turned off | Correct for novices |
| Live legend swatch on each data row | The list matches the map |
| Swipe-left red Remove; standard drag reorder | Same as Reminders and Mail |
| Share sheet / Open In (path B) | This is how files arrive on iOS |
| Copy into the app on import | Simple mental model; works offline |
| "Ignore": tab bar, side drawer, ribbon | Correct |

## Tear down

| Now | Problem on iPhone | Replace with |
|---|---|---|
| Blue "Open URL" pill, top-left | The most expert action gets the strongest color. Top-left is the "Back" spot and is hard to reach with one hand. | "+" next to search. "Paste link" lives inside it. |
| Status strip at the bottom | A desktop status bar. In the spike screenshot it covers the MapLibre logo and the attribution button, which must stay visible. | Progress on the data row while it loads. A short toast only for errors and Undo. |
| Ask bar as the main bottom control | It is the Pro (LLM) feature, it is a stub, and it shows with no data. Breaks principle 12 (deterministic before AI). | Last row of search: "Ask about this map". Ship after the tap → card loop works. |
| Three floating controls in two corners | No single home. Eyes jump from top to bottom. | One persistent bottom sheet (like Apple Maps and Find My). Small controls top-right: map style, my location. |
| Layers as a modal sheet (state B) | The doc says the map still reacts while it is open. The code uses a plain `.sheet`, which blocks the map. | The same list inside the persistent sheet at medium height. The map stays live. |
| Row tap opens Style (state C) | Hides the data's insight, assumptions and actions. Style is one action, not the whole layer. | Row tap opens the data card. "Change look" is one action on it. |
| Style = 11 color dots + size + opacity | "Which color" is the least useful style question. The v1 scope says "color by a field", but that is not built. | "Show by" previews (see Change look). |
| Footer text: "Basemap stays on and is not listed. Swipe left to Remove. Edit to reorder." | If a list needs a manual, the list is wrong. It also teaches the word "basemap". | Delete it. |
| Browse-tap (path A) as the main open path | A web page inside a native app. Small links, downloads can jump to Safari, two catalogs to keep in step. | Native data library (path F) from the same catalog JSON. One "More on nimblemap.com" link. |
| Google data search (path G) | Google's Custom Search JSON API is closed to new customers and ends on 2027-01-01. Google is blocked in mainland China. | Free catalog search now. Paid web search postponed. |
| First-run card with six choices | Too many doors. Principle 10 says three at most. | Example data already on the map, and one hint. |

## The new model

### 1. Maps, not files

- The thing a person owns is a **map**. A map saves itself, like a Freeform board or a note.
  There is no Save button.
- Data added to a map is **copied into it**, like a photo put into a Keynote slide.
- The library shows map thumbnails. Names come from the content: "Schools · Shanghai".
- `.lmap` is how a map **leaves** the app (Share) or syncs with the Mac later. It is not the home.
- In v1, do **not** show the app's folder in the Files app. One library is easier than two.
- No file paths and no file extensions in primary UI. Words: "Add data", "Remove from map",
  "Map style".

### 2. Home: the map and one sheet

```
┌────────────────────────────────┐
│                       [map][⌖] │  map style · my location
│                                │
│            full map            │
│                                │
│   ( Tap any area to learn )    │  one hint, first run only
│                                │
├────────────────────────────────┤
│              ───               │
│ [ Search places or data ]  (+) │  small height
│ World example · 1 data set     │
└────────────────────────────────┘
```

The sheet has three heights:

| Height | Shows |
|---|---|
| Small | Search field and "+" |
| Medium | Data on this map: swatch, name, eye; Remove by swipe; drag to reorder |
| Large | Your maps (library) and recent data |

- The sheet title is the map's name. The word "Layers" is not needed.
- The sheet never closes fully. The map stays live at small and medium heights.
- **First run:** open with example data already on the map and the single hint "Tap any area to
  learn about it". No welcome card.

### 3. Tap anything → card

Like a place card in Apple Maps.

```
┌────────────────────────────────────┐
│ District 7                     (x) │
│ Districts · area · 23.4 km²        │
│ ┌────────────────────────────────┐ │
│ │ Avg. temperature 31.2 °C       │ │
│ │ 3rd hottest of 17              │ │
│ │ +2.1 °C above the median       │ │
│ │ ·  · ·  | ·  ·   ●             │ │
│ └────────────────────────────────┘ │
│ [Summarize heat][Compare][Measure] │
│            More actions            │
└────────────────────────────────────┘
```

- Order: name, kind and size, up to three insight rows, up to three actions, "More actions".
  This is the desktop object card (`spec.md` › Inspector: object card), unchanged in content.
- Hit target at least 44 pt. If several features are under the finger, show a small menu:
  "3 things here".
- **Long-press** on the map drops a pin and shows the value of every visible layer at that point.
  This is also how a person reads a raster cell.
- **Tap a data row** in the sheet → data card: legend, assumptions (purple dot, Change), field
  summary, three actions. "Change look" and "View as list" are actions here.
- Assumptions must be visible on the phone too (principle 1). A purple dot on the row and a line
  on the data card.

### 4. Actions: like editing a photo

1. Tap an action. The sheet shows the sentence composer: "Count Schools in each District". Each
   token is a menu.
2. Tap Preview. The sheet lowers. The striped preview shows on the map.
3. A bottom bar shows **Keep** and **Discard**.
4. Keep shows the result card (top-5 bars, "Show all 17").

### 5. Change look

Like Photos edit mode: the map stays big, the controls sit in a strip at the bottom.

```
┌────────────────────────────────┐
│ (Cancel)                (Done) │
│                                │
│       map, live preview        │
│                                │
├────────────────────────────────┤
│ Show by                        │
│ [One color][Temp.][Pop.][Type] │  live thumbnails
│ ◐ ─────────────●──────         │  opacity
└────────────────────────────────┘
```

- "Show by" lists the data's fields as small previews of the map colored by each one. Tap one to
  apply it.
- Ramps and classes follow `spec.md` › Data palettes (viridis default, "Heat" for temperature,
  Okabe–Ito for categories). No ramp editor in v1.
- "Customize" (palette chips, classes) comes later, behind one link.

### 6. Add data ("+")

```
┌────────────────────────────────┐
│ Add data                   (x) │
│ [ Search the data library ]    │
│ Data library                   │
│ [World cities]  [Summer heat]  │  thumbnails
│ ─ From Files                   │
│ ─ Paste link                   │
│ Got a file in Mail or WeChat?  │
│ Tap Share, then NimbleMap.     │
└────────────────────────────────┘
```

Order by what the person wants, not by mechanism:

1. **Data library** (native list from `library-catalog.md` JSON): thumbnail, title, kind, size,
   license in plain words, "Add".
2. **Search** (free sources only; see Data search).
3. **From Files** (document picker, copy on open).
4. **Paste link** (system `PasteButton`, so no paste permission prompt).

### 7. Files from other apps

When a file arrives from Mail, WeChat, AirDrop or Safari:

- The app opens a small sheet: "Add schools.csv to *Shanghai schools*". Second choice: "New map".
- Formats for day one, ordered by what phone users have: CSV with coordinates, GeoJSON, KML/KMZ,
  GPX, shapefile as `.zip`, GeoPackage. The Rust data crate already reads most of these.
- A lone `.shp` without its partner files: "This shapefile is missing its partner files. Ask the
  sender to share the .zip." Never a technical error.
- After any add, **fly the camera to the new data**. The spike does not do this yet, so data far
  from the view seems to do nothing.

### 8. Share a page to NimbleMap (new)

For our first user this may be the most useful path, and it costs nothing per query.

```
┌────────────────────────────────┐
│ This page has 3 map files      │
│ From example-city.gov          │
│ Parks    Areas · 2.4 MB   (Add)│
│ Trails   Lines · 0.8 MB   (Add)│
│ Benches  Points · 0.1 MB  (Add)│
│ [ Add all to a new map ]       │
│ 1 file skipped: a PDF report   │
└────────────────────────────────┘
```

- The person shares a **web page** (not a file) from Safari or WeChat.
- The search service finds the map files on that page:
  - Known portals (ArcGIS Hub, Socrata, CKAN, data.gov): ask the portal's API for the page's files.
  - Other pages: scan the links, then run the file check (below).
- Needs a Share extension (or an App Intent) that accepts URLs.

### 9. Smaller pieces

- **View as list:** on iPhone the table is a list like Contacts: search field, sort menu, tap a
  row → its card. The grid stays for iPad.
- **Undo:** after Remove or Keep, a toast: "Removed Roads · Undo". Shake to undo also works.
- **Share:** Map image · Data (CSV, GeoJSON) · NimbleMap map.
- **Ask (Pro):** a row at the bottom of search results, "Ask about this map". Not a permanent bar.

## Data search

### Principle

The engine is the small choice. The big choice is **where results come from**. A web search
engine guesses from a link whether a result is a map file. A data catalog stores the format,
size, license and often the area. So the format gate is exact.

### v1: free sources only

| Source | Why | Notes |
|---|---|---|
| NimbleMap library (`library-catalog.md`) | Curated, previewed, known to open | Primary |
| ArcGIS Hub Search API | Follows OGC API – Records; filters such as `type='Feature Service'` | Public items; check Esri terms |
| Socrata Discovery API | Searches government datasets across all Socrata portals; no key needed | Free app token raises rate limits |
| CKAN portals (HDX, data.gov.uk, open.canada.ca, EU portal, many cities) | One connector covers many portals; HDX has a "Geodata" filter | `package_search` with format filters |
| data.gov Catalog API | Replaced CKAN in 2026; has spatial lookups | Check whether a key is needed |
| Chinese open-data portals | Needed for China users | **Not checked:** which portals have an API |

### Result groups (one search field)

1. **Ready to map**: catalog results.
2. **From your shared pages / recent data**.
3. *(Postponed)* **Places**: brand or place-type queries ("Starbucks", "hospitals") as points.
4. *(Postponed)* **From the web**: checked files found by a web-search API.

When v1 finds nothing, say so in plain words and point to the free path: "No ready-to-map data for
'starbucks'. Found it on a web page? Share the page to NimbleMap."

### Result card

Thumbnail map preview · title · kind in plain words ("Points", "Areas", "Grid") · size · license in
plain words ("Free to use", "Credit the source") · source name · "Add". Never a raw URL as the
main text.

### Ranking for novices

Has a preview; covers the current map area; small enough for a phone; recently updated; open
license. Dull titles are normal for good datasets, so do not rank by title wording alone.

### File check (all sources)

Before a result shows as "Opens in NimbleMap":

- the link downloads (no auth wall, no HTML error page behind a `.geojson` URL);
- the first bytes match the claimed format;
- a CSV has coordinate columns (same detection as `spec.md` › Assumption catalog);
- a `.zip` holds a complete shapefile set or another supported file;
- the size is under the phone limit.

### Search service (server)

Needed even with free sources only:

- one endpoint for the app; the app never calls providers directly;
- providers can change without an app update (Google's shutdown shows why);
- region routing (China / elsewhere);
- the file check runs once, on the server;
- catalog results can be cached.

This fits the existing guardrail: network calls only through the basemap, **search**, library and
update services (`AGENTS.md`).

## Postponed: paid search

**Decision requested: postpone every option that costs money per query or needs a paid server.**

| Option | Status (checked 2026-10-31) | Cost | Why postponed |
|---|---|---|---|
| Google Custom Search JSON API | Closed to new customers since 2026-01-20; ends 2027-01-01 | — | Not available |
| Bing Search API | Retired 2025-08-11 | — | Not available |
| Brave Search API | Available; supports `ext:` / `filetype:` (experimental) | 5 USD per 1,000 queries; 5 USD free credit a month | Costs money per query. Terms forbid storing or caching results beyond short-term use. |
| Bocha (博查) web search | Available; Chinese web; up to 50 results per query | Paid, prepaid balance (price not checked) | Costs money. File-type filtering not confirmed. |
| Vertex AI Search | Google's suggested successor, for up to 50 sites | Paid Google Cloud product | Costs money. Blocked in mainland China. |
| Places via OpenStreetMap (Overpass) | Public servers ask commercial apps to self-host or pay a provider | Paid provider or own server | Costs money. |

Cost example: if 1,000 people each run 5 searches a day, that is 150,000 queries a month. At
Brave's price:

$$150{,}000 \times \frac{5\ \text{USD}}{1{,}000} = 750\ \text{USD per month}$$

### When to revisit

Revisit paid search only when **all three** are true:

1. Free search (catalogs, library, share-a-page) is live on the App Store.
2. Measured data shows a large share of searches end with no usable result.
3. A budget is approved, or Pro revenue covers the cost (paid web search could then be a Pro
   feature, like Ask).

### Free leads to check before then (not verified)

- **Tianditu (天地图) place search** for mainland China: needs a free developer key with quotas.
  Check its terms before saving results as a layer.
- **MapKit local search** (`MKLocalSearch`) on iPhone: no per-query fee to us. Check Apple's terms
  for showing results on a non-Apple map and saving them as data. In China its coordinates may use
  an offset system.

## Fixes in the spike

- **Build with Xcode 26.** The spike README says Xcode 16.4. App Store Connect requires Xcode 26
  and the iOS 26 SDK for all uploads since 2026-04-28. With that SDK, system controls get Liquid
  Glass; replace the hand-made `.ultraThinMaterial` capsules with system buttons and toolbars.
- **Persistent sheet:** `.presentationDetents([.height(…), .medium, .large])`,
  `.presentationBackgroundInteraction(.enabled(upThrough: .medium))`,
  `.interactiveDismissDisabled()`.
- **Fit the camera** to the data bounds after decode.
- **Never cover attribution.**
- **Feature hit-test** at 44 pt with the overlap menu.
- **Wire MapLibre paint** to `LayerStore` so style changes show on the map (stubbed in #53).

## Test before locking

Five people, including one child. No help. A clickable build.

1. "A friend sent this file in WeChat. Put it on a map."
2. "Which district is hottest?"
3. "Hide the roads."
4. "Color the districts by population."
5. "You found this data page in Safari. Get it on a map." *(tests share-a-page)*
6. "Send this map to a friend."

Pass: four of five people finish each task with no help.

## Changes to existing decisions

| Doc · item | Was | Proposed |
|---|---|---|
| `ios-open-paths.md` · A Browse-tap | Emphasize (lead) | Ignore for v1; one link to the site |
| `ios-open-paths.md` · F Library list | Keep / lean emphasize | **Emphasize: primary** |
| `ios-open-paths.md` · B Share sheet | Emphasize | Emphasize; top build priority |
| `ios-open-paths.md` · (new) Share a page | — | **Emphasize** |
| `ios-open-paths.md` · G Google data search | Keep (secondary) | Replace with free catalog search; **paid web search postponed** |
| `ios-open-paths.md` · Files visible in Files app | Open question | No, for v1 |
| `ios-open-paths.md` · Empty state | Six choices | Example data on the map + one hint |
| `ios-layers-symbology.md` · A idle pills | Emphasize | Replace with one persistent sheet + top-right controls |
| `ios-layers-symbology.md` · B Layers modal sheet | Emphasize | Same list, inside the persistent sheet (not modal) |
| `ios-layers-symbology.md` · Row tap → C | Emphasize | Row tap → data card; "Change look" is an action |
| `ios-layers-symbology.md` · C symbology form | Emphasize | Photos-style "Show by" previews |
| `ios-layers-symbology.md` · Ask pill | Keep | Move into search results |

## Build order

All steps up to 4 have **no per-query cost**.

1. Persistent sheet, tap → card with insight, first run with example data.
2. Add data: library, Files, Paste link; inbound files from other apps; fly to data.
3. Share a page to NimbleMap.
4. Free catalog search (Hub, Socrata, CKAN, data.gov) through the search service.
5. Actions, preview, keep, result card; Change look.
6. *(Postponed, see gate above)* Paid places and web search.

## Out of scope

iPad layout. App Store copy. Merging the spike into product crates. Ask (Pro) design beyond its
entry point.

## Open questions

- What share of first iPhone users are in mainland China? This decides one search stack or two.
- Which Chinese open-data portals have a usable API?
- Should Places ("Starbucks" as points) wait with paid search, or ship if a free source (Tianditu,
  MapKit local search) passes a terms check?
- Does the data.gov Catalog API need a key for our volume?

## Sources

- Google Custom Search JSON API closure: https://www.firecrawl.dev/blog/google-search-json-api-alternative
- Google suggests Vertex AI Search for up to 50 sites: https://brightdata.com/blog/web-data/google-search-api
- Brave search operators: https://api-dashboard.search.brave.com/documentation/resources/search-operators
- Brave operators are experimental: https://safe.search.brave.com/help/operators
- Brave pricing and free credit: https://docs.openclaw.ai/tools/brave-search
- Brave Search API terms (no storing or caching): https://api-dashboard.search.brave.com/terms-of-service
- Bocha web search: https://glama.ai/mcp/servers/@BochaAI/bocha-search-mcp/tree
- ArcGIS Hub Search API: https://developers.arcgis.com/hub/services/search/
- Socrata Discovery API: https://support.socrata.com/hc/en-us/articles/226377827-API-Release-Notes-for-8-30-2016-Discovery-API-Imports-API-and-SNI
- Socrata APIs need no key: https://apify.com/themineworks/socrata-open-data
- HDX Geodata filter: https://docs.humdata.org/find
- data.gov Catalog API: https://docs.rs/data-gov-catalog/latest/data_gov_catalog/
- Overpass API usage policy: https://wiki.openstreetmap.org/wiki/Overpass_API
- Tianditu place search: https://plugins.qgis.org/plugins/qgis_cn_place_search/
- Xcode 26 requirement: https://capgo.app/blog/xcode-26-requirement-for-capacitor-apps/
