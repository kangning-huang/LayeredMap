#!/usr/bin/env bash
# Build lm-ios-alt-b-decode-ffi staticlib for iOS simulator / device.
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

rustup target add "$TARGET" >/dev/null
cargo build -p lm-ios-alt-b-decode-ffi --release --target "$TARGET"

OUT="target/${TARGET}/release/liblm_ios_alt_b_decode_ffi.a"
echo "built $OUT"
