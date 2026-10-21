#!/bin/bash
# Record one tour run of the real app, window only.
#   capture/capture.sh <name>            reads tours/<name>.tour (one step per line, # comments)
# Optional tours/<name>.env: one VAR=value per line for the app (e.g. LM_DEV=1 to show export).
# Optional tours/<name>.files: one file path per line, opened at launch (as Open With would).
# Writes raw/<name>.mov (ScreenCaptureKit, the app window only, 60 fps, no system pointer) and
# raw/<name>.log (the app's stdout/stderr with the recorder's start line).
# Never sends keystrokes or clicks: the app drives itself through its --tour script mode.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
NAME="$1"
BIN="${LM_BIN:-/Volumes/LaCie/cargo-target/layeredmap-video/release/layeredmap}"
WREC="$HERE/capture/wrec"
[ -x "$WREC" ] || swiftc -O -o "$WREC" "$HERE/capture/wrec.swift"
mkdir -p "$HERE/raw"
TOUR="$(grep -v '^\s*#' "$HERE/tours/$NAME.tour" | sed '/^\s*$/d' | paste -sd ';' -)"
FILES=()
if [ -f "$HERE/tours/$NAME.files" ]; then
  while IFS= read -r f; do [ -n "$f" ] && FILES+=("$HERE/$f"); done < "$HERE/tours/$NAME.files"
fi
echo "tour: $TOUR"
ENVS=()
[ -f "$HERE/tours/$NAME.env" ] && while IFS= read -r e; do [ -n "$e" ] && ENVS+=("$e"); done < "$HERE/tours/$NAME.env"
env ${ENVS[@]+"${ENVS[@]}"} "$BIN" ${FILES[@]+"${FILES[@]}"} -NSRequiresAquaSystemAppearance YES --tour "$TOUR" > "$HERE/raw/$NAME.log" 2>&1 &
APP=$!
# Wait for the window and start recording at once (the tour's first step fires ~2 s after the
# first map render, so the recording always contains it).
for _ in $(seq 1 200); do
  WID="$("$HERE/capture/winid" $APP 2>/dev/null || true)"
  [ -n "$WID" ] && break
  sleep 0.05
done
[ -n "$WID" ] || { echo "no window"; kill $APP; exit 1; }
"$WREC" "$WID" "$HERE/raw/$NAME.mov" 240 60 | tee -a "$HERE/raw/$NAME.log"
wait $APP || true
ffprobe -v error -show_entries format=duration -of csv=p=0 "$HERE/raw/$NAME.mov"
