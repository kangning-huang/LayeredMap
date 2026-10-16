#!/usr/bin/env bash
# Builds LayeredMap.app (and, with signing credentials, signs, notarizes and staples it).
#
#   packaging/lm/bundle-macos.sh [out-dir]
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

app="$out/LayeredMap.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$target_dir/release/layeredmap" "$app/Contents/MacOS/layeredmap"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>LayeredMap</string>
  <key>CFBundleDisplayName</key><string>LayeredMap</string>
  <key>CFBundleIdentifier</key><string>app.layeredmap.desktop</string>
  <key>CFBundleExecutable</key><string>layeredmap</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${build}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.education</string>
  <!-- Open With (WP2.1): LayeredMap can open these, but never claims to be their default app
       (LSHandlerRank Alternate) and only views them (role Viewer). -->
  <key>CFBundleDocumentTypes</key>
  <array>
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
  </array>
</dict>
</plist>
PLIST

if [[ -n "${LM_SIGN_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp --sign "$LM_SIGN_IDENTITY" "$app"
  codesign --verify --strict --verbose=2 "$app"
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
  codesign --force --sign - "$app"
fi
ditto -c -k --keepParent "$app" "$out/LayeredMap-${version}-${build}-macos-arm64.zip"
echo "bundle-macos: $app"
