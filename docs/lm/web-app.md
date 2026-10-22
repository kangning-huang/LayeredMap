# NimbleMap in the browser (web preview)

**Status (2026-10-22):** a working prototype on branch `web-app`. The same interface as the desktop
app (`lm_ui::Shell`) runs in the browser through eframe's web runner. The plan is to host it at
**https://nimblemap.app**. Nothing is deployed yet: the steps below are for the owner.

Code: `apps/nimblemap-web` (`src/web.rs` is the browser shell, `src/files.rs` the file handling,
`index.html` the page, `static/` the headers and manifest, `cloudflare/functions` the basemap
proxy, `build.sh` the build).

## Build and run

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked   # the Cargo.lock version of wasm-bindgen
mamba create -n wasmtools -c conda-forge binaryen           # wasm-opt (optional, recommended)
PATH=$HOME/miniforge3/envs/wasmtools/bin:$PATH apps/nimblemap-web/build.sh /path/to/dist
```

- zstd-sys and ring compile C for wasm32. Apple's clang can't, so `build.sh` uses Homebrew's LLVM
  (`brew install llvm`) when it finds it (`CC_wasm32_unknown_unknown`, `AR_wasm32_unknown_unknown`).
- It builds with the `web` profile (fat LTO, one codegen unit; about 2.5 min warm, 5 min cold on
  the reference Mac). Then it runs `wasm-bindgen --target web` and `wasm-opt -O2`, names the files by
  content hash, and copies the page, icons, `_headers`, the examples, the Chinese font and the
  place index. Set `EXAMPLES=0` to leave the examples out.
- The output is a static folder. Any static host works if it sends `.wasm` as `application/wasm`.
  The one non-static part is `/basemap/*` (see Hosting).
- Preview locally with `python3 -I apps/nimblemap-web/serve.py /path/to/dist 8790` and open
  http://127.0.0.1:8790/. The script serves on 127.0.0.1 only, sends brotli the way a CDN would,
  and proxies `/basemap/*` the way the Pages Function does.

URL options: `?lang=ko` sets the interface language for one visit (it follows the browser
otherwise). `?example=storms` opens an example (ids: `world`, `heatwaves`, `storms`, `walls`,
`earthquakes`, `volcanoes`, `airports`, `power`). `?view=lon,lat,zoom` sets the starting view.
`?webgl` forces WebGL2 instead of WebGPU.

**"Desktop is faster" banner.** `index.html` shows a thin strip above the canvas (the canvas starts
below it, so it covers nothing): the browser version is slower than the desktop app, with a
Download button and a close button. It is plain HTML/CSS/JS, so changing it needs no WebAssembly
rebuild, only `build.sh` (or editing `index.html` in the dist). Details:
- Download goes to the Mac `.dmg` on a Mac (GitCode's for Chinese), the Windows installer on
  Windows, and the download page otherwise (iPad, phones, Linux). `window.__nimblemap_banner.os = "win"`
  then `.render(true)` in the console simulates another OS.
- Text in the 11 languages, picked like the app (`?lang=`, then `lm.language`, then the browser).
  It follows `lm.theme` (or the system) for light/dark, and rechecks both every 1.5 s, so the
  app's menu changes it too.
- Closing it stores `lm.web.banner` = `{v: <js file name>, t}`. It comes back after 7 days or on the
  next build.

## Architecture

- **One shell, two hosts.** `apps/nimblemap-web` plays the role `apps/layeredmap` plays on the
  desktop. It creates the `Shell` and passes in the browser's languages and dark mode. It stores the
  prefs in `localStorage` (the same `lm.*` keys as the desktop) and answers the shell's
  `FileRequest`s: Open shows the browser's file picker, Save downloads.
- **Files stay in memory.** A dropped or picked file is read into memory and held in
  `lm_jobs::vfs` under `/dropped/<n>/<name>`, with one folder per drop so a Shapefile's parts sit
  together. It then opens with the usual `Command::Open(paths)`, so the desktop's whole import
  pipeline runs unchanged: format detection, profiling, question cards, CRS. The importers now read
  through `lm_jobs::vfs`, which falls through to `std::fs` on the desktop, where nothing is ever
  held in memory. Nothing is uploaded.
- **Examples load on demand.** They aren't in the WebAssembly. Opening one makes the shell ask
  for `FileRequest::Fetch(paths)` (browser build only). The page fetches `examples/<file>` from its
  own site into `vfs`, then opens it.
- **No threads.** The page runs on one thread (no `SharedArrayBuffer`, so no COOP/COEP headers
  are needed):
  - `lm_core::task::spawn` replaces `std::thread::Builder::spawn` in imports, the table's sorting,
    the place index, previews, export rendering and the analysis job pool. On the desktop it still
    starts a thread. In the browser it queues the work, and the page runs the queue between frames
    for up to about 40 ms per frame (`lm_core::task::pump`).
  - The map renderer (`lm_mapview::worker`) draws on the page's thread when the UI polls for a
    frame.
  - `lm_jobs::Pool` starts no workers in the browser and queues its jobs the same way. Cancelling
    and the stale-result guard work as on the desktop. A long job blocks the page until it ends:
    the analyses tried took well under a second.
  - Real Web Worker threads (wasm threads + `SharedArrayBuffer`) would need
    `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`.
    COEP would then block the cross-origin imagery (GIBS) and the font CDN unless they send
    `Cross-Origin-Resource-Policy`. Not worth it yet.
- **Time:** `std::time::Instant` and `SystemTime` panic on `wasm32-unknown-unknown`, so the shipped
  lm-* code that runs in the browser now uses `web-time`, which is the same `std::time` on the
  desktop.
- **Network:** everything goes through `fetch` and still asks `lm_net`'s gate.
  - Basemap tiles and imagery use `rustgis_tiles::cache::set_fetcher`, with byte ranges for
    PMTiles. Each response up to 4 MB is copied into the **Cache API** store `nimblemap-tiles-v1`,
    keyed by URL and range. It survives reloads, but the browser may evict it, and nothing trims it
    yet. The tile cache in memory (160 MB in the browser) sits in front of that.
  - Protomaps' daily planet builds (`build.protomaps.com`) answer no cross-origin requests. The page
    reads them through its own site at `/basemap/<YYYYMMDD>.pmtiles`, and finds the newest build
    from `/basemap/builds.json` (`lm_mapview::archive::set_found`). NASA GIBS (satellite) sends
    `Access-Control-Allow-Origin: *` and is fetched directly.
- **Fonts and the place index are fetched after start:**
  - Noto Sans SC (8.3 MB) and the GeoNames index (2.9 MB) come from the site, not the WebAssembly
    (`lm_ui::theme::register_font_file`, `lm_ui::search::provide_cities`). Chinese text and place
    search work about a second after the map appears.
  - Bold Chinese uses the regular face, which saves another 8.3 MB.
  - Korean: the desktop reads the system's Korean face, which a page can't. When the interface is
    in Korean, the page downloads Noto Sans KR (SIL OFL) from jsDelivr, pinned to a `google/fonts`
    commit, about 5 MB compressed.

## What works (tested in Chrome 2026-10-22)

| | |
|---|---|
| Launch screen, welcome card, examples menu | yes |
| Examples | all 8. Storm tracks (GeoPackage) and heatwaves (12.6 MB NetCDF-4) included |
| Drop / pick files | GeoJSON, CSV with coordinates (the GCJ-02 question card included), Shapefile (all parts together, or a `.zip`), GeoPackage, GeoTIFF, NetCDF. Also KML/KMZ, GPX, Excel/ODS (same importers; not tried) |
| Basemap | Protomaps vector (Auto/Light/Dark) through the proxy; NASA GIBS satellite; offline Natural Earth. Detail tiles at city zoom checked (Kyoto) |
| Layer cards, styling, filters, table, Find | yes. Filter "depth_km above 300" gave 655 of 14,537 |
| Place search | yes (GeoNames index fetched after start) |
| Time slider | yes (NetCDF time steps) |
| Free analysis actions | composer, preview, Keep result, Undo. "Count points in each county" checked |
| Share › Map image (PNG/PDF) | downloads (1920×1080 PNG checked) |
| Share › Data… | downloads (same `SaveBytes` path) |
| Undo / Redo | yes |
| Languages | follows the browser (`navigator.languages`), menu › Language, or `?lang=`. All 11 languages; Korean needs the font download |
| Prefs | theme, units, basemap, language and view are kept in `localStorage` |

Replacing the native menu bar: a ☰ button beside the app's name has Open…, Open Example ▸, Export
Map Image…, Export Data…, Remove All Layers, Zoom to All Data, View ▸ (appearance, units),
Language ▸, Help (opens nimblemap.com/help.html in the interface language) and Credits. The labels
are the desktop menu's, so they are already translated.

## What doesn't (yet)

- **Sign-in and Ask.** In the browser the Ask box says "Ask about your layers · sign-in comes to the web version
  soon". What's missing:
  - `lm-account`'s HTTP calls still use ureq, which is native only. In the browser they need an
    async `fetch` transport.
  - The sign-in has to be a web redirect: PKCE with `redirect_to` set to the page's own origin,
    the code exchanged on return, and the session kept in `localStorage`.
  - The Ask transport has to use `fetch`.
  - The Supabase `ask` function must answer CORS (below).

  About a day of work once the function allows it.
- **Saving a map file (`.lmap`), map packages, Open Recent, Locate…** all write or read paths. Save
  As says "Saving a map file comes to the web version soon" and Share hides Map package. A future
  version can download the `.lmap` and open one that is dropped.
- **Update Now / Check for Updates, voice input (dictation), Send Feedback, the data library,
  diagnostics:** not offered. They were only in the native menu or are native only. A web page
  updates when it is reloaded.
- **Japanese** draws kanji and kana with Noto Sans SC, so kanji take Chinese shapes. Japanese
  fonts would need the same download as Korean.
- **Performance:** the map is drawn on the CPU on the page's thread. Panning a world map with
  5,000 storm tracks stayed smooth on an M-series Mac. A large import blocks the page while it
  reads, with no progress shown. Web Workers would fix both (see Architecture).
- **Memory:** the browser caps the WebAssembly memory at 4 GB, and files are held whole in
  memory. A 1 GB GeoTIFF won't open.

## Sizes and load time

Measured 2026-10-22 on the build in `/private/tmp/claude-501/nimblemap-web/dist` (Chrome on the reference
M-series Mac; the local server sent brotli like a CDN).

| File | Raw | gzip -9 | brotli |
|---|---|---|---|
| `nimblemap-<hash>.wasm` | 22.9 MB (22,936,063 B) | 8.1 MB | 5.7 MB (q11), 6.5 MB (q9) |
| `nimblemap-<hash>.js` (wasm-bindgen glue) | 166 KB | | 20 KB |
| `index.html` | 5 KB | | 2 KB |
| `fonts/NotoSansSC-Regular.otf` (after start) | 8.3 MB | | 6.6 MB |
| `geonames/cities.tsv` (after start) | 2.9 MB | | 0.9 MB |
| Examples (on demand) | 26 MB in all | | 16 MB in all. The heatwaves `.nc` (12.6 MB) doesn't compress; storm tracks 7.5 → 1.6 MB with brotli (see Hosting) |

- **First load before the map:** 6.6 MB with brotli (wasm, glue, page, icons). The
  first basemap tiles add about 0.4 MB. The font and the place index follow in the background.
- **Time to first map:** 0.4 s on localhost with an empty cache (WebAssembly compile
  included). 7.0 s with Chrome's "Fast 4G" throttling: the WebAssembly download is most of it
  (14.7 s before the font and the index moved out). On a 100 Mbit/s line expect 2–3 s. A return visit, with `Cache-Control: immutable`
  on the hashed files and the browser's compiled-code cache, starts in well under a second.
- **Memory:** about 88 MB of WebAssembly memory after start. 130 MB with the heatwaves NetCDF
  open. The JS heap stays under 5 MB.
- **What makes the WebAssembly large:** about 16.5 MB of code (wgpu+naga for WebGPU/WebGL, the
  readers: Parquet, Excel, NetCDF/HDF5, GeoTIFF/zstd, proj, SQLite-free GeoPackage) and about
  5.5 MB of data (Inter, the i18n catalogs, Natural Earth, the Phosphor icons, thumbnails). Next steps if it
  matters: fetch the less common readers' code on demand (needs wasm splitting, not yet practical
  with wasm-bindgen), and drop wgpu for eframe's glow backend (WebGL2 only; about −4 MB).

**Cloudflare Pages limits each file to 25 MiB (26,214,400 bytes).** The WebAssembly is 22,936,063
bytes, under the limit only because the Chinese font (8.3 MB) and the place index (2.9 MB) moved
out of it: before that it was 34 MB. Keep an eye on
it: `build.sh` prints the size.

## Browser support

The app needs WebAssembly with bulk memory, the Cache API (optional), `fetch` with streams, and
WebGPU or WebGL2:

| Browser | |
|---|---|
| Chrome / Edge 113+ (desktop) | WebGPU. Tested |
| Firefox 141+ (Windows), other Firefox | WebGPU, or WebGL2 |
| Safari 17+ (macOS, iPadOS) | WebGL2; WebGPU in Safari 26 |
| Phones | runs, but the interface is laid out for a window of 1024×640 points or larger. Not a target yet |

## Hosting at nimblemap.app

Recommended: **Cloudflare Pages**. The domain is already on Cloudflare, Pages serves brotli, reads
`_headers`, and runs the `/basemap/*` proxy as a Pages Function on the same origin.

1. Build: `apps/nimblemap-web/build.sh dist/nimblemap-web`.
2. Create the project once (Cloudflare dashboard › Workers & Pages › Create › Pages › Direct upload,
   name `nimblemap-web`), or run `npx wrangler pages project create nimblemap-web`.
3. Deploy from the folder that holds `functions/`, so the proxy goes with it:
   ```bash
   cd apps/nimblemap-web/cloudflare
   npx wrangler pages deploy ../../../dist/nimblemap-web --project-name nimblemap-web --branch main
   ```
   The preview URL (`https://<hash>.nimblemap-web.pages.dev`) works at once.
4. **DNS (owner):** Pages project › Custom domains › Set up a domain › `nimblemap.app` (and
   `www.nimblemap.app` with a redirect rule to the apex). Cloudflare adds the CNAME record itself,
   because the zone is in the same account.
5. Headers come from `dist/_headers`:
   - `application/wasm` for `.wasm`
   - `immutable` caching for the hashed `.wasm`/`.js`
   - `no-cache` for `index.html`
   - a week for `examples/*`, 30 days for `fonts/*` and `geonames/*`
   - `nosniff`, `Referrer-Policy` and `Permissions-Policy`

   No COOP/COEP: the app is single-threaded.
6. Optional: Rules › Compression Rules › compress `application/geopackage+sqlite3` too. Storm
   tracks drop from 7.5 MB to about 1.6 MB with brotli. Cloudflare compresses only text types by
   default.
7. The proxy (`cloudflare/functions/basemap/[[path]].js`):
   - It passes only `builds.json` and `YYYYMMDD.pmtiles` byte ranges, and caches each range at the
     edge for a day.
   - Protomaps' daily builds are a download host, not a tile service (docs/lm/basemap-hosting.md).
     For real traffic, extract a planet or regional `.pmtiles` once a month to an **R2 bucket**
     (`pmtiles extract`, `pmtiles upload`) and point the function at the bucket binding instead.
     R2 has no egress fees and has no 25 MiB limit.

Alternative: **GitHub Pages + Cloudflare**. Put the dist in a `gh-pages` branch of a public repo,
proxy it through Cloudflare (orange cloud), add the headers with a Transform Rule, and add a Worker
route `nimblemap.app/basemap/*` running the same proxy code. GitHub Pages allows files up to
100 MB, but it doesn't read `_headers` and needs the Worker for the basemap. It has more moving
parts than Pages.

## Supabase settings for sign-in and Ask from the web (not changed)

Checked 2026-10-22 against the International project (`mmjurjuuhqmdjpfcghhh`):

- **Auth API CORS:** `/auth/v1/*` already answers preflights with `Access-Control-Allow-Origin: *`.
  No change is needed.
- **Redirect URLs** (Authentication › URL Configuration › Redirect URLs): add
  - `https://nimblemap.app/**`
  - `https://*.nimblemap-web.pages.dev/**` (previews)
  - `http://localhost:*/**` and `http://127.0.0.1:*/**` (local testing)

  Leave the Site URL as it is. The OAuth providers (Apple, Google, GitHub, LinkedIn, X) need no
  change: they return to Supabase's own callback, which then redirects to `redirect_to`.
- **The `ask` Edge Function** answers the preflight (`OPTIONS`) with **405**, so a browser can't
  call it. It needs to:
  1. answer `OPTIONS` with `204` and
     - `Access-Control-Allow-Origin: <the request's Origin when it is https://nimblemap.app, a pages.dev preview or localhost>`
     - `Access-Control-Allow-Headers: authorization, apikey, content-type, x-client-info`
     - `Access-Control-Allow-Methods: POST, OPTIONS`
     - `Access-Control-Max-Age: 86400`
     - `Vary: Origin`
  2. add the same `Access-Control-Allow-Origin` and `Vary` to every `POST` response, errors
     included.

  The `feedback` function needs the same if Send Feedback comes to the web.
- The publishable (anon) key is public by design and would be compiled in, as on the desktop. The
  web build doesn't embed it yet, because sign-in isn't wired.

## Changes to shared crates (desktop unaffected)

- `lm-core`: `task` (spawn/pump).
- `lm-jobs`: `vfs`; the importers read through it; `Pool` queues jobs in the browser; `web-time`.
- `lm-mapview`:
  - the worker draws in place in the browser
  - `archive::set_found` (browser)
- `lm-ui`:
  - thread spawns go through `lm_core::task`; `web-time`
  - examples load through `FileRequest::Fetch` (browser)
  - `Shell::notify`
  - Map package is hidden and the Ask box shows its placeholder (browser)
  - `theme::noto_sc()`, `register_font_file`, `search::provide_cities` (the browser fetches these)
  - `getrandom`'s `js` feature, because ring builds for wasm32
- `xtask`: `nimblemap-web` has a row in the module table (`apps/` crates may use eframe and rfd).
- `cargo xtask ci` passes. The web app isn't in CI's wasm job yet: it needs wasm-bindgen-cli and
  an LLVM clang on the runner (Ubuntu's clang targets wasm32, so adding
  `cargo build --locked --target wasm32-unknown-unknown -p nimblemap-web` to the job should work).
