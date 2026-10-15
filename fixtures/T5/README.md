# T5 — Excel workbook, 3 sheets

`survey.xlsx`: sheets `Summary` (text), `Schools` (20 rows with
`longitude`/`latitude`), `Budget` (no coordinates). Only `Schools` has
coordinates and it is the **second** sheet. Workbook timestamps and zip member
times are fixed for reproducibility (seed 5).

Tests: sheet choice — expect `Schools`.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
