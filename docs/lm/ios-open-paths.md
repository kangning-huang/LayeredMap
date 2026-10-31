# iOS open paths: design debate

*Status: debate / not decided. Dated 2026-10-31 (amended same day: Google data search).
Lead candidate from the owner: browse-tap. Not an ADR yet. Layer and symbology chrome on
iPhone is a separate debate (PR #53 spike).*

The iPhone companion spike (`spikes/ios-companion/`, PR #50) already opens a paste HTTPS URL,
downloads off the main thread, decodes GeoJSON in Rust, and draws on MapLibre. Smoothness is
good enough to keep that stack. This page pressure-tests **how people get data into the app**
before productizing Open.

Product principles that matter here: the map is the home screen; feel is a feature; question
budget stays low; technical words are findable, never required (`AGENTS.md`).

## The problem

iPhone users do not have a desktop-style filesystem. Most people open a file by sending a copy
to another app (Share sheet / Open In), by picking something already in Files or iCloud Drive, or
by tapping a download in Safari. A blank "paste a URL" field asks them to leave the app, find a
link, copy it, and come back. That is power-user friction, not a first-run path.

The companion is view / Ask on datasets first. iPad denser chrome comes later. Empty state and
"open something" therefore have to work with one hand, without teaching GIS vocabulary, and
without covering the map forever.

## Candidate paths

| Id | Path | One-line idea |
|---|---|---|
| A | Browse-tap (Ken lead) | In-app browser on the NimbleMap site data / library page; tap download or deep link; app opens and caches |
| B | Share sheet / Open In | Safari, Mail, Files, WeChat, Drive share a file or URL into NimbleMap |
| C | Files picker | Document picker for copies already on the device (including iCloud Drive) |
| D | Cloud pickers | Drive / Box / Dropbox SDKs; defer |
| E | Paste URL | Power-user / fallback; not empty-state primary |
| F | In-app library / Examples | Curated catalog already on desktop; native list or reuse site |
| G | Google data search | In-app search; customized Google inquiry returns only NimbleMap-openable geospatial files |

### A. Browse-tap (lead)

**Happy path.** Empty state or Open offers "Browse NimbleMap data…". That opens an in-app
browser (WKWebView preferred for custom "Open in NimbleMap" interception; SFSafariViewController
is simpler and shares Safari cookies, but hands less control to the app). The person lands on the
site's data / library tab, taps a download. The site either serves a file the app registers for,
or a universal link / `nimblemap://` / content-type handler that brings the companion forward with
the dataset. After a successful open, the app copies into its Documents (Files-visible) so the
next launch can show Recents without the network.

**Failure modes.** Offline or slow network; site page not phone-tuned; download behind auth the
in-app browser does not have; link opens Safari instead of the app; large file kills memory;
user abandons mid-browse with no map yet.

**Trust / security.** Prefer allow-listed hosts (nimblemap.com and known library mirrors) for
automatic download. Arbitrary https from a page the person navigated to still needs the same
size / type / decode guards as paste-URL. Never execute page script as app logic.

**HIG / App Store.** An in-app browser that only frames your own site is normal. Do not trap the
person with no way to Safari or Done. If using a custom URL scheme, document it; universal links
are preferred for https continuity. Review cares that downloads are user-initiated.

**Files cache.** On success, write under the app container with a clear name (title + date or
catalog id). Expose via `UIFileSharingEnabled` / Files app if we want "already on my phone"
visible outside the app; otherwise keep in app Documents and list Recents inside Open.

**Empty state.** Primary button: Browse NimbleMap data. Secondary: Open from Files… /
Examples. Paste URL behind "More…".

**WKWebView vs SFSafariViewController.** WKWebView: intercept navigation, inject open buttons,
stay in product chrome. Cost: cookie and password state diverge from Safari; more review surface.
SFSafariViewController: trusted chrome, shared Safari session; harder to catch a download into
the app without a universal link. Debate leans WKWebView for the data tab if the site adds
explicit open-in-app links; otherwise SFSafari + universal link may be enough.

### B. Share sheet / Open In

**Happy path.** Person has a `.geojson` / `.csv` / `.lmap` (or a link) in Mail, Files, WeChat,
Safari download list, etc. Share → NimbleMap, or Open In → NimbleMap. The system launches the
app with a security-scoped URL. App copies into Documents, decodes, shows the map.

**Failure modes.** App not in the Share sheet (missing UTI / share extension); WeChat or other
apps strip types; temporary inbound URL expires if not bookmarked or copied; multi-file shapefiles
arrive incomplete.

**Trust / security.** User explicitly sent the file to us. Still validate type and size before
decode. Do not follow arbitrary redirects from a shared URL without the same host rules as A/E.

**HIG / App Store.** Declaring document types and appearing in Open In is expected. A share
extension is optional for v1 if the full app open is enough.

**Files cache.** Same as A: copy in, then Recents.

**Empty state.** Hint line: "Or share a file into NimbleMap from Mail or Files." Not the only
primary, because first-time users often have nothing to share yet.

### C. Files picker

**Happy path.** Open → "On My iPhone / iCloud Drive…" → `UIDocumentPickerViewController` for
imported types. Person picks a file already saved (including a previous NimbleMap cache if Files
sharing is on).

**Failure modes.** Person cannot find the file; iCloud not downloaded locally; picker feels like
"computer files" and intimidates novices; shapefile sidecars.

**Trust / security.** System-granted access; use security-scoped bookmarks if opening in place
(`LSSupportsOpeningDocumentsInPlace`) or copy-on-open for a simpler mental model.

**HIG / App Store.** Standard. Prefer copy-into-app for v1 so Recents and offline match A/B
without lingering bookmark renewal bugs.

**Empty state.** Secondary control, not the only door.

### D. Cloud pickers (defer)

Drive / Box / Dropbox SDKs add sign-in, App Store privacy labels, and China storefront issues.
Prior direction: defer. Person can still use the Files provider for iCloud, or share from the
vendor app (path B). Revisit when desktop cloud open is productized and phone demand is clear.

### E. Paste URL (fallback)

**Happy path.** As in the spike: paste https, download, decode, map.

**Failure modes.** Typos; http; auth walls; huge files; user does not know what a URL is.

**Trust / security.** Same allow-list / size / content-type checks. Prefer https only.

**Empty state.** Behind More or a small "Open URL…" on the Open sheet. Never the only big button.

### F. In-app library / Examples

Desktop already has Examples and a data library catalog (`docs/lm/library-catalog.md`). Phone can
ship a native list (titles, sizes, Add) talking to the same catalog JSON, without a browser.

**Relation to A.** Native list is faster and offline-friendlier for curated sets; browse-tap
covers the full website and future pages we do not want to re-implement in Swift. Proposed:
Examples / library as a first-class list inside Open, and browse-tap for "see everything on the
site" and marketing data tabs. They share the same post-open cache.


### G. Google data search (Ken, 2026-10-31)

**Intent.** After layers/symbology, open-paths continue. Ken: let people search for datasets in
the app with a **customized Google inquiry** so results are only data NimbleMap can open — not
general web pages. Example: type "Starbucks" → shapefiles, GeoPackage, or CSV with lat/lon —
never HTML, PDFs, images, news, or unrelated downloads. (Wording: shapefiles, not "sharp files".)

**Working title.** Google data search. Format-gated search; not unconstrained Google.

**Happy path.** Open sheet offers "Search for data…" (secondary/tertiary; browse-tap and Recents
stay ahead). Person types a topic. The app builds a biased Google query (extensions / filetype /
geo keywords, optional known open-data `site:` hosts), runs it through a sanctioned search API
(not a raw scrape), and lists only candidates that look like openable geospatial files. Person
picks one → download → copy into Files/Documents cache → same decode pipeline as A/B/C/E.

**Allowed result types (v1).**

- Shapefile — typically a `.zip` of `.shp` + siblings, or a discoverable multi-file set the
  download can assemble
- GeoPackage — `.gpkg`
- CSV with latitude/longitude columns (or clearly geo CSV)

**Must not return.** HTML pages, PDFs, images, news articles, app stores, proprietary GIS
formats NimbleMap cannot open yet, or any result that is not a plausible downloadable file of
the types above.

**Query biasing (design).** Append OR-grouped filetype/extension terms, e.g. `filetype:shp`
`filetype:gpkg` `filetype:csv` `filetype:zip` plus geo keywords (`shapefile`, `geopackage`,
`geojson` only if/when listed in allowed types, `lat`, `lon`, `WGS84`). Optional `site:` filters
for known open-data hosts (data.gov, ArcGIS Hub, university mirrors, NimbleMap library) to raise
precision. Exact operator set is an implementation detail; the product rule is **format-gated**.

**API vs scrape.** Prefer Google Programmable Search / Custom Search JSON API (or equivalent
documented search API) with an app key, quota, and ToS-compliant use. Raw HTML scrape of google.com
is fragile, against ToS, and a review/App Store risk — **ignore** as a design path. Tradeoffs to
weigh later: API cost and ranking vs. control; whether a Custom Search engine restricted to
allow-listed hosts is enough for v1 or whether the open web with post-filters is required.

**Post-filter.** Before listing a hit: check URL extension / Content-Type / MIME from HEAD when
cheap; drop HTML and non-geo types. For CSV: heuristic on filename/snippet, and when feasible a
ranged GET or small sample to confirm lat/lon (or lon/lat) columns exist — do not list a bare
CSV as openable without that signal. For shapefile zips: prefer links that look like archives;
note multi-file packs may need a zip or a folder the downloader understands. Dead links and
soft-404 HTML behind a `.shp` URL are failures, not results.

**UX placement.** Secondary or tertiary on the Open sheet relative to locked priorities:
browse-tap NimbleMap site links and Files/Recents cache stay primary. Proposed sketch: Primary
Browse NimbleMap data; then Recents / Examples / On my iPhone; then **Search for data…**; Paste
URL still under More. Empty-state footnote can mention search once browse-tap ships. Do not make
Google search the first-run story — curated NimbleMap data teaches the product; search is for
"I have a topic, find me a file."

**After pick.** Same pipeline as other open paths: download off the UI thread → write into app
Documents → Recents → decode via existing Rust FFI. Failures get plain-language errors (bad link,
not a map file, CSV has no coordinates, file too large).

**Failure modes / risks.** Google ToS and API billing; rate limits; false-positive CSV without
coords; shapefile sibling sets incomplete; ranking that still surfaces junk despite filetype
operators; dead or paywalled links; China storefront / Google availability (may need a mirror
search provider or hide the control where Google is unreachable); user expects "Starbucks near
me" POI magic rather than a downloadable dataset.

**Keep / ignore / emphasize.**

- **Emphasize** format-gated search (only NimbleMap-openable files).
- **Ignore** unconstrained Google web search as an in-app results list.
- **Keep** browse-tap as primary for curated NimbleMap data; search does not replace it.
- **Keep** the post-open Files cache pipeline shared with A–F.

## Comparison (keep / ignore / emphasize)

| Path | Proposed for v1 debate | Why |
|---|---|---|
| A Browse-tap | **Emphasize** | Matches how Ken wants novices to discover data; low paste friction; teaches the product site |
| B Share sheet | **Emphasize** | Matches how iOS users already move files; catches Mail / WeChat / Downloads |
| Local Recents / Files cache | **Emphasize** | Makes second open automatic; map home screen stays useful offline |
| C Files picker | **Keep** (secondary) | Needed for "I already saved it"; not the story for first open |
| E Paste URL | **Keep** (secondary) | Spike path; power users and support |
| F Examples / library list | **Keep** / lean emphasize for curated sets | Reuses desktop catalog; pairs with A |
| G Google data search | **Keep** (secondary/tertiary) | Ken: topic → only openable geospatial files; not empty-state primary |
| D Cloud SDKs | **Defer** | Scope, privacy, China; Files + Share cover many cases |
| Teach paste-long-URL as primary | **Ignore** | Lazy-friction; fights novices |
| Unconstrained Google web results | **Ignore** | Pages/PDFs/news are not openable data; fights the format gate |

## Proposed v1 stack (awaiting Ken)

1. Empty / Open sheet: **Browse NimbleMap data** (A), **Examples** (F), **Open from Files** (C),
   share-inbound always registered (B), Recents from cache.
2. **Search for data…** (G) as secondary/tertiary on the same sheet — format-gated Google, not
   primary empty-state.
3. After any successful open (including a search pick): copy into app Documents; show on Recents
   next launch.
4. Paste URL under More.
5. No Drive/Box/Dropbox SDK in v1.
6. Site work: phone-usable data tab and stable open-in-app links (universal link preferred over
   custom scheme alone).
7. Search work (when built): sanctioned search API + post-filter; v1 allow-list shapefile /
   GeoPackage / lat-lon CSV only.

## Technical notes (only as they affect UX)

- **UTIs.** Declare GeoJSON, CSV, and `.lmap` (and later the desktop set) as imported types so
  Open In and the document picker see us. Prefer public / community UTIs where they exist;
  document any custom UTI in Info.plist.
- **Opening in place.** `LSSupportsOpeningDocumentsInPlace` is attractive for huge files; copy-
  on-open is simpler for Recents and offline. Debate default: copy for v1.
- **Security-scoped bookmarks.** Required if we reopen in place or remember iCloud picks without
  copying. Recents that point at our own Documents need no bookmark.
- **App group.** Only if a share extension or future widgets need shared container; not required
  for full-app Open In.
- **Universal links vs custom scheme.** Universal links keep https and App Site Association;
  custom scheme (`nimblemap://`) is easier to test and works from pages that cannot host AASA.
  Prefer universal links for production site → app; keep a custom scheme as a documented
  fallback for spikes and mirrors.
- **Download pipeline.** Reuse spike pattern: never on the UI thread; write temp then rename;
  decode via existing Rust FFI; surface plain-language errors.
- **Google data search (G).** Design-only here: biased query + API results + extension/MIME /
  CSV lat-lon post-filter; then the same download pipeline. No scrape of google.com HTML.

## First-run empty state (sketch)

Map still fills the screen (principle 3). A light card, not a wall:

- Title: "Open a map"
- Primary: Browse NimbleMap data
- Secondary row: Examples · On my iPhone
- Tertiary: Search for data… (G; after browse-tap feels solid, or behind the same sheet)
- Footnote: "You can also share a file into NimbleMap from other apps."
- Chevron or More: Open URL…

Once Recents exist, the card becomes a short Recents list with the same actions below.

## Out of scope here

Layer list, symbology, and Ask chrome on a small screen (next debate, with mockups). iPad
layout. App Store listing copy. Merging the spike into product crates.

## Open questions for Ken

- Is browse-tap the empty-state **primary**, or should Examples win for offline-first demos?
- WKWebView (more control) or SFSafariViewController (more trust, shared cookies) for A?
- Should cached datasets show in the system Files app, or only inside NimbleMap Recents?
- Copy-on-open for every inbound file, or open-in-place for large ones?
- Which formats must Open In support on day one (GeoJSON only, or CSV / `.lmap` too)?
- Must the public site grow a phone data tab and universal links before browse-tap ships, or can
  v1 use a native library list (F) with browse-tap right behind?
- Any China-specific open path (WeChat file open, mirror hosts) that must be in v1?
- Confirm deferral of Drive / Box / Dropbox SDKs until after phone view/Ask feels solid.
- Google data search (G): secondary on Open, or only after browse-tap + Recents ship?
- G: Programmable Search over the open web with post-filters, or a Custom Search engine limited
  to allow-listed open-data hosts for higher precision?
- G: must CSV hits prove lat/lon (HEAD/sample) before listing, or is filename/snippet enough for
  v1 with a clear error on open?
- G: where Google is unreachable (some China networks), hide Search, swap provider, or rely on
  browse-tap / Files only?
