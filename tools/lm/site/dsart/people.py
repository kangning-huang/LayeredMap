"""Cities and people: world cities, life expectancy, population grid."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, box_faces, Ortho, globe, globe_shade,
                  countries, cities, scallop)


def _box(a: Art, x0, x1, y0, y1, z0, z1, o, front, top, right, hair=True):
    """An oblique box spanning x0..x1, depth y0..y1, height z0..z1."""
    f = [ob(x0, y0, z1, o), ob(x1, y0, z1, o), ob(x1, y0, z0, o), ob(x0, y0, z0, o)]
    t = [ob(x0, y0, z1, o), ob(x0, y1, z1, o), ob(x1, y1, z1, o), ob(x1, y0, z1, o)]
    r = [ob(x1, y0, z1, o), ob(x1, y1, z1, o), ob(x1, y1, z0, o), ob(x1, y0, z0, o)]
    a.path(P(f, True) + "", fill=front)
    a.path(P(t, True), fill=top)
    a.path(P(r, True), fill=right)
    if hair:
        a.path(P(f, True) + P([t[1], t[2], r[2]]), stroke=PAL["ink"], sw=.35, so=.7, fill="none", lj="round")


def _slab(a, o, Wd, D, th, top):
    f, t, r = box_faces(0, Wd, -th, 0, D, o)
    a.path(P(t, True), fill=top)
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    return f, t, r


def _outline(a, f, t, r):
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)


def cities_art():
    """A settled region: a river plain with a city, towns and villages joined by roads, each
    settlement ringed by a disc whose area is proportional to its population (the rank-size
    pattern a world cities layer shows), the central city's towers rising at its core."""
    a = Art("cobalt", seed=61)
    o = (12, 160)
    Wd, D = 150.0, 214.0
    a.shadow(o[0] + 150, o[1] + 3, 150, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, 6, PAL["sage1"])
    sxp, syp = Wd / 208, D / 150
    rnd = a.rng
    # fields texture, river and roads
    a.dots([ob(rnd.uniform(2, Wd - 2), rnd.uniform(2, D - 2), 0, o) for _ in range(300)], r=.35, color=PAL["moss"], op=.45)
    river = [ob(x * sxp, y * syp, 0, o) for x, y in ((0, 30), (40, 44), (74, 70), (92, 92), (130, 104), (170, 128), (208, 140))]
    a.path(smooth(river), stroke=PAL["water3"], sw=3.2, fill="none", lc="round")
    a.path(smooth(river), stroke=PAL["water2"], sw=1.6, fill="none", lc="round")
    places = [  # (x, y, population weight)
        (96, 66, 1.0), (40, 112, .32), (170, 96, .28), (150, 30, .2), (34, 30, .12), (186, 140, .1),
        (120, 128, .14), (76, 10, .07), (14, 72, .06), (200, 56, .08), (60, 148, .05), (130, 70, .05)]
    places = [(x * sxp, y * syp, w) for x, y, w in places]
    roads = [(0, 1), (0, 2), (0, 3), (0, 4), (2, 5), (0, 6), (3, 7), (1, 8), (2, 9), (1, 10), (0, 11), (6, 5)]
    for i, j in roads:
        p, q = places[i], places[j]
        a.path(P([ob(p[0], p[1], 0, o), ob(q[0], q[1], 0, o)]), stroke="#fff", sw=1.0 + .8 * min(p[2], q[2]) ** .3, fill="none", so=.9)
    # proportional discs on the ground (area ~ population)
    for (x, y, w) in sorted(places, key=lambda p: -p[2]):
        rr = 26 * math.sqrt(w)
        pts = [ob(x + rr * math.cos(t_), y + rr * math.sin(t_), 0, o) for t_ in [2 * math.pi * k / 40 for k in range(40)]]
        a.path(P(pts, True), fill=a.A, fo=.14, stroke=a.A, sw=.7, so=.8)
    # buildings: many small boxes, taller towards each centre; painter's order back to front
    boxes = []
    for (x, y, w) in places:
        nb = int(3 + 34 * w)
        spread = 5 + 16 * math.sqrt(w)
        for _ in range(nb):
            dx, dy = rnd.gauss(0, spread * .5), rnd.gauss(0, spread * .5)
            d = math.hypot(dx, dy) / spread
            hgt = max(2, (4 + 48 * w) * math.exp(-d * d * 2.5) * rnd.uniform(.5, 1.1))
            s = 3 + 2.2 * rnd.random()
            boxes.append((x + dx, y + dy, s, hgt))
    for (x, y, s, hgt) in sorted(boxes, key=lambda b: (-b[1], b[0])):
        if not (1 < x < Wd - s - 1 and 1 < y < D - s - 1):
            continue
        _box(a, x, x + s, y, y + s, 0, hgt, o, PAL["paper2"], "#fff", PAL["rock1"], hair=hgt > 4)
    _outline(a, f, t, r)
    return a.render()


def _survival(x, k):
    """Share of a birth cohort still alive at age x; k blends 1850 (0) to 2023 (1) mortality."""
    infant = lerp(.28, .02, k)
    a_ = lerp(.008, .0004, k)
    b = lerp(.004, .00002, k)
    c = lerp(.075, .105, k)
    return (1 - infant * (1 - math.exp(-x / 1.2))) * math.exp(-a_ * x - b / c * (math.exp(c * x) - 1))


def life_expectancy():
    """Survival curves (share of people alive at each age) rectangularising as mortality falls,
    from the mid-nineteenth century to today, and the present-day pattern by country on a globe."""
    a = Art("cobalt", seed=63)
    x0, x1, yb, yt = 20, 168, 152, 30
    a.path(P([(x0, yt), (x0, yb), (x1, yb)]), cls="o", sw=.8)
    sx = lambda age: lerp(x0, x1 - 4, age / 105)
    sy = lambda v: lerp(yb, yt + 6, v)
    ks = [0, .35, .7, 1]
    curves = []
    for k in ks:
        pts = [(sx(age), sy(_survival(age, k))) for age in range(0, 106, 3)]
        curves.append(pts)
    a.path(P(curves[-1] + [(sx(105), yb), (x0, yb)], True), fill=a.AP, fo=.8)
    a.path(P(curves[0] + [(sx(105), yb), (x0, yb)], True), fill=a.AL, fo=.35)
    for i, pts in enumerate(curves):
        last = i == len(curves) - 1
        a.path(smooth(pts), stroke=a.A if last else PAL["ink"], sw=1.3 if last else .7, so=1 if last else .35 + .2 * i, fill="none")
    for k in range(0, 106, 20):
        a.path(f"M{n(sx(k))} {n(yb)}v2.5", cls="h", so=.8)
    for v in (.25, .5, .75, 1):
        a.path(f"M{n(x0)} {n(sy(v))}h-2.5", cls="h", so=.8)
    a.path(f"M{n(x0)} {n(sy(.5))}H{n(x1 - 4)}", cls="h", da="2 1.6", so=.5)
    a.arrow([(sx(32), sy(.5) + 0), (sx(80), sy(.5))], color=a.AD, w=1.0, head=4.4, curve=False)
    # ---- globe: life expectancy by country, darker = longer lives (continent averages)
    G = Ortho(20, 20, 62, (240, 92))
    disc, cp = globe(a, G, land=False, grat=True)
    le = {"Africa": .25, "Asia": .6, "Europe": .92, "North America": .8, "South America": .7, "Oceania": .85}
    import json
    from .kit import REPO
    d = json.loads((REPO / "assets" / "examples" / "countries.geojson").read_text(encoding="utf-8"))
    groups = {}
    for ft in d["features"]:
        cont = ft["properties"].get("continent")
        v = le.get(cont, .6)
        iso = ft["properties"].get("iso_a3")
        v += (sum(map(ord, iso or "x")) % 5 - 2) * .04
        col = mix(a.AP, a.AD, max(0, min(1, v)))
        groups.setdefault(col, set()).add(iso)
    a.group(cp=cp)
    for col, isos in groups.items():
        a.path(G.land(.8, only=isos), fill=col, stroke="#fff", sw=.3, lj="round")
    a.end()
    globe_shade(a, G, disc, op=.2)
    return a.render()


def _pip(x, y, ring):
    ins = False
    for (x1, y1), (x2, y2) in zip(ring, ring[1:] + ring[:1]):
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1 + 1e-12) + x1:
            ins = not ins
    return ins


def population():
    """A population surface for China as extruded 1 km-style grid cells (coarsened here), built
    from the bundled cities: dense in the east and south-east, sparse in the west, split by the
    Hu Line from Heihe to Tengchong."""
    a = Art("amber", seed=65)
    o = (12, 164)
    lon0, lon1, lat0, lat1 = 73.0, 135.0, 18.0, 54.0
    Wd, D = 170.0, 196.0
    X = lambda lon: (lon - lon0) / (lon1 - lon0) * Wd
    Y = lambda lat: (lat - lat0) / (lat1 - lat0) * D
    a.shadow(o[0] + 160, o[1] + 3, 150, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, 5, PAL["water1"])
    chn = [ring for iso, ring in countries() if iso == "CHN"]
    main = max(chn, key=len)
    for ring in chn:
        a.path(P([ob(X(lo), Y(la), 0, o) for lo, la in ring], True), fill=PAL["sand1"], stroke=PAL["ink"], sw=.5, lj="round")
    a.path(P([ob(X(lo), Y(la), 0, o) for lo, la in main], True), fill=PAL["sand1"], stroke=PAL["ink"], sw=.5, lj="round")
    ch = [c for c in cities() if lon0 < c[0] < lon1 and lat0 < c[1] < lat1 and _pip(c[0], c[1], main)]
    dl, dp = 2.4, 2.0
    cells = []
    lat = lat1 - dp
    while lat >= lat0:
        lon = lon0
        while lon < lon1:
            cx, cy = lon + dl / 2, lat + dp / 2
            if _pip(cx, cy, main):
                v = 0.0
                for (clo, cla, pop, _) in ch:
                    dd = ((clo - cx) / 2.2) ** 2 + ((cla - cy) / 2.0) ** 2
                    v += (pop or 0) * math.exp(-dd)
                v = math.sqrt(v / 1.5e6)
                v += .35 * max(0, (cx - 100) / 35) if cx > 100 else 0
                cells.append((lon, lat, v))
            lon += dl
        lat -= dp
    vmax = max(c[2] for c in cells)
    for (lon, lat, v) in cells:      # back (north) to front, west to east
        hgt = 1 + 44 * (v / vmax) ** .8
        t_ = min(1, v / vmax * 1.4)
        top = mix(a.AP, a.A, round(t_ * 8) / 8)
        x0_, x1_ = X(lon) + .25, X(lon + dl) - .25
        y0_, y1_ = Y(lat) + .25, Y(lat + dp) - .25
        _box(a, x0_, x1_, y0_, y1_, 0, hgt, o, mix(top, PAL["ink"], .12), top, mix(top, PAL["ink"], .3), hair=False)
    # the Hu Line
    hu = [ob(X(127.5), Y(50.2), 0, o), ob(X(98.5), Y(25.0), 0, o)]
    a.path(P(hu), stroke=PAL["ink"], sw=1.0, da="4 2.4")
    for p in hu:
        a.circle(p, 1.8, fill="#fff", stroke=PAL["ink"], sw=.7)
    _outline(a, f, t, r)
    return a.render()
