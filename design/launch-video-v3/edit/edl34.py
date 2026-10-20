"""NimbleMap launch film v3.4: v3.3's one take, every shot re-recorded with the fixed build
(/Volumes/LaCie/lm-video-build-f0f19e3, phase-4 f0f19e3: rasters repeat across world copies),
holding after every move until the basemap is sharp; the owner's opening lines; Barcelona,
16 Aug 2022 for the heat beat (a 2022 GDO file in the folder); one spatial Pro question about
Tokyo. About 75 s.

The picture reuses v3.3's compositor on a warped clock (edit/compose34.py): v3.4 time t maps to
v3.3 time by WARP below (two short holds and the new Pro section), and v3.3's literal source
times are replaced by the v3.4 takes. This module gives the frame cache its frames and the sound
its beats, in v3.4 time.

Takes (raw/*.mov; tours/*.tour):
  g4_tyA    storms: world; zoom (-80,24,z4.6); pan (-62,33,z4.6); New York City landing; long holds
  g4_tyB    storms: the layer's Find field, "Sandy" (2 found); Sandy (2012) clicked; regional view
  ty_plate4 the empty world view (aligned onto g4_tyA by (5, 1) px)
  g4_eq     earthquakes (light, by magnitude): Pacific; Tokyo landing; Tohoku clicked; Japan view
  eq_plate4 the empty Pacific view (aligned by (-2, -1) px)
  g4_hw     heatwaves 2022: slider 1 Jun -> 16 Aug (step 76); Barcelona landing; cell clicked
  f4_tif, f4_csv   the GeoTIFF and the volcano CSV
  p4_ty     Pro: "How many Category 5 storms have passed within 100 km of Tokyo?" (1 paid call;
            a second take, p4b_ty, framed Tokyo off-screen and is unused; 2 paid calls in all)
"""
FPS = 30
TITLE = 64

# v3.4 time -> v3.3 time: (t34 start, t33 start, slope)
F1 = 11.8                       # hold while the GeoTIFF window is open (the longer "open" line)
H1 = 1.4
F2 = 44.2                       # hold under "36.10 C" (the longer heat line)
H2 = 0.4
PRO0_33, PRO1_33 = 46.2, 59.0   # v3.3's Pro section, replaced
PRO0 = PRO0_33 + H1 + H2        # 48.0: the native Pro section (v3.4 time) starts
PRO1 = 64.570                   # ...and ends (the hit then lands on the drop grid)
WARP = [(0.0, 0.0, 1.0), (F1, F1, 0.0), (F1 + H1, F1, 1.0), (F2 + H1, F2, 0.0), (F2 + H1 + H2, F2, 1.0),
        (PRO0, None, None), (PRO1, PRO1_33, 1.0)]


def t33(t):
    """v3.3 time for v3.4 time t (None inside the native Pro section)."""
    seg = WARP[0]
    for s in WARP:
        if t >= s[0]:
            seg = s
    if seg[1] is None:
        return None
    return seg[1] + (t - seg[0]) * seg[2]


def t34(x):
    """v3.4 time for v3.3 time x (outside the replaced Pro section)."""
    if x < F1:
        return x
    if x < F2:
        return x + H1
    if x < PRO1_33:
        return x + H1 + H2
    return x - PRO1_33 + PRO1


DUR = round(t34(69.0), 3)

# Heatwave take: `time k` lands at 17.07 + 2.5362 (k - 1) (k >= 1; 76 = 16 Aug 2022); a frame from
# the middle of each step.
HW_LAST = 76


def hw_step(k):
    return 12.0 if k <= 0 else round(17.07 + 2.5362 * (k - 1) + 1.2, 3)


NEEDED = {
    'g4_tyA': [12.0, (15.50, 15.80, 60), 17.4, (29.18, 29.45, 60), 30.9, (42.87, 43.60, 60), 45.8],
    'g4_tyB': [10.5, (11.90, 13.40, 30), 13.6, 45.0],
    'ty_plate4': [9.0],
    'g4_eq': [12.0, (20.63, 21.00, 60), 30.0, 60.0],
    'eq_plate4': [9.0],
    'g4_hw': [hw_step(k) for k in range(HW_LAST + 1)] + [(212.30, 212.75, 60), 224.0, (228.55, 229.55, 30), 242.0],
    'f4_tif': [9.0], 'f4_csv': [10.0],
    'p4_ty': [12.0, (18.05, 22.55, 30), 30.0],
}
SOURCES = {
    'g4_tyA': {'crop': (0, 64, 2814, 1690)}, 'g4_tyB': {'crop': (0, 64, 2818, 1690)},
    'ty_plate4': {'crop': (0, 64, 2802, 1682), 'shift': (5.0, 1.0)},
    'g4_eq': {'crop': (0, 64, 2814, 1690)}, 'eq_plate4': {'crop': (0, 64, 2818, 1690), 'shift': (-2.0, -1.0)},
    'g4_hw': {'crop': (0, 64, 2818, 1690)}, 'f4_tif': {'crop': (0, 64, 2818, 1690)},
    'f4_csv': {'crop': (0, 64, 2822, 1694)}, 'p4_ty': {'crop': (0, 64, 2544, 1592)},
}

# ---------------------------------------------------------------- beats in v3.4 time (for the sound)
T_LIFT_TIF = 10.15
T_OPEN_TIF = 11.00
T_OPEN_CSV = t34(12.30)
T_LIFT1 = t34(12.45)
T_OPEN1 = t34(13.00)
T_SANDY = t34(24.30)
T_TRAVEL = (t34(25.20), t34(25.60))
T_OPEN2 = (t34(26.10), t34(26.70))
T_RING = (t34(26.70), t34(28.60))
T_JAPAN = t34(30.70)
T_M91 = t34(32.45)
T_DROP = t34(36.30)
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

# Typed (v3.4 time): the place names as in v3.3; the Pro question in the native section
ASK_TEXT4 = 'How many Category 5 storms have passed within 100 km of Tokyo?'
TYPE4 = (ASK_TEXT4, [PRO0 + 3.55 + i * 0.068 for i in range(len(ASK_TEXT4))])
_k = lambda xs: [t34(x) for x in xs]  # noqa: E731
TYPE1 = ('New York', _k([18.70, 18.79, 18.87, 18.97, 19.06, 19.14, 19.23, 19.33]))
TYPE_FIND = ('Sandy', _k([21.40, 21.52, 21.63, 21.74, 21.84]))
TYPE2 = ('Tokyo', _k([29.30, 29.41, 29.52, 29.63, 29.73]))
TYPE3 = ('Barcelona', _k([40.22, 40.29, 40.36, 40.43, 40.50, 40.57, 40.64, 40.71, 40.78]))

# Narration (edge-tts en-US-AndrewNeural, +4%): (id, start of its first word, v3.4 s)
VO = [('folder', 0.50), ('open', 9.95), ('storms_a', t34(13.40)), ('storms_b', t34(18.30)),
      ('storms_c', t34(20.10)), ('quakes_a', t34(25.60)), ('quakes_b', t34(29.40)), ('heat_a', t34(36.60)),
      ('heat_b', t34(40.30)), ('pro_a', PRO0 + 0.10), ('pro_b', PRO0 + 3.45), ('pro_c', PRO0 + 8.65),
      ('end', t34(63.85))]
VO_SCRIPT, VO_DIR = 'narration34.txt', 'vo_edge34'
T_ENTER = T_OPEN1


def key_times():
    ks = TYPE1[1] + TYPE_FIND[1] + TYPE2[1] + TYPE3[1] + TYPE4[1][::2]
    return sorted(ks), [T_ENTER1, t34(21.90), T_ENTER2, T_ENTER3, T_SEND]
