#!/bin/zsh
# Full build: audio -> lossless video master -> deliverables -> contact sheet.
# Usage: ./build.sh [workers]
set -euo pipefail
cd "$(dirname "$0")"
FF=${FFMPEG:-/Users/kangninghuang/.local/bin/ffmpeg}
WORKERS=${1:-5}
mkdir -p out

# 1. audio (numpy/scipy synthesis, mastered to -14 LUFS)
~/miniforge3/bin/mamba run -n lm-video python audio/synth.py out/audio.wav

# 2. picture: 1200 frames at 60 fps -> FFV1 lossless master
[ -d node_modules ] || npm install
node render.mjs video out/master.mkv "$WORKERS" 60

# 3. 1080p60 delivery: H.264 High, yuv420p, AAC 320k, faststart
"$FF" -y -loglevel error -i out/master.mkv -i out/audio.wav \
  -map 0:v -map 1:a -c:v libx264 -profile:v high -preset slow -crf 15 -tune film \
  -pix_fmt yuv420p -color_primaries bt709 -color_trc bt709 -colorspace bt709 \
  -c:a aac -b:a 320k -ar 48000 -t 20 -movflags +faststart out/NimbleMap-launch-1080p60.mp4

# 4. 720p web version (< 8 MB): two-pass at a fixed bitrate
"$FF" -y -loglevel error -i out/master.mkv -vf scale=1280:720:flags=lanczos -c:v libx264 -profile:v high \
  -preset slow -b:v 2600k -pass 1 -passlogfile out/x264web -pix_fmt yuv420p -an -f mp4 /dev/null
"$FF" -y -loglevel error -i out/master.mkv -i out/audio.wav -map 0:v -map 1:a \
  -vf scale=1280:720:flags=lanczos -c:v libx264 -profile:v high -preset slow -b:v 2600k -pass 2 \
  -passlogfile out/x264web -pix_fmt yuv420p -c:a aac -b:a 128k -t 20 -movflags +faststart out/NimbleMap-launch-720p-web.mp4
rm -f out/x264web*

# 5. poster frame (logo end card, before the fade) + contact sheet every 0.5 s
node render.mjs stills 19.2 out/poster_tmp
mv out/poster_tmp/f_19.200.png out/NimbleMap-launch-poster.png && rmdir out/poster_tmp
"$FF" -y -loglevel error -i out/NimbleMap-launch-1080p60.mp4 -vf "fps=2,scale=384:-1,tile=8x5:padding=4:color=black" \
  -frames:v 1 out/contact-sheet.png

# exFAT drives grow AppleDouble files; remove ours
find . -name '._*' -delete 2>/dev/null || true
ls -la out/*.mp4 out/*.png
