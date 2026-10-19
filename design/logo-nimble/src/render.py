"""Render concept SVGs to png/ (1024/128/64/32/16; 32 and 16 from the hand-simplified
files) plus dark/tinted 256 px and the wordmark lockups, and write review contact sheets to png/review/.

Usage:  mamba run -n lm-logo python src/render.py
"""
import glob, os
import cairosvg
from PIL import Image

D = "/Volumes/LaCie/repos/LayeredMap/design/logo-nimble"
os.makedirs(f"{D}/png/review", exist_ok=True)
mains = sorted(p for p in glob.glob(f"{D}/concept-*.svg") if not p.endswith(("-32.svg", "-16.svg")))


def render(src, out, z):
    cairosvg.svg2png(url=src, write_to=out, output_width=z, output_height=z)
    return Image.open(out).convert("RGBA")


rows = []
for s in mains:
    base = os.path.basename(s)[:-4]
    ims = {}
    for z in (1024, 128, 64, 32, 16):
        src = f"{D}/{base}-{z}.svg" if os.path.exists(f"{D}/{base}-{z}.svg") else s
        ims[z] = render(src, f"{D}/png/{base}-{z}.png", z)
    for v in ("dark", "tinted"):
        ims[v] = render(f"{D}/variants/{base}-{v}.svg", f"{D}/png/{base}-{v}-256.png", 256)
    rows.append((base, ims))

# review sheet: light half | dark half; each: 320 master, 128, 32 x4, 16 x8, true 32 and 16, then dark & tinted variants
W, H = 2 * 900 + 2 * 150, 350
sheet = Image.new("RGBA", (W, H * len(rows)), "white")
for r, (base, ims) in enumerate(rows):
    for side, bg in enumerate([(246, 246, 246, 255), (30, 30, 32, 255)]):
        t = Image.new("RGBA", (900, H), bg)
        t.alpha_composite(ims[1024].resize((320, 320), Image.LANCZOS), (10, 15))
        t.alpha_composite(ims[128], (340, 110))
        t.alpha_composite(ims[32].resize((128, 128), Image.NEAREST), (488, 110))
        t.alpha_composite(ims[16].resize((128, 128), Image.NEAREST), (636, 110))
        t.alpha_composite(ims[32], (790, 120)); t.alpha_composite(ims[16], (840, 128))
        sheet.alpha_composite(t, (side * 900, r * H))
    v = Image.new("RGBA", (300, H), (0, 0, 0, 255))
    v.alpha_composite(ims["dark"].resize((140, 140), Image.LANCZOS), (5, 105))
    v.alpha_composite(ims["tinted"].resize((140, 140), Image.LANCZOS), (155, 105))
    sheet.alpha_composite(v, (1800, r * H))
sheet.convert("RGB").save(f"{D}/png/review/contact.png")
for r, (base, _) in enumerate(rows):
    sheet.crop((0, r * H, W, (r + 1) * H)).convert("RGB").save(f"{D}/png/review/row-{base}.png")
# v1 of the revised concepts, and the website mark
os.makedirs(f"{D}/png/v1", exist_ok=True)
for p in sorted(glob.glob(f"{D}/v1/*-v1.svg")):
    base = os.path.basename(p)[:-4]
    for z in (1024, 128, 32, 16):
        src = f"{D}/v1/{base}-{z}.svg" if os.path.exists(f"{D}/v1/{base}-{z}.svg") else p
        render(src, f"{D}/png/v1/{base}-{z}.png", z)
for z in (30, 60, 90, 256):
    render(f"{D}/peek-site.svg", f"{D}/png/peek-site-{z}.png", z)
# wordmark lockups (outlined text, written by src/lockup.py)
for p in sorted(glob.glob(f"{D}/lockups/*-lockup-*.svg")):
    cairosvg.svg2png(url=p, write_to=f"{D}/png/{os.path.basename(p)[:-4]}.png")
print("ok", len(rows))
