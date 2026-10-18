# NetCDF reader spike (WP6.12), 2026-10-18

**Verdict: `netcdf-reader` 0.9.1 (pure Rust, MIT OR Apache-2.0, features `netcdf4` and `cf`) does
everything WP6.12 needs. No fallback to the C-based `netcdf` crate.** Nothing failed on the test
files; one robustness point (below) is handled on LayeredMap's side.

## What was tested

Files in `/Volumes/LaCie/data/layeredmap-netcdf-test/` (sources, licences and the synthetic
file's script in its `README.md`), read through LayeredMap's import pipeline
(`crates/lm-jobs/src/import/netcdf.rs`) by the ignored test `netcdf_spike`:

```bash
LM_NETCDF_SPIKE_DIR=/Volumes/LaCie/data/layeredmap-netcdf-test \
  cargo test -p lm-jobs --release --lib netcdf_spike -- --ignored --nocapture
```

| File | Format | What it exercises | Result |
|---|---|---|---|
| `sresa1b_ncar_ccsm3-example.nc` (Unidata, 2.8 MB) | NetCDF-3 classic | noleap calendar from year 0, Gaussian latitudes, 4-D variable on 17 pressure levels, `_FillValue` 1e20 | ✓ 5 grid variables; "16 May 2000 12:00"; uneven latitudes drawn at most 0.04° off |
| `tos_O1_2001-2002.nc` (Unidata, 2.9 MB) | NetCDF-3 classic | 360_day calendar, 0–360 longitudes, land filled | ✓ 24 monthly steps, January 2001 → December 2002 |
| `sst.mnmean.nc` (NOAA OISST v2, 64 MB) | NetCDF-3 classic | int16 with `scale_factor`, `missing_value`, 494 steps | ✓ |
| `air.mon.mean.nc` (NCEP/NCAR R1, 30 MB) | NetCDF-4 classic, deflate, chunked | "hours since 1800-01-01 00:00:0.0", latitude north to south incl. ±90 | ✓ step 700 (May 2006): mean 5.567275 over 10,512 cells, **identical to xarray** |
| `sst.mon.mean.nc` (NOAA OISST v2.1 0.25°, **2.24 GB**) | NetCDF-4 classic, chunked (1, 720, 1440) | > 2 GB, 541 steps, 0–360 longitudes | ✓ |
| `synthetic_large_deflate_shuffle.nc` (made with netCDF4-python, **2.26 GB**) | NetCDF-4, **deflate + shuffle**, chunked (1, 360, 720) | > 2 GB compressed, int16 packed with `scale_factor`/`add_offset` and `_FillValue`, noleap, 0–360, south-up, 1,320 steps | ✓ step 700 (May 2008): mean 269.164958 over 979,200 cells, **identical to xarray** |

**The owner's heatwave data** (JRC Global Drought Observatory, Heat-wave Intensity, daily 2023,
`jrc_warm_spell/hwi2a_m_gdo_20230101_20231231_d.nc`, 125 MB, CC BY 4.0; documented in that folder's
README; not copied into the repository) is checked by the ignored tests
`jrc_heatwave_opens_fast_and_plays` (lm-jobs) and `the_jrc_heatwave_file_plays` (lm-ui), which run
when the drive is mounted. It is NetCDF-4 with zlib 9 + shuffle in chunks of (92, 1, 180, 360):
365 days, a `band` dimension of one, latitude north to south, `_FillValue` NaN, a `spatial_ref`
grid-mapping variable, `lat`/`lon` with no attributes at all, and no `units` or `long_name` on
`hwi2a` (the title is the global `04_TITLE`, the unit `07_DATA_UOM` "degree Celsius [°C]"). It
needed three changes, all made:

- `lat` and `lon` are recognised by name when they have no units;
- a dimension of length one (`band`) is squeezed: no level picker, no line; `spatial_ref` (a
  scalar) and `band` are never offered as variables;
- a file's lone variable without a long name or units takes the global title and unit, so the row
  reads **"Heat-wave Intensity (°C)"**.

Its chunks hold 92 days each, so reading one day inflates 32 chunks of 92 days (0.24 s a day at
first). LayeredMap learns a variable's time chunk on its first read and then reads the steps
around the one asked for in the same pass (up to 32 steps, 128 MB) and keeps them: playing costs
about 0.5 s every 32 days and nothing in between (0.05 s a day on average), and the colour range
is sampled from one step per chunk (4 steps here), so the file opens in about 1.5 s. 10 July 2023
matches xarray (mean 0.9472581 °C over 1,036,800 cells).

The repository's fixtures (`fixtures/R8/`: NetCDF-3 CDF-1 and CDF-2, NetCDF-4 with deflate,
shuffle, chunks, packing and a noleap calendar, 0–360 longitudes, a curvilinear refusal) are
compared cell by cell with xarray in `cargo test -p lm-jobs netcdf` (1,520 cells, every slice's
count, min, max and mean, every date against cftime).

## Speed (Apple M4, release build, files on an external USB drive)

| File | Open + profile (header, coordinates, dates, 12 sampled steps for the colour range) | One step not yet read |
|---|---|---|
| 2.26 GB deflate + shuffle (1440 × 720) | 1.2–2.0 s cold, 0.13 s warm | 0.06–0.15 s |
| 2.24 GB uncompressed chunked (1440 × 720) | 0.65–1.0 s | 0.03 s |
| 64 MB NetCDF-3 (360 × 180) | 0.4–0.65 s | 0.001 s |
| JRC heatwave 2023, 125 MB, zlib 9 + shuffle, 92-day chunks (1440 × 720) | 1.5 s (4 sampled steps, one per chunk) | 0.5 s per 32-step block, then 0 (0.05 s a day on average) |
| small files | ≤ 0.15 s | ≤ 0.001 s |

Opening never reads the cells beyond the 12 sampled steps; each step is read when shown (a
2-D hyperslab) and the last ones are kept (384 MB budget), so Play at 2 steps a second is never
waiting on the file once the next step is prefetched. xarray took 0.30 s to open the synthetic
file and read one step, for comparison.

## Notes

- **No memory mapping.** `NcFile::open` memory-maps NetCDF-3 files; a file truncated or rewritten
  while mapped would crash the app (SIGBUS). LayeredMap opens every file through
  `NcFile::from_storage_with_options` with `FileStorage` (positional reads), so such a file gives
  an error instead.
- **Time units must be ASCII.** The reader's `parse_time_units` slices the units string by byte
  positions of its lower-cased form, which can panic on non-ASCII text. LayeredMap only passes
  ASCII units and calendars (anything else: steps are numbered), and the import runs inside a
  panic guard anyway. Worth reporting upstream.
- "months since …" is decoded by the reader only for 360_day; LayeredMap counts whole calendar
  months itself for the other calendars (common in practice, though CF discourages it).
- The default fill value of floats (9.96921e36) is drawn as No data even when a file doesn't
  declare it (xarray doesn't mask it; no test file relies on this).
- Dependencies added: `netcdf-reader`, `netcdf-core`, `hdf5-reader`, `hdf5-core` (all MIT OR
  Apache-2.0, same authors), plus `ndarray`, `memmap2` (unused at run time), `lru`, `lz4_flex`
  0.11, `twox-hash`, `matrixmultiply`, `rawpointer`, `num-complex` (MIT and/or Apache-2.0). They
  build for `wasm32-unknown-unknown`.
- Not tested: szip-compressed files (the reader has no szip filter; such a file fails with a read
  error), HDF5 virtual datasets, variables in NetCDF-4 subgroups (LayeredMap reads the root group
  only).
