# NimbleMap launch film (20 s, 16:9)

A 20-second motion-graphics launch piece for NimbleMap. It is built from the app's own interface language: item tags, status icons, the ⌘F search, layer and object cards, and metric cards, cut on a 122 BPM beat in dark mode. Everything is code, and the film renders the same way every time. There is no footage, no screenshot, no sample and no downloaded music.

## Outputs (in `out/`, git-ignored)

| File | Spec |
|---|---|
| `NimbleMap-launch-1080p60.mp4` | 1920×1080, 60 fps, H.264 High, yuv420p (BT.709), CRF 15, AAC 320 kb/s 48 kHz, faststart |
| `NimbleMap-launch-720p-web.mp4` | 1280×720, 60 fps, two-pass 2.6 Mb/s, AAC 128 kb/s, under 8 MB |
| `NimbleMap-launch-poster.png` | 1920×1080 logo end card (t = 19.2 s) |
| `contact-sheet.png` | one frame every 0.5 s (8 × 5 grid) |
| `audio.wav`, `audio_cues.txt` | the mastered track and every sound cue's time (s and beat) |
| `master.mkv` | FFV1 lossless 4:4:4 picture master (no audio) |

## How to render

```bash
cd design/launch-video
# one-time: Python env for the audio
~/miniforge3/bin/mamba create -y -n lm-video -c conda-forge python=3.12 numpy scipy pip
~/miniforge3/bin/mamba run -n lm-video pip install pyloudnorm
npm install                 # puppeteer-core only; it drives the installed Google Chrome
./build.sh 5                # 5 parallel Chrome pages, about 5 min on an M-series Mac
```

- `node render.mjs stills 1.5,7.2,19.2 out/stills` renders single frames, which is useful when tweaking.
- Open `index.html` through any static server rooted at the repo, then call `renderFrame(t)` in the console to scrub.
- `render.mjs` serves the repo root on localhost so the page can load `assets/fonts/Inter-*.ttf`, `NotoSansSC-Medium.otf` and the Peek logo SVG straight from the repo.

**Determinism.** Every frame is a pure function of `t`. The noise comes from a seeded PRNG (mulberry32 in JS, `default_rng(122)` in Python), and nothing reads a clock. Chrome runs with `--disable-gpu`, so it uses the software rasteriser, and frames can be rendered in any order by any worker.

## Tech

- `scene.js` holds the whole film: Canvas 2D, about 900 lines, one function per section plus three transition compositors.
  - 3D is hand-rolled. The globe is an orthographic projection with a graticule. The tag field is a perspective point cloud with depth fog and a depth-of-field crossfade to pre-blurred sprites.
  - The board is a true perspective camera. Each card gets its own affine fit to its projected corners, so text stays crisp on the tilted plane.
  - The metrics use an isometric affine basis with extruded card edges.
- **Motion blur.** On fast moves, `renderFrame` averages 8–12 sub-frames across a 180° shutter (1/120 s). The blade is drawn once on top of that average as a streak, so it does not strobe.
- **Grain.** A ±2-level seeded dither keeps the near-black gradients from banding in 8-bit H.264.
- `audio/synth.py` synthesises everything with numpy/scipy:
  - instruments: kick, clap, hats, a warm one-pole-filtered saw bass, a detuned saw pad and a plucked arpeggio
  - sound effects: band-swept noise whooshes, risers, zaps, metallic "shing", key clacks, pops, bell-partial chimes and a pitch-dropping sub boom
  - mix: a noise-IR convolution reverb on a send bus, and sidechain ducking from the kick
  - master: integrated loudness to −14 LUFS (pyloudnorm), a look-ahead peak limiter, and a true-peak check at 4× oversampling (≤ −1 dBTP)
- Fonts: Inter Regular/SemiBold and Noto Sans SC Medium come from `assets/fonts`. **Geist Mono** (OFL, copied with its licence into `fonts/`) is used for IDs, file names and other mono text.

## Palette

| Token | Hex | Use |
|---|---|---|
| near-black | `#0B0C10` → `#1A1B23` | radial background, vignette to black |
| panels | `#17181F` / `#191A22`, border `rgba(255,255,255,.10–.13)` | dark rounded panels, 1 px translucent border, soft shadows |
| white | `#FFFFFF` (body text `#E3E6EC`) | type |
| **accent** | **`#2563EB`** | the only accent: glows, check disc, highlight row, Haiyan track, Pro badge, "Share." |
| amber | `#F2C94C` | "Locating" pie, grid/point glyphs |
| green | `#4EBA86` | "Styling" pie, finished bars, the 0 counter, the "Ready" badge |
| gray | `#8A8F98` | metadata, the dashed "Queued" ring, 凌波微图 |

## Storyboard and timing (beat = 60/122 = 0.4918 s)

The brief's section boundaries (2 / 4.5 / 7.5 / 10.5 / 13.5 / 16.5 s) are snapped to the nearest beat, so every cut and hit lands on the grid. All boundaries are within 0.25 s of the brief.

| Section | Beats (time) | Picture | Sound |
|---|---|---|---|
| 1 Slice | 0–4 (0–1.97 s) | A pinpoint glows (0.04 s), stretches into a line on beat 1 (0.49 s), and rotates 45° as a white graticule globe opens on beat 2 (0.98 s). The line splits into 4 diagonal stripes on beat 2.5 (1.23 s). On beat 3 (1.48 s) the globe is sliced into 5 bands that shear apart. The camera dives in from 1.55 s and reaches whiteout on beat 4 | laser rise from 0.2 s; zap + shing at 1.475 s; dive whoosh from 1.55 s |
| 2 Tags | 4–8.25 (1.97–4.06 s) | Flash out of white into a 3D field of ~100 drifting item tags (storm ID 2013306N07162, D01–D17, "earthquakes · point", EPSG:4326, GCJ-02, districts.geojson, 凌波微图 …). Line 1 "Double-click any map file." snaps on beat 5 (2.46 s). Line 2 "It lands on the map." snaps on beat 6 (2.95 s). Five format chips (Shapefile · GeoJSON · CSV · GeoTIFF · NetCDF) tick in on 16ths from beat 6.5 (3.20 s). "on the map." shifts from accent to white on beat 7.5 (3.69 s) | kick + crash enter at 1.967 s; pops at 2.46 and 2.95 s; 5 chip ticks |
| ↘ blade | 8.25–9 (4.06–4.43 s) | A diagonal blade (62°) wipes the frame left to right | whoosh + shing at 4.24 s (the blade crosses mid-frame) |
| 3 Status | 9–15 (4.43–7.38 s) | The icon appears (8.7 B) and morphs on each beat: **Queued** dashed ring (b9) → **Reading** hollow ring + spinner (b10, 4.92 s) → **Locating** amber half pie (b11, 5.41 s) → **Styling** green ¾ pie (b12, 5.90 s) → **On the map** accent disc, check and 16 burst ticks + ring wave (b13, 6.39 s). Labels slide; mono notes give districts.geojson · 17 areas · EPSG:4326 · colour by a value · D01–D17; a 5-segment step track fills. Zoom into the check, b14–15 | click per state (b9–b12, pitched up a minor third each); chime (D6 + A6) + sparkle at 6.39 s; riser from 6.89 s |
| 4 ⌘F | 15–21 (7.38–10.33 s) | The full-frame blue splits on the diagonal. Keycap ⌘ (b15) and F (b15.5) appear, both press on b16 (7.87 s) and shrink into the field's ⌘F hint as the menu pops open on b16.5 (8.11 s) with "Search places or your data". Typing h · a · i · y on b17, 17.5, 18, 18.5 filters 8 rows down to **Haiyan (2013) · 2013306N07162**. Enter on b19 (9.34 s): the row flashes, the storm map zooms and Haiyan's track draws in accent. On b19.5 (9.59 s) the panel collapses into the toast "Zoomed to Haiyan" | swoosh + crash at 7.38 s; key pops; double clack at 7.87 s; open pop at 8.11 s; 4 typing clacks; confirm pop at 9.34 s; toast swish + two-note chime at 9.59 s |
| ↘ push | 20.75–21.3 | Depth push: the map recedes, the board flies in | whoosh |
| 5 Board | 21–27 (10.33–13.28 s) | A perspective board with columns Open · Click · Share, layer cards (districts, cases, elevation, heatwaves) and object cards (Haiyan "99% of 4,547 storms", D07). On each beat b22–b26 one card lifts with a growing shadow, travels and settles while the others reflow. Bottom left: **Open.** (b22) **Click.** (b24) **Share.** (b26, accent) | slide whoosh per card (alternating pan), settle click; low pop on each verb |
| ↘ slice | 26.5–27.05 | The board is sliced into 5 diagonal bands that fly apart alternately | whoosh + shing |
| 6 Metrics | 27–34 (13.28–16.72 s) | Four isometric metric cards rise in a stagger. **Coverage** chart climbs to 85 % (b28–30.5). **Layers** shows 4 bars filling to 100 %. **Not placed** rolls 4→3→2→1→0 on b29, 29.5, 30, 30.5, then turns green ("all 17 districts placed"). The **Plan** badge flips Ready (green) → Pro (accent) on b31 (15.25 s), and the Ask box types "Which district has the most cases?". The cards tilt back from b32 and push through the camera, b32.9–34 | rising arpeggio, 12 × 16ths from 13.77 s; tick per counter step; flip swish + pop at 15.25 s; riser + whoosh into b34 |
| 7 Logo | 34–40.7 (16.72–20 s) | Radial accent beams bloom; the Peek mark (dark appearance) scales in on b34. On b35 (17.21 s) it slides left to reveal "NimbleMap" (Inter SemiBold) and │ 凌波微图. The tagline "Open a map file. See what it says." fades up on b36 (17.70 s), and the pill kangning-huang.github.io/NimbleMap pops on b37 (18.20 s). A sheen sweeps the wordmark on b38 (18.69 s). Fade to black 19.3–19.95 s | **sub-bass boom** + crash at 16.72 s; Dm(add9) pad; swoosh at 17.21 s; shimmer at 17.70 s; pop at 18.20 s; glassy shimmer at 18.69 s; audio fades with the picture |

Music: D minor, Dm–B♭–F–C per bar. The pad runs from 0 s. Kick on every beat from b4 to b33, off-beat bass pulses, 16th hats from b8 (open hats from b21), and claps on 2 and 4 from b9. `out/audio_cues.txt` lists every cue.

## Design decisions

- **⌘F, not ⌘K.** NimbleMap has no ⌘K command palette. ⌘F is its real shortcut: it focuses the search field (`crates/lm-ui/src/lib.rs`, `consume_key(COMMAND, Key::F)`), and the field's real hint is "Search places or your data" (`chrome.rs`). The menu is drawn as that search's results list.
- **Accent `#2563EB`, not `#6F9CF2`.** On near-black, `#2563EB` reads unmistakably as the brand blue. The film uses the accent mostly for large fills, glows and display type, where its contrast holds. `#6F9CF2` exists in the app for small text on dark, and next to the brand blue it reads as a pastel second colour. Small text in the film stays white or gray.
- **Logo.** The film uses the Peek mark's own dark appearance (`design/logo-nimble/variants/concept-8-peek-dark.svg`), drawn live from the SVG. Its viridis lens and coral dot are the mark's colours and are not re-tinted. They are the only exception to the five-colour palette. The wordmark is Inter SemiBold, with │ 凌波微图 in Noto Sans SC Medium, gray, as in the repo lockups.
- **Section 2 type.** The brief asks for "the one line on what it does", and the tagline is saved for the end card, so section 2 uses "Double-click any map file. / It lands on the map." It comes straight from the product description. The last words go accent → white.
- **Import steps.** Queued → Reading → Locating → Styling → On the map, for `districts.geojson`.
- **Metrics.** Coverage 85 % (heatwaves grid), Layers 4 of 4 loading, records not placed 4 → 0 (cases.csv joined to districts), Plan badge Ready → Pro with the Ask box. Haiyan's numbers ("99 % of 4,547 storms") come from the app's real card. Hagupit (2014), Haitang (2005) and Hai Phong appear only as search rows, as real names. D07's "88 % of 17 districts" is illustrative example data.
- **Timing.** Boundaries are snapped to beats (see above). The status icon starts appearing just before b9, so the frame after the blade is never empty.
- **Fades.** Transitions are a flash out of whiteout, a diagonal blade, a zoom into the check, a diagonal split, a depth push, a diagonal slice and a push through the camera. The only plain fade is the final fade to black, as the brief asks. Small text reveals inside sections, such as the tagline "fades up", use a rise plus opacity.
- **Easing.** Expo/quint out (and expo in-out for travel) throughout, with no back or elastic and no overshoot.
- **60 fps** throughout; it was practical (~5 min render).

## Possible improvements

- A proper Icon Composer–style layered logo animation: lens, raster and coral dot assembling separately, instead of scaling the whole mark.
- A real coastline (Natural Earth, simplified) under the storm tracks in the ⌘F scene. The tracks are procedural for now.
- Listen-through tuning of the mix on monitors. Levels were set by metering (RMS and LUFS per 0.5 s), not by ear.
