"""NimbleMap launch film v3.1: "open any geospatial data you find online".

The same one-take style as v3 (edit/edl.py), with a new story: it opens on a folder of real
downloaded files (Shapefile, GeoPackage, NetCDF, GeoTIFF, CSV), and one file at a time goes into
NimbleMap and becomes its film. The search bar searches well-known places only (New York, Tokyo,
Palermo), exactly as the app's place search lands.

Units as in edl.py: world units = raw px of the recordings (2x); "N x" zoom = N * X1.

Sources (raw/*.mov): the app opened each converted file as a command-line file argument (the path
Open With and drag-and-drop take), driving itself with --tour:
  g_ty      IBTrACS_storm_tracks_1980-2026.shp: world view; New York City (the search's landing);
            Sandy (2012) clicked; a regional view around New York with its card
  ty_plate  the same world view with nothing open (from v3)
  g_eq      usgs_earthquakes_M6plus_1900-2026.gpkg (dark, by magnitude): Pacific; Tokyo (landing);
            Tohoku clicked; Japan with its card
  eq_plate  the same dark Pacific view with nothing open (from v3)
  g_hw      GDO_heatwave_intensity_JJA_2023.nc: North Atlantic, slider 1 Jun -> 25 Jul; Palermo
            (landing); its cell clicked; a regional view with the card
  finder    a window-only capture of the real Finder window of the folder (reference for the
            re-layout; not shown, because it carries the owner's sidebar and app-specific icons)
"""

DUR = 31.0
FPS = 30
X1 = 1920 / 2814
TITLE = 64

SOURCES = {
    'g_ty': {'crop': (0, 64, 2822, 1694)},
    'ty_plate': {'crop': (0, 64, 2818, 1690)},                              # aligned (0, 0)
    'g_eq': {'crop': (0, 65, 2818, 1689)},
    'eq_plate': {'crop': (0, 65, 2814, 1689), 'shift': (1.6, 1.2)},       # aligned onto g_eq
    'g_hw': {'crop': (0, 64, 2822, 1694)},
}


# Heatwave take: `time k` (k >= 1) lands at 12.78 + 2.5306 (k - 1) (date label changes measured in
# the recording); a frame from the middle of each step is used. Step 54 = 25 Jul 2023.
def hw_step(k):
    if k <= 0:
        return 11.0
    return round(12.78 + 2.5306 * (k - 1) + 1.2, 3)


HW_LAST = 54

NEEDED = {
    'g_ty': [11.0, (12.70, 13.40, 60), (13.40, 15.60, 12), 17.0, 40.0],
    'ty_plate': [8.0],
    'g_eq': [15.0, (17.95, 18.60, 60), (18.60, 20.60, 12), 21.0, 40.0],
    'eq_plate': [10.0],
    'g_hw': [hw_step(k) for k in range(HW_LAST + 1)] + [(150.15, 152.65, 30), 156.0, (157.95, 158.65, 30), 168.0],
}

# The search bar in all three takes (the windows were 2822/2818 px wide): its fill rect, the
# placeholder's text start and baseline; Inter Regular 27.5 px.
_light = {'rect': (786, 84, 1744, 148), 'x': 849.5, 'base': 125, 'fill': (245, 245, 247),
          'ink': (29, 29, 31), 'hint': (90, 95, 103)}
BAR = {'g_ty': _light, 'g_hw': _light,
       'g_eq': dict(_light, fill=(41, 41, 46), ink=(236, 236, 240), hint=(176, 180, 187))}
BAR_FONT = 27.5
BAR_RADIUS = 12

# Cards (raw px)
SANDY_RANK = (2100, 645, 2812, 722)       # "higher than 75% of 4,547 lines with values" + strip
SANDY_WIND = (2278, 586, 2348, 634)       # "100" (the value only, without the field name)
EQ_MAG = (2098, 410, 2772, 590)           # "magnitude 9.10" + "higher than 99% of 14,537 earthquakes"
HW_VALUE = (2100, 345, 2792, 530)         # "36.50 C" + "Heat-wave intensity (C), 25 Jul 2023" + rank
HW_TEMP = (2100, 350, 2340, 405)          # "36.50 C"
# Map points (Web Mercator from each tour view): Sandy's landfall near Atlantic City in the New
# York regional view (-75,39.5,z6.4); the Tohoku dot in the Japan view (140.8,37.3,z5.6); the
# Palermo cell in the Sicily view (12.5,38.6,z5.4).
SANDY_PX = (1483, 976)
TOHOKU_PX = (1517, 876)
PALERMO_PX = (1463, 999)
HW_SLIDER_PANEL = (1078, 1587, 2323, 1685)
HW_TRACK = (1335, 1632, 1962)
TY_KEEP = [(0, 64, 620, 380), (1040, 1410, 1770, 1700)]
EQ_KEEP = [(0, 64, 620, 380), (1040, 1410, 1770, 1700)]
EQ_RING_CENTRE = (1521, 1010)
EQ_RING_START = (1164, 738)               # Japan in the Pacific view

# ---------------------------------------------------------------- the folder (a re-layout of the
# real Finder list: the real file names and sizes, generic document icons)
FOLDER = 'map data'
FILES = [  # (name, size as Finder shows it)
    ('ETOPO_2022_relief_30min.tif', '375 KB'),
    ('GDO_heatwave_intensity_JJA_2023.nc', '12.6 MB'),
    ('IBTrACS_storm_tracks_1980-2026.cpg', '5 bytes'),
    ('IBTrACS_storm_tracks_1980-2026.dbf', '2.6 MB'),
    ('IBTrACS_storm_tracks_1980-2026.prj', '145 bytes'),
    ('IBTrACS_storm_tracks_1980-2026.shp', '5.3 MB'),
    ('IBTrACS_storm_tracks_1980-2026.shx', '40 KB'),
    ('NCEI_volcano_locations.csv', '106 KB'),
    ('usgs_earthquakes_M6plus_1900-2026.gpkg', '2.2 MB'),
]
FILE_DATE = 'Today at 08:38'
ROW_SHP, ROW_NC, ROW_GPKG = 5, 1, 8
FORMATS = 'Shapefile · GeoPackage · NetCDF · GeoTIFF · CSV'

# ---------------------------------------------------------------- the table (world layout)
W = 2822
H = 1758
TL1 = (-1411.0, -879.0)
FSCALE = 1.6                                  # the folder window, drawn at 1.6 x its 2x layout
FOLDER_W, FOLDER_H = 1560, 820                # its layout size (2x px)
FTL = (TL1[0] - FOLDER_W * FSCALE - 700.0, -FOLDER_H * FSCALE / 2)
TL2 = (2300.0, -677.0)
CHIP3_C = (TL2[0] + 1409.0, TL2[1] + H + 300.0)
TL3 = (TL2[0] + W + 700.0, CHIP3_C[1] - HW_TRACK[1])
CHIP_SCALE = 1.6

# ---------------------------------------------------------------- beats (s)
T_LIFT1 = 3.0            # the .shp row lifts out of the folder
T_OPEN1 = 4.20           # it lands: the window opens out of it (the music's first downbeat)
T_REVEAL1 = (4.85, 5.75)
T_ENTER1 = 6.95
T_SANDY = 7.95
T_TRAVEL = (10.80, 11.20)
T_OPEN2 = (12.05, 12.70)
T_RING = (12.70, 14.00)
T_ENTER2 = 14.70
T_JAPAN = 15.35
T_M91 = 16.00
T_DROP = 16.90
T_FILM3 = 17.25
T_ENTER3 = 20.15
T_PALERMO = 20.85
T_TEMP = 21.30
T_GATHER = (23.30, 25.90)
T_STOP = 26.80
T_PEN = (27.30, 28.40)
T_END = 28.50

TYPE1 = ('New York', [6.00, 6.11, 6.20, 6.33, 6.45, 6.55, 6.66, 6.78])
TYPE2 = ('Tokyo', [14.15, 14.26, 14.37, 14.48, 14.58])
TYPE3 = ('Palermo', [19.55, 19.63, 19.72, 19.80, 19.89, 19.97, 20.05])
# What the bar holds after Enter: the chosen place's name (the app writes the place's label
# into the field: GeoNames' "New York City" for the query "New York").
LABEL1, LABEL2, LABEL3 = 'New York City', 'Tokyo', 'Palermo'

# For audio/synth.py (shared with v3)
T_ENTER = T_OPEN1
T_END_ = T_END


def key_times():
    ks = TYPE1[1] + TYPE2[1] + TYPE3[1]
    return sorted(ks), [T_ENTER1, T_ENTER2, T_ENTER3]
