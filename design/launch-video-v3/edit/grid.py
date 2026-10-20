"""Tile PNG stills into one review sheet: python edit/grid.py out.png a.png b.png ... [--w 640]"""
import sys
from PIL import Image, ImageDraw
args = sys.argv[1:]
w = 640
if '--w' in args:
    i = args.index('--w'); w = int(args[i + 1]); del args[i:i + 2]
out, files = args[0], args[1:]
ims = [Image.open(f).convert('RGB').resize((w, w * 9 // 16)) for f in files]
cols = 3
h = w * 9 // 16
sheet = Image.new('RGB', (cols * w, h * ((len(ims) + cols - 1) // cols)), 'white')
for i, (im, f) in enumerate(zip(ims, files)):
    ImageDraw.Draw(im).text((6, 4), f.split('/')[-1], fill=(255, 0, 0))
    sheet.paste(im, ((i % cols) * w, (i // cols) * h))
sheet.save(out)
