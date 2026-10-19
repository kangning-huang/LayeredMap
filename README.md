# NimbleMap

**See how places relate.**

*NimbleMap is a working name (formerly LayeredMap; the trademark check is pending). The repository,
crates and the `layeredmap` binary keep the old name; see [`docs/lm/naming.md`](docs/lm/naming.md).*

NimbleMap is a map app for anyone curious about spatial data. Open a file, and it appears on
the map. Open another, and it lands on top. Schools over districts. Districts over a
temperature map. Instantly, you can see how they relate.

No setup. No toolboxes. No coordinate systems to learn first. Just your data, on a map,
where it belongs.

## Design philosophy

We believe a great map app should be simple to start and powerful when you need it. Every
decision in NimbleMap comes from a few ideas.

### The map comes first

When you open NimbleMap, you see a map. Not a wizard, not a project screen, not an empty
toolbox. The interface stays quiet and out of the way, so your data can take center stage.

### It just opens

Drag in a CSV, a Shapefile, a GeoJSON or a GeoTIFF, and it lands in the right place. The app
works out the coordinates, the text encoding and the styling for you. A million rows open in
seconds, and the map stays smooth the whole time.

### Smart guesses, shown clearly

Files often leave things unsaid. Which columns are longitude and latitude? Is −9999 "no data"?
NimbleMap makes a sensible choice and tells you in one plain line, such as *"Located using lon
and lat as GPS coordinates."* If it's wrong, one click changes it. The app asks a question only
when the answer truly matters, and never more than it has to.

### Honest answers

Every number comes from your data, never from what happens to be drawn on screen. If some
records can't be placed, the app says how many and why. Your original files are never changed.

### Every step can be undone

Try anything. Every action can be undone, and every result can explain how it was made, in plain
language.

### It feels like a map should

Panning, zooming and pinching are fast and fluid. We measure it: on our reference Mac, not one of
more than 20,000 test frames took longer than a sixtieth of a second.

## Status

NimbleMap is in early development, built in stages that each end with hands-on testing.

| Stage | What it adds | |
|---|---|---|
| 0 | Foundations and test data | Done |
| 1 | A fast, fluid map with place search | Done |
| 2 | Open your files with one action; visible assumptions | In progress |
| 3 | Click anything to see what it is and where it stands | Planned |
| 4 | Ask simple questions of your layers and keep the answers | Planned |
| 5 | Save, share and a classroom pilot | Planned |
| 6 | Polish, Windows, and a public beta | Planned |

Details live in [`gates/`](gates/) and the full plan in [`docs/lm/spec.md`](docs/lm/spec.md).

## Try it

There are no published releases yet. To build it yourself you need
[Rust](https://rustup.rs) 1.90 or newer and, on a Mac, the Xcode Command Line Tools.

```bash
git clone https://github.com/kangning-huang/LayeredMap.git
cd LayeredMap
cargo run -p layeredmap --release
```

To make a Mac app, run `packaging/lm/bundle-macos.sh`; it writes `dist/lm/NimbleMap.app`.
Builds aren't notarized yet, so the first time you open one on another Mac, allow it in **System
Settings › Privacy & Security**.

NimbleMap is developed on macOS first. Windows support is planned, and the code is
cross-platform Rust.

## For contributors

- Start with [`AGENTS.md`](AGENTS.md) and the plan in [`docs/lm/spec.md`](docs/lm/spec.md).
- Product code lives in `crates/lm-*` and `apps/layeredmap`; choices made along the way are
  logged in [`docs/lm/decisions.md`](docs/lm/decisions.md).
- Run `cargo xtask ci` before every commit. It checks formatting, lints, tests and the rules
  that keep the interface from doing spatial math itself.
- Real files that open badly, and wording that confuses, are the most useful reports.

## Credits and license

NimbleMap is built on [RustGIS](https://github.com/opengeos/RustGIS), the open-source GIS engine
from the [opengeos](https://github.com/opengeos) project. Its data readers, rendering and
geoprocessing make NimbleMap possible; see [`UPSTREAM.md`](UPSTREAM.md).

Map data © OpenStreetMap contributors, through Protomaps. Offline basemap from Natural Earth.
Place search from GeoNames (CC BY 4.0). Fonts: Inter and Noto Sans SC (SIL Open Font License).
Icons: Phosphor (MIT). More in [`ATTRIBUTION.md`](ATTRIBUTION.md) and
[`fixtures/LICENSES.md`](fixtures/LICENSES.md).

Licensed under the [Apache License 2.0](LICENSE).
