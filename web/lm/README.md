# NimbleMap website and help (WP6.10)

A landing page, a searchable glossary and three step-by-step workflows, in English and
Simplified Chinese. Plain HTML, CSS and JavaScript: no build step, no libraries, no web fonts, no
analytics and no requests to any other host, so the pages work offline, from `file://`, and from
any host reachable in mainland China.

**Nothing here is deployed.** No workflow publishes this folder.

## Why `web/lm/`

- `/site` is git-ignored: it is the output folder of upstream's docs build (Zensical,
  `.github/workflows/docs.yml`).
- Everything under `docs/` is part of upstream's RustGIS docs site, so pages there would be
  published to rustgis.app.
- `apps/rustgis-web` is upstream's browser app (`web-deploy.yml`).

`web/lm/` follows the fork's `<area>/lm` convention (`docs/lm`, `tools/lm`, `packaging/lm`) and
touches no upstream file or workflow.

## Pages

| English | 中文 | What |
|---|---|---|
| `index.html` | `zh/index.html` | Landing for the map-curious: what NimbleMap is, examples, Free vs Pro, a link to the download page |
| `download.html` | `zh/download.html` | For people who already have a GIS file: what it opens, why layers line up, the download buttons (placeholders), NimbleMap or QGIS, and a FAQ in the words people search with |
| `help.html` | `zh/help.html` | Help start page (what Help › NimbleMap Help opens): the workflows and the window in one minute |
| `glossary.html` | `zh/glossary.html` | 34 terms, searched in the browser in both languages (`?q=buffer`, `#gcj02` deep links, `/` focuses the box) |
| `workflow-typhoons.html` | `zh/…` | Typhoon tracks: click, rank, colour, filter to Category 5; Pro for counting near cities |
| `workflow-heatwave.html` | `zh/…` | Heatwave NetCDF: time slider, reading cells, no data, export a map image |
| `workflow-walled-cities.html` | `zh/…` | Walled cities 1866: satellite basemap, compare, filter, style, share |

`assets/site.css` (light and dark, follows the system, with a switch remembered in
`localStorage`), `assets/site.js` (the switch), `assets/glossary-data.js` (every term, both
languages, with where it appears in the app) and `assets/glossary.js` (drawing and search).
Header and footer are repeated in each page; change them in all fourteen.

## Preview

Open `web/lm/index.html` in a browser, or serve the folder:

```sh
python3 -m http.server --directory web/lm 8000   # then http://localhost:8000/
```

## Search and link previews

Every page carries a canonical link, `hreflang` links (English, Chinese, and English as the
default), Open Graph tags for link previews, and a title that says what the page shows in the words
people search with. The landing and download pages also carry `SoftwareApplication` structured
data. In this folder those URLs are relative, so the pages still work offline and inside the app.
Publishing goes through the stamp script, which makes them absolute and writes `sitemap.xml` and
`robots.txt`:

```sh
python3 -I tools/lm/site/stamp.py https://<the site's domain> /path/to/new/folder
python3 -I -m pytest -q tools/lm/site    # checks every page's head tags and links
```

Placeholders such as the download links stay in HTML comments (`TODO(owner)`), never as visible
text; the tests check this.

## In the app

Help › **NimbleMap Help** (`apps/layeredmap/src/help.rs`) opens `help.html`, or
`zh/help.html` when the Mac's first language is Chinese, in the default browser. The pages ship
inside the app: `packaging/lm/bundle-macos.sh` copies this folder (minus this README) to
`NimbleMap.app/Contents/Resources/help/`; a development build opens the checkout's copy. Opening a
local file makes no network request, so the network gate is not involved.

## TODO (owner)

- **Name and domain.** Settle the name (spec risk 14) before publishing; the name appears in every title,
  the structured data and the stamp script's ORIGIN.
- **Demo video.** A short muted clip of a file being dropped in and lining up, for the download
  page's hero (an mp4 with a poster, not a GIF).

- **Hosting.** Choose a host reachable from mainland China and elsewhere (and an ICP filing if
  the site is hosted in mainland China). Then add the public URL to `help.rs` as a fallback if the
  bundled pages are missing.
- **Download links.** The three download buttons in `download.html` and `zh/download.html` are disabled
  placeholders marked `TODO(owner)`: the signed .dmg, a mainland China mirror, Pro sign-up and
  pricing.
- **Chinese interface.** The app is English-only for now, so the Chinese pages quote the English
  button labels with a Chinese gloss. When the Chinese interface (WP5.5) lands, swap in its labels
  and retake the screenshots in Chinese.
- **Share in Free builds.** Share › Map image… and Data… appear in a Free release build once the
  build reaches Phase 5 (`EXPORT_PHASE`; today they show in Pro and developer builds). The export
  screenshots were taken from a developer build; the sheet is the same in every tier.
- **Pro examples.** The Ask questions quoted on the pages are examples of the kind of request the
  assistant takes; only “Which country has the most cities?” (the screenshot) and the questions in
  `docs/lm/demo-picks.md` were run. Run the others through the evaluation set before launch.

## Screenshots: sources and licences

All screenshots are real captures of the NimbleMap window from this branch, taken with the app's
`--tour` script steps (`apps/layeredmap/src/main.rs`; `card`, `time` and `export` were added for
these pages, `theme` on 2026-10-19) and `screencapture -l` on the app's window only, then scaled to
1600 px and saved as JPEG.

- **Light appearance, retaken 2026-10-19** (`theme light`, the Light basemap except the satellite
  shots `walls-satellite.jpg` and `walls-region.jpg`): every image except the two Pro shots below.
  Two were added for the landing page's rotating hero: `points-earthquakes.jpg` (the earthquakes
  example coloured by magnitude, `card style_by_value magnitude`, with the 2011 Tohoku earthquake
  selected) and `areas-countries.jpg` (`countries.geojson` coloured by `pop_est`, India selected).
  The hero's Lines and Grids slides reuse `typhoons-card.jpg` and `heatwave-slider.jpg`.
- **Still dark, not retaken:** `pro-ask.jpg` and `typhoons-pro.jpg` need the paid Ask step. They
  date from 2026-10-18; `pro-ask.jpg` is an earlier capture of the Pro test build with the test
  build's cost line removed. Retake them in light when the owner approves an Ask run.

Map content in the screenshots, and the credit each needs:

| Source | Where | Credit / licence |
|---|---|---|
| OpenStreetMap, via Protomaps basemap tiles | Light, dark and satellite labels | © OpenStreetMap contributors, ODbL; Protomaps |
| Natural Earth | Offline outline, borders; countries (`areas-countries.jpg`) | Public domain |
| USGS earthquake catalogue | Earthquakes (`points-earthquakes.jpg`) | Public domain (U.S. Geological Survey) |
| NASA Blue Marble Next Generation via NASA GIBS | Satellite basemap (walled cities) | NASA Earth Observatory, GIBS; not copyrighted, credit NASA, no endorsement implied |
| NOAA NCEI IBTrACS v04r01 | Typhoon tracks | NOAA/WMO open access; cite Knapp et al. 2010 and Gahtan et al. 2024 (WMO Resolution 40 for commercial use is the owner's call) |
| Copernicus GDO (JRC) warm spell intensity, ERA5, 2023 | Heatwave grids | CC BY 4.0, © European Union 1995–2026 |
| Xue et al. 2021, CUED 1866 | Walled cities | CC BY 4.0 |
| Natural Earth cities (Pro built-in layers) | `pro-ask.jpg` | Public domain |

Full source details for the example data: `assets/examples/README.md`. The site's logo is the
app icon (`design/logo/strata-raster.svg`).
