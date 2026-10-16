# Attribution

Every asset file in this repository, with its source and licence.

| File | Author / source | Licence |
|---|---|---|
| Sample data (`countries`, `countries_chn`, `cities`, `rivers`, `lakes` GeoJSON), embedded at build time from [rustgis-assets](https://github.com/opengeos/rustgis-assets/tree/main/data) | Natural Earth 1:110m, attributes trimmed; `countries_chn` is Natural Earth's China point of view (1:10m, simplified) | Public domain |
| QGIS screenshot (`qgis-world-atlas.webp` in [rustgis-assets](https://github.com/opengeos/rustgis-assets/tree/main/images)), on the [RustGIS and QGIS](docs/comparison/qgis.md) page | Taken by the RustGIS project: [QGIS](https://qgis.org) 4.2 showing the Natural Earth sample data above | QGIS is GPL-2.0-or-later; the data is public domain |
| Example points (`assets/examples/populated_places.geojson`, Try an example), made by `tools/lm/make_examples.py`; sources in `assets/examples/README.md` | [Natural Earth](https://www.naturalearthdata.com) 1:50m populated places, version 5.1.2; five fields kept | Public domain |
| Example areas (`assets/examples/countries.geojson`, Try an example), made by `tools/lm/make_examples.py` | Natural Earth 1:110m admin-0 countries, version 5.1.1; eight fields kept, not simplified | Public domain |
| Example raster (`assets/examples/elevation.tif`, Try an example), made by `tools/lm/make_examples.py` | ETOPO 2022 v1 60 arc-second surface elevation (`ETOPO_2022_v1_60s_N90W180_surface.tif`), NOAA National Centers for Environmental Information, [doi:10.25921/fd45-gt74](https://doi.org/10.25921/fd45-gt74); averaged to 30 arc-minutes | Public domain (US Government work) |
| Mainland-China boundary (`crates/lm-profile/src/china_boundary.rs`, a Rust constant of 307 vertices used only to test whether points lie in China), made by `reference/make_china_boundary.py` | Natural Earth 1:50m admin-0 countries, 'China' (mainland and Hainan; without Taiwan, Hong Kong and Macau), buffered 0.1° and simplified | Public domain |

Raster coordinate systems are looked up by EPSG code in the PROJ definitions of the
[crs-definitions](https://crates.io/crates/crs-definitions) crate (CC0-1.0), compiled into the
app.

The GeoLibre tool suite ([geolibre-rust](https://github.com/opengeos/geolibre-rust), MIT, built
on [whitebox-wasm](https://github.com/opengeos/whitebox-wasm), MIT) is not part of RustGIS: the
desktop app and command line run its separately downloaded `geolibre` runner, and the web app
loads its WebAssembly runner (the `geolibre-wasm` npm package, from jsDelivr) when asked. Its
tool names, descriptions and categories come from the runner's manifests.

Icons are original vector drawings in code (`crates/ui-egui/src/icons.rs`). Fonts are not
bundled: the interface uses an installed system sans-serif when one is found, otherwise the
Ubuntu Light face that ships inside `epaint_default_fonts` (Ubuntu Font Licence 1.0).
