"""Captions for the v3.2 narration: audio/narration.txt (caption text) x out/audio-v3.2-vo.txt
(each line's measured start/end on the timeline, written by audio/synth32.py).
    python audio/srt.py out/audio-v3.2-vo.txt out/NimbleMap-launch-v3.2-1080p.srt"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
caps = {}
for line in open(os.path.join(HERE, os.environ.get('VO_SCRIPT', 'narration.txt')), encoding='utf-8'):
    if line.startswith('#') or '|' not in line:
        continue
    vid, _, cap = [p.strip() for p in line.split('|')]
    caps[vid] = cap


def ts(s):
    ms = int(round(s * 1000))
    return f'{ms // 3600000:02d}:{ms // 60000 % 60:02d}:{ms // 1000 % 60:02d},{ms % 1000:03d}'


rows = [line.split('\t') for line in open(sys.argv[1]) if line.strip()]
out = []
for k, (vid, a, b) in enumerate(rows):
    end = float(b) + 0.25
    if k + 1 < len(rows):
        end = min(end, float(rows[k + 1][1]) - 0.05)     # captions never overlap
    out.append(f'{k + 1}\n{ts(float(a))} --> {ts(end)}\n{caps[vid]}\n')
open(sys.argv[2], 'w', encoding='utf-8').write('\n'.join(out))
print(open(sys.argv[2], encoding='utf-8').read())
