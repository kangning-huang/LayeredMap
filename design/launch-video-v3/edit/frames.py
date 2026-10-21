"""Frame cache: the few source frames the film uses, decoded once from the CFR copies (cfr/*.mov)
into cache/<src>/<t ms>.png, and an LRU reader over them.

    python edit/frames.py          # extract everything edl.NEEDED lists (skips what exists)
    LM_CUT=3.1 python edit/frames.py   # the same for v3.1 (edit/edl31.py)

The film is built mostly from held frames (each is one real frame of the app) and a few short
runs (the dive, the click, the card appearing), so only a few hundred frames are ever read.
"""
import os
import subprocess
import sys
from collections import OrderedDict

import cv2
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
if os.environ.get('LM_CUT') == '3.1':
    import edl31 as edl  # noqa: E402
elif os.environ.get('LM_CUT') == '3.2':
    import edl32 as edl  # noqa: E402
elif os.environ.get('LM_CUT') == '3.3':
    import edl33 as edl  # noqa: E402
elif os.environ.get('LM_CUT') == '3.4':
    import edl34 as edl  # noqa: E402
elif os.environ.get('LM_CUT') == '3.5':
    import edl35 as edl  # noqa: E402
elif os.environ.get('LM_CUT') == '3.6':
    import edl36 as edl  # noqa: E402
else:
    import edl  # noqa: E402

CACHE = os.path.join(ROOT, 'cache')


def wanted():
    """[(src, t)] for every frame listed in edl.NEEDED (singles and ranges at a rate)."""
    out = []
    for src, items in edl.NEEDED.items():
        for it in items:
            if isinstance(it, tuple):
                a, b, fps = it
                n = int(round((b - a) * fps))
                out += [(src, round(a + k / fps, 4)) for k in range(n + 1)]
            else:
                out.append((src, float(it)))
    return out


def path(src, t):
    return os.path.join(CACHE, src, f'{int(round(t * 1000)):07d}.png')


def extract():
    by = {}
    for src, t in wanted():
        by.setdefault(src, set()).add(t)
    for src, ts in by.items():
        os.makedirs(os.path.join(CACHE, src), exist_ok=True)
        todo = sorted(t for t in ts if not os.path.exists(path(src, t)))
        print(src, len(ts), 'frames,', len(todo), 'to extract', flush=True)
        for t in todo:
            # CFR source at 60 fps: seek to the frame centre so rounding never lands a frame late
            subprocess.run(['ffmpeg', '-v', 'error', '-y', '-ss', f'{t + 1 / 120:.4f}', '-i',
                            os.path.join(ROOT, 'cfr', src + '.mov'), '-frames:v', '1',
                            '-compression_level', '1', path(src, t)], check=True)


class Frames:
    """Nearest cached frame of a source at time t, content-cropped, RGB uint8 (LRU)."""

    def __init__(self, cap=24):
        self.lru = OrderedDict()
        self.cap = cap
        self.index = {}
        for src, t in wanted():
            self.index.setdefault(src, set()).add(t)
        self.index = {k: np.array(sorted(v)) for k, v in self.index.items()}

    def get(self, src, t):
        ts = self.index[src]
        tt = float(ts[np.argmin(np.abs(ts - t))])
        k = (src, tt)
        if k in self.lru:
            self.lru.move_to_end(k)
            return self.lru[k]
        im = cv2.imread(path(src, tt), cv2.IMREAD_COLOR)[:, :, ::-1]
        x, y, w, h = edl.SOURCES[src]['crop']
        im = np.ascontiguousarray(im[y:y + h, x:x + w])
        self.lru[k] = im
        if len(self.lru) > self.cap:
            self.lru.popitem(last=False)
        return im


if __name__ == '__main__':
    extract()
