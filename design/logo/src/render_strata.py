"""Render strata-v* SVGs to PNG (1024/128/32/16) and a review contact sheet."""
import glob, os, sys
import cairosvg
from PIL import Image

D = "/Volumes/LaCie/repos/LayeredMap/design/logo"
SHEET = sys.argv[1] if len(sys.argv) > 1 else os.path.join(D, "png", "_strata-contact.png")
mains = sorted(p for p in glob.glob(f"{D}/strata-v*.svg") if not p.endswith(("-32.svg", "-16.svg")))
rows = []
for s in mains:
    base = os.path.basename(s)[:-4]
    ims = {}
    for z in (1024, 128, 32, 16):
        src = f"{D}/{base}-{z}.svg" if os.path.exists(f"{D}/{base}-{z}.svg") else s
        out = f"{D}/png/{base}-{z}.png"
        cairosvg.svg2png(url=src, write_to=out, output_width=z, output_height=z)
        ims[z] = Image.open(out).convert("RGBA")
    rows.append(ims)
W, H = 1720, 350
sheet = Image.new("RGBA", (W, H * len(rows)), "white")
for r, ims in enumerate(rows):
    for side, bg in enumerate([(246, 246, 246, 255), (30, 30, 32, 255)]):
        t = Image.new("RGBA", (W // 2, H), bg)
        t.alpha_composite(ims[1024].resize((320, 320), Image.LANCZOS), (10, 15))
        t.alpha_composite(ims[128], (340, 110))
        t.alpha_composite(ims[32].resize((128, 128), Image.NEAREST), (488, 110))
        t.alpha_composite(ims[16].resize((128, 128), Image.NEAREST), (636, 110))
        t.alpha_composite(ims[32], (784, 120)); t.alpha_composite(ims[16], (830, 128))
        sheet.alpha_composite(t, (side * W // 2, r * H))
sheet.convert("RGB").save(SHEET)
print("ok", len(rows))
