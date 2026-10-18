#!/usr/bin/env bash
# Builds LayeredMap.app (and, with signing credentials, signs, notarizes and staples it).
#
#   packaging/lm/bundle-macos.sh [out-dir]
#   LM_TIER=pro packaging/lm/bundle-macos.sh [out-dir]   # LayeredMap Pro.app
#
# Signing runs only when these are set (CI secrets; see docs/lm/signing.md):
#   LM_SIGN_IDENTITY   "Developer ID Application: <name> (<team id>)" — in the keychain
#   LM_NOTARY_APPLE_ID, LM_NOTARY_TEAM_ID, LM_NOTARY_PASSWORD (an app-specific password)
# Without them the bundle is ad-hoc signed, which runs locally but warns on other Macs.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:-$root/dist/lm}"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
build="${LM_BUILD_NUMBER:-0}"

cargo build --release --locked -p layeredmap --manifest-path "$root/Cargo.toml"

# The tier is the app (until accounts, Phase 7): Pro is its own bundle with its own id, so both
# can be installed side by side; the app reads Contents/Resources/tier.
tier="${LM_TIER:-free}"
if [[ "$tier" == "pro" ]]; then
  name="LayeredMap Pro"; bundle_id="app.layeredmap.pro"
else
  name="LayeredMap"; bundle_id="app.layeredmap.desktop"
fi
app="$out/$name.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$target_dir/release/layeredmap" "$app/Contents/MacOS/layeredmap"
# The Examples menu (WP2.9): the app looks in Contents/Resources/examples.
mkdir -p "$app/Contents/Resources/examples"
rsync -a --exclude '._*' "$root/assets/examples/" "$app/Contents/Resources/examples/"
# The app icon (the Strata logo; tools/lm/make_app_icon.py builds it from design/logo).
cp "$root/packaging/lm/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
echo "$tier" > "$app/Contents/Resources/tier"
# Licences (Gate 5 licence audit): the assets' sources and every Rust crate's licence text.
cp "$root/LICENSE" "$root/ATTRIBUTION.md" "$app/Contents/Resources/"
[[ -f "$root/ATTRIBUTION-crates.md" ]] && cp "$root/ATTRIBUTION-crates.md" "$app/Contents/Resources/"
if [[ "$tier" == "pro" ]]; then
  # The Ask box's microphone (WP7.1): Apple's speech recognizer, in a small Swift helper.
  # It carries its own Info.plist (usage texts), as macOS asks of any process that listens.
  dictate_plist="$(mktemp -t lm-dictate-plist)"
  cat > "$dictate_plist" <<DPLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>${bundle_id}.dictate</string>
  <key>CFBundleName</key><string>${name}</string>
  <key>NSMicrophoneUsageDescription</key><string>LayeredMap Pro listens when you click the microphone in the Ask box, to turn your request into text.</string>
  <key>NSSpeechRecognitionUsageDescription</key><string>LayeredMap Pro turns what you say in the Ask box into text, on this Mac when it can.</string>
</dict></plist>
DPLIST
  swiftc -O -o "$app/Contents/MacOS/lm-dictate" "$root/packaging/lm/dictate.swift" \
    -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist -Xlinker "$dictate_plist"
  rm -f "$dictate_plist"
  # Built-in reference layers (Natural Earth 1:10m, WorldClim elevation; hidden, used by the
  # assistant): tools/lm/reference/build_reference.py writes them to LM_REFERENCE_DATA.
  ref="${LM_REFERENCE_DATA:-/Volumes/LaCie/data/layeredmap-reference}"
  if [[ -f "$ref/manifest.json" ]]; then
    mkdir -p "$app/Contents/Resources/reference"
    rsync -a --exclude raw --exclude '._*' "$ref/" "$app/Contents/Resources/reference/"
  else
    echo "bundle-macos: no reference data at $ref (built-in layers left out)" >&2
  fi
  # Test builds: the owner's OpenRouter key from this Mac's keychain, built into the bundle
  # (scrambled, not secret: anyone determined can recover it). Never in the repo. The release
  # talks to LayeredMap's assistant service instead (WP7.3). LM_NO_KEY=1 leaves it out.
  if [[ -z "${LM_NO_KEY:-}" ]]; then
    python3 - "$app/Contents/Resources/assistant.dat" <<'PY'
import subprocess, sys
key = subprocess.run(["/usr/bin/security", "find-generic-password", "-s", "openrouter", "-a", "layeredmap", "-w"],
                     capture_output=True, text=True).stdout.strip()
if not key:
    sys.exit("bundle-macos: no OpenRouter key in the keychain (service openrouter, account layeredmap); LM_NO_KEY=1 to build without")
pad = b"LayeredMap Pro test build: not a secret store."
data = bytes(b ^ pad[i % len(pad)] for i, b in enumerate(key.encode()))
open(sys.argv[1], "w").write(data.hex())
print("bundle-macos: assistant key built in")
PY
  fi
fi
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>${name}</string>
  <key>CFBundleDisplayName</key><string>${name}</string>
  <key>CFBundleIdentifier</key><string>${bundle_id}</string>
  <key>CFBundleExecutable</key><string>layeredmap</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${build}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>LayeredMap Pro listens when you click the microphone in the Ask box, to turn your request into text.</string>
  <key>NSSpeechRecognitionUsageDescription</key><string>LayeredMap Pro turns what you say in the Ask box into text, on this Mac when it can.</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.education</string>
  <!-- Open With (WP2.1): LayeredMap can open data files, but never claims to be their default
       app (LSHandlerRank Alternate) and only views them (role Viewer). -->
  <key>CFBundleDocumentTypes</key>
  <array>
    <!-- LayeredMap's own map files (WP5.1): its type, opened for editing. -->
    <dict>
      <key>CFBundleTypeName</key><string>LayeredMap map</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Owner</string>
      <key>LSItemContentTypes</key><array><string>app.layeredmap.map</string></array>
      <key>CFBundleTypeExtensions</key><array><string>lmap</string></array>
      <key>CFBundleTypeIconFile</key><string>AppIcon</string>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>CSV or TSV table</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>public.comma-separated-values-text</string><string>public.tab-separated-values-text</string></array>
      <key>CFBundleTypeExtensions</key><array><string>csv</string><string>tsv</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>GeoJSON</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>org.geojson.geojson</string><string>public.json</string></array>
      <key>CFBundleTypeExtensions</key><array><string>geojson</string><string>json</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Shapefile</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>org.esri.shapefile</string><string>public.zip-archive</string></array>
      <key>CFBundleTypeExtensions</key><array><string>shp</string><string>zip</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>GeoTIFF</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>public.tiff</string></array>
      <key>CFBundleTypeExtensions</key><array><string>tif</string><string>tiff</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>GeoPackage</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>org.opengeospatial.geopackage</string></array>
      <key>CFBundleTypeExtensions</key><array><string>gpkg</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Excel workbook</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>org.openxmlformats.spreadsheetml.sheet</string></array>
      <key>CFBundleTypeExtensions</key><array><string>xlsx</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>KML or KMZ</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>com.google.earth.kml</string><string>com.google.earth.kmz</string></array>
      <key>CFBundleTypeExtensions</key><array><string>kml</string><string>kmz</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>GPX track</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>com.topografix.gpx</string></array>
      <key>CFBundleTypeExtensions</key><array><string>gpx</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>NetCDF grid</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>edu.ucar.unidata.netcdf</string></array>
      <key>CFBundleTypeExtensions</key><array><string>nc</string><string>nc4</string><string>cdf</string></array>
    </dict>
  </array>
  <!-- The .lmap type, which LayeredMap owns and declares (docs/lm/format-v1.md). -->
  <key>UTExportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>app.layeredmap.map</string>
      <key>UTTypeDescription</key><string>LayeredMap map</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string><string>public.content</string></array>
      <key>UTTypeIconFile</key><string>AppIcon</string>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>lmap</string></array><key>public.mime-type</key><string>application/vnd.layeredmap.map+zip</string></dict>
    </dict>
  </array>
  <!-- Types macOS has no system UTI for. Imported, not exported: LayeredMap does not own them. -->
  <key>UTImportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>org.geojson.geojson</string>
      <key>UTTypeDescription</key><string>GeoJSON</string>
      <key>UTTypeConformsTo</key><array><string>public.json</string></array>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>geojson</string></array><key>public.mime-type</key><string>application/geo+json</string></dict>
    </dict>
    <dict>
      <key>UTTypeIdentifier</key><string>org.esri.shapefile</string>
      <key>UTTypeDescription</key><string>Esri Shapefile</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string></array>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>shp</string></array></dict>
    </dict>
    <dict>
      <key>UTTypeIdentifier</key><string>org.opengeospatial.geopackage</string>
      <key>UTTypeDescription</key><string>GeoPackage</string>
      <key>UTTypeConformsTo</key><array><string>public.database</string><string>public.data</string></array>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>gpkg</string></array><key>public.mime-type</key><string>application/geopackage+sqlite3</string></dict>
    </dict>
    <dict>
      <key>UTTypeIdentifier</key><string>com.google.earth.kml</string>
      <key>UTTypeDescription</key><string>KML</string>
      <key>UTTypeConformsTo</key><array><string>public.xml</string></array>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>kml</string></array><key>public.mime-type</key><string>application/vnd.google-earth.kml+xml</string></dict>
    </dict>
    <dict>
      <key>UTTypeIdentifier</key><string>com.google.earth.kmz</string>
      <key>UTTypeDescription</key><string>KMZ</string>
      <key>UTTypeConformsTo</key><array><string>public.zip-archive</string></array>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>kmz</string></array><key>public.mime-type</key><string>application/vnd.google-earth.kmz</string></dict>
    </dict>
    <dict>
      <key>UTTypeIdentifier</key><string>com.topografix.gpx</string>
      <key>UTTypeDescription</key><string>GPX</string>
      <key>UTTypeConformsTo</key><array><string>public.xml</string></array>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>gpx</string></array><key>public.mime-type</key><string>application/gpx+xml</string></dict>
    </dict>
    <dict>
      <key>UTTypeIdentifier</key><string>edu.ucar.unidata.netcdf</string>
      <key>UTTypeDescription</key><string>NetCDF</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string></array>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>nc</string><string>nc4</string><string>cdf</string></array><key>public.mime-type</key><string>application/x-netcdf</string></dict>
    </dict>
  </array>
</dict>
</plist>
PLIST

if [[ -n "${LM_SIGN_IDENTITY:-}" ]]; then
  # Pro's microphone needs the audio-input entitlement under the hardened runtime.
  ent=()
  if [[ "$tier" == "pro" ]]; then
    ent=(--entitlements "$root/packaging/lm/pro.entitlements")
    codesign --force --options runtime --timestamp "${ent[@]}" --sign "$LM_SIGN_IDENTITY" "$app/Contents/MacOS/lm-dictate"
  fi
  codesign --force --options runtime --timestamp "${ent[@]}" --sign "$LM_SIGN_IDENTITY" "$app"
  codesign --verify --strict --verbose=2 "$app"
  # The hardened runtime (WP5.7), which notarization requires: the signature must carry it.
  if ! codesign --display --verbose=2 "$app" 2>&1 | grep -q 'flags=.*runtime'; then
    echo "bundle-macos: $app is signed without the hardened runtime" >&2
    exit 1
  fi
  if [[ -n "${LM_NOTARY_APPLE_ID:-}" ]]; then
    zip="$out/LayeredMap-notarize.zip"
    ditto -c -k --keepParent "$app" "$zip"
    xcrun notarytool submit "$zip" --apple-id "$LM_NOTARY_APPLE_ID" --team-id "$LM_NOTARY_TEAM_ID" \
      --password "$LM_NOTARY_PASSWORD" --wait
    xcrun stapler staple "$app"
    xcrun stapler validate "$app"
    spctl --assess --type execute --verbose "$app"
    rm -f "$zip"
  fi
else
  echo "bundle-macos: no LM_SIGN_IDENTITY; ad-hoc signing (will warn on other Macs)" >&2
  [[ -f "$app/Contents/MacOS/lm-dictate" ]] && codesign --force --sign - "$app/Contents/MacOS/lm-dictate"
  codesign --force --sign - "$app"
fi
ditto -c -k --keepParent "$app" "$out/${name// /-}-${version}-${build}-macos-arm64.zip"
echo "bundle-macos: $app"
