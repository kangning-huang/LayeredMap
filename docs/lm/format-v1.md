# LayeredMap map file, format v1 (`.lmap`)

A `.lmap` file is one LayeredMap map: its layers, how each is drawn and filtered, the answers the
user gave, the results it kept with their recipes, and the view. It is a single ZIP file, so it can
be emailed, put on a USB drive or handed in as coursework. Code: `crates/lm-doc/src/container.rs`
(save and open), `crates/lm-doc/src/autosave.rs` (autosave and recovery).

Source data files are **referenced, never copied**. The data the map made itself (results) is
stored inside. A **map package** (container 2, below) is the exception: the same file with its
source files inside, so the map opens on another computer.

## Container

| Entry | Compression | Contents |
|---|---|---|
| `mimetype` | stored, first entry | `application/vnd.layeredmap.map+zip` (no newline). A file is recognised by its first bytes: `PK\x03\x04`, the name `mimetype` at byte 30 and the type at byte 38. |
| `manifest.json` | Deflate | The document and its tables (below), UTF-8 JSON. At most 256 MiB. |
| `results/layer-<id>.fgb` | Deflate | An owned vector result, FlatGeobuf without a spatial index. |
| `results/layer-<id>.geojson` | Deflate | An owned table, or vector result with features that have no geometry. |
| `results/layer-<id>.tif` | Deflate | An owned raster result, GeoTIFF. |

Entries larger than 4 GiB use Zip64. Readers ignore entries they don't know.

On macOS the type is `app.layeredmap.map` (exported by the app bundle; conforms to `public.data`
and `public.content`; extension `lmap`; MIME type as above). LayeredMap is its owner, in the Editor
role.

## `manifest.json`

```json
{
  "format": "layeredmap",
  "container": 1,
  "version": 0,
  "saved_at": 1797206400,
  "software": "LayeredMap 0.5.0",
  "sources": [
    { "layers": [1, 2], "path": "/Users/ana/Downloads/schools.csv",
      "relative": "../Downloads/schools.csv",
      "size": 18204, "mtime": 1791000000, "sha256": null }
  ],
  "owned": [
    { "layer": 3, "entry": "results/layer-3.fgb", "format": "FlatGeobuf", "name": "Schools near parks" }
  ],
  "layers": [ … ],
  "view": [121.47, 31.23, 10.5],
  "next_layer_id": 4,
  "user_ordered": false,
  "results": [ … ],
  "next_result_id": 2
}
```

| Field | Meaning |
|---|---|
| `format` | Always `"layeredmap"`. Anything else is not a LayeredMap map. |
| `container` | The container version, **1** for this format. A reader refuses a larger number ("saved by a newer version of LayeredMap"). |
| `version` | The document schema version (`lm_core::document::DOCUMENT_VERSION`), still **0**. Until Gate 5 a phase only adds fields, so it stays 0 and older builds ignore what they don't know. |
| `saved_at`, `software` | When and by which build the file was written. Informational. |
| `sources` | One row per source file: the layers read from it, its absolute path when saved, its path relative to the folder holding the `.lmap` (`/`-separated; absent when the two share no root), and its fingerprint. |
| `owned` | One row per owned result: the layer, the ZIP entry, its format and the dataset's own name. |
| everything else | The document itself, field for field as `lm_core::document::Document` serialises: layers (bottom to top) with name, visibility, profile (source reference, fields, assumptions and answers, exclusions, joins), **style** (`style`), filter, field roles and starred fields; kept results with their **recipes** (`results[].recipe`, with earlier versions in `history`); the saved view. |

Styles and recipes live in the document fields, not in separate entries: they are small, and
keeping them in one JSON keeps the manifest a complete description of the map.

### Source references

A source is identified three ways:

1. **Absolute path** (`sources[].path`, and each layer's `profile.source.path`).
2. **Relative path** from the `.lmap`'s folder (`sources[].relative`).
3. **Fingerprint**: size in bytes, modification time (seconds since the Unix epoch), and the
   SHA-256 of the bytes when it was computed (`lm_doc::fingerprint`). Kept results also record
   their own input fingerprints in their recipes (`lm_jobs::fingerprint`: length and a hash of the
   first and last MiB); those decide "Source changed".

On open, for each source:

- Found at its absolute path: read from there (a changed file is read as it is now, with a
  notice).
- Missing there, but present at its relative path with the same fingerprint (size, then hash if
  recorded, else time): the layers are pointed there silently. This is the folder that was moved
  or copied whole, with the map inside it.
- Missing: the layer shows the missing-source card, "Can't find heat_july.tif. It was last seen in
  Downloads.", with **Locate…** and **Remove layer**. Locate… checks the chosen file's
  fingerprint. If it differs, the card asks "This file looks different (its size changed). Use it
  anyway?" (or "its contents changed", or "its modification date changed" when no hash was
  recorded). Yes points the layers at the new file and reads it; kept results made from the old
  file then show "Source changed", because the new file no longer matches their recipes.

### Owned results

A layer's data is stored in the file when no source file can give it back:

- layers LayeredMap made itself (results of actions, drawn areas): their `profile.source` has no
  size and no time;
- layers with a joined table (`profile.joins` is not empty);
- layers a kept result points to (`results[].layer`), such as a layer an action added fields to.

Other layers are read again from their sources and their recorded choices are re-applied.

Formats:

- **Vectors: FlatGeobuf**, WGS 84, written **without a spatial index** so features keep their
  order: results, selections and filters refer to rows by position. Fields are Integer (Long),
  Double, Boolean or Text; null values are left out of a feature.
- **Tables and features without geometry: GeoJSON** (RFC 7946, `"geometry": null`), since
  FlatGeobuf holds only features with geometry.
- **Rasters: GeoTIFF**, Deflate-compressed, north-up only. 8-bit colour rasters are written as
  RGB(A); any other raster as its **first band**, 32-bit float, with its NoData value.

A result that can't be written (a rotated raster, for example) is left out with a notice on save;
its layer is then read from its source on open, if it has one.

## Saving

Saving is atomic. The whole file is written to `.<name>.lmap.saving-<pid>-<n>` in the same
folder, synced to disk (`F_FULLFSYNC` on macOS), and renamed over the old file; the folder is
then synced. Until the rename succeeds the old file is untouched, so a crash or `kill -9` at any
moment leaves either the old map or the new one. Temporary files that a killed save left behind
are removed by a later save once they are 10 minutes old.

⌘S writes the user's file; the first ⌘S, and ⇧⌘S, ask where. Nothing else ever writes it.

## Autosave and recovery

Every 30 seconds while there are unsaved changes, and whenever the app loses focus, the map is
saved in this same format to
`~/Library/Application Support/LayeredMap/Autosave/autosave-<start>-<pid>.lmap`. A journal beside
it, `session.json`, records the running session (`pid`, `started_at`, `clean`), its autosave
(`name`, `file`, `saved_at`, and the user's `document` path if any), and an offer not yet answered.

- A clean quit marks the journal clean and removes the autosave.
- Saving (or undoing back to the saved state) removes the autosave.
- A launch that finds the journal unclean with an autosave asks: "LayeredMap quit unexpectedly.
  Restore 'Heat study' from 2 minutes ago?" **Restore** opens the autosave (marked Edited, tied to
  the user's file if it had one); **Discard** deletes it. Until one is chosen, the offer survives
  further crashes and quits.

## Migration from v0

Gates 2–4 saved documents as a folder holding `manifest.json`, with `"format": "layeredmap"`,
`"version": 0` and the document fields, and no container fields
(`crates/lm-doc/src/manifest.rs`). A v1 reader opens such a folder (or its `manifest.json`) as
is: every v0 field means the same in v1, and a v0 document has no owned results, so its derived
layers read nothing until it is saved again as `.lmap`. It opens as Edited and saves through Save
As. The fixture documents `fixtures/D/D1`–`D3` (one per gate) are reopened and saved as `.lmap`
in CI (`container::tests::fixture_documents_migrate_and_round_trip`).

Going back: a v1 `manifest.json` taken out of the ZIP still opens in a Phase 4 build, which
ignores `container`, `sources` and `owned` (its layers made by LayeredMap then have no data).

## Map packages (container 2, WP6.5)

Share › Map package… (File › Export Map Package…) writes a `.lmap` that carries its data. Code:
`crates/lm-doc/src/package.rs`; the dialog is `crates/lm-ui/src/package.rs`.

**Layout.** The container above, plus the source files:

| Entry | Compression | Contents |
|---|---|---|
| `data/<n>/<file>` | Deflate | The `n`-th source file (1-based, in `sources` order), byte for byte, with its companions beside it: files of the same name in its folder with a known sidecar extension (a Shapefile's `.shx` `.dbf` `.prj` `.cpg` and indexes, world files `.tfw` `.wld` `.jgw` `.pgw`, `.aux.xml`, `.ovr`, `.hdr`, …). |
| `data/<n>/<folder>/…` | Deflate | A source that is a folder (a folder-based format): its files, macOS `._*` and `.DS_Store` left out. |

**Manifest.**

- `container` is **2**. A build that reads only container 1 refuses the file ("saved by a newer
  version of LayeredMap") instead of opening layers with no data. Plain maps are still written as
  container 1, so older builds keep opening them.
- Each packaged `sources` row has `embedded`, the ZIP entries holding the file and its
  companions (the file first), and `license` when LayeredMap knows it (the examples'; the data
  library's from its catalog). Its `path` is the file's entry (`data/1/schools.csv`) and
  `relative` is absent.
- Layers read from a packaged file have `profile.source.path` = that entry.
- `package`: `{ "dropped": [ { "layer": 4, "fields": ["phone"] } ], "paths_removed": 3 }`.

**What packaging removes.**

- **Absolute paths.** Every string in the manifest that is an absolute path (`/…/…`, `C:\…`,
  `\\server\…`) — a joined table's source, a layer kept as a result, anything else — is replaced by
  its file name; packaged sources by their entry. The dialog lists each path before saving.
- **Personal fields the user leaves out.** Fields that look like people's names, emails, phone
  or ID numbers (`lm_jobs::export::sensitive`) are listed, checked to be left out. A field the
  style (colour, size, labels) or the filter uses can't be left out, so the map still looks the
  same. A layer that loses fields is stored as an owned result (`results/`) without them, and its
  profile, field roles and starred fields lose them too; its source file is then not carried
  (unless another layer reads it), and its `profile.source` keeps only the file name, with no
  size or time.
- Layers already stored as owned results never carry their source file.

**Opening.** A package's files are unpacked into a folder of the app's own,
`~/Library/Application Support/LayeredMap/Packages/<hash>/<n>/…` (beside the autosaves; `<hash>`
is the first 16 hex digits of the manifest's SHA-256, so opening the same package again reuses
it), through a temporary file and a rename. Entry names are checked (`data/<digits>/` then plain
names only: no `..`, no absolute parts, no `\` or `:`). Each file's modification time is set
to the recorded `mtime`, so the source reads as unchanged and kept results still match their
recipes. The layers are pointed at the unpacked files, and the document is then like any other:
undo starts fresh, autosaves are plain container-1 maps referencing the unpacked files (which
stay where they are), and ⌘S writes the user's file as a package again (the app remembers that
the file is one; after a crash it asks the file).

Not relinked: a package never looks for its sources beside itself.

## Compatibility rules

- Readers ignore unknown fields and entries.
- Additions that older readers can safely ignore keep `container` at 1 and `version` at 0.
- A change older readers would misread raises `container` (layout) or `version` (document
  schema); older builds then refuse the file with a message saying a newer LayeredMap saved it.
  Map packages are the first such change: container 2 (above).
