"""Climate and oceans: temperature anomalies, sea surface temperature, precipitation."""
import math

from .kit import PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, Ortho, globe, globe_shade, scallop, ring_pts, terrain, draw_box, box_faces, conifer, countries

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


def _temperature_v(lo_, la_):
    """Warming almost everywhere, strongest toward the Arctic; Southern Ocean near zero."""
    return (.45 + 1.2 * max(0, la_ / 90) ** 2 - .55 * math.exp(-((la_ + 60) / 12) ** 2)
            + .05 * math.sin(math.radians(lo_ * 2 + la_)))


_TEMPERATURE_RINGS = None


def _temperature_rings():
    """Country rings with bounding boxes, for a cheap land test."""
    global _TEMPERATURE_RINGS
    if _TEMPERATURE_RINGS is None:
        _TEMPERATURE_RINGS = [(min(x for x, _ in r), max(x for x, _ in r), min(y for _, y in r),
                               max(y for _, y in r), r) for _iso, r in countries()]
    return _TEMPERATURE_RINGS


def _temperature_is_land(lo, la):
    for xa, xb, ya, yb, r in _temperature_rings():
        if not (xa <= lo <= xb and ya <= la <= yb):
            continue
        inside = False
        for (x1, y1), (x2, y2) in zip(r, r[1:] + r[:1]):
            if (y1 > la) != (y2 > la) and lo < x1 + (la - y1) * (x2 - x1) / (y2 - y1):
                inside = not inside
        if inside:
            return True
    return False


def _temperature_cell(lon, lat):
    """Cell anomaly: the latitude field, with land warming up to ~1.8x faster than ocean (by the
    cell's land fraction on a 3x3 sample)."""
    f = sum(_temperature_is_land(lon + 2.5 + 5 * i, lat + 2.5 + 5 * j) for i in range(3) for j in range(3)) / 9
    v = _temperature_v(lon + 7.5, lat + 7.5)
    # capped at the polar-cap value, so the Arctic stays the warmest step
    return min(v * (1 + .8 * f), _temperature_v(0, 78)) if v > 0 else v


def _temperature_land(G):
    """Land with back-side points dropped (a chord, not a run along the limb), so large rings
    that wrap behind the globe never sweep a false shadow over the Arctic."""
    out = []
    for _iso, ring in countries():
        pts = [p for p, v in (G(lo, la, clamp=False) for lo, la in ring) if v]
        pts = [p for i, p in enumerate(pts) if i == 0 or math.dist(p, pts[i - 1]) > 1.4 * .5]
        if len(pts) >= 3:
            out.append(P(pts, True))
    return "".join(out)


def _temperature_means():
    """GISTEMP v4 global five-year mean anomalies (deg C vs 1951-1980), 1880-84 ... 2020-24."""
    return [-.20, -.24, -.27, -.18, -.22, -.36, -.38, -.30, -.27, -.19, -.14, -.11, .05, -.06,
            -.10, -.04, -.01, -.06, .00, .06, .18, .23, .32, .42, .53, .62, .68, .93, 1.04]


def _temperature_bar(a: Art, d, v):
    """One five-year bar. Near-zero years (|v| < 0.08) get a pale tint by sign with a hairline
    edge in the same hue (data, not an empty box); the rest get the anomaly colour with an ink
    outline, at least one colour step from paper so no outlined bar reads as blank."""
    if abs(v) < .08:
        hue = PAL["water4"] if v < 0 else a.A
        a.path(d, fill=mix(PAL["paper"], hue, .3), stroke=hue, sw=.35, so=.55)
    else:
        a.path(d, fill=_anom_colour(a, math.copysign(max(abs(v), .11), v)), stroke=PAL["ink"], sw=.45, so=.3)


def temperature():
    """Gridded temperature anomalies on the globe, warmest in the Arctic (Arctic amplification),
    beside the global anomaly since 1880 as five-year bars against the 1951-1980 baseline."""
    a = Art("verm", seed=41)
    # ---- chart: five-year mean anomalies, coloured on the same scale as the globe
    x0, x1, yb, yt = 22, 140, 131, 52
    zero = 110
    sx = lambda yr: lerp(x0 + 4, x1 - 2, (yr - 1880) / 145)
    sy = lambda v: zero - v * 50
    step = sx(1885) - sx(1880)
    w = step * .78
    # the 1951-1980 baseline period as a light band behind the bars (never over them)
    bx0, bx1 = sx(1950) + (step - w) / 2 - 1, sx(1975) + (step + w) / 2 + 1  # 1950-54 ... 1975-79
    a.path(f"M{n(bx0)} {n(yt + 4)}H{n(bx1)}V{n(yb)}H{n(bx0)}Z", fill=PAL["ink"], fo=.07)
    a.path(P([(x0, zero), (x1, zero)]), cls="h", da="2.5 2", so=.6)
    for i, v in enumerate(_temperature_means()):
        xa = sx(1880 + 5 * i) + (step - w) / 2
        y0, y1 = sorted((zero, sy(v)))
        if y1 - y0 < 1.2:  # keep every five-year mark readable, even a 0.00 one
            y0, y1 = y0 - .6, y0 + .6
        d = f"M{n(xa)} {n(y0)}H{n(xa + w)}V{n(y1)}H{n(xa)}Z"
        _temperature_bar(a, d, v)
    a.path(P([(x0, yt), (x0, yb), (x1, yb)]), cls="o", sw=.9)
    a.path("".join(f"M{n(sx(yr) + step / 2)} {n(yb)}v2.2" for yr in range(1880, 2025, 20))
           + "".join(f"M{n(x0)} {n(sy(v))}h-2.2" for v in (-.5, .5, 1.0) if yt <= sy(v) <= yb),
           cls="o", sw=.6)

    # ---- globe of 15-degree cells
    G = Ortho(-20, 32, 66, (234, 92))
    disc, cp = globe(a, G, land=False, grat=False)
    a.group(cp=cp)
    cells = {}
    for lat in range(-60, 75, 15):
        for lon in range(-180, 180, 15):
            pts, vis = [], 0
            for (lo, la) in ((lon, lat), (lon + 15, lat), (lon + 15, lat + 15), (lon, lat + 15)):
                p, v = G(lo, la)
                pts.append(p)
                vis += v
            if vis == 0:
                continue
            cells.setdefault(_anom_colour(a, _temperature_cell(lon, lat)), []).append(P(pts, True))
    # one small polar cap above 75N, a step darker than the ring below (Arctic amplification)
    # the Antarctic caps are always on the far side at this view
    cap = [G(lo, 75)[0] for lo in range(-180, 180, 5)]
    cells.setdefault(_anom_colour(a, _temperature_v(0, 78)), []).append(P(cap, True))
    for col, ds in cells.items():
        a.path("".join(ds), fill=col, stroke="#fff", sw=.5, so=.7)
    # continents as a thin coastline only (no tint, so the colour stays the anomaly): a mask
    # band just outside the union of the country rings, so internal borders never show
    land, mid = _temperature_land(G), a.uid("m")
    a.defs.append(f'<mask id="{mid}" maskUnits="userSpaceOnUse" x="0" y="0" width="320" height="180">'
                  f'<path d="{land}" fill="#fff" stroke="#fff" stroke-width="2.3" stroke-linejoin="round"/>'
                  f'<path d="{land}" fill="#000" stroke="#000" stroke-width="1.4" stroke-linejoin="round"/></mask>')
    a.path(f"M{n(G.c[0] - G.R)} {n(G.c[1] - G.R)}h{n(2 * G.R)}v{n(2 * G.R)}h{n(-2 * G.R)}Z",
           fill=PAL["ink"], fo=.42, mask=f"url(#{mid})")
    a.end()
    globe_shade(a, G, disc, op=.22)
    return a.render()


def _sea_temperature_lobes(pts, bulge=.32):
    """A cloud outline whose edges bulge outward as lobes, except the closing edge (last point
    back to the first), which stays straight as a flat cloud base."""
    from .kit import pt
    area = sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))
    sgn = 1 if area > 0 else -1
    d = f"M{pt(pts[0])}"
    for p, q in zip(pts, pts[1:]):
        dx, dy = q[0] - p[0], q[1] - p[1]
        L = math.hypot(dx, dy)
        c = ((p[0] + q[0]) / 2 + dy / L * sgn * L * bulge, (p[1] + q[1]) / 2 - dx / L * sgn * L * bulge)
        d += f"Q{pt(c)} {pt(q)}"
    return d + "Z"


def sea_temperature():
    """An El Nino section of the equatorial Pacific: a warm surface layer spread east along the
    equator over a flattened thermocline, deep convection shifted to the central Pacific, and a
    moored buoy whose thermistor chain samples the warm layer."""
    a = Art("verm", seed=43)
    o = (26, 166)
    Wd, D, Hh = 186.0, 124.0, 58.0
    f, t, r = box_faces(0, Wd, 0, Hh, D, o)
    a.shadow(o[0] + 150, o[1] + 4, 160, 7, op=.2)
    # ---- front face: the vertical section along the equator, west on the left
    a.path(P(f, True), fill=a.lin([(0, PAL["water3"]), (1, PAL["water5"])], 0, 0, 0, 1))

    def thermo(x):
        return Hh - 20 - 4 * math.sin(x / Wd * math.pi) + x / Wd * 5
    warm = [ob(x, 0, thermo(x), o) for x in [Wd * k / 24 for k in range(25)]]
    # warmest at the surface (matching the sea-surface band along the front edge), cooling with
    # depth down to the thermocline
    sub = mix(a.AL, PAL["water2"], .25)
    a.path(P([ob(0, 0, Hh, o), ob(Wd, 0, Hh, o)] + warm[::-1], True),
           fill=a.lin([(0, a.A), (.6, a.AL), (1, sub)], 0, 0, 0, 1))
    a.path(smooth(warm), stroke=PAL["ink"], sw=1.1, fill="none")
    # the warm pool sloshes east as the trade winds relax
    a.arrow([ob(Wd * .07, 0, Hh - 10, o), ob(Wd * .5, 0, Hh - 9, o)], color="#fff", w=1.6, head=6, curve=False)
    NB = 2.0
    outer = mix(a.AL, PAL["water2"], .45)
    # ---- right face (the eastern cut), with the warm layer continuing onto it
    a.path(P(r, True), fill=a.lin([(0, PAL["water4"]), (1, PAL["water5"])], 0, 0, 0, 1))
    # the warm layer on the eastern cut: thickest at the equator (front edge), thinning north;
    # the front edge is a straight segment so the fill can't bulge onto the front face
    y1 = 40 * NB * (1 - .35)
    th = Hh - thermo(Wd)
    lens = [ob(Wd, y1 * k / 16, Hh - th * math.cos(math.pi / 2 * k / 16) ** .6, o) for k in range(17)]
    sl = smooth(lens)
    a.path(P([ob(Wd, 0, Hh, o), lens[0]]) + sl[sl.index("c"):] + "Z",
           fill=a.lin([(0, a.A), (.3, a.AL), (1, outer)], 0, 0, 1, 0))
    # ---- sea surface: the warm tongue along the equator, widest in the west
    a.path(P(t, True), fill=a.lin([(0, PAL["water2"]), (1, PAL["water1"])], 0, 0, 1, 0))
    for w, col in ((40, outer), (22, a.AL), (9, a.A)):
        edge = [ob(x, min(D - 1, w * NB * (1 - .35 * (x / Wd) ** 1.4)), Hh, o) for x in [Wd * j / 16 for j in range(17)]]
        a.path(smooth(edge[::-1]) + "".join(f"L{n(p[0])} {n(p[1])}" for p in (ob(0, 0, Hh, o), ob(Wd, 0, Hh, o))) + "Z", fill=col)
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o")
    # ---- deep convection over the central Pacific: one cumulonimbus silhouette, a tower with a
    # flat base flaring into the anvil, and rain falling from the base to the sea
    cb = ob(Wd * .5, 18, Hh, o)
    cx, base = cb[0], cb[1] - 24
    tower = _sea_temperature_lobes([(cx + 22, base), (cx + 25, cb[1] - 44), (cx + 18, cb[1] - 64),
                                    (cx + 4, cb[1] - 76), (cx - 12, cb[1] - 72), (cx - 22, cb[1] - 56),
                                    (cx - 22, base)])
    # the anvil: flat-topped where the updraft hits the tropopause, sheared out downwind
    anv = smooth([(cx - 34, cb[1] - 74), (cx - 16, cb[1] - 83), (cx + 16, cb[1] - 86), (cx + 50, cb[1] - 85),
                  (cx + 70, cb[1] - 82), (cx + 40, cb[1] - 77), (cx + 16, cb[1] - 71), (cx - 10, cb[1] - 69)], True)
    # outline both shapes first, then fill both over the inner halves of the strokes, so the
    # tower and anvil read as one silhouette with no seam where they join
    for d in (tower, anv):
        a.path(d, fill="none", stroke=PAL["slate2"], sw=1.4, so=.8)
    a.path(tower, fill=a.lin([(0, PAL["cloud"]), (1, PAL["cloud2"])], 0, 0, .3, 1))
    a.path(anv, fill=a.lin([(0, "#fff"), (1, PAL["cloud"])], 0, 0, 0, 1))
    a.path(f"M{n(cx - 22)} {n(base)}h44", stroke=PAL["slate2"], sw=2, lc="round")
    a.path("".join(f"M{n(cx - 15 + k * 6)} {n(base + 2)}l-2 {n(cb[1] - base - 2)}" for k in range(6)),
           stroke=PAL["water4"], sw=.9, so=.85, fill="none", lc="round")
    # ---- moored buoy on the section plane: float at the surface, thermistor chain to the anchor
    bx = Wd * .74
    bp = ob(bx, 0, Hh, o)
    bot = ob(bx, 0, 0, o)
    a.path(P([bp, (bot[0], bot[1] - 3)]), stroke=PAL["ink"], sw=.8, fill="none")
    a.path(f"M{n(bot[0] - 4)} {n(bot[1])}h8v-3h-8Z", fill=PAL["slated"])
    for k in range(1, 4):
        a.circle(ob(bx, 0, Hh - k * 12, o), 2, fill="#fff", stroke=PAL["ink"], sw=.6)
    a.path(f"M{n(bp[0])} {n(bp[1] - 3)}v-9", stroke=PAL["ink"], sw=.9, fill="none", lc="round")
    a.circle((bp[0], bp[1] - 13), 1.8, fill=a.AD, stroke=PAL["ink"], sw=.5)
    a.path(f"M{n(bp[0] - 6)} {n(bp[1] - 3.5)}h12l-2 5h-8Z", fill=a.AD, stroke=PAL["ink"], sw=.5)
    return a.render()


def _precipitation_outline(a: Art, o, Wd, D, base, h, nx):
    """terrain()'s outline, but with the back hairline replaced by the true skyline of the surface
    (the shared helper draws the whole back edge over everything, even through the front face)."""
    xs = [Wd * i / nx for i in range(nx + 1)]
    a.path(P([ob(x, 0, h(x, 0), o) for x in xs]), cls="o")
    a.path(P([ob(Wd, D * j / 20, h(Wd, D * j / 20), o) for j in range(21)]), cls="o")
    a.path(P([ob(0, 0, h(0, 0), o), ob(0, 0, -base, o), ob(Wd, 0, -base, o), ob(Wd, D, -base, o), ob(Wd, D, h(Wd, D), o)])
           + P([ob(Wd, 0, -base, o), ob(Wd, 0, h(Wd, 0), o)]), cls="o")
    # skyline: for each screen column the highest surface point, i.e. the visible back edge
    # where it shows and the ridge itself where the ridge hides it
    top = {}
    for j in range(int(D * 2) + 1):
        y = j / 2
        for i in range(int(Wd * 2) + 1):
            x = i / 2
            X, Y = ob(x, y, h(x, y), o)
            k = round(X * 2)
            if k not in top or Y < top[k]:
                top[k] = Y
    ks = sorted(top)
    a.path(P([(k / 2, top[k]) for k in ks[::4]] + [(ks[-1] / 2, top[ks[-1]])]), cls="h", so=.6)


def precipitation():
    """Orographic precipitation over a mountain range: moist air forced up the windward slope
    cools into cloud; rain falls on the windward slope and snow above the freezing level, while
    the air sinks dry into the rain shadow in the lee. A satellite radar measures it from orbit."""
    a = Art("cobalt", seed=47)
    o = (14, 166)
    Wd, D = 214.0, 84.0
    YR = 4.0                        # depth of the cloud and its rain curtain
    ZC = 76.0                       # cloud base altitude
    ZF = 30.0                       # freezing level: snow on the ground above it
    NX = 36

    def h(x, y):
        return 46 * math.exp(-((x - 122) / 30) ** 2) * (0.97 + 0.03 * y / D) + 3   # one straight ridge, rising gently to the back

    def cover(x, y):
        z = h(x, y)
        if z > ZF + 2:
            return "snow"
        if x > 138:
            return "dry"
        return "wet" if z < 7 else "slope"

    dx = ob(0, 1)[0]                # screen x shift per unit of depth

    def seen(x, z):
        """Screen y where a vertical line at (x, YR) first disappears behind nearer terrain."""
        X, Y = ob(x, YR, z, o)
        for k in range(1, int(YR) + 1):
            yy = YR - k
            xx = X - o[0] - yy * dx
            Y = min(Y, ob(xx, yy, h(xx, yy), o)[1])
        return Y
    cols = {"snow": "#f4f6f7", "wet": PAL["moss"], "slope": PAL["sage2"], "dry": PAL["sand2"]}
    a.shadow(o[0] + 150, o[1] + 4, 125, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=NX, ny=10, cover=cover, colours=cols, base=10,
                   strata=[(5, PAL["clay"]), (99, PAL["rock"])], outline=False)
    _precipitation_outline(a, o, Wd, D, 10, h, NX)

    def trees(pts):
        a.path("".join(conifer(*surf(x, y, -.5), 13, 7) for x, y in pts), fill=PAL["forest"], stroke=PAL["forestd"], sw=.4)
    # a few conifers on the wet lowland upwind of the range
    trees([(30, 62), (52, 34), (20, 26)])
    # satellite almost overhead, its radar swath reaching down into the cloud (drawn behind it)
    bl = ob(62, YR, ZC, o)
    br = ob(134, YR, ZC, o)
    sat = (lerp(bl[0], br[0], .5), 22)
    a.path(P([(sat[0] - 4, sat[1] + 7), (bl[0] + 22, bl[1] - 24), (br[0] - 8, bl[1] - 24), (sat[0] + 4, sat[1] + 7)], True),
           fill=a.AP, fo=.55, stroke=a.AD, sw=.6, so=.7, da="2.4 1.8")
    a.path(f"M{n(sat[0] - 25)} {n(sat[1] - 4.5)}h16v9h-16ZM{n(sat[0] + 9)} {n(sat[1] - 4.5)}h16v9h-16Z",
           fill=PAL["slate2"], stroke=PAL["ink"], sw=.6)
    a.path(f"M{n(sat[0] - 9)} {n(sat[1])}h18", stroke=PAL["ink"], sw=.8)
    a.path(f"M{n(sat[0] - 6.5)} {n(sat[1] - 6.5)}h13v13h-13Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.7)
    # orographic cloud over the windward slope: flat base at ZC, banked up against the summit
    cl = scallop([bl, (bl[0] + 2, bl[1] - 14), (bl[0] + 18, bl[1] - 27), (bl[0] + 46, bl[1] - 34),
                  (br[0] - 4, bl[1] - 32), (br[0] + 12, bl[1] - 18), (br[0] + 10, bl[1] - 2), br],
                 bulge=.28, seed=3)
    # rain from the cloud base onto the windward slope, slanting downwind (cut where nearer terrain hides it)
    rain = []
    for k in range(5):
        x = 72 + k * 6
        zg = h(x, YR)
        if zg > ZF - 4:
            continue
        top = ob(x - 3, YR, ZC + 2, o)
        bot = ob(x, YR, zg + 3, o)  # stop just above the ground
        yb = seen(x, zg + 3) - 2.5
        if yb < bot[1]:               # hidden lower part: stop where the line meets the nearer slope
            t = (yb - top[1]) / (bot[1] - top[1])
            bot = lerp2(top, bot, t)
        rain.append(f"M{n(top[0])} {n(top[1])}L{n(bot[0])} {n(bot[1])}")
    a.path("".join(rain), stroke=a.A, sw=1.1, lc="round", fill="none")
    cl = cl.rsplit("Q", 1)[0] + "Z"   # flat cloud base
    a.path(cl, fill=a.lin([(0, "#fff"), (.55, PAL["cloud"]), (1, PAL["slate1"])], 0, 0, .5, 1))
    a.path(cl, cls="h", so=.6)
    # snow falling from the cloud onto the slope above the freezing level
    flakes = ""
    r = 6.2
    for x, z in ((100, 68), (110, 61), (120, 68)):
        assert ZC - 2 > z + r * .87 and z - r * .87 > h(x, YR) + 3
        c = ob(x, YR, z, o)
        flakes += (f"M{n(c[0] - r)} {n(c[1])}h{n(2 * r)}M{n(c[0] - r / 2)} {n(c[1] - r * .87)}l{n(r)} {n(r * 1.73)}"
                   f"M{n(c[0] + r / 2)} {n(c[1] - r * .87)}l{n(-r)} {n(r * 1.73)}")
    a.path(flakes, stroke=a.AD, sw=1.3, lc="round", fill="none")
    trees([(16, 2), (40, 1)])
    # moist air: in low from the left, forced up the windward slope, sinking dry in the lee
    pts = [surf(x, 2, dz) for x, dz in ((22, 9), (48, 8), (78, 7), (104, 6), (124, 6), (148, 8), (178, 8), (210, 8))]
    a.arrow(pts, color=a.A, w=1.6, head=8)
    return a.render()
