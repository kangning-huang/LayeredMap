#!/usr/bin/env bash
# Builds the NimbleMap installer disk image (WP5.7): the app beside a link to /Applications, so
# installing is one drag. Uses only hdiutil and the tools bundle-macos.sh already needs.
#
#   packaging/lm/build-dmg.sh [out-dir]
#
# One app, one .dmg: Pro unlocks when someone signs in (docs/lm/accounts-design.md › Entitlement).
#
# The app is built (and, with credentials, signed, notarized and stapled) by bundle-macos.sh.
# With the same environment as there (docs/lm/signing.md), the .dmg is then signed, notarized,
# stapled, and checked with `stapler validate` and `spctl`:
#   LM_SIGN_IDENTITY, and LM_NOTARY_PROFILE (a notarytool keychain profile) or
#   LM_NOTARY_APPLE_ID, LM_NOTARY_TEAM_ID, LM_NOTARY_PASSWORD
# Without them the .dmg is unsigned (macOS warns when an app from it is first opened elsewhere).
#
# Download host (an owner decision: spec › WP5.7, "the China-reachable download host"):
#   LM_DOWNLOAD_BASE  e.g. https://download.example.cn/layeredmap/ — where the .dmg files will be
#                     uploaded. When set, update-feed.json is written next to the .dmg, in the
#                     format of docs/lm/update-feed.md (under both "free" and "pro", which point
#                     at the one .dmg, so apps from before the merge still find it).
#   LM_DOWNLOAD_PAGE  the download page the feed falls back to (default: LM_DOWNLOAD_BASE).
#   LM_RELEASE_NOTES, LM_RELEASE_NOTES_ZH   one-line notes for the feed (optional).
set -euo pipefail

# The display name (app, volume and .dmg names); keep in step with lm_core::brand and
# bundle-macos.sh (docs/lm/naming.md).
brand="NimbleMap"

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:-$root/dist/lm}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
build="${LM_BUILD_NUMBER:-0}"
name="$brand"
arch="$(uname -m)"
[[ "$arch" == "x86_64" ]] || arch="arm64"

# The app, signed and notarized when configured.
"$root/packaging/lm/bundle-macos.sh" "$out"
app="$out/$name.app"
[[ -d "$app" ]] || { echo "build-dmg: $app was not built" >&2; exit 1; }

# The image's contents: the app and the Applications link, nothing else.
stage="$(mktemp -d -t lm-dmg)"
trap 'rm -rf "$stage"' EXIT
ditto "$app" "$stage/$name.app"
ln -s /Applications "$stage/Applications"

dmg="$out/${name// /-}-${version}-${build}-macos-${arch}.dmg"
rm -f "$dmg"
# HFS+ and zlib compression (UDZO): opens on every macOS the app supports (13 and later).
hdiutil create -volname "$name $version" -srcfolder "$stage" -fs HFS+ -format UDZO -imagekey zlib-level=9 -ov "$dmg" >/dev/null
hdiutil verify "$dmg" >/dev/null
echo "build-dmg: $dmg"

if [[ -n "${LM_SIGN_IDENTITY:-}" ]]; then
  codesign --force --timestamp --sign "$LM_SIGN_IDENTITY" "$dmg"
  codesign --verify --strict --verbose=2 "$dmg"
  if [[ -n "${LM_NOTARY_PROFILE:-}" ]]; then
    notary=(--keychain-profile "$LM_NOTARY_PROFILE")
  elif [[ -n "${LM_NOTARY_APPLE_ID:-}" ]]; then
    notary=(--apple-id "$LM_NOTARY_APPLE_ID" --team-id "$LM_NOTARY_TEAM_ID" --password "$LM_NOTARY_PASSWORD")
  else
    notary=()
  fi
  if (( ${#notary[@]} )); then
    xcrun notarytool submit "$dmg" "${notary[@]}" --wait
    xcrun stapler staple "$dmg"
    # Gate 5: "The .dmg passes xcrun stapler validate and an spctl assessment."
    xcrun stapler validate "$dmg"
    spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
  else
    echo "build-dmg: signed but not notarized (no LM_NOTARY_PROFILE or LM_NOTARY_APPLE_ID)" >&2
  fi
else
  echo "build-dmg: no LM_SIGN_IDENTITY; the .dmg is unsigned" >&2
fi

sha="$(shasum -a 256 "$dmg" | cut -d' ' -f1)"
echo "build-dmg: sha256 $sha"

# The update feed for the download host, when its address is known.
if [[ -n "${LM_DOWNLOAD_BASE:-}" ]]; then
  python3 - "$out/update-feed.json" "$version" "${LM_DOWNLOAD_BASE%/}/$(basename "$dmg")" \
    "${LM_DOWNLOAD_PAGE:-$LM_DOWNLOAD_BASE}" "$sha" "${LM_RELEASE_NOTES:-}" "${LM_RELEASE_NOTES_ZH:-}" <<'PY'
import datetime, json, os, sys
path, version, url, page, sha, notes, notes_zh = sys.argv[1:]
if not url.startswith("https://") or not page.startswith("https://"):
    sys.exit("build-dmg: LM_DOWNLOAD_BASE and LM_DOWNLOAD_PAGE must be https addresses")
feed = {}
if os.path.exists(path):
    with open(path) as f:
        feed = json.load(f)
if feed.get("version") != version:
    feed = {}
feed.update({"format": 1, "version": version, "released": datetime.date.today().isoformat(),
             "page": page, "minimum_macos": "13.0"})
# One app: "free" is what apps read; "pro" stays for Pro apps from before the merge (0.7.0 and
# earlier), so they update to the one app.
feed["download"] = {"free": url, "pro": url}
feed["sha256"] = {"free": sha, "pro": sha}
if notes or notes_zh:
    feed["notes"] = {k: v for k, v in (("en", notes), ("zh", notes_zh)) if v}
with open(path, "w") as f:
    json.dump(feed, f, ensure_ascii=False, indent=2)
    f.write("\n")
print(f"build-dmg: {path} ({url})")
PY
else
  echo "build-dmg: no LM_DOWNLOAD_BASE; update-feed.json not written (the host is an owner decision)" >&2
fi
