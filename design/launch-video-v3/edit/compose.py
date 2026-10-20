"""NimbleMap launch film v3 ("one take"): compositor.

One camera over one atlas table (a warm-grey surface with faint graticule lines). The three films
are the app's real windows (frames from the recordings in raw/, via the cache built by
edit/frames.py), placed on the table; the search bars are the windows' own bars, shown alone
before each window opens around its bar. Drawn on top, and nothing else: the typed queries (in
the bar's own font, size, colour and baseline, with a block cursor), the reveal order of the
tracks and dots (a wipe of real pixels over a real plate of the same view), ripples, the lifted
card rows (real crops), captions, and the end card.

    python edit/compose.py out/seg.mkv [--from 0 --to 31]
    python edit/compose.py out/stills --stills 1.0,4.6,...

Every frame is a pure function of t, so renders are repeatable and segments can run in parallel.
"""
import argparse
import math
import os
import subprocess
import sys

import cv2
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(ROOT))
sys.path.insert(0, HERE)
import edl  # noqa: E402
from frames import Frames  # noqa: E402

OW, OH = 1920, 1080
FONT_SEMI = os.path.join(REPO, 'assets/fonts/Inter-SemiBold.ttf')
FONT_REG = os.path.join(REPO, 'assets/fonts/Inter-Regular.ttf')
ACCENT = (37, 99, 235)
INK = (17, 24, 39)
TABLE = np.array([229, 226, 220], np.float32)      # warm grey
GRAT = np.array([214, 210, 203], np.float32)
FR = Frames()


def font(path, size, cache={}):
    k = (path, round(float(size), 2))
    if k not in cache:
        cache[k] = ImageFont.truetype(path, k[1])
    return cache[k]


# ----------------------------------------------------------------- easing / keys
def clamp(u):
    return min(1.0, max(0.0, u))


def smooth(u):
    u = clamp(u)
    return u * u * (3 - 2 * u)


def ease(u):
    u = clamp(u)
    return 0.5 - 0.5 * math.cos(math.pi * u)


def ease_out(u):
    u = clamp(u)
    return 1 - (1 - u) ** 3


def ease_in(u):
    u = clamp(u)
    return u ** 3


def whip(u):
    """Fast in the middle, soft at both ends (a quintic in-out) for the travels."""
    u = clamp(u)
    return 16 * u ** 5 if u < 0.5 else 1 - (-2 * u + 2) ** 5 / 2


def ramp(t, a, b):
    return clamp((t - a) / (b - a))


# ----------------------------------------------------------------- camera
X1 = edl.X1


def W1C():
    return (edl.TL1[0] + 1407, edl.TL1[1] + 877)


def bar_centre(tl, src, dx=0.0):
    x0, y0, x1, y1 = edl.bar_world(tl, src)
    return ((x0 + x1) / 2 + dx, (y0 + y1) / 2)


TL3 = edl.TL3
B1 = bar_centre(edl.TL1, 'ty')
B2 = bar_centre(edl.TL2, 'eq')
W2C = (edl.TL2[0] + 1407, edl.TL2[1] + 877)
W3C = (TL3[0] + 1409, TL3[1] + 877)
CARD1 = (edl.TL1[0] + 2433, edl.TL1[1] + 650)
CARD2 = (edl.TL2[0] + 2435, edl.TL2[1] + 560)

# End layout: the three films in a row (same height) over the bar. The camera ends at ZE; the
# row sits at screen y 120..444, the bar at y 560.
ZE = 520 / 2814
GAP = 40 / ZE
GX, GY = W3C[0] - (2814 + GAP), W3C[1]
CYE = GY - (282 - 540) / ZE
CE = (GX, CYE)


def row_tl(i):
    cx = GX + (i - 1) * (2814 + GAP)
    return (cx - 1407, GY - 877)


# keys: (t, cx, cy, zoom in "x" units, rotation deg, easing of the segment that ENDS here)
CAM = [
    (0.00, B1[0] - 120, B1[1], 2.25, 0, ease),
    (1.00, B1[0] - 120, B1[1], 2.25, 0, ease),          # hold 1 (dead still)
    (3.00, B1[0] - 20, B1[1], 2.36, 0, ease),           # one slow glide
    (4.20, B1[0] - 20, B1[1], 2.36, 0, ease),           # hold 2
    (4.90, W1C()[0], W1C()[1], 0.88, 0, ease),          # ENTER: pull out as the window opens
    (5.90, W1C()[0] + 60, W1C()[1] + 40, 1.0, -1.2, ease),     # push + tilt
    (6.60, W1C()[0] + 30, W1C()[1] + 60, 1.16, 0, ease),       # dive (the app's own fly-in)
    (7.85, W1C()[0] + 200, W1C()[1] + 40, 1.12, 0, ease),
    (8.45, W1C()[0], W1C()[1], 0.92, 1.8, ease),               # pull + slight dutch
    (9.15, CARD1[0], CARD1[1] - 40, 1.62, 0, ease),            # onto the card
    (10.30, CARD1[0] - 80, CARD1[1] - 40, 1.5, 0, ease),       # settle
    (10.80, CARD1[0] - 80, CARD1[1] - 40, 1.5, 0, ease),
    (11.20, B2[0] - 120, B2[1], 2.0, 0, whip),                 # travel right to bar 2
    (12.05, B2[0] - 120, B2[1], 2.0, 0, ease),                 # dead still while typing
    (12.70, W2C[0], W2C[1], 1.02, 0, ease),                    # bar 2 opens: frame fills
    (15.30, W2C[0] - 40, W2C[1] - 30, 1.1, 0, ease),           # handheld creep
    (15.60, W2C[0] - 60, W2C[1] - 40, 1.16, 0, ease),          # the dot lands; ripple
    (16.00, CARD2[0], CARD2[1], 1.85, 0, ease),                # push in on the card
    (16.30, W2C[0], (edl.TL2[1] + edl.BAR3_C[1] + 110) / 2, 0.655, 0, ease),   # film 2 + bar 3
    (16.90, W2C[0], (edl.TL2[1] + edl.BAR3_C[1] + 110) / 2, 0.655, 0, ease),   # dead still
    (17.25, TL3[0] + 1150, TL3[1] + 1150, 1.0, 0, whip),        # whip along the line
    (19.20, TL3[0] + 1350, TL3[1] + 1080, 1.18, 0, ease),       # pan, following the slider
    (20.55, TL3[0] + 1650, TL3[1] + 1000, 1.2, 0, ease),
    (21.10, TL3[0] + 1900, TL3[1] + 820, 1.12, 0, ease),        # the 16 Jul card
    (23.20, W3C[0], W3C[1], 0.82, 0, ease),                     # pull out to the window
    (23.30, W3C[0], W3C[1], 0.82, 0, ease),
    (25.90, CE[0], CE[1], ZE / X1, 0, ease),                    # pull back over the row
    (27.30, CE[0], CE[1], ZE / X1, 0, ease),
    (28.40, CE[0] - 0.0, CE[1] + 30 / ZE, ZE / X1 * 1.06, 0, ease),   # push in, following the pen
    (31.00, CE[0] - 0.0, CE[1] + 30 / ZE, ZE / X1 * 1.06, 0, ease),
]


def handheld(t):
    """A small, slow wander during the earthquake film (decorative camera motion)."""
    a = smooth(ramp(t, 12.6, 13.0)) * (1 - smooth(ramp(t, 15.2, 15.5)))
    if a <= 0:
        return 0.0, 0.0, 0.0
    return (a * (9 * math.sin(t * 1.7) + 5 * math.sin(t * 3.1 + 1)),
            a * (7 * math.sin(t * 1.3 + 2) + 4 * math.sin(t * 2.7)),
            a * 0.25 * math.sin(t * 0.9))


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


def cam_matrix(cam):
    cx, cy, z, rot = cam
    r = math.radians(rot)
    c, s = math.cos(r) * z, math.sin(r) * z
    # screen = R*z*(w - c) + centre
    return np.array([[c, -s, OW / 2 - c * cx + s * cy], [s, c, OH / 2 - s * cx - c * cy]], np.float64)


def compose_affine(A, B):
    """A after B, both 2x3."""
    A3 = np.vstack([A, [0, 0, 1]])
    B3 = np.vstack([B, [0, 0, 1]])
    return (A3 @ B3)[:2]


def apply(M, x, y):
    return M[0, 0] * x + M[0, 1] * y + M[0, 2], M[1, 0] * x + M[1, 1] * y + M[1, 2]


# ----------------------------------------------------------------- table
def table(cam):
    img = np.empty((OH, OW, 3), np.float32)
    img[:] = TABLE
    yy = np.linspace(-1, 1, OH)[:, None]
    xx = np.linspace(-1, 1, OW)[None, :]
    img -= ((xx ** 2 * 0.5 + yy ** 2 * 0.8) * 10)[..., None]
    M = cam_matrix(cam)
    Minv = cv2.invertAffineTransform(M)
    corners = [apply(Minv, x, y) for x, y in [(0, 0), (OW, 0), (0, OH), (OW, OH)]]
    xs = [c[0] for c in corners]
    ys = [c[1] for c in corners]
    step = 750.0
    z = cam[2]
    lw = 1 if z * step > 60 else 0
    if lw == 0:
        step *= 2
    over = np.zeros((OH, OW), np.uint8)
    for gx in np.arange(math.floor(min(xs) / step) * step, max(xs) + step, step):
        p0 = apply(M, gx, min(ys) - step)
        p1 = apply(M, gx, max(ys) + step)
        cv2.line(over, (int(p0[0] * 16), int(p0[1] * 16)), (int(p1[0] * 16), int(p1[1] * 16)), 255, 2, cv2.LINE_AA, 4)
    for gy in np.arange(math.floor(min(ys) / step) * step, max(ys) + step, step):
        p0 = apply(M, min(xs) - step, gy)
        p1 = apply(M, max(xs) + step, gy)
        cv2.line(over, (int(p0[0] * 16), int(p0[1] * 16)), (int(p1[0] * 16), int(p1[1] * 16)), 255, 2, cv2.LINE_AA, 4)
    a = (over.astype(np.float32) / 255 * 0.55)[..., None]
    img = img * (1 - a) + GRAT * a
    # tiny ordered dither against banding
    d = ((np.arange(OW)[None, :] * 7 + np.arange(OH)[:, None] * 13) % 5 - 2) * 0.35
    return img + d[..., None]


# ----------------------------------------------------------------- title bars
def titlebar(width, dark):
    key = (width, dark)
    if key in TITLES:
        return TITLES[key]
    h = edl.TITLE
    g = np.linspace(0, 1, h)[:, None, None]
    if dark:
        top, bot, line, txt = np.array([52, 52, 56]), np.array([44, 44, 48]), (20, 20, 22), (220, 220, 226)
    else:
        top, bot, line, txt = np.array([246, 246, 247]), np.array([236, 236, 239]), (214, 214, 218), (60, 60, 67)
    bar = (top * (1 - g) + bot * g) * np.ones((h, width, 3))
    bar[-2:] = line
    im = Image.fromarray(bar.astype(np.uint8))
    d = ImageDraw.Draw(im)
    for i, col in enumerate([(255, 95, 87), (254, 188, 46), (40, 200, 64)]):
        cx, cy, r = 40 + i * 40, h / 2, 12.5
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=col, outline=tuple(int(c * 0.85) for c in col), width=2)
    d.text((width / 2, h / 2), 'NimbleMap', font=font(FONT_SEMI, 27), fill=txt, anchor='mm')
    TITLES[key] = np.asarray(im)
    return TITLES[key]


TITLES = {}


# ----------------------------------------------------------------- source frames, plates, reveals
def get(src, t):
    return FR.get(src, t)


def aligned_plate(src, t):
    im = get(src, t)
    sh = edl.SOURCES[src].get('shift')
    if sh:
        M = np.float32([[1, 0, sh[0]], [0, 1, sh[1]]])
        im = cv2.warpAffine(im, M, (im.shape[1], im.shape[0]), flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_REPLICATE)
    return im


def fit(im, w, h):
    """Crop or pad (edge) an image to w x h (the takes differ by a few px)."""
    out = im[:h, :w]
    if out.shape[0] < h or out.shape[1] < w:
        out = cv2.copyMakeBorder(out, 0, h - out.shape[0], 0, w - out.shape[1], cv2.BORDER_REPLICATE)
    return out


CACHE = {}


def plate_with_keep(plate_src, plate_t, take_src, take_t, keep):
    k = ('pk', plate_src, take_src, take_t)
    if k not in CACHE:
        take = get(take_src, take_t)
        p = fit(aligned_plate(plate_src, plate_t), take.shape[1], take.shape[0]).copy()
        for x0, y0, x1, y1 in keep:
            y0c, y1c = y0 - edl.TITLE, y1 - edl.TITLE
            p[max(0, y0c):y1c, x0:x1] = take[max(0, y0c):y1c, x0:x1]
        CACHE[k] = p
    return CACHE[k]


def noise_field(h, w, cell, seed):
    rng = np.random.default_rng(seed)
    n = rng.random((h // cell + 2, w // cell + 2)).astype(np.float32)
    return cv2.resize(n, (w, h), interpolation=cv2.INTER_NEAREST if cell <= 8 else cv2.INTER_CUBIC)


def ty_threshold(h, w):
    """Storm tracks draw in east -> west (storms travel west), with a ragged, organic front."""
    k = ('thr_ty', h, w)
    if k not in CACHE:
        x = np.arange(w, dtype=np.float32)[None, :] / w
        n = cv2.GaussianBlur(noise_field(h, w, 40, 3), (0, 0), 30)
        n = (n - n.mean()) / (n.std() + 1e-6)
        fine = noise_field(h, w, 6, 4)
        CACHE[k] = (1 - x) * 0.82 + n * 0.05 + fine * 0.13
    return CACHE[k]


def eq_threshold(h, w):
    """The Ring of Fire lights up dot by dot: a clockwise sweep around the Pacific from Japan,
    each ~8 px cell (one dot) given its own moment, so dots pop one at a time."""
    k = ('thr_eq', h, w)
    if k not in CACHE:
        cx, cy = 1519, 1010 - edl.TITLE
        jx, jy = edl.TOHOKU_PX[0], edl.TOHOKU_PX[1] - edl.TITLE
        yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
        a0 = math.atan2(jy - cy, jx - cx)
        ang = (np.arctan2(yy - cy, xx - cx) - a0) % (2 * math.pi) / (2 * math.pi)
        cells = noise_field(h, w, 8, 7)
        CACHE[k] = ang * 0.78 + cells * 0.22
    return CACHE[k]


def reveal(plate, take, thr, u, soft=0.03):
    if u <= 0:
        return plate
    if u >= 1:
        return take
    m = np.clip((u * (1 + soft) - thr) / soft, 0, 1)[..., None]
    return (plate.astype(np.float32) * (1 - m) + take.astype(np.float32) * m).astype(np.uint8)


def xfade(a, b, u):
    if u <= 0:
        return a
    if u >= 1:
        return b
    return cv2.addWeighted(a, 1 - u, b, u, 0)


# ----------------------------------------------------------------- the films (window content)
def ty_at(f, src_t):
    return get('ty', src_t)


def film1(t):
    """Window 1 content at film time t (the film's own timeline: same as the main one)."""
    A = get('ty', 12.5)
    if t < 4.9:
        return plate_with_keep('ty_plate', 8.0, 'ty', 12.5, edl.TY_KEEP)
    if t < 5.9:
        P = plate_with_keep('ty_plate', 8.0, 'ty', 12.5, edl.TY_KEEP)
        return reveal(P, A, ty_threshold(*A.shape[:2]), ease_in(ramp(t, 4.9, 5.9)) ** 0.6, 0.05)
    if t < 6.6:
        return get('ty', 15.40 + (t - 5.9) * (0.9 / 0.7))
    if t < 7.0:
        return get('ty', 16.30 + (t - 6.6) * 12.5)
    if t < 7.85:
        return get('ty', 21.30 + (t - 7.0) * (1.3 / 0.85))
    if t < 8.45:
        return xfade(get('ty', 22.6), get('ty', 31.0), ease(ramp(t, 7.85, 8.45)))
    return get('ty', 31.0)


def film1_loop(p):
    """Film 1 in the end row: a 3.2 s loop."""
    A = get('ty', 12.5)
    P = plate_with_keep('ty_plate', 8.0, 'ty', 12.5, edl.TY_KEEP)
    if p < 0.35:
        return reveal(P, A, ty_threshold(*A.shape[:2]), p / 0.35, 0.05)
    if p < 0.55:
        return get('ty', 15.40 + (p - 0.35) / 0.2 * 0.9)
    if p < 0.65:
        return xfade(get('ty', 16.3), get('ty', 31.0), (p - 0.55) / 0.1)
    if p > 0.95:
        return xfade(get('ty', 31.0), P, (p - 0.95) / 0.05)
    return get('ty', 31.0)


def film2(t):
    E = get('eq', 18.0)
    P = plate_with_keep('eq_plate', 10.0, 'eq', 18.0, edl.EQ_KEEP)
    if t < 12.7:
        return P
    if t < 15.3:
        return reveal(P, E, eq_threshold(*E.shape[:2]), ramp(t, 12.7, 15.2), 0.015)
    return xfade(E, get('eq', 36.0), ease(ramp(t, 15.3, 15.55)))


def film2_loop(p):
    E = get('eq', 18.0)
    P = plate_with_keep('eq_plate', 10.0, 'eq', 18.0, edl.EQ_KEEP)
    if p < 0.55:
        return reveal(P, E, eq_threshold(*E.shape[:2]), p / 0.55, 0.015)
    if p < 0.62:
        return xfade(E, get('eq', 36.0), (p - 0.55) / 0.07)
    if p > 0.95:
        return xfade(get('eq', 36.0), P, (p - 0.95) / 0.05)
    return get('eq', 36.0)


def hw_index(t):
    """The slider's date index (0 = 1 Jun ... 45 = 16 Jul) during film 3."""
    if t < 17.25:
        return 0
    if t < 20.6:
        return int(45 * ease(ramp(t, 17.35, 20.5)) + 1e-6)
    return 45


def film3(t):
    if t < 20.6:
        return get('hw', edl.hw_step(hw_index(t)))
    if t < 21.10:
        return get('hw', 130.15 + (t - 20.6) * 1.6) if t < 21.05 else get('hw', 133.0)
    if t < 21.45:
        # it wipes back to its first frame (left -> right, a soft edge)
        a, b = get('hw', 133.0), get('hw', edl.hw_step(0))
        u = ease(ramp(t, 21.10, 21.45))
        w = a.shape[1]
        x = np.arange(w, dtype=np.float32) / w
        m = np.clip((u * 1.1 - x) / 0.1, 0, 1)[None, :, None]
        return (a * (1 - m) + b * m).astype(np.uint8)
    if t < 23.2:
        return get('hw', edl.hw_step(int(45 * ease(ramp(t, 21.5, 23.1)) + 1e-6)))
    return get('hw', edl.hw_step(45))


def film3_loop(p):
    if p < 0.7:
        return get('hw', edl.hw_step(int(45 * p / 0.7 + 1e-6)))
    if p < 0.78:
        return get('hw', 130.15 + (p - 0.7) / 0.08 * 0.8)
    return get('hw', 133.0)


# ----------------------------------------------------------------- the search bar (typed text)
def typed(spec, t):
    text, keys = spec
    n = sum(1 for k in keys if t >= k)
    return text[:n]


def draw_query(img, src, text, cursor, placeholder=False, ink=None, hint_fade=0.0):
    """Type into the real search bar of a window image (raw px, title rows included): the
    placeholder is cleared, the text set in the bar's own font/size/colour on its baseline, and a
    block cursor (stylised, terminal-style) drawn after it."""
    b = edl.BAR[src]
    x0, y0, x1, y1 = b['rect']
    sub = Image.fromarray(img[y0:y1, x0:x1].copy())
    d = ImageDraw.Draw(sub)
    f = font(FONT_REG, edl.BAR_FONT)
    base = b['base'] - y0
    tx = b['x'] - x0
    ink = ink or b['ink']
    if not placeholder or hint_fade > 0:
        d.rectangle([tx - 3, 6, x1 - x0 - 14, y1 - y0 - 6], fill=b['fill'])
        if placeholder:
            # the placeholder steps aside for the block cursor (stylised: a fade, 0.3 s)
            hint = tuple(int(c * (1 - hint_fade) + fc * hint_fade) for c, fc in zip(b['hint'], b['fill']))
            d.text((tx, base), 'Search places or your data', font=f, fill=hint, anchor='ls')
        elif text:
            d.text((tx, base), text, font=f, fill=ink, anchor='ls')
    if cursor > 0:
        cx = tx + (d.textlength(text, font=f) + 3 if (text and not placeholder) else 0)
        col = tuple(int(c * cursor + fc * (1 - cursor)) for c, fc in zip(ink, b['fill']))
        d.rectangle([cx, base - 22, cx + 14, base + 6], fill=col)
    out = img.copy() if img.flags.writeable is False else img
    out[y0:y1, x0:x1] = np.asarray(sub)
    return out


def blink(t, t_last_key, period=1.0):
    """Block cursor: solid while typing and for a moment after, then a soft 1 Hz blink."""
    if t - t_last_key < 0.45:
        return 1.0
    ph = ((t - t_last_key - 0.45) / period) % 1.0
    return 1.0 if ph < 0.55 else 0.0


# ----------------------------------------------------------------- windows
def window_image(content, dark):
    w = content.shape[1]
    return np.vstack([titlebar(w, dark), content])


def rounded_mask(w, h, rect, rad, scale):
    """A mask of the window-local rect (x0, y0, x1, y1) with corner radius, drawn at `scale`."""
    sw, sh = max(1, int(w * scale)), max(1, int(h * scale))
    m = Image.new('L', (sw * 2, sh * 2), 0)
    x0, y0, x1, y1 = [v * scale * 2 for v in rect]
    ImageDraw.Draw(m).rounded_rectangle([x0, y0, x1, y1], max(0.5, rad * scale * 2), fill=255)
    return np.asarray(m.resize((sw, sh), Image.BILINEAR))


def draw_window(canvas, img, M, rect=None, rad=36, alpha=1.0, shadow=1.0):
    """Composite a window-local image (raw px) through the 2x3 affine M (local -> screen),
    clipped to `rect` (local px; default the whole window) with rounded corners."""
    h, w = img.shape[:2]
    if rect is None:
        rect = (0, 0, w, h)
    # visible?
    pts = [apply(M, x, y) for x, y in [(rect[0], rect[1]), (rect[2], rect[1]), (rect[0], rect[3]), (rect[2], rect[3])]]
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    if max(xs) < -200 or min(xs) > OW + 200 or max(ys) < -200 or min(ys) > OH + 200 or alpha <= 0.002:
        return
    k = math.hypot(M[0, 0], M[1, 0])
    src, Ms = img, M
    if k < 0.7:
        f = min(1.0, k * 1.4)
        src = cv2.resize(img, (max(1, int(w * f)), max(1, int(h * f))), interpolation=cv2.INTER_AREA)
        Ms = M.copy()
        Ms[:, :2] = M[:, :2] / f
    # only the bounding box of the window on screen
    bx0, by0 = max(0, int(min(xs)) - 2), max(0, int(min(ys)) - 2)
    bx1, by1 = min(OW, int(max(xs)) + 3), min(OH, int(max(ys)) + 3)
    # shadow (soft, below), over a padded box
    if shadow > 0:
        sc = 0.125
        ms = rounded_mask(w, h, rect, rad, sc)
        Mm = M.copy()
        Mm[:, :2] = M[:, :2] / sc
        Mm = Mm.astype(np.float64)
        Mm[:, 2] = Mm[:, 2] * 0.25
        Mm[:, :2] = Mm[:, :2] * 0.25
        sm = cv2.warpAffine(ms, Mm, (OW // 4, OH // 4), flags=cv2.INTER_LINEAR)
        sig = max(2.0, 9 * k / 0.68)
        sm = cv2.GaussianBlur(sm.astype(np.float32) / 255, (0, 0), sig)
        sm = np.roll(sm, max(1, int(round(4 * k / 0.68))), axis=0)
        sm = cv2.resize(sm, (OW, OH), interpolation=cv2.INTER_LINEAR)
        a = (sm * 0.32 * shadow * alpha)[..., None]
        canvas *= (1 - a)
        canvas += np.array([24, 26, 34], np.float32) * a
    if bx1 <= bx0 or by1 <= by0:
        return
    T = np.array([[1, 0, -bx0], [0, 1, -by0]], np.float64)
    Mb = compose_affine(T, Ms)
    body = cv2.warpAffine(src, Mb, (bx1 - bx0, by1 - by0), flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_REPLICATE)
    sc = 0.5
    mm = rounded_mask(w, h, rect, rad, sc)
    Mm = M.copy()
    Mm[:, :2] = M[:, :2] / sc
    Mm = compose_affine(T, Mm)
    a = cv2.warpAffine(mm, Mm, (bx1 - bx0, by1 - by0), flags=cv2.INTER_LINEAR).astype(np.float32)[..., None] / 255 * alpha
    reg = canvas[by0:by1, bx0:bx1]
    reg *= (1 - a)
    reg += body.astype(np.float32) * a


def local_to_screen(cam, tl, scale=1.0):
    """2x3: window-local raw px -> screen, for a window at world top-left tl, drawn at scale."""
    L = np.array([[scale, 0, tl[0]], [0, scale, tl[1]]], np.float64)
    return compose_affine(cam_matrix(cam), L)


def lerp_rect(a, b, u):
    return tuple(x + (y - x) * u for x, y in zip(a, b))


# ----------------------------------------------------------------- screen-space overlays
def caption(im, text, t, t0, t1, pos='bl'):
    a = smooth(ramp(t, t0, t0 + 0.25)) * (1 - smooth(ramp(t, t1 - 0.25, t1)))
    if a <= 0.01:
        return
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    f = font(FONT_SEMI, 40)
    tw = d.textlength(text, font=f)
    h = 76
    x0 = 64 if pos == 'bl' else OW - 64 - (tw + 64)
    y0 = (OH - 64 - h if pos == 'bl' else 64) + (1 - smooth(ramp(t, t0, t0 + 0.25))) * 14
    sh = Image.new('L', im.size, 0)
    ImageDraw.Draw(sh).rounded_rectangle([x0, y0 + 8, x0 + tw + 64, y0 + h + 8], h / 2, fill=int(80 * a))
    sh = sh.filter(ImageFilter.GaussianBlur(14))
    ov.paste((15, 23, 42, 255), (0, 0), sh)
    d.rounded_rectangle([x0, y0, x0 + tw + 64, y0 + h], h / 2, fill=(255, 255, 255, int(244 * a)))
    d.text((x0 + 32, y0 + h / 2 + 1), text, font=f, fill=INK + (int(255 * a),), anchor='lm')
    im.alpha_composite(ov)


def big_words(im, text, t, t0, t1, xy, size=210, light=False, anchor='lm'):
    a = smooth(ramp(t, t0, t0 + 0.18)) * (1 - smooth(ramp(t, t1 - 0.2, t1)))
    if a <= 0.01:
        return
    s = 0.92 + 0.08 * ease_out(ramp(t, t0, t0 + 0.35))
    f = font(FONT_SEMI, size * s)
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    sh = Image.new('L', im.size, 0)
    ImageDraw.Draw(sh).text((xy[0], xy[1] + 6), text, font=f, fill=int(150 * a), anchor=anchor)
    sh = sh.filter(ImageFilter.GaussianBlur(16))
    ov.paste((0, 0, 0, 255) if light else (255, 255, 255, 255), (0, 0), sh)
    d = ImageDraw.Draw(ov)
    d.text(xy, text, font=f, fill=((255, 255, 255) if light else INK) + (int(255 * a),), anchor=anchor)
    im.alpha_composite(ov)


def wash(canvas, a, color):
    if a > 0:
        canvas *= (1 - a)
        canvas += np.array(color, np.float32) * a


def lift_chip(im, crop, M, t, t0, t_out, target=None, scale_to=1.35, bg=(255, 255, 255), dur=0.35):
    """A real crop of a card row (window-local rect `crop` of image `crop[4]`) lifted off the
    card: it rises from where it sits on screen, grows and gains a shadow and its own plate."""
    if t < t0 or t > t_out:
        return
    x0, y0, x1, y1, img = crop
    u = ease_out(ramp(t, t0, t0 + dur))
    a = 1 - smooth(ramp(t, t_out - 0.25, t_out))
    p0 = apply(M, x0, y0)
    p1 = apply(M, x1, y1)
    k = math.hypot(M[0, 0], M[1, 0])
    w0, h0 = (x1 - x0) * k, (y1 - y0) * k
    cx0, cy0 = (p0[0] + p1[0]) / 2, (p0[1] + p1[1]) / 2
    s = 1 + (scale_to - 1) * u
    if target is not None:
        tu = ease(ramp(t, target[2], target[2] + target[3]))
        cx0 += (target[0] - cx0) * tu
        cy0 += (target[1] - cy0) * tu
        s *= 1 + (target[4] - 1) * tu
    cy0 -= 18 * u
    w, h = w0 * s, h0 * s
    chip = Image.fromarray(img[y0:y1, x0:x1]).resize((max(1, int(w)), max(1, int(h))), Image.LANCZOS)
    pad = 22 * s * k / 0.68 * 0.6
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    sh = Image.new('L', im.size, 0)
    bx0, by0, bx1, by1 = cx0 - w / 2 - pad, cy0 - h / 2 - pad, cx0 + w / 2 + pad, cy0 + h / 2 + pad
    ImageDraw.Draw(sh).rounded_rectangle([bx0, by0 + 16 * u, bx1, by1 + 16 * u], 18, fill=int(110 * u * a))
    sh = sh.filter(ImageFilter.GaussianBlur(20))
    ov.paste((10, 14, 30, 255), (0, 0), sh)
    d = ImageDraw.Draw(ov)
    d.rounded_rectangle([bx0, by0, bx1, by1], 18, fill=bg + (int(255 * a),))
    ca = chip.convert('RGBA')
    ca.putalpha(int(255 * a))
    ov.alpha_composite(ca, (int(cx0 - w / 2), int(cy0 - h / 2)))
    im.alpha_composite(ov)


def ripple(im, x, y, u, color=ACCENT, r0=8, r1=90):
    if not (0 <= u <= 1):
        return
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    for lag, w_ in [(0.0, 5), (0.22, 3)]:
        v = clamp((u - lag) / (1 - lag))
        if v <= 0 or v >= 1:
            continue
        r = r0 + (r1 - r0) * ease_out(v)
        a = int(230 * (1 - v) ** 1.5)
        d.ellipse([x - r, y - r, x + r, y + r], outline=color + (a,), width=w_)
    im.alpha_composite(ov)


# ----------------------------------------------------------------- the end
def end_bar(im, cam, t):
    """The bar under the row of films: a white pill with the grey magnifier; the block cursor
    writes "NimbleMap" in one stroke (27.3-28.4)."""
    a = smooth(ramp(t, 24.4, 25.3))
    if a <= 0:
        return
    M = cam_matrix(cam)
    k = cam[2] / ZE
    cx, cy = apply(M, CE[0], CE[1] + 20 / ZE)
    w, h = 860 * k, 108 * k
    rise = (1 - ease_out(ramp(t, 24.4, 25.3))) * 30
    cy += rise
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    sh = Image.new('L', im.size, 0)
    ImageDraw.Draw(sh).rounded_rectangle([cx - w / 2, cy - h / 2 + 12 * k, cx + w / 2, cy + h / 2 + 12 * k], h / 2, fill=int(70 * a))
    sh = sh.filter(ImageFilter.GaussianBlur(22 * k))
    ov.paste((20, 22, 30, 255), (0, 0), sh)
    d = ImageDraw.Draw(ov)
    d.rounded_rectangle([cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2], h / 2, fill=(255, 255, 255, int(255 * a)))
    # magnifier (the app's glyph: a ring and a short handle), grey
    mx, my, r = cx - w / 2 + 62 * k, cy - 4 * k, 17 * k
    grey = (110, 116, 126, int(255 * a))
    d.ellipse([mx - r, my - r, mx + r, my + r], outline=grey, width=max(2, int(4.5 * k)))
    d.line([mx + r * 0.72, my + r * 0.72, mx + r * 1.55, my + r * 1.55], fill=grey, width=max(2, int(5 * k)))
    f = font(FONT_SEMI, 62 * k)
    tx = cx - w / 2 + 112 * k
    base = cy + 22 * k
    name = 'NimbleMap'
    tw = d.textlength(name, font=f)
    pen = ease(ramp(t, edl.T_PEN[0], edl.T_PEN[1]))
    frozen = edl.T_STOP <= t < edl.T_PEN[0]
    if t < edl.T_PEN[0]:
        d.text((tx + 44 * k, base), 'Search places or your data', font=font(FONT_REG, 50 * k), fill=(120, 126, 136, int(255 * a)), anchor='ls')
        cur_x = tx
        on = 1.0 if frozen else blink(t, 25.3)
    else:
        layer = Image.new('L', im.size, 0)
        ImageDraw.Draw(layer).text((tx, base), name, font=f, fill=255, anchor='ls')
        clip = np.zeros((OH, OW), np.float32)
        xe = tx + tw * pen
        x0i = int(max(0, tx - 4))
        x1i = int(min(OW, xe))
        if x1i > x0i:
            clip[:, x0i:x1i] = 1
            # a soft leading edge, like ink
            e0 = int(max(x0i, xe - 10 * k))
            clip[:, e0:x1i] = np.linspace(1, 0.2, x1i - e0)[None, :]
        al = (np.asarray(layer).astype(np.float32) * clip * a).astype(np.uint8)
        ink = Image.new('RGBA', im.size, INK + (255,))
        ink.putalpha(Image.fromarray(al))
        ov.alpha_composite(ink)
        cur_x = xe + 6 * k
        on = 1.0 if pen < 1 else blink(t, edl.T_PEN[1] + 0.2)
    if on > 0:
        d.rectangle([cur_x, base - 46 * k, cur_x + 30 * k, base + 10 * k], fill=INK + (int(235 * a * on),))
    im.alpha_composite(ov)


def end_card(im, t):
    def fade(t0, dur=0.45):
        return smooth((t - t0) / dur)
    a1, a2, a3 = fade(edl.T_END), fade(edl.T_END + 0.35), fade(edl.T_END + 0.7)
    if a1 <= 0:
        return
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    d.text((OW / 2, 735 + (1 - a1) * 12), 'Explore real data about the planet.', font=font(FONT_REG, 52),
           fill=(40, 44, 54, int(255 * a1)), anchor='mm')
    f = font(FONT_SEMI, 38)
    txt = 'Free for Mac'
    tw = d.textlength(txt, font=f)
    py = 836 + (1 - a2) * 10
    d.rounded_rectangle([OW / 2 - tw / 2 - 32, py - 35, OW / 2 + tw / 2 + 32, py + 35], 35, fill=ACCENT + (int(255 * a2),))
    d.text((OW / 2, py + 1), txt, font=f, fill=(255, 255, 255, int(255 * a2)), anchor='mm')
    d.text((OW / 2, 940 + (1 - a3) * 8), 'kangning‐huang.github.io/NimbleMap', font=font(FONT_REG, 38),
           fill=(55, 60, 72, int(255 * a3)), anchor='mm')
    im.alpha_composite(ov)


# ----------------------------------------------------------------- one frame
def window_state(t):
    """Per window: (top-left, scale) on the table; during the gather they slide into the row."""
    out = {}
    g = ease(ramp(t, edl.T_GATHER[0], edl.T_GATHER[1] - 0.3))
    for i, tl in enumerate([edl.TL1, edl.TL2, TL3]):
        rt = row_tl(i)
        out[i] = ((tl[0] + (rt[0] - tl[0]) * g, tl[1] + (rt[1] - tl[1]) * g), 1.0)
    return out, g


def loop_phase(t, i):
    """The films playing in the row (a frozen frame from the stop on)."""
    tt = min(t, edl.T_STOP)
    period = [3.2, 3.0, 2.8][i]
    return ((tt - 23.2) / period + [0.62, 0.35, 0.0][i]) % 1.0


def render(t):
    cam = camera(t)
    canvas = table(cam)
    im_over = []
    pos, g = window_state(t)

    # ---- window 1: typhoons
    tl1, s1 = pos[0]
    M1 = local_to_screen(cam, tl1, s1)
    bar1 = edl.BAR['ty']['rect']
    full = (0, 0, 2814, 1686 + edl.TITLE)
    if t < 23.25:
        c1 = film1(t) if t < 23.0 else film1_loop(0.62)
    else:
        c1 = film1_loop(loop_phase(t, 0))
    img1 = window_image(c1, False)
    q1 = typed(edl.TYPE1, t)
    cur1 = blink(t, max([0.0] + [k for k in edl.TYPE1[1] if k <= t])) if t < edl.T_ENTER else 0.0
    if t < edl.T_ENTER and t >= edl.T_ENTER - 0.06:
        cur1 = 1.0
    hf1 = smooth(ramp(t, 0.55, 0.8))
    if t < 0.8:
        cur1 = 0.0 if t < 0.62 else 1.0
    img1 = draw_query(img1, 'ty', q1, cur1, placeholder=(q1 == ''), hint_fade=hf1)
    if t < edl.T_ENTER:
        draw_window(canvas, img1, M1, rect=bar1, rad=edl.BAR_RADIUS, shadow=0.6)
    else:
        u = ease_out(ramp(t, edl.T_ENTER, edl.T_ENTER + 0.6))
        rect = lerp_rect(bar1, full, u)
        draw_window(canvas, img1, M1, rect=rect, rad=edl.BAR_RADIUS + (36 - edl.BAR_RADIUS) * u, shadow=0.6 + 0.4 * u)

    # ---- window 2: earthquakes (dark). Bar 2 is shown as a white pill until it opens.
    tl2, s2 = pos[1]
    M2 = local_to_screen(cam, tl2, s2)
    if t > 10.7:
        bar2 = edl.BAR['eq']['rect']
        full2 = (0, 0, 2814, 1689 + edl.TITLE)
        q2 = typed(edl.TYPE2, t)
        if t < edl.T_OPEN2[0]:
            # the white pill: the light bar (from film 1's frame) at bar 2's place on the table
            light = window_image(get('ty', 12.5), False)
            cur2 = blink(t, max([10.9] + [k for k in edl.TYPE2[1] if k <= t]))
            light = draw_query(light, 'ty', q2, cur2, placeholder=(q2 == ''), hint_fade=1.0)
            Mp = local_to_screen(cam, (tl2[0] + bar2[0] - bar1[0], tl2[1] + bar2[1] - bar1[1]), 1.0)
            a = smooth(ramp(t, 10.75, 11.05))
            draw_window(canvas, light, Mp, rect=bar1, rad=edl.BAR_RADIUS, alpha=a, shadow=0.6)
        else:
            if t < 23.25:
                c2 = film2(t) if t < 23.0 else film2_loop(0.35)
            else:
                c2 = film2_loop(loop_phase(t, 1))
            img2 = draw_query(window_image(c2, True), 'eq', q2, 0.0)
            u = ease_out(ramp(t, edl.T_OPEN2[0], edl.T_OPEN2[1] - 0.05))
            rect = lerp_rect(bar2, full2, u)
            draw_window(canvas, img2, M2, rect=rect, rad=edl.BAR_RADIUS + (36 - edl.BAR_RADIUS) * u, shadow=0.6 + 0.4 * u)
            if u < 1:   # the white pill fades into the dark bar as the window opens
                light = draw_query(window_image(get('ty', 12.5), False), 'ty', q2, 0.0)
                Mp = local_to_screen(cam, (tl2[0] + bar2[0] - bar1[0], tl2[1] + bar2[1] - bar1[1]), 1.0)
                draw_window(canvas, light, Mp, rect=bar1, rad=edl.BAR_RADIUS, alpha=1 - smooth(ramp(t, edl.T_OPEN2[0], edl.T_OPEN2[0] + 0.3)), shadow=0)

    # ---- bar 3 (a free pill on the table, 2x) and the line it becomes
    if 15.9 < t < 17.6:
        hwimg = window_image(get('hw', edl.hw_step(0)), False)
        q3 = typed(edl.TYPE3, t)
        cur3 = blink(t, max([16.05] + [k for k in edl.TYPE3[1] if k <= t])) if t < edl.T_DROP else 0.0
        hwimg = draw_query(hwimg, 'hw', q3, cur3, placeholder=(q3 == ''), hint_fade=smooth(ramp(t, 16.15, 16.3)))
        b3 = edl.BAR['hw']['rect']
        sc = edl.BAR3_SCALE
        tlb = (edl.BAR3_C[0] - sc * (b3[0] + b3[2]) / 2, edl.BAR3_C[1] - sc * (b3[1] + b3[3]) / 2)
        Mb = local_to_screen(cam, tlb, sc)
        a = smooth(ramp(t, 15.95, 16.2))
        # DROP: the pill collapses into a line (16.90-17.02)
        cu = ease_in(ramp(t, edl.T_DROP, edl.T_DROP + 0.12))
        ymid = (b3[1] + b3[3]) / 2
        rect = (b3[0], b3[1] + (ymid - 2.5 - b3[1]) * cu, b3[2], b3[3] - (b3[3] - ymid - 2.5) * cu)
        if t < edl.T_DROP + 0.14:
            draw_window(canvas, hwimg, Mb, rect=rect, rad=edl.BAR_RADIUS * (1 - cu) + 2, alpha=a, shadow=0.6 * (1 - cu))
    # the line shoots across the table and becomes the time slider's track
    if edl.T_DROP + 0.06 < t < 17.7:
        M = cam_matrix(cam)
        sc = edl.BAR3_SCALE
        b3 = edl.BAR['hw']['rect']
        lx0 = edl.BAR3_C[0] - sc * (b3[2] - b3[0]) / 2
        ly = edl.BAR3_C[1]
        tx0, tx1 = TL3[0] + edl.HW_TRACK[0], TL3[0] + edl.HW_TRACK[2]
        shoot = whip(ramp(t, edl.T_DROP + 0.06, 17.2))
        tail = ease(ramp(t, 17.0, 17.35))
        xa = lx0 + (tx0 - lx0) * tail
        xb = (edl.BAR3_C[0] + sc * (b3[2] - b3[0]) / 2) + (tx1 - (edl.BAR3_C[0] + sc * (b3[2] - b3[0]) / 2)) * shoot
        fade = 1 - smooth(ramp(t, 17.35, 17.65))
        p0, p1 = apply(M, xa, ly), apply(M, xb, ly)
        th = max(2, int(round(9 * cam[2])))
        lay = np.zeros((OH, OW), np.uint8)
        cv2.line(lay, (int(p0[0] * 16), int(p0[1] * 16)), (int(p1[0] * 16), int(p1[1] * 16)), 255, th, cv2.LINE_AA, 4)
        al = (lay.astype(np.float32) / 255 * fade)[..., None]
        canvas *= (1 - al)
        canvas += np.array(ACCENT, np.float32) * al

    # ---- window 3: heatwaves: it opens out of its own slider track
    tl3, s3 = pos[2]
    M3 = local_to_screen(cam, tl3, s3)
    if t > 17.0:
        if t < 23.25:
            c3 = film3(t)
        else:
            c3 = film3_loop(loop_phase(t, 2))
        img3 = draw_query(window_image(c3, False), 'hw', 'heat', 0.0)
        tx0, ty_, tx1 = edl.HW_TRACK
        start = (tx0, ty_ - 3, tx1, ty_ + 3)
        mid = edl.HW_SLIDER_PANEL
        full3 = (0, 0, 2818, 1690 + edl.TITLE)
        u1 = ease_out(ramp(t, 17.06, 17.2))
        u2 = ease_out(ramp(t, 17.16, 17.5))
        rect = lerp_rect(lerp_rect(start, mid, u1), full3, u2)
        rad = 3 + 33 * u2 + 20 * u1 * (1 - u2)
        draw_window(canvas, img3, M3, rect=rect, rad=rad, shadow=u1)

    # ---- washes behind the big words
    wash(canvas, 0.78 * smooth(ramp(t, 9.35, 9.6)) * (1 - smooth(ramp(t, 10.75, 11.0))), (250, 249, 246))
    wash(canvas, 0.45 * smooth(ramp(t, 15.95, 16.15)) * (1 - smooth(ramp(t, 16.8, 16.95))), (14, 16, 24))

    # ---- motion blur on the fast travels (along the camera's screen velocity)
    c0, c1_ = camera(t - 1 / 60), camera(t + 1 / 60)
    vx = (c1_[0] - c0[0]) * cam[2] / 2
    vy = (c1_[1] - c0[1]) * cam[2] / 2
    ln = math.hypot(vx, vy)
    travel = (edl.T_TRAVEL[0] <= t <= edl.T_TRAVEL[1]) or (edl.T_DROP <= t <= edl.T_FILM3)
    if travel and ln > 6:
        L = int(min(120, ln * 0.9))
        ker = np.zeros((L * 2 + 1, L * 2 + 1), np.float32)
        cv2.line(ker, (L - int(vx / ln * L), L - int(vy / ln * L)), (L + int(vx / ln * L), L + int(vy / ln * L)), 1.0, 1)
        ker /= ker.sum()
        canvas = cv2.filter2D(canvas, -1, ker)

    im = Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8)).convert('RGBA')

    # ---- overlays
    # film 1
    caption(im, '4,999 storm tracks', t, 5.0, 7.3)
    if 7.0 <= t <= 7.8:
        x, y = apply(M1, *edl.HAIYAN_PX)
        ripple(im, x, y, (t - 7.04) / 0.7)
    rank_src = window_image(get('ty', 31.0), False)
    lift_chip(im, (*edl.TY_WIND_RANK, rank_src), M1, t, 8.75, 9.45, scale_to=1.3)
    lift_chip(im, (*edl.TY_WIND_ROW, rank_src), M1, t, 9.40, 10.85, scale_to=1.15,
              target=(700, 330, 9.45, 0.55, 1.55))
    big_words(im, '170 knots.', t, 9.62, 10.85, (130, 650), size=230)
    # film 2
    if edl.T_OPEN2[0] <= t <= 15.4:
        # "Tohoku" flies from the bar to the corner
        u = ease(ramp(t, edl.T_OPEN2[0] + 0.12, edl.T_OPEN2[1]))
        bx, by = apply(M2, edl.BAR['eq']['x'], edl.BAR['eq']['base'])
        k = math.hypot(M2[0, 0], M2[1, 0])
        tw_end = font(FONT_SEMI, 66).getlength('Tohoku')
        x = bx + (OW - 80 - tw_end - bx) * u
        y = by + (130 - by) * u
        size = edl.BAR_FONT * k + (66 - edl.BAR_FONT * k) * u
        a = (1 - smooth(ramp(t, 15.1, 15.4))) * smooth(ramp(t, edl.T_OPEN2[0] + 0.12, edl.T_OPEN2[0] + 0.3))
        ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
        sh = Image.new('L', im.size, 0)
        ImageDraw.Draw(sh).text((x, y + 4), 'Tohoku', font=font(FONT_SEMI, size), fill=int(160 * a * u), anchor='ls')
        sh = sh.filter(ImageFilter.GaussianBlur(10))
        ov.paste((0, 0, 0, 255), (0, 0), sh)
        ImageDraw.Draw(ov).text((x, y), 'Tohoku', font=font(FONT_SEMI if u > 0.5 else FONT_REG, size),
                                fill=(245, 245, 248, int(255 * a)), anchor='ls')
        im.alpha_composite(ov)
    caption(im, '14,537 earthquakes since 1900', t, 12.95, 15.25)
    if 15.3 <= t <= 16.1:
        x, y = apply(M2, *edl.TOHOKU_PX)
        ripple(im, x, y, (t - 15.32) / 0.75, color=(255, 255, 255), r1=110)
    eq_src = window_image(get('eq', 36.0), True)
    lift_chip(im, (*edl.EQ_MAG, eq_src), M2, t, 15.6, 15.98, scale_to=1.25, bg=(31, 31, 35))
    big_words(im, 'M 9.1.', t, 16.02, 16.92, (120, 300), size=230, light=True)
    # film 3
    caption(im, 'Heatwaves, summer 2023 · 92 daily steps', t, 17.45, 20.5, pos='tr')
    if 20.62 <= t <= 21.4:
        x, y = apply(M3, *edl.HEAT_PX)
        ripple(im, x, y, (t - 20.64) / 0.7)
    # the end
    end_bar(im, cam, t)
    end_card(im, t)
    return np.asarray(im.convert('RGB'))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--fps', type=int, default=edl.FPS)
    ap.add_argument('--from', dest='t0', type=float, default=0.0)
    ap.add_argument('--to', dest='t1', type=float, default=edl.DUR)
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
