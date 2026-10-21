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
    """A city block of streets with the branches of one chain and their market areas: each
    branch serves the part of the city nearer to it than to any other branch (Voronoi cells),
    while a few competitors' shops sit on the same streets."""
    a = Art("verm", seed=121)
    o = (12, 146)
    Wd, D = 144.0, 224.0
    a.shadow(o[0] + 150, o[1] + 4, 145, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, 8, PAL["paper2"])
    top = P(t, True)
    q = lambda x, y: ob(x, y, 0, o)
    sites = [(30, 40), (104, 30), (66, 100), (122, 120), (24, 160), (96, 196)]
    cells = voronoi(sites, (0, 0, Wd, D))
    focus = 2                                   # one market area picked out
    for k, c in enumerate(cells):
        a.path(P([q(x, y) for x, y in c], True), fill=a.AL if k == focus else a.AP, fo=.6 if k == focus else .35)
    # a plain street grid: three avenues each way
    st = "".join(P([q(x, 0), q(x, D)]) for x in (35, 70, 105)) + "".join(P([q(0, y), q(Wd, y)]) for y in (55, 110, 165))
    # streets and market-area boundaries, clipped to the block top so none run past its edges
    a.group(cp=a.clip(top))
    a.path(st, stroke="#fff", sw=2.2, fill="none", so=.85)
    a.path("".join(P([q(x, y) for x, y in c], True) for c in cells), fill="none", stroke=a.A, sw=1.1, lj="round")
    a.end()
    _outline(a, f, t, r)
    # competitors: a handful of grey shops on the streets
    others = [(35, 84), (70, 30), (105, 70), (52, 165), (128, 165), (70, 140), (105, 210), (14, 110)]
    for (x, y) in sorted(others, key=lambda p: -p[1]):
        a.circle(q(x, y), 2.5, fill=PAL["slate1"], stroke=PAL["ink"], sw=.55)
    # the chain's branches, standing pins with their tips on the ground
    for (x, y) in sorted(sites, key=lambda p: -p[1]):
        p = q(x, y)
        a.ellipse((p[0] + 1.8, p[1] + .4), 3.8, 1.3, fill=PAL["ink"], fo=.25)
        a.path(f"M{n(p[0])} {n(p[1])}c-3.6-4.8-6-7.2-6-10.2a6 6 0 0 1 12 0c0 3-2.4 5.4-6 10.2Z", fill=a.AD if sites.index((x, y)) == focus else a.A, stroke=PAL["ink"], sw=.6)
        a.circle((p[0], p[1] - 10.2), 2.3, fill="#fff")
    return a.render()


def openstreetmap():
    """The OpenStreetMap data model pulled apart in layers: the map as drawn, then its ways (an
    ordered list of nodes, open for roads, closed for buildings, crossing roads sharing a node),
    then the tagged nodes on top, with the features matching an overpass query picked out and lifted."""
    a = Art("cobalt", seed=123)
    Wd, D = 165.0, 80.0
    ox, gap = 50, 46
    levels = [(0, 160), (1, 160 - gap), (2, 160 - 2 * gap)]          # (index, origin y)
    # two roads that cross at a shared node, three buildings, four points of interest
    roads = [[(0, 26), (60, 30), (112, 34), (165, 40)], [(84, 0), (88, 32), (96, 80)]]
    bldgs = [[(16, 44), (50, 44), (50, 66), (16, 66)], [(112, 50), (146, 50), (146, 70), (112, 70)],
             [(26, 6), (58, 6), (58, 18), (26, 18)]]
    park = [(116, 6), (122, 24), (156, 26), (154, 6)]
    pois = [(33, 55), (129, 60), (130, 14), (42, 12)]
    match = {0, 1}
    for (lv, oy) in levels:
        o = (ox, oy)
        q = lambda pts: [ob(x, y, 0, o) for x, y in pts]
        plane = q([(0, 0), (0, D), (Wd, D), (Wd, 0)])
        if lv == 0:
            a.shadow(ox + 110, oy + 3, 115, 5, op=.18)
            a.path(P(plane, True), fill=PAL["sand1"])
            a.group(cp=a.clip(P(plane, True)))          # keep road casings inside the map edge
            a.path(P(q(park), True), fill=PAL["sage2"])
            for rd in roads:
                a.path(P(q(rd)), stroke=PAL["rock"], sw=3.6, fill="none", lj="round", so=.5)
                a.path(P(q(rd)), stroke="#fff", sw=2.6, fill="none", lj="round")
            a.path("".join(P(q(b), True) for b in bldgs), fill=PAL["rock1"], stroke=PAL["ink"], sw=.5)
            a.end()
            a.path(P(plane, True), cls="o", sw=.9)
        else:
            a.path(P(plane, True), fill="#ffffff", fo=.6, stroke=PAL["ink"], sw=.5, so=.55)
        if lv == 1:
            for b in bldgs + [park]:
                a.path(P(q(b), True), stroke=PAL["ink2"], sw=.9, fill="none")
                a.dots(q(b), r=1.3, color=PAL["ink"])
            for rd in roads:
                a.path(P(q(rd)), stroke=PAL["ink2"], sw=1.0, fill="none")
                a.dots(q(rd), r=1.3, color=PAL["ink"])
            a.circle(ob(88, 32, 0, o), 2.2, fill="#fff", stroke=PAL["ink"], sw=.9)   # shared node
        if lv == 2:
            for k, (x, y) in enumerate(pois):
                p = ob(x, y, 0, o)
                if k in match:
                    a.ellipse(p, 3.8, 1.5, fill=a.AP, stroke=a.A, sw=.6)
                    a.path(P([p, (p[0], p[1] - 13)]), stroke=a.A, sw=1.0)
                    a.circle((p[0], p[1] - 16.5), 4, fill=a.A, stroke="#fff", sw=.9)
                    a.path(f"M{n(p[0] + 6)} {n(p[1] - 18)}h8M{n(p[0] + 6)} {n(p[1] - 14)}h5", stroke=a.AD, sw=1.1, lc="round")
                else:
                    a.circle(p, 2.4, fill=PAL["slate1"], stroke=PAL["ink"], sw=.5)
    # corner connectors between the layers (the three visible corners)
    for (x, y) in ((0, 0), (Wd, 0), (Wd, D)):
        a.path(P([ob(x, y, 0, (ox, levels[0][1])), ob(x, y, 0, (ox, levels[2][1]))]), cls="h", da="1.6 1.6", so=.6)
    return a.render()


def _city_extract_river(x):
    return 68 + 16 * math.sin(x / 32)


# plan-view buildings (x0, x1, y0, y1, height): clear of the streets, the river and the box edges
_CITY_EXTRACT_BLDS = [
    (60, 80, 30, 44, 8), (92, 114, 30, 42, 6), (44, 52, 30, 40, 5),       # inside the box, south of the river
    (98, 116, 92, 105, 13), (124, 133, 92, 104, 7), (60, 82, 94, 104, 6),  # inside the box, north of the river
    (156, 178, 28, 42, 7), (124, 146, 112, 126, 8), (6, 18, 108, 124, 6),  # outside the box
    (162, 178, 100, 120, 10),
]


def _city_extract_layer(a, o, x0, x1, y0, y1, z, slab, base, under=None):
    """One slab of map: ground, river, a coarse street grid and a few buildings, all cut to
    x0..x1, y0..y1 and sitting at height z. `under` draws on the ground before the buildings."""
    q = lambda x, y: ob(x, y, z, o)
    f, t, r = box_faces(x0, x1, z - slab, z, y1 - y0, ob(0, y0, 0, o))
    if base:
        a.shadow(o[0] + 150, o[1] + 3, 150, 6, op=.18)
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    a.path(P(t, True), fill=PAL["sand1"])
    xs = [x0] + [x for x in range(int(x0) // 6 * 6 + 6, int(x1), 6) if x > x0] + [x1]
    rv = [(x, _city_extract_river(x)) for x in xs]
    a.path(P([q(x, y - 5) for x, y in rv] + [q(x, y + 5) for x, y in rv][::-1], True), fill=PAL["water2"])
    st = ""
    for x in (24, 56, 88, 120, 150, 184):
        if x0 < x < x1:
            st += P([q(x, y0), q(x, y1)])
    for y in (18, 50, 88, 132):
        if y0 < y < y1:
            st += P([q(x0, y), q(x1, y)])
    a.path(st, stroke="#fff", sw=1.8, fill="none", lc="butt")
    if base:
        a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)
    else:
        a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), stroke=a.AD, sw=1.1, fill="none", lj="round")
    if under:
        under()
    bl = [b for b in _CITY_EXTRACT_BLDS if x0 < b[0] and b[1] < x1 and y0 < b[2] and b[3] < y1]
    for (bx0, bx1, by0, by1, h) in sorted(bl, key=lambda b: (-b[2], b[0])):
        box3(a, bx0, bx1, by0, by1, z, z + h, o, PAL["paper2"], "#fff", PAL["rock1"], hair=True, sw=.45)


def city_extract():
    """A box drawn round a city on the OpenStreetMap base, and the extract lifted out of it as a
    block of its own: the streets, river and buildings inside the bounding box, nothing outside."""
    a = Art("cobalt", seed=125)
    o = (10, 170)
    Wd, D, th = 190.0, 140.0, 5.0
    bx0, bx1, by0, by1 = 40.0, 136.0, 24.0, 108.0
    lift = 74.0

    def footprint():
        # the bounding box drawn on the base: the planet file keeps its data, the box only selects
        a.path(P([ob(bx0, by0, 0, o), ob(bx0, by1, 0, o), ob(bx1, by1, 0, o), ob(bx1, by0, 0, o)], True),
               fill=a.AP, fo=.75, stroke=a.A, sw=1.1, da="3.2 2")

    _city_extract_layer(a, o, 0, Wd, 0, D, 0.0, th, True, footprint)
    for (x, y) in ((bx0, by0), (bx0, by1), (bx1, by1), (bx1, by0)):
        p = ob(x, y, 0, o)
        a.path(f"M{n(p[0] - 2.2)} {n(p[1] - 2.2)}h4.4v4.4h-4.4Z", fill="#fff", stroke=a.A, sw=.9)
    # the extract, lifted straight up: the same slab, cut to the box
    for (x, y) in ((bx0, by0), (bx1, by0), (bx1, by1)):
        a.arrow([ob(x, y, 3, o), ob(x, y, lift - th, o)], color=a.A, w=.8, head=3.6, curve=False, da="2 1.6")
    _city_extract_layer(a, o, bx0, bx1, by0, by1, lift, th, False)
    return a.render()


def _country_extract_slab(o, x0, x1, y0, y1, z, th):
    """Front and right faces of a lifted layer (x0..x1, y0..y1 at height z, th thick)."""
    return box_faces(x0, x1, z - th, z, y1 - y0, ob(0, y0, 0, o))


def country_extract():
    """A whole country from OpenStreetMap on its base block, and the thematic layers of its
    shapefile extract lifted above it like a cut-away: land use and rivers on the block itself,
    the road network on the first layer, settlements on the top one, every layer the same
    outline with its features at the same places."""
    a = Art("cobalt", seed=127)
    ring = max((r for iso, r in countries() if iso == "DEU"), key=len)
    lo0 = min(p[0] for p in ring)
    la0 = min(p[1] for p in ring)
    S = 12.5
    kx = math.cos(math.radians(51))
    mx, my = 36.0, 9.0                         # country offset on the slab

    def g(lon, lat):
        return (mx + (lon - lo0) * S * kx, my + (lat - la0) * S)
    pts = [g(lo, la) for lo, la in ring]
    bx0 = min(p[0] for p in pts) - 5
    bx1 = max(p[0] for p in pts) + 5
    by0 = min(p[1] for p in pts) - 4
    by1 = max(p[1] for p in pts) + 4
    Wd, D = 156.0, by1 + 7
    o = (45, 153)
    th = 4.0
    lifts = (0.0, 40.0, 80.0)
    city = {k: g(*v) for k, v in {
        "ber": (13.40, 52.52), "ham": (10.00, 53.55), "muc": (11.58, 48.14), "cgn": (6.96, 50.94),
        "fra": (8.68, 50.11), "str": (9.18, 48.78), "lei": (12.37, 51.34), "han": (9.73, 52.37)}.items()}
    rivers = [[g(*p) for p in r] for r in (
        [(7.60, 47.60), (7.95, 48.6), (8.35, 49.4), (8.10, 50.02), (7.55, 50.40), (6.96, 50.94), (6.75, 51.45), (6.10, 51.85)],
        [(14.25, 50.88), (13.0, 51.35), (11.95, 51.75), (11.62, 52.15), (11.0, 53.0), (10.00, 53.55), (8.90, 53.90)],
        [(8.20, 47.95), (9.6, 48.25), (11.0, 48.75), (12.1, 48.90), (13.45, 48.57)])]
    roads = [("ham", "han"), ("han", "fra"), ("fra", "str"), ("str", "muc"), ("ham", "ber"), ("ber", "lei"),
             ("lei", "muc"), ("cgn", "han"), ("han", "ber"), ("cgn", "fra"), ("lei", "fra")]
    sizes = {"ber": 6.5, "ham": 5.4, "muc": 5.4, "cgn": 4.6, "fra": 4.2, "str": 3.6, "lei": 3.6, "han": 3.6}
    rnd = a.rng
    q = lambda x, y, z: ob(x, y, z, o)
    sea = [(Wd, D)] + [(x, D - 26 - 5 * math.sin(x / 22) - (10 if 70 < x < 140 else 0)) for x in range(int(Wd), -1, -10)] + [(0, D)]
    foot = [(bx0, by0), (bx0, by1), (bx1, by1), (bx1, by0)]

    def surface(z, x0, x1, y0, y1, land):
        """A map surface at height z: neighbouring land, sea, and the country on top."""
        top = [q(x0, y0, z), q(x0, y1, z), q(x1, y1, z), q(x1, y0, z)]
        a.path(P(top, True), fill=PAL["sand1"])
        a.group(cp=a.clip(P(top, True)))
        a.path(smooth([q(x, y, z) for x, y in sea], True), fill=PAL["water2"])
        a.end()
        outline = [q(x, y, z) for x, y in pts]
        a.path(P(outline, True), fill=land)
        return outline
    # the base block
    a.shadow(o[0] + 135, o[1] + 3, 115, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, 6, PAL["sand1"])
    outline0 = surface(0, 0, Wd, 0, D, PAL["sage1"])
    a.group(cp=a.clip(P(outline0, True)))
    for cx, cy, rx, ry, c in ((.30, .25, 15, 10, "forest"), (.78, .14, 13, 9, "forest"), (.55, .55, 12, 8, "moss"),
                              (.22, .72, 11, 7, "moss"), (.80, .82, 13, 7, "sand2")):
        cx, cy = lerp(bx0, bx1, cx), lerp(by0, by1, cy)
        a.path(smooth([q(cx + rx * math.cos(u) * (1 + .15 * math.sin(3 * u)), cy + ry * math.sin(u), 0)
                       for u in [2 * math.pi * i / 8 for i in range(8)]], True), fill=PAL[c], fo=.7)
    a.end()
    for rv in rivers:
        a.path(smooth([q(x, y, 0) for x, y in rv]), stroke=PAL["water4"], sw=1.2, fill="none", lc="round")
    a.path(P(outline0, True), stroke=PAL["ink2"], sw=.6, fill="none", lj="round")
    # the extract's footprint left on the base, with its corner handles
    a.path(P([q(x, y, 0) for x, y in foot], True), fill=a.AP, fo=.25, stroke=a.A, sw=.9, da="3 1.8")
    for (x, y) in foot:
        p = q(x, y, 0)
        a.path(f"M{n(p[0] - 2)} {n(p[1] - 2)}h4v4h-4Z", fill="#fff", stroke=a.A, sw=.8)
    _outline(a, f, t, r)
    # the lifted layers: thin blocks, each carrying the same country outline
    for k in (1, 2):
        z0, z = lifts[k - 1], lifts[k]
        for (x, y) in ((bx0, by0), (bx1, by0), (bx1, by1)):
            a.arrow([q(x, y, z0 + 2), q(x, y, z - th)], color=a.A, w=.7, head=3.4, curve=False, da="1.8 1.4")
        ff, tt, rr = _country_extract_slab(o, bx0, bx1, by0, by1, z, th)
        a.path(P(ff, True), fill=PAL["soil"])
        a.path(P(rr, True), fill=PAL["umber"])
        a.group(cp=a.clip(P(tt, True)))
        outline = surface(z, bx0, bx1, by0, by1, mix(PAL["sage1"], "#fff", .45))
        a.end()
        if k == 1:
            a.path("".join(P([q(*city[u], z), q(*city[v], z)]) for u, v in roads), stroke=a.A, sw=1.1, fill="none", lc="round")
            for c in city.values():
                a.circle(q(*c, z), 1.4, fill=a.AD)
        else:
            d = ""
            for key, (cx, cy) in city.items():
                rad = sizes[key]
                for _ in range(int(rad * 1.3)):
                    ang, dist = rnd.uniform(0, 2 * math.pi), rad * math.sqrt(rnd.random())
                    x, y = cx + dist * math.cos(ang), cy + dist * math.sin(ang) * 1.4
                    w, dd = rnd.uniform(2.2, 3.0), rnd.uniform(2.6, 3.8)
                    d += P([q(x, y, z), q(x + w, y, z), q(x + w, y + dd, z), q(x, y + dd, z)], True)
            a.path(d, fill=PAL["slate2"], stroke=PAL["ink"], sw=.25, so=.5)
        a.path(P(outline, True), stroke=PAL["ink2"], sw=.55, fill="none", lj="round")
        a.path(P(ff, True) + P(tt, True) + P([rr[1], rr[2], rr[3]]), stroke=a.AD, sw=1.0, fill="none", lj="round")
    return a.render()


def _pip(x, y, ring):
    ins = False
    for (x1, y1), (x2, y2) in zip(ring, ring[1:] + ring[:1]):
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1 + 1e-12) + x1:
            ins = not ins
    return ins


def _hawker_plate(a, x0, z0, x1, z1, y0, y1, o, th, top, end, cut):
    """A roof plate of thickness th whose underside runs straight from (x0, z0) to (x1, z1),
    extruded in depth from y0 (the cut face, nearest the viewer) to y1."""
    a.path(P([ob(x0, y0, z0 + th, o), ob(x1, y0, z1 + th, o), ob(x1, y1, z1 + th, o), ob(x0, y1, z0 + th, o)], True),
           fill=top, stroke=PAL["ink"], sw=.5, lj="round")
    xe, ze = (x1, z1) if x1 > x0 else (x0, z0)      # the end that faces +x is visible
    a.path(P([ob(xe, y0, ze, o), ob(xe, y1, ze, o), ob(xe, y1, ze + th, o), ob(xe, y0, ze + th, o)], True),
           fill=end, stroke=PAL["ink"], sw=.5, lj="round")
    a.path(P([ob(x0, y0, z0, o), ob(x1, y0, z1, o), ob(x1, y0, z1 + th, o), ob(x0, y0, z0 + th, o)], True),
           fill=cut, stroke=PAL["ink"], sw=1.1, lj="round")


def hawker():
    """A Singapore hawker centre: an open-sided pavilion with a row of cooking stalls and shared
    tables under one pitched roof with a raised vent along its ridge, seen with its near end cut
    as a section. Cooler air comes in at the open sides; air warmed by the stoves rises and leaves
    through the vent (the stack effect)."""
    a = Art("verm", seed=129)
    gy = 150
    D = 70.0
    o = (16, gy)
    W = 248.0
    # ground slab
    f, t, r = box_faces(0, W, -6, 0, D, o)
    a.shadow(170, gy + 8, 150, 6, op=.2)
    a.path(P(t, True), fill="#e7e1d4")
    a.path(P(f, True), fill=PAL["rock1"])
    a.path(P(r, True), fill=PAL["rock"])
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)
    # roof geometry (cross-section in x-z, ridge running in depth)
    X0, X1 = 30.0, 218.0
    xm = (X0 + X1) / 2
    th = 3.0
    el, er = X0 - 14, X1 + 14          # eaves
    zr = 44.0                          # underside height at the eaves
    g = 11.0                           # half-width of the open ridge
    s = .3                             # roof pitch
    zi = zr + (xm - g - el) * s        # underside at the ridge opening
    vo = 20.0                          # vent cap half-width (overhangs the opening)
    zv = zi + th + 9                   # vent cap underside at its eaves
    zp = zv + 6                        # vent cap underside at its ridge

    def under(x):                      # underside of the roof above x
        return zr + ((x - el) if x < xm else (er - x)) * s
    cy0, cy1 = -4.0, D + 4.0
    cols = (X0, X0 + 62, X1 - 62, X1)

    def column(x, y, near):
        p, q = ob(x, y, 0, o), ob(x, y, under(x) + 1, o)
        if near:
            a.path(P([p, q]), stroke=PAL["ink"], sw=3.6)
            a.path(P([p, q]), stroke=PAL["rock1"], sw=2.4)
        else:
            a.path(P([p, q]), stroke=PAL["rock"], sw=2.6)
        a.ellipse(p, 2.6, 1, fill=PAL["ink"], fo=.25)
    for x in cols:
        column(x, D - 6, False)
    # one row of stalls: a rear wall and counters with a wok each
    wall_y = D - 12
    box3(a, X0 + 8, X1 - 8, wall_y, wall_y + 3, 0, 26, o, PAL["sand2"], "#f3eee4", PAL["soil"], sw=.4)
    for k in range(3):
        x = X0 + 14 + k * 56
        box3(a, x, x + 44, wall_y - 14, wall_y, 0, 12, o, "#f6f2ea", "#fdfbf7", PAL["sand2"], sw=.45)
        a.ellipse(ob(x + 22, wall_y - 7, 12, o), 6, 1.9, fill=PAL["slated"])
    # a few shared tables, each with two stools
    for i in range(3):
        cx, cy = X0 + 38 + i * 56, 24
        for dx in (-13, 13):
            p = ob(cx + dx, cy, 0, o)
            a.path(f"M{n(p[0])} {n(p[1])}v-6", stroke=PAL["slate2"], sw=1.8)
            a.ellipse((p[0], p[1] - 6), 3.8, 1.5, fill=PAL["sand2"], stroke=PAL["ink"], sw=.4)
        p = ob(cx, cy, 0, o)
        a.path(f"M{n(p[0])} {n(p[1])}v-9", stroke=PAL["slate2"], sw=2.2)
        a.ellipse((p[0], p[1] - 9), 8.5, 3.2, fill="#f8f6f0", stroke=PAL["ink"], sw=.5)
    for x in cols:
        column(x, 6, True)
    # roof: one muted terracotta gable, open along the ridge under a raised vent cap
    terra = mix(a.A, PAL["clay"], .62)
    light, darkf = mix(terra, "#fff", .32), mix(terra, "#fff", .12)
    cut = mix(a.AD, PAL["umber"], .45)
    _hawker_plate(a, el, zr, xm - g, zi, cy0, cy1, o, th, light, darkf, cut)
    _hawker_plate(a, xm + g, zi, er, zr, cy0, cy1, o, th, darkf, darkf, cut)
    zu = lambda x: zv + (vo - abs(x - xm)) / vo * (zp - zv)
    for sx in (-1, 1):
        x = xm + sx * (g - 1.5)
        for y in (cy0 + 2, D / 2, cy1 - 2):
            a.path(P([ob(x, y, zi + th, o), ob(x, y, zu(x), o)]), stroke=PAL["slate2"], sw=1.3)
    _hawker_plate(a, xm - vo, zv, xm, zp, cy0, cy1, o, th, light, darkf, cut)
    _hawker_plate(a, xm, zp, xm + vo, zv, cy0, cy1, o, th, darkf, darkf, cut)
    # the stack effect: air drawn in low at the open side, warmed, rising out through the vent
    a.arrow([ob(-8, cy0, 7, o), ob(30, cy0, 8, o), ob(xm - 24, cy0, 14, o), ob(xm - 4, cy0, zi - 12, o),
             ob(xm + 1, cy0, zi + th + 2, o), ob(xm + 12, cy0, zi + th + 5, o), ob(xm + 38, cy0, zi + th + 10, o)],
            color=a.A, w=1.7, head=5.6)
    return a.render()
