"""NimbleMap launch film v2: music and UI sounds, synthesised from scratch (no samples).

Calm, warm electronic bed at 100 BPM in F major (Fmaj9 - Am7 - Dm9 - Bbmaj9, one chord per bar;
Bbmaj9 - C6/9 for the Pro beat), arranged per story beat: a sparse opening, soft kick, shaker and
sub from "built in", a lighter octave-up texture for "your own file", a rim on 2 and 4 for "share",
bell sparkle for Pro. A soft rising bell stinger on the Haiyan click, airy swishes under caption
changes, soft UI clicks on every drawn cursor click (times from edit/edl.py), a resolving bell on
the end card. Mastered to about
-14 LUFS integrated, true peak <= -1 dBTP.

    mamba run -n lm-video python audio/synth.py out/audio.wav
"""
import os
import sys

import numpy as np
import pyloudnorm as pyln
from scipy import signal
from scipy.io import wavfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), 'edit'))
import edl  # noqa: E402

SR = 48000
DUR = edl.DUR
N = int(SR * DUR)
BPM = 100.0
B = 60.0 / BPM
BAR = 4 * B
rng = np.random.default_rng(100)


def tt(d):
    return np.arange(int(d * SR)) / SR


class Bus:
    def __init__(self):
        self.x = np.zeros((N, 2))

    def add(self, sig, t, gain=1.0, pan=0.0):
        i = int(round(t * SR))
        if sig.ndim == 1:
            a = (pan + 1) * np.pi / 4
            sig = np.stack([sig * np.cos(a), sig * np.sin(a)], axis=1) * np.sqrt(2)
        if i < 0:
            sig, i = sig[-i:], 0
        n = min(len(sig), N - i)
        if n > 0:
            self.x[i:i + n] += sig[:n] * gain


drums, bass, pads, keys, sfx, send = Bus(), Bus(), Bus(), Bus(), Bus(), Bus()


def lp(x, f, order=2):
    return signal.sosfilt(signal.butter(order, f, btype='low', fs=SR, output='sos'), x)


def hp(x, f, order=2):
    return signal.sosfilt(signal.butter(order, f, btype='high', fs=SR, output='sos'), x)


def bp(x, lo, hi, order=2):
    return signal.sosfilt(signal.butter(order, [lo, hi], btype='band', fs=SR, output='sos'), x)


def mtof(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def saw(f, d, nh=10):
    t = tt(d)
    out = np.zeros(len(t))
    for k in range(1, nh + 1):
        if f * k < SR / 2.2:
            out += np.sin(2 * np.pi * f * k * t) / k
    return out * 0.6


def tri(f, d):
    t = tt(d)
    return 2 / np.pi * np.arcsin(np.sin(2 * np.pi * f * t))


# ----------------------------------------------------------------- instruments
def soft_kick():
    t = tt(0.5)
    f = 48 + 70 * np.exp(-t / 0.03)
    x = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.22)
    return np.tanh(x * 1.2) * 0.9


def shaker(accent=1.0):
    d = 0.09
    t = tt(d)
    n = bp(rng.standard_normal(len(t)), 5000, 11000) * (np.clip(t / 0.012, 0, 1) * np.exp(-t / 0.03))
    return n * 0.22 * accent


def pad(ms, d, attack=1.2, release=1.6, cutoff=1100):
    t = tt(d)
    x = np.zeros((len(t), 2))
    for m in ms:
        for det, side in [(-0.07, 0), (0.07, 1), (0.0, 0), (0.0, 1)]:
            x[:, side] += saw(mtof(m + det), d, 7) * 0.18
            x[:, side] += tri(mtof(m + det) * 0.5, d) * 0.05
    # slow filter swell
    x = lp(x.T, cutoff).T
    e = np.clip(t / attack, 0, 1) * np.clip((d - t) / release, 0, 1)
    return x * e[:, None]


def pluck(m, d=0.6):
    t = tt(d)
    f = mtof(m)
    x = (np.sin(2 * np.pi * f * t) + 0.35 * np.sin(4 * np.pi * f * t) * np.exp(-t / 0.08)
         + 0.12 * np.sin(6 * np.pi * f * t) * np.exp(-t / 0.04))
    return x * np.clip(t / 0.003, 0, 1) * np.exp(-t / 0.22) * 0.5


def sub(m, d):
    t = tt(d)
    x = np.sin(2 * np.pi * mtof(m) * t) + 0.25 * np.sin(4 * np.pi * mtof(m) * t)
    e = np.clip(t / 0.02, 0, 1) * np.clip((d - t) / 0.08, 0, 1)
    return x * e * 0.55


def ui_click():
    """Soft, rounded UI click: short sine 'tock' plus a faint noise tick."""
    t = tt(0.06)
    body = np.sin(2 * np.pi * 1350 * t) * np.exp(-t / 0.006)
    low = np.sin(2 * np.pi * 420 * t) * np.exp(-t / 0.012)
    tick = hp(rng.standard_normal(len(t)), 4000) * np.exp(-t / 0.0012)
    return (body * 0.5 + low * 0.35 + tick * 0.12)


def rim():
    t = tt(0.08)
    body = np.sin(2 * np.pi * 820 * t) * np.exp(-t / 0.012)
    n = bp(rng.standard_normal(len(t)), 1500, 5000) * np.exp(-t / 0.006)
    return (body * 0.4 + n * 0.3) * 0.6


def air(d):
    """A soft, airy swish (band-passed noise, gentle swell) for caption changes."""
    t = tt(d)
    n = bp(rng.standard_normal(len(t)), 2500, 9000)
    return n * np.sin(np.pi * t / d) ** 2 * 0.12


def bell(base, d=3.0):
    t = tt(d)
    x = np.zeros(len(t))
    for r, a, tau in [(1, 1, 1.4), (2.0, 0.35, 0.8), (3.01, 0.18, 0.5), (4.2, 0.08, 0.3)]:
        x += a * np.sin(2 * np.pi * base * r * t) * np.exp(-t / tau)
    return x * np.clip(t / 0.004, 0, 1) * 0.3


def swell(d):
    t = tt(d)
    n = rng.standard_normal(len(t))
    out = np.zeros(len(t))
    blk = 512
    for s in range(0, len(t), blk):
        u = s / len(t)
        f = 300 * (3000 / 300) ** u
        seg = bp(n[max(0, s - 2048):s + blk], f * 0.7, f * 1.3)[-min(blk, len(t) - s):]
        out[s:s + len(seg)] = seg
    return out * np.sin(np.pi * t / d) ** 2 * 0.25


# ----------------------------------------------------------------- score
# F major, one chord per bar (2.4 s). The harmony lifts for Pro (Bbmaj9 - C6/9) and resolves on F.
FMAJ9 = ([53, 57, 60, 64, 67], 41, [65, 69, 72, 76, 79, 76, 72, 69])
AM7 = ([57, 60, 64, 67], 45, [64, 69, 72, 76, 79, 76, 72, 69])
DM9 = ([50, 57, 60, 64, 65], 38, [62, 65, 69, 72, 76, 72, 69, 65])
BBMAJ9 = ([58, 62, 65, 69, 72], 46, [62, 65, 70, 74, 77, 74, 70, 65])
C69 = ([60, 64, 67, 69, 74], 48, [64, 67, 69, 72, 74, 76, 79, 76])
END = edl.END_T0
CAP = {c[2]: c[0] for c in edl.CAPTIONS}
T_BUILTIN, T_FILE, T_SHARE, T_PRO = 8.8, 14.5, 19.8, 25.5


def section(t):
    if t < T_BUILTIN:
        return 'hook'
    if t < T_FILE:
        return 'builtin'
    if t < T_SHARE:
        return 'file'
    if t < T_PRO:
        return 'share'
    return 'pro'


nbars = int(np.ceil(END / BAR)) + 1
for b in range(nbars):
    t0 = b * BAR
    if t0 >= END:
        break
    sec = section(t0 + 0.01)
    if sec == 'pro':
        ch, root, arp = [BBMAJ9, C69][b % 2]
    else:
        ch, root, arp = [FMAJ9, AM7, DM9, BBMAJ9][b % 4]
    cutoff = {'hook': 900, 'builtin': 1300, 'file': 750, 'share': 1500, 'pro': 1700}[sec]
    pads.add(pad(ch, min(BAR + 1.6, END - t0 + 0.6), attack=0.9 if b else 1.6, cutoff=cutoff), t0, 0.55)
    for k in range(8):
        tk = t0 + k * B / 2
        if tk >= END - 0.02:
            break
        sk = section(tk)
        m = arp[k]
        if sk == 'hook':
            # sparse: quarter notes, from the click on
            if k % 2 == 0 and tk > 0.9:
                keys.add(pluck(m), tk, 0.26, pan=(-0.3 if k % 4 else 0.3))
                send.add(pluck(m), tk, 0.12)
        elif sk == 'file':
            # an octave up, softer, every other eighth: a lighter, "your own" moment
            if k % 2 == 1:
                keys.add(pluck(m + 12, 0.5), tk, 0.18, pan=(-0.4 if k % 4 == 1 else 0.4))
                send.add(pluck(m + 12, 0.5), tk, 0.10)
        else:
            g = 0.30 if k % 2 == 0 else 0.22
            keys.add(pluck(m), tk, g, pan=(-0.35 if k % 2 else 0.35))
            send.add(pluck(m), tk, 0.12)
            if sk == 'pro' and k % 2 == 0:
                keys.add(bell(mtof(m + 12), 1.2), tk, 0.05)
    for k in range(4):
        tk = t0 + k * B
        if tk >= END:
            break
        sk = section(tk)
        if sk in ('builtin', 'share', 'pro'):
            bass.add(sub(root, B * 0.9), tk, 0.7)
            if k in (0, 2) or (sk == 'share'):
                drums.add(soft_kick(), tk, 0.55 if k in (0, 2) else 0.35)
            if sk == 'share' and k in (1, 3):
                drums.add(rim(), tk, 0.5)
        elif sk == 'file' and k == 0:
            bass.add(sub(root, BAR * 0.9), tk, 0.55)
        elif sk == 'hook' and k == 0 and tk > 0.5:
            bass.add(sub(root, BAR * 0.9), tk, 0.45)
    for k in range(8):
        tk = t0 + k * B / 2
        sk = section(tk)
        if tk < END - 0.05 and sk in ('builtin', 'share', 'pro', 'file'):
            gain = 0.55 if sk == 'file' else 1.0
            drums.add(shaker(1.0 if k % 2 else 0.6), tk + 0.008 * (k % 2), gain, pan=0.2)

# Hook stinger on the Haiyan click: a soft rising bell arpeggio over a low swell.
tc0 = edl.click_times()[0]
for i, m in enumerate([77, 81, 84, 88]):
    sfx.add(bell(mtof(m), 2.4), tc0 + 0.03 + i * 0.07, 0.30 - i * 0.04)
    send.add(bell(mtof(m), 2.4), tc0 + 0.03 + i * 0.07, 0.25)
bass.add(sub(29, 1.6), tc0, 0.5)
sfx.add(swell(0.9), tc0 - 0.6, 0.35)
# smaller chime for the Beijing click
sfx.add(bell(mtof(84), 1.6), edl.click_times()[1] + 0.03, 0.16)
send.add(bell(mtof(84), 1.6), edl.click_times()[1] + 0.03, 0.18)

# Under each caption change: a subtle airy swish.
for (t0c, t1c, text) in edl.CAPTIONS:
    sfx.add(air(0.35), t0c - 0.06, 0.32)

# Section transitions: a soft swell into each beat.
for tt_ in (T_BUILTIN, T_FILE, T_SHARE, T_PRO):
    sfx.add(swell(0.8), tt_ - 0.7, 0.30)

# End: resolve on F with a long pad and a bell (the logo sting).
pads.add(pad([41, 53, 57, 60, 64, 69], DUR - END + 0.2, attack=0.3, release=2.4, cutoff=1500), END, 0.6)
sfx.add(bell(mtof(77)), END + 0.05, 0.55)
send.add(bell(mtof(77)), END + 0.05, 0.4)
sfx.add(bell(mtof(84)), END + 0.40, 0.25)
sfx.add(bell(mtof(89), 2.0), END + 1.5, 0.12)   # under the sheen
bass.add(sub(29, 3.5), END, 0.8)
sfx.add(swell(1.2), END - 1.1, 0.6)

# UI clicks on every drawn cursor click
for tc in edl.click_times():
    sfx.add(ui_click(), tc, 0.55)
    send.add(ui_click(), tc, 0.12)


# ----------------------------------------------------------------- mix
def reverb(x, rt=2.2, seed=7):
    r = np.random.default_rng(seed)
    n = int(rt * SR)
    t = np.arange(n) / SR
    ir = np.stack([r.standard_normal(n), r.standard_normal(n)], 1) * np.exp(-t * 6.9 / rt)[:, None]
    ir = lp(ir.T, 5000).T
    ir /= np.sqrt((ir ** 2).sum(0))
    return np.stack([signal.fftconvolve(x[:, c], ir[:, c])[:N] for c in range(2)], 1)


def delay(x, d=B * 0.75, fb=0.35, mix=0.3):
    out = x.copy()
    k = int(d * SR)
    g = mix
    for i in range(1, 6):
        out[k * i:] += x[:-k * i] * g
        g *= fb
    return out


mix = (drums.x * 0.85 + bass.x * 0.85 + pads.x * 0.7 + delay(keys.x) * 0.8 + sfx.x * 0.9
       + reverb(send.x + pads.x * 0.15) * 0.5)
mix = hp(mix.T, 28).T
t = np.arange(N) / SR
fade_in = np.clip(t / 0.25, 0, 1)
fade_out = np.clip((DUR - t) / 1.8, 0, 1) ** 1.3
mix *= (fade_in * fade_out)[:, None]


def limiter(x, ceiling=10 ** (-1.5 / 20), look=0.002, rel=0.08):
    from scipy.ndimage import maximum_filter1d
    la = int(look * SR)
    pk = maximum_filter1d(np.max(np.abs(x), 1), size=2 * la + 1)
    gt = np.minimum(1.0, ceiling / np.maximum(pk, 1e-9))
    g = np.empty_like(gt)
    a = np.exp(-1 / (rel * SR))
    cur = 1.0
    for i in range(len(gt)):
        cur = gt[i] if gt[i] < cur else a * cur + (1 - a) * gt[i]
        g[i] = cur
    return x * g[:, None]


meter = pyln.Meter(SR)
for _ in range(5):
    lufs = meter.integrated_loudness(mix)
    mix *= 10 ** ((-14.0 - lufs) / 20)
    mix = limiter(mix)
tp = np.max(np.abs(signal.resample_poly(mix, 4, 1, axis=0)))
if tp > 10 ** (-1.0 / 20):
    mix *= 10 ** (-1.0 / 20) / tp
lufs = meter.integrated_loudness(mix)
tp = np.max(np.abs(signal.resample_poly(mix, 4, 1, axis=0)))
out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(HERE), 'out/audio.wav')
wavfile.write(out, SR, (np.clip(mix, -1, 1) * 32767).astype(np.int16))
print(f'wrote {out}: {lufs:.2f} LUFS integrated, true peak {20 * np.log10(tp):.2f} dBTP')
