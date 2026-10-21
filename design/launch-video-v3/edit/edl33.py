"""NimbleMap launch film v3.3: the richer script (audio/narration33.txt), about 69 s.

Builds on v3.2 (edit/edl32.py) and v3.1 (edit/edl31.py). New takes (raw/*.mov, release build,
window-only, driving itself with --tour; files opened as file arguments, light theme):
  g3_ty   the storm shapefile: world view; a real zoom (-80,24,z4.6) and pan (-62,33,z4.6);
          the place search's landing for "New York" (-74.006,40.714,z10.055)
  g3_tyB  the storm shapefile again: the layer's Find field, "Sandy" -> "2 found" (both storms
          named Sandy selected and listed, the map zooms to both); Sandy (2012) clicked; a regional
          view around New York with its card (the app had zoomed back to the layer on opening)
  f_tif, f_csv   the GeoTIFF and the volcano CSV opened (from v3.1's format checks)
  p_ty    Pro: "Show only Category 5 storms" (v3.2's capture, reused)
  p_eq    Pro: "Which earthquakes were deeper than 300 km?" on the earthquake GeoPackage (1 paid call)
  g_eqL, eq_plateL, g_hw, ty_plate   as in v3.2
"""
from edl32 import *  # noqa: F401,F403
import edl31 as _b
import edl32 as _c

DUR = 69.0

SOURCES = dict(_c.SOURCES,
               g3_ty={'crop': (0, 64, 2818, 1690)}, g3_tyB={'crop': (0, 64, 2818, 1690)},
               f_tif={'crop': (0, 64, 2822, 1694)}, f_csv={'crop': (0, 64, 2822, 1694)},
               p_eq={'crop': (0, 64, 2560, 1600)})
for _k in ('g_ty',):
    SOURCES.pop(_k, None)
NEEDED = dict(_c.NEEDED,
              g3_ty=[11.0, (12.80, 13.20, 60), 14.0, 16.0, (18.80, 19.10, 60), 22.0, (24.85, 25.60, 60),
                     (25.60, 27.20, 12), 30.0],
              g3_tyB=[5.0, (6.90, 8.60, 30), 10.0, 30.0],
              f_tif=[7.0], f_csv=[7.0],
              p_eq=[12.0, (15.55, 17.45, 30), 22.0])
NEEDED.pop('g_ty', None)

_l2818 = {'rect': (788, 84, 1743, 148), 'x': 849.5, 'base': 124, 'fill': (245, 245, 247),
          'ink': (29, 29, 31), 'hint': (90, 95, 103)}
BAR = dict(_c.BAR, g3_ty=_l2818, g3_tyB=_l2818)

# The layer's Find field in g3_tyB (raw px): Inter Regular 27.5 px, text start and baseline, the
# area cleared while the typing is drawn, its fill and ink.
FIND = {'x': 2168, 'base': 411, 'clear': (2160, 379, 2756, 429), 'size': 27.5, 'bg': (245, 245, 247),
        'ink': (23, 23, 25)}
SANDY_PX = (1481, 975)
ASK2 = {'x': 1032, 'base': 1559, 'clear': (1028, 1530, 1996, 1572), 'size': 27.7, 'bg': (253, 253, 253),
        'ink': (29, 29, 31)}
ASK2_TEXT = 'Which earthquakes were deeper than 300 km?'
PRO2_CARD = (952, 1150, 2191, 1420)
PW2, PH2 = 2560, 1664

ROW_TIF, ROW_CSV = 0, 7
ROWS_SHP = [2, 3, 4, 5, 6]

# ---------------------------------------------------------------- the table (overrides)
FTL = (TL1[0] - FOLDER_W * FSCALE - 2600.0, -FOLDER_H * FSCALE / 2)
MS = 0.55                                         # the GeoTIFF and CSV windows, drawn at 0.55
TIF_TL = (TL1[0] - 2050.0, -879.0)
CSV_TL = (TL1[0] - 2050.0, -879.0 + 1758 * MS + 120)

# ---------------------------------------------------------------- beats (s)
PULSES = [(0.58, ROWS_SHP), (1.63, [8]), (2.93, [1]), (4.09, [0])]   # as each type is named
T_FORMATS = (4.8, 9.4)
T_SELECT = 9.40          # the .tif row is selected, lifts, flies; its window opens
T_LIFT_TIF = 9.65
T_OPEN_TIF = 10.50
T_OPEN_CSV = 12.30
T_OPEN1 = 13.00          # the .shp lands: the storm window opens; the music's first downbeat
T_LIFT1 = 12.45          # (the .shp row comes in from the left)
T_REVEAL1 = (13.60, 14.50)
T_ZOOM = 16.55           # real flights: zoom (source 12.85), pan (source 18.87)
T_PAN = 17.45
T_ENTER1 = 19.50
T_FIND = (20.35, 20.85)  # push to the layer's Find field (crossfade to the Find take)
T_FINDKEY = 21.95        # the real result ("2 found"), then the zoom to both
T_SANDY = 24.30
T_TRAVEL = (25.20, 25.60)
T_OPEN2 = (26.10, 26.70)
T_RING = (26.70, 28.60)
T_ENTER2 = 29.85
T_JAPAN = 30.70
T_M91 = 32.45
T_DROP = 36.30
T_FILM3 = 36.65
T_ENTER3 = 40.85
T_PALERMO = 41.65
T_TEMP = 42.10
T_PRO = (45.20, 46.20)
T_SEND = 50.30
T_PRO2 = (53.70, 54.60)
T_SEND2 = 56.40
T_GATHER = (59.30, 61.90)
T_STOP = 62.912          # one beat before the hit's bar (13 bars after the drop at 115 BPM)
T_PEN = (63.434, 64.53)
T_END = 64.63

TYPE1 = ('New York', [18.70, 18.79, 18.87, 18.97, 19.06, 19.14, 19.23, 19.33])
TYPE_FIND = ('Sandy', [21.40, 21.52, 21.63, 21.74, 21.84])
TYPE2 = ('Tokyo', [29.30, 29.41, 29.52, 29.63, 29.73])
TYPE3 = ('Palermo', [40.30, 40.38, 40.47, 40.55, 40.64, 40.72, 40.80])
TYPE4 = (ASK_TEXT, [49.00 + i * 0.044 for i in range(len(ASK_TEXT))])
TYPE5 = (ASK2_TEXT, [54.70 + i * 0.037 for i in range(len(ASK2_TEXT))])

# Narration (edge-tts en-US-AndrewNeural, +4%): (id, start of its first word, s)
VO = [('folder', 0.50), ('open', 9.90), ('storms_a', 13.40), ('storms_b', 18.30), ('storms_c', 20.10),
      ('quakes_a', 25.60), ('quakes_b', 29.40), ('heat_a', 36.60), ('heat_b', 40.30), ('pro_a', 45.50),
      ('pro_b', 48.90), ('pro_c', 54.00), ('end', 63.85)]
VO_SCRIPT, VO_DIR = 'narration33.txt', 'vo_edge33'

T_ENTER = T_OPEN1


def key_times():
    ks = TYPE1[1] + TYPE_FIND[1] + TYPE2[1] + TYPE3[1] + TYPE4[1][::2] + TYPE5[1][::2]
    return sorted(ks), [T_ENTER1, T_FINDKEY - 0.05, T_ENTER2, T_ENTER3, T_SEND, T_SEND2]
