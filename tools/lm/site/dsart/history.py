"""History and culture: heritage sites, ancient places, Roman roads, county seats, temples, castles."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, box_faces, terrain, box3, hiproof, gable,
                  cylinder, merlons, conifer, broadleaf, scallop)


def slab(a: Art, o, Wd, D, th, top, front=None, right=None):
    f, t, r = box_faces(0, Wd, -th, 0, D, o)
    a.path(P(t, True), fill=top)
    a.path(P(f, True), fill=front or PAL["soil"])
    a.path(P(r, True), fill=right or PAL["umber"])
    return f, t, r


def slab_outline(a: Art, f, t, r):
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)


def _ellipse_on(surf, cx, cy, rx, ry, dz=.4, k=48):
    return [surf(cx + rx * math.cos(t), cy + ry * math.sin(t), dz) for t in [2 * math.pi * i / k for i in range(k)]]


def _heritage_visible_runs(h, pts3, o, step=1.5, smax=160.0):
    """Split a closed loop of surface points (x, y, z) into runs that the terrain h does not hide.
    In the oblique view a sight line towards the viewer moves -y, +DX*s in x, and rises -DY per unit."""
    from .kit import DX, DY
    def seen(x, y, z):
        s = step
        while s < min(smax, y + 1e-6):
            if h(x + DX * s, y - s) > z - DY * s + .3:
                return False
            s += step
        return True
    vis = [seen(*q) for q in pts3]
    k = len(pts3)
    if all(vis):
        return [[ob(*q, o) for q in pts3] + [ob(*pts3[0], o)]]
    start = next(i for i in range(k) if not vis[i])
    runs, cur = [], []
    for m in range(1, k + 1):
        i = (start + m) % k
        if vis[i]:
            cur.append(ob(*pts3[i], o))
        elif cur:
            runs.append(cur)
            cur = []
    if cur:
        runs.append(cur)
    return [r for r in runs if len(r) > 1]


def _heritage_mesa(a: Art, o, cx, cy, rx, ry, prof, land, cliff, k=40):
    """An elliptical flat-topped hill: bands between the rings of prof (radius factor, height),
    each facet shaded by its slope against the kit's light, drawn back to front, then the summit."""
    from .kit import LIGHT
    Ln = math.sqrt(sum(c * c for c in LIGHT))
    ts = [2 * math.pi * i / k for i in range(k)]
    quads = []
    for b, ((r0, h0), (r1, h1)) in enumerate(zip(prof, prof[1:])):
        col = cliff if b == 0 else land
        for i in range(k):
            t0, t1 = ts[i], ts[(i + 1) % k]
            tm = (t0 + t1) / 2 + (math.pi if i == k - 1 else 0)
            # outward slope normal of the band at mid-angle
            gx, gy = math.cos(tm) / rx, math.sin(tm) / ry
            s = (h0 - h1) / (r1 - r0)                      # height drop per unit radius factor
            nrm = (s * gx, s * gy, 1.0)
            nl = math.sqrt(sum(c * c for c in nrm))
            lam = sum(p * q for p, q in zip(nrm, LIGHT)) / (nl * Ln)
            lvl = round(max(0.0, min(1.0, (lam - .2) / .7)) * 6) / 6
            shade = mix(mix(col, PAL["ink"], .22), mix(col, "#ffffff", .35), lvl)
            q = [ob(cx + r0 * rx * math.cos(t), cy + r0 * ry * math.sin(t), h0, o) for t in (t0, t1)] + \
                [ob(cx + r1 * rx * math.cos(t), cy + r1 * ry * math.sin(t), h1, o) for t in (t1, t0)]
            quads.append((math.sin(tm), b, q, shade))
    runs = []                                          # merge consecutive facets of one shade, keeping the order
    for _, _, q, shade in sorted(quads, key=lambda e: (-e[0], -e[1])):
        if runs and runs[-1][0] == shade:
            runs[-1][1].append(P(q, True))
        else:
            runs.append((shade, [P(q, True)]))
    for shade, ds in runs:
        a.path("".join(ds), fill=shade, stroke=shade, sw=.35, lj="round")
    r0, h0 = prof[0]
    top = [ob(cx + r0 * rx * math.cos(t), cy + r0 * ry * math.sin(t), h0, o) for t in ts]
    a.path(P(top, True), fill=mix(land, "#ffffff", .2))


def heritage():
    """A Doric temple on the levelled top of its acropolis, in oblique: stepped base, columns,
    entablature and pedimented roof. The flat summit is the property's core zone (filled); the
    wider buffer zone that World Heritage nominations map rings it on the plain below."""
    a = Art("verm", seed=81)
    Wd, D = 180.0, 156.0
    o = (20, 160)
    cx, cy = 90.0, 78.0              # centre of the acropolis plateau and of the temple
    zp, z0 = 22.0, 3.0               # summit and plain heights
    prx, pry = 36.0, 50.0            # radii of the levelled summit
    prof = [(1.0, zp), (1.07, z0 + .45 * (zp - z0)), (1.25, z0)]   # cliff, then talus (radius factor, height)

    def h(x, y):
        r = math.hypot((x - cx) / prx, (y - cy) / pry)
        if r >= prof[-1][0]:
            return z0
        for (r0, h0), (r1, h1) in zip(prof, prof[1:]):
            if r <= r1:
                return h0 if r <= r0 else h0 + (h1 - h0) * (r - r0) / (r1 - r0)
        return zp
    land = mix(PAL["sage2"], PAL["sand2"], .35)
    a.shadow(o[0] + Wd * .85, o[1] + 4, 140, 7, op=.2)
    terrain(a, o, Wd, D, lambda x, y: z0, nx=2, ny=2, colours={"land": land}, base=12,
            strata=[(3, PAL["umber"]), (6, PAL["sand2"]), (99, PAL["rock1"])])
    # buffer zone on the plain: dashed, with its far side hidden behind the hill
    k = 72
    ring = [(cx + 78 * math.cos(t), cy + 73 * math.sin(t)) for t in [2 * math.pi * i / k for i in range(k)]]
    for run in _heritage_visible_runs(h, [(x, y, z0 + .4) for x, y in ring], o):
        a.path(P(run), fill="none", stroke=a.AL, sw=1.6, da="5 3")
    # the acropolis: a levelled summit ringed by a cliff and a talus slope
    _heritage_mesa(a, o, cx, cy, prx, pry, prof, land, mix(land, PAL["rock1"], .55))
    surf = lambda x, y, dz=0.0: ob(x, y, h(x, y) + dz, o)
    # core zone: the levelled summit itself
    a.path(P(_ellipse_on(surf, cx, cy, prx - 2, pry - 2, k=56), True), fill=a.AP, fo=.5, stroke=a.A, sw=1.4, da="4 2.4")
    # ---- the temple (hexastyle peripteral, plan about 1:2), gable end towards the viewer
    tx0, tx1, ty0, ty1 = cx - 15, cx + 15, cy - 30, cy + 30
    stone, stone_l, stone_d = "#f4efe4", "#faf7f0", PAL["rock1"]
    zb, st = zp, 3.0
    for m in range(2):               # stepped base (crepidoma)
        box3(a, tx0 - 4 + 2 * m, tx1 + 4 - 2 * m, ty0 - 4 + 2 * m, ty1 + 4 - 2 * m, zb + st * m, zb + st * (m + 1), o,
             stone, stone_l, stone_d, sw=.4)
    zs, colh, w = zb + 2 * st, 22.0, 3.6
    box3(a, tx0 + 7, tx1 - 7, ty0 + 8, ty1 - 6, zs, zs + colh, o, PAL["sand1"], None, mix(PAL["sand1"], PAL["ink"], .15), sw=.35)

    g = a.lin([(0, "#fffdf8"), (.5, stone), (1, stone_d)], 0, 0, 1, 0)     # one shared left-lit shaft gradient

    def column(x, y):
        b, t_ = ob(x, y, zs, o), ob(x, y, zs + colh - 1.6, o)
        a.path(f"M{n(b[0] - w / 2)} {n(b[1])}L{n(t_[0] - w * .4)} {n(t_[1])}H{n(t_[0] + w * .4)}L{n(b[0] + w / 2)} {n(b[1])}Z", fill=g,
               stroke=PAL["ink"], sw=.4)
        a.path(f"M{n(t_[0] - w * .65)} {n(t_[1])}h{n(w * 1.3)}v-1.6h{n(-w * 1.3)}Z", fill=stone, stroke=PAL["ink"], sw=.35)  # capital
    xl, xr = tx0 + w / 2 + .4, tx1 - w / 2 - .4
    yf, yb, nf = ty0 + 2.4, ty1 - 2.4, 10
    for m in range(nf, 0, -1):       # right flank (11 with the corner), back to front
        column(xr, yf + m * (yb - yf) / nf)
    for m in range(6):               # the six columns of the front
        column(xl + m * (xr - xl) / 5, yf)
    ze = zs + colh
    box3(a, tx0, tx1, ty0, ty1, ze, ze + 5, o, stone, None, stone_d, sw=.45)          # entablature
    fl, fr_, rf = gable(a, tx0, tx1, ty0, ty1, ze + 5, 10, o, PAL["clay"])
    a.path(P([ob(tx0 - 1.2, ty1, ze + 5, o), ob((tx0 + tx1) / 2, ty1, ze + 15, o)]), stroke=PAL["ink"], sw=.45, so=.85)  # back verge
    a.path(P([fl, rf, fr_], True), fill="#f6f1e6", stroke=PAL["ink"], sw=.5)          # pediment
    return a.render()


def _ancient_places_lens(x, cx, hk, sk):
    """Top of one occupation deposit along the section: a flat-topped lens, thickest under the
    summit and pinching out towards the edges of the mound."""
    return hk * math.exp(-(((x - cx) ** 2) / sk) ** 2)


def _ancient_places_mound(a: Art, o, Wd, D, h, land, eps=.4):
    """The plain and the mound's surface: terrain shading softened by a slight blur and clipped to
    the block's top (plain plus the mound's silhouette), so the faceted quads read as calm,
    continuous light and shade with crisp outer edges. Returns the skyline's screen points."""
    from .kit import DX
    top, bot = [], []
    sx0, sx1 = int(o[0]), int(o[0] + Wd + D * DX) + 1
    for sxi in range(sx0, sx1 + 1):
        lo, hi, zt = 1e9, -1e9, 0.0
        for k in range(int(D * 2) + 1):
            y = k / 2
            x = sxi - o[0] - y * DX
            if not 0 <= x <= Wd:
                continue
            z = h(x, y)
            if z < eps:
                continue
            p = ob(x, y, z, o)
            if p[1] < lo:
                lo, zt = p[1], z
            hi = max(hi, ob(x, y, 0, o)[1])
        if hi > -1e9:
            top.append((sxi, lo, zt))
            bot.append((sxi, hi))
    region = [(x, y) for x, y, _ in top[::2] + top[-1:]] + (bot[::2] + bot[-1:])[::-1]
    _, t, _ = box_faces(0, Wd, 0, 0, D, o)
    area = lambda q: sum(q[i][0] * q[i - 1][1] - q[i - 1][0] * q[i][1] for i in range(len(q)))
    if (area(region) > 0) != (area(t) > 0):
        region = region[::-1]
    a.group(cp=a.clip(P(t, True) + P(region, True)))
    a.group(filter=a.blur(1.8))
    terrain(a, o, Wd, D, h, nx=24, ny=10, levels=5, colours={"land": land}, base=1, strata=[(99, land)], outline=False)
    a.end()
    a.end()
    back = ob(0, D, 0, o)[1]                 # the skyline stops where it meets the plain's back edge
    cover = {x: (y, b) for (x, y, _), (_, b) in zip(top, bot)}
    return [(x, y) for x, y, z in top if z > 4 and y < back - .5], cover


def ancient_places():
    """A tell cut in half: centuries of settlement layers piled into a mound on a flat plain,
    each deposit a lens thickest at the centre, buried mudbrick walls standing on old floors,
    and the single representative point a gazetteer gives the place."""
    a = Art("verm", seed=83)
    o = (30, 150)
    Wd, D, base = 196.0, 84.0, 15.0
    cx, cy, Hm = 96.0, 18.0, 52.0            # mound centre (the section cuts just in front of it)
    sx, sy = 2300.0, 1900.0

    def h(x, y):    # a tell: flat top, steep flanks, on a flat plain
        return Hm * math.exp(-((x - cx) ** 2 / sx + (y - cy) ** 2 / sy) ** 2)
    fy = h(cx, 0) / Hm                       # share of the mound's height left at the cut face
    a.shadow(o[0] + Wd / 2 + 34, o[1] + base + 1, Wd / 2 + 22, 5, op=.16)
    land = mix(PAL["sand1"], PAL["sage2"], .55)
    # the flat plain (top face) and the right cut face, which shows only natural ground
    f, t, r = box_faces(0, Wd, -base, 0, D, o)
    a.path(P(t, True), fill=land)
    skyline, cover = _ancient_places_mound(a, o, Wd, D, h, land)
    a.path(P(r, True), fill=mix(PAL["rock1"], PAL["ink"], .16))
    a.path(P([ob(Wd, 0, -4, o), ob(Wd, D, -4, o)]), stroke=PAL["rockd"], sw=.55, so=.6)
    a.path(smooth(skyline[::3] + [skyline[-1]]), cls="h")      # the mound's skyline

    def surf(x, y, dz=0.0):
        return ob(x, y, h(x, y) + dz, o)
    # ---- the front cut face: natural ground below z = 0, occupation lenses above it
    xs = [Wd * i / 56 for i in range(57)]
    face = lambda zs: [ob(x, 0, z, o) for x, z in zip(xs, zs)]
    a.path(P([ob(0, 0, 0, o), ob(Wd, 0, 0, o), ob(Wd, 0, -base, o), ob(0, 0, -base, o)], True), fill=PAL["rock1"])
    a.path(P([ob(0, 0, -4, o), ob(Wd, 0, -4, o)]), stroke=PAL["rock"], sw=.55, fill="none")
    # (height share of the full mound, spread, colour) from the oldest deposit up
    lenses = [(.24, 900, PAL["umber"]), (.46, 1250, PAL["clay"]), (.68, 1600, PAL["soil"]), (.86, 1950, PAL["sand2"]),
              (1.0, 0, mix(PAL["sand1"], PAL["soil"], .35))]
    surface = [h(x, 0) for x in xs]
    tops = [[min(_ancient_places_lens(x, cx, k * Hm * fy, s), zt) if s else zt for x, zt in zip(xs, surface)] for (k, s, _) in lenses]
    prev = [0.0] * len(xs)
    for (k, s, col), top in zip(lenses, tops):
        top = [max(t, p) for t, p in zip(top, prev)]
        a.path(P(face(top) + face(prev)[::-1], True), fill=col)
        prev = top
    for top in tops[:-1]:                    # old floors: where each deposit ended
        a.path(P(face(top)), stroke=PAL["ink"], sw=.55, so=.45, fill="none")
    # buried mudbrick walls, each standing on an old floor and covered by the next deposit
    for (x, layer, w) in ((74, 0, 11), (104, 2, 11)):
        lo = [_ancient_places_lens(x + d, cx, lenses[layer][0] * Hm * fy, lenses[layer][1]) for d in (0, w)]
        hi = [_ancient_places_lens(x + d, cx, lenses[layer + 1][0] * Hm * fy, lenses[layer + 1][1]) for d in (0, w)]
        z0 = min(lo)                         # footing on the floor, sunk in at its higher end
        hh = .82 * (min(hi) - z0)            # stays below the next floor
        b = ob(x, 0, z0, o)
        a.path(f"M{n(b[0])} {n(b[1])}h{n(w)}v{n(-hh)}h{n(-w)}Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.5)
        a.path(f"M{n(b[0])} {n(b[1] - hh / 2)}h{n(w)}M{n(b[0] + w / 2)} {n(b[1])}v{n(-hh / 2)}"
               f"M{n(b[0] + w / 4)} {n(b[1] - hh / 2)}v{n(-hh / 2)}M{n(b[0] + w * .75)} {n(b[1] - hh / 2)}v{n(-hh / 2)}",
               stroke=PAL["ink"], sw=.4, so=.6)
    # ---- outline of the block (main line weight)
    front_top = face(surface)
    a.path(P(front_top) + P([ob(Wd, 0, 0, o), ob(Wd, D, 0, o)]), cls="o")
    a.path(P([ob(0, 0, h(0, 0), o), ob(0, 0, -base, o), ob(Wd, 0, -base, o), ob(Wd, D, -base, o), ob(Wd, D, h(Wd, D), o)])
           + P([ob(Wd, 0, -base, o), ob(Wd, 0, h(Wd, 0), o)]), cls="o")
    # back and left edges of the plain, hidden where the mound stands in front of them
    edge, run = [], []
    for k in range(int(Wd) + 1):
        p = ob(k, D, 0, o)
        lo, hi = cover.get(round(p[0]), (1e9, -1e9))
        if lo < p[1] - .3 < hi:
            if len(run) > 1:
                edge.append(run)
            run = []
        else:
            run.append(p)
    edge.append(run)
    a.path("".join(P([q[0], q[-1]]) for q in edge if len(q) > 1) + P([ob(0, 0, 0, o), ob(0, D, 0, o)]), cls="h", so=.6)
    # ---- the gazetteer's representative point: one pin on the summit, with its ground ring
    a.path(P(_ellipse_on(surf, cx, cy, 9, 9, dz=.3), True), fill=a.AP, fo=.5, stroke=a.AD, sw=.7)
    c = surf(cx, cy, 0)
    a.path(f"M{n(c[0])} {n(c[1])}V{n(c[1] - 20)}", stroke=PAL["ink"], sw=.9, lc="round")
    a.path(f"M{n(c[0])} {n(c[1] - 16)}c-3.6-4.6-6-7-6-10a6 6 0 0 1 12 0c0 3-2.4 5.4-6 10Z", fill=a.A, stroke=PAL["ink"], sw=.6)
    a.circle((c[0], c[1] - 26.2), 2.2, fill="#fff")
    return a.render()


def _roman_roads_stone(x0, x1, z0, z1, rnd, o):
    """A rough, rounded block in the front section face: a jittered octagon with soft corners."""
    c = min(x1 - x0, z1 - z0) * .32
    pts = [(x0 + c, z0), (x1 - c, z0), (x1, z0 + c), (x1, z1 - c), (x1 - c, z1), (x0 + c, z1), (x0, z1 - c), (x0, z0 + c)]
    pts = [ob(x + rnd.uniform(-.8, .8), 0, z + rnd.uniform(-.7, .7), o) for (x, z) in pts]
    m = [((pts[i][0] + pts[(i + 1) % 8][0]) / 2, (pts[i][1] + pts[(i + 1) % 8][1]) / 2) for i in range(8)]
    d = f"M{n(m[-1][0])} {n(m[-1][1])}"
    for i in range(8):
        d += f"Q{n(pts[i][0])} {n(pts[i][1])} {n(m[i][0])} {n(m[i][1])}"
    return d + "Z"


def roman_roads():
    """A Roman road in section and in its landscape: the large-stone statumen, the rudus of
    rubble and lime, the fine nucleus and the cambered summa crusta of paving stones between
    kerbs, with side ditches, a milestone and the road's mapped centre line."""
    a = Art("verm", seed=85)
    o = (10, 122)
    Wd, D, th = 184.0, 168.0, 48.0
    rx0, rx1 = 50.0, 130.0                 # road corridor (paving between the kerbs)
    xm, kw = (rx0 + rx1) / 2, 3.0          # crown line, kerb width
    a.shadow(o[0] + 160, o[1] + th - 6, 150, 7, op=.2)
    f, t, r = box_faces(0, Wd, -th, 0, D, o)
    a.path(P(t, True), fill=mix(PAL["sage1"], PAL["sand1"], .3))
    a.path(P(f, True), fill=PAL["clay"])
    a.path(P([ob(0, 0, 0, o), ob(Wd, 0, 0, o), ob(Wd, 0, -7, o), ob(0, 0, -7, o)], True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    slab_outline(a, f, t, r)               # drawn first: the section below covers the cut edge

    def camber(ze, zm, x0, x1, y=0.0, k=12):
        """A cambered course top, z = ze at the edges rising to zm at the crown."""
        c = (x0 + x1) / 2
        return [ob(x, y, lerp(ze, zm, 1 - ((x - c) / ((x1 - x0) / 2)) ** 2), o) for x in [x0 + (x1 - x0) * i / k for i in range(k + 1)]]

    # side ditches along both verges, cut into the front face
    ditches = ((rx0 - 19, rx0 - 9), (rx1 + 5, rx1 + 15))
    for (d0, d1) in ditches:
        a.path(P([ob(d0, 0, 0, o), ob(d0, D, 0, o), ob(d1, D, 0, o), ob(d1, 0, 0, o)], True), fill=mix(PAL["water2"], PAL["water3"], .6))
        a.path(P([ob(d0, 0, 0, o), ob(d0, D, 0, o)]) + P([ob(d1, 0, 0, o), ob(d1, D, 0, o)]), cls="h")
    # ---- the layered section in its trench (bottom up): three clear courses
    b0, b1, zb = rx0 - 3, rx1 + 3, -30.0
    courses = [(-18, -17, PAL["rockd"]), (2, 4.5, PAL["sand2"])]   # statumen bed, rudus fill
    prev = [ob(b0, 0, zb, o), ob(b1, 0, zb, o)]
    tops = []
    for (ze, zm, col) in courses:
        top = camber(ze, zm, b0, b1)
        a.path(P(top + prev[::-1], True), fill=col)
        tops.append(top)
        prev = top
    # statumen: a bed of large, rough-rounded stones
    xs = [b0 + 1.5, b0 + 18, b0 + 35.5, b0 + 51, b0 + 69, b1 - 1.5]
    for i in range(5):
        x0, x1 = xs[i] + .8, xs[i + 1] - .8
        z0, z1 = zb + .8 + a.rng.uniform(0, 1.2), -19.5 - a.rng.uniform(0, 2.5)
        a.path(_roman_roads_stone(x0, x1, z0, z1, a.rng, o), fill=PAL["rock1"], stroke=PAL["ink"], sw=.4)
    # rudus: a few coarse lumps of rubble in lime
    for i in range(6):
        x = b0 + 8 + (b1 - b0 - 16) * i / 5
        p = ob(x, 0, -8 + (3 if i % 2 else 0), o)
        a.circle(p, 1.9, fill=PAL["rockd"])
    # summa crusta: a course of thick paving blocks between the kerbs
    pz = (5.0, 9.0)
    pave = camber(pz[0], pz[1], rx0, rx1)
    base = camber(2, 4.5, rx0, rx1)
    a.path(P(pave + base[::-1], True), fill=PAL["rock1"])
    a.path("".join(P([pave[i], base[i]]) for i in (3, 6, 9)), stroke=PAL["ink"], sw=.45, so=.7)
    a.path("".join(P(top) for top in tops[:1] + [base, pave]), stroke=PAL["ink"], sw=.45, so=.7, fill="none")
    a.path(P([ob(b0, 0, 2, o), ob(b0, 0, zb, o), ob(b1, 0, zb, o), ob(b1, 0, 2, o)]), cls="h")
    # left kerb (umbo) first: the paving hides the lower part of its inner face
    kc, kt, kd = "#ece6d8", "#f7f3ea", PAL["rock1"]
    box3(a, rx0 - kw, rx0, 0, D, 0, pz[0] + 2.5, o, kc, kt, kd, sw=.35)
    # ---- the paved top, running back across the slab: a plain cambered band,
    # its sunlit left half paler than the right, with a few broad, staggered joints
    zc = lambda x: lerp(pz[0], pz[1], 1 - ((x - xm) / ((rx1 - rx0) / 2)) ** 2)
    S = lambda x, y: ob(x, y, zc(x), o)
    left = [S(rx0 + (xm - rx0) * i / 6, 0) for i in range(7)]
    leftb = [S(rx0 + (xm - rx0) * i / 6, D) for i in range(7)]
    right = [S(xm + (rx1 - xm) * i / 6, 0) for i in range(7)]
    rightb = [S(xm + (rx1 - xm) * i / 6, D) for i in range(7)]
    a.path(P(left + leftb[::-1], True), fill=mix(PAL["rock1"], "#ffffff", .35))
    a.path(P(right + rightb[::-1], True), fill=mix(PAL["rock1"], "#ffffff", .12))
    a.path(P([S(rx0 + (rx1 - rx0) * i / 12, D) for i in range(13)]), cls="h")
    joints = ""
    for j in range(1, 5):
        y = j * D / 5
        joints += P([S(rx0 + (rx1 - rx0) * i / 12, y) for i in range(13)])
    for j in range(5):       # one staggered side joint per course: a few large stones
        xj = rx0 + (rx1 - rx0) * (.28 if j % 2 else .72)
        joints += P([S(xj, j * D / 5), S(xj, (j + 1) * D / 5)])
    a.path(joints, stroke=PAL["rockd"], sw=.55, so=.6, fill="none", lj="round")
    # the road's mapped alignment (the dataset): a polyline with vertices, along the crown
    cl = [ob(xm, y, pz[1] + .3, o) for y in (0, D / 2, D)]
    a.path(P(cl), stroke=a.A, sw=1.3)
    for p in cl:
        a.circle(p, 1.7, fill=a.A, stroke="#ffffff", sw=.5)
    # right kerb (umbo); it stands in front of the paving's right edge
    box3(a, rx1, rx1 + kw, 0, D, 0, pz[0] + 2.5, o, kc, kt, kd, sw=.35)
    # ditch profiles in the front face
    for (d0, d1) in ditches:
        p0, p1 = ob(d0, 0, 0, o), ob(d1, 0, 0, o)
        w = p1[0] - p0[0]
        a.path(f"M{n(p0[0])} {n(p0[1] - .6)}h{n(w)}v.6c-1.5 7-{n(w - 3)} 7-{n(w)} 0Z", fill=PAL["water3"])
        a.path(f"M{n(p0[0])} {n(p0[1])}c1.5 7 {n(w - 1.5)} 7 {n(w)} 0", stroke=PAL["ink"], sw=.45, fill="none")
    # a milestone on the right verge and two cypresses (back to front)
    items = [(rx1 + 26, 66, "m"), (rx0 - 32, 96, "t"), (rx1 + 32, 146, "t")]
    for (x, y, kind) in sorted(items, key=lambda q: -q[1]):
        if kind == "m":
            cylinder(a, x, y, 4.5, 0, 19, o, "#efe9dc")
        else:
            b = ob(x, y, 0, o)
            a.path(f"M{n(b[0])} {n(b[1] - 26)}c4.5 7 5 17 2.4 25.5h-4.8c-2.6-8.5-2.1-18.5 2.4-25.5Z", fill=PAL["forest"], stroke=PAL["forestd"], sw=.45)
    return a.render()


def _county_seats_roof(a: Art, x0, x1, y0, y1, z, rh, o, colour, along_x=True, overhang=1.5):
    """A plain hipped roof (no tile hairlines, which vanish at card size); the ridge runs along x
    (front wall towers, halls) or along the depth axis y (side wall towers)."""
    x0, x1, y0, y1 = x0 - overhang, x1 + overhang, y0 - overhang, y1 + overhang
    fl, fr, bl, br = ob(x0, y0, z, o), ob(x1, y0, z, o), ob(x0, y1, z, o), ob(x1, y1, z, o)
    light, mid, dark = mix(colour, "#ffffff", .25), colour, mix(colour, PAL["ink"], .25)
    if along_x:
        ym, i = (y0 + y1) / 2, (y1 - y0) / 2 * .9
        r0, r1 = ob(x0 + i, ym, z + rh, o), ob(x1 - i, ym, z + rh, o)
        a.path(P([bl, br, r1, r0], True), fill=light)
        a.path(P([fl, bl, r0], True), fill=light)
        a.path(P([fl, fr, r1, r0], True), fill=mid)
        a.path(P([fr, br, r1], True), fill=dark)
        a.path(P([fl, fr, br]) + P([fl, r0, r1, fr]) + P([r1, br]), stroke=PAL["ink"], sw=.45, so=.85, fill="none", lj="round")
    else:
        xm, i = (x0 + x1) / 2, (x1 - x0) / 2 * .9
        rf, rb = ob(xm, y0 + i, z + rh, o), ob(xm, y1 - i, z + rh, o)
        a.path(P([fl, bl, rb, rf], True), fill=light)
        a.path(P([bl, br, rb], True), fill=light)
        a.path(P([fl, fr, rf], True), fill=mid)
        a.path(P([fr, br, rb, rf], True), fill=dark)
        a.path(P([fl, fr, br]) + P([fl, rf, fr]) + P([rf, rb, br]), stroke=PAL["ink"], sw=.45, so=.85, fill="none", lj="round")


def _county_seats_arch(a: Art, p, w, h, horiz=True):
    """A dark gate passage on a wall face: p is the foot centre on screen; the face is either a
    front-facing face (horizontal base) or a right-facing face (base sheared along the depth axis)."""
    if horiz:
        a.path(f"M{n(p[0] - w / 2)} {n(p[1])}v{n(-(h - w / 2))}a{n(w / 2)} {n(w / 2)} 0 0 1 {n(w)} 0v{n(h - w / 2)}Z",
               fill=PAL["slated"])
    else:
        # on a right face, x is fixed and the width runs along y: shear the arch
        hw = w / 2
        DX, DY = ob(0, 1)
        l, r = (p[0] - hw * DX, p[1] - hw * DY), (p[0] + hw * DX, p[1] + hw * DY)
        k = 8
        top = [(p[0] + hw * DX * math.cos(math.pi * (1 - j / k)), p[1] + hw * DY * math.cos(math.pi * (1 - j / k))
                - (h - hw) - hw * math.sin(math.pi * j / k)) for j in range(k + 1)]
        a.path(P([l] + top + [r], True), fill=PAL["slated"])


def county_seats():
    """A walled Chinese county seat (xian) in oblique: the wall and its moat, a gate tower over each
    of the four gates, the cross streets joining the gates, and the vermilion yamen (county office)
    compound north of the crossing beside a few courtyard houses."""
    a = Art("verm", seed=87)
    Wd, D = 172.0, 188.0
    o = (13, 133)
    a.shadow(o[0] + 160, o[1] + 4, 140, 7, op=.18)
    f, t, r = slab(a, o, Wd, D, 5, mix(PAL["sage1"], PAL["sand1"], .3))
    slab_outline(a, f, t, r)     # outline now, so its back edge never cuts across the buildings
    wt, wh = 9.0, 9.0            # wall thickness and height
    g0, g1 = 17.0, 24.0          # moat: inner and outer offset from the wall's outer face; the inner
    #                              offset clears the ground the wall hides on its west side (wh*DX/|DY|)
    x0, x1 = g1 + 3, Wd - g1 - 3  # outer face of the wall, in x and in y
    y0, y1 = g1 + 3, D - g1 - 3
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    tile = PAL["slate2"]
    wallc, wallt, wallr = PAL["rock1"], mix(PAL["rock1"], "#fff", .35), PAL["rock"]
    sand = mix(PAL["sand2"], PAL["soil"], .6)    # streets: dark enough to read at card size

    def quad(qx0, qx1, qy0, qy1, z=0):
        return [ob(qx0, qy0, z, o), ob(qx0, qy1, z, o), ob(qx1, qy1, z, o), ob(qx1, qy0, z, o)]
    # moat ring, then the cross roads running from the board edge through the gates (bridging the moat)
    a.path(P(quad(x0 - g1, x1 + g1, y0 - g1, y1 + g1), True) + P(quad(x0 - g0, x1 + g0, y0 - g0, y1 + g0)[::-1], True),
           fill=PAL["water2"], fr="evenodd", stroke=PAL["water4"], sw=.6)
    s = 4.0
    a.path(P(quad(cx - s, cx + s, 0, D), True) + P(quad(0, Wd, cy - s, cy + s), True), fill=sand)

    def tower_x(wy):             # gate tower on the north or south wall (ridge along x), centred on it
        box3(a, cx - 10, cx + 10, wy + 1, wy + wt - 1, wh, wh + 8, o, a.A, None, a.AD, sw=.4)
        _county_seats_roof(a, cx - 10, cx + 10, wy + 1, wy + wt - 1, wh + 8, 6, o, tile, overhang=2.2)

    def tower_y(wx):             # gate tower on the west or east wall (ridge along y), centred on it
        box3(a, wx + 1, wx + wt - 1, cy - 10, cy + 10, wh, wh + 8, o, a.A, None, a.AD, sw=.4)
        _county_seats_roof(a, wx + 1, wx + wt - 1, cy - 10, cy + 10, wh + 8, 6, o, tile, along_x=False, overhang=2.2)

    # north wall (gate opening on its visible inner face), then the west wall (opening on its inner face)
    box3(a, x0, x1, y1 - wt, y1, 0, wh, o, wallc, wallt, wallr, sw=.45)
    _county_seats_arch(a, ob(cx, y1 - wt, 0, o), 6, 7.5)
    tower_x(y1 - wt)
    box3(a, x0, x0 + wt, y0, y1 - wt, 0, wh, o, wallc, wallt, wallr, sw=.45)
    _county_seats_arch(a, ob(x0 + wt, cy, 0, o), 6, 7.5, horiz=False)
    tower_y(x0)
    # inside, back to front: the yamen north of the crossing (NW quadrant), then a house in each
    # eastern quadrant (the south-west quadrant is hidden behind the south gate tower anyway)
    ix0, iy0, iy1 = x0 + wt, y0 + wt, y1 - wt
    # yamen: a low vermilion compound wall, open to the south on the hall's axis, round its main hall
    yx0, yx1, yy0, yy1 = ix0 + 4, cx - s - 4, cy + s + 5, iy1 - 8
    ym, ww, whh = (yx0 + yx1) / 2, 1.8, 3.5
    box3(a, yx0, yx1, yy1 - ww, yy1, 0, whh, o, a.A, a.AL, a.AD, sw=.35)
    box3(a, yx0, yx0 + ww, yy0, yy1 - ww, 0, whh, o, a.A, a.AL, a.AD, sw=.35)
    box3(a, ym - 11, ym + 11, yy0 + 14, yy0 + 26, 0, 9, o, a.A, None, a.AD, sw=.4)
    _county_seats_roof(a, ym - 11, ym + 11, yy0 + 14, yy0 + 26, 9, 7, o, tile, overhang=2)
    box3(a, yx1 - ww, yx1, yy0, yy1 - ww, 0, whh, o, a.A, a.AL, a.AD, sw=.35)
    box3(a, yx0, ym - 4, yy0, yy0 + ww, 0, whh, o, a.A, a.AL, a.AD, sw=.35)
    box3(a, ym + 4, yx1, yy0, yy0 + ww, 0, whh, o, a.A, a.AL, a.AD, sw=.35)

    def house(hx, hy):
        box3(a, hx, hx + 24, hy, hy + 13, 0, 6, o, PAL["paper2"], None, mix(PAL["paper2"], PAL["ink"], .18), sw=.4)
        _county_seats_roof(a, hx, hx + 24, hy, hy + 13, 6, 5, o, tile, overhang=1.4)
    house(cx + s + 5, cy + s + 6)
    house(cx + s + 12, iy0 + 26)
    # east wall, then the south wall, each with the gate opening on its outer face and its tower
    box3(a, x1 - wt, x1, y0 + wt, y1 - wt, 0, wh, o, wallc, wallt, wallr, sw=.45)
    _county_seats_arch(a, ob(x1, cy, 0, o), 6, 7.5, horiz=False)
    tower_y(x1 - wt)
    box3(a, x0, x1, y0, y0 + wt, 0, wh, o, wallc, wallt, wallr, sw=.45)
    _county_seats_arch(a, ob(cx, y0, 0, o), 6, 7.5)
    tower_x(y0)
    return a.render()


def _temples_roof(a: Art, x0, x1, y0, y1, z, rh, o, colour, inset=None, overhang=1.5):
    """kit.hiproof without the tile-course hairlines, which vanish at card size."""
    x0, x1, y0, y1 = x0 - overhang, x1 + overhang, y0 - overhang, y1 + overhang
    ym = (y0 + y1) / 2
    i = inset if inset is not None else (y1 - y0) / 2 * .9
    r0, r1 = ob(x0 + i, ym, z + rh, o), ob(x1 - i, ym, z + rh, o)
    fl, fr, bl, br = ob(x0, y0, z, o), ob(x1, y0, z, o), ob(x0, y1, z, o), ob(x1, y1, z, o)
    light, dark = mix(colour, "#ffffff", .25), mix(colour, PAL["ink"], .25)
    a.path(P([bl, br, r1, r0], True) + P([fl, bl, r0], True), fill=light)
    a.path(P([fl, fr, r1, r0], True), fill=colour)
    a.path(P([fr, br, r1], True), fill=dark)
    a.path(P([fl, fr, br]) + P([fl, r0, r1, fr]) + P([r1, br]), stroke=PAL["ink"], sw=.4, so=.8, fill="none", lj="round")


def _temples_storeys(a: Art, cx, cy, sizes, heights, rises, o, body, side, tile, ov):
    """Square storeys stacked on pyramidal roofs; each storey starts exactly where its faces meet the
    roof below (front slope and hip both rise 1:1 in plan, so the footing is level). Returns the top z."""
    z = 0.0
    for k, (s, hh, rh) in enumerate(zip(sizes, heights, rises)):
        box3(a, cx - s / 2, cx + s / 2, cy - s / 2, cy + s / 2, z, z + hh, o, body, None, side, sw=.4)
        z += hh
        half = s / 2 + ov
        _temples_roof(a, cx - s / 2, cx + s / 2, cy - s / 2, cy + s / 2, z, rh, o, tile, inset=half, overhang=ov)
        if k + 1 < len(sizes):
            z += rh * (half - sizes[k + 1] / 2) / half
        else:
            z += rh
    return z


def _temples_hall(a: Art, cx, cy, hw, hd, h, o, wall, walld, tile, upper=False):
    """A timber hall on a stone terrace: red walls, a row of columns, a hipped roof; with upper=True a
    second, smaller storey rises out of the lower roof (the double eave of a main hall)."""
    tz = 2.5
    box3(a, cx - hw - 3, cx + hw + 3, cy - hd - 3, cy + hd + 3, 0, tz, o, PAL["paper2"], "#faf6ee", PAL["rock1"], sw=.4)
    x0, x1, y0, y1 = cx - hw, cx + hw, cy - hd, cy + hd
    box3(a, x0, x1, y0, y1, tz, tz + h, o, wall, None, walld, sw=.4)
    cols = "".join(P([ob(x0 + k * (x1 - x0) / 6, y0, tz, o), ob(x0 + k * (x1 - x0) / 6, y0, tz + h, o)]) for k in range(1, 6))
    a.path(cols, stroke=walld, sw=1.8, so=.9)
    ov = 4.0
    half = hd + ov
    if not upper:
        _temples_roof(a, x0, x1, y0, y1, tz + h, 9, o, tile, overhang=ov)
        return
    rh1 = 7.0
    _temples_roof(a, x0, x1, y0, y1, tz + h, rh1, o, tile, inset=half, overhang=ov)
    d = 6.0   # upper storey set in by d on the front and right, so its faces meet both slopes at one level
    z0 = tz + h + rh1 * (d + ov) / half
    box3(a, x0 + d, x1 - d, y0 + d, y1 - d, z0, z0 + 12, o, wall, None, walld, sw=.4)
    _temples_roof(a, x0 + d, x1 - d, y0 + d, y1 - d, z0 + 12, 11, o, tile, overhang=ov)


def temples():
    """A Chinese Buddhist temple laid out on its north-south axis: the mountain gate in the front wall,
    a paved walk across the open court to the main hall on its terrace, and a pagoda behind on the
    same axis, inside a vermilion-walled precinct."""
    a = Art("verm", seed=89)
    o = (30, 143)
    Wd, D = 180.0, 125.0
    a.shadow(o[0] + 140, o[1] + 5, 118, 6, op=.16)
    f, t, r = slab(a, o, Wd, D, 5, mix(PAL["sage1"], PAL["sand1"], .25))
    slab_outline(a, f, t, r)   # drawn first, so everything standing on the block hides its back edge
    tile = PAL["slate2"]
    wall, walld = a.A, a.AD
    cope = mix(PAL["slate1"], "#fff", .2)
    x0, x1, y0, y1 = 28.0, 152.0, 12.0, 116.0
    xm = (x0 + x1) / 2
    hy, hd = 66.0, 10.0
    a.path(P([ob(x0, y0, 0, o), ob(x0, y1, 0, o), ob(x1, y1, 0, o), ob(x1, y0, 0, o)], True), fill="#ece5d6")
    # the paved axial walk from the gate across the court to the hall terrace
    a.path(P([ob(xm - 6, y0, 0, o), ob(xm - 6, hy - hd - 3, 0, o), ob(xm + 6, hy - hd - 3, 0, o),
              ob(xm + 6, y0, 0, o)], True), fill=mix(PAL["sand2"], PAL["clay"], .3), stroke="#faf5ea", sw=1)
    # back and left walls
    box3(a, x0, x1, y1 - 2.5, y1, 0, 8, o, wall, cope, walld, sw=.4)
    box3(a, x0, x0 + 2.5, y0, y1, 0, 8, o, wall, cope, walld, sw=.4)
    # pagoda behind the hall on the same axis, then the main hall
    py = 102
    ztop = _temples_storeys(a, xm, py, [17, 14.5, 12.5, 10.5, 8.5], [18, 9.5, 8.5, 8, 7.5], [5, 4.5, 4, 3.5, 5.5], o,
                            "#efe8da", mix("#efe8da", PAL["ink"], .18), tile, 3.5)
    tip = ob(xm, py, ztop, o)
    a.path(f"M{n(tip[0])} {n(tip[1] + .5)}v-11", stroke=PAL["ink"], sw=1.3)
    a.circle((tip[0], tip[1] - 4.5), 1.7, fill=PAL["ink"])
    _temples_hall(a, xm, hy, 30, hd, 15, o, wall, walld, tile)
    # right and front walls, the mountain gate in the front wall
    box3(a, x1 - 2.5, x1, y0 + 2.5, y1, 0, 8, o, wall, cope, walld, sw=.4)
    box3(a, x0, xm - 12, y0, y0 + 2.5, 0, 8, o, wall, cope, walld, sw=.4)
    box3(a, xm + 12, x1, y0, y0 + 2.5, 0, 8, o, wall, cope, walld, sw=.4)
    box3(a, xm - 12, xm + 12, y0 - 2.5, y0 + 5, 0, 12, o, wall, None, walld, sw=.4)
    g = ob(xm, y0 - 2.5, 0, o)
    a.path(f"M{n(g[0] - 5)} {n(g[1])}v-5a5 5 0 0 1 10 0v5Z", fill=PAL["slated"])
    _temples_roof(a, xm - 12, xm + 12, y0 - 2.5, y0 + 5, 12, 5, o, tile, overhang=3)
    # a few trees outside the precinct, back to front
    for (x, y) in [(167, 96), (168, 46), (13, 8)]:
        b = ob(x, y, 0, o)
        a.path(f"M{n(b[0])} {n(b[1])}v-8", stroke=PAL["umber"], sw=1.4)
        a.path(broadleaf(b[0], b[1], 20, seed=int(x + y)), fill=PAL["moss"], stroke=PAL["forest"], sw=.6)
    return a.render()


def _castles_ring(cx, cy, ax, ay, k=64, p=4.0):
    """Points (x, y) on a rounded-rectangle superellipse |dx/ax|^p + |dy/ay|^p = 1."""
    pts = []
    for i in range(k):
        t = 2 * math.pi * i / k
        c, s = math.cos(t), math.sin(t)
        pts.append((cx + ax * math.copysign(abs(c) ** (2 / p), c), cy + ay * math.copysign(abs(s) ** (2 / p), s)))
    return pts


def _castles_merlons(a, x0, x1, y0, y1, z, along, o, cols, w=3.0, hgt=2.6, every=14.0):
    """Battlements as small oblique blocks standing on a wall top x0..x1, y0..y1 at height z,
    spaced along the wall's long axis ('x' or 'y'), centred so none overhangs the wall's ends.
    The spacing (14) is wide enough that neighbouring blocks never overlap on screen."""
    lo, hi = (x0, x1) if along == "x" else (y0, y1)
    L = hi - lo
    if L < w:
        return
    k = int((L - w) / every) + 1
    s0 = lo + (L - (k - 1) * every - w) / 2
    starts = [s0 + i * every for i in range(k)]
    _front, top, right = cols
    dark = lit = ""
    for s in starts:
        bx = (s, s + w, y0, y1) if along == "x" else (x0, x1, s, s + w)
        f = [ob(bx[0], bx[2], z + hgt, o), ob(bx[1], bx[2], z + hgt, o), ob(bx[1], bx[2], z, o), ob(bx[0], bx[2], z, o)]
        t1, t2, r2 = ob(bx[0], bx[3], z + hgt, o), ob(bx[1], bx[3], z + hgt, o), ob(bx[1], bx[3], z, o)
        dark += P([f[0], t1, t2, r2, f[2], f[3]], True)       # whole silhouette: shows as the shaded right face
        lit += P([f[3], f[0], t1, t2, f[1], f[2]], True) + P([f[0], f[1]])   # lit front and top over it
    # the spacing keeps neighbouring blocks from overlapping on screen, so one path per tone suffices
    a.path(dark, fill=right, stroke=PAL["ink"], sw=.3, so=.75, lj="round")
    a.path(lit, fill=top, stroke=PAL["ink"], sw=.3, so=.75, lj="round")


def _castles_lwall(a, x0, x1, y0, y1, t, z0, z1, o, cols):
    """The front and right walls of a rectangular curtain as one L-shaped solid of thickness t,
    so the two meet at a single clean corner edge."""
    front, top, right = cols
    f = [ob(x0, y0, z1, o), ob(x1, y0, z1, o), ob(x1, y0, z0, o), ob(x0, y0, z0, o)]
    r = [ob(x1, y0, z1, o), ob(x1, y1, z1, o), ob(x1, y1, z0, o), ob(x1, y0, z0, o)]
    tp = [ob(x, y, z1, o) for x, y in ((x0, y0), (x1, y0), (x1, y1), (x1 - t, y1), (x1 - t, y0 + t), (x0, y0 + t))]
    a.path(P(f, True), fill=front)
    a.path(P(r, True), fill=right)
    a.path(P(tp, True), fill=top)
    a.path(P(f, True) + P([r[0], r[1], r[2], r[3]]) + P(tp[2:] + [tp[0]]), stroke=PAL["ink"], sw=.35, so=.75,
           fill="none", lj="round")


def castles():
    """A concentric castle on level ground: a water-filled moat crossed by a bridge, a low outer
    curtain wall with round corner towers and a twin-towered gatehouse, a taller inner curtain,
    and the square keep rising highest at the centre, so each line of defence overlooks the one
    in front of it."""
    a = Art("verm", seed=91)
    o = (14, 146)
    Wd, D = 206.0, 132.0
    cx, cy = 103.0, 70.0
    g0 = 6.0                                     # level ground inside and around the moat
    mx, my = 88.0, 56.0                          # outer edge of the moat

    def h(x, y):
        return g0                                # the castle stands on level ground

    a.shadow(o[0] + 160, o[1] + 8, 150, 7, op=.2)
    terrain(a, o, Wd, D, h, nx=1, ny=1, colours={"land": PAL["sage2"]}, base=18,
            strata=[(3, PAL["umber"]), (7, PAL["clay"]), (99, PAL["rock"])])
    # moat: a level water ring that clears the curtain wall and its corner towers all round
    outer = [ob(x, y, g0 + .3, o) for x, y in _castles_ring(cx, cy, mx, my)]
    inner = [ob(x, y, g0 + .3, o) for x, y in _castles_ring(cx, cy, mx - 10, my - 9)]
    a.path(P(outer, True) + P(inner[::-1], True), fill=PAL["water2"], fr="evenodd", stroke=PAL["water4"], sw=.5)
    stone, stone_t, stone_d = "#ece6da", "#f7f3ea", PAL["rock1"]

    def wall(x0, x1, y0, y1, z0, z1):
        box3(a, x0, x1, y0, y1, z0, z1, o, stone, stone_t, stone_d)

    def mer(x0, x1, y0, y1, z, along):
        _castles_merlons(a, x0, x1, y0, y1, z, along, o, (stone, stone_t, stone_d))

    # outer curtain (low): x0..x1, y0..y1, round towers at the four corners
    ox0, ox1, oy0, oy1 = cx - 62, cx + 62, cy - 36, cy + 36
    wt, wh, tr = 3.5, 15.0, 7.0
    th = wh + 8
    oz = g0
    yb = math.sqrt(tr * tr - (wt / 2) ** 2)      # where a side wall meets a tower's face
    ys0, ys1 = oy0 + wt / 2 + tr + .5, oy1 - wt / 2 - tr - .5    # side-wall stretch clear of both towers
    # back wall, its corner towers, then the left wall running forward from them
    wall(ox0, ox1, oy1 - wt, oy1, oz, oz + wh)
    mer(ox0 + tr, ox1 - tr, oy1 - wt, oy1, oz + wh, "x")
    cylinder(a, ox0, oy1 - wt / 2, tr, oz, oz + th, o, stone)
    wall(ox0, ox0 + wt, oy0 + yb, oy1 - wt / 2 - yb, oz, oz + wh)
    mer(ox0, ox0 + wt, ys0, ys1, oz + wh, "y")
    # inner curtain (taller): back and left walls, the keep, then the front and right walls
    ix0, ix1, iy0, iy1 = cx - 33, cx + 33, cy - 20, cy + 20
    iz, ih = g0, 27.0
    wall(ix0, ix1, iy1 - wt, iy1, iz, iz + ih)
    mer(ix0 + wt + 2, ix1 - wt - 2, iy1 - wt, iy1, iz + ih, "x")
    wall(ix0, ix0 + wt, iy0, iy1, iz, iz + ih)
    mer(ix0, ix0 + wt, iy0 + wt + 3, iy1 - wt - 3, iz + ih, "y")
    # the keep, highest of all, with arrow slits and the single accent: its pennant
    kx0, kx1, ky0, ky1 = cx - 13, cx + 13, cy - 10, cy + 10
    kz = iz + 54
    wall(kx0, kx1, ky0, ky1, iz, kz)
    for sx in (kx0 + 7.5, kx1 - 7.5):
        p = ob(sx, ky0, kz - 12, o)
        a.path(f"M{n(p[0])} {n(p[1])}v6", stroke=PAL["ink"], sw=1.6)
    # keep parapet: a merlon at each corner and mid-side, back row first; the pennant pole stands mid-roof
    km, kxm, kym = 3.0, cx - 1.5, cy - 1.5
    for x, y in ((kx0, ky1 - km), (kxm, ky1 - km), (kx1 - km, ky1 - km), (kx0, kym), (kx1 - km, kym)):
        mer(x, x + km, y, y + km, kz, "x")
    fp = ob(cx, cy, kz, o)
    a.path(f"M{n(fp[0])} {n(fp[1])}v-16", stroke=PAL["ink"], sw=.9)
    a.path(f"M{n(fp[0])} {n(fp[1] - 16)}l11 3-11 3Z", fill=a.A)
    for x in (kx0, kxm, kx1 - km):
        mer(x, x + km, ky0, ky0 + km, kz, "x")
    _castles_lwall(a, ix0, ix1, iy0, iy1, wt, iz, iz + ih, o, (stone, stone_t, stone_d))
    mer(ix1 - wt, ix1, iy0 + wt + 3, iy1 - wt - 3, iz + ih, "y")
    mer(ix0, ix1, iy0, iy0 + wt, iz + ih, "x")
    # outer right wall between its corner towers (back tower first, wall in front of it)
    cylinder(a, ox1, oy1 - wt / 2, tr, oz, oz + th, o, stone)
    wall(ox1 - wt, ox1, oy0 + yb, oy1 - wt / 2 - yb, oz, oz + wh)
    mer(ox1 - wt, ox1, ys0, ys1, oz + wh, "y")
    # bridge across the moat to the gate, lying on the ground
    b0, b1 = ob(cx, cy - my - 1.5, g0 + .4, o), ob(cx, oy0, g0 + .4, o)
    a.path(P([(b0[0] - 4, b0[1]), (b1[0] - 4, b1[1]), (b1[0] + 4, b1[1]), (b0[0] + 4, b0[1])], True),
           fill=PAL["clay"], stroke=PAL["ink"], sw=.4)
    # front wall with the gatehouse: a raised gate block between two drum towers
    gx = 13.0
    wall(ox0, cx - gx, oy0, oy0 + wt, oz, oz + wh)
    wall(cx + gx, ox1, oy0, oy0 + wt, oz, oz + wh)
    wall(cx - gx, cx + gx, oy0, oy0 + wt, oz, oz + wh + 6)
    mer(ox0 + tr, cx - gx - tr, oy0, oy0 + wt, oz + wh, "x")
    mer(cx + gx + tr, ox1 - tr, oy0, oy0 + wt, oz + wh, "x")
    g = ob(cx, oy0, oz, o)
    a.path(f"M{n(g[0] - 5)} {n(g[1])}v-7a5 5 0 0 1 10 0v7Z", fill=PAL["slated"])
    for x in (cx - gx, cx + gx):
        cylinder(a, x, oy0 + wt / 2, tr, oz, oz + wh + 11, o, stone)
    for x in (ox0, ox1):
        cylinder(a, x, oy0 + wt / 2, tr, oz, oz + th, o, stone)
    return a.render()
