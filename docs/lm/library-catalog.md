# Data library catalog (WP5.4)

The data library reads one JSON file, the **catalog**, hosted next to the basemap. Each entry
describes a course dataset; the app shows it as a card (thumbnail, title, size, licence) and
"Add to map" downloads its files into the cache, checks their SHA-256 and opens the first file
through the normal import pipeline, assumptions included.

Code: `crates/lm-jobs/src/library.rs` (format, cache, downloads), `crates/lm-ui/src/library.rs`
(the panel). Candidate datasets for the production catalog: `library-candidates.md`.

## Where the catalog comes from

1. `LM_LIBRARY_URL` (any `https://`, `http://` or `file://` URL, or a plain path).
2. Otherwise the production catalog, `lm_jobs::library::DEFAULT_CATALOG_URL`. It is unset until
   the owner chooses the host (Basemap host decision, Gate 0 / WP5.7), so release builds show no
   library yet.
3. Otherwise, in developer builds only, the local test catalog `fixtures/library/catalog.json`.

The entry points (welcome card, File › Browse data library…) are shown only when a catalog is
set, and only from Phase 5 on (or on the developer surface: `LM_DEV=1` or a debug build).

## Format

```json
{
  "format": "layeredmap-library",
  "version": 1,
  "updated": "2026-10-18",
  "datasets": [
    {
      "id": "world-cities",
      "title": { "en": "World cities", "zh": "世界城市" },
      "description": { "en": "1,251 cities and towns …", "zh": "1,251 个城市和城镇 …" },
      "kind": "points",
      "extent": [-175.23, -90, 179.22, 78.23],
      "license": { "id": "public-domain", "name": "Public domain", "url": "https://www.naturalearthdata.com/about/terms-of-use/" },
      "source": "https://www.naturalearthdata.com/downloads/50m-cultural-vectors/50m-populated-places/",
      "citation": "Made with Natural Earth. Free vector and raster map data @ naturalearthdata.com.",
      "date": "v5.1.2",
      "attribution": "Natural Earth",
      "thumbnail": "thumbs/world-cities.png",
      "files": [
        {
          "name": "cities.geojson",
          "urls": ["https://cn-mirror.example/library/cities.geojson", "https://cdn.example/library/cities.geojson"],
          "size": 244807,
          "sha256": "35bfa5d15bc5e6a898aeb67cc74ac13b8cc9493c1a71ed370fc5246fbe7eb07d"
        }
      ]
    }
  ]
}
```

### Top level

| Field | Required | Meaning |
|---|---|---|
| `format` | yes | Always `"layeredmap-library"`. Anything else is refused. |
| `version` | yes | `1`. A higher version is refused ("needs a newer NimbleMap"); add fields within version 1 only if older apps can ignore them. |
| `updated` | no | When the catalog was last edited (ISO date). |
| `datasets` | yes | The entries, in the order the panel shows them. |

### Dataset

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | 1–64 lowercase letters, digits or hyphens; unique in the catalog. Also the cache folder's name. |
| `title` | yes | `{ "en": …, "zh": … }`, both non-empty. Short: it is the card's title. |
| `description` | yes | `{ "en": …, "zh": … }`, both non-empty. One or two sentences; shown on hover. |
| `kind` | yes | `points`, `lines`, `areas`, `raster` (shown as "Grid") or `table`. |
| `extent` | yes | `[west, south, east, north]` in degrees (WGS 84). |
| `license` | yes | `{ "id": SPDX id or a short id such as "public-domain", "name": what the card shows, "url": the licence page }`. `licence` is accepted as the key too. |
| `source` | yes | The provider's page for the data. |
| `citation` | yes | How to cite the data, as the provider asks (verbatim where they give one). |
| `date` | yes | The data's date or version ("2022", "v5.1.2"). |
| `attribution` | no | The credit the licence asks to show on maps and exports ("© OpenStreetMap contributors"). |
| `thumbnail` | no | A PNG, 3:2, about 400 × 266 px and under 2 MB. Without one the card shows the kind's symbol. |
| `files` | yes | One or more files (below). The **first** is the one opened; the others are kept beside it (a Shapefile's `.dbf`, `.shx`, `.prj`, `.cpg`). |

### File

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | The file's name in the cache: no folders, not starting with a dot, unique in the dataset. Its extension decides how it is read, as for Open. |
| `urls` | yes | Where to download it, **tried in order**: put a China-reachable mirror first, a global CDN after. A 404, a connection failure or a checksum mismatch moves on to the next URL. |
| `size` | yes | Bytes. Progress is measured against it, and a download larger than this is stopped. The card shows the sum over the files. |
| `sha256` | yes | The file's SHA-256, 64 hexadecimal digits. A file that doesn't match is deleted and never opened. |

URLs (file and thumbnail) may be relative: they resolve against the catalog's own URL, so a
catalog and its files can be moved together (`../examples/cities.geojson` steps up a folder).

A single bad entry (a missing Chinese title, an unsafe file name, a malformed checksum, a repeated
id…) is skipped and logged; the rest of the catalog still shows.

## Cache

`~/Library/Caches/LayeredMap/library/` on macOS (`$XDG_CACHE_HOME/layeredmap/library` elsewhere):

| Path | What |
|---|---|
| `catalogs/<hash of the catalog URL>.json` | The last catalog read, shown when the host can't be reached ("Offline: showing the list from your last visit"). |
| `thumbnails/<hash of the thumbnail URL>.png` | Thumbnails, fetched once. |
| `datasets/<id>/<name>` | The downloaded files. |
| `datasets/<id>/<name>.sha256` | Written after the file's checksum was verified. A file counts as cached only when its size and this record match the catalog, so a dataset whose checksum changes in the catalog is downloaded again. |

Downloads go to `<name>.part` and are renamed only after the checksum matches; Cancel and every
failure remove the `.part` file. A cached dataset opens without touching the network.

## Network

All of the library's network requests (catalog, thumbnails, files) go through one function,
`lm_jobs::library::open_url`, so the strict offline switch (Phase 6) can gate the whole library in
one place. `file://` URLs and plain paths are read from disk and are never gated. HTTP requests
use ureq with a 10 s connect and 20 s response timeout; there is no overall timeout, because a
large dataset on a slow link can take minutes, and Cancel is always available.

## Hosting checklist (for the production catalog)

- Host the catalog, thumbnails and files on the China-reachable host chosen for the basemap
  (`basemap-hosting.md`); list a global mirror second in `urls`.
- Compute `size` and `sha256` from the exact hosted bytes (`shasum -a 256 file`).
- Prefer formats that open without conversion and stay small: GeoJSON or GeoPackage for vectors,
  Cloud-Optimized GeoTIFF for rasters; clip global rasters to the course area where that is what
  the course needs.
- Record the licence page, citation and attribution exactly as the provider gives them
  (`library-candidates.md`).
