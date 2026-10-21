"""Fully-loaded detection: for each move in a recording (a burst of change after a still hold),
take the LAST frame of the hold that follows as the reference (by then every tile has arrived),
and report the first frame after the move that matches it (mean abs difference below `tol` grey
levels on a 704x440 copy, ignoring the bottom-right attribution corner).

    python capture/loaded.py cfr/<name>.mov [tol=0.25]
prints: move start, first-matching ("loaded") time, reference time, and the residual there.
"""
import subprocess, sys, numpy as np
p = sys.argv[1]; tol = float(sys.argv[2]) if len(sys.argv) > 2 else 0.25
w, h = 704, 440
raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', p, '-vf', f'fps=30,scale={w}:{h}', '-f', 'rawvideo', '-pix_fmt', 'gray', '-'],
                     capture_output=True).stdout
f = np.frombuffer(raw, np.uint8).reshape(-1, h, w).astype(np.float32)
d = np.abs(np.diff(f, axis=0)).mean(axis=(1, 2))
moving = d > 0.6
starts = [i + 1 for i in range(1, len(d)) if moving[i] and not moving[i - 1]]
# group bursts: a move = first burst after >= 3 s of near-stillness
moves, last = [], -999
for s in starts:
    if s - last > 90:
        moves.append(s)
    last = s
moves.append(len(f) - 1)
for a, b in zip(moves, moves[1:]):
    ref_i = b - 15                     # 0.5 s before the next move: fully loaded
    ref = f[ref_i]
    diffs = np.abs(f[a:ref_i + 1] - ref).mean(axis=(1, 2))
    ok = np.where(diffs < tol)[0]
    j = a + (ok[0] if len(ok) else len(diffs) - 1)
    print(f'move {a / 30:7.2f}  loaded {j / 30:7.2f}  ref {ref_i / 30:7.2f}  ({b / 30 - a / 30:5.1f} s hold)')
