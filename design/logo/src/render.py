import glob, io, os, sys
import cairosvg
from PIL import Image

D = "/Volumes/LaCie/repos/LayeredMap/design/logo"
SCR = os.path.dirname(os.path.abspath(__file__))
svgs = sorted(glob.glob(f"{D}/concept-*.svg"))
sizes = [1024, 128, 32, 16]
rows = []
for s in svgs:
    base = os.path.basename(s)[:-4]
    ims = {}
    for z in sizes:
        out = f"{D}/png/{base}-{z}.png"
        cairosvg.svg2png(url=s, write_to=out, output_width=z, output_height=z)
        ims[z] = Image.open(out).convert("RGBA")
    rows.append((base, ims))

# contact sheet: per concept row, on light and dark: 320 view, 128, 32 (x4 nearest), 16 (x8 nearest), plus 32/16 actual
W = 2 * (340 + 148 + 148 + 148 + 60) + 20
Hrow = 350
sheet = Image.new("RGBA", (W, Hrow * len(rows)), (255, 255, 255, 255))
for r, (base, ims) in enumerate(rows):
    for side, bgc in enumerate([(246, 246, 246, 255), (30, 30, 32, 255)]):
        x0 = side * (W // 2)
        tile = Image.new("RGBA", (W // 2, Hrow), bgc)
        big = ims[1024].resize((320, 320), Image.LANCZOS)
        tile.alpha_composite(big, (10, 15))
        tile.alpha_composite(ims[128], (340, 110))
        tile.alpha_composite(ims[32].resize((128, 128), Image.NEAREST), (488, 110))
        tile.alpha_composite(ims[16].resize((128, 128), Image.NEAREST), (636, 110))
        tile.alpha_composite(ims[32], (784, 120))
        tile.alpha_composite(ims[16], (830, 128))
        sheet.alpha_composite(tile, (x0, r * Hrow))
which = sys.argv[1] if len(sys.argv) > 1 else "all"
sheet.convert("RGB").save(f"{SCR}/contact.png")
for r, (base, ims) in enumerate(rows):
    sheet.crop((0, r * Hrow, W, (r + 1) * Hrow)).convert("RGB").save(f"{SCR}/row-{base}.png")
print("ok", len(rows))
