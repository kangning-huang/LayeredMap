"""NimbleMap launch film v3.7: v3.6's cut with every map shot re-made as one continuous keyframed
camera, rendered offline frame by frame by the app itself (render/<shot>/frames, made by
path/render.sh with crates/lm-ui/examples/film_render.rs).

    python edit/compose37.py out/seg.mkv [--from 0 --to 78.83]
    python edit/compose37.py out/stills --stills 21.0,33.5,...

What changes from v3.6 (edit/compose36.py, whose timeline, captions, table and sound are kept):
- The storm, earthquake, heatwave and Pro windows show the renders: each map move is a smooth
  path (van Wijk & Nuij flights, eased log-zoom moves, a slow drift on every hold), every frame
  fully loaded, with motion blur on fast moves. The searches land directly on the regional view.
- The search bars, the Find field and the Ask box are typed by the app (real fields, real result
  lists); the compositor no longer draws typing into them.
- Ripples follow the place they mark through the moving camera.
- The table camera passes through its keys with continuous speed (monotone cubic Hermite in
  x, y, log zoom and roll) instead of easing to a stop at each one; its whips are kept.
"""
import argparse
import math
import os
import subprocess
import sys
from collections import OrderedDict

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(ROOT, 'path'))
import compose36 as S  # noqa: E402  (sets up v3.6's compositor)
import cv2  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image  # noqa: E402

import shots as SH  # noqa: E402
from camera import merc  # noqa: E402

N, E, Q, B, C, P = S.N, S.E, S.Q, S.B, S.C, S.P
OW, OH = C.OW, C.OH
ramp, ease = C.ramp, C.ease
FPS = 30
RENDER = os.path.join(ROOT, 'render')
TITLE = 64
# v3.7-zh (LM_FILM_LANG=zh): the same cut on the Chinese renders (render/<shot>_zh), with the
# overlays in Chinese (see the end of this file)
ZH = os.environ.get('LM_FILM_LANG') == 'zh'
SFX = '_zh' if ZH else ''


# ---------------------------------------------------------------- the renders
class Renders:
    """Frame n of a shot is render/<shot>/frames/NNNNN.png (time n/30 on the film's clock);
    outside a shot's range its first or last frame is held."""

    def __init__(self, cap=48):
        self.lru = OrderedDict()
        self.cap = cap
        self.range = {}
        for s in ('ty', 'eq', 'hw', 'pro', 'ty_plate', 'eq_plate'):
            d = os.path.join(RENDER, s + SFX, 'frames')
            ns = sorted(int(f[:5]) for f in os.listdir(d) if f.endswith('.png') and f[:5].isdigit()) if os.path.isdir(d) else []
            if ns:
                self.range[s] = (ns[0], ns[-1])

    def get(self, shot, t):
        a, b = self.range[shot]
        n = min(b, max(a, int(round(t * FPS))))
        k = (shot, n)
        if k in self.lru:
            self.lru.move_to_end(k)
            return self.lru[k]
        im = cv2.imread(os.path.join(RENDER, shot + SFX, 'frames', f'{n:05d}.png'), cv2.IMREAD_COLOR)[:, :, ::-1]
        im = np.ascontiguousarray(im)
        self.lru[k] = im
        if len(self.lru) > self.cap:
            self.lru.popitem(last=False)
        return im


R = Renders()
CUR = {'t': 0.0}          # the film's own time of the frame being drawn


def now():
    return CUR['t']


# the cameras, to place ripples on the places they mark
CHAINS = {k: f() for k, f in SH.CHAINS.items()}


def window_px(shot, lon, lat, t):
    """Where lon/lat is in a window image (2x pixels, title bar included) at time t."""
    v = SH.wrapped(CHAINS[shot].at(t))
    x, y = SH.screen_pt(v, lon, lat)
    return x * 2, y * 2 + TITLE


# Sandy's landfall, from v3.6's measured point in its regional view
_sv = SH.SANDY_REGION
SANDY_LL = None
if True:
    from camera import unmerc
    _mx, _my = _sv.to_map(E.SANDY_PX[0] / 2, (E.SANDY_PX[1] - TITLE) / 2 - SH.TOOLBAR)
    SANDY_LL = unmerc(_mx - 2 * math.pi * 6378137.0, _my)


# ---------------------------------------------------------------- the films (x: v3.3 time)
_plates = {}


def plate(name, take_shot, take_t, keep):
    """The empty map of a shot's first view (rendered with nothing open), with the take's own
    panels where it has them (the reveal draws the data in over it)."""
    k = (name, take_shot, take_t)
    if k not in _plates:
        take = R.get(take_shot, take_t)
        p = R.get(name, 15.0).copy()
        for x0, y0, x1, y1 in keep:
            y0c, y1c = y0 - TITLE, y1 - TITLE
            p[max(0, y0c):y1c, x0:x1] = take[max(0, y0c):y1c, x0:x1]
        _plates[k] = p
    return _plates[k]


T_TY_FULL = 15.9          # the storm film's first view, fully drawn (before its drift starts)
T_EQ_FULL = 31.35


def film1(x):
    A = R.get('ty', T_TY_FULL)
    pl = plate('ty_plate', 'ty', T_TY_FULL, E.TY_KEEP)
    if x < E.T_REVEAL1[0]:
        return pl
    if x < E.T_REVEAL1[1]:
        return C.reveal(pl, A, C.ty_threshold(*A.shape[:2]), C.ease_in(ramp(x, *E.T_REVEAL1)) ** 0.6, 0.05)
    return R.get('ty', now())


def film1_loop(p):
    both, last = R.get('ty', 26.25), R.get('ty', 28.6)
    if p < 0.4:
        return both
    if p < 0.5:
        return C.xfade(both, last, (p - 0.4) / 0.1)
    return last


def film2(x):
    Ef = R.get('eq', T_EQ_FULL)
    pl = plate('eq_plate', 'eq', T_EQ_FULL, E.EQ_KEEP)
    if x < E.T_RING[0]:
        return pl
    if x < E.T_RING[1]:
        return C.reveal(pl, Ef, B.eq_thr(*Ef.shape[:2]), ramp(x, E.T_RING[0], E.T_RING[1] - 0.05), 0.015)
    return R.get('eq', now())


def film2_loop(p):
    Ef = R.get('eq', T_EQ_FULL)
    pl = plate('eq_plate', 'eq', T_EQ_FULL, E.EQ_KEEP)
    card = R.get('eq', 40.3)
    if p < 0.5:
        return C.reveal(pl, Ef, B.eq_thr(*Ef.shape[:2]), p / 0.5, 0.015)
    if p < 0.6:
        return C.xfade(Ef, card, (p - 0.5) / 0.1)
    if p > 0.95:
        return C.xfade(card, pl, (p - 0.95) / 0.05)
    return card


_step_t = {}
for _n in range(int(round(39.9 * FPS)), int(round(N.T_ENTER3 * FPS))):
    _step_t.setdefault(SH.hw_index(_n / FPS), _n / FPS)


def film3(x):
    return R.get('hw', now())


def film3_loop(p):
    card = R.get('hw', 50.4)
    if p < 0.6:
        k = int(N.HW_LAST * p / 0.6 + 1e-6)
        return R.get('hw', _step_t.get(k, max(_step_t.values())))
    if p < 0.7:
        return C.xfade(R.get('hw', _step_t[N.HW_LAST]), card, (p - 0.6) / 0.1)
    return card


def film4(x):
    return R.get('pro', 49.9) if x < 50.0 else R.get('pro', 69.6)


Q.film1, Q.film1_loop, Q.film2, Q.film2_loop, Q.film4 = film1, film1_loop, film2, film2_loop, film4
B.film3, B.film3_loop = film3, film3_loop

# single frames v3.3's render reads (the lifted card rows) -> the renders
_get36 = S.get
LIFTS = {('g_eqL', 40.0): ('eq', 35.40), ('g_hw', 168.0): ('hw', 47.00)}


def get(src, t):
    if (src, t) in LIFTS:
        return R.get(*LIFTS[(src, t)])
    return _get36(src, t)


Q.get = get

# the app types into its own fields now
C.draw_query = lambda img, *a, **k: img


# ---------------------------------------------------------------- the table camera, C1
def _pchip_tangents(ts, vs, ok):
    """Fritsch-Butland tangents; 0 at run ends (ok[i] False: key i joins a non-eased segment)."""
    n = len(ts)
    m = [0.0] * n
    for i in range(1, n - 1):
        if not ok[i]:
            continue
        h0, h1 = ts[i] - ts[i - 1], ts[i + 1] - ts[i]
        if h0 <= 1e-9 or h1 <= 1e-9:
            continue
        d0, d1 = (vs[i] - vs[i - 1]) / h0, (vs[i + 1] - vs[i]) / h1
        if d0 * d1 <= 0:
            continue
        w1, w2 = 2 * h1 + h0, h1 + 2 * h0
        m[i] = (w1 + w2) / (w1 / d0 + w2 / d1)
    return m


_cam_cache = {}


def _prep(keys):
    ts = [k[0] for k in keys]
    eased = [k[5] is ease for k in keys]            # segment ending at key i eases (k[i][5])
    # key i is interior to an eased run when the segments on both sides ease
    ok = [False] + [eased[i] and eased[i + 1] for i in range(1, len(keys) - 1)] + [False]
    comps = []
    for j, f in ((1, lambda v: v), (2, lambda v: v), (3, math.log), (4, lambda v: v)):
        vs = [f(k[j]) for k in keys]
        comps.append((vs, _pchip_tangents(ts, vs, ok)))
    return ts, comps


def camera(t):
    keys = B.CAM
    kid = id(keys)
    if kid not in _cam_cache:
        _cam_cache[kid] = _prep(keys)
    ts, comps = _cam_cache[kid]
    if t <= ts[0]:
        i, u = 0, 0.0
        out = [c[0][0] for c in comps]
    elif t >= ts[-1]:
        out = [c[0][-1] for c in comps]
    else:
        i = max(j for j in range(len(ts) - 1) if ts[j] <= t)
        h = max(1e-9, ts[i + 1] - ts[i])
        u = (t - ts[i]) / h
        fn = keys[i + 1][5]
        if fn is ease:
            h00, h10, h01, h11 = 2 * u ** 3 - 3 * u ** 2 + 1, u ** 3 - 2 * u ** 2 + u, -2 * u ** 3 + 3 * u ** 2, u ** 3 - u ** 2
            out = [h00 * vs[i] + h10 * h * m[i] + h01 * vs[i + 1] + h11 * h * m[i + 1] for vs, m in comps]
        else:
            e = fn(u)
            out = [vs[i] + (vs[i + 1] - vs[i]) * e for vs, m in comps]
    cx, cy, lz, rot = out
    z = math.exp(lz) * B.X1
    hx, hy, hr = B.handheld(t)
    return cx + hx / z, cy + hy / z, z, rot + hr


B.camera = camera          # compose33.camera sets B.CAM and B.handheld, then calls this


# ---------------------------------------------------------------- the native Pro section
def render_pro(t):
    B.CAM = S.PCAM
    cam = B.camera(t)
    canvas = C.table(cam)
    img = C.window_image(R.get('pro', t), False)
    M = C.local_to_screen(cam, Q.TL4, 1.0)
    C.draw_window(canvas, img, M, shadow=1.0)
    im = Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8)).convert('RGBA')
    C.caption(im, 'Pro: ask in plain words', t, S.P0 - 2.0, S.P0 + 9.0, pos='tr')
    return np.asarray(im.convert('RGB'))


S.render_pro = render_pro

# ripples: start as the camera lands, on the place itself
E.T_SANDY = N.t33(SH.T_FIND_ROW + 1.20) - 0.25
E.T_JAPAN = N.t33(SH.T_ROW + 0.05) - 0.05
E.T_PALERMO = N.t33(SH.T_CELL + 0.05) - 0.1


def render(t):
    CUR['t'] = t
    E.SANDY_PX = window_px('ty', SANDY_LL[0], SANDY_LL[1], t)
    E.TOHOKU_PX = window_px('eq', *SH.TOHOKU, t)
    E.PALERMO_PX = window_px('hw', *SH.BARCELONA, t)
    return S.render(t)



# ---------------------------------------------------------------- v3.7-zh: the overlays in Chinese
if ZH:
    from PIL import ImageDraw, ImageFilter

    CJK_REG = os.path.join(C.REPO, 'assets/fonts/NotoSansSC-Regular.otf')
    CJK_SEMI = os.path.join(C.REPO, 'assets/fonts/NotoSansSC-Medium.otf')
    # v3.3's overlay strings (as compose33 passes them) -> the Chinese, matching the narration
    ZH_TEXT = {
        '4,999 storm tracks': '4,999 条气旋路径',
        '14,537 earthquakes since 1900': '1900 年以来 14,537 次地震',
        'M 9.1.': '9.1 级。',
        'Tōhoku, 2011 · higher than 99% of 14,537 earthquakes': '2011 年东日本大地震 · 比 14,537 次地震中的 99% 都强',
        'Heatwaves, summer 2023 · 92 daily steps': '2022 年夏季热浪 · 92 天逐日数据',
        '36.50 °C': '36.10 °C',
        'heat-wave intensity, Palermo, 25 Jul 2023 · higher than 93% of cells':
            '巴塞罗那，2022 年 8 月 16 日 · 热浪强度高于 98% 的格点',
        'Pro: ask in plain words': 'Pro：用大白话提问',
    }
    # (compose36 has already turned v3.3's Palermo/2023 strings into these)
    ZH_TEXT.update({S.TEXT[k]: ZH_TEXT[k] for k in ('36.50 °C', 'Heatwaves, summer 2023 · 92 daily steps',
                                                    'heat-wave intensity, Palermo, 25 Jul 2023 · higher than 93% of cells')
                    if k in S.TEXT})
    ZH_TEXT['heat-wave intensity, Barcelona, 16 Aug 2022 · higher than 98% of cells'] = \
        ZH_TEXT['heat-wave intensity, Palermo, 25 Jul 2023 · higher than 93% of cells']

    def _cjk(fn):
        """Draw with Noto Sans SC (the repo's CJK font) in place of Inter, for Chinese text only;
        the file-type strip and anything else in Latin keeps Inter."""
        def wrapped(im, text, *a, **k):
            zh = ZH_TEXT.get(text)
            if zh is None:
                return fn(im, text, *a, **k)
            reg, semi = C.FONT_REG, C.FONT_SEMI
            C.FONT_REG, C.FONT_SEMI = CJK_REG, CJK_SEMI
            try:
                return fn(im, zh, *a, **k)
            finally:
                C.FONT_REG, C.FONT_SEMI = reg, semi
        return wrapped

    # compose36's caption/big_words map v3.3's strings to the 2022 data first; the Chinese is
    # keyed on v3.3's strings, so it wraps the originals underneath them
    S._cap, S._big = _cjk(S._cap), _cjk(S._big)

    _titlebar = C.titlebar

    def titlebar(width, dark):
        """The window frames' title: the app's Chinese name."""
        key = ('zh', width, dark)
        if key not in C.TITLES:
            bar = np.array(_titlebar(width, dark))
            h = C.edl.TITLE
            # clear the centred English title and write the Chinese name there
            row = bar[:, width // 2 - 260:width // 2 + 260]
            row[4:h - 4] = bar[4:h - 4, width // 2 - 262:width // 2 - 261]
            im = Image.fromarray(bar)
            ImageDraw.Draw(im).text((width / 2, h / 2), '凌波微图', font=C.font(CJK_SEMI, 27),
                                    fill=(220, 220, 226) if dark else (60, 60, 67), anchor='mm')
            C.TITLES[key] = np.asarray(im)
        return C.TITLES[key]

    C.titlebar = titlebar

    END_NAME = '凌波微图 NimbleMap'
    END_HINT = '搜索地名或你的数据'          # the app's own Chinese placeholder (zh.tsv)

    def end_bar(im, cam, t):
        """v3's end pill (compose.end_bar), in Chinese: the app's placeholder, then the block
        cursor writes 凌波微图 NimbleMap."""
        edl = C.edl
        ease_out, ramp, ease, blink = C.ease_out, C.ramp, C.ease, C.blink
        a = C.smooth(ramp(t, 24.4, 25.3))
        if a <= 0:
            return
        M = C.cam_matrix(cam)
        k = cam[2] / C.ZE
        cx, cy = C.apply(M, C.CE[0], C.CE[1] + 20 / C.ZE)
        w, h = 860 * k, 108 * k
        cy += (1 - ease_out(ramp(t, 24.4, 25.3))) * 30
        ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
        sh = Image.new('L', im.size, 0)
        ImageDraw.Draw(sh).rounded_rectangle([cx - w / 2, cy - h / 2 + 12 * k, cx + w / 2, cy + h / 2 + 12 * k], h / 2,
                                             fill=int(70 * a))
        sh = sh.filter(ImageFilter.GaussianBlur(22 * k))
        ov.paste((20, 22, 30, 255), (0, 0), sh)
        d = ImageDraw.Draw(ov)
        d.rounded_rectangle([cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2], h / 2, fill=(255, 255, 255, int(255 * a)))
        mx, my, r = cx - w / 2 + 62 * k, cy - 4 * k, 17 * k
        grey = (110, 116, 126, int(255 * a))
        d.ellipse([mx - r, my - r, mx + r, my + r], outline=grey, width=max(2, int(4.5 * k)))
        d.line([mx + r * 0.72, my + r * 0.72, mx + r * 1.55, my + r * 1.55], fill=grey, width=max(2, int(5 * k)))
        f = C.font(CJK_SEMI, 58 * k)
        tx = cx - w / 2 + 112 * k
        base = cy + 21 * k
        tw = d.textlength(END_NAME, font=f)
        pen = ease(ramp(t, edl.T_PEN[0], edl.T_PEN[1]))
        frozen = edl.T_STOP <= t < edl.T_PEN[0]
        if t < edl.T_PEN[0]:
            d.text((tx + 44 * k, base), END_HINT, font=C.font(CJK_REG, 46 * k), fill=(120, 126, 136, int(255 * a)),
                   anchor='ls')
            cur_x = tx
            on = 1.0 if frozen else blink(t, 25.3)
        else:
            layer = Image.new('L', im.size, 0)
            ImageDraw.Draw(layer).text((tx, base), END_NAME, font=f, fill=255, anchor='ls')
            clip = np.zeros((OH, OW), np.float32)
            xe = tx + tw * pen
            x0i, x1i = int(max(0, tx - 4)), int(min(OW, xe))
            if x1i > x0i:
                clip[:, x0i:x1i] = 1
                e0 = int(max(x0i, xe - 10 * k))
                clip[:, e0:x1i] = np.linspace(1, 0.2, x1i - e0)[None, :]
            al = (np.asarray(layer).astype(np.float32) * clip * a).astype(np.uint8)
            ink = Image.new('RGBA', im.size, C.INK + (255,))
            ink.putalpha(Image.fromarray(al))
            ov.alpha_composite(ink)
            cur_x = xe + 6 * k
            on = 1.0 if pen < 1 else blink(t, edl.T_PEN[1] + 0.2)
        if on > 0:
            d.rectangle([cur_x, base - 46 * k, cur_x + 30 * k, base + 10 * k], fill=C.INK + (int(235 * a * on),))
        im.alpha_composite(ov)

    C.end_bar = end_bar

    def end_card(im, t):
        edl = C.edl

        def fade(t0, dur=0.45):
            return C.smooth((t - t0) / dur)
        a1, a2, a3 = fade(edl.T_END), fade(edl.T_END + 0.35), fade(edl.T_END + 0.7)
        if a1 <= 0:
            return
        ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
        d = ImageDraw.Draw(ov)
        d.text((OW / 2, 735 + (1 - a1) * 12), '探索地球的真实数据', font=C.font(CJK_REG, 52),
               fill=(40, 44, 54, int(255 * a1)), anchor='mm')
        f = C.font(CJK_SEMI, 38)
        txt = 'Mac 版免费'
        tw = d.textlength(txt, font=f)
        py = 836 + (1 - a2) * 10
        d.rounded_rectangle([OW / 2 - tw / 2 - 32, py - 35, OW / 2 + tw / 2 + 32, py + 35], 35,
                            fill=C.ACCENT + (int(255 * a2),))
        d.text((OW / 2, py + 1), txt, font=f, fill=(255, 255, 255, int(255 * a2)), anchor='mm')
        d.text((OW / 2, 940 + (1 - a3) * 8), 'nimblemap.com/zh', font=C.font(C.FONT_REG, 38),
               fill=(55, 60, 72, int(255 * a3)), anchor='mm')
        im.alpha_composite(ov)

    C.end_card = end_card

    # The Chinese answer frames itself elsewhere (shots.PRO_ANSWER_ZH): the table camera's push
    # onto the 100 km circle follows the circle there (window pixels at the push, P0 + 12.0).
    _cx, _cy = window_px('pro_zh', *SH.TOKYO, S.P0 + 12.0)
    _circ = (Q.TL4[0] + _cx, Q.TL4[1] + _cy)
    S.PCAM = [(k[0], _circ[0] + (k[1] - S.CIRC[0]), _circ[1], *k[3:]) if k[2] == S.CIRC[1] and k[1] - S.CIRC[0] in (0, 20)
              else k for k in S.PCAM]

# v3.7.1-zh (LM_FILM_TRIM=1): v3.7-zh shortened by retiming its folder section (edit/edl371zh.py)
TRIM = os.environ.get('LM_FILM_TRIM') == '1'
if TRIM:
    import edl371zh as TR
    _render_full = render

    def render(t):  # noqa: F811
        return _render_full(TR.old_time(t))


# v3.8 (LM_FILM_V38=1, en or zh): the fast opening (edit/edl38.py): a cold open on the storm map,
# a dissolve into one quick folder beat, then v3.7 from 9.9 s on, SHIFT earlier
V38 = os.environ.get('LM_FILM_V38') == '1'
if V38:
    import edl38 as V8
    _render_v37 = render

    def render(t):  # noqa: F811
        w = V8.mix(t)
        if w <= 0:
            return _render_v37(V8.cold_time(t))
        if w >= 1:
            return _render_v37(V8.old_time(t))
        a = _render_v37(V8.cold_time(t)).astype(np.float32)
        b = _render_v37(V8.folder_time(t)).astype(np.float32)
        return np.clip(a + (b - a) * w + 0.5, 0, 255).astype(np.uint8)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--fps', type=int, default=N.FPS)
    ap.add_argument('--from', dest='t0', type=float, default=0.0)
    ap.add_argument('--to', dest='t1', type=float, default=V8.DUR if V38 else TR.DUR if TRIM else N.DUR)
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
