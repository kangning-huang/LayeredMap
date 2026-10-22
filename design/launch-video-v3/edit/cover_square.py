"""Covers for the square v3.8-zh (WeChat Channels / RedNote): 1080x1080 and 3:4 (1080x1440).

    python edit/cover_square.py <out-dir>

Both use the film's own first shot (the storm world map, 4,999 tracks), rendered by the square
compositor without its overlays, with the title set in Noto Sans SC from the repo.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import square38 as Q  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image, ImageDraw, ImageFilter  # noqa: E402

C = Q.C
INK = (17, 24, 39)
SUB = (55, 62, 76)
ACCENT = C.ACCENT
TITLE = '凌波微图'
NAME = 'NimbleMap'
LINE = '网上下载的地图数据，拖进来就能看'
FORMATS = 'Shapefile · GeoPackage · NetCDF · GeoTIFF · CSV'
FREE = 'Mac 和 Windows 免费'
URL = 'nimblemap.com'


def map_still(t, cx, cy, s, size):
    """The film's frame at t, zoomed to (cx, cy, s) master px, no overlays, as a size x size image."""
    Q.Z['z'] = (cx, cy, s)
    Q.REC.clear()
    f = Q.K.render(t)
    return Image.fromarray(np.ascontiguousarray(f[:, Q.X0:Q.X0 + Q.SQ])).resize((size, size), Image.LANCZOS)


def pill(d, cx, cy, text, f, fill, ink, pad=36, h=78):
    w = d.textlength(text, font=f)
    d.rounded_rectangle([cx - w / 2 - pad, cy - h / 2, cx + w / 2 + pad, cy + h / 2], h / 2, fill=fill)
    d.text((cx, cy + 1), text, font=f, fill=ink, anchor='mm')


def text_block(im, top):
    d = ImageDraw.Draw(im)
    W = im.size[0]
    d.text((W / 2, top), TITLE, font=C.font(Q.CJK_SEMI, 150), fill=INK, anchor='mt')
    d.text((W / 2, top + 196), NAME, font=C.font(C.FONT_SEMI, 64), fill=SUB, anchor='mt')
    d.text((W / 2, top + 300), LINE, font=C.font(Q.CJK_SEMI, 54), fill=INK, anchor='mt')
    return d


def fade_band(im, solid, end, color):
    """`color` over the map: opaque from the top down to `solid`, fading out by `end`."""
    W, H = im.size
    a = np.zeros((H, 1), np.float32)
    a[:solid, 0] = 1.0
    u = np.linspace(0, 1, end - solid)
    a[solid:end, 0] = 1.0 * (1 - u * u * (3 - 2 * u))
    mask = Image.fromarray((np.repeat(a, W, 1) * 255).astype(np.uint8))
    im.paste(Image.new('RGB', im.size, color), (0, 0), mask)


def square_cover(out):
    im = map_still(1.0, 1060, 470, 1120, 1080).convert('RGB')
    fade_band(im, 500, 640, (247, 246, 243))
    d = text_block(im, 70)
    d.text((540, 455), FORMATS, font=C.font(C.FONT_REG, 32), fill=SUB, anchor='mt')
    f = C.font(Q.CJK_SEMI, 40)
    pill(d, 540, 980, FREE + '  ·  ' + URL, f, ACCENT, (255, 255, 255), h=84)
    im.save(out, quality=92)


def tall_cover(out):
    W, H = 1080, 1440
    im = Image.new('RGB', (W, H), (247, 246, 243))
    m = map_still(1.0, 1060, 500, 1120, 1080)
    im.paste(m, (0, 360))
    fade_band(im, 540, 700, (247, 246, 243))
    d = text_block(im, 90)
    d.text((W / 2, 480), FORMATS, font=C.font(C.FONT_REG, 34), fill=SUB, anchor='mt')
    f = C.font(Q.CJK_SEMI, 44)
    pill(d, W / 2, 1310, FREE + '  ·  ' + URL, f, ACCENT, (255, 255, 255), h=92)
    im.save(out, quality=92)


if __name__ == '__main__':
    o = sys.argv[1]
    os.makedirs(o, exist_ok=True)
    square_cover(os.path.join(o, 'NimbleMap-launch-v3.8-zh-square-cover.jpg'))
    tall_cover(os.path.join(o, 'NimbleMap-launch-v3.8-zh-cover-3x4.jpg'))
