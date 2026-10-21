#!/bin/bash
# Render the v3.2 narration (audio/narration.txt) to audio/vo/<id>.aiff with macOS `say`, locally.
#   audio/voice.sh [voice, default Samantha] [rate wpm, default 158]
set -euo pipefail
cd "$(dirname "$0")"
VOICE="${1:-Samantha}"
RATE="${2:-158}"
mkdir -p vo
grep -v '^#' narration.txt | while IFS='|' read -r id text cap; do
  id=$(echo "$id" | xargs)
  [ -n "$id" ] || continue
  say -v "$VOICE" -r "$RATE" -o "vo/$id.aiff" "$(echo "$text" | sed 's/^ *//;s/ *$//')"
  echo "$id $(ffprobe -v error -show_entries format=duration -of csv=p=0 vo/$id.aiff)"
done
