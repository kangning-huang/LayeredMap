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

### What if a GIS were designed the way Apple designs its apps?

QGIS and ArcGIS are remarkable tools. They can do almost anything with spatial data, and
professionals rely on them every day. But open one for the first time and you meet hundreds of
tools, a project to set up, and a coordinate system to choose before you've seen a single map.

So we asked a different question. Suppose the people who made the iPhone had been handed a GIS.
What would they keep? What would they hide? What would they refuse to ship? Surely not a toolbox
with a thousand buttons. More likely a window that shows your map the moment you open a file, and
a single click that tells you something true about it.

NimbleMap is our answer to that thought experiment. It isn't a smaller QGIS. It's a different
idea of who a map app is for: everyone who is curious about places, not only the people trained
to run GIS software. *(NimbleMap is an independent project, not affiliated with Apple, Esri or the
QGIS project.)*

### The map comes first

What would you see first? Not a wizard, not a project screen, not an empty toolbox. When you open
NimbleMap, you see a map. The interface stays quiet so your data can take center stage.

### It just opens

Why should opening a file need a manual? Drag in a CSV, a Shapefile, a GeoJSON or a GeoTIFF, and
it lands in the right place. The app works out the coordinates, the text encoding and the styling
for you. A million rows open in seconds, and the map stays smooth the whole time.

### Smart guesses, shown clearly

Files often leave things unsaid. Which columns are longitude and latitude? Is −9999 "no data"?
A good assistant doesn't interrogate you; it makes a sensible choice and tells you. NimbleMap says
so in one plain line, such as *"Located using lon and lat as GPS coordinates."* If it's wrong, one
click changes it. The app asks a question only when the answer truly matters.

### Answers before tools

Most GIS software hands you tools and leaves the question to you. NimbleMap starts with the
question people actually have: *what is this, and is it unusual?* Click a typhoon and the card
says its winds were stronger than 99% of storms on record. The tools are there when you need them,
one step behind the answer.

### Honest answers

Every number comes from your data, never from what happens to be drawn on screen. If some records
can't be placed, the app says how many and why. Your original files are never changed.

### Every step can be undone

Curiosity needs a safety net. Try anything: every action can be undone, and every result can
explain how it was made, in plain language.

### It feels like a map should

Panning, zooming and pinching are fast and fluid. We measure it: on our reference Mac, not one of
more than 20,000 test frames took longer than a sixtieth of a second.

## Status

NimbleMap is in early development, built in stages that each end with hands-on testing. A stage
is **Done** when that testing has signed it off.

| Stage | What it adds | |
|---|---|---|
| 0 | Foundations and test data | Done |
| 1 | A fast, fluid map with place search | Done |
| 2 | Open your files with one action; visible assumptions | Done |
| 3 | Click anything to see what it is and where it stands; filters, joins, the table | Built, in testing |
| 4 | The analysis engine: previews, result cards, recipes (it powers Pro's assistant) | Done |
| 5 | Save, export a finished map or a map package, work offline | Built, in testing |
| 6 | Polish: keyboard and VoiceOver, help pages, Windows | Built, in testing |
| 7 | NimbleMap Pro: ask questions of your layers in plain words | In test builds; accounts next |

Then: a signed public beta for macOS and Windows.

Details live in [`gates/`](gates/) and the full plan in [`docs/lm/spec.md`](docs/lm/spec.md).

## Try it

There are no published releases yet; build it yourself. You need [Rust](https://rustup.rs) 1.90
or newer, plus a C compiler for two bundled C libraries (SQLite and Zstandard):

- **macOS** (Apple silicon or Intel): the Xcode Command Line Tools (`xcode-select --install`).
- **Windows 10 or 11** (x64): the Visual Studio Build Tools with "Desktop development with C++",
  and Rust's default MSVC toolchain. Set two Git options **before cloning**; see
  [`docs/lm/windows.md`](docs/lm/windows.md) › Building.

```bash
git clone https://github.com/kangning-huang/LayeredMap.git
cd LayeredMap
cargo run -p layeredmap --release
```

To make an app, run `packaging/lm/bundle-macos.sh` on a Mac (it writes `dist/lm/NimbleMap.app`)
or `packaging/lm/windows/build-installer.ps1` on Windows (an installer; it needs Inno Setup 6).
Builds aren't signed yet, so the first time you open one on another computer, macOS asks you to
allow it in **System Settings › Privacy & Security** and Windows shows a SmartScreen warning.

NimbleMap is developed on macOS first; the same code builds and is tested on Windows. What
differs on Windows is in [`docs/lm/windows.md`](docs/lm/windows.md).

## For contributors

Read [`CONTRIBUTING.md`](CONTRIBUTING.md): how to build and test on macOS or Windows, what a
pull request needs, and what not to commit. In short:

- Product code lives in `crates/lm-*` and `apps/layeredmap`; the plan is
  [`docs/lm/spec.md`](docs/lm/spec.md) and choices made along the way are logged in
  [`docs/lm/decisions.md`](docs/lm/decisions.md).
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
