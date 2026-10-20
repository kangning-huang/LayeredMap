"""NimbleMap launch film v3, "one take": the timeline, the layout of the atlas table, and every
measured position in the app footage.

Units
  World units are raw pixels of the window recordings (2x Retina): a film window is about
  2814 x 1754 world units, title bar included. Camera zoom Z is output px per world unit; the
  beat sheet's "N x" is N * X1, where X1 = 1920 / 2814 (1x = the window just fills the frame
  width). Times are seconds on the film's timeline.

Sources (raw/*.mov, window-only recordings of the release build driving itself with --tour)
  ty        storms example (light): world view, dive to Leyte, click Haiyan, pull out to its track
  ty_plate  the same world view with nothing open (the plate the tracks draw in over)
  eq        earthquakes example (dark, coloured by magnitude): Pacific view, Tohoku clicked
  eq_plate  the same dark Pacific view with nothing open
  hw        heatwaves example (light): North Atlantic, slider stepped 1 Jun -> 16 Jul, cell clicked
"""

DUR = 31.0
FPS = 30
X1 = 1920 / 2814

# Content crops (x, y, w, h) of the raw frames: the native title bar (64 px) is cut away and a
# clean one is drawn in its place, so content y + TITLE = raw y (positions below are raw).
TITLE = 64
SOURCES = {
    'ty': {'crop': (0, 64, 2814, 1686)},
    'ty_plate': {'crop': (0, 64, 2818, 1690), 'shift': (-3.97, -1.43)},   # aligned onto ty (phase correlation)
    'eq': {'crop': (0, 65, 2814, 1689)},
    'eq_plate': {'crop': (0, 65, 2814, 1689)},
    'hw': {'crop': (0, 64, 2818, 1690)},
}

# Heatwave take: `time k` lands at HW_T0 + HW_DT * k (measured from the recording), the cell
# click at HW_CLICK. A frame from the middle of each step is used.
HW_T0, HW_DT, HW_CLICK = 12.92, 2.5505, 130.233


def hw_step(k):
    return round(HW_T0 + HW_DT * k + 1.25, 3)


# Frames read from the recordings: single times, or (start, end, fps) runs.
NEEDED = {
    'ty': [12.5, (15.40, 16.30, 60), (16.30, 21.30, 12), (21.30, 22.70, 60), 31.0],
    'ty_plate': [8.0],
    'eq': [18.0, 36.0],
    'eq_plate': [10.0],
    'hw': [hw_step(k) for k in range(46)] + [(130.15, 130.95, 30), 133.0],
}

# ---------------------------------------------------------------- measured app positions (raw px)
# The search bar: fill rect, its placeholder text start and baseline, font size (Inter Regular,
# fitted to the placeholder's width: 350 px for "Search places or your data" -> 27.5 px).
BAR = {
    'ty': {'rect': (784, 84, 1741, 148), 'x': 847.5, 'base': 125, 'fill': (245, 245, 247),
           'ink': (29, 29, 31), 'hint': (90, 95, 103)},
    'eq': {'rect': (784, 84, 1743, 148), 'x': 848.5, 'base': 125, 'fill': (41, 41, 46),
           'ink': (236, 236, 240), 'hint': (176, 180, 187)},
    'hw': {'rect': (788, 84, 1743, 148), 'x': 849.5, 'base': 124, 'fill': (245, 245, 247),
           'ink': (29, 29, 31), 'hint': (90, 95, 103)},
}
BAR_FONT = 27.5
BAR_RADIUS = 12

# Haiyan click (view 126,11,z7.5; click 124.53,11.05), Tohoku dot (view 175,12,z2.4), heat cell
# (view -28,32,z3.4; click -15,28): Web Mercator from the view, checked against the frames.
HAIYAN_PX = (1029, 948)
TOHOKU_PX = (1162, 738)
HEAT_PX = (1604, 1030)

# Cards (raw px): the row blocks lifted out of them.
TY_CARD = (2070, 195, 2792, 1255)
TY_WIND_ROW = (2096, 548, 2770, 600)        # "max_wind_kt 170"
TY_WIND_RANK = (2096, 618, 2770, 700)       # "higher than 99% of 4,547 storms with values" + strip
EQ_CARD = (2073, 192, 2797, 1215)
EQ_MAG = (2098, 410, 2770, 590)             # "magnitude 9.10" + "higher than 99% of 14,537 earthquakes"
HW_CARD = (2076, 195, 2798, 900)
HW_VALUE = (2100, 345, 2780, 530)           # "50.55 C" + "16 Jul 2023" + "higher than 96% ..."
# Time slider (before the card opens): its panel and track; after the card opens it moves left.
HW_SLIDER_PANEL = (1078, 1587, 2323, 1685)
HW_TRACK = (1335, 1632, 1962)               # x0, y, x1
# Shelf + toast regions that always come from the take, never the plate (raw px, generous).
TY_KEEP = [(0, 64, 620, 380), (1040, 1410, 1770, 1700)]
EQ_KEEP = [(0, 64, 620, 380), (1040, 1410, 1770, 1700)]

# ---------------------------------------------------------------- the atlas table (world layout)
W = 2814
H = 1754
TL1 = (-1407.0, -877.0)                      # film 1 (typhoons) window, top-left
TL2 = (2300.0, -677.0)                       # film 2 (earthquakes)
BAR3_SCALE = 3.0                             # bar 3 is a free pill on the table, drawn 3x
BAR3_C = (TL2[0] + 1407.0, TL2[1] + H + 340.0)
TL3 = (TL2[0] + W + 700.0, BAR3_C[1] - HW_TRACK[1])   # film 3: its slider track sits on bar 3's line


def bar_world(tl, src):
    x0, y0, x1, y1 = BAR[src]['rect']
    return (tl[0] + x0, tl[1] + y0, tl[0] + x1, tl[1] + y1)


# ---------------------------------------------------------------- beats (s)
T_ENTER = 4.20
T_FILM1 = 4.90
T_LIFT = 9.40
T_TRAVEL = (10.80, 11.20)
T_TYPE2 = (11.20, 12.00)
T_OPEN2 = (12.05, 12.70)
T_RING = (12.70, 15.30)
T_TOHOKU = 15.30
T_M91 = 16.00
T_DROP = 16.90
T_FILM3 = 17.25
T_LOOP = 21.10
T_GATHER = (23.30, 25.90)
T_STOP = 26.80
T_PEN = (27.30, 28.40)
T_END = 28.50

# Typed text: (text, [key times]) - a human rhythm, two holds and a glide around bar 1.
TYPE1 = ('Haiyan', [1.05, 1.27, 1.43, 1.71, 1.90, 2.16])
TYPE2 = ('Tohoku', [11.32, 11.43, 11.55, 11.66, 11.78, 11.88])
TYPE3 = ('heat', [16.38, 16.50, 16.61, 16.72])


def key_times():
    """Every key click on the timeline (for the sound): letters, and the Enter presses."""
    ks = TYPE1[1] + TYPE2[1] + TYPE3[1]
    return sorted(ks), [T_ENTER, 12.02, 16.86]
