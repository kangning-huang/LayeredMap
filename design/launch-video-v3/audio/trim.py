"""v3.7.1-zh's sound: v3.7-zh's masters (audio/synth36.py) with the span TRIM_AT..TRIM_OLD of the
old timeline (edit/edl371zh.py) removed, so everything after
it, narration, key clicks and music, comes SHIFT earlier with its picture. The span is inside the
folder's sustained pad chord, before the music's first downbeat and between two narration lines,
so the join is a short equal-power crossfade inside one held chord. The masters are brought back to
-14 LUFS integrated, true peak <= -1 dBTP; the voice-only track keeps its level.

    python audio/trim.py out/seg/full-zh.wav out/audio-v3.7.1-zh.wav
reads <in>.wav, <in>-music-only.wav, <in>-voice.wav, <in>-vo.txt; writes the same four for <out>.
"""
import os
import sys

import numpy as np
import pyloudnorm as pyln
from scipy import signal
from scipy.io import wavfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), 'edit'))
import edl371zh as TR  # noqa: E402

XF = 0.4          # crossfade, s
CUT_FROM = TR.TRIM_AT     # the span removed, in the old timeline (the folder line ended at 5.23 s,
CUT_TO = TR.TRIM_OLD      # the next line's audio starts at 9.91 s)


def cut(x, sr):
    """x without [CUT_FROM, CUT_TO): the XF before each edge is crossfaded (equal power), so the
    removed length is exactly CUT_TO - CUT_FROM and nothing after CUT_TO (the next line's first
    word) is touched."""
    a, b = int(round(CUT_FROM * sr)), int(round(CUT_TO * sr))
    n = int(XF * sr)
    u = np.linspace(0, 1, n)[:, None] if x.ndim == 2 else np.linspace(0, 1, n)
    mid = x[a - n:a] * np.cos(u * np.pi / 2) + x[b - n:b] * np.sin(u * np.pi / 2)
    return np.concatenate([x[:a - n], mid, x[b:]])


def master(x, sr):
    meter = pyln.Meter(sr)
    x = pyln.normalize.loudness(x, meter.integrated_loudness(x), -14.0)
    tp = np.max(np.abs(signal.resample_poly(x, 4, 1, axis=0)))
    if tp > 10 ** (-1.0 / 20):
        x = x * 10 ** (-1.0 / 20) / tp
    return x, meter.integrated_loudness(x), 20 * np.log10(np.max(np.abs(signal.resample_poly(x, 4, 1, axis=0))))


def read(p):
    sr, x = wavfile.read(p)
    return sr, x.astype(np.float64) / 32768.0


def write(p, sr, x):
    wavfile.write(p, sr, (np.clip(x, -1, 1) * 32767).astype(np.int16))


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
            a, b = float(a), float(b)
            if a >= CUT_TO - 0.1:
                a, b = a - TR.SHIFT, b - TR.SHIFT
            elif b > CUT_FROM:
                raise SystemExit(f'{vid} ({a}-{b}) overlaps the cut {CUT_FROM}-{CUT_TO}')
            f.write(f'{vid}\t{a:.3f}\t{b:.3f}\n')
    print(open(out + '-vo.txt').read())


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
