"""Blank-tile check for a language version: the map in render/<shot><sfx>/frames should match the
English render of the same frame (same camera, same data) everywhere except the interface. Prints
frames where a 64-px block of the map differs strongly (a tile drawn empty, a patch missing).

    python path/tilecheck.py _zh ty eq hw pro
"""
import os
import sys

import cv2
import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
B = 64


def ui_mask(h, w):
    m = np.zeros((h, w), bool)
    m[:110] = True                      # toolbar
    m[110:640, :640] = True             # layer shelf
    m[110:, w - 760:] = True            # inspector / cards
    m[h - 150:, :] = True               # slider, scale bar, credits
    m[1000:1600, 750:2400] = True       # search results, Ask box and answer card, chooser
    return m


def check(sfx, shot):
    d0, d1 = os.path.join(ROOT, 'render', shot, 'frames'), os.path.join(ROOT, 'render', shot + sfx, 'frames')
    bad = []
    for f in sorted(x for x in os.listdir(d1) if x[:5].isdigit()):
        a, b = cv2.imread(os.path.join(d0, f)), cv2.imread(os.path.join(d1, f))
        if a is None or b is None:
            continue
        h, w = a.shape[:2]
        diff = np.abs(a.astype(np.int16) - b.astype(np.int16)).max(axis=2).astype(np.float32)
        diff[ui_mask(h, w)] = 0
        blocks = diff[:h // B * B, :w // B * B].reshape(h // B, B, w // B, B).mean(axis=(1, 3))
        hot = blocks > 30
        n = int(hot.sum())
        if n >= 4:
            # which side is blank: the flatter one (an empty tile has no texture)
            def flat(im):
                g = cv2.cvtColor(im, cv2.COLOR_BGR2GRAY).astype(np.float32)[:h // B * B, :w // B * B]
                sd = g.reshape(h // B, B, w // B, B).std(axis=(1, 3))
                return float(sd[hot].mean())
            fa, fb = flat(a), flat(b)
            side = sfx if fb < fa * 0.6 else ('en' if fa < fb * 0.6 else '?')
            bad.append((f[:5], n, side))
    print(shot + sfx, len(bad), 'frames differ;', 'blank in', {k: sum(1 for x in bad if x[2] == k) for k in (sfx, 'en', '?')})
    return bad


if __name__ == '__main__':
    for s in sys.argv[2:]:
        check(sys.argv[1], s)
