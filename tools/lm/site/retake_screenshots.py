"""Image steps for retake_screenshots.sh (run with `mamba run -n lm-logo python -I`).

  shot RAW.png OUT.jpg [x0,y0,x1,y1]   optional crop of the raw window capture (raw pixels), then
                                       scale to 1600 px wide, JPEG quality 90 (4:4:4)
  cut SRC.jpg OUT.jpg x0,y0,x1,y1 [WxH] cut a box from a 1600-px screenshot, optionally scaled
"""
import sys

from PIL import Image

WIDTH = 1600
# Pillow quality 90 with full-resolution colour (4:4:4) keeps UI text crisp; it lands near the
# file sizes of the earlier `sips` quality-85 JPEGs.
QUALITY = 90


def box(s):
    return tuple(int(v) for v in s.split(","))


def save(im, out):
    im.convert("RGB").save(out, "JPEG", quality=QUALITY, subsampling=0, optimize=True)
    print(f"{out}: {im.width}x{im.height}")


def shot(raw, out, crop=None):
    im = Image.open(raw)
    if crop:
        im = im.crop(box(crop))
    h = round(im.height * WIDTH / im.width)
    save(im.resize((WIDTH, h), Image.LANCZOS), out)


def cut(src, out, b, size=None):
    im = Image.open(src).crop(box(b))
    if size:
        w, h = (int(v) for v in size.lower().split("x"))
        im = im.resize((w, h), Image.LANCZOS)
    save(im, out)


if __name__ == "__main__":
    cmd, *args = sys.argv[1:]
    {"shot": shot, "cut": cut}[cmd](*args)
