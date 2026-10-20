"""Contact sheet of one recording: python edit/sheet.py cfr/<name>.mov out.png t1,t2,... [width]"""
import subprocess, sys
from PIL import Image, ImageDraw
src, out, ts = sys.argv[1], sys.argv[2], [float(x) for x in sys.argv[3].split(',')]
w = int(sys.argv[4]) if len(sys.argv) > 4 else 700
ims = []
for t in ts:
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-ss', str(t), '-i', src, '-frames:v', '1', '-vf', f'scale={w}:-2',
                          '-f', 'image2pipe', '-vcodec', 'png', '-'], capture_output=True).stdout
    import io
    im = Image.open(io.BytesIO(raw)).convert('RGB')
    ImageDraw.Draw(im).text((8, 8), f'{t:.2f}', fill=(255, 0, 0))
    ims.append(im)
cols = 3
h = ims[0].height
sheet = Image.new('RGB', (w * cols, h * ((len(ims) + cols - 1) // cols)), 'white')
for i, im in enumerate(ims):
    sheet.paste(im, ((i % cols) * w, (i // cols) * h))
sheet.save(out)
