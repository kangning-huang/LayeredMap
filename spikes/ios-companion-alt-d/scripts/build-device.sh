#!/usr/bin/env bash
# Build Alt D for a real iPhone and install it.
#
# Everything here is the same as the simulator build except signing, which needs an Apple ID.
# A free one is enough for running on your own phone (the build expires after seven days).
#
#   1. Xcode › Settings › Accounts › + › Apple ID, and sign in.
#   2. Find your team id:  xcrun security find-identity -v -p codesigning
#      or read it from Xcode › Settings › Accounts › Manage Certificates.
#   3. Plug the phone in, unlock it, and tap Trust.
#   4. ./scripts/build-device.sh <TEAM_ID>
#
# The conda scrub is the same one the simulator script needs: a conda base environment exports
# LD/CC/AR/CFLAGS/LDFLAGS and they hijack Xcode's linker.
set -euo pipefail

TEAM="${1:-}"
HERE="$(cd "$(dirname "$0")/.." && pwd)"

if [[ -z "$TEAM" ]]; then
  echo "usage: $0 <APPLE_DEVELOPMENT_TEAM_ID>" >&2
  echo >&2
  echo "Signed-in teams on this Mac:" >&2
  security find-identity -v -p codesigning 2>/dev/null | sed 's/^/  /' >&2 || true
  exit 2
fi

unset LD CC CXX AR RANLIB CFLAGS CXXFLAGS CPPFLAGS LDFLAGS LDFLAGS_LD \
      SDKROOT HOST BUILD CC_FOR_BUILD CONDA_TOOLCHAIN_HOST CONDA_TOOLCHAIN_BUILD \
      CONDA_BACKUP_HOST DEBUG_CFLAGS DEBUG_CPPFLAGS DEBUG_CXXFLAGS 2>/dev/null || true
export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"

DEVICE_ID="$(xcrun devicectl list devices 2>/dev/null \
  | awk '/iPhone|iPad/ && $0 !~ /Simulator/ {print $(NF-1)}' | head -1)"
if [[ -z "$DEVICE_ID" ]]; then
  echo "No iPhone found. Plug it in, unlock it, and tap Trust." >&2
  exit 1
fi
echo "device: $DEVICE_ID"

DD="${HERE}/.build-device"
cd "$HERE"
xcodebuild \
  -project ios/NimbleMapAltD.xcodeproj \
  -scheme NimbleMapAltD \
  -configuration Debug \
  -destination "platform=iOS,id=${DEVICE_ID}" \
  -derivedDataPath "$DD" \
  -allowProvisioningUpdates \
  DEVELOPMENT_TEAM="$TEAM" \
  CODE_SIGN_STYLE=Automatic \
  build

APP="$(find "$DD/Build/Products" -maxdepth 2 -name 'NimbleMapAltD.app' | head -1)"
echo "installing $APP"
xcrun devicectl device install app --device "$DEVICE_ID" "$APP"
echo
echo "Installed. On the phone: Settings › General › VPN & Device Management ›"
echo "trust your developer certificate the first time, then open NimbleMap D."
