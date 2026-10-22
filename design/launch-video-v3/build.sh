#!/bin/bash
# Render the v3 launch film ("one take") or v3.1 (the folder of files) from the window recordings
# in raw/ (see README.md).
#   ./build.sh [parallel jobs, default 6] [cut: 3 (default) | 3.1 … 3.6 | 3.7]
# v3.7 first needs its map shots: path/render.sh ty eq hw pro ty_plate eq_plate
set -euo pipefail
cd "$(dirname "$0")"
PY="${PY:-$HOME/miniforge3/envs/lm-video/bin/python}"
J="${1:-6}"
CUT="${2:-3}"
if [ "$CUT" = "3.1" ]; then COMP=edit/compose31.py; EDL=edl31; SUF=v3.1; elif [ "$CUT" = "3.2" ]; then COMP=edit/compose32.py; EDL=edl32; SUF=v3.2; elif [ "$CUT" = "3.3" ]; then COMP=edit/compose33.py; EDL=edl33; SUF=v3.3; elif [ "$CUT" = "3.4" ]; then COMP=edit/compose34.py; EDL=edl34; SUF=v3.4; elif [ "$CUT" = "3.5" ]; then COMP=edit/compose35.py; EDL=edl35; SUF=v3.5; elif [ "$CUT" = "3.6" ]; then COMP=edit/compose36.py; EDL=edl36; SUF=v3.6; elif [ "$CUT" = "3.7" ]; then COMP=edit/compose37.py; EDL=edl36; SUF=v3.7; else COMP=edit/compose.py; EDL=edl; SUF=v3; fi
export LM_CUT=$CUT
# v3.7 is v3.6's timeline (edl36) with the map shots rendered offline (path/render.sh)
[ "$CUT" = "3.7" ] && export LM_CUT=3.6
PT=30.5; [ "$CUT" = "3.2" ] && PT=44.0; [ "$CUT" = "3.3" ] && PT=68.0; [ "$CUT" = "3.4" ] && PT=74.0; [ "$CUT" = "3.5" ] && PT=78.3; [ "$CUT" = "3.6" ] && PT=78.3; [ "$CUT" = "3.7" ] && PT=78.3
FPS=30
mkdir -p out/seg cfr
# constant-frame-rate copies (ScreenCaptureKit writes frames only on change; exact seeks need CFR)
for f in raw/*.mov; do
  n=$(basename "$f" .mov)
  [ cfr/$n.mov -nt "$f" ] || ffmpeg -v error -y -i "$f" -vf fps=60 -c:v libx264 -preset fast -crf 12 -g 30 -pix_fmt yuv420p cfr/$n.mov
done
# the few hundred source frames the film reads
$PY edit/frames.py
DUR=$($PY -c "import sys; sys.path.insert(0,'edit'); import $EDL as e; print(e.DUR)")
pids=()
for k in $(seq 0 $((J - 1))); do
  a=$($PY -c "print(round($DUR*$k/$J*$FPS)/$FPS)")
  b=$($PY -c "print(round($DUR*($k+1)/$J*$FPS)/$FPS)")
  $PY $COMP out/seg/s$k.mkv --fps $FPS --from "$a" --to "$b" > out/seg/s$k.log 2>&1 &
  pids+=($!)
done
for p in "${pids[@]}"; do wait "$p"; done
: > out/seg/list.txt
for k in $(seq 0 $((J - 1))); do echo "file 's$k.mkv'" >> out/seg/list.txt; done
ffmpeg -v error -y -f concat -safe 0 -i out/seg/list.txt -c copy out/master-$SUF.mkv
# sound (v3.2: narration + ducked music, and the music alone; captions)
if [ "$CUT" = "3.2" ] || [ "$CUT" = "3.3" ] || [ "$CUT" = "3.4" ] || [ "$CUT" = "3.5" ] || [ "$CUT" = "3.6" ] || [ "$CUT" = "3.7" ]; then
  # narration: edge-tts (en-US-AndrewNeural, +4%), rendered only when missing; VO_ENGINE=say (v3.2)
  # uses macOS say instead
  if [ "$CUT" = "3.6" ] || [ "$CUT" = "3.7" ]; then export VO_SCRIPT=narration34.txt VO_OUT=vo_edge34; SYN=audio/synth36.py
  elif [ "$CUT" = "3.5" ]; then export VO_SCRIPT=narration34.txt VO_OUT=vo_edge34; SYN=audio/synth35.py
  elif [ "$CUT" = "3.4" ]; then export VO_SCRIPT=narration34.txt VO_OUT=vo_edge34; SYN=audio/synth34.py
  elif [ "$CUT" = "3.3" ]; then export VO_SCRIPT=narration33.txt VO_OUT=vo_edge33; SYN=audio/synth33.py
  else export VO_SCRIPT=narration.txt VO_OUT=vo_edge; SYN=audio/synth32.py; fi
  if [ "${VO_ENGINE:-edge}" = say ]; then audio/voice.sh Samantha 158 > /dev/null
  elif ! ls audio/$VO_OUT/*.mp3 > /dev/null 2>&1; then "$HOME/miniforge3/bin/mamba" run -n lm-video python audio/voice_edge.py > /dev/null; fi
  $PY $SYN out/audio-$SUF.wav
  $PY audio/srt.py out/audio-$SUF-vo.txt out/NimbleMap-launch-$SUF-1080p.srt > /dev/null
  ffmpeg -v error -y -i out/master-$SUF.mkv -i out/audio-$SUF-music-only.wav -map 0:v -map 1:a \
    -c:v libx264 -preset slow -crf 16 -pix_fmt yuv420p -profile:v high -r $FPS \
    -colorspace bt709 -color_primaries bt709 -color_trc bt709 -vf "scale=out_color_matrix=bt709" \
    -c:a aac -b:a 256k -ar 48000 -movflags +faststart -shortest out/NimbleMap-launch-$SUF-music-only.mp4
  [ -f out/audio-$SUF-voice.wav ] && ffmpeg -v error -y -i out/audio-$SUF-voice.wav -c:a libmp3lame -b:a 192k out/NimbleMap-launch-$SUF-narration.mp3
else
  $PY audio/synth.py out/audio-$SUF.wav
fi
# 1080p: H.264 High, yuv420p (BT.709), 30 fps, AAC, faststart
ffmpeg -v error -y -i out/master-$SUF.mkv -i out/audio-$SUF.wav -map 0:v -map 1:a \
  -c:v libx264 -preset slow -crf 16 -pix_fmt yuv420p -profile:v high -r $FPS \
  -colorspace bt709 -color_primaries bt709 -color_trc bt709 -vf "scale=out_color_matrix=bt709" \
  -c:a aac -b:a 256k -ar 48000 -movflags +faststart -shortest out/NimbleMap-launch-$SUF-1080p.mp4
# muted web loop: 1280x720, 30 fps, no audio, about 3 MB. Its last 0.5 s cross-fades into its own
# first 0.5 s (end card -> the empty bar), so it loops without a jump; two-pass to a size budget.
XF=0.5
LOOP="[0:v]scale=1280:-2:flags=lanczos,format=yuv420p,split[a][b];[a]trim=start=$XF,setpts=PTS-STARTPTS[body];[b]trim=0:$XF,setpts=PTS-STARTPTS[head];[body][head]xfade=transition=fade:duration=$XF:offset=$($PY -c "print($DUR-2*$XF)"),format=yuv420p[v]"
BR=$($PY -c "print(int(2.9*8*1024*1024/($DUR-0.5)/1000))")k
ffmpeg -v error -y -i out/master-$SUF.mkv -filter_complex "$LOOP" -map "[v]" -an \
  -c:v libx264 -preset slow -b:v "$BR" -pass 1 -passlogfile out/seg/pass -f mp4 /dev/null
ffmpeg -v error -y -i out/master-$SUF.mkv -filter_complex "$LOOP" -map "[v]" -an \
  -c:v libx264 -preset slow -b:v "$BR" -pass 2 -passlogfile out/seg/pass -profile:v high -movflags +faststart \
  out/NimbleMap-launch-$SUF-web-loop.mp4
# poster: the end frame (the three films over the bar, the name written, the line)
$PY $COMP out/poster --stills $PT && mv out/poster/t$(printf %06.2f $PT).png out/NimbleMap-launch-$SUF-poster.png && rmdir out/poster
# contact sheet, one frame per second
ffmpeg -v error -y -i out/NimbleMap-launch-$SUF-1080p.mp4 -vf "fps=1,scale=480:-1,tile=6x12" -frames:v 1 out/contact-sheet-$SUF.png
find . -name '._*' -delete 2>/dev/null || true
ls -la out/*$SUF*
