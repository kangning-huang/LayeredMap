# iOS layers and symbology chrome

*Status: owner lean locked 2026-10-31. Details still refine. Not an ADR yet.
Companion to `docs/lm/ios-open-paths.md` (how data gets in). This page is how layers and
symbology sit on an iPhone once data is open.*

The iPhone companion spike (`spikes/ios-companion/`, PR #50) already draws on MapLibre with
floating Open / Ask / Layers chrome. Smoothness is good enough to keep that stack. This page
records the **A / B / C chrome flow** Ken locked so layer list and style never fight the map.

Product principles that matter here: the map is the home screen; feel is a feature; technical
words are findable, never required (`AGENTS.md`).

## The hard constraint

Principle 3: the map is the home screen. Idle chrome stays tiny. Editing chrome may grow, then
must go away. A person who finishes reordering or styling and returns to pan/zoom must land back
on a nearly full-screen map, not a shrunk GIS panel.

## Locked flow (Ken, 2026-10-31)

| State | What it is | How you reach it | How it ends |
|---|---|---|---|
| **A** Idle / default | Map-first. Floating glass pills only (Open, Ask, Layers). Map owns pan and zoom. | Resting state after launch, after dismiss, after pan resumes. | Tap Layers → B. |
| **B** Layers sheet | Bottom sheet: **data layers only** (see Basemap below). Visibility eye toggles. Standard iOS reorder. | Tap Layers on A. | Drag-down / Done / map pan → A. Tap a layer row → C. |
| **C** Symbology | Nested detail for that one layer: field, color ramp, classify, opacity (v1 scope below). Collapsible. | Tap a layer row in B. | Done / Back / finish setup → collapse toward B then A (or straight to A if the sheet stack dismisses). Pan resumes on A. |

**Summary Ken locked:** A is the default view. B and C show themselves as needed and collapse
back to A as soon as the user is done reordering layers and settling symbology and back to
panning the map.

### Basemap is not a layer (locked)

The basemap is **always on** in the background. It never appears as a row in B. There is no
Basemap / Base Map entry, no eye toggle for it, and no way to turn it off. Turning the basemap
off does not make sense on this product.

B lists **data layers only** (opened GeoJSON / CSV / catalog layers and later kept results). If
streets vs satellite (or other basemap appearance) is offered at all, that is a **separate
control** elsewhere (for example near Open or Settings), never a layer row and never competing
with data-layer reorder. This doc emphasizes data-layer chrome; basemap picker details stay out
of scope here.

## A. Idle map-first

Open, Ask, and Layers are floating glass controls. The map fills roughly the whole screen. Layers
never stays open while the person pans. Ask does not own the bottom sheet (see Ask vs Layers).

Empty or one-layer maps still use A; B is only a tap away when there is something to list.

## B. Layers bottom sheet

Grabber, title such as Layers, one row per **data** layer. Each row: leading **legend swatch**
(see below), visibility eye, layer name, and a chevron or disclosure into C. The sheet covers
about the lower third so the map above still reacts when eyes toggle.

### Legend swatch = current symbology (locked)

Ken, 2026-10-31: each layer's leading icon in B must show that layer's **current symbology as a
legend**, so the list matches what the person sees on the map. Do **not** use generic type
glyphs (building / tree / road icons, or a fixed geometry badge that ignores style).

Examples of honest swatches:

- Cities styled as blue points → a blue point circle
- Roads styled as gray lines → a short gray line sample
- Parks styled as green fills → a green fill square (or the fill color the style uses)

When the person changes symbology in C (color, ramp, opacity, classify), the B swatch updates to
match. Multi-class ramps may show a compact chip (mini ramp or dominant class) rather than every
break; the point is recognition, not a full on-map legend. Basemap still never appears in B, so
it never gets a swatch here.

### Reorder (important, locked)

Reorder uses the **same UI pattern as other iOS apps** for reordering list items. Familiar
references: Reminders, Settings, Music playlists. In SwiftUI terms that means Edit mode and
`onMove` (or the system drag handles), not a custom long-press invention and not a desktop-style
up/down button column.

Proposed v1: when there are two or more data layers, the sheet offers Edit (or always shows the
standard reorder handles in an editable list). Drag to change draw order. Top of the list is the
topmost drawn data layer (match whatever the desktop document already means by order; do not
invent a second order model for phone).

### Visibility

Eye toggles hide or show that data layer on the map immediately. They never affect the basemap.

### Dismiss

Drag the sheet down, tap Done if present, or resume map pan/zoom in a way that dismisses the
sheet. Any of those returns to A. Do not leave B open as a permanent dock.

## C. Collapsible symbology

Tap a layer row in B opens C: detailed symbology for that layer only. C is nested under B (push
or nested sheet), not a second permanent column.

### Collapse rule (locked)

C is collapsible. It hides as soon as the user finishes setting up that layer. Done / Back
collapses C; finishing and returning to pan collapses the whole stack to A. C must not remain
on screen as an always-on inspector.

### Live preview

While C is open, keep a map peek above the sheet so color and opacity changes show live. Prefer
auto style on first open of a layer; open C when the person taps Style or the row, not as a
forced wizard every time.

### Symbology scope for v1

**Must have in C (short list):**

- Attribute / field to color by (sensible default if the layer already has one)
- A small set of color themes / ramps (about three presets, not a full color picker lab)
- Classify: auto plus a very short preset list (for example equal interval / quantile); no dense
  GIS classify panel
- Opacity
- Done / Back

**Defer:**

- Full desktop classify UI, custom break editing, multi-field styles
- Label chrome, hatch fills, advanced point symbols
- Per-class legend editor beyond what the ramp implies
- Anything that needs a permanent side inspector

## Ask vs Layers

Both want attention near the bottom. Locked lean: **Ask stays a top or edge pill** (as on A).
**Layers owns the bottom sheet.** They must not fight for the same sheet stack. Opening Ask does
not present the layers list; opening Layers does not expand Ask into the sheet.

## Legend (open detail)

**In B:** locked — each data-layer row already carries a live symbology swatch (above). That is
the list-as-legend.

**On the map (A):** not fully locked. Lean from the debate: a thin legend chip only while C (or
active styling) is open, then hide so idle A stays clear. Alternative: no on-map chip at all,
rely on B's swatches. Flag for a later mockup pass; do not ship an always-on on-map legend card
that blocks the map.

## Emphasize / keep / ignore

| Item | Call | Why |
|---|---|---|
| A idle floating chrome | **Emphasize** | Map is home |
| B bottom sheet, data layers only | **Emphasize** | Matches Ken lock; basemap excluded |
| iOS-native reorder in B | **Emphasize** | Owner: same as other iOS apps |
| C nested, collapsible symbology | **Emphasize** | Show when needed, hide when done |
| Collapse B/C → A on done / pan | **Emphasize** | Resting state is always A |
| Eye toggles on data layers | **Keep** | Fast show/hide without opening C |
| B leading swatch = live symbology | **Emphasize** | Owner: list rows match the map; not type glyphs |
| Short on-map legend while styling | **Keep** (lean) | Open detail; not locked; B swatches already locked |
| Ask as non-bottom-sheet pill | **Keep** | Avoid chrome collision |
| Basemap as a layer row / eye off | **Ignore** | Locked: basemap always on, never listed |
| Persistent side drawer | **Ignore** | Mockup D anti-pattern |
| Always-on inspector / desktop ribbon | **Ignore** | Blocks the map |
| Bottom tab bar Map / Layers / Identify | **Ignore** | Desktop GIS shrunk to a phone |

## Out of scope here

How files open (see `ios-open-paths.md`). iPad denser chrome. Exact SwiftUI sheet detent heights.
Basemap appearance picker UI. App Store copy. Merging the spike into product crates.

## Open questions (refine, not reopen the flow)

- Exact Edit vs always-visible reorder handles when there are many layers
- Whether dismissing C returns to B or straight to A when the person taps the map
- On-map legend chip vs relying only on B swatches (B swatches themselves are locked; see above)
- Compact multi-class swatch shape in B (mini ramp vs single chip)
- Where a future basemap appearance control lives (not in B)
- How many classify presets ship before "Advanced…"
