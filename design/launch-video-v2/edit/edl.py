"""Edit decision list for the v2 launch film (rough cut).

Times are seconds. Shot fields:
  src   raw recording (raw/<src>.mov)
  t0    start on the timeline; dur its length
  map   [(u, src_t), ...] piecewise-linear shot time -> source time (speed ramps, time-lapse)
  cam   [(u, cx, cy, zoom)] camera keys: content point (0..1) at the frame centre, zoom (1 = whole window)
  cursor [(u, x, y)] drawn pointer path in content coords; clicks [{'t': u, 'p': (x, y)}]
  overlays: extra drawn elements ('file' chip that is double-clicked, 'typing' caption)
Content coords are fractions of the app content (the window minus the macOS title bar).\nConsecutive shots dissolve over XF seconds: the outgoing shot keeps running underneath.
"""

DUR = 35.0
END_T0 = 30.5

# Content crops (x, y, w, h) in raw pixels: the native title bar (64 px) is cut away and replaced
# by a drawn, clean macOS-style frame; a few px of window edge are trimmed where present.
SOURCES = {
    'hook': {'crop': (0, 64, 2818, 1690)},
    'walls': {'crop': (0, 64, 2806, 1682)},
    'heat': {'crop': (0, 64, 2806, 1686)},
    'color': {'crop': (0, 64, 2818, 1690)},
    'share': {'crop': (0, 64, 2810, 1686)},
    'file': {'crop': (0, 64, 2810, 1686)},
    'pro2': {'crop': (0, 64, 2552, 1594)},
    'ex_earthquakes': {'crop': (0, 64, 2810, 1686)},
    'ex_power': {'crop': (0, 64, 2810, 1686)},
}

XF = 0.25                     # dissolve between consecutive shots (the outgoing shot runs on)
ZMAX = 1.5                    # push-ins are capped here

HAIYAN = (0.3655, 0.523)      # click 124.53,11.05 at view 126,11,z7.5
BEIJING = (0.5, 0.531)        # click 116.39,39.91 at the map centre
COLOR_BTN = (0.848, 0.579)    # "Color by a value" on the storms card (free-build color run)
SHARE_BTN = (0.963, 0.030)    # toolbar Share (opens the map image sheet; LM_DEV share run)
ASK_SEND = (0.6607, 0.928)    # Ask box send arrow (pro2 run)

# The question typed into the real Ask box (pro2 run). The `ask` tour step fills the box in one go,
# so the typing is drawn into the source frame itself, in the box's own font, size and colour:
# Inter Regular 27.9 px (raw, 2x) on the box's #FDFDFD, the text starting where the placeholder
# starts, on its baseline. Coordinates are content pixels of the crop.
ASKFIELD = {'x': 687, 'baseline': 1491, 'clear': (684, 1462, 990, 1500), 'size': 27.9,
            'bg': (253, 253, 253), 'ink': (29, 29, 31)}

SHOTS = [
    # 1 HOOK: click Haiyan, the card appears; ease in on "higher than 99% of 4,547 storms"
    dict(src='hook', t0=0.0, dur=5.6, map=[(0, 9.40), (5.6, 15.0)],
         cam=[(0, 0.5, 0.5, 1.0), (1.2, 0.5, 0.5, 1.0), (3.3, 0.79, 0.41, 1.5), (4.75, 0.795, 0.41, 1.5), (5.6, 0.5, 0.5, 1.0)],
         cursor=[(0, 0.60, 0.80), (0.85, *HAIYAN), (1.05, *HAIYAN), (2.0, 0.44, 0.63)],
         clicks=[{'t': 0.95, 'p': HAIYAN}]),
    # 2 second click: Beijing's walls, 35.3 km2, higher than 99% of 1,789
    dict(src='walls', t0=5.6, dur=3.2, map=[(0, 18.267), (3.2, 21.467)],
         cam=[(0, 0.5, 0.5, 1.0), (0.75, 0.5, 0.5, 1.0), (1.95, 0.80, 0.33, 1.5), (2.65, 0.805, 0.33, 1.5), (3.2, 0.5, 0.5, 1.0)],
         cursor=[(0, 0.64, 0.70), (0.48, *BEIJING), (0.7, *BEIJING), (1.4, 0.60, 0.62)],
         clicks=[{'t': 0.55, 'p': BEIJING}]),
    # 3-6 BUILT IN: storms, earthquakes, power plants, heatwaves with the time slider playing
    dict(src='hook', t0=8.8, dur=1.3, map=[(0, 2.7), (1.3, 4.0)],
         cam=[(0, 0.5, 0.5, 1.0), (1.55, 0.47, 0.5, 1.07)]),
    dict(src='ex_earthquakes', t0=10.1, dur=1.3, map=[(0, 6.0), (1.3, 7.3)],
         cam=[(0, 0.42, 0.5, 1.02), (1.55, 0.40, 0.5, 1.09)]),
    dict(src='ex_power', t0=11.4, dur=1.2, map=[(0, 6.0), (1.2, 7.2)],
         cam=[(0, 0.42, 0.5, 1.02), (1.45, 0.44, 0.5, 1.09)]),
    dict(src='heat', t0=12.6, dur=1.9, map=[(0, 4.2), (1.9, 31.0)],
         cam=[(0, 0.5, 0.5, 1.0), (2.15, 0.46, 0.52, 1.07)]),
    # 7-8 BRING YOUR OWN: double-click a file -> it lands, with the one-line note
    dict(src='hook', t0=14.5, dur=1.5, map=[(0, 0.0), (1.5, 1.5)],
         cam=[(0, 0.5, 0.5, 1.0), (1.75, 0.5, 0.5, 1.0)],
         cursor=[(0, 0.70, 0.78), (0.65, 0.5, 0.44), (1.75, 0.5, 0.44)],
         clicks=[{'t': 0.78, 'p': (0.5, 0.44)}, {'t': 0.93, 'p': (0.5, 0.44)}],
         overlays=[{'type': 'file', 'name': 'places to visit.csv', 'p': (0.5, 0.44), 'open': 1.05}]),
    # (source starts at 2.95 s: the basemap has finished fading in, no flash)
    dict(src='file', t0=16.0, dur=3.8, map=[(0, 2.95), (3.8, 6.75)],
         cam=[(0, 0.5, 0.5, 1.0), (0.6, 0.5, 0.5, 1.0), (2.2, 0.80, 0.36, 1.5), (3.2, 0.805, 0.36, 1.5), (3.8, 0.5, 0.5, 1.0)]),
    # 9-10 SHARE: colour by wind (free build), then the map image sheet (LM_DEV take, Style tab
    # already showing, so no developer-only actions are ever on screen)
    dict(src='color', t0=19.8, dur=2.6, map=[(0, 9.42), (2.6, 12.02)],
         cam=[(0, 0.5, 0.5, 1.0), (2.6, 0.48, 0.52, 1.06)],
         cursor=[(0, 0.60, 0.62), (0.75, *COLOR_BTN), (1.1, *COLOR_BTN), (1.9, 0.62, 0.55)],
         clicks=[{'t': 0.86, 'p': COLOR_BTN}]),
    dict(src='share', t0=22.4, dur=3.1, map=[(0, 18.47), (3.1, 21.57)],
         cam=[(0, 0.48, 0.52, 1.06), (0.3, 0.48, 0.52, 1.06), (1.0, 0.5, 0.5, 1.0), (2.3, 0.40, 0.47, 1.2), (2.7, 0.40, 0.47, 1.2), (3.1, 0.5, 0.5, 1.0)],
         cursor=[(0, 0.80, 0.30), (0.4, *SHARE_BTN), (0.62, *SHARE_BTN), (1.4, 0.82, 0.50)],
         clicks=[{'t': 0.47, 'p': SHARE_BTN}]),
    # 11-12 PRO: the question is typed into the Ask box, sent, answered (cities only: calm setup)
    dict(src='pro2', t0=25.5, dur=2.4, map=[(0, 5.163), (2.4, 7.563)],
         cam=[(0, 0.5, 0.5, 1.0), (0.3, 0.5, 0.5, 1.0), (1.6, 0.48, 0.76, 1.4), (2.65, 0.48, 0.76, 1.42)],
         cursor=[(0, 0.75, 0.62), (1.5, *ASK_SEND), (2.4, *ASK_SEND)],
         clicks=[{'t': 1.70, 'p': ASK_SEND}],
         overlays=[{'type': 'askfield', 'text': 'Which capital city sits highest above sea level?',
                    't0': 0.25, 't1': 1.45, 'until': 1.72}]),
    dict(src='pro2', t0=27.9, dur=2.6, map=[(0, 12.95), (2.6, 15.55)],
         cam=[(0, 0.48, 0.76, 1.42), (0.3, 0.48, 0.76, 1.42), (1.8, 0.475, 0.52, 1.45), (2.6, 0.475, 0.52, 1.46)],
         fade_out=0.5),
]

CAPTIONS = [
    (0.25, 2.65, 'Click anything.'),
    (2.95, 8.55, 'It tells you where it stands.'),
    (9.0, 14.3, 'Eight maps built in.'),
    (14.7, 17.4, 'Or double-click your own file.'),
    (17.6, 19.6, 'It lands in the right place.'),
    (20.0, 25.3, 'Colour it. Export it.'),
    (25.7, 28.25, 'Pro: ask in plain words.'),
    (28.45, 30.2, 'Free with an account.'),
]


def click_times():
    return sorted(s['t0'] + c['t'] for s in SHOTS for c in s.get('clicks', []))
