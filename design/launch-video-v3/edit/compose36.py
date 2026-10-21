"""NimbleMap launch film v3.5: compositor (v3.4's, re-cut on the v3.5 takes: every move played sped up
to its fully loaded frame, then that frame held; regional framing; gentler push-ins).
Originally: NimbleMap launch film v3.4: compositor. v3.3's compositor (edit/compose33.py) on a warped clock
(edit/edl34.py WARP), fed by the v3.4 takes (fixed build, sharp basemap holds), with Barcelona 2022
for the heat beat and a native Pro section (one spatial question about Tokyo).

    python edit/compose34.py out/seg.mkv [--from 0 --to 74.6]
    python edit/compose34.py out/stills --stills 1.0,4.6,...

Moves are the app's own flights at their real (or half) speed, then a short dissolve to the
moment the view has settled, so no held frame shows tiles still loading.
"""
import argparse
import math
import os
import subprocess
import sys

os.environ['LM_CUT'] = '3.6'
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import edl36 as N  # noqa: E402
import edl33 as E  # noqa: E402
import frames  # noqa: E402

# ---- v3.3's layout constants, set to the v3.4 takes before compose33 reads them
_l2814 = {'rect': (784, 84, 1741, 148), 'x': 847.5, 'base': 125, 'fill': (245, 245, 247),
          'ink': (29, 29, 31), 'hint': (90, 95, 103)}
_l2818 = dict(E.BAR['g3_ty'])
E.BAR = dict(E.BAR, g3_ty=_l2818, g_eqL=_l2818, g_hw=_l2818)
E.PW, E.PH = 2814, 1754
E.ASK = {'x': 1160, 'base': 1648, 'clear': (1156, 1620, 2236, 1662), 'size': 27.7, 'bg': (253, 253, 253),
         'ink': (29, 29, 31)}
E.PRO_CARD = (1080, 784, 2322, 1559)
E.T_PRO2, E.T_SEND2 = (1e9, 1e9), 1e9
E.T_SELECT, E.T_LIFT_TIF, E.T_OPEN_TIF = 9.90, 10.15, 11.00
E.EQ_MAG = (2098, 410, 2772, 590)
E.TOHOKU_PX = (1619, 673)                    # the Tohoku dot in the regional view (140.6, 36.4, z6.4)
E.PALERMO_PX = (1165, 977)                   # Barcelona, in the regional view (5.5, 41.6, z5.5)
E.SANDY_PX = (1375, 1020)                    # Sandy's landfall, regional view (-74.5, 39.8, z6.4)
E.FIND = dict(E.FIND, x=E.FIND['x'] + 4, clear=(E.FIND['clear'][0] + 4,) + E.FIND['clear'][1:])
E.T_FIND = (20.60, 20.85)
E.HW_VALUE = (2096, 345, 2788, 530)
E.HW_TRACK = (1333, 1628, 1960)
E.HW_SLIDER_PANEL = (1076, 1583, 2321, 1681)
E.HW_LAST = N.HW_LAST
E.hw_step = N.hw_step
E.TYPE3, E.LABEL3 = ('Barcelona', [N.t33(x) for x in N.TYPE3[1]]), 'Barcelona'
E.FILES = [(n, s) if not n.startswith('GDO_') else ('GDO_heatwave_intensity_JJA_2022.nc', '6.8 MB') for n, s in E.FILES]
E.SOURCES = N.SOURCES
E.NEEDED = N.NEEDED
frames.edl = N

import compose33 as Q  # noqa: E402
import compose32 as P  # noqa: E402
import compose31 as B  # noqa: E402
import compose as C  # noqa: E402

frames.edl = N
C.edl = E
C.FR = frames.Frames()
for m in (P, B, Q):
    m.E = E
P.T = Q.T = E

import cv2  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image  # noqa: E402

OW, OH = C.OW, C.OH
ease, ease_out, smooth, ramp, whip = C.ease, C.ease_out, C.smooth, C.ramp, C.whip
_get = C.get

# names v3.3's render uses for single frames -> the v3.4 takes
ALIAS = {('g_eqL', 40.0): ('g5_eq', 152.63), ('g_hw', 168.0): ('g5_hw', 303.9), ('f_tif', 7.0): ('f4_tif', 9.0),
         ('f_csv', 7.0): ('f4_csv', 10.0)}


def get(src, t):
    s, tt = ALIAS.get((src, t), (src, t))
    return _get(s, tt)


Q.get = get
_xf = C.xfade


def xfade(a, b, u):
    """The v3.4 takes differ by a few px in width (2814/2818): fit b to a before mixing."""
    if a.shape != b.shape:
        b = C.fit(b, a.shape[1], a.shape[0])
    return _xf(a, b, u)


C.xfade = Q.C.xfade = xfade

# text in v3.3's overlays -> v3.4
TEXT = {'36.50 °C': '36.10 °C',
        'heat-wave intensity, Palermo, 25 Jul 2023 · higher than 93% of cells':
            'heat-wave intensity, Barcelona, 16 Aug 2022 · higher than 98% of cells',
        'Heatwaves, summer 2023 · 92 daily steps': 'Heatwaves, summer 2022 · 92 daily steps'}
_cap, _big = C.caption, C.big_words


def caption(im, text, *a, **k):
    return _cap(im, TEXT.get(text, text), *a, **k)


def big_words(im, text, *a, **k):
    return _big(im, TEXT.get(text, text), *a, **k)


C.caption, C.big_words = caption, big_words


def settle(src, t, a, play, b, rate=1.0, fade=0.3):
    """A move: source a.. at `rate` for `play` s, then a dissolve to the settled frame b."""
    if t < play:
        return _get(src, a + t * rate)
    return C.xfade(_get(src, a + play * rate), _get(src, b), ease(ramp(t, play, play + fade)))


# ---------------------------------------------------------------- the films (v3.3 time)
def ty_plate():
    return C.plate_with_keep('ty_plate4', 9.0, 'g5_tyA', 19.5, E.TY_KEEP)


def move(src, t, t0, dur, a, b, ref, fade=0.0):
    """A move played from source a to its first fully loaded frame b in `dur` (v3.3 s), then the
    loaded reference frame of its hold."""
    u = t - t0
    if u < dur:
        return _get(src, a + (b - a) * u / dur)
    return _get(src, ref)


def film1(t):
    T = E
    A = _get('g5_tyA', 19.5)
    if t < T.T_REVEAL1[0]:
        return ty_plate()
    if t < T.T_REVEAL1[1]:
        return C.reveal(ty_plate(), A, C.ty_threshold(*A.shape[:2]), C.ease_in(ramp(t, *T.T_REVEAL1)) ** 0.6, 0.05)
    if t < T.T_ZOOM:
        return A
    if t < T.T_PAN:                  # the real zoom, sped up to its loaded frame
        return move('g5_tyA', t, T.T_ZOOM, 0.75, 20.0, 21.83, 50.83)
    if t < T.T_ENTER1:               # the real pan
        return move('g5_tyA', t, T.T_PAN, 0.55, 51.33, 52.40, 82.37)
    if t < 19.85:                    # the flight to New York City (the search's landing), sped up
        return move('g5_tyA', t, T.T_ENTER1, 0.25, 82.87, 85.83, 113.87)
    if t < T.T_FIND[0]:              # zoom out to the region (z6.5)
        return move('g5_tyA', t, 19.85, 0.35, 114.37, 117.53, 145.4)
    if t < T.T_FIND[1]:
        return C.xfade(_get('g5_tyA', 145.4), find_frame(t), ease(ramp(t, T.T_FIND[0], T.T_FIND[1])))
    if t < T.T_FINDKEY:
        return find_frame(t)
    if t < T.T_SANDY:                # "2 found", the map zooms to both
        return move('g5_tyB', t, T.T_FINDKEY, 1.6, 11.73, 15.70, 29.0)
    return C.xfade(_get('g5_tyB', 29.0), _get('g5_tyB', 91.97), ease(ramp(t, T.T_SANDY, T.T_SANDY + 0.45)))


def find_frame(t):
    img = _get('g5_tyB', 9.5)
    if t < E.TYPE_FIND[1][0] - 0.4:
        return img
    q = C.typed(E.TYPE_FIND, t)
    last = max([E.TYPE_FIND[1][0] - 0.4] + [k for k in E.TYPE_FIND[1] if k <= t])
    return Q.draw_field(img, E.FIND, q, C.blink(t, last), title_offset=E.TITLE)


def film1_loop(p):
    if p < 0.4:
        return _get('g5_tyB', 29.0)
    if p < 0.5:
        return C.xfade(_get('g5_tyB', 29.0), _get('g5_tyB', 91.97), (p - 0.4) / 0.1)
    return _get('g5_tyB', 91.97)


def eq_plate():
    return C.plate_with_keep('eq_plate4', 9.0, 'g5_eq', 31.73, E.EQ_KEEP)


def film2(t):
    T = E
    Ef = _get('g5_eq', 31.73)
    if t < T.T_RING[0]:
        return eq_plate()
    if t < T.T_RING[1]:
        return C.reveal(eq_plate(), Ef, B.eq_thr(*Ef.shape[:2]), ramp(t, T.T_RING[0], T.T_RING[1] - 0.05), 0.015)
    if t < T.T_ENTER2:
        return Ef
    if t < 30.10:                    # the flight to Tokyo (the search's landing), sped up
        return move('g5_eq', t, T.T_ENTER2, 0.22, 32.23, 36.70, 63.27)
    if t < T.T_JAPAN:                # zoom out to the region (z6.4)
        return move('g5_eq', t, 30.10, 0.30, 63.77, 65.50, 94.77)
    return C.xfade(_get('g5_eq', 94.77), _get('g5_eq', 152.63), ease(ramp(t, T.T_JAPAN, T.T_JAPAN + 0.3)))


def film2_loop(p):
    Ef = _get('g5_eq', 31.73)
    if p < 0.5:
        return C.reveal(eq_plate(), Ef, B.eq_thr(*Ef.shape[:2]), p / 0.5, 0.015)
    if p < 0.6:
        return C.xfade(Ef, _get('g5_eq', 152.63), (p - 0.5) / 0.1)
    if p > 0.95:
        return C.xfade(_get('g5_eq', 152.63), eq_plate(), (p - 0.95) / 0.05)
    return _get('g5_eq', 152.63)


def film3(t):
    T = E
    if t < T.T_ENTER3:
        return _get('g5_hw', N.hw_step(Q.hw_index(t)))
    if t < T.T_PALERMO:              # the search's flight begins, then a cut (dissolve) to the
        # loaded regional view: the flight (tiles loading) and the city-zoom landing are cut out
        return C.xfade(_get('g5_hw', N.hw_step(N.HW_LAST)), _get('g5_hw', 276.0), ease(ramp(t, T.T_ENTER3, T.T_PALERMO)))
    return C.xfade(_get('g5_hw', 276.0), _get('g5_hw', 303.9), ease(ramp(t, T.T_PALERMO, T.T_PALERMO + 0.3)))


def film3_loop(p):
    if p < 0.6:
        return _get('g5_hw', N.hw_step(int(N.HW_LAST * p / 0.6 + 1e-6)))
    if p < 0.7:
        return C.xfade(_get('g5_hw', N.hw_step(N.HW_LAST)), _get('g5_hw', 303.9), (p - 0.6) / 0.1)
    return _get('g5_hw', 303.9)


def film4(t):
    """The Pro window, in v3.3 time outside the native section: the question not yet asked
    before it, the answer after it."""
    return _get('p7_ty', 28.0) if t < 50.0 else _get('p7_ty', 137.03)


Q.film1, Q.film1_loop, Q.film2, Q.film2_loop, Q.film4 = film1, film1_loop, film2, film2_loop, film4
B.film3, B.film3_loop = film3, film3_loop

# ---- the end row: four films (storms, earthquakes, heatwaves, Pro)
Q.WIDTHS = [2814, 2814, 2818, E.PW * Q.S4]
Q.ZE = 0.13
Q.GAP = 40 / Q.ZE
Q.ROW_W = sum(Q.WIDTHS) + 3 * Q.GAP
Q.GX = (Q.W3C[0] + Q.W4C[0]) / 2 - 1500
Q.CE = (Q.GX, Q.GY - (300 - 540) / Q.ZE)
C.CE = P.CE = Q.CE
C.ZE = P.ZE = Q.ZE
Q.Z = Q.ZE / Q.X1


def row_tl(i):
    return (Q.GX - Q.ROW_W / 2 + sum(Q.WIDTHS[:i]) + i * Q.GAP, Q.GY - E.H / 2)


Q.row_tl = row_tl
_ws = Q.window_state


def window_state(t):
    out = _ws(t)
    out[4] = ((1e7, 1e7), 1.0)          # v3.3's second Pro window is not in this cut
    return out


Q.window_state = window_state
# camera: the tif beat later (the longer line); the Pro section ends on the Pro window; the row
cam = []
for k in Q.CAM:
    if k[0] in (9.30, 9.65, 10.40):
        k = ({9.30: 9.80, 9.65: 10.15, 10.40: 10.90}[k[0]],) + k[1:]
    if 50.3 <= k[0] <= 59.40 or k[0] >= 61.9:
        continue
    cam.append(k)
cam += [(59.0, Q.W4C[0], Q.W4C[1], 1.0, 0, ease), (59.4, Q.W4C[0], Q.W4C[1], 0.85, 0, ease),
        (61.9, Q.CE[0], Q.CE[1], Q.Z, 0, ease), (E.T_PEN[0], Q.CE[0], Q.CE[1], Q.Z, 0, ease),
        (E.T_PEN[1], Q.CE[0], Q.CE[1] + 30 / Q.ZE, Q.Z * 1.03, 0, ease),
        (69.0, Q.CE[0], Q.CE[1] + 30 / Q.ZE, Q.Z * 1.03, 0, ease)]
cam.sort(key=lambda k: k[0])
Q.CAM = [(k[0],) + k[1:3] + (min(k[3], 1.85),) + k[4:] for k in cam]
_cam = []
for k in Q.CAM:                       # the New York beat: the region holds before the Find field
    if k[0] == 20.35:
        k = (20.60,) + k[1:]
    if k[0] == 20.85:
        k = (20.85,) + k[1:3] + (1.75,) + k[4:]
    _cam.append(k)
Q.CAM = _cam


# ---------------------------------------------------------------- the native Pro section (v3.4 time)
P0 = N.PRO0
ASKP = (Q.TL4[0] + 1500, Q.TL4[1] + 1500)
CARDP = (Q.TL4[0] + 1700, Q.TL4[1] + 1170)
CIRC = (Q.TL4[0] + 1700, Q.TL4[1] + 480)
PCAM = [
    (P0 + 0.0, Q.W4C[0], Q.W4C[1], 1.0, 0, ease),
    (P0 + 3.2, Q.W4C[0] + 30, Q.W4C[1], 1.03, 0, ease),
    (P0 + 3.7, ASKP[0], ASKP[1], 1.7, 0, ease),                  # the Ask box (sharp, loaded)
    (P0 + 8.0, ASKP[0] + 100, ASKP[1], 1.74, 0, ease),           # the question typed
    (P0 + 8.75, Q.W4C[0], Q.W4C[1], 1.0, 0, ease),               # sent; the app zooms to the answer
    (P0 + 10.6, Q.W4C[0], Q.W4C[1], 1.0, 0, ease),               # 7 tracks + the 100 km circle
    (P0 + 12.0, CIRC[0], CIRC[1], 1.35, 0, ease),                # around Tokyo
    (P0 + 13.3, CIRC[0] + 20, CIRC[1], 1.37, 0, ease),
    (P0 + 14.3, CARDP[0], CARDP[1], 1.6, 0, ease),               # the card: 7 lines
    (N.PRO1 - 0.6, CARDP[0] + 20, CARDP[1], 1.62, 0, ease),
    (N.PRO1, Q.W4C[0], Q.W4C[1], 1.0, 0, ease),
]


def pro_film(t):
    """The plan is replayed (LM_ASK_FIXTURE), so the answer arrives at once: the box clears on
    send, then the answer and the app's zoom to it play to the loaded frame, which is held."""
    s = N.T_SEND
    if t < s:
        return _get('p7_ty', 28.0)
    if t < s + 0.8:
        return _get('p7_ty', 30.4)
    if t < s + 1.3:
        return _get('p7_ty', 30.67 + (t - s - 0.8) * (1.65 / 0.5))
    return _get('p7_ty', 137.03)


def render_pro(t):
    B.CAM = PCAM
    cam = B.camera(t)
    canvas = C.table(cam)
    img = C.window_image(pro_film(t), False)
    if P0 + 3.3 < t < N.T_SEND:
        q = C.typed(N.TYPE4, t)
        last = max([P0 + 3.35] + [k for k in N.TYPE4[1] if k <= t])
        img = Q.draw_field(img, E.ASK, q, C.blink(t, last))
    M = C.local_to_screen(cam, Q.TL4, 1.0)
    C.draw_window(canvas, img, M, shadow=1.0)
    im = Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8)).convert('RGBA')
    C.caption(im, 'Pro: ask in plain words', t, P0 - 2.0, P0 + 9.0, pos='tr')
    return np.asarray(im.convert('RGB'))


def render(t):
    x = N.t33(t)
    if x is None:
        return render_pro(t)
    B.CAM = Q.CAM
    return Q.render(x)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--fps', type=int, default=N.FPS)
    ap.add_argument('--from', dest='t0', type=float, default=0.0)
    ap.add_argument('--to', dest='t1', type=float, default=N.DUR)
    ap.add_argument('--stills', default='')
    a = ap.parse_args()
    if a.stills:
        os.makedirs(a.out, exist_ok=True)
        for t in [float(v) for v in a.stills.split(',')]:
            Image.fromarray(render(t)).save(os.path.join(a.out, f't{t:06.2f}.png'))
            print('still', t, flush=True)
        return
    n0, n1 = int(round(a.t0 * a.fps)), int(round(a.t1 * a.fps))
    ff = subprocess.Popen(['ffmpeg', '-v', 'error', '-y', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', f'{OW}x{OH}',
                           '-r', str(a.fps), '-i', '-', '-c:v', 'ffv1', '-level', '3', '-pix_fmt', 'yuv444p', a.out],
                          stdin=subprocess.PIPE)
    for n in range(n0, n1):
        ff.stdin.write(render(n / a.fps).tobytes())
        if n % 30 == 0:
            print(f'frame {n}/{n1}', flush=True)
    ff.stdin.close()
    ff.wait()


if __name__ == '__main__':
    main()
