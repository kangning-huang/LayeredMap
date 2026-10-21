"""NimbleMap launch film v3.2: compositor. v3.1's story and helpers (edit/compose31.py), retimed for
narration (edit/edl32.py), polished, with a Pro beat (the Ask box on the opened storm shapefile).

    python edit/compose32.py out/seg.mkv [--from 0 --to 45]
    python edit/compose32.py out/stills --stills 1.0,4.6,...

Polish over v3.1: the file row lifts out whole (selection highlight, date and size intact) and
flies as that row; the big-number beats show the card's real row lifting, then the big words, with
no lone value chip; easing and holds retimed around the voice.
"""
import argparse
import math
import os
import subprocess
import sys

os.environ['LM_CUT'] = '3.2'
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import edl32 as E  # noqa: E402
import frames  # noqa: E402
import compose31 as B  # noqa: E402
import compose as C  # noqa: E402

B.E = E
C.edl = E
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


# ----------------------------------------------------------------- the folder and the file rows
def row_draw(d, i, x0, y, w, selected, rh=66):
    name, size = E.FILES[i]
    cols = (x0 + 40, x0 + 930, x0 + w - 30)
    ink = (255, 255, 255) if selected else (28, 28, 32)
    sub = (230, 236, 255) if selected else (120, 120, 128)
    B.doc_icon(d, cols[0] + 2, y + 4, 42, name.rsplit('.', 1)[1])
    d.text((cols[0] + 64, y + rh / 2 - 2), name, font=font(FONT_REG, 30), fill=ink, anchor='lm')
    d.text((cols[1], y + rh / 2 - 2), E.FILE_DATE, font=font(FONT_REG, 28), fill=sub, anchor='lm')
    d.text((cols[2], y + rh / 2 - 2), size, font=font(FONT_REG, 28), fill=sub, anchor='rm')


def folder_image(selected=None, lifted=None):
    key = ('folder32', selected, lifted)
    if key in C.CACHE:
        return C.CACHE[key]
    base = Image.fromarray(B.folder_image(None, None).copy())
    d = ImageDraw.Draw(base)
    for i in (selected, lifted):
        if i is None:
            continue
        x0, y0, x1, y1 = B.row_rect(i)
        d.rounded_rectangle([x0, y0, x1, y1 + 2], 12, fill=(244, 245, 247) if i % 2 == 1 else (255, 255, 255))
        if i == selected and i != lifted:
            d.rounded_rectangle([x0, y0, x1, y1 + 2], 12, fill=(37, 99, 235))
            row_draw(d, i, x0, y0, x1 - x0, True)
        elif i == lifted:
            # the slot it left: a faint outline
            d.rounded_rectangle([x0 + 4, y0 + 2, x1 - 4, y1], 12, outline=(214, 220, 236), width=2)
    C.CACHE[key] = np.asarray(base)
    return C.CACHE[key]


def chip_image(i):
    """The whole selected row, as it lifts out of the folder: blue highlight, icon, name, date, size."""
    key = ('chip32', i)
    if key in C.CACHE:
        return C.CACHE[key]
    x0, y0, x1, y1 = B.row_rect(i)
    w, h = x1 - x0, y1 - y0 + 2
    im = Image.new('RGB', (w, h), (37, 99, 235))
    row_draw(ImageDraw.Draw(im), i, 0, 0, w, True)
    C.CACHE[key] = np.asarray(im)
    return C.CACHE[key]


B.chip_image = chip_image


def folder_row_centre_world(i):
    x0, y0, x1, y1 = B.row_rect(i)
    return (E.FTL[0] + (x0 + x1) / 2 * E.FSCALE, E.FTL[1] + (y0 + y1 + 2) / 2 * E.FSCALE)


def draw_chip(canvas, cam, i, centre, alpha=1.0, scale=E.FSCALE, squash=0.0, lift=1.0):
    im = chip_image(i)
    h, w = im.shape[:2]
    tl = (centre[0] - w * scale / 2, centre[1] - h * scale / 2)
    M = C.local_to_screen(cam, tl, scale)
    ym = h / 2
    rect = (0, ym - (ym - 2) * (1 - squash), w, ym + (ym - 2) * (1 - squash))
    C.draw_window(canvas, im, M, rect=rect, rad=12 * (1 - squash) + 2, alpha=alpha, shadow=0.9 * lift * (1 - squash))


def chip_rect_world(i, centre, scale=E.FSCALE):
    im = chip_image(i)
    w, h = im.shape[1] * scale, im.shape[0] * scale
    return (centre[0] - w / 2, centre[1] - h / 2, centre[0] + w / 2, centre[1] + h / 2)


CHIP_S = 1.0          # the rows that come in without the folder in view

# ----------------------------------------------------------------- layout
TL1, TL2, TL3 = E.TL1, E.TL2, E.TL3
TL4 = (TL3[0] + E.W + 900.0, TL3[1] - 200.0)
W1C, W2C, W3C = B.W1C, B.W2C, B.W3C
W4C = (TL4[0] + E.PW / 2, TL4[1] + E.PH / 2)
FC = B.FC
B1, B2, B3 = B.B1, B.B2, B.B3
CARD1, CARD2, CARD3 = B.CARD1, B.CARD2, B.CARD3
CARD4 = (TL4[0] + (E.PRO_CARD[0] + E.PRO_CARD[2]) / 2, TL4[1] + (E.PRO_CARD[1] + E.PRO_CARD[3]) / 2)
ASKC = (TL4[0] + 1300, TL4[1] + 1550)
C3 = E.CHIP3_C
MID23 = (W2C[0], (TL2[1] + C3[1] + 90) / 2)
# End: four films in a row, all at the films' height (the Pro window scaled up to match).
S4 = E.H / E.PH
WIDTHS = [E.W, E.W, E.W, E.PW * S4]
ZE = 0.145
GAP = 40 / ZE
ROW_W = sum(WIDTHS) + 3 * GAP
GX, GY = (W3C[0] + W4C[0]) / 2, W3C[1]
CYE = GY - (300 - 540) / ZE
CE = (GX, CYE)
C.CE, C.ZE = CE, ZE


def row_tl(i):
    x = GX - ROW_W / 2 + sum(WIDTHS[:i]) + i * GAP
    return (x, GY - E.H / 2)


T = E
CAM = [
    (0.00, FC[0], FC[1], 0.86, 0, ease),
    (4.10, FC[0] + 40, FC[1] + 30, 0.96, 0, ease),                # a slow push on the folder
    (4.55, FC[0] + 40, FC[1] + 30, 0.96, 0, ease),
    (5.40, W1C[0], W1C[1], 0.95, 0, whip),                        # follow the row across the table
    (6.05, W1C[0], W1C[1], 0.88, 0, ease),                        # the window opens out of it
    (9.50, W1C[0] + 50, W1C[1] + 30, 1.0, -0.8, ease),            # tracks; a slow push + tilt
    (9.75, B1[0], B1[1], 2.0, 0, ease),                           # onto the bar
    (10.75, B1[0] + 60, B1[1], 2.06, 0, ease),                    # "New York"
    (11.30, W1C[0], W1C[1], 1.0, 0, ease),                        # Enter: the app flies there
    (12.05, W1C[0], W1C[1], 1.02, 0, ease),
    (12.70, W1C[0], W1C[1], 0.92, 1.6, ease),                     # pull + slight dutch: Sandy
    (13.40, CARD1[0], CARD1[1], 1.62, 0, ease),                   # onto the card
    (14.50, CARD1[0] - 60, CARD1[1], 1.55, 0, ease),
    (14.90, CARD1[0] - 60, CARD1[1], 1.55, 0, ease),
    (15.30, W2C[0], W2C[1], 1.35, 0, whip),                       # travel right with the next file
    (16.15, W2C[0], W2C[1], 1.35, 0, ease),
    (16.80, W2C[0], W2C[1], 1.02, 0, ease),                       # it opens: frame fills
    (19.00, W2C[0] - 40, W2C[1] - 30, 1.1, 0, ease),              # the ring (handheld)
    (19.15, B2[0], B2[1], 2.0, 0, ease),
    (19.75, B2[0] + 40, B2[1], 2.05, 0, ease),                    # "Tokyo"
    (20.25, W2C[0], W2C[1], 1.0, 0, ease),
    (20.40, W2C[0], W2C[1], 1.0, 0, ease),
    (20.85, W2C[0] + 40, W2C[1], 1.08, 0, ease),                  # Japan + the Tohoku card
    (21.30, CARD2[0], CARD2[1], 1.7, 0, ease),
    (21.70, MID23[0], MID23[1], 0.655, 0, ease),                  # film 2 + the next file
    (22.75, MID23[0], MID23[1], 0.655, 0, ease),                  # held
    (23.10, TL3[0] + 1150, TL3[1] + 1150, 1.0, 0, whip),          # whip along the line
    (25.65, TL3[0] + 1600, TL3[1] + 1050, 1.15, 0, ease),         # pan, following the slider
    (25.80, B3[0], B3[1], 2.0, 0, ease),
    (26.40, B3[0] + 50, B3[1], 2.06, 0, ease),                    # "Palermo"
    (26.80, W3C[0], W3C[1], 1.0, 0, ease),
    (27.10, W3C[0], W3C[1], 1.0, 0, ease),
    (27.55, W3C[0] + 200, W3C[1] - 60, 1.1, 0, ease),             # Sicily + the card
    (28.05, CARD3[0], CARD3[1], 1.62, 0, ease),
    (29.30, CARD3[0] - 60, CARD3[1], 1.55, 0, ease),
    (29.70, CARD3[0] - 60, CARD3[1], 1.55, 0, ease),
    (30.10, W4C[0], W4C[1], 1.25, 0, whip),                       # travel to the Pro window
    (30.70, W4C[0], W4C[1], 1.0, 0, ease),
    (30.95, ASKC[0], ASKC[1] - 120, 1.9, 0, ease),                # onto the Ask box
    (32.10, ASKC[0] + 60, ASKC[1] - 120, 1.95, 0, ease),
    (32.75, W4C[0], W4C[1], 1.02, 0, ease),                       # the answer on the map
    (33.60, CARD4[0], CARD4[1] - 40, 1.45, 0, ease),              # and its plan card
    (34.80, CARD4[0] - 40, CARD4[1] - 60, 1.38, 0, ease),
    (35.30, W4C[0], W4C[1], 0.85, 0, ease),
    (37.90, CE[0], CE[1], ZE / X1, 0, ease),                      # pull back over the row
    (T.T_PEN[0], CE[0], CE[1], ZE / X1, 0, ease),
    (T.T_PEN[1], CE[0], CE[1] + 30 / ZE, ZE / X1 * 1.06, 0, ease),
    (E.DUR, CE[0], CE[1] + 30 / ZE, ZE / X1 * 1.06, 0, ease),
]


def handheld(t):
    a = smooth(ramp(t, 16.7, 17.0)) * (1 - smooth(ramp(t, 18.9, 19.1)))
    if a <= 0:
        return 0.0, 0.0, 0.0
    return (a * (9 * math.sin(t * 1.7) + 5 * math.sin(t * 3.1 + 1)),
            a * (7 * math.sin(t * 1.3 + 2) + 4 * math.sin(t * 2.7)), a * 0.25 * math.sin(t * 0.9))


def camera(t):
    B.CAM = CAM
    B.handheld = handheld
    return B.camera(t)


def hw_index(t):
    if t < T.T_FILM3 + 0.05:
        return 0
    return min(E.HW_LAST, int(E.HW_LAST * ease(ramp(t, T.T_FILM3 + 0.1, 25.55)) + 1e-6))


B.hw_index = hw_index


# ----------------------------------------------------------------- the earthquake film (light retake)
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
    if t < T.T_ENTER2 + 0.3:       # the flight to Tokyo
        return get('g_eqL', 15.25 + (t - T.T_ENTER2) * (0.65 / 0.3))
    if t < T.T_ENTER2 + 0.45:
        return get('g_eqL', 15.9 + (t - T.T_ENTER2 - 0.3) * (2.0 / 0.15))
    if t < T.T_JAPAN:
        return get('g_eqL', 18.5)
    return C.xfade(get('g_eqL', 18.5), get('g_eqL', 40.0), ease(ramp(t, T.T_JAPAN, T.T_JAPAN + 0.4)))


def film2_loop(p):
    Ef = get('g_eqL', 12.5)
    if p < 0.5:
        return C.reveal(eq_plate(), Ef, B.eq_thr(*Ef.shape[:2]), p / 0.5, 0.015)
    if p < 0.6:
        return get('g_eqL', 18.5) if p > 0.55 else get('g_eqL', 15.25 + (p - 0.5) / 0.05 * 0.65)
    if p < 0.7:
        return C.xfade(get('g_eqL', 18.5), get('g_eqL', 40.0), (p - 0.6) / 0.1)
    if p > 0.95:
        return C.xfade(get('g_eqL', 40.0), eq_plate(), (p - 0.95) / 0.05)
    return get('g_eqL', 40.0)


# ----------------------------------------------------------------- the Pro window
def film4(t):
    if t < T.T_SEND:
        return get('p_ty', 12.0)
    if t < T.T_SEND + 0.6:     # "Planning ..." (the real request, sped up from 3 s)
        return get('p_ty', 12.95 + (t - T.T_SEND) * (3.1 / 0.6))
    if t < T.T_SEND + 0.75:
        return get('p_ty', 16.05 + (t - T.T_SEND - 0.6) * (0.2 / 0.15))
    return get('p_ty', 21.0)


def film4_loop(p):
    if p < 0.4:
        return get('p_ty', 12.0)
    if p < 0.5:
        return get('p_ty', 12.95 + (p - 0.4) / 0.1 * 3.1)
    return get('p_ty', 21.0)


def draw_ask(img, text, cursor):
    """The question typed into the real Ask box (its font, size, colour, start and baseline)."""
    a = E.ASK
    im = Image.fromarray(img)
    d = ImageDraw.Draw(im)
    x0, y0, x1, y1 = a['clear']
    d.rectangle([x0, y0, x1, y1], fill=a['bg'])
    f = font(FONT_REG, a['size'])
    if text:
        d.text((a['x'], a['base']), text, font=f, fill=a['ink'], anchor='ls')
    if cursor > 0:
        cx = a['x'] + (d.textlength(text, font=f) + 3 if text else 0)
        d.rectangle([cx, a['base'] - 22, cx + 14, a['base'] + 6], fill=a['ink'])
    return np.asarray(im)


# ----------------------------------------------------------------- end pill (v3's, retimed)
def end_bar(im, cam, t):
    g0, g1 = T.T_GATHER
    a = smooth(ramp(t, g1 - 0.3, g1 + 0.6))
    if a <= 0:
        return
    shift = (g1 + 0.6) - 25.3
    # reuse v3's pill: it reads its own clock, so hand it a shifted time with this cut's stop/pen
    C.end_bar(im, cam, t - shift if t < T.T_STOP else t)


def window_state(t):
    g = ease(ramp(t, T.T_GATHER[0], T.T_GATHER[1] - 0.3))
    out = {}
    for i, tl in enumerate([TL1, TL2, TL3, TL4]):
        rt = row_tl(i)
        s = 1.0 + ((S4 if i == 3 else 1.0) - 1.0) * g
        out[i] = ((tl[0] + (rt[0] - tl[0]) * g, tl[1] + (rt[1] - tl[1]) * g), s)
    return out


def loop_phase(t, i):
    tt = min(t, T.T_STOP)
    period = [3.2, 3.0, 2.8, 3.4][i]
    return ((tt - T.T_GATHER[0]) / period + [0.72, 0.75, 0.75, 0.6][i]) % 1.0


def render(t):
    cam = camera(t)
    canvas = C.table(cam)
    pos = window_state(t)
    LOOP = T.T_GATHER[0] - 0.05

    # ---- the folder
    sel = E.ROW_SHP if T.T_SELECT <= t else None
    lifted = E.ROW_SHP if t >= T.T_LIFT1 else None
    C.draw_window(canvas, folder_image(sel, lifted), C.local_to_screen(cam, E.FTL, E.FSCALE), rad=30, shadow=1.0)

    # ---- window 1: storms (shapefile)
    (tl1, s1) = pos[0]
    M1 = C.local_to_screen(cam, tl1, s1)
    full1 = (0, 0, 2822, 1694 + E.TITLE)
    if t >= T.T_OPEN1 - 0.02:
        c1 = B.film1(t) if t < LOOP else B.film1_loop(loop_phase(t, 0))
        img1 = C.window_image(c1, False)
        q, cur, ph, hf = B.bar_state(t, E.TYPE1, E.LABEL1, T.T_ENTER1, 9.62)
        img1 = C.draw_query(img1, 'g_ty', q, cur, placeholder=ph, hint_fade=hf)
        u = ease_out(ramp(t, T.T_OPEN1, T.T_OPEN1 + 0.65))
        cr = chip_rect_world(E.ROW_SHP, W1C)
        start = (cr[0] - tl1[0], cr[1] - tl1[1], cr[2] - tl1[0], cr[3] - tl1[1])
        C.draw_window(canvas, img1, M1, rect=C.lerp_rect(start, full1, u), rad=20 * (1 - u) + 36 * u, shadow=0.6 + 0.4 * u)
    if T.T_LIFT1 <= t < T.T_OPEN1 + 0.35:
        p0 = folder_row_centre_world(E.ROW_SHP)
        u = whip(ramp(t, T.T_LIFT1 + 0.3, T.T_OPEN1))
        c = (p0[0] + (W1C[0] - p0[0]) * u, p0[1] + (W1C[1] - p0[1]) * u - math.sin(math.pi * u) * 260)
        rise = ease_out(ramp(t, T.T_LIFT1, T.T_LIFT1 + 0.3))
        a = 1 - smooth(ramp(t, T.T_OPEN1, T.T_OPEN1 + 0.3))
        draw_chip(canvas, cam, E.ROW_SHP, c, alpha=a, scale=E.FSCALE * (1 + 0.05 * rise), lift=rise)

    # ---- window 2: earthquakes (GeoPackage, dark)
    (tl2, s2) = pos[1]
    M2 = C.local_to_screen(cam, tl2, s2)
    if T.T_TRAVEL[0] - 0.1 < t < T.T_OPEN2[0] + 0.35:
        u = whip(ramp(t, T.T_TRAVEL[0] - 0.05, T.T_TRAVEL[1]))
        p0 = (W2C[0] - 3400, W2C[1] + 300)
        c = (p0[0] + (W2C[0] - p0[0]) * u, p0[1] + (W2C[1] - p0[1]) * u - math.sin(math.pi * u) * 200)
        a = smooth(ramp(t, T.T_TRAVEL[0] - 0.05, T.T_TRAVEL[0] + 0.1)) * (1 - smooth(ramp(t, T.T_OPEN2[0], T.T_OPEN2[0] + 0.3)))
        draw_chip(canvas, cam, E.ROW_GPKG, c, alpha=a, scale=CHIP_S)
    if t >= T.T_OPEN2[0]:
        c2 = film2(t) if t < LOOP else film2_loop(loop_phase(t, 1))
        img2 = C.window_image(c2, False)
        q, cur, ph, hf = B.bar_state(t, E.TYPE2, E.LABEL2, T.T_ENTER2, 19.02)
        img2 = C.draw_query(img2, 'g_eqL', q, cur, placeholder=ph, hint_fade=hf)
        u = ease_out(ramp(t, T.T_OPEN2[0], T.T_OPEN2[1] - 0.05))
        cr = chip_rect_world(E.ROW_GPKG, W2C, CHIP_S)
        start = (cr[0] - tl2[0], cr[1] - tl2[1], cr[2] - tl2[0], cr[3] - tl2[1])
        C.draw_window(canvas, img2, M2, rect=C.lerp_rect(start, (0, 0, 2822, 1694 + E.TITLE), u),
                      rad=20 * (1 - u) + 36 * u, shadow=0.6 + 0.4 * u)

    # the drop line: the .nc row collapses into it, it becomes the slider's track
    if T.T_DROP + 0.06 < t < T.T_FILM3 + 0.45:
        M = C.cam_matrix(cam)
        cw = chip_image(E.ROW_NC).shape[1] * CHIP_S * 1.6
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
        q, cur, ph, hf = B.bar_state(t, E.TYPE3, E.LABEL3, T.T_ENTER3, 25.67)
        img3 = C.draw_query(img3, 'g_hw', q, cur, placeholder=ph, hint_fade=hf)
        tx0, ty_, tx1 = E.HW_TRACK
        u1 = ease_out(ramp(t, T.T_DROP + 0.16, T.T_DROP + 0.3))
        u2 = ease_out(ramp(t, T.T_DROP + 0.26, T.T_DROP + 0.62))
        rect = C.lerp_rect(C.lerp_rect((tx0, ty_ - 3, tx1, ty_ + 3), E.HW_SLIDER_PANEL, u1), (0, 0, 2822, 1694 + E.TITLE), u2)
        C.draw_window(canvas, img3, M3, rect=rect, rad=3 + 33 * u2 + 20 * u1 * (1 - u2), shadow=u1)

    # ---- window 4: NimbleMap Pro, the same storm shapefile; it opens out of the .shp row
    (tl4, s4) = pos[3]
    M4 = C.local_to_screen(cam, tl4, s4)
    if T.T_PRO[0] - 0.1 < t < T.T_PRO[0] + 0.75:
        u = whip(ramp(t, T.T_PRO[0] - 0.05, T.T_PRO[0] + 0.4))
        p0 = (W4C[0] - 3400, W4C[1] + 300)
        c = (p0[0] + (W4C[0] - p0[0]) * u, p0[1] + (W4C[1] - p0[1]) * u - math.sin(math.pi * u) * 200)
        a = smooth(ramp(t, T.T_PRO[0] - 0.05, T.T_PRO[0] + 0.1)) * (1 - smooth(ramp(t, T.T_PRO[0] + 0.4, T.T_PRO[0] + 0.7)))
        draw_chip(canvas, cam, E.ROW_SHP, c, alpha=a, scale=CHIP_S)
    if t >= T.T_PRO[0] + 0.4:
        c4 = film4(t) if t < LOOP else film4_loop(loop_phase(t, 3))
        img4 = C.window_image(c4, False)
        if t < T.T_SEND and t < LOOP:
            q = C.typed(E.TYPE4, t)
            last = max([31.0] + [k for k in E.TYPE4[1] if k <= t])
            cur = C.blink(t, last) if t > 30.8 else 0.0
            if t > 30.8:
                img4 = draw_ask(img4, q, cur)
        u = ease_out(ramp(t, T.T_PRO[0] + 0.4, T.T_PRO[1]))
        cr = chip_rect_world(E.ROW_SHP, W4C, CHIP_S)
        start = (cr[0] - tl4[0], cr[1] - tl4[1], cr[2] - tl4[0], cr[3] - tl4[1])
        C.draw_window(canvas, img4, M4, rect=C.lerp_rect(start, (0, 0, E.PW, E.PH - 64 + E.TITLE), u),
                      rad=20 * (1 - u) + 36 * u, shadow=0.6 + 0.4 * u)

    # ---- big-number beats: blur and wash behind
    a1 = smooth(ramp(t, 13.45, 13.7)) * (1 - smooth(ramp(t, 14.85, 15.1)))
    a2 = smooth(ramp(t, T.T_M91 - 0.05, T.T_M91 + 0.15)) * (1 - smooth(ramp(t, 22.55, 22.75)))
    a3 = smooth(ramp(t, 28.15, 28.35)) * (1 - smooth(ramp(t, 29.65, 29.9)))
    canvas = B.blur_wash(canvas, a1, (250, 249, 246))
    canvas = B.blur_wash(canvas, a2, (250, 249, 246))
    canvas = B.blur_wash(canvas, a3, (250, 249, 246))

    # the .nc row arrives under film 2 (over the wash); at the DROP it collapses into the line
    if T.T_M91 - 0.1 < t < T.T_DROP + 0.16:
        u = whip(ramp(t, T.T_M91 + 0.4, T.T_M91 + 0.8))
        p0 = (C3[0] - 3600, C3[1] + 200)
        c = (p0[0] + (C3[0] - p0[0]) * u, p0[1] + (C3[1] - p0[1]) * u - math.sin(math.pi * u) * 220)
        sq = ease_in(ramp(t, T.T_DROP, T.T_DROP + 0.12))
        draw_chip(canvas, cam, E.ROW_NC, c, alpha=smooth(ramp(t, T.T_M91 + 0.4, T.T_M91 + 0.5)), scale=CHIP_S * 1.6, squash=sq)

    # ---- motion blur on the fast travels
    c0, c1_ = camera(t - 1 / 60), camera(t + 1 / 60)
    vx, vy = (c1_[0] - c0[0]) * cam[2] / 2, (c1_[1] - c0[1]) * cam[2] / 2
    ln = math.hypot(vx, vy)
    travel = (4.6 <= t <= T.T_OPEN1) or (T.T_TRAVEL[0] <= t <= T.T_TRAVEL[1]) or (T.T_DROP <= t <= T.T_FILM3) \
        or (T.T_PRO[0] <= t <= T.T_PRO[0] + 0.4)
    if travel and ln > 6:
        L = int(min(120, ln * 0.9))
        ker = np.zeros((L * 2 + 1, L * 2 + 1), np.float32)
        cv2.line(ker, (L - int(vx / ln * L), L - int(vy / ln * L)), (L + int(vx / ln * L), L + int(vy / ln * L)), 1.0, 1)
        canvas = cv2.filter2D(canvas, -1, ker / ker.sum())

    im = Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8)).convert('RGBA')

    # ---- overlays
    C.caption(im, E.FORMATS, t, 0.6, 4.3)
    C.caption(im, '4,999 storm tracks', t, 6.2, 9.4)
    if T.T_SANDY + 0.3 <= t <= T.T_SANDY + 1.2:
        x, y = C.apply(M1, *E.SANDY_PX)
        C.ripple(im, x, y, (t - T.T_SANDY - 0.35) / 0.75)
    ty40 = C.window_image(get('g_ty', 40.0), False)
    C.lift_chip(im, (*E.SANDY_RANK, ty40), M1, t, 12.95, 13.65, scale_to=1.3)
    C.big_words(im, '100 knots.', t, 13.7, 14.95, (130, 600), size=230)
    C.big_words(im, 'Sandy, 2012 · Category 3 · higher than 75% of 4,547 tracks', t, 13.85, 14.95, (138, 750), size=46)
    C.caption(im, '14,537 earthquakes since 1900', t, 16.9, 18.95)
    if T.T_JAPAN + 0.15 <= t <= T.T_M91:
        x, y = C.apply(M2, *E.TOHOKU_PX)
        C.ripple(im, x, y, (t - T.T_JAPAN - 0.2) / 0.7, r1=110)
    eq40 = C.window_image(get('g_eqL', 40.0), False)
    C.lift_chip(im, (*E.EQ_MAG, eq40), M2, t, 20.85, 21.35, scale_to=1.25)
    C.big_words(im, 'M 9.1.', t, T.T_M91 + 0.02, 22.75, (120, 300), size=230)
    C.big_words(im, 'Tōhoku, 2011 · higher than 99% of 14,537 earthquakes', t, T.T_M91 + 0.15, 22.75, (128, 450), size=46)
    C.caption(im, 'Heatwaves, summer 2023 · 92 daily steps', t, 23.3, 25.6, pos='tr')
    if T.T_PALERMO + 0.1 <= t <= 28.1:
        x, y = C.apply(M3, *E.PALERMO_PX)
        C.ripple(im, x, y, (t - T.T_PALERMO - 0.15) / 0.7)
    hw168 = C.window_image(get('g_hw', 168.0), False)
    C.lift_chip(im, (*E.HW_VALUE, hw168), M3, t, 27.6, 28.2, scale_to=1.2)
    C.big_words(im, '36.50 °C', t, 28.25, 29.7, (130, 600), size=230)
    C.big_words(im, 'heat-wave intensity, Palermo, 25 Jul 2023 · higher than 93% of cells', t, 28.4, 29.7, (138, 750), size=46)
    C.caption(im, 'Pro: ask in plain words', t, 30.7, 35.1, pos='tr')
    end_bar(im, cam, t)
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
