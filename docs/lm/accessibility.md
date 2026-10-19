# Accessibility: keyboard and screen readers

WP5.6 (keyboard and basic accessibility) and WP6.3 (screen readers). Status on 2026-10-18,
branch `wp6.3-screen-readers`. The code is `crates/lm-ui/src/a11y.rs`; the tests are
`crates/lm-ui/src/a11y/tests.rs`, which walk the AccessKit tree the way VoiceOver reads it.

## How it works

egui exposes every widget to AccessKit, which VoiceOver (macOS) and Narrator (Windows) read.
NimbleMap draws much of its chrome with the painter and uses Phosphor icon-font glyphs, so
egui alone gave many controls no name, or a private-use character for a name, which a screen
reader reads as nothing or as garbage. `a11y.rs` fills those gaps:

- **Names.** Every custom-drawn control and icon-only button gets a name and role
  (`a11y::button`, `toggle`, `choice`, `label`, `text`). Text drawn with the painter that
  carries meaning (result-card rankings, table rows, search results, find results) is exposed
  as text or as the control's name.
- **The map canvas.** One focusable node, role canvas ("map"), named in words: the visible
  layers and the extent ("Map. Showing districts. About 60 kilometres across, centred at 31.2
  degrees north, 121.5 degrees east."). Its description tells how to reach the records. While
  it has the keyboard, arrows pan (← → step time when a time layer shows), + and − zoom, Tab
  leaves.
- **The alternative to the canvas** (spec WP6.3): search (⌘F), Find in a layer, and the table
  (⌘T) list every record; choosing one selects it.
- **Selection summaries.** A polite live region reads each selection change:
  "Jiading selected. population 1,834,258, 5th highest of 16." The value is insight's first
  field: the field the layer is styled by, else the first starred or measure field, with the
  rank among the layer's records. Several records: "3 districts selected." A layer: "districts
  layer selected. 17 areas …". A raster cell: its layer, position, value and comparison.
  Cleared: "Selection cleared." Units are spoken in words ("°C" → "degrees Celsius").
- **Live regions** (polite): notices (the pill under the toolbar), the composer's preview
  summary and a kept result, the Ask box's answer, and the highlighted search result as the
  arrows move it.
- **Focus.** Tab and ⇧Tab follow toolbar → (welcome card) → shelf → map → map controls →
  inspector → table drawer. F6 / ⇧F6 jump between those regions. The toolbar buttons are made
  left to right so Tab meets them in reading order. A 2 px accent ring marks keyboard focus;
  a pointer press hides it again (like the web's `:focus-visible`); text fields keep their own
  focus outline. Return and Space press the focused control; Esc closes menus and popovers and
  then walks the Esc ladder.
- **The table by keyboard.** ⌘T opens the table with the keyboard on the selected record (or
  the first); ↑ ↓ Home End move through the rows, selecting as they go (the summary is read),
  and a row already in view doesn't scroll. Each row reads as its record's name and its first
  six "field: value" pairs.

## Checklist (automated)

| Check | Test |
|---|---|
| No control without a usable name on the main screens: first launch, Examples menu, search results, basemap chooser, layers and overview, object card, Style tab, Details tab, table and Find, a joined table, composer and result card, Pro with the Ask box and time slider, export sheet, data export, recovery prompt, missing source, data library, credits | `every_control_on_the_main_screens_has_a_name` (lists every offender) |
| Selection announcement text and rank | `selecting_an_object_announces_its_summary`, `the_summary_ranks_by_the_styled_field` |
| Keyboard only: Tab to Examples → World basics → ⌘F, type, Return → the summary is read → ⌘T → ↓ selects and reads the next row | `keyboard_only_open_an_example_select_and_hear_the_summary` |
| F6 reaches the map; arrows pan it and it keeps the keyboard | `f6_reaches_the_map_and_arrows_pan_it` |
| Notices are live regions | `notices_are_live_regions` |
| Units spoken in words; glyphs stripped from names | `units_are_spoken` |
| Focus-ring and accent contrast ≥ 3:1 | `theme::tests::every_token_passes_its_contrast_check` (existing) |

## Still needs a person

**VoiceOver pass (owner, Gate 6: "complete assignment 1 with VoiceOver and the screen curtain
on").** Nothing here was checked with VoiceOver running: the tests read the AccessKit tree
that the macOS adapter turns into NSAccessibility objects. In the session, listen for:

1. Live regions are actually spoken by VoiceOver (AccessKit's macOS adapter posts an
   announcement when a polite node appears or its value changes), not too chatty, and not
   cut off by the focus change that caused them.
2. The map canvas: is "Map. Showing …" read on focus, and is the description (VoiceOver's hint,
   after a pause) useful? Does VoiceOver's own cursor (VO-arrows) reach the panels' text, or
   only Tab?
3. The table: whether row names of six fields are too long; whether VoiceOver users expect a
   real table (rows × columns) instead of rows named as sentences.
4. Popups and menus (Examples, Share, the layer's right-click menu): are they announced as
   menus, and does Esc return focus to the button? A context menu has no keyboard key yet
   (no ⇧F10 / VO-⇧M); the same actions are in the Style tab, the table and the menu bar.
5. The export sheet: the legend can only be dragged with a pointer; its placement has Auto.
6. Drawing an area (WP4.4) has no keyboard path: points are placed with the pointer.
7. Reading order inside the inspector (insight rows, facts grid) and the Style tab's custom
   controls (segmented controls, palette tiles, sliders).
8. The pill notice's 4 s lifetime is fine for VoiceOver (announced once) but short for a
   sighted keyboard user.

**Narrator (Windows, with WP6.2).** AccessKit's Windows adapter (UI Automation) is already in
the build through eframe. Repeat the checklist with Narrator when the Windows build exists;
live regions map to UIA notifications there, which Narrator handles differently from
VoiceOver.

## Known gaps

- Labels' text can't be selected any more (a selectable label is a Tab stop); egui's scroll
  bars are still Tab stops, and a wrapped link's second line is a second stop for the same link.
- Names are also repaired after each frame (`a11y::strip_glyphs`): glyphs are stripped from
  names, decorative icon labels hidden, a pop-up or slider without a name takes the caption on
  its left, and egui's unnamed menu and window backgrounds become groups.

- The map's extent is described by coordinates, not a place name: there is no reverse
  geocoder (the GeoNames index searches names only). A nearest-place lookup would read better
  ("around Shanghai").
- Selection by pointer on the map is announced; there is no keyboard way to pick a record *on
  the map* (by design: search and the table are the alternative).
- Disabled toolbar buttons of later phases aren't in the tree (they can't take focus).
