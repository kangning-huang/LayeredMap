"""Build the NetCDF fixtures (R8, WP6.12) and their xarray reference values.

Four small synthetic grids (CC0-1.0), written with netCDF4-python from closed-form values:

- `r8_classic.nc`: NetCDF-3 classic (CDF-1). `t2m` (time, lat, lon) in degC with a `_FillValue`
  and two filled cells; `ta` (time, level, lat, lon) in K on two pressure levels; `orog`
  (lat, lon) with no time. Latitude runs south to north (LayeredMap flips it). Monthly steps,
  standard calendar ("days since 2023-06-01": June, July, August 2023).
- `r8_packed_noleap.nc`: NetCDF-4, deflate level 4 with shuffle, chunked (1, 4, 5). `sst` is
  int16 packed with `scale_factor` 0.01 and `add_offset` 20, land cells at `_FillValue` −32768;
  `sst_anom` is float32 with a `missing_value` only. Daily steps in the noleap calendar from
  27 February 2004, so the third step is 1 March (2004 is a leap year in the standard calendar).
- `r8_lon360.nc`: NetCDF-3 64-bit offset (CDF-2). `pr` (time, lat, lon) on longitudes 0–337.5
  (LayeredMap shows them as −180–180), latitude north to south, yearly steps in the 360_day
  calendar ("days since 2000-01-01": 0, 360, 720, 1080).
- `r8_curvilinear.nc`: NetCDF-4 with 2-D latitude and longitude (a curvilinear grid), which
  LayeredMap refuses with a plain message.

Reference values come from xarray (CF decoding: masking, scale and offset, cftime dates):

- `expected_dates.csv`: file, time_index, date (cftime, `YYYY-MM-DD HH:MM:SS`).
- `expected_values.csv`: every cell of every time step and level of every grid variable:
  file, variable, time_index, level_index, lon (in −180–180), lat, value (blank: no data).
- `expected_slices.csv`: per 2-D slice, the count of valid cells and their min, max and mean.

Usage:
  python -I reference/make_netcdf_fixtures.py
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common  # noqa: E402, F401  (pins GDAL/PROJ data first)
from _common import FIX, fmt, write_csv  # noqa: E402

import netCDF4  # noqa: E402
import numpy as np  # noqa: E402
import xarray as xr  # noqa: E402

R8 = FIX / "R8"


def _common_attrs(ds: netCDF4.Dataset, title: str) -> None:
    ds.setncattr("Conventions", "CF-1.8")
    ds.setncattr("title", title)
    ds.setncattr("source", "LayeredMap test fixture (synthetic), reference/make_netcdf_fixtures.py")
    ds.setncattr("license", "CC0-1.0")


def _coord(ds, name, values, units, standard_name, axis, dtype="f8", **extra):
    v = ds.createVariable(name, dtype, (name,))
    v[:] = values
    v.units = units
    v.standard_name = standard_name
    v.axis = axis
    for k, val in extra.items():
        v.setncattr(k, val)
    return v


def make_classic(path) -> None:
    lat = np.array([-50.0, -30.0, -10.0, 10.0, 30.0, 50.0])  # south to north
    lon = np.arange(-157.5, 180.0, 45.0)  # 8 columns, −157.5 … 157.5
    ds = netCDF4.Dataset(path, "w", format="NETCDF3_CLASSIC")
    _common_attrs(ds, "R8 classic: monthly 2 m temperature, air temperature on levels, surface height")
    ds.createDimension("time", None)
    ds.createDimension("level", 2)
    ds.createDimension("lat", lat.size)
    ds.createDimension("lon", lon.size)
    t = ds.createVariable("time", "f8", ("time",))
    t.units = "days since 2023-06-01"
    t.calendar = "standard"
    t.standard_name = "time"
    t.axis = "T"
    t[:] = [0.0, 30.0, 61.0]
    lev = _coord(ds, "level", [850.0, 500.0], "hPa", "air_pressure", "Z", positive="down", long_name="pressure level")
    del lev
    _coord(ds, "lat", lat, "degrees_north", "latitude", "Y")
    _coord(ds, "lon", lon, "degrees_east", "longitude", "X")
    yy, xx = np.meshgrid(lat, lon, indexing="ij")
    t2m = ds.createVariable("t2m", "f4", ("time", "lat", "lon"), fill_value=np.float32(-999.0))
    t2m.long_name = "2 m temperature"
    t2m.units = "degC"
    data = np.stack([30.0 - 0.4 * np.abs(yy) + 0.01 * xx + k for k in range(3)]).astype("f4")
    data[0, 0, 0] = -999.0
    data[2, 5, 7] = -999.0
    t2m[:] = data
    ta = ds.createVariable("ta", "f4", ("time", "level", "lat", "lon"), fill_value=np.float32(1e20))
    ta.long_name = "Air temperature"
    ta.units = "K"
    ta[:] = np.stack([np.stack([290.0 - 0.3 * np.abs(yy) - 25.0 * lv + 0.5 * k for lv in range(2)]) for k in range(3)]).astype("f4")
    orog = ds.createVariable("orog", "f4", ("lat", "lon"))
    orog.long_name = "Surface height"
    orog.units = "m"
    orog[:] = (100.0 + 10.0 * (yy + 50.0) + 0.5 * (xx + 157.5)).astype("f4")
    ds.close()


def make_packed_noleap(path) -> None:
    lat = np.arange(40.0, -41.0, -10.0)  # 9 rows, north to south
    lon = np.arange(-162.0, 180.0, 36.0)  # 10 columns
    ds = netCDF4.Dataset(path, "w", format="NETCDF4")
    _common_attrs(ds, "R8 packed: daily sea surface temperature, noleap calendar")
    ds.createDimension("time", 4)
    ds.createDimension("lat", lat.size)
    ds.createDimension("lon", lon.size)
    t = ds.createVariable("time", "f8", ("time",))
    t.units = "days since 2004-02-27 00:00:00"
    t.calendar = "noleap"
    t.standard_name = "time"
    t.axis = "T"
    t[:] = [0.0, 1.0, 2.0, 3.0]
    _coord(ds, "lat", lat, "degrees_north", "latitude", "Y")
    _coord(ds, "lon", lon, "degrees_east", "longitude", "X")
    yy, xx = np.meshgrid(lat, lon, indexing="ij")
    land = (np.abs(yy - 10.0) <= 10.0) & (np.abs(xx + 18.0) <= 36.0)
    sst = ds.createVariable(
        "sst", "i2", ("time", "lat", "lon"), zlib=True, complevel=4, shuffle=True, chunksizes=(1, 4, 5), fill_value=np.int16(-32768)
    )
    sst.long_name = "Sea surface temperature"
    sst.units = "degC"
    sst.scale_factor = np.float32(0.01)
    sst.add_offset = np.float32(20.0)
    sst.set_auto_maskandscale(False)
    packed = []
    for k in range(4):
        v = 28.0 - 0.3 * np.abs(yy) + 0.02 * xx + 0.25 * k
        raw = np.round((v - 20.0) / 0.01).astype("i2")
        raw[land] = -32768
        packed.append(raw)
    sst[:] = np.stack(packed)
    anom = ds.createVariable("sst_anom", "f4", ("time", "lat", "lon"), zlib=True, complevel=4, shuffle=True, chunksizes=(1, 4, 5))
    anom.long_name = "Sea surface temperature anomaly"
    anom.units = "K"
    anom.missing_value = np.float32(-99.0)
    a = np.stack([0.1 * k + 0.01 * yy - 0.005 * xx for k in range(4)]).astype("f4")
    a[:, land] = -99.0
    anom[:] = a
    ds.close()


def make_lon360(path) -> None:
    lat = np.array([60.0, 30.0, 0.0, -30.0, -60.0])  # north to south
    lon = np.arange(0.0, 360.0, 22.5)  # 16 columns, 0 … 337.5
    ds = netCDF4.Dataset(path, "w", format="NETCDF3_64BIT_OFFSET")
    _common_attrs(ds, "R8 lon360: yearly precipitation, longitudes 0-360, 360_day calendar")
    ds.createDimension("time", None)
    ds.createDimension("lat", lat.size)
    ds.createDimension("lon", lon.size)
    t = ds.createVariable("time", "f8", ("time",))
    t.units = "days since 2000-01-01"
    t.calendar = "360_day"
    t.standard_name = "time"
    t.axis = "T"
    t[:] = [0.0, 360.0, 720.0, 1080.0]
    _coord(ds, "lat", lat, "degrees_north", "latitude", "Y")
    _coord(ds, "lon", lon, "degrees_east", "longitude", "X")
    yy, xx = np.meshgrid(lat, lon, indexing="ij")
    pr = ds.createVariable("pr", "f4", ("time", "lat", "lon"), fill_value=np.float32(1e20))
    pr.long_name = "Precipitation"
    pr.units = "mm/day"
    pr[:] = np.stack([2.0 + 3.0 * np.cos(np.radians(yy)) + xx / 100.0 + 0.5 * k for k in range(4)]).astype("f4")
    ds.close()


def make_curvilinear(path) -> None:
    ds = netCDF4.Dataset(path, "w", format="NETCDF4")
    _common_attrs(ds, "R8 curvilinear: 2-D latitude and longitude")
    ds.createDimension("y", 3)
    ds.createDimension("x", 4)
    jj, ii = np.meshgrid(np.arange(3.0), np.arange(4.0), indexing="ij")
    la = ds.createVariable("lat", "f8", ("y", "x"))
    la.units = "degrees_north"
    la.standard_name = "latitude"
    la[:] = 30.0 + jj + 0.1 * ii
    lo = ds.createVariable("lon", "f8", ("y", "x"))
    lo.units = "degrees_east"
    lo.standard_name = "longitude"
    lo[:] = 120.0 + ii + 0.1 * jj
    v = ds.createVariable("tos", "f4", ("y", "x"))
    v.long_name = "Sea surface temperature"
    v.units = "degC"
    v.coordinates = "lat lon"
    v[:] = (15.0 + jj + ii).astype("f4")
    ds.close()


FILES = {
    "r8_classic.nc": make_classic,
    "r8_packed_noleap.nc": make_packed_noleap,
    "r8_lon360.nc": make_lon360,
    "r8_curvilinear.nc": make_curvilinear,
}
# The grids LayeredMap opens, with their grid variables.
GRIDS = {
    "r8_classic.nc": ["t2m", "ta", "orog"],
    "r8_packed_noleap.nc": ["sst", "sst_anom"],
    "r8_lon360.nc": ["pr"],
}


def references() -> None:
    dates, values, slices = [], [], []
    for name, variables in GRIDS.items():
        ds = xr.open_dataset(R8 / name, decode_times=xr.coders.CFDatetimeCoder(use_cftime=True))
        for i, d in enumerate(ds["time"].values):
            dates.append([name, i, d.strftime("%Y-%m-%d %H:%M:%S")])
        for var in variables:
            da = ds[var].astype("float64")
            nt = da.sizes.get("time", 1)
            nl = da.sizes.get("level", 1)
            for ti in range(nt):
                for li in range(nl):
                    s = da
                    if "time" in s.dims:
                        s = s.isel(time=ti)
                    if "level" in s.dims:
                        s = s.isel(level=li)
                    s = s.transpose("lat", "lon")
                    arr = s.values
                    for r, la in enumerate(ds["lat"].values):
                        for c, lo in enumerate(ds["lon"].values):
                            lo180 = ((float(lo) + 180.0) % 360.0) - 180.0
                            v = arr[r, c]
                            values.append([name, var, ti, li, fmt(lo180), fmt(float(la)), fmt(None if np.isnan(v) else float(v))])
                    ok = arr[~np.isnan(arr)]
                    slices.append([name, var, ti, li, ok.size, fmt(ok.min()), fmt(ok.max()), fmt(ok.mean())])
        ds.close()
    write_csv(R8 / "expected_dates.csv", ["file", "time_index", "date"], dates)
    write_csv(R8 / "expected_values.csv", ["file", "variable", "time_index", "level_index", "lon", "lat", "value"], values)
    write_csv(R8 / "expected_slices.csv", ["file", "variable", "time_index", "level_index", "valid", "min", "max", "mean"], slices)


def main() -> None:
    R8.mkdir(parents=True, exist_ok=True)
    for name, make in FILES.items():
        p = R8 / name
        if p.exists():
            p.unlink()
        make(p)
    references()
    print(f"R8: {len(FILES)} NetCDF files and 3 expected CSVs (xarray {xr.__version__}, netCDF4 {netCDF4.__version__})")


if __name__ == "__main__":
    main()
