# NimbleMap icon concepts

These are nine app-icon directions for the rename from LayeredMap to **NimbleMap** / **千层图**. The Chinese name means "thousand-layer map", after 千层酥 (mille-feuille). The current Strata icon (stacked isometric sheets) was made to say "layered". These concepts aim to say *nimble, light, quick, deft, curious* and still read clearly as a map app, in a flat, elegant Apple style. Because the Chinese name is about layers, each concept also has a "千层图 fit" rating (see below), and concept 9 was added to bridge both ideas.

Open `index.html` to compare them. Each concept is shown:

- large, on light and dark backgrounds
- at 128/64/32/16 px (the 32 and 16 px sizes use hand-simplified artwork)
- in its default, dark and tinted appearances
- in a mock Dock (light and dark), with the current Strata icon for reference
- beside the "NimbleMap | 千层图" wordmark
- with notes on strengths, weaknesses, possible resemblance to existing logos, and how well it fits 千层图

The page is fully offline. The SVGs are inlined, and the wordmark fonts are embedded as small base64 WOFF2 subsets of the repo's bundled OFL fonts. There are no network requests.

Nothing outside `design/logo-nimble/` was changed. The shipped icon, `assets/icon` and the packaging are untouched.

## Concepts

| # | Name | Direction | Files | Idea |
|---|------|-----------|-------|------|
| 1 | Dart | motion | `concept-1-dart.svg` (+`-32`, `-16`) | A paper dart folded from a map sheet, with a river and a park printed on the wing. It sits at the end of a dotted route, flying over a coast. |
| 2 | Feather | lightness | `concept-2-feather.svg` (+`-32`, `-16`) | A feather whose two vanes are sea and land. The shaft is the coastline, and a coral dot marks a place. |
| 3 | Lift | lightness | `concept-3-lift.svg` (+`-32`, `-16`) | A single isometric map sheet, with a pin on it, lifting off the ground and leaving its shadow behind. |
| 4 | Hover | creature | `concept-4-hover.svg` (+`-32`, `-16`) | A geometric hummingbird hovering over a quiet map, its beak pointing at one spot ("click anything to see where it stands"). |
| 5 | Hop | quick pin | `concept-5-hop.svg` (+`-32`, `-16`) | A coral pin leaning into a hop. A dotted arc runs from the last place, and the pin is about to land in a ring. |
| 6 | Wings | folded map | `concept-6-wings.svg` (+`-32`, `-16`) | A two-panel paper map opened like wings and banking like a glider. One panel is sea, the other streets, and one route crosses the fold. |
| 7 | Route N (v2) | abstract N | `concept-7-route-n.svg` (+`-32`, `-16`); v1 in `v1/` | A map first (land, lake, hill with layered contour tints, street grid). The N is a major road in it, with a teal route drawn along it from a navy start pin to a coral arrival dot. **Owner's #3.** |
| 8 | Peek (v2, **chosen**) | lens | `concept-8-peek.svg` (+`-32`, `-16`), `peek-site.svg`; v1 in `v1/` | A navy magnifier (ring + handle) over a calm street map. Inside is a stepped viridis raster patch with the coral clicked feature off-centre. **Owner's #1, the icon for now.** |
| 9 | Riffle (v2) | nimble + layers | `concept-9-riffle.svg` (+`-32`, `-16`); v1 in `v1/` | Three big thin map sheets (raster, polygons, streets with the coral place) fanned from the bottom-left corner, filling the squircle. **Owner's #2.** |

The palette is shared across all nine. It uses the app accent `#2563EB` and one "place" accent, coral `#FF5A3C`, with soft map neutrals (sea `#BFDBFE`, land `#D3EEDB`) and viridis only where a raster is meant (Peek, Riffle).

## Round 3: owner's ranking and revisions

The owner ranked **1 Peek, 2 Riffle, 3 Route N**. **Peek is the icon for now.** The previous versions of these three are kept as v1 in `v1/` (master, `-32`, `-16`, `-dark`; PNGs in `png/v1/`). The page shows a v1 → v2 row in each of their cards. The root `concept-7/8/9-*.svg` files are now v2.

**8 Peek v2 (production polish).**
- The ring and handle are one solid colour: navy `#1E3A8A` on light, `#E2E8F0` in the dark variant. The handle sits at 45° and makes it a magnifier rather than a rifle scope. There is no crosshair, and the clicked feature is off-centre.
- The inside is a stepped 4 × 4 viridis raster (crisp cells rather than a radial blob, so it no longer reads as a globe). One magnified white street runs below the centre, and the coral feature sits upper right. A single flat glint arc marks the glass; there is no gradient or blur.
- Shimmer clean-up: the outside map is down to four 34 px streets plus a water corner (2.1 px at 64), and the thin 6 px lens outline and the 7 px boundary lines are gone. The thinnest line left in the master is the 22 px glint, 1.4 px at 64.
- `concept-8-peek-32.svg` and `-16.svg` are hand-drawn as described in the small-size table: bold ring, handle, a two-colour patch at 16, and the coral feature. Checked enlarged on light and dark.
- `peek-site.svg` is for the website header at about 30 px. It uses the same 1024 viewBox and squircle format as the current `web/lm/assets/logo.svg`, with the 32 px drawing made slightly bolder (ring 80, handle 124, feature r 56). `png/peek-site-{30,60,90,256}.png` and `png/review/peek-site-check.png` show it on light and dark. I did not edit `web/lm`; copy the file over `web/lm/assets/logo.svg` when ready.

**9 Riffle v2** ("the layers are too small"). The sheets are now 520 px (540 px in the small art), up from 330. They fan from a shared bottom-left pivot (−22°, −11°, 0°) and together span about 145–860 px horizontally, filling most of the squircle. The front map is legible: sea, park, two streets, a dotted route and the coral place. The middle sheet's polygons and the back sheet's 6 × 6 raster are placed in the wedges that stay visible. The sheets are flat and fanned, still not Nimble Storage's stacked green isometric plates.

**7 Route N v2** ("only the N stands out"). The map now carries real weight: a lake with a shoreline, a hill with three contour tints and 10 px contour lines, and a 24 px street grid. The N is a major road in that map (pale fill, darker casing) with a 40 px teal route along it. It starts at a navy start pin and ends at the coral arrival dot. It reads as "a map with a route" first and "N" second. At 16 px the road itself turns teal so the N survives.

## Files

- `concept-N-<name>.svg`: the 1024 × 1024 master. It has an 824 px macOS squircle (superellipse n = 5) and no text, masks or filters.
- `concept-N-<name>-32.svg`, `-16.svg`: hand-simplified small artwork on the same canvas, rendered at 32 and 16 px. The changes are thicker strokes, enlarged key shapes, and dropped textures, small roads and labels. The changes for each concept are listed below.
- `variants/concept-N-<name>-dark.svg`: the dark appearance, with dark backgrounds and the same foreground.
- `variants/concept-N-<name>-tinted.svg`: the tinted (mono) appearance, which is the luminance of the dark variant. The system applies the tint colour.
- `lockups/concept-N-<name>-lockup-{light,dark}.svg`: the icon with "NimbleMap" in Inter SemiBold and "千层图" in Noto Sans SC Medium. The text is converted to outlines, so the files need no installed font.
- `png/`:
  - `concept-N-<name>-{1024,128,64,32,16}.png`; the 32 and 16 px renders come from the simplified files
  - `-dark-256.png` and `-tinted-256.png`
  - `-lockup-{light,dark}.png`
  - `png/review/contact.png` and `row-*.png`, the review sheets: light half, dark half, then the dark and tinted variants. They show 32 px blown up ×4 and 16 px ×8 with nearest-neighbour scaling.
- `v1/`: the previous versions of Route N, Peek and Riffle (`concept-N-<name>-v1{,-32,-16,-dark}.svg`), with renders in `png/v1/`.
- `peek-site.svg`: the Peek website header mark for about 30 px (replaces `web/lm/assets/logo.svg`).
- `fonts/`: the WOFF2 subsets used by the page, `Inter-SemiBold-NimbleMap.woff2` (3.8 KB) and `NotoSansSC-Medium-QianCengTu.woff2` (2.0 KB, glyphs 千层图), plus their OFL licence texts. The sources are `assets/fonts/Inter-SemiBold.ttf` and `assets/fonts/NotoSansSC-Medium.otf`.
- `src/gen.py`: writes all icon SVGs (master, 32, 16, dark, tinted). All geometry is computed here:
  - the squircle
  - pin tangents
  - the feather outline, from a Bézier spine with a width profile
  - the isometric sheet, using the same 30° matrix as Strata, plus an in-plane tilt
  - the affine panel matrices for Wings
- `src/lockup.py`: subsets the fonts and writes the outlined lockups.
- `src/render.py`: renders the PNGs and the review sheets.
- `src/page.py`: builds `index.html`. It also inlines the current Strata icon from `../logo/strata-raster*.svg` for reference.

Every SVG's ids carry a per-file prefix (`n4-`, `n4x32-`, `n4d-`, …), and `page.py` re-prefixes each inline copy, so any number of them can share one page.

## How to regenerate

```bash
cd /Volumes/LaCie/repos/LayeredMap/design/logo-nimble
~/miniforge3/bin/mamba run -n lm-logo       python src/gen.py      # SVGs (cairosvg env from the previous round)
~/miniforge3/bin/mamba run -n lm-logo-fonts python src/lockup.py   # font subsets + outlined lockups
~/miniforge3/bin/mamba run -n lm-logo       python src/render.py   # png/ and png/review/
python3 src/page.py                                                # index.html (standard library only)
```

`lm-logo-fonts` is a new environment I created for this round. The existing `lm-logo` environment is unchanged.

```bash
~/miniforge3/bin/mamba create -y -n lm-logo-fonts -c conda-forge python=3.12 fonttools brotli brotli-python
```

## Small-size notes

I checked every concept at 1024, 128, 64, 32 and 16 px on light and dark backgrounds, and also enlarged the 32 and 16 px renders on light and dark.

| # | 32 px | 16 px | Simplifications in `-32` / `-16` |
|---|-------|-------|----------------------------------|
| 1 Dart | clear plane + route | pale on light; the plane still reads, and the dark variant is much stronger | larger plane; the wing print is reduced to one river (32) or removed (16); the dotted route becomes a solid coral stroke (16) |
| 2 Feather | clear | clean diagonal blade (blue/green) | no barbs or notch; thicker shaft; larger dot (32); no dot (16) |
| 3 Lift | readable sheet + pin | weakest: a white rhombus with a coral fleck | larger sheet; stronger blue sea; darker shadow |
| 4 Hover | clear bird + target | bird silhouette survives | thicker body and beak; no eye or street blocks; no target at 16 |
| 5 Hop | clear | coral pin + blue arc read well | no streets or park; the dotted arc becomes solid at 16; bigger pin |
| 6 Wings | clear | blue tile with a white V; the map content is gone | larger panels; plain panels; thicker route |
| 7 Route N v2 | road + route, pin, coral dot, lake and hill all read | teal N road, lake, hill, coral dot (no pin) | 2 contour tints, no contour lines; one street pair; wider road and route; bigger pin. At 16 the road itself is teal (a thin route line would vanish) |
| 8 Peek v2 | ring, handle, 3 × 3 raster patch, coral feature | bold navy ring + handle, two-colour inside (teal + lime), coral feature | outside streets dropped (32 keeps the water corner, 16 is plain); thicker ring and handle; 3 × 3 cells (32) or two flat colours (16); no glint or street inside the lens |
| 9 Riffle v2 | clear fan: raster and polygon wedges behind a big front map | white front sheet with a coral dot, a two-tone raster edge and a blue polygon edge; the fan is still visible | wider fan; 3 × 3 raster (32) or 2 × 2 (16); polygon lines dropped at 16; front map reduced to a sea band and one street (32) or a sea band (16) |

Apple guidance applied (HIG, App icons):

- one centred idea per icon with clearly defined edges
- no text, and no UI replicas
- no custom shadows, blurs or glows (Lift's ground shadow and Peek's lens edge are flat shapes)
- dark and tinted appearances that keep the same features

For Icon Composer, each concept splits naturally into a background layer (the sky, map or tile gradient) and one or two foreground groups (the hero object and the coral accent). The squircle in these files is for preview only; production layers should be full-bleed squares that the system masks.

## Distinctiveness check

These are the logos I checked against: Apple Maps, Google Maps, Amap/高德, Baidu Maps, Mapbox, Esri/ArcGIS, QGIS, Felt, Kepler.gl, Carto, Strava, AllTrails, Telegram, Twitter/X, Nimble CRM and HPE Nimble Storage. I looked up the Nimble CRM and Nimble Storage marks online for this check. The others are well known.

- **Nimble CRM** uses a white star-figure on a blue tile with **yellow ghosted copies trailing behind it** (a motion trail). I avoided ghost or echo trails on purpose; motion is shown with one dotted arc (Hop, Dart).
- **HPE Nimble Storage** uses **green stacked isometric layers** beside the word "nimble". The current **Strata** icon, with its blue/white/green stacked sheets, paired with the name "Nimble", comes close to this. That is a concrete reason to change the icon along with the name. Lift's single sheet avoids it, but don't add more sheets.
- **1 Dart** carries the highest risk. Paper planes are Telegram's mark (white plane on a blue circle) and the generic "send" glyph, and Amap's white arrow mark has a similar dart silhouette. The printed map, the coastline and the pale sky set it apart, but the silhouette is shared.
- **5 Hop**: pins are shared with Google Maps, Baidu Maps and many others. The coral pin is generic rather than a copy, but it is the least ownable shape.
- **3 Lift**: a pin on a map sheet is the generic map-app picture. It doesn't match any one brand.
- **7 Route N**: green tiles also appear in AllTrails and QGIS, but neither uses an N or a route letterform. If it is chosen, check the teal against AllTrails' green.
- **4 Hover**: bird logos are common. Twitter's bird is round and rising, and Apple's Swift logo is a swift on orange. A geometric hummingbird in blue over a pale map is clearly different from both.
- **9 Riffle**: a fan of cards can recall wallet or card-stack icons. It is deliberately *not* Nimble Storage: the sheets are flat (not isometric), fanned (not stacked) and three different layers (not a green plate stack). Because they are distinct layers rather than fading copies, it is not Nimble CRM's motion trail either.
- **2 Feather**, **6 Wings** and **8 Peek** are not close to any listed logo. Their only overlaps are generic: quill or eco apps, open-book or reader icons, and search or loupe icons.

## Fit with 千层图 (nimble and layers)

The English name says *nimble* and the Chinese name says *layers*. The best icon carries both, or at least does not contradict the Chinese name.

| # | 千层图 fit | Why |
|---|-----------|-----|
| 1 Dart | weak | One folded sheet; nothing about layers. |
| 2 Feather | medium potential, weak as drawn | Plumage is naturally many thin overlapping layers. A redraw with two or three stacked translucent vanes would bridge both names; the current two-vane drawing does not. |
| 3 Lift | medium | One sheet lifting off implies a stack below and keeps the Strata heritage. Adding visible sheets beneath would drift toward Nimble Storage, so keep it to one. |
| 4 Hover | weak | The map is a single plane under the bird. The story is nimble, not layers. |
| 5 Hop | weak | A pin and a ground plane. |
| 6 Wings | medium-good | Two panels with different content are already two sheets. A third accordion panel would make it a light, flying stack of map layers. |
| 7 Route N | none | A Latin initial; it fits NimbleMap only. |
| 8 Peek | good | The lens shows a second data layer under the street map: looking through the layers. |
| 9 Riffle | strong | Designed for this: a few thin, different layers flicked open in one quick move. |
| Strata (current) | strong, but | It literally says layers, but next to the word "Nimble" it comes close to HPE Nimble Storage's green stacked isometric layers. |

## Recommendation

**Decided: 8 Peek** (owner's #1), with Riffle v2 and Route N v2 as the alternates.

Before Peek ships, I suggest:
1. Build the Icon Composer layers: the background is the map (gradient, water, streets), and the foreground groups are the raster patch, the ring and handle, and the coral feature.
2. Check the dark and tinted appearances on a device. The tinted variant here is a luminance approximation.
3. Copy `peek-site.svg` to `web/lm/assets/logo.svg`.

The earlier recommendations (Riffle first after the 千层图 rename, Hover before that) are superseded by the owner's choice.
