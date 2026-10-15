# LayeredMap Phase 0 design mock (WP0.7)

A clickable HTML mock of first launch, the loaded workspace, the object card with its insight strip, and the layer overview. It is built from `docs/lm/spec.md`: *Visual design system* and *Interaction specification*, plus *Assumptions and questions* and *Insight cards and suggested actions*. It exists for the WP0.7 hallway test: three students, 10 minutes each, one task.

- **Open:** `index.html`. It is one self-contained file with no network access, no CDN and no embedded fonts. It uses the Inter font if installed, otherwise the system font (PingFang SC / Noto Sans SC for Chinese). Tested in Chrome 1280 × 800. The minimum window is 1024 × 640, as in the spec.
- **Data:** the real fixtures. `fixtures/V1/districts.geojson` has 17 districts, drawn from their own polygons, including Jing'an's hole (Huangpu) and Chongming's two parts. `fixtures/T1/schools.csv` has 248 records, 242 of them located. The CSV is inlined verbatim and parsed in the page.
- **Synthetic field (not in the fixture):** `july_temp_c`, "July mean surface temperature (°C)". `src/build.py` adds it to each district: `35.8 − 0.165 × (km from lon 121.44, lat 31.26) + a small fixed per-district offset`, rounded to 0.1 °C. The values run from 28.9 to 35.8 °C. Huangpu (35.8 °C) is the unique hottest, just ahead of Jing'an (35.5 °C), which surrounds it. These are made-up values for the test and have no physical meaning.
- **Numbers on screen:** the page's JS computes all of them from the inlined data. That covers ranks, ties, medians, ranges, histograms, counts in areas, the point-in-district test and extents. Areas come from the file's `area_km2` field.
- **Rebuild after editing:** edit `src/template.html`, then run `python3 -I design/phase-0-mock/src/build.py`. Never edit `index.html` by hand.

## Screens and the spec subsections they implement

| # | Screen / state | How to reach it by clicking | Test URL hash | Spec subsections |
|---|---|---|---|---|
| 1 | First launch: a regional basemap with the welcome card at bottom center, "Open a file to see your data on the map." Buttons: **Open file…** and **Try an example**. | Open the file | `#launch` | Screen states (First launch); Layout; Selection → suggested actions row 1 |
| 1b | Simulated file dialog (districts.geojson + schools.csv, both ticked) | Open file… / Open / ⌘O / "+ Open file" | — | Toolbar (Open) |
| 2 | Importing: shelf rows show a spinner and "Reading 3.2 of 5.2 KB". The map stays interactive. | Try an example, or Open | `#importing` (frozen) | Screen states (Importing); Layer shelf (spinner badge) |
| 3 | Loaded workspace: 52 px toolbar, 280 px floating shelf, eye / swatch / name / badge rows (Schools has the purple assumption dot), "+ Open file" row, 36 × 36 map controls, scale bar, attribution. The camera flies to the data. | after import | `#workspace` | Layout; Toolbar; Layer shelf; Default symbols; Motion |
| 4 | Object card for a district: 3 px accent outline plus 2 px halo, then the 360 px inspector. It has a header, an insight strip (3 measure rows with rank phrase, 120 px dot strip and median line), Facts (up to 6 fields, then "Show all 6 fields"), row-7 actions, More actions…, and the footer "View in table · Zoom to". | click a district | `#district=D11` | Inspector: object card; Insight rows; Selection → suggested actions row 7 |
| 5 | Compare with other Districts: the insight strip expands into a full ranking of all 17 districts. The field token switches between Population, Area and July surface temperature. Click a row to select that district. | Compare with other Districts | `#district=D17&compare=july_temp_c` | "What the attribute-only actions do"; Insight rows |
| 6 | Missing value: Zhabei's population shows "No value", and the others say "of 16 with values". | click Zhabei | `#district=D08` | Insight rows (Missing values) |
| 7 | Point card for a school, with row-3 actions. "Which District is it in?" answers inline. | click a school dot | `#school=1&inline=which` | Inspector: object card; Selection → suggested actions row 3 |
| 8 | Layer overview, schools: Overview · Style · Details tabs; "248 points · schools.csv"; the purple assumption line with Change; the partial-import line "248 records opened; 6 have no usable location. Review 6"; legend + Style button; field summary with a 20-bin histogram; row-5 actions | click the Schools row | `#layer=schools` | Inspector: layer overview; Assumption catalog row 1; Error and status copy (Partial import) |
| 9 | Details tab: source path (read-only), format, record counts, coordinate system, extent, fields with role chips and evidence, and assumptions. The purple shelf dot opens it. | Details tab, or the purple dot | `#layer=schools&tab=details` | Inspector: layer overview (Details); Layer shelf badges |
| 10 | Color by a value: the sequential Heat palette (YlOrRd) is offered first for °C. The legend has units and a hatched "No data" entry. The shelf shows a compact legend. Undo names the step. | Districts row → Color by a value → July surface temperature | `#layer=districts&color=july_temp_c` | Data palettes; Toolbar (Undo); Selection → suggested actions row 10 |
| 11 | More actions… popover, grouped Understand / Compare / Measure / Change look / Keep | More actions… | `#district=D11&popover=more` | Relevance rules step 3 |
| 12 | Result card "Schools in each district" (top 5, Show all 17, context lines, "How this was calculated" with Copy). The composer and preview steps are skipped. | Districts row → Count Schools in each District | `#count` | Result card (partial) |
| 13 | Rank Districts by a value (result card) | Districts row → Rank Districts by a value | `#rank=july_temp_c&all` | Selection → suggested actions row 10 |
| 14 | Dark theme (moon button in the toolbar; mock only) | toolbar moon | add `&dark` to any hash | Color tokens (dark column); Elevation |
| 15 | Style tab: one color or by a value, a palette picker with Heat first for °C, and a legend | Style tab / Style button | `#layer=districts&tab=style&color=july_temp_c` | Data palettes |

The page also implements these behaviors:

- **Map interactions:** drag to pan with inertia. A mouse wheel zooms 0.5 levels per notch around the pointer. Pinch zooms and two-finger scroll pans. Double-click zooms in, and ⌥ double-click zooms out. The + / − controls and keys animate over 250 ms. Arrow keys pan a quarter view.
- **Shortcuts:** ⌘0 zooms to all data, ⌘F focuses search, ⌘L toggles the shelf, ⌘O opens files and ⌘Z undoes.
- **Esc ladder:** Esc closes a popover, then closes the comparison or inline result, then clears the selection, then closes the inspector. After the selection is cleared, the active layer's overview stays open.
- **Clicking empty map** clears the object selection.
- **Search** ("Search places or your data") lists "Your data" (district and school names) and "Places" (a small bundled list), then the "Find an action…" row.
- **Hiding a layer** with the eye switches the suggested actions to the "none" rows (6, 11, 2, 4). Hidden layers are never used as companions.
- **Narrow windows:** below 1180 px the shelf collapses, and the Layers button opens it.
- **Reduced motion:** with `prefers-reduced-motion`, panels appear without sliding and flights become jumps with a 100 ms fade.
- **Panel motion:** panels open in 180 ms and close in 140 ms, ease-out. Card content crossfades in 120 ms.

Anything not in the mock shows a one-line notice, "… isn’t part of this mock." Examples are the table, filters, distance areas and export.

### Screenshots (`screens/`, 1280 × 800 unless noted)

`01-first-launch` · `02-importing` · `03-workspace` · `04-object-card-xuhui` · `05-compare-temperature-huangpu` · `06-missing-value-zhabei` · `07-point-card-which-district` · `08-layer-overview-schools` · `09-layer-details-schools` · `10-color-by-heat-districts` · `11-more-actions-popover` · `12-count-result` · `13-dark-object-card` · `14-whats-inside-jingan` · `15-rank-result-temperature` · `16-narrow-1100px` (1100 × 720) · `17-style-tab-districts`

### Verification built into the page

- `#…&selftest` (add it to any state) writes `SELFTEST PASS 167 checks` into the page title. The same result is stored in the `data-selfcheck` attribute on `<html>`. It runs these checks:
  - The population rank phrases and median match `fixtures/V1/expected_insight.csv`. The mock adds " with values" when values are missing, and the check strips it before comparing.
  - School counts per district match `fixtures/T1/expected_count_in_areas.csv` (Chongming's channel point counts in none).
  - Schools: 242 located and 6 excluded.
  - The hottest district is unique.
  - No forbidden insight words (significant, cause, because, explains, better, worse) and no "!" appear on any district card or the first 40 school cards.
- `#e2e` drives the real UI with synthetic clicks and key presses. It covers the welcome card, the import, a click on Xuhui, Compare, switching the field to temperature (Huangpu, 35.8 °C, on top), Esc ×4 against the ladder, the schools overview copy, the school card and "Which District", an empty-map click, Color by temperature (Heat palette, legend, Undo) and the count result. It writes `E2E PASS 27 checks` into the title.
- Uncaught JS errors set `data-jserror` on `<html>`. In headless Chrome, every hash state listed above reported none.

Command used (headless Chrome, no installs):
`"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --virtual-time-budget=9000 --dump-dom "file://$PWD/index.html#e2e" | grep -o '<title>[^<]*</title>'`

## Hallway-test script (WP0.7)

**Setup (per student, about 2 min)**

- Use a laptop at 1280 × 800 or larger with a mouse or trackpad. Turn off notifications.
- Open `index.html#launch` in a fresh tab. Use the light theme.
- Have a timer and a notes sheet ready. If you can, record the screen and audio with the student's consent.
- The observer log is a hidden timestamped list of clicks, selections, searches, actions and Esc presses. **Ctrl+Shift+L** toggles it. Copy it after each session, then click "Reset session", which reloads to `#launch`. Students should not see the log.

**Briefing (read it word for word)**
> "This is an early mock of a map app, not finished software, and we're testing the app, not you. Please think aloud: say what you're looking for and what you expect to happen. I can't help you during the task. Your task: **find the hottest district, and say how you know.** Tell me when you're done. You have up to 10 minutes."

Do not mention example data, layers, temperature fields or the Compare action.

**Record for each student**

| Measure | How |
|---|---|
| Success | Correct = 黄浦区 Huangpu, 35.8 °C. Note whether they say a reason ("1st highest of 17", the ranking chart, darkest red). Jing'an (35.5 °C) is the expected near-miss, because Huangpu is the small square inside it. |
| Time to first data on map | Log: from `session start` to `import done` |
| Time to answer | Log: from `session start` to the spoken answer |
| Path taken | Which route did they use: clicking districts one by one, Compare with other Districts, Color by a value, Rank Districts by a value, or search? Note the first click on the loaded map. |
| Hesitations over 5 s | Where did they stop, and what did they say? |
| Misreads | Did they confuse "1st highest" or "tied … lowest"? Was the dot strip or median line read wrongly? Did they read population as temperature? |
| Purple dot | Did they notice it, and what did they think it meant? (Ask after the task.) |
| Esc / closing | Did they try Esc or the ×, and were they surprised by what closed? |
| Dead ends | "Isn't part of this mock" notices hit (they are in the log) |
| Words they used | Their own words for "district", "rank", "temperature" and "compare". Collect them as candidate search aliases. |

**Debrief (3 min, after the task)**

1. "How sure are you of your answer? What told you?"
2. Point to the purple dot: "What do you think this means?"
3. Point to the insight strip: "What does this line say about this district?" Probe the dot strip and median line.
4. "Was anything surprising or confusing?"
5. Rate 1–5: "How easy was it to find the answer?"

**Pass signal for the design track (suggested; the owner decides):** at least 2 of 3 students give the correct district with a valid reason within 10 minutes, and no one misreads a rank phrase. File every misread or hesitation of more than 30 s as an issue against the spec subsection it touches.

## Ambiguities found while building (questions for the owner)

These are the main output of the design track. Each entry says what the mock does now so the decision can be made against something concrete.

**Object card and insight strip**

1. **Header with two Name fields.** The spec says to use "the first text field whose role is Name". For V1 that gives "徐汇区" only. The mock shows "徐汇区 Xuhui" (name_zh + name_en), as the brief asked. Should bilingual name pairs be joined? If so, what is the rule?
2. **Subtitle size precision and source.** The spec example is "23.4 km²" (one decimal), but the file holds 237.93 (two decimals). The spec also says "measured geodesically". If the file has its own area field and the computed area differs, which one is shown? And what is the "size" of a point? The mock shows "Schools · point".
3. **Rank phrase for the extremes.** The reference (`reference/rank_check.py`) gives "1st highest of 17" and "1st lowest of 17". Should the extremes read "Highest of 17" / "Lowest of 17"? Students may find "1st lowest" odd.
4. **"of N" with ties.** The spec example "tied 3rd highest" has no "of N", but the reference adds it ("tied 8th highest of 16"). The mock follows the reference.
5. **Missing-value denominator.** The spec says "of 16 with values", but `expected_insight.csv` says "of 16". The mock follows the spec, and the self-check strips the suffix. One of the two documents needs updating so the Gate 3 snapshot tests have a single truth.
6. **Median line wording.** The spec gives only "+2.1 °C above the median". The mock uses "−0.9 °C below the median" (with a sign) and "At the median" when the rounded difference is zero. Fields without a unit read "+693,386 above the median". Should the line say "people", and where would that noun come from?
7. **Units and labels for vector fields.** The Assumption catalog infers units only for rasters. The mock reads °C from the `_c` ending and km² from `_km2`. It also shows aliases ("July surface temperature", "Area", "Name (Chinese)") rather than raw names (`july_temp_c`). Does the app infer units from field names? Does it show aliases, and from where? Is it a visible assumption?
8. **Category rows versus "only Measure fields qualify".** The Insight rows section says only Measure fields qualify, yet it also defines a "Category row". The mock shows the Type category row on school cards as the second of up to three rows. When should category rows appear, and do they count toward the three?
9. **Facts versus insight.** Should Facts repeat fields already shown in the insight strip? The mock repeats them. Are coordinate columns (lon/lat) facts? The mock hides them until "Show all 5 fields".
10. **Denominator for points without a location.** The spec says the layer subtitle is "248 points · schools.csv", but only 242 points exist on the map. School ranks in the mock say "of 248", because all records have `students`. Should ranks and the subtitle count records or located points?
11. **Default field for Compare.** "Compare with other [Districts]" expands "for one field" but doesn't say which. The mock uses the first insight row, which is Population unless the layer is colored by a value. For the hallway task this means a student must switch the token to temperature. Should Compare open on the row the user last looked at? Should clicking an insight row open Compare for that field? (Not built.)
12. **Bars for interval data.** The Result card says "horizontal bars". Bars from zero hide a 28.9–35.8 °C spread, and bars from the minimum mislead. The mock draws a dot-on-a-scale ranking for °C and zero-based bars for counts and other ratio data. Is that the rule?

**Selection, Esc and panels**

13. **Clicking empty map.** The Screen states table says "Object selected → Exit: click empty map", which leads to *Loaded* (inspector closed). The Map gestures table says "clear the object selection (the active layer stays)", and the Esc ladder clears the selection before closing the inspector, so the inspector stays. The mock keeps the active layer's overview open. Which is right?
14. **Is the expanded comparison a "composer" step in the Esc ladder?** The mock treats it, and the inline results of What's inside? / Measure / Which District, as step 4: Esc collapses them before clearing the selection.
15. **Inspector close button.** The spec doesn't list one. The mock adds × to the card and overview headers, because students without a keyboard habit need a visible way out.
16. **Attribution "never hidden" versus the inspector.** The inspector can extend to 12 px from the bottom on the right, where the attribution sits. The mock moves the attribution left of the inspector together with the map controls.
17. **Row height versus hit target.** Shelf rows are 32 px tall, but icon buttons need at least 36 × 36. The eye button inside a row can only be 32 × 32.
18. **Importing row text.** "Reading 1.2 of 3.4 MB" plus the file name doesn't fit in a 280 px shelf row. The mock truncates the name in the middle ("distr…geojson") and uses KB for small files.
19. **Where does "248 records opened; 6 have no usable location" live, and for how long?** The mock shows it only in the layer overview (amber icon), so students may never see it. Is it a shelf badge, a toast or a notice in the overview? Is the badge amber?
20. **Row 6 with fewer than three eligible actions.** Districts have no category field, so "Filter by [Type]" drops out when no point layer is visible, leaving two actions. Is that acceptable, or does the next eligible action move up?

**Map and style**

21. **First launch "world map".** The mock opens on an East Asia regional view of a drawn basemap. Should first launch show the whole world, the user's region, or the last place?
22. **Data labels.** The spec has no rule for labelling data features, so the mock shows none. The hottest district, Huangpu, is a small square inside Jing'an, so the lack of labels may decide the task. Should area layers label features by their Name field by default?
23. **Fill opacity for value-colored areas.** Areas fill at 35% by default. At 35%, a YlOrRd choropleth over the basemap is hard to read. The mock uses 78% for value-colored areas. Does the 35% rule apply only to single-color areas?
24. **Classed versus continuous color.** The spec forbids more than 7 classes but doesn't say whether "Color by a value" defaults to classed or continuous. The mock is continuous, with a min–max legend in the field's unit.
25. **Legend on the map.** The spec puts the legend only in the layer overview, so the legend disappears when the user then clicks a district. The mock adds a compact legend under the shelf row. Is that allowed, or should there be a map legend?
26. **Point edge on value-colored or dark maps.** The 1.5 px white edge is kept in dark mode, as specified. Vermillion school points over the red end of Heat are hard to see. Should the default point color avoid the active area palette?
27. **Theme switch.** The spec defines dark tokens but no way to choose a theme. The mock adds a toolbar toggle. Should the app follow the OS setting, and does it need a setting at all?
28. **Layer display names.** "Districts" comes from `districts.geojson`. What is the rule (capitalization, plural, removing the extension), and is it the same as the rename alias?
29. **"Measure its area" when the area is already known.** The subtitle already shows the area. Is this action still useful in row 7, or should it be replaced when the subtitle shows the same value?

## What is simulated or left out

- **Left out entirely:** the composer, preview and Keep result flow (the count and rank cards appear directly, marked "Preview · not kept"); the table drawer; filters; distance areas; export; drag-and-drop; the overlap chooser; right-click menus; and ⌘K action search.
- **Simulated:**
  - The file dialog.
  - The Change link on the assumption opens the choices, but only WGS 84 can be chosen.
  - The basemap is a hand-drawn coarse coastline, not real tiles. Districts and schools come from the real fixtures, which are themselves synthetic: V1's geometry is a 0.15° grid.
- **Font:** Inter is not embedded. Tabular figures depend on the system font.
