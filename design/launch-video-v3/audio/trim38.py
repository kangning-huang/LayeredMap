"""v3.8's sound: v3.7's masters made on edit/edl38.py's VO (audio/synth36.py with LM_SYNTH_EDL=edl38:
the new first line placed at SHIFT + 0.30 on v3.7's clock), with their first SHIFT seconds removed,
so everything from v3.7 9.9 s on (narration, key clicks, music) comes SHIFT earlier with its
picture. What is removed is the start of the folder's sustained pad chord (before the music's first
downbeat, with no other sound in it), so v3.8 opens inside that chord with a short fade-in. The
masters are brought back to -14 LUFS integrated, true peak <= -1 dBTP; the voice track keeps its level.

    python audio/trim38.py out/seg/full-v3.8.wav out/audio-v3.8.wav
reads <in>.wav, <in>-music-only.wav, <in>-voice.wav, <in>-vo.txt; writes the same four for <out>.
"""
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(os.path.dirname(HERE), 'edit'))
import edl38 as V8  # noqa: E402
from trim import master, read, write  # noqa: E402

FADE_IN = 0.25    # s


def cut(x, sr):
    y = x[int(round(V8.SHIFT * sr)):].copy()
    n = int(FADE_IN * sr)
    u = np.sin(np.linspace(0, 1, n) * np.pi / 2)
    y[:n] *= u[:, None] if y.ndim == 2 else u
    return y


def main(src, dst):
    base, out = src[:-4], dst[:-4]
    for suffix, remaster in (('', True), ('-music-only', True), ('-voice', False)):
        sr, x = read(base + suffix + '.wav')
        y = cut(x, sr)
        if remaster:
            y, lufs, tp = master(y, sr)
            print(f'wrote {out + suffix}.wav: {len(y) / sr:.2f} s, {lufs:.2f} LUFS integrated, true peak {tp:.2f} dBTP')
        else:
            print(f'wrote {out + suffix}.wav: {len(y) / sr:.2f} s')
        write(out + suffix + '.wav', sr, y)
    with open(out + '-vo.txt', 'w') as f:
        for line in open(base + '-vo.txt'):
            vid, a, b = line.split('\t')
            a, b = float(a) - V8.SHIFT, float(b) - V8.SHIFT
            if a < 0:
                raise SystemExit(f'{vid} starts before the cut ({a + V8.SHIFT:.2f} < {V8.SHIFT})')
            f.write(f'{vid}\t{a:.3f}\t{b:.3f}\n')
    print(open(out + '-vo.txt').read())


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
