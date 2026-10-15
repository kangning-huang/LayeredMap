# V5 — Shapefile with GBK-encoded .dbf and no .cpg

`places_gbk.*`: 12 Shanghai landmarks (approximate WGS 84 positions)
with Chinese `name`, `district`, `category` attributes. Written by GDAL with
`ENCODING=GBK`, then the `.cpg` was deleted **and the dBASE language-driver byte
(offset 29) was set to 0**, so nothing in the files declares the encoding. The
`.prj` (WGS 84) is kept so only the encoding is under test.

Tests: encoding detection. Expected: GBK (GB18030/CP936 acceptable), first row
name `人民广场`. Decoding as UTF-8 or Latin-1 must not be presented silently.

Made by `reference/make_handmade.py` (synthetic, CC0-1.0).
