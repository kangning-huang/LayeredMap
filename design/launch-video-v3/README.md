# NimbleMap launch film v3: "one take" (31 s, 16:9)

Three searches, three maps, one camera move. The search bar is typed three times, and each search opens into a short film about one of the app's built-in maps. All three films sit on one atlas table, and the camera never cuts. Every app pixel is a frame of the real release build, recorded window-only while the app drove itself through `--tour`. No keystrokes or clicks were sent to any app, and no paid (Ask/Pro) call was made.

## Beat sheet as built (30 fps; all within ±0.1 s of the owner's prompt)

| t (s) | Beat |
|---|---|
| 0.00–4.20 | Bar 1 (the window's real bar) alone on the table at 2.25×: placeholder, block cursor, types **Haiyan**. Two still holds and one glide. Key clicks only |
| 4.20 | ENTER and music downbeat; the window opens out of its own bar while the camera pulls out to 0.88× |
| 4.90–9.40 | Typhoons: 4,999 tracks draw in (east to west) over a plate of the same view; push and tilt; the app's own dive to Leyte; Haiyan card; pull with a slight dutch; the "higher than 99% of 4,547 storms" row lifts out of the card |
| 9.40–10.80 | The `max_wind_kt 170` row lifts off; **170 knots.** |
| 10.80–11.20 | Travel right to bar 2 (motion blur) |
| 11.20–12.00 | Bar 2 types **Tohoku**, held still |
| 12.05–12.70 | Bar 2 opens into the dark earthquake window; "Tohoku" flies to the corner |
| 12.70–15.30 | The Ring of Fire lights up dot by dot (a clockwise sweep from Japan over a plate); handheld creep; breakdown in the music |
| 15.30–16.00 | The Tohoku card (real, with ripple on its dot); the magnitude row lifts; push in on the card |
| 16.00–16.90 | **M 9.1.**; pull back to film 2 and bar 3; bar 3 types **heat** |
| 16.90 | DROP: bar 3 collapses to a line, the line shoots across the table and becomes the time slider's track; the camera whips along it |
| 17.25–21.10 | Heatwaves over the North Atlantic: slider plays 1 Jun → 16 Jul 2023 (one real frame per daily step); the cell card at 16 Jul (50.55 °C, higher than 96%) |
| 21.10–23.20 | Wipes back to 1 Jun and plays once more while pulling out to the window |
| 23.30–25.90 | The three windows slide into one row (same height) over the bar; the camera pulls back |
| 26.80 | Stop: everything freezes; one silent beat |
| 27.30–28.40 | Hit; the block cursor writes **NimbleMap** in one stroke; push in |
| 28.50–31.00 | *Explore real data about the planet.* · **Free for Mac** · kangning‑huang.github.io/NimbleMap |

Music: original, synthesised in `audio/synth.py` (numpy/scipy, no samples), 115 BPM, D minor. Silent until ENTER at 4.20, which is a downbeat. The two-bar breakdown runs 12.73–16.90 on the drop's grid. The drop lands at 16.90, the stop is at 26.81 (one beat before the drop grid's bar 6), and the hit is at 27.30. Mastered to −14.0 LUFS integrated, true peak −1.1 dBTP.

## What is drawn (not app UI)

- the table and its graticule
- the window frames and title bars
- the typed queries (Inter Regular 27.5 px at 2×, fitted to the placeholder's width, on its baseline, in the bar's ink) and the block cursor
- the reveal order of tracks and dots (real pixels wiped in over a real plate of the same view)
- ripples and the accent line at the drop
- the lifted card rows (real crops, enlarged)
- the captions
- the end pill and end card

Bar 3 is shown 3× as a free pill. The end pill is a re-layout of the bar.

## Data checks (`assets/examples`, release build, site screenshots)

- **4,999 storm tracks:** checked; 4,547 have max wind.
- **Haiyan 2013:** 170 kt, "higher than 99% of 4,547 storms with values". The data also holds Haiyan 2001 (90 kt) and Haiyan 2007 (50 kt).
- **14,537 earthquakes, M ≥ 6, since 1900:** checked.
- **Tohoku:** "2011 Great Tohoku Earthquake, Japan", magnitude 9.10, "higher than 99% of 14,537 earthquakes". It ties with Sumatra 2004 (9.1), behind Alaska 1964 (9.2) and Chile 1960 (9.5).
- **Heatwaves summer 2023:** 92 daily steps, 1 Jun to 31 Aug. `time 45` is 16 Jul ("46 of 92").
- **Placeholder:** "Search places or your data".
- **The search bar:** it matches places (GeoNames) and values in the open layers' first five text fields. It uses case-insensitive substring matching with no accent folding. So:
  - "Typhoon Haiyan, 2013" matches nothing.
  - "Haiyan" matches the three storms named Haiyan, in row order 2001, 2007, 2013, plus the place Haiyang.
  - "Tōhoku" (with macron) matches nothing in the data; "Tohoku" matches the one quake.
  - "heat" matches the layer name "heatwaves summer 2023".

## Sources and capture

`capture/capture.sh <name>` launches the release build (`/Volumes/LaCie/cargo-target/layeredmap-video/release/layeredmap`) with `--tour` from `tours/<name>.tour`. It also passes optional `.files`, `.env` and `.args` and `-NSRequiresAquaSystemAppearance YES`. It finds its own window by PID and records only that window (ScreenCaptureKit, `capture/wrec.swift`).

| Take | Tour | Used for |
|---|---|---|
| `ty` | `example storms` · `esc` · `150,15,2.3` · hold · `126,11,7.5` · `click 124.53,11.05` · hold · `128,14,4.4` · holds | tracks (12.5 s), dive (15.40–16.30), tiles sharpening, click and card (21.3–22.6), Haiyan's whole track with card (31.0) |
| `ty_plate` | holds, started with `--view 150,15,2.3` | the empty plate the tracks draw in over. It is aligned by (−3.97, −1.43) px because the window was 2 pt larger; the shelf and toast regions always come from `ty` |
| `eq` | `theme dark` · `example earthquakes` · `card style_by_value magnitude` · `esc` · `175,12,2.4` · hold · `142.373,38.297,10` · `click 142.373,38.297` · `175,12,2.4` · holds | the ring (18.0) and the Tohoku card at the same view (36.0). The click is made close up, as the site screenshot does, and then flown back |
| `eq_plate` | `theme dark` · holds, `--view 175,12,2.4` | dark plate |
| `hw` | `example heatwaves` · `esc` · `-28,32,3.4` · `time 0` … `time 45` · `click -15,28` · holds | one frame per daily step; the 16 Jul cell card |

## Re-render

```bash
cd design/launch-video-v3
./capture/capture.sh ty   # etc.: ty ty_plate eq eq_plate hw (leave the screen alone while they run)
./build.sh 6              # CFR copies, frame cache (cache/), 6 parallel segments, sound, encodes: about 2 min
python edit/compose.py out/stills --stills 4.6,16.5    # single frames
```

The pieces:
- `edit/edl.py`: the timeline, the layout of the table, and every measured position in the footage.
- `edit/frames.py`: the frame cache.
- `edit/compose.py`: the compositor. Every frame is a pure function of t.
- `audio/synth.py`: the sound.
- `edit/sheet.py` and `edit/grid.py`: review sheets.

The Python env `lm-video` is shared with v2 (see `../launch-video-v2/README.md`).

---

# v3.1: "open any geospatial data you find online" (31 s)

This cut has the same one-take style, music, drop and silent stop as v3, but a new story. It opens on a folder of real downloaded files. One file at a time goes into NimbleMap and becomes its film. The search bar only searches well-known places. Build it with `./build.sh 6 3.1`; the code is in `edit/edl31.py` and `edit/compose31.py`, and `audio/synth.py` runs with `LM_CUT=3.1`. The v3 outputs are untouched.

## The files: conversions (scratch folder outside the repo; nothing committed)

`data/make_files.sh "<folder>"` writes the app's example data out as real files, using GDAL 3.13.3 (`ogr2ogr`, `gdal_translate`) and netCDF `nccopy` from the mamba env `layeredmap-ref`. It was run into the session scratch folder `…/scratchpad/map data/`.

| File (as a download would be named) | Size | From (`assets/examples`) | Command |
|---|---|---|---|
| `IBTrACS_storm_tracks_1980-2026.shp` + `.shx .dbf .prj .cpg` | 5.3 MB (shp) | `storm tracks.gpkg` (IBTrACS) | `ogr2ogr -f "ESRI Shapefile" -lco ENCODING=UTF-8 -nln IBTrACS_storm_tracks_1980-2026 IBTrACS_storm_tracks_1980-2026.shp "storm tracks.gpkg" storms` (DBF field names are limited to 10 characters, so `max_wind_kt` becomes `max_wind_k`) |
| `usgs_earthquakes_M6plus_1900-2026.gpkg` | 2.2 MB | `earthquakes.csv` (USGS) | `ogr2ogr -f GPKG -nln earthquakes -a_srs EPSG:4326 -oo X_POSSIBLE_NAMES=longitude -oo Y_POSSIBLE_NAMES=latitude -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES …gpkg earthquakes.csv` |
| `GDO_heatwave_intensity_JJA_2023.nc` | 12.6 MB | `heatwaves summer 2023.nc` (Copernicus GDO, NetCDF-4) | `nccopy "heatwaves summer 2023.nc" GDO_heatwave_intensity_JJA_2023.nc` |
| `ETOPO_2022_relief_30min.tif` | 375 KB | `elevation.tif` (NOAA ETOPO 2022) | `gdal_translate -co COMPRESS=DEFLATE -co PREDICTOR=2 elevation.tif ETOPO_2022_relief_30min.tif` |
| `NCEI_volcano_locations.csv` | 106 KB | `volcanoes.csv` (NOAA NCEI; latitude/longitude columns) | copied as is |

**Verified opening.** Each file was passed to the release build as a command-line file argument. That is the same `Command::Open` path that Open With and drag-and-drop use (`apps/layeredmap/src/main.rs`). Takes `f_tif`, `f_csv`, `f_ty`, `f_eq` and `f_hw` show all five formats open:
- the GeoTIFF shows a Band 1 raster from −9,734 to 5,734;
- the CSV shows 1,609 points, located from latitude/longitude;
- the shapefile shows 4,999 lines and the same Haiyan card (170, higher than 99% of 4,547);
- the GeoPackage shows 14,537 points and the same Tohoku card;
- the NetCDF shows 92 steps and the same 16 Jul cell card (50.55 °C, higher than 96%).

The film's takes `g_ty`, `g_eq` and `g_hw` open these files, not the examples. The cards name the files' own layers, for example "IBTrACS_storm_tracks_1980-2026 · line". The shapefile's cards say "lines" where the example says "storms".

## The folder shot

A real Finder window of the folder was opened with `open` and captured window-only (`raw/finder.png`, git-ignored). It isn't used on screen: it carries the owner's sidebar (account name, cloud drives), and this Mac's installed apps give .shp, .gpkg and .nc recognised kinds and icons, so it isn't the generic Finder people see. The film uses a faithful re-layout instead: the real names, sizes and modified time ("Today at 08:38"), and generic document icons with the extension printed on them. The file "going into" NimbleMap is composited as a chip of that row flying into place; the window then opens out of it, and every map after that is real. No "no application" dialog was triggered. The Finder window was left open, because closing it would take a keystroke or an Apple Event to Finder.

## Place searches (`assets/geonames/cities.tsv`, `crates/lm-core/src/places.rs`, `crates/lm-ui/src/search.rs`)

The search can't be driven from a tour. Each landing is therefore recorded with the tour's `lon,lat,zoom` step. The place search calls the same `fly_to`, at the place's coordinates and `zoom = 17 − log10(population)`, clamped to 10–13. The typed text is drawn into the real bar. After Enter, the bar shows the place's own label, as the app writes it.

| Typed | First result | Landing | Data rows ahead of it |
|---|---|---|---|
| New York | New York City, New York, United States (the only prefix match) | −74.00597, 40.71427, z 10.055 | none (no storm value contains it) |
| Tokyo | Tokyo, Japan (exact) | 139.69171, 35.6895, z 10.012 | none (no quake place contains it) |
| Palermo | Palermo, Sicily, Italy (exact; ranked above Palermo, Argentina by population) | 13.3636, 38.1166, z 11.188 | none |

The result list that appears while typing isn't shown, because no tour step can open it.

## Real numbers used

- **Sandy (2012):** max_wind_k 100, higher than 75% of 4,547 lines with values, Category 3. It was clicked inland in Pennsylvania close up, then shown in a regional view around New York; the ripple marks its landfall near Atlantic City.
- **Tohoku (2011):** magnitude 9.10, higher than 99% of 14,537 earthquakes.
- **Palermo cell, 25 Jul 2023 (step 54, "55 of 92"):** 36.50 °C heat-wave intensity, higher than 93% of this layer's cells. That is the cell's summer peak. On 16 Jul, Rome was 2.94 °C and Phoenix 3.19 °C, and Madrid had no data.

## Beat sheet as built

| t (s) | Beat |
|---|---|
| 0.00–3.00 | The folder `map data` (9 files) with a slow push; caption **Shapefile · GeoPackage · NetCDF · GeoTIFF · CSV** (0.45–3.35); the .shp row is selected at 2.5 |
| 3.00–4.20 | The .shp file lifts out and flies across the table (motion blur) |
| 4.20 | It lands on the music's first downbeat, and the storm window opens out of it |
| 4.85–5.75 | The 4,999 tracks draw in over a plate of the same view |
| 6.00–6.95 | Push onto the bar; it types **New York**; Enter at 6.95 and the bar reads "New York City" |
| 6.95–7.95 | The app's own flight to New York City (the search's landing): few tracks at city zoom |
| 7.95–8.55 | Pull with a slight dutch to the regional view: Sandy highlighted, its card, a ripple at landfall |
| 8.55–9.40 | Onto the card; "higher than 75% of 4,547 lines" lifts |
| 9.40–10.80 | **100** lifts (the value only); **100 knots.** · *Sandy, 2012 · Category 3*; the frame behind is blurred and washed |
| 10.80–12.05 | Travel right; the .gpkg file comes in and lands |
| 12.05–12.70 | The dark earthquake window opens out of it |
| 12.70–14.00 | The Ring of Fire lights up dot by dot (the breakdown starts at 12.73) |
| 14.15–14.70 | Push onto the bar; it types **Tokyo**; Enter |
| 14.70–15.35 | The flight to Tokyo (no quakes at city zoom) |
| 15.35–16.00 | Japan with the Tohoku card, a ripple on the dot; the magnitude row lifts |
| 16.00–16.90 | **M 9.1.** · *Tōhoku, 2011*; the .nc file arrives under the window; held still |
| 16.90 | DROP: the file collapses to a line, which becomes the slider's track; whip along it |
| 17.25–19.40 | Heatwaves over the North Atlantic, 1 Jun → 25 Jul 2023 |
| 19.55–20.15 | Push onto the bar; it types **Palermo**; Enter |
| 20.15–20.85 | The flight to Palermo: one 0.5° cell fills the window; it is clicked and its card appears |
| 20.85–21.75 | The regional view of Sicily with the card; a ripple on Palermo; onto the card |
| 21.95–23.05 | **36.50 °C** · *heat-wave intensity, Palermo, 25 Jul 2023* (blur and wash) |
| 23.20–31.00 | As in v3: pull out; the three films gather over the bar; the stop at 26.80 (silent until 27.30); the hit; "NimbleMap" is written; the end card |

Audio: the same synthesiser. A quiet pad sits under the folder; there are whooshes as each file flies; the downbeat comes at 4.20 when the first file lands; key clicks follow the three typed place names; the breakdown runs 12.73–16.90, then the drop at 16.90, the stop at 26.81–27.30 and the hit at 27.30. Mastered to −14.0 LUFS.

## Also fixed from the v3 review (v3.1 only)

The big-number beats now blur the frame (σ 16 px) under an 80% wash, so no card text reads through. The lifted row is the value alone ("100", "36.50 °C"), without the raw field name.

---

# v3.2: v3.1 polished and narrated, with a Pro beat (45 s)

Build it with `./build.sh 6 3.2`. The code is in `edit/edl32.py` and `edit/compose32.py`, which reuse v3.1's code. The script is `audio/narration.txt` and the voice is rendered by `audio/voice.sh`. The music, ducking and two masters come from `audio/synth32.py`, and the captions from `audio/srt.py`. The v3 and v3.1 outputs are untouched.

## Polish

- **Row lift.** The `.shp` row lifts out whole: blue highlight, icon, name, date and size. It flies across as that row, and its slot is left as a faint outline. The `.gpkg`, `.nc` and Pro beats arrive as the same kind of row.
- **Big numbers.** The lone value chip is gone. Each beat lifts a real row from the card, then shows the big words and a one-line subtitle with the card's real numbers: Sandy 100 knots, higher than 75% of 4,547 tracks; Tōhoku M 9.1, higher than 99% of 14,537; Palermo 36.50 °C, higher than 93% of cells.
- **Retiming.** Holds and pushes are retimed around the voice.

## Narration

The voice is **Microsoft `en-US-AndrewNeural` via `edge-tts` 7.3.1, rate +4%** (`audio/voice_edge.py`, mamba env `lm-video`). This was the owner's choice; only the script text goes to Microsoft's TTS service. Each line is saved as an mp3 with its word boundaries, and the boundaries place the line (its first word starts at the time in `edl32.VO`) and set the SRT spans.

The text is the caption column of `audio/narration.txt`, with normal spellings. "Tohoku" is plain. "NimbleMap" is kept as written: its word boundary (0.54 s) matches "Nimble Map" (0.24 + 0.28 s), so Andrew splits it. Whisper isn't installed, so there was no transcription check. The fallback (`VO_ENGINE=say ./build.sh 6 3.2`) uses macOS `say` with Samantha at 158 wpm (`audio/voice.sh`).

Claims checked against the files:
- The storm file holds 4,999 tracks, seasons 1980–2026, so "nearly five thousand, since 1980".
- The earthquake file holds 14,537 rows, magnitude 6.0 and up, dated 1900-01-05 to 2026-10-16.
- Palermo's card says "higher than 93% of this layer's cells (estimated from a sample)", so the line says "more intense than most of the map".

## Pro beat (one paid call, of the 4 allowed)

- **Take:** `p_ty`, with `LM_TIER=pro` (as in v2), the storm shapefile opened as a file argument, and the tour `esc · 150,15,2.3 · hold · ask Show only Category 5 storms · holds`. No key was read or logged.
- **Answer (real footage):** the plan "Show only IBTrACS_storm_tracks_1980-2026 where strength is Category 5: 228 of 4,999", with the matching tracks highlighted on the map and Keep / Discard.
- **Verified:** `select count(*) from storms where strength='Category 5'` gives 228 of 4,999.
- **On screen:** the typed question is drawn into the real Ask box (Inter Regular 27.7 px, fitted to the placeholder, on its baseline). The "Planning…" row is sped up from 3 s to 0.6 s.

## Music and mix

v3's synthesiser is rewritten as a calmer bed:
- warmer, longer pads;
- softer hats and claps, a quieter arpeggio and lead;
- the same grid, with the drop and silent stop moved with the beats: first downbeat 5.40, breakdown 18.58, drop 22.75, stop 38.92 (one silent beat, with no narration), hit 39.45;
- key clicks quieter;
- the music sidechain-ducked 9 dB under the voice (100 ms lookahead, 120 ms attack, 450 ms release);
- the voice EQ'd for laptop speakers (high-pass 110 Hz, presence lift), with its speech RMS about 12 dB over the ducked bed.

Both masters are −14.0 LUFS integrated, true peak ≤ −1.2 dBTP.

## v3.2 light earthquake retake

The earthquake film is retaken in the light theme to match the other two: take `g_eqL` (the `g_eq` tour without `theme dark`) and the light plate `eq_plateL` (`--view 175,12,2.4`, nothing open; alignment (0, 0)). The light basemap is the app's default, the same as the storms and heatwave films. The dots keep the app's own symbology, "Color by a value: magnitude" (Viridis), which reads well on the light map without any retinting. Timing, camera, narration and music are unchanged. The "M 9.1." beat now uses the light wash, and the ripple uses the accent colour. Line voicing is cached: `build.sh` calls edge-tts only when `audio/vo_edge/*.mp3` is missing. To re-voice after script edits, edit `audio/narration.txt` (3rd column), run `mamba run -n lm-video python audio/voice_edge.py`, then adjust `edl32.VO` if line lengths change.

---

# v3.3: the richer script (69 s)

Build it with `./build.sh 6 3.3`. The code is in `edit/edl33.py` and `edit/compose33.py`, which reuse v3.2's code. The script is `audio/narration33.txt`, and the voice is `audio/vo_edge33/` (edge-tts Andrew, +4%). The sound comes from `audio/synth33.py`, which also writes the voice-only track.

## New footage

- **`g3_ty`** (the storm shapefile): the world view, then a real zoom (to −80, 24, z4.6) and pan (to −62, 33, z4.6), then the landing for "New York". The flights play at half speed. A 0.35 s dissolve to the settled view covers the moment when the tiles are still loading and part of the map is blank in the recording.
- **`g3_tyB`** (the storm shapefile again; tour: `layer 0 · find Sandy · … · click · fly`): the layer's real **Find** field shows **"Sandy — 2 found"**, lists both rows (name: Sandy), selects both tracks and zooms to them. They are Sandy 1985 (South Indian, Category 4) and Sandy 2012 (North Atlantic, Category 3). Clicking then opens the 2012 card. The typing is drawn into the real Find field (Inter 27.5 px, on its baseline).
  - The toolbar search dropdown still can't be opened by a tour step. Feature search is therefore shown with the layer's own Find field, which is real footage.
  - The take started at the layer extent, not New York, so the cut from the New York view into the Find take is a 0.5 s dissolve while the camera pushes in on the inspector.
  - The first attempt at a single take stalled for about 200 s after the New York flight, so the footage is split into two takes.
- **`f_tif`, `f_csv`**: the GeoTIFF relief and the CSV volcanoes, from v3.1's format checks. They open in quick succession from the folder.
- **`p_eq`** (Pro, `LM_TIER=pro`, 1 paid call): "Which earthquakes were deeper than 300 km?" The answer is "Show only earthquakes where depth_km above 300: 655 of 14,537", with those quakes highlighted.
  - Verified: the CSV has 655 rows with `depth_km > 300` (657 with ≥ 300; 103 rows have no depth).
- **`p_ty`**: reused from v3.2. "Show only Category 5 storms" gives 228 of 4,999, which matches the data.

Paid ask calls for this film: 2 in total (Category 5 in v3.2, deep earthquakes here), of the 4 allowed.

## Music

The same bed, retimed:
- quiet pad under the folder, with bells as the GeoTIFF and the CSV open;
- first downbeat when the storm window opens (13.0);
- breakdown 32.13;
- drop 36.30;
- silent stop 62.91–63.43 (no voice);
- hit 63.43.

The music ducks 9 dB under the voice. Both masters are about −14 LUFS.

---

# v3.4 (74.6 s)

Build with `./build.sh 6 3.4`. The code is in `edit/edl34.py` and `edit/compose34.py`: v3.3's compositor runs on a warped clock and is fed by the new takes. Two short holds give the longer lines room, and the Pro section is new. The sound comes from `audio/synth34.py`, the script is `audio/narration34.txt`, and the voice files are in `audio/vo_edge34/`.

- **Build:** every shot was recorded with the frozen fixed build `/Volumes/LaCie/lm-video-build-f0f19e3/layeredmap` (phase-4 f0f19e3), set via `LM_BIN`.
  - GeoTIFF relief now fills every world copy.
  - The 0.5° heatwave grid has no strip at the antimeridian (`f4_tif`, `hw4_wrap`).
- **Sharp holds:**
  - Tours hold 7.5–10 s after every move (`layer 99` steps).
  - `capture/settle.py` finds when the picture stops changing after each move.
  - The edit plays the real flight, then dissolves (0.25–0.35 s) to the settled frame, so every held frame has its tiles. Labels were checked at each hold.
- **Heat (Barcelona, 16 Aug 2022):** the GDO Warm Spell Intensity ERA5 v3.0.0 2022 file was downloaded to `/Volumes/LaCie/data/layeredmap-netcdf-test/jrc_warm_spell/` (SHA-256 `493c5c36…e0e2`). It was converted with `data/make_heatwave_nc.py` to `GDO_heatwave_intensity_JJA_2022.nc` in the scratch folder. That converter reproduces the shipped 2023 example exactly. The card reads 36.10 °C, "higher than 98% of this layer's cells (estimated from a sample)".
- **Pro (2 paid calls):** "How many Category 5 storms have passed within 100 km of Tokyo?" The plan was: filter Category 5 (228 of 4,999), then make an area within 100 km of Tokyo (built-in World cities), then count lines in it. The result was **Tokyo: 7**.
  - GeoPandas, using the same Natural Earth Tokyo point and geodesic distance, also gives 7: Abby 1983, Ma-On 2004, Neoguri 2014, Vongfong 2014, Noul 2015, Hagibis 2019, Kong-Rey 2024.
  - Take `p4_ty` is used. `p4b_ty` was reframed and is unused.
- **Music grid:** downbeat 14.40, breakdown 33.53, drop 37.70, silent stop 68.48–69.00, hit 69.00.

---

# v3.5 (78.8 s)

Build: `./build.sh 6 3.5`. Code: `edit/edl35.py` and `edit/compose35.py` (v3.4's compositor, re-cut); sound from `audio/synth35.py`; narration as in v3.4 (`audio/narration34.txt`, `vo_edge34`).

- **Takes.** g5_tyA, g5_tyB, g5_eq and g5_hw were recorded on the frozen build f0f19e3, holding 20–31 s after every move.
- **Loaded detection.** `capture/loaded.py` takes the last frame of each hold as the reference (by then every tile has arrived). It finds the first frame after the move that matches that reference. The edit plays the move sped up to that frame, then holds the reference.
- **Framing.**
  - New York: the search's landing (about 0.3 s, loaded), then the z6.5 region.
  - Tokyo: the same, then the z6.4 region.
  - Barcelona: the flight and the city-zoom landing are cut out (a dissolve from the slider view to the loaded z5.5 region).
  - Push-ins are at most about 1.26× over the 2× capture.
- **Pro.** Take p6_ty used the askfix build `/Volumes/LaCie/lm-video-build-askfix/layeredmap` (0.7.1, a0cfd7e) with `LM_TIER=pro LM_DEV_PRO=1`: 1 real call, the 7th of the owner's 8.
  - The question was the owner's own: "How many Category 5 storms have passed within 100 km of Tokyo?"
  - The plan: filter strength = Category 5 (228 of 4,999), then within 100 km of Tokyo, then lines in it.
  - The card reads **7 lines**, "Category 5 lines in Tokyo". The map shows only the 7 tracks and the 100 km circle.
  - GeoPandas gives the same 7: Abby 1983, Ma-On 2004, Neoguri 2014, Vongfong 2014, Noul 2015, Hagibis 2019, Kong-Rey 2024.
- **Music grid.** Downbeat 14.40; drop 39.87; silent stop 72.74–73.26; hit 73.26 (16 bars after the drop).

---

# v3.6

This is v3.5 with only the Tokyo Pro beat re-recorded. Build it with `./build.sh 6 3.6`; the code is in `edit/edl36.py`, `edit/compose36.py` and `audio/synth36.py`.

- **Take.** p7_ty was recorded with the askfix build at phase-4 bc3b439 (`/Volumes/LaCie/lm-video-build-askfix/layeredmap`).
- **Reply.** The 7th call's raw model reply was never saved, so the plan was replayed with `LM_DEV_PRO=1 LM_ASK_FIXTURE=/Volumes/LaCie/lm-video-build-askfix/tokyo_plan_real.json` (no paid call). It is the same 3 steps as that call: filter Category 5, buffer World cities Tokyo by 100 km (one area per feature), then count_in.
- **Card.** It reads "Lines within 100 km of Tokyo", "7 lines" and "Category 5 lines within 100 km of Tokyo". The map shows exactly 7 tracks and the 100 km circle.
- **Edit.** The timing is unchanged from v3.5.

## v3.6 Pro shot (Tokyo)

The final cut's Ask answer replays `tours/tokyo_plan_real.json` (`tours/p7_ty.env`, no model call).
The real model call recorded for v3.5 produced this three-step plan (filter Category 5 → 100 km
buffer around Tokyo, one area per city → count lines), but its reply wasn't saved; the file is
that plan rebuilt from the recorded frames. The app computes the answer (7) live from the IBTrACS
file. Seven of the eight approved paid calls were used across the video's versions.

---

# v3.7: one continuous camera per shot, rendered offline (78.8 s)

v3.6's cut, timing, narration, captions and music, with every map shot re-made the way Google Earth Studio / After Effects map sequences are made: one continuous keyframed camera path per shot, rendered offline frame by frame by the app itself, so every frame is fully loaded and the timing no longer depends on how fast tiles arrive. Build:

```bash
cd design/launch-video-v3
path/render.sh ty eq hw pro ty_plate eq_plate   # the app renders the map shots (about 12 min, 4 in parallel)
./build.sh 6 3.7                                # composite, sound, encodes (about 5 min)
python path/evidence.py <v3.6 1080p.mp4> out/NimbleMap-launch-v3.7-1080p.mp4 <dir>   # plots, side-by-side
```

## The renderer (`crates/lm-ui/examples/film_render.rs`; developer tool, not part of the app)

- The real NimbleMap interface (`lm_ui::Shell`), offscreen through egui_kittest's wgpu harness, at the capture's size: 1409 × 845 pt at 2× (2818 × 1690 px, the window less its title bar).
- egui's clock is set to each frame's film time. App animations (label fades, the inspector, the toast) therefore run on film time, and the clock is held still while the frame loads.
- The camera is set exactly for every frame. A search's own fly-to is replaced by the film's path in the same frame (`MapView::set_camera` ends any app animation).
- `MapView::set_frame_by_frame` (off in the app) swaps finished map renders in at once instead of crossfading them.
- A frame is saved only when all of these hold:
  - `MapView::settled()`: the shown render is of exactly this camera and size, no tile was missing (no lower-zoom stand-ins), and no newer render is on the way;
  - there are no tile errors and the daily basemap archive has been found (the pinned Protomaps build has expired; until the lookup ends the map shows the offline outline);
  - nothing is loading, no Ask answer is pending (`Shell::ask_pending`) and no workspace job (Find index, card statistics) is running (`Workspace::jobs_running`).
- Events are the app's own:
  - typing into the search bar, the layer's Find field and the Ask box, plus Enter;
  - a `--tour`-style `click lon,lat` on the map, and `click_at` on a point of the window (the overlap chooser's row, a Find result row);
  - `layer N`, `time N` and the card actions.

## The paths (`path/camera.py`, `path/shots.py`)

- **Flights** (each search's landing, Find's zoom to both storms, Pro's zoom to its answer): van Wijk & Nuij's optimal smooth zoom/pan, ρ = 1.42 as in Mapbox's flyTo, eased along the path's perceived length.
- **Local moves** (the opening zoom and pan): the two views' similarity, interpolated about its fixed point, with log-resolution linear in the eased progress, so zoom changes at a perceptually constant rate and nothing swoops. Consecutive moves overlap, so the camera never stops between them.
- **Drift:** every shot drifts in by 0.015 zoom levels/s (about 1 %/s; 0.009 in Pro). The map never fully stops.
- **Motion blur:** frames where the map moves more than about a point within a 180° shutter are rendered as 2–8 sub-frames across the shutter and averaged in linear light (`path/blend.py`).
- **The searches land directly on the regional view.** New York goes to −73.5, 40.2, z6.5, Tokyo to 140.6, 36.4, z6.4 and Barcelona to 5.5, 41.6, z5.5. There is no city-zoom landing and no pull-back. The place is the search's own first result, chosen by Enter in the real field; the bar then reads "New York City", "Tokyo" or "Barcelona".

## The shots, as the app now does them

| Film time (s) | Shot | What happens in the app |
|---|---|---|
| 14.4–28.6 | `ty` (storm shapefile) | Tracks drawn in over the empty map (`ty_plate`). Zoom to −80, 24, z4.6 and pan to −62, 33 (overlapping). "New York" typed in the search bar (the results list shows "Your data" storm names while "N…" is typed, then New York City first). Enter at 20.90 and a flight to the New York region. `layer 0` opens the panel. "Sandy" typed into Find: "2 found" (for about 0.2 s while "S", "Sa" are typed, Find highlights the many matching tracks, as the app does). The Find field's own zoom to both storms is replaced by a flight to 43.48, 14.83, z2.01. The second result row (Sandy 2012) is clicked and opens its card ("‹ All 2 found", 100, higher than 75 % of 4,547, Category 3), then a flight in to −74.5, 39.8, z6.4 |
| 28.8–40.3 | `eq` (earthquake GeoPackage) | The Ring of Fire drawn in over `eq_plate`. "Tokyo" typed, Enter at 32.60, a flight to 140.6, 36.4, z6.4. A click on Tohoku's dot opens the app's overlap chooser ("4 objects here"; at this zoom the 2011 quake and three aftershocks are under the pointer). Its first row, "2011 Great Tohoku Earthquake, Japan", is clicked: the card shows 9.10, higher than 99 % of 14,537 |
| 39.9–50.4 | `hw` (heatwave NetCDF) | The slider steps 1 Jun → 16 Aug 2022 (`time 0…76`, one step per frame by v3.3's schedule). "Barcelona" typed, Enter at 44.40, a flight to the Barcelona region. The cell clicked: 36.10 °C, higher than 98 % |
| 49.9–69.6 | `pro` (storm shapefile, `LM_TIER=pro LM_DEV_PRO=1 LM_ASK_FIXTURE=tours/tokyo_plan_real.json`, no paid call) | The question typed into the Ask box, Enter at 59.00. The replayed plan computes 7 lines within 100 km of Tokyo. The app's zoom to its answer (135.054, 29.125, z5.451) is replaced by a flight |

The compositor (`edit/compose37.py`) is v3.6's, with these changes:
- the windows show the renders;
- the drawn-in typing is gone;
- ripples follow their places through the moving camera;
- the table camera is a monotone cubic through its keys (continuous speed; the whips are kept), instead of easing to a stop at each key.
