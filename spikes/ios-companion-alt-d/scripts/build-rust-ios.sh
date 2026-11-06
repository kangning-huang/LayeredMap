#!/usr/bin/env bash
# Build lm-ios-alt-d-decode-ffi staticlib for iOS simulator / device.
# Requires full Xcode (iPhoneOS / iPhoneSimulator SDKs), not Command Line Tools alone.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

TARGET="${1:-aarch64-apple-ios-sim}"
case "$TARGET" in
  aarch64-apple-ios-sim|aarch64-apple-ios|x86_64-apple-ios) ;;
  *)
    echo "usage: $0 [aarch64-apple-ios-sim|aarch64-apple-ios|x86_64-apple-ios]" >&2
    exit 2
    ;;
esac

if ! xcrun --sdk iphonesimulator --show-sdk-path >/dev/null 2>&1 && [[ "$TARGET" == *sim* ]]; then
  echo "error: iPhoneSimulator SDK missing. Install Xcode from the App Store, then:" >&2
  echo "  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer" >&2
  exit 1
fi
if ! xcrun --sdk iphoneos --show-sdk-path >/dev/null 2>&1 && [[ "$TARGET" == "aarch64-apple-ios" ]]; then
  echo "error: iPhoneOS SDK missing. Install Xcode and select it with xcode-select." >&2
  exit 1
fi

# Without this rustc links the device build against iOS 10, while the C dependencies (zstd,
# sqlite) are compiled against the current SDK — the mismatch ends in "Undefined symbols for
# architecture arm64". The simulator target happens to pick a modern default and so never showed it.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"

rustup target add "$TARGET" >/dev/null
cargo build -p lm-ios-alt-d-decode-ffi --release --target "$TARGET"

OUT="target/${TARGET}/release/liblm_ios_alt_d_decode_ffi.a"
echo "built $OUT"
