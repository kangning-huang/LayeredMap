"""Climate and oceans: temperature anomalies, sea surface temperature, precipitation."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, Ortho, globe, globe_shade, scallop,
                  ring_pts, terrain, draw_box, box_faces, conifer)

# Approximate GISTEMP global annual anomaly (deg C vs 1951-1980) at decade-ish anchors, used
# only for the shape of the inset bar chart (no numbers are shown).
_GISTEMP = [(1880, -.18), (1890, -.25), (1900, -.2), (1910, -.42), (1920, -.27), (1930, -.15), (1940, .1),
            (1950, -.17), (1960, -.03), (1970, .03), (1980, .26), (1990, .45), (2000, .42), (2010, .72),
            (2016, 1.01), (2020, 1.02), (2023, 1.17), (2024, 1.28)]


def _gis(y):
    for (y0, v0), (y1, v1) in zip(_GISTEMP, _GISTEMP[1:]):
        if y0 <= y <= y1:
            return lerp(v0, v1, (y - y0) / (y1 - y0))
    return _GISTEMP[-1][1]


def _anom_colour(a: Art, v):
    """Diverging: muted ocean blues for cool, the accent for warm, in 0.2-degree steps."""
    v = round(v * 5) / 5
    if v < 0:
        return mix(PAL["paper"], PAL["water4"], min(1, -v / .9))
    return mix(PAL["paper"], a.A if v < 1.6 else a.AD, min(1, v / 2.0))


def temperature():
    """Gridded temperature anomalies on the globe, warmest in the Arctic (Arctic amplification),
    beside the global annual anomaly since 1880 as bars against the 1951-1980 baseline."""
    a = Art("verm", seed=41)
    # ---- inset chart
    x0, x1, yb, yt = 18, 140, 150, 40
    zero = 112
    a.path(P([(x0, yt), (x0, yb), (x1, yb)]), cls="o", sw=.8)
    a.path(P([(x0, zero), (x1, zero)]), cls="h", da="2 1.5", so=.6)
    sx = lambda yr: lerp(x0 + 3, x1 - 2, (yr - 1880) / 144)
    sy = lambda v: zero - v * 52
    a.path(P([(sx(1951), yt + 4), (sx(1951), yb), (sx(1980), yb), (sx(1980), yt + 4)], True), fill=PAL["ink"], fo=.06)
    bars = {}
    rnd = a.rng
    vals = []
    for yr in range(1880, 2025):
        v = _gis(yr) + rnd.uniform(-.09, .09)
        vals.append(v)
        c = _anom_colour(a, v)
        bars.setdefault(c, []).append(f"M{n(sx(yr))} {n(zero)}V{n(sy(v))}")
    for c, d in bars.items():
        a.path("".join(d), stroke=c, sw=.75, fill="none")
    sm = [(sx(yr), sy(sum(vals[max(0, i - 5):i + 6]) / len(vals[max(0, i - 5):i + 6]))) for i, yr in enumerate(range(1880, 2025)) if i % 4 == 0]
    a.path(smooth(sm), stroke=PAL["ink"], sw=1.0, fill="none")
    for k in range(0, 145, 20):          # ticks every 20 years
        a.path(f"M{n(sx(1880 + k))} {n(yb)}v2.5", cls="h", so=.8)
    for v in (-.5, .5, 1.0):
        a.path(f"M{n(x0)} {n(sy(v))}h-2.5", cls="h", so=.8)

    # ---- globe of 10-degree cells
    G = Ortho(-20, 32, 66, (234, 92))
    disc, cp = globe(a, G, land=False, grat=False)
    a.group(cp=cp)
    cells = {}
    for lat in range(-90, 90, 10):
        for lon in range(-180, 180, 10):
            c = (lon + 5, lat + 5)
            pts, vis = [], 0
            for (lo, la) in ((lon, lat), (lon + 10, lat), (lon + 10, lat + 10), (lon, lat + 10)):
                p, v = G(lo, la)
                pts.append(p)
                vis += v
            if vis == 0:
                continue
            la_ = c[1]
            v = (.3 + 2.2 * max(0, la_ / 90) ** 2 + .35 * max(0, -la_ / 90) - .35 * math.exp(-((la_ + 55) / 12) ** 2)
                 + .45 * math.sin(math.radians(c[0] * 2.3)) * math.cos(math.radians(la_ * 2.1))
                 + .25 * math.cos(math.radians(c[0] * 3.7 + la_ * 1.3)))
            col = _anom_colour(a, v)
            cells.setdefault(col, []).append(P(pts, True))
    for col, ds in cells.items():
        a.path("".join(ds), fill=col, stroke="#fff", sw=.3, so=.6)
    a.path(G.land(.9), fill="none", stroke=PAL["ink"], sw=.45, so=.6, lj="round")
    a.end()
    globe_shade(a, G, disc, op=.22)
    return a.render()


def sea_temperature():
    """An El Nino section of the equatorial Pacific: warm surface water spread east over a
    flattened thermocline (the normal, tilted one dashed), deep convection shifted to the central
    Pacific, the Andes on the eastern shore, and the ship, moored buoy and Argo float whose
    readings build the record."""
    a = Art("verm", seed=43)
    o = (14, 164)
    Wd, D, Hh = 186.0, 124.0, 58.0
    f, t, r = box_faces(0, Wd, 0, Hh, D, o)
    a.shadow(o[0] + 150, o[1] + 4, 160, 7, op=.2)
    # ---- front face: the vertical section along the equator, west on the left
    a.path(P(f, True), fill=a.lin([(0, PAL["water3"]), (1, PAL["water5"])], 0, 0, 0, 1))

    def thermo(x, nino=True):
        if nino:
            return Hh - 20 - 4 * math.sin(x / Wd * math.pi) + x / Wd * 5
        return Hh - 34 + x / Wd * 27
    warm = [ob(x, 0, thermo(x), o) for x in [Wd * k / 24 for k in range(25)]]
    a.path(P([ob(0, 0, Hh, o), ob(Wd, 0, Hh, o)] + warm[::-1], True),
           fill=a.lin([(0, a.A), (.55, a.AL), (1, mix(a.AL, PAL["water2"], .35))], 0, 0, 1, 0))
    for k in range(1, 4):
        a.path(smooth([ob(x, 0, thermo(x) - 9 * k, o) for x in [Wd * j / 12 for j in range(13)]]), stroke="#fff", sw=.45, so=.35, fill="none")
    a.path(smooth(warm), stroke=PAL["ink"], sw=.9, fill="none")
    a.path(smooth([ob(x, 0, thermo(x, False), o) for x in [Wd * k / 12 for k in range(13)]]), cls="h", da="2.4 1.8", so=.9)
    # ---- right face (the eastern cut)
    a.path(P(r, True), fill=a.lin([(0, PAL["water4"]), (1, PAL["water5"])], 0, 0, 0, 1))
    a.path(P([ob(Wd, 0, Hh, o), ob(Wd, D, Hh, o), ob(Wd, D, thermo(Wd), o), ob(Wd, 0, thermo(Wd), o)], True), fill=mix(a.AL, PAL["water3"], .5))
    # ---- sea surface: SST with the warm tongue along the equator spreading east
    a.path(P(t, True), fill=a.lin([(0, PAL["water2"]), (1, PAL["water1"])], 0, 0, 1, 0))
    eq = D * .42
    for k, (w, col) in enumerate(((44, mix(a.AL, PAL["water2"], .55)), (30, mix(a.AL, PAL["water2"], .2)), (18, a.AL), (8, a.A))):
        band = []
        for j in range(17):
            x = Wd * j / 16
            band.append(ob(x, max(1, eq - w * (1 - .35 * (x / Wd) ** 1.4)), Hh, o))
        for j in range(16, -1, -1):
            x = Wd * j / 16
            band.append(ob(x, min(D - 1, eq + w * .8 * (1 - .35 * (x / Wd) ** 1.4)), Hh, o))
        a.path(smooth(band, True), fill=col)
    a.path(P([ob(0, eq, Hh, o), ob(Wd, eq, Hh, o)]), cls="h", da="3 2", so=.5)     # the equator
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o")
    # ---- the Andes along the eastern shore: a ridge lit from the west
    crest = [ob(Wd + 7, D * k / 16, Hh + 9 + 4 * math.sin(k * 1.9) + 2 * math.cos(k * 3.3), o) for k in range(17)]
    west = [ob(Wd + 1, D * k / 16, Hh, o) for k in range(17)]
    east = [ob(Wd + 15, D * k / 16, Hh, o) for k in range(17)]
    a.path(smooth(crest) + "L" + "L".join(f"{n(p[0])} {n(p[1])}" for p in east[::-1]) + "Z", fill=mix(PAL["sand2"], PAL["ink"], .18))
    a.path(smooth(crest) + "L" + "L".join(f"{n(p[0])} {n(p[1])}" for p in west[::-1]) + "Z", fill=PAL["sand1"])
    a.path(smooth(crest), cls="h", so=.8)
    a.path("".join(P([crest[k], lerp2(crest[k], west[k], .8)]) for k in range(1, 16, 2)), cls="h", so=.35)
    # ---- atmosphere: deep convection over the central Pacific; the Walker cell aloft
    cb = ob(Wd * .5, eq, Hh, o)
    cu = scallop([(cb[0] - 20, cb[1] - 26), (cb[0] - 18, cb[1] - 52), (cb[0] - 8, cb[1] - 68), (cb[0] + 8, cb[1] - 70),
                  (cb[0] + 18, cb[1] - 56), (cb[0] + 22, cb[1] - 26)], bulge=.3, seed=4)
    anv = smooth([(cb[0] - 40, cb[1] - 72), (cb[0], cb[1] - 84), (cb[0] + 44, cb[1] - 74), (cb[0] + 10, cb[1] - 68), (cb[0] - 14, cb[1] - 68)], True)
    a.path(anv, fill=a.lin([(0, "#fff"), (1, PAL["cloud2"])], 0, 0, 0, 1))
    a.path(anv, cls="h", so=.5)
    a.path(cu, fill=a.lin([(0, "#fff"), (.6, PAL["cloud"]), (1, PAL["slate1"])], 0, 0, .6, 1))
    a.path(cu, cls="h", so=.5)
    a.path(f"M{n(cb[0] - 20)} {n(cb[1] - 26)}h42", stroke=PAL["slate2"], sw=1.8, lc="round")
    a.path("".join(f"M{n(cb[0] - 16 + k * 4)} {n(cb[1] - 24)}l-2 22" for k in range(9)), stroke=PAL["water4"], sw=.55, so=.8, fill="none")
    a.arrow([(cb[0] - 6, cb[1] - 30), (cb[0] - 6, cb[1] - 62)], color=a.AD, w=1.1, head=4.6, curve=False)
    a.arrow([(cb[0] + 48, cb[1] - 78), (cb[0] + 84, cb[1] - 74), (cb[0] + 102, cb[1] - 54)], w=.9, head=4.4)
    a.arrow([(cb[0] - 46, cb[1] - 78), (cb[0] - 82, cb[1] - 74), (cb[0] - 98, cb[1] - 54)], w=.9, head=4.4)
    # ---- measurement platforms: ship, moored buoy (with its thermistor chain), Argo float
    sp = ob(40, eq + 26, Hh, o)
    a.path(f"M{n(sp[0] - 10)} {n(sp[1] - 3)}h20l-3 4.4h-14Z", fill=PAL["slated"])
    a.path(f"M{n(sp[0] - 3)} {n(sp[1] - 3)}v-5h6v5Z", fill=PAL["white"], stroke=PAL["ink"], sw=.4)
    a.path(f"M{n(sp[0] + 14)} {n(sp[1] + .5)}h14", stroke="#fff", sw=.6, so=.8)
    bx = Wd * .72
    bp = ob(bx, 0, Hh, o)
    a.path(P([bp, ob(bx, 0, 0, o)]), stroke=PAL["ink"], sw=.5, fill="none")
    for k in range(1, 6):
        a.circle(ob(bx, 0, Hh - k * 9.5, o), .9, fill="#fff", stroke=PAL["ink"], sw=.35)
    a.path(f"M{n(bp[0] - 3.4)} {n(bp[1])}h6.8l-1.2 3h-4.4Z", fill=a.AD)
    a.path(f"M{n(bp[0])} {n(bp[1])}v-8m-2.4 2h4.8", stroke=PAL["ink"], sw=.6, fill="none")
    ax = Wd * .88
    prof = [ob(ax - 5, 0, Hh, o), ob(ax - 3, 0, Hh - 24, o), ob(ax - 1, 0, 8, o), ob(ax + 3, 0, 8, o),
            ob(ax + 5, 0, Hh - 24, o), ob(ax + 7, 0, Hh - 1, o)]
    a.arrow(prof[:3], color="#fff", w=.7, head=3.4, da="1.4 1.2", curve=False)
    a.arrow(prof[3:], color="#fff", w=.7, head=3.4, da="1.4 1.2", curve=False)
    fp = prof[2]
    a.path(f"M{n(fp[0] + .6)} {n(fp[1] - 6)}h3v9h-3Z", fill="#fff", stroke=PAL["ink"], sw=.4)
    return a.render()


def precipitation():
    """Orographic precipitation over a mountain range: moist air forced up the windward slope,
    clouds and rain on that side, snow above the freezing level, a dry rain shadow in the lee,
    measured by a rain gauge on the ground and a satellite radar scanning from orbit."""
    a = Art("cobalt", seed=47)
    o = (14, 166)
    Wd, D = 214.0, 84.0

    def h(x, y):
        ridge = 46 * math.exp(-((x - 120) / 30) ** 2) * (0.82 + 0.18 * math.sin(y / 13 + 1.2))
        return ridge + 3 + 2 * math.sin(x / 17 + y / 11)

    def cover(x, y):
        z = h(x, y)
        if z > 33:
            return "snow"
        if x < 112:
            return "wet"
        return "dry" if x > 136 else "mid"
    cols = {"snow": "#f4f6f7", "wet": PAL["moss"], "mid": PAL["sage2"], "dry": PAL["sand2"]}
    a.shadow(o[0] + 170, o[1] + 4, 170, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=36, ny=10, cover=cover, colours=cols, base=10,
                   strata=[(3, PAL["umber"]), (8, PAL["clay"]), (99, PAL["rock"])])
    # forest on the wet windward foothills
    rnd = a.rng
    tr = []
    for j in range(9, 0, -1):
        for i in range(16):
            x, y = 6 + i * 7.6 + rnd.uniform(-2, 2), j * 9 + rnd.uniform(-3, 3)
            if h(x, y) < 26:
                tr.append(surf(x, y))
    a.path("".join(conifer(p[0], p[1] + .5, 6, 3.2) for p in tr), fill=PAL["forest"], stroke=PAL["forestd"], sw=.3)
    # moist air: flowing in low from the left, forced up the windward slope, sinking in the lee
    for k, (y, clr) in enumerate(((16, 10), (64, 16))):
        pts = [surf(x, y, clr + (8 if 90 < x < 150 else 0)) for x in (0, 30, 64, 92, 118, 146, 180, 212)]
        pts = [(p[0], p[1]) for p in pts]
        a.arrow(pts, color=a.A, w=1.15 - .3 * k, head=4.8, op=1 - .35 * k)
    # cloud bank against the windward slope
    top = surf(120, 50)
    cl = scallop([(70, 118), (72, 100), (88, 86), (110, 78), (136, 76), (top[0] - 2, 82), (top[0] + 2, 96), (top[0] - 10, 106), (112, 116)],
                 bulge=.3, seed=2)
    a.path(cl, fill=a.lin([(0, "#fff"), (.6, PAL["cloud"]), (1, PAL["slate1"])], 0, 0, .6, 1))
    a.path(cl, cls="h", so=.5)
    # rain from the cloud base down to the windward slope; snow over the summit
    rain = []
    for k in range(16):
        x = 78 + k * 4.4
        y0 = 114 + rnd.uniform(-2, 1)
        xg = x - 6 - 14 - 42 * .52 + 14         # back-project to the ground at mid-depth
        gy = surf(max(1, x - 14 - 42 * .52), 42)[1]
        if gy - y0 < 6:
            continue
        rain.append(f"M{n(x)} {n(y0)}L{n(x - 4)} {n(gy - 1)}")
    a.path("".join(rain), stroke=a.A, sw=.6, so=.8, fill="none")
    flakes = ""
    for k in range(6):
        c = (top[0] - 10 + k * 4 + rnd.uniform(-1, 1), top[1] - 18 + (k % 3) * 4)
        flakes += f"M{n(c[0] - 1.3)} {n(c[1])}h2.6M{n(c[0])} {n(c[1] - 1.3)}v2.6M{n(c[0] - .9)} {n(c[1] - .9)}l1.8 1.8M{n(c[0] + .9)} {n(c[1] - .9)}l-1.8 1.8"
    a.path(flakes, stroke=a.AD, sw=.45, fill="none")
    # rain gauge on the windward plain
    g = surf(34, 30)
    a.path(f"M{n(g[0] - 2)} {n(g[1])}v-8h4v8Z", fill=PAL["paper2"], stroke=PAL["ink"], sw=.5)
    a.ellipse((g[0], g[1] - 8), 2.6, .9, fill=a.AP, stroke=PAL["ink"], sw=.5)
    # satellite with a radar scan swath
    sat = (232, 24)
    a.path(f"M{n(sat[0] - 13)} {n(sat[1] - 2.5)}h9v5h-9ZM{n(sat[0] + 4)} {n(sat[1] - 2.5)}h9v5h-9Z", fill=PAL["slate2"], stroke=PAL["ink"], sw=.4)
    a.path(f"M{n(sat[0] - 4)} {n(sat[1] - 4)}h8v8h-8Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.5)
    a.path(P([(sat[0] - 2, sat[1] + 4), (88, 82), (160, 86), (sat[0] + 2, sat[1] + 4)], True), fill=a.AP, fo=.3, stroke=a.AD, sw=.4, so=.6, da="1.6 1.4")
    return a.render()
