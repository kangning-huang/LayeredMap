"""v3.4: one summer (1 June - 31 August) of the Copernicus GDO warm-spell intensity file, made the
same way as the app's `heatwaves summer 2023.nc` example: 2x2 block mean 0.25 -> 0.5 degree,
zeros (no warm spell) set to no data, variable `heatwave_intensity` with long name and unit.
Checked by rebuilding 2023 and comparing with assets/examples/heatwaves summer 2023.nc.

    mamba run -n layeredmap-ref python data/make_heatwave_nc.py <hwi2a_m_gdo_YYYY...nc> <out.nc>
"""
import datetime
import sys

import netCDF4 as nc
import numpy as np

src, out = sys.argv[1], sys.argv[2]
s = nc.Dataset(src)
t = s['time'][:]
year = int(s['time'].units.split()[2][:4])
d0 = (datetime.date(year, 6, 1) - datetime.date(year, 1, 1)).days
d1 = (datetime.date(year, 8, 31) - datetime.date(year, 1, 1)).days
sel = np.where((t >= d0) & (t <= d1))[0]
v = s['hwi2a'][sel, 0].astype(np.float64)
v = np.ma.filled(v, np.nan)
n, h, w = v.shape
m = np.nanmean(v.reshape(n, h // 2, 2, w // 2, 2), axis=(2, 4))
m[m == 0] = np.nan
lat = s['lat'][:].reshape(-1, 2).mean(1)
lon = s['lon'][:].reshape(-1, 2).mean(1)
d = nc.Dataset(out, 'w', format='NETCDF4')
d.title = f'Heat-wave intensity, summer {year} (June–August), 0.5°'
d.source = f'Copernicus Global Drought Observatory (JRC), GDO Warm Spell Intensity ERA5 v3.0.0, {year} file, averaged from 0.25° to 0.5°'
d.license = 'CC BY 4.0'
d.copyright = '(c) European Union, 1995-2026'
d.createDimension('time', len(sel))
d.createDimension('lat', h // 2)
d.createDimension('lon', w // 2)
tv = d.createVariable('time', 'i8', ('time',))
tv.units = f'days since {year}-01-01'
tv.calendar = 'proleptic_gregorian'
tv[:] = t[sel]
b = d.createVariable('band', 'i8', ())
b.assignValue(1)
la = d.createVariable('lat', 'f8', ('lat',), fill_value=np.nan)
la[:] = lat
lo = d.createVariable('lon', 'f8', ('lon',), fill_value=np.nan)
lo[:] = lon
hv = d.createVariable('heatwave_intensity', 'f4', ('time', 'lat', 'lon'), fill_value=np.float32(np.nan), zlib=True,
                      complevel=4, shuffle=True)
hv.long_name = 'Heat-wave intensity'
hv.units = '°C'
hv.comment = 'Accumulated warm-spell intensity (temperature anomaly method), daily; no data where there is no warm spell.'
hv.coordinates = 'band'
hv[:] = m.astype(np.float32)
d.close()
print(out, year, len(sel), 'days')
