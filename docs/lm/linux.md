# NimbleMap on Linux (best effort)

Linux is best effort (`spec.md`): the same code as on macOS and Windows, shipped as a portable
tarball for x64, built in CI but not tested by hand before a release unless the owner does so
(see the Gate note below). This page is the counterpart of `windows.md`; the build files are in
`packaging/lm/linux/`.

## Layout

The tarball unpacks to one folder:

```
NimbleMap-<version>-<build>-linux-x64/
  layeredmap              the app
  Resources/              examples, help, tier, LICENSE, ATTRIBUTION*.md (Pro: reference/, account.json)
  README-linux.txt        how to run it, the libraries it needs, how to add a menu entry
  nimblemap.desktop       menu entry (optional)
  nimblemap-mime.xml      the .lmap type (optional)
  nimblemap.png           the icon (optional)
```

`Resources` sits beside the executable, as on Windows (`lm_core::platform::resources_dir`, `Os::Other`),
so the folder works wherever it's unpacked. Free and Pro are separate tarballs; Pro's
`Resources/tier` reads `pro`.

## What is different on Linux

| Area | Linux |
|---|---|
| Resources | `Resources/` beside `layeredmap` |
| Question log | `$XDG_DATA_HOME/layeredmap` (default `~/.local/share/layeredmap`) |
| Autosave | `$XDG_STATE_HOME/layeredmap/autosave` (default `~/.local/state/layeredmap/autosave`) |
| Logs, crash logs | `$XDG_STATE_HOME/layeredmap/logs` (default `~/.local/state/layeredmap/logs`) |
| Library cache | `$XDG_CACHE_HOME/layeredmap/library` (default `~/.cache/layeredmap/library`) |
| Window | Wayland or X11 (winit); app id and window class `app.layeredmap.desktop` (Pro: `app.layeredmap.pro`), matched by the desktop entry's `StartupWMClass` |
| Open and Save dialogs | Through `xdg-desktop-portal` |
| Open With | After the optional menu install: `.lmap` (`application/x-layeredmap-project`), GeoJSON, shapefiles, GeoPackage, KML, KMZ, GPX and NetCDF. TIFF, CSV, TSV and XLSX are left out: on Linux a listed type can become the app's default when nothing else claims it, and these are too general |
| Speaking (Pro) | Not available (no `lm-dictate` counterpart) |
| Signing | None; the sha256 is published with the download |

XDG variables that are empty or not absolute paths are ignored, as the XDG spec says. Builds
before the tarball used macOS-style folders under the home folder on Linux; they aren't moved.

## Building

```sh
# Ubuntu or Debian: the libraries winit, wgpu and the clipboard build against
sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxcb1-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libegl1-mesa-dev libgl1-mesa-dev libvulkan-dev

cargo run -p layeredmap                          # the app, from the checkout
packaging/lm/linux/build-tarball.sh              # dist/lm/NimbleMap-<v>-<build>-linux-x64.tar.gz
LM_TIER=pro packaging/lm/linux/build-tarball.sh  # NimbleMap-Pro-…
```

Environment variables: `LM_TIER`, `LM_BUILD_NUMBER`, `CARGO_TARGET_DIR`, and for Pro
`LM_REFERENCE_DATA` and `LM_ACCOUNT_CONFIG`, as on macOS and Windows (`packaging/lm/linux/README.md`).

## Running

Run-time libraries (most desktops have them): a Wayland or X11 session (`libwayland-client`, or
`libX11`, `libXcursor`, `libXi`, `libXrandr`), `libxkbcommon` (and `libxkbcommon-x11` on X11),
a Vulkan or OpenGL ES 3 driver (Mesa's `mesa-vulkan-drivers` and `libegl1`, or the GPU vendor's),
and `xdg-desktop-portal` for the file dialogs. On Debian or Ubuntu:

```sh
sudo apt install libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libegl1 mesa-vulkan-drivers xdg-desktop-portal
```

## CI

`.github/workflows/lm-ci.yml`, job `linux-tarball` on `ubuntu-latest`: installs the build
libraries, runs `lm-core`'s platform tests on Linux, builds the Free tarball and uploads it as
`NimbleMap-linux-x64`. The full `cargo xtask ci` suites run on macOS and Windows only.

## Gate note

No gate requires Linux. Before a Linux tarball is linked from the download page, the owner
smoke-tests it on one Wayland and one X11 desktop (for example Ubuntu 24.04 in each session):
unpack, launch, open an example, open and save a `.lmap`, use the Open dialog, quit and reopen
(autosave restored), Help › NimbleMap Help, and Help › Export Diagnostics… (the log is in
`~/.local/state/layeredmap/logs`). Then the optional menu install from `README-linux.txt`:
the entry appears with its icon, the running window groups under it, and a `.lmap`
double-clicked in the file manager opens in NimbleMap. Record the distribution, desktop, GPU and
driver in the report.

## Known gaps

- x64 only; no arm64 tarball is built (the script names one `linux-arm64` when run on arm64).
- No .deb, .rpm, Flatpak or AppImage, and no update feed entry for Linux.
- The menu entry is installed by hand (`README-linux.txt`); nothing removes it automatically.
- A second file opened from the file manager starts a second window, as on Windows.
