# NimbleMap on Windows (WP6.2)

Windows 10 (1809) and 11, x64. The app is the same code as on macOS; this page covers what is
different, how to build and test it, and the checklist for the owner's Gate 6 session on a
Windows laptop. Decisions are in `decisions.md` (2026-10-18), certificates in `signing.md`.

## What is different on Windows

| Area | macOS | Windows |
|---|---|---|
| Menu bar | The app's menu bar (NimbleMap, File, Edit, View, Window, Help) | A menu bar on the window: File (… Exit), Edit, View (… Full Screen F11), Help (… About). Ctrl in place of ⌘. The menu shows shortcuts; the keys themselves go through egui to the shell (`lm_ui::Shell::shortcuts`), so a text field still gets Ctrl+Z/C/V first. Ctrl+Y also redoes. |
| Shortcut labels | ⌘Z, ⇧⌘Z | Ctrl+Z, Ctrl+Shift+Z (`lm_ui::keys`) |
| Resources | `NimbleMap.app/Contents/Resources` | `Resources\` beside the exe (`lm_core::platform::resources_dir`) |
| Autosave | `~/Library/Application Support/LayeredMap/Autosave` | `%LOCALAPPDATA%\LayeredMap\Autosave` |
| Question log | `~/Library/Application Support/LayeredMap` | `%APPDATA%\LayeredMap` (roaming) |
| Logs, crash logs | `~/Library/Logs/LayeredMap` | `%LOCALAPPDATA%\LayeredMap\Logs` |
| Library cache | `~/Library/Caches/LayeredMap/library` | `%LOCALAPPDATA%\LayeredMap\Cache\library` |
| Region (country borders) | System Settings › Language & Region (`AppleLocale`) | Settings › Time & language › Language & region › **Country or region** (registry `HKCU\Control Panel\International\Geo\Name`), else the regional format |
| Update notes in Chinese | First preferred language | First preferred language (registry `User Profile\Languages`) |
| Update feed links | `download.free/pro` (.dmg) | `download_windows.free/pro` (installer), else `page` (`update-feed.md`) |
| Fonts | Bundled Inter, Noto Sans SC, Phosphor | The same, then Segoe UI, Segoe UI Semibold, Microsoft YaHei UI and Segoe UI Symbol from `%WINDIR%\Fonts` as fallbacks (never bundled) |
| Open With | Finder's Open With, Dock drops (`macos.rs`) | Explorer starts `layeredmap.exe "<file>"`; drops on the window come through winit |
| Speaking (Pro) | The `lm-dictate` helper | "Speaking isn't available on Windows yet. Press Windows+H to type with your voice instead." |
| Edited state | The window's dot and proxy icon | "— Edited" after the title |
| Reduce Motion | Read from Accessibility settings | Not read yet (animations stay on) |
| Assistant key (test builds) | Keychain, or `Resources/assistant.dat` | `OPENROUTER_API_KEY`, or `Resources\assistant.dat` |
| Minimum window | 1024×640 points | 960×540 points (a 1366×768 laptop at 125 %) |
| Saving | Atomic rename | Atomic rename, retried for half a second if a scanner or sync client holds the old file |

Scaling: winit makes the process per-monitor DPI aware (v2) and the embedded manifest
(`packaging/lm/windows/layeredmap.exe.manifest`) declares it, with long paths and UTF-8. egui lays
out in points, so 125 % and 150 % change the pixels per point, not the layout; the map is rendered
at the screen's own resolution (`the_map_renders_at_the_screens_resolution`). Exported images
don't depend on the screen at all (`lm_mapview::export`).

Touchpad: Windows reports a precision touchpad as wheel events, so the map reads them differently
there (`lm_mapview::wheel`): whole wheel notches zoom by half a level, fractional or sideways
scrolling pans, Ctrl + wheel (which is how Windows delivers a pinch to an app like this) zooms
continuously about the pointer, and Shift + wheel pans sideways.

## Building

### Setting up a Windows PC

1. **C++ build tools.** Install the Visual Studio 2022 Build Tools with the "Desktop development
   with C++" workload (or Visual Studio with the same workload). From a terminal:
   `winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`.
   SQLite and Zstandard are compiled from bundled C sources with it; nothing else native is needed.
2. **Rust.** `winget install Rustlang.Rustup`, then in a new terminal `rustup default stable-msvc`
   (Rust 1.90 or newer; `rustup update` if older).
3. **Git, set before cloning.** `winget install Git.Git`, then:

   ```powershell
   git config --global core.autocrlf false   # keep the repository's LF line endings
   git config --global core.longpaths true   # allow paths over Windows' 260-character limit
   git clone https://github.com/kangning-huang/LayeredMap.git
   ```

   These are the settings CI uses; a checkout made with Git for Windows' default
   (`autocrlf true`) rewrites line endings and isn't what CI tests. If you cloned before
   changing them, clone again.
4. **Inno Setup 6**, only to build the installer: `winget install JRSoftware.InnoSetup`.

The first build takes 10–20 minutes; later ones are incremental. Keep the clone on a local drive,
not in OneDrive, which slows the build and can lock files.

### Commands

```powershell
cargo xtask ci                                         # the same suites as on macOS
cargo run -p layeredmap                                # the app
pwsh packaging/lm/windows/build-installer.ps1          # dist\lm\NimbleMap-<v>-<build>-windows-x64-setup.exe
$env:LM_TIER = "pro"; pwsh packaging/lm/windows/build-installer.ps1   # NimbleMap-Pro-…
```

Pixel snapshots don't run on Windows (they are `#[ignore]` off macOS), so a change that alters
what the interface looks like needs its snapshots re-approved on a Mac: say so in the pull
request and a maintainer will do it (`CONTRIBUTING.md`).

The installer (`packaging/lm/windows/layeredmap.iss`):

- installs for the current user without administrator rights (`%LOCALAPPDATA%\Programs\NimbleMap`);
  "Install for all users" in the first page needs administrator rights;
- installs Free and Pro side by side (own AppId, folder, Start menu entry, `layeredmap.exe` /
  `layeredmap-pro.exe`, and ProgIDs `LayeredMap.*` / `LayeredMapPro.*`);
- adds a Start menu entry, and a desktop icon if ticked;
- makes `.lmap` NimbleMap's own type (the tier installed last is its default) and offers
  NimbleMap in Open With for `.geojson .shp .gpkg .kml .kmz .gpx .nc .nc4 .cdf .tif .tiff .csv
  .tsv .xlsx`, never as their default (as `LSHandlerRank Alternate` on macOS); `.json` and `.zip`
  are left out on Windows as too general;
- carries the examples, the tier file, the licences and, for Pro, the reference layers
  (`LM_REFERENCE_DATA`), like `bundle-macos.sh`;
- is in English, and in Chinese when Inno Setup has its Chinese Simplified messages (6.5 and later);
- is signed when `LM_SIGNTOOL_ARGS` is set (`signing.md`); otherwise unsigned.

The exe's icon is `packaging/lm/windows/layeredmap.ico` (16–256 px, the macOS artwork on its
rounded tile; a tile-less Windows variant is the owner's call), made with the macOS icon by
`tools/lm/make_app_icon.py`; `apps/layeredmap/build.rs` embeds it with the version information and
the manifest when the target is Windows.

From a Mac (development only): `rustup target add x86_64-pc-windows-msvc`, `cargo install
cargo-xwin`, then `cargo xwin clippy --target x86_64-pc-windows-msvc -p layeredmap …` checks and
`cargo xwin build --target x86_64-pc-windows-msvc -p layeredmap` links (it downloads Microsoft's
CRT and SDK headers once). The result can't be run or tested on the Mac.

The approved images for 125 % and 150 % are `crates/lm-ui/tests/snapshots/scale125_*.png` and
`scale150_*.png` (`screenshots::scaling`): first launch, an object card (also on a 1366×768
screen at 125 %) and the export sheet, in the window a 1920×1080 laptop gives at each scale.

## CI

`.github/workflows/lm-ci.yml`:

- `windows`: `cargo xtask ci` on `windows-latest` (fmt, clippy `-D warnings`, every test, the
  layer check). Pixel snapshots are `#[ignore]` off macOS (the approved images come from the Mac's
  GPU and text rasteriser); everything else runs, including the 125 %/150 % map-resolution test,
  the save kill test (with `TerminateProcess`) and the offline harness.
- `windows-installer`: the Free installer, uploaded as `NimbleMap-windows-x64`. Signed with Azure
  Artifact Signing when the `LM_ARTIFACT_SIGNING_*` and `LM_AZURE_*` secrets exist, unsigned
  otherwise.

## Gate 6: the Windows laptop session

Use the low-end reference laptop (8 GB RAM, integrated graphics). Record the model, CPU, memory,
GPU, screen size and resolution, and the Windows version (Settings › System › About) in the gate
report. Work in a fresh Windows user account for the first part.

**Install and SmartScreen**
1. Download the installer from the download host as a student would. Note whether Edge or
   SmartScreen warns ("Windows protected your PC"), and what it says. Unsigned or new-certificate
   builds will warn: record the exact screens, since they go into the quickstart.
2. Install without administrator rights. The Start menu has NimbleMap; it opens with its icon in
   the taskbar and in Alt+Tab.
3. Install Pro too: both are in the Start menu and run at the same time; Settings › Apps lists both.
4. Uninstall Free: Pro still opens files; `.lmap` files still open (in Pro).

**Scaling (Settings › System › Display › Scale)**
5. At 100 %, 125 % and 150 %, and at the screen's recommended setting: text is sharp (not blurry),
   icons are centred in their buttons, nothing is cut off in the layer list, the inspector, the
   object card, the export sheet and the Ask box (Pro). Take a screenshot of each.
6. With an external monitor at a different scale, drag the window between the screens: it
   re-scales once as it crosses, without a blurry frame staying.
7. On a 1366×768 screen at 125 % (or the laptop's smallest window), the layer list, card and
   export sheet still fit; the window can be made that small. In the snapshots the object card's
   last line (View in table · Zoom to) and, at 150 % on 1920×1080, the export sheet's Legend
   section are below the fold and reached by scrolling: check that a student finds them.
8. First launch on a 1920×1080 screen at 150 %: the window fits above the taskbar (eframe clamps
   it to the monitor, not the work area; if it reaches under the taskbar, note it).

**Precision touchpad** (needs a real device: Windows sends no touchpad events to a Mac)
9. Two-finger scroll pans the map smoothly in all four directions, at a comfortable speed (not too
   fast, not sluggish). Scrolling in the layer list scrolls the list, not the map.
10. Pinch zooms smoothly about the fingers, without jumps; pinching in a panel does nothing to the map.
11. With a mouse: each wheel notch zooms half a level about the pointer; Shift+wheel pans sideways.
    If a "smooth scrolling" mouse pans instead of zooming, note its model.
12. Tap to click, two-finger tap for the context menu on a layer row, double-tap to zoom in.

**File associations**
13. Double-click a `.lmap`: it opens in NimbleMap. Right-click a `.geojson`, `.shp`, `.gpkg`,
    `.kml`, `.kmz`, `.gpx`, `.nc`, `.csv` and `.xlsx` › Open with: NimbleMap is listed, but none of
    them changed their default app.
14. With NimbleMap already open, double-click another file: note what happens (a second window
    opens today; a single-instance hand-off is not built yet).
15. Drag files from Explorer onto the map: they open. A file in a folder with Chinese characters
    and spaces, and one with a path over 260 characters, open too.

**Chinese**
16. Switch Windows to Chinese (Settings › Time & language › Language: 中文(简体) first, and Country
    or region: China). Restart NimbleMap. Repeat assignment 1. Chinese text in the interface, the
    layer names and attribute values is clear at 125 % and 150 %, with no boxes (□) and no mixed-up
    glyphs. Country borders follow China's point of view (Pro, reference layers).
17. Type Chinese with the Microsoft Pinyin IME in the search box and the Ask box: the candidate
    window appears next to the text, and the chosen words go in.
18. Pro: in the Ask box, press Windows+H and speak. The words appear in the box. Clicking the
    microphone shows the "isn't available on Windows yet" line.

**The Gate 5 rehearsal on Windows**
19. Repeat the rehearsal (spec › Gate 5): all three pilot assignments, save, quit and reopen,
    move a source file and relink it; end the task in Task Manager mid-task, reopen and restore;
    export a slide image and an A4 image; turn off Wi-Fi before launch and repeat assignment 1.
20. Check `%LOCALAPPDATA%\LayeredMap\Logs` for the log and any crash log; Help › Export
    Diagnostics… names Windows and its version.

**Stress (low-end laptop)**: rerun the earlier stress tests (spec › Gate 6) and note frame times
with View › Show Frame Rate.

## Known gaps

- Speaking in Pro uses Windows' own voice typing; no helper like `lm-dictate` yet.
- A second file opened from Explorer starts a second window (no single-instance hand-off).
- Reduce Motion (Settings › Accessibility › Visual effects › Animation effects) isn't read yet.
- The Credential Manager isn't read for a developer's assistant key (`OPENROUTER_API_KEY` instead).
- Narrator is WP6.3.
- ARM64 Windows (Snapdragon laptops) runs the x64 build under emulation. A native
  `aarch64-pc-windows-msvc` build isn't packaged and wasn't checked: from a Mac, `ring`'s C code
  doesn't build for that target with cargo-xwin (a toolchain issue, not NimbleMap's code).
