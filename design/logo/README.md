# LayeredMap logo concepts

Six app-icon directions for LayeredMap. The brief asked for a map, a sense that several layers can be opened and compared in space, and a flat, elegant Apple look. Open `index.html` to compare all six: each is shown large, at 128/64/32/16 px, on light and dark, in a mock Dock, and next to the "LayeredMap" wordmark in Inter SemiBold.

All icons are 1024×1024 SVGs. Each uses an 824 px macOS squircle body (a superellipse with n = 5) centred on the canvas, with flat fills or very soft gradients and no text. The colours are the app accent `#2563EB`, a viridis ramp for raster data, Okabe–Ito orange `#E69F00` (Fold only) and soft map neutrals.

## Concepts

| # | Name | File | Idea |
|---|------|------|------|
| 1 | Strata | `concept-1-strata.svg` | Three true-isometric (30°) map sheets stacked: raster, polygons, points. |
| 2 | Overlay | `concept-2-overlay.svg` | A buffer circle and a district polygon overlap. Only the intersection is accent blue, with one feature inside it. |
| 3 | Fold | `concept-3-fold.svg` | A tri-fold map. Each panel is a different layer (contours, districts, points) and one route runs across all three. |
| 4 | Strata Pin | `concept-4-strata-pin.svg` | A location pin cut into three bands (point / polygon / raster colours) on a quiet street grid. |
| 5 | Swipe | `concept-5-swipe.svg` | One map split by a swipe divider: base map on the left, the same geography as a choropleth on the right. |
| 6 | Peel | `concept-6-peel.svg` | A blue base-map sheet with its corner peeled back to show a raster layer underneath. |

`index.html` lists the strengths and weaknesses of each one.

## Files

- `concept-N-<name>.svg` are the six icon sources.
- `png/concept-N-<name>-{1024,128,32,16}.png` are renders of each concept, 24 PNGs in total.
- `index.html` is the self-contained comparison page. The SVGs are inlined with a unique ID prefix for each instance, and the only external load is Inter from Google Fonts.
- `src/gen.py` writes the SVGs. All geometry is computed there: the squircle, the isometric matrix `matrix(w, h, -w, h, cx, cy-h)` with h = w·tan 30°, the pin tangents and rounded tip, the reflection used for the peel flap, and the shear for the fold panels.
- `src/render.py` renders the PNGs and a review contact sheet (light and dark backgrounds, with 32/16 px blown up using nearest-neighbour).
- `src/page.py` builds `index.html` from the SVGs.
- Files named `._*` are macOS AppleDouble metadata that the exFAT LaCie volume creates automatically. They can be ignored or deleted.

## How the PNGs were rendered

None of the usual command-line renderers were on this Mac (`rsvg-convert`, ImageMagick and Inkscape were all missing), so I created a dedicated mamba environment. I did not change any existing environment.

```bash
~/miniforge3/bin/mamba create -y -n lm-logo -c conda-forge python=3.12 cairosvg pillow
~/miniforge3/bin/mamba run -n lm-logo python src/gen.py      # writes SVGs
~/miniforge3/bin/mamba run -n lm-logo python src/render.py   # writes png/*.png (CairoSVG, output_width = 1024/128/32/16)
python3 src/page.py                                          # writes index.html
```

The scripts use absolute paths to `/Volumes/LaCie/repos/LayeredMap/design/logo`. The SVGs deliberately avoid masks and filters because CairoSVG does not render them, so the PNGs match what browsers show. I checked every render at full size and at enlarged 32/16 px on both a light and a dark background.

## Small-size notes

- 16 px: Strata (a blue / white / green chevron stack), Peel (a blue tile with a light corner), Strata Pin and Overlay all stay recognisable. Swipe becomes a two-tone tile. Fold turns to mush.
- Dark mode: every icon sits on its own light or blue squircle, so the same artwork works on dark backgrounds and needs no separate variant.
- A production macOS icon set would normally get hand-simplified 16/32 px versions, for example Strata with the point markers removed.

## Recommendation

My pick is **1 Strata**. It states the app's model most directly (separate datasets stacked over one footprint), it is the clearest of the six at 16 and 32 px, and its blue / white / green order uses the app's own accent and viridis palette. Its weakness is that stacked sheets are a familiar "layers" glyph, so it is less distinctive as a brand.

The other options are just as valid, depending on what matters most:

- **2 Overlay** if the logo should express the *relationship* (overlap / intersection) rather than the layers. It is the most abstract and ownable.
- **6 Peel** if Dock presence matters most. It has the boldest silhouette and colour block.
- **5 Swipe** if the comparison workflow is the selling point.
- **4 Strata Pin** and **3 Fold** are the most literal "map app" readings. They are also the closest to generic map icons.

The owner makes the final choice.

## Strata variations

The owner chose concept 1, Strata. These five variations all keep the stacked isometric sheets (true 30° isometric, one shared footprint) and each changes a different thing. They are made by `src/gen_strata.py` and rendered by `src/render_strata.py`, using the same `lm-logo` environment as before.

Every `id` carries a per-file prefix (`sv1-`, `sv2-`, … and `sv1x32-` / `sv1x16-` for the small v1 artwork), so the SVGs can be inlined together. There are no masks, filters or external references.

| # | Name | File | What changes |
|---|------|------|--------------|
| v1 | Bold | `strata-v1-bold.svg` + `strata-v1-bold-32.svg`, `strata-v1-bold-16.svg` | Three larger, plainer sheets with a wider exploded gap. The only detail is one point and a faint road. It has **hand-simplified small-size artwork**: the 32 px version has thicker edges and a bigger point; the 16 px version is three flat rhombi (blue / outlined white / green) with no edges or content. `png/strata-v1-bold-{32,16}.png` are rendered from those files. |
| v2 | Link | `strata-v2-link.svg` | A vertical drop-line runs from one point on the top sheet through the polygon sheet to the matching cell of the raster, with a marker on each layer. This makes "same place, different data" explicit. |
| v3 | Blue | `strata-v3-blue.svg` | An accent-blue squircle with white sheets: an opaque top sheet with blue points, and translucent lower sheets with darker blue edges between them. It has the strongest Dock presence and matches the app accent. |
| v4 | Cutout | `strata-v4-cutout.svg` | Only the bottom sheet is a full square. The bottom is the raster, the middle is a road network on a faint plate, and the top is a single district shape, all cut to the same footprint. Each layer looks like its data type. |
| v5 | Night | `strata-v5-night.svg` | A dark navy squircle with the sheets fanned (0°, −9°, −18° in-plane rotation) like cards spread over one spot. The raster glows against the dark. |

Renders are in `png/strata-v*-{1024,128,32,16}.png`. I checked every one at full size and at enlarged 32/16 px on both light and dark backgrounds.

What I found at small sizes:

- At 16 px, v1 (hand-tuned), v2, v4 and v5 all keep a clear blue-over-green stack.
- v3 turns into a white blob on blue at 16 px and would need its own hand-made 16 px artwork if chosen.
- v2's drop-line and v4's road layer are only visible at 128 px and above. At small sizes they fall back to the plain stack, which is acceptable.
- v5 sits dark-on-dark in a dark Dock. A faint white rim keeps the shape visible.

### Recommendation (Strata variations)

My pick is **v2 Link**, with v1 Bold's simplified 32/16 px artwork used for small sizes. Of the five, v2 is the only one that shows the *spatial relationship* and not just the *layers*: one location, traced through every dataset, which is exactly what the app is for. At large sizes it still reads as the Strata icon the owner picked, and at small sizes it falls back cleanly to the bold stack. If a bolder Dock presence matters more than the story, v3 Blue is the alternative, but it needs a hand-made 16 px version. The owner makes the final choice.
