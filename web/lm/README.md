# NimbleMap website and help (WP6.10)

A landing page, a searchable glossary and three step-by-step workflows, in English, Simplified
Chinese, German, Spanish, French, Italian, Dutch, Brazilian Portuguese, Russian, Japanese and
Korean (`<folder>/` per language; the nine newest are machine translations awaiting native
review, see `docs/lm/localization.md`). Plain HTML, CSS and JavaScript: no build step, no libraries, no web fonts, no
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
| `index.html` | `zh/index.html` | Landing: “Found map data online? Drag it into NimbleMap.” A folder of real downloaded files beside the map each opens (hero; a hook for the launch-video loop), what it opens, a Data page teaser, four “once it’s open” screenshots, a slim Pro band, the built-in examples as practice, sharing, a final download button |
| `download.html` | `zh/download.html` | For people who already have a GIS file: what it opens, why layers line up, the download buttons, why NimbleMap differs from a typical GIS (for people who know GIS), NimbleMap or QGIS, and a FAQ in the words people search with |
| `pro.html` | `zh/pro.html` | Free vs Pro: what Pro adds (free with an account, unlocked by signing in inside the app, coming in the next version); the other languages' footers link the English page |
| `help.html` | `zh/help.html` | Help start page (what Help › NimbleMap Help opens): the workflows and the window in one minute |
| `glossary.html` | `zh/glossary.html` | 34 terms, searched in the browser in both languages (`?q=buffer`, `#gcj02` deep links, `/` focuses the box) |
| `workflow-typhoons.html` | `zh/…` | Typhoon tracks: click, rank, colour, filter to Category 5; Pro for counting near cities |
| `workflow-heatwave.html` | `zh/…` | Heatwave NetCDF: time slider, reading cells, no data, export a map image |
| `workflow-walled-cities.html` | `zh/…` | Walled cities 1866: satellite basemap, compare, filter, style, share |

`assets/site.css` (light and dark, follows the system, with a switch remembered in
`localStorage`), `assets/site.js` (the switch), `assets/glossary-data.js` (every term, both
languages, with where it appears in the app) and `assets/glossary.js` (drawing and search).
Header and footer are repeated in each page. The shared parts (the `hreflang` alternates,
`og:locale`, the header's page links and language menu, and the footer's links, non-affiliation
line and language list) are written by `tools/lm/site/langs.py` from its `LANGS` and `WORDS`
tables: add a language's folder, add its rows, run the script. Everything else in the header and
footer is changed by hand in every page. Each glossary page also loads `assets/glossary-<folder>.js`.

## Preview

Open `web/lm/index.html` in a browser, or serve the folder:

```sh
python3 -m http.server --directory web/lm 8000   # then http://localhost:8000/
```

## Search and link previews

Every page carries a canonical link, `hreflang` links (every language that has the page, and
English as the default), Open Graph tags for link previews, and a title that says what the page shows in the words
people search with. The landing and download pages also carry `SoftwareApplication` structured
data. In this folder those URLs are relative, so the pages still work offline and inside the app.
Publishing goes through the stamp script, which makes them absolute and writes `sitemap.xml` and
`robots.txt`:

```sh
python3 -I tools/lm/site/stamp.py https://<the site's domain> /path/to/new/folder
python3 -I tools/lm/site/langs.py --check  # every page lists every language
python3 -I -m pytest -q tools/lm/site    # checks every page's head tags and links
```

Placeholders such as the download links stay in HTML comments (`TODO(owner)`), never as visible
text; the tests check this, and the stamp script drops them from the published copy.

### Where it is published

The stamped site is served by GitHub Pages from the public
[NimbleMap](https://github.com/kangning-huang/NimbleMap) repository (`main` › `/docs`) at its
custom domain, https://nimblemap.com/ (nimblemap.app redirects there at Cloudflare; don't point the
Pages CNAME at it). That repository holds only the website, the changelog and user feedback: never
source code, packaging scripts or keys. To update it, stamp into a new folder and copy it over that
repository's `docs/`, keeping `docs/.nojekyll` (and the `backups/` folder at the repository root):

```sh
python3 -I tools/lm/site/stamp.py /path/to/new/folder        # ORIGIN defaults to https://nimblemap.com/
rsync -a --delete --exclude .nojekyll /path/to/new/folder/ /path/to/NimbleMap/docs/
```

Stamping for a host root also writes `robots.txt` and `CNAME` (holding `nimblemap.com`), so the
copy keeps the custom domain. Always stamp with the live origin: stamping with another one rewrites
every canonical and hreflang URL to it.

## In the app

Help › **NimbleMap Help** (`apps/layeredmap/src/help.rs`) opens `help.html`, or
the help page in the interface's language (`zh/help.html`, `ja/help.html`…), in the default
browser. The pages ship
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
- **Download links.** The Mac button links the latest GitHub release's .dmg (the landing pages'
  buttons too). The mainland China mirror and Pro buttons in `download.html` are disabled
  placeholders marked `TODO(owner)`.
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

**Retake all screenshots after any significant UI change: `tools/lm/site/retake_screenshots.sh`.**
It builds the release app, runs each screenshot's tour script, captures only the window that run
opens (found by its process id, so other NimbleMap windows are ignored) at a set delay, scales it to
1600 px (Pillow, JPEG quality 90, 4:4:4) and cuts the landing-page crops. The table at the top of the
script lists every screenshot's tour script, capture delay and crop, so adding one is one line. Pass
names to retake a subset (`retake_screenshots.sh typhoons-card walls-satellite`; a crop is re-cut
when its source is retaken). It never sends keys or clicks. Window chrome is forced light with the
per-process Cocoa default `-NSRequiresAquaSystemAppearance YES`, so captures match in Dark mode.
`pro-ask` spends one Ask request and runs only with `PAID=1`; `typhoons-pro` runs as Pro
(`LM_TIER=pro`, unbundled, so the key comes from the keychain and no cost line shows) without an
Ask call.

- **Home page, 2026-10-20 (`open-*.jpg`):** captured the same way (release build, `--tour`, light
  theme, window only, `retake_screenshots.py shot`/`cut`) but from the real downloaded-style files
  that `design/launch-video-v3/data/make_files.sh "<folder>"` writes (IBTrACS storms `.shp`, USGS
  earthquakes `.gpkg`, GDO heatwave `.nc`, ETOPO `.tif`, NCEI volcano `.csv`), passed as file
  arguments. They are not in `retake_screenshots.sh` because those files live outside the repo.
  Tours (steps are 2.5 s apart, 6 s after a fly): storms
  `theme light;basemap light;tab 0;tab 0;card style_by_value max_wind_k;tab 0;150,22,3.2;tab 0;150,22,3.2…`
  (capture at 30 s); search: the same with `find Haiyan` after the style step (29 s); zoom:
  `…card style_by_value max_wind_k;tab 0;124,16,4.6;esc;esc;124,16,4.6…` (36 s); earthquakes
  `…175,12,2.4 ×3;card style_by_value magnitude;tab 0…` (34 s); click: `142.373,38.297,10 ×2;card
  style_by_value magnitude;142.373,38.297,10;click 142.373,38.297;160,20,2.6…` (36 s); heatwave
  `-5,25,2.6 ×3;time 45;tab 0…` (34 s); relief `60,10,1.9 ×3;tab 0…` (30 s); volcanoes
  `175,10,2.3 ×3;card style_by_value elevation_m;tab 0…` (32 s). Cuts (from the 1600-px shots, to
  800×500): zoom 345,100,1545,850; search 700,100,1600,662; click 600,110,1600,735; time (from the
  heatwave shot) 210,420,1130,995.
- **Retaken 2026-10-19 after the line-transparency fix** (55db6c3: tracks coloured by basin or
  season keep the layer's faint line transparency, so overlaps show density), all with the script
  on the phase-4 build, Light appearance (`theme light`; the Light basemap except the
  satellite shots `walls-satellite.jpg` and `walls-region.jpg`). The exact tour scripts are the
  script's table. `points-earthquakes.jpg` is the earthquakes example coloured by magnitude
  (`card style_by_value magnitude`) with the 2011 Tohoku earthquake selected; `areas-countries.jpg`
  is `countries.geojson` coloured by `pop_est` with India selected. `typhoons-hero.jpg` is a crop of
  its capture (raw pixels x 254–2564 of 2818, the framing of the earlier hero). The hero's Lines and
  Grids slides reuse `typhoons-card.jpg` and `heatwave-slider.jpg`.
  The landing page's principle crops are 800×500 cuts of these captures: `step-open.jpg` from
  `welcome.jpg` (x 380–1240, y 458–996, scaled), `step-click.jpg` from `points-earthquakes.jpg`
  (x 1000–1600, y 98–473, scaled), `step-share.jpg` from `heatwave-export.jpg` (x 60–1150,
  y 170–851, scaled), `step-honest.jpg` from `walls-compare.jpg` (x 640–1600, y 110–710, scaled)
  and `step-try.jpg` from `typhoons-filter.jpg` (x 340–1140, y 496–996, not scaled).
  `img/icon-256.png` is a copy of `assets/icon/icon-256.png` (the app icon) for the final call to
  action.
  `heatwave-export.jpg` runs with `LM_DEV=1` (a developer build), since Share › Map image… only
  appears in a Free release build from Phase 5; the sheet is the same in every tier.
- **Pro shots, retaken 2026-10-19 in light** as NimbleMap Pro (`LM_TIER=pro`, the unbundled release
  binary, so the Ask key comes from the keychain and no cost line shows): `typhoons-pro.jpg` (Haiyan
  selected with the Ask box showing; no Ask call) and `pro-ask.jpg` (“Which country has the most
  cities?” over `cities.geojson` with the built-in world layers; one Ask request, `PAID=1`).

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
