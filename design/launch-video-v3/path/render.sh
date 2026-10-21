#!/bin/bash
# Render v3.7 shots with the app's offline renderer, then blend their motion-blur sub-frames.
#   path/render.sh ty eq hw pro ty_plate eq_plate     (in parallel; frames already rendered are kept)
# The renderer is the lm-ui example `film_render` (crates/lm-ui/examples/film_render.rs), built
# into $CARGO_TARGET_DIR (default /Volumes/LaCie/cargo-target/layeredmap-smooth).
set -euo pipefail
cd "$(dirname "$0")/.."
PY="${PY:-$HOME/miniforge3/envs/lm-video/bin/python}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/Volumes/LaCie/cargo-target/layeredmap-smooth}"
[ -n "${NOBUILD:-}" ] || (cd ../.. && cargo build --release -p lm-ui --example film_render 2>&1 | grep -E "^error|Finished")
find "$CARGO_TARGET_DIR" -name '._*' -delete 2>/dev/null || true
BIN="$CARGO_TARGET_DIR/release/examples/film_render"
$PY path/shots.py "$@"
pids=()
for s in "$@"; do
  ENVS=()
  [ -f render/$s/env ] && while IFS= read -r e; do [ -n "$e" ] && ENVS+=("$e"); done < render/$s/env
  env ${ENVS[@]+"${ENVS[@]}"} "$BIN" --render-path render/$s/shot.json --out render/$s/sub > render/$s/log.txt 2>&1 &
  pids+=($!)
done
for p in "${pids[@]}"; do wait "$p"; done
for s in "$@"; do tail -1 render/$s/log.txt; grep -h "warning" render/$s/log.txt || true; done
$PY path/blend.py "$@"
