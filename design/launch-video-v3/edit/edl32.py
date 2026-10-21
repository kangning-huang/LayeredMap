"""NimbleMap launch film v3.2: v3.1's story, polished and narrated, with a Pro beat.

Everything not overridden here is v3.1's (edit/edl31.py): sources, measured positions, the folder.
New: the Pro take `p_ty` (the storm shapefile opened in NimbleMap Pro; one plain-words request in
the Ask box, one paid call), the narration (audio/narration.txt, rendered by audio/voice.sh), and a
timeline stretched to 45 s so the voice is unhurried. Music sync points move with the beats.
"""
from edl31 import *  # noqa: F401,F403
import edl31 as _b

DUR = 45.0

# The earthquake film is the light retake (g_eqL: light theme and basemap, like the other two films;
# its tour starts 2.5 s earlier than g_eq's, having no `theme dark` step), over a light plate.
SOURCES = dict(_b.SOURCES, p_ty={'crop': (0, 64, 2552, 1596)}, g_eqL={'crop': (0, 64, 2822, 1694)},
               eq_plateL={'crop': (0, 64, 2818, 1690)})
for _k in ('g_eq', 'eq_plate'):
    SOURCES.pop(_k)
NEEDED = dict(_b.NEEDED, p_ty=[12.0, (12.95, 16.25, 30), 21.0],
              g_eqL=[12.5, (15.25, 15.90, 60), (15.90, 17.90, 12), 18.5, 40.0], eq_plateL=[8.0])
for _k in ('g_eq', 'eq_plate'):
    NEEDED.pop(_k)
BAR = dict(_b.BAR, g_eqL=_b._light)
EQ_MAG = (2102, 410, 2776, 590)           # (the light window is 4 px wider: the card sits 4 px right)
TOHOKU_PX = (1519, 878)
EQ_RING_CENTRE = (1523, 1012)
EQ_RING_START = (1166, 740)

# The Ask box in the Pro take (raw px): Inter Regular fitted to the placeholder "Ask about your
# layers" (282 px wide -> 27.7 px), its start and baseline, the area cleared, the box's fill.
ASK = {'x': 1028, 'base': 1555, 'clear': (1024, 1528, 1990, 1568), 'size': 27.7, 'bg': (253, 253, 253),
       'ink': (29, 29, 31)}
ASK_TEXT = 'Show only Category 5 storms'
PRO_CARD = (980, 1150, 2170, 1415)          # the answer: "... where strength is Category 5: 228 of 4,999"
PW, PH = 2552, 1660

# ---------------------------------------------------------------- beats (s)
T_SELECT = 4.20
T_LIFT1 = 4.40
T_OPEN1 = 5.40           # the .shp lands: the window opens; the music's first downbeat
T_REVEAL1 = (6.05, 6.95)
T_ENTER1 = 10.75
T_SANDY = 12.05
T_TRAVEL = (14.90, 15.30)
T_OPEN2 = (16.15, 16.80)
T_RING = (16.80, 19.00)
T_ENTER2 = 19.75
T_JAPAN = 20.40
T_M91 = 21.30
T_DROP = 22.75
T_FILM3 = 23.10
T_ENTER3 = 26.40
T_PALERMO = 27.10
T_TEMP = 27.55
T_PRO = (29.70, 30.70)   # travel to the Pro window; it opens out of the .shp row
T_SEND = 32.10
T_GATHER = (35.30, 37.90)
T_STOP = 38.925          # one beat before the drop grid's bar 9 (one silent beat)
T_PEN = (39.446, 40.55)  # the hit, on that bar
T_END = 40.65

TYPE1 = ('New York', [9.80, 9.91, 10.00, 10.13, 10.25, 10.35, 10.46, 10.58])
TYPE2 = ('Tokyo', [19.20, 19.31, 19.42, 19.53, 19.63])
TYPE3 = ('Palermo', [25.80, 25.88, 25.97, 26.05, 26.14, 26.22, 26.30])
TYPE4 = (ASK_TEXT, [30.95 + i * 0.042 for i in range(len(ASK_TEXT))])

# Narration: (id, start s). Texts and captions: audio/narration.txt.
# (timed for edge-tts en-US-AndrewNeural at +4%; the first word of each line starts here)
VO = [('folder', 0.80), ('open', 5.50), ('storms_a', 6.90), ('storms_b', 10.70), ('quakes_a', 16.10),
      ('quakes_b', 19.75), ('heat_a', 23.40), ('heat_b', 26.00), ('pro', 30.85), ('end', 39.85)]

T_ENTER = T_OPEN1


def key_times():
    ks = TYPE1[1] + TYPE2[1] + TYPE3[1] + TYPE4[1][::2]
    return sorted(ks), [T_ENTER1, T_ENTER2, T_ENTER3, T_SEND]
