# X5 — CSV cells that look like spreadsheet formulas (synthetic)

`x5_formulas.csv`: five located points near Shanghai whose `name` and `note`
cells begin with `=`, `+`, `-` or `@` (one row is plain). Columns
`name, lon, lat, note`.

Tests: no spreadsheet-formula injection on export (Gate 5). Every exported CSV
cell that begins with `=`, `+`, `-` or `@` (or a tab or carriage return) is
written with a leading apostrophe; numbers, such as negative longitudes, are
written as numbers (`lm_jobs::export`,
`x5_cells_that_look_like_formulas_are_neutralized`).

The file is byte-for-byte what `cargo xtask fixtures --only X5` writes to
`fixtures/generated/X5/`; it is kept here so the tests run without generating
fixtures first. Synthetic, CC0-1.0.
