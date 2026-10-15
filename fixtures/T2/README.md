# T2 — Ambiguous coordinate columns

`deliveries.csv`: 30 rows, columns `id, name, x, y, lat2, lng2`. Both `x/y` and
`lng2/lat2` hold plausible Shanghai lon/lat pairs (pickup / drop-off).
Swapped rows: in data rows [5, 13, 21] `x` holds the latitude and `y` the
longitude; in rows [9, 26] `lat2` holds the longitude and `lng2` the latitude.

Tests: the **question card** — the app must ask which pair is the location
rather than silently choosing, and should flag the swapped rows (seed 22).

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
