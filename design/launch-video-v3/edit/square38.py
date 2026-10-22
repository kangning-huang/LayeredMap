"""NimbleMap launch film v3.8-zh, square (1080x1080) re-edit for WeChat Channels / RedNote.

    python edit/square38.py out/sq/seg.mkv [--from 0 --to 72.53]
    python edit/square38.py out/sq/stills --stills 1.0,4.6,...   [--guides]

Not a crop of the 1080p master: every frame is re-rendered by v3.8-zh's own compositor
(compose37, LM_FILM_V38=1, LM_FILM_LANG=zh) with one extra screen-space zoom applied after the
table camera. The zoom picks, per moment, the square of the 16:9 frame that carries the action
(KEYS below: centre and side in 16:9 master pixels), so the app windows are drawn from their 2x
renders straight at the square's scale: UI text stays sharp when the square pushes in, and when
the square needs more than the 16:9 frame holds (the gathered films at the end) it simply pulls
out over the table.

The 16:9 film's screen-space overlays (caption pills, big numbers, end card) are not drawn at
their 16:9 positions: their calls are recorded and re-laid out for the square (top band for the
pills, larger type, a square end card). Subtitles are burned in later (path/subs.py ... zh-square).
Same clock as v3.8-zh: picture, narration and music line up frame for frame.
"""
import argparse
import math
import os
import subprocess
import sys

os.environ['LM_FILM_V38'] = '1'
os.environ['LM_FILM_LANG'] = 'zh'
os.environ['LM_CUT'] = '3.6'
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import compose37 as K  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image, ImageDraw, ImageFilter  # noqa: E402
from scipy.interpolate import PchipInterpolator  # noqa: E402

C, S = K.C, K.S
OW, OH = C.OW, C.OH            # the compositor still draws 1920x1080; the square is its middle
SQ = 1080
X0 = (OW - SQ) // 2
FPS = 30
DUR = K.V8.DUR
CJK_REG, CJK_SEMI = K.CJK_REG, K.CJK_SEMI

# ---------------------------------------------------------------- the square's path
# (t in v3.8 s, cx, cy, side): the square shows the 16:9 master's region centred on (cx, cy) with
# that side (master px); side < 1080 pushes in (drawn from the 2x renders), > 1080 pulls out.
# The action is kept above y ~ 800 of the square (the subtitles and the apps' overlays below).
KEYS = [
    # cold open: the storm world map, 4,999 tracks (a map from frame 0)
    (0.00, 1060, 600, 1250),
    (1.78, 1060, 590, 1180),
    # the folder: names and dates (the size column is cut)
    (2.00, 820, 640, 1250),
    (3.50, 820, 620, 1200),
    # the .tif row flies; the GeoTIFF window opens, then the CSV under it
    (4.00, 760, 520, 1300),
    (4.70, 960, 370, 760),
    (6.60, 950, 380, 780),
    (7.30, 930, 590, 1120),
    # the shapefile opens into the storm film
    (7.90, 960, 560, 1150),
    (8.60, 1060, 600, 1250),
    (11.60, 1050, 590, 1200),
    (12.40, 900, 580, 1080),
    # search: the bar, 纽约, the list, the landing
    (13.00, 960, 420, 1080),
    (13.50, 1000, 730, 1080),
    (14.40, 1000, 700, 1080),
    (14.80, 1000, 600, 1080),
    (16.20, 1000, 580, 1080),
    # the layer panel, the Find field, Sandy x2
    (16.80, 1250, 500, 1150),
    (17.05, 1130, 580, 1000),
    (17.30, 980, 620, 1000),
    (18.25, 940, 660, 980),
    (18.55, 1360, 500, 900),
    (19.00, 1200, 560, 1300),
    (20.50, 1340, 560, 1150),
    # Sandy 2012's card
    (21.00, 1356, 620, 950),
    (21.25, 1150, 620, 920),
    (21.50, 960, 620, 900),
    (21.60, 960, 620, 900),
    # the GeoPackage arrives; the Ring of Fire
    (22.10, 1000, 560, 1400),
    (22.60, 1050, 600, 1080),
    (25.00, 1050, 600, 1080),
    # 东京: bar, list, flight, card
    (25.50, 1000, 640, 1080),
    (26.20, 1000, 720, 1080),
    (26.60, 1000, 600, 1080),
    (27.60, 1000, 560, 1080),
    (28.00, 1400, 520, 1040),
    (28.50, 1460, 420, 920),
    (29.20, 960, 600, 1060),
    # 9.1 级
    (30.00, 960, 560, 1080),
    (32.40, 960, 560, 1080),
    (32.60, 1450, 420, 1000),
    (33.00, 970, 520, 1300),
    # heatwaves: the slider plays
    (33.60, 1150, 640, 1080),
    (35.80, 1150, 640, 1080),
    (36.40, 1000, 660, 1000),
    (37.00, 1140, 716, 1000),
    # 巴塞罗那: bar, list, the region, the cell's card
    (37.40, 1000, 730, 1080),
    (38.20, 1000, 640, 1080),
    (38.60, 1200, 640, 1150),
    (39.50, 1250, 560, 1150),
    (40.00, 1230, 560, 1350),
    (40.50, 1000, 560, 1000),
    # 36.10 °C
    (41.00, 960, 560, 1080),
    (43.60, 960, 560, 1080),
    # Pro: the Ask box, the question typed
    (44.00, 1050, 540, 1080),
    (45.00, 1000, 760, 1080),
    (48.20, 1000, 800, 1080),
    (48.80, 780, 700, 900),
    (51.80, 800, 700, 900),
    # the answer: 7 lines, the 100 km circle around Tokyo
    (52.40, 1100, 420, 1250),
    (53.00, 1000, 560, 1300),
    (55.50, 1080, 680, 1200),
    (56.00, 1450, 660, 1150),
    (57.80, 1480, 660, 1150),
    (58.40, 930, 620, 1350),
    (61.60, 930, 600, 1350),
    (62.20, 930, 560, 1350),
    # the films gather over the bar; the name is written; the end card
    (63.00, 960, 500, 1500),
    (64.00, 960, 420, 1750),
    (66.00, 960, 450, 1700),
    (67.00, 960, 700, 1650),
    (68.00, 960, 759, 1650),
    (DUR + 0.1, 960, 759, 1650),
]

_kt = np.array([k[0] for k in KEYS])
_kx = PchipInterpolator(_kt, [k[1] for k in KEYS])
_ky = PchipInterpolator(_kt, [k[2] for k in KEYS])
_ks = PchipInterpolator(_kt, [math.log(k[3]) for k in KEYS])


def zoom(t):
    t = min(max(t, _kt[0]), _kt[-1])
    return float(_kx(t)), float(_ky(t)), math.exp(float(_ks(t)))


Z = {'z': (960.0, 540.0, 1080.0)}
_cam_matrix = C.cam_matrix


def cam_matrix(cam):
    """The table camera, then the square's zoom (master (cx, cy) -> the frame's centre)."""
    M = _cam_matrix(cam)
    cx, cy, s = Z['z']
    k = SQ / s
    A = np.array([[k, 0, OW / 2 - k * cx], [0, k, OH / 2 - k * cy]], np.float64)
    return C.compose_affine(A, M)


C.cam_matrix = cam_matrix

# ---------------------------------------------------------------- the overlays, recorded
REC = []


def _zh(text):
    return K.ZH_TEXT.get(text, text)


def rec_caption(im, text, t, t0, t1, pos='bl'):
    REC.append(('cap', _zh(text), t, t0, t1, pos))


def rec_big(im, text, t, t0, t1, xy, size=210, light=False, anchor='lm'):
    REC.append(('big', _zh(text), t, t0, t1, size, light))


def rec_end(im, t):
    REC.append(('end', t))


S._cap, S._big = rec_caption, rec_big
C.end_card = rec_end


def font(path, size):
    return C.font(path, size)


def fit_font(d, text, path, size, width):
    while size > 20 and d.textlength(text, font=font(path, size)) > width:
        size -= 1
    return font(path, size)


def sq_caption(im, text, t, t0, t1, pos):
    a = C.smooth(C.ramp(t, t0, t0 + 0.25)) * (1 - C.smooth(C.ramp(t, t1 - 0.25, t1)))
    if a <= 0.01:
        return
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    f = fit_font(d, text, CJK_SEMI, 44, SQ - 2 * 56 - 72)
    tw = d.textlength(text, font=f)
    h = 82
    w = tw + 72
    x0 = 56          # all pills top left in the square (the heat and the Ask live on the right)
    y0 = 100 + (1 - C.smooth(C.ramp(t, t0, t0 + 0.25))) * -14
    sh = Image.new('L', im.size, 0)
    ImageDraw.Draw(sh).rounded_rectangle([x0, y0 + 8, x0 + w, y0 + h + 8], h / 2, fill=int(80 * a))
    sh = sh.filter(ImageFilter.GaussianBlur(14))
    ov.paste((15, 23, 42, 255), (0, 0), sh)
    d.rounded_rectangle([x0, y0, x0 + w, y0 + h], h / 2, fill=(255, 255, 255, int(244 * a)))
    d.text((x0 + 36, y0 + h / 2 + 1), text, font=f, fill=C.INK + (int(255 * a),), anchor='lm')
    im.alpha_composite(ov)


def sq_big(im, text, t, t0, t1, size, light):
    """The big number (size >= 200) high on the left; its line (size 46) split at ' · ' below it."""
    a = C.smooth(C.ramp(t, t0, t0 + 0.18)) * (1 - C.smooth(C.ramp(t, t1 - 0.2, t1)))
    if a <= 0.01:
        return
    big = size >= 100
    s = 0.92 + 0.08 * C.ease_out(C.ramp(t, t0, t0 + 0.35))
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    sh = Image.new('L', im.size, 0)
    dsh, d = ImageDraw.Draw(sh), ImageDraw.Draw(ov)
    ink = ((255, 255, 255) if light else C.INK)
    if big:
        f = font(CJK_SEMI, 230 * s)
        items = [((72, 330), text, f)]
    else:
        f = font(CJK_SEMI, 54)
        items = [((80, 500 + i * 80), part, f) for i, part in enumerate(text.split(' · '))]
    for xy, txt, ff in items:
        dsh.text((xy[0], xy[1] + 6), txt, font=ff, fill=int(150 * a), anchor='lm')
    sh = sh.filter(ImageFilter.GaussianBlur(16))
    ov.paste((0, 0, 0, 255) if light else (255, 255, 255, 255), (0, 0), sh)
    for xy, txt, ff in items:
        d.text(xy, txt, font=ff, fill=ink + (int(255 * a),), anchor='lm')
    im.alpha_composite(ov)


END_TAG = '探索地球的真实数据'
END_FREE = 'Mac 和 Windows 免费'
END_URL = 'nimblemap.com'


def sq_end(im, t):
    edl = C.edl

    def fade(t0, dur=0.45):
        return C.smooth((t - t0) / dur)
    a1, a2, a3 = fade(edl.T_END), fade(edl.T_END + 0.35), fade(edl.T_END + 0.7)
    if a1 <= 0:
        return
    ov = Image.new('RGBA', im.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    d.text((SQ / 2, 505 + (1 - a1) * 12), END_TAG, font=font(CJK_REG, 56), fill=(40, 44, 54, int(255 * a1)), anchor='mm')
    f = font(CJK_SEMI, 44)
    tw = d.textlength(END_FREE, font=f)
    py = 592 + (1 - a2) * 10
    d.rounded_rectangle([SQ / 2 - tw / 2 - 38, py - 40, SQ / 2 + tw / 2 + 38, py + 40], 40, fill=C.ACCENT + (int(255 * a2),))
    d.text((SQ / 2, py + 1), END_FREE, font=f, fill=(255, 255, 255, int(255 * a2)), anchor='mm')
    d.text((SQ / 2, 676 + (1 - a3) * 8), END_URL, font=font(C.FONT_SEMI, 50), fill=(45, 50, 62, int(255 * a3)), anchor='mm')
    im.alpha_composite(ov)


def guides(im):
    """Review only: the top 8% / bottom 12% bands the platforms cover."""
    d = ImageDraw.Draw(im)
    d.rectangle([0, 0, SQ, int(SQ * 0.08)], outline=(255, 0, 0, 255), width=2)
    d.rectangle([0, int(SQ * 0.88), SQ, SQ], outline=(255, 0, 0, 255), width=2)


def render(t, show_guides=False):
    Z['z'] = zoom(t)
    REC.clear()
    frame = K.render(t)
    im = Image.fromarray(np.ascontiguousarray(frame[:, X0:X0 + SQ])).convert('RGBA')
    for r in REC:
        if r[0] == 'cap':
            sq_caption(im, *r[1:])
        elif r[0] == 'big':
            sq_big(im, *r[1:])
        elif r[0] == 'end':
            sq_end(im, r[1])
    if show_guides:
        guides(im)
    return np.asarray(im.convert('RGB'))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--fps', type=int, default=FPS)
    ap.add_argument('--from', dest='t0', type=float, default=0.0)
    ap.add_argument('--to', dest='t1', type=float, default=DUR)
    ap.add_argument('--stills', default='')
    ap.add_argument('--guides', action='store_true')
    a = ap.parse_args()
    if a.stills:
        os.makedirs(a.out, exist_ok=True)
        for t in [float(v) for v in a.stills.split(',')]:
            Image.fromarray(render(t, a.guides)).save(os.path.join(a.out, f't{t:06.2f}.png'))
            print('still', t, flush=True)
        return
    n0, n1 = int(round(a.t0 * a.fps)), int(round(a.t1 * a.fps))
    ff = subprocess.Popen(['ffmpeg', '-v', 'error', '-y', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', f'{SQ}x{SQ}',
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
