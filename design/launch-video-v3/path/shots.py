"""The v3.7 shots: one continuous keyframed camera per film, in the film's own time (the v3.6
timeline, seconds), written as render scripts for the app's offline renderer.

    python path/shots.py <shot> [...]        # writes render/<shot>/shot.json
    python path/shots.py --list

Each output frame (30 fps) is rendered once, or as 2-8 sub-frames across a 180-degree shutter when
the camera moves more than about a point per shutter (motion blur; edit/compose37.py averages
them). The events are the app's own: typing into its search field, Enter, a click, a time step.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(ROOT, 'edit'))
from camera import Chain, View, screen_motion, smootherstep, wrapped  # noqa: E402
import edl36 as N  # noqa: E402

FPS = 30
SHUTTER = 0.5            # 180 degrees
MAX_SUB = 8
DATA = os.environ.get('LM_FILM_DATA', '/private/tmp/claude-501/-Users-kangninghuang-Documents-repos/'
                      '9ecd2d65-d0e0-4a28-9c11-3cb7cc58e11b/scratchpad/map data')
SHP = os.path.join(DATA, 'IBTrACS_storm_tracks_1980-2026.shp')
GPKG = os.path.join(DATA, 'usgs_earthquakes_M6plus_1900-2026.gpkg')
NC = os.path.join(DATA, 'GDO_heatwave_intensity_JJA_2022.nc')
RENDER = os.path.join(ROOT, 'render')
DRIFT = 0.015            # zoom levels per second (about 1% a second)


def L(lon, lat, z):
    return View.ll(lon, lat, z)


def typing(text, times, field='search', enter=None, focus_at=None):
    ev = [{'t': (focus_at if focus_at is not None else times[0] - 0.3), 'focus': field}]
    ev += [{'t': t, 'text': ch} for ch, t in zip(text, times)]
    if enter is not None:
        ev.append({'t': enter, 'key': 'Enter'})
    return ev


TOOLBAR = 52.0          # points above the map in the window


def screen_pt(view, lon, lat):
    """Where lon/lat is in the window, in points (the map starts under the toolbar)."""
    from camera import merc
    x, y = view.to_screen(*merc(lon, lat))
    return x, y + TOOLBAR


def t33(t):
    return N.t33(t)


def hw_index(t):
    """v3.3's time-slider schedule (compose33.hw_index): 76 daily steps after the drop."""
    x = t33(t)
    if x is None or x < 36.65 + 0.05:
        return 0
    u = min(1.0, max(0.0, (x - 36.75) / (40.1 - 36.75)))
    return min(N.HW_LAST, int(N.HW_LAST * (0.5 - 0.5 * math.cos(math.pi * u)) + 1e-6))


# ---------------------------------------------------------------- film 1: storms (shapefile)
NY_REGION = L(-73.5 + 360, 40.2, 6.5)
SANDY_REGION = L(-74.5 + 360, 39.8, 6.4)
BOTH_SANDYS = L(43.4782 + 360, 14.8311, 2.014)      # where Find's "zoom to both" goes (app-camera)
T_FIND_ROW = 26.25                                  # Sandy (2012), the second row of "2 found"


def film1_chain():
    """World (after the tracks draw in) -> zoom to the Caribbean -> pan up the Atlantic ->
    the search's flight straight to the New York region (z6.5) -> Find "Sandy": out to both
    storms -> Sandy (2012) chosen in the list, in to its region."""
    c = Chain(15.9, L(150, 15, 2.3), drift=(DRIFT, 0, 0))
    c.move(17.95, 18.95, L(-80 + 360, 24, 4.6))                  # "Zoom ..."
    c.move(18.60, 19.75, L(-62 + 360, 33, 4.6))                  # "... and pan anywhere" (overlapping)
    c.move(N.T_ENTER1 + 0.05, N.T_ENTER1 + 1.45, NY_REGION, 'fly')   # the search's landing
    c.move(24.70, 26.00, BOTH_SANDYS, 'fly')                     # "it finds both storms"
    c.move(T_FIND_ROW + 0.05, T_FIND_ROW + 1.40, SANDY_REGION, 'fly')
    return c


def shot_ty():
    ev = typing(N.TYPE1[0], N.TYPE1[1], enter=N.T_ENTER1, focus_at=19.95)
    ev += [{'t': 22.75, 'step': 'layer 0'}]                      # the layer's panel, with Find
    ev += typing(N.TYPE_FIND[0], N.TYPE_FIND[1], field='find', focus_at=23.85)
    ev += [{'t': T_FIND_ROW, 'click_at': [1100, 280]}]
    return dict(name='ty', chain=film1_chain(), t0=14.40, t1=28.60, files=[SHP], setup=['esc'], events=ev)


def shot_ty_ny():
    """The prototype: the New York landing alone."""
    return dict(name='ny', chain=film1_chain(), t0=19.70, t1=23.30, files=[SHP], setup=['esc'],
                events=typing(N.TYPE1[0], N.TYPE1[1], enter=N.T_ENTER1, focus_at=19.95))


def shot_ty_plate():
    return dict(name='ty_plate', chain=Chain(0, L(150, 15, 2.3), drift=(0, 0, 0)), t0=15.0, t1=15.0, files=[])


# ---------------------------------------------------------------- film 2: earthquakes (GeoPackage)
TOKYO_REGION = L(140.6, 36.4, 6.4)
TOHOKU = (142.373, 38.297)
T_DOT = 34.10             # the dot clicked: 4 objects there, so the app's chooser opens
T_ROW = 34.40             # its first row: 2011 Great Tohoku Earthquake -> the card
ROW_OFFSET = None         # (row - dot) in points, measured in a probe at TOKYO_REGION


def film2_chain():
    c = Chain(31.35, L(175, 12, 2.4), drift=(DRIFT, 0, 0))
    c.move(N.T_ENTER2 + 0.05, N.T_ENTER2 + 1.45, TOKYO_REGION, 'fly')
    return c


def shot_eq():
    c = film2_chain()
    dot0 = screen_pt(TOKYO_REGION, *TOHOKU)
    off = (950 - dot0[0], 360 - dot0[1])                         # the probe's row, relative to its dot
    dot = screen_pt(wrapped(c.at(T_DOT)), *TOHOKU)
    ev = typing(N.TYPE2[0], N.TYPE2[1], enter=N.T_ENTER2, focus_at=31.95)
    ev += [{'t': T_DOT, 'step': f'click {TOHOKU[0]},{TOHOKU[1]}'},
           {'t': T_ROW, 'click_at': [round(dot[0] + off[0], 1), round(dot[1] + off[1], 1)]}]
    return dict(name='eq', chain=c, t0=28.80, t1=40.30, files=[GPKG], setup=['card style_by_value magnitude', 'esc'],
                events=ev)


def shot_eq_plate():
    return dict(name='eq_plate', chain=Chain(0, L(175, 12, 2.4), drift=(0, 0, 0)), t0=15.0, t1=15.0, files=[])


# ---------------------------------------------------------------- film 3: heatwaves (NetCDF)
BARCELONA_REGION = L(5.5, 41.6, 5.5)
BARCELONA = (2.15899, 41.38879)
T_CELL = 45.95


def film3_chain():
    c = Chain(40.30, L(-28, 32, 3.4), drift=(DRIFT, 0, 0))
    c.move(N.T_ENTER3 + 0.05, N.T_ENTER3 + 1.45, BARCELONA_REGION, 'fly')
    return c


def shot_hw():
    ev, last = [], 0
    for n in range(int(round(39.90 * FPS)), int(round(N.T_ENTER3 * FPS))):
        k = hw_index(n / FPS)
        if k != last:
            ev.append({'t': n / FPS - 1e-4, 'step': f'time {k}'})
            last = k
    ev += typing(N.TYPE3[0], N.TYPE3[1], enter=N.T_ENTER3, focus_at=43.55)
    ev += [{'t': T_CELL, 'step': f'click {BARCELONA[0]},{BARCELONA[1]}'}]
    return dict(name='hw', chain=film3_chain(), t0=39.90, t1=50.40, files=[NC], setup=['esc', 'time 0'], events=ev)


# ---------------------------------------------------------------- Pro (the storm shapefile)
PRO_START = L(143.5, 35.69, 6.3)
PRO_ANSWER = L(135.0543, 29.1250, 5.451)            # where the app zooms to its answer (app-camera)


def pro_chain():
    c = Chain(51.0, PRO_START, drift=(DRIFT * 0.6, 0, 0))
    c.move(N.T_SEND + 0.05, N.T_SEND + 1.45, PRO_ANSWER, 'fly')
    return c


def shot_pro():
    ev = typing(N.TYPE4[0], N.TYPE4[1], field='ask', enter=N.T_SEND, focus_at=N.PRO0 + 3.35)
    return dict(name='pro', chain=pro_chain(), t0=49.90, t1=69.60, files=[SHP], setup=['esc'], events=ev,
                dev_pro=True, env={'LM_TIER': 'pro', 'LM_DEV_PRO': '1',
                                   'LM_ASK_FIXTURE': os.path.join(ROOT, 'tours', 'tokyo_plan_real.json')})


SHOTS = {'ny': shot_ty_ny, 'ty': shot_ty, 'ty_plate': shot_ty_plate, 'eq': shot_eq, 'eq_plate': shot_eq_plate,
         'hw': shot_hw, 'pro': shot_pro}
CHAINS = {'ty': film1_chain, 'eq': film2_chain, 'hw': film3_chain, 'pro': pro_chain}


# ---------------------------------------------------------------- writing
def samples(chain, t0, t1):
    out = []
    for n in range(int(round(t0 * FPS)), int(round(t1 * FPS)) + 1):
        t = n / FPS
        m = screen_motion(chain.at(t - 0.5 / FPS), chain.at(t + 0.5 / FPS))   # points per frame
        k = 1 if m * SHUTTER < 1.0 else min(MAX_SUB, int(math.ceil(m * SHUTTER / 1.5)))
        for j in range(k):
            ts = t + ((j + 0.5) / k - 0.5) * SHUTTER / FPS if k > 1 else t
            lon, lat, z = wrapped(chain.at(ts)).lonlatz()
            out.append({'t': round(ts, 6), 'cam': [lon, lat, z], 'out': f'{n:05d}_{j}.png', 'frame': n, 'sub': k})
    return out


def write(shot):
    d = os.path.join(RENDER, shot['name'])
    os.makedirs(d, exist_ok=True)
    fr = samples(shot['chain'], shot['t0'], shot['t1'])
    if shot.get('env'):
        with open(os.path.join(d, 'env'), 'w') as f:
            f.write('\n'.join(f'{k}={v}' for k, v in shot['env'].items()) + '\n')
    js = {'size': [1409, 845], 'ppp': 2.0, 'theme': shot.get('theme', 'light'), 'basemap': 'auto',
          'dev_pro': shot.get('dev_pro', False), 'files': shot['files'], 'setup': shot.get('setup', []),
          'frames': fr, 'events': shot.get('events', [])}
    with open(os.path.join(d, 'shot.json'), 'w') as f:
        json.dump(js, f, indent=0)
    nf = len({x['frame'] for x in fr})
    print(f"{shot['name']}: {nf} frames, {len(fr)} renders -> {d}/shot.json")


if __name__ == '__main__':
    if '--list' in sys.argv:
        print(' '.join(SHOTS))
    for name in sys.argv[1:]:
        if name in SHOTS:
            write(SHOTS[name]())
