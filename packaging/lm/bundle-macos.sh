#!/usr/bin/env bash
# Builds NimbleMap.app (and, with signing credentials, signs, notarizes and staples it).
#
#   packaging/lm/bundle-macos.sh [out-dir]
#
# One app: Pro's features unlock when someone signs in (docs/lm/accounts-design.md › Entitlement).
#
# Signing runs only when these are set (CI secrets; see docs/lm/signing.md):
#   LM_SIGN_IDENTITY   "Developer ID Application: <name> (<team id>)" — in the keychain
#   LM_NOTARY_PROFILE  a notarytool keychain profile (xcrun notarytool store-credentials), on a Mac;
#   or LM_NOTARY_APPLE_ID, LM_NOTARY_TEAM_ID, LM_NOTARY_PASSWORD (an app-specific password), in CI
# Without them the bundle is ad-hoc signed, which runs locally but warns on other Macs.
#   LM_SIGNING_KEYCHAIN  the keychain holding the Developer ID identity (default
#                     ~/Library/Keychains/nimblemap-signing.keychain-db on the owner's Mac). When it
#                     exists and LM_SIGN_IDENTITY is set, it is unlocked first with its password from
#                     the login keychain (service nimblemap-signing-keychain, account developer-id);
#                     the password is never printed or written anywhere.
#
# Updates (WP5.7, docs/lm/update-feed.md): the feed addresses are compiled in by default:
#   LM_UPDATE_FEED_URL           default https://nimblemap.com/update/feed.json
#   LM_UPDATE_FEED_FALLBACK_URL  default the same file in the GitCode mirror (mainland China)
# Set either to "none" to leave it out (a build with neither never checks).
#   LM_CARGO_FEATURES  extra cargo features, for the update dry run only (update-test-hook);
#                     refused unless the version in Cargo.toml contains "-test".
set -euo pipefail

# The display name (bundle, menus, Finder); keep in step with lm_core::brand (docs/lm/naming.md).
# Bundle identifiers, the executable and the .lmap type keep their layeredmap names.
brand="NimbleMap"
# The Chinese name (lm_core::brand::NAME_ZH): Finder and the menu bar show it on a Mac whose
# language is Chinese (zh-Hans.lproj below). They follow the system's language, not the app's
# View › Language.
brand_zh="凌波微图"

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:-$root/dist/lm}"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
build="${LM_BUILD_NUMBER:-0}"

export LM_UPDATE_FEED_URL="${LM_UPDATE_FEED_URL:-https://nimblemap.com/update/feed.json}"
export LM_UPDATE_FEED_FALLBACK_URL="${LM_UPDATE_FEED_FALLBACK_URL:-https://api.gitcode.com/api/v5/repos/KN-Huang/NimbleMap/raw/docs/update/feed.json?ref=main}"
echo "bundle-macos: update feed $LM_UPDATE_FEED_URL (fallback $LM_UPDATE_FEED_FALLBACK_URL)"
features=()
if [[ -n "${LM_CARGO_FEATURES:-}" ]]; then
  if [[ "$version" != *-test* ]]; then
    echo "bundle-macos: LM_CARGO_FEATURES ($LM_CARGO_FEATURES) is for test builds only, not version $version" >&2
    exit 1
  fi
  features=(--features "$LM_CARGO_FEATURES")
fi

cargo build --release --locked -p layeredmap --manifest-path "$root/Cargo.toml" ${features[@]+"${features[@]}"}

# One app (owner's decision, 2026-10-19; docs/lm/decisions.md): Pro unlocks when someone signs
# in, so there is one bundle, one id and one set of resources.
name="$brand"; name_zh="$brand_zh"; bundle_id="app.layeredmap.desktop"
app="$out/$name.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$target_dir/release/layeredmap" "$app/Contents/MacOS/layeredmap"
# The Examples menu (WP2.9): the app looks in Contents/Resources/examples.
mkdir -p "$app/Contents/Resources/examples"
rsync -a --exclude '._*' "$root/assets/examples/" "$app/Contents/Resources/examples/"
# Help › NimbleMap Help (WP6.10): the help pages, read from disk so they work offline.
mkdir -p "$app/Contents/Resources/help"
# The home page's hero video (web/lm/video, 3 MB) stays on the website; in the app its poster shows.
# The update feed (web/lm/update) is for the website, not the help pages.
rsync -a --exclude '._*' --exclude 'README.md' --exclude 'video/' --exclude 'update/' "$root/web/lm/" "$app/Contents/Resources/help/"
# The app icon (the Strata logo; tools/lm/make_app_icon.py builds it from design/logo).
cp "$root/packaging/lm/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
# Licences (Gate 5 licence audit): the assets' sources and every Rust crate's licence text.
cp "$root/LICENSE" "$root/ATTRIBUTION.md" "$app/Contents/Resources/"
[[ -f "$root/ATTRIBUTION-crates.md" ]] && cp "$root/ATTRIBUTION-crates.md" "$app/Contents/Resources/"
# The Ask box's microphone (WP7.1, a Pro feature): Apple's speech recognizer, in a small Swift
# helper. It carries its own Info.plist (usage texts), as macOS asks of any process that listens.
dictate_plist="$(mktemp -t lm-dictate-plist)"
cat > "$dictate_plist" <<DPLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>${bundle_id}.dictate</string>
  <key>CFBundleName</key><string>${name}</string>
  <key>NSMicrophoneUsageDescription</key><string>${name} listens when you click the microphone in the Ask box, to turn your request into text.</string>
  <key>NSSpeechRecognitionUsageDescription</key><string>${name} turns what you say in the Ask box into text, on this Mac when it can.</string>
</dict></plist>
DPLIST
swiftc -O -o "$app/Contents/MacOS/lm-dictate" "$root/packaging/lm/dictate.swift" \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist -Xlinker "$dictate_plist"
rm -f "$dictate_plist"
# Built-in reference layers (Natural Earth 1:10m, WorldClim elevation; hidden, used by the
# assistant once Pro is unlocked): tools/lm/reference/build_reference.py writes them to
# LM_REFERENCE_DATA.
ref="${LM_REFERENCE_DATA:-/Volumes/LaCie/data/layeredmap-reference}"
if [[ -f "$ref/manifest.json" ]]; then
  mkdir -p "$app/Contents/Resources/reference"
  rsync -a --exclude raw --exclude '._*' "$ref/" "$app/Contents/Resources/reference/"
else
  echo "bundle-macos: warning: no reference data at $ref (built-in layers left out)" >&2
fi
# The account config (docs/lm/accounts-design.md): the sign-in service's URL and public key, from
# the owner's copy outside the repo (LM_ACCOUNT_CONFIG, a path; or LM_ACCOUNT_CONFIG_JSON, its
# text, as CI passes it; or the copy in this Mac's Application Support). Without it nobody can sign
# in, so Pro can't be unlocked: the Free features work and the Pro ones stay hidden.
account_config="${LM_ACCOUNT_CONFIG:-$HOME/Library/Application Support/LayeredMap/account.json}"
if [[ -n "${LM_ACCOUNT_CONFIG_JSON:-}" ]]; then
  printf '%s\n' "$LM_ACCOUNT_CONFIG_JSON" > "$app/Contents/Resources/account.json"
  echo "bundle-macos: account config from LM_ACCOUNT_CONFIG_JSON"
elif [[ -f "$account_config" ]]; then
  cp "$account_config" "$app/Contents/Resources/account.json"
  echo "bundle-macos: account config from $account_config"
else
  echo "bundle-macos: warning: no account config (LM_ACCOUNT_CONFIG); this build can't sign in, so Pro stays locked" >&2
fi
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>${name}</string>
  <key>CFBundleDisplayName</key><string>${name}</string>
  <key>CFBundleIdentifier</key><string>${bundle_id}</string>
  <!-- The interface's languages (lm_core::i18n), so the system's own panels match; the name is
       localized in zh-Hans.lproj/InfoPlist.strings. -->
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleLocalizations</key><array><string>en</string><string>zh-Hans</string><string>de</string><string>es</string><string>fr</string><string>it</string><string>nl</string><string>pt-BR</string><string>ru</string><string>ja</string><string>ko</string></array>
  <key>LSHasLocalizedDisplayName</key><true/>
  <key>CFBundleExecutable</key><string>layeredmap</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${build}</string>
  <key>NSHumanReadableCopyright</key><string>© 2026 Kangning Huang</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>${name} listens when you click the microphone in the Ask box, to turn your request into text.</string>
  <key>NSSpeechRecognitionUsageDescription</key><string>${name} turns what you say in the Ask box into text, on this Mac when it can.</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.education</string>
  <!-- Open With (WP2.1): ${brand} can open data files, but never claims to be their default
       app (LSHandlerRank Alternate) and only views them (role Viewer). -->
  <key>CFBundleDocumentTypes</key>
  <array>
    <!-- ${brand}'s own map files (WP5.1): its type, opened for editing. -->
    <dict>
      <key>CFBundleTypeName</key><string>${brand} map</string>
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
  <!-- The .lmap type, which ${brand} owns and declares (docs/lm/format-v1.md). -->
  <key>UTExportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>app.layeredmap.map</string>
      <key>UTTypeDescription</key><string>${brand} map</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string><string>public.content</string></array>
      <key>UTTypeIconFile</key><string>AppIcon</string>
      <key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>lmap</string></array><key>public.mime-type</key><string>application/vnd.layeredmap.map+zip</string></dict>
    </dict>
  </array>
  <!-- Types macOS has no system UTI for. Imported, not exported: ${brand} does not own them. -->
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
# The Chinese name in Finder, the Dock and the menu bar on a Chinese Mac.
mkdir -p "$app/Contents/Resources/zh-Hans.lproj"
cat > "$app/Contents/Resources/zh-Hans.lproj/InfoPlist.strings" <<STRINGS
CFBundleName = "${name_zh}";
CFBundleDisplayName = "${name_zh}";
STRINGS
plutil -lint "$app/Contents/Resources/zh-Hans.lproj/InfoPlist.strings" >/dev/null

# The About panel's credits (owner: "introduce myself, the developer, with my personal website").
# muda passes no Credits option (AboutMetadata.credits would be plain text, without links), so
# AppKit reads Credits.rtf from Resources, links working. Like the bundle name above, it follows
# the system's language: the English copy in Resources, one per *.lproj. RTF with no colour table,
# so the text takes the panel's own colour in Dark Mode (Credits.html would come out black).
# $1 is "Made by " in that language; rtf_text escapes what isn't ASCII (\uN, the signed UTF-16
# code, then a space that RTF drops; every character here is in the BMP).
rtf_text() { perl -CSA -e 'for (split //, $ARGV[0]) { my $c = ord($_); print $c < 128 ? $_ : sprintf("\\u%d ", $c > 32767 ? $c - 65536 : $c) }' "$1"; }
credits_rtf() {
  cat <<RTF
{\\rtf1\\ansi\\ansicpg1252\\uc0
{\\fonttbl\\f0\\fnil\\fcharset0 .AppleSystemUIFont;}
\\pard\\qc\\f0\\fs22 $(rtf_text "$1")Kangning Huang\\
{\\field{\\*\\fldinst{HYPERLINK "https://kangning-huang.com/"}}{\\fldrslt kangning-huang.com}}\\
\\
{\\field{\\*\\fldinst{HYPERLINK "https://nimblemap.com/"}}{\\fldrslt nimblemap.com}}}
RTF
}
credits_rtf "Made by " > "$app/Contents/Resources/Credits.rtf"
# lproj|"Made by"| (the last bar keeps a trailing space). The name and the addresses stay as they are.
while IFS='|' read -r lproj made_by _; do
  mkdir -p "$app/Contents/Resources/$lproj.lproj"
  credits_rtf "$made_by" > "$app/Contents/Resources/$lproj.lproj/Credits.rtf"
done <<'CREDITS'
zh-Hans|开发者：|
de|Entwickelt von |
es|Desarrollado por |
fr|Développé par |
it|Sviluppato da |
nl|Ontwikkeld door |
pt-BR|Desenvolvido por |
ru|Разработчик: |
ja|開発者：|
ko|개발자: |
CREDITS

# On an exFAT or FAT drive macOS keeps extended attributes in AppleDouble `._*` files, which
# codesign rejects as unsigned subcomponents: drop them and the attributes before signing.
xattr -cr "$app" 2>/dev/null || true
find "$app" -name '._*' -delete

if [[ -n "${LM_SIGN_IDENTITY:-}" ]]; then
  # The Developer ID identity's own keychain (scripts get errSecInternalComponent from the login
  # keychain): unlock it with the password the login keychain keeps. Never echoed.
  signing_kc="${LM_SIGNING_KEYCHAIN:-$HOME/Library/Keychains/nimblemap-signing.keychain-db}"
  if [[ -f "$signing_kc" ]]; then
    if kc_pw="$(security find-generic-password -s nimblemap-signing-keychain -a developer-id -w 2>/dev/null)"; then
      security unlock-keychain -p "$kc_pw" "$signing_kc"
      echo "bundle-macos: unlocked $signing_kc"
    else
      echo "bundle-macos: warning: no password for $signing_kc in the login keychain (nimblemap-signing-keychain)" >&2
    fi
    unset kc_pw
  fi
  # The microphone needs the audio-input entitlement under the hardened runtime.
  ent=(--entitlements "$root/packaging/lm/app.entitlements")
  codesign --force --options runtime --timestamp "${ent[@]}" --sign "$LM_SIGN_IDENTITY" "$app/Contents/MacOS/lm-dictate"
  codesign --force --options runtime --timestamp "${ent[@]}" --sign "$LM_SIGN_IDENTITY" "$app"
  codesign --verify --strict --verbose=2 "$app"
  # The hardened runtime (WP5.7), which notarization requires: the signature must carry it.
  # Captured first: with pipefail, grep -q exiting on its match would make codesign fail on SIGPIPE.
  signature="$(codesign --display --verbose=2 "$app" 2>&1)"
  if ! grep -q 'flags=.*runtime' <<< "$signature"; then
    echo "bundle-macos: $app is signed without the hardened runtime" >&2
    exit 1
  fi
  if [[ -n "${LM_NOTARY_PROFILE:-}" ]]; then
    notary=(--keychain-profile "$LM_NOTARY_PROFILE")
  elif [[ -n "${LM_NOTARY_APPLE_ID:-}" ]]; then
    notary=(--apple-id "$LM_NOTARY_APPLE_ID" --team-id "$LM_NOTARY_TEAM_ID" --password "$LM_NOTARY_PASSWORD")
  else
    notary=()
  fi
  if (( ${#notary[@]} )); then
    zip="$out/${brand}-notarize.zip"
    ditto -c -k --keepParent "$app" "$zip"
    xcrun notarytool submit "$zip" "${notary[@]}" --wait
    xcrun stapler staple "$app"
    xcrun stapler validate "$app"
    spctl --assess --type execute --verbose "$app"
    rm -f "$zip"
  fi
else
  echo "bundle-macos: no LM_SIGN_IDENTITY; ad-hoc signing (will warn on other Macs)" >&2
  codesign --force --sign - "$app/Contents/MacOS/lm-dictate"
  codesign --force --sign - "$app"
fi
ditto -c -k --keepParent "$app" "$out/${name// /-}-${version}-${build}-macos-arm64.zip"
echo "bundle-macos: $app"
