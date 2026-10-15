# T4 — GBK CSV, semicolons, decimal commas, codes, dates

`stations_gbk.csv`: 10 rows, **GBK**-encoded, `;`-delimited, decimal
**commas** (`121,473701`, `1234,50`), a code column `编码` with leading zeros
(`001234`, `000056`, …) and ISO dates `日期` (`2024-02-29` is a leap day).
Coordinates are approximate WGS 84 metro station positions.

`expected_raw_utf8.csv` is byte-for-byte the same text re-encoded as UTF-8 —
the exact raw strings the app must preserve.

Tests: raw-string preservation (codes stay text with zeros; raw cell text is
kept), encoding + delimiter + decimal-comma detection.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
