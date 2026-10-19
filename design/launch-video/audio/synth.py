"""NimbleMap launch film: score and sound design, synthesised from scratch.

No samples or downloaded audio: every sound below is built from sine/saw
oscillators and seeded noise with numpy/scipy, then mixed and mastered to
about -14 LUFS integrated with true peak under -1 dBTP.

    mamba run -n lm-video python audio/synth.py out/audio.wav
Writes the WAV plus out/audio_cues.txt (every cue time, in seconds and beats).
"""
import sys
import numpy as np
from scipy import signal
import pyloudnorm as pyln
from scipy.io import wavfile

SR = 48000
DUR = 20.0
N = int(SR * DUR)
BPM = 122.0
B = 60.0 / BPM
rng = np.random.default_rng(122)


def at(k):
    return k * B


CUES = []


def cue(t, name):
    CUES.append((t, name))


# ----------------------------------------------------------------- buses
class Bus:
    def __init__(self):
        self.x = np.zeros((N, 2))

    def add(self, sig, t, gain=1.0, pan=0.0):
        """Place mono or stereo sig at time t (s). pan -1..1 (equal power)."""
        i = int(round(t * SR))
        if sig.ndim == 1:
            a = (pan + 1) * np.pi / 4
            sig = np.stack([sig * np.cos(a), sig * np.sin(a)], axis=1) * np.sqrt(2)
        if i < 0:
            sig = sig[-i:]
            i = 0
        n = min(len(sig), N - i)
        if n > 0:
            self.x[i:i + n] += sig[:n] * gain


drums, bass, music, sfx, verb_send = Bus(), Bus(), Bus(), Bus(), Bus()


def tt(d):
    return np.arange(int(d * SR)) / SR


def env_exp(d, tau, attack=0.002):
    t = tt(d)
    e = np.exp(-t / tau)
    if attack > 0:
        e *= np.clip(t / attack, 0, 1)
    return e


def noise(d):
    return rng.standard_normal(int(d * SR))


def bp(x, lo, hi, order=2):
    sos = signal.butter(order, [lo, hi], btype='band', fs=SR, output='sos')
    return signal.sosfilt(sos, x)


def hp(x, f, order=2):
    return signal.sosfilt(signal.butter(order, f, btype='high', fs=SR, output='sos'), x)


def lp(x, f, order=2):
    return signal.sosfilt(signal.butter(order, f, btype='low', fs=SR, output='sos'), x)


def sweep_phase(f):
    return 2 * np.pi * np.cumsum(f) / SR


def saw(f, d, nharm=14):
    t = tt(d)
    ph = sweep_phase(np.full(len(t), f) if np.isscalar(f) else f)
    out = np.zeros(len(t))
    for k in range(1, nharm + 1):
        out += np.sin(k * ph) / k
    return out * 0.6


def mtof(m):
    return 440.0 * 2 ** ((m - 69) / 12)


# ----------------------------------------------------------------- instruments
def kick(level=1.0, boom=False):
    d = 2.6 if boom else 0.45
    t = tt(d)
    if boom:
        f = 31 + 70 * np.exp(-t / 0.06)
        body = np.sin(sweep_phase(f)) * np.exp(-t / 0.9)
    else:
        f = 46 + 120 * np.exp(-t / 0.028)
        body = np.sin(sweep_phase(f)) * np.exp(-t / 0.24)
    body = np.tanh(body * 1.6) / np.tanh(1.6)
    click = hp(noise(0.006), 2500) * env_exp(0.006, 0.0015, 0)
    out = body.copy()
    out[:len(click)] += click * 0.35
    return out * level


def clap():
    d = 0.35
    x = np.zeros(int(d * SR))
    for k, off in enumerate([0, 0.011, 0.021, 0.032]):
        b = bp(noise(d), 900, 2800) * env_exp(d, 0.012 if k < 3 else 0.09, 0.0005)
        i = int(off * SR)
        x[i:] += b[:len(x) - i] * (0.7 if k < 3 else 1.0)
    return x * 0.5


def hat(open_=False):
    d = 0.22 if open_ else 0.05
    return hp(noise(d), 7500, 4) * env_exp(d, 0.07 if open_ else 0.012, 0.0005) * 0.5


def bass_note(m, d=0.22):
    f = mtof(m)
    t = tt(d)
    x = saw(f, d, 10) + 0.5 * np.sin(2 * np.pi * f / 2 * t)
    cutoff = 220 + 900 * np.exp(-t / 0.05)
    # time-varying one-pole lowpass (warm pluck)
    y = np.zeros_like(x)
    acc = 0.0
    a = 1 - np.exp(-2 * np.pi * cutoff / SR)
    for i in range(len(x)):
        acc += a[i] * (x[i] - acc)
        y[i] = acc
    return np.tanh(y * 1.4) * env_exp(d, 0.11, 0.004)


def pad_chord(ms, d, attack=0.6, release=1.2, cutoff=1400):
    t = tt(d)
    x = np.zeros((len(t), 2))
    for m in ms:
        for det, side in [(-0.08, 0), (0.08, 1), (0.0, 0), (0.0, 1)]:
            f = mtof(m + det)
            x[:, side] += saw(f, d, 8) * 0.25
    x = lp(x.T, cutoff).T
    e = np.clip(t / attack, 0, 1) * np.clip((d - t) / release, 0, 1)
    return x * e[:, None]


def pluck(m, d=0.3, bright=4000):
    f = mtof(m)
    t = tt(d)
    x = saw(f, d, 12)
    x = lp(x, bright)
    return x * env_exp(d, 0.09, 0.002)


def click(f0=2400):
    d = 0.05
    t = tt(d)
    blip = np.sin(2 * np.pi * f0 * t) * np.exp(-t / 0.008)
    n = hp(noise(d), 3000) * np.exp(-t / 0.002)
    return (blip * 0.7 + n * 0.4)


def keyclack(level=1.0, body_f=190):
    d = 0.09
    t = tt(d)
    top = bp(noise(d), 1800, 5200) * np.exp(-t / 0.006)
    body = np.sin(2 * np.pi * body_f * t) * np.exp(-t / 0.018)
    tick = hp(noise(d), 6000) * np.exp(-t / 0.0015)
    return (top * 0.8 + body * 0.6 + tick * 0.3) * level


def chime(base):
    d = 2.2
    t = tt(d)
    x = np.zeros(len(t))
    for ratio, amp, tau in [(1, 1, 0.9), (2.0, 0.45, 0.5), (2.76, 0.3, 0.35), (5.4, 0.12, 0.18), (1.5, 0.5, 0.7)]:
        x += amp * np.sin(2 * np.pi * base * ratio * t) * np.exp(-t / tau)
    return x * np.clip(t / 0.002, 0, 1) * 0.35


def whoosh(d, f0, f1, peak=0.7, q=0.6):
    """Band-passed noise sweeping f0→f1 with a swell peaking at fraction `peak`."""
    n = int(d * SR)
    x = noise(d)
    out = np.zeros(n)
    blk = 256
    for s in range(0, n, blk):
        u = s / n
        f = f0 * (f1 / f0) ** u
        lo, hi = max(40, f * (1 - q / 2)), min(SR / 2 - 100, f * (1 + q / 2))
        sos = signal.butter(2, [lo, hi], btype='band', fs=SR, output='sos')
        seg = x[max(0, s - 2048):s + blk]
        y = signal.sosfilt(sos, seg)[-min(blk, n - s):]
        out[s:s + len(y)] = y
    u = np.linspace(0, 1, n)
    e = np.where(u < peak, (u / peak) ** 2.2, np.exp(-(u - peak) / (1 - peak) * 4))
    return out * e


def riser(d, f0, f1):
    t = tt(d)
    f = f0 * (f1 / f0) ** (t / d) ** 1.6
    tone = np.sin(sweep_phase(f)) + 0.3 * np.sin(sweep_phase(f * 2.01))
    e = (t / d) ** 2
    return tone * e * 0.5 + whoosh(d, 400, 9000, peak=0.98) * 0.6


def zap():
    d = 0.18
    t = tt(d)
    f = 300 + 3200 * np.exp(-t / 0.025)
    return np.sin(sweep_phase(f)) * np.exp(-t / 0.05) * 0.6 + hp(noise(d), 4000) * np.exp(-t / 0.02) * 0.5


def shing():
    d = 0.6
    t = tt(d)
    x = sum(np.sin(2 * np.pi * f * t) * np.exp(-t / tau) for f, tau in [(5200, 0.12), (7400, 0.08), (3900, 0.18)])
    return x * 0.18 + hp(noise(d), 6000) * np.exp(-t / 0.05) * 0.3


def crash(d=1.8):
    t = tt(d)
    return hp(noise(d), 4500, 2) * np.exp(-t / 0.5) * np.clip(t / 0.002, 0, 1) * 0.35


def pop(f0=600, f1=1100, d=0.08):
    t = tt(d)
    f = f0 + (f1 - f0) * (1 - np.exp(-t / 0.01))
    return np.sin(sweep_phase(f)) * np.exp(-t / 0.022) * np.clip(t / 0.001, 0, 1)


def tick():
    d = 0.04
    t = tt(d)
    return (np.sin(2 * np.pi * 1850 * t) * np.exp(-t / 0.006) + bp(noise(d), 2500, 6000) * np.exp(-t / 0.002) * 0.5)


def shimmer(d=0.9):
    t = tt(d)
    f = 2400 * (1 + 0.8 * t / d)
    x = np.sin(sweep_phase(f)) + 0.6 * np.sin(sweep_phase(f * 1.5)) + 0.4 * np.sin(sweep_phase(f * 2.003))
    e = np.sin(np.pi * np.clip(t / d, 0, 1)) ** 2
    return x * e * 0.12


# ----------------------------------------------------------------- score
# D minor, 122 BPM. Bars of four beats; chord per bar: Dm - Bb - F - C
PROG = [(38, [62, 65, 69]), (34, [58, 62, 65]), (41, [60, 65, 69]), (36, [60, 64, 67])]

# pad: soft from the start, holds through; final Dm(add9) swell on the logo
for bar in range(0, 9):
    t0 = at(bar * 4)
    root, ch = PROG[bar % 4]
    lvl = 0.10 if bar == 0 else 0.13
    music.add(pad_chord(ch, 4 * B + 0.6, attack=0.5 if bar else 1.4, release=0.6, cutoff=900 + 120 * bar), t0, lvl)
    verb_send.add(pad_chord(ch, 4 * B + 0.6, attack=0.5, release=0.6, cutoff=900), t0, 0.05)
end_pad = pad_chord([50, 57, 62, 64, 69], 20.0 - at(34) + 0.2, attack=0.25, release=1.2, cutoff=1600)
music.add(end_pad, at(34), 0.2)
verb_send.add(end_pad, at(34), 0.12)

# kick: every beat from the whiteout (beat 4) to the push (beat 33)
for k in range(4, 34):
    drums.add(kick(0.9 if k % 4 else 1.0), at(k))
cue(at(4), 'kick enters (impact out of whiteout)')

# bass: off-beat warm pulses
for k in range(4, 34):
    root = PROG[(k // 4) % 4][0]
    bass.add(bass_note(root + 12), at(k + 0.5), 0.55)
    if k >= 15:
        bass.add(bass_note(root + 12, 0.16), at(k + 0.75), 0.25)

# hats: 16ths from beat 8, open hats on off-beats from beat 21
for s in range(8 * 4, 34 * 4):
    k = s / 4
    vel = [0.35, 0.18, 0.6, 0.2][s % 4]
    drums.add(hat(), at(k), vel * 0.55, pan=0.25 if s % 2 else -0.15)
    if k >= 21 and s % 4 == 2:
        drums.add(hat(True), at(k), 0.22, pan=0.1)

# clap on 2 and 4 from beat 9
for k in range(9, 34):
    if k % 2 == 1:
        drums.add(clap(), at(k), 0.55)
        verb_send.add(clap(), at(k), 0.15)

# ----------------------------------------------------------------- sound design on the picture
# S1: laser rise into the slice, whoosh into the whiteout
sfx.add(riser(at(3) - 0.2, 180, 2600), 0.2, 0.55)
cue(0.2, 'laser rise starts (pinpoint)')
sfx.add(zap(), at(3), 0.8)
sfx.add(shing(), at(3), 0.7)
verb_send.add(shing(), at(3), 0.3)
cue(at(3), 'slice zap + shing (stripes cut the globe)')
sfx.add(whoosh(at(4) - at(3.15), 200, 6000, peak=0.97), at(3.15), 0.9)
cue(at(3.15), 'dive whoosh')
drums.add(crash(2.0), at(4), 0.8)
sfx.add(kick(0.7, boom=False), at(4), 0.4)
# S2: type snaps, format chips, blade
for k, nm in [(5, 'line 1 snaps'), (6, 'line 2 snaps')]:
    sfx.add(pop(220, 140, 0.12), at(k), 0.6)
    sfx.add(click(3200), at(k), 0.25)
    cue(at(k), nm)
for i in range(5):
    sfx.add(click(2000 + 260 * i), at(6.5) + i * B / 4, 0.18, pan=-0.4 + 0.2 * i)
cue(at(6.5), 'format chip ticks x5 (16ths)')
sfx.add(whoosh(BL := (at(9) - at(8.25)), 300, 8000, peak=0.55), at(8.25), 0.9)
sfx.add(shing(), at(8.25) + BL * 0.5, 0.6)
cue(at(8.25) + BL * 0.5, 'blade wipe (whoosh + shing at mid-frame)')
# S3: status clicks on each beat, chime on the check
sfx.add(pop(400, 700, 0.06), at(8.75), 0.35)
for i, k in enumerate([9, 10, 11, 12]):
    sfx.add(click(2000 * 2 ** (i * 3 / 12)), at(k), 0.6)
    verb_send.add(click(2000), at(k), 0.15)
    cue(at(k), f'status click {i + 1} ({["Queued", "Reading", "Locating", "Styling"][i]})')
sfx.add(click(3000), at(13), 0.5)
sfx.add(chime(mtof(86)), at(13), 0.9)       # D6
sfx.add(chime(mtof(93)), at(13) + 0.03, 0.5)  # A6
verb_send.add(chime(mtof(86)), at(13), 0.5)
for i in range(8):
    sfx.add(tick(), at(13) + 0.02 + i * 0.025, 0.12, pan=-0.7 + i * 0.2)
cue(at(13), 'chime on the check + burst sparkle')
sfx.add(whoosh(at(15) - at(14), 150, 5000, peak=0.98), at(14), 0.8)
cue(at(14), 'zoom-into-check riser')
sfx.add(whoosh(0.45, 6000, 400, peak=0.08), at(15), 0.6)
drums.add(crash(1.2), at(15), 0.35)
cue(at(15), 'blue split (swoosh)')
# S4: keys, menu, typing, confirm pop, toast
sfx.add(keyclack(0.5, 150), at(15) + 0.04, 0.5)
sfx.add(keyclack(0.5, 170), at(15.5), 0.5)
sfx.add(keyclack(1.0, 150), at(16), 0.9)
sfx.add(keyclack(1.0, 200), at(16) + 0.022, 0.8)
cue(at(16), 'cmd+F key clack')
sfx.add(pop(300, 620, 0.1), at(16.5), 0.55)
sfx.add(whoosh(0.2, 2000, 600, peak=0.2), at(16.5), 0.2)
cue(at(16.5), 'menu pops open')
for i, k in enumerate([17, 17.5, 18, 18.5]):
    sfx.add(keyclack(0.75, 210 + 15 * i), at(k), 0.6, pan=-0.1 + 0.08 * i)
    cue(at(k), f'type "{"haiy"[:i + 1]}" key clack')
sfx.add(keyclack(0.9, 160), at(19), 0.6)
sfx.add(pop(880, 1320, 0.12), at(19), 0.75)
verb_send.add(pop(880, 1320, 0.12), at(19), 0.3)
cue(at(19), 'Enter: confirm pop')
sfx.add(whoosh(0.35, 1500, 400, peak=0.3), at(19.5), 0.35)
sfx.add(chime(mtof(81)) * 0.6, at(19.5) + 0.18, 0.35)
sfx.add(chime(mtof(86)) * 0.6, at(19.5) + 0.26, 0.3)
cue(at(19.5), 'toast "Zoomed to Haiyan"')
# S5: depth push, card slides, verbs
sfx.add(whoosh(at(21.3) - at(20.75), 120, 3000, peak=0.6), at(20.75), 0.8)
cue(at(20.75), 'depth push to the board')
for i, k in enumerate([22, 23, 24, 25, 26]):
    sfx.add(whoosh(0.32, 900, 3500, peak=0.45, q=0.8), at(k), 0.6, pan=(-0.35 if i % 2 else 0.35))
    sfx.add(click(1400), at(k) + 0.36, 0.2)
    cue(at(k), f'card slide {i + 1}')
for k in [22, 24, 26]:
    sfx.add(pop(180, 120, 0.1), at(k), 0.5)
# S6: slice in, arpeggio, counter ticks, badge flip, push
sfx.add(whoosh(at(27.05) - at(26.5), 400, 9000, peak=0.5), at(26.5), 0.8)
sfx.add(shing(), at(26.75), 0.5)
cue(at(26.75), 'diagonal slice to metrics')
scale = [62, 65, 67, 69, 72, 74, 77, 79, 81, 84, 86, 89]
for i, m in enumerate(scale):
    tt0 = at(28) + i * B / 4
    music.add(pluck(m, 0.28, 2500 + 400 * i), tt0, 0.28, pan=-0.5 + i / 11)
    verb_send.add(pluck(m, 0.28), tt0, 0.08)
cue(at(28), 'rising arpeggio (12 x 16ths, D4 → F6)')
for k in [29, 29.5, 30, 30.5]:
    sfx.add(tick(), at(k), 0.7)
    sfx.add(tick(), at(k) + 0.05, 0.3)
    cue(at(k), 'counter tick')
sfx.add(whoosh(0.3, 3000, 800, peak=0.4), at(31), 0.4)
sfx.add(pop(660, 990, 0.1), at(31) + 0.12, 0.6)
cue(at(31), 'badge flip Ready -> Pro')
sfx.add(riser(at(34) - at(32.4), 120, 1800), at(32.4), 0.45)
sfx.add(whoosh(at(34) - at(32.9), 100, 7000, peak=0.97), at(32.9), 0.8)
cue(at(32.9), 'push-into-camera riser')
# S7: logo boom, slide, pill, sheen
drums.add(kick(1.0, boom=True), at(34), 1.15)
sfx.add(crash(3.0), at(34), 0.5)
verb_send.add(crash(2.0), at(34), 0.4)
cue(at(34), 'SUB-BASS BOOM: logo mark appears')
sfx.add(whoosh(0.5, 300, 1500, peak=0.35), at(35), 0.4)
cue(at(35), 'mark slides left, wordmark reveals')
sfx.add(shimmer(0.7), at(36), 0.5)
cue(at(36), 'tagline fades up (soft shimmer)')
sfx.add(pop(500, 750, 0.09), at(37), 0.45)
cue(at(37), 'pill pops in')
sfx.add(shimmer(0.62), at(38), 1.0)
verb_send.add(shimmer(0.62), at(38), 0.5)
cue(at(38), 'sheen sweep')
cue(19.3, 'fade to black begins (audio fades with picture)')


# ----------------------------------------------------------------- mix
def reverb(x, rt=1.6, seed=5):
    r = np.random.default_rng(seed)
    n = int(rt * SR)
    t = np.arange(n) / SR
    ir = np.stack([r.standard_normal(n), r.standard_normal(n)], 1) * np.exp(-t * 6.9 / rt)[:, None]
    ir = lp(ir.T, 6000).T
    ir /= np.sqrt((ir ** 2).sum(0))
    return np.stack([signal.fftconvolve(x[:, c], ir[:, c])[:N] for c in range(2)], 1)


def sidechain(x, beats, depth=0.5, rel=0.18):
    g = np.ones(N)
    t = np.arange(N) / SR
    for k in beats:
        d = t - at(k)
        m = (d >= 0) & (d < 0.4)
        g[m] = np.minimum(g[m], 1 - depth * np.exp(-d[m] / rel))
    return x * g[:, None]


kick_beats = list(range(4, 34))
mix = (drums.x * 0.9
       + sidechain(bass.x, kick_beats, 0.45) * 0.9
       + sidechain(music.x, kick_beats, 0.3) * 0.8
       + sfx.x * 0.85
       + reverb(verb_send.x) * 0.55)
mix = hp(mix.T, 25).T

# master fade with the picture
t = np.arange(N) / SR
fade = np.clip((19.98 - t) / (19.98 - 19.3), 0, 1) ** 1.5
fade[t < 19.3] = 1
mix *= fade[:, None]


def limiter(x, ceiling=10 ** (-1.7 / 20), look=0.002, rel=0.08):
    la = int(look * SR)
    peak = np.max(np.abs(x), 1)
    # running max over the look-ahead window
    from scipy.ndimage import maximum_filter1d
    pk = maximum_filter1d(peak, size=2 * la + 1)
    g_target = np.minimum(1.0, ceiling / np.maximum(pk, 1e-9))
    g = np.empty_like(g_target)
    a = np.exp(-1 / (rel * SR))
    cur = 1.0
    for i in range(len(g_target)):
        cur = g_target[i] if g_target[i] < cur else a * cur + (1 - a) * g_target[i]
        g[i] = cur
    return x * g[:, None]


meter = pyln.Meter(SR)
for _ in range(5):
    lufs = meter.integrated_loudness(mix)
    mix *= 10 ** ((-14.0 - lufs) / 20)
    mix = limiter(mix)
# true-peak safety (4x oversampled)
tp = np.max(np.abs(signal.resample_poly(mix, 4, 1, axis=0)))
if tp > 10 ** (-1.0 / 20):
    mix *= 10 ** (-1.0 / 20) / tp
lufs = meter.integrated_loudness(mix)
tp = np.max(np.abs(signal.resample_poly(mix, 4, 1, axis=0)))

out = sys.argv[1] if len(sys.argv) > 1 else 'out/audio.wav'
wavfile.write(out, SR, (np.clip(mix, -1, 1) * 32767).astype(np.int16))
with open(out.replace('.wav', '_cues.txt'), 'w') as f:
    f.write(f'# integrated loudness {lufs:.2f} LUFS, true peak {20 * np.log10(tp):.2f} dBTP, beat = {B:.4f} s\n')
    for tc, nm in sorted(CUES):
        f.write(f'{tc:7.3f} s  beat {tc / B:6.2f}  {nm}\n')
print(f'wrote {out}: {lufs:.2f} LUFS integrated, true peak {20 * np.log10(tp):.2f} dBTP')
