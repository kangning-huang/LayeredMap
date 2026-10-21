"""Smoothness evidence: apparent motion and frame differences of a clip, measured the same way for
any source (v3.6's edit or v3.7's renders), plus the exact camera speed of a v3.7 path.

    python path/metrics.py video <clip.mp4> <t0> <t1> <out.csv>      # from a film's output
    python path/metrics.py frames <dir> <out.csv>                       # a folder of PNGs (30 fps)
    python path/metrics.py plot <out.png> <title> <label=csv> [...]

Per frame: `diff` = mean absolute grey difference to the previous frame (0-255); `flow` = median
optical-flow displacement (Farneback, on a 704-px-wide grey copy, in px of that copy) - the
apparent speed of the picture. A smooth move is a smooth curve with no spikes, plateaus at zero
in the middle of a move, or sudden jumps.
"""
import csv
import glob
import os
import subprocess
import sys

import cv2
import numpy as np

W = 704


def grey_frames_video(p, t0, t1):
    h = None
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-ss', str(t0), '-t', str(t1 - t0), '-i', p, '-vf',
                          f'fps=30,scale={W}:-2', '-f', 'rawvideo', '-pix_fmt', 'gray', '-'], capture_output=True).stdout
    probe = subprocess.run(['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries', 'stream=width,height',
                            '-of', 'csv=p=0', p], capture_output=True, text=True).stdout.strip().split(',')
    w0, h0 = int(probe[0]), int(probe[1])
    h = int(round(h0 * W / w0 / 2) * 2)
    f = np.frombuffer(raw, np.uint8).reshape(-1, h, W)
    return [t0 + i / 30 for i in range(len(f))], list(f)


def grey_frames_dir(d):
    out = []
    for p in sorted(glob.glob(os.path.join(d, '*.png'))):
        im = cv2.imread(p, cv2.IMREAD_GRAYSCALE)
        out.append(cv2.resize(im, (W, int(round(im.shape[0] * W / im.shape[1]))), interpolation=cv2.INTER_AREA))
    return [i / 30 for i in range(len(out))], out


def measure(ts, fr):
    rows = []
    for i in range(1, len(fr)):
        a, b = fr[i - 1], fr[i]
        diff = float(np.abs(a.astype(np.float32) - b.astype(np.float32)).mean())
        flow = cv2.calcOpticalFlowFarneback(a, b, None, 0.5, 4, 21, 3, 5, 1.1, 0)
        mag = np.hypot(flow[..., 0], flow[..., 1])
        rows.append((ts[i], diff, float(np.median(mag)), float(np.percentile(mag, 90))))
    return rows


def write(rows, out):
    with open(out, 'w', newline='') as f:
        w = csv.writer(f)
        w.writerow(['t', 'diff', 'flow', 'flow_p90'])
        w.writerows([[f'{x:.4f}' for x in r] for r in rows])


def plot(out, title, series):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(2, 1, figsize=(11, 6.2), sharex=False)
    for label, path in series:
        r = np.loadtxt(path, delimiter=',', skiprows=1, ndmin=2)
        t = r[:, 0] - r[0, 0]
        ax[0].plot(t, r[:, 2], label=label, lw=1.6)
        ax[1].plot(t, r[:, 1], label=label, lw=1.6)
    ax[0].set_ylabel('apparent speed\n(median flow, px/frame)')
    ax[1].set_ylabel('frame difference\n(mean |Δgrey|)')
    ax[1].set_xlabel('seconds from the start of the clip')
    for a in ax:
        a.grid(alpha=0.3)
        a.legend(frameon=False)
    ax[0].set_title(title)
    fig.tight_layout()
    fig.savefig(out, dpi=130)


if __name__ == '__main__':
    mode = sys.argv[1]
    if mode == 'video':
        write(measure(*grey_frames_video(sys.argv[2], float(sys.argv[3]), float(sys.argv[4]))), sys.argv[5])
    elif mode == 'frames':
        write(measure(*grey_frames_dir(sys.argv[2])), sys.argv[3])
    elif mode == 'plot':
        plot(sys.argv[2], sys.argv[3], [a.split('=', 1) for a in sys.argv[4:]])
