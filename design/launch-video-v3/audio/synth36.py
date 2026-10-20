"""NimbleMap launch film v3.5 (audio/synth34.py retimed to edit/edl35.py): music bed, sounds and narration mix. Built on v3's synthesiser
(audio/synth.py), written as a fuller, calmer bed under speech. Writes two masters:
  <out>.wav             music + narration (the music sidechain-ducked about 9 dB under the voice)
  <out>-music-only.wav  the same music without the voice
Narration: audio/vo_edge/*.mp3 (edge-tts en-US-AndrewNeural, +4%; audio/voice_edge.py) or, with
VO_ENGINE=say, audio/vo/*.aiff (macOS say; audio/voice.sh), placed at edl32.VO.

(v3's notes follow.) NimbleMap launch film v3 ("one take"): music and sounds, synthesised from scratch (numpy/scipy,
no samples, no licensed recordings). Extends v2's instruments.

115 BPM (beat 0.5217 s, bar 2.087 s) in D minor, arranged on the beat sheet:
  0.00-4.20   no music: only the key clicks of bar 1 (times from edit/edl.py)
  4.20        ENTER = a downbeat: the groove starts (Dm9 - Bbmaj7 - F - C, one chord per bar)
  10.80       a whoosh under the travel; key clicks for bar 2, its Enter at 12.02
  12.73-16.90 the two-bar breakdown (on the drop's grid): no drums, pad and soft "dot" blips
              under the Ring of Fire, a boom on "M 9.1." (16.02), a riser into the drop
  16.90       DROP on bar 3: four-on-the-floor, clap, bass, chords, a bright lead
  21.07       a crash as the heatwave film wipes back and loops; 23.16 a swell for the gather
  26.81       the stop: one silent beat (everything cut)
  27.30       a hit that carries the name: Fmaj9 chord, bell, sub, a shimmer under the pen
Mastered to about -14 LUFS integrated, true peak <= -1 dBTP.

    python audio/synth.py out/audio.wav
"""
import os
import sys

import numpy as np
import pyloudnorm as pyln
from scipy import signal
from scipy.io import wavfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), 'edit'))
CUT = '3.6'
import edl36 as edl  # noqa: E402

SR = 48000
DUR = edl.DUR
N = int(SR * DUR)
BPM = 115.0
B = 60.0 / BPM
BAR = 4 * B
T_A = edl.T_ENTER                     # grid A: from ENTER
T_DROP = edl.T_DROP                   # grid B: from the drop (the breakdown is its two bars before)
T_BRK = T_DROP - 2 * BAR              # 12.726
NDROP = int(round((edl.T_PEN[0] - T_DROP) / BAR))   # bars from the drop to the hit
T_STOP = T_DROP + NDROP * BAR - B    # one beat before the hit's bar
T_HIT = edl.T_PEN[0]
rng = np.random.default_rng(115)


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


drums, bass, pads, keys, sfx, send, clicks = Bus(), Bus(), Bus(), Bus(), Bus(), Bus(), Bus()


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
def kick(punch=1.0):
    t = tt(0.45)
    f = 46 + 110 * np.exp(-t / 0.025)
    x = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.20)
    click = hp(rng.standard_normal(len(t)), 3000) * np.exp(-t / 0.002) * 0.15 * punch
    return np.tanh((x + click) * 1.6) * 0.9


def hat(open_=False):
    d = 0.25 if open_ else 0.06
    t = tt(d)
    n = hp(rng.standard_normal(len(t)), 7000) * np.exp(-t / (0.07 if open_ else 0.014))
    return n * 0.22


def clap():
    t = tt(0.25)
    n = bp(rng.standard_normal(len(t)), 900, 4500)
    env = np.zeros(len(t))
    for o in (0.0, 0.011, 0.022):
        env += np.where(t >= o, np.exp(-(t - o) / 0.008), 0) * 0.6
    env += np.exp(-t / 0.08) * 0.5
    return n * env * 0.5


def pad(ms, d, attack=0.6, release=1.2, cutoff=1400):
    t = tt(d)
    x = np.zeros((len(t), 2))
    for m in ms:
        for det, side in [(-0.08, 0), (0.08, 1), (0.0, 0), (0.0, 1)]:
            x[:, side] += saw(mtof(m + det), d, 7) * 0.16
            x[:, side] += tri(mtof(m + det) * 0.5, d) * 0.05
    x = lp(x.T, cutoff).T
    e = np.clip(t / attack, 0, 1) * np.clip((d - t) / release, 0, 1)
    return x * e[:, None]


def pluck(m, d=0.5, bright=1.0):
    t = tt(d)
    f = mtof(m)
    x = (np.sin(2 * np.pi * f * t) + 0.35 * bright * np.sin(4 * np.pi * f * t) * np.exp(-t / 0.08)
         + 0.15 * bright * np.sin(6 * np.pi * f * t) * np.exp(-t / 0.04))
    return x * np.clip(t / 0.003, 0, 1) * np.exp(-t / 0.2) * 0.5


def subbass(m, d, drive=1.0):
    t = tt(d)
    x = np.sin(2 * np.pi * mtof(m) * t) + 0.3 * np.sin(4 * np.pi * mtof(m) * t)
    e = np.clip(t / 0.01, 0, 1) * np.clip((d - t) / 0.05, 0, 1)
    return np.tanh(x * drive) * e * 0.55


def bell(base, d=3.0):
    t = tt(d)
    x = np.zeros(len(t))
    for r, a, tau in [(1, 1, 1.4), (2.0, 0.35, 0.8), (3.01, 0.18, 0.5), (4.2, 0.08, 0.3)]:
        x += a * np.sin(2 * np.pi * base * r * t) * np.exp(-t / tau)
    return x * np.clip(t / 0.004, 0, 1) * 0.3


def blip(m):
    """A soft, short sine blip: one dot lighting up."""
    t = tt(0.18)
    return np.sin(2 * np.pi * mtof(m) * t) * np.exp(-t / 0.05) * np.clip(t / 0.002, 0, 1) * 0.25


def keyclick(strong=False):
    """A key on a laptop keyboard: a short click, a soft plastic body, a little bottom-out."""
    d = 0.09 if not strong else 0.14
    t = tt(d)
    tick = hp(rng.standard_normal(len(t)), 2500) * np.exp(-t / 0.0018)
    body = bp(rng.standard_normal(len(t)), 600, 2400) * np.exp(-t / 0.012)
    thump = np.sin(2 * np.pi * (180 if strong else 230) * t) * np.exp(-t / 0.02)
    up = np.zeros(len(t))
    k = int(0.045 * SR)
    if k < len(t):
        up[k:] = hp(rng.standard_normal(len(t) - k), 3000) * np.exp(-np.arange(len(t) - k) / SR / 0.0012) * 0.4
    g = 1.25 if strong else 1.0
    return (tick * 0.5 + body * 0.35 + thump * (0.5 if strong else 0.3) + up) * 0.6 * g


def whoosh(d, up=True):
    t = tt(d)
    n = rng.standard_normal(len(t))
    out = np.zeros(len(t))
    blk = 512
    for s in range(0, len(t), blk):
        u = s / len(t)
        u = u if up else 1 - u
        f = 400 * (6000 / 400) ** u
        seg = bp(n[max(0, s - 2048):s + blk], f * 0.6, min(f * 1.6, 20000))[-min(blk, len(t) - s):]
        out[s:s + len(seg)] = seg
    return out * np.sin(np.pi * t / d) ** 2 * 0.35


def riser(d):
    t = tt(d)
    f = 200 * (1600 / 200) ** (t / d)
    tone = np.sin(2 * np.pi * np.cumsum(f) / SR) * 0.15 + np.sin(2 * np.pi * np.cumsum(f * 1.5) / SR) * 0.08
    noise = whoosh(d) * 0.8
    e = (t / d) ** 2
    return (tone + noise) * e


def boom():
    t = tt(1.6)
    f = 38 + 50 * np.exp(-t / 0.06)
    x = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.6)
    n = lp(rng.standard_normal(len(t)), 600) * np.exp(-t / 0.15) * 0.3
    return np.tanh((x + n) * 1.4) * 0.8


def crash():
    t = tt(2.0)
    n = hp(rng.standard_normal(len(t)), 4000) * np.exp(-t / 0.7)
    return n * 0.18


def shimmer(d):
    """The pen: a fast run of soft high notes up an Fmaj9 arpeggio."""
    out = np.zeros(int(d * SR) + SR)
    notes = [77, 81, 84, 88, 89, 91, 93, 96]
    for i in range(16):
        tk = i * d / 16
        m = notes[i % len(notes)] + (0 if i < 8 else 0)
        s = pluck(m, 0.35, 0.6) * 0.55
        k = int(tk * SR)
        out[k:k + len(s)] += s[:len(out) - k]
    return out


# ----------------------------------------------------------------- harmony (D minor)
DM9 = ([50, 57, 60, 64, 65], 38, [62, 65, 69, 72, 74, 72, 69, 65])
BBMAJ7 = ([46, 53, 57, 62, 65], 34, [58, 62, 65, 69, 70, 69, 65, 62])
FMAJ = ([53, 57, 60, 65, 69], 41, [60, 65, 69, 72, 77, 72, 69, 65])
CADD9 = ([48, 55, 62, 64, 67], 36, [60, 62, 67, 72, 74, 72, 67, 62])
PROG = [DM9, BBMAJ7, FMAJ, CADD9]
GM9 = ([43, 50, 53, 57, 58], 31, [62, 65, 69, 70, 74, 70, 69, 65])
A7SUS = ([45, 52, 55, 57, 62], 33, [64, 67, 69, 74, 76, 74, 69, 67])
FMAJ9 = [41, 53, 57, 60, 64, 67, 72]


def groove_bar(t0, ch, level, drop=False, last_beat_cut=False):
    """One bar of the groove at t0. level 1 = section A, drop=True = the drop's fuller kit."""
    notes, root, arp = ch
    nb = 3 if last_beat_cut else 4
    pads.add(pad(notes, nb * B + (0.05 if last_beat_cut else 0.5), attack=0.05 if drop else 0.3,
                 release=0.04 if last_beat_cut else 0.9, cutoff=1500 if drop else 1050), t0, 0.62 if drop else 0.6)
    for k in range(nb):
        tb = t0 + k * B
        drums.add(kick(1.2 if drop else 1.0), tb, 0.85 if drop else (0.7 if k % 2 == 0 else 0.0))
        if drop and k in (1, 3):
            drums.add(clap(), tb, 0.45)
        elif not drop and k in (1, 3):
            drums.add(clap(), tb, 0.2)
        bass.add(subbass(root + (0 if k % 2 == 0 else 12 if drop else 0), B * 0.85, 1.6 if drop else 1.0), tb + (B / 2 if drop else 0), 0.75)
        if drop:
            bass.add(subbass(root, B * 0.4, 1.4), tb, 0.45)
    for k in range(nb * 2):
        te = t0 + k * B / 2
        drums.add(hat(open_=False), te, (0.45 if k % 2 else 0.22) * (1.0 if drop else 0.7), pan=0.25)
        m = arp[k % 8]
        keys.add(pluck(m + (12 if drop else 0), 0.45, 1.0), te, 0.17 if k % 2 == 0 else 0.11, pan=-0.3 if k % 2 else 0.3)
        send.add(pluck(m, 0.45), te, 0.10)



E = edl
# ---- the folder (0 -> first downbeat): a warm pad under the voice
pads.add(pad([38, 50, 57, 62, 64], T_A + 0.4, attack=1.8, release=0.3, cutoff=650), 0.0, 0.42)
sfx.add(whoosh(0.9), E.T_LIFT_TIF + 0.2, 0.6, pan=0.3)              # the .tif row flies across
for tt_ in (E.T_OPEN_TIF, E.T_OPEN_CSV):                              # the first two files open
    sfx.add(bell(mtof(81), 1.8), tt_ + 0.05, 0.22)
    send.add(bell(mtof(81), 1.8), tt_ + 0.05, 0.2)
sfx.add(whoosh(0.45), E.T_OPEN_CSV - 0.45, 0.45, pan=-0.3)
sfx.add(whoosh(0.45), E.T_LIFT1, 0.5, pan=-0.3)

# ---- key clicks (quieter than v3)
letters, enters = E.key_times()
for kt in letters:
    clicks.add(keyclick(), kt, 0.55, pan=rng.uniform(-0.15, 0.15))
for et in enters:
    clicks.add(keyclick(strong=True), et, 0.65)

# ---- section A from the first downbeat to the breakdown
NA = int((T_BRK - T_A) // BAR)
for b in range(NA):
    groove_bar(T_A + b * BAR, PROG[b % 4], 1)
pads.add(pad(DM9[0], T_BRK - (T_A + NA * BAR) + 0.4, attack=0.05, release=0.3, cutoff=900), T_A + NA * BAR, 0.5)
sfx.add(boom() * 0.6, T_A, 0.6)
sfx.add(crash(), T_A, 0.6)
send.add(bell(mtof(74), 2.5), T_A + 0.02, 0.35)
sfx.add(bell(mtof(81), 2.0), E.T_SANDY + 0.35, 0.2)
send.add(bell(mtof(81), 2.0), E.T_SANDY + 0.35, 0.2)
sfx.add(whoosh(0.5), E.T_TRAVEL[0] - 0.05, 0.6, pan=-0.3)            # travel, the .gpkg comes in
drums.add(kick(0.6), E.T_TRAVEL[1], 0.3)
sfx.add(whoosh(0.7), E.T_OPEN2[0] - 0.05, 0.4)

# ---- the two-bar breakdown (on the drop grid)
pads.add(pad(GM9[0], BAR + 0.3, attack=0.4, release=0.4, cutoff=850), T_BRK, 0.6)
pads.add(pad(A7SUS[0], BAR, attack=0.3, release=0.05, cutoff=1000), T_BRK + BAR, 0.6)
bass.add(subbass(31, BAR * 0.95), T_BRK, 0.45)
bass.add(subbass(33, BAR * 0.95), T_BRK + BAR, 0.45)
dots = np.sort(rng.uniform(E.T_RING[0] + 0.05, E.T_RING[1] - 0.1, 30))
scale = [74, 77, 79, 81, 84, 86, 89]
for td in dots:
    m = scale[int(rng.integers(0, len(scale)))]
    keys.add(blip(m), td, 0.3 + 0.2 * rng.random(), pan=rng.uniform(-0.7, 0.7))
    send.add(blip(m), td, 0.2)
sfx.add(bell(mtof(86), 2.4), E.T_JAPAN + 0.2, 0.3)
send.add(bell(mtof(86), 2.4), E.T_JAPAN + 0.2, 0.25)
sfx.add(boom(), E.T_M91 + 0.02, 0.75)
sfx.add(whoosh(0.4), E.T_M91 + 0.38, 0.45, pan=-0.3)                 # the .nc comes in
sfx.add(riser(1.6), T_DROP - 1.6, 0.55)

# ---- the drop, to the stop (the last beat before the hit's bar is cut)
for b in range(NDROP):
    groove_bar(T_DROP + b * BAR, PROG[b % 4], 2, drop=True, last_beat_cut=(b == NDROP - 1))
sfx.add(boom(), T_DROP, 0.75)
sfx.add(crash(), T_DROP, 0.8)
sfx.add(whoosh(0.4), T_DROP - 0.05, 0.7, pan=0.4)
sfx.add(boom() * 0.5, E.T_TEMP_BOOM, 0.5)                                   # "36.50 C"
sfx.add(whoosh(0.5), E.T_PRO[0] - 0.05, 0.5, pan=-0.3)               # to the Pro window
sfx.add(whoosh(0.5), E.T_PRO2[0] - 0.05, 0.5, pan=-0.3)
send.add(bell(mtof(88), 2.0), E.T_SEND2 + 0.65, 0.25)
send.add(bell(mtof(88), 2.0), E.T_SEND + 0.65, 0.25)                 # the answer lands
sfx.add(whoosh(1.4), E.T_GATHER[0] - 0.4, 0.4)
LEAD = [(0, 74, 1), (1.5, 72, 0.5), (2, 69, 1), (3, 72, 1), (4, 77, 1.5), (5.5, 76, 0.5), (6, 74, 2),
        (8, 74, 1), (9.5, 76, 0.5), (10, 77, 1), (11, 81, 1), (12, 79, 2), (14, 77, 1), (15, 76, 1),
        (16, 74, 2), (18, 72, 1)]
for rep in range(0, NDROP, 5):
    for beat, m, ln in LEAD:
        tl = T_DROP + rep * BAR + beat * B
        if tl + ln * B > T_STOP or beat + rep * 4 >= NDROP * 4:
            continue
        keys.add(pluck(m + 12, ln * B + 0.2, 1.0), tl, 0.13, pan=0.1)
        send.add(pluck(m + 12, ln * B, 0.8), tl, 0.08)

# ---- the hit that carries the name, then the resolve
pads.add(pad(FMAJ9, DUR - T_HIT, attack=0.01, release=3.0, cutoff=2000), T_HIT, 0.75)
bass.add(subbass(29, 3.2, 1.3), T_HIT, 0.85)
drums.add(kick(1.4), T_HIT, 0.9)
sfx.add(crash(), T_HIT, 0.8)
sfx.add(bell(mtof(77)), T_HIT + 0.01, 0.55)
send.add(bell(mtof(77)), T_HIT + 0.01, 0.45)
sfx.add(shimmer(E.T_PEN[1] - T_HIT), T_HIT + 0.05, 0.4)
send.add(shimmer(E.T_PEN[1] - T_HIT), T_HIT + 0.05, 0.25)
sfx.add(bell(mtof(84), 2.5), E.T_END, 0.25)
send.add(bell(mtof(84), 2.5), E.T_END, 0.2)

# ---- narration
from scipy.io import wavfile as _wf  # noqa: E402
import subprocess as _sp  # noqa: E402
# The voice: edge-tts en-US-AndrewNeural (audio/voice_edge.py; default) or macOS say (audio/voice.sh;
# VO_ENGINE=say). With edge-tts, each line is placed by its word boundaries: its first word starts
# at the line's start time, and the caption span runs to the end of its last word.
import json as _json  # noqa: E402
ENGINE = os.environ.get('VO_ENGINE', 'edge')
VO_DIR = os.path.join(HERE, E.VO_DIR)
vo = np.zeros(N)
spans = []
for vid, t0 in E.VO:
    src = os.path.join(VO_DIR, vid + ('.mp3' if ENGINE == 'edge' else '.aiff'))
    raw = _sp.run(['ffmpeg', '-v', 'error', '-i', src, '-ac', '1', '-ar', str(SR), '-f', 'f32le', '-'],
                  capture_output=True, check=True).stdout
    x = np.frombuffer(raw, np.float32).astype(np.float64)
    if ENGINE == 'edge':
        words = _json.load(open(os.path.join(VO_DIR, vid + '.json')))['words']
        w0 = words[0]['offset']
        w1 = words[-1]['offset'] + words[-1]['duration']
        x = x[max(0, int((w0 - 0.04) * SR)):int((w1 + 0.25) * SR)]
        lead, speech = 0.04, w1 - w0
    else:
        nz = np.where(np.abs(x) > 0.003)[0]
        x = x[nz[0]:nz[-1] + 1]
        lead, speech = 0.0, len(x) / SR
    x = hp(x, 110)
    x = x + 0.25 * bp(x, 2500, 5000)
    i = int((t0 - lead) * SR)
    n = min(len(x), N - i)
    vo[i:i + n] += x[:n]
    spans.append((vid, t0, t0 + speech))
vo /= np.max(np.abs(vo)) + 1e-9

# ----------------------------------------------------------------- mix
def reverb(x, rt=2.0, seed=7):
    r = np.random.default_rng(seed)
    n = int(rt * SR)
    t = np.arange(n) / SR
    ir = np.stack([r.standard_normal(n), r.standard_normal(n)], 1) * np.exp(-t * 6.9 / rt)[:, None]
    ir = lp(ir.T, 5000).T
    ir /= np.sqrt((ir ** 2).sum(0))
    return np.stack([signal.fftconvolve(x[:, c], ir[:, c])[:N] for c in range(2)], 1)


def delay(x, d=B * 0.75, fb=0.3, mix=0.25):
    out = x.copy()
    k = int(d * SR)
    g = mix
    for i in range(1, 5):
        out[k * i:] += x[:-k * i] * g
        g *= fb
    return out


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



t = np.arange(N) / SR
pump = np.ones(N)
for b in range(NDROP):
    for k in range(4):
        tb = T_DROP + b * BAR + k * B
        i = int(tb * SR)
        L = int(0.25 * SR)
        if i + L < N:
            pump[i:i + L] = np.minimum(pump[i:i + L], 0.7 + 0.3 * (np.arange(L) / L) ** 0.6)
music = (drums.x * 0.7 + bass.x * 0.8 + (pads.x * 0.75 + delay(keys.x) * 0.75) * pump[:, None]
         + reverb(send.x + pads.x * 0.15) * 0.5)
music = hp(music.T, 28).T
gate = np.ones(N)
stop0, stop1 = int(T_STOP * SR), int((T_HIT - 0.004) * SR)
gate[stop0:stop1] = 0
r = int(0.004 * SR)
gate[stop0 - r:stop0] = np.linspace(1, 0, r)
music *= gate[:, None]
fx = sfx.x * 0.85
fx[stop0:stop1] = 0
bed = music + fx + clicks.x * 1.0
fade_out = np.clip((DUR - t) / 1.8, 0, 1) ** 1.3

# sidechain duck: -9 dB while the voice speaks (100 ms lookahead, 120 ms attack, 450 ms release)
from scipy.ndimage import maximum_filter1d  # noqa: E402
env = maximum_filter1d((np.abs(vo) > 0.02).astype(float), size=int(0.18 * SR))
env = np.roll(env, -int(0.10 * SR))
duck = np.empty(N)
cur = 0.0
aa, ar = np.exp(-1 / (0.12 * SR)), np.exp(-1 / (0.45 * SR))
for i in range(N):
    e = env[i]
    cur = aa * cur + (1 - aa) * e if e > cur else ar * cur + (1 - ar) * e
    duck[i] = cur
gain = 10 ** (-9.0 * duck / 20)


def master(x):
    meter = pyln.Meter(SR)
    for _ in range(5):
        lufs = meter.integrated_loudness(x)
        x = x * 10 ** ((-14.0 - lufs) / 20)
        x = limiter(x)
    tp = np.max(np.abs(signal.resample_poly(x, 4, 1, axis=0)))
    if tp > 10 ** (-1.0 / 20):
        x = x * 10 ** (-1.0 / 20) / tp
    return x, meter.integrated_loudness(x), 20 * np.log10(np.max(np.abs(signal.resample_poly(x, 4, 1, axis=0))))


out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(HERE), 'out/audio-v3.6.wav')
music_only = bed * fade_out[:, None]
bed_rms = np.sqrt(np.mean(music_only ** 2)) + 1e-9
# voice level: its speech RMS about 12 dB above the ducked bed
vo_rms = np.sqrt(np.mean(vo[np.abs(vo) > 0.02] ** 2)) + 1e-9
voice = np.stack([vo, vo], 1) / vo_rms * bed_rms * 10 ** ((-9.0 + 12.0) / 20)
with_vo = (bed * gain[:, None] + voice) * fade_out[:, None]
voice_only = voice * fade_out[:, None]
_vt = voice_only / (np.max(np.abs(voice_only)) + 1e-9) * 10 ** (-1.5 / 20)
_wf.write(out.replace('.wav', '-voice.wav'), SR, (_vt * 32767).astype(np.int16))
for name, x in ((out, with_vo), (out.replace('.wav', '-music-only.wav'), music_only)):
    y, lufs, tpk = master(x)
    _wf.write(name, SR, (np.clip(y, -1, 1) * 32767).astype(np.int16))
    print(f'wrote {name}: {lufs:.2f} LUFS integrated, true peak {tpk:.2f} dBTP')
print(f'grid: downbeat {T_A:.3f}, breakdown {T_BRK:.3f}, drop {T_DROP:.3f}, stop {T_STOP:.3f}, hit {T_HIT:.3f}')
with open(out.replace('.wav', '-vo.txt'), 'w') as f:
    for vid, a, b in spans:
        f.write(f'{vid}\t{a:.3f}\t{b:.3f}\n')
for vid, a, b in spans:
    print(f'vo {vid:9s} {a:6.2f} - {b:6.2f}')
