# N7 — R4 across 3,000 polygons (performance, spot correctness)

## The stress run

R4 (`cargo xtask fixtures --only R4`, a 2.9 GB tiled BigTIFF COG: 23,040² float32
cells of 5.2e-5° over Shanghai in 512 px tiles, with 6 overviews) and N7's 3,000
polygons (`cargo xtask fixtures --only N7` → `fixtures/generated/N7/n7_polygons.geojson`,
id `area_id`; every 7th has a hole, every 11th is a multipolygon; the western column
and northern row, and some eastern and southern areas, cross the raster's edge) are
generated, never committed (`fixtures/generated/` is ignored by git).
`lm-analysis` test `zonal::tests::n7_r4_cog_stress` (ignored; run in release)
summarizes R4 across the 3,000 polygons with the window reader
(`lm_analysis::zonal::TiffSource`), times it and a cancel, and spot-checks a sample
against `expected_zonal_mean_sample.csv`:

```bash
cargo xtask fixtures --only R4,N7
cargo test --release -p lm-analysis zonal::tests::n7_r4_cog_stress -- --ignored --nocapture
# or: cargo xtask stress (all the stress tests)
```

`expected_zonal_mean_sample.csv` — `reference/n7_sample.py`: the 31 polygons with
index ≡ 0 (mod 100) and the last one, by exactextract on each polygon's window of
R4 with a geodesic cell-area weight raster (same definition as
`reference/zonal_mean.py`), cross-checked with exact shapely fractions (all 31
agree; `source` column). Tolerance 1 part per million.

## Reader layouts

`layouts/`: 150 × 110 GeoTIFFs written by GDAL (rasterio
1.5.2, GDAL 3.13.3), one per layout the window
reader must decode. Every band holds a formula of the column `c` and row `r` with
`v = (7c + 13r) mod 251`, and NoData wherever `(3c + 5r) mod 97 = 0`:

| file | type | bands | block | compression | interleave | CRS | NoData | overviews |
|---|---|---|---|---|---|---|---|---|
| `cog_deflate_f32.tif` | float32 | 1 | 64 × 64 | DEFLATE | - | EPSG:4326 | -9999 | 2 |
| `tiled_lzw_i16.tif` | int16 | 1 | 32 × 32 | LZW | - | EPSG:4326 | -32768 | 0 |
| `striped_none_u8.tif` | uint8 | 1 | 150 × 7 | none | - | EPSG:4326 | 255 | 0 |
| `striped_deflate_u16.tif` | uint16 | 1 | 150 × 16 | DEFLATE | - | EPSG:4326 | 65535 | 0 |
| `tiled_none_f32_utm.tif` | float32 | 1 | 48 × 48 | none | - | EPSG:32651 | nan | 0 |
| `planar_tiled_u8_3band.tif` | uint8 | 3 | 32 × 32 | DEFLATE | BAND | EPSG:4326 | 255 | 0 |
| `chunky_striped_u16_3band.tif` | uint16 | 3 | 150 × 5 | LZW | PIXEL | EPSG:4326 | 65535 | 0 |
| `striped_packbits_i16.tif` | int16 | 1 | 150 × 9 | PACKBITS | - | EPSG:4326 | -32768 | 0 |
| `tiled_zstd_f32.tif` | float32 | 1 | 64 × 32 | ZSTD | - | EPSG:4326 | -9999 | 0 |
| `bigtiff_tiled_f64.tif` | float64 | 1 | 16 × 16 | DEFLATE | - | EPSG:4326 | -1e+30 | 0 |

Values: `cog_deflate_f32` v + 0.25; `tiled_lzw_i16` v − 100; `striped_none_u8` v;
`striped_deflate_u16` 200·v; `tiled_none_f32_utm` v + 0.5; `striped_packbits_i16` v − 100;
`tiled_zstd_f32` v + 0.25; `planar_tiled_u8_3band`
band b (0-based) (v + 3b) mod 251; `chunky_striped_u16_3band` v + 1000·b;
`bigtiff_tiled_f64` v/8 − 3. Geographic files: origin 120.5°E, 31.9°N, cells
0.002° × 0.0025°; the UTM file (EPSG:32651): origin 350,000 E, 3,460,000 N, 30 m.

Made by `reference/make_summary_fixtures.py` (synthetic, CC0-1.0).
