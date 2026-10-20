"""NimbleMap launch film v3.3: compositor for the richer script (edit/edl33.py, audio/narration33.txt).
Reuses v3.2/v3.1/v3 helpers (edit/compose32.py, compose31.py, compose.py).

    python edit/compose33.py out/seg.mkv [--from 0 --to 69]
    python edit/compose33.py out/stills --stills 1.0,4.6,...

New over v3.2: the folder names each file type as it is spoken (a tint on its rows); the GeoTIFF
and the CSV open in quick succession before the storms; a real zoom and pan; the place search
("New York") and the layer's real Find field ("Sandy": 2 found); a second Pro question on the
earthquake GeoPackage; five films in the end row.
"""
import argparse
import math
import os
import subprocess
import sys

os.environ['LM_CUT'] = '3.3'
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import edl33 as E  # noqa: E402
import frames  # noqa: E402
import compose32 as P  # noqa: E402
import compose31 as B  # noqa: E402
import compose as C  # noqa: E402

for m in (P, B):
    m.E = E
P.T = E
C.edl = E
frames.edl = E
C.FR = frames.Frames()

import cv2  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image, ImageDraw  # noqa: E402

OW, OH = C.OW, C.OH
X1 = E.X1
font, FONT_REG = C.font, C.FONT_REG
ease, ease_out, ease_in, smooth, whip, ramp = C.ease, C.ease_out, C.ease_in, C.smooth, C.whip, C.ramp
get = C.get
ACCENT = C.ACCENT
T = E
CHIP_S = 1.0

# ----------------------------------------------------------------- layout
TL1, TL2, TL3 = E.TL1, E.TL2, E.TL3
TL4 = (TL3[0] + E.W + 900.0, TL3[1] - 200.0)
TL5 = (TL4[0] + E.PW + 700.0, TL4[1])
W1C = (TL1[0] + 1409, TL1[1] + 877)
W2C, W3C = B.W2C, B.W3C
W4C = (TL4[0] + E.PW / 2, TL4[1] + E.PH / 2)
W5C = (TL5[0] + E.PW2 / 2, TL5[1] + E.PH2 / 2)
FC = (E.FTL[0] + E.FOLDER_W * E.FSCALE / 2, E.FTL[1] + E.FOLDER_H * E.FSCALE / 2)
MIDC = (E.TIF_TL[0] + 2822 * E.MS / 2, (E.TIF_TL[1] + E.CSV_TL[1] + 1758 * E.MS) / 2)


def bar_c(tl, src, dx=-150.0):
    x0, y0, x1, y1 = E.BAR[src]['rect']
    return (tl[0] + (x0 + x1) / 2 + dx, tl[1] + (y0 + y1) / 2)


B1, B2, B3 = bar_c(TL1, 'g3_ty'), bar_c(TL2, 'g_eqL'), bar_c(TL3, 'g_hw')
FIND1 = (TL1[0] + 2440, TL1[1] + 470)
CARD1 = (TL1[0] + 2440, TL1[1] + 560)
CARD2 = (TL2[0] + 2440, TL2[1] + 560)
CARD3 = (TL3[0] + 2440, TL3[1] + 470)
ASK4 = (TL4[0] + 1300, TL4[1] + 1430)
ASK5 = (TL5[0] + 1300, TL5[1] + 1430)
CARD4 = (TL4[0] + (E.PRO_CARD[0] + E.PRO_CARD[2]) / 2, TL4[1] + (E.PRO_CARD[1] + E.PRO_CARD[3]) / 2)
CARD5 = (TL5[0] + (E.PRO2_CARD[0] + E.PRO2_CARD[2]) / 2, TL5[1] + (E.PRO2_CARD[1] + E.PRO2_CARD[3]) / 2)
C3 = E.CHIP3_C
MID23 = (W2C[0], (TL2[1] + C3[1] + 90) / 2)
S4, S5 = E.H / E.PH, E.H / E.PH2
WIDTHS = [2818, 2822, 2822, E.PW * S4, E.PW2 * S5]
ZE = 0.106
GAP = 40 / ZE
ROW_W = sum(WIDTHS) + 4 * GAP
GX, GY = W4C[0], W3C[1]
CE = (GX, GY - (300 - 540) / ZE)
C.CE, C.ZE = CE, ZE
P.CE, P.ZE = CE, ZE


def row_tl(i):
    return (GX - ROW_W / 2 + sum(WIDTHS[:i]) + i * GAP, GY - E.H / 2)


Z = ZE / X1
CAM = [
    (0.00, FC[0], FC[1], 0.84, 0, ease),
    (9.30, FC[0] + 60, FC[1] + 40, 0.97, 0, ease),            # a slow push on the folder
    (9.65, FC[0] + 60, FC[1] + 40, 0.97, 0, ease),
    (10.40, MIDC[0], MIDC[1], 0.64, 0, whip),                  # follow the .tif to its window
    (12.50, MIDC[0] + 40, MIDC[1], 0.66, 0, ease),             # .tif, then .csv open
    (13.00, W1C[0], W1C[1], 0.95, 0, whip),                    # the .shp: the storm window
    (13.60, W1C[0], W1C[1], 0.88, 0, ease),
    (16.40, W1C[0] + 40, W1C[1] + 30, 0.97, -0.6, ease),       # tracks
    (17.90, W1C[0], W1C[1], 1.0, 0, ease),                     # the real zoom and pan
    (18.35, B1[0], B1[1], 2.0, 0, ease),                       # onto the bar
    (19.50, B1[0] + 60, B1[1], 2.06, 0, ease),                 # "New York"
    (20.00, W1C[0], W1C[1], 1.0, 0, ease),                     # the flight there
    (20.35, W1C[0], W1C[1], 1.0, 0, ease),
    (20.85, FIND1[0], FIND1[1], 1.9, 0, ease),                 # onto the layer's Find field
    (21.95, FIND1[0] + 30, FIND1[1], 1.95, 0, ease),           # "Sandy"
    (22.70, W1C[0], W1C[1], 0.98, 0, ease),                    # both storms named Sandy
    (24.20, W1C[0], W1C[1], 1.0, 0, ease),
    (24.85, CARD1[0], CARD1[1], 1.45, 0, ease),                # Sandy (2012) and its card
    (25.20, CARD1[0] - 30, CARD1[1], 1.42, 0, ease),
    (25.60, W2C[0], W2C[1], 1.35, 0, whip),                    # travel with the .gpkg
    (26.10, W2C[0], W2C[1], 1.35, 0, ease),
    (26.70, W2C[0], W2C[1], 1.02, 0, ease),
    (28.60, W2C[0] - 40, W2C[1] - 30, 1.1, 0, ease),           # the ring
    (29.15, B2[0], B2[1], 2.0, 0, ease),
    (29.85, B2[0] + 40, B2[1], 2.05, 0, ease),                 # "Tokyo"
    (30.35, W2C[0], W2C[1], 1.0, 0, ease),
    (30.70, W2C[0], W2C[1], 1.0, 0, ease),
    (31.20, W2C[0] + 40, W2C[1], 1.08, 0, ease),               # click a dot: the Tohoku card
    (31.80, CARD2[0], CARD2[1], 1.7, 0, ease),
    (32.40, CARD2[0] - 30, CARD2[1], 1.68, 0, ease),
    (35.30, CARD2[0] - 30, CARD2[1], 1.68, 0, ease),
    (35.70, MID23[0], MID23[1], 0.655, 0, ease),               # film 2 + the .nc
    (36.30, MID23[0], MID23[1], 0.655, 0, ease),
    (36.65, TL3[0] + 1150, TL3[1] + 1150, 1.0, 0, whip),       # DROP: whip along the line
    (38.60, TL3[0] + 1400, TL3[1] + 1100, 1.1, 0, ease),
    (39.90, TL3[0] + 1650, TL3[1] + 1500, 1.7, 0, ease),       # "scrub through time": the slider
    (40.30, B3[0], B3[1], 2.0, 0, ease),
    (40.85, B3[0] + 50, B3[1], 2.06, 0, ease),                 # "Palermo"
    (41.25, W3C[0], W3C[1], 1.0, 0, ease),
    (41.65, W3C[0], W3C[1], 1.0, 0, ease),
    (42.10, W3C[0] + 200, W3C[1] - 60, 1.1, 0, ease),          # Sicily + the card
    (42.55, CARD3[0], CARD3[1], 1.62, 0, ease),
    (44.90, CARD3[0] - 40, CARD3[1], 1.55, 0, ease),
    (45.60, W4C[0], W4C[1], 1.25, 0, whip),                    # Pro: the storm shapefile
    (46.30, W4C[0], W4C[1], 1.0, 0, ease),
    (48.50, W4C[0], W4C[1], 1.03, 0, ease),
    (48.95, ASK4[0], ASK4[1], 1.8, 0, ease),                   # the Ask box
    (50.30, ASK4[0] + 60, ASK4[1], 1.85, 0, ease),
    (50.95, W4C[0], W4C[1], 1.02, 0, ease),                    # the answer on the map
    (51.80, CARD4[0], CARD4[1] - 40, 1.45, 0, ease),           # "228 of 4,999"
    (53.40, CARD4[0] - 30, CARD4[1] - 40, 1.4, 0, ease),
    (54.10, W5C[0], W5C[1], 1.25, 0, whip),                    # Pro: the earthquake GeoPackage
    (54.60, ASK5[0], ASK5[1], 1.75, 0, ease),
    (56.40, ASK5[0] + 80, ASK5[1], 1.8, 0, ease),
    (57.05, W5C[0], W5C[1], 1.02, 0, ease),                    # deep quakes highlighted
    (57.80, CARD5[0], CARD5[1] - 60, 1.45, 0, ease),           # "655 of 14,537"
    (59.00, CARD5[0] - 30, CARD5[1] - 60, 1.4, 0, ease),
    (59.40, W5C[0], W5C[1], 0.85, 0, ease),
    (61.90, CE[0], CE[1], Z, 0, ease),                         # pull back over the row
    (T.T_PEN[0], CE[0], CE[1], Z, 0, ease),
    (T.T_PEN[1], CE[0], CE[1] + 30 / ZE, Z * 1.03, 0, ease),
    (E.DUR, CE[0], CE[1] + 30 / ZE, Z * 1.03, 0, ease),
]


def handheld(t):
    a = smooth(ramp(t, 26.6, 26.9)) * (1 - smooth(ramp(t, 28.5, 28.7)))
    if a <= 0:
        return 0.0, 0.0, 0.0
    return (a * (9 * math.sin(t * 1.7) + 5 * math.sin(t * 3.1 + 1)),
            a * (7 * math.sin(t * 1.3 + 2) + 4 * math.sin(t * 2.7)), a * 0.25 * math.sin(t * 0.9))


def camera(t):
    B.CAM = CAM
    B.handheld = handheld
    return B.camera(t)


# ----------------------------------------------------------------- the films
def ty_plate():
    return C.plate_with_keep('ty_plate', 8.0, 'g3_ty', 11.0, E.TY_KEEP)


def film1(t):
    A = get('g3_ty', 11.0)
    if t < T.T_REVEAL1[0]:
        return ty_plate()
    if t < T.T_REVEAL1[1]:
        return C.reveal(ty_plate(), A, C.ty_threshold(*A.shape[:2]), ease_in(ramp(t, *T.T_REVEAL1)) ** 0.6, 0.05)
    if t < T.T_ZOOM:
        return A
    # the real zoom and pan, at half speed while they move; the tiles that load in behind them
    # (blank for a moment in the recording) are covered by a short dissolve to the settled view
    if t < T.T_ZOOM + 0.55:
        return get('g3_ty', 12.85 + (t - T.T_ZOOM) * 0.5)
    if t < T.T_ZOOM + 0.9:
        return C.xfade(get('g3_ty', 13.12), get('g3_ty', 14.0), ease(ramp(t, T.T_ZOOM + 0.55, T.T_ZOOM + 0.9)))
    if t < T.T_PAN:
        return get('g3_ty', 16.0)
    if t < T.T_PAN + 0.4:
        return get('g3_ty', 18.87 + (t - T.T_PAN) * 0.5)
    if t < T.T_PAN + 0.75:
        return C.xfade(get('g3_ty', 19.07), get('g3_ty', 22.0), ease(ramp(t, T.T_PAN + 0.4, T.T_PAN + 0.75)))
    if t < T.T_ENTER1:
        return get('g3_ty', 22.0)
    if t < T.T_ENTER1 + 0.7:         # the flight to New York City (source 24.85-25.55)
        return get('g3_ty', 24.85 + (t - T.T_ENTER1))
    if t < T.T_ENTER1 + 0.9:
        return get('g3_ty', 25.6 + (t - T.T_ENTER1 - 0.7) * 8)
    if t < T.T_FIND[0] + 0.1:
        return get('g3_ty', 30.0)
    if t < T.T_FIND[1]:              # into the Find take (the inspector of the selected layer)
        return C.xfade(get('g3_ty', 30.0), find_frame(t), ease(ramp(t, T.T_FIND[0] + 0.1, T.T_FIND[1])))
    if t < T.T_FINDKEY:
        return find_frame(t)
    if t < T.T_FINDKEY + 1.7:        # "2 found", and the map zooms to both (source 6.90-8.60)
        return get('g3_tyB', 6.90 + (t - T.T_FINDKEY))
    if t < T.T_SANDY:
        return get('g3_tyB', 10.0)
    return C.xfade(get('g3_tyB', 10.0), get('g3_tyB', 30.0), ease(ramp(t, T.T_SANDY, T.T_SANDY + 0.45)))


def find_frame(t):
    """The Find take before its step: the empty field, with the typing drawn into it."""
    img = get('g3_tyB', 5.0)
    if t < T.TYPE_FIND[1][0] - 0.4:
        return img
    q = C.typed(T.TYPE_FIND, t)
    last = max([T.TYPE_FIND[1][0] - 0.4] + [k for k in T.TYPE_FIND[1] if k <= t])
    return draw_field(img, E.FIND, q, C.blink(t, last), title_offset=E.TITLE)


def draw_field(img, spec, text, cursor, title_offset=0):
    a = spec
    im = Image.fromarray(img.copy())
    d = ImageDraw.Draw(im)
    o = title_offset
    x0, y0, x1, y1 = a['clear']
    d.rectangle([x0, y0 - o, x1, y1 - o], fill=a['bg'])
    f = font(FONT_REG, a['size'])
    if text:
        d.text((a['x'], a['base'] - o), text, font=f, fill=a['ink'], anchor='ls')
    if cursor > 0:
        cx = a['x'] + (d.textlength(text, font=f) + 3 if text else 0)
        d.rectangle([cx, a['base'] - o - 22, cx + 14, a['base'] - o + 6], fill=a['ink'])
    return np.asarray(im)


def film1_loop(p):
    if p < 0.4:
        return get('g3_tyB', 10.0)
    if p < 0.5:
        return C.xfade(get('g3_tyB', 10.0), get('g3_tyB', 30.0), (p - 0.4) / 0.1)
    return get('g3_tyB', 30.0)


def eq_plate():
    return C.plate_with_keep('eq_plateL', 8.0, 'g_eqL', 12.5, E.EQ_KEEP)


def film2(t):
    Ef = get('g_eqL', 12.5)
    if t < T.T_RING[0]:
        return eq_plate()
    if t < T.T_RING[1]:
        return C.reveal(eq_plate(), Ef, B.eq_thr(*Ef.shape[:2]), ramp(t, T.T_RING[0], T.T_RING[1] - 0.05), 0.015)
    if t < T.T_ENTER2:
        return Ef
    if t < T.T_ENTER2 + 0.3:
        return get('g_eqL', 15.25 + (t - T.T_ENTER2) * (0.65 / 0.3))
    if t < T.T_ENTER2 + 0.45:
        return get('g_eqL', 15.9 + (t - T.T_ENTER2 - 0.3) * (2.0 / 0.15))
    if t < T.T_JAPAN:
        return get('g_eqL', 18.5)
    return C.xfade(get('g_eqL', 18.5), get('g_eqL', 40.0), ease(ramp(t, T.T_JAPAN, T.T_JAPAN + 0.35)))


def film2_loop(p):
    Ef = get('g_eqL', 12.5)
    if p < 0.5:
        return C.reveal(eq_plate(), Ef, B.eq_thr(*Ef.shape[:2]), p / 0.5, 0.015)
    if p < 0.6:
        return C.xfade(Ef, get('g_eqL', 40.0), (p - 0.5) / 0.1)
    if p > 0.95:
        return C.xfade(get('g_eqL', 40.0), eq_plate(), (p - 0.95) / 0.05)
    return get('g_eqL', 40.0)


def hw_index(t):
    if t < T.T_FILM3 + 0.05:
        return 0
    return min(E.HW_LAST, int(E.HW_LAST * ease(ramp(t, T.T_FILM3 + 0.1, 40.1)) + 1e-6))


B.hw_index = hw_index


def film4(t):
    if t < T.T_SEND:
        return get('p_ty', 12.0)
    if t < T.T_SEND + 0.6:
        return get('p_ty', 12.95 + (t - T.T_SEND) * (3.1 / 0.6))
    if t < T.T_SEND + 0.75:
        return get('p_ty', 16.05 + (t - T.T_SEND - 0.6) * (0.2 / 0.15))
    return get('p_ty', 21.0)


def film5(t):
    if t < T.T_SEND2:
        return get('p_eq', 12.0)
    if t < T.T_SEND2 + 0.6:          # "Planning ..." (the real request, sped up from 1.8 s)
        return get('p_eq', 15.55 + (t - T.T_SEND2) * (1.7 / 0.6))
    if t < T.T_SEND2 + 0.75:
        return get('p_eq', 17.25 + (t - T.T_SEND2 - 0.6) * (0.2 / 0.15))
    return get('p_eq', 22.0)


# ----------------------------------------------------------------- folder pulses
def folder_now(t):
    sel = None
    if T.T_SELECT <= t < T.T_OPEN_TIF + 0.4:
        sel = E.ROW_TIF
    lifted = E.ROW_TIF if t >= T.T_LIFT_TIF else None
    img = P.folder_image(sel, lifted)
    tint = {}
    for t0, rows in E.PULSES:
        a = smooth(ramp(t, t0 - 0.05, t0 + 0.2)) * (1 - smooth(ramp(t, t0 + 1.0, t0 + 1.6)))
        for r in rows:
            tint[r] = max(tint.get(r, 0), a)
    if not any(v > 0.01 for v in tint.values()):
        return img
    out = img.astype(np.float32).copy()
    c = np.array([196, 214, 255], np.float32) / 255
    for r, a in tint.items():
        if a <= 0.01 or r == sel or r == lifted:
            continue
        x0, y0, x1, y1 = B.row_rect(r)
        reg = out[y0:y1 + 2, x0:x1]
        reg *= 1 - a * (1 - c)
    return out.astype(np.uint8)


def fly(canvas, cam, t, row, p0, p1, t0, t1, scale0, scale1, arc=220):
    """A file row flying from p0 to p1 (world), t0 -> t1; it fades as its window opens."""
    if not (t0 - 0.02 <= t < t1 + 0.35):
        return
    u = whip(ramp(t, t0, t1))
    c = (p0[0] + (p1[0] - p0[0]) * u, p0[1] + (p1[1] - p0[1]) * u - math.sin(math.pi * u) * arc)
    a = smooth(ramp(t, t0 - 0.02, t0 + 0.1)) * (1 - smooth(ramp(t, t1, t1 + 0.3)))
    P.draw_chip(canvas, cam, row, c, alpha=a, scale=scale0 + (scale1 - scale0) * u)


def open_window(canvas, cam, img, tl, scale, row, t, t0, dur=0.6, full=None, shadow=1.0):
    """A window opening out of a file row (the row's rect -> the full window)."""
    if t < t0 - 0.02:
        return
    h, w = img.shape[:2]
    full = full or (0, 0, w, h)
    u = ease_out(ramp(t, t0, t0 + dur))
    cw = P.chip_image(row).shape[1] * CHIP_S
    chh = P.chip_image(row).shape[0] * CHIP_S
    cx, cy = w / 2, h / 2
    start = (cx - cw / 2 / scale, cy - chh / 2 / scale, cx + cw / 2 / scale, cy + chh / 2 / scale)
    M = C.local_to_screen(cam, tl, scale)
    C.draw_window(canvas, img, M, rect=C.lerp_rect(start, full, u), rad=20 * (1 - u) + 36 * u, shadow=(0.6 + 0.4 * u) * shadow)


def window_state(t):
    g = ease(ramp(t, T.T_GATHER[0], T.T_GATHER[1] - 0.3))
    out = {}
    for i, (tl, s) in enumerate([(TL1, 1.0), (TL2, 1.0), (TL3, 1.0), (TL4, S4), (TL5, S5)]):
        rt = row_tl(i)
        sc = 1.0 + (s - 1.0) * g
        out[i] = ((tl[0] + (rt[0] - tl[0]) * g, tl[1] + (rt[1] - tl[1]) * g), sc)
    return out


def loop_phase(t, i):
    tt = min(t, T.T_STOP)
    return ((tt - T.T_GATHER[0]) / [3.2, 3.0, 2.8, 3.4, 3.4][i] + [0.6, 0.75, 0.75, 0.8, 0.8][i]) % 1.0


def render(t):
    cam = camera(t)
    canvas = C.table(cam)
    pos = window_state(t)
    LOOP = T.T_GATHER[0] - 0.05

    # ---- the folder
    C.draw_window(canvas, folder_now(t), C.local_to_screen(cam, E.FTL, E.FSCALE), rad=30, shadow=1.0)

    # ---- the GeoTIFF and the CSV, in quick succession (real files, real windows)
    tif_c = (E.TIF_TL[0] + 2822 * E.MS / 2, E.TIF_TL[1] + 1758 * E.MS / 2)
    csv_c = (E.CSV_TL[0] + 2822 * E.MS / 2, E.CSV_TL[1] + 1758 * E.MS / 2)
    if t < T.T_GATHER[1]:
        if t >= T.T_OPEN_TIF:
            open_window(canvas, cam, C.window_image(get('f_tif', 7.0), False), E.TIF_TL, E.MS, E.ROW_TIF, t, T.T_OPEN_TIF)
        if t >= T.T_OPEN_CSV:
            open_window(canvas, cam, C.window_image(get('f_csv', 7.0), False), E.CSV_TL, E.MS, E.ROW_CSV, t, T.T_OPEN_CSV)
    fly(canvas, cam, t, E.ROW_TIF, P.folder_row_centre_world(E.ROW_TIF), tif_c, T.T_LIFT_TIF + 0.2, T.T_OPEN_TIF,
        E.FSCALE, CHIP_S, arc=300)
    fly(canvas, cam, t, E.ROW_CSV, (csv_c[0] - 3600, csv_c[1] + 200), csv_c, T.T_OPEN_CSV - 0.45, T.T_OPEN_CSV,
        CHIP_S, CHIP_S)

    # ---- window 1: storms (shapefile)
    (tl1, s1) = pos[0]
    M1 = C.local_to_screen(cam, tl1, s1)
    if t >= T.T_OPEN1 - 0.02:
        c1 = film1(t) if t < LOOP else film1_loop(loop_phase(t, 0))
        img1 = C.window_image(c1, False)
        q, cur, ph, hf = B.bar_state(t, E.TYPE1, E.LABEL1, T.T_ENTER1, 18.45)
        img1 = C.draw_query(img1, 'g3_ty', q, cur, placeholder=ph, hint_fade=hf)
        open_window(canvas, cam, img1, tl1, s1, E.ROW_SHP, t, T.T_OPEN1, 0.65)
    fly(canvas, cam, t, E.ROW_SHP, (W1C[0] - 3400, W1C[1] + 300), W1C, T.T_LIFT1, T.T_OPEN1, CHIP_S, CHIP_S)

    # ---- window 2: earthquakes (GeoPackage)
    (tl2, s2) = pos[1]
    M2 = C.local_to_screen(cam, tl2, s2)
    fly(canvas, cam, t, E.ROW_GPKG, (W2C[0] - 3400, W2C[1] + 300), W2C, T.T_TRAVEL[0] - 0.05, T.T_OPEN2[0],
        CHIP_S, CHIP_S)
    if t >= T.T_OPEN2[0]:
        c2 = film2(t) if t < LOOP else film2_loop(loop_phase(t, 1))
        img2 = C.window_image(c2, False)
        q, cur, ph, hf = B.bar_state(t, E.TYPE2, E.LABEL2, T.T_ENTER2, 29.17)
        img2 = C.draw_query(img2, 'g_eqL', q, cur, placeholder=ph, hint_fade=hf)
        open_window(canvas, cam, img2, tl2, s2, E.ROW_GPKG, t, T.T_OPEN2[0])

    # the drop line: the .nc row collapses into it; it becomes the slider's track
    if T.T_DROP + 0.06 < t < T.T_FILM3 + 0.45:
        M = C.cam_matrix(cam)
        cw = P.chip_image(E.ROW_NC).shape[1] * CHIP_S * 1.6
        lx0, lx1, ly = C3[0] - cw / 2, C3[0] + cw / 2, C3[1]
        tx0, tx1 = TL3[0] + E.HW_TRACK[0], TL3[0] + E.HW_TRACK[2]
        shoot = whip(ramp(t, T.T_DROP + 0.06, T.T_DROP + 0.3))
        tail = ease(ramp(t, T.T_DROP + 0.1, T.T_DROP + 0.45))
        xa, xb = lx0 + (tx0 - lx0) * tail, lx1 + (tx1 - lx1) * shoot
        fade = 1 - smooth(ramp(t, T.T_DROP + 0.45, T.T_DROP + 0.75))
        p0, p1 = C.apply(M, xa, ly), C.apply(M, xb, ly)
        th = max(2, int(round(9 * cam[2])))
        lay = np.zeros((OH, OW), np.uint8)
        cv2.line(lay, (int(p0[0] * 16), int(p0[1] * 16)), (int(p1[0] * 16), int(p1[1] * 16)), 255, th, cv2.LINE_AA, 4)
        al = (lay.astype(np.float32) / 255 * fade)[..., None]
        canvas *= (1 - al)
        canvas += np.array(ACCENT, np.float32) * al

    # ---- window 3: heatwaves (NetCDF), out of its slider track
    (tl3, s3) = pos[2]
    M3 = C.local_to_screen(cam, tl3, s3)
    if t > T.T_DROP + 0.1:
        c3 = B.film3(t) if t < LOOP else B.film3_loop(loop_phase(t, 2))
        img3 = C.window_image(c3, False)
        q, cur, ph, hf = B.bar_state(t, E.TYPE3, E.LABEL3, T.T_ENTER3, 40.17)
        img3 = C.draw_query(img3, 'g_hw', q, cur, placeholder=ph, hint_fade=hf)
        tx0, ty_, tx1 = E.HW_TRACK
        u1 = ease_out(ramp(t, T.T_DROP + 0.16, T.T_DROP + 0.3))
        u2 = ease_out(ramp(t, T.T_DROP + 0.26, T.T_DROP + 0.62))
        rect = C.lerp_rect(C.lerp_rect((tx0, ty_ - 3, tx1, ty_ + 3), E.HW_SLIDER_PANEL, u1), (0, 0, 2822, 1694 + E.TITLE), u2)
        C.draw_window(canvas, img3, M3, rect=rect, rad=3 + 33 * u2 + 20 * u1 * (1 - u2), shadow=u1)

    # ---- Pro windows: the storm shapefile, then the earthquake GeoPackage
    (tl4, s4) = pos[3]
    M4 = C.local_to_screen(cam, tl4, s4)
    fly(canvas, cam, t, E.ROW_SHP, (W4C[0] - 3400, W4C[1] + 300), W4C, T.T_PRO[0] - 0.05, T.T_PRO[0] + 0.4, CHIP_S, CHIP_S)
    if t >= T.T_PRO[0] + 0.4:
        img4 = C.window_image(film4(t) if t < LOOP else film4(99), False)
        if 48.7 < t < T.T_SEND:
            q = C.typed(E.TYPE4, t)
            last = max([48.75] + [k for k in E.TYPE4[1] if k <= t])
            img4 = draw_field(img4, E.ASK, q, C.blink(t, last))
        open_window(canvas, cam, img4, tl4, s4, E.ROW_SHP, t, T.T_PRO[0] + 0.4, 0.6)
    (tl5, s5) = pos[4]
    M5 = C.local_to_screen(cam, tl5, s5)
    fly(canvas, cam, t, E.ROW_GPKG, (W5C[0] - 3400, W5C[1] + 300), W5C, T.T_PRO2[0] - 0.05, T.T_PRO2[0] + 0.35, CHIP_S, CHIP_S)
    if t >= T.T_PRO2[0] + 0.35:
        img5 = C.window_image(film5(t) if t < LOOP else film5(99), False)
        if 54.5 < t < T.T_SEND2:
            q = C.typed(E.TYPE5, t)
            last = max([54.55] + [k for k in E.TYPE5[1] if k <= t])
            img5 = draw_field(img5, E.ASK2, q, C.blink(t, last))
        open_window(canvas, cam, img5, tl5, s5, E.ROW_GPKG, t, T.T_PRO2[0] + 0.35, 0.55)

    # ---- big-number beats: blur and wash behind
    a2 = smooth(ramp(t, T.T_M91 - 0.05, T.T_M91 + 0.15)) * (1 - smooth(ramp(t, 35.2, 35.45)))
    a3 = smooth(ramp(t, 42.75, 42.95)) * (1 - smooth(ramp(t, 44.75, 45.0)))
    canvas = B.blur_wash(canvas, a2, (250, 249, 246))
    canvas = B.blur_wash(canvas, a3, (250, 249, 246))

    # the .nc row arrives under film 2 (over the wash); at the DROP it collapses into the line
    if 34.8 < t < T.T_DROP + 0.16:
        u = whip(ramp(t, 34.9, 35.3))
        p0 = (C3[0] - 3600, C3[1] + 200)
        c = (p0[0] + (C3[0] - p0[0]) * u, p0[1] + (C3[1] - p0[1]) * u - math.sin(math.pi * u) * 220)
        sq = ease_in(ramp(t, T.T_DROP, T.T_DROP + 0.12))
        P.draw_chip(canvas, cam, E.ROW_NC, c, alpha=smooth(ramp(t, 34.9, 35.0)), scale=CHIP_S * 1.6, squash=sq)

    # ---- motion blur on the fast travels
    c0, c1_ = camera(t - 1 / 60), camera(t + 1 / 60)
    vx, vy = (c1_[0] - c0[0]) * cam[2] / 2, (c1_[1] - c0[1]) * cam[2] / 2
    ln = math.hypot(vx, vy)
    travel = (9.7 <= t <= 10.4) or (12.5 <= t <= 13.0) or (25.2 <= t <= 25.6) or (T.T_DROP <= t <= T.T_FILM3) \
        or (45.2 <= t <= 45.6) or (53.7 <= t <= 54.1)
    if travel and ln > 6:
        L = int(min(120, ln * 0.9))
        ker = np.zeros((L * 2 + 1, L * 2 + 1), np.float32)
        cv2.line(ker, (L - int(vx / ln * L), L - int(vy / ln * L)), (L + int(vx / ln * L), L + int(vy / ln * L)), 1.0, 1)
        canvas = cv2.filter2D(canvas, -1, ker / ker.sum())

    im = Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8)).convert('RGBA')

    # ---- overlays
    C.caption(im, E.FORMATS, t, *T.T_FORMATS)
    C.caption(im, '4,999 storm tracks', t, 13.8, 16.4)
    if T.T_SANDY + 0.25 <= t <= T.T_SANDY + 1.1:
        x, y = C.apply(M1, *E.SANDY_PX)
        C.ripple(im, x, y, (t - T.T_SANDY - 0.3) / 0.75)
    C.caption(im, '14,537 earthquakes since 1900', t, 26.8, 28.9)
    if T.T_JAPAN + 0.05 <= t <= T.T_JAPAN + 0.85:
        x, y = C.apply(M2, *E.TOHOKU_PX)
        C.ripple(im, x, y, (t - T.T_JAPAN - 0.05) / 0.75, r1=110)
    eq40 = C.window_image(get('g_eqL', 40.0), False)
    C.lift_chip(im, (*E.EQ_MAG, eq40), M2, t, 31.95, 32.5, scale_to=1.25)
    C.big_words(im, 'M 9.1.', t, T.T_M91 + 0.02, 35.3, (120, 300), size=230)
    C.big_words(im, 'Tōhoku, 2011 · higher than 99% of 14,537 earthquakes', t, T.T_M91 + 0.15, 35.3, (128, 450), size=46)
    C.caption(im, 'Heatwaves, summer 2023 · 92 daily steps', t, 36.9, 39.0, pos='tr')
    if T.T_PALERMO + 0.1 <= t <= 42.6:
        x, y = C.apply(M3, *E.PALERMO_PX)
        C.ripple(im, x, y, (t - T.T_PALERMO - 0.15) / 0.7)
    hw168 = C.window_image(get('g_hw', 168.0), False)
    C.lift_chip(im, (*E.HW_VALUE, hw168), M3, t, 42.15, 42.75, scale_to=1.2)
    C.big_words(im, '36.50 °C', t, 42.8, 44.95, (130, 600), size=230)
    C.big_words(im, 'heat-wave intensity, Palermo, 25 Jul 2023 · higher than 93% of cells', t, 42.95, 44.95, (138, 750), size=46)
    C.caption(im, 'Pro: ask in plain words', t, 45.9, 59.1, pos='tr')
    P.end_bar(im, cam, t)
    C.end_card(im, t)
    return np.asarray(im.convert('RGB'))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--fps', type=int, default=E.FPS)
    ap.add_argument('--from', dest='t0', type=float, default=0.0)
    ap.add_argument('--to', dest='t1', type=float, default=E.DUR)
    ap.add_argument('--stills', default='')
    a = ap.parse_args()
    if a.stills:
        os.makedirs(a.out, exist_ok=True)
        for t in [float(x) for x in a.stills.split(',')]:
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
