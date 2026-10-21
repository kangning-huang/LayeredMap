"""Points of interest and OpenStreetMap: chain stores, overpass queries, city and country extracts,
hawker centres."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, box_faces, box3, hiproof, gable, countries, scallop)


def _clip_half(poly, a, b):
    """Keep the part of poly closer to site a than to site b (Sutherland-Hodgman)."""
    mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
    nx, ny = b[0] - a[0], b[1] - a[1]
    inside = lambda p: (p[0] - mx) * nx + (p[1] - my) * ny <= 0
    out = []
    for i in range(len(poly)):
        p, q = poly[i], poly[(i + 1) % len(poly)]
        ip, iq = inside(p), inside(q)
        if ip:
            out.append(p)
        if ip != iq:
            dp = (p[0] - mx) * nx + (p[1] - my) * ny
            dq = (q[0] - mx) * nx + (q[1] - my) * ny
            t = dp / (dp - dq)
            out.append((lerp(p[0], q[0], t), lerp(p[1], q[1], t)))
    return out


def voronoi(sites, rect):
    x0, y0, x1, y1 = rect
    cells = []
    for i, s in enumerate(sites):
        poly = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        for j, t in enumerate(sites):
            if i != j and poly:
                poly = _clip_half(poly, s, t)
        cells.append(poly)
    return cells


def _slab(a, o, Wd, D, th, top):
    f, t, r = box_faces(0, Wd, -th, 0, D, o)
    a.path(P(t, True), fill=top)
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    return f, t, r


def _outline(a, f, t, r, sw=.9):
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=sw)


def chain_stores():
    """A city's street grid with the branches of one chain and its market areas: each branch
    serves the part of the city nearer to it than to any other branch (Voronoi cells), while
    competitors' branches dot the same streets."""
    a = Art("verm", seed=121)
    o = (12, 162)
    Wd, D = 170.0, 190.0
    a.shadow(o[0] + 150, o[1] + 3, 150, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, 6, PAL["paper2"])
    rnd = a.rng
    # blocks between streets, with a slight irregularity, and a river
    xs = [0, 18, 38, 60, 80, 100, 124, 146, 170]
    ys = [0, 20, 44, 66, 92, 116, 140, 166, 190]
    blocks = []
    for i in range(len(xs) - 1):
        for j in range(len(ys) - 1):
            q = [ob(xs[i] + 2, ys[j] + 2, 0, o), ob(xs[i] + 2, ys[j + 1] - 2, 0, o), ob(xs[i + 1] - 2, ys[j + 1] - 2, 0, o), ob(xs[i + 1] - 2, ys[j] + 2, 0, o)]
            blocks.append(P(q, True))
    a.path("".join(blocks), fill="#f1ece1", stroke=PAL["rock1"], sw=.4)
    park = [ob(62, 68, 0, o), ob(62, 90, 0, o), ob(78, 90, 0, o), ob(78, 68, 0, o)]
    a.path(P(park, True), fill=PAL["sage2"])
    sites = [(28, 30), (92, 22), (150, 40), (50, 104), (112, 84), (152, 128), (24, 168), (90, 152), (136, 176)]
    cells = voronoi(sites, (0, 0, Wd, D))
    for k, c in enumerate(cells):
        a.path(P([ob(x, y, 0, o) for x, y in c], True), fill=a.AP if k % 2 else mix(a.AP, "#fff", .4), fo=.6, stroke=a.A, sw=.8, so=.85)
    # competitors (grey) and the chain (accent pins)
    others = [(rnd.uniform(4, Wd - 4), rnd.uniform(4, D - 4)) for _ in range(26)]
    for (x, y) in sorted(others, key=lambda p: -p[1]):
        p = ob(x, y, 0, o)
        a.circle(p, 1.5, fill=PAL["slate1"], stroke=PAL["ink"], sw=.4)
    for (x, y) in sorted(sites, key=lambda p: -p[1]):
        p = ob(x, y, 0, o)
        a.ellipse((p[0] + 2, p[1] + .5), 2.6, .9, fill=PAL["ink"], fo=.25)
        a.path(f"M{n(p[0])} {n(p[1])}c-2.4-3.2-4-4.8-4-6.8a4 4 0 0 1 8 0c0 2-1.6 3.6-4 6.8Z", fill=a.A, stroke=PAL["ink"], sw=.5)
        a.circle((p[0], p[1] - 6.8), 1.5, fill="#fff")
    _outline(a, f, t, r)
    return a.render()


def openstreetmap():
    """The OpenStreetMap data model pulled apart in layers: the map as drawn, then its ways (an
    ordered list of nodes, open for roads, closed for buildings), then the tagged nodes on top,
    with the features matching an overpass query picked out and lifted."""
    a = Art("cobalt", seed=123)
    Wd, D = 150.0, 110.0
    levels = [(0, 160), (1, 118), (2, 76)]          # (index, origin y)
    ox = 30
    roads = [[(0, 30), (40, 34), (80, 46), (120, 50), (150, 56)], [(56, 0), (60, 30), (66, 70), (70, 110)],
             [(96, 0), (100, 48), (110, 110)], [(0, 84), (66, 76), (150, 92)]]
    bldgs = [[(10, 44), (30, 44), (30, 60), (10, 60)], [(74, 56), (92, 56), (92, 68), (74, 68)], [(108, 62), (130, 62), (130, 80), (108, 80)],
             [(16, 92), (40, 92), (40, 104), (16, 104)], [(110, 14), (136, 14), (136, 32), (110, 32)], [(20, 6), (44, 6), (44, 22), (20, 22)]]
    pois = [(36, 72), (86, 22), (124, 98), (46, 38), (84, 96), (138, 70)]
    match = {0, 2, 4}
    for (lv, oy) in levels:
        o = (ox, oy)
        plane = [ob(0, 0, 0, o), ob(0, D, 0, o), ob(Wd, D, 0, o), ob(Wd, 0, 0, o)]
        if lv == 0:
            a.shadow(ox + 100, oy + 3, 110, 5, op=.18)
            a.path(P(plane, True), fill=PAL["sand1"])
            a.path(P([ob(0, 64, 0, o), ob(40, 58, 0, o), ob(150, 70, 0, o), ob(150, 60, 0, o), ob(40, 50, 0, o), ob(0, 56, 0, o)], True), fill=PAL["water2"])
            a.path(P([ob(118, 84, 0, o), ob(118, 108, 0, o), ob(148, 108, 0, o), ob(148, 84, 0, o)], True), fill=PAL["sage2"])
            for rd in roads:
                a.path(P([ob(x, y, 0, o) for x, y in rd]), stroke="#fff", sw=2.6, fill="none", lj="round")
                a.path(P([ob(x, y, 0, o) for x, y in rd]), stroke=PAL["rock"], sw=.4, fill="none", so=.6)
            a.path("".join(P([ob(x, y, 0, o) for x, y in b], True) for b in bldgs), fill=PAL["rock1"], stroke=PAL["ink"], sw=.4)
        else:
            a.path(P(plane, True), fill="#ffffff", fo=.55, stroke=PAL["ink"], sw=.4, so=.5)
        if lv == 1:
            for rd in roads:
                a.path(P([ob(x, y, 0, o) for x, y in rd]), stroke=PAL["ink2"], sw=.8, fill="none")
                a.dots([ob(x, y, 0, o) for x, y in rd], r=1.1, color=PAL["ink"])
                a.dots([ob(x, y, 0, o) for x, y in rd], r=.5, color="#fff")
            for b in bldgs:
                a.path(P([ob(x, y, 0, o) for x, y in b], True), stroke=PAL["ink2"], sw=.7, fill="none")
                a.dots([ob(x, y, 0, o) for x, y in b], r=1.0, color=PAL["ink"])
        if lv == 2:
            for k, (x, y) in enumerate(pois):
                p = ob(x, y, 0, o)
                if k in match:
                    a.path(P([p, (p[0], p[1] - 10)]), stroke=a.A, sw=.7)
                    a.circle((p[0], p[1] - 12), 3, fill=a.A, stroke="#fff", sw=.7)
                    a.path(f"M{n(p[0] + 4)} {n(p[1] - 14)}h6M{n(p[0] + 4)} {n(p[1] - 11.5)}h4", stroke=a.AD, sw=.6)   # tag lines
                    a.ellipse(p, 3, 1.2, fill=a.AP, stroke=a.A, sw=.5)
                else:
                    a.circle(p, 1.6, fill=PAL["slate1"], stroke=PAL["ink"], sw=.4)
                    a.path(f"M{n(p[0] + 2.6)} {n(p[1] - 1)}h4", stroke=PAL["ink"], sw=.4, so=.5)
        if lv == 0:
            a.path(P(plane, True), cls="o", sw=.9)
    # corner connectors between the layers
    for (x, y) in ((0, 0), (Wd, 0), (Wd, D)):
        a.path(P([ob(x, y, 0, (ox, 160)), ob(x, y, 0, (ox, 76))]), cls="h", da="1.6 1.6", so=.6)
    return a.render()


def city_extract():
    """A box drawn round a city on the OpenStreetMap base, and the extract lifted out of it as a
    block of its own: streets, river, buildings and shops inside the bounding box, nothing outside."""
    a = Art("cobalt", seed=125)
    o = (10, 170)
    Wd, D = 190.0, 140.0
    rnd = a.rng
    river = lambda x: 70 + 18 * math.sin(x / 30)

    def city(o, x0, x1, y0, y1, lift=0.0, buildings=True):
        q = lambda x, y: ob(x, y, lift, o)
        a.path(P([q(x0, y0), q(x0, y1), q(x1, y1), q(x1, y0)], True), fill=PAL["sand1"])
        rv = [(x, river(x)) for x in range(int(x0), int(x1) + 1, 6)]
        a.path(P([q(x, y - 5) for x, y in rv] + [q(x, y + 5) for x, y in rv][::-1], True), fill=PAL["water2"])
        st = ""
        for k in range(int(x0 // 16) + 1, int(x1 // 16) + 1):
            x = k * 16
            if x0 < x < x1:
                st += P([q(x, y0), q(x + 6, y1)])
        for k in range(int(y0 // 18) + 1, int(y1 // 18) + 1):
            y = k * 18
            if y0 < y < y1:
                st += P([q(x0, y), q(x1, y - 4)])
        a.path(st, stroke="#fff", sw=1.4, fill="none")
        if buildings:
            for (bx, by) in sorted(blds, key=lambda b: (-b[1], b[0])):
                if x0 + 2 < bx < x1 - 6 and y0 + 2 < by < y1 - 6:
                    hh = 2 + 4 * rnd.random()
                    box3(a, bx, bx + 4.5, by, by + 4.5, lift, lift + hh, o, PAL["paper2"], "#fff", PAL["rock1"], hair=False)
    blds = []
    for _ in range(260):
        x, y = rnd.uniform(4, Wd - 8), rnd.uniform(4, D - 8)
        if abs(y - river(x)) > 9 and (x % 16) > 3 and (y % 18) > 3:
            blds.append((x, y))
    a.shadow(o[0] + 150, o[1] + 3, 150, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, 5, PAL["sand1"])
    bx0, bx1, by0, by1 = 40.0, 136.0, 26.0, 108.0
    city(o, 0, Wd, 0, D, buildings=False)
    # the hole left in the base where the extract was lifted
    a.path(P([ob(bx0, by0, 0, o), ob(bx0, by1, 0, o), ob(bx1, by1, 0, o), ob(bx1, by0, 0, o)], True), fill=a.AP, fo=.6, stroke=a.A, sw=.9, da="3 1.8")
    for (x, y) in ((bx0, by0), (bx0, by1), (bx1, by1), (bx1, by0)):
        p = ob(x, y, 0, o)
        a.path(f"M{n(p[0] - 2)} {n(p[1] - 2)}h4v4h-4Z", fill="#fff", stroke=a.A, sw=.8)
    _outline(a, f, t, r)
    # the lifted extract: a thin block with the city on top
    lift = 42.0
    ff, tt, rr = box_faces(bx0, bx1, lift - 6, lift, by1 - by0, ob(0, by0, 0, o))
    city(o, bx0, bx1, by0, by1, lift=lift)
    a.path(P(ff, True), fill=PAL["soil"])
    a.path(P(rr, True), fill=PAL["umber"])
    a.path(P(ff, True) + P(tt, True) + P([rr[1], rr[2], rr[3]]), stroke=a.AD, sw=1.0, fill="none", lj="round")
    for (x, y) in ((bx0, by0), (bx1, by0), (bx1, by1)):
        a.arrow([ob(x, y, 2, o), ob(x, y, lift - 6, o)], color=a.A, w=.7, head=3.4, curve=False, da="1.8 1.4")
    return a.render()


def country_extract():
    """A whole country from OpenStreetMap separated into the thematic layers of a shapefile
    extract, stacked like a cut-away: land use and water at the bottom, then roads, railways,
    buildings and points of interest, each a layer of the same outline."""
    a = Art("cobalt", seed=127)
    ring = max((r for iso, r in countries() if iso == "DEU"), key=len)
    lons = [p[0] for p in ring]
    lats = [p[1] for p in ring]
    lo0, lo1, la0, la1 = min(lons), max(lons), min(lats), max(lats)
    S = 15.0

    def X(lon):
        return (lon - lo0) * S * math.cos(math.radians(51))

    def Y(lat):
        return (lat - la0) * S * 1.0
    pts = [(X(lo), Y(la)) for lo, la in ring]
    Wd, D = max(p[0] for p in pts), max(p[1] for p in pts)
    rnd = a.rng
    inside = lambda x, y: _pip(x, y, pts)
    layers = ["base", "roads", "rail", "bldg", "poi"]
    gap = 21.0
    ox, oy = 70, 168
    for k, kind in enumerate(layers):
        o = (ox + k * 6, oy - k * gap)
        outline = [ob(x, y, 0, o) for x, y in pts]
        if k == 0:
            a.path(P([(p[0] + 3, p[1] + 3) for p in outline], True), fill=PAL["ink"], fo=.12)
        thick = [(p[0], p[1] + 2.4) for p in outline]
        a.path(P(thick, True), fill=PAL["rock"] if kind != "roads" else a.AD)
        fill = {"base": PAL["sage1"], "roads": "#ffffff", "rail": "#ffffff", "bldg": "#ffffff", "poi": "#ffffff"}[kind]
        a.path(P(outline, True), fill=fill, fo=1 if kind == "base" else .82, stroke=PAL["ink"] if kind != "roads" else a.A,
               sw=.6 if kind != "roads" else .9, lj="round")
        cp = a.clip(P(outline, True))
        a.group(cp=cp)
        if kind == "base":
            for _ in range(10):
                cx, cy = rnd.uniform(0, Wd), rnd.uniform(0, D)
                a.path(smooth([ob(cx + 9 * math.cos(t), cy + 7 * math.sin(t), 0, o) for t in [2 * math.pi * i / 7 for i in range(7)]], True),
                       fill=PAL["forest"] if rnd.random() < .6 else PAL["sand2"], fo=.6)
            a.path(smooth([ob(x, y, 0, o) for x, y in ((Wd * .55, D), (Wd * .5, D * .7), (Wd * .38, D * .45), (Wd * .3, D * .2), (Wd * .2, 0))]),
                   stroke=PAL["water3"], sw=1.2, fill="none")
        elif kind == "roads":
            hubs = [(Wd * .3, D * .8), (Wd * .7, D * .75), (Wd * .45, D * .45), (Wd * .25, D * .3), (Wd * .7, D * .2), (Wd * .55, D * .9)]
            d = ""
            for i, p in enumerate(hubs):
                for q in hubs[i + 1:]:
                    if math.hypot(p[0] - q[0], p[1] - q[1]) < Wd * .5:
                        d += P([ob(*p, 0, o), ob(*q, 0, o)])
            a.path(d, stroke=a.A, sw=1.0, fill="none")
        elif kind == "rail":
            d = P([ob(Wd * .2, D * .85, 0, o), ob(Wd * .45, D * .45, 0, o), ob(Wd * .72, D * .2, 0, o)]) + P([ob(Wd * .3, D * .3, 0, o), ob(Wd * .7, D * .78, 0, o)])
            a.path(d, stroke=PAL["ink"], sw=1.2, fill="none")
            a.path(d, stroke="#fff", sw=.6, da="2 2", fill="none")
        elif kind == "bldg":
            dots = []
            for _ in range(220):
                x, y = rnd.uniform(0, Wd), rnd.uniform(0, D)
                near = min(math.hypot(x - hx, y - hy) for hx, hy in ((Wd * .3, D * .8), (Wd * .7, D * .75), (Wd * .45, D * .45), (Wd * .7, D * .2)))
                if near < 14 + rnd.random() * 8:
                    dots.append(ob(x, y, 0, o))
            a.path("".join(f"M{n(p[0])} {n(p[1])}h1.4v-1h-1.4Z" for p in dots), fill=PAL["slate2"])
        else:
            pp = [ob(rnd.uniform(0, Wd), rnd.uniform(0, D), 0, o) for _ in range(40)]
            for p in pp:
                a.circle(p, .9, fill=PAL["slate1"], stroke=PAL["ink"], sw=.3)
        a.end()
    for (x, y) in (pts[min(range(len(pts)), key=lambda i: pts[i][0])], pts[max(range(len(pts)), key=lambda i: pts[i][0])]):
        a.path(P([ob(x, y, 0, (ox, oy)), ob(x, y, 0, (ox + 24, oy - 4 * gap))]), cls="h", da="1.6 1.6", so=.6)
    return a.render()


def _pip(x, y, ring):
    ins = False
    for (x1, y1), (x2, y2) in zip(ring, ring[1:] + ring[:1]):
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1 + 1e-12) + x1:
            ins = not ins
    return ins


def hawker():
    """A Singapore hawker centre in section: an open-sided pavilion under a raised, ventilated
    roof, stalls along both sides with their cooking hoods, shared tables in the middle and
    ceiling fans; cooler air comes in at the open sides and warm air and cooking fumes rise out
    through the roof vent (the stack effect)."""
    a = Art("verm", seed=129)
    gy = 150
    D = 70.0
    o = (16, gy)
    # ground slab
    f, t, r = box_faces(0, 248, -6, 0, D, o)
    a.shadow(170, gy + 8, 150, 6, op=.2)
    a.path(P(t, True), fill="#e7e1d4")
    a.path(P(f, True), fill=PAL["rock1"])
    a.path(P(r, True), fill=PAL["rock"])
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)
    a.path("".join(P([ob(x, 0, 0, o), ob(x, D, 0, o)]) for x in range(30, 248, 30)), stroke=PAL["ink"], sw=.3, so=.25)
    X0, X1, colh = 26.0, 222.0, 46.0
    # columns at the back row
    for x in (X0, X0 + 65, X0 + 130, X1):
        p, q = ob(x, D - 6, 0, o), ob(x, D - 6, colh, o)
        a.path(P([p, q]), stroke=PAL["rock"], sw=2.4)
    # stalls along the back: counters, hoods, woks
    for k in range(6):
        x = X0 + 4 + k * 32
        box3(a, x, x + 28, D - 22, D - 8, 0, 12, o, mix(PAL["sand1"], "#fff", .3), "#fbf9f4", PAL["sand2"], sw=.3)
        box3(a, x + 2, x + 26, D - 20, D - 10, 26, 30, o, PAL["rock1"], "#f1efe9", PAL["rock"], sw=.3)    # hood
        for j in range(2):
            p = ob(x + 8 + j * 12, D - 15, 12, o)
            a.ellipse(p, 3.6, 1.2, fill=PAL["slated"])
            a.path(f"M{n(p[0] - 1)} {n(p[1] - 2)}c-2-4 2-6 0-10M{n(p[0] + 2)} {n(p[1] - 2)}c-2-4 2-6 0-10", stroke=PAL["slate1"], sw=.5, fill="none", so=.8)
        a.path(P([ob(x + 15, D - 15, 30, o), ob(x + 15, D - 15, colh, o)]), stroke=PAL["rock"], sw=1.0)        # exhaust duct
    # tables and stools in the middle
    for i in range(5):
        for j in range(2):
            cx, cy = X0 + 20 + i * 38, 18 + j * 20
            for (dx, dy) in ((-7, 0), (7, 0), (0, -6), (0, 6)):
                p = ob(cx + dx, cy + dy, 0, o)
                a.path(f"M{n(p[0])} {n(p[1])}v-4", stroke=PAL["slate2"], sw=.8)
                a.ellipse((p[0], p[1] - 4), 1.8, .7, fill=a.AL)
            p = ob(cx, cy, 0, o)
            a.path(f"M{n(p[0])} {n(p[1])}v-7", stroke=PAL["slate2"], sw=1.2)
            a.ellipse((p[0], p[1] - 7), 5.4, 2.2, fill="#f6f3ec", stroke=PAL["ink"], sw=.4)
    # front columns
    for x in (X0, X0 + 65, X0 + 130, X1):
        p, q = ob(x, 4, 0, o), ob(x, 4, colh, o)
        a.path(P([p, q]), stroke=PAL["ink"], sw=3.0)
        a.path(P([p, q]), stroke=PAL["rock1"], sw=2.2)
    # the roof, cut away over the near half so the hall shows: lower slopes, the vent gap and
    # the raised clerestory roof above it; cut edges drawn as a profile
    zr = colh
    ov = 10.0
    xm = (X0 + X1) / 2
    yc = D * .5                       # cut plane
    el, er = X0 - ov, X1 + ov
    prof = lambda y: [ob(el, y, zr, o), ob(xm - 22, y, zr + 16, o)]
    low = [ob(el, yc, zr, o), ob(xm - 22, yc, zr + 16, o), ob(xm - 22, D + 4, zr + 16, o), ob(el, D + 4, zr, o)]
    rgt = [ob(xm + 22, yc, zr + 16, o), ob(er, yc, zr, o), ob(er, D + 4, zr, o), ob(xm + 22, D + 4, zr + 16, o)]
    a.path(P(low, True), fill=mix(a.A, "#fff", .25), stroke=PAL["ink"], sw=.5)
    a.path("".join(P([lerp2(low[0], low[1], k / 8), lerp2(low[3], low[2], k / 8)]) for k in range(1, 8)), stroke=PAL["ink"], sw=.25, so=.35)
    vent = [ob(xm - 22, yc, zr + 16, o), ob(xm + 22, yc, zr + 16, o), ob(xm + 22, yc, zr + 21, o), ob(xm - 22, yc, zr + 21, o)]
    a.path(P(vent, True), fill=PAL["slated"], fo=.8)
    a.path("".join(P([lerp2(vent[0], vent[1], k / 10), lerp2(vent[3], vent[2], k / 10)]) for k in range(11)), stroke=PAL["rock1"], sw=.6)
    up = [ob(xm - 30, yc, zr + 20, o), ob(xm, yc, zr + 29, o), ob(xm + 30, yc, zr + 20, o)]
    ul = [up[0], up[1], ob(xm, D + 4, zr + 29, o), ob(xm - 30, D + 4, zr + 20, o)]
    ur = [up[1], up[2], ob(xm + 30, D + 4, zr + 20, o), ob(xm, D + 4, zr + 29, o)]
    a.path(P(rgt, True), fill=mix(a.A, PAL["ink"], .15), stroke=PAL["ink"], sw=.5)
    a.path(P(ul, True), fill=mix(a.A, "#fff", .25), stroke=PAL["ink"], sw=.5)
    a.path(P(ur, True), fill=mix(a.A, PAL["ink"], .15), stroke=PAL["ink"], sw=.5)
    # cut profile (thick) along the section plane
    a.path(P([low[0], low[1]]) + P([rgt[0], rgt[1]]) + P(up), stroke=PAL["ink"], sw=2.0, fill="none", lc="round")
    # ceiling fans
    for x in (X0 + 40, X1 - 40):
        p = ob(x, 20, zr - 2, o)
        a.path(f"M{n(p[0])} {n(p[1] - 6)}v6", stroke=PAL["ink"], sw=.5)
        a.ellipse(p, 9, 1.4, fill=PAL["slate1"], stroke=PAL["ink"], sw=.35)
    # airflow: cool air in at the sides, warm air rising out of the vent
    for side in (-1, 1):
        x0 = (X0 - 22) if side < 0 else (X1 + 22)
        a.arrow([ob(x0, 10, 14, o), ob(x0 - side * 30, 10, 12, o), ob(xm - side * 40, 10, 22, o)], color=PAL["water4"], w=1.0, head=4)
    a.arrow([ob(xm - 14, yc, 18, o), ob(xm - 10, yc, 36, o), ob(xm - 16, yc, zr + 18, o), ob(xm - 44, yc, zr + 30, o)], color=a.A, w=1.1, head=4.4)
    a.arrow([ob(xm + 14, yc, 18, o), ob(xm + 10, yc, 36, o), ob(xm + 16, yc, zr + 18, o), ob(xm + 44, yc, zr + 30, o)], color=a.A, w=1.1, head=4.4)
    return a.render()
