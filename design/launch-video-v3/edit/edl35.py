"""NimbleMap launch film v3.5: v3.4 re-cut on the v3.5 takes (fixed build; 20-31 s holds after
every move). Each move is played sped up from its start to the first frame that matches the fully
loaded end of its hold (capture/loaded.py), then that reference frame is held. Regional framing:
New York z6.5, Tokyo z6.4, Barcelona z5.5. The Tokyo Pro beat is a slot (to be recaptured with the
askfix build).

The picture runs v3.3's compositor on a warped clock: KNOTS map v3.3 time to v3.5 time (holds, and
three half-speed stretches where the app's own moves play), with the native Pro section between
PRO0 and PRO1. The hit lands 16 bars (115 BPM) after the drop.
"""
FPS = 30
TITLE = 64
BAR = 4 * 60 / 115.0

# (v3.3 time, v3.5 time) knots; a repeated v3.3 time is a hold
KNOTS = [(0.0, 0.0), (11.8, 11.8), (11.8, 13.2),            # GeoTIFF window open (longer "open" line)
         (19.5, 20.9), (20.85, 23.6),                      # New York: flight, landing, zoom out (half speed)
         (29.85, 32.6), (30.70, 34.3),                     # Tokyo: flight, landing, zoom out (half speed)
         (40.85, 44.4), (41.65, 46.0),                     # Barcelona: flight, landing, zoom out (half speed)
         (44.2, 48.55), (44.2, 48.95), (46.2, 50.95)]       # hold under "36.10 C"; Pro starts
PRO0 = 50.95
T_DROP = 36.3 + (39.05 - 36.3) if False else None        # set below
_D33 = 36.3


def _t34_pre(x):
    for (a, ta), (b, tb) in zip(KNOTS, KNOTS[1:]):
        if a <= x <= b and b > a:
            return ta + (x - a) * (tb - ta) / (b - a)
    return None


T_DROP = _t34_pre(_D33)
T_HIT = T_DROP + 16 * BAR
PRO1 = T_HIT - (63.434 - 59.0)
KNOTS_AFTER = [(59.0, PRO1), (69.0, PRO1 + 10.0)]


def t33(t):
    """v3.3 time for v3.5 time t (None inside the native Pro section)."""
    if PRO0 <= t < PRO1:
        return None
    if t >= PRO1:
        return 59.0 + (t - PRO1)
    last = None
    for (a, ta), (b, tb) in zip(KNOTS, KNOTS[1:]):
        if ta <= t <= tb:
            last = a if tb == ta else a + (t - ta) * (b - a) / (tb - ta)
    return last


def t34(x):
    if x >= 59.0:
        return PRO1 + (x - 59.0)
    best = None
    for (a, ta), (b, tb) in zip(KNOTS, KNOTS[1:]):
        if a <= x <= b and b > a:
            best = ta + (x - a) * (tb - ta) / (b - a)
    return best


DUR = round(t34(69.0), 3)

HW_LAST = 76


def hw_step(k):
    return 12.0 if k <= 0 else round(20.33 + 2.5398 * (k - 1) + 1.2, 3)


# loaded-reference frames (capture/loaded.py) and the move windows played
NEEDED = {
    'g5_tyA': [19.5, (20.0, 21.85, 60), 50.83, (51.33, 52.45, 60), 82.37, (82.87, 85.85, 30), 113.87,
               (114.37, 117.55, 30), 145.4],
    'g5_tyB': [9.5, (11.73, 15.73, 30), 29.0, 91.97],
    'ty_plate4': [9.0],
    'g5_eq': [31.73, (32.23, 36.73, 30), 63.27, (63.77, 65.53, 30), 94.77, 152.63],
    'eq_plate4': [9.0],
    'g5_hw': [hw_step(k) for k in range(HW_LAST + 1)] + [(218.53, 220.83, 30), 244.47, (244.97, 246.03, 30), 276.0, 303.9],
    'f4_tif': [9.0], 'f4_csv': [10.0],
    'p6_ty': [28.0, (30.6, 34.0, 30), (34.07, 35.10, 60), 136.87],
}
SOURCES = {
    'g5_tyA': {'crop': (0, 64, 2818, 1690)}, 'g5_tyB': {'crop': (0, 64, 2822, 1694)},
    'ty_plate4': {'crop': (0, 64, 2802, 1682), 'shift': (6.0, 1.0)},
    'g5_eq': {'crop': (0, 64, 2818, 1690)}, 'eq_plate4': {'crop': (0, 64, 2818, 1690)},
    'g5_hw': {'crop': (0, 64, 2818, 1690)}, 'f4_tif': {'crop': (0, 64, 2818, 1690)},
    'f4_csv': {'crop': (0, 64, 2822, 1694)}, 'p6_ty': {'crop': (0, 64, 2822, 1694)},
}

# beats (v3.5 time), for the sound
T_LIFT_TIF, T_OPEN_TIF = 10.15, 11.00
T_OPEN_CSV, T_LIFT1, T_OPEN1 = t34(12.30), t34(12.45), t34(13.00)
T_SANDY = t34(24.30)
T_TRAVEL = (t34(25.20), t34(25.60))
T_OPEN2 = (t34(26.10), t34(26.70))
T_RING = (t34(26.70), t34(28.60))
T_JAPAN = t34(30.70)
T_M91 = t34(32.45)
T_FILM3 = t34(36.65)
T_TEMP_BOOM = t34(42.82)
T_PRO = (t34(45.20), t34(46.20))
T_PRO2 = (1e9, 1e9)
T_SEND = PRO0 + 8.05
T_SEND2 = 1e9
T_GATHER = (t34(59.30), t34(61.90))
T_STOP = t34(62.912)
T_PEN = (t34(63.434), t34(64.53))
T_END = t34(64.63)
T_ENTER1, T_ENTER2, T_ENTER3 = t34(19.50), t34(29.85), t34(40.85)

ASK_TEXT4 = 'How many Category 5 storms have passed within 100 km of Tokyo?'
TYPE4 = (ASK_TEXT4, [PRO0 + 3.55 + i * 0.068 for i in range(len(ASK_TEXT4))])
_k = lambda xs: [t34(x) for x in xs]  # noqa: E731
TYPE1 = ('New York', _k([18.70, 18.79, 18.87, 18.97, 19.06, 19.14, 19.23, 19.33]))
TYPE_FIND = ('Sandy', _k([21.40, 21.52, 21.63, 21.74, 21.84]))
TYPE2 = ('Tokyo', _k([29.30, 29.41, 29.52, 29.63, 29.73]))
TYPE3 = ('Barcelona', _k([40.22, 40.29, 40.36, 40.43, 40.50, 40.57, 40.64, 40.71, 40.78]))

# narration (v3.4's lines and voice files), first-word starts in v3.5 time
VO = [('folder', 0.50), ('open', 9.95), ('storms_a', t34(13.40)), ('storms_b', t34(18.30)),
      ('storms_c', t34(20.85) - 0.3), ('quakes_a', t34(25.60)), ('quakes_b', t34(29.40)), ('heat_a', t34(36.60)),
      ('heat_b', t34(40.30)), ('pro_a', PRO0 + 0.10), ('pro_b', PRO0 + 3.45), ('pro_c', PRO0 + 8.65),
      ('end', t34(63.85))]
VO_SCRIPT, VO_DIR = 'narration34.txt', 'vo_edge34'
T_ENTER = T_OPEN1


def key_times():
    ks = TYPE1[1] + TYPE_FIND[1] + TYPE2[1] + TYPE3[1] + TYPE4[1][::2]
    return sorted(ks), [T_ENTER1, t34(21.90), T_ENTER2, T_ENTER3, T_SEND]
