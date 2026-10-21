"""Motion and settling in a recording: prints each burst of change (a move, or tiles arriving)
and the time the picture is still again, so the edit can cut to the moment a view is sharp.
    python capture/settle.py cfr/<name>.mov [threshold]"""
import subprocess, sys, numpy as np
p = sys.argv[1]; thr = float(sys.argv[2]) if len(sys.argv) > 2 else 0.08
w, h = 352, 220
raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', p, '-vf', f'scale={w}:{h}', '-f', 'rawvideo', '-pix_fmt', 'gray', '-'], capture_output=True).stdout
f = np.frombuffer(raw, np.uint8).reshape(-1, h, w).astype(np.float32)
d = np.abs(np.diff(f, axis=0)).mean(axis=(1, 2))
on, start, quiet = False, 0, 0
for i, v in enumerate(d):
    t = (i + 1) / 60
    if v > thr:
        if not on:
            on, start = True, t
        quiet = 0
    elif on:
        quiet += 1
        if quiet >= 45:          # 0.75 s without change: settled
            print(f'change {start:7.2f} -> still at {t - 0.75:7.2f}')
            on = False
if on:
    print(f'change {start:7.2f} -> (end)')
