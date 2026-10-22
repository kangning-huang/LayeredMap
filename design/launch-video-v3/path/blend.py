"""Motion blur: average each output frame's sub-frames (render/<shot>/sub/NNNNN_k.png, rendered
across a 180-degree shutter) into render/<shot>/frames/NNNNN.png. Frames with one sub-frame are
copied as they are.

    python path/blend.py <shot> [...]
"""
import glob
import os
import shutil
import sys
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor

import cv2
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
RENDER = os.path.join(os.path.dirname(HERE), 'render')


def blend(paths, out):
    if len(paths) == 1:
        shutil.copyfile(paths[0], out)
        return
    acc = None
    for p in paths:
        im = cv2.imread(p, cv2.IMREAD_COLOR).astype(np.float32)
        # average in linear light, so bright labels blur as they would through a shutter
        lin = (im / 255.0) ** 2.2
        acc = lin if acc is None else acc + lin
    im = (acc / len(paths)) ** (1 / 2.2) * 255.0
    cv2.imwrite(out, np.clip(im + 0.5, 0, 255).astype(np.uint8), [cv2.IMWRITE_PNG_COMPRESSION, 1])


def run(shot):
    sub = os.path.join(RENDER, shot, 'sub')
    dst = os.path.join(RENDER, shot, 'frames')
    os.makedirs(dst, exist_ok=True)
    by = defaultdict(list)
    for p in sorted(glob.glob(os.path.join(sub, '[0-9]*_*.png'))):
        by[os.path.basename(p).split('_')[0]].append(p)
    todo = []
    for n, ps in sorted(by.items()):
        out = os.path.join(dst, n + '.png')
        if os.path.exists(out) and os.path.getmtime(out) >= max(os.path.getmtime(p) for p in ps):
            continue
        todo.append((ps, out))
    with ThreadPoolExecutor(8) as ex:
        list(ex.map(lambda a: blend(*a), todo))
    print(f'{shot}: {len(by)} frames ({len(todo)} blended)')


if __name__ == '__main__':
    for s in sys.argv[1:]:
        run(s)
