#!/bin/bash
# Render the v2 launch film from the raw window recordings in raw/ (see README.md).
#   ./build.sh [parallel jobs, default 6]
set -euo pipefail
cd "$(dirname "$0")"
PY="${PY:-$HOME/miniforge3/envs/lm-video/bin/python}"
J="${1:-6}"
FPS=60
mkdir -p out/seg cfr
# constant-frame-rate copies of the recordings (ScreenCaptureKit writes frames only on change, so
# seeking in the raw files lands late); exact seeking needs CFR
for f in raw/*.mov; do
  n=$(basename "$f" .mov)
  [ cfr/$n.mov -nt "$f" ] || ffmpeg -v error -y -i "$f" -vf fps=$FPS -c:v libx264 -preset fast -crf 12 -g 30 -pix_fmt yuv420p cfr/$n.mov
done
DUR=$($PY -c "import sys; sys.path.insert(0,'edit'); import edl; print(edl.DUR)")
# picture: J segments in parallel, each a pure function of t
pids=()
for k in $(seq 0 $((J - 1))); do
  a=$($PY -c "print(round($DUR*$k/$J*$FPS)/$FPS)")
  b=$($PY -c "print(round($DUR*($k+1)/$J*$FPS)/$FPS)")
  $PY edit/compose.py out/seg/s$k.mkv --fps $FPS --from "$a" --to "$b" > out/seg/s$k.log 2>&1 &
  pids+=($!)
done
for p in "${pids[@]}"; do wait "$p"; done
: > out/seg/list.txt
for k in $(seq 0 $((J - 1))); do echo "file 's$k.mkv'" >> out/seg/list.txt; done
ffmpeg -v error -y -f concat -safe 0 -i out/seg/list.txt -c copy out/master.mkv
# sound
$PY audio/synth.py out/audio.wav
# 1080p master: H.264 High, yuv420p (BT.709), AAC, faststart
ffmpeg -v error -y -i out/master.mkv -i out/audio.wav -map 0:v -map 1:a \
  -c:v libx264 -preset slow -crf 16 -pix_fmt yuv420p -profile:v high \
  -colorspace bt709 -color_primaries bt709 -color_trc bt709 -vf "scale=out_color_matrix=bt709" \
  -c:a aac -b:a 256k -ar 48000 -movflags +faststart -shortest out/NimbleMap-launch-v2-1080p.mp4
# muted web loop (<= 4 MB): 1280x720, 30 fps, no audio. Its last 0.5 s cross-fades into its own
# first 0.5 s (end card -> hook), so it loops without a jump; two-pass to a size budget.
XF=0.5
LOOP="[0:v]scale=1280:-2:flags=lanczos,fps=30,format=yuv420p,split[a][b];[a]trim=start=$XF,setpts=PTS-STARTPTS[body];[b]trim=0:$XF,setpts=PTS-STARTPTS[head];[body][head]xfade=transition=fade:duration=$XF:offset=$($PY -c "print($DUR-2*$XF)"),format=yuv420p[v]"
BR=$($PY -c "print(int(3.7*8*1024*1024/($DUR-0.5)/1000))")k
ffmpeg -v error -y -i out/master.mkv -filter_complex "$LOOP" -map "[v]" -an \
  -c:v libx264 -preset slow -b:v "$BR" -pass 1 -passlogfile out/seg/pass -f mp4 /dev/null
ffmpeg -v error -y -i out/master.mkv -filter_complex "$LOOP" -map "[v]" -an \
  -c:v libx264 -preset slow -b:v "$BR" -pass 2 -passlogfile out/seg/pass -profile:v high -movflags +faststart \
  out/NimbleMap-launch-v2-web-loop.mp4
# poster: the Haiyan card moment
$PY edit/compose.py out/poster --stills 4.6 && mv out/poster/t004.60.png out/NimbleMap-launch-v2-poster.png && rmdir out/poster
# contact sheet, one frame per second
ffmpeg -v error -y -i out/NimbleMap-launch-v2-1080p.mp4 -vf "fps=1,scale=480:-1,tile=6x6" -frames:v 1 out/contact-sheet.png
ls -la out/*.mp4 out/*.png
