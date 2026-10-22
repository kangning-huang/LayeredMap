#!/usr/bin/env bash
# Builds NimbleMap's web app into a static folder (docs/lm/web-app.md).
#
#   apps/nimblemap-web/build.sh [OUT_DIR]        (default: dist/nimblemap-web)
#
# Needs: the wasm32-unknown-unknown target, wasm-bindgen-cli at the Cargo.lock version of
# wasm-bindgen, a clang that targets wasm32 (zstd-sys and ring compile C: set CC_wasm32_unknown_unknown
# when the system clang can't, e.g. Homebrew's llvm on macOS), and optionally wasm-opt (binaryen)
# and brotli/gzip for the size report. EXAMPLES=0 leaves the example files out.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="${1:-$root/dist/nimblemap-web}"
target_dir="${CARGO_TARGET_DIR:-$root/target}"

if [[ -z "${CC_wasm32_unknown_unknown:-}" && -x /opt/homebrew/opt/llvm/bin/clang ]]; then
  export CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang
  export AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar
fi

cd "$root"
cargo build --locked --profile web --target wasm32-unknown-unknown -p nimblemap-web
wasm="$target_dir/wasm32-unknown-unknown/web/nimblemap-web.wasm"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
wasm-bindgen --target web --no-typescript --weak-refs --out-dir "$stage" --out-name nimblemap "$wasm"
if command -v wasm-opt >/dev/null; then
  # -O2: the map is drawn on the CPU in WebAssembly, so speed matters as much as size.
  wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
    --enable-reference-types --enable-multivalue -o "$stage/nimblemap_bg.opt.wasm" "$stage/nimblemap_bg.wasm"
  mv "$stage/nimblemap_bg.opt.wasm" "$stage/nimblemap_bg.wasm"
fi

# Content-hashed names, so a host can cache them for good.
h="$(shasum -a 256 "$stage/nimblemap_bg.wasm" | cut -c1-12)"
wasm_name="nimblemap-$h.wasm"
js_name="nimblemap-$h.js"
rm -rf "$out"
mkdir -p "$out"
mv "$stage/nimblemap_bg.wasm" "$out/$wasm_name"
sed "s/nimblemap_bg\.wasm/$wasm_name/g" "$stage/nimblemap.js" > "$out/$js_name"
sed -e "s/__JS__/$js_name/g" -e "s/__WASM__/$wasm_name/g" "$here/index.html" > "$out/index.html"
cp "$here/static/_headers" "$here/static/manifest.webmanifest" "$out/"
cp "$root/web/lm/assets/logo.svg" "$root/web/lm/favicon.ico" "$root/web/lm/apple-touch-icon.png" "$out/"
cp "$root/assets/icon/icon-256.png" "$out/icon-256.png"
# Fetched by the app after it starts, not built into the WebAssembly (docs/lm/web-app.md › Sizes).
mkdir -p "$out/fonts" "$out/geonames"
cp "$root/assets/fonts/NotoSansSC-Regular.otf" "$root/assets/fonts/OFL-NotoSansSC.txt" "$out/fonts/"
cp "$root/assets/geonames/cities.tsv" "$root/assets/geonames/README.md" "$out/geonames/"
if [[ "${EXAMPLES:-1}" != 0 ]]; then
  mkdir -p "$out/examples"
  cp "$root"/assets/examples/*.{csv,geojson,gpkg,tif,nc} "$root/assets/examples/README.md" "$out/examples/"
fi
find "$out" -name '._*' -delete

echo "built $out"
raw=$(wc -c < "$out/$wasm_name")
echo "wasm: $raw bytes raw"
command -v gzip >/dev/null && echo "wasm: $(gzip -9c "$out/$wasm_name" | wc -c) bytes gzip -9"
command -v brotli >/dev/null && echo "wasm: $(brotli -c -q 11 "$out/$wasm_name" | wc -c) bytes brotli -q 11"
