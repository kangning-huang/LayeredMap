# R8 — NetCDF grids (WP6.12)

Four small synthetic NetCDF files and their xarray reference values:

- `r8_classic.nc`: NetCDF-3 classic (CDF-1). `t2m` (time, lat, lon) "2 m temperature" in degC
  with `_FillValue` −999 on two cells; `ta` (time, level, lat, lon) "Air temperature" in K on two
  pressure levels (850 and 500 hPa); `orog` (lat, lon) with no time. Latitude runs south to north
  (LayeredMap shows it north up). Three monthly steps, standard calendar: June, July, August 2023.
- `r8_packed_noleap.nc`: NetCDF-4, deflate level 4 with shuffle, chunked (1, 4, 5). `sst` is int16
  packed with `scale_factor` 0.01 and `add_offset` 20, land at `_FillValue` −32768; `sst_anom` is
  float32 with only a `missing_value`. Four daily steps in the noleap calendar from 27 February
  2004: the third is 1 March (2004 has a 29 February in the standard calendar).
- `r8_lon360.nc`: NetCDF-3 64-bit offset (CDF-2). `pr` "Precipitation" in mm/day on longitudes
  0–337.5 (shown as −180–180), latitude north to south; four yearly steps in the 360_day calendar
  (2000 to 2003).
- `r8_curvilinear.nc`: NetCDF-4 with 2-D latitude and longitude: LayeredMap refuses it with a
  plain message.

Expected values (xarray with CF decoding: masking, scale and offset, cftime dates):

- `expected_dates.csv`: each time step's date.
- `expected_values.csv`: every cell of every step and level of every grid variable, with its
  longitude in −180–180 and latitude; a blank value is No data.
- `expected_slices.csv`: each 2-D slice's count of valid cells, min, max and mean.

Checked by `cargo test -p lm-jobs netcdf` (`crates/lm-jobs/src/import/netcdf/tests.rs`) and the
approved profile snapshot `fixtures/snapshots/R8.json`. Seeds for the `netcdf` fuzz target.

Made by `reference/make_netcdf_fixtures.py` (synthetic, CC0-1.0).
