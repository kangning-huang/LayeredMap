#!/bin/bash
# Retake the website's app screenshots (web/lm/img/) from a fresh release build.
#
#   tools/lm/site/retake_screenshots.sh                       # every free screenshot, then the crops
#   tools/lm/site/retake_screenshots.sh typhoons-card walls-satellite   # just these (+ their crops)
#   PAID=1 tools/lm/site/retake_screenshots.sh pro-ask        # shots that spend an Ask call
#   NOBUILD=1 …                                               # skip the cargo build
#   OPEN_FILES=<folder> …                                     # also the home page's open-* shots
#
# Each shot launches the app with a `--tour` script (apps/layeredmap/src/main.rs › Step), finds the
# window that process opens (by its PID, so other NimbleMap windows on screen are ignored), captures
# that window only with `screencapture -x -o -l<id>` DELAY seconds after launch, and saves it 1600 px
# wide as JPEG (Pillow quality 90, 4:4:4). It never sends keys or clicks. Tour runs quit by themselves at the end
# and never save prefs. Steps run 2.5 s apart (6 s after a fly, 15 s after `ask`), starting ~2 s after
# the first frame; the repeated flies at the end of a tour only keep the window open for the capture.
#
# Every row runs the plain release build signed out, as a person sees it after installing (tours
# start the account but never sign in, so Pro's features show locked and the Ask box shows its
# sign-in prompt). PAID=1 is needed for rows marked `paid`: their tour uses `ask`, which sends one
# request to the owner's OpenRouter key (from the keychain, LM_ASK_ROUTE=direct; no build carries a
# key, and the release build shows no cost line). Rows marked `pro` and `paid` unlock Pro with the
# developer override (LM_DEV_PRO=1); `pro` rows cost nothing. Rows marked `dev` run with LM_DEV=1.
# Rows marked `files` open the downloaded-style files of the home page (`@name` in the file args is
# `$OPEN_FILES/name`); they run only with OPEN_FILES set to a folder written by
# `design/launch-video-v3/data/make_files.sh "<folder>"` (outside the repo; it converts the bundled
# examples with GDAL into .shp, .gpkg, .nc, .tif and .csv as they are downloaded).
#
# Adding a screenshot is one line in SHOTS (name | kind | delay s | crop of raw capture | tour | file
# args), and a derived cut is one line in CUTS (name | source | box in the 1600-px image | size).
# A shot whose name starts with `_` is only a source for cuts: it is kept with the raw captures, not
# in web/lm/img/.
set -euo pipefail

SHOTS=$(cat <<'EOF'
welcome            | -    | 14 | -                | theme light;basemap light;85,32,3.0;85,32,3.0 | --view 85,32,3.0
typhoons-hero      | -    | 25 | 254,0,2564,1754  | theme light;basemap light;example storms;preset 1;esc;108,22,4.1;108,22,4.1;108,22,4.1 |
typhoons-card      | -    | 33 | -                | theme light;basemap light;example storms;preset 1;153.65,6.2,9;click 153.65,6.2;119,16,4.3;119,16,4.3;119,16,4.3 |
typhoons-pick      | -    | 33 | -                | theme light;basemap light;example storms;preset 1;155.8,6.2,7;click 155.8,6.2;119,16,4.3;119,16,4.3;119,16,4.3 |
typhoons-color     | -    | 27 | -                | theme light;basemap light;example storms;preset 2;tab 1;113,18,3.6;113,18,3.6;113,18,3.6 |
typhoons-filter    | -    | 36 | -                | theme light;basemap light;example storms;preset 1;153.65,6.2,9;click 153.65,6.2;113,24,4.1;card filter strength;113,24,4.1;113,24,4.1 |
typhoons-filtered  | -    | 39 | -                | theme light;basemap light;example storms;preset 1;153.65,6.2,9;click 153.65,6.2;113,20,3.7;card filter strength;card apply_filter;113,20,3.7;113,20,3.7 |
points-earthquakes | -    | 31 | -                | theme light;basemap light;example earthquakes;card style_by_value magnitude;142.373,38.297,10;click 142.373,38.297;175,12,2.4;175,12,2.4;175,12,2.4 |
areas-countries    | -    | 26 | -                | theme light;basemap light;card style_by_value pop_est;click 79,22;40,22,2.5;40,22,2.5;40,22,2.5 | assets/examples/countries.geojson
heatwave-slider    | -    | 27 | -                | theme light;basemap light;example heatwaves;time 45;-20,20,2.4;-20,20,2.4;-20,20,2.4 |
heatwave-click     | -    | 30 | -                | theme light;basemap light;example heatwaves;time 45;20,44,4.3;click 0.5,35.8;20,44,4.3;20,44,4.3 |
heatwave-export    | -    | 26 | -                | theme light;basemap light;example heatwaves;time 45;-20,20,2.4;export;-20,20,2.4;-20,20,2.4 |
walls-satellite    | -    | 30 | -                | theme light;basemap satellite;example walls;116.4,39.9,9.3;click 116.39,39.93;116.4,39.9,9.3;116.4,39.9,9.3 |
walls-compare      | -    | 32 | -                | theme light;basemap light;example walls;116.4,39.9,9.3;click 116.39,39.93;card compare_peers area_km2;116.4,39.9,9.3;116.4,39.9,9.3 |
walls-region       | -    | 28 | -                | theme light;basemap satellite;example walls;preset 1;116.2,37.6,6.4;116.2,37.6,6.4;116.2,37.6,6.4 |
walls-filter       | -    | 36 | -                | theme light;basemap light;example walls;116.4,39.9,9.3;click 116.39,39.93;102,27,4.1;card filter type;102,27,4.1;102,27,4.1 |
typhoons-pro       | pro  | 33 | -                | theme light;basemap light;example storms;preset 1;153.65,6.2,9;click 153.65,6.2;132.8,13.1,4.2;132.8,13.1,4.2;132.8,13.1,4.2 |
pro-ask            | paid | 48 | -                | theme light;basemap light;100,30,2.5;ask Which country has the most cities?;100,30,2.5;100,30,2.5;100,30,2.5;100,30,2.5;100,30,2.5 | assets/examples/cities.geojson
open-storms        | files | 30 | -               | theme light;basemap light;tab 0;tab 0;card style_by_value max_wind_k;tab 0;150,22,3.2;tab 0;150,22,3.2;150,22,3.2;150,22,3.2;150,22,3.2 | @IBTrACS_storm_tracks_1980-2026.shp
_open-search       | files | 29 | -               | theme light;basemap light;tab 0;tab 0;card style_by_value max_wind_k;find Haiyan;tab 0;150,22,3.2;tab 0;150,22,3.2;150,22,3.2;150,22,3.2;150,22,3.2 | @IBTrACS_storm_tracks_1980-2026.shp
_open-zoom         | files | 36 | -               | theme light;basemap light;tab 0;tab 0;card style_by_value max_wind_k;tab 0;124,16,4.6;esc;esc;124,16,4.6;124,16,4.6;124,16,4.6;124,16,4.6;124,16,4.6 | @IBTrACS_storm_tracks_1980-2026.shp
open-earthquakes   | files | 34 | -               | theme light;basemap light;175,12,2.4;175,12,2.4;175,12,2.4;card style_by_value magnitude;tab 0;175,12,2.4;175,12,2.4;175,12,2.4;175,12,2.4 | @usgs_earthquakes_M6plus_1900-2026.gpkg
_open-click        | files | 36 | -               | theme light;basemap light;142.373,38.297,10;142.373,38.297,10;card style_by_value magnitude;142.373,38.297,10;click 142.373,38.297;160,20,2.6;160,20,2.6;160,20,2.6;160,20,2.6;160,20,2.6 | @usgs_earthquakes_M6plus_1900-2026.gpkg
open-heatwave      | files | 34 | -               | theme light;basemap light;-5,25,2.6;-5,25,2.6;-5,25,2.6;time 45;tab 0;-5,25,2.6;-5,25,2.6;-5,25,2.6;-5,25,2.6 | @GDO_heatwave_intensity_JJA_2023.nc
open-relief        | files | 30 | -               | theme light;basemap light;60,10,1.9;60,10,1.9;60,10,1.9;tab 0;60,10,1.9;60,10,1.9;60,10,1.9;60,10,1.9 | @ETOPO_2022_relief_30min.tif
open-volcanoes     | files | 32 | -               | theme light;basemap light;175,10,2.3;175,10,2.3;175,10,2.3;card style_by_value elevation_m;tab 0;175,10,2.3;175,10,2.3;175,10,2.3;175,10,2.3 | @NCEI_volcano_locations.csv
EOF
)

CUTS=$(cat <<'EOF'
step-open   | welcome            | 380,458,1240,996  | 800x500
step-click  | points-earthquakes | 1000,98,1600,473  | 800x500
step-share  | heatwave-export    | 60,170,1150,851   | 800x500
step-honest | walls-compare      | 640,110,1600,710  | 800x500
step-try    | typhoons-filter    | 340,496,1140,996  | -
open-search | _open-search       | 700,100,1600,662  | 800x500
open-zoom   | _open-zoom         | 345,100,1545,850  | 800x500
open-click  | _open-click        | 600,110,1600,735  | 800x500
open-time   | open-heatwave      | 210,420,1130,995  | 800x500
EOF
)

ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
IMG="$ROOT/web/lm/img"
TARGET=${CARGO_TARGET_DIR:-/Volumes/LaCie/cargo-target/layeredmap}
APP="$TARGET/release/layeredmap"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/lm-retake.XXXXXX")
PY=(~/miniforge3/bin/mamba run -n lm-logo python -I "$ROOT/tools/lm/site/retake_screenshots.py")
# Light window chrome (title bar) even when macOS is in Dark mode: a per-process Cocoa default.
AQUA=(-NSRequiresAquaSystemAppearance YES)
trim() { local s="$1"; s="${s#"${s%%[![:space:]]*}"}"; printf '%s' "${s%"${s##*[![:space:]]}"}"; }

dest() {  # where a shot's 1600-px JPEG goes: web/lm/img/, or the work folder for `_` sources
  if [[ "$1" == _* ]]; then printf '%s' "$WORK/$1.jpg"; else printf '%s' "$IMG/$1.jpg"; fi
}

want() {  # is this name selected on the command line (or no names given)?
  [[ ${#NAMES[@]} -eq 0 ]] && return 0
  local n; for n in "${NAMES[@]}"; do [[ "$n" == "$1" ]] && return 0; done; return 1
}
NAMES=("$@")

if [[ -z "${NOBUILD:-}" ]]; then
  (cd "$ROOT" && CARGO_TARGET_DIR="$TARGET" CARGO_INCREMENTAL=0 cargo build --release -p layeredmap)
fi
swiftc -O -o "$WORK/window_ids" "$ROOT/tools/lm/site/window_ids.swift"

shoot() {  # name kind delay crop tour args
  local name=$1 kind=$2 delay=$3 crop=$4 tour=$5 args=$6
  local raw="$WORK/$name.png" envs=() files=()
  [[ "$kind" == pro || "$kind" == paid ]] && envs=(LM_DEV_PRO=1 LM_ASK_ROUTE=direct)
  [[ "$kind" == dev ]] && envs=(LM_DEV=1)
  local f; for f in $args; do
    if [[ "$f" == @* ]]; then files+=("$OPEN_FILES/${f#@}")
    elif [[ "$f" == --* || "$f" == [0-9-]* ]]; then files+=("$f")
    else files+=("$ROOT/$f"); fi
  done
  local out; out=$(dest "$name")
  echo "== $name (capture at ${delay}s)"
  local t0=$SECONDS
  (cd "$ROOT" && exec env ${envs[@]+"${envs[@]}"} "$APP" --tour "$tour" ${files[@]+"${files[@]}"} "${AQUA[@]}") >"$WORK/$name.log" 2>&1 &
  local pid=$! wid=""
  while [[ -z "$wid" ]] && kill -0 "$pid" 2>/dev/null; do
    wid=$("$WORK/window_ids" "$pid" | head -1); [[ -z "$wid" ]] && sleep 0.5
  done
  if [[ -z "$wid" ]]; then echo "   no window (see $WORK/$name.log)"; return 1; fi
  local left=$(( delay - (SECONDS - t0) )); (( left > 0 )) && sleep "$left"
  if ! kill -0 "$pid" 2>/dev/null; then echo "   the tour ended before the capture; lower the delay"; return 1; fi
  screencapture -x -o -l"$wid" "$raw" || { sleep 1; screencapture -x -o -l"$wid" "$raw"; } || true
  if [[ ! -s "$raw" ]]; then echo "   capture failed (window $wid)"; kill "$pid" 2>/dev/null; return 1; fi
  local old=$(stat -f %z "$out" 2>/dev/null || echo 0)
  "${PY[@]}" shot "$raw" "$out" ${crop:+"$crop"}
  echo "   window $wid; $old -> $(stat -f %z "$out") bytes"
  local i=0; while kill -0 "$pid" 2>/dev/null && (( i < 120 )); do sleep 0.5; i=$((i + 1)); done
  kill "$pid" 2>/dev/null || true
}

DONE=()
while IFS='|' read -r name kind delay crop tour args; do
  name=$(trim "$name"); kind=$(trim "$kind"); delay=$(trim "$delay"); crop=$(trim "$crop")
  tour=$(trim "$tour"); args=$(trim "$args"); [[ "$crop" == - ]] && crop=""
  [[ -z "$name" ]] && continue
  want "$name" || continue
  if [[ "$kind" == paid && -z "${PAID:-}" ]]; then echo "== $name skipped: its tour uses ask (set PAID=1)"; continue; fi
  if [[ "$kind" == files && -z "${OPEN_FILES:-}" ]]; then echo "== $name skipped: set OPEN_FILES (see the top of this script)"; continue; fi
  shoot "$name" "$kind" "$delay" "$crop" "$tour" "$args" && DONE+=("$name")
done <<< "$SHOTS"

while IFS='|' read -r name src b size; do
  name=$(trim "$name"); src=$(trim "$src"); b=$(trim "$b"); size=$(trim "$size"); [[ "$size" == - ]] && size=""
  [[ -z "$name" ]] && continue
  want "$name" || want "$src" || continue
  if [[ ! -f "$(dest "$src")" ]]; then echo "== $name skipped: $src was not taken in this run"; continue; fi
  echo "== $name (cut from $src)"
  "${PY[@]}" cut "$(dest "$src")" "$IMG/$name.jpg" "$b" ${size:+"$size"}
done <<< "$CUTS"

find "$IMG" -name '._*' -delete 2>/dev/null || true
echo "Raw captures and logs: $WORK"
