#!/usr/bin/env bash
# Builds the portable NimbleMap tarball for Linux (best effort, docs/lm/linux.md), the counterpart
# of bundle-macos.sh and windows/build-installer.ps1. Run on Linux with Rust and the build
# libraries listed in packaging/lm/linux/README.md:
#
#   packaging/lm/linux/build-tarball.sh [out-dir]
#   LM_TIER=pro packaging/lm/linux/build-tarball.sh [out-dir]   # NimbleMap Pro
#
# Writes NimbleMap-<version>-<build>-linux-<arch>.tar.gz (or NimbleMap-Pro-...) to out-dir,
# dist/lm by default. The tarball holds one folder: the layeredmap executable with Resources beside
# it (lm_core::platform::resources_dir), a README, and a desktop entry, MIME type and icon for
# anyone who wants NimbleMap in their application menu. None of those are needed to run it.
#
# Environment variables, as on macOS and Windows: LM_TIER (free or pro), LM_BUILD_NUMBER,
# CARGO_TARGET_DIR, and for Pro LM_REFERENCE_DATA (the built-in reference layers) and
# LM_ACCOUNT_CONFIG (the sign-in service's config). Nothing is signed: Linux has no counterpart of
# Developer ID or Authenticode that users check; the sha256 printed at the end is what to publish.
set -euo pipefail

# The display name in the tarball's name; keep in step with lm_core::brand (docs/lm/naming.md).
# The executable keeps its layeredmap name.
brand="NimbleMap"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "build-tarball: run this on Linux (on a Mac use packaging/lm/build-dmg.sh)" >&2
  exit 1
fi

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
out="${1:-$root/dist/lm}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
build="${LM_BUILD_NUMBER:-0}"
tier="${LM_TIER:-free}"
[[ "$tier" == "pro" ]] || tier="free"
# Architecture names as in the other downloads (macos-arm64, windows-x64).
case "$(uname -m)" in
  x86_64 | amd64) arch="x64" ;;
  aarch64 | arm64) arch="arm64" ;;
  *) arch="$(uname -m)" ;;
esac

cargo build --release --locked -p layeredmap --manifest-path "$root/Cargo.toml"

# The staged folder is the tarball's folder: the executable, and Resources beside it.
stage="$out/stage-linux-$tier"
rm -rf "$stage"
res="$stage/Resources"
mkdir -p "$res"
cp "$target_dir/release/layeredmap" "$stage/layeredmap"
chmod 755 "$stage/layeredmap"
# The Examples menu (WP2.9): Resources/examples.
cp -R "$root/assets/examples" "$res/examples"
# Help › NimbleMap Help (WP6.10): the help pages, read from disk so they work offline.
cp -R "$root/web/lm" "$res/help"
rm -f "$res/help/README.md"
echo "$tier" > "$res/tier"
# Licences (Gate 5 licence audit): the assets' sources and every Rust crate's licence text.
cp "$root/LICENSE" "$root/ATTRIBUTION.md" "$res/"
[[ -f "$root/ATTRIBUTION-crates.md" ]] && cp "$root/ATTRIBUTION-crates.md" "$res/"

if [[ "$tier" == "pro" ]]; then
  # Built-in reference layers (tools/lm/reference/build_reference.py), as in bundle-macos.sh.
  # There is no default folder on Linux: set LM_REFERENCE_DATA.
  ref="${LM_REFERENCE_DATA:-}"
  if [[ -n "$ref" && -f "$ref/manifest.json" ]]; then
    mkdir -p "$res/reference"
    cp -R "$ref/." "$res/reference/"
    rm -rf "$res/reference/raw"
  else
    echo "build-tarball: no reference data at '${ref}' (built-in layers left out)" >&2
  fi
  # Pro's account config (docs/lm/accounts-design.md), as in bundle-macos.sh.
  if [[ -n "${LM_ACCOUNT_CONFIG:-}" ]]; then
    cp "$LM_ACCOUNT_CONFIG" "$res/account.json"
    echo "build-tarball: account config from $LM_ACCOUNT_CONFIG"
  fi
fi
# AppleDouble files, when the checkout or the data sits on an exFAT drive.
find "$stage" -name '._*' -delete

# For a menu entry (README-linux.txt says how): the desktop entry, the .lmap type and the icon.
# Pro's entry has its own name and window class (the app id in apps/layeredmap/src/main.rs).
if [[ "$tier" == "pro" ]]; then
  sed -e "s/^Name=$brand\$/Name=$brand Pro/" -e "s/^StartupWMClass=.*/StartupWMClass=app.layeredmap.pro/" \
    "$here/nimblemap.desktop" > "$stage/nimblemap.desktop"
else
  cp "$here/nimblemap.desktop" "$stage/nimblemap.desktop"
fi
cp "$here/nimblemap-mime.xml" "$stage/"
# The Strata logo, the same picture as the window icon (tools/lm/make_app_icon.py).
[[ -f "$root/assets/icon/icon-256.png" ]] && cp "$root/assets/icon/icon-256.png" "$stage/nimblemap.png"

# Free and Pro each get their own menu entry, so both can be installed.
menu_name="$brand"; desktop_id="nimblemap.desktop"
[[ "$tier" == "pro" ]] && menu_name="$brand Pro" && desktop_id="nimblemap-pro.desktop"
cat > "$stage/README-linux.txt" <<README
$brand $version (build $build) for Linux, $arch$([[ "$tier" == "pro" ]] && echo ", Pro")

Linux is best effort: NimbleMap is made and tested on macOS and Windows.

Run
  Unpack the folder anywhere you can write to, and run ./layeredmap in it.
  Keep the Resources folder beside layeredmap: the examples, help and licences are in it.
  To open a file: ./layeredmap path/to/map.lmap

Needs (installed on most desktops already)
  - a Wayland or X11 desktop: libwayland-client, or libX11, libXcursor, libXi and libXrandr
  - libxkbcommon (and libxkbcommon-x11 on X11)
  - a GPU driver with Vulkan or OpenGL ES 3: Mesa (mesa-vulkan-drivers, libegl1) or the vendor's
  - xdg-desktop-portal, for the Open and Save dialogs
  On Debian or Ubuntu:
    sudo apt install libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libegl1 \\
      mesa-vulkan-drivers xdg-desktop-portal
  On Fedora:
    sudo dnf install libxkbcommon libxkbcommon-x11 libwayland-client mesa-vulkan-drivers \\
      mesa-libEGL xdg-desktop-portal

Add it to the application menu (optional)
  From this folder, after moving it where it will stay:
    mkdir -p ~/.local/share/applications
    sed "s|^Exec=layeredmap|Exec=\"\$PWD/layeredmap\"|" nimblemap.desktop \\
      > ~/.local/share/applications/$desktop_id
    install -Dm644 nimblemap.png ~/.local/share/icons/hicolor/256x256/apps/nimblemap.png
    xdg-mime install --novendor nimblemap-mime.xml
    update-desktop-database ~/.local/share/applications
  $menu_name then appears in the menu, opens .lmap files, and is offered in Open With for
  GeoJSON, shapefiles, GeoPackage, KML, KMZ, GPX and NetCDF.
  To remove it, delete ~/.local/share/applications/$desktop_id.

Your files
  Maps you save stay where you save them. NimbleMap's own folders:
    autosaves  \${XDG_STATE_HOME:-~/.local/state}/layeredmap/autosave
    logs       \${XDG_STATE_HOME:-~/.local/state}/layeredmap/logs
    other      \${XDG_DATA_HOME:-~/.local/share}/layeredmap
    downloads  \${XDG_CACHE_HOME:-~/.cache}/layeredmap
  Help > Export Diagnostics... collects the log for a bug report.

Licences: Resources/LICENSE and Resources/ATTRIBUTION*.md.
README

prefix="$brand"; [[ "$tier" == "pro" ]] && prefix="$brand-Pro"
pkg="$prefix-$version-$build-linux-$arch"
tarball="$out/$pkg.tar.gz"
rm -f "$tarball"
# The files go into a folder named like the tarball. GNU tar and bsdtar spell the rename differently.
if tar --version 2>/dev/null | grep -q "GNU tar"; then
  tar -C "$stage" --owner=0 --group=0 --transform "s|^\.|$pkg|" -czf "$tarball" .
else
  tar -C "$stage" -s "|^\.|$pkg|" -czf "$tarball" .
fi
hash="$( (sha256sum "$tarball" 2>/dev/null || shasum -a 256 "$tarball") | cut -d' ' -f1)"
echo "build-tarball: $tarball"
echo "build-tarball: sha256 $hash"
