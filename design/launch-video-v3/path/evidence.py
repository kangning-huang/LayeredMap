"""Smoothness evidence for v3.7 against v3.6, and the owner's side-by-side clip.

    python path/evidence.py <v3.6 1080p mp4> <v3.7 1080p mp4> <out dir>

Writes into <out dir>:
  camera-speed-v3.7.png / .csv   the exact map-camera speed of every v3.7 shot (from its keyframed
                                 path): screen motion in px/frame (2x window pixels, 30 fps) and
                                 zoom rate in levels/s, with the largest frame-to-frame jump
  window-*.png / .csv            the same for the map window alone (v3.6 rebuilt through its own
                                 compositor; v3.7's renders)
  ny-*.png, tokyo-*.png / .csv   apparent speed (optical flow) and frame difference of both films
                                 over the New York and Tokyo landings, measured the same way
  summary.txt                    the numbers
  NimbleMap-launch-v3.6-vs-v3.7-landings.mp4   side by side: v3.6 | v3.7, New York then Tokyo
"""
import csv
import math
import os
import subprocess
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(ROOT))
sys.path.insert(0, HERE)
import metrics  # noqa: E402
import shots as SH  # noqa: E402
from camera import screen_motion  # noqa: E402

FPS = 30
CLIPS = {'ny': (19.6, 23.6, 'New York · search and landing'), 'tokyo': (31.9, 35.4, 'Tokyo · search and landing · Tohoku')}
SHOT_RANGES = {'ty': (15.9, 28.6), 'eq': (31.35, 40.3), 'hw': (40.3, 50.4), 'pro': (51.0, 69.6)}
FONT = os.path.join(REPO, 'assets/fonts/Inter-SemiBold.ttf')


def camera_speed(out):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    rows, lines = [], []
    fig, ax = plt.subplots(2, 1, figsize=(12, 6.4))
    for name, (a, b) in SHOT_RANGES.items():
        ch = SH.CHAINS[name]()
        ts = np.arange(int(round(a * FPS)), int(round(b * FPS))) / FPS
        px = np.array([screen_motion(ch.at(t), ch.at(t + 1 / FPS)) * 2 for t in ts])      # 2x pixels
        zr = np.array([(ch.at(t + 1 / FPS).lonlatz()[2] - ch.at(t).lonlatz()[2]) * FPS for t in ts])
        acc = np.abs(np.diff(px))
        for t, p, z in zip(ts, px, zr):
            rows.append((name, round(t, 4), round(p, 3), round(z, 4)))
        lines.append(f'{name:4s} {a:5.2f}-{b:5.2f} s: largest frame-to-frame camera move {px.max():6.1f} px '
                     f'({px.max() / 2818 * 100:4.2f}% of the window width), largest change of that move between '
                     f'frames {acc.max():5.2f} px; slowest frame {px.min():5.2f} px (never stops: drift); '
                     f'fastest zoom {np.abs(zr).max():4.2f} levels/s')
        ax[0].plot(ts, px, label=name)
        ax[1].plot(ts, zr, label=name)
    ax[0].set_ylabel('map motion\n(px/frame, 2x window)')
    ax[1].set_ylabel('zoom rate\n(levels/s)')
    ax[1].set_xlabel('film time (s)')
    for x in ax:
        x.grid(alpha=0.3)
        x.legend(frameon=False, ncol=4)
    ax[0].set_title('v3.7: exact map-camera speed of every shot (from the keyframed paths)')
    fig.tight_layout()
    fig.savefig(os.path.join(out, 'camera-speed-v3.7.png'), dpi=130)
    with open(os.path.join(out, 'camera-speed-v3.7.csv'), 'w', newline='') as f:
        w = csv.writer(f)
        w.writerow(['shot', 't', 'motion_px_per_frame', 'zoom_levels_per_s'])
        w.writerows(rows)
    return lines


def film_metrics(v36, v37, out):
    lines = []
    for key, (a, b, title) in CLIPS.items():
        paths = []
        for label, v in (('v3.6', v36), ('v3.7', v37)):
            p = os.path.join(out, f'{key}-{label}.csv')
            metrics.write(metrics.measure(*metrics.grey_frames_video(v, a, b)), p)
            r = np.loadtxt(p, delimiter=',', skiprows=1)
            fl, d = r[:, 2], r[:, 1]
            jerk = np.abs(np.diff(fl))
            lines.append(f'{key:5s} {label}: apparent speed max {fl.max():5.2f} px/frame (704-px copy), largest '
                         f'frame-to-frame change of speed {jerk.max():5.2f}, frames frozen (<0.02 px) '
                         f'{int((fl < 0.02).sum())}/{len(fl)}, largest frame difference {d.max():5.1f}')
            paths.append((label, p))
        metrics.plot(os.path.join(out, f'{key}-speed.png'), f'{title} ({a}-{b} s of the film, whole frame)', paths)
    return lines


WINDOWS = {'ny': ('film1', 'ty', 19.7, 23.3), 'tokyo': ('film2', 'eq', 31.9, 34.9)}


def window36(film, a, b, dst):
    """v3.6's window picture for film time a..b, through v3.6's own compositor (bar typing
    included), content only (no title bar), one PNG per frame."""
    sys.path.insert(0, os.path.join(ROOT, 'edit'))
    import compose36 as S36
    os.makedirs(dst, exist_ok=True)
    keys = {'film1': ('TYPE1', 'LABEL1', 'T_ENTER1', 18.45, 'g3_ty'), 'film2': ('TYPE2', 'LABEL2', 'T_ENTER2', 29.17, 'g_eqL')}
    ty, lab, ent, t0, bar = keys[film]
    for n in range(int(round(a * FPS)), int(round(b * FPS)) + 1):
        x = S36.N.t33(n / FPS)
        win = S36.C.window_image(getattr(S36.Q, film)(x), False)
        q, cur, ph, hf = S36.B.bar_state(x, getattr(S36.E, ty), getattr(S36.E, lab), getattr(S36.E, ent), t0)
        win = S36.C.draw_query(win, bar, q, cur, placeholder=ph, hint_fade=hf)
        import cv2
        cv2.imwrite(os.path.join(dst, f'{n:05d}.png'), np.ascontiguousarray(win[64:, :, ::-1]), [cv2.IMWRITE_PNG_COMPRESSION, 1])


def window_metrics(out):
    """The map window alone (what the camera moves), v3.6's edit against v3.7's renders."""
    import shutil
    lines = []
    for key, (film, shot, a, b) in WINDOWS.items():
        tmp36, tmp37 = os.path.join(out, f'_w36-{key}'), os.path.join(out, f'_w37-{key}')
        subprocess.run([sys.executable, '-I', os.path.abspath(__file__), 'win36', film, str(a), str(b), tmp36], check=True,
                       cwd=ROOT)
        os.makedirs(tmp37, exist_ok=True)
        for n in range(int(round(a * FPS)), int(round(b * FPS)) + 1):
            shutil.copyfile(os.path.join(ROOT, 'render', shot, 'frames', f'{n:05d}.png'), os.path.join(tmp37, f'{n:05d}.png'))
        paths = []
        for label, d in (('v3.6', tmp36), ('v3.7', tmp37)):
            p = os.path.join(out, f'window-{key}-{label}.csv')
            metrics.write(metrics.measure(*metrics.grey_frames_dir(d)), p)
            r = np.loadtxt(p, delimiter=',', skiprows=1)
            fl = r[:, 2]
            lines.append(f'{key:5s} window {label}: apparent speed max {fl.max():5.2f} px/frame (704-px copy), largest '
                         f'frame-to-frame change of speed {np.abs(np.diff(fl)).max():5.2f}, frames with the map '
                         f'frozen (<0.02 px) {int((fl < 0.02).sum())}/{len(fl)}')
            paths.append((label, p))
        metrics.plot(os.path.join(out, f'window-{key}-speed.png'), f'{CLIPS[key][2]}: the map window alone ({a}-{b} s)',
                     paths)
        shutil.rmtree(tmp36, ignore_errors=True)
        shutil.rmtree(tmp37, ignore_errors=True)
    return lines


def side_by_side(v36, v37, out):
    parts = []
    for key, (a, b, title) in CLIPS.items():
        a, b = round(a - 0.4, 3), round(b + 0.4, 3)
        dst = os.path.join(out, f'_sbs-{key}.mp4')
        lab = (f"drawtext=fontfile='{FONT}':fontsize=30:fontcolor=white:box=1:boxcolor=black@0.55:boxborderw=12:"
               f"x=24:y=24:text=")
        fc = (f"[0:v]trim={a}:{b},setpts=PTS-STARTPTS,scale=960:540,{lab}'v3.6'[l];"
              f"[1:v]trim={a}:{b},setpts=PTS-STARTPTS,scale=960:540,{lab}'v3.7'[r];"
              f"[l][r]hstack,pad=1920:1080:0:270:color=0x111111,"
              f"drawtext=fontfile='{FONT}':fontsize=40:fontcolor=white:x=(w-tw)/2:y=110:text='{title}'[v]")
        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', v36, '-i', v37, '-filter_complex', fc, '-map', '[v]',
                        '-an', '-r', str(FPS), '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-pix_fmt',
                        'yuv420p', dst], check=True)
        parts.append(dst)
    lst = os.path.join(out, '_sbs.txt')
    with open(lst, 'w') as f:
        f.write(''.join(f"file '{p}'\n" for p in parts))
    final = os.path.join(out, 'NimbleMap-launch-v3.6-vs-v3.7-landings.mp4')
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-f', 'concat', '-safe', '0', '-i', lst, '-c', 'copy',
                    '-movflags', '+faststart', final], check=True)
    for p in parts + [lst]:
        os.remove(p)
    return final


if __name__ == '__main__':
    if sys.argv[1] == 'win36':
        window36(sys.argv[2], float(sys.argv[3]), float(sys.argv[4]), sys.argv[5])
        sys.exit(0)
    v36, v37, out = (os.path.abspath(a) for a in sys.argv[1:4])
    os.makedirs(out, exist_ok=True)
    lines = camera_speed(out) + [''] + window_metrics(out) + [''] + film_metrics(v36, v37, out)
    clip = side_by_side(v36, v37, out)
    with open(os.path.join(out, 'summary.txt'), 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print('\n'.join(lines))
    print(clip)
