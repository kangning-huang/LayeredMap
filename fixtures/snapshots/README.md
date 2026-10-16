# Profile snapshots (Gate 2)

Gate 2 › Automated: *"Every V, T, C and R fixture opens, and its profile matches the approved
JSON snapshot: counts, field roles, coordinate system, assumptions. Every X fixture is refused
with its expected message."*

Each `<ID>.json` is the import result for every input file of fixture `<ID>`, made by
`lm_jobs::import::snapshot::summary`: per layer the name, archive member, format, kind, count,
coordinate system and its confidence, extent (longitude/latitude, rounded to 6 decimals), text
encoding, each field's role and confidence, the assumptions (catalog row, treatment, chosen
answer, line), the number of excluded records, the notices and the import status; for a file
that doesn't open, the kind of refusal and the message shown. No paths, no modification times.

| Test | Fixtures |
|---|---|
| `import::gate2_tests::every_hand_made_fixture_matches_its_approved_snapshot` | every hand-made `V*`, `T*`, `C*`, `R*` folder |
| `import::gate2_tests::generated_fixtures_match_their_approved_snapshots` | V7, V8, R7, X1–X5 (skipped, with a message, without the generated fixtures) |
| `import::tests::large_fixtures` (`cargo xtask stress`) | T3, R4 |

The generated fixtures are found in `$LM_FIXTURES_DIR`, else `fixtures/generated`. Both tests
also check that no file in the fixture's folder changes (SHA-256 before and after).

Any difference fails the test with a line diff (`-` approved, `+` now). After a deliberate
change, rewrite the snapshots and explain each changed one in the gate report (Gate 2 › Debug):

```bash
LM_UPDATE_SNAPSHOTS=1 LM_FIXTURES_DIR=/path/to/generated cargo test -p lm-jobs snapshot
```

T3's and R4's snapshots are written by the stress run:
`LM_UPDATE_SNAPSHOTS=1 cargo xtask stress`.

## Initial snapshots: where they disagree with `expected.json`

The first snapshots (2026-10-16) record what the importer does now (T5 and V9 were rewritten after
the rebase onto `phase-2`, which merged the GeoPackage and Excel adapters; no other snapshot changed); they are **not yet approved
by the owner**. The fixtures' hand-written `expected.json` files were not edited. Every place
where the two disagree is listed here for the owner's decision at Gate 2: either the behaviour
is fixed (and the snapshot rewritten), or the expectation is changed with a note.

Recurring across fixtures (not repeated below):

- **Extra China line.** Every WGS 84-assumed layer inside mainland China (V1, V2, V3, T1, T2, T4,
  R6 points, X5) gets the visible line *"If these came from a Chinese map app (Amap, Tencent or
  Baidu), they may sit a few hundred meters off."* (`ChinaOffsetPossible`); no `expected.json`
  lists it. T5 has it too.
- **Extra silent ID / category lines.** `IdentifierField` ("Treated as an ID.") and
  `FewValuesCategorical` ("Colored as categories.") appear, silently, where `expected.json`
  lists no assumptions (V1, V2, V3, V4, V5, V9, T2, T4, R6, C2).
- **Extra raster unit line.** Every raster gets the visible line *"Unit unknown."*
  (`RasterNoUnit`): R3, R6, R7.
- **Coordinate roles.** `expected.json` names coordinate columns `longitude` / `latitude`
  (`longitude?` for T2); the profile has a single role, `Coordinate`. Not a disagreement in
  substance, but the snapshot can't tell longitude from latitude.

| Fixture | `expected.json` says | The snapshot has |
|---|---|---|
| V1 | CRS *declared* (GeoJSON default), `assumptions: []` | visible `NoCrsLonLat` "Assumed GPS coordinates (WGS 84), because the file doesn't say.", confidence Likely. The same for V2, V10 and every generated GeoJSON (V7, V8). RFC 7946 makes WGS 84 the GeoJSON default, so this line may be one the owner wants silent. |
| V2 | `assumptions: []` | silent `MixedGeometrySplit` "Split into 3 layers: points, lines, areas" on each of the 3 layers (the split itself matches: 4 points, 3 lines, 3 areas) |
| V3 | `crs_wgs84_assumed`, visible | matches (`NoCrsLonLat`, visible); plus the recurring lines |
| V4 | `crs_unknown_projected`, *visible* | `NoCrsProjected` as a **question** ("Where should this data be?"), status Needs a decision. The expectation's text ("user must confirm the place") reads like a question too. |
| V5 | `encoding_detected` "Text encoding detected as GBK (no .cpg)", **visible** | `ShapefileGbk` "Read text as Chinese (GBK).", **silent**. Encoding GBK and the first row match. |
| V9 | GeoPackage `city_layers.gpkg` opens as a group of 3 layers (schools 8 points, roads 6 lines, parks 3 areas, declared EPSG:4326), `assumptions: []` | matches since the GeoPackage adapter (WP2.2 P1) was merged: 3 layers, members named after their tables, CRS WGS 84 *Certain*; the roads layer carries the recurring silent ID and category lines. The zipped Shapefile matches too: 6 lines, from the `.prj`. |
| V10 | 9 features, one layer; the antimeridian polygon *"must NOT draw/measure a 358°-wide polygon"*; empty geometries reported | split into 2 layers (3 lines, 6 areas, counting the features without geometry; 2 exclusions are reported, for 3 empty cases), silent `MixedGeometrySplit` and visible `NoCrsLonLat`. The areas layer's extent is −180…180 and the lines layer's −179.5…179.5: the antimeridian cases are read naively. |
| T1 | `crs_wgs84_assumed` visible | matches (`CsvCoordinateColumns` visible, "Located using lon and lat as GPS coordinates (WGS 84)."), 248 rows, 6 excluded |
| T2 | question **"Which columns hold the location?"**, candidates x/y and lng2/lat2, no silent default | question worded **"Which columns locate these records?"**, recommended (preselected and drawn) lng2/lat2; 2 rows excluded under that pair |
| T4 | three visible assumptions: `encoding_detected`, `decimal_comma`, `crs_wgs84_assumed` | only `CsvCoordinateColumns` (visible). The encoding is recorded (GBK) but has **no line**, and there is **no decimal-comma line**: the assumption catalog has no row for either in a CSV. Raw strings (leading zeros, dates) are kept (unit test). |
| T5 | sheet "Schools" chosen (not the first sheet), `sheet_chosen` visible, `crs_wgs84_assumed` visible | matches since the Excel adapter (WP2.2 P1) was merged: visible `ExcelSheet` "Opened sheet 'Schools'.", visible `CsvCoordinateColumns` on longitude/latitude, 20 points; plus the recurring China line. (The earlier misleading "holds no Shapefile" refusal is gone: the zip adapter no longer claims .xlsx.) |
| C1 | `offset_system_gcj02`, **visible** (and `offset_question.asked: true`) | `ChinaOffsetNamed` as a **question**: "These look like Amap (GCJ-02) coordinates. Convert to GPS coordinates?", preselected gcj02; status Needs a decision |
| C2 | (no `expected.json`; `vectors.csv` holds conversion test vectors, not an import case) | opens as 58 points with an ambiguous-columns question (13 coordinate-like columns). `landmarks_TODO.csv` is owner-supplied and skipped. |
| C3 | `offset_system_bd09`, **visible** | `ChinaOffsetNamed` as a **question** ("These look like Baidu (BD-09) coordinates. …"), preselected bd09 |
| R3 | `assumptions: []`, NoData 0 | NoData 0 matches; plus visible `RasterCellMeaning` "Treated as categories (shares, not averages)." and the unit line |
| R6 | scale 0.1, offset −100 (`value = raw * scale + offset`); `nodata_assumed` −9999 visible | the NoData line matches ("Treated −9999 as No data."), but **scale and offset are not read** (`scale: null, offset: null`): the importer passes none to the profiler and the upstream GeoTIFF reader doesn't expose them, so values would be shown raw. Cell meaning is a silent deferred question (measured). `points.csv` (the sampling points) opens as 8 points. |

The X fixtures have no `expected.json`; their snapshots hold the messages shown:

| Fixture | Result |
|---|---|
| X1 | refused: "This archive has other archives inside it. …" |
| X2 | refused: "This archive contains a file that points outside the archive ("../../evil.shp"), …" |
| X3 | `x3_nesting.geojson` fails ("nested too deeply to be valid GeoJSON"); `x3_nan.geojson` fails ("isn't valid GeoJSON (a problem on line 1)"; NaN isn't JSON); `x3_huge_array.geojson` **opens** as one line with raw coordinates and the question "Where should this data be?" |
| X4 | refused: "x4_billion.tif says it is 1,000,000,000 × 1,000,000,000 pixels, more than the 200,000 …" |
| X5 | **opens** as 5 points (the formula cells are defused only in "Export problem list", tested separately) |
