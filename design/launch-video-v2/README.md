# NimbleMap launch film v2: final pass (35 s, 16:9)

This film is built from real footage of the real app. Every app pixel comes from a window recording of the release build, which drives itself through its `--tour` script mode. The edit adds only four things: a clean macOS-style window frame, a drawn cursor with click ripples, captions, and the end card. The owner approved the rough cut's story; this is the polished pass, with the same story, captions and timings. The previous film (`../launch-video/`, abstract motion graphics) was rejected because it never showed the app.

## Final pass: what changed from the rough cut

- **Camera.** Push-ins are capped at 1.5× and eased in and out. Every push into a card eases back out to the whole window before the next shot. Consecutive shots **dissolve** over 0.25 s while the outgoing shot keeps running underneath (`XF` in `edl.py`). Because both shots are at 1× when they meet, only the window's content cross-fades.
- **Pro beat.** It is re-shot on a calm setup (take `pro2`): only `cities.geojson` is open on the light basemap, with no elevation raster. Pro's built-in reference layers supply the elevation, and the answer is the same: La Paz 4,405, Quito, Thimphu, Bogota, Sucre. The `ask` step fills the box in one go, so the typing is drawn **into the real Ask box in the source frame**. It uses the box's own font (Inter Regular), size (27.9 px at 2×, measured against the placeholder), colour, text start and baseline, with a caret. It replaces the placeholder until the request is sent, then the real *Planning…* row takes over.
- **File take.** It starts at source 2.95 s, after the basemap has finished fading in, so there is no flash.
- **Colour shot.** It is re-shot in the free build (take `color`), so the card has no developer-only actions. The export shot still uses the `LM_DEV=1` take, but it starts with the Style tab already showing, so the two developer-only card actions never appear on screen.
- **Audio.** The music is arranged per beat:
  - the opening is sparse
  - kick, shaker and sub come in for "built in"
  - "your own file" gets a lighter, octave-up texture with no kick
  - "share" adds a rim on 2 and 4
  - Pro moves to B♭maj9–C6/9 with bell sparkle
  - the end resolves on F
  - a soft rising bell stinger plays on the Haiyan click, with a small chime on the Beijing click
  - airy swishes sit under each caption change, and soft swells lead into each beat

  The mix is still −14.0 LUFS integrated, with a peak of −2.4 dBFS.
- **End card.** It has a calm version of the old film's logo sting: a soft accent glow blooms behind the icon as it settles, then a sheen sweeps across the wordmark at about 32 s, under a high bell.

## Outputs (`out/`, git-ignored)

| File | Spec |
|---|---|
| `NimbleMap-launch-v2-1080p.mp4` | 1920×1080, 60 fps, H.264 High, yuv420p, BT.709, CRF 16, AAC 256 kb/s 48 kHz, faststart. −14.0 LUFS integrated, peak −2.4 dBFS |
| `NimbleMap-launch-v2-web-loop.mp4` | 1280×720, 30 fps, no audio, two-pass, under 4 MB. The last 0.5 s cross-fades into the first 0.5 s, so the loop has no jump (34.5 s) |
| `NimbleMap-launch-v2-poster.png` | 1920×1080 frame at t = 4.6 s, the Haiyan card ("higher than 99% of 4,547 storms") |
| `contact-sheet.png` | one frame per second |
| `master.mkv`, `audio.wav` | lossless picture master (FFV1 4:4:4) and the mastered sound |

## Storyboard as built

| Time (s) | Caption | Picture (recording · source seconds) |
|---|---|---|
| 0.0–5.6 | **Click anything.** (0.25–2.65), then **It tells you where it stands.** (from 2.95) | `hook` 9.4–15.0. Storms over the Philippines (view 126,11 z7.5). The cursor moves to Haiyan's track and clicks it (0.95 s). The track highlights and the card appears. From 1.35 s the camera pushes in on the card: *max_wind_kt 170, higher than 99% of 4,547 storms with values* |
| 5.6–8.8 | It tells you where it stands. (to 8.55) | `walls` 18.3–21.5. The cursor clicks Beijing's walls (6.15 s). The camera pushes in on the card: *35.32 km², higher than 99% of 1,789 areas* |
| 8.8–14.5 | **Eight maps built in.** | Built-in examples, opened with no file: storms worldwide (`hook` 2.7–4.0), big earthquakes (`ex_earthquakes`), power plants (`ex_power`), and heatwaves of summer 2023 with the time slider stepping through June. The heatwaves clip is `heat` 4.2–31.0, sped up 14×; it shows 1 to 17 June |
| 14.5–16.0 | **Or double-click your own file.** | `hook` 0.05–1.55: the empty app ("Open a file to see your data on the map"). A drawn file chip, *places to visit.csv*, is double-clicked (15.28 and 15.43 s) and shrinks away |
| 16.0–19.8 | **It lands in the right place.** (from 17.6) | `file` 2.95–6.75: the CSV opened at launch, 22 points placed worldwide. The camera pushes in on the card's note, *Located using lon and lat as GPS coordinates (WGS 84). Change* |
| 19.8–22.4 | **Colour it. Export it.** | `color` 9.42–12.02 (free build). The cursor clicks *Color by a value* on the storms card (20.66 s). The tracks recolour by max_wind_kt (Viridis, 5 groups) and the Style tab opens |
| 22.4–25.5 | Colour it. Export it. | `share` 18.5–21.6. The cursor clicks *Share* (22.87 s) and the *Export map image* sheet opens. The finished slide renders: legend, lat/lon frame, caption and credits |
| 25.5–27.9 | **Pro: ask in plain words.** | `pro2` 5.16–7.56 (NimbleMap PRO badge; only world cities open). *Which capital city sits highest above sea level?* is typed into the real Ask box, and the cursor clicks send (27.2 s). The real Ask box shows *Planning "Which capital city…"* |
| 27.9–30.5 | **Free with an account.** (from 28.45) | `pro2` 12.95–15.55. The answer appears as a preview: a 3-step plan and *Cities ranked by World elevation*, with La Paz 4,405, Quito, Thimphu, Bogota and Sucre. The camera pushes in on the list, and the window fades out over the last 0.45 s |
| 30.5–35.0 | (end card) | The app icon, then **NimbleMap**, *Explore real data about the planet.*, a **Free for Mac** pill, and kangning‑huang.github.io/NimbleMap, which fade in one after another |

Sound: a calm 100 BPM bed in F major (Fmaj9–Am7–Dm9–B♭maj9). It has a pad from the start, a delayed pluck arpeggio from half a bar in, and a soft kick, shaker and sub bass from bar 2. There is a soft UI click on each of the 8 drawn clicks, and a bell with a resolving F chord on the end card. It is all synthesised in `audio/synth.py` (numpy/scipy, no samples). Click times are read from `edit/edl.py`.

## How it was captured

Hard rules, all kept:
- No keystrokes or clicks went to any app.
- The app drives itself with `--tour`.
- Only the app's own window is recorded, never the screen.
- No new permission prompt was triggered.

- **Build**: `CARGO_TARGET_DIR=/Volumes/LaCie/cargo-target/layeredmap-video CARGO_INCREMENTAL=0 cargo build --release -p layeredmap`, run from the repo root. The build was redone after 55db6c3 (coloured storm tracks keep their transparency). The `share` take was recorded on that build.
- **`capture/capture.sh <name>`** does the following:
  1. Launches the binary directly with `--tour "<tours/<name>.tour, steps joined by ;>"`.
  2. Opens any files listed in `tours/<name>.files`; command-line files open as they do with Open With or a double-click.
  3. Sets any environment variables from `tours/<name>.env`.
  4. Passes `-NSRequiresAquaSystemAppearance YES`, a Cocoa argument-domain default, so the app and its title bar are light even when macOS is in dark mode. With that, the `theme light` step isn't needed, and dropping it avoids a basemap reload flash.
  5. Finds **its own** window by the launched PID (`capture/winid.swift`), so another agent's instance is never recorded.
  6. Records it with `capture/wrec.swift`.
- **`capture/wrec.swift`** is a ScreenCaptureKit recorder. It uses `SCContentFilter(desktopIndependentWindow:)` for that one window, at 60 fps, with no system pointer, written by AVAssetWriter as H.264 at 80 Mb/s and 2× (Retina) size. It stops when the tour closes the window. ScreenCaptureKit only writes a frame when the window changes, so `build.sh` first makes constant-frame-rate copies in `cfr/`. Seeking in the raw VFR files lands late.
- `capture/events.py raw/<name>.mov` prints the moments where the picture changes a lot, which are the tour steps landing. Those moments set the source in-points so that each drawn click lands just before the app reacts.

### Tours used (`tours/`)

The first step fires about 2 s after the first map render. Map steps such as `example` and `click` then run 2.5 s apart, and flights run 6 s apart. `layer 99` (select a layer that doesn't exist) is used as a 2.5 s "hold" that changes nothing.

| Tour | Steps | Used for |
|---|---|---|
| `hook` | `example storms` · `126,11,7.5` · `click 124.53,11.05` · `126,11,7.5` | Haiyan click and card; storms worldwide; the empty app at launch |
| `walls` | `example walls` · `basemap satellite` · `114.5,36.5,6.4` · `116.39,39.91,10.6` · `click 116.39,39.91` · hold | Beijing card. At zoom 10.6 the app falls back from satellite to the map, by design |
| `heat` | `example heatwaves` · `time 4` · `time 8` … `time 88` | time-slider beat |
| `ex_earthquakes`, `ex_power` | `example …` · 3 holds | built-in montage |
| `file` (+ `file.files` = `data/places to visit.csv`) | 3 holds | own-file beat |
| `color` | `example storms` · `160,22,3.1` · `card style_by_value max_wind_kt` · 2 holds | colour (free build) |
| `share` (+ `LM_DEV=1`) | `example storms` · `160,22,3.1` · `card style_by_value max_wind_kt` · hold · `export` · hold | export sheet only |
| `pro2` (+ `LM_DEV_PRO=1`, `pro2.files` = `assets/examples/cities.geojson`) | 2 holds · `ask Which capital city sits highest above sea level?` · 3 holds | Pro beat (1 paid call) |
| `pro` (+ `LM_DEV_PRO=1`) | `example world` · hold · `ask …` · 3 holds | rough cut's Pro take, now unused (elevation raster too busy) |
| `ex_volcanoes`, `ex_airports`, `ex_wallsat` | `example …` · holds | captured alternates, not in this cut |

`data/places to visit.csv` holds 22 real cities (name, country, lat, lon, population) taken from the app's own Natural Earth `cities.geojson`.

## How to re-render

```bash
cd design/launch-video-v2
# one-time: the Python env (shared with ../launch-video)
~/miniforge3/bin/mamba create -y -n lm-video -c conda-forge python=3.12 numpy scipy pip pillow opencv
~/miniforge3/bin/mamba run -n lm-video pip install pyloudnorm
# re-capture any take (about 10-60 s each; the app runs on screen, so leave it alone)
./capture/capture.sh hook      # etc.: hook walls heat ex_earthquakes ex_power file color share pro2
./build.sh 6                   # CFR copies, 6 parallel picture segments, sound, encodes: about 4 min
python edit/compose.py out/stills --stills 1.2,4.6,27.6   # single frames for tweaking
```

- `edit/edl.py` is the whole edit: the shots (source in/out, speed maps, camera keys, cursor paths, clicks, overlays), the captions, and the end-card time. The content points for the clicks are worked out with Web Mercator from the tour's view, then checked against the frames.
- `edit/compose.py` is the compositor. It uses Pillow and OpenCV and writes FFV1. Every frame is a pure function of t.

If you re-capture a take, run `capture/events.py` on it and update that shot's `map` in-point. A re-run lands within a frame or two of the old timing.

## Notes and known limits

- **Export needs `LM_DEV=1` in this build.** The map image sheet is a Phase 5 surface. The free release build at the current `BUILD_PHASE` hides it (the toolbar Share button is greyed), so the `share` take runs with `LM_DEV=1`. Only its Style-tab and export-sheet frames are used, and no developer-only UI is on screen.
- **Paid ask calls:** 3 in total. Two were for the rough cut: one sent before any layer was open, so the Ask box was hidden and nothing showed, and the `pro` take. One more was for the calm `pro2` take used here.
- **Drawn elements that aren't app UI:** the window frame and title bar, the cursor and click ripples, the *places to visit.csv* file chip (it stands in for the Finder double-click), the typed question in the Pro beat (drawn into the real Ask box, because the `ask` step fills the box in one go), the captions, and the end card.
- The Pro window is a little smaller (1276×830 pt against 1405×875 pt) because macOS keeps a separate saved frame for the Pro app id.
- Walls over satellite didn't read: the walls are a few pixels wide at the zooms where the satellite imagery is sharp. The montage uses earthquakes and power plants instead, and the walls appear on the light map in the Beijing click.
