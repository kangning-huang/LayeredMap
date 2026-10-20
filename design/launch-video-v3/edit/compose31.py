"""NimbleMap launch film v3.1: compositor. Same one-take style and helpers as v3 (edit/compose.py),
new story (edit/edl31.py): a folder of real downloaded files; each file goes into NimbleMap and
becomes its film; the search bar only searches well-known places.

    python edit/compose31.py out/seg.mkv [--from 0 --to 31]
    python edit/compose31.py out/stills --stills 1.0,4.6,...

Drawn, not app UI: the table, window frames, the folder window (a re-layout of the real Finder list:
real names and sizes, generic document icons), the file chips, the typed place names (in the bar's
own font and baseline) with a block cursor, the reveal order of tracks and dots (real pixels over a
real plate), ripples, the drop line, lifted card crops, captions, the end pill and card.
"""
import argparse
import math
import os
import subprocess
import sys

os.environ['LM_CUT'] = '3.1'
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import edl31 as E  # noqa: E402
import frames  # noqa: E402
import compose as C  # noqa: E402

C.edl = E           # the shared helpers read positions from the v3.1 EDL
frames.edl = E

import cv2  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image, ImageDraw, ImageFilter  # noqa: E402

OW, OH = C.OW, C.OH
X1 = E.X1
font, FONT_SEMI, FONT_REG = C.font, C.FONT_SEMI, C.FONT_REG
ease, ease_out, ease_in, smooth, whip, ramp, clamp = C.ease, C.ease_out, C.ease_in, C.smooth, C.whip, C.ramp, C.clamp
get = C.get
INK, ACCENT = C.INK, C.ACCENT


# ----------------------------------------------------------------- the folder window (re-layout)
def folder_image(selected=None, lift=None):
    """The folder as Finder lists it: title bar with the folder's name, Name / Date Modified / Size
    columns, the real files with generic document icons (the extension printed on the page, as
    macOS shows a file no app has claimed). 2x px."""
    key = ('folder', selected, lift)
    if key in C.CACHE:
        return C.CACHE[key]
    w, h = E.FOLDER_W, E.FOLDER_H
    im = Image.new('RGB', (w, h), (255, 255, 255))
    d = ImageDraw.Draw(im)
    # toolbar
    d.rectangle([0, 0, w, 104], fill=(246, 246, 247))
    d.line([0, 104, w, 104], fill=(222, 222, 226), width=2)
    for i, col in enumerate([(255, 95, 87), (254, 188, 46), (40, 200, 64)]):
        cx, cy, r = 44 + i * 40, 52, 12.5
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=col, outline=tuple(int(c * 0.85) for c in col), width=2)
    grey = (150, 150, 156)
    for k, x in enumerate((196, 246)):
        s = -1 if k == 0 else 1
        d.line([x - 8 * s, 40, x + 8 * s, 52, x - 8 * s, 64], fill=grey, width=4, joint='curve')
    d.text((300, 52), E.FOLDER, font=font(FONT_SEMI, 34), fill=(40, 40, 46), anchor='lm')
    # header
    cols = (70, 960, w - 60)        # name x, date x, size right edge
    d.text((cols[0] + 64, 140), 'Name', font=font(FONT_SEMI, 25), fill=(90, 90, 98), anchor='lm')
    d.text((cols[1], 140), 'Date Modified', font=font(FONT_REG, 25), fill=(120, 120, 128), anchor='lm')
    d.text((cols[2], 140), 'Size', font=font(FONT_REG, 25), fill=(120, 120, 128), anchor='rm')
    d.line([30, 172, w - 30, 172], fill=(228, 228, 232), width=2)
    y0, rh = 186, 66
    for i, (name, size) in enumerate(E.FILES):
        y = y0 + i * rh
        if i == selected:
            d.rounded_rectangle([30, y, w - 30, y + rh - 4], 12, fill=(37, 99, 235))
        elif i % 2 == 1:
            d.rounded_rectangle([30, y, w - 30, y + rh - 4], 12, fill=(244, 245, 247))
        if i == lift:
            continue
        ink = (255, 255, 255) if i == selected else (28, 28, 32)
        sub = (230, 236, 255) if i == selected else (120, 120, 128)
        doc_icon(d, cols[0] + 2, y + 4, 42, name.rsplit('.', 1)[1])
        d.text((cols[0] + 64, y + rh / 2 - 2), name, font=font(FONT_REG, 30), fill=ink, anchor='lm')
        d.text((cols[1], y + rh / 2 - 2), E.FILE_DATE, font=font(FONT_REG, 28), fill=sub, anchor='lm')
        d.text((cols[2], y + rh / 2 - 2), size, font=font(FONT_REG, 28), fill=sub, anchor='rm')
    C.CACHE[key] = np.asarray(im)
    return C.CACHE[key]


def doc_icon(d, x, y, s, ext):
    """A generic macOS document: a white page with a folded corner, the extension on it."""
    w, h = s, s * 1.27
    f = s * 0.32
    d.polygon([(x, y), (x + w - f, y), (x + w, y + f), (x + w, y + h), (x, y + h)], fill=(252, 252, 253), outline=(170, 172, 180))
    d.polygon([(x + w - f, y), (x + w - f, y + f), (x + w, y + f)], fill=(222, 224, 230), outline=(170, 172, 180))
    d.text((x + w / 2, y + h * 0.72), ext.upper(), font=font(FONT_SEMI, s * 0.3), fill=(104, 108, 118), anchor='mm')


def row_rect(i):
    """Folder-local rect of row i."""
    y = 186 + i * 66
    return (30, y, E.FOLDER_W - 30, y + 62)


def chip_image(i):
    """One file as a chip: its icon and full name on a white pill (what is 'dropped' into the app)."""
    key = ('chip', i)
    if key in C.CACHE:
        return C.CACHE[key]
    name = E.FILES[i][0]
    tw = font(FONT_REG, 30).getlength(name)
    w, h = int(tw + 64 + 70 + 40), 76
    im = Image.new('RGB', (w, h), (255, 255, 255))
    d = ImageDraw.Draw(im)
    doc_icon(d, 34, 12, 40, name.rsplit('.', 1)[1])
    d.text((98, h / 2 - 1), name, font=font(FONT_REG, 30), fill=(28, 28, 32), anchor='lm')
    C.CACHE[key] = np.asarray(im)
    return C.CACHE[key]


def chip_rect_world(i, centre, scale=E.CHIP_SCALE):
    im = chip_image(i)
    w, h = im.shape[1] * scale, im.shape[0] * scale
    return (centre[0] - w / 2, centre[1] - h / 2, centre[0] + w / 2, centre[1] + h / 2)


def folder_row_centre_world(i):
    x0, y0, x1, y1 = row_rect(i)
    # the chip lifts from the row's name (left part)
    cw = chip_image(i).shape[1]
    return (E.FTL[0] + (x0 + cw / 2) * E.FSCALE, E.FTL[1] + (y0 + y1) / 2 * E.FSCALE)


def draw_chip(canvas, cam, i, centre, alpha=1.0, scale=E.CHIP_SCALE, squash=0.0, lift=1.0):
    im = chip_image(i)
    h, w = im.shape[:2]
    tl = (centre[0] - w * scale / 2, centre[1] - h * scale / 2)
    M = C.local_to_screen(cam, tl, scale)
    ym = h / 2
    rect = (0, ym - (ym - 2) * (1 - squash), w, ym + (ym - 2) * (1 - squash))
    C.draw_window(canvas, im, M, rect=rect, rad=h / 2 * (1 - squash) + 2, alpha=alpha, shadow=0.9 * lift * (1 - squash))


# ----------------------------------------------------------------- camera
TL1, TL2, TL3 = E.TL1, E.TL2, E.TL3
W1C = (TL1[0] + 1411, TL1[1] + 879)
W2C = (TL2[0] + 1409, TL2[1] + 877)
W3C = (TL3[0] + 1411, TL3[1] + 879)
FC = (E.FTL[0] + E.FOLDER_W * E.FSCALE / 2, E.FTL[1] + E.FOLDER_H * E.FSCALE / 2)


def bar_c(tl, src, dx=0.0):
    x0, y0, x1, y1 = E.BAR[src]['rect']
    return (tl[0] + (x0 + x1) / 2 + dx, tl[1] + (y0 + y1) / 2)


B1, B2, B3 = bar_c(TL1, 'g_ty', -150), bar_c(TL2, 'g_eq', -150), bar_c(TL3, 'g_hw', -150)
CARD1 = (TL1[0] + 2450, TL1[1] + 560)
CARD2 = (TL2[0] + 2435, TL2[1] + 560)
CARD3 = (TL3[0] + 2440, TL3[1] + 470)
ZE = 520 / 2822
GAP = 40 / ZE
GX, GY = W3C[0] - (2822 + GAP), W3C[1]
CYE = GY - (282 - 540) / ZE
CE = (GX, CYE)
C.CE, C.ZE = CE, ZE                       # for the shared end pill


def row_tl(i):
    cx = GX + (i - 1) * (2822 + GAP)
    return (cx - 1411, GY - 879)


C3 = E.CHIP3_C
MID23 = (W2C[0], (TL2[1] + C3[1] + 90) / 2)
CAM = [
    (0.00, FC[0], FC[1], 0.86, 0, ease),
    (2.80, FC[0] + 40, FC[1] + 30, 0.95, 0, ease),            # a slow push on the folder
    (3.30, FC[0] + 40, FC[1] + 30, 0.95, 0, ease),
    (4.20, W1C[0], W1C[1], 0.95, 0, whip),                      # follow the file across the table
    (4.85, W1C[0], W1C[1], 0.88, 0, ease),                      # the window opens out of it
    (5.75, W1C[0] + 40, W1C[1] + 30, 0.98, -1.0, ease),         # tracks draw in; push + tilt
    (6.00, B1[0], B1[1], 2.0, 0, ease),                         # onto the bar
    (6.95, B1[0] + 60, B1[1], 2.06, 0, ease),                   # "New York"
    (7.45, W1C[0], W1C[1], 1.0, 0, ease),                       # Enter: the app flies there
    (7.95, W1C[0], W1C[1], 1.02, 0, ease),
    (8.55, W1C[0], W1C[1], 0.92, 1.8, ease),                    # pull + slight dutch: Sandy
    (9.20, CARD1[0], CARD1[1], 1.62, 0, ease),                  # onto the card
    (10.30, CARD1[0] - 80, CARD1[1], 1.5, 0, ease),
    (10.80, CARD1[0] - 80, CARD1[1], 1.5, 0, ease),
    (11.20, W2C[0], W2C[1], 1.35, 0, whip),                     # travel right with the next file
    (12.05, W2C[0], W2C[1], 1.35, 0, ease),
    (12.70, W2C[0], W2C[1], 1.02, 0, ease),                     # it opens: frame fills
    (14.00, W2C[0] - 40, W2C[1] - 30, 1.1, 0, ease),            # the ring (handheld)
    (14.15, B2[0], B2[1], 2.0, 0, ease),                        # onto the bar
    (14.70, B2[0] + 40, B2[1], 2.05, 0, ease),                  # "Tokyo"
    (15.15, W2C[0], W2C[1], 1.0, 0, ease),
    (15.35, W2C[0], W2C[1], 1.0, 0, ease),
    (15.75, W2C[0] + 40, W2C[1], 1.08, 0, ease),                # Japan + the Tohoku card
    (16.00, CARD2[0], CARD2[1], 1.75, 0, ease),
    (16.30, MID23[0], MID23[1], 0.655, 0, ease),                # film 2 + the next file
    (16.90, MID23[0], MID23[1], 0.655, 0, ease),                # dead still
    (17.25, TL3[0] + 1150, TL3[1] + 1150, 1.0, 0, whip),        # whip along the line
    (19.40, TL3[0] + 1600, TL3[1] + 1050, 1.15, 0, ease),       # pan, following the slider
    (19.55, B3[0], B3[1], 2.0, 0, ease),                        # onto the bar
    (20.15, B3[0] + 50, B3[1], 2.06, 0, ease),                  # "Palermo"
    (20.55, W3C[0], W3C[1], 1.0, 0, ease),
    (20.85, W3C[0], W3C[1], 1.0, 0, ease),
    (21.30, W3C[0] + 200, W3C[1] - 60, 1.1, 0, ease),           # Sicily + the card
    (21.75, CARD3[0], CARD3[1], 1.62, 0, ease),
    (22.40, CARD3[0] - 80, CARD3[1], 1.5, 0, ease),
    (23.20, W3C[0], W3C[1], 0.82, 0, ease),                     # pull out to the window
    (23.30, W3C[0], W3C[1], 0.82, 0, ease),
    (25.90, CE[0], CE[1], ZE / X1, 0, ease),                    # pull back over the row
    (27.30, CE[0], CE[1], ZE / X1, 0, ease),
    (28.40, CE[0], CE[1] + 30 / ZE, ZE / X1 * 1.06, 0, ease),
    (31.00, CE[0], CE[1] + 30 / ZE, ZE / X1 * 1.06, 0, ease),
]


def handheld(t):
    a = smooth(ramp(t, 12.6, 12.9)) * (1 - smooth(ramp(t, 13.9, 14.1)))
    if a <= 0:
        return 0.0, 0.0, 0.0
    return (a * (9 * math.sin(t * 1.7) + 5 * math.sin(t * 3.1 + 1)),
            a * (7 * math.sin(t * 1.3 + 2) + 4 * math.sin(t * 2.7)), a * 0.25 * math.sin(t * 0.9))


def camera(t):
    k = CAM
    if t <= k[0][0]:
        a = b = k[0]
        u = 0
    else:
        for a, b in zip(k, k[1:]):
            if t <= b[0]:
                break
        u = b[5]((t - a[0]) / max(1e-9, b[0] - a[0]))
    cx = a[1] + (b[1] - a[1]) * u
    cy = a[2] + (b[2] - a[2]) * u
    z = math.exp(math.log(a[3]) + (math.log(b[3]) - math.log(a[3])) * u) * X1
    rot = a[4] + (b[4] - a[4]) * u
    hx, hy, hr = handheld(t)
    return cx + hx / z, cy + hy / z, z, rot + hr


# ----------------------------------------------------------------- the films
def ty_plate():
    return C.plate_with_keep('ty_plate', 8.0, 'g_ty', 11.0, E.TY_KEEP)


def eq_plate():
    return C.plate_with_keep('eq_plate', 10.0, 'g_eq', 15.0, E.EQ_KEEP)


def eq_thr(h, w):
    k = ('thr_eq31', h, w)
    if k not in C.CACHE:
        cx, cy = E.EQ_RING_CENTRE[0], E.EQ_RING_CENTRE[1] - E.TITLE
        jx, jy = E.EQ_RING_START[0], E.EQ_RING_START[1] - E.TITLE
        yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
        a0 = math.atan2(jy - cy, jx - cx)
        ang = (np.arctan2(yy - cy, xx - cx) - a0) % (2 * math.pi) / (2 * math.pi)
        C.CACHE[k] = ang * 0.78 + C.noise_field(h, w, 8, 7) * 0.22
    return C.CACHE[k]


def film1(t):
    A = get('g_ty', 11.0)
    if t < E.T_REVEAL1[0]:
        return ty_plate()
    if t < E.T_REVEAL1[1]:
        return C.reveal(ty_plate(), A, C.ty_threshold(*A.shape[:2]), ease_in(ramp(t, *E.T_REVEAL1)) ** 0.6, 0.05)
    if t < E.T_ENTER1:
        return A
    if t < E.T_ENTER1 + 0.55:      # the app's flight to New York City (the place search's landing)
        return get('g_ty', 12.72 + (t - E.T_ENTER1) * (0.66 / 0.55))
    if t < E.T_ENTER1 + 0.8:
        return get('g_ty', 13.40 + (t - E.T_ENTER1 - 0.55) * (2.2 / 0.25))
    if t < E.T_SANDY:
        return get('g_ty', 17.0)
    if t < E.T_SANDY + 0.6:
        return C.xfade(get('g_ty', 17.0), get('g_ty', 40.0), ease(ramp(t, E.T_SANDY, E.T_SANDY + 0.6)))
    return get('g_ty', 40.0)


def film1_loop(p):
    A = get('g_ty', 11.0)
    if p < 0.35:
        return C.reveal(ty_plate(), A, C.ty_threshold(*A.shape[:2]), p / 0.35, 0.05)
    if p < 0.45:
        return get('g_ty', 12.72 + (p - 0.35) / 0.1 * 0.66)
    if p < 0.6:
        return get('g_ty', 17.0)
    if p < 0.7:
        return C.xfade(get('g_ty', 17.0), get('g_ty', 40.0), (p - 0.6) / 0.1)
    if p > 0.95:
        return C.xfade(get('g_ty', 40.0), ty_plate(), (p - 0.95) / 0.05)
    return get('g_ty', 40.0)


def film2(t):
    Ef = get('g_eq', 15.0)
    if t < E.T_RING[0]:
        return eq_plate()
    if t < E.T_RING[1]:
        return C.reveal(eq_plate(), Ef, eq_thr(*Ef.shape[:2]), ramp(t, E.T_RING[0], E.T_RING[1] - 0.05), 0.015)
    if t < E.T_ENTER2:
        return Ef
    if t < E.T_ENTER2 + 0.3:       # the flight to Tokyo
        return get('g_eq', 17.95 + (t - E.T_ENTER2) * (0.65 / 0.3))
    if t < E.T_ENTER2 + 0.45:
        return get('g_eq', 18.6 + (t - E.T_ENTER2 - 0.3) * (2.0 / 0.15))
    if t < E.T_JAPAN:
        return get('g_eq', 21.0)
    return C.xfade(get('g_eq', 21.0), get('g_eq', 40.0), ease(ramp(t, E.T_JAPAN, E.T_JAPAN + 0.4)))


def film2_loop(p):
    Ef = get('g_eq', 15.0)
    if p < 0.5:
        return C.reveal(eq_plate(), Ef, eq_thr(*Ef.shape[:2]), p / 0.5, 0.015)
    if p < 0.6:
        return get('g_eq', 21.0) if p > 0.55 else get('g_eq', 17.95 + (p - 0.5) / 0.05 * 0.65)
    if p < 0.7:
        return C.xfade(get('g_eq', 21.0), get('g_eq', 40.0), (p - 0.6) / 0.1)
    if p > 0.95:
        return C.xfade(get('g_eq', 40.0), eq_plate(), (p - 0.95) / 0.05)
    return get('g_eq', 40.0)


def hw_index(t):
    if t < 17.3:
        return 0
    return min(E.HW_LAST, int(E.HW_LAST * ease(ramp(t, 17.35, 19.35)) + 1e-6))


def film3(t):
    if t < E.T_ENTER3:
        return get('g_hw', E.hw_step(hw_index(t)))
    if t < E.T_ENTER3 + 0.4:       # the flight to Palermo
        return get('g_hw', 150.15 + (t - E.T_ENTER3) * (2.5 / 0.4))
    if t < E.T_PALERMO - 0.2:
        return get('g_hw', 156.0)
    if t < E.T_PALERMO:            # the cell is clicked: its card
        return get('g_hw', 157.95 + (t - (E.T_PALERMO - 0.2)) * (0.7 / 0.2))
    return C.xfade(get('g_hw', 158.65), get('g_hw', 168.0), ease(ramp(t, E.T_PALERMO, E.T_PALERMO + 0.4)))


def film3_loop(p):
    if p < 0.6:
        return get('g_hw', E.hw_step(int(E.HW_LAST * p / 0.6 + 1e-6)))
    if p < 0.7:
        return C.xfade(get('g_hw', E.hw_step(E.HW_LAST)), get('g_hw', 168.0), (p - 0.6) / 0.1)
    return get('g_hw', 168.0)


# ----------------------------------------------------------------- bars
def bar_state(t, spec, label, t_enter, t_ready):
    """(text, cursor, placeholder, hint_fade) for a bar that is typed into, then shows the place."""
    if t >= t_enter:
        return label, 0.0, False, 0.0
    q = C.typed(spec, t)
    if t < t_ready:
        return '', 0.0, True, 0.0
    hf = smooth(ramp(t, t_ready, t_ready + 0.15))
    last = max([t_ready] + [k for k in spec[1] if k <= t])
    cur = C.blink(t, last)
    if t >= t_enter - 0.06:
        cur = 1.0
    return q, cur, q == '', hf


def window_state(t):
    g = ease(ramp(t, E.T_GATHER[0], E.T_GATHER[1] - 0.3))
    out = {}
    for i, tl in enumerate([TL1, TL2, TL3]):
        rt = row_tl(i)
        out[i] = (tl[0] + (rt[0] - tl[0]) * g, tl[1] + (rt[1] - tl[1]) * g)
    return out


def loop_phase(t, i):
    tt = min(t, E.T_STOP)
    period = [3.2, 3.0, 2.8][i]
    return ((tt - 23.2) / period + [0.72, 0.75, 0.75][i]) % 1.0


def blur_wash(canvas, a, color, sigma=16):
    """Behind the big words: the frame is blurred and washed, so no card text reads through."""
    if a <= 0:
        return canvas
    small = cv2.resize(canvas, (OW // 4, OH // 4), interpolation=cv2.INTER_AREA)
    small = cv2.GaussianBlur(small, (0, 0), sigma / 4)
    bl = cv2.resize(small, (OW, OH), interpolation=cv2.INTER_LINEAR)
    canvas = canvas * (1 - a) + bl * a
    w = 0.80 * a
    return canvas * (1 - w) + np.array(color, np.float32) * w


def render(t):
    cam = camera(t)
    canvas = C.table(cam)
    pos = window_state(t)

    # ---- the folder (left of film 1 on the table)
    sel = E.ROW_SHP if 2.5 <= t < 4.3 else None
    lift = E.ROW_SHP if t >= E.T_LIFT1 else None
    fimg = folder_image(sel, lift)
    Mf = C.local_to_screen(cam, E.FTL, E.FSCALE)
    C.draw_window(canvas, fimg, Mf, rad=30, shadow=1.0)

    # ---- window 1: storms (shapefile)
    M1 = C.local_to_screen(cam, pos[0])
    full1 = (0, 0, 2822, 1694 + E.TITLE)
    if t >= E.T_OPEN1 - 0.02:
        c1 = film1(t) if t < 23.25 else film1_loop(loop_phase(t, 0))
        img1 = C.window_image(c1, False)
        q, cur, ph, hf = bar_state(t, E.TYPE1, E.LABEL1, E.T_ENTER1, 5.85)
        img1 = C.draw_query(img1, 'g_ty', q, cur, placeholder=ph, hint_fade=hf)
        u = ease_out(ramp(t, E.T_OPEN1, E.T_OPEN1 + 0.6))
        cr = chip_rect_world(E.ROW_SHP, W1C)
        start = (cr[0] - pos[0][0], cr[1] - pos[0][1], cr[2] - pos[0][0], cr[3] - pos[0][1])
        rect = C.lerp_rect(start, full1, u)
        C.draw_window(canvas, img1, M1, rect=rect, rad=60 * (1 - u) + 36 * u, shadow=0.6 + 0.4 * u)
    # the .shp file flies out of the folder into the app
    if E.T_LIFT1 <= t < E.T_OPEN1 + 0.35:
        p0 = folder_row_centre_world(E.ROW_SHP)
        u = whip(ramp(t, E.T_LIFT1 + 0.3, E.T_OPEN1))
        c = (p0[0] + (W1C[0] - p0[0]) * u, p0[1] + (W1C[1] - p0[1]) * u - math.sin(math.pi * u) * 260)
        s0 = E.FSCALE
        sc = s0 + (E.CHIP_SCALE - s0) * u
        rise = ease_out(ramp(t, E.T_LIFT1, E.T_LIFT1 + 0.3))
        a = 1 - smooth(ramp(t, E.T_OPEN1, E.T_OPEN1 + 0.3))
        draw_chip(canvas, cam, E.ROW_SHP, c, alpha=a, scale=sc * (1 + 0.06 * rise), lift=rise)

    # ---- window 2: earthquakes (GeoPackage, dark)
    M2 = C.local_to_screen(cam, pos[1])
    full2 = (0, 0, 2818, 1689 + E.TITLE)
    if 10.7 < t < E.T_OPEN2[0] + 0.35:
        # the .gpkg file comes in from the folder's side and lands where its window will be
        u = whip(ramp(t, 10.75, 11.2))
        p0 = (W2C[0] - 3400, W2C[1] + 300)
        c = (p0[0] + (W2C[0] - p0[0]) * u, p0[1] + (W2C[1] - p0[1]) * u - math.sin(math.pi * u) * 200)
        a = smooth(ramp(t, 10.75, 10.9)) * (1 - smooth(ramp(t, E.T_OPEN2[0], E.T_OPEN2[0] + 0.3)))
        draw_chip(canvas, cam, E.ROW_GPKG, c, alpha=a)
    if t >= E.T_OPEN2[0]:
        c2 = film2(t) if t < 23.25 else film2_loop(loop_phase(t, 1))
        img2 = C.window_image(c2, True)
        q, cur, ph, hf = bar_state(t, E.TYPE2, E.LABEL2, E.T_ENTER2, 14.02)
        img2 = C.draw_query(img2, 'g_eq', q, cur, placeholder=ph, hint_fade=hf)
        u = ease_out(ramp(t, E.T_OPEN2[0], E.T_OPEN2[1] - 0.05))
        cr = chip_rect_world(E.ROW_GPKG, W2C)
        start = (cr[0] - pos[1][0], cr[1] - pos[1][1], cr[2] - pos[1][0], cr[3] - pos[1][1])
        C.draw_window(canvas, img2, M2, rect=C.lerp_rect(start, full2, u), rad=60 * (1 - u) + 36 * u, shadow=0.6 + 0.4 * u)

    if E.T_DROP + 0.06 < t < 17.7:
        M = C.cam_matrix(cam)
        cw = chip_image(E.ROW_NC).shape[1] * E.CHIP_SCALE * 1.5
        lx0, lx1, ly = C3[0] - cw / 2, C3[0] + cw / 2, C3[1]
        tx0, tx1 = TL3[0] + E.HW_TRACK[0], TL3[0] + E.HW_TRACK[2]
        shoot = whip(ramp(t, E.T_DROP + 0.06, 17.2))
        tail = ease(ramp(t, 17.0, 17.35))
        xa = lx0 + (tx0 - lx0) * tail
        xb = lx1 + (tx1 - lx1) * shoot
        fade = 1 - smooth(ramp(t, 17.35, 17.65))
        p0, p1 = C.apply(M, xa, ly), C.apply(M, xb, ly)
        th = max(2, int(round(9 * cam[2])))
        lay = np.zeros((OH, OW), np.uint8)
        cv2.line(lay, (int(p0[0] * 16), int(p0[1] * 16)), (int(p1[0] * 16), int(p1[1] * 16)), 255, th, cv2.LINE_AA, 4)
        al = (lay.astype(np.float32) / 255 * fade)[..., None]
        canvas *= (1 - al)
        canvas += np.array(ACCENT, np.float32) * al

    # ---- window 3: heatwaves (NetCDF): it opens out of its own slider track
    M3 = C.local_to_screen(cam, pos[2])
    if t > 17.0:
        c3 = film3(t) if t < 23.25 else film3_loop(loop_phase(t, 2))
        img3 = C.window_image(c3, False)
        q, cur, ph, hf = bar_state(t, E.TYPE3, E.LABEL3, E.T_ENTER3, 19.42)
        img3 = C.draw_query(img3, 'g_hw', q, cur, placeholder=ph, hint_fade=hf)
        tx0, ty_, tx1 = E.HW_TRACK
        u1 = ease_out(ramp(t, 17.06, 17.2))
        u2 = ease_out(ramp(t, 17.16, 17.5))
        rect = C.lerp_rect(C.lerp_rect((tx0, ty_ - 3, tx1, ty_ + 3), E.HW_SLIDER_PANEL, u1), (0, 0, 2822, 1694 + E.TITLE), u2)
        C.draw_window(canvas, img3, M3, rect=rect, rad=3 + 33 * u2 + 20 * u1 * (1 - u2), shadow=u1)

    # ---- the big-number beats: blur and wash what is behind
    a1 = smooth(ramp(t, 9.35, 9.6)) * (1 - smooth(ramp(t, 10.75, 11.0)))
    a2 = smooth(ramp(t, 15.95, 16.15)) * (1 - smooth(ramp(t, 16.8, 16.95)))
    a3 = smooth(ramp(t, 21.95, 22.15)) * (1 - smooth(ramp(t, 22.9, 23.1)))
    canvas = blur_wash(canvas, a1, (250, 249, 246))
    canvas = blur_wash(canvas, a2, (14, 16, 24))
    canvas = blur_wash(canvas, a3, (250, 249, 246))

    # ---- the .nc file (drawn over the wash, so it reads under "M 9.1."): it arrives under film 2,
    # then at the DROP collapses into the line
    if 15.95 < t < E.T_DROP + 0.16:
        u = whip(ramp(t, 15.98, 16.35))
        p0 = (C3[0] - 3600, C3[1] + 200)
        c = (p0[0] + (C3[0] - p0[0]) * u, p0[1] + (C3[1] - p0[1]) * u - math.sin(math.pi * u) * 220)
        sq = ease_in(ramp(t, E.T_DROP, E.T_DROP + 0.12))
        draw_chip(canvas, cam, E.ROW_NC, c, alpha=smooth(ramp(t, 15.98, 16.1)), scale=E.CHIP_SCALE * 1.5, squash=sq)
    # ---- motion blur on the fast travels
    c0, c1_ = camera(t - 1 / 60), camera(t + 1 / 60)
    vx, vy = (c1_[0] - c0[0]) * cam[2] / 2, (c1_[1] - c0[1]) * cam[2] / 2
    ln = math.hypot(vx, vy)
    travel = (3.4 <= t <= 4.2) or (10.8 <= t <= 11.2) or (E.T_DROP <= t <= E.T_FILM3)
    if travel and ln > 6:
        L = int(min(120, ln * 0.9))
        ker = np.zeros((L * 2 + 1, L * 2 + 1), np.float32)
        cv2.line(ker, (L - int(vx / ln * L), L - int(vy / ln * L)), (L + int(vx / ln * L), L + int(vy / ln * L)), 1.0, 1)
        canvas = cv2.filter2D(canvas, -1, ker / ker.sum())

    im = Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8)).convert('RGBA')

    # ---- overlays
    C.caption(im, E.FORMATS, t, 0.45, 3.35)
    C.caption(im, '4,999 storm tracks', t, 4.95, 5.85)
    if E.T_SANDY + 0.3 <= t <= E.T_SANDY + 1.2:
        x, y = C.apply(M1, *E.SANDY_PX)
        C.ripple(im, x, y, (t - E.T_SANDY - 0.35) / 0.75)
    ty40 = C.window_image(get('g_ty', 40.0), False)
    C.lift_chip(im, (*E.SANDY_RANK, ty40), M1, t, 8.8, 9.45, scale_to=1.3)
    C.lift_chip(im, (*E.SANDY_WIND, ty40), M1, t, 9.40, 10.85, scale_to=1.2, target=(330, 330, 9.45, 0.55, 2.6))
    C.big_words(im, '100 knots.', t, 9.62, 10.85, (130, 650), size=230)
    C.big_words(im, 'Sandy, 2012 · Category 3', t, 9.8, 10.85, (138, 800), size=52)
    C.caption(im, '14,537 earthquakes since 1900', t, 12.8, 13.95)
    if E.T_JAPAN + 0.15 <= t <= 16.0:
        x, y = C.apply(M2, *E.TOHOKU_PX)
        C.ripple(im, x, y, (t - E.T_JAPAN - 0.2) / 0.7, color=(255, 255, 255), r1=110)
    eq40 = C.window_image(get('g_eq', 40.0), True)
    C.lift_chip(im, (*E.EQ_MAG, eq40), M2, t, 15.6, 15.98, scale_to=1.25, bg=(31, 31, 35))
    C.big_words(im, 'M 9.1.', t, 16.02, 16.92, (120, 300), size=230, light=True)
    C.big_words(im, 'Tōhoku, 2011', t, 16.15, 16.92, (128, 450), size=52, light=True)
    C.caption(im, 'Heatwaves, summer 2023 · 92 daily steps', t, 17.45, 19.35, pos='tr')
    if E.T_PALERMO + 0.1 <= t <= 21.6:
        x, y = C.apply(M3, *E.PALERMO_PX)
        C.ripple(im, x, y, (t - E.T_PALERMO - 0.15) / 0.7)
    hw168 = C.window_image(get('g_hw', 168.0), False)
    C.lift_chip(im, (*E.HW_VALUE, hw168), M3, t, 21.45, 22.0, scale_to=1.2)
    C.lift_chip(im, (*E.HW_TEMP, hw168), M3, t, 21.95, 23.05, scale_to=1.1, target=(420, 330, 22.0, 0.5, 1.9))
    C.big_words(im, '36.50 °C', t, 22.15, 23.05, (130, 650), size=230)
    C.big_words(im, 'heat-wave intensity, Palermo, 25 Jul 2023', t, 22.3, 23.05, (138, 800), size=48)
    # the end (shared with v3)
    C.end_bar(im, cam, t)
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
