#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
cargo test -p lm-ios-alt-c-decode-ffi --locked
echo "host FFI OK"
