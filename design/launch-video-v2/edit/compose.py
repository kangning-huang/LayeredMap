"""NimbleMap launch film v2: compositor.

Reads the edit decision list (edit/edl.py), pulls frames from the real window recordings in
raw/*.mov, and composes each output frame: calm background -> soft shadow -> a macOS-style
window frame holding the real app pixels (camera push-ins/pans applied) -> drawn cursor with
click ripples -> baked-in captions -> end card. Pipes raw frames into ffmpeg.

    python edit/compose.py out/picture.mov [--fps 60] [--from 0 --to 30] [--stills t1,t2,...]

Every frame is a pure function of t (no clocks, no randomness), so renders are repeatable.
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

W, H = 1920, 1080
FONT_SEMI = os.path.join(REPO, 'assets/fonts/Inter-SemiBold.ttf')
FONT_REG = os.path.join(REPO, 'assets/fonts/Inter-Regular.ttf')
ICON = os.path.join(REPO, 'assets/icon/icon-256.png')

# Window layout at camera zoom 1: the app content is 1280x800 pt (aspect 1.6) under a drawn
# 28 pt title bar.
BASE_W = 1440.0            # content width in output px at zoom 1
TB_PT = 28.0               # title bar height, in app points
ACCENT = (37, 99, 235)


def font(path, size, cache={}):
    k = (path, round(float(size), 2))
    if k not in cache:
        cache[k] = ImageFont.truetype(path, k[1])
    return cache[k]


# ----------------------------------------------------------------- easing
def smooth(u):
    u = min(1.0, max(0.0, u))
    return u * u * (3 - 2 * u)


def ease_io(u):
    u = min(1.0, max(0.0, u))
    return 0.5 - 0.5 * math.cos(math.pi * u)


def interp_keys(keys, t, ease=ease_io):
    """keys: [(t, v0, v1, ...)] sorted; eased piecewise interpolation, held at the ends."""
    if t <= keys[0][0]:
        return keys[0][1:]
    for a, b in zip(keys, keys[1:]):
        if t <= b[0]:
            u = ease((t - a[0]) / max(1e-9, b[0] - a[0]))
            return tuple(x + (y - x) * u for x, y in zip(a[1:], b[1:]))
    return keys[-1][1:]


# ----------------------------------------------------------------- background
def make_background():
    y, x = np.mgrid[0:H, 0:W].astype(np.float32)
    # calm, warm-cool light gradient
    top = np.array([236, 240, 246], np.float32)   # RGB
    bot = np.array([222, 229, 240], np.float32)
    u = (y / H)[..., None]
    img = top * (1 - u) + bot * u
    r = np.sqrt(((x - W * 0.5) / W) ** 2 + ((y - H * 0.42) / H) ** 2)[..., None]
    img += (1 - np.clip(r * 1.6, 0, 1)) * 8
    # tiny ordered dither against banding
    img += (((x.astype(int) * 7 + y.astype(int) * 13) % 5) - 2)[..., None] * 0.35
    return np.clip(img, 0, 255).astype(np.uint8)


BG = make_background()


# ----------------------------------------------------------------- sources
class Source:
    """Sequential CFR frame reader over one raw recording (content crop applied)."""

    def __init__(self, path, crop, fps):
        self.path, self.crop, self.fps = path, crop, fps
        self.proc = None
        self.idx = -1
        self.frame = None
        self.start = None

    def _open(self, t0):
        if self.proc:
            self.proc.kill()
        x, y, w, h = self.crop
        self.w, self.h = w, h
        cmd = ['ffmpeg', '-v', 'error', '-ss', f'{t0:.4f}', '-i', self.path,
               '-vf', f'fps={self.fps},crop={w}:{h}:{x}:{y}', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-']
        self.proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, bufsize=w * h * 3 * 4)
        self.start = t0
        self.idx = -1

    def get(self, ts):
        """Frame at source time ts (s). Seeks forward by reading; reopens to go back."""
        want = int(round(ts * self.fps))
        if self.proc is None or want < self.idx + int(round(self.start * self.fps)) or \
                want - (self.idx + int(round(self.start * self.fps))) > self.fps * 4:
            self._open(max(0.0, want / self.fps))
        base = int(round(self.start * self.fps))
        n = self.w * self.h * 3
        while self.idx + base < want:
            buf = self.proc.stdout.read(n)
            if len(buf) < n:
                break
            self.frame = np.frombuffer(buf, np.uint8).reshape(self.h, self.w, 3)
            self.idx += 1
        return self.frame


SOURCES = {}


def source(name, fps, key=None):
    """One sequential reader per shot (key), so a dissolve never makes a reader seek back."""
    k = (name, key)
    if k not in SOURCES:
        meta = edl.SOURCES[name]
        SOURCES[k] = Source(os.path.join(ROOT, 'cfr', name + '.mov'), meta['crop'], fps)
    return SOURCES[k]


# ----------------------------------------------------------------- drawing helpers
def rounded_mask(w, h, r, ss=1):
    m = Image.new('L', (w * ss, h * ss), 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, w * ss - 1, h * ss - 1], r * ss, fill=255)
    return m if ss == 1 else m.resize((w, h), Image.LANCZOS)


def window_geometry(cam, aspect=1.667):
    """Output-space rect of the app content for camera (cx, cy, z)."""
    cx, cy, z = cam
    cw = BASE_W * z
    ch = cw / aspect
    tb = TB_PT * cw / 1405.0
    # window total (title bar + content) sits 40 px from the top at zoom 1, the caption band below.
    win_h0 = BASE_W / aspect + TB_PT * BASE_W / 1405.0
    centre_y = 40 + win_h0 / 2   # top margin 40 px at zoom 1
    # camera point (cx, cy) in content-normalised coords sits at the base position of that point
    # plus a pull toward the frame centre as we zoom.
    bx0 = W / 2 - BASE_W / 2
    by0 = centre_y - win_h0 / 2 + TB_PT * BASE_W / 1405.0
    base_px = bx0 + cx * BASE_W
    base_py = by0 + cy * BASE_W / aspect
    k = min(1.0, max(0.0, (z - 1.0) / 0.35))   # 0 at z=1 -> 1 when zoomed in
    tx = base_px + (W / 2 - base_px) * k
    ty = base_py + (H * 0.46 - base_py) * k
    x0 = tx - cx * cw
    y0 = ty - cy * ch
    return x0, y0, cw, ch, tb


def to_out(cam, p, aspect=1.667):
    x0, y0, cw, ch, _ = window_geometry(cam, aspect)
    return x0 + p[0] * cw, y0 + p[1] * ch


def draw_window(canvas, frame, cam, alpha=1.0):
    x0, y0, cw, ch, tb = window_geometry(cam, frame.shape[1] / frame.shape[0])
    wx0, wy0 = x0, y0 - tb
    ww, wh = cw, ch + tb
    rad = 18 * cw / BASE_W
    # shadow
    pad = 120
    sx0, sy0 = int(math.floor(wx0)) - pad, int(math.floor(wy0)) - pad
    sw, sh = int(ww) + 2 * pad, int(wh) + 2 * pad
    sh_img = Image.new('L', (sw, sh), 0)
    ImageDraw.Draw(sh_img).rounded_rectangle([pad, pad + 18, pad + ww, pad + 18 + wh], rad, fill=int(70 * alpha))
    sh_img = sh_img.filter(ImageFilter.GaussianBlur(34))
    sh2 = Image.new('L', (sw, sh), 0)
    ImageDraw.Draw(sh2).rounded_rectangle([pad, pad + 3, pad + ww, pad + 3 + wh], rad, fill=int(55 * alpha))
    sh2 = sh2.filter(ImageFilter.GaussianBlur(5))
    comp_alpha(canvas, (20, 30, 50), np.maximum(np.asarray(sh_img), np.asarray(sh2)), sx0, sy0)

    # window body (title bar + content), warped at sub-pixel precision
    fh, fw = frame.shape[:2]
    s = cw / fw
    src = frame
    if s < 0.95:
        sig = 0.42 / s
        src = cv2.GaussianBlur(frame, (0, 0), sig)
    ix0, iy0 = int(math.floor(wx0)), int(math.floor(wy0))
    iw, ih = int(math.ceil(ww)) + 2, int(math.ceil(wh)) + 2
    M = np.float32([[s, 0, x0 - ix0], [0, s, y0 - iy0]])
    body = cv2.warpAffine(src, M, (iw, ih), flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_CONSTANT, borderValue=(255, 255, 255))
    # title bar (drawn frame: light macOS style)
    tbh = int(round(y0 - iy0))
    body = draw_titlebar(body, tbh, cw / BASE_W, x0 - ix0)
    # rounded mask at sub-pixel offset
    m = Image.new('L', (iw * 2, ih * 2), 0)
    ox, oy = (wx0 - ix0) * 2, (wy0 - iy0) * 2
    ImageDraw.Draw(m).rounded_rectangle([ox, oy, ox + ww * 2, oy + wh * 2], rad * 2, fill=255)
    m = np.asarray(m.resize((iw, ih), Image.BILINEAR)).astype(np.float32) / 255 * alpha
    # hairline border
    blit(canvas, body, m, ix0, iy0)
    border = Image.new('L', (iw * 2, ih * 2), 0)
    ImageDraw.Draw(border).rounded_rectangle([ox, oy, ox + ww * 2, oy + wh * 2], rad * 2, outline=255, width=2)
    border = np.asarray(border.resize((iw, ih), Image.BILINEAR)).astype(np.float32) / 255 * 0.16 * alpha
    comp_alpha(canvas, (0, 0, 0), border * 255, ix0, iy0)


def draw_titlebar(body, tbh, k, cx0):
    if tbh <= 0:
        return body
    body = body.copy()
    bar = np.zeros((tbh, body.shape[1], 3), np.float32)
    g = np.linspace(0, 1, tbh)[:, None, None]
    bar[:] = np.array([246, 246, 247]) * (1 - g) + np.array([238, 238, 240]) * g
    body[:tbh] = bar.astype(np.uint8)
    body[tbh - 1:tbh] = (np.array([214, 214, 218])).astype(np.uint8)
    im = Image.fromarray(body)
    d = ImageDraw.Draw(im)
    r = 6.2 * k * 1.12
    cy = tbh / 2
    for i, col in enumerate([(255, 95, 87), (254, 188, 46), (40, 200, 64)]):
        cx = cx0 + (20 + i * 20) * k * 1.12
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=col, outline=tuple(int(c * 0.85) for c in col))
    f = font(FONT_SEMI, 13.5 * k * 1.12)
    title = 'NimbleMap'
    tw = d.textlength(title, font=f)
    d.text((cx0 + BASE_W * k / 2 - tw / 2, cy), title, font=f, fill=(60, 60, 67), anchor='lm')
    return np.asarray(im)


def blit(canvas, img, a, x, y):
    H_, W_ = canvas.shape[:2]
    h, w = img.shape[:2]
    x1, y1 = max(0, x), max(0, y)
    x2, y2 = min(W_, x + w), min(H_, y + h)
    if x2 <= x1 or y2 <= y1:
        return
    sub = canvas[y1:y2, x1:x2].astype(np.float32)
    im = img[y1 - y:y2 - y, x1 - x:x2 - x].astype(np.float32)
    aa = a[y1 - y:y2 - y, x1 - x:x2 - x][..., None]
    canvas[y1:y2, x1:x2] = (sub * (1 - aa) + im * aa + 0.5).astype(np.uint8)


def comp_alpha(canvas, color, a255, x, y):
    a = np.asarray(a255, np.float32) / 255
    img = np.empty(a.shape + (3,), np.float32)
    img[:] = color
    blit(canvas, img, a, x, y)


# ----------------------------------------------------------------- cursor
ARROW = [(0, 0), (0, 17), (4.2, 13.2), (7.0, 19.6), (9.6, 18.5), (6.9, 12.2), (12.4, 12.2)]


def draw_cursor(im, x, y, scale=1.0, press=0.0):
    s = 1.9 * scale * (1 - 0.12 * press)
    d = ImageDraw.Draw(im, 'RGBA')
    pts = [(x + px * s, y + py * s) for px, py in ARROW]
    # shadow
    sh = Image.new('L', im.size, 0)
    ImageDraw.Draw(sh).polygon([(px + 1.5, py + 3) for px, py in pts], fill=110)
    sh = sh.crop((int(x) - 20, int(y) - 20, int(x) + 70, int(y) + 80)).filter(ImageFilter.GaussianBlur(3))
    im.paste((0, 0, 0), (int(x) - 20, int(y) - 20), sh)
    # white outline then black body
    d.polygon(pts, fill=(255, 255, 255, 255))
    # an inset black arrow (about a 1.5 px white border)
    cxs = sum(p[0] for p in ARROW) / len(ARROW)
    cys = sum(p[1] for p in ARROW) / len(ARROW)
    inner = [(x + (cxs + (px - cxs) * 0.78) * s - 0.05 * s, y + (cys + (py - cys) * 0.80) * s + 0.25 * s) for px, py in ARROW]
    d.polygon(inner, fill=(18, 18, 20, 255))


def draw_ripple(im, x, y, u):
    """Click ripple, u in [0, 1]."""
    if not (0 <= u <= 1):
        return
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    r = 10 + 46 * smooth(u)
    a = int(200 * (1 - u) ** 1.4)
    d.ellipse([x - r, y - r, x + r, y + r], outline=ACCENT + (a,), width=4)
    r2 = 10 + 22 * smooth(min(1, u * 1.6))
    a2 = int(70 * (1 - min(1, u * 1.6)))
    d.ellipse([x - r2, y - r2, x + r2, y + r2], fill=ACCENT + (a2,))
    im.alpha_composite(ov) if im.mode == 'RGBA' else im.paste(ov, (0, 0), ov)


# ----------------------------------------------------------------- captions
def draw_caption(im, text, u_in, u_out):
    """u_in: 0..1 fade/slide in progress; u_out: 0..1 fade out progress."""
    a = smooth(u_in) * (1 - smooth(u_out))
    if a <= 0.001:
        return
    text = text.replace('-', '\u2010')   # Inter's U+2010 hyphen spaces better without shaping
    f = font(FONT_SEMI, 60)
    d0 = ImageDraw.Draw(im)
    tw = d0.textlength(text, font=f)
    padx, h = 44, 104
    w = tw + 2 * padx
    cx = W / 2
    cy = H - 92 + (1 - smooth(u_in)) * 18
    x0, y0 = cx - w / 2, cy - h / 2
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    sh = Image.new('L', im.size, 0)
    ImageDraw.Draw(sh).rounded_rectangle([x0, y0 + 10, x0 + w, y0 + h + 10], h / 2, fill=int(90 * a))
    sh = sh.filter(ImageFilter.GaussianBlur(18))
    ov.paste((15, 23, 42, 255), (0, 0), sh)
    d = ImageDraw.Draw(ov)
    d.rounded_rectangle([x0, y0, x0 + w, y0 + h], h / 2, fill=(255, 255, 255, int(246 * a)),
                        outline=(15, 23, 42, int(28 * a)), width=2)
    d.text((cx, cy + 2), text, font=f, fill=(17, 24, 39, int(255 * a)), anchor='mm')
    im.alpha_composite(ov)


# ----------------------------------------------------------------- end card
ICON_IMG = Image.open(ICON).convert('RGBA')


def draw_endcard(im, u):
    """u: seconds since the end card began. A calm version of the old film's logo sting: a soft
    accent glow blooms behind the icon as it settles (0-1.2 s), then a sheen sweeps across the
    wordmark (1.5-2.3 s)."""
    def fade(t0, dur=0.5):
        return smooth((u - t0) / dur)
    a_icon = fade(0.0, 0.7)
    a_name = fade(0.35)
    a_tag = fade(0.8)
    a_free = fade(1.25)
    cy = H * 0.40
    icx, icy = W / 2, cy - 140
    # glow: a radial accent bloom that rises and settles to a faint halo
    g = smooth(u / 0.6) * (1 - 0.65 * smooth((u - 0.6) / 1.2))
    if g > 0.003:
        R = 330
        y, x = np.mgrid[-R:R, -R:R].astype(np.float32)
        r = np.sqrt(x * x + y * y) / R
        a = np.clip(1 - r, 0, 1) ** 2.2 * 0.28 * g
        glow = np.zeros((2 * R, 2 * R, 4), np.uint8)
        glow[..., :3] = ACCENT
        glow[..., 3] = (a * 255).astype(np.uint8)
        im.alpha_composite(Image.fromarray(glow, 'RGBA'), (int(icx - R), int(icy - R)))
    size = int(round(220 * (0.86 + 0.14 * smooth(u / 0.9))))
    if a_icon > 0:
        ic = ICON_IMG.resize((size, size), Image.LANCZOS)
        ic.putalpha(Image.fromarray((np.asarray(ic.split()[3]).astype(np.float32) * a_icon).astype(np.uint8)))
        im.alpha_composite(ic, (int(icx - size / 2), int(icy - size / 2)))
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    ny = cy + 20 + (1 - a_name) * 14
    d.text((W / 2, ny), 'NimbleMap', font=font(FONT_SEMI, 104), fill=(17, 24, 39, int(255 * a_name)), anchor='mm')
    d.text((W / 2, cy + 128 + (1 - a_tag) * 12), 'Explore real data about the planet.', font=font(FONT_REG, 50),
           fill=(55, 65, 81, int(255 * a_tag)), anchor='mm')
    f = font(FONT_SEMI, 36)
    txt = 'Free for Mac'
    tw = d.textlength(txt, font=f)
    py = cy + 246 + (1 - a_free) * 10
    d.rounded_rectangle([W / 2 - tw / 2 - 30, py - 34, W / 2 + tw / 2 + 30, py + 34], 34, fill=ACCENT + (int(255 * a_free),))
    d.text((W / 2, py + 1), txt, font=f, fill=(255, 255, 255, int(255 * a_free)), anchor='mm')
    d.text((W / 2, py + 92), 'kangning\u2010huang.github.io/NimbleMap', font=font(FONT_REG, 36),
           fill=(55, 65, 81, int(255 * a_free)), anchor='mm')
    im.alpha_composite(ov)
    # sheen across the wordmark
    sv = (u - 1.5) / 0.8
    if 0 < sv < 1:
        m = Image.new('L', im.size, 0)
        ImageDraw.Draw(m).text((W / 2, ny), 'NimbleMap', font=font(FONT_SEMI, 104), fill=255, anchor='mm')
        x0, x1 = int(W / 2 - 330), int(W / 2 + 330)
        y0, y1 = int(ny - 70), int(ny + 70)
        mm = np.asarray(m)[y0:y1, x0:x1].astype(np.float32) / 255
        yy, xx = np.mgrid[y0:y1, x0:x1].astype(np.float32)
        band_c = x0 - 120 + (x1 - x0 + 240) * smooth(sv)
        band = np.exp(-(((xx + (yy - ny) * 0.35) - band_c) / 38) ** 2)
        a = (mm * band * 0.75 * np.sin(np.pi * sv)) [..., None]
        sub = np.asarray(im)[y0:y1, x0:x1].astype(np.float32)
        out = sub.copy()
        out[..., :3] = sub[..., :3] * (1 - a) + np.array([140, 175, 255], np.float32) * a
        im.paste(Image.fromarray(out.astype(np.uint8), 'RGBA'), (x0, y0))


# ----------------------------------------------------------------- frame
def shot_at(t):
    for sh in edl.SHOTS:
        if sh['t0'] <= t < sh['t0'] + sh['dur']:
            return sh
    return None


def src_time(sh, u):
    """Source time for shot-relative time u: piecewise-linear 'map' [(u, src_t), ...]."""
    m = sh['map']
    if u <= m[0][0]:
        return m[0][1] + (u - m[0][0])
    for a, b in zip(m, m[1:]):
        if u <= b[0]:
            return a[1] + (b[1] - a[1]) * (u - a[0]) / (b[0] - a[0])
    return m[-1][1] + (u - m[-1][0])


def draw_file_chip(im, x, y, name, u_open, t):
    """A plain document icon with its file name on the desktop-coloured stage, double-clicked by
    the drawn cursor. It is the one element that stands in for the Finder (not app UI)."""
    if t > u_open + 0.45:
        return
    k = 1.0 - 0.35 * smooth((t - u_open) / 0.45)
    a = 1.0 - smooth((t - u_open) / 0.45)
    a *= smooth(t / 0.2)
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    bw, bh = 300 * k, 250 * k
    x0, y0 = x - bw / 2, y - bh / 2
    sh = Image.new('L', im.size, 0)
    ImageDraw.Draw(sh).rounded_rectangle([x0, y0 + 12, x0 + bw, y0 + bh + 12], 22 * k, fill=int(80 * a))
    sh = sh.filter(ImageFilter.GaussianBlur(20))
    ov.paste((15, 23, 42, 255), (0, 0), sh)
    d.rounded_rectangle([x0, y0, x0 + bw, y0 + bh], 22 * k, fill=(255, 255, 255, int(245 * a)))
    # document icon with a folded corner and a "CSV" band
    iw, ih = 92 * k, 116 * k
    ix, iy = x - iw / 2, y0 + 30 * k
    f = 26 * k
    d.polygon([(ix, iy), (ix + iw - f, iy), (ix + iw, iy + f), (ix + iw, iy + ih), (ix, iy + ih)],
              fill=(248, 250, 252, int(255 * a)), outline=(148, 163, 184, int(255 * a)))
    d.polygon([(ix + iw - f, iy), (ix + iw - f, iy + f), (ix + iw, iy + f)], fill=(203, 213, 225, int(255 * a)))
    d.rounded_rectangle([ix + 10 * k, iy + ih - 40 * k, ix + iw - 10 * k, iy + ih - 14 * k], 5 * k, fill=(22, 163, 74, int(255 * a)))
    d.text((x, iy + ih - 27 * k), 'CSV', font=font(FONT_SEMI, 17 * k), fill=(255, 255, 255, int(255 * a)), anchor='mm')
    d.text((x, y0 + bh - 46 * k), name, font=font(FONT_SEMI, 25 * k), fill=(17, 24, 39, int(255 * a)), anchor='mm')
    im.alpha_composite(ov)


def draw_askfield(frame, spec, u):
    """Type the question into the real Ask box: drawn into the source frame (content pixels), in
    the box's own font, size and colour, replacing the placeholder until the request is sent."""
    t0, t1, until = spec['t0'], spec['t1'], spec['until']
    if u < t0 or u >= until:
        return frame
    a = edl.ASKFIELD
    text = spec['text']
    n = int(round(len(text) * min(1.0, max(0.0, (u - t0) / (t1 - t0)))))
    im = Image.fromarray(frame.copy())
    d = ImageDraw.Draw(im)
    x0, y0, x1, y1 = a['clear']
    d.rectangle([x0, y0, x1, y1], fill=a['bg'])
    f = font(FONT_REG, a['size'])
    shown = text[:n]
    # long text scrolls left inside the field, as a text field does
    avail = 1640 - a['x']
    tw = d.textlength(shown, font=f)
    x = a['x'] - max(0.0, tw - avail)
    if tw > 0:
        # clip to the field's text area
        layer = Image.new('L', im.size, 0)
        ImageDraw.Draw(layer).text((x, a['baseline']), shown, font=f, fill=255, anchor='ls')
        clip = Image.new('L', im.size, 0)
        ImageDraw.Draw(clip).rectangle([a['x'] - 2, y0 - 10, 1650, y1 + 10], fill=255)
        layer = Image.fromarray(np.minimum(np.asarray(layer), np.asarray(clip)))
        im.paste(a['ink'], (0, 0), layer)
    # caret: steady while typing, blinking after
    if u < t1 or int((u - t1) * 2.2) % 2 == 0:
        cx = min(x + tw + 2, 1648)
        d.rectangle([cx, a['baseline'] - 24, cx + 2, a['baseline'] + 5], fill=a['ink'])
    return np.asarray(im)


def render_shot(sh, idx, t, fps):
    """The stage for one shot at timeline time t (may run past the shot's end for a dissolve)."""
    canvas = BG.copy()
    u = t - sh['t0']
    frame = source(sh['src'], fps, idx).get(src_time(sh, u))
    for ov in sh.get('overlays', []):
        if ov['type'] == 'askfield':
            frame = draw_askfield(frame, ov, u)
    aspect = frame.shape[1] / frame.shape[0]
    cx, cy, z = interp_keys(sh['cam'], u)
    cam = (cx, cy, min(z, edl.ZMAX))
    alpha = 1.0
    if 'fade_out' in sh:
        alpha = 1 - smooth((u - (sh['dur'] - sh['fade_out'])) / sh['fade_out'])
    if alpha > 0.002:
        draw_window(canvas, frame, cam, alpha)
    im = Image.fromarray(canvas).convert('RGBA')
    for ov in sh.get('overlays', []):
        if ov['type'] == 'file':
            x, y = to_out(cam, ov['p'], aspect)
            draw_file_chip(im, x, y, ov['name'], ov['open'], u)
    for c in sh.get('clicks', []):
        ux = u - c['t']
        if 0 <= ux <= 0.7:
            x, y = to_out(cam, c['p'], aspect)
            draw_ripple(im, x, y, ux / 0.7)
    if sh.get('cursor') and alpha > 0.5:
        px, py = interp_keys(sh['cursor'], u)
        press = 0.0
        for c in sh.get('clicks', []):
            ux = u - c['t']
            if -0.08 <= ux <= 0.16:
                press = max(press, 1 - abs(ux - 0.02) / 0.14)
        x, y = to_out(cam, (px, py), aspect)
        draw_cursor(im, x, y, 1.0, max(0, press))
    return im


def render(t, fps):
    shots = edl.SHOTS
    cur = None
    for i, sh in enumerate(shots):
        if sh['t0'] <= t < sh['t0'] + sh['dur']:
            cur = i
    if cur is None:
        im = Image.fromarray(BG).convert('RGBA')
    else:
        im = render_shot(shots[cur], cur, t, fps)
        u = t - shots[cur]['t0']
        prev = cur - 1
        if prev >= 0 and u < edl.XF and abs(shots[prev]['t0'] + shots[prev]['dur'] - shots[cur]['t0']) < 1e-6:
            a = smooth(u / edl.XF)
            im = Image.blend(render_shot(shots[prev], prev, t, fps), im, a)
    end_u = t - edl.END_T0
    if end_u >= 0:
        draw_endcard(im, end_u)
    for c in edl.CAPTIONS:
        t0, t1, text = c
        if t0 - 0.01 <= t <= t1 + 0.3:
            draw_caption(im, text, (t - t0) / 0.3, (t - t1) / 0.3)
    return np.asarray(im.convert('RGB'))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--fps', type=int, default=60)
    ap.add_argument('--from', dest='t0', type=float, default=0.0)
    ap.add_argument('--to', dest='t1', type=float, default=edl.DUR)
    ap.add_argument('--stills', default='')
    a = ap.parse_args()
    if a.stills:
        os.makedirs(a.out, exist_ok=True)
        for t in [float(x) for x in a.stills.split(',')]:
            Image.fromarray(render(t, a.fps)).save(os.path.join(a.out, f't{t:06.2f}.png'))
            print('still', t)
        return
    n0, n1 = int(round(a.t0 * a.fps)), int(round(a.t1 * a.fps))
    ff = subprocess.Popen(['ffmpeg', '-v', 'error', '-y', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', f'{W}x{H}',
                           '-r', str(a.fps), '-i', '-', '-c:v', 'ffv1', '-level', '3', '-pix_fmt', 'yuv444p', a.out],
                          stdin=subprocess.PIPE)
    for n in range(n0, n1):
        ff.stdin.write(render(n / a.fps, a.fps).tobytes())
        if n % 60 == 0:
            print(f'frame {n}/{n1}', flush=True)
    ff.stdin.close()
    ff.wait()


if __name__ == '__main__':
    main()
