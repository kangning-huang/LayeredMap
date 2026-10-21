"""Print the times (s) where a raw recording changes a lot (tour steps landing), at 60 fps."""
import subprocess, sys, numpy as np
p = sys.argv[1]
w, h = 176, 110
raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', p, '-vf', f'fps=60,scale={w}:{h}', '-f', 'rawvideo', '-pix_fmt', 'gray', '-'], capture_output=True).stdout
f = np.frombuffer(raw, np.uint8).reshape(-1, h, w).astype(np.float32)
d = np.abs(np.diff(f, axis=0)).mean(axis=(1, 2))
thr = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
last = -10
for i, v in enumerate(d):
    if v > thr and (i - last) > 6:
        print(f'{(i + 1) / 60:7.3f}s  diff {v:6.2f}')
    if v > thr:
        last = i
print('frames', len(f))
